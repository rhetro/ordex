# Ordex

**Ordex** is a strict, generational arena allocator for Rust designed for systems programming.

It provides a safe, deterministic architecture to manage lifecycles and handle simultaneous mutable access to multiple disparate elements—bypassing runtime borrow-checking overhead like `Rc<RefCell<T>>` while strictly enforcing Rust's aliasing rules.

## Core Philosophy

When building non-linear architectures or cross-cutting domain logic (e.g., AST manipulation, network multicast routing, or ECS), you frequently require simultaneous mutable access to multiple disparate nodes. Standard Rust collections reject this because the compiler cannot prove index disjointness at compile time.

Ordex solves this through two distinct mechanical approaches, split explicitly by their usage patterns:

1. **Targeted Alignment (`align!`):** Extracts up to 16 distinct mutable references simultaneously. Verification is done via fixed-size stack arrays. Capped at `N=16`, the `O(N^2)` unrolled stack checks are optimized by LLVM auto-vectorization (SIMD) and CPU out-of-order execution, running in constant CPU cycles with zero heap allocations.
2. **Hybrid Dynamic Routing (`ordex`):** Provides safe, batch mutable access for dynamically generated target lists beyond `N=16`. Ordex v0.3.0 uses a **cache-aware hierarchical arithmetic routing pipeline** powered by `arsort`. For L1/L2 cache-bound workloads (`N <= 20,000`), it executes branchless arithmetic routing directly; for larger DRAM-bound sets, it performs a single-pass disjoint scatter into cache-sized buckets before local sorting—yielding **strictly ascending memory order** and **amortized `O(N)` validation**.
3. **Strict ABA Prevention:** Uses generational indices to physically block access from stale pointers (dangling indices).

## The Aliasing Trilemma (Why Ordex?)

In system programming with Rust (e.g., graphs, ECS, network routing, or GUI widget trees), simultaneous mutable access to multiple disparate nodes typically forces a compromise between **Safety**, **Performance**, and **Ergonomics**.

Existing approaches resolve this by sacrificing at least one:

- **The Runtime Overhead (`Rc<RefCell<T>>`)**: Maintains safety and ergonomics but incurs reference counter updates and dynamic borrow flag checks per access.
- **Manual Unsafe in Application Code (Raw Pointers)**: Achieves maximum performance but bypasses compiler-guaranteed memory safety, placing full responsibility for ABA problems and aliasing violations on the developer.
- **Traditional Arenas (`slotmap`, `generational-arena`)**: Highly optimized for single-element access or full iteration, but lacking zero-allocation, safe APIs for simultaneous mutable access to 3 or more arbitrary elements.

**Ordex resolves this trilemma through static stack validation and amortized dynamic batching.**
Instead of relying on dynamic borrow flags, Ordex provides two explicit mechanisms:

1. **Static Validation (`align!`)**: For small, fixed target sets (capped at `N=16`), safety validation uses unrolled stack arrays, allowing LLVM to optimize checks in parallel without heap allocation.
2. **Dynamic Validation (`ordex`)**: For dynamic target lists, it sorts indices into strict ascending order via hierarchical arithmetic routing (`arsort` + single-pass disjoint partitioning) and detects duplicates in a single linear `O(N)` pass. This maximizes hardware prefetcher efficiency during arena traversal while remaining amortized zero-allocation during buffer reuses via `clear_and_verify`.

## Performance Overview

Ordex is designed to minimize abstraction overhead, leaving access latencies bounded primarily by physical CPU cache and memory hierarchies.

**1. Static Alignment (`align!` for N=16)**
- **Execution Time:** `~79 ns` total (~4.9 ns per element in local cache hits).
- **Behavior:** The unrolled `O(N^2)` safety checks are effectively overlapped with memory pipeline accesses via CPU out-of-order execution.

**2. Dynamic Batching (`ordex` for N=10,000)**
- **v0.3.0 Arithmetic Routing Engine:** `~318 µs` total (~31.8 ns per element, including full ascending sort and random DRAM arena traversal).
- **Behavior:** Branchless arithmetic routing produces strictly ascending index order for sequential hardware prefetching while maintaining amortized zero heap allocations when reusing buffers via `clear_and_verify`.

## Real-World Application: Wasm Boids Simulation

To demonstrate Ordex's capability in a highly dynamic environment, a continuous Lotka-Volterra (Boids) simulation was implemented to run in the browser via WebAssembly.

