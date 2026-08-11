---
name: rust-error-handling
description: Rust error handling — Result over panic, unwrap/expect policy, thiserror for libraries vs anyhow for binaries, ? propagation, error hierarchies, and testing errors. Load when designing error types, handling fallible operations, or reviewing error paths in Rust.
---

## Prefer `Result`, avoid panics

- Fallible functions return `Result<T, E>` — never panic for expected failure.
- `panic!` only for unrecoverable conditions (tests, assertions, genuine bugs). Prefer the intent-revealing macros: `todo!` (code missing, known), `unreachable!` (proven impossible, want an alert if it becomes possible), `unimplemented!` (deliberately not implemented, with a reason).

## `unwrap` / `expect` policy

Only in tests/test helpers, when failure is provably impossible, or when no smarter option fits. Otherwise:

- Fixed early-return that doesn't need the `Err` value: `let Ok(x) = expr else { return Err(MyError::...) };`
- Recovery logic that doesn't need the `Err` value: `if let Ok(x) = expr { ... } else { ... }`
- Fallback values: `unwrap_or`, `unwrap_or_else`, `unwrap_or_default`.
- Functions that must handle `None` internally should return `Result<T, E>` with a module/crate-level error.

## `thiserror` for libraries/crates

```rust
#[derive(Debug, thiserror::Error)]
pub enum MyError {
    #[error("Network Timeout")]
    Timeout,
    #[error("Invalid data: {0}")]
    InvalidData(String),
    #[error(transparent)]
    Serialization(#[from] serde_json::Error),
}
```

- Layered systems: nest error enums and convert with `#[from]` (`ServiceError::Db(#[from] DbError)`).
- Single failure mode → a `struct` error with one `#[error("...")]`, no enum needed.

## `anyhow` for binaries only

`anyhow` + `.context("...")` is great in `main`-adjacent binary code and test helpers. Don't use it in libraries: `anyhow::Result` erases the error types callers need, and scattered context strings are harder to maintain than `thiserror` messages. Likewise avoid `Box<dyn std::error::Error>` in library APIs.

## Propagation and inspection

- Bubble with `?`, not `match` chains.
- Recovery: `or_else` / `map_err` / `if let Ok(..)`. Logging: `inspect_err`:

```rust
value
    .inspect_err(|err| tracing::error!("fn_name: {err}"))
    .map_err(GeneralError::from)?;
```

## Async errors

Errors crossing `.await` boundaries or spawned tasks must be `Send + Sync + 'static`.

## Test your errors

Unit tests should exercise error paths, not just happy paths. If the error type lacks `PartialEq`, assert on `err.to_string()`; otherwise `assert_eq!` the error value directly. This keeps error messages meaningful and validated.