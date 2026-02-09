#[allow(dead_code)]
pub const DEFAULT_MODEL: &str = "claude-3-opus-20240229";
pub const MAX_GREP_HITS: usize = 50;

/// Get max tokens for a given model based on its context window
/// Conservative estimate: leaves 1k tokens for safety margin
pub fn get_max_tokens_for_model(model: &str) -> u32 {
    match model.to_lowercase() {
        // GPT-4 models (8k context)
        m if m.contains("gpt-4") && !m.contains("turbo") && !m.contains("32k") => 4096,
        m if m.contains("gpt-4-turbo") || m.contains("gpt-4-1106") => 100000,
        m if m.contains("gpt-4-32k") => 30000,

        // GPT-5 models (128k+ context)
        m if m.contains("gpt-5") => 120000,

        // Claude models (200k context)
        m if m.contains("claude-3") || m.contains("claude-opus") => 190000,
        m if m.contains("claude") => 190000,

        // OpenRouter models (various)
        m if m.contains("anthropic") => 190000,
        m if m.contains("openai") => 100000,

        // Default to 100k for unknown models
        _ => 100000,
    }
}
