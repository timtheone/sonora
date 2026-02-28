import { memo } from "react";
import type { DictationMode } from "../services/phase1";
import { useAppControllerContext } from "../context/AppControllerContext";

function Phase2SettingsPanelComponent() {
  const {
    available,
    hotkey,
    mode,
    modelProfile,
    modelPath,
    selectedMicrophoneId,
    availableMicrophones,
    clipboardFallback,
    launchAtStartup,
    settingsSavedAt,
    setHotkey,
    setMode,
    setModelProfile,
    setModelPathInput,
    setSelectedMicrophoneId,
    setClipboardFallback,
    setLaunchAtStartup,
    saveSettings,
    saveModelPathOnly,
    refreshMicrophones,
  } = useAppControllerContext();

  return (
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
          onChange={(event) =>
            setModelProfile(event.currentTarget.value as "fast" | "balanced")
          }
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
        <button disabled={!available} onClick={saveSettings}>
          Save Settings
        </button>
        <button disabled={!available} onClick={saveModelPathOnly}>
          Save Model Path
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
