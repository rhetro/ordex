# Ordex

**Ordex** is a strict, generational arena allocator for Rust designed for systems programming. 

It provides a safe, deterministic architecture to manage lifecycles and handle simultaneous mutable access to multiple disparate elements—bypassing the need for runtime borrow checking overhead like `Rc<RefCell<T>>` while strictly enforcing Rust's aliasing rules.

## Core Philosophy

When building non-linear architectures or cross-cutting domain logic (e.g., AST manipulation, network multicast routing, or ECS), you frequently require simultaneous mutable access to multiple disparate nodes. Standard Rust collections reject this because the compiler cannot prove index disjointness at compile time.

Ordex solves this through two distinct mechanical approaches, split explicitly by their algorithmic thresholds:

1. **Targeted Alignment (`align!`):** Extracts up to 16 distinct mutable references simultaneously. Verification is done via fixed-size stack arrays. This macro is strictly capped at `N=16` because, within this exact boundary, LLVM auto-vectorization (SIMD) and modern CPU branch predictors execute the array checks in effectively `O(1)` time with zero memory allocations.
2. **Batch Ordering (`ordex`):** Provides safe, batch mutable access for dynamically generated target lists. Beyond `N=16`, the `O(N^2)` unrolled comparisons bloat the instruction cache. Therefore, this fallback API brings order to chaos via `O(N log N)` sorting and linear `O(N)` sweep validation, which becomes mathematically strictly superior for larger datasets.
3. **Strict ABA Prevention:** Uses generational indices to physically block access from stale pointers (dangling indices).

## The Aliasing Trilemma (Why Ordex?)

In system programming with Rust (e.g., graphs, ECS, network routing, or GUI widget trees), simultaneous mutable access to multiple disparate nodes typically forces a compromise between **Safety**, **Performance**, and **Ergonomics**.

Existing approaches resolve this by sacrificing at least one:

* **The Runtime Overhead (`Rc<RefCell<T>>`)**: Maintains safety and ergonomics but severely degrades performance in tight loops. Every access incurs reference counter updates and dynamic borrow flag checks, polluting the CPU cache line. This creates a severe bottleneck for high-throughput routing, high-FPS simulations, or complex GUI state propagation.
* **Manual Unsafe in Application Code (Raw Pointers)**: Achieves maximum performance but bypasses compiler-guaranteed memory safety. The programmer assumes full liability for ABA problems, aliasing violations, and undefined behavior.
* **Traditional Arenas (`slotmap`, `generational-arena`)**: Highly optimized for single-element access or iteration. However, they lack zero-allocation, safe APIs for simultaneous mutable access to 3 or more elements. You are forced to allocate temporary structures for runtime checks or use awkward extract-and-return patterns.

**Ordex breaks this trilemma through hardware-friendly static arrays and amortized dynamic batching.**
Instead of relying on generic software abstractions (like runtime state management), Ordex provides two distinct mechanical approaches:

1. **Static Validation (`align!`)**: For known, small targets (capped at `N=16`), it maps the safety validation to fixed-size stack arrays. This leverages LLVM auto-vectorization (SIMD) to resolve the `O(N^2)` aliasing verification in parallel, resulting in effectively `O(1)` execution time and strictly zero heap allocation.
2. **Dynamic Validation (`ordex`)**: For dynamic or large sets (e.g., spatial queries), it uses `O(N log N)` sorting and linear scanning over a reusable buffer. This guarantees safe multi-mutable iteration without cache-polluting borrow flags, achieving amortized zero heap allocation.

Together, they provide strict compiler safety and ergonomic multi-mutable access without requiring per-access heap allocations.

## Performance Proof: Hitting the Hardware Limit

Ordex is designed to map directly to CPU hardware capabilities, completely eliminating software abstraction overhead. Benchmarks prove that Ordex's latency is bounded strictly by physical memory access (Memory Wall), not by safety validation.

