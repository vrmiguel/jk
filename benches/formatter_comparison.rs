use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use jk::Formatter;

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
    let mut file_cases = Vec::new();

    if let Ok(content) = std::fs::read_to_string("canada.json") {
        file_cases.push(("5MB", content));
    }

    let mut group = c.benchmark_group("formatter_comparison");

    for (name, content) in &file_cases {
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
