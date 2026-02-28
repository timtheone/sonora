class SonoraMicCaptureProcessor extends AudioWorkletProcessor {
  constructor(options) {
    super();
    this.chunkSize = options?.processorOptions?.chunkSize ?? 2048;
    this.pending = new Float32Array(0);
  }

  process(inputs) {
    const input = inputs[0];
    if (!input || input.length === 0) {
      return true;
    }

    const channel = input[0];
    if (!channel || channel.length === 0) {
      return true;
    }

    const combined = new Float32Array(this.pending.length + channel.length);
    combined.set(this.pending, 0);
    combined.set(channel, this.pending.length);

    let offset = 0;
    while (combined.length - offset >= this.chunkSize) {
      const chunk = combined.slice(offset, offset + this.chunkSize);
      this.port.postMessage(chunk);
      offset += this.chunkSize;
    }

    this.pending = combined.slice(offset);
    return true;
  }
}

registerProcessor("sonora-mic-capture", SonoraMicCaptureProcessor);
