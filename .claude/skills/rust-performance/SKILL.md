---
name: rust-performance
description: Rust performance mindset — measure before optimizing, flamegraph/bench workflow, ownership vs borrowing for speed, Cow, stack vs heap sizing, and zero-cost iterator rules. Load when optimizing Rust code, investigating slowness, or making allocation/ownership decisions in hot paths.
---

## Golden rule: don't guess, measure

- Always benchmark/profile with `--release`. "Rust is slower than X" is almost always a missing `--release`.
- `cargo clippy -- -D clippy::perf` for quick wins.
- `cargo bench` for micro-benchmarks; treat >5% improvement over the original as meaningful.
- `cargo flamegraph` (or `samply` on macOS) for profiling: `cargo flamegraph --bin=name`, `--unit-test`, `--test`, `--bench some_benchmark -- --bench`. Wide boxes = time spent (per call or call count); y-axis = stack depth; colors are random and meaningless.

## Cloning and ownership in hot paths

- If you must clone, clone at the **last possible moment**.
- Legitimate clones: API requires owned data, snapshot comparisons, `Arc`/`Rc` handles, cheap-handle structs that share internals (e.g. hyper `Client` shares its connection pool), owned-builder chains, ownership used to model intent (`Validate::try_from(raw)`).
- Prefer signatures that borrow: `fn process(values: &[T])`, not `fn process(values: Vec<T>)`. Read-only access → `.iter()` or slices. Cross-thread mutation of borrowed data → `&mut T`.
- "Maybe owned" data → `std::borrow::Cow<'_, str>` accepts both `Cow::Borrowed` and `Cow::Owned` without forcing allocation.

## Stack vs heap: be size-smart

- Small `Copy` types stay on the stack; return them by value.
- Don't pass/move types > ~512 bytes by value — take `&T`/`&mut T`.
- Heap-allocate recursive types (`Box<[Node<T>; 8]>`).
- Avoid huge stack allocations: `Box::new([0u8; 65536])` still builds the array on the stack first — use `vec![0; 65536].into_boxed_slice()`. Consider `smallvec` for arrays that are usually small but occasionally large.
- `#[inline]` only when a benchmark proves it — rustc already inlines well.

## Iterators are zero-cost

- Chained `.filter().map().rev().skip().take()` fuses into one tight loop at compile time; prefer iterator chains over manual index loops for collections.
- `.iter()` only creates a reference — you can hold several iterators over one collection.
- Never build an intermediate `Vec` just to feed another operation — accept/pass `impl Iterator<Item = _>` instead of collecting.