use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, watch, RwLock};

use looprs_desktop_baml_client::types::{
    AdaptiveColors, MessageAnalysis, Mood, Sentiment, SentimentContext, Severity, UiNode,
};
use looprs_desktop_baml_client::{init as baml_init, B};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SentimentUiCommand {
    AnalyzeText { text: String, context: String },
    UpdateMood { mood: String },
    SetSeverityThreshold { threshold: String },
    RegenerateWithSentiment { goal: String },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SentimentUiUpdate {
    pub status: String,
    pub seq: u64,

    // Current sentiment state
    pub current_sentiment: Option<String>,
    pub current_severity: Option<String>,
    pub current_mood: Option<String>,
    pub sentiment_confidence: f64,

    // Sentiment history (last 10)
    pub sentiment_history: Vec<SentimentHistoryEntry>,

    // Generated UI
    pub component_tree_json: Value,
    pub component_code: String,
    pub adaptive_styling: Value,

    // Dynamic text cache
    pub dynamic_texts: Vec<DynamicTextEntry>,

    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentimentHistoryEntry {
    pub timestamp: u64,
    pub text: String,
    pub sentiment: String,
    pub severity: String,
    pub mood: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicTextEntry {
    pub component_id: String,
    pub template: String,
    pub generated_text: String,
    pub sentiment_adjusted: bool,
}

#[derive(Debug, Clone)]
pub struct SentimentUiHandle {
    pub updates: watch::Receiver<Arc<SentimentUiUpdate>>,
    command_tx: mpsc::Sender<SentimentUiCommand>,
    shutdown_tx: watch::Sender<bool>,
}

impl SentimentUiHandle {
    pub fn stop(&self) {
        let _ = self.shutdown_tx.send(true);
    }

    pub fn send(&self, command: SentimentUiCommand) {
        let _ = self.command_tx.try_send(command);
    }

    pub fn analyze_text(&self, text: String, context: String) {
        let _ = self
            .command_tx
            .try_send(SentimentUiCommand::AnalyzeText { text, context });
    }
}

pub fn start_sentiment_ui(update_interval: Duration) -> SentimentUiHandle {
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let (command_tx, command_rx) = mpsc::channel::<SentimentUiCommand>(64);

    let initial_update = SentimentUiUpdate::default();
    let (update_tx, update_rx) = watch::channel(Arc::new(initial_update));

    tokio::spawn(sentiment_loop(
        update_interval,
        command_rx,
        update_tx,
        shutdown_rx,
    ));

    SentimentUiHandle {
        updates: update_rx,
        command_tx,
        shutdown_tx,
    }
}

async fn sentiment_loop(
    interval: Duration,
    mut command_rx: mpsc::Receiver<SentimentUiCommand>,
    update_tx: watch::Sender<Arc<SentimentUiUpdate>>,
    mut shutdown_rx: watch::Receiver<bool>,
) {
    if std::env::var("OPENAI_API_KEY")
        .ok()
        .is_none_or(|v| v.trim().is_empty())
    {
        let mut update = (*update_tx.borrow()).as_ref().clone();
        update.status = "Missing OPENAI_API_KEY".to_string();
        update.last_error = Some("Set OPENAI_API_KEY to enable sentiment analysis".to_string());
        let _ = update_tx.send(Arc::new(update));
        return;
    }

    baml_init();

    let sentiment_history = Arc::new(RwLock::new(VecDeque::<SentimentHistoryEntry>::new()));
    let current_sentiment_context = Arc::new(RwLock::new(None::<MessageAnalysis>));
    let mut seq: u64 = 0;

    {
        let mut update = (*update_tx.borrow()).as_ref().clone();
        update.status = "Sentiment UI ready".to_string();
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
                    SentimentUiCommand::AnalyzeText { text, context } => {
                        seq = seq.saturating_add(1);

                        match B.AnalyzeSentiment.call(text.clone(), context).await {
                            Ok(analysis) => {
                                // Store in history
                                let entry = SentimentHistoryEntry {
                                    timestamp: std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap()
                                        .as_secs(),
                                    text: text.clone(),
                                    sentiment: format!("{:?}", analysis.sentiment),
                                    severity: format!("{:?}", analysis.severity),
                                    mood: format!("{:?}", analysis.mood),
                                    confidence: analysis.confidence,
                                };

                                let mut history = sentiment_history.write().await;
                                history.push_back(entry.clone());
                                if history.len() > 10 {
                                    history.pop_front();
                                }
                                drop(history);

                                // Update current context
                                *current_sentiment_context.write().await = Some(analysis.clone());

                                // Generate sentiment-aware UI
                                let state = serde_json::json!({
                                    "message": text,
                                    "sentiment": format!("{:?}", analysis.sentiment),
                                    "severity": format!("{:?}", analysis.severity),
                                    "mood": format!("{:?}", analysis.mood),
                                });

                                let sentiment_json = serde_json::to_string_pretty(&analysis)
                                    .unwrap_or_default();

                                match B.GenerateSentimentAwareUi.call(
                                    "Display message with sentiment-aware styling".to_string(),
                                    serde_json::to_string(&state).unwrap(),
                                    sentiment_json,
                                ).await {
                                    Ok(ui_tree) => {
                                        let component_code = render_sentiment_component(&ui_tree);
                                        let adaptive_styling = extract_adaptive_styling(&ui_tree);
                                        let dynamic_texts = extract_dynamic_texts(&ui_tree);

                                        let history_vec = sentiment_history.read().await.iter().cloned().collect();

                                        let update = SentimentUiUpdate {
                                            status: "Updated with sentiment analysis".to_string(),
                                            seq,
                                            current_sentiment: Some(format!("{:?}", analysis.sentiment)),
                                            current_severity: Some(format!("{:?}", analysis.severity)),
                                            current_mood: Some(format!("{:?}", analysis.mood)),
                                            sentiment_confidence: analysis.confidence,
                                            sentiment_history: history_vec,
                                            component_tree_json: ui_node_to_json(&ui_tree),
                                            component_code,
                                            adaptive_styling,
                                            dynamic_texts,
                                            last_error: None,
                                        };

                                        let _ = update_tx.send(Arc::new(update));
                                    }
                                    Err(e) => {
                                        let mut update = (*update_tx.borrow()).as_ref().clone();
                                        update.seq = seq;
                                        update.status = "UI generation failed".to_string();
                                        update.last_error = Some(e.to_string());
                                        let _ = update_tx.send(Arc::new(update));
                                    }
                                }
                            }
                            Err(e) => {
                                let mut update = (*update_tx.borrow()).as_ref().clone();
                                update.seq = seq;
                                update.status = "Sentiment analysis failed".to_string();
                                update.last_error = Some(e.to_string());
                                let _ = update_tx.send(Arc::new(update));
                            }
                        }
                    }

                    SentimentUiCommand::RegenerateWithSentiment { goal } => {
                        seq = seq.saturating_add(1);

                        if let Some(ref analysis) = *current_sentiment_context.read().await {
                            let state = serde_json::json!({
                                "goal": goal,
                                "sentiment": format!("{:?}", analysis.sentiment),
                            });

                            let sentiment_json = serde_json::to_string_pretty(&analysis)
                                .unwrap_or_default();

                            match B.GenerateSentimentAwareUi.call(
                                goal,
                                serde_json::to_string(&state).unwrap(),
                                sentiment_json,
                            ).await {
                                Ok(ui_tree) => {
                                    let component_code = render_sentiment_component(&ui_tree);
                                    let adaptive_styling = extract_adaptive_styling(&ui_tree);
                                    let dynamic_texts = extract_dynamic_texts(&ui_tree);

                                    let mut update = (*update_tx.borrow()).as_ref().clone();
                                    update.seq = seq;
                                    update.status = "Regenerated with sentiment".to_string();
                                    update.component_tree_json = ui_node_to_json(&ui_tree);
                                    update.component_code = component_code;
                                    update.adaptive_styling = adaptive_styling;
                                    update.dynamic_texts = dynamic_texts;
                                    let _ = update_tx.send(Arc::new(update));
                                }
                                Err(e) => {
                                    let mut update = (*update_tx.borrow()).as_ref().clone();
                                    update.seq = seq;
                                    update.last_error = Some(e.to_string());
                                    let _ = update_tx.send(Arc::new(update));
                                }
                            }
                        }
                    }

                    _ => {}
                }
            }
        }
    }
}

