---
name: rust-type-state
description: Type State Pattern — encoding states as types with PhantomData so illegal states and transitions fail at compile time; compile-time-checked builders and protocol state machines. Load when designing Rust APIs with required initialization, state machines, or builders with mandatory fields.
---

Encode system **states as types** (not runtime flags/enums) so invalid states and transitions become compile errors. The `PhantomData` marker is erased at compile time — zero memory cost.

## When to use it

- You want compile-time state safety and API constraints (e.g. can't `read` before `open`, can't `send` before `connect`).
- Replacing runtime booleans/enums with type-safe code paths.
- Library APIs where reaching a state guarantees data validity (no re-checks needed).

## When NOT to

- Trivial states a plain enum handles fine; runtime flexibility is required; the generics get overcomplicated. Costs: verbosity, complex signatures, some field duplication, `PhantomData` is unintuitive for beginners. Use it when it **saves bugs**, not for cleverness.

## Core shape

```rust
struct FileNotOpened;
struct FileOpened;

struct File<State> {
    path: PathBuf,
    handle: Option<std::fs::File>,
    _state: std::marker::PhantomData<State>,
}

impl File<FileNotOpened> {
    // The only entry point: a valid path yields the next state.
    fn open(path: &Path) -> io::Result<File<FileOpened>> {
        let file = std::fs::File::open(path)?;
        Ok(File { path: path.to_path_buf(), handle: Some(file), _state: PhantomData })
    }
}

impl File<FileOpened> {
    fn read(&mut self) -> io::Result<String> {
        let Some(handle) = self.handle.as_mut() else {
            unreachable!("state only reachable when file is open");
        };
        // ...
    }
}
```

Transitions consume `self` and return the struct parameterized by the next state type. Fields guaranteed set by the state can use `unreachable!` in the impossible branch.

## Builder with required fields

Use one marker parameter per required field (`Builder<HasName, HasAge>`), starting at `Builder<MissingName, MissingAge>`. Each setter returns the builder with that marker flipped (`MissingName` → `NameSet`); optional fields (e.g. `email`) appear only where allowed; `build()` is implemented **only** on `Builder<NameSet, AgeSet>`. Forgetting a required field then fails to compile:

```rust
Builder::new().name(n).age(30).build();  // ✅
Builder::new().age(30).build();          // ❌ compile error: name required
```

Same technique models protocol state machines: `Client<Disconnected>::connect()` → `Client<Connected>::send()` — sending before connecting simply doesn't compile.