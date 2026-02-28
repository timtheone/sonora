import { memo, useState } from "react";
import { useAppControllerContext } from "../context/AppControllerContext";

function Phase2InsertionPanelComponent() {
  const { available, insertText } = useAppControllerContext();
  const [insertInput, setInsertInput] = useState("phase 2 insertion test text");

  return (
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
        <button disabled={!available} onClick={() => insertText(insertInput)}>
          Insert Text
        </button>
      </div>
    </section>
  );
}

export const Phase2InsertionPanel = memo(Phase2InsertionPanelComponent);
