use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, watch};

use looprs_desktop_baml_client::types::{UiNode, Union2KlabelOrKrect};
use looprs_desktop_baml_client::{B, init as baml_init};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GenerativeUiCommand {
    SetIntervalSecs { secs: u64 },
    PatchState { patch: Value },
    SetState { state: Value },
    SetGoal { goal: String },
    Regenerate,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GenerativeUiUpdate {
    pub status: String,
    pub seq: u64,
    pub interval_secs: u64,
    pub state: Value,
    pub state_pretty: String,
    pub goal: String,
    pub component_tree_json: Value,
    pub component_tree_pretty: String,
    pub component_code: String,
    pub component_code_preview: String,
    pub component_code_truncated: bool,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LiveGenerativeUiHandle {
    pub updates: watch::Receiver<Arc<GenerativeUiUpdate>>,
    command_tx: mpsc::Sender<GenerativeUiCommand>,
    shutdown_tx: watch::Sender<bool>,
}

impl LiveGenerativeUiHandle {
    pub fn stop(&self) {
        let _ = self.shutdown_tx.send(true);
    }

    pub fn send(&self, command: GenerativeUiCommand) {
        let _ = self.command_tx.try_send(command);
    }
}

pub fn start_live_generative_ui(interval: Duration) -> LiveGenerativeUiHandle {
    let initial_state = serde_json::json!({
        "theme": "dark",
        "title": "Live Generative UI",
        "accent": "rgb(60, 90, 132)",
        "layout": "split",
    });

    let initial_goal =
        "Generate a Freya 0.4 builder-style UI component that reflects the current state. Output only Rust code.".to_string();

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let (command_tx, command_rx) = mpsc::channel::<GenerativeUiCommand>(64);

    let initial_update = GenerativeUiUpdate {
        status: "Starting...".to_string(),
        seq: 0,
        interval_secs: interval.as_secs().max(1),
        state: initial_state.clone(),
        state_pretty: pretty_json(&initial_state),
        goal: initial_goal.clone(),
        component_tree_json: Value::Null,
        component_tree_pretty: "null".to_string(),
        component_code: String::new(),
        component_code_preview: String::new(),
        component_code_truncated: false,
        last_error: None,
    };
    let (update_tx, update_rx) = watch::channel(Arc::new(initial_update));

    tokio::spawn(generator_loop(
        initial_state,
        initial_goal,
        interval,
        command_rx,
        update_tx,
        shutdown_rx,
    ));

    LiveGenerativeUiHandle {
        updates: update_rx,
        command_tx,
        shutdown_tx,
    }
}

async fn generator_loop(
    initial_state: Value,
    initial_goal: String,
    initial_interval: Duration,
    mut command_rx: mpsc::Receiver<GenerativeUiCommand>,
    update_tx: watch::Sender<Arc<GenerativeUiUpdate>>,
    mut shutdown_rx: watch::Receiver<bool>,
) {
    let mut seq: u64 = 0;
    let mut state = initial_state;
    let mut goal = initial_goal;
    let mut interval = initial_interval;
    let mut tick = tokio::time::interval(interval);

    if std::env::var("OPENAI_API_KEY")
        .ok()
        .is_none_or(|v| v.trim().is_empty())
    {
        let mut update = (*update_tx.borrow()).as_ref().clone();
        update.status = "Missing OPENAI_API_KEY".to_string();
        update.last_error = Some("Set OPENAI_API_KEY to enable BAML generation".to_string());
        let _ = update_tx.send(Arc::new(update));
        return;
    }

    baml_init();
    {
        let mut update = (*update_tx.borrow()).as_ref().clone();
        update.status = "BAML ready: openai/gpt-4o-mini".to_string();
        update.last_error = None;
        let _ = update_tx.send(Arc::new(update));
    }

    loop {
        if *shutdown_rx.borrow() {
            return;
        }

        tokio::select! {
            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() {
                    return;
                }
            }
            Some(cmd) = command_rx.recv() => {
                match cmd {
                    GenerativeUiCommand::SetIntervalSecs { secs } => {
                        let secs = secs.max(1);
                        interval = Duration::from_secs(secs);
                        tick = tokio::time::interval(interval);

                        let mut update = (*update_tx.borrow()).as_ref().clone();
                        update.interval_secs = secs;
                        update.status = format!("Interval set to {secs}s");
                        let _ = update_tx.send(Arc::new(update));
                    }
                    GenerativeUiCommand::SetState { state: new_state } => {
                        state = new_state;

                        let mut update = (*update_tx.borrow()).as_ref().clone();
                        update.state = state.clone();
                        update.state_pretty = pretty_json(&state);
                        update.status = "State set".to_string();
                        let _ = update_tx.send(Arc::new(update));
                    }
                    GenerativeUiCommand::PatchState { patch } => {
                        merge_json_in_place(&mut state, patch);

                        let mut update = (*update_tx.borrow()).as_ref().clone();
                        update.state = state.clone();
                        update.state_pretty = pretty_json(&state);
                        update.status = "State patched".to_string();
                        let _ = update_tx.send(Arc::new(update));
                    }
                    GenerativeUiCommand::SetGoal { goal: new_goal } => {
                        goal = new_goal;

                        let mut update = (*update_tx.borrow()).as_ref().clone();
                        update.goal = goal.clone();
                        update.status = "Goal set".to_string();
                        let _ = update_tx.send(Arc::new(update));
                    }
                    GenerativeUiCommand::Regenerate => tick.reset(),
                }
            }
            _ = tick.tick() => {
                seq = seq.saturating_add(1);

                let state_json = serde_json::to_string_pretty(&state)
                    .unwrap_or_else(|_| "{}".to_string());
                let result = B
                    .GenerateUiTree
                    .call(goal.clone(), state_json)
                    .await;

                match result {
                    Ok(tree) => {
                        let component_tree_json = ui_node_to_json(&tree);
                        let component_code = render_component_code(&tree);
                        let component_tree_pretty = pretty_json(&component_tree_json);
                        let (component_code_preview, component_code_truncated) =
                            truncate_string(&component_code, 8_000);
                        let update = GenerativeUiUpdate {
                            status: "Updated".to_string(),
                            seq,
                            interval_secs: interval.as_secs().max(1),
                            state: state.clone(),
                            state_pretty: pretty_json(&state),
                            goal: goal.clone(),
                            component_tree_json,
                            component_tree_pretty,
                            component_code,
                            component_code_preview,
                            component_code_truncated,
                            last_error: None,
                        };
                        let _ = update_tx.send(Arc::new(update));
                    }
                    Err(e) => {
                        let mut update = (*update_tx.borrow()).as_ref().clone();
                        update.seq = seq;
                        update.status = "Generation failed".to_string();
                        update.last_error = Some(e.to_string());
                        let _ = update_tx.send(Arc::new(update));
                    }
                }
            }
        }
    }
}

fn pretty_json(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".to_string())
}

pub fn truncate_string(input: &str, max_chars: usize) -> (String, bool) {
    if max_chars == 0 {
        return (String::new(), !input.is_empty());
    }

    let mut count = 0usize;
    for (index, _) in input.char_indices() {
        if count == max_chars {
            return (input[..index].to_string(), true);
        }
        count = count.saturating_add(1);
    }
    (input.to_string(), false)
}

fn ui_node_to_json(node: &UiNode) -> Value {
    let tag = match node.tag {
        Union2KlabelOrKrect::Krect => "rect",
        Union2KlabelOrKrect::Klabel => "label",
    };

    let mut attrs = node
        .attrs
        .iter()
        .map(|(k, v)| (k.clone(), Value::String(v.clone())))
        .collect::<serde_json::Map<_, _>>();

    let children = node
        .children
        .iter()
        .map(|c| ui_node_to_json(c.as_ref()))
        .collect::<Vec<_>>();

    attrs.insert("tag".to_string(), Value::String(tag.to_string()));
    attrs.insert(
        "text".to_string(),
        node.text
            .as_ref()
            .map(|s| Value::String(s.clone()))
            .unwrap_or(Value::Null),
    );
    attrs.insert("children".to_string(), Value::Array(children));

    Value::Object(attrs)
}

fn render_component_code(root: &UiNode) -> String {
    let mut out = String::new();
    out.push_str("use freya::prelude::*;\n\n");

    out.push_str("pub fn generated_component() -> impl IntoElement {\n");
    render_node_expr(root, 1, &mut out);
    out.push_str("}\n");
    out
}

fn render_node_expr(node: &UiNode, indent: usize, out: &mut String) {
    let pad = "    ".repeat(indent);
    let chain_pad = "    ".repeat(indent + 1);

    match node.tag {
        Union2KlabelOrKrect::Klabel => {
            let text = node.text.as_deref().unwrap_or("");
            out.push_str(&format!(
                "{pad}label().text(\"{}\")",
                escape_rsx_string(text)
            ));
        }
        Union2KlabelOrKrect::Krect => {
            out.push_str(&format!("{pad}rect()"));
        }
    }

    let mut keys = node.attrs.keys().collect::<Vec<_>>();
    keys.sort();
    for k in keys {
        if let Some(v) = node.attrs.get(k)
            && let Some(method) = render_attr_method(k.as_str(), v.as_str())
        {
            out.push_str(&format!("\n{chain_pad}{method}"));
        }
    }

    for child in &node.children {
        out.push_str(&format!("\n{chain_pad}.child(\n"));
        render_node_expr(child.as_ref(), indent + 2, out);
        out.push_str(&format!("\n{chain_pad})"));
    }

    out.push('\n');
}

fn render_attr_method(key: &str, value: &str) -> Option<String> {
    match key {
        "direction" => match value {
            "vertical" => Some(".vertical()".to_string()),
            "horizontal" => Some(".horizontal()".to_string()),
            _ => None,
        },
        "spacing" => value
            .trim()
            .parse::<f32>()
            .ok()
            .map(|n| format!(".spacing({n})")),
        "padding" => value
            .trim()
            .parse::<f32>()
            .ok()
            .map(|n| format!(".padding(Gaps::new_all({n}))")),
        "width" => parse_size(value).map(|s| format!(".width({s})")),
        "height" => parse_size(value).map(|s| format!(".height({s})")),
        "background" => parse_rgb(value).map(|(r, g, b)| format!(".background(({r}, {g}, {b}))")),
        "corner_radius" => value
            .trim()
            .parse::<f32>()
            .ok()
            .map(|n| format!(".corner_radius({n})")),
        _ => None,
    }
}

pub fn parse_size(raw: &str) -> Option<String> {
    let v = raw.trim();
    if v == "fill" || v == "100%" {
        return Some("Size::fill()".to_string());
    }
    if let Some(stripped) = v.strip_suffix('%') {
        return stripped
            .trim()
            .parse::<f32>()
            .ok()
            .map(|n| format!("Size::percent({n})"));
    }
    v.parse::<f32>().ok().map(|n| format!("Size::px({n})"))
}

pub fn parse_rgb(raw: &str) -> Option<(u8, u8, u8)> {
    let v = raw.trim();
    let v = v.strip_prefix("rgb(")?.strip_suffix(')')?;
    let mut parts = v.split(',').map(|p| p.trim().parse::<u8>().ok());
    let r = parts.next()??;
    let g = parts.next()??;
    let b = parts.next()??;
    Some((r, g, b))
}

pub fn escape_rsx_string(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "")
}

