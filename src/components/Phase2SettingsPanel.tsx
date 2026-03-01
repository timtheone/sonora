import { memo } from "react";
import { useAppControllerContext } from "../context/AppControllerContext";

const WHISPER_CPP_MODELS = [
  {
    value: "models/ggml-tiny.en-q8_0.bin",
    label: "tiny.en q8 (fastest)",
  },
  {
    value: "models/ggml-base.en-q5_1.bin",
    label: "base.en q5_1 (balanced)",
  },
  {
    value: "models/ggml-base.en-q8_0.bin",
    label: "base.en q8",
  },
  {
    value: "models/ggml-small.en-q8_0.bin",
    label: "small.en q8",
  },
  {
    value: "models/ggml-large-v3-turbo-q8_0.bin",
    label: "large-v3-turbo q8 (quality)",
  },
] as const;

const FASTER_WHISPER_MODEL_PRESETS = ["small.en", "distil-large-v3", "large-v3"] as const;
const DEFAULT_WHISPER_MODEL = "models/ggml-base.en-q5_1.bin";
const DEFAULT_FASTER_WHISPER_MODEL = "distil-large-v3";

function Phase2SettingsPanelComponent() {
  const {
    available,
    sttEngine,
    modelPath,
    selectedMicrophoneId,
    micSensitivityPercent,
    chunkDurationMs,
    partialCadenceMs,
    whisperBackendPreference,
    fasterWhisperModel,
    fasterWhisperComputeType,
    fasterWhisperBeamSize,
    availableMicrophones,
    clipboardFallback,
    launchAtStartup,
    settingsSavedAt,
    setSttEngine,
    setModelPathInput,
    setSelectedMicrophoneId,
    setMicSensitivityPercent,
    setChunkDurationMs,
    setPartialCadenceMs,
    setWhisperBackendPreference,
    setFasterWhisperModel,
    setFasterWhisperComputeType,
    setFasterWhisperBeamSize,
    setClipboardFallback,
    setLaunchAtStartup,
    saveSettings,
    refreshMicrophones,
  } = useAppControllerContext();

  const whisperSelectedModel = WHISPER_CPP_MODELS.some((entry) => entry.value === modelPath)
    ? modelPath
    : DEFAULT_WHISPER_MODEL;

  const fasterWhisperPresetValue = FASTER_WHISPER_MODEL_PRESETS.includes(
    fasterWhisperModel as (typeof FASTER_WHISPER_MODEL_PRESETS)[number],
  )
    ? fasterWhisperModel
    : DEFAULT_FASTER_WHISPER_MODEL;

  return (
    <section className="panel">
      <h2>Phase 2 Settings</h2>
      <label className="field">
        <span>Transcription engine</span>
        <select
          disabled={!available}
          value={sttEngine}
          onChange={(event) =>
            setSttEngine(event.currentTarget.value as "whisper_cpp" | "faster_whisper")
          }
        >
          <option value="whisper_cpp">whisper.cpp sidecar</option>
          <option value="faster_whisper">faster-whisper</option>
        </select>
      </label>
      <p className="muted">
        faster-whisper requires worker setup via <code>pnpm sidecar:setup:faster-whisper</code>.
      </p>
      <p className="muted">Current faster-whisper presets are tuned for English dictation.</p>
      {sttEngine === "whisper_cpp" ? (
        <label className="field">
          <span>whisper.cpp model</span>
          <select
            disabled={!available}
            value={whisperSelectedModel}
            onChange={(event) => setModelPathInput(event.currentTarget.value)}
          >
            {WHISPER_CPP_MODELS.map((entry) => (
              <option key={entry.value} value={entry.value}>
                {entry.label}
              </option>
            ))}
          </select>
        </label>
      ) : null}
      {sttEngine === "faster_whisper" ? (
        <>
          <label className="field">
            <span>faster-whisper model</span>
            <select
              disabled={!available}
              value={fasterWhisperPresetValue}
              onChange={(event) => setFasterWhisperModel(event.currentTarget.value)}
            >
              <option value="small.en">small.en</option>
              <option value="distil-large-v3">distil-large-v3</option>
              <option value="large-v3">large-v3</option>
            </select>
          </label>
          <label className="field">
            <span>faster-whisper compute type</span>
            <select
              disabled={!available}
              value={fasterWhisperComputeType}
              onChange={(event) =>
                setFasterWhisperComputeType(
                  event.currentTarget.value as "auto" | "int8" | "float16" | "float32",
                )
              }
            >
              <option value="auto">Auto</option>
              <option value="int8">int8</option>
              <option value="float16">float16</option>
              <option value="float32">float32</option>
            </select>
          </label>
          <label className="field">
            <span>faster-whisper beam size ({fasterWhisperBeamSize})</span>
            <input
              type="range"
              min={1}
              max={8}
              step={1}
              disabled={!available}
              value={fasterWhisperBeamSize}
              onChange={(event) => setFasterWhisperBeamSize(Number(event.currentTarget.value))}
            />
          </label>
        </>
      ) : null}
      <label className="field inline">
        <input
          type="checkbox"
          disabled={!available}
          checked={clipboardFallback}
          onChange={(event) => setClipboardFallback(event.currentTarget.checked)}
        />
        <span>Enable clipboard fallback insertion</span>
      </label>
      <label className="field inline">
        <input
          type="checkbox"
          disabled={!available}
          checked={launchAtStartup}
          onChange={(event) => setLaunchAtStartup(event.currentTarget.checked)}
        />
        <span>Launch app at startup (persistence in place)</span>
      </label>
      <label className="field">
        <span>Microphone</span>
        <select
          disabled={!available}
          value={selectedMicrophoneId}
          onChange={(event) => setSelectedMicrophoneId(event.currentTarget.value)}
        >
          <option value="">Default microphone</option>
          {availableMicrophones.map((mic) => (
            <option key={mic.id} value={mic.id}>
              {mic.label}
            </option>
          ))}
        </select>
      </label>
      <label className="field">
        <span>Mic sensitivity ({micSensitivityPercent}%)</span>
        <input
          type="range"
          min={50}
          max={300}
          step={5}
          disabled={!available}
          value={micSensitivityPercent}
          onChange={(event) => setMicSensitivityPercent(Number(event.currentTarget.value))}
        />
      </label>
      <label className="field">
        <span>Chunk duration ({chunkDurationMs} ms)</span>
        <input
          type="range"
          min={500}
          max={4000}
          step={50}
          disabled={!available}
          value={chunkDurationMs}
          onChange={(event) => setChunkDurationMs(Number(event.currentTarget.value))}
        />
      </label>
      <label className="field">
        <span>Partial cadence ({partialCadenceMs} ms)</span>
        <input
          type="range"
          min={300}
          max={2500}
          step={50}
          disabled={!available}
          value={partialCadenceMs}
          onChange={(event) => setPartialCadenceMs(Number(event.currentTarget.value))}
        />
      </label>
      <label className="field">
        <span>Inference backend</span>
        <select
          disabled={!available}
          value={whisperBackendPreference}
          onChange={(event) =>
            setWhisperBackendPreference(event.currentTarget.value as "auto" | "cpu" | "cuda")
          }
        >
          <option value="auto">Auto detect (recommended)</option>
          <option value="cuda">CUDA (NVIDIA GPU)</option>
          <option value="cpu">CPU only</option>
        </select>
      </label>
      <div className="actions">
        <button disabled={!available} onClick={saveSettings}>
          Save Settings
        </button>
        <button disabled={!available} onClick={refreshMicrophones}>
          Refresh Microphones
        </button>
      </div>
      <p className="muted">
        {settingsSavedAt ? `Settings saved at ${settingsSavedAt}` : "No settings saved in this session yet."}
      </p>
    </section>
  );
}

export const Phase2SettingsPanel = memo(Phase2SettingsPanelComponent);
