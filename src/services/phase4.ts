import { invoke } from "@tauri-apps/api/core";

export type SessionType = "x11" | "wayland" | "unknown";
export type PermissionState = "ready" | "needs_setup" | "unknown";

export interface EnvironmentHealth {
  os: string;
  session_type: SessionType;
  input_injection_permission: PermissionState;
  notes: string[];
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
