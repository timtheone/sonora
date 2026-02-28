import { memo } from "react";
import { useAppControllerContext } from "../context/AppControllerContext";

function RecentTranscriptsPanelComponent() {
  const { recentTranscripts } = useAppControllerContext();

  return (
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
  );
}

export const RecentTranscriptsPanel = memo(RecentTranscriptsPanelComponent);
