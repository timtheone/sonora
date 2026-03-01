import { useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  CHUNK_DURATION_MAX_MS,
  CHUNK_DURATION_MIN_MS,
  DEFAULT_SETTINGS,
  PARTIAL_CADENCE_MAX_MS,
  PARTIAL_CADENCE_MIN_MS,
  effectiveChunkDurationMs,
  effectivePartialCadenceMs,
} from "../domain/settings";
import {
  cancelPhase1,
  listPhase1Microphones,
  sendPhase1HotkeyDown,
  getPhase1Status,
  type DictationState,
  type TranscriptPayload,
  type PipelineStatus,
} from "../services/phase1";
import {
  getPhase2Settings,
  updatePhase2Settings,
  type SttEngine,
} from "../services/phase2";
import {
  autoSelectHardwareProfile,
  getHardwareProfileStatus,
  getModelStatus,
  type HardwareProfileStatus,
  type ModelStatus,
} from "../services/phase3";
import {
  acknowledgeRecoveryNotice,
  clearRuntimeLogs,
  getEnvironmentHealth,
  markPerfTranscriptReceived,
  getRecoveryCheckpoint,
  getRuntimeLogs,
  getTranscriberStatus,
  markCleanShutdown,
  type EnvironmentHealth,
  type RecoveryCheckpoint,
  type TranscriberStatus,
} from "../services/phase4";

const FALLBACK_STATE: DictationState = "idle";

function isTauriRuntime(): boolean {
  const windowWithTauri = window as Window & {
    __TAURI_INTERNALS__?: unknown;
  };
  return Boolean(windowWithTauri.__TAURI_INTERNALS__);
}

function defaultWhisperModelPath(): string {
  return "models/ggml-base.en-q5_1.bin";
}

function defaultFasterWhisperModel(): string {
  return "distil-large-v3";
}

