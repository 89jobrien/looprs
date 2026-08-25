pub const MAX_GREP_HITS: usize = 50;
pub const MAX_GLOB_HITS: usize = 1000;
pub const MAX_GLOB_OUTPUT_CHARS: usize = 16_000;

const _: () = {
    assert!(MAX_GREP_HITS > 0);
    assert!(MAX_GLOB_HITS > 0);
    assert!(MAX_GLOB_OUTPUT_CHARS > 0);
    assert!(MAX_GREP_HITS < MAX_GLOB_HITS);
    assert!(MAX_GLOB_OUTPUT_CHARS >= MAX_GLOB_HITS);
};
