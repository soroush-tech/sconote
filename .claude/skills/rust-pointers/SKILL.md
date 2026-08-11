---
name: rust-pointers
description: Choosing the right Rust pointer/container — &T, Box, Rc/Arc, Cell/RefCell, Mutex/RwLock, OnceCell/OnceLock, LazyCell/LazyLock — with Send/Sync thread-safety rules. Load when picking a smart pointer, sharing state across threads, or debugging Send/Sync bounds.
---

## Thread safety basics

- `Send`: the value can move to another thread. `Sync`: it can be referenced from multiple threads.
- A pointer is only as thread-safe as the data behind it.

## Selection table

| Type | Use for | Send + Sync? |
|------|---------|--------------|
| `&T` | Shared read-only access | Yes |
| `&mut T` | Exclusive mutation | Not Send |
| `Box<T>` | Single-owner heap data; recursive types; large structs | If `T` is |
| `Rc<T>` | Multiple owners, single thread | No |
| `Arc<T>` | Multiple owners across threads (`Arc<[T]>` for shared slices) | Yes |
| `Cell<T>` | Interior mutability for `Copy` types, single thread | Not Sync |
| `RefCell<T>` | Interior mutability, runtime-checked borrows, single thread — **can panic** | Not Sync |
| `Mutex<T>` | Exclusive mutable access across threads (usually `Arc<Mutex<T>>`) | Yes |
| `RwLock<T>` | Many readers OR one writer across threads (usually in `Arc`) | Yes |
| `OnceCell<T>` | One-time init, single thread | Not Sync |
| `LazyCell<T>` | Lazy closure-based init, single thread | Not Sync |
| `OnceLock<T>` | One-time init in a `static`, thread-safe | Yes |
| `LazyLock<T>` | Lazy `static` with complex init, thread-safe | Yes |
| `*const T` / `*mut T` | FFI / raw memory only, `unsafe` | Manual |

## Rules of thumb

- Default to `&T` / `&mut T`; reach for smart pointers only when ownership or sharing demands them.
- Recursive enums/structs need `Box` (note `Vec` is already heap-allocated — `Multi(Vec<T>)` needs no extra box).
- Escalation ladder for shared mutable state: single thread `Cell` (Copy) → `RefCell`; multi-thread `Arc<Mutex<T>>` → `Arc<RwLock<T>>` if reads dominate.
- `RefCell` enforces borrow rules at **runtime**: holding a `borrow()` while taking `borrow_mut()` panics.
- Global/lazy statics: `OnceLock` for set-once values, `LazyLock` when initialization is a closure:

```rust
static CONFIG: LazyLock<HashMap<String, Value>> = LazyLock::new(|| {
    read_config().into()
});
```

Further reading: [Mara Bos — Rust Atomics and Locks](https://marabos.nl/atomics/).