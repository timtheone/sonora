import type { DictationMode } from "./dictation-machine";

export type ModelProfile = "balanced" | "fast";

export interface AppSettings {
  hotkey: string;
  mode: DictationMode;
  language: "en";
  modelProfile: ModelProfile;
  microphoneId: string | null;
  clipboardFallback: boolean;
}

export const DEFAULT_SETTINGS: AppSettings = {
  hotkey: "CtrlOrCmd+Shift+U",
  mode: "push_to_toggle",
  language: "en",
  modelProfile: "balanced",
  microphoneId: null,
  clipboardFallback: true,
};

export function normalizeSettings(input: Partial<AppSettings> = {}): AppSettings {
  return {
    hotkey: input.hotkey?.trim() || DEFAULT_SETTINGS.hotkey,
    mode: input.mode ?? DEFAULT_SETTINGS.mode,
    language: "en",
    modelProfile: input.modelProfile ?? DEFAULT_SETTINGS.modelProfile,
    microphoneId: input.microphoneId ?? DEFAULT_SETTINGS.microphoneId,
    clipboardFallback: input.clipboardFallback ?? DEFAULT_SETTINGS.clipboardFallback,
  };
}
