use ordex::prelude::*;

/// Data structure containing heap allocations.
/// Used to verify memory leak detection and Drop soundness under Miri.
#[derive(Debug, PartialEq, Clone)]
struct TrackedPayload {
    id: usize,
    label: String,
    history: Vec<u64>,
}

impl TrackedPayload {
    fn new(id: usize) -> Self {
        Self {
            id,
            label: format!("node_{}", id),
            history: vec![id as u64],
        }
    }
}

/// 1. Stacked Borrows / Tree Borrows verification for `align!` (up to 16 elements)
#[test]
fn miri_align16_aliasing_and_drop_soundness() {
    let mut arena = OrdexArena::new();
    let handles: Vec<_> = (0..16)
        .map(|i| arena.insert(TrackedPayload::new(i)))
        .collect();

    align!(
        arena,
        handles[0],
        handles[1],
        handles[2],
        handles[3],
        handles[4],
        handles[5],
        handles[6],
        handles[7],
        handles[8],
        handles[9],
        handles[10],
        handles[11],
        handles[12],
        handles[13],
        handles[14],
        handles[15],
        |r0, r1, r2, r3, r4, r5, r6, r7, r8, r9, r10, r11, r12, r13, r14, r15| {
            let refs = [
                r0, r1, r2, r3, r4, r5, r6, r7, r8, r9, r10, r11, r12, r13, r14, r15,
            ];
            for (idx, item) in refs.into_iter().enumerate() {
                item.label.push_str("_aligned");
                item.history.push((idx * 10) as u64);
            }
        }
    );

    for (i, &h) in handles.iter().enumerate() {
        let item = arena.get(h).unwrap();
        assert_eq!(item.label, format!("node_{}_aligned", i));
        assert_eq!(item.history, vec![i as u64, (i * 10) as u64]);
    }
}

/// 2. Verification of the L1/L2 cache-bound `arsort` path (`N <= MACRO_PARTITION_THRESHOLD`) and `OrdexIter`
#[test]
fn miri_ordex_small_path_soundness() {
    let total = if cfg!(miri) { 48 } else { 256 };
    let mut arena = OrdexArena::new();
    let handles: Vec<_> = (0..total)
        .map(|i| arena.insert(TrackedPayload::new(i)))
        .collect();

    let mut targets: Vec<_> = handles.iter().step_by(2).copied().rev().collect();
    let mut verified = VerifiedIndices::new(targets.clone());

    arena.ordex(&verified, |iter| {
        assert_eq!(iter.len(), targets.len());
        for item in iter {
            item.label.push_str("_pass1");
            item.history.push(1);
        }
    });

    targets = handles.iter().skip(1).step_by(2).copied().rev().collect();
    verified.clear_and_verify(&targets);

    arena.ordex(&verified, |iter| {
        for item in iter {
            item.label.push_str("_pass2");
            item.history.push(2);
        }
    });

    for (i, &h) in handles.iter().enumerate() {
        let item = arena.get(h).unwrap();
        if i % 2 == 0 {
            assert_eq!(item.label, format!("node_{}_pass1", i));
        } else {
            assert_eq!(item.label, format!("node_{}_pass2", i));
        }
    }
}

/// 3. Verification of `MaybeUninit`, `set_len`, and buffer reallocation in the macro-partitioned path (`N > MACRO_PARTITION_THRESHOLD`)
#[test]
fn miri_macro_partition_uninit_and_buffer_reuse() {
    let (total_items, initial_len, small_len, expanded_len) = if cfg!(miri) {
        (160, 100, 30, 140)
    } else {
        (20_100, 20_020, 500, 20_080)
    };

    let mut arena = OrdexArena::new();
    let handles: Vec<_> = (0..total_items).map(|i| arena.insert(i as u64)).collect();

    // Reverse order to force full scatter across buckets and sequential write-back
    let initial_targets: Vec<_> = handles[..initial_len].iter().copied().rev().collect();
    let mut verified = VerifiedIndices::new(initial_targets);

    let slice = verified.as_slice();
    assert_eq!(slice.len(), initial_len);
    for (expected_idx, item) in slice.iter().enumerate() {
        assert_eq!(item.index as usize, expected_idx);
    }

    arena.ordex(&verified, |iter| {
        let mut count = 0;
        for val in iter {
            *val += 100;
            count += 1;
        }
        assert_eq!(count, initial_len);
    });

    // Reuse Pass A: Shrink down to a small batch (`N <= MACRO_PARTITION_THRESHOLD`)
    let small_targets: Vec<_> = handles[10..(10 + small_len)]
        .iter()
        .copied()
        .rev()
        .collect();
    verified.clear_and_verify(&small_targets);
    assert_eq!(verified.as_slice().len(), small_len);

    // Reuse Pass B: Expand beyond the previous `buffer.capacity()`
    let expanded_targets: Vec<_> = handles[..expanded_len].iter().copied().rev().collect();
    verified.clear_and_verify(&expanded_targets);
    assert_eq!(verified.as_slice().len(), expanded_len);

    arena.ordex(&verified, |iter| {
        for (i, val) in iter.enumerate() {
            if i < initial_len {
                assert_eq!(*val, (i as u64) + 100);
            } else {
                assert_eq!(*val, i as u64);
            }
        }
    });
}

/// 4. Verification of skewed key distributions (mixing empty and single-element buckets)
#[test]
fn miri_macro_partition_skewed_distribution() {
    let (total_items, dense_len) = if cfg!(miri) {
        (200, 80)
    } else {
        (25_000, 20_010)
    };

    let mut arena = OrdexArena::new();
    let handles: Vec<_> = (0..total_items).map(|i| arena.insert(i as u64)).collect();

    let mut skewed_targets: Vec<_> = handles[..dense_len].iter().copied().rev().collect();
    skewed_targets.push(handles[total_items - 1]);

    let verified = VerifiedIndices::new(skewed_targets);
    assert_eq!(verified.as_slice().len(), dense_len + 1);
    assert_eq!(
        verified.as_slice().last().unwrap().index as usize,
        total_items - 1
    );

    let mut visited = 0;
    arena.ordex(&verified, |iter| {
        for val in iter {
            *val = 999;
            visited += 1;
        }
    });
    assert_eq!(visited, dense_len + 1);
    assert_eq!(*arena.get(handles[total_items - 1]).unwrap(), 999);
}

/// 5. Panic verification for all-identical keys (`min_key == max_key`) in the macro-partitioned path
#[test]
#[should_panic(expected = "Aliasing detected")]
fn miri_macro_partition_all_identical_aliasing_panics() {
    let dup_len = if cfg!(miri) { 70 } else { 20_005 };
    let mut arena = OrdexArena::new();
    let h0 = arena.insert(42u64);

    let duplicated = vec![h0; dup_len];
    let _ = VerifiedIndices::new(duplicated);
}
