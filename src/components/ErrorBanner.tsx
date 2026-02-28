import { memo } from "react";
import { useAppControllerContext } from "../context/AppControllerContext";

function ErrorBannerComponent() {
  const { error, clearErrorMessage } = useAppControllerContext();

  if (!error) {
    return null;
  }

  return (
    <div className="error-block">
      <p className="error">Error: {error}</p>
      <button onClick={clearErrorMessage}>Clear Error</button>
    </div>
  );
}

export const ErrorBanner = memo(ErrorBannerComponent);
