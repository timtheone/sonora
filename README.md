# Sonora Dictation

Offline desktop dictation app (Windows, macOS, Linux X11) built with Tauri + TypeScript + Rust.

## Tech stack

- Tauri v2
- React + TypeScript (frontend)
- Rust (native backend)
- whisper.cpp sidecar integration (Phase 1 in progress)
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

## Notes

- This repository follows TDD for all new features.
- Current scope is English-only with `Ctrl/Cmd + Shift + U` default hotkey.
