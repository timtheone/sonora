import process from "node:process";
import { spawn } from "node:child_process";

const extraArgs = process.argv.slice(2);
const args = ["tauri", "dev", ...extraArgs];
const commonOptions = {
  stdio: "inherit",
  env: {
    ...process.env,
    SONORA_PERF: "1",
  },
};

function quoteWindowsArg(value) {
  if (value.length === 0) {
    return '""';
  }
  if (!/[\s"&()<>^|]/.test(value)) {
    return value;
  }
  return `"${value.replace(/"/g, '""')}"`;
}

function spawnOnWindows() {
  try {
    return spawn("pnpm.cmd", args, commonOptions);
  } catch (error) {
    const isInvalidDirectSpawn =
      error instanceof Error &&
      "code" in error &&
      String(error.code).toUpperCase() === "EINVAL";

    if (!isInvalidDirectSpawn) {
      throw error;
    }

    const command = ["pnpm", ...args.map(quoteWindowsArg)].join(" ");
    return spawn("cmd.exe", ["/d", "/s", "/c", command], commonOptions);
  }
}

const child = process.platform === "win32" ? spawnOnWindows() : spawn("pnpm", args, commonOptions);

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
