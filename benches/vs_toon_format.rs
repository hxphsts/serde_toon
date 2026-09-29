//! Head-to-head comparison with `toon-format`, the official Rust TOON crate.
//!
//! Both crates encode and decode identical data with default options, so the
//! numbers are comparable. Run with `cargo bench --bench vs_toon_format`.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
struct Employee {
    id: u64,
    name: String,
    department: String,
    salary: f64,
    active: bool,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
struct Team {
    name: String,
    lead: Employee,
    tags: Vec<String>,
    members: Vec<Employee>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
struct Org {
    name: String,
    teams: Vec<Team>,
}

fn employees(n: usize) -> Vec<Employee> {
    (0..n)
        .map(|i| Employee {
            id: i as u64,
            name: format!("Employee {i}"),
            department: ["Engineering", "Sales", "Support"][i % 3].to_string(),
            salary: 50_000.0 + (i as f64) * 12.5,
            active: i % 5 != 0,
        })
        .collect()
}

fn org() -> Org {
    Org {
        name: "Acme".into(),
        teams: (0..20)
            .map(|t| Team {
                name: format!("team-{t}"),
                lead: employees(1).remove(0),
                tags: vec!["core".into(), "on-call, weekends".into(), format!("t{t}")],
                members: employees(25),
            })
            .collect(),
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
struct Wrapper<T> {
    items: T,
}

fn bench_tabular(c: &mut Criterion) {
    let mut g = c.benchmark_group("tabular");
    for n in [10usize, 100, 1_000, 10_000] {
        let data = Wrapper {
            items: employees(n),
        };
        let ours = serde_toon::to_string(&data).unwrap();
        let theirs = toon_format::encode_default(&data).unwrap();
        assert_eq!(ours, theirs, "outputs should match for the tabular case");
        g.throughput(Throughput::Bytes(ours.len() as u64));

        g.bench_with_input(BenchmarkId::new("encode/serde_toon", n), &data, |b, d| {
            b.iter(|| serde_toon::to_string(black_box(d)).unwrap())
        });
        g.bench_with_input(BenchmarkId::new("encode/toon_format", n), &data, |b, d| {
            b.iter(|| toon_format::encode_default(black_box(d)).unwrap())
        });
        g.bench_with_input(BenchmarkId::new("decode/serde_toon", n), &ours, |b, s| {
            b.iter(|| serde_toon::from_str::<Wrapper<Vec<Employee>>>(black_box(s)).unwrap())
        });
        g.bench_with_input(BenchmarkId::new("decode/toon_format", n), &ours, |b, s| {
            b.iter(|| toon_format::decode_default::<Wrapper<Vec<Employee>>>(black_box(s)).unwrap())
        });
    }
    g.finish();
}

fn bench_nested(c: &mut Criterion) {
    let mut g = c.benchmark_group("nested");
    let data = org();
    let ours = serde_toon::to_string(&data).unwrap();
    let theirs = toon_format::encode_default(&data).unwrap();
    g.throughput(Throughput::Bytes(ours.len() as u64));

    g.bench_function("encode/serde_toon", |b| {
        b.iter(|| serde_toon::to_string(black_box(&data)).unwrap())
    });
    g.bench_function("encode/toon_format", |b| {
        b.iter(|| toon_format::encode_default(black_box(&data)).unwrap())
    });
    // Each decoder reads its own encoder's output, in case the two crates'
    // spec versions lay out nested list items differently.
    g.bench_function("decode/serde_toon", |b| {
        b.iter(|| serde_toon::from_str::<Org>(black_box(&ours)).unwrap())
    });
    g.bench_function("decode/toon_format", |b| {
        b.iter(|| toon_format::decode_default::<Org>(black_box(&theirs)).unwrap())
    });
    g.finish();
}

fn bench_strings(c: &mut Criterion) {
    let mut g = c.benchmark_group("strings");
    let data = Wrapper {
        items: (0..1_000)
            .map(|i| format!("line {i}: \"quoted\", with commas, tabs\tand unicode — ✓ {i}"))
            .collect::<Vec<_>>(),
    };
    let ours = serde_toon::to_string(&data).unwrap();
    let theirs = toon_format::encode_default(&data).unwrap();
    g.throughput(Throughput::Bytes(ours.len() as u64));

    g.bench_function("encode/serde_toon", |b| {
        b.iter(|| serde_toon::to_string(black_box(&data)).unwrap())
    });
    g.bench_function("encode/toon_format", |b| {
        b.iter(|| toon_format::encode_default(black_box(&data)).unwrap())
    });
    g.bench_function("decode/serde_toon", |b| {
        b.iter(|| serde_toon::from_str::<Wrapper<Vec<String>>>(black_box(&ours)).unwrap())
    });
    g.bench_function("decode/toon_format", |b| {
        b.iter(|| toon_format::decode_default::<Wrapper<Vec<String>>>(black_box(&theirs)).unwrap())
    });
    g.finish();
}

criterion_group!(benches, bench_tabular, bench_nested, bench_strings);
criterion_main!(benches);
