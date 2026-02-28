import { invoke } from "@tauri-apps/api/core";
import type { DictationMode } from "./phase1";

export type WhisperBackendPreference = "auto" | "cpu" | "cuda";
export type SttEngine = "whisper_cpp" | "faster_whisper";
export type FasterWhisperComputeType = "auto" | "int8" | "float16" | "float32";

export interface AppSettings {
  hotkey: string;
  mode: DictationMode;
  language: "en";
  model_profile: "fast" | "balanced";
  stt_engine: SttEngine;
  model_path: string | null;
  microphone_id: string | null;
  mic_sensitivity_percent: number;
  chunk_duration_ms: number | null;
  partial_cadence_ms: number | null;
  whisper_backend_preference: WhisperBackendPreference;
  faster_whisper_model: string | null;
  faster_whisper_compute_type: FasterWhisperComputeType;
  faster_whisper_beam_size: number;
  clipboard_fallback: boolean;
  launch_at_startup: boolean;
}

export interface AppSettingsPatch {
  hotkey?: string;
  mode?: DictationMode;
  model_profile?: "fast" | "balanced";
  stt_engine?: SttEngine;
  model_path?: string | null;
  microphone_id?: string | null;
  mic_sensitivity_percent?: number;
  chunk_duration_ms?: number;
  partial_cadence_ms?: number;
  whisper_backend_preference?: WhisperBackendPreference;
  faster_whisper_model?: string | null;
  faster_whisper_compute_type?: FasterWhisperComputeType;
  faster_whisper_beam_size?: number;
  clipboard_fallback?: boolean;
  launch_at_startup?: boolean;
}

export type InsertionStatus = "success" | "fallback" | "failure";

export interface InsertionRecord {
  text: string;
  status: InsertionStatus;
}

export async function getPhase2Settings(): Promise<AppSettings> {
  return invoke<AppSettings>("phase2_get_settings");
}

export async function updatePhase2Settings(
  patch: AppSettingsPatch,
): Promise<AppSettings> {
  return invoke<AppSettings>("phase2_update_settings", { patch });
}

export async function getPhase2RecentInsertions(): Promise<InsertionRecord[]> {
  return invoke<InsertionRecord[]>("phase2_get_recent_insertions");
}

export async function insertPhase2Text(text: string): Promise<InsertionRecord> {
  return invoke<InsertionRecord>("phase2_insert_text", { text });
}
