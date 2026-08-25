#![no_main]
use libfuzzer_sys::fuzz_target;
use looprs_core::api::ContentBlock;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        // Provider responses carry content blocks as untrusted JSON.
        // Parsing arbitrary input into the domain type must never panic.
        if let Ok(blocks) = serde_json::from_str::<Vec<ContentBlock>>(s) {
            // Invariant: whatever parses must round-trip through serde_json
            // without loss or panic.
            let json = serde_json::to_string(&blocks).expect("parsed blocks must re-serialize");
            let back: Vec<ContentBlock> =
                serde_json::from_str(&json).expect("re-serialized blocks must re-parse");
            assert_eq!(back.len(), blocks.len(), "block count must be stable");
        }
    }
});
