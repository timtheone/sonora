import { invoke } from "@tauri-apps/api/core";
import type { DictationMode } from "./phase1";

export interface AppSettings {
  hotkey: string;
  mode: DictationMode;
  language: "en";
  model_profile: string;
  microphone_id: string | null;
  clipboard_fallback: boolean;
}

export interface AppSettingsPatch {
  hotkey?: string;
  mode?: DictationMode;
  model_profile?: string;
  microphone_id?: string | null;
  clipboard_fallback?: boolean;
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
