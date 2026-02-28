import { memo } from "react";
import { useAppControllerContext } from "../context/AppControllerContext";

function RecentInsertionsPanelComponent() {
  const { insertions } = useAppControllerContext();

  return (
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
  );
}

export const RecentInsertionsPanel = memo(RecentInsertionsPanelComponent);
