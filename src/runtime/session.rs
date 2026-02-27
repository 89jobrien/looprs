use crate::agent::Agent;
use crate::errors::AgentError;
use std::collections::HashMap;

pub async fn run_single_turn(
    agent: &mut Agent,
    prompt: impl Into<String>,
    metadata: HashMap<String, String>,
) -> Result<String, AgentError> {
    let prompt = prompt.into();
    agent.set_turn_metadata(metadata);
    agent.add_user_message(prompt);
    agent.run_turn().await?;
    Ok(agent
        .latest_assistant_text()
        .unwrap_or_else(|| "No assistant text blocks were returned.".to_string()))
}
