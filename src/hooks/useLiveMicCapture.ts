import { useCallback, useEffect, useRef, useState } from "react";
import { downsampleTo16k } from "../domain/audio-resample";
import type { PipelineStatus } from "../services/phase1";

const MIC_CAPTURE_WORKLET_URL = new URL("../audio/mic-capture.worklet.js", import.meta.url);

interface UseLiveMicCaptureOptions {
  available: boolean;
  selectedMicrophoneId: string;
  ensureListening: () => Promise<PipelineStatus | null>;
  stopListening: () => Promise<PipelineStatus | null>;
  feedAudioChunk: (samples: number[]) => Promise<string | null>;
  onMicLevel: (level: number, peak: number, active: boolean) => void;
  onError: (cause: unknown) => void;
}

export function useLiveMicCapture({
  available,
  selectedMicrophoneId,
  ensureListening,
  stopListening,
  feedAudioChunk,
  onMicLevel,
  onError,
}: UseLiveMicCaptureOptions) {
  const [liveMicActive, setLiveMicActive] = useState(false);

  const audioContextRef = useRef<AudioContext | null>(null);
  const mediaStreamRef = useRef<MediaStream | null>(null);
  const sourceNodeRef = useRef<MediaStreamAudioSourceNode | null>(null);
  const workletNodeRef = useRef<AudioWorkletNode | null>(null);
  const micSmoothedLevelRef = useRef(0);
  const micPeakLevelRef = useRef(0);

  const stopLiveMicInternal = useCallback(async () => {
    if (workletNodeRef.current) {
      workletNodeRef.current.port.onmessage = null;
      workletNodeRef.current.disconnect();
    }
    sourceNodeRef.current?.disconnect();
    mediaStreamRef.current?.getTracks().forEach((track) => track.stop());

    if (audioContextRef.current) {
      await audioContextRef.current.close();
    }

    workletNodeRef.current = null;
    sourceNodeRef.current = null;
    mediaStreamRef.current = null;
    audioContextRef.current = null;
    micSmoothedLevelRef.current = 0;
    micPeakLevelRef.current = 0;
    onMicLevel(0, 0, false);
    setLiveMicActive(false);
  }, [onMicLevel]);

  const startLiveMic = useCallback(async () => {
    if (!available || liveMicActive) {
      return;
    }

    try {
      const listeningStatus = await ensureListening();
      if (!listeningStatus) {
        return;
      }

      const minChunkSamples = Math.max(
        8_000,
        listeningStatus.tuning?.min_chunk_samples ?? 32_000,
      );
      const maxChunkSamples = minChunkSamples * 3;
      const partialCadenceMs = Math.max(
        300,
        listeningStatus.tuning?.partial_cadence_ms ?? 1_200,
      );

      const mediaConstraints: MediaStreamConstraints = selectedMicrophoneId
        ? { audio: { deviceId: { exact: selectedMicrophoneId } } }
        : { audio: true };

      const stream = await navigator.mediaDevices.getUserMedia(mediaConstraints);
      const audioContext = new AudioContext();
      const source = audioContext.createMediaStreamSource(stream);
      await audioContext.audioWorklet.addModule(MIC_CAPTURE_WORKLET_URL);
      const workletNode = new AudioWorkletNode(audioContext, "sonora-mic-capture", {
        numberOfInputs: 1,
        numberOfOutputs: 0,
        channelCount: 1,
        processorOptions: {
          chunkSize: 2048,
        },
      });

      let feeding = false;
      let pendingSamples: number[] = [];
      let lastFeedAtMs = 0;
      workletNode.port.onmessage = async (event: MessageEvent<Float32Array>) => {
        if (!workletNodeRef.current) {
          return;
        }

        const input = event.data;
        let energySum = 0;
        let peak = 0;
        for (let index = 0; index < input.length; index += 1) {
          const sample = input[index];
          const absolute = Math.abs(sample);
          energySum += sample * sample;
          if (absolute > peak) {
            peak = absolute;
          }
        }

        const rms = Math.sqrt(energySum / input.length);
        const scaledLevel = Math.min(1, rms * 14);
        const previousLevel = micSmoothedLevelRef.current;
        const smoothedLevel =
          scaledLevel >= previousLevel
            ? scaledLevel
            : previousLevel * 0.84 + scaledLevel * 0.16;
        micSmoothedLevelRef.current = smoothedLevel;
        micPeakLevelRef.current = Math.max(micPeakLevelRef.current * 0.96, peak);
        const signalActive = smoothedLevel > 0.08 || peak > 0.12;
        onMicLevel(smoothedLevel, micPeakLevelRef.current, signalActive);

        const downsampled = downsampleTo16k(input, audioContext.sampleRate);
        if (downsampled.length === 0) {
          return;
        }

        pendingSamples.push(...downsampled);
        if (feeding) {
          return;
        }

        if (pendingSamples.length < minChunkSamples) {
          return;
        }

        const now = performance.now();
        if (now - lastFeedAtMs < partialCadenceMs) {
          return;
        }

        const nextChunkSize = Math.min(maxChunkSamples, pendingSamples.length);
        const chunk = pendingSamples.splice(0, nextChunkSize);

        if (pendingSamples.length > maxChunkSamples * 5) {
          pendingSamples = pendingSamples.slice(-maxChunkSamples * 2);
        }

        feeding = true;
        lastFeedAtMs = now;
        try {
          await feedAudioChunk(chunk);
        } catch (cause) {
          onError(cause);
        } finally {
          feeding = false;
        }
      };

      source.connect(workletNode);

      mediaStreamRef.current = stream;
      audioContextRef.current = audioContext;
      sourceNodeRef.current = source;
      workletNodeRef.current = workletNode;

      setLiveMicActive(true);
    } catch (cause) {
      await stopLiveMicInternal();
      await stopListening();
      onError(cause);
    }
  }, [
    available,
    liveMicActive,
    ensureListening,
    selectedMicrophoneId,
    feedAudioChunk,
    onMicLevel,
    onError,
    stopListening,
    stopLiveMicInternal,
  ]);

  const stopLiveMic = useCallback(async () => {
    await stopLiveMicInternal();
    await stopListening();
  }, [stopLiveMicInternal, stopListening]);

  useEffect(() => {
    return () => {
      void stopLiveMicInternal();
    };
  }, [stopLiveMicInternal]);

  return {
    liveMicActive,
    startLiveMic,
    stopLiveMic,
    stopLiveMicInternal,
  };
}
