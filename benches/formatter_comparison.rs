use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use jk::Formatter;

fn generate_large_json(target_mb: usize) -> String {
    let mut json = String::with_capacity(target_mb * 1024 * 1024);
    json.push('[');

    let mut current_size = 0;
    let target_size = target_mb * 1024 * 1024;
    let mut user_id = 0;

    while current_size < target_size {
        if user_id > 0 {
            json.push(',');
        }

        let user = format!(
            r#"{{"id":{},"name":"User {}","email":"user{}@example.com","age":{},"active":{},"score":{}.{},"address":{{"street":"Street {}","city":"City {}","zip":"{}","country":"Country"}},"tags":["tag1","tag2","tag3"],"metadata":{{"created":"2024-01-01","updated":"2024-11-05","version":1}}}}"#,
            user_id,
            user_id,
            user_id,
            20 + (user_id % 60),
            user_id % 2 == 0,
            user_id % 100,
            user_id % 1000,
            user_id % 1000,
            user_id % 100,
            10000 + user_id,
        );

        current_size += user.len();
        json.push_str(&user);
        user_id += 1;
    }

    json.push(']');
    json
}

fn format_to_string(input: &str) -> String {
    let mut bytes = Vec::new();
    Formatter::new(input).format_to(&mut bytes).unwrap();
    String::from_utf8(bytes).unwrap()
}

fn format_to_string_serde_transmute(input: &str) -> String {
    use serde_json::ser::PrettyFormatter;
    use serde_json::{self, Deserializer};

    let mut de = Deserializer::from_str(input);
    let mut out = Vec::with_capacity(input.len() + input.len() / 2);
    let formatter = PrettyFormatter::with_indent(b"  ");
    let mut ser = serde_json::Serializer::with_formatter(&mut out, formatter);

    serde_transcode::transcode(&mut de, &mut ser).unwrap();
    String::from_utf8(out).expect("serde_json only emits UTF-8")
}

fn format_to_string_serde(input: &str) -> String {
    let value = serde_json::from_str::<serde_json::Value>(input).unwrap();
    let mut out = Vec::new();
    serde_json::to_writer_pretty(&mut out, &value).unwrap();
    String::from_utf8(out).expect("serde_json only emits UTF-8")
}

fn format_to_string_simd_json(input: &str) -> String {
    // simd_json requires mutable input for zero-copy parsing
    let mut bytes = input.as_bytes().to_vec();
    let value = simd_json::to_borrowed_value(&mut bytes).unwrap();
    let mut out = Vec::new();
    simd_json::to_writer_pretty(&mut out, &value).unwrap();
    String::from_utf8(out).expect("simd_json only emits UTF-8")
}

fn benchmark_formatters(c: &mut Criterion) {
    let mut test_cases = Vec::new();

    for size_mb in [5, 50, 150] {
        println!("Generating {}MB JSON for benchmarking...", size_mb);
        let json = generate_large_json(size_mb);
        println!("Generated {} MB of JSON", json.len() / (1024 * 1024));
        test_cases.push((format!("generated_{}mb", size_mb), json));
    }

    let mut group = c.benchmark_group("formatter_comparison");

    for (name, content) in &test_cases {
        group.bench_with_input(
            BenchmarkId::new("jk", name),
            content.as_str(),
            |b, input| b.iter(|| format_to_string(black_box(input))),
        );

        group.bench_with_input(
            BenchmarkId::new("serde_json_transmute", name),
            content.as_str(),
            |b, input| b.iter(|| format_to_string_serde_transmute(black_box(input))),
        );

        group.bench_with_input(
            BenchmarkId::new("serde_json", name),
            content.as_str(),
            |b, input| b.iter(|| format_to_string_serde(black_box(input))),
        );

        group.bench_with_input(
            BenchmarkId::new("simd_json", name),
            content.as_str(),
            |b, input| b.iter(|| format_to_string_simd_json(black_box(input))),
        );
    }

    group.finish();
}

criterion_group!(benches, benchmark_formatters);
criterion_main!(benches);
