#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        // strip_ansi must never panic on arbitrary input.
        let result = looprs::sanitize::strip_ansi(s);

        // Invariant: output contains no ESC bytes.
        assert!(
            !result.contains('\x1b'),
            "strip_ansi output still contains ESC"
        );

        // Invariant: idempotent — stripping twice yields same result.
        let second = looprs::sanitize::strip_ansi(&result);
        assert_eq!(result, second, "strip_ansi is not idempotent");
    }
});
