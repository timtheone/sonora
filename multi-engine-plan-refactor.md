# Multi-Engine STT Refactor Plan

## Goal

Add a multi-engine transcription architecture to Sonora so we can switch between engines (starting with `whisper.cpp` and `faster-whisper`) while preserving:

- local/offline-only behavior,
- low-latency live dictation,
- packaged/self-contained installer support (no manual runtime setup required by end users).

Parakeet is intentionally deferred for now, but this plan keeps the architecture ready for it.

## Scope and Constraints

- Keep current Tauri app structure: React UI + Rust backend orchestration.
- Keep current Rust audio pipeline (capture, VAD, chunking, profiling).
- Continue using the `Transcriber` abstraction as the boundary for engine integration.
- Avoid introducing a required external Python installation for users.
- All engine/runtime artifacts must be bundleable in installers.

## Success Criteria

- Engine can be switched from UI without app restart.
- Runtime status clearly shows selected engine, model, backend device (CPU/CUDA), and readiness.
- `pnpm perf:watch` can compare latency metrics between engines.
- Installers run on clean machines with no extra setup.
- No regression in existing whisper.cpp path.

---

## Phase 1 — Engine Abstraction Refactor

### Objective

Generalize runtime transcription plumbing so multiple engines can plug in cleanly.

### Deliverables

1. **Engine domain model in settings/config**
   - Add `stt_engine` enum (initial values: `whisper_cpp`, `faster_whisper`).
   - Add engine-specific options container (device, model id/path, compute type, beam size, etc.).
   - Keep backward-compatible defaults to current whisper.cpp behavior.

2. **Engine runtime abstraction**
   - Introduce an engine factory layer in Rust:
     - `EngineSpec` (settings-derived intent)
     - `EngineRuntime` (ready/not-ready + diagnostics)
     - `EngineTranscriber` adapter implementing existing `Transcriber` trait.
   - Keep pipeline API unchanged (`process_audio_chunk_*` remains engine-agnostic).

3. **Command/status extensions**
   - Extend transcriber status payload to include:
     - active engine,
     - selected model,
     - backend/device,
     - readiness diagnostics,
     - fallback reason.
   - Ensure settings update command rebuilds runtime engine safely.

4. **UI updates**
   - Add engine selector in settings panel.
   - Show active engine + readiness in environment/status panel.

5. **Profiling compatibility**
   - Add `engine` and `model` fields to `perf.chunk` events.
   - Keep watcher backward-compatible.

### TDD / Validation

- Rust unit tests for:
  - settings deserialization defaults,
  - runtime factory selection,
  - safe rebuild on settings change,
  - diagnostic payload shape.
- TS tests for settings normalization and UI contract.
- Full regression run:
  - `cargo test --manifest-path src-tauri/Cargo.toml`
  - `pnpm test`
  - `pnpm build`

### Exit Criteria

- whisper.cpp path behaves exactly as before when `stt_engine=whisper_cpp`.
- engine selector persists and round-trips through settings.
- architecture is ready for faster-whisper sidecar integration.

---

## Phase 2 — Faster-Whisper Integration (Self-Contained Packaging)

### Objective

Add faster-whisper as a production-capable engine with packaged runtime support.

### Runtime Strategy (packaged-first)

Use a **persistent sidecar worker** process (not per-chunk spawn) and bundle all runtime dependencies in installer artifacts.

#### Recommended implementation

- Sidecar process: `faster-whisper-worker` (Python entrypoint or compiled wrapper) that:
  - loads model once,
  - keeps it warm in memory/VRAM,
  - accepts chunk requests over stdio JSON-RPC (or local named pipe/socket),
  - returns transcript + timings.

- Rust host responsibilities:
  - worker lifecycle (start/health/restart/shutdown),
  - chunk request/response routing,
  - timeout/retry policy,
  - diagnostics mapping to UI.

### Packaging Plan

1. **Bundle runtime assets per OS**
   - worker executable/entrypoint,
   - Python runtime (if Python-based path is used),
   - required wheels/libs (`ctranslate2`, `faster-whisper`, `onnxruntime`/CUDA libs as needed),
   - model cache directory conventions.

2. **Installer integration**
   - Include faster-whisper runtime files in Tauri bundle resources.
   - Add startup checks for dependency integrity (hash/version checks).
   - Emit actionable diagnostics if CUDA is unavailable and fallback to CPU is selected.

3. **Device/backend controls**
   - UI options: `auto`, `cuda`, `cpu`.
   - Runtime verifies requested device and reports effective device.

4. **Model options**
   - Add curated faster-whisper model presets (small/medium/large-v3 variants).
   - Model manager supports download, existence checks, and active model switch.

### Performance and Reliability Requirements

- First transcript latency and steady-state latency measured via existing profiler.
- Worker restart protection if process crashes.
- Explicit queue backpressure metrics.

### TDD / Validation

- Rust integration tests around worker protocol handling (mock worker).
- Packaging smoke tests on clean VMs:
  - Windows (required first),
  - macOS/Linux as follow-up target.
- Acceptance benchmarks:
  - no per-chunk startup overhead,
  - infer latency stable under sustained dictation.

### Exit Criteria

- User can choose `faster_whisper` in settings and transcribe successfully.
- Installer on clean machine works without manual Python/CUDA toolkit install.
- Profiling clearly reports faster-whisper timing path.

---

## Phase 3 — Engine Comparison, Presets, and Decision Workflow

### Objective

Make engine/model experimentation easy and data-driven.

### Deliverables

1. **Engine/model preset UX**
   - Presets for latency/quality targets (e.g., Low Latency, Balanced, Quality).
   - One-click apply + save.

2. **Comparison tooling**
   - Extend `perf:watch` and/or add `perf:report` command:
     - aggregate by engine/model,
     - p50/p95 infer/total,
     - dropped/chunk stats.

3. **A/B test workflow**
   - Scripted test mode for fixed dictation sample set.
   - Output CSV/JSON summary for repeatable comparisons.

4. **Quality triage support**
   - Optional transcript capture mode for side-by-side output review.
   - Hallucination/error tagging notes for manual evaluation.

### Exit Criteria

- We can objectively select a default engine/model profile for target hardware tiers.
- User-facing settings expose stable presets and advanced overrides.

---

## Deferred: Parakeet (Future Phase)

Parakeet integration remains out of current scope. When resumed, reuse the same engine plugin interface created in Phase 1 and the packaging/runtime patterns proven in Phase 2.

---

## Cross-Phase Engineering Rules

- TDD first for behavior changes.
- No regression to offline/local guarantees.
- No destructive migration of existing user settings.
- Keep runtime diagnostics explicit and user-readable.
- Keep profiling on-demand (`SONORA_PERF`) to avoid overhead in normal mode.

## Risks and Mitigations

- **Packaging size growth**
  - Mitigate with optional model bundles + post-install download option.
- **GPU dependency mismatch (driver/CUDA runtime)**
  - Strong startup diagnostics + automatic CPU fallback.
- **Worker protocol fragility**
  - Strict schema versioning + health checks + restart policy.
- **Cross-platform runtime drift**
  - CI smoke tests per OS artifact.

## Milestone Summary

- **M1:** Engine abstraction merged, whisper.cpp unchanged behavior.
- **M2:** Faster-whisper engine functional in dev + packaged clean-machine support.
- **M3:** Benchmark/preset workflow complete, default recommendations documented.
