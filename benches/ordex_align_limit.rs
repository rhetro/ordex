use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
use ordex::prelude::*;
use rand::seq::SliceRandom;
use rand::thread_rng;

fn bench_align16_limit(c: &mut Criterion) {
    let mut group = c.benchmark_group("ordex_align16_limit");

    let mut arena = OrdexArena::new();
    let mut handles = Vec::with_capacity(10_000_000);
    for i in 0..10_000_000 {
        handles.push(arena.insert(i as u64));
    }

    group.bench_function("align16_random_extreme", |b| {
        b.iter_batched(
            || {
                let mut rng = thread_rng();
                let sample: Vec<_> = handles.choose_multiple(&mut rng, 16).cloned().collect();
                (
                    sample[0], sample[1], sample[2], sample[3], sample[4], sample[5], sample[6],
                    sample[7], sample[8], sample[9], sample[10], sample[11], sample[12],
                    sample[13], sample[14], sample[15],
                )
            },
            |(h0, h1, h2, h3, h4, h5, h6, h7, h8, h9, h10, h11, h12, h13, h14, h15)| {
                align!(
                    arena,
                    h0,
                    h1,
                    h2,
                    h3,
                    h4,
                    h5,
                    h6,
                    h7,
                    h8,
                    h9,
                    h10,
                    h11,
                    h12,
                    h13,
                    h14,
                    h15,
                    |r0, r1, r2, r3, r4, r5, r6, r7, r8, r9, r10, r11, r12, r13, r14, r15| {
                        black_box(r0);
                        black_box(r1);
                        black_box(r2);
                        black_box(r3);
                        black_box(r4);
                        black_box(r5);
                        black_box(r6);
                        black_box(r7);
                        black_box(r8);
                        black_box(r9);
                        black_box(r10);
                        black_box(r11);
                        black_box(r12);
                        black_box(r13);
                        black_box(r14);
                        black_box(r15);
                    }
                );
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

criterion_group!(benches, bench_align16_limit);
criterion_main!(benches);
