use ordex::prelude::*;

/// Complex data structure for testing.
/// Containing heap allocations (String) ensures that memory leaks and
/// double-free bugs are strictly caught during verification.
#[derive(Debug, PartialEq, Clone)]
struct TestEntity {
    id: u32,
    payload: String,
}

impl TestEntity {
    fn new(id: u32) -> Self {
        Self {
            id,
            payload: format!("payload_{}", id),
        }
    }
}

// =====================================================================
// 1. Hash Collision (Bucket Collision) Escape Verification
// =====================================================================

#[test]
fn test_hash_collision_escape_success() {
    let mut arena = OrdexArena::new();
    let mut handles = Vec::new();

    // Insert handles with a strided index pattern that tends to map to the same chunk_id/offset
    for i in 0..64 {
        handles.push(arena.insert(TestEntity::new(i * 1000)));
    }

    // Verify a target list with zero true aliasing, but frequent bucket collisions
    let verified = VerifiedIndices::new(handles.clone());

    // Ensure all collisions are resolved via trailing_zeros() without false-positive panics
    let mut visited_count = 0;
    arena.ordex(&verified, |mut iter| {
        while let Some(entity) = iter.next() {
            entity.payload.push_str("_processed");
            visited_count += 1;
        }
    });

    assert_eq!(visited_count, 64);
    for handle in handles {
        assert!(arena.get(handle).unwrap().payload.ends_with("_processed"));
    }
}

// =====================================================================
// 2. Real Aliasing Detection Verification
// =====================================================================

#[test]
#[should_panic(expected = "Aliasing detected")]
fn test_real_aliasing_triggers_panic_even_in_same_slot() {
    let mut arena = OrdexArena::new();
    let handle1 = arena.insert(TestEntity::new(1));
    let handle2 = arena.insert(TestEntity::new(2));

    // Include duplicate handle1 (True aliasing, not a simple hash collision)
    let duplicated_targets = vec![handle1, handle2, handle1];

    // Must panic immediately
    let _ = VerifiedIndices::new(duplicated_targets);
}

// =====================================================================
// 3. Chunk Overflow Evacuation and Recovery Verification
// =====================================================================

#[test]
fn test_chunk_overflow_and_recovery() {
    let mut arena = OrdexArena::new();
    let mut handles = Vec::new();

    // Concentrate 32 elements into a dense range to force overflow past a single 16-slot chunk
    for i in 0..32 {
        handles.push(arena.insert(TestEntity::new(i)));
    }

    // Ensures overflow elements are evacuated to workspace.overflow and restored safely
    let verified = VerifiedIndices::new(handles.clone());

    assert_eq!(verified.as_slice().len(), 32);

    let mut count = 0;
    arena.ordex(&verified, |mut iter| {
        while iter.next().is_some() {
            count += 1;
        }
    });

    assert_eq!(count, 32);
}

// =====================================================================
// 4. Workspace Sanitization via clear_and_verify
// =====================================================================

#[test]
fn test_clear_and_verify_workspace_sanitization() {
    let mut arena = OrdexArena::new();
    let mut handles = Vec::new();

    for i in 0..100 {
        handles.push(arena.insert(TestEntity::new(i)));
    }

    // Frame 1: Trigger collisions and overflow
    let frame1_targets = handles[0..50].to_vec();
    let mut verified = VerifiedIndices::new(frame1_targets);

    // Frame 2: Reuse the workspace for a completely disjoint set of indices
    let frame2_targets = handles[50..100].to_vec();
    verified.clear_and_verify(&frame2_targets);

    // Ensure no residual metadata (is_dirty, bitmap, overflow) corrupts Frame 2
    assert_eq!(verified.as_slice().len(), 50);

    let mut frame2_read_ids = Vec::new();
    arena.ordex(&verified, |mut iter| {
        while let Some(entity) = iter.next() {
            frame2_read_ids.push(entity.id);
        }
    });

    assert_eq!(frame2_read_ids.len(), 50);
    for id in frame2_read_ids {
        assert!((50..100).contains(&id));
    }
}

// =====================================================================
// 5. Generational Mismatch Detection Verification
// =====================================================================

#[test]
#[should_panic(expected = "Generation mismatch")]
fn test_generation_mismatch_in_ordex() {
    let mut arena = OrdexArena::new();
    let handle_old = arena.insert(TestEntity::new(10));

    // Remove element and reuse slot to advance generation
    arena.remove(handle_old);
    let _handle_new = arena.insert(TestEntity::new(20));

    // Construct VerifiedIndices with the stale index
    let verified = VerifiedIndices::new(vec![handle_old]);

    // Must panic due to generational mismatch
    arena.ordex(&verified, |_| {});
}
