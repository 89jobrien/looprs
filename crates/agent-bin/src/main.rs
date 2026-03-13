use http_api_adapter::{AppState, router};
use jira_adapter::JiraAdapter;
use llm_openai_adapter::OpenAiLlm;
use std::sync::Arc;
use trace_sqlite_adapter::SqliteTraceAdapter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    // --- Build adapters (all implement inward-facing ports) ---

    let llm = Arc::new(OpenAiLlm::new(
        std::env::var("OPENAI_API_KEY").unwrap_or_else(|_| "sk-placeholder".into()),
        std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4.1-mini".into()),
    )) as Arc<dyn agent_domain::LlmPort>;

    let jira = Arc::new(JiraAdapter::new(
        std::env::var("JIRA_BASE_URL").unwrap_or_else(|_| "https://jira.example.com".into()),
        std::env::var("JIRA_EMAIL").unwrap_or_else(|_| "bot@example.com".into()),
        std::env::var("JIRA_API_TOKEN").unwrap_or_else(|_| "placeholder".into()),
    )) as Arc<dyn agent_domain::JiraPort>;

    let trace_path = std::env::var("TRACE_DB_PATH").unwrap_or_else(|_| "trace.db".into());
    let trace = Arc::new(SqliteTraceAdapter::new(&trace_path)?)
        as Arc<dyn agent_domain::TracePort>;

    // --- Wire state and start HTTP server ---

    let state = Arc::new(AppState { llm, jira, trace });
    let app = router(state);

    let bind = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:3000".into());
    let listener = tokio::net::TcpListener::bind(&bind).await?;

    eprintln!("agent-bin listening on {bind}");
    axum::serve(listener, app).await?;

    Ok(())
}
