import { describe, expect, it } from "vitest";
import {
  generateSilenceSamples,
  generateSpeechLikeSamples,
  SAMPLE_RATE_HZ,
} from "./audio-chunk";

describe("audio chunk helpers", () => {
  it("uses 16 kHz sample rate constant", () => {
    expect(SAMPLE_RATE_HZ).toBe(16_000);
  });

  it("generates silence chunks", () => {
    const chunk = generateSilenceSamples(12);
    expect(chunk).toEqual(Array.from({ length: 12 }, () => 0));
  });

  it("generates speech-like chunks", () => {
    const chunk = generateSpeechLikeSamples(12);
    expect(chunk).toHaveLength(12);
    expect(chunk.some((sample) => Math.abs(sample) > 0)).toBe(true);
  });
});
