#![no_main]
use libfuzzer_sys::fuzz_target;
use std::path::PathBuf;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let markdown_path = PathBuf::from("/fuzz/SKILL.md");
        let yaml_path = PathBuf::from("/fuzz/skill.yml");
        let _ = looprs::skills::parser::parse_skill_file(&markdown_path, s);
        let _ = looprs::skills::parser::parse_yaml_skill(&yaml_path, s);
    }
});
