import { invoke } from "@tauri-apps/api/core";

export type DictationMode = "push_to_toggle" | "push_to_talk";
export type DictationState = "idle" | "listening" | "transcribing";

export interface ProfileTuning {
  min_chunk_samples: number;
  partial_cadence_ms: number;
}

export interface PipelineStatus {
  mode: DictationMode;
  state: DictationState;
  model_profile: "fast" | "balanced";
  tuning: ProfileTuning;
}

export interface TranscriptPayload {
  text: string;
}

export interface InputMicrophone {
  id: string;
  label: string;
  is_default: boolean;
}

export interface MicLevelPayload {
  level: number;
  peak: number;
  active: boolean;
}

export async function getPhase1Status(): Promise<PipelineStatus> {
  return invoke<PipelineStatus>("phase1_get_status");
}

export async function setPhase1Mode(mode: DictationMode): Promise<PipelineStatus> {
  return invoke<PipelineStatus>("phase1_set_mode", { mode });
}

export async function sendPhase1HotkeyDown(): Promise<PipelineStatus> {
  return invoke<PipelineStatus>("phase1_hotkey_down");
}

export async function sendPhase1HotkeyUp(): Promise<PipelineStatus> {
  return invoke<PipelineStatus>("phase1_hotkey_up");
}

export async function cancelPhase1(): Promise<PipelineStatus> {
  return invoke<PipelineStatus>("phase1_cancel");
}

export async function feedPhase1Audio(samples: number[]): Promise<string | null> {
  return invoke<string | null>("phase1_feed_audio", { samples });
}

export async function listPhase1Microphones(): Promise<InputMicrophone[]> {
  return invoke<InputMicrophone[]>("phase1_list_microphones");
}

export async function getPhase1LiveCaptureActive(): Promise<boolean> {
  return invoke<boolean>("phase1_get_live_capture_active");
}

export async function startPhase1LiveCapture(
  microphoneId: string | null,
): Promise<boolean> {
  return invoke<boolean>("phase1_start_live_capture", {
    microphone_id: microphoneId,
  });
}

export async function stopPhase1LiveCapture(): Promise<boolean> {
  return invoke<boolean>("phase1_stop_live_capture");
}