fn ui_node_to_json(node: &UiNode) -> Value {
    let mut obj = serde_json::Map::new();
    obj.insert("tag".to_string(), Value::String(node.tag.to_string()));

    if let Some(ref text) = node.text {
        obj.insert("text".to_string(), Value::String(text.clone()));
    }

    obj.insert(
        "attrs".to_string(),
        serde_json::to_value(&node.attrs).unwrap_or(Value::Null),
    );

    if let Some(ref sentiment_ctx) = node.sentiment_context {
        obj.insert(
            "sentiment_context".to_string(),
            serde_json::json!({
                "sentiment": format!("{:?}", sentiment_ctx.sentiment),
                "severity": format!("{:?}", sentiment_ctx.severity),
                "mood": format!("{:?}", sentiment_ctx.mood),
                "confidence": sentiment_ctx.confidence,
            }),
        );
    }

    if let Some(ref template) = node.dynamic_text_template {
        obj.insert(
            "dynamic_text_template".to_string(),
            Value::String(template.clone()),
        );
    }

    let children = node
        .children
        .iter()
        .map(|c| ui_node_to_json(c.as_ref()))
        .collect::<Vec<_>>();
    obj.insert("children".to_string(), Value::Array(children));

    Value::Object(obj)
}

fn render_sentiment_component(root: &UiNode) -> String {
    let mut out = String::new();
    out.push_str("use freya::prelude::*;\n\n");
    out.push_str("pub fn sentiment_aware_component() -> impl IntoElement {\n");
    render_node_with_sentiment(root, 1, &mut out);
    out.push_str("}\n");
    out
}

