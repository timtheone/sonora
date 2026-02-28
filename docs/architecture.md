# Architecture (Phase 0/1/2/3/4)

## Objectives

- Keep all speech processing local and offline.
- Provide a tray-based UX for configuration and status.
- Support global dictation hotkey with two activation modes:
  - push-to-toggle
  - push-to-talk

## High-level layout

- **TypeScript UI (Tauri window):** settings, status, and recent insertion history.
- **Rust core (Tauri commands/events):** orchestrates hotkey state, audio stream, VAD, STT process, and insertion adapters.
- **whisper.cpp sidecar:** offline transcription binary invoked by Rust service layer.

## Runtime services

1. **HotkeyService (Rust)**
   - Phase 1: hotkey event handling commands are wired (`phase1_hotkey_down`, `phase1_hotkey_up`).
   - Planned next: OS-global hotkey registration.

2. **AudioService (Rust)**
   - Phase 1: 16 kHz mono format contracts and sample helpers are in place.
   - Current test path: live microphone capture from UI via Web Audio API with 16 kHz downsampling fed to Rust pipeline.
   - Planned next: move capture fully to Rust `cpal`.

3. **VadService (Rust)**
   - Segments speech from silence.
   - Emits speech segments to TranscriptionService.

4. **TranscriptionService (Rust)**
   - Phase 1: transcriber abstraction and whisper.cpp sidecar argument builder are implemented.
   - Current runtime supports whisper.cpp sidecar execution when model and sidecar binary are available.
   - Includes runtime readiness diagnostics (resolved binary/model paths and fallback reasons).

5. **ProfileService (Rust)**
   - Phase 3: detects hardware tier from logical CPU cores.
   - Recommends and auto-applies model profile (`fast` or `balanced`).
   - Exposes model path validation status and profile-based chunking/cadence tuning.

6. **InsertionService (Rust)**
   - Phase 2: fallback-aware insertion status and recent history ring buffer (size 3) are implemented.
   - Current direct/clipboard OS adapters are stubbed for command/event wiring.
   - Planned next: wire per-OS direct insertion adapters.

7. **SettingsService (TS + Rust)**
   - Phase 2: persists settings to disk and applies runtime mode updates.
   - Phase 4: includes launch-at-startup persistence flag and microphone selection persistence.
   - Planned next: wire global hotkey registration updates from settings.

8. **EnvironmentHealthService (Rust)**
   - Phase 4: reports OS/session state and permission guidance for input injection.

9. **PostProcessingService (Rust)**
   - Phase 4: normalizes transcript punctuation/casing and suppresses duplicate outputs.

10. **RuntimeLogService (Rust)**
   - Phase 4: writes local runtime logs and exposes commands for reading/clearing recent logs.

11. **RecoveryService (Rust)**
   - Phase 4: persists startup/shutdown checkpoint state.
   - Flags recovery notice when previous session did not exit cleanly.
   - Supports acknowledgement flow for recovery notice in UI.

## Dictation state machine

States:

- `idle`
- `listening`
- `transcribing`

Phase 1 runtime currently uses:

- `idle`
- `listening`
- `transcribing`

Canonical flow:

Phase 1 flow:

`idle -> listening -> transcribing -> listening`

Planned full flow:

`idle -> listening -> transcribing -> inserting -> idle`

Mode rules:

- `push_to_toggle`: first hotkey press enters listening, second press exits listening.
- `push_to_talk`: hotkey down enters listening, hotkey up exits listening.

## Settings schema (v1)

- `hotkey`: string (`CtrlOrCmd+Shift+U` default)
- `mode`: `push_to_toggle` | `push_to_talk`
- `language`: `en`
- `modelProfile`: `balanced` | `fast`
- `modelPath`: string | null
- `microphoneId`: string | null
- `clipboardFallback`: boolean (default true)
- `launchAtStartup`: boolean (default false)

## Test strategy

- **TDD rule:** tests first for each behavior change.
- **TypeScript tests:** Vitest (`pnpm test`)
  - dictation state machine transitions
  - audio chunk helper behavior
  - settings normalization and defaults
- **Rust tests:** `cargo test` (`pnpm test:rust`)
  - default settings and model/profile invariants
  - hardware profile detection and model path resolution
  - settings persistence and patching
  - insertion fallback resolution/history truncation
  - environment/session mapping and transcript post-processing
  - runtime log append/read/clear behavior
  - adapter and orchestration units as they are added

## CI command baseline

- `pnpm install --frozen-lockfile`
- `pnpm test`
- `cargo test --manifest-path src-tauri/Cargo.toml --no-default-features`
- `pnpm build`

## Packaging pipeline (Phase 5)

- Multi-OS packaging workflow: `.github/workflows/package.yml`
- Builds installers on `ubuntu-22.04`, `windows-latest`, and `macos-latest`
- Runs tests before packaging and uploads bundle artifacts per OS
