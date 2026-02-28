import { memo } from "react";
import { useAppControllerContext } from "../context/AppControllerContext";

function Phase4EnvironmentPanelComponent() {
  const {
    available,
    environmentHealth,
    transcriberStatus,
    runtimeLogs,
    refreshPhase4Health,
    clearLogs,
  } = useAppControllerContext();

  return (
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
          <li>Compute backend: {transcriberStatus.compute_backend}</li>
          <li>GPU active: {transcriberStatus.using_gpu ? "yes" : "no"}</li>
          <li>Sidecar binary: {transcriberStatus.resolved_binary_path ?? "not resolved"}</li>
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
        <button disabled={!available} onClick={refreshPhase4Health}>
          Refresh Health + Logs
        </button>
        <button disabled={!available} onClick={clearLogs}>
          Clear Logs
        </button>
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
  );
}

export const Phase4EnvironmentPanel = memo(Phase4EnvironmentPanelComponent);
