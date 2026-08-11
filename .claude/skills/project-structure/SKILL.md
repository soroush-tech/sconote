---
description: Folder structure and logic-split conventions for this Rust + TypeScript monorepo — workspace layout, co-location doctrine, one-file-per-helper extraction, the test tiers on both sides (Rust `_test.rs`/`_spec.rs`/`tests/`, TS `*.test.ts`/`*.spec.ts`/`*.e2e.ts`), and when to promote local code to a shared crate/package. Use when creating files, deciding where code lives, extracting helpers, or placing tests.
---

## Workspace layout

```
repo/
├── Cargo.toml            # [workspace] — members = ["crates/*"]
├── pnpm-workspace.yaml   # packages: ["apps/*", "packages/*"]
├── crates/               # Rust workspace members
│   └── <crate-name>/
│       ├── Cargo.toml
│       ├── src/
│       └── tests/        # black-box integration tests only (see below)
├── packages/             # shared TS packages (scoped @org/*)
└── apps/                 # TS apps (and any Rust binaries' TS frontends)
```

- Rust crates never live under `packages/`; TS never lives under `crates/`.
  A feature that needs both (e.g. a Rust core with WASM/N-API bindings consumed
  by TS) is **two** workspace members: `crates/<name>` + `packages/<name>`,
  with the generated binding artifacts gitignored in the TS package.
- Cross-language contracts (types shared between Rust and TS) are generated
  from the Rust side (e.g. via a codegen step) — never hand-maintained twice.

## Co-location doctrine (both languages)

**Everything a unit of code needs sits next to it: tests, helpers, constants,
static data.** Distance is earned by reuse, not by category.

The promotion rule: code stays local to the module/component that owns it
until a **second** consumer appears. Then:

- TS: promote to the package's `src/utils/` or `src/hooks/` — or to a
  `packages/*` workspace package if the second consumer is another package.
- Rust: promote to the crate root's `src/utils/` module — or to a shared
  crate under `crates/` if the second consumer is another crate.

Never create a `shared/`, `common/`, or `helpers/` dumping ground
speculatively.

## Splitting logic out of a module

When a module grows mixed concerns, extract by kind — same rules both sides:

| Concern              | TypeScript                                    | Rust                                        |
| -------------------- | --------------------------------------------- | ------------------------------------------- |
| Pure helpers (few)   | `Name/utils.ts` + `utils.test.ts`             | `name/utils.rs` (with inline `mod tests`)   |
| Pure helpers (many)  | `Name/utils/helperName.ts` + its `.test.ts`   | `name/utils/helper_name.rs` + its tests     |
| Constants            | `Name/const.ts`                               | `name/consts.rs`                            |
| Static data          | `Name/Name.data.ts`                           | `name/data.rs` (or a `.json`/`.toml` asset) |

- **One file per helper** once there's more than a couple: each helper gets
  its own file named after it, with its test file (TS) or test module (Rust)
  co-located. No grab-bag modules that grow forever.
- A helper file exports/declares exactly one public item (plus its types).
- Rust module folders use the modern layout: `name.rs` + `name/` subfolder,
  not `name/mod.rs`.

## Test tiers

All tiers co-located next to source. The unit/integration split is by suffix
in both languages, so tiers are filterable in both runners.

| Tier        | TypeScript             | Rust                            | Access discipline                      |
| ----------- | ---------------------- | ------------------------------- | -------------------------------------- |
| Unit        | `foo.test.ts(x)`       | `foo_test.rs` (sibling)         | internals allowed (`super::*`)         |
| Integration | `foo.spec.ts(x)`       | `foo_spec.rs` (sibling)         | public API only (`crate::…`)           |
| Black-box   | `*.e2e.ts` (Playwright)| `tests/*.rs` at crate root      | separate crate / real browser          |

### Rust wiring

Sibling test files don't match the default module layout, so declare them
with `#[path]` under `#[cfg(test)]`:

```rust
// src/parser.rs
#[cfg(test)]
#[path = "parser_test.rs"]
mod parser_test;

#[cfg(test)]
#[path = "parser_spec.rs"]
mod parser_spec;
```

- Tiny test blocks may stay as an inline `#[cfg(test)] mod tests` instead of
  a sibling file; extract to `_test.rs` once it grows past a screen.
- `_spec.rs` files must not contain `use super::` — they exercise the crate
  through `crate::` public paths only. This is convention, not compiler-
  enforced; grep for `super::` in `*_spec.rs` as the lint.
- Filter tiers: `cargo test _test` / `cargo test _spec` (module path match).
- **Every `*_test.rs` / `*_spec.rs` must have a matching `mod` declaration**
  — undeclared files are silently ignored by Cargo. CI runs a check script
  asserting the pairing; never delete a test file's `mod` line without the
  file.
- `tests/` at crate root is reserved for true black-box tests that must be
  compiled as a separate crate (public-API guarantees, linking behavior).
  Don't put there what a co-located `_spec.rs` can cover.

### TypeScript wiring

- Unit + integration run under vitest, one project; e2e under Playwright,
  next to the page/entry it covers, shared e2e infra in `src/test/e2e/`.
- Filter tiers by suffix glob: `vitest run **/*.spec.*` etc.

## Coverage

100% on all touched files, both sides:

- TS: `pnpm test:coverage` per package; per-file gate via the repo's
  coverage-check script over `lcov.info`.
- Rust: `cargo llvm-cov --workspace --lcov` — the same per-file gate script
  parses its lcov output. No `#[cfg(not(tarpaulin))]`-style coverage-ignore
  attributes; reach 100% with real tests or delete dead code.

## Imports

- TS: workspace packages via their scope (`@org/<pkg>`), app-internal via the
  `src/` alias — never relative `../../`.
- Rust: workspace crates via `workspace = true` dependencies in `Cargo.toml`;
  version and shared dependency declarations live in the root
  `[workspace.dependencies]` table, members inherit.
