use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ordex::prelude::*;

fn bench_iter_mut_10k(c: &mut Criterion) {
    let mut arena = OrdexArena::new();
    for i in 0..10_000 {
        arena.insert(i);
    }

    c.bench_function("arena_iter_mut_10k", |b| {
        b.iter(|| {
            for (_idx, val) in arena.iter_mut() {
                *val = black_box(*val + 1);
            }
        })
    });
}

fn bench_alignment_comparison(c: &mut Criterion) {
    let mut arena = OrdexArena::new();
    let indices: Vec<_> = (0..16).map(|i| arena.insert(i)).collect();

    // 1. align! (Static O(N^2) unrolled)
    c.bench_function("ordex_align_16", |b| {
        b.iter(|| {
            align!(
                arena,
                indices[0],
                indices[1],
                indices[2],
                indices[3],
                indices[4],
                indices[5],
                indices[6],
                indices[7],
                indices[8],
                indices[9],
                indices[10],
                indices[11],
                indices[12],
                indices[13],
                indices[14],
                indices[15],
                |i0, i1, i2, i3, i4, i5, i6, i7, i8, i9, i10, i11, i12, i13, i14, i15| {
                    let sum = *i0
                        + *i1
                        + *i2
                        + *i3
                        + *i4
                        + *i5
                        + *i6
                        + *i7
                        + *i8
                        + *i9
                        + *i10
                        + *i11
                        + *i12
                        + *i13
                        + *i14
                        + *i15;
                    *i0 = black_box(sum);
                }
            );
        })
    });

    // 2. ordex (Dynamic O(N log N) sorting)
    let verified = VerifiedIndices::new(indices.clone());
    c.bench_function("ordex_dynamic_16", |b| {
        b.iter(|| {
            arena.ordex(&verified, |mut iter| {
                let mut sum = 0;
                while let Some(val) = iter.next() {
                    sum += *val;
                    *val = black_box(sum);
                }
            });
        })
    });
}

criterion_group!(benches, bench_iter_mut_10k, bench_alignment_comparison);
criterion_main!(benches);