**1. Static Alignment (`align!` for N=16)**
* **Optimal Cache (L1 Hit):** `~129 ns` total (~8 ns per element).
* **Extreme Cache Miss (Forced L2/L3 DRAM fetch):** `~327 ns` total (~20 ns per element).
* **Conclusion:** 20ns aligns perfectly with the physical latency of fetching from L3/DRAM. The array-based `O(N^2)` safety check is entirely hidden by CPU out-of-order execution during the memory stall.

**2. Dynamic Batching (`ordex` for N=1000)**
* **Extreme Cache Miss:** `~91 µs` total (~91 ns per element).
* **Conclusion:** Even under extreme fragmentation, extracting 1,000 completely random mutable references—including `O(N log N)` sorting, deduplication, and generation verification—completes in under 100 microseconds with **strictly zero allocations**. 

## Real-World Application: Wasm Boids Simulation

To demonstrate Ordex's capability in a highly dynamic and constrained environment, I implemented a continuous Lotka-Volterra (Boids) running entirely in the browser via WebAssembly.

* **Workload:** 3,500+ active entities (predators and prey) continuously spawning, moving, and being removed.
* **Interactions:** Dynamic spatial hashing where nodes resolve simultaneous mutable state changes with 5+ neighbors per frame.
* **Performance:** Maintains a stable 100 FPS (capped by display Vsync) on a single Wasm thread.
* **Memory Profile:** Reuses internal buffers (`clear_and_verify`) to prevent per-frame heap allocations during the simulation loop, avoiding allocator overhead and heap fragmentation.

