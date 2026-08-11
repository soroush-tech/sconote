---
name: rust-linting
description: Clippy discipline — the full lint command, key lints to respect, expect-over-allow policy, and Cargo.toml lint tables. Load when running lints, fixing clippy warnings, or configuring lint levels in a Rust workspace.
---

## The command

Run clippy as part of every change (also belongs in CI/Makefile/xtask):

```shell
cargo clippy --all-targets --all-features --locked -- -D warnings
```

- `--all-targets`: lib, tests, benches, examples.
- `--all-features`: all features enabled.
- `--locked`: requires an up-to-date `Cargo.lock` (fix with `cargo update`).
- `-D warnings`: warnings are errors.
- Optional stricter passes: `-W clippy::pedantic`, `-W clippy::nursery`.

## Lints to take seriously

| Lint | Why |
|------|-----|
| `redundant_clone` | Unnecessary clones — performance |
| `needless_borrow` | Redundant `&` |
| `map_unwrap_or` / `unnecessary_map_or` | Simplify nested Option/Result handling |
| `manual_ok_or` | Use `.ok_or_else` instead of `match` |
| `large_enum_variant` | Oversized variant — `Box` it |
| `unnecessary_wraps` | Function always returns `Ok`/`Some` → drop the wrapper |
| `clone_on_copy` | `.clone()` on a `Copy` type |
| `needless_collect` | Collecting an iterator that didn't need allocation |

## Fix warnings, don't silence them

- Never blanket-`#[allow(...)]`. If a lint is genuinely wrong for the code, use `#[expect(clippy::lint_name)]` **with a comment explaining why** — `expect` warns again if the lint stops firing, so the suppression can't go stale.
- For false positives: first try refactoring so the warning improves; only then `#[expect]` locally. Avoid crate-global overrides.

## Configuring lint levels in Cargo.toml

Package (`[lints.*]`) or workspace (`[workspace.lints.*]`); higher `priority` wins on conflicts:

```toml
[workspace.lints.rust]
future-incompatible = "warn"
nonstandard_style = "deny"

[workspace.lints.clippy]
all = { level = "deny", priority = 10 }
redundant_clone = { level = "deny", priority = 9 }
pedantic = { level = "warn", priority = 3 }
```