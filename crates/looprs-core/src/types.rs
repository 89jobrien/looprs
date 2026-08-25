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
    /// The default Anthropic model.
    ///
    /// Despite the name this returns `claude-sonnet-4-6`, not an Opus model;
    /// the name is retained for backward compatibility with existing callers.
    pub fn claude_opus() -> Self {
        Self::new("claude-sonnet-4-6")
    }

    /// The default small OpenAI model, `gpt-5-mini`.
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

    /// Best-effort context window for this model, in tokens.
    ///
    /// Matching is substring-based on the lowercased model ID, so unknown or
    /// newly released models fall back to a conservative 100,000 tokens
    /// rather than failing.
    pub fn max_tokens(&self) -> u32 {
        let model = self.0.to_lowercase();
        match model.as_str() {
            // Specific gpt-4 variants must be checked before the generic
            // `gpt-4` substring match, otherwise they are unreachable.
            m if m.contains("gpt-4-turbo") || m.contains("gpt-4-1106") => TOKENS_GPT4_TURBO,
            m if m.contains("gpt-4-32k") => TOKENS_GPT4_32K,
            m if m.contains("gpt-4") => TOKENS_GPT4_BASE,
            m if m.contains("gpt-5") => TOKENS_GPT5,
            m if m.contains("claude-3") || m.contains("claude-opus") => TOKENS_CLAUDE,
            m if m.contains("claude") => TOKENS_CLAUDE,
            m if m.contains("anthropic") => TOKENS_CLAUDE,
            m if m.contains("openai") => TOKENS_DEFAULT,
            _ => TOKENS_DEFAULT,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructor_values_are_stable() {
        assert_eq!(ModelId::claude_opus().as_str(), "claude-sonnet-4-6");
        assert_eq!(ModelId::gpt_5_mini().as_str(), "gpt-5-mini");
    }

    #[test]
    fn max_tokens_table() {
        let cases: &[(&str, u32)] = &[
            ("gpt-4", TOKENS_GPT4_BASE),
            ("GPT-4o", TOKENS_GPT4_BASE),
            ("gpt-4-turbo-preview", TOKENS_GPT4_TURBO),
            ("gpt-4-1106-preview", TOKENS_GPT4_TURBO),
            ("gpt-4-32k", TOKENS_GPT4_32K),
            ("gpt-5-mini", TOKENS_GPT5),
            ("gpt-5", TOKENS_GPT5),
            ("claude-3-opus-20240229", TOKENS_CLAUDE),
            ("claude-opus-4-1", TOKENS_CLAUDE),
            ("claude-sonnet-4-6", TOKENS_CLAUDE),
            ("some-anthropic-proxy", TOKENS_CLAUDE),
            ("openai-o1", TOKENS_DEFAULT),
            ("totally-unknown-model", TOKENS_DEFAULT),
        ];
        for (id, expected) in cases {
            assert_eq!(ModelId::new(*id).max_tokens(), *expected, "model: {id}");
        }
    }

    #[test]
    fn estimate_cost_known_models() {
        // claude-sonnet-4: $3/M input, $15/M output
        let sonnet = ModelId::new("claude-sonnet-4-6");
        assert!((sonnet.estimate_cost(1_000_000, 0) - 3.0).abs() < 1e-9);
        assert!((sonnet.estimate_cost(0, 1_000_000) - 15.0).abs() < 1e-9);
        assert!((sonnet.estimate_cost(500_000, 100_000) - (1.5 + 1.5)).abs() < 1e-9);

        // gpt-4o: $2.50/M input, $10/M output
        let gpt4o = ModelId::new("gpt-4o");
        assert!((gpt4o.estimate_cost(2_000_000, 0) - 5.0).abs() < 1e-9);

        // haiku: $0.80/M input
        let haiku = ModelId::new("claude-haiku-4-5");
        assert!((haiku.estimate_cost(1_000_000, 0) - 0.8).abs() < 1e-9);
    }

    #[test]
    fn estimate_cost_unknown_model_is_zero() {
        assert_eq!(
            ModelId::new("mystery-model").estimate_cost(1_000_000, 1_000_000),
            0.0
        );
    }

    #[test]
    fn estimate_cost_zero_tokens_is_zero() {
        assert_eq!(ModelId::claude_opus().estimate_cost(0, 0), 0.0);
    }

    #[test]
    fn estimate_cost_matching_is_case_insensitive_and_substring_based() {
        let upper = ModelId::new("CLAUDE-HAIKU-4-5");
        assert!(upper.estimate_cost(1_000_000, 0) > 0.0);
    }

    #[test]
    fn newtype_ids_compare_by_value() {
        use std::collections::HashSet;

        let a = ToolId::new("tu-1");
        let b = ToolId::new("tu-1");
        let c = ToolId::new("tu-2");

        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.clone(), a);

        let mut set = HashSet::new();
        set.insert(a.clone());
        set.insert(b);
        assert_eq!(set.len(), 1, "equal ids must hash identically");
    }

    #[test]
    fn newtype_id_display_and_serde_are_transparent() {
        let id = ToolName::new("bash");

        assert_eq!(format!("{id}"), "bash");

        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"bash\"", "serde must be transparent over String");

        let back: ToolName = serde_json::from_str(&json).unwrap();
        assert_eq!(back, id);
    }
}

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    #[kani::proof]
    fn gpt4o_estimate_cost_is_non_negative() {
        let input_tokens: u32 = kani::any();
        let output_tokens: u32 = kani::any();

        let model = ModelId::new("gpt-4o");
        let cost = model.estimate_cost(input_tokens, output_tokens);
        assert!(cost >= 0.0);
    }

    #[kani::proof]
    fn gpt4o_estimate_cost_respects_bounded_upper_limit() {
        let input_tokens: u32 = kani::any();
        let output_tokens: u32 = kani::any();
        kani::assume(input_tokens <= 1_000_000);
        kani::assume(output_tokens <= 1_000_000);

        let model = ModelId::new("gpt-4o");
        let cost = model.estimate_cost(input_tokens, output_tokens);

        // gpt-4o pricing in this table is $2.5/M input + $10/M output.
        assert!(cost <= 12.5);
    }
}
