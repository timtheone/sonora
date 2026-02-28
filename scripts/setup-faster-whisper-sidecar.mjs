import { existsSync } from "node:fs";
import { promises as fs } from "node:fs";
import path from "node:path";
import process from "node:process";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(__dirname, "..");
const cacheRoot = path.join(projectRoot, ".cache", "faster-whisper-sidecar");
const venvDir = path.join(cacheRoot, "venv");
const pyInstallerWorkDir = path.join(cacheRoot, "pyinstaller-work");
const pyInstallerSpecDir = path.join(cacheRoot, "pyinstaller-spec");
const outputDir = path.join(projectRoot, "src-tauri", "resources", "bin");
const workerSource = path.join(projectRoot, "src-tauri", "resources", "faster-whisper", "worker.py");

const platform = process.platform;
const executableName = platform === "win32" ? "faster-whisper-worker.exe" : "faster-whisper-worker";
const metadataFileName = "faster-whisper-sidecar.json";

function missingCommandHelp(command) {
  const base = `Missing required command: '${command}'.`;
  if (platform === "win32") {
    return `${base}\nInstall Python 3 and reopen your shell.`;
  }
  if (platform === "darwin") {
    return `${base}\nInstall with: brew install python`;
  }
  return `${base}\nInstall with: sudo apt-get update && sudo apt-get install -y python3 python3-venv`;
}

function commandExists(command, args = ["--version"]) {
  const result = spawnSync(command, args, { stdio: "ignore" });
  if (result.error && result.error.code === "ENOENT") {
    return false;
  }
  return result.status === 0;
}

function runCommand(command, args, options = {}) {
  const result = spawnSync(command, args, {
    stdio: "inherit",
    ...options,
  });

  if (result.error) {
    if (result.error.code === "ENOENT") {
      throw new Error(missingCommandHelp(command));
    }
    throw new Error(`Failed to start '${command}': ${result.error.message}`);
  }
  if (result.status !== 0) {
    throw new Error(`Command failed (${result.status}): ${command} ${args.join(" ")}`);
  }
}

function parseArgs(argv) {
  const args = argv.slice(2);
  if (args.includes("--help") || args.includes("-h")) {
    return { help: true, force: false, pythonCommand: null };
  }

  let pythonCommand = null;
  for (let i = 0; i < args.length; i += 1) {
    const arg = args[i];
    if (arg === "--python") {
      pythonCommand = args[i + 1] ?? null;
      i += 1;
    } else if (arg.startsWith("--python=")) {
      pythonCommand = arg.slice("--python=".length);
    }
  }

  return {
    help: false,
    force: args.includes("--force"),
    pythonCommand,
  };
}

function printHelp() {
  process.stdout.write(
    [
      "Build the faster-whisper worker sidecar for current OS.",
      "",
      "Usage:",
      "  pnpm sidecar:setup:faster-whisper [--force] [--python <command>]",
      "",
      "Options:",
      "  --force            Recreate Python virtual environment",
      "  --python <cmd>     Python command to use (default: auto-detect)",
      "",
      "Output:",
      `  src-tauri/resources/bin/${executableName}`,
      `  src-tauri/resources/bin/${metadataFileName}`,
      "",
      "Requirements:",
      "  - Python 3.10+ with venv support",
      "  - Internet access during build for Python dependencies",
      "",
    ].join("\n"),
  );
}

function resolvePythonCommand(preferred) {
  const candidates = [];
  if (preferred) {
    candidates.push(preferred);
  }
  if (platform === "win32") {
    candidates.push("python", "py");
  } else {
    candidates.push("python3", "python");
  }

  for (const command of candidates) {
    const args = command === "py" ? ["-3", "--version"] : ["--version"];
    if (commandExists(command, args)) {
      return command;
    }
  }

  throw new Error(missingCommandHelp("python"));
}

function venvPythonPath() {
  if (platform === "win32") {
    return path.join(venvDir, "Scripts", "python.exe");
  }
  return path.join(venvDir, "bin", "python3");
}

async function ensureDir(dir) {
  await fs.mkdir(dir, { recursive: true });
}

async function ensureVenv(pythonCommand, force) {
  if (force && existsSync(venvDir)) {
    await fs.rm(venvDir, { recursive: true, force: true });
  }

  if (existsSync(venvPythonPath())) {
    return;
  }

  process.stdout.write("Creating Python virtual environment...\n");
  if (pythonCommand === "py") {
    runCommand("py", ["-3", "-m", "venv", venvDir]);
  } else {
    runCommand(pythonCommand, ["-m", "venv", venvDir]);
  }
}

