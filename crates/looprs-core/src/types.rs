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
        Self::new("claude-sonnet-4-6")
    }

    pub fn gpt_5_mini() -> Self {
        Self::new("gpt-5-mini")
    }

    /// Approximate cost in USD for the given token counts.
    /// Prices per million tokens (input, output). Returns 0.0 for unknown models.
    pub fn estimate_cost(&self, input_tokens: u32, output_tokens: u32) -> f64 {
        let m = self.0.to_lowercase();
        let (input_pm, output_pm): (f64, f64) = if m.contains("claude-opus-4") {
            (15.0, 75.0)
        } else if m.contains("claude-sonnet-4") {
            (3.0, 15.0)
        } else if m.contains("claude-haiku-4") {
            (0.8, 4.0)
        } else if m.contains("gpt-5-mini") {
            (1.1, 4.4)
        } else if m.starts_with("gpt-4o") {
            (2.5, 10.0)
        } else if m.starts_with("gemini-2.0-flash") {
            (0.1, 0.4)
        } else if m.starts_with("gemini-2.5-pro") {
            (1.25, 10.0)
        } else {
            return 0.0;
        };
        (input_tokens as f64 / 1_000_000.0) * input_pm
            + (output_tokens as f64 / 1_000_000.0) * output_pm
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
