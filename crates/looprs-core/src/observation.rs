use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::types::ToolId;

const OUTPUT_PREVIEW_LEN: usize = 500;

/// A captured observation from tool usage in a session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    /// Name of the tool that was executed.
    pub tool_name: String,
    /// Structured input passed to the tool.
    pub input: Value,
    /// Raw output returned by the tool.
    pub output: String,
    /// Provider-specific tool-use identifier, when available.
    pub tool_use_id: Option<ToolId>,
    /// Unix timestamp (seconds) when observation was captured.
    pub timestamp: u64,
    /// Session identifier this observation belongs to.
    pub session_id: String,
    /// Optional human note about why this observation matters.
    pub context: Option<String>,
}

impl Observation {
    /// Capture an observation, stamping it with the current Unix time.
    ///
    /// The timestamp falls back to `0` if the system clock is before the Unix
    /// epoch. [`Observation::context`] starts unset; add it with
    /// [`Observation::with_context`].
    pub fn new(
        tool_name: String,
        input: Value,
        output: String,
        tool_use_id: Option<ToolId>,
        session_id: String,
    ) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Observation {
            tool_name,
            input,
            output,
            tool_use_id,
            timestamp,
            session_id,
            context: None,
        }
    }

    /// Attach a human-readable note explaining why this observation matters.
    ///
    /// The context also becomes the basis of [`Observation::to_title`].
    pub fn with_context(mut self, context: String) -> Self {
        self.context = Some(context);
        self
    }

    /// Render the observation as Markdown for injection into model context.
    ///
    /// Output longer than 500 characters is truncated with a trailing `...`.
    /// Truncation counts Unicode scalar values (`char`s), not bytes, so it
    /// never panics or splits a multibyte character. The tool-use ID and
    /// context sections are omitted when unset.
    pub fn to_description(&self) -> String {
        let input_str = serde_json::to_string_pretty(&self.input).unwrap_or_default();
        let output_preview = if self.output.chars().count() > OUTPUT_PREVIEW_LEN {
            let truncated: String = self.output.chars().take(OUTPUT_PREVIEW_LEN).collect();
            format!("{truncated}...")
        } else {
            self.output.clone()
        };

        let time_str = match SystemTime::UNIX_EPOCH
            .checked_add(std::time::Duration::from_secs(self.timestamp))
        {
            Some(t) => format!("{t:?}"),
            None => format!("{} (unix timestamp)", self.timestamp),
        };

        format!(
            "**Tool:** {}\n{}**Time:** {}\n\n**Input:**\n```\n{}\n```\n\n**Output:**\n```\n{}\n```{}",
            self.tool_name,
            self.tool_use_id
                .as_ref()
                .map(|id| format!("**Tool Use ID:** {id}\n"))
                .unwrap_or_default(),
            time_str,
            input_str,
            output_preview,
            if let Some(ctx) = &self.context {
                format!("\n\n**Context:** {ctx}")
            } else {
                String::new()
            }
        )
    }

    /// Build a short `Observation: ...` label for this capture.
    ///
    /// Uses the first 60 characters of [`Observation::context`] when set,
    /// otherwise the tool name.
    pub fn to_title(&self) -> String {
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
    fn observation_creation() {
        let obs = Observation::new(
            "bash".to_string(),
            serde_json::json!({"command": "cargo test"}),
            "test result: ok".to_string(),
            Some(ToolId::new("tool_123")),
            "sess-123".to_string(),
        );

        assert_eq!(obs.tool_name, "bash");
        assert_eq!(
            obs.tool_use_id.map(|v| v.as_str().to_string()),
            Some("tool_123".to_string())
        );
        assert_eq!(obs.session_id, "sess-123");
        assert!(obs.context.is_none());
    }

    #[test]
    fn observation_with_context() {
        let obs = Observation::new(
            "bash".to_string(),
            serde_json::json!({}),
            "output".to_string(),
            None,
            "sess-123".to_string(),
        )
        .with_context("Testing changes".to_string());

        assert_eq!(obs.context, Some("Testing changes".to_string()));
    }

    #[test]
    fn observation_description() {
        let obs = Observation::new(
            "bash".to_string(),
            serde_json::json!({"command": "test"}),
            "success".to_string(),
            Some(ToolId::new("tool_7")),
            "sess-123".to_string(),
        )
        .with_context("Test execution".to_string());

        let desc = obs.to_description();
        assert!(desc.contains("bash"));
        assert!(desc.contains("tool_7"));
        assert!(desc.contains("success"));
        assert!(desc.contains("Test execution"));
    }

    #[test]
    fn description_truncates_multibyte_output_without_panicking() {
        // 501 multibyte chars (3 bytes each): a byte-index slice at 500
        // would land inside a character and panic.
        let long_output: String = std::iter::repeat_n('\u{2603}', 501).collect();
        let obs = Observation::new(
            "bash".to_string(),
            serde_json::json!({}),
            long_output,
            None,
            "sess-123".to_string(),
        );

        let desc = obs.to_description();
        let expected_preview: String = std::iter::repeat_n('\u{2603}', 500).collect();
        assert!(desc.contains(&format!("{expected_preview}...")));
    }

    #[test]
    fn title_with_context() {
        let obs = Observation::new(
            "bash".to_string(),
            serde_json::json!({}),
            "output".to_string(),
            None,
            "sess-123".to_string(),
        )
        .with_context("Fixed parser edge case".to_string());

        let title = obs.to_title();
        assert!(title.contains("Fixed parser edge case"));
        assert!(title.starts_with("Observation:"));
    }
}
