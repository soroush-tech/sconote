---
name: rust-dispatch
description: Generics, static dispatch (impl Trait) vs dynamic dispatch (dyn Trait) — trade-offs, trait object ergonomics, and object safety. Load when designing polymorphic Rust APIs, choosing between generics and trait objects, or reviewing Box<dyn Trait> usage.
---

> Static where you can, dynamic where you must.

## Static dispatch: `<T: Trait>` / `impl Trait`

Monomorphized at compile time — zero runtime cost, inlined, specialized machine code per concrete type. Best when: performance matters, types are known at compile time, tight loops, single-use implementations.

```rust
fn specialized_sum<U: Sum + RandomMapping>(iter: impl Iterator<Item = U>) -> U {
    iter.map(|x| x.random_mapping()).sum()
}
```

## Dynamic dispatch: `dyn Trait`

Runtime vtable behind a pointer (`Box<dyn T>`, `Arc<dyn T>`, `&dyn T`). Use only when type erasure or runtime polymorphism is essential: heterogeneous collections (`Vec<Box<dyn Animal>>`), plugin/hot-swappable architectures, hiding internals behind a stable library interface.

## Trade-offs

| | Static (`impl Trait`) | Dynamic (`dyn Trait`) |
|---|---|---|
| Runtime perf | ✅ inlined | ❌ vtable indirection |
| Compile time | ❌ monomorphization | ✅ shared code |
| Binary size | ❌ per-type codegen | ✅ smaller |
| Mixing types in one collection | ❌ | ✅ |
| Error messages | ✅ clearer | ❌ erased types confuse |

When unsure: start with trait-bounded generics; switch to `Box<dyn Trait>` only when flexibility outweighs speed.

## Trait object ergonomics

- Prefer `&dyn Trait` over `Box<dyn Trait>` when you don't need ownership.
- Cross-thread sharing: `Arc<dyn Trait + Send + Sync>`.
- Don't use `dyn Trait` if the trait has methods returning `Self`.
- **Don't box too early**: `struct Renderer<B: RenderBackend> { backend: B }` beats `backend: Box<dyn RenderBackend>` unless boxing is required (e.g. recursion) or proven beneficial. If a public API must expose `dyn Trait`, box at the boundary, not internally.
- Object safety — `dyn Trait` requires: no generic methods, no `Self: Sized` bound, and all methods take `&self`/`&mut self`/`self`.