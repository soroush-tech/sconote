# @sconote/react-native

React Native bindings for the sconote pitch-detection core, generated from
[`crates/sconote-ffi`](../../crates/sconote-ffi) with
[uniffi-bindgen-react-native](https://github.com/jhugman/uniffi-bindgen-react-native)
(UniFFI → JSI turbo-module).

## Generated code

Everything under `src/`, `cpp/`, `android/`, `ios/` plus the podspec is
**generated** and gitignored — never hand-edit it. To (re)generate bindings and
build the native libraries:

```sh
pnpm ubrn:android   # Linux/macOS — needs Android NDK + Rust android targets
pnpm ubrn:ios       # macOS only — builds the xcframework
```

Neither works on Windows; CI (`.github/workflows/native-bindings.yml`) runs
both. After changing the Rust API in `sconote-ffi`, re-run the commands above
(they pass `--and-generate`, which refreshes the TypeScript/C++ bindings).

Keep the `uniffi` version in `sconote-ffi/Cargo.toml` in lockstep with the
`uniffi-bindgen-react-native` version in `package.json`.
