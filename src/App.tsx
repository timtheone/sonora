import { useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import "./App.css";
import { DEFAULT_SETTINGS } from "./domain/settings";
import {
  generateSilenceSamples,
  generateSpeechLikeSamples,
} from "./domain/audio-chunk";
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
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!isTauriRuntime()) {
      setAvailable(false);
      return;
    }

    setAvailable(true);
    getPhase1Status()
      .then((status) => {
        setMode(status.mode);
        setState(status.state);
      })
      .catch((cause) => {
        setError(String(cause));
      });

    const unlisten = listen<TranscriptPayload>("dictation:transcript", (event) => {
      setRecentTranscripts((previous) => [event.payload.text, ...previous].slice(0, 3));
    });

    return () => {
      unlisten.then((dispose) => dispose());
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

  return (
    <main className="app">
      <h1>Sonora Dictation</h1>
      <p>{statusLabel}</p>
      <section className="panel">
        <h2>Locked defaults</h2>
        <ul>
          <li>Hotkey: {DEFAULT_SETTINGS.hotkey}</li>
          <li>Mode: {DEFAULT_SETTINGS.mode}</li>
          <li>Language: {DEFAULT_SETTINGS.language}</li>
          <li>Profile: {DEFAULT_SETTINGS.modelProfile}</li>
        </ul>
      </section>
      <section className="panel">
        <h2>Phase 1 Controls</h2>
        <div className="actions">
          <button disabled={!available} onClick={() => updateMode("push_to_toggle")}>
            Use Toggle Mode
          </button>
          <button disabled={!available} onClick={() => updateMode("push_to_talk")}>
            Use Push Mode
          </button>
          <button disabled={!available} onClick={onHotkeyDown}>
            Hotkey Down
          </button>
          <button disabled={!available} onClick={onHotkeyUp}>
            Hotkey Up
          </button>
          <button disabled={!available} onClick={feedSilence}>
            Feed Silence Chunk
          </button>
          <button disabled={!available} onClick={feedSpeech}>
            Feed Speech Chunk
          </button>
          <button disabled={!available} onClick={onCancel}>
            Cancel
          </button>
        </div>
      </section>
      <section className="panel">
        <h2>Recent transcripts (Phase 1 event stream)</h2>
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
      {error ? <p className="error">Error: {error}</p> : null}
    </main>
  );
}

export default App;
