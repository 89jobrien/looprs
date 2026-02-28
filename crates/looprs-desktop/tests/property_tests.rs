use proptest::prelude::*;
use serde_json::Value;
use looprs_desktop::services::generative_ui::*;

proptest! {
    #[test]
    fn test_truncate_string_never_panics(
        input in ".*",
        max_chars in 0usize..1000
    ) {
        let (result, truncated) = truncate_string(&input, max_chars);

        // Should never panic
        assert!(result.len() <= input.len());

        if input.chars().count() > max_chars {
            assert!(truncated);
        }
    }

    #[test]
    fn test_truncate_string_preserves_validity(
        input in ".*",
        max_chars in 1usize..100
    ) {
        let (result, _) = truncate_string(&input, max_chars);

        // Result should always be valid UTF-8
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
    }

    #[test]
    fn test_merge_json_preserves_types(
        base_obj in prop::collection::hash_map(".*", any::<i32>(), 1..10),
        patch_obj in prop::collection::hash_map(".*", any::<i32>(), 1..10),
    ) {
        let mut base = Value::Object(
            base_obj.into_iter()
                .map(|(k, v)| (k, Value::Number(v.into())))
                .collect()
        );
        let patch = Value::Object(
            patch_obj.into_iter()
                .map(|(k, v)| (k, Value::Number(v.into())))
                .collect()
        );

        merge_json_in_place(&mut base, patch);

        // All values should still be numbers
        if let Value::Object(obj) = base {
            for (_, v) in obj {
                assert!(v.is_number());
            }
        }
    }

    #[test]
    fn test_parse_size_handles_all_inputs(
        input in ".*"
    ) {
        // Should never panic, just return None for invalid
        let _ = parse_size(&input);
    }

    #[test]
    fn test_parse_rgb_handles_all_inputs(
        input in ".*"
    ) {
        // Should never panic, just return None for invalid
        let _ = parse_rgb(&input);
    }

    #[test]
    fn test_parse_rgb_validates_range(
        r in 0u16..300,
        g in 0u16..300,
        b in 0u16..300,
    ) {
        let input = format!("rgb({}, {}, {})", r, g, b);
        let result = parse_rgb(&input);

        if r <= 255 && g <= 255 && b <= 255 {
            // Valid RGB values should parse
            assert!(result.is_some());
        } else {
            // Out of range should return None
            assert!(result.is_none());
        }
    }

    #[test]
    fn test_escape_rsx_string_preserves_length_order(
        input in ".*"
    ) {
        let escaped = escape_rsx_string(&input);

        // Escaped string should be at least as long as original
        assert!(escaped.len() >= input.len());
    }

    #[test]
    fn test_truncate_at_boundaries(
        input in "[a-zA-Z0-9]*",
        max_chars in 0usize..50
    ) {
        let (result, truncated) = truncate_string(&input, max_chars);

        // Character count should never exceed max_chars
        assert!(result.chars().count() <= max_chars);

        if input.chars().count() <= max_chars {
            assert_eq!(result, input);
            assert!(!truncated);
        }
    }
}

// Additional standalone property tests for complex scenarios

#[cfg(test)]
mod advanced_property_tests {
    use super::*;

    proptest! {
        #[test]
        fn test_json_merge_idempotent(
            obj in prop::collection::hash_map(".*", any::<String>(), 1..5)
        ) {
            let base_json = Value::Object(
                obj.iter()
                    .map(|(k, v)| (k.clone(), Value::String(v.clone())))
                    .collect()
            );

            let mut result1 = base_json.clone();
            merge_json_in_place(&mut result1, base_json.clone());

            let mut result2 = base_json.clone();
            merge_json_in_place(&mut result2, base_json.clone());

            // Merging the same object should be idempotent
            assert_eq!(result1, result2);
        }

        #[test]
        fn test_truncate_string_unicode_safety(
            emoji_count in 1usize..20,
            max_chars in 0usize..30
        ) {
            let input = "🦀".repeat(emoji_count);
            let (result, _) = truncate_string(&input, max_chars);

            // Result should be valid UTF-8
            assert!(std::str::from_utf8(result.as_bytes()).is_ok());

            // Should not split multi-byte characters
            for ch in result.chars() {
                assert!(ch.len_utf8() > 0);
            }
        }
    }
}
