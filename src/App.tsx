import { useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import "./App.css";
import { DEFAULT_SETTINGS } from "./domain/settings";
import {
  generateSilenceSamples,
  generateSpeechLikeSamples,
} from "./domain/audio-chunk";
import { downsampleTo16k } from "./domain/audio-resample";
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
  acknowledgeRecoveryNotice,
  clearRuntimeLogs,
  getEnvironmentHealth,
  getRecoveryCheckpoint,
  getRuntimeLogs,
  getTranscriberStatus,
  markCleanShutdown,
  type EnvironmentHealth,
  type RecoveryCheckpoint,
  type TranscriberStatus,
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
  const [recoveryCheckpoint, setRecoveryCheckpoint] =
    useState<RecoveryCheckpoint | null>(null);
  const [transcriberStatus, setTranscriberStatus] =
    useState<TranscriberStatus | null>(null);
  const [liveMicActive, setLiveMicActive] = useState(false);
  const [micInputLevel, setMicInputLevel] = useState(0);
  const [micPeakLevel, setMicPeakLevel] = useState(0);
  const [micSignalActive, setMicSignalActive] = useState(false);
  const [runtimeLogs, setRuntimeLogs] = useState<string[]>([]);
  const [settingsSavedAt, setSettingsSavedAt] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const audioContextRef = useRef<AudioContext | null>(null);
  const mediaStreamRef = useRef<MediaStream | null>(null);
  const sourceNodeRef = useRef<MediaStreamAudioSourceNode | null>(null);
  const processorNodeRef = useRef<ScriptProcessorNode | null>(null);
  const micSmoothedLevelRef = useRef(0);

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
      },
    );

    const unlistenInsertion = listen<InsertionRecord>("dictation:insertion", (event) => {
      setInsertions((previous) => appendInsertionRecord(previous, event.payload));
    });

    return () => {
      void stopLiveMicInternal();
      unlistenTranscript.then((dispose) => dispose());
      unlistenInsertion.then((dispose) => dispose());
      window.removeEventListener("beforeunload", onBeforeUnload);
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

  async function clearErrorMessage() {
    setError(null);
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

  async function startLiveMic() {
    if (!available || liveMicActive) {
      return;
    }

    try {
      const currentStatus = await getPhase1Status();
      const listeningStatus =
        currentStatus.state === "listening"
          ? currentStatus
          : await sendPhase1HotkeyDown();
      setState(listeningStatus.state);
      const minChunkSamples = Math.max(
        8_000,
        listeningStatus.tuning?.min_chunk_samples ?? 32_000,
      );
      const maxChunkSamples = minChunkSamples * 3;
      const partialCadenceMs = Math.max(
        300,
        listeningStatus.tuning?.partial_cadence_ms ?? 1_200,
      );

      const mediaConstraints: MediaStreamConstraints = selectedMicrophoneId
        ? { audio: { deviceId: { exact: selectedMicrophoneId } } }
        : { audio: true };

      const stream = await navigator.mediaDevices.getUserMedia(mediaConstraints);
      const audioContext = new AudioContext();
      const source = audioContext.createMediaStreamSource(stream);
      const processor = audioContext.createScriptProcessor(4096, 1, 1);

      let feeding = false;
      let pendingSamples: number[] = [];
      let lastFeedAtMs = 0;
      processor.onaudioprocess = async (event) => {
        if (!processorNodeRef.current) {
          return;
        }

        const input = event.inputBuffer.getChannelData(0);
        let energySum = 0;
        let peak = 0;
        for (let index = 0; index < input.length; index += 1) {
          const sample = input[index];
          const absolute = Math.abs(sample);
          energySum += sample * sample;
          if (absolute > peak) {
            peak = absolute;
          }
        }

        const rms = Math.sqrt(energySum / input.length);
        const scaledLevel = Math.min(1, rms * 14);
        const previousLevel = micSmoothedLevelRef.current;
        const smoothedLevel =
          scaledLevel >= previousLevel
            ? scaledLevel
            : previousLevel * 0.84 + scaledLevel * 0.16;
        micSmoothedLevelRef.current = smoothedLevel;
        setMicInputLevel(smoothedLevel);
        setMicPeakLevel((previousPeak) => Math.max(previousPeak * 0.96, peak));
        setMicSignalActive(smoothedLevel > 0.08 || peak > 0.12);

        const downsampled = downsampleTo16k(input, audioContext.sampleRate);
        if (downsampled.length === 0) {
          return;
        }

        pendingSamples.push(...downsampled);
        if (feeding) {
          return;
        }

        if (pendingSamples.length < minChunkSamples) {
          return;
        }

        const now = performance.now();
        if (now - lastFeedAtMs < partialCadenceMs) {
          return;
        }

        const nextChunkSize = Math.min(maxChunkSamples, pendingSamples.length);
        const chunk = pendingSamples.splice(0, nextChunkSize);

        if (pendingSamples.length > maxChunkSamples * 5) {
          pendingSamples = pendingSamples.slice(-maxChunkSamples * 2);
        }

        feeding = true;
        lastFeedAtMs = now;
        try {
          const transcript = await feedPhase1Audio(chunk);
          if (transcript) {
            setRecentTranscripts((previous) => [transcript, ...previous].slice(0, 3));
          }
        } catch (cause) {
          setError(String(cause));
        } finally {
          feeding = false;
        }
      };

      source.connect(processor);
      processor.connect(audioContext.destination);

      mediaStreamRef.current = stream;
      audioContextRef.current = audioContext;
      sourceNodeRef.current = source;
      processorNodeRef.current = processor;

      setLiveMicActive(true);
      setError(null);
    } catch (cause) {
      await stopLiveMicInternal();
      try {
        const status = await cancelPhase1();
        setState(status.state);
      } catch {
        // noop
      }
      setError(String(cause));
    }
  }

  async function stopLiveMic() {
    await stopLiveMicInternal();
    try {
      const status = await cancelPhase1();
      setState(status.state);
    } catch {
      // noop
    }
  }

  async function stopLiveMicInternal() {
    processorNodeRef.current?.disconnect();
    sourceNodeRef.current?.disconnect();
    mediaStreamRef.current?.getTracks().forEach((track) => track.stop());

    if (audioContextRef.current) {
      await audioContextRef.current.close();
    }

    processorNodeRef.current = null;
    sourceNodeRef.current = null;
    mediaStreamRef.current = null;
    audioContextRef.current = null;
    micSmoothedLevelRef.current = 0;
    setMicInputLevel(0);
    setMicPeakLevel(0);
    setMicSignalActive(false);
    setLiveMicActive(false);
  }

  return (
    <main className="app">
      <h1>Sonora Dictation</h1>
      <p>{statusLabel}</p>

      <section className="panel">
        <h2>Phase 1 Controls</h2>
        <p className="muted">Live mic: {liveMicActive ? "active" : "inactive"}</p>
        <div className="mic-indicator" aria-live="polite">
          <div className="mic-indicator-head">
            <span className="mic-indicator-label">Mic capture</span>
            <span className={`mic-dot ${micSignalActive ? "active" : "idle"}`}>
              {micSignalActive ? "Signal" : "Quiet"}
            </span>
          </div>
          <div className="mic-meter-track">
            <div
              className="mic-meter-fill"
              style={{ width: `${Math.round(micInputLevel * 100)}%` }}
            />
          </div>
          <p className="muted mic-stats">
            Level {Math.round(micInputLevel * 100)}% | Peak {Math.round(micPeakLevel * 100)}%
          </p>
        </div>
        <div className="actions">
          <button disabled={!available} onClick={() => updateMode("push_to_toggle")}>Use Toggle Mode</button>
          <button disabled={!available} onClick={() => updateMode("push_to_talk")}>Use Push Mode</button>
          <button disabled={!available} onClick={onHotkeyDown}>Hotkey Down</button>
          <button disabled={!available} onClick={onHotkeyUp}>Hotkey Up</button>
          <button disabled={!available || liveMicActive} onClick={startLiveMic}>Start Live Mic</button>
          <button disabled={!available || !liveMicActive} onClick={stopLiveMic}>Stop Live Mic</button>
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
        {transcriberStatus ? (
          <ul>
            <li>Transcriber ready: {transcriberStatus.ready ? "yes" : "no"}</li>
            <li>Transcriber: {transcriberStatus.description}</li>
            <li>
              Sidecar binary: {transcriberStatus.resolved_binary_path ?? "not resolved"}
            </li>
            <li>Resolved model: {transcriberStatus.resolved_model_path}</li>
            <li>Model exists: {transcriberStatus.model_exists ? "yes" : "no"}</li>
            <li>Tried binary paths:</li>
            {transcriberStatus.checked_binary_paths.map((path, index) => (
              <li key={`binary-path-${index}`}>{path}</li>
            ))}
          </ul>
        ) : (
          <p>Transcriber status not loaded yet.</p>
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

      <section className="panel">
        <h2>Phase 4 Recovery Status</h2>
        {recoveryCheckpoint ? (
          <ul>
            <li>Clean shutdown last session: {recoveryCheckpoint.clean_shutdown ? "yes" : "no"}</li>
            <li>Recovery notice pending: {recoveryCheckpoint.recovery_notice_pending ? "yes" : "no"}</li>
            <li>Launch count: {recoveryCheckpoint.launch_count}</li>
          </ul>
        ) : (
          <p>Recovery checkpoint not loaded yet.</p>
        )}
        <div className="actions">
          <button disabled={!available} onClick={acknowledgeRecovery}>Acknowledge Recovery Notice</button>
          <button disabled={!available} onClick={refreshPhase4Health}>Refresh Recovery State</button>
        </div>
      </section>

      {error ? (
        <div className="error-block">
          <p className="error">Error: {error}</p>
          <button onClick={clearErrorMessage}>Clear Error</button>
        </div>
      ) : null}
    </main>
  );
}

export default App;
