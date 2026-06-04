newtype_id!(ToolId);
newtype_id!(ToolName);
newtype_id!(ModelId);

// Token limits by model family.
const TOKENS_GPT4_BASE: u32 = 4_096;
const TOKENS_GPT4_32K: u32 = 30_000;
const TOKENS_GPT4_TURBO: u32 = 100_000;
const TOKENS_GPT5: u32 = 120_000;
const TOKENS_CLAUDE: u32 = 190_000;
const TOKENS_DEFAULT: u32 = 100_000;

impl ModelId {
    pub fn claude_opus() -> Self {
        Self::new("claude-3-opus-20240229")
    }

    pub fn gpt_5_mini() -> Self {
        Self::new("gpt-5-mini")
    }

    pub fn max_tokens(&self) -> u32 {
        let model = self.0.to_lowercase();
        match model.as_str() {
            m if m.contains("gpt-4") && !m.contains("turbo") && !m.contains("32k") => {
                TOKENS_GPT4_BASE
            }
            m if m.contains("gpt-4-turbo") || m.contains("gpt-4-1106") => TOKENS_GPT4_TURBO,
            m if m.contains("gpt-4-32k") => TOKENS_GPT4_32K,
            m if m.contains("gpt-5") => TOKENS_GPT5,
            m if m.contains("claude-3") || m.contains("claude-opus") => TOKENS_CLAUDE,
            m if m.contains("claude") => TOKENS_CLAUDE,
            m if m.contains("anthropic") => TOKENS_CLAUDE,
            m if m.contains("openai") => TOKENS_DEFAULT,
            _ => TOKENS_DEFAULT,
        }
    }
}
