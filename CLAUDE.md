# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Behavioral guidelines

### 1. Think before coding

**Don't assume. Don't hide confusion. Surface tradeoffs.**

- State your assumptions explicitly. If uncertain, ask.
- If multiple interpretations exist, present them - don't pick silently.
- If a simpler approach exists, say so. Push back when warranted.

### 2. Propose before implementing

**Always present a plan and wait for confirmation before writing code.**

Cover: which files change, what changes in each, why. Then wait for explicit approval.

Exception: self-evident one-liners (typo fix, missing import).

### 3. Simplicity first

**Minimum code that solves the problem. Nothing speculative.**

- No features, abstractions, or error handling beyond what was asked.
- If you write 200 lines and it could be 50, rewrite it.

Ask yourself: "Would a senior engineer say this is overcomplicated?" If yes, simplify.

### 4. Surgical changes

**Touch only what you must. Clean up only your own mess.**

- Don't refactor, reformat, or "improve" adjacent code.
- Remove imports/variables/functions that YOUR changes made unused.
- Match existing style.
- If you notice unrelated dead code, mention it - don't delete it.

### 5. Goal-driven execution

**Define success criteria. Loop until verified.**

Transform tasks into verifiable goals: "Fix the bug" → "Write a test that reproduces it, then make it pass."

- "Refactor X" → "Ensure tests pass before and after"

For multi-step tasks, state a brief plan:

```
1. [Step] → verify: [check]
2. [Step] → verify: [check]
3. [Step] → verify: [check]
```

Strong success criteria let you loop independently. Weak criteria ("make it work") require constant clarification.

**These guidelines are working if:** fewer unnecessary changes in diffs, fewer rewrites due to overcomplication, and clarifying questions come before implementation rather than after mistakes.

### 6. Test coverage after implementation

**After any implementation, add tests for the new logic, run `cargo test`, and check coverage of touched files with `cargo llvm-cov`.**

- Coverage gaps in touched files should be deliberate (e.g. `main`, trivial glue), not overlooked. Mention any gaps you leave.
- Requires a one-time install: `cargo install cargo-llvm-cov`.


## Project

Sconote - monophonic pitch detection (instrument → note name) shipped as one Rust core with a mobile app (React Native) and a web app (WASM) on top. Rust workspace + pnpm/turborepo monorepo. Toolchain: stable MSVC (rustc 1.97+), edition 2024, via rustup; Node 26 + pnpm.

## Commands

```
cargo test --workspace                                  # all Rust tests
cargo test -p sconote-core <name>                       # single test by name filter
cargo clippy --workspace --all-targets                  # lint
cargo fmt                                               # format
cargo build -p sconote-wasm --target wasm32-unknown-unknown   # verify WASM-cleanliness
cargo llvm-cov --workspace                              # coverage (rule 6)
pnpm install                                            # JS deps (incl. wasm-pack binary)
pnpm build                                              # turbo: wasm-pack → vite
pnpm dev --filter web                                   # tuner app at localhost:5173 (rebuilds the wasm first, ~25 s)
```

## Architecture

Dual-binding strategy: the DSP core is pure Rust; each platform gets its own thin binding crate, and API changes must be mirrored in BOTH binding crates.

- `crates/sconote-core` - all logic lives here. `PitchEngine` accumulates arbitrary-size PCM chunks into analysis windows (McLeod Pitch Method via the `pitch-detection` crate), emits `NoteEvent` (frequency, MIDI, note name, cents offset, clarity). `NoteTracker` (`tracker.rs`) layers noise filtering on top: clarity gate + consecutive-window debounce turning raw detections into a confirmed note history. `note.rs` is the frequency⇄note math. **No audio capture in the SDK** - platforms own the microphone (Web Audio / native modules) and feed f32 samples in [-1, 1].
- `crates/sconote-wasm` - wasm-bindgen wrapper → built by `packages/sconote-web` (`@sconote/web`, wasm-pack into its gitignored `dist/`), consumed by `apps/web` as a workspace dependency.
- `crates/sconote-ffi` - UniFFI proc-macro wrapper → consumed by `packages/sconote-react-native`. The `uniffi` version must stay in lockstep with the `uniffi-bindgen-react-native` version in that package (currently 0.31). uniffi objects must be `Send + Sync`: `sconote-core` has a documented `unsafe impl Send` for the detector (whose `Rc<RefCell>` scratch buffers never escape it) - don't remove it without reading that safety comment.
- `apps/web` - Vite tuner app: mic → AudioWorklet (128-sample chunks) → `NoteTracker` (live tuner display + note history). `vite.config.js` must keep `assetsInlineLimit: 0` (audioWorklet.addModule rejects data: URLs in some browsers - the worklet must stay a real file) and `optimizeDeps.exclude: ["@sconote/web"]` (pre-bundling breaks the glue code's wasm URL resolution).
- `packages/sconote-react-native` - create-react-native-library turbo-module scaffold + uniffi-bindgen-react-native. Everything under its `src/`, `cpp/`, `android/`, `ios/` and the podspec is **generated and gitignored**; generation/native builds run only on Linux/macOS (`pnpm ubrn:android` / `ubrn:ios`, or CI via `.github/workflows/native-bindings.yml`) - neither works on Windows.
- `apps/mobile` - planned, not yet scaffolded.

Binding crates stay logic-free (0% coverage there is accepted; core must stay covered). iOS builds require a Mac - not possible on this machine.

## Environment quirks

- Smart App Control is ON and sometimes blocks freshly compiled build scripts (`os error 4551`) on first execution. For workspace cargo commands it's transient: simply re-run. But it blocks the `uniffi-bindgen-react-native` CLI build (proc-macro DLLs, retries don't help) permanently - bindings generation only works on Linux/macOS/CI.
- Crate names use `sconote` (directory `scornOn` is not a valid crate name).
