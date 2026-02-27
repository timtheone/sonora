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
  const [clipboardFallback, setClipboardFallback] = useState(true);
  const [insertInput, setInsertInput] = useState("phase 2 insertion test text");
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
    ])
      .then(([phase1Status, settings, recentInsertions]) => {
        setMode(phase1Status.mode);
        setState(phase1Status.state);
        setHotkey(settings.hotkey);
        setClipboardFallback(settings.clipboard_fallback);
        setInsertions(recentInsertions);
      })
      .catch((cause) => {
        setError(String(cause));
      });

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
        clipboard_fallback: clipboardFallback,
      });
      setHotkey(updated.hotkey);
      setClipboardFallback(updated.clipboard_fallback);
      setMode(updated.mode);
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
        <label className="field inline">
          <input
            type="checkbox"
            disabled={!available}
            checked={clipboardFallback}
            onChange={(event) => setClipboardFallback(event.currentTarget.checked)}
          />
          <span>Enable clipboard fallback insertion</span>
        </label>
        <div className="actions">
          <button disabled={!available} onClick={saveSettings}>Save Settings</button>
        </div>
        <p className="muted">
          {settingsSavedAt
            ? `Settings saved at ${settingsSavedAt}`
            : "No settings saved in this session yet."}
        </p>
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
