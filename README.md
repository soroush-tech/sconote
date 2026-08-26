# Sconote

Turn playing into notes. Sconote is a Rust DSP core that does two things:

- **Live tuner** - monophonic pitch detection (McLeod Pitch Method): mic in, note
  name + cents offset out, per analysis window.
- **Offline transcription** - polyphonic note transcription (Spotify's Basic
  Pitch CNN, run in pure Rust via `tract`): a recording in, notes out - and from
  those, MIDI, MusicXML and engraved sheet music.

One core, two thin binding crates (WASM for the web, UniFFI for iOS/Android),
and the apps on top. All DSP and inference is on-device; nothing is uploaded.

**Demo:** <https://soroush-tech.github.io/sconote/> - the web app, running
entirely in your browser.

**Stack:** Rust workspace (edition 2024, stable MSVC) + pnpm/turborepo monorepo
(Node 26).

---

## Repository map

| Path | What it is |
| --- | --- |
| `crates/sconote-core` | Monophonic pitch engine + note tracker. No I/O, no audio capture. |
| `crates/sconote-poly` | Polyphonic transcription: decode → resample → CNN → notes → MIDI/MusicXML, plus the scoring harness it's tuned with. |
| `crates/sconote-wasm` | `wasm-bindgen` wrapper over both engines. |
| `crates/sconote-ffi` | UniFFI wrapper over both engines (Swift/Kotlin). |
| `packages/sconote-web` | `@sconote/web` - the `wasm-pack` build of `sconote-wasm`. |
| `packages/sconote-react-native` | `@sconote/react-native` - turbo-module scaffold; native sources are generated. |
| `apps/web` | Vite tuner + transcriber app. |
| `apps/mobile` | Planned, not yet scaffolded. |
| `examples/` | Reference material and Node scripts for accuracy comparison. |

### How the pieces depend on each other

```mermaid
flowchart TD
    subgraph rust["Rust workspace - Cargo"]
        core["sconote-core<br/><i>live monophonic engine</i>"]
        poly["sconote-poly<br/><i>offline polyphonic engine</i>"]
        onnx[("models/nmp.onnx<br/>230 KB, embedded")]
        wasmc["sconote-wasm<br/><i>wasm-bindgen</i>"]
        ffic["sconote-ffi<br/><i>uniffi 0.31</i>"]
        onnx -.->|"include_bytes!"| poly
        core --> wasmc
        poly --> wasmc
        core --> ffic
        poly --> ffic
    end

    subgraph js["JS workspace - pnpm + turbo"]
        pkgweb["@sconote/web<br/><i>wasm-pack build</i>"]
        pkgrn["@sconote/react-native<br/><i>ubrn build</i>"]
        appweb["apps/web<br/><i>Vite</i>"]
        appmob["apps/mobile<br/><i>planned</i>"]
        pkgweb -->|workspace dep| appweb
        pkgrn -.->|workspace dep| appmob
    end

    wasmc -->|"wasm32-unknown-unknown"| pkgweb
    ffic -->|"JSI + Swift/Kotlin<br/>Linux/macOS/CI only"| pkgrn
```

The two binding crates are deliberately logic-free - **any API change must be
mirrored in both.** 0% test coverage there is accepted; the core must stay
covered.

---

## Root folder - build & test workflow

`turbo` drives the JS side; the `@sconote/web` build task declares the Rust
sources as inputs, so touching a `.rs` file correctly invalidates the wasm cache.

```mermaid
flowchart LR
    A["pnpm dev --filter web"] --> B["turbo task: build @sconote/web"]
    B --> C["wasm-pack build<br/>crates/sconote-wasm<br/>--target web"]
    C --> D[("packages/sconote-web/dist/<br/>sconote.js + sconote_bg.wasm<br/><i>gitignored</i>")]
    D --> E["vite dev server<br/>localhost:5173"]

    F["cargo test --workspace"] --> G["core + poly unit/spec tests"]
    H["cargo clippy --all-targets"] --> I["lint gate"]
    J["cargo llvm-cov --workspace"] --> K["coverage of touched files"]
    L["git push → GitHub Actions"] --> M[".github/workflows/<br/>native-bindings.yml"]
    M --> N["ubrn:android on ubuntu<br/>ubrn:ios on macos"]
```

```bash
cargo test --workspace          # all Rust tests
cargo clippy --workspace --all-targets
cargo fmt
cargo build -p sconote-wasm --target wasm32-unknown-unknown   # WASM-cleanliness check
cargo llvm-cov --workspace      # coverage

pnpm install                    # JS deps (pulls the wasm-pack binary)
pnpm build                      # turbo: wasm-pack → vite
pnpm dev --filter web           # tuner app, rebuilds wasm first (~25 s)
```

Release profile is size-tuned (`opt-level = "s"`, LTO, `panic = "abort"`) because
the same artifact ships to WASM and to mobile.

---

## `crates/sconote-core` - live monophonic engine

Platform-agnostic: **there is no audio capture in the SDK.** Callers feed mono
f32 samples in `[-1, 1]`; `PitchEngine` accumulates arbitrary-size chunks (Web
Audio delivers 128 at a time) into 2048-sample analysis windows.

`NoteTracker` wraps the engine and answers two different questions from one
stream - what to *show* right now, and what to *record* as a played note.

```mermaid
flowchart TD
    IN["PCM chunk<br/>mono f32, -1 to 1"] --> BUF["PitchEngine buffer"]
    BUF -->|"under window_size"| WAIT["accumulate → None"]
    BUF -->|"window full"| MPM["McLeod Pitch Method<br/>power ≥ 0.15, clarity ≥ 0.7"]
    MPM -->|no clear pitch| NONE["None - silence / unpitched"]
    MPM -->|frequency + clarity| NE["NoteEvent<br/>freq · midi · name · cents · clarity"]

    NE --> LIVE["TrackerUpdate.live<br/><b>every</b> window → tuner needle"]
    NE --> GATE{"clarity ≥ 0.8 ?"}
    GATE -->|no| Q
    GATE -->|yes| SAME{"same MIDI as<br/>candidate?"}
    SAME -->|no| RESET["new candidate, count = 1"]
    SAME -->|yes| COUNT["count += 1"]
    COUNT -->|"count = 3 consecutive<br/>≈130 ms @ 48 kHz"| STARTED["TrackerUpdate.noteStarted<br/><b>once</b> per held note → history"]
    Q["quiet_windows += 1"] -->|"3 quiet windows"| REL["release - same pitch<br/>can be confirmed again"]
```

The gate is stricter than the engine's own threshold on purpose: a borderline
window still moves the tuner needle but never enters the note history.

`McLeodDetector` is `!Send` only because of internal `Rc<RefCell>` scratch
buffers that never escape it - hence the documented `unsafe impl Send` in
`lib.rs`. Don't remove it without reading the SAFETY comment.

**Files:** `lib.rs` (`PitchEngine`, `NoteEvent`) · `note.rs` (frequency ⇄ note
math) · `tracker.rs` (`NoteTracker`) · `tracker_test.rs` / `tracker_spec.rs` ·
`test_signals.rs`.

---

## `crates/sconote-poly` - offline polyphonic transcription

The heavy crate. Raw audio bytes in, sheet music out. The vendored
`models/nmp.onnx` is Spotify's Basic Pitch "nmp" network (Apache-2.0) - its
graph *includes* the CQT + harmonic-stacking frontend as Conv ops, so the input
is raw 22.05 kHz audio, not a spectrogram.

```mermaid
flowchart TD
    BYTES["file bytes"] --> SNIFF{"starts with RIFF?"}
    SNIFF -->|yes| WAV["wav.rs - hound<br/>int or float, any channels"]
    SNIFF -->|no| MP3["mp3.rs - symphonia<br/>MP3 only"]
    WAV --> MONO["MonoAudio<br/>mono f32 + sample_rate"]
    MP3 --> MONO
    MONO --> RS["resample.rs<br/>windowed sinc, 12 lobes<br/>→ 22 050 Hz"]

    RS --> WIN["transcribe.rs - window<br/>2 s windows, 30-frame overlap,<br/>half-overlap front pad"]
    WIN --> CNN["model.rs - BasicPitch via tract<br/>per window: 172 frames × 88 pitches"]
    CNN --> STITCH["stitch: trim 15 frames<br/>off each window edge"]
    STITCH --> ACT["Activations<br/>onset + frame probability matrices"]

    ACT --> NC["note_creation.rs<br/>onset peaks → track forward through frames<br/>zero consumed energy → melodia pass"]
    NC --> FILT["ghost filters<br/>subharmonic · retrigger octave veto · overtone"]
    FILT --> NOTES["TranscribedNote list<br/>midi · onset_s · offset_s"]

    NOTES --> MIDI["midi_export.rs<br/>→ .mid, 120 BPM grid"]
    NOTES --> TEMPO["tempo.rs<br/>estimate_bpm / track_beats"]
    TEMPO --> XML["musicxml.rs<br/>16th quantize · key detection<br/>grand staff split at middle C"]
    NOTES --> XML
    XML --> SCORE["MusicXML → any engraver"]
```

`Activations` are computed once and cheap to re-extract notes from under
different thresholds - which is what makes threshold tuning practical.

### Two entry points, same pipeline

| API | Use |
| --- | --- |
| `transcribe(&audio, &model, &options)` | Blocking, whole recording. Native / background thread. With the `parallel` feature (on in `sconote-ffi`) the windows run across all cores via rayon. |
| `WindowedTranscription::new` → `process_next_window` → `finish` | Cooperative. The browser drives one window per tick, yielding to the event loop so the page stays alive. |
| `WindowedTranscription::new` → `window_samples` / `predict_window` / `set_window` → `finish` | Distributed. Windows are independent: the web app hands them to a pool of Web Workers (each with its own `Transcriber`) and stores the results in any order. |

Windows are independent, so all three produce identical activations; only
the order of appending matters, and `finish` stitches by window index.

### Tuning loop

`note_creation.rs` deviates from the reference implementation in five documented
places, all aimed at spurious notes on real recordings. Those deviations are
justified by a measurement harness that also lives in this crate:

```mermaid
flowchart LR
    REC["real recording<br/>.wav / .mp3"] --> TR["transcribe"]
    REF[".mid ground truth"] --> GT["ground_truth.rs<br/>notes_from_midi<br/><i>channel 10 excluded</i>"]
    GT -.->|synthetic path| SY["synth.rs - render_notes<br/>harmonic tone + ADSR"]
    SY --> TR
    TR --> SC["score.rs - score_notes<br/>exact pitch + onset within tolerance<br/><i>offsets ignored: decay tails lie</i>"]
    GT --> SC
    SC --> REP["ScoreReport<br/>matched · missed · spurious"]
    REP --> KNOBS["adjust NoteCreationOptions"]
    KNOBS --> TR
```

### Example drivers

```bash
cargo run --release -p sconote-poly --example tune     -- <session.wav|mp3> <ref.mid>...
cargo run --release -p sconote-poly --example ablate   -- <render.wav> <ref.mid>
cargo run --release -p sconote-poly --example to_midi  -- <in.wav|mp3> <out.mid>
cargo run --release -p sconote-poly --example peek     -- <wav> <from_s> <to_s>
cargo run --release -p sconote-poly --example xml_dump -- <notes.mid> <out.musicxml>
```

- **`tune`** - grid-searches thresholds against ground truth, auto-solving the
  unknown time offset between a recording and the reference.
- **`ablate`** - which note-creation stage invents the extra notes? Re-extracts
  from one cached activation matrix under each combination of heuristics.
- **`xml_dump`** - the same MusicXML the apps ship, inspectable without a browser.

Feature choices are WASM-driven: `midly` without `parallel` (rayon panics on
wasm32), `symphonia` with MP3 only, and the crate's own `parallel` feature
(rayon over transcription windows) is off by default - only `sconote-ffi`
enables it. **This crate must stay WASM-clean.**

---

## `crates/sconote-wasm` → `packages/sconote-web` → `apps/web`

```mermaid
flowchart TD
    subgraph build["build"]
        RS["sconote-wasm/src/lib.rs"] -->|wasm-pack --target web| DIST[("@sconote/web/dist/")]
        DIST -->|"workspace:*"| VITE["apps/web via Vite"]
    end

    subgraph live["runtime - live tuner"]
        MIC["getUserMedia<br/>AEC/NS/AGC off"] --> WL["capture-worklet.js<br/>audio thread, 128-sample blocks"]
        WL -->|postMessage| MAIN["main thread"]
        MAIN --> NT["NoteTracker.process - wasm"]
        NT -->|"update.live"| NEEDLE["needle + note name<br/>±50 cents"]
        NT -->|"update.noteStarted"| HIST["note history"]
        MAIN -.->|while recording| CHUNKS["accumulate chunks"]
    end

    subgraph offline["runtime - transcription"]
        SRC{"source"}
        CHUNKS --> SRC
        UP["file upload"] --> DEC{"WAV or MP3?"}
        DEC -->|yes| RUSTDEC["decodeAudio - wasm<br/><i>identical samples in every browser</i>"]
        DEC -->|"no - m4a/ogg/..."| BROWSER["OfflineAudioContext @ 22 050 Hz<br/><i>resampled exactly once</i>"]
        RUSTDEC --> SRC
        BROWSER --> SRC
        SRC --> JOB["Transcriber.begin → TranscriptionJob"]
        JOB --> POOL["Web Worker pool - one Transcriber each<br/>windowSamples → predictWindow → setWindow"]
        JOB -.->|no workers| LOOP["processNextWindow + yield<br/>main-thread fallback"]
        POOL --> FIN["finish thresholds → TranscribedNotes"]
        LOOP --> FIN
        FIN --> OUT1["piano roll canvas"]
        FIN --> OUT2["toMidi → .mid"]
        FIN --> OUT3["toMusicXml → Verovio → SVG pages"]
        OUT3 --> PDF["jsPDF → A4 .pdf"]
        CHUNKS --> WAVDL["wav-encoder.js → .wav"]
    end
```

