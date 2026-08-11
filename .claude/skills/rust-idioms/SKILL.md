---
name: rust-idioms
description: Idiomatic Rust coding rules — borrowing vs cloning, Copy types, Option/Result pattern matching, lazy _else variants, iterators vs for loops, import ordering, and when to extract a function. Load when writing or reviewing any Rust code in this repo.
---

## Borrowing over cloning

- Default to `&T`. Prefer `&str` over `String`, `&[T]` over `Vec<T>`/`&Vec<T>` in signatures.
- `.clone()` is fine for: `Arc`/`Rc` handles, cached results, immutable snapshots, APIs that require owned data, and avoiding massive refactors of non-hot code.
- Never clone a `&T` argument to get ownership — change the signature to take `T` so the caller passes ownership explicitly.
- Don't `.map(|x| x.clone())` inside iterator chains — use `.cloned()`/`.copied()` at the end.
- If data is "maybe owned", use `Cow<'_, str>` / `Cow<'_, [T]>` instead of forcing an allocation.

## Pass by value (Copy)

- Pass small `Copy` types (`u32`, `bool`, `f32`, small structs) by value, not `&T`.
- Derive `Copy` only when: all fields are `Copy`, the struct is ≤ ~24 bytes (2–3 machine words), it's plain data (no heap: no `String`/`Vec`), **and it does not implement `Iterator`** (Copy + Iterator is a silent-wrong-results footgun; implement `IntoIterator` returning a separate iterator struct instead).
- Enums: `Copy` is fine for tag-like enums; remember enum size = largest variant.

## Option / Result handling

- `match` — when you pattern-match the inner `T`/`E` variants or transform shapes (e.g. `Result<T,E>` → `Result<Option<U>,E>`).
- `let Some(x) = expr else { return/continue/break ... };` — when the diverging arm needs no extra computation.
- `if let ... else` — only when the else arm needs real computation (otherwise use `let-else`).
- Don't hand-write Result→Option conversion matches — use `.ok()`, `.ok_or()`, `.ok_or_else()`.
- If you don't care about the `Err` value, propagate with `?`.
- No `unwrap()`/`expect()` outside tests (see the rust-error-handling skill).

## Prevent early allocation

- Use `_else` variants when the fallback allocates or computes: `ok_or_else`, `map_or_else`, `unwrap_or_else`. Plain `ok_or`/`map_or`/`unwrap_or` only for cheap, pre-existing values; `unwrap_or_default` over `unwrap_or(Vec::new())`.
- To log and convert errors: `.inspect_err(|e| tracing::error!(...)).map_err(Into::into)?`.

## Iterators vs `for`

- `for` when you need early exit (`break`/`continue`/`return`) or simple side-effecting iteration.
- Iterator chains when transforming collections, composing steps, using `.enumerate()`, `.windows()`, `.chunks()`, or fusing multiple sources without intermediate allocations.
- Iterators are lazy — nothing runs until a consumer (`collect`, `sum`, `for_each`).
- Don't `.collect()` an intermediate `Vec` just to iterate it again — pass the iterator (`impl Iterator<Item = _>`).
- Prefer `.iter()` over `.into_iter()` unless you need ownership; prefer `.sum()` over `.fold()` for summing.

## Imports (`use`) ordering

Group in this order: `std` → external crates → workspace crates → `super::` → `crate::`. Enforce via `rustfmt.toml`:

```toml
reorder_imports = true
imports_granularity = "Crate"
group_imports = "StdExternalCrate"
```

(needs `cargo +nightly fmt` as of Rust 1.88)

## When to extract a function (and when not to)

- Extract when: the logic is the **same decision** used in 3+ places (Rule of Three), the name adds meaning (`is_retryable`), it's a unit worth testing, or it hides complexity behind a small honest signature.
- Don't extract when: it's 1–2 lines in <3 places, unifying call sites would need a flag/`bool` parameter, or you're guessing at a future abstraction. "Duplication is far cheaper than the wrong abstraction" (Sandi Metz).
- DRY is about **knowledge**, not text — similar-looking lines that encode different decisions (coincidental duplication) should stay inline.
- Found a wrong abstraction? Unwind it: re-inline into every caller, simplify each site, then re-extract only what's genuinely shared.
- In tests, prefer duplication (DAMP): share **setup/fixtures** only; keep each test's action and assertion inline.