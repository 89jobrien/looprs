use agent_app::IncidentUseCase;
use agent_domain::{Incident, JiraPort, LlmPort, TracePort};
use axum::{Json, Router, extract::State, http::StatusCode, routing::{get, post}};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Shared application state
// ---------------------------------------------------------------------------

pub struct AppState {
    pub llm: Arc<dyn LlmPort>,
    pub jira: Arc<dyn JiraPort>,
    pub trace: Arc<dyn TracePort>,
}

// ---------------------------------------------------------------------------
// Request / response DTOs
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct RunIncidentRequest {
    id: String,
    title: String,
    raw_text: String,
}

#[derive(Serialize)]
struct RunIncidentResponse {
    trace_id: String,
    plan: agent_domain::RemediationPlan,
}

#[derive(Serialize)]
struct TraceResponse {
    steps: Vec<agent_domain::TraceStep>,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

async fn run_incident(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RunIncidentRequest>,
) -> Result<Json<RunIncidentResponse>, (StatusCode, String)> {
    let use_case = IncidentUseCase::new(
        state.llm.as_ref(),
        state.jira.as_ref(),
        state.trace.as_ref(),
    );

    let incident = Incident {
        id: req.id,
        title: req.title,
        raw_text: req.raw_text,
    };

    let (trace_id, plan) = use_case
        .execute(incident)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(RunIncidentResponse { trace_id, plan }))
}

async fn get_trace(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(trace_id): axum::extract::Path<String>,
) -> Result<Json<TraceResponse>, (StatusCode, String)> {
    let steps = state
        .trace
        .load_trace(&trace_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(TraceResponse { steps }))
}

// ---------------------------------------------------------------------------
// Router construction
// ---------------------------------------------------------------------------

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/incidents/run", post(run_incident))
        .route("/traces/{trace_id}", get(get_trace))
        .with_state(state)
}
