// Context-Aware UI System
// Comprehensive context types for adaptive UI generation

// ============================================================================
// User Intent Detection
// ============================================================================

enum IntentCategory {
  Debug
  Optimize
  Monitor
  Configure
  Explore
  Report
}

class UserIntent {
  category IntentCategory
  goal string                    // Natural language goal
  confidence float               // 0.0 - 1.0
  suggested_actions string[]     // Next steps
  required_context string[]      // What info is needed
}

// ============================================================================
// System Health Monitoring
// ============================================================================

enum HealthStatus {
  Healthy
  Degraded
  Critical
  Unknown
}

class SystemHealth {
  status HealthStatus
  cpu_usage float               // 0.0 - 100.0
  memory_usage float            // 0.0 - 100.0
  error_rate float              // errors per minute
  response_time_p95 float       // milliseconds
  active_alerts int
  recommendations string[]
}

// ============================================================================
// Workflow Progress Tracking
// ============================================================================

enum WorkflowStage {
  NotStarted
  Planning
  Executing
  Reviewing
  Completed
  Failed
}

class WorkflowState {
  stage WorkflowStage
  progress float                // 0.0 - 1.0
  current_step string
  total_steps int
  blockers string[]
  estimated_remaining_time string?
}

// ============================================================================
// Data Anomaly Detection
// ============================================================================

enum AnomalyType {
  Spike
  Drop
  Outlier
  Trend
  Missing
}

enum AnomalySeverity {
  Info
  Warning
  Critical
}

class DataAnomaly {
  type AnomalyType
  severity AnomalySeverity
  metric string
  current_value float
  expected_range (float, float)
  description string
  timestamp string
}

// ============================================================================
// Unified Context
// Note: Sentiment, Severity, Mood, and SentimentContext are defined in sentiment_ui.baml
// ============================================================================

class UiContext {
  sentiment SentimentContext?
  intent UserIntent?
  system_health SystemHealth?
  workflow_state WorkflowState?
  anomalies DataAnomaly[]
  timestamp string
}

// ============================================================================
// Context-Aware UI Node
// Note: AdaptiveColors is defined in sentiment_ui.baml with a different structure
// We define MaterialColors here for Material Design integration
// ============================================================================

class MaterialColors {
  primary (int, int, int)
  container (int, int, int)
  on_container (int, int, int)
  border (int, int, int)?
}

class ContextAwareUiNode {
  tag "rect" | "label" | "paragraph" | "card" | "button" | "chip" | "badge"
  text string?
  attrs map<string, string>
  children ContextAwareUiNode[]

  // Context-driven properties
  context_signals string[]       // Which context types influenced this node
  material_colors MaterialColors?
  dynamic_text_template string?
  urgency_level int              // 0-5, combines all context signals
  recommended_action string?     // User action to take
}

// ============================================================================
// Functions
// Note: AnalyzeSentiment is defined in sentiment_ui.baml
// ============================================================================

function AnalyzeIntent(
  user_input: string,
  conversation_history: string
) -> UserIntent {
  client "openai/gpt-4o-mini"
  prompt #"
    Analyze what the user is trying to accomplish.

    User input: {{ user_input }}
    History: {{ conversation_history }}

    Determine:
    - category: Debug/Optimize/Monitor/Configure/Explore/Report
    - goal: What they want to achieve (1-2 sentences)
    - confidence: How certain you are (0.0-1.0)
    - suggested_actions: 1-3 next steps to help them
    - required_context: What additional info would help

    {{ ctx.output_format }}
  "#
}

function AssessSystemHealth(
  metrics: string
) -> SystemHealth {
  client "openai/gpt-4o-mini"
  prompt #"
    Assess system health from these metrics.

    Metrics (JSON): {{ metrics }}

    Determine:
    - status: Healthy/Degraded/Critical/Unknown
    - Extract: cpu_usage, memory_usage, error_rate, response_time_p95
    - active_alerts: Count of current alerts (default 0)
    - recommendations: 2-4 actions to improve health

    Rules for status:
    - Critical: CPU >90% OR memory >85% OR error_rate >10/min
    - Degraded: CPU >70% OR memory >70% OR error_rate >5/min
    - Healthy: Otherwise
    - Unknown: If metrics are missing or invalid

    {{ ctx.output_format }}
  "#
}

function DetectAnomalies(
  metrics_history: string,
  current_metrics: string
) -> DataAnomaly[] {
  client "openai/gpt-4o-mini"
  prompt #"
    Detect anomalies in the current metrics compared to history.

    Historical data: {{ metrics_history }}
    Current data: {{ current_metrics }}

    For each anomaly found, provide:
    - type: Spike/Drop/Outlier/Trend/Missing
    - severity: Info/Warning/Critical
    - metric: Name of the metric
    - current_value: Actual value
    - expected_range: (min, max) tuple
    - description: What's unusual (1 sentence)
    - timestamp: Current ISO 8601 timestamp

    Return empty array if no anomalies detected.

    {{ ctx.output_format }}
  "#
}

function GenerateContextAwareUi(
  goal: string,
  context: UiContext
) -> ContextAwareUiNode {
  client "openai/gpt-4o-mini"
  prompt #"
    Generate a Freya UI component that adapts to the provided context.

    Goal: {{ goal }}
    Context: {{ context }}

    Rules:
    1. Combine ALL context signals to determine styling:
       - sentiment + system_health → color (positive+healthy=green, negative+critical=red)
       - workflow_stage + progress → layout (planning=loose, executing=focused)
       - anomalies → urgency (multiple critical anomalies = highest urgency)
       - intent → recommended_action (what user should do next)

    2. Urgency level (0-5) based on:
       - Level 5: Critical health + critical anomalies + negative sentiment
       - Level 4: Critical health OR critical anomalies
       - Level 3: Degraded health + anomalies
       - Level 2: Degraded health OR warnings
       - Level 1: Minor issues
       - Level 0: All healthy

    3. Material Design colors based on urgency (set material_colors field):
       - 5: primary=(244,67,54), container=(255,205,210), on_container=(183,28,28), border=(244,67,54)
       - 4: primary=(255,152,0), container=(255,224,178), on_container=(230,81,0), border=(255,152,0)
       - 3: primary=(255,235,59), container=(255,249,196), on_container=(245,127,23), border=(255,235,59)
       - 2: primary=(33,150,243), container=(187,222,251), on_container=(1,87,155), border=null
       - 0-1: primary=(76,175,80), container=(200,230,201), on_container=(27,94,32), border=null

    4. Include recommended_action when intent is clear

    5. Set context_signals to list which context types influenced the design

    {{ ctx.output_format }}
  "#
}
