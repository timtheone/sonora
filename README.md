# Sonora Dictation

Offline desktop dictation app (Windows, macOS, Linux X11) built with Tauri + TypeScript + Rust.

## Tech stack

- Tauri v2
- React + TypeScript (frontend)
- Rust (native backend)
- whisper.cpp sidecar integration scaffold (runtime invocation pending)
- pnpm (JavaScript package manager)

## Development

Install dependencies:

```bash
pnpm install
```

Run frontend only:

```bash
pnpm dev
```

Run the Tauri app:

```bash
pnpm tauri dev
```

Build frontend bundle:

```bash
pnpm build
```

Download default bundled models (balanced + fast):

```bash
pnpm model:download
```

Download a single profile model:

```bash
pnpm model:download balanced
pnpm model:download fast
```

Download/build whisper.cpp sidecar binary for current OS:

```bash
pnpm sidecar:setup
```

Force a clean re-clone + rebuild of sidecar:

```bash
pnpm sidecar:setup:force
```

Troubleshooting sidecar setup:

- `ENOENT cmake`: install CMake and C/C++ build tools.
  - Linux (Ubuntu/Debian): `sudo apt-get update && sudo apt-get install -y cmake build-essential`
  - macOS: `brew install cmake` and `xcode-select --install`
  - Windows: install CMake + Visual Studio C++ Build Tools
- Build the sidecar on the same OS as the package target (Windows package expects `whisper-cli.exe`).

## Test commands

TypeScript unit tests (Vitest):

```bash
pnpm test
```

Rust unit tests:

```bash
pnpm test:rust
```

Rust desktop-linked tests (requires Tauri Linux/macOS/Windows system prerequisites):

```bash
pnpm test:rust:desktop
```

Run all tests:

```bash
pnpm test:all
```

Build installer/bundles (auto-downloads missing default models first):

```bash
pnpm tauri:build
```


## Notes

- This repository follows TDD for all new features.
- Current scope is English-only with `Ctrl/Cmd + Shift + U` default hotkey.
- Settings are persisted locally via the Rust backend store.
- Phase 2 currently includes insertion status/history plumbing; OS-native insertion adapters are next.
- Phase 3 adds hardware profile detection and model path/profile status commands.
- Phase 4 adds environment health checks, transcript post-processing, and local runtime logs.
- Phase 4 also adds recovery checkpoint tracking for dirty-shutdown detection.
- Runtime transcriber now attempts whisper.cpp sidecar execution when binary + model are available.
- UI includes a live mic capture test path (Web Audio -> 16 kHz feed into Rust dictation pipeline).
- Model binaries are downloaded via `pnpm model:download` into `src-tauri/resources/models/`.
- Sidecar binary is generated via `pnpm sidecar:setup` into `src-tauri/resources/bin/`.
