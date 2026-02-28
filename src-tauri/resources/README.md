# Bundled Sidecar Binaries

Place the `whisper.cpp` CLI binary here for packaging.

Preferred setup:

```bash
pnpm sidecar:setup
```

- Windows: `whisper-cli.exe`
- macOS/Linux: `whisper-cli`

This directory is used by runtime binary discovery and can be bundled in installers.
