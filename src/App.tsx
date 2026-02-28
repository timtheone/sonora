import "./App.css";
import { Phase1Controls } from "./components/Phase1Controls";
import { Phase2SettingsPanel } from "./components/Phase2SettingsPanel";
import { Phase3StatusPanel } from "./components/Phase3StatusPanel";
import { Phase2InsertionPanel } from "./components/Phase2InsertionPanel";
import { Phase4EnvironmentPanel } from "./components/Phase4EnvironmentPanel";
import { RecentTranscriptsPanel } from "./components/RecentTranscriptsPanel";
import { RecentInsertionsPanel } from "./components/RecentInsertionsPanel";
import { Phase4RecoveryPanel } from "./components/Phase4RecoveryPanel";
import { ErrorBanner } from "./components/ErrorBanner";
import {
  AppControllerProvider,
  useAppControllerContext,
} from "./context/AppControllerContext";

function AppShell() {
  const { statusLabel } = useAppControllerContext();

  return (
    <main className="app">
      <h1>Sonora Dictation</h1>
      <p>{statusLabel}</p>

      <Phase1Controls />
      <Phase2SettingsPanel />
      <Phase3StatusPanel />
      <Phase2InsertionPanel />
      <Phase4EnvironmentPanel />
      <RecentTranscriptsPanel />
      <RecentInsertionsPanel />
      <Phase4RecoveryPanel />
      <ErrorBanner />
    </main>
  );
}

function App() {
  return (
    <AppControllerProvider>
      <AppShell />
    </AppControllerProvider>
  );
}

export default App;
