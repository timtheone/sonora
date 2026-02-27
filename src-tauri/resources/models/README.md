# Bundled Models

Place Whisper model files here for packaging.

Recommended way to populate this directory:

```bash
pnpm model:download
```

Expected default filenames:

- `ggml-base.en-q5_1.bin` (balanced)
- `ggml-tiny.en-q8_0.bin` (fast)

When building installers (`pnpm tauri build`), files in this directory are bundled as app resources.
