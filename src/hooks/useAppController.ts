import { useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { DEFAULT_SETTINGS } from "../domain/settings";
import { appendInsertionRecord } from "../domain/insertion-history";
import {
  cancelPhase1,
  listPhase1Microphones,
  sendPhase1HotkeyDown,
  sendPhase1HotkeyUp,
  setPhase1Mode,
  getPhase1Status,
  type DictationMode,
  type DictationState,
  type TranscriptPayload,
  type PipelineStatus,
} from "../services/phase1";
import {
  getPhase2RecentInsertions,
  getPhase2Settings,
  insertPhase2Text,
  updatePhase2Settings,
  type InsertionRecord,
} from "../services/phase2";
import {
  autoSelectHardwareProfile,
  getHardwareProfileStatus,
  getModelStatus,
  setModelPath,
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

export function useAppController() {
  const [available, setAvailable] = useState(false);
  const [state, setState] = useState<DictationState>(FALLBACK_STATE);
  const [mode, setMode] = useState<DictationMode>(DEFAULT_SETTINGS.mode);
  const [recentTranscripts, setRecentTranscripts] = useState<string[]>([]);
  const [insertions, setInsertions] = useState<InsertionRecord[]>([]);
  const [hotkey, setHotkey] = useState(DEFAULT_SETTINGS.hotkey);
  const [modelProfile, setModelProfile] = useState<"fast" | "balanced">(
    DEFAULT_SETTINGS.modelProfile,
  );
  const [modelPath, setModelPathInput] = useState<string>("");
  const [selectedMicrophoneId, setSelectedMicrophoneId] = useState<string>("");
  const [micSensitivityPercent, setMicSensitivityPercent] =
    useState<number>(DEFAULT_SETTINGS.micSensitivityPercent);
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
      getPhase2RecentInsertions(),
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
          recentInsertions,
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
          setModelPathInput(settings.model_path ?? "");
          setSelectedMicrophoneId(settings.microphone_id ?? "");
          setMicSensitivityPercent(settings.mic_sensitivity_percent);
          setClipboardFallback(settings.clipboard_fallback);
          setLaunchAtStartup(settings.launch_at_startup);
          setInsertions(recentInsertions);
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
        setRecentTranscripts((previous) => [event.payload.text, ...previous].slice(0, 3));
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

    const unlistenInsertion = listen<InsertionRecord>("dictation:insertion", (event) => {
      setInsertions((previous) => appendInsertionRecord(previous, event.payload));
    });

    return () => {
      unlistenTranscript.then((dispose) => dispose());
      unlistenInsertion.then((dispose) => dispose());
      window.removeEventListener("beforeunload", onBeforeUnload);
    };
  }, []);

  async function updateMode(nextMode: DictationMode) {
    try {
      const status = await setPhase1Mode(nextMode);
      applyPhaseStatus(status);
      setError(null);
      return status;
    } catch (cause) {
      setError(String(cause));
      return null;
    }
  }

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

  async function onHotkeyUp() {
    try {
      const status = await sendPhase1HotkeyUp();
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
      const updated = await updatePhase2Settings({
        hotkey,
        mode,
        model_profile: modelProfile,
        model_path: modelPath.trim() ? modelPath.trim() : null,
        microphone_id: selectedMicrophoneId.trim() ? selectedMicrophoneId : null,
        mic_sensitivity_percent: Math.max(50, Math.min(300, Math.round(micSensitivityPercent))),
        clipboard_fallback: clipboardFallback,
        launch_at_startup: launchAtStartup,
      });
      setHotkey(updated.hotkey);
      setClipboardFallback(updated.clipboard_fallback);
      setMode(updated.mode);
      setModelProfile(updated.model_profile);
      setModelPathInput(updated.model_path ?? "");
      setSelectedMicrophoneId(updated.microphone_id ?? "");
      setMicSensitivityPercent(updated.mic_sensitivity_percent);
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

  async function insertText(text: string) {
    try {
      const record = await insertPhase2Text(text);
      setInsertions((previous) => appendInsertionRecord(previous, record));
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
      setModelPathInput(updatedSettings.model_path ?? "");
      setTranscriberStatus(await getTranscriberStatus());
      setSettingsSavedAt(new Date().toLocaleTimeString());
      setError(null);
    } catch (cause) {
      setError(String(cause));
    }
  }

  async function saveModelPathOnly() {
    try {
      const updated = await setModelPath(modelPath.trim() ? modelPath.trim() : null);
      setModelPathInput(updated.model_path ?? "");
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

  return {
    available,
    state,
    mode,
    statusLabel,
    recentTranscripts,
    insertions,
    hotkey,
    modelProfile,
    modelPath,
    selectedMicrophoneId,
    micSensitivityPercent,
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
    setModelPathInput,
    setSelectedMicrophoneId,
    setMicSensitivityPercent,
    setClipboardFallback,
    setLaunchAtStartup,
    updateMode,
    onHotkeyDown,
    onHotkeyUp,
    onCancel,
    saveSettings,
    insertText,
    detectAndApplyProfile,
    saveModelPathOnly,
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
