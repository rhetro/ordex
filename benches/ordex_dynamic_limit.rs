use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
use ordex::prelude::*;
use rand::seq::SliceRandom;
use rand::thread_rng;

fn bench_ordex_dynamic_limit(c: &mut Criterion) {
    let mut group = c.benchmark_group("ordex_dynamic_limit");

    let mut arena = OrdexArena::new();
    let mut handles = Vec::with_capacity(10_000_000);
    for i in 0..10_000_000 {
        handles.push(arena.insert(i as u64));
    }

    for &n in &[1000, 2000, 5000, 10000, 20000] {
        group.bench_function(format!("ordex_dynamic_N={}", n), |b| {
            b.iter_batched(
                || {
                    let mut rng = thread_rng();
                    handles
                        .choose_multiple(&mut rng, n)
                        .cloned()
                        .collect::<Vec<_>>()
                },
                |sample_handles| {
                    let verified = VerifiedIndices::new(sample_handles);
                    arena.ordex(&verified, |mut iter| {
                        while let Some(item) = iter.next() {
                            black_box(item);
                        }
                    });
                },
                BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

criterion_group!(benches, bench_ordex_dynamic_limit);
criterion_main!(benches);
