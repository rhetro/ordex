use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ordex::prelude::*;

fn bench_freelist_limit(c: &mut Criterion) {
    let mut group = c.benchmark_group("ordex_freelist_limit");

    let mut arena = OrdexArena::new();
    let mut handles = Vec::with_capacity(1_000_000);

    for i in 0..1_000_000 {
        handles.push(arena.insert(i as u64));
    }

    group.bench_function("freelist_shallow_cycle", |b| {
        b.iter(|| {
            // pop → remove → insert → push
            let idx = handles.pop().unwrap();
            let _ = arena.remove(idx);
            let new_idx = arena.insert(black_box(123u64));
            handles.push(new_idx);
        });
    });

    group.bench_function("freelist_deep_chain", |b| {
        for h in handles.drain(..) {
            let _ = arena.remove(h);
        }

        b.iter(|| {
            let idx = arena.insert(black_box(999u64));
            let _ = arena.remove(idx);
        });
    });

    group.finish();
}

criterion_group!(benches, bench_freelist_limit);
criterion_main!(benches);
