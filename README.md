# CiaForge

CiaForge is a focused Tauri desktop app for converting supported Nintendo 3DS
`.cci` and `.3ds` images to `.cia`. It pairs a native file picker and
drag-and-drop queue with a Rust conversion engine.

> Use only files you are legally entitled to convert and install. CiaForge does
> not include game content, keys, or firmware material.

## Features

- Drag in one or many `.cci` / `.3ds` files, or select them from the native
  file browser.
- See file names, sizes, conversion status, per-file progress, and full error
  details on hover when text is truncated.
- Choose a single output policy in Settings: next to each source file (default)
  or one shared output folder.
- Add more files before a batch begins; clear the batch to return to the import
  screen.
- Keep conversion sequential and prevent duplicate batch starts while a batch
  is running.
- Avoid overwrites automatically: `Game.cia`, `Game_1.cia`, `Game_2.cia`, and
  so on.
- Reveal a successfully converted file in Finder.
- Switch the interface between English and Simplified Chinese.

## Current support and limits

The current Rust engine supports **unencrypted** NCCH content inside `.cci` or
`.3ds` images. It validates CCI/NCCH structure, patches required CIA metadata,
and writes CIA content records with SHA-256 integrity data.

Original-NCCH and zero-key encrypted inputs are detected but are not supported
yet. They fail with a clear per-file error instead of producing a partial CIA.

## Development

### Prerequisites

- A current Node.js LTS release and npm
- A current stable Rust toolchain
- The native build prerequisites required by [Tauri v2](https://v2.tauri.app/start/prerequisites/)

### Run locally

```bash
npm install
npm run tauri dev
```

### Verify

```bash
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
```

### Build a distributable application

```bash
CI=false npm run tauri -- build --target aarch64-apple-darwin --bundles dmg
```

Use `x86_64-apple-darwin` instead of `aarch64-apple-darwin` for an Intel Mac.
The DMG is written beneath `src-tauri/target/<target>/release/bundle/dmg/`;
these generated files are intentionally ignored by Git.

## GitHub Actions builds

The workflow at [`.github/workflows/build-desktop.yml`](.github/workflows/build-desktop.yml)
builds separate macOS arm64 and x86_64 DMGs, plus the existing Windows bundle.
It runs for pull requests, manual dispatches, and tags beginning with `v` (for
example, `v0.2.0`). Each run uploads three downloadable artifacts:

- `CiaForge-macOS-arm64` (DMG)
- `CiaForge-macOS-x86_64` (DMG)
- `CiaForge-Windows` (EXE)

When a version tag is pushed, a separate release job downloads the build
artifacts and publishes the two macOS DMGs plus the Windows `.exe` installer to
the matching GitHub Release. The macOS DMGs use an ad-hoc signature for bundle
integrity, but are not notarized without Apple signing secrets.

### Opening the ad-hoc-signed macOS build

Without an Apple Developer account, macOS builds are ad-hoc signed but
unnotarized; a quarantined download may therefore still be reported as damaged
or rejected by Gatekeeper. Only use an artifact that you built yourself or
obtained from a repository you trust. Do not disable Gatekeeper system-wide. A
production release that opens normally after download needs a Developer ID
Application signature and notarization.

After moving `CiaForge.app` to `/Applications`, use either of these per-app
options:

1. In Finder, Control-click `CiaForge.app`, choose **Open**, then confirm the
   dialog; or attempt to open it once and choose **Open Anyway** in **System
   Settings → Privacy & Security**.
2. If macOS still keeps the downloaded-file quarantine attribute, remove it for
   this app only:

   ```bash
   xattr -dr com.apple.quarantine "/Applications/CiaForge.app"
   open "/Applications/CiaForge.app"
   ```

The command removes only CiaForge's download quarantine attribute. It does not
turn off Gatekeeper for other applications.

## Repository layout

```text
src/                       Frontend state, interactions, and styles
src-tauri/src/              Tauri commands and Rust conversion engine
src-tauri/assets/           Runtime CIA template assets
src-tauri/capabilities/     Tauri permissions
src-tauri/icons/            Application icons used for bundling
public/                     Frontend static assets
```

See [ARCHITECTURE.md](ARCHITECTURE.md) for module responsibilities and the
conversion flow.

## Third-party attribution

The conversion behaviour and embedded retail CIA templates are derived from
[ihaveamac/3dsconv](https://github.com/ihaveamac/3dsconv). CiaForge does not
execute or bundle the Python project. See [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)
for the required attribution and license text.

## License

MIT. See [LICENSE](LICENSE).
