import { listen } from "@tauri-apps/api/event";
import { memo, useEffect, useRef, useState } from "react";
import {
  MicLevelPeakIndicator,
  type MicLevelPeakIndicatorHandle,
} from "./MicLevelPeakIndicator";
import { useAppControllerContext } from "../context/AppControllerContext";
import {
  getPhase1LiveCaptureActive,
  startPhase1LiveCapture,
  stopPhase1LiveCapture,
  type MicLevelPayload,
} from "../services/phase1";

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
  const reportErrorRef = useRef(reportError);
  const [liveMicActive, setLiveMicActive] = useState(false);

  useEffect(() => {
    reportErrorRef.current = reportError;
  }, [reportError]);

  useEffect(() => {
    if (!available) {
      setLiveMicActive(false);
      indicatorRef.current?.reset();
      return;
    }

    let cancelled = false;
    let disposeMicLevel: (() => void) | null = null;
    let disposeLiveMic: (() => void) | null = null;

    void (async () => {
      try {
        disposeMicLevel = await listen<MicLevelPayload>("dictation:mic-level", (event) => {
          indicatorRef.current?.update(event.payload.level, event.payload.peak, event.payload.active);
        });

        disposeLiveMic = await listen<{ active: boolean }>("dictation:live-mic", (event) => {
          setLiveMicActive(event.payload.active);
          if (!event.payload.active) {
            indicatorRef.current?.reset();
          }
        });

        const active = await getPhase1LiveCaptureActive();
        if (!cancelled) {
          setLiveMicActive(active);
          if (!active) {
            indicatorRef.current?.reset();
          }
        }
      } catch (cause) {
        if (!cancelled) {
          reportErrorRef.current(cause);
        }
      }
    })();

    return () => {
      cancelled = true;
      disposeMicLevel?.();
      disposeLiveMic?.();
      indicatorRef.current?.reset();
    };
  }, [available]);

  async function startLiveMic() {
    try {
      const current = await syncPhaseStatus();
      if (current?.state !== "listening") {
        const next = await onHotkeyDown();
        if (!next || next.state !== "listening") {
          return;
        }
      }

      await startPhase1LiveCapture(selectedMicrophoneId.trim() ? selectedMicrophoneId : null);
      setLiveMicActive(true);
    } catch (cause) {
      reportError(cause);
    }
  }

  async function stopLiveMic() {
    try {
      await stopPhase1LiveCapture();
      await onCancel();
      setLiveMicActive(false);
      indicatorRef.current?.reset();
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
        <button disabled={!available} onClick={onCancel}>
          Cancel
        </button>
      </div>
    </section>
  );
}

export const Phase1Controls = memo(Phase1ControlsComponent);
