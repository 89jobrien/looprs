use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ToolId(String);

impl ToolId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ToolId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ToolName(String);

impl ToolName {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ToolName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ModelId(String);

impl ModelId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn claude_opus() -> Self {
        Self::new("claude-3-opus-20240229")
    }

    pub fn gpt_4_turbo() -> Self {
        Self::new("gpt-4-turbo")
    }

    pub fn gpt_5_mini() -> Self {
        Self::new("gpt-5-mini")
    }

    pub fn max_tokens(&self) -> u32 {
        let model = self.0.to_lowercase();
        match model.as_str() {
            m if m.contains("gpt-4") && !m.contains("turbo") && !m.contains("32k") => 4096,
            m if m.contains("gpt-4-turbo") || m.contains("gpt-4-1106") => 100000,
            m if m.contains("gpt-4-32k") => 30000,
            m if m.contains("gpt-5") => 120000,
            m if m.contains("claude-3") || m.contains("claude-opus") => 190000,
            m if m.contains("claude") => 190000,
            m if m.contains("anthropic") => 190000,
            m if m.contains("openai") => 100000,
            _ => 100000,
        }
    }
}

impl fmt::Display for ModelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
