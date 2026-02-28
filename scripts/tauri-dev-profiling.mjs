import process from "node:process";
import { spawn } from "node:child_process";

const extraArgs = process.argv.slice(2);
const args = ["tauri", "dev", ...extraArgs];

const child = spawn("pnpm", args, {
  stdio: "inherit",
  shell: process.platform === "win32",
  env: {
    ...process.env,
    SONORA_PERF: "1",
  },
});

child.on("exit", (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }
  process.exitCode = code ?? 0;
});

child.on("error", (error) => {
  process.stderr.write(`Failed to start tauri dev profiling mode: ${error.message}\n`);
  process.exitCode = 1;
});
