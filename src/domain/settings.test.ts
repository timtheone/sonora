import { describe, expect, it } from "vitest";
import { DEFAULT_SETTINGS, normalizeSettings } from "./settings";

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
});