▶️ **Live Demo (Play in Browser):** [Non-Linear Network Boids Simulation (Lotka-Volterra Model / Rust + Wasm)](https://rhetro.pages.dev/rust/ordex/)

## Comparison

How does Ordex compare to standard Rust patterns and existing arena allocators?

| Feature | `Rc<RefCell<T>>` | Standard Arenas (e.g., generational) | **Ordex** |
| :--- | :--- | :--- | :--- |
| **Simultaneous Mutable Access** | Impossible (Runtime Panic) | Limited (usually 1-2 elements via `get2_mut`) | **Up to 16 (Static) or N (Dynamic)** |
| **Runtime Overhead** | Heavy (Ref counts & flag updates) | Low (Bounds & generation checks) | **Minimal (Verified upfront, register-level checks after)** |
| **Memory Efficiency** | Bloated (Pointers + counters) | High | **Highly Compact (8-byte NPO Index)** |
| **Architecture** | Decentralized & Chaotic | Centralized, but strict aliasing limits | Centralized & **Non-Linear** Multi-mutable |

## Key Features

* **Strict Aliasing Prevention**: Automatically panics if you attempt to request multiple mutable references to the same physical memory index, preventing silent data corruption.
* **Generational Tracking**: Every memory slot tracks its generation. Reused slots increment their generation, making old indices instantly invalid.
* **Zero-Allocation Dynamic Batching**: Dedicated API (`clear_and_verify`) allows reusing internal buffers for dynamic target resolution, achieving zero heap allocations during tight loops (e.g., game ECS ticks).
* **Fail-Fast vs. Safe Observation**: Distinct API boundaries between absolute enforcement (`align!` / `ordex`, which panic on stale access) and safe observation (`get` / `get_mut`, which return `Option`).
* **Null Pointer Optimization (NPO)**: Uses `NonZeroU32` for generation tracking by default, ensuring that `Option<Index>` remains exactly 8 bytes. Eliminates enum tag overhead and maximizes CPU cache efficiency.

## Quick Start

Add Ordex to your `Cargo.toml`:

```toml
[dependencies]
ordex = "0.1.0"
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

    // 1. Insert elements
    let idx_a = arena.insert(Node { value: 10 });
    let idx_b = arena.insert(Node { value: 20 });
    let idx_c = arena.insert(Node { value: 30 });

    // 2. Static Alignment: Safely mutate distinct elements simultaneously
    align!(arena, idx_a, idx_b, |a, b| {
        a.value += 5;
        b.value *= 2;
    });

    assert_eq!(arena.get(idx_a).unwrap().value, 15);
    assert_eq!(arena.get(idx_b).unwrap().value, 40);

    // 3. Memory Reuse & Generation Tracking
    arena.remove(idx_a); // Slot becomes Free

    // Reuses the physical slot of idx_a, but advances the generation
    let idx_new = arena.insert(Node { value: 99 }); 

    // Attempting to access the old index will result in a safe panic,
    // completely preventing the ABA problem.
    // align!(arena, idx_a, idx_c, |a, c| { ... }); // PANICS!
}
```

### Dynamic Batch Processing (Zero Allocation)

For scenarios where the targets are determined at runtime (e.g., multicast network routing, Area-of-Effect in games):

```rust
use ordex::prelude::*;

fn main() {
    let mut arena = OrdexArena::<i32>::new();
    let i0 = arena.insert(100);
    let i1 = arena.insert(200);
    let i2 = arena.insert(300);

    // Dynamic targets determined at runtime
    let targets = vec![i2, i0];
    
    // VerifiedIndices ensures no aliasing and sorts indices for optimal sequential access.
    // Make it mutable so we can reuse the allocated buffer later.
    let mut verified = VerifiedIndices::new(targets);

    // Bring "Ordex" (Order) to the dynamic list and mutate them in batch
    arena.ordex(&verified, |mut iter| {
        while let Some(value) = iter.next() {
            *value += 50;
        }
    });

    // In the next frame/tick, reuse the buffer to STRICTLY PREVENT memory allocations.
    // This is vital for ECS or high-frequency game loops.
    let new_targets = vec![i1, i2];
    verified.clear_and_verify(&new_targets); // Modified: Pass by slice reference
    
    arena.ordex(&verified, |mut iter| {
        // Safe, zero-allocation batch mutation
    });
}
```

## API Boundaries

Ordex enforces a strict contract regarding how you access data:

* **Use `align!` / `ordex`** when your domain logic *guarantees* the targets must exist. If a target is missing or aliased, it indicates a critical state synchronization bug, and Ordex will **panic** to fail-fast.
* **Use `get` / `get_mut`** when the target's existence is asynchronous or uncertain (e.g., UI updates, async task cancellation). It safely returns an `Option<T>`.

## Real-world Scenarios

Check out the `tests/` directories for executable documentation on applying Ordex to various domains:
* Compiler AST manipulation (Constant Folding)
* Network Session Multiplexing
* Async Task Scheduler / Worker Pools
* Entity Component System (ECS) Combat simulations

## Architectural Limits & Feature Flags

To maintain a compact 8-byte footprint for the `Index` struct and optimize CPU L1/L2 cache usage, Ordex uses `u32` for indices and `NonZeroU32` for generations by default. This default design introduces two hard limits:

1. **Maximum Capacity (Index Overflow):** The `OrdexArena` can hold a maximum of 4,294,967,295 (`u32::MAX`) active elements simultaneously. Attempting to insert more elements than this will explicitly panic (fail-fast) to strictly prevent silent memory corruption and UB.
2. **Generation Wrap-around (The ABA Edge Case):** If a *single specific physical memory slot* is freed and reused exactly 4,294,967,296 times, its generation counter will wrap around to 1. For short-lived applications or standard games, reaching this is practically impossible. However, highly active systems running continuously for months (e.g., game servers, routers) *can* hit this boundary on frequently recycled slots.

### Feature Flags: Extended Generation Tracking
If you are building long-running infrastructure where strict mathematical safety against the ABA problem is required over maximum CPU cache efficiency, opt into 64-bit indices:

```toml
[dependencies]
ordex = { version = "0.1.0", features = ["large-indices"] }
```

This forces the `Index` to use `u64` and `NonZeroU64` (16 bytes in total). This extends the generational wrap-around horizon far beyond the lifespan of the universe, providing strict safety for enterprise-grade continuous uptimes.

## License

This project is licensed under either of

* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or[http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0))
* MIT license ([LICENSE-MIT](LICENSE-MIT) or[http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))

at your option.
