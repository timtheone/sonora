import { useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import "./App.css";
import { DEFAULT_SETTINGS } from "./domain/settings";
import {
  generateSilenceSamples,
  generateSpeechLikeSamples,
} from "./domain/audio-chunk";
import { appendInsertionRecord } from "./domain/insertion-history";
import {
  cancelPhase1,
  feedPhase1Audio,
  getPhase1Status,
  sendPhase1HotkeyDown,
  sendPhase1HotkeyUp,
  setPhase1Mode,
  type DictationMode,
  type DictationState,
  type TranscriptPayload,
} from "./services/phase1";
import {
  getPhase2RecentInsertions,
  getPhase2Settings,
  insertPhase2Text,
  updatePhase2Settings,
  type InsertionRecord,
} from "./services/phase2";
import {
  autoSelectHardwareProfile,
  getHardwareProfileStatus,
  getModelStatus,
  setModelPath,
  type HardwareProfileStatus,
  type ModelStatus,
} from "./services/phase3";
import {
  clearRuntimeLogs,
  getEnvironmentHealth,
  getRuntimeLogs,
  type EnvironmentHealth,
} from "./services/phase4";

const FALLBACK_STATE: DictationState = "idle";

function isTauriRuntime(): boolean {
  const windowWithTauri = window as Window & {
    __TAURI_INTERNALS__?: unknown;
  };
  return Boolean(windowWithTauri.__TAURI_INTERNALS__);
}

function App() {
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
  const [availableMicrophones, setAvailableMicrophones] = useState<
    Array<{ id: string; label: string }>
  >([]);
  const [clipboardFallback, setClipboardFallback] = useState(true);
  const [launchAtStartup, setLaunchAtStartup] = useState(false);
  const [insertInput, setInsertInput] = useState("phase 2 insertion test text");
  const [hardwareProfile, setHardwareProfile] =
    useState<HardwareProfileStatus | null>(null);
  const [modelStatus, setModelStatus] = useState<ModelStatus | null>(null);
  const [environmentHealth, setEnvironmentHealth] =
    useState<EnvironmentHealth | null>(null);
  const [runtimeLogs, setRuntimeLogs] = useState<string[]>([]);
  const [settingsSavedAt, setSettingsSavedAt] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

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
        ]) => {
        setMode(phase1Status.mode);
        setState(phase1Status.state);
        setHotkey(settings.hotkey);
        setModelProfile(settings.model_profile);
        setModelPathInput(settings.model_path ?? "");
        setSelectedMicrophoneId(settings.microphone_id ?? "");
        setClipboardFallback(settings.clipboard_fallback);
        setLaunchAtStartup(settings.launch_at_startup);
        setInsertions(recentInsertions);
        setHardwareProfile(hardware);
        setModelStatus(profileStatus);
          setEnvironmentHealth(envHealth);
          setRuntimeLogs(logs);
        },
      )
      .catch((cause) => {
        setError(String(cause));
      });

    refreshMicrophones();

    const unlistenTranscript = listen<TranscriptPayload>(
      "dictation:transcript",
      (event) => {
        setRecentTranscripts((previous) => [event.payload.text, ...previous].slice(0, 3));
      },
    );

    const unlistenInsertion = listen<InsertionRecord>("dictation:insertion", (event) => {
      setInsertions((previous) => appendInsertionRecord(previous, event.payload));
    });

    return () => {
      unlistenTranscript.then((dispose) => dispose());
      unlistenInsertion.then((dispose) => dispose());
    };
  }, []);

  const statusLabel = useMemo(() => {
    if (!available) {
      return "Web preview mode (run `pnpm tauri dev` for native commands).";
    }
    return `State: ${state} | Mode: ${mode}`;
  }, [available, mode, state]);

  async function updateMode(nextMode: DictationMode) {
    try {
      const status = await setPhase1Mode(nextMode);
      setMode(status.mode);
      setState(status.state);
      setError(null);
    } catch (cause) {
      setError(String(cause));
    }
  }

  async function onHotkeyDown() {
    try {
      const status = await sendPhase1HotkeyDown();
      setState(status.state);
      setError(null);
    } catch (cause) {
      setError(String(cause));
    }
  }

  async function onHotkeyUp() {
    try {
      const status = await sendPhase1HotkeyUp();
      setState(status.state);
      setError(null);
    } catch (cause) {
      setError(String(cause));
    }
  }

  async function onCancel() {
    try {
      const status = await cancelPhase1();
      setState(status.state);
      setError(null);
    } catch (cause) {
      setError(String(cause));
    }
  }

  async function feedSilence() {
    try {
      const transcript = await feedPhase1Audio(generateSilenceSamples());
      if (transcript) {
        setRecentTranscripts((previous) => [transcript, ...previous].slice(0, 3));
      }
      setError(null);
    } catch (cause) {
      setError(String(cause));
    }
  }

  async function feedSpeech() {
    try {
      const transcript = await feedPhase1Audio(generateSpeechLikeSamples());
      if (transcript) {
        setRecentTranscripts((previous) => [transcript, ...previous].slice(0, 3));
      }
      setError(null);
    } catch (cause) {
      setError(String(cause));
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
        clipboard_fallback: clipboardFallback,
        launch_at_startup: launchAtStartup,
      });
      setHotkey(updated.hotkey);
      setClipboardFallback(updated.clipboard_fallback);
      setMode(updated.mode);
      setModelProfile(updated.model_profile);
      setModelPathInput(updated.model_path ?? "");
      setSelectedMicrophoneId(updated.microphone_id ?? "");
      setLaunchAtStartup(updated.launch_at_startup);
      const status = await getModelStatus();
      setModelStatus(status);
      setSettingsSavedAt(new Date().toLocaleTimeString());
      setError(null);
    } catch (cause) {
      setError(String(cause));
    }
  }

  async function insertText() {
    try {
      const record = await insertPhase2Text(insertInput);
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
      const status = await getModelStatus();
      setModelStatus(status);
      setSettingsSavedAt(new Date().toLocaleTimeString());
      setError(null);
    } catch (cause) {
      setError(String(cause));
    }
  }

  async function refreshPhase4Health() {
    try {
      const [envHealth, logs] = await Promise.all([
        getEnvironmentHealth(),
        getRuntimeLogs(30),
      ]);
      setEnvironmentHealth(envHealth);
      setRuntimeLogs(logs);
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

  async function refreshMicrophones() {
    if (!navigator.mediaDevices?.enumerateDevices) {
      return;
    }

    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      stream.getTracks().forEach((track) => track.stop());
      const devices = await navigator.mediaDevices.enumerateDevices();
      const microphones = devices
        .filter((device) => device.kind === "audioinput")
        .map((device, index) => ({
          id: device.deviceId,
          label: device.label || `Microphone ${index + 1}`,
        }));
      setAvailableMicrophones(microphones);
      if (microphones.length > 0 && !selectedMicrophoneId) {
        setSelectedMicrophoneId(microphones[0].id);
      }
    } catch {
      setAvailableMicrophones([]);
    }
  }

  return (
    <main className="app">
      <h1>Sonora Dictation</h1>
      <p>{statusLabel}</p>

      <section className="panel">
        <h2>Phase 1 Controls</h2>
        <div className="actions">
          <button disabled={!available} onClick={() => updateMode("push_to_toggle")}>Use Toggle Mode</button>
          <button disabled={!available} onClick={() => updateMode("push_to_talk")}>Use Push Mode</button>
          <button disabled={!available} onClick={onHotkeyDown}>Hotkey Down</button>
          <button disabled={!available} onClick={onHotkeyUp}>Hotkey Up</button>
          <button disabled={!available} onClick={feedSilence}>Feed Silence Chunk</button>
          <button disabled={!available} onClick={feedSpeech}>Feed Speech Chunk</button>
          <button disabled={!available} onClick={onCancel}>Cancel</button>
        </div>
      </section>

      <section className="panel">
        <h2>Phase 2 Settings</h2>
        <label className="field">
          <span>Hotkey</span>
          <input
            disabled={!available}
            value={hotkey}
            onChange={(event) => setHotkey(event.currentTarget.value)}
            placeholder="CtrlOrCmd+Shift+U"
          />
        </label>
        <label className="field">
          <span>Mode</span>
          <select
            disabled={!available}
            value={mode}
            onChange={(event) => setMode(event.currentTarget.value as DictationMode)}
          >
            <option value="push_to_toggle">Push to toggle</option>
            <option value="push_to_talk">Push to talk</option>
          </select>
        </label>
        <label className="field">
          <span>Model profile</span>
          <select
            disabled={!available}
            value={modelProfile}
            onChange={(event) => setModelProfile(event.currentTarget.value as "fast" | "balanced")}
          >
            <option value="balanced">Balanced (base.en quantized)</option>
            <option value="fast">Fast (tiny.en quantized)</option>
          </select>
        </label>
        <label className="field">
          <span>Model path override (optional)</span>
          <input
            disabled={!available}
            value={modelPath}
            onChange={(event) => setModelPathInput(event.currentTarget.value)}
            placeholder="models/ggml-base.en-q5_1.bin"
          />
        </label>
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
        <div className="actions">
          <button disabled={!available} onClick={saveSettings}>Save Settings</button>
          <button disabled={!available} onClick={saveModelPathOnly}>Save Model Path</button>
          <button disabled={!available} onClick={refreshMicrophones}>Refresh Microphones</button>
        </div>
        <p className="muted">
          {settingsSavedAt
            ? `Settings saved at ${settingsSavedAt}`
            : "No settings saved in this session yet."}
        </p>
      </section>

      <section className="panel">
        <h2>Phase 3 Profile + Model Status</h2>
        {hardwareProfile ? (
          <ul>
            <li>Logical cores: {hardwareProfile.logical_cores}</li>
            <li>Detected tier: {hardwareProfile.hardware_tier}</li>
            <li>Recommended profile: {hardwareProfile.recommended_profile}</li>
          </ul>
        ) : (
          <p>Hardware profile not loaded yet.</p>
        )}
        {modelStatus ? (
          <ul>
            <li>Active profile: {modelStatus.profile}</li>
            <li>Resolved model path: {modelStatus.model_path}</li>
            <li>Model exists: {modelStatus.model_exists ? "yes" : "no"}</li>
            <li>Tried paths:</li>
            {modelStatus.checked_paths.map((path, index) => (
              <li key={`checked-path-${index}`}>{path}</li>
            ))}
            <li>
              Tuning: min chunk {modelStatus.tuning.min_chunk_samples} samples, cadence {modelStatus.tuning.partial_cadence_ms} ms
            </li>
          </ul>
        ) : (
          <p>Model status not loaded yet.</p>
        )}
        <div className="actions">
          <button disabled={!available} onClick={detectAndApplyProfile}>Auto-select Profile</button>
        </div>
      </section>

      <section className="panel">
        <h2>Phase 2 Insertion Test</h2>
        <label className="field">
          <span>Text to insert</span>
          <input
            disabled={!available}
            value={insertInput}
            onChange={(event) => setInsertInput(event.currentTarget.value)}
          />
        </label>
        <div className="actions">
          <button disabled={!available} onClick={insertText}>Insert Text</button>
        </div>
      </section>

      <section className="panel">
        <h2>Phase 4 Environment + Runtime Logs</h2>
        {environmentHealth ? (
          <ul>
            <li>OS: {environmentHealth.os}</li>
            <li>Session: {environmentHealth.session_type}</li>
            <li>Input permission state: {environmentHealth.input_injection_permission}</li>
            {environmentHealth.notes.map((note, index) => (
              <li key={`note-${index}`}>{note}</li>
            ))}
          </ul>
        ) : (
          <p>Environment health not loaded yet.</p>
        )}
        <div className="actions">
          <button disabled={!available} onClick={refreshPhase4Health}>Refresh Health + Logs</button>
          <button disabled={!available} onClick={clearLogs}>Clear Logs</button>
        </div>
        {runtimeLogs.length === 0 ? (
          <p>No runtime logs yet.</p>
        ) : (
          <div className="log-box">
            {runtimeLogs.map((line, index) => (
              <pre key={`log-${index}`}>{line}</pre>
            ))}
          </div>
        )}
      </section>

      <section className="panel">
        <h2>Recent transcripts (last 3)</h2>
        {recentTranscripts.length === 0 ? (
          <p>No transcripts emitted yet.</p>
        ) : (
          <ul>
            {recentTranscripts.map((item, index) => (
              <li key={`${item}-${index}`}>{item}</li>
            ))}
          </ul>
        )}
      </section>

      <section className="panel">
        <h2>Recent insertions (last 3)</h2>
        {insertions.length === 0 ? (
          <p>No insertion attempts yet.</p>
        ) : (
          <ul>
            {insertions.map((item, index) => (
              <li key={`${item.text}-${index}`}>
                <strong>[{item.status}]</strong> {item.text}
              </li>
            ))}
          </ul>
        )}
      </section>

      {error ? <p className="error">Error: {error}</p> : null}
    </main>
  );
}

export default App;
