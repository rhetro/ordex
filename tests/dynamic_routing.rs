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
// 1. Strict Ascending Routing & Batch Mutation Verification
// =====================================================================

#[test]
fn test_strict_ascending_routing_and_mutation() {
    let mut arena = OrdexArena::new();
    let mut handles = Vec::new();

    for i in 0..64 {
        handles.push(arena.insert(TestEntity::new(i)));
    }

    // Reverse the target order to verify that VerifiedIndices sorts them into strict ascending order
    let reversed_targets: Vec<_> = handles.iter().copied().rev().collect();
    let verified = VerifiedIndices::new(reversed_targets);

    assert_eq!(verified.as_slice().len(), 64);

    let mut visited_ids = Vec::with_capacity(64);
    arena.ordex(&verified, |iter| {
        for entity in iter {
            entity.payload.push_str("_processed");
            visited_ids.push(entity.id);
        }
    });

    // Traversal must follow strict ascending index order regardless of input order
    let expected_ids: Vec<u32> = (0..64).collect();
    assert_eq!(visited_ids, expected_ids);

    for handle in handles {
        assert!(arena.get(handle).unwrap().payload.ends_with("_processed"));
    }
}

// =====================================================================
// 2. Aliasing Detection Verification
// =====================================================================

#[test]
#[should_panic(expected = "Aliasing detected")]
fn test_aliasing_detection_in_verified_indices() {
    let mut arena = OrdexArena::new();
    let handle1 = arena.insert(TestEntity::new(1));
    let handle2 = arena.insert(TestEntity::new(2));

    // Include duplicate handle1 to trigger aliasing detection
    let duplicated_targets = vec![handle1, handle2, handle1];

    // Must panic immediately
    let _ = VerifiedIndices::new(duplicated_targets);
}

// =====================================================================
// 3. Empty Target Handling Verification
// =====================================================================

#[test]
fn test_empty_targets_handling() {
    let mut arena = OrdexArena::new();
    let handle = arena.insert(TestEntity::new(1));

    // Constructing VerifiedIndices with an empty vector
    let mut verified = VerifiedIndices::new(vec![]);
    assert!(verified.as_slice().is_empty());

    arena.ordex(&verified, |mut iter| {
        assert_eq!(iter.len(), 0);
        assert!(iter.next().is_none());
    });

    // Populate with a valid target, then clear back to empty
    verified.clear_and_verify(&[handle]);
    assert_eq!(verified.as_slice().len(), 1);

    verified.clear_and_verify(&[]);
    assert!(verified.as_slice().is_empty());

    arena.ordex(&verified, |mut iter| {
        assert_eq!(iter.len(), 0);
        assert!(iter.next().is_none());
    });
}

// =====================================================================
// 4. Workspace Buffer Reuse via clear_and_verify
// =====================================================================

#[test]
fn test_clear_and_verify_buffer_reuse() {
    let mut arena = OrdexArena::new();
    let mut handles = Vec::new();

    for i in 0..100 {
        handles.push(arena.insert(TestEntity::new(i)));
    }

    // Frame 1: Verify and traverse the first half in reverse order
    let frame1_targets: Vec<_> = handles[0..50].iter().copied().rev().collect();
    let mut verified = VerifiedIndices::new(frame1_targets);

    // Frame 2: Reuse the internal buffer for a completely disjoint set of indices
    let frame2_targets: Vec<_> = handles[50..100].iter().copied().rev().collect();
    verified.clear_and_verify(&frame2_targets);

    // Ensure no stale indices from Frame 1 remain in Frame 2
    assert_eq!(verified.as_slice().len(), 50);

    let mut frame2_read_ids = Vec::new();
    arena.ordex(&verified, |iter| {
        for entity in iter {
            frame2_read_ids.push(entity.id);
        }
    });

    let expected_frame2_ids: Vec<u32> = (50..100).collect();
    assert_eq!(frame2_read_ids, expected_frame2_ids);
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

// =====================================================================
// 6. Freed Entry (Dangling Pointer) Access Verification
// =====================================================================

#[test]
#[should_panic(expected = "Dangling pointer: Entry at index 0 is already freed.")]
fn test_freed_entry_access_in_ordex() {
    let mut arena = OrdexArena::new();
    let handle_freed = arena.insert(TestEntity::new(10));
    let handle_alive = arena.insert(TestEntity::new(20));

    // Construct VerifiedIndices while both entries are still alive
    let verified = VerifiedIndices::new(vec![handle_freed, handle_alive]);

    // Free the first entry before executing ordex()
    arena.remove(handle_freed);

    // Must panic immediately upon detecting the freed slot
    arena.ordex(&verified, |_| {});
}
