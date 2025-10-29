use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use serde::{Deserialize, Serialize};
use serde_toon::{from_str, to_string};

#[derive(Serialize, Deserialize, Clone)]
struct User {
    id: u32,
    name: String,
    email: String,
    active: bool,
}

#[derive(Serialize, Deserialize, Clone)]
struct Product {
    sku: String,
    name: String,
    price: f64,
    quantity: u32,
}

#[derive(Serialize, Deserialize, Clone)]
struct NestedData {
    id: u32,
    metadata: Metadata,
    tags: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone)]
struct Metadata {
    created: String,
    updated: String,
    version: u32,
}

fn benchmark_serialize_simple(c: &mut Criterion) {
    let user = User {
        id: 123,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
        active: true,
    };

    c.bench_function("serialize_simple_struct", |b| {
        b.iter(|| to_string(black_box(&user)))
    });
}

fn benchmark_deserialize_simple(c: &mut Criterion) {
    let toon = "active: true\nemail: alice@example.com\nid: 123\nname: Alice";

    c.bench_function("deserialize_simple_struct", |b| {
        b.iter(|| from_str::<User>(black_box(toon)))
    });
}

fn benchmark_serialize_array(c: &mut Criterion) {
    let mut group = c.benchmark_group("serialize_array");

    for size in [10, 50, 100, 500].iter() {
        let products: Vec<Product> = (0..*size)
            .map(|i| Product {
                sku: format!("SKU{}", i),
                name: format!("Product {}", i),
                price: 9.99 + f64::from(i),
                quantity: i,
            })
            .collect();

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| to_string(black_box(&products)))
        });
    }
    group.finish();
}

fn benchmark_deserialize_array(c: &mut Criterion) {
    let mut group = c.benchmark_group("deserialize_array");

    for size in [10, 50, 100, 500].iter() {
        let products: Vec<Product> = (0..*size)
            .map(|i| Product {
                sku: format!("SKU{}", i),
                name: format!("Product {}", i),
                price: 9.99 + f64::from(i),
                quantity: i,
            })
            .collect();
        let toon = to_string(&products).unwrap();

        group.bench_with_input(BenchmarkId::from_parameter(size), &toon, |b, toon| {
            b.iter(|| from_str::<Vec<Product>>(black_box(toon)))
        });
    }
    group.finish();
}

fn benchmark_serialize_nested(c: &mut Criterion) {
    let data = NestedData {
        id: 42,
        metadata: Metadata {
            created: "2023-01-01T00:00:00Z".to_string(),
            updated: "2023-12-31T23:59:59Z".to_string(),
            version: 3,
        },
        tags: vec![
            "important".to_string(),
            "verified".to_string(),
            "production".to_string(),
        ],
    };

    c.bench_function("serialize_nested_struct", |b| {
        b.iter(|| to_string(black_box(&data)))
    });
}

fn benchmark_deserialize_nested(c: &mut Criterion) {
    let data = NestedData {
        id: 42,
        metadata: Metadata {
            created: "2023-01-01T00:00:00Z".to_string(),
            updated: "2023-12-31T23:59:59Z".to_string(),
            version: 3,
        },
        tags: vec![
            "important".to_string(),
            "verified".to_string(),
            "production".to_string(),
        ],
    };
    let toon = to_string(&data).unwrap();

    c.bench_function("deserialize_nested_struct", |b| {
        b.iter(|| from_str::<NestedData>(black_box(&toon)))
    });
}

fn benchmark_string_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("serialize_strings");

    let short = "short";
    let medium = "This is a medium length string with some content";
    let long = "This is a very long string that contains a lot of text and might require more processing time";

    group.bench_function("short_string", |b| b.iter(|| to_string(black_box(&short))));

    group.bench_function("medium_string", |b| {
        b.iter(|| to_string(black_box(&medium)))
    });

    group.bench_function("long_string", |b| b.iter(|| to_string(black_box(&long))));

    group.finish();
}

fn benchmark_primitive_array(c: &mut Criterion) {
    let mut group = c.benchmark_group("primitive_array");

    let numbers: Vec<i32> = (0..100).collect();
    let bools: Vec<bool> = (0..100).map(|i| i % 2 == 0).collect();
    let floats: Vec<f64> = (0..100).map(|i| i as f64 * 1.5).collect();

    group.bench_function("serialize_integers", |b| {
        b.iter(|| to_string(black_box(&numbers)))
    });

    group.bench_function("serialize_booleans", |b| {
        b.iter(|| to_string(black_box(&bools)))
    });

    group.bench_function("serialize_floats", |b| {
        b.iter(|| to_string(black_box(&floats)))
    });

    let numbers_toon = to_string(&numbers).unwrap();
    let bools_toon = to_string(&bools).unwrap();
    let floats_toon = to_string(&floats).unwrap();

    group.bench_function("deserialize_integers", |b| {
        b.iter(|| from_str::<Vec<i32>>(black_box(&numbers_toon)))
    });

    group.bench_function("deserialize_booleans", |b| {
        b.iter(|| from_str::<Vec<bool>>(black_box(&bools_toon)))
    });

    group.bench_function("deserialize_floats", |b| {
        b.iter(|| from_str::<Vec<f64>>(black_box(&floats_toon)))
    });

    group.finish();
}

fn benchmark_comparison_with_json(c: &mut Criterion) {
    let user = User {
        id: 123,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
        active: true,
    };

    let mut group = c.benchmark_group("comparison");

    group.bench_function("toon_serialize", |b| {
        b.iter(|| serde_toon::to_string(black_box(&user)))
    });

    group.bench_function("json_serialize", |b| {
        b.iter(|| serde_json::to_string(black_box(&user)))
    });

    let toon_str = serde_toon::to_string(&user).unwrap();
    let json_str = serde_json::to_string(&user).unwrap();

    group.bench_function("toon_deserialize", |b| {
        b.iter(|| serde_toon::from_str::<User>(black_box(&toon_str)))
    });

    group.bench_function("json_deserialize", |b| {
        b.iter(|| serde_json::from_str::<User>(black_box(&json_str)))
    });

    group.finish();
}

fn benchmark_roundtrip(c: &mut Criterion) {
    let user = User {
        id: 123,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
        active: true,
    };

    c.bench_function("roundtrip_simple", |b| {
        b.iter(|| {
            let serialized = to_string(black_box(&user)).unwrap();
            let _deserialized: User = from_str(black_box(&serialized)).unwrap();
        })
    });
}

criterion_group!(
    benches,
    benchmark_serialize_simple,
    benchmark_deserialize_simple,
    benchmark_serialize_array,
    benchmark_deserialize_array,
    benchmark_serialize_nested,
    benchmark_deserialize_nested,
    benchmark_string_serialization,
    benchmark_primitive_array,
    benchmark_comparison_with_json,
    benchmark_roundtrip
);
criterion_main!(benches);
