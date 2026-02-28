use criterion::{black_box, criterion_group, criterion_main, Criterion};
use looprs_desktop::services::generative_ui::*;
use serde_json::json;

fn bench_json_merge(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_merge");

    group.bench_function("small_objects", |b| {
        let mut base = json!({"a": 1, "b": 2});
        let patch = json!({"c": 3});

        b.iter(|| {
            let mut base_copy = base.clone();
            merge_json_in_place(black_box(&mut base_copy), black_box(patch.clone()));
        });
    });

    group.bench_function("deep_nesting", |b| {
        let mut base = json!({"a": {"b": {"c": {"d": {"e": 1}}}}});
        let patch = json!({"a": {"b": {"c": {"d": {"f": 2}}}}});

        b.iter(|| {
            let mut base_copy = base.clone();
            merge_json_in_place(black_box(&mut base_copy), black_box(patch.clone()));
        });
    });

    group.bench_function("wide_objects", |b| {
        let mut base = json!({
            "a": 1, "b": 2, "c": 3, "d": 4, "e": 5,
            "f": 6, "g": 7, "h": 8, "i": 9, "j": 10
        });
        let patch = json!({
            "k": 11, "l": 12, "m": 13, "n": 14, "o": 15
        });

        b.iter(|| {
            let mut base_copy = base.clone();
            merge_json_in_place(black_box(&mut base_copy), black_box(patch.clone()));
        });
    });

    group.finish();
}

fn bench_string_truncation(c: &mut Criterion) {
    let mut group = c.benchmark_group("truncate_string");

    let long_string = "x".repeat(10000);

    group.bench_function("ascii_truncate_1000", |b| {
        b.iter(|| {
            truncate_string(black_box(&long_string), black_box(1000));
        });
    });

    group.bench_function("ascii_truncate_100", |b| {
        b.iter(|| {
            truncate_string(black_box(&long_string), black_box(100));
        });
    });

    let emoji_string = "🦀".repeat(1000);

    group.bench_function("unicode_truncate_500", |b| {
        b.iter(|| {
            truncate_string(black_box(&emoji_string), black_box(500));
        });
    });

    group.bench_function("no_truncation_needed", |b| {
        let short = "hello world";
        b.iter(|| {
            truncate_string(black_box(short), black_box(100));
        });
    });

    group.finish();
}

fn bench_parse_functions(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_functions");

    group.bench_function("parse_rgb_valid", |b| {
        b.iter(|| {
            parse_rgb(black_box("rgb(255, 128, 64)"));
        });
    });

    group.bench_function("parse_rgb_invalid", |b| {
        b.iter(|| {
            parse_rgb(black_box("invalid"));
        });
    });

    group.bench_function("parse_size_fill", |b| {
        b.iter(|| {
            parse_size(black_box("fill"));
        });
    });

    group.bench_function("parse_size_percent", |b| {
        b.iter(|| {
            parse_size(black_box("75%"));
        });
    });

    group.bench_function("parse_size_px", |b| {
        b.iter(|| {
            parse_size(black_box("250"));
        });
    });

    group.finish();
}

fn bench_escape_rsx_string(c: &mut Criterion) {
    let mut group = c.benchmark_group("escape_rsx_string");

    group.bench_function("no_escaping_needed", |b| {
        let simple = "simple text without special chars";
        b.iter(|| {
            escape_rsx_string(black_box(simple));
        });
    });

    group.bench_function("with_quotes", |b| {
        let quoted = r#"Text with "quotes" and 'apostrophes'"#;
        b.iter(|| {
            escape_rsx_string(black_box(quoted));
        });
    });

    group.bench_function("with_newlines", |b| {
        let newlines = "Line 1\nLine 2\nLine 3\n";
        b.iter(|| {
            escape_rsx_string(black_box(newlines));
        });
    });

    group.bench_function("all_special_chars", |b| {
        let complex = "Text with\n\"quotes\"\nand\\backslashes\r\n";
        b.iter(|| {
            escape_rsx_string(black_box(complex));
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_json_merge,
    bench_string_truncation,
    bench_parse_functions,
    bench_escape_rsx_string
);
criterion_main!(benches);
