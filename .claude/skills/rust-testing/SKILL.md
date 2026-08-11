---
name: rust-testing
description: Rust testing practices — descriptive test names, one behavior per test, rstest, doc tests, this repo's co-located _test.rs/_spec.rs test tiers, assert macros, insta snapshot testing, property testing/fuzzing, and compile-fail tests. Load when writing or reviewing Rust tests.
---

## Tests as living documentation

Tests are the first place people look to understand code. Name them so `cargo test` output reads like a sentence: **unit_of_work** (function under test — often the test `mod` name) + **expected behavior** + **state checked**.

```rust
// src/thing_test.rs (sibling unit-test file — see wiring below)
use super::*;

mod process {
    use super::*;

    #[test]
    fn returns_error_xyz_when_b_is_negative() { ... }
    #[test]
    fn returns_invalid_input_error_when_a_and_b_not_present() { ... }
}
```

- Never `fn test_add_happy_path()`; group per-function tests in a `mod` instead of prefixing names.
- **One behavior per test, ideally one assertion.** Repetitive cases → shared setup function or `rstest` cases with descriptive `#[case::name(...)]` labels (caveat: rstest tests are harder for IDEs/humans to locate individually).
- Share **setup/fixtures only** — keep each test's action and assertion inline even if repetitive (DAMP over DRY; test helpers have no tests of their own).
- `Ok`-scenario asserts should print the `Err` on failure: `assert!(r.is_ok(), "error: {:?}", r.unwrap_err())`.

## Test tiers (this repo: co-located sibling files)

This repo maps the TS `*.test.ts` / `*.spec.ts` convention onto Rust (full rules in the project-structure skill). Tests sit **next to the source file**, not in a distant `tests/` tree:

| Tier | File | Access |
|------|------|--------|
| Unit | `foo_test.rs` sibling of `foo.rs` | internals allowed (`use super::*`) |
| Integration | `foo_spec.rs` sibling of `foo.rs` | public API only (`crate::…`, never `super::`) |
| Black-box | `tests/*.rs` at crate root | separate crate; only for public-API/linking guarantees |

Wire sibling files in with `#[path]` under `#[cfg(test)]` — **every `*_test.rs`/`*_spec.rs` needs its `mod` declaration or Cargo silently ignores it**:

```rust
// src/foo.rs
#[cfg(test)]
#[path = "foo_test.rs"]
mod foo_test;

#[cfg(test)]
#[path = "foo_spec.rs"]
mod foo_spec;
```

- **Unit tests** (`_test.rs`): focus on implementation, errors, and edge cases. Tiny test blocks may stay inline as `#[cfg(test)] mod tests`; extract to the sibling file once they grow past a screen. `#[ignore = "reason"]` for unimplemented, `#[should_panic(expected = "substring")]` only when panic is the desired behavior — always pass `expected` (without it any panic passes), and note it doesn't work on `Result`-returning test fns.
- **Integration tests** (`_spec.rs`): happy paths and real use cases through public `crate::` paths; external state allowed (testcontainers helps). For binaries, split `src/main.rs` (executable) from `src/lib.rs` (functions) so tests can reach the logic.
- Filter tiers by module-path match: `cargo test _test` / `cargo test _spec`.
- **Doc tests**: `///` examples run under `cargo test` (but NOT `cargo nextest run` — run `cargo t --doc` separately). Hide setup lines with a leading `#`. Duplication between doc tests and unit tests is fine. Block attributes: `no_run` (compile only), `should_panic`, `compile_fail` (demonstrate wrong usage), `ignore` (avoid; use `text` for non-code). A doctest using `?` can skip `fn main` by ending with a hidden `# Ok::<_, Box<dyn Error>>(())` line.
- **Examples**: fully-fledged programs in `examples/` (`cargo run --example name`) show typical library usage; not executed by `cargo test`.
- Test helpers live under `#[cfg(test)]` so they stay out of production builds; reach for mocking crates only when genuinely needed.

## Asserting

- `assert!`/`assert_eq!` accept format strings — say what the actual state was and how it differs.
- Pattern-only checks: since Rust 1.96 prefer `assert_matches!(error, MyError::BadInput(_))` over `assert!(matches!(...))` — it prints the actual value for free.
- Helpers: `rstest` (fixtures/cases), `pretty_assertions` (colored diffs).
- In production code, cheap precondition/postcondition checks can live as `assert!`/`debug_assert!` — they self-verify whether or not the code is under test.

## Property testing and fuzzing

- Property tests generate random inputs against an invariant: outputs stay in bounds, type invariants hold (e.g. len ≤ capacity), parsers may fail but never crash on garbage, fast and naive implementations agree. Good tools **shrink** failing inputs to a minimal case.
- Crates: `quickcheck` (fast, simple) or `proptest` (richer strategies + failure persistence — failing cases are saved to files and re-run as regressions):

```rust
proptest! {
    #[test]
    fn check_count_correct(haystack: Vec<u8>, needle: u8) {
        prop_assert_eq!(count(&haystack, needle), naive_count(&haystack, needle));
    }
}
```

- Coverage-guided fuzzing via `cargo-fuzz` (`cargo fuzz init` / `add` / `run`, nightly toolchain) for anything that consumes arbitrary bytes.
- Compile-fail beyond doctests: `trybuild` (or rustc's `compiletest`) UI tests assert on compiler error output — worth it when "this misuse doesn't compile" is a core guarantee of your API. Expect churn when rustc diagnostics improve.

## Snapshot testing (`insta`)

Use for output that is visual/structural: generated code, serialized data, rendered HTML, CLI output. Prefer YAML snapshots (`insta = { features = ["yaml"] }`); review with `cargo insta test` / `cargo insta review`.

- Name snapshots (`assert_snapshot!("app_config/http", cfg.http)`) and keep them **small** — never snapshot a huge object.
- Don't snapshot simple values (use `assert_eq!`), critical-path logic (use precise unit tests), or external resources (mock them).
- Redact unstable fields (timestamps, uuids): `".created_at" => "[timestamp]"`.
- Commit `snapshots/` to git and review diffs carefully before accepting.