#![no_main]
use libfuzzer_sys::fuzz_target;
use std::path::PathBuf;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let path = PathBuf::from("/fuzz/SKILL.md");
        // parse_skill_file must never panic on arbitrary input.
        let _ = looprs::skills::parser::parse_skill_file(&path, s);
    }
});
