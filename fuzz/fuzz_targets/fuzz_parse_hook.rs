#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        // Hook YAML deserialization must never panic on arbitrary input.
        if let Ok(hook) = serde_yaml::from_str::<looprs::hooks::Hook>(s) {
            // Invariant: a successfully parsed hook re-serializes without error
            // and reports the same action count.
            let actions_before = hook.actions.len();
            let reserialized = serde_yaml::to_string(&hook).expect("parsed hook must re-serialize");
            let reparsed: looprs::hooks::Hook =
                serde_yaml::from_str(&reserialized).expect("re-serialized hook must re-parse");
            assert_eq!(
                reparsed.actions.len(),
                actions_before,
                "action count must survive a serialize/parse round trip"
            );
        }
    }
});