pub fn merge_json_in_place(target: &mut Value, patch: Value) {
    match (target, patch) {
        (Value::Object(target_obj), Value::Object(patch_obj)) => {
            for (k, v) in patch_obj {
                match target_obj.get_mut(&k) {
                    Some(existing) => merge_json_in_place(existing, v),
                    None => {
                        target_obj.insert(k, v);
                    }
                }
            }
        }
        (target_slot, patch_value) => {
            *target_slot = patch_value;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_json_patch_merges_objects() {
        let mut base = serde_json::json!({"a": 1, "b": {"c": 2}});
        merge_json_in_place(&mut base, serde_json::json!({"b": {"d": 3}, "e": 4}));
        assert_eq!(base["a"], 1);
        assert_eq!(base["b"]["c"], 2);
        assert_eq!(base["b"]["d"], 3);
        assert_eq!(base["e"], 4);
    }

    fn live_tests_enabled() -> bool {
        std::env::var("LOOPRS_RUN_LIVE_LLM_TESTS")
            .ok()
            .is_some_and(|v| v == "1" || v == "true")
    }

    #[test]
    fn test_truncate_string_exact_boundary() {
        let input = "x".repeat(100);
        let (result, truncated) = truncate_string(&input, 100);

        assert_eq!(result.len(), 100);
        assert_eq!(truncated, false);
    }

    #[test]
    fn test_truncate_string_multibyte_chars() {
        let input = "🦀".repeat(10); // Each emoji is 4 bytes
        let (result, truncated) = truncate_string(&input, 5);

        assert_eq!(result, "🦀🦀🦀🦀🦀");
        assert_eq!(truncated, true);
    }

    #[test]
    fn test_truncate_string_zero_max() {
        let input = "test";
        let (result, truncated) = truncate_string(&input, 0);

        assert_eq!(result, "");
        assert_eq!(truncated, true);
    }

    #[test]
    fn test_merge_json_patch_deep_nesting() {
        let mut base = serde_json::json!({"a": {"b": {"c": 1}}});
        merge_json_in_place(&mut base, serde_json::json!({"a": {"b": {"d": 2}}}));

        assert_eq!(base["a"]["b"]["c"], 1);
        assert_eq!(base["a"]["b"]["d"], 2);
    }

    #[test]
    fn test_render_component_code_snapshot() {
        let tree = UiNode {
            tag: Union2KlabelOrKrect::Krect,
            text: None,
            attrs: [("direction".to_string(), "vertical".to_string())].into(),
            children: vec![Arc::new(UiNode {
                tag: Union2KlabelOrKrect::Klabel,
                text: Some("Hello".to_string()),
                attrs: Default::default(),
                children: vec![],
            })],
        };

        let code = render_component_code(&tree);

        insta::assert_snapshot!(code);
    }

    #[test]
    fn test_parse_rgb_valid_formats() {
        assert_eq!(parse_rgb("rgb(255, 128, 0)"), Some((255, 128, 0)));
        assert_eq!(parse_rgb("rgb(0,0,0)"), Some((0, 0, 0)));
    }

    #[test]
    fn test_parse_rgb_invalid_formats() {
        assert_eq!(parse_rgb("rgb(256, 0, 0)"), None); // Out of range
        assert_eq!(parse_rgb("#FF8000"), None); // Wrong format
        assert_eq!(parse_rgb("invalid"), None);
    }

    #[test]
    fn test_parse_size_variants() {
        assert_eq!(parse_size("fill"), Some("Size::fill()".to_string()));
        assert_eq!(parse_size("100%"), Some("Size::fill()".to_string()));
        assert_eq!(parse_size("50%"), Some("Size::percent(50.0)".to_string()));
        assert_eq!(parse_size("200"), Some("Size::px(200.0)".to_string()));
    }

    #[tokio::test]
    async fn test_generative_ui_handle_stop() {
        if !live_tests_enabled() {
            return;
        }

        let handle = start_live_generative_ui(Duration::from_secs(60));

        // Wait for initialization
        tokio::time::sleep(Duration::from_millis(100)).await;

        handle.stop();

        // Should gracefully shut down
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    #[tokio::test]
    #[ignore]
    async fn baml_generate_ui_tree_smoke() {
        if !live_tests_enabled() {
            return;
        }
        if std::env::var("OPENAI_API_KEY")
            .ok()
            .is_none_or(|v| v.trim().is_empty())
        {
            return;
        }

        baml_init();
        let tree = B
            .GenerateUiTree
            .call(
                "Generate a tiny UI".to_string(),
                "{\"title\":\"Hello\"}".to_string(),
            )
            .await
            .expect("BAML call failed");

        let json = ui_node_to_json(&tree);
        assert_eq!(json["tag"], "rect");
    }
}
