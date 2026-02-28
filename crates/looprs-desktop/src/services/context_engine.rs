use looprs_desktop_baml_client::types::{UiContext, WorkflowStage, WorkflowState};
use looprs_desktop_baml_client::{init as baml_init, B};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::{mpsc, watch, RwLock};

const MAX_HISTORY_SIZE: usize = 100;

/// Commands for the context engine
#[derive(Debug, Clone)]
pub enum ContextCommand {
    AnalyzeText {
        text: String,
        history: String,
    },
    UpdateMetrics {
        metrics: serde_json::Value,
    },
    UpdateWorkflow {
        stage: String,
        progress: f64,
        current_step: String,
        total_steps: i64,
        blockers: Vec<String>,
    },
    Reset,
}

/// System metrics for health monitoring
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SystemMetrics {
    pub cpu_usage: f64,
    pub memory_usage: f64,
    pub error_rate: f64,
    pub response_time_p95: f64,
}

/// Context engine manages unified context from multiple sources
pub struct ContextEngine {
    current_context: Arc<RwLock<UiContext>>,
    health_provider: SystemHealthProvider,
    command_rx: mpsc::Receiver<ContextCommand>,
    update_tx: watch::Sender<Arc<UiContext>>,
}

impl ContextEngine {
    pub fn new() -> (Self, ContextEngineHandle) {
        let (command_tx, command_rx) = mpsc::channel(100);
        let (update_tx, update_rx) = watch::channel(Arc::new(default_ui_context()));

        let current_context = Arc::new(RwLock::new(default_ui_context()));

        let engine = Self {
            current_context: current_context.clone(),
            health_provider: SystemHealthProvider::new(),
            command_rx,
            update_tx,
        };

        let handle = ContextEngineHandle {
            command_tx,
            update_rx,
            current_context,
        };

        (engine, handle)
    }

    pub async fn run(&mut self) {
        // Initialize BAML client
        baml_init();

        loop {
            tokio::select! {
                Some(cmd) = self.command_rx.recv() => {
                    self.handle_command(cmd).await;
                }
                else => break,
            }
        }
    }

    async fn handle_command(&mut self, cmd: ContextCommand) {
        match cmd {
            ContextCommand::AnalyzeText { text, history } => {
                self.analyze_text(text, history).await;
            }
            ContextCommand::UpdateMetrics { metrics } => {
                self.update_metrics(metrics).await;
            }
            ContextCommand::UpdateWorkflow {
                stage,
                progress,
                current_step,
                total_steps,
                blockers,
            } => {
                self.update_workflow(stage, progress, current_step, total_steps, blockers)
                    .await;
            }
            ContextCommand::Reset => {
                self.reset().await;
            }
        }
    }

    async fn analyze_text(&mut self, text: String, history: String) {
        // Run intent analysis and sentiment analysis in parallel
        let intent_future = B.AnalyzeIntent.call(text.clone(), history);
        let sentiment_future = B.AnalyzeSentiment.call(text, "user_input".to_string());

        let (intent_result, sentiment_result) = tokio::join!(intent_future, sentiment_future);

        let mut ctx = self.current_context.write().await;

        // Update intent
        if let Ok(intent) = intent_result {
            ctx.intent = Some(intent);
        }

        // Update sentiment
        if let Ok(analysis) = sentiment_result {
            // Convert MessageAnalysis to SentimentContext
            ctx.sentiment = Some(looprs_desktop_baml_client::types::SentimentContext {
                sentiment: analysis.sentiment,
                severity: analysis.severity,
                mood: analysis.mood,
                confidence: analysis.confidence,
                keywords: analysis.key_phrases,
                tone_adjustments: std::collections::HashMap::new(),
            });
        }

        // Update timestamp
        ctx.timestamp = chrono::Utc::now().to_rfc3339();

        // Notify subscribers
        let _ = self.update_tx.send(Arc::new(ctx.clone()));
    }

    async fn update_metrics(&mut self, metrics: serde_json::Value) {
        let metrics_json = serde_json::to_string(&metrics).unwrap_or_default();

        // Get historical metrics for anomaly detection
        let history_json = self.health_provider.get_history_json();

        // Run health assessment and anomaly detection in parallel
        let health_future = B.AssessSystemHealth.call(metrics_json.clone());
        let anomalies_future = B.DetectAnomalies.call(history_json, metrics_json.clone());

        let (health_result, anomalies_result) = tokio::join!(health_future, anomalies_future);

        // Store metrics in history
        if let Ok(system_metrics) = serde_json::from_value::<SystemMetrics>(metrics) {
            self.health_provider.add_metrics(system_metrics);
        }

        let mut ctx = self.current_context.write().await;

        // Update system health
        if let Ok(health) = health_result {
            ctx.system_health = Some(health);
        }

        // Update anomalies
        if let Ok(anomalies) = anomalies_result {
            ctx.anomalies = anomalies;
        }

        // Update timestamp
        ctx.timestamp = chrono::Utc::now().to_rfc3339();

        // Notify subscribers
        let _ = self.update_tx.send(Arc::new(ctx.clone()));
    }

