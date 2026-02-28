import { memo, useEffect, useRef } from "react";
import {
  MicLevelPeakIndicator,
  type MicLevelPeakIndicatorHandle,
} from "./MicLevelPeakIndicator";
import { useAppControllerContext } from "../context/AppControllerContext";
import { feedPhase1Audio } from "../services/phase1";
import { generateSilenceSamples, generateSpeechLikeSamples } from "../domain/audio-chunk";
import { useLiveMicCapture } from "../hooks/useLiveMicCapture";

function Phase1ControlsComponent() {
  const {
    available,
    mode,
    updateMode,
    onHotkeyDown,
    onHotkeyUp,
    onCancel,
    syncPhaseStatus,
    selectedMicrophoneId,
    reportError,
  } = useAppControllerContext();

  const indicatorRef = useRef<MicLevelPeakIndicatorHandle>(null);

  const { liveMicActive, startLiveMic, stopLiveMic, stopLiveMicInternal } = useLiveMicCapture({
    available,
    selectedMicrophoneId,
    ensureListening: async () => {
      const current = await syncPhaseStatus();
      if (current?.state === "listening") {
        return current;
      }
      return onHotkeyDown();
    },
    stopListening: () => onCancel(),
    feedAudioChunk: (samples) => feedPhase1Audio(samples),
    onMicLevel: (level, peak, active) => {
      indicatorRef.current?.update(level, peak, active);
    },
    onError: reportError,
  });

  useEffect(() => {
    return () => {
      void stopLiveMicInternal();
    };
  }, [stopLiveMicInternal]);

  async function feedSilence() {
    try {
      await feedPhase1Audio(generateSilenceSamples());
    } catch (cause) {
      reportError(cause);
    }
  }

  async function feedSpeech() {
    try {
      await feedPhase1Audio(generateSpeechLikeSamples());
    } catch (cause) {
      reportError(cause);
    }
  }

  return (
    <section className="panel">
      <h2>Phase 1 Controls</h2>
      <p className="muted">Live mic: {liveMicActive ? "active" : "inactive"}</p>
      <MicLevelPeakIndicator ref={indicatorRef} />
      <div className="actions">
        <button
          disabled={!available || mode === "push_to_toggle"}
          onClick={() => updateMode("push_to_toggle")}
        >
          Use Toggle Mode
        </button>
        <button
          disabled={!available || mode === "push_to_talk"}
          onClick={() => updateMode("push_to_talk")}
        >
          Use Push Mode
        </button>
        <button disabled={!available} onClick={onHotkeyDown}>
          Hotkey Down
        </button>
        <button disabled={!available} onClick={onHotkeyUp}>
          Hotkey Up
        </button>
        <button disabled={!available || liveMicActive} onClick={startLiveMic}>
          Start Live Mic
        </button>
        <button disabled={!available || !liveMicActive} onClick={stopLiveMic}>
          Stop Live Mic
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
  );
}

export const Phase1Controls = memo(Phase1ControlsComponent);