export function useAppController() {
  const [available, setAvailable] = useState(false);
  const [state, setState] = useState<DictationState>(FALLBACK_STATE);
  const [mode, setMode] = useState(DEFAULT_SETTINGS.mode);
  const [recentTranscripts, setRecentTranscripts] = useState<string[]>([]);
  const [hotkey, setHotkey] = useState(DEFAULT_SETTINGS.hotkey);
  const [modelProfile, setModelProfile] = useState<"fast" | "balanced">(
    DEFAULT_SETTINGS.modelProfile,
  );
  const [sttEngine, setSttEngine] = useState<SttEngine>(DEFAULT_SETTINGS.sttEngine);
  const [modelPath, setModelPathInput] = useState<string>("");
  const [selectedMicrophoneId, setSelectedMicrophoneId] = useState<string>("");
  const [micSensitivityPercent, setMicSensitivityPercent] =
    useState<number>(DEFAULT_SETTINGS.micSensitivityPercent);
  const [chunkDurationMs, setChunkDurationMs] =
    useState<number>(DEFAULT_SETTINGS.chunkDurationMs);
  const [partialCadenceMs, setPartialCadenceMs] =
    useState<number>(DEFAULT_SETTINGS.partialCadenceMs);
  const [whisperBackendPreference, setWhisperBackendPreference] =
    useState<"auto" | "cpu" | "cuda">(DEFAULT_SETTINGS.whisperBackendPreference);
  const [fasterWhisperModel, setFasterWhisperModel] =
    useState<string>(DEFAULT_SETTINGS.fasterWhisperModel ?? "");
  const [fasterWhisperComputeType, setFasterWhisperComputeType] =
    useState<"auto" | "int8" | "float16" | "float32">(
      DEFAULT_SETTINGS.fasterWhisperComputeType,
    );
  const [fasterWhisperBeamSize, setFasterWhisperBeamSize] =
    useState<number>(DEFAULT_SETTINGS.fasterWhisperBeamSize);
  const [vadDisabled, setVadDisabled] = useState<boolean>(DEFAULT_SETTINGS.vadDisabled);
  const [vadRmsThresholdMilli, setVadRmsThresholdMilli] =
    useState<number>(DEFAULT_SETTINGS.vadRmsThresholdMilli);
  const [availableMicrophones, setAvailableMicrophones] = useState<
    Array<{ id: string; label: string }>
  >([]);
  const [clipboardFallback, setClipboardFallback] = useState(true);
  const [launchAtStartup, setLaunchAtStartup] = useState(false);
  const [hardwareProfile, setHardwareProfile] =
    useState<HardwareProfileStatus | null>(null);
  const [modelStatus, setModelStatus] = useState<ModelStatus | null>(null);
  const [environmentHealth, setEnvironmentHealth] =
    useState<EnvironmentHealth | null>(null);
  const [recoveryCheckpoint, setRecoveryCheckpoint] =
    useState<RecoveryCheckpoint | null>(null);
  const [transcriberStatus, setTranscriberStatus] =
    useState<TranscriberStatus | null>(null);
  const [runtimeLogs, setRuntimeLogs] = useState<string[]>([]);
  const [settingsSavedAt, setSettingsSavedAt] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const activeTranscriptSessionId = useRef<number | null>(null);

  const statusLabel = useMemo(() => {
    if (!available) {
      return "Web preview mode (run `pnpm tauri dev` for native commands).";
    }
    return `State: ${state} | Mode: ${mode}`;
  }, [available, mode, state]);

  async function refreshMicrophones() {
    try {
      const microphones = await listPhase1Microphones();
      setAvailableMicrophones(microphones);
      if (microphones.length > 0 && !selectedMicrophoneId) {
        setSelectedMicrophoneId(microphones[0].id);
      }
    } catch {
      setAvailableMicrophones([]);
    }
  }

  function applyPhaseStatus(status: PipelineStatus) {
    setState(status.state);
    setMode(status.mode);
  }

  async function syncPhaseStatus() {
    try {
      const status = await getPhase1Status();
      applyPhaseStatus(status);
      return status;
    } catch (cause) {
      setError(String(cause));
      return null;
    }
  }

  useEffect(() => {
    if (!isTauriRuntime()) {
      setAvailable(false);
      return;
    }

    setAvailable(true);
    Promise.all([
      getPhase1Status(),
      getPhase2Settings(),
      getHardwareProfileStatus(),
      getModelStatus(),
      getEnvironmentHealth(),
      getRuntimeLogs(30),
      getRecoveryCheckpoint(),
      getTranscriberStatus(),
    ])
      .then(
        ([
          phase1Status,
          settings,
          hardware,
          profileStatus,
          envHealth,
          logs,
          recovery,
          runtimeTranscriber,
        ]) => {
          applyPhaseStatus(phase1Status);
          setHotkey(settings.hotkey);
          setModelProfile(settings.model_profile);
          setSttEngine(settings.stt_engine);
          setModelPathInput(settings.model_path ?? "");
          setSelectedMicrophoneId(settings.microphone_id ?? "");
          setMicSensitivityPercent(settings.mic_sensitivity_percent);
          setChunkDurationMs(
            effectiveChunkDurationMs(settings.model_profile, settings.chunk_duration_ms),
          );
          setPartialCadenceMs(
            effectivePartialCadenceMs(settings.model_profile, settings.partial_cadence_ms),
          );
          setWhisperBackendPreference(settings.whisper_backend_preference);
          setFasterWhisperModel(settings.faster_whisper_model ?? "");
          setFasterWhisperComputeType(settings.faster_whisper_compute_type);
          setFasterWhisperBeamSize(Math.max(1, Math.min(8, settings.faster_whisper_beam_size)));
          setVadDisabled(settings.vad_disabled);
          setVadRmsThresholdMilli(
            Math.max(1, Math.min(80, settings.vad_rms_threshold_milli ?? 9)),
          );
          setClipboardFallback(settings.clipboard_fallback);
          setLaunchAtStartup(settings.launch_at_startup);
          setHardwareProfile(hardware);
          setModelStatus(profileStatus);
          setEnvironmentHealth(envHealth);
          setRuntimeLogs(logs);
          setRecoveryCheckpoint(recovery);
          setTranscriberStatus(runtimeTranscriber);
        },
      )
      .catch((cause) => {
        setError(String(cause));
      });

    refreshMicrophones();

    const onBeforeUnload = () => {
      markCleanShutdown().catch(() => {
        // noop
      });
    };
    window.addEventListener("beforeunload", onBeforeUnload);

    const unlistenTranscript = listen<TranscriptPayload>(
      "dictation:transcript",
      (event) => {
        const sessionId =
          typeof event.payload.session_id === "number" ? event.payload.session_id : null;

        setRecentTranscripts((previous) => {
          if (
            sessionId !== null &&
            activeTranscriptSessionId.current === sessionId &&
            previous.length > 0
          ) {
            return [event.payload.text, ...previous.slice(1)].slice(0, 8);
          }

          activeTranscriptSessionId.current = sessionId;
          return [event.payload.text, ...previous].slice(0, 8);
        });
        if (
          typeof event.payload.chunk_id === "number" &&
          typeof event.payload.emitted_unix_ms === "number"
        ) {
          markPerfTranscriptReceived(event.payload.chunk_id, event.payload.emitted_unix_ms).catch(
            () => {
              // noop
            },
          );
        }
      },
    );

    return () => {
      unlistenTranscript.then((dispose) => dispose());
      window.removeEventListener("beforeunload", onBeforeUnload);
    };
  }, []);

  async function onHotkeyDown() {
    try {
      const status = await sendPhase1HotkeyDown();
      applyPhaseStatus(status);
      setError(null);
      return status;
    } catch (cause) {
      setError(String(cause));
      return null;
    }
  }

  async function onCancel() {
    try {
      const status = await cancelPhase1();
      applyPhaseStatus(status);
      setError(null);
      return status;
    } catch (cause) {
      setError(String(cause));
      return null;
    }
  }

  async function saveSettings() {
    try {
      const selectedWhisperModelPath = modelPath.trim()
        ? modelPath.trim()
        : defaultWhisperModelPath();
      const selectedFasterWhisperModel = fasterWhisperModel.trim()
        ? fasterWhisperModel.trim()
        : defaultFasterWhisperModel();

      const updated = await updatePhase2Settings({
        hotkey,
        mode,
        model_profile: modelProfile,
        stt_engine: sttEngine,
        model_path: selectedWhisperModelPath,
        microphone_id: selectedMicrophoneId.trim() ? selectedMicrophoneId : null,
        mic_sensitivity_percent: Math.max(50, Math.min(300, Math.round(micSensitivityPercent))),
        chunk_duration_ms: Math.max(
          CHUNK_DURATION_MIN_MS,
          Math.min(CHUNK_DURATION_MAX_MS, Math.round(chunkDurationMs)),
        ),
        partial_cadence_ms: Math.max(
          PARTIAL_CADENCE_MIN_MS,
          Math.min(PARTIAL_CADENCE_MAX_MS, Math.round(partialCadenceMs)),
        ),
        whisper_backend_preference: whisperBackendPreference,
        faster_whisper_model: selectedFasterWhisperModel,
        faster_whisper_compute_type: fasterWhisperComputeType,
        faster_whisper_beam_size: Math.max(1, Math.min(8, Math.round(fasterWhisperBeamSize))),
        vad_disabled: vadDisabled,
        vad_rms_threshold_milli: Math.max(1, Math.min(80, Math.round(vadRmsThresholdMilli))),
        clipboard_fallback: clipboardFallback,
        launch_at_startup: launchAtStartup,
      });
      setHotkey(updated.hotkey);
      setClipboardFallback(updated.clipboard_fallback);
      setMode(updated.mode);
      setModelProfile(updated.model_profile);
      setSttEngine(updated.stt_engine);
      setModelPathInput(updated.model_path ?? "");
      setSelectedMicrophoneId(updated.microphone_id ?? "");
      setMicSensitivityPercent(updated.mic_sensitivity_percent);
      setChunkDurationMs(effectiveChunkDurationMs(updated.model_profile, updated.chunk_duration_ms));
      setPartialCadenceMs(
        effectivePartialCadenceMs(updated.model_profile, updated.partial_cadence_ms),
      );
      setWhisperBackendPreference(updated.whisper_backend_preference);
      setFasterWhisperModel(updated.faster_whisper_model ?? "");
      setFasterWhisperComputeType(updated.faster_whisper_compute_type);
      setFasterWhisperBeamSize(Math.max(1, Math.min(8, updated.faster_whisper_beam_size)));
      setVadDisabled(updated.vad_disabled);
      setVadRmsThresholdMilli(Math.max(1, Math.min(80, updated.vad_rms_threshold_milli ?? 9)));
      setLaunchAtStartup(updated.launch_at_startup);
      const [status, runtimeTranscriber] = await Promise.all([
        getModelStatus(),
        getTranscriberStatus(),
      ]);
      setModelStatus(status);
      setTranscriberStatus(runtimeTranscriber);
      setSettingsSavedAt(new Date().toLocaleTimeString());
      setError(null);
    } catch (cause) {
      setError(String(cause));
    }
  }

  async function detectAndApplyProfile() {
    try {
      const updatedSettings = await autoSelectHardwareProfile();
      const [hardware, status] = await Promise.all([
        getHardwareProfileStatus(),
        getModelStatus(),
      ]);
      setHardwareProfile(hardware);
      setModelStatus(status);
      setModelProfile(updatedSettings.model_profile);
      setSttEngine(updatedSettings.stt_engine);
      setModelPathInput(updatedSettings.model_path ?? "");
      setChunkDurationMs(
        effectiveChunkDurationMs(updatedSettings.model_profile, updatedSettings.chunk_duration_ms),
      );
      setPartialCadenceMs(
        effectivePartialCadenceMs(updatedSettings.model_profile, updatedSettings.partial_cadence_ms),
      );
      setWhisperBackendPreference(updatedSettings.whisper_backend_preference);
      setFasterWhisperModel(updatedSettings.faster_whisper_model ?? "");
      setFasterWhisperComputeType(updatedSettings.faster_whisper_compute_type);
      setFasterWhisperBeamSize(
        Math.max(1, Math.min(8, updatedSettings.faster_whisper_beam_size)),
      );
      setVadDisabled(updatedSettings.vad_disabled);
      setVadRmsThresholdMilli(
        Math.max(1, Math.min(80, updatedSettings.vad_rms_threshold_milli ?? 9)),
      );
      setTranscriberStatus(await getTranscriberStatus());
      setSettingsSavedAt(new Date().toLocaleTimeString());
      setError(null);
    } catch (cause) {
      setError(String(cause));
    }
  }

  async function refreshPhase4Health() {
    try {
      const [envHealth, logs, recovery, runtimeTranscriber] = await Promise.all([
        getEnvironmentHealth(),
        getRuntimeLogs(30),
        getRecoveryCheckpoint(),
        getTranscriberStatus(),
      ]);
      setEnvironmentHealth(envHealth);
      setRuntimeLogs(logs);
      setRecoveryCheckpoint(recovery);
      setTranscriberStatus(runtimeTranscriber);
      setError(null);
    } catch (cause) {
      setError(String(cause));
    }
  }

  async function clearLogs() {
    try {
      await clearRuntimeLogs();
      setRuntimeLogs([]);
      setError(null);
    } catch (cause) {
      setError(String(cause));
    }
  }

  async function acknowledgeRecovery() {
    try {
      const checkpoint = await acknowledgeRecoveryNotice();
      setRecoveryCheckpoint(checkpoint);
      setError(null);
    } catch (cause) {
      setError(String(cause));
    }
  }

  function clearErrorMessage() {
    setError(null);
  }

  function reportError(cause: unknown) {
    setError(String(cause));
  }

  function applyHighAccuracyPreset() {
    setChunkDurationMs(2600);
    setPartialCadenceMs(1200);
    setFasterWhisperBeamSize(5);
    setVadDisabled(false);
    setVadRmsThresholdMilli(7);
  }

  return {
    available,
    state,
    mode,
    statusLabel,
    recentTranscripts,
    hotkey,
    modelProfile,
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
    vadDisabled,
    vadRmsThresholdMilli,
    availableMicrophones,
    clipboardFallback,
    launchAtStartup,
    hardwareProfile,
    modelStatus,
    environmentHealth,
    recoveryCheckpoint,
    transcriberStatus,
    runtimeLogs,
    settingsSavedAt,
    error,
    setMode,
    setHotkey,
    setModelProfile,
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
    setVadDisabled,
    setVadRmsThresholdMilli,
    setClipboardFallback,
    setLaunchAtStartup,
    applyHighAccuracyPreset,
    onHotkeyDown,
    onCancel,
    saveSettings,
    detectAndApplyProfile,
    refreshPhase4Health,
    clearLogs,
    acknowledgeRecovery,
    clearErrorMessage,
    refreshMicrophones,
    syncPhaseStatus,
    reportError,
  };
}

export type AppController = ReturnType<typeof useAppController>;
