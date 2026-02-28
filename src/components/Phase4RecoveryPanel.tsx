import { memo } from "react";
import { useAppControllerContext } from "../context/AppControllerContext";

function Phase4RecoveryPanelComponent() {
  const {
    available,
    recoveryCheckpoint,
    acknowledgeRecovery,
    refreshPhase4Health,
  } = useAppControllerContext();

  return (
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
        <button disabled={!available} onClick={acknowledgeRecovery}>
          Acknowledge Recovery Notice
        </button>
        <button disabled={!available} onClick={refreshPhase4Health}>
          Refresh Recovery State
        </button>
      </div>
    </section>
  );
}

export const Phase4RecoveryPanel = memo(Phase4RecoveryPanelComponent);
