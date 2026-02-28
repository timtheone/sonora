import { invoke } from "@tauri-apps/api/core";

export type SessionType = "x11" | "wayland" | "unknown";
export type PermissionState = "ready" | "needs_setup" | "unknown";

export interface EnvironmentHealth {
  os: string;
  session_type: SessionType;
  input_injection_permission: PermissionState;
  notes: string[];
}

export interface RecoveryCheckpoint {
  clean_shutdown: boolean;
  recovery_notice_pending: boolean;
  launch_count: number;
  last_start_unix_ms: number | null;
  last_shutdown_unix_ms: number | null;
}

export interface TranscriberStatus {
  ready: boolean;
  description: string;
  resolved_binary_path: string | null;
  checked_binary_paths: string[];
  resolved_model_path: string;
  model_exists: boolean;
}

export async function getEnvironmentHealth(): Promise<EnvironmentHealth> {
  return invoke<EnvironmentHealth>("phase4_get_environment_health");
}

export async function getRuntimeLogs(limit?: number): Promise<string[]> {
  return invoke<string[]>("phase4_get_runtime_logs", { limit });
}

export async function clearRuntimeLogs(): Promise<void> {
  return invoke<void>("phase4_clear_runtime_logs");
}

export async function getTranscriberStatus(): Promise<TranscriberStatus> {
  return invoke<TranscriberStatus>("phase4_get_transcriber_status");
}

export async function getRecoveryCheckpoint(): Promise<RecoveryCheckpoint> {
  return invoke<RecoveryCheckpoint>("phase4_get_recovery_checkpoint");
}

export async function acknowledgeRecoveryNotice(): Promise<RecoveryCheckpoint> {
  return invoke<RecoveryCheckpoint>("phase4_acknowledge_recovery_notice");
}

export async function markCleanShutdown(): Promise<RecoveryCheckpoint> {
  return invoke<RecoveryCheckpoint>("phase4_mark_clean_shutdown");
}
