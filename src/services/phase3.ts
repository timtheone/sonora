import { invoke } from "@tauri-apps/api/core";
import type { AppSettings } from "./phase2";

export type HardwareTier = "low" | "mid" | "high";
export type ModelProfile = "fast" | "balanced";

export interface HardwareProfileStatus {
  logical_cores: number;
  hardware_tier: HardwareTier;
  recommended_profile: ModelProfile;
}

export interface ProfileTuning {
  min_chunk_samples: number;
  partial_cadence_ms: number;
}

export interface ModelStatus {
  profile: ModelProfile;
  hardware_tier: HardwareTier;
  model_path: string;
  model_exists: boolean;
  checked_paths: string[];
  tuning: ProfileTuning;
}

export async function getHardwareProfileStatus(): Promise<HardwareProfileStatus> {
  return invoke<HardwareProfileStatus>("phase3_get_hardware_profile");
}

export async function autoSelectHardwareProfile(): Promise<AppSettings> {
  return invoke<AppSettings>("phase3_auto_select_profile");
}

export async function getModelStatus(): Promise<ModelStatus> {
  return invoke<ModelStatus>("phase3_get_model_status");
}

export async function setModelPath(path: string | null): Promise<AppSettings> {
  return invoke<AppSettings>("phase3_set_model_path", { path });
}