fn render_node_with_sentiment(node: &UiNode, indent: usize, out: &mut String) {
    let pad = "    ".repeat(indent);
    let chain_pad = "    ".repeat(indent + 1);

    // Determine background color based on sentiment
    let bg_color = if let Some(ref adaptive) = node.adaptive_colors {
        if let Some(ref ctx) = node.sentiment_context {
            match ctx.sentiment {
                Sentiment::VeryPositive | Sentiment::Positive => {
                    adaptive.background_positive.clone()
                }
                Sentiment::Neutral => adaptive.background_neutral.clone(),
                Sentiment::Negative | Sentiment::VeryNegative => {
                    adaptive.background_negative.clone()
                }
            }
        } else {
            adaptive.background_neutral.clone()
        }
    } else {
        "rgb(245, 245, 245)".to_string()
    };

    match node.tag.as_str() {
        "label" => {
            let text = node.text.as_deref().unwrap_or("");
            out.push_str(&format!("{pad}label().text(\"{text}\")"));
        }
        "rect" => {
            out.push_str(&format!("{pad}rect()"));
        }
        _ => {
            out.push_str(&format!("{pad}rect()"));
        }
    }

    // Add background from sentiment
    out.push_str(&format!("\n{chain_pad}.background({bg_color})"));

    // Add other attrs
    for (k, v) in &node.attrs {
        if let Some(method) = render_attr_method(k, v) {
            out.push_str(&format!("\n{chain_pad}{method}"));
        }
    }

    // Render children
    for child in &node.children {
        out.push_str(&format!("\n{chain_pad}.child(\n"));
        render_node_with_sentiment(child.as_ref(), indent + 2, out);
        out.push_str(&format!("\n{chain_pad})"));
    }

    out.push('\n');
}

fn render_attr_method(key: &str, value: &str) -> Option<String> {
    match key {
        "width" => Some(format!(".width(Size::px({value}))")),
        "height" => Some(format!(".height(Size::px({value}))")),
        "padding" => value
            .parse::<f32>()
            .ok()
            .map(|n| format!(".padding(Gaps::new_all({n}))")),
        "corner_radius" => value
            .parse::<f32>()
            .ok()
            .map(|n| format!(".corner_radius({n})")),
        _ => None,
    }
}

fn extract_adaptive_styling(node: &UiNode) -> Value {
    let mut styles = Vec::new();

    fn collect_styles(node: &UiNode, styles: &mut Vec<Value>) {
        if let Some(ref adaptive) = node.adaptive_colors {
            styles.push(serde_json::json!({
                "background_positive": adaptive.background_positive,
                "background_neutral": adaptive.background_neutral,
                "background_negative": adaptive.background_negative,
                "text_positive": adaptive.text_positive,
                "text_neutral": adaptive.text_neutral,
                "text_negative": adaptive.text_negative,
            }));
        }

        for child in &node.children {
            collect_styles(child.as_ref(), styles);
        }
    }

    collect_styles(node, &mut styles);
    Value::Array(styles)
}

fn extract_dynamic_texts(node: &UiNode) -> Vec<DynamicTextEntry> {
    let mut entries = Vec::new();

    fn collect_dynamic_texts(node: &UiNode, entries: &mut Vec<DynamicTextEntry>, id_counter: &mut usize) {
        if let Some(ref template) = node.dynamic_text_template {
            entries.push(DynamicTextEntry {
                component_id: format!("node_{}", id_counter),
                template: template.clone(),
                generated_text: node.text.clone().unwrap_or_default(),
                sentiment_adjusted: node.sentiment_context.is_some(),
            });
            *id_counter += 1;
        }

        for child in &node.children {
            collect_dynamic_texts(child.as_ref(), entries, id_counter);
        }
    }

    let mut counter = 0;
    collect_dynamic_texts(node, &mut entries, &mut counter);
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    fn live_tests_enabled() -> bool {
        std::env::var("LOOPRS_RUN_LIVE_LLM_TESTS")
            .ok()
            .is_some_and(|v| v == "1" || v == "true")
    }

    #[tokio::test]
    async fn test_sentiment_ui_handle_creation() {
        let handle = start_sentiment_ui(Duration::from_secs(60));
        tokio::time::sleep(Duration::from_millis(100)).await;
        handle.stop();
    }

    #[tokio::test]
    #[ignore]
    async fn test_sentiment_analysis_integration() {
        if !live_tests_enabled() {
            return;
        }

        let handle = start_sentiment_ui(Duration::from_secs(60));

        handle.analyze_text(
            "This is amazing! Everything is working perfectly!".to_string(),
            "User feedback".to_string(),
        );

        tokio::time::sleep(Duration::from_secs(2)).await;

        let update = handle.updates.borrow().clone();
        assert!(update.current_sentiment.is_some());
        assert!(update.sentiment_confidence > 0.0);

        handle.stop();
    }
}