- **Workload:** 3,500+ active entities (predators and prey) continuously spawning, moving, and being removed.
- **Interactions:** Dynamic spatial hashing where nodes resolve simultaneous mutable state changes with 5+ neighbors per frame.
- **Performance:** Maintains a stable 100 FPS (capped by display Vsync) on a single Wasm thread.
- **Memory Profile:** Reuses internal buffers (`clear_and_verify`) to avoid per-frame heap allocations during the main loop.

▶️ **Live Demo (Play in Browser):** [Non-Linear Network Boids Simulation (Lotka-Volterra Model / Rust + Wasm)](https://rhetro.pages.dev/rust/ordex/)

## Comparison

| Feature | `Rc<RefCell<T>>` | Standard Arenas | **Ordex (v0.3.0)** |
| :--- | :--- | :--- | :--- |
| **Simultaneous Mutable Access** | Impossible (Runtime Panic) | Limited (usually 1–2 via `get2_mut`) | **Up to 16 (Static) or N (Dynamic)** |
| **Runtime Checking** | Ref counts & borrow flags | Bounds & generation checks | **Unrolled Stack Checks (`align!`) / Arithmetic Routing (`ordex`)** |
| **Memory Efficiency** | High overhead (Pointers + flags) | Compact | **Compact (8-byte NPO Index)** |
| **Algorithmic Complexity** | Dynamic Flag Checks per access | Bounds Check per access | **`O(N^2)` Unrolled Static (`N <= 16`) / Amortized `O(N)` Dynamic** |

## Quick Start

Add Ordex to your `Cargo.toml`:

```toml
[dependencies]
ordex = "0.3.0"
```

### Basic Usage: Targeted Alignment

```rust
use ordex::prelude::*;

#[derive(Debug)]
struct Node {
    value: i32,
}

fn main() {
    let mut arena = OrdexArena::new();

    let idx_a = arena.insert(Node { value: 10 });
    let idx_b = arena.insert(Node { value: 20 });
    let idx_c = arena.insert(Node { value: 30 });

    // Static Alignment: Safely mutate distinct elements simultaneously
    align!(arena, idx_a, idx_b, |a, b| {
        a.value += 5;
        b.value *= 2;
    });

    assert_eq!(arena.get(idx_a).unwrap().value, 15);
    assert_eq!(arena.get(idx_b).unwrap().value, 40);

    // Generation Tracking & Memory Reuse
    arena.remove(idx_a);
    let idx_new = arena.insert(Node { value: 99 }); 

    // Accessing old indices safely panics (ABA prevention)
    // align!(arena, idx_a, idx_c, |a, c| { ... }); // PANICS!
}
```

### Dynamic Batch Processing (Zero Allocation)

```rust
use ordex::prelude::*;

fn main() {
    let mut arena = OrdexArena::<i32>::new();
    let i0 = arena.insert(100);
    let i1 = arena.insert(200);
    let i2 = arena.insert(300);

    let targets = vec![i2, i0];
    
    // VerifiedIndices uses hierarchical arithmetic routing for batch alias validation
    let mut verified = VerifiedIndices::new(targets);

    arena.ordex(&verified, |mut iter| {
        while let Some(value) = iter.next() {
            *value += 50;
        }
    });

    // Reuse internal workspace for zero-allocation game/ECS loops
    let new_targets = vec![i1, i2];
    verified.clear_and_verify(&new_targets);
    
    arena.ordex(&verified, |mut iter| {
        // Safe, zero-allocation batch mutation
    });
}
```

## API Boundaries

- **Use `align!` / `ordex`** when domain logic guarantees target existence. Missing or aliased targets trigger a panic (fail-fast).
- **Use `get` / `get_mut`** when target existence is uncertain. It safely returns an `Option<T>`.

## Memory Safety & Miri Verification

Although Ordex uses internal `unsafe` pointer arithmetic and `MaybeUninit` buffers for minimal-overhead mutable access and zero-initialization scatter routing, all invariants are strictly verified under [Miri](https://github.com/rust-lang/miri).

The entire test suite—including disjoint mutable access (`align!` and `ordex`), uninitialized workspace reuse, generational ABA checks, and drop/panic safety—passes with **zero undefined behavior (UB) and zero memory leaks** under:

- **Stacked Borrows** (default Miri aliasing model)
- **Tree Borrows** (`-Zmiri-tree-borrows`)
- Both default (`u32`) and `--features large-indices` (`u64`) configurations

To run the Miri verification suite locally:

```bash
cargo +nightly miri test
cargo +nightly miri test --features large-indices
MIRIFLAGS="-Zmiri-tree-borrows" cargo +nightly miri test
```

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE).
