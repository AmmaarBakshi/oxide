use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_startup(c: &mut Criterion) {
    c.bench_function("shell_startup_mock", |b| {
        // b.iter is the actual loop that measures performance
        b.iter(|| {
            // black_box prevents the compiler from optimizing this away
            let prompt = black_box(String::from("oxide > "));
            black_box(prompt)
        })
    });
}

// Register the benchmark
criterion_group!(benches, bench_startup);
criterion_main!(benches);