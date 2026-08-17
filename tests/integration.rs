use ordex::prelude::*;

// =====================================================================
// 1. Domain Model Definition (Unified)
// =====================================================================

/// Complex data structure for testing.
/// By having String and Vec allocate memory on the heap, it proves that memory leaks and
/// double-free errors never occur.
#[derive(Debug, PartialEq, Clone)]
pub struct Entity {
    pub name: String,
    pub tags: Vec<String>,
    pub power: f32,
}

impl Entity {
    pub fn new(name: &str, power: f32) -> Self {
        Self {
            name: name.to_string(),
            tags: vec![],
            power,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =====================================================================
    // 2. Proof of Static Expansion (align!)
    // =====================================================================

    #[test]
    fn test_align2_success_with_complex_data() {
        let mut arena = OrdexArena::new();
        let alpha = arena.insert(Entity::new("Alpha", 100.0));
        let beta = arena.insert(Entity::new("Beta", 50.0));

        // Simultaneous manipulation of mutable references of complex data
        align!(arena, alpha, beta, |a, b| {
            a.name.push_str(" (Awakened)");
            a.tags.push("Leader".to_string());
            a.power += 20.0;

            b.tags.push("Follower".to_string());
            b.power -= 10.0;
        });

        // Verification of state
        align!(arena, alpha, beta, |a, b| {
            assert_eq!(a.name, "Alpha (Awakened)");
            assert_eq!(a.tags, vec!["Leader".to_string()]);
            assert_eq!(a.power, 120.0);

            assert_eq!(b.name, "Beta");
            assert_eq!(b.tags, vec!["Follower".to_string()]);
            assert_eq!(b.power, 40.0);
        });
    }

    #[test]
    #[should_panic(expected = "Aliasing detected")]
    fn test_align2_aliasing() {
        let mut arena = OrdexArena::new();
        let target = arena.insert(Entity::new("Target", 10.0));
        arena.align2(target, target, |_, _| {});
    }

    // =====================================================================
    // 3. Generatinal Management and Memory Reuse (Proof of Drop)
    // =====================================================================

    #[test]
    fn test_memory_reuse_and_drop_semantics() {
        let mut arena = OrdexArena::new();

        let n1 = arena.insert(Entity::new("N1", 0.0));
        let _n2 = arena.insert(Entity::new("N2", 0.0));

        // Remove n1. The heap memory of String and Vec is correctly dropped here.
        let removed = arena.remove(n1).unwrap();
        assert_eq!(removed.name, "N1");

        // Proof of reuse: When n3 is inserted, index 0 of n1 is reused and the generation advances.
        let n3 = arena.insert(Entity::new("N3", 99.9));

        assert_eq!(n3.index, 0);
        assert!(n3.generation > n1.generation);
    }

    #[test]
    #[should_panic(
        expected = "Generation mismatch at index 0: This element has already been removed or replaced."
    )]
    fn test_aba_problem_prevention() {
        let mut arena = OrdexArena::new();
        let old_idx = arena.insert(Entity::new("Old", 100.0));

        arena.remove(old_idx);
        let _new_idx = arena.insert(Entity::new("New", 200.0));

        let another_idx = arena.insert(Entity::new("Another", 300.0));
        // Access with an index of different generation (overwritten by new data)
        align!(arena, old_idx, another_idx, |_, _| {});
    }

    #[test]
    #[should_panic(expected = "Dangling pointer: Entry at index 0 is already freed.")]
    fn test_freed_entry_access() {
        let mut arena = OrdexArena::new();
        let idx1 = arena.insert(Entity::new("To Remove", 100.0));
        let idx2 = arena.insert(Entity::new("Safe", 200.0));

        arena.remove(idx1); // Make if a free slot

        arena.align2(idx1, idx2, |_, _| {});
    }

    // =====================================================================
    // 4. Proof of Dynamic Expansion (ordex)
    // =====================================================================

    #[test]
    fn test_ordex_success_with_complex_data() {
        let mut arena = OrdexArena::new();
        let i0 = arena.insert(Entity::new("E0", 10.0));
        let i1 = arena.insert(Entity::new("E1", 20.0));
        let i2 = arena.insert(Entity::new("E2", 30.0));

        let verified = VerifiedIndices::new(vec![i2, i0, i1]);

        let result = arena.ordex(&verified, |mut iter| {
            let e0 = iter.next().unwrap();
            let e1 = iter.next().unwrap();
            let e2 = iter.next().unwrap();

            e0.power += 1.0;
            e1.power += 2.0;
            e2.power += 3.0;

            assert!(iter.next().is_none());
            "Success"
        });

        assert_eq!(result, "Success");
        assert_eq!(arena.get(i0).unwrap().power, 11.0);
        assert_eq!(arena.get(i1).unwrap().power, 22.0);
        assert_eq!(arena.get(i2).unwrap().power, 33.0);
    }

    // =====================================================================
    // 5. Proof of Macro Routing
    // =====================================================================

    #[test]
    fn test_router_macro_align5_with_complex_data() {
        let mut arena = OrdexArena::new();
        let i0 = arena.insert(Entity::new("A", 1.0));
        let i1 = arena.insert(Entity::new("B", 2.0));
        let i2 = arena.insert(Entity::new("C", 3.0));
        let i3 = arena.insert(Entity::new("D", 4.0));
        let i4 = arena.insert(Entity::new("E", 5.0));

        align!(arena, i0, i1, i2, i3, i4, |a, _b, c, _d, e| {
            a.name = "A+".to_string();
            c.name = "C+".to_string();
            e.name = "E+".to_string();
        });

        assert_eq!(arena.get(i0).unwrap().name, "A+");
        assert_eq!(arena.get(i2).unwrap().name, "C+");
        assert_eq!(arena.get(i4).unwrap().name, "E+");
    }

    // =====================================================================
    // 6. Proof of Full-Element Scanning (Iterators)
    // =====================================================================

    #[test]
    fn test_arena_iteration_with_complex_data() {
        let mut arena = OrdexArena::new();
        let idx0 = arena.insert(Entity::new("Target 1", 10.0));
        let idx1 = arena.insert(Entity::new("To Delete", 20.0));
        let idx2 = arena.insert(Entity::new("Target 2", 30.0));

        // Delete to create a "Free slot" in memory.
        // Here, the String of "To Delete" is also correctly released.
        arena.remove(idx1);

        // Test for iter_mut(): Batch rewrite String and f32 of only living elements.
        for (_idx, entity) in arena.iter_mut() {
            entity.name.push_str(" (Processed)");
            entity.power *= 2.0;
        }

        let e0 = arena.get(idx0).unwrap();
        assert_eq!(e0.name, "Target 1 (Processed)");
        assert_eq!(e0.power, 20.0);

        let e2 = arena.get(idx2).unwrap();
        assert_eq!(e2.name, "Target 2 (Processed)");
        assert_eq!(e2.power, 60.0);

        // Naturally, idx1, which was already deleted, remains None.
        assert_eq!(arena.get(idx1), None);
    }

    #[test]
    fn test_npo_optimization() {
        use ordex::prelude::*;
        use std::mem::size_of;

        assert_eq!(size_of::<Index>(), 8);
        assert_eq!(size_of::<Option<Index>>(), 8);
    }
}
