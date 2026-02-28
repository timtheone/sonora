import { forwardRef, memo, useImperativeHandle, useRef, useState } from "react";

export interface MicLevelPeakIndicatorHandle {
  update: (level: number, peak: number, active: boolean) => void;
  reset: () => void;
}

const UI_UPDATE_INTERVAL_MS = 90;

const MicLevelPeakIndicator = memo(
  forwardRef<MicLevelPeakIndicatorHandle>(function MicLevelPeakIndicatorComponent(_, ref) {
    const [levelPercent, setLevelPercent] = useState(0);
    const [peakPercent, setPeakPercent] = useState(0);
    const [active, setActive] = useState(false);
    const lastUpdateAtRef = useRef(0);

    useImperativeHandle(ref, () => ({
      update(level, peak, nextActive) {
        const now = performance.now();
        if (now - lastUpdateAtRef.current < UI_UPDATE_INTERVAL_MS) {
          return;
        }
        lastUpdateAtRef.current = now;

        setLevelPercent(Math.round(Math.max(0, Math.min(1, level)) * 100));
        setPeakPercent(Math.round(Math.max(0, Math.min(1, peak)) * 100));
        setActive(nextActive);
      },
      reset() {
        lastUpdateAtRef.current = 0;
        setLevelPercent(0);
        setPeakPercent(0);
        setActive(false);
      },
    }));

    return (
      <div className="mic-indicator" aria-live="polite">
        <div className="mic-indicator-head">
          <span className="mic-indicator-label">Mic capture</span>
          <span className={`mic-dot ${active ? "active" : "idle"}`}>
            {active ? "Signal" : "Quiet"}
          </span>
        </div>
        <div className="mic-meter-track">
          <div className="mic-meter-fill" style={{ width: `${levelPercent}%` }} />
        </div>
        <p className="muted mic-stats">Level {levelPercent}% | Peak {peakPercent}%</p>
      </div>
    );
  }),
);

export { MicLevelPeakIndicator };
