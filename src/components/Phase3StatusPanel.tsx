import { memo } from "react";
import { useAppControllerContext } from "../context/AppControllerContext";

function Phase3StatusPanelComponent() {
  const { available, hardwareProfile, modelStatus, detectAndApplyProfile } =
    useAppControllerContext();

  return (
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
        <button disabled={!available} onClick={detectAndApplyProfile}>
          Auto-select Profile
        </button>
      </div>
    </section>
  );
}

export const Phase3StatusPanel = memo(Phase3StatusPanelComponent);
