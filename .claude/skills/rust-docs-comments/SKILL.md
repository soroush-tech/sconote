---
name: rust-docs-comments
description: Rust comments vs documentation — when to use // comments (why, not what), named comment tags (SAFETY/PERF/CONTEXT), TODO→issue policy, /// and //! doc comments, doc lints, and a doc coverage checklist. Load when writing comments, rustdoc, or reviewing documentation in Rust code.
---

## `//` comments: why, not what or how

Good code with expressive types and names mostly needs no comments. Write one only for what code can't express:

- Safety guarantees — `// SAFETY: ptr is non-null and aligned by caller` (name your tags: `SAFETY:`, `PERF:`, `CONTEXT:`).
- Performance quirks and workarounds; platform/legacy behavior (prefer `#[cfg(..)]` where possible).
- Non-obvious assumptions, with links to design docs/ADRs for depth.

Avoid: restating the obvious (`// increment by 1`), walls of text (use `///` if it describes the item), stale history notes, and anything a better name or extracted function could replace. **Extract when the name adds meaning** — not to dedupe a couple of similar lines (see rust-idioms §extraction).

Comments are not "living documentation" — they rot, mislead, and clutter. If it deserves to outlive the PR, put it in an ADR/design doc, in types and doc comments, or in tests. When you find a comment, read it critically; fix or delete it if outdated — a misleading comment is worse than none.

## TODOs become issues

No orphan `// TODO:` comments. File a GitHub issue/ticket and reference it: `// TODO(issue #42): remove workaround after bugfix`.

## `///` and `//!` doc comments

| | Use for |
|---|---|
| `///` | Public items: fn, struct, enum, trait, const — purpose, usage, `# Examples`, `# Errors`, `# Panics`, `# Safety` |
| `//!` | Module/crate purpose at top of `lib.rs`/`mod.rs` — what it does, quickstart example |

- Document **all public items**: what it does, parameters, return behavior, edge cases, examples.
- Prefer runnable examples (they double as doc tests); hide assertion lines with a leading `#`.
- Link types with intra-doc links: `` [`MyError::FileNotFound`] ``.
- Check output with `cargo doc --open`.

```rust
/// Loads [`User`] profile from disk.
///
/// # Errors
/// - [`MyError::FileNotFound`] if the file is missing.
/// - [`MyError::InvalidJson`] if the content is invalid JSON.
fn load_user(path: &Path) -> Result<User, MyError> { ... }
```

## Doc lints worth enabling

`missing_docs`, `broken_intra_doc_links`, `clippy::empty_docs`, `clippy::missing_panics_doc`, `clippy::missing_errors_doc`, `clippy::missing_safety_doc`. For full coverage enforcement: `#![deny(missing_docs)]` at crate root.

## Coverage checklist

- Crate: `//!` explaining what it does and what problems it solves, with examples.
- Modules: `//!` purpose, exports, invariants.
- Types: role, invariants, example usage; consider `#[non_exhaustive]` if external users may match on it.
- Functions: behavior, params, returns, `# Panics`/`# Errors`, `# Examples`.
- Traits: purpose, per-method when/why to implement, which defaults to override.