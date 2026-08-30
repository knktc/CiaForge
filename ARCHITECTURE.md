# CiaForge architecture

## Scope

CiaForge is a Tauri desktop application that reimplements the CCI / 3DS to CIA conversion behaviour in Rust. The Python `3dsconv` project is a behavioural reference only; it is not a runtime dependency.

The current release accepts `.cci` and `.3ds` files, converts them one at a time, and presents a queue UI:

- source file plus parent folder and size;
- current state and progress for each job;
- output policy selected once for the whole queue: beside each source (default), or one shared folder;
- collision-safe output names (`Game.cia`, `Game_1.cia`, ...);
- completed output can be revealed in Finder.

## Workspace shape

```text
src-tauri/
  src/
    lib.rs              Tauri commands and progress-channel bridge
    conversion/
      mod.rs            Public conversion exports
      cci.rs            CCI / NCSD parsing and validation
      ncch.rs           NCCH headers, crypto mode detection and content layout
      cia.rs            CIA header, TMD, content records and writer
      copy.rs           content copying, hashing, and progress reporting
      engine.rs         unencrypted conversion and atomic output replacement
      error.rs          typed user-facing conversion errors
      templates.rs      embedded retail CIA template loading
      writer.rs         CIA header serialization
  assets/               compressed retail CIA template data
  capabilities/         frontend permissions
src/
  main.ts               drag-drop, queue state and Tauri channel subscription
  styles.css            approved dark single-workspace design
```

## Modules and seams

`convert_unencrypted` is the core conversion seam. Its interface is intentionally small:

```rust
fn convert_unencrypted(input: &Path, output: &Path, progress: &mut impl ProgressSink) -> Result<(), ConversionError>;
```

`ProgressSink` receives byte-based progress updates without knowing about Tauri. The Tauri adapter resolves an available output path and forwards progress through a Channel, while unit tests use an in-memory recorder.

The UI crosses one conversion seam: `start_conversion(requests, output_mode, output_path, channel)`. It never reads ROM data or decides CIA layout. Rust resolves the selected output policy and the next available output name before calling the engine, so shared-folder and source-folder collisions behave identically.

## Conversion stages

1. Validate input suffix and NCSD / NCCH headers.
2. Detect unencrypted, zero-key, or original-NCCH material; reject unsupported encryption explicitly.
3. Resolve a collision-safe output name.
4. Write CIA header and TMD placeholders.
5. Copy each content region while computing SHA-256, reporting bytes after every bounded buffer write.
6. Write final content and information hashes, then atomically rename the temporary CIA.

Any error removes the partial file and preserves existing CIA files. Existing names receive an incrementing suffix rather than being overwritten.

## Tauri responsibilities

- The frontend uses Tauri's native webview drag-drop event to receive paths.
- Rust resolves output paths and validates input structure before conversion.
- The `start_conversion` command streams typed progress updates through a Tauri Channel.
- Finder reveal is a narrow Rust command available only for a completed output path.
- No frontend file-system capability is required for conversion.

## Delivery order

1. Extend encryption support only with legal fixtures and output comparisons against a known-good implementation.
2. Keep source, shared-folder, collision, and error behaviour covered by unit tests.
3. Package only after explicit approval.
