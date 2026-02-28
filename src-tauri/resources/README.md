# Bundled Sidecar Binaries

Place the `whisper.cpp` CLI binary here for packaging.

Preferred setup:

```bash
pnpm sidecar:setup
```

For NVIDIA CUDA builds:

```bash
pnpm sidecar:setup:cuda
```

- Windows: `whisper-cli.exe`
- macOS/Linux: `whisper-cli`

This directory is used by runtime binary discovery and can be bundled in installers.
`whisper-sidecar.json` is generated here to hint runtime backend selection.