async function installDependencies(venvPython) {
  const versionResult = spawnSync(
    venvPython,
    ["-c", "import sys; print(f'{sys.version_info.major}.{sys.version_info.minor}')"],
    { encoding: "utf8" },
  );
  if (versionResult.status !== 0) {
    throw new Error(
      `Failed to detect Python version for faster-whisper worker: ${versionResult.stderr || "unknown error"}`,
    );
  }

  const detectedVersion = (versionResult.stdout ?? "").trim();
  const [majorRaw, minorRaw] = detectedVersion.split(".");
  const major = Number(majorRaw);
  const minor = Number(minorRaw);
  if (!Number.isFinite(major) || !Number.isFinite(minor)) {
    throw new Error(`Unable to parse Python version '${detectedVersion}'`);
  }

  const numpyRequirement = major > 3 || (major === 3 && minor >= 13) ? "numpy>=2,<3" : "numpy<2";

  process.stdout.write("Installing faster-whisper worker dependencies...\n");
  process.stdout.write(
    `Detected Python ${detectedVersion}; using ${numpyRequirement} and PyInstaller>=6.15,<7\n`,
  );
  runCommand(venvPython, ["-m", "pip", "install", "--upgrade", "pip"]);
  runCommand(venvPython, [
    "-m",
    "pip",
    "install",
    numpyRequirement,
    "faster-whisper>=1.0.3",
    "requests",
    "huggingface-hub",
    "pyinstaller>=6.15,<7",
  ]);
}

async function buildWorkerBinary(venvPython) {
  await ensureDir(outputDir);
  await ensureDir(pyInstallerWorkDir);
  await ensureDir(pyInstallerSpecDir);

  const outputBaseName = platform === "win32" ? "faster-whisper-worker" : "faster-whisper-worker";

  const args = [
    "-m",
    "PyInstaller",
    "--onefile",
    "--clean",
    "--noconfirm",
    "--name",
    outputBaseName,
    "--distpath",
    outputDir,
    "--workpath",
    pyInstallerWorkDir,
    "--specpath",
    pyInstallerSpecDir,
    "--collect-all",
    "faster_whisper",
    "--collect-all",
    "ctranslate2",
    "--collect-all",
    "tokenizers",
    "--collect-all",
    "requests",
    "--collect-all",
    "huggingface_hub",
    "--collect-all",
    "numpy",
    "--collect-all",
    "av",
    "--hidden-import",
    "numpy._core._exceptions",
    "--hidden-import",
    "requests",
    workerSource,
  ];

  process.stdout.write("Building faster-whisper worker executable...\n");
  runCommand(venvPython, args);
}

function smokeTestWorkerExecutable() {
  const executablePath = path.join(outputDir, executableName);
  if (!existsSync(executablePath)) {
    throw new Error(`Built worker executable not found at ${executablePath}`);
  }

  const probe = spawnSync(executablePath, ["--stdio"], {
    input: '{"op":"ping","id":"smoke-test"}\n',
    encoding: "utf8",
    env: {
      ...process.env,
      SONORA_FASTER_WHISPER_MODEL_CACHE: path.join(projectRoot, "src-tauri", "resources", "models", "faster-whisper-cache"),
    },
  });

  if (probe.error) {
    throw new Error(`Failed to launch built faster-whisper worker: ${probe.error.message}`);
  }
  if (probe.status !== 0) {
    throw new Error(
      `Built faster-whisper worker failed smoke test (exit ${probe.status}): ${probe.stderr || "no stderr"}`,
    );
  }

  const firstLine = (probe.stdout || "")
    .split(/\r?\n/)
    .map((line) => line.trim())
    .find((line) => line.length > 0);

  if (!firstLine) {
    throw new Error("Built faster-whisper worker smoke test produced no output");
  }

  let parsed;
  try {
    parsed = JSON.parse(firstLine);
  } catch (error) {
    throw new Error(`Built faster-whisper worker smoke output is not JSON: ${firstLine}`);
  }

  if (parsed.id !== "smoke-test") {
    throw new Error("Built faster-whisper worker smoke response did not match request id");
  }
  process.stdout.write("faster-whisper worker smoke test passed.\n");
}

async function writeMetadata() {
  const metadataPath = path.join(outputDir, metadataFileName);
  const payload = {
    engine: "faster_whisper",
    executable: executableName,
    generated_at: new Date().toISOString(),
    platform,
  };

  await fs.writeFile(metadataPath, `${JSON.stringify(payload, null, 2)}\n`, "utf8");
  process.stdout.write(`Wrote sidecar metadata to ${metadataPath}\n`);
}

async function main() {
  const options = parseArgs(process.argv);
  if (options.help) {
    printHelp();
    return;
  }

  if (!existsSync(workerSource)) {
    throw new Error(`Worker source file not found: ${workerSource}`);
  }

  const pythonCommand = resolvePythonCommand(options.pythonCommand);
  await ensureDir(cacheRoot);
  await ensureVenv(pythonCommand, options.force);

  const venvPython = venvPythonPath();
  if (!existsSync(venvPython)) {
    throw new Error(`Virtualenv python not found at ${venvPython}`);
  }

  await installDependencies(venvPython);
  await buildWorkerBinary(venvPython);
  smokeTestWorkerExecutable();
  await writeMetadata();
}

main().catch((error) => {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exitCode = 1;
});
