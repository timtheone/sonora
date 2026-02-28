# Bundled Sidecar Binaries

Place the `whisper.cpp` CLI binary here for packaging.

Preferred setup:

```bash
pnpm sidecar:setup
```

For NVIDIA CUDA builds:

```bash
pnpm sidecar:setup --backend cuda
```

Build faster-whisper worker executable for packaging:

```bash
pnpm sidecar:setup:faster-whisper
```

Prefetch model cache for offline faster-whisper usage:

```bash
pnpm model:download -- --engine faster_whisper
pnpm model:download -- --engine faster_whisper --faster-models small.en,distil-large-v3,large-v3
```

- Windows: `whisper-cli.exe`
- macOS/Linux: `whisper-cli`
- Windows: `faster-whisper-worker.exe`
- macOS/Linux: `faster-whisper-worker`

This directory is used by runtime binary discovery and can be bundled in installers.
`whisper-sidecar.json` is generated here to hint runtime backend selection.
`faster-whisper-sidecar.json` is generated here when faster-whisper worker build succeeds.
Model cache is stored under `models/faster-whisper-cache/` when prefetched.
