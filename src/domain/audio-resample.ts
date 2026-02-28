export function downsampleTo16k(input: Float32Array, sourceSampleRate: number): number[] {
  if (sourceSampleRate === 16_000) {
    return Array.from(input);
  }

  if (sourceSampleRate < 16_000) {
    return [];
  }

  const ratio = sourceSampleRate / 16_000;
  const outputLength = Math.floor(input.length / ratio);
  const output = new Array<number>(outputLength);

  let position = 0;
  for (let index = 0; index < outputLength; index += 1) {
    const nextPosition = Math.min(Math.floor((index + 1) * ratio), input.length);
    let sum = 0;
    let count = 0;

    for (let cursor = position; cursor < nextPosition; cursor += 1) {
      sum += input[cursor];
      count += 1;
    }

    output[index] = count > 0 ? sum / count : 0;
    position = nextPosition;
  }

  return output;
}
