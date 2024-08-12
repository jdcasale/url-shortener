use criterion::{criterion_group, criterion_main, Criterion};


fn pop_vec_benchmark(c: &mut Criterion) {
    let mut list: Vec<i32> = Vec::new();
    for i in 0..1000 {
        list.push(i);
    }
    c.bench_function("pop vec 1000", |b| b.iter(|| {
        for _ in 0..1000 {
            list.pop();
        }
    }));
}

criterion_group!(benches, pop_vec_benchmark);
criterion_main!(benches);