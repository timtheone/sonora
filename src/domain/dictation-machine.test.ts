import { describe, expect, it } from "vitest";
import { transitionState } from "./dictation-machine";

describe("dictation machine", () => {
  it("enters listening on hotkey down from idle", () => {
    expect(transitionState("idle", "push_to_toggle", "hotkey_down")).toBe(
      "listening",
    );
  });

  it("stops listening on second hotkey down in toggle mode", () => {
    expect(
      transitionState("listening", "push_to_toggle", "hotkey_down"),
    ).toBe("idle");
  });

  it("stops listening on hotkey up in push mode", () => {
    expect(transitionState("listening", "push_to_talk", "hotkey_up")).toBe(
      "idle",
    );
  });

  it("moves to transcribing when a speech segment is ready", () => {
    expect(
      transitionState("listening", "push_to_toggle", "speech_segment_ready"),
    ).toBe("transcribing");
  });

  it("moves to inserting when transcription completes", () => {
    expect(
      transitionState("transcribing", "push_to_toggle", "transcription_complete"),
    ).toBe("inserting");
  });

  it("always returns to idle on cancel", () => {
    expect(transitionState("listening", "push_to_toggle", "cancel")).toBe(
      "idle",
    );
    expect(transitionState("transcribing", "push_to_toggle", "cancel")).toBe(
      "idle",
    );
    expect(transitionState("inserting", "push_to_toggle", "cancel")).toBe(
      "idle",
    );
  });
});
