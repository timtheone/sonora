export const SAMPLE_RATE_HZ = 16_000;

export function generateSilenceSamples(sampleCount = 1024): number[] {
  return Array.from({ length: sampleCount }, () => 0);
}

export function generateSpeechLikeSamples(sampleCount = 1024): number[] {
  return Array.from({ length: sampleCount }, (_, index) => {
    const angle = index * 0.1;
    return Math.sin(angle) * 0.2;
  });
}