    async fn update_workflow(
        &mut self,
        stage: String,
        progress: f64,
        current_step: String,
        total_steps: i64,
        blockers: Vec<String>,
    ) {
        let mut ctx = self.current_context.write().await;

        let workflow_stage = match stage.as_str() {
            "not_started" => WorkflowStage::NotStarted,
            "planning" => WorkflowStage::Planning,
            "executing" => WorkflowStage::Executing,
            "reviewing" => WorkflowStage::Reviewing,
            "completed" => WorkflowStage::Completed,
            "failed" => WorkflowStage::Failed,
            _ => WorkflowStage::NotStarted,
        };

        ctx.workflow_state = Some(WorkflowState {
            stage: workflow_stage,
            progress,
            current_step,
            total_steps,
            blockers,
            estimated_remaining_time: None,
        });

        // Update timestamp
        ctx.timestamp = chrono::Utc::now().to_rfc3339();

        // Notify subscribers
        let _ = self.update_tx.send(Arc::new(ctx.clone()));
    }

    async fn reset(&mut self) {
        let mut ctx = self.current_context.write().await;
        *ctx = default_ui_context();
        ctx.timestamp = chrono::Utc::now().to_rfc3339();

        // Notify subscribers
        let _ = self.update_tx.send(Arc::new(ctx.clone()));
    }
}

impl Default for ContextEngine {
    fn default() -> Self {
        Self::new().0
    }
}

/// Handle for interacting with the context engine
#[derive(Clone)]
pub struct ContextEngineHandle {
    command_tx: mpsc::Sender<ContextCommand>,
    update_rx: watch::Receiver<Arc<UiContext>>,
    current_context: Arc<RwLock<UiContext>>,
}

impl ContextEngineHandle {
    pub fn send(&self, cmd: ContextCommand) {
        let tx = self.command_tx.clone();
        tokio::spawn(async move {
            let _ = tx.send(cmd).await;
        });
    }

    pub async fn get_context(&self) -> Arc<UiContext> {
        self.current_context.read().await.clone().into()
    }

    pub fn subscribe(&self) -> watch::Receiver<Arc<UiContext>> {
        self.update_rx.clone()
    }
}

/// System health provider tracks metrics history
struct SystemHealthProvider {
    metrics_history: VecDeque<SystemMetrics>,
}

impl SystemHealthProvider {
    fn new() -> Self {
        Self {
            metrics_history: VecDeque::with_capacity(MAX_HISTORY_SIZE),
        }
    }

    fn add_metrics(&mut self, metrics: SystemMetrics) {
        if self.metrics_history.len() >= MAX_HISTORY_SIZE {
            self.metrics_history.pop_front();
        }
        self.metrics_history.push_back(metrics);
    }

    fn get_history_json(&self) -> String {
        let history: Vec<&SystemMetrics> = self.metrics_history.iter().collect();
        serde_json::to_string(&history).unwrap_or_else(|_| "[]".to_string())
    }
}

// Helper function to create default UiContext
pub fn default_ui_context() -> UiContext {
    UiContext {
        sentiment: None,
        intent: None,
        system_health: None,
        workflow_state: None,
        anomalies: Vec::new(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_context_engine_creation() {
        let (_engine, handle) = ContextEngine::new();
        assert!(handle.get_context().await.sentiment.is_none());
    }

    #[tokio::test]
    async fn test_update_workflow() {
        let (mut engine, handle) = ContextEngine::new();

        // Spawn engine in background
        tokio::spawn(async move {
            engine.run().await;
        });

        // Send workflow update
        handle.send(ContextCommand::UpdateWorkflow {
            stage: "executing".to_string(),
            progress: 0.5,
            current_step: "Building UI".to_string(),
            total_steps: 5,
            blockers: vec![],
        });

        // Wait for update
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let ctx = handle.get_context().await;
        assert!(ctx.workflow_state.is_some());
        let workflow = ctx.workflow_state.as_ref().unwrap();
        assert_eq!(workflow.progress, 0.5);
    }
}
