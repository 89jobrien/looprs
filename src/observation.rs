use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

/// A captured observation from tool usage in a session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    /// Tool that was executed (bash, read, write, grep, glob, edit)
    pub tool_name: String,
    /// Input to the tool (as JSON)
    pub input: Value,
    /// Output from the tool (as string)
    pub output: String,
    /// When this observation was captured (unix timestamp)
    pub timestamp: u64,
    /// Session ID this observation belongs to
    pub session_id: String,
    /// Brief context/summary (generated from surrounding conversation)
    pub context: Option<String>,
}

impl Observation {
    /// Create a new observation
    pub fn new(tool_name: String, input: Value, output: String, session_id: String) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Observation {
            tool_name,
            input,
            output,
            timestamp,
            session_id,
            context: None,
        }
    }

    /// Add context/summary to observation
    pub fn with_context(mut self, context: String) -> Self {
        self.context = Some(context);
        self
    }

    /// Format observation as a bd issue description
    pub fn to_bd_description(&self) -> String {
        let input_str = serde_json::to_string_pretty(&self.input).unwrap_or_default();
        let output_preview = if self.output.len() > 500 {
            format!("{}...", &self.output[..500])
        } else {
            self.output.clone()
        };

        // Convert timestamp to readable format
        let time_str = match SystemTime::UNIX_EPOCH
            .checked_add(std::time::Duration::from_secs(self.timestamp))
        {
            Some(t) => format!("{:?}", t),
            None => format!("{} (unix timestamp)", self.timestamp),
        };

        format!(
            "**Tool:** {}\n**Time:** {}\n\n**Input:**\n```\n{}\n```\n\n**Output:**\n```\n{}\n```{}",
            self.tool_name,
            time_str,
            input_str,
            output_preview,
            if let Some(ctx) = &self.context {
                format!("\n\n**Context:** {}", ctx)
            } else {
                String::new()
            }
        )
    }

    /// Generate a title for the bd issue
    pub fn to_bd_title(&self) -> String {
        if let Some(ctx) = &self.context {
            format!("Observation: {}", ctx.chars().take(60).collect::<String>())
        } else {
            format!("Observation: {}", self.tool_name)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_observation_creation() {
        let obs = Observation::new(
            "bash".to_string(),
            serde_json::json!({"command": "cargo test"}),
            "test result: ok".to_string(),
            "sess-123".to_string(),
        );

        assert_eq!(obs.tool_name, "bash");
        assert_eq!(obs.session_id, "sess-123");
        assert!(obs.context.is_none());
    }

    #[test]
    fn test_observation_with_context() {
        let obs = Observation::new(
            "bash".to_string(),
            serde_json::json!({"command": "cargo test"}),
            "test result: ok".to_string(),
            "sess-123".to_string(),
        )
        .with_context("Testing changes to agent.rs".to_string());

        assert_eq!(obs.context, Some("Testing changes to agent.rs".to_string()));
    }

    #[test]
    fn test_observation_bd_title() {
        let obs = Observation::new(
            "bash".to_string(),
            serde_json::json!({}),
            "output".to_string(),
            "sess-123".to_string(),
        )
        .with_context("Fixed parser edge case".to_string());

        let title = obs.to_bd_title();
        assert!(title.contains("Fixed parser edge case"));
        assert!(title.starts_with("Observation:"));
    }

    #[test]
    fn test_observation_bd_description() {
        let obs = Observation::new(
            "bash".to_string(),
            serde_json::json!({"command": "test"}),
            "success".to_string(),
            "sess-123".to_string(),
        )
        .with_context("Test execution".to_string());

        let desc = obs.to_bd_description();
        assert!(desc.contains("bash"));
        assert!(desc.contains("success"));
        assert!(desc.contains("Test execution"));
    }
}
