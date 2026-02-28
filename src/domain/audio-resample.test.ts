import { describe, expect, it } from "vitest";
import { downsampleTo16k } from "./audio-resample";

describe("audio resample", () => {
  it("returns same samples for 16k input", () => {
    const input = new Float32Array([0.1, -0.2, 0.3]);
    const output = downsampleTo16k(input, 16_000);
    expect(output[0]).toBeCloseTo(0.1, 6);
    expect(output[1]).toBeCloseTo(-0.2, 6);
    expect(output[2]).toBeCloseTo(0.3, 6);
  });

  it("returns empty when source sample rate is below 16k", () => {
    const input = new Float32Array([0.1, 0.2, 0.3]);
    const output = downsampleTo16k(input, 8_000);
    expect(output).toEqual([]);
  });

  it("downsamples 48k to 16k by averaging windows", () => {
    const input = new Float32Array([1, 1, 1, 0, 0, 0]);
    const output = downsampleTo16k(input, 48_000);
    expect(output.length).toBe(2);
    expect(output[0]).toBeCloseTo(1, 4);
    expect(output[1]).toBeCloseTo(0, 4);
  });
});
