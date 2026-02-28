# Sonora Dictation

Offline desktop dictation app (Windows, macOS, Linux X11) built with Tauri + TypeScript + Rust.

## Tech stack

- Tauri v2
- React + TypeScript (frontend)
- React 19 Compiler enabled via Babel plugin
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

Run Tauri dev with profiling instrumentation enabled:

```bash
pnpm tauri:dev:profiling
```

Watch live perf traces in a table:

```bash
pnpm perf:watch
```

Optional custom log path:

```bash
pnpm perf:watch -- --path /absolute/path/to/runtime.log
```

Build frontend bundle:

```bash
pnpm build
```

Download all bundled whisper.cpp model profiles (default behavior):

```bash
pnpm model:download
```

Download only the smaller default profile bundle:

```bash
pnpm model:download default
```

Download specific whisper.cpp profile bundles:

```bash
pnpm model:download balanced
pnpm model:download fast
pnpm model:download q8
pnpm model:download quality
pnpm model:download all
```

Download faster-whisper models (same command, different engine):

```bash
pnpm model:download -- --engine faster_whisper
pnpm model:download -- --engine faster_whisper --faster-models small.en,distil-large-v3,large-v3
```

Download both whisper.cpp and faster-whisper model bundles:

```bash
pnpm model:download -- all --engine all
```

Download/build whisper.cpp sidecar binary for current OS:

```bash
pnpm sidecar:setup
```

Build explicitly with NVIDIA CUDA backend:

```bash
pnpm sidecar:setup --backend cuda
```

You can also choose backend manually:

```bash
pnpm sidecar:setup --backend auto
pnpm sidecar:setup --backend cpu
pnpm sidecar:setup --backend cuda
```

Force a clean re-clone + rebuild of sidecar:

```bash
pnpm sidecar:setup --force-clone
```

Build faster-whisper worker sidecar (packaged worker executable):

```bash
pnpm sidecar:setup:faster-whisper
```

Build installer bundles including both whisper.cpp and faster-whisper sidecars:

```bash
pnpm tauri:build:full
```

Troubleshooting sidecar setup:

- `ENOENT cmake`: install CMake and C/C++ build tools.
  - Linux (Ubuntu/Debian): `sudo apt-get update && sudo apt-get install -y cmake build-essential`
  - macOS: `brew install cmake` and `xcode-select --install`
  - Windows: install CMake + Visual Studio C++ Build Tools
- Build the sidecar on the same OS as the package target (Windows package expects `whisper-cli.exe`).
- CUDA backend requires NVIDIA driver + CUDA Toolkit (`nvcc`) in your PATH.
- Sidecar setup writes backend metadata to `src-tauri/resources/bin/whisper-sidecar.json`; runtime uses it to prefer CPU/CUDA mode.
- Runtime override: set `SONORA_WHISPER_BACKEND=cpu|cuda|auto` before launching the app.
- Faster-whisper worker setup requires Python 3.10+ and internet access during build.
- Faster-whisper model cache is written to `src-tauri/resources/models/faster-whisper-cache`.
- Default faster-whisper bundle includes English models: `small.en`, `distil-large-v3`, `large-v3`.
- whisper.cpp q8 bundle now includes `ggml-base.en-q8_0.bin`, `ggml-small.en-q8_0.bin`, and quality bundle includes `ggml-large-v3-turbo-q8_0.bin`.
- Faster-whisper runtime binary override: set `SONORA_FASTER_WHISPER_BIN` before launching the app.

## Test commands

TypeScript unit tests (Vitest):

```bash
pnpm test
```

Rust unit tests:

```bash
pnpm test:rust
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
- Settings UI includes latency tuning controls (chunk duration + partial cadence) for live experimentation.
- Settings UI includes inference backend preference (`auto`/`cpu`/`cuda`) so GPU usage can be toggled without env vars.
- Settings UI includes an STT engine selector (`whisper.cpp` or `faster-whisper`).
- Faster-whisper settings now include model id/path, compute type, and beam size controls.
- Set `SONORA_PERF=1` to enable chunk-level perf trace events in runtime logs.
- `pnpm perf:watch` reads those events and renders a live timing table (`capture/queue/VAD/inference/emit`).
- UI includes a live mic capture test path (Web Audio -> 16 kHz feed into Rust dictation pipeline).
- Model binaries are downloaded via `pnpm model:download` into `src-tauri/resources/models/`.
- Sidecar binary is generated via `pnpm sidecar:setup` into `src-tauri/resources/bin/`.
- Faster-whisper worker binary is generated via `pnpm sidecar:setup:faster-whisper` into `src-tauri/resources/bin/`.
