import "./App.css";
import { DEFAULT_SETTINGS } from "./domain/settings";

function App() {
  return (
    <main className="app">
      <h1>Sonora Dictation</h1>
      <p>Phase 0 scaffold is ready. Core dictation is next.</p>
      <section className="panel">
        <h2>Current defaults</h2>
        <ul>
          <li>Hotkey: {DEFAULT_SETTINGS.hotkey}</li>
          <li>Mode: {DEFAULT_SETTINGS.mode}</li>
          <li>Language: {DEFAULT_SETTINGS.language}</li>
          <li>Profile: {DEFAULT_SETTINGS.modelProfile}</li>
        </ul>
      </section>
    </main>
  );
}

export default App;