Engraving is delegated to Verovio: the Rust core makes the *notation* decisions
(quantization, key signature, staff split) and emits MusicXML; Verovio only
typesets it.

Two `vite.config.js` settings are load-bearing:

- `assetsInlineLimit: 0` - `audioWorklet.addModule` rejects `data:` URLs in some
  browsers, so the worklet must stay a real file.
- `optimizeDeps.exclude: ["@sconote/web", "verovio"]` - pre-bundling breaks the
  glue code's `new URL("..._bg.wasm", import.meta.url)` resolution.

WASM objects are manually freed (`.free()`) after each update - there's no GC
across the boundary.

---

## `crates/sconote-ffi` → `packages/sconote-react-native`

Same API surface as the WASM binding, expressed as UniFFI records and objects.
UniFFI objects must be `Send + Sync`, so the engines sit behind a `Mutex`.

```mermaid
flowchart TD
    FFI["sconote-ffi - uniffi::Object<br/>PitchDetector · NoteTracker · Transcriber"]
    FFI -->|"ubrn build android --and-generate"| AND["Rust .so per ABI<br/>+ Kotlin + JSI C++"]
    FFI -->|"ubrn build ios --and-generate"| IOS["Rust staticlib<br/>+ Swift + JSI C++"]
    AND --> RN["@sconote/react-native<br/>src/ cpp/ android/ ios/ - <i>all generated, gitignored</i>"]
    IOS --> RN
    RN -.-> MOB["apps/mobile - planned"]

    WIN["Windows dev machine"] -.->|"blocked: Smart App Control<br/>kills the ubrn CLI build<br/>(proc-macro DLLs, os error 4551)"| X["✗"]
    CI["GitHub Actions<br/>native-bindings.yml"] -->|ubuntu-latest| AND
    CI -->|macos| IOS
```

Everything under that package's `src/`, `cpp/`, `android/`, `ios/` and the
podspec is **generated and gitignored**. Generation runs on Linux/macOS or CI
only. The `uniffi` crate version must stay in lockstep with
`uniffi-bindgen-react-native` (both 0.31).

---

## `examples/` - accuracy reference material

A scratch directory (no `package.json`, plain Node scripts) holding a worked
end-to-end case: Bach BWV 846 as published MP3 + MIDI + PDF, alongside what
Sconote produced from it.

```mermaid
flowchart LR
    ORIG["bach_846/*.mp3 + *.mid + *.csv<br/><i>published reference</i>"] --> SC["Sconote<br/>web app or example driver"]
    SC --> OUT["sconote-transcription.json / .mid<br/>sconote-score.musicxml / .pdf"]
    OUT --> EX["extract.csv.js<br/>notes → CSV, chords low→high"]
    ORIG --> CMP["compare.mjs"]
    EX --> CMP
    OUT --> CMP
    CMP --> M1["sequence mode - LCS<br/><i>CSV has no timestamps</i>"]
    CMP --> M2["timing mode - pitch + onset tolerance<br/><i>after solving the offset</i>"]
    MF["midi-file.mjs<br/><i>hand-rolled SMF reader</i>"] -.-> CMP
```

```bash
node examples/compare.mjs examples/bach_846/...csv  .../sconote-transcription.json   # sequence
node examples/compare.mjs examples/bach_846/...mid  .../sconote-transcription.mid    # timing
```

---

## Conventions

- Tests are co-located: `foo.rs` + `foo_test.rs` (unit) + `foo_spec.rs`
  (behavioural). See `.claude/skills/project-structure`.
- After any implementation: add tests, run `cargo test`, check
  `cargo llvm-cov` on touched files. Coverage gaps should be deliberate.
- API changes must land in **both** `sconote-wasm` and `sconote-ffi`.
- Crate names use `sconote`; the checkout directory is `scornOn` (not a valid
  crate name).

### Environment quirks (Windows)

- **Smart App Control** sometimes blocks freshly compiled build scripts
  (`os error 4551`) on first run. For workspace `cargo` commands it's transient -
  re-run. For the `uniffi-bindgen-react-native` CLI it's permanent; bindings
  generation only works on Linux/macOS/CI.
- iOS builds require a Mac.

## Licence & attribution

The bundled `crates/sconote-poly/models/nmp.onnx` is Spotify's
[Basic Pitch](https://github.com/spotify/basic-pitch) `icassp_2022/nmp.onnx`
model, Apache-2.0. `note_creation.rs` is a port of that project's
`output_to_notes_polyphonic`, with five documented deviations.
