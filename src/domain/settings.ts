import type { DictationMode } from "./dictation-machine";

export type ModelProfile = "balanced" | "fast";

export interface AppSettings {
  hotkey: string;
  mode: DictationMode;
  language: "en";
  modelProfile: ModelProfile;
  modelPath: string | null;
  microphoneId: string | null;
  micSensitivityPercent: number;
  clipboardFallback: boolean;
  launchAtStartup: boolean;
}

export const DEFAULT_SETTINGS: AppSettings = {
  hotkey: "CtrlOrCmd+Shift+U",
  mode: "push_to_toggle",
  language: "en",
  modelProfile: "balanced",
  modelPath: null,
  microphoneId: null,
  micSensitivityPercent: 170,
  clipboardFallback: true,
  launchAtStartup: false,
};

export function normalizeSettings(input: Partial<AppSettings> = {}): AppSettings {
  return {
    hotkey: input.hotkey?.trim() || DEFAULT_SETTINGS.hotkey,
    mode: input.mode ?? DEFAULT_SETTINGS.mode,
    language: "en",
    modelProfile: input.modelProfile ?? DEFAULT_SETTINGS.modelProfile,
    modelPath: input.modelPath ?? DEFAULT_SETTINGS.modelPath,
    microphoneId: input.microphoneId ?? DEFAULT_SETTINGS.microphoneId,
    micSensitivityPercent:
      input.micSensitivityPercent === undefined
        ? DEFAULT_SETTINGS.micSensitivityPercent
        : Math.max(50, Math.min(300, input.micSensitivityPercent)),
    clipboardFallback: input.clipboardFallback ?? DEFAULT_SETTINGS.clipboardFallback,
    launchAtStartup: input.launchAtStartup ?? DEFAULT_SETTINGS.launchAtStartup,
  };
}
