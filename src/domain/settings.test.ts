import { describe, expect, it } from "vitest";
import {
  DEFAULT_SETTINGS,
  effectiveChunkDurationMs,
  effectivePartialCadenceMs,
  normalizeSettings,
} from "./settings";

describe("settings", () => {
  it("returns defaults when no settings are provided", () => {
    expect(normalizeSettings()).toEqual(DEFAULT_SETTINGS);
  });

  it("merges partial settings", () => {
    expect(
      normalizeSettings({
        mode: "push_to_talk",
        modelProfile: "fast",
      }),
    ).toMatchObject({
      mode: "push_to_talk",
      modelProfile: "fast",
      modelPath: DEFAULT_SETTINGS.modelPath,
      hotkey: DEFAULT_SETTINGS.hotkey,
      language: "en",
    });
  });

  it("falls back to default hotkey for empty value", () => {
    expect(normalizeSettings({ hotkey: " " }).hotkey).toBe(DEFAULT_SETTINGS.hotkey);
  });

  it("preserves launch-at-startup setting when provided", () => {
    expect(normalizeSettings({ launchAtStartup: true }).launchAtStartup).toBe(true);
  });

  it("clamps microphone sensitivity into supported range", () => {
    expect(normalizeSettings({ micSensitivityPercent: 20 }).micSensitivityPercent).toBe(50);
    expect(normalizeSettings({ micSensitivityPercent: 190 }).micSensitivityPercent).toBe(190);
    expect(normalizeSettings({ micSensitivityPercent: 500 }).micSensitivityPercent).toBe(300);
  });

  it("uses model defaults for missing latency tuning", () => {
    expect(effectiveChunkDurationMs("balanced", null)).toBe(2000);
    expect(effectiveChunkDurationMs("fast", null)).toBe(1000);
    expect(effectivePartialCadenceMs("balanced", null)).toBe(1400);
    expect(effectivePartialCadenceMs("fast", null)).toBe(900);
  });

  it("clamps latency tuning overrides", () => {
    expect(effectiveChunkDurationMs("balanced", 200)).toBe(500);
    expect(effectiveChunkDurationMs("balanced", 5000)).toBe(4000);
    expect(effectivePartialCadenceMs("balanced", 100)).toBe(300);
    expect(effectivePartialCadenceMs("balanced", 3000)).toBe(2500);
  });
});
