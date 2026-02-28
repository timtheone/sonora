import type { DictationMode } from "./dictation-machine";

export type ModelProfile = "balanced" | "fast";
export type WhisperBackendPreference = "auto" | "cpu" | "cuda";

export const CHUNK_DURATION_MIN_MS = 500;
export const CHUNK_DURATION_MAX_MS = 4000;
export const PARTIAL_CADENCE_MIN_MS = 300;
export const PARTIAL_CADENCE_MAX_MS = 2500;

function defaultChunkDurationMsForProfile(profile: ModelProfile): number {
  return profile === "fast" ? 1000 : 2000;
}

function defaultPartialCadenceMsForProfile(profile: ModelProfile): number {
  return profile === "fast" ? 900 : 1400;
}

export function effectiveChunkDurationMs(
  profile: ModelProfile,
  value: number | null | undefined,
): number {
  if (value === null || value === undefined) {
    return defaultChunkDurationMsForProfile(profile);
  }
  return Math.max(CHUNK_DURATION_MIN_MS, Math.min(CHUNK_DURATION_MAX_MS, Math.round(value)));
}

export function effectivePartialCadenceMs(
  profile: ModelProfile,
  value: number | null | undefined,
): number {
  if (value === null || value === undefined) {
    return defaultPartialCadenceMsForProfile(profile);
  }
  return Math.max(PARTIAL_CADENCE_MIN_MS, Math.min(PARTIAL_CADENCE_MAX_MS, Math.round(value)));
}

export interface AppSettings {
  hotkey: string;
  mode: DictationMode;
  language: "en";
  modelProfile: ModelProfile;
  modelPath: string | null;
  microphoneId: string | null;
  micSensitivityPercent: number;
  chunkDurationMs: number;
  partialCadenceMs: number;
  whisperBackendPreference: WhisperBackendPreference;
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
  chunkDurationMs: 2000,
  partialCadenceMs: 1400,
  whisperBackendPreference: "auto",
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
    chunkDurationMs: effectiveChunkDurationMs(
      input.modelProfile ?? DEFAULT_SETTINGS.modelProfile,
      input.chunkDurationMs,
    ),
    partialCadenceMs: effectivePartialCadenceMs(
      input.modelProfile ?? DEFAULT_SETTINGS.modelProfile,
      input.partialCadenceMs,
    ),
    whisperBackendPreference:
      input.whisperBackendPreference ?? DEFAULT_SETTINGS.whisperBackendPreference,
    clipboardFallback: input.clipboardFallback ?? DEFAULT_SETTINGS.clipboardFallback,
    launchAtStartup: input.launchAtStartup ?? DEFAULT_SETTINGS.launchAtStartup,
  };
}
