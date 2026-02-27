# Architecture (Phase 0)

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
   - Registers/unregisters global hotkey.
   - Emits activation events to DictationService.

2. **AudioService (Rust)**
   - Captures microphone audio at 16 kHz mono using `cpal`.
   - Buffers frames into VAD-friendly chunks.

3. **VadService (Rust)**
   - Segments speech from silence.
   - Emits speech segments to TranscriptionService.

4. **TranscriptionService (Rust)**
   - Executes whisper.cpp sidecar with configured model.
   - Returns partial/final text.

5. **InsertionService (Rust)**
   - Direct typing via OS adapters.
   - Clipboard-paste fallback if direct insertion fails.
   - Writes insertion outcomes to recent history ring buffer (size 3).

6. **SettingsService (TS + Rust)**
   - Persists validated settings.
   - Applies runtime updates (hotkey, mode, profile, mic).

## Dictation state machine

States:

- `idle`
- `listening`
- `transcribing`
- `inserting`

Canonical flow:

`idle -> listening -> transcribing -> inserting -> idle`

Mode rules:

- `push_to_toggle`: first hotkey press enters listening, second press exits listening.
- `push_to_talk`: hotkey down enters listening, hotkey up exits listening.

## Settings schema (v1)

- `hotkey`: string (`CtrlOrCmd+Shift+U` default)
- `mode`: `push_to_toggle` | `push_to_talk`
- `language`: `en`
- `modelProfile`: `balanced` | `fast`
- `microphoneId`: string | null
- `clipboardFallback`: boolean (default true)

## Test strategy

- **TDD rule:** tests first for each behavior change.
- **TypeScript tests:** Vitest (`pnpm test`)
  - dictation state machine transitions
  - settings normalization and defaults
- **Rust tests:** `cargo test` (`pnpm test:rust`)
  - default settings and model/profile invariants
  - adapter and orchestration units as they are added

## CI command baseline

- `pnpm install --frozen-lockfile`
- `pnpm test`
- `cargo test --manifest-path src-tauri/Cargo.toml --no-default-features`
- `pnpm build`
