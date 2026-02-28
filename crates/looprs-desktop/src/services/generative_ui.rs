use serde::{Deserialize, Serialize};
use serde_json::Value;
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
    pub goal: String,
    pub component_tree_json: Value,
    pub component_code: String,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LiveGenerativeUiHandle {
    pub updates: watch::Receiver<GenerativeUiUpdate>,
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

    let initial_goal = "Generate a Freya (rsx!) UI component that reflects the current state. Output only Rust code.".to_string();

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let (command_tx, command_rx) = mpsc::channel::<GenerativeUiCommand>(64);

    let initial_update = GenerativeUiUpdate {
        status: "Starting...".to_string(),
        seq: 0,
        interval_secs: interval.as_secs().max(1),
        state: initial_state.clone(),
        goal: initial_goal.clone(),
        component_tree_json: Value::Null,
        component_code: String::new(),
        last_error: None,
    };
    let (update_tx, update_rx) = watch::channel(initial_update.clone());

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
    update_tx: watch::Sender<GenerativeUiUpdate>,
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
        let mut update = (*update_tx.borrow()).clone();
        update.status = "Missing OPENAI_API_KEY".to_string();
        update.last_error = Some("Set OPENAI_API_KEY to enable BAML generation".to_string());
        let _ = update_tx.send(update);
        return;
    }

    baml_init();
    {
        let mut update = (*update_tx.borrow()).clone();
        update.status = "BAML ready: openai/gpt-4o-mini".to_string();
        update.last_error = None;
        let _ = update_tx.send(update);
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

                        let mut update = (*update_tx.borrow()).clone();
                        update.interval_secs = secs;
                        update.status = format!("Interval set to {secs}s");
                        let _ = update_tx.send(update.clone());
                    }
                    GenerativeUiCommand::SetState { state: new_state } => {
                        state = new_state;

                        let mut update = (*update_tx.borrow()).clone();
                        update.state = state.clone();
                        update.status = "State set".to_string();
                        let _ = update_tx.send(update);
                    }
                    GenerativeUiCommand::PatchState { patch } => {
                        merge_json_in_place(&mut state, patch);

                        let mut update = (*update_tx.borrow()).clone();
                        update.state = state.clone();
                        update.status = "State patched".to_string();
                        let _ = update_tx.send(update);
                    }
                    GenerativeUiCommand::SetGoal { goal: new_goal } => {
                        goal = new_goal;

                        let mut update = (*update_tx.borrow()).clone();
                        update.goal = goal.clone();
                        update.status = "Goal set".to_string();
                        let _ = update_tx.send(update);
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
                        let update = GenerativeUiUpdate {
                            status: "Updated".to_string(),
                            seq,
                            interval_secs: interval.as_secs().max(1),
                            state: state.clone(),
                            goal: goal.clone(),
                            component_tree_json,
                            component_code,
                            last_error: None,
                        };
                        let _ = update_tx.send(update);
                    }
                    Err(e) => {
                        let mut update = (*update_tx.borrow()).clone();
                        update.seq = seq;
                        update.status = "Generation failed".to_string();
                        update.last_error = Some(e.to_string());
                        let _ = update_tx.send(update);
                    }
                }
            }
        }
    }
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
    out.push_str("pub fn generated_component() -> Element {\n");
    out.push_str("    rsx!(\n");
    render_node(root, 2, &mut out);
    out.push_str("    )\n");
    out.push_str("}\n");
    out
}

fn render_node(node: &UiNode, indent: usize, out: &mut String) {
    let pad = "    ".repeat(indent);
    match node.tag {
        Union2KlabelOrKrect::Klabel => {
            let text = node.text.as_deref().unwrap_or("");
            out.push_str(&format!(
                "{pad}label {{ \"{}\" }}\n",
                escape_rsx_string(text)
            ));
        }
        Union2KlabelOrKrect::Krect => {
            out.push_str(&format!("{pad}rect {{\n"));

            let mut keys = node.attrs.keys().collect::<Vec<_>>();
            keys.sort();
            for k in keys {
                if let Some(v) = node.attrs.get(k) {
                    let v_escaped = escape_rsx_string(v);
                    out.push_str(&format!("{pad}    {k}: \"{v_escaped}\",\n"));
                }
            }

            for child in &node.children {
                render_node(child.as_ref(), indent + 1, out);
            }

            out.push_str(&format!("{pad}}}\n"));
        }
    }
}

fn escape_rsx_string(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "")
}

fn merge_json_in_place(target: &mut Value, patch: Value) {
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
