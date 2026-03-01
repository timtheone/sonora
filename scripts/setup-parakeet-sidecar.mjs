import { existsSync } from "node:fs";
import { promises as fs } from "node:fs";
import path from "node:path";
import process from "node:process";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(__dirname, "..");
const cacheRoot = path.join(projectRoot, ".cache", "parakeet-sidecar");
const venvDir = path.join(cacheRoot, "venv");
const pyInstallerWorkDir = path.join(cacheRoot, "pyinstaller-work");
const pyInstallerSpecDir = path.join(cacheRoot, "pyinstaller-spec");
const outputDir = path.join(projectRoot, "src-tauri", "resources", "bin");
const workerSource = path.join(projectRoot, "src-tauri", "resources", "parakeet", "worker.py");

const platform = process.platform;
const executableName = platform === "win32" ? "parakeet-worker.exe" : "parakeet-worker";
const metadataFileName = "parakeet-sidecar.json";
const DEFAULT_TORCH_CUDA_CHANNEL = "cu124";

function hasNvidiaDriver() {
  const result = spawnSync("nvidia-smi", ["-L"], { encoding: "utf8" });
  if (result.error && result.error.code === "ENOENT") {
    return false;
  }
  return result.status === 0 && Boolean(result.stdout?.trim());
}

function parseBackend(value) {
  const normalized = (value ?? "").trim().toLowerCase();
  if (!normalized) {
    return null;
  }
  if (normalized === "auto" || normalized === "cpu" || normalized === "cuda") {
    return normalized;
  }
  throw new Error(`Unsupported backend '${value}'. Use auto, cpu, or cuda.`);
}

function resolveTorchChannel(value) {
  const normalized = (value ?? "").trim().toLowerCase();
  if (!normalized) {
    return DEFAULT_TORCH_CUDA_CHANNEL;
  }
  if (["cu118", "cu121", "cu124", "cu126"].includes(normalized)) {
    return normalized;
  }
  throw new Error(`Unsupported torch CUDA channel '${value}'. Use cu118, cu121, cu124, or cu126.`);
}

function resolveBuildBackend(requestedBackend) {
  if (requestedBackend === "cpu") {
    return "cpu";
  }
  if (requestedBackend === "cuda") {
    return "cuda";
  }
  return hasNvidiaDriver() ? "cuda" : "cpu";
}

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
  const raw = argv.slice(2);
  const args = raw[0] === "--" ? raw.slice(1) : raw;
  if (args.includes("--help") || args.includes("-h")) {
    return {
      help: true,
      force: false,
      pythonCommand: null,
      requestedBackend: "auto",
      torchCudaChannel: DEFAULT_TORCH_CUDA_CHANNEL,
    };
  }

  let pythonCommand = null;
  let requestedBackend = parseBackend(process.env.SONORA_PARAKEET_BACKEND) ?? "auto";
  let torchCudaChannel =
    resolveTorchChannel(process.env.SONORA_PARAKEET_TORCH_CHANNEL ?? DEFAULT_TORCH_CUDA_CHANNEL);
  for (let i = 0; i < args.length; i += 1) {
    const arg = args[i];
    if (arg === "--python") {
      pythonCommand = args[i + 1] ?? null;
      i += 1;
    } else if (arg.startsWith("--python=")) {
      pythonCommand = arg.slice("--python=".length);
    } else if (arg === "--backend") {
      requestedBackend = parseBackend(args[i + 1]) ?? "auto";
      i += 1;
    } else if (arg.startsWith("--backend=")) {
      requestedBackend = parseBackend(arg.slice("--backend=".length)) ?? "auto";
    } else if (arg === "--cuda") {
      requestedBackend = "cuda";
    } else if (arg === "--cpu") {
      requestedBackend = "cpu";
    } else if (arg === "--torch-cuda-channel") {
      torchCudaChannel = resolveTorchChannel(args[i + 1]);
      i += 1;
    } else if (arg.startsWith("--torch-cuda-channel=")) {
      torchCudaChannel = resolveTorchChannel(arg.slice("--torch-cuda-channel=".length));
    }
  }

  return {
    help: false,
    force: args.includes("--force"),
    pythonCommand,
    requestedBackend,
    torchCudaChannel,
  };
}

function printHelp() {
  process.stdout.write(
    [
      "Build the parakeet worker sidecar for current OS.",
      "",
      "Usage:",
      "  pnpm sidecar:setup:parakeet [--force] [--backend auto|cpu|cuda] [--torch-cuda-channel cu124] [--python <command>]",
      "",
      "Options:",
      "  --force            Recreate Python virtual environment",
      "  --python <cmd>     Python command to use (default: auto-detect)",
      "  --backend          Build runtime target (default: auto)",
      "  --cuda             Shortcut for --backend cuda",
      "  --cpu              Shortcut for --backend cpu",
      "  --torch-cuda-channel  CUDA wheel channel (cu118|cu121|cu124|cu126)",
      "",
      "Output:",
      `  src-tauri/resources/bin/${executableName}`,
      `  src-tauri/resources/bin/${metadataFileName}`,
      "",
      "Requirements:",
      "  - Python 3.10+ with venv support",
      "  - Internet access during build for Python dependencies",
      "  - NVIDIA driver for CUDA runtime checks (when backend=cuda)",
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

async function installDependencies(venvPython, backend, torchCudaChannel) {
  const versionResult = spawnSync(
    venvPython,
    ["-c", "import sys; print(f'{sys.version_info.major}.{sys.version_info.minor}')"],
    { encoding: "utf8" },
  );
  if (versionResult.status !== 0) {
    throw new Error(
      `Failed to detect Python version for parakeet worker: ${versionResult.stderr || "unknown error"}`,
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

  process.stdout.write("Installing parakeet worker dependencies...\n");
  process.stdout.write(
    `Detected Python ${detectedVersion}; using ${numpyRequirement} and PyInstaller>=6.15,<7\n`,
  );
  runCommand(venvPython, ["-m", "pip", "install", "--upgrade", "pip"]);
  runCommand(venvPython, [
    "-m",
    "pip",
    "install",
    numpyRequirement,
    "torch>=2.4",
    "transformers>=4.44",
    "librosa>=0.10.2",
    "soundfile>=0.12.1",
    "scipy>=1.11",
    "sentencepiece",
    "safetensors",
    "pyinstaller>=6.15,<7",
  ]);

  if (backend === "cuda") {
    const cudaIndexUrl = `https://download.pytorch.org/whl/${torchCudaChannel}`;
    process.stdout.write(
      `Installing CUDA-enabled torch from ${cudaIndexUrl} (force-reinstall)...\n`,
    );
    runCommand(venvPython, [
      "-m",
      "pip",
      "install",
      "--upgrade",
      "--force-reinstall",
      "--index-url",
      cudaIndexUrl,
      "torch",
    ]);
  } else {
    process.stdout.write("Installing CPU torch runtime (force-reinstall)...\n");
    runCommand(venvPython, [
      "-m",
      "pip",
      "install",
      "--upgrade",
      "--force-reinstall",
      "torch>=2.4",
    ]);
  }
}

async function buildWorkerBinary(venvPython) {
  await ensureDir(outputDir);
  await ensureDir(pyInstallerWorkDir);
  await ensureDir(pyInstallerSpecDir);

  const outputBaseName = platform === "win32" ? "parakeet-worker" : "parakeet-worker";

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
    "transformers",
    "--collect-all",
    "tokenizers",
    "--collect-all",
    "torch",
    "--collect-all",
    "numpy",
    "--collect-all",
    "librosa",
    "--collect-all",
    "soundfile",
    "--collect-all",
    "scipy",
    "--collect-all",
    "numba",
    "--collect-all",
    "llvmlite",
    "--collect-all",
    "safetensors",
    "--collect-all",
    "sentencepiece",
    workerSource,
  ];

  process.stdout.write("Building parakeet worker executable...\n");
  runCommand(venvPython, args);
}

function smokeTestWorkerExecutable(expectedBackend) {
  const executablePath = path.join(outputDir, executableName);
  if (!existsSync(executablePath)) {
    throw new Error(`Built worker executable not found at ${executablePath}`);
  }

  const probe = spawnSync(executablePath, ["--stdio"], {
    input: '{"op":"ping","id":"smoke-test"}\n',
    encoding: "utf8",
    env: {
      ...process.env,
      SONORA_PARAKEET_MODEL_CACHE: path.join(projectRoot, "src-tauri", "resources", "models", "parakeet-cache"),
    },
  });

  if (probe.error) {
    throw new Error(`Failed to launch built parakeet worker: ${probe.error.message}`);
  }
  if (probe.status !== 0) {
    throw new Error(
      `Built parakeet worker failed smoke test (exit ${probe.status}): ${probe.stderr || "no stderr"}`,
    );
  }

  const firstLine = (probe.stdout || "")
    .split(/\r?\n/)
    .map((line) => line.trim())
    .find((line) => line.length > 0);

  if (!firstLine) {
    throw new Error("Built parakeet worker smoke test produced no output");
  }

  let parsed;
  try {
    parsed = JSON.parse(firstLine);
  } catch {
    throw new Error(`Built parakeet worker smoke output is not JSON: ${firstLine}`);
  }

  if (parsed.id !== "smoke-test") {
    throw new Error("Built parakeet worker smoke response did not match request id");
  }

  const cudaAvailable = Boolean(parsed.cuda_available);
  if (expectedBackend === "cuda" && !cudaAvailable) {
    throw new Error(
      "Parakeet worker smoke test reports cuda_available=false. Rebuild with --force --backend cuda and ensure CUDA-enabled torch installs correctly.",
    );
  }

  process.stdout.write(
    `parakeet worker runtime: torch=${parsed.torch_version ?? "unknown"}, torch_cuda=${parsed.torch_cuda_version ?? "unknown"}, cuda_available=${cudaAvailable}\n`,
  );
  process.stdout.write("parakeet worker smoke test passed.\n");

  return {
    cudaAvailable,
    torchVersion: parsed.torch_version ?? null,
    torchCudaVersion: parsed.torch_cuda_version ?? null,
  };
}

async function writeMetadata(metadata) {
  const metadataPath = path.join(outputDir, metadataFileName);
  const payload = {
    engine: "parakeet",
    executable: executableName,
    generated_at: new Date().toISOString(),
    platform,
    ...metadata,
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
  const resolvedBackend = resolveBuildBackend(options.requestedBackend);
  const torchCudaChannel = resolveTorchChannel(options.torchCudaChannel);

  process.stdout.write(
    `Resolved parakeet backend: requested=${options.requestedBackend}, resolved=${resolvedBackend}\n`,
  );
  if (resolvedBackend === "cuda") {
    process.stdout.write(`Torch CUDA channel: ${torchCudaChannel}\n`);
  }

  await ensureDir(cacheRoot);
  await ensureVenv(pythonCommand, options.force);

  const venvPython = venvPythonPath();
  if (!existsSync(venvPython)) {
    throw new Error(`Virtualenv python not found at ${venvPython}`);
  }

  await installDependencies(venvPython, resolvedBackend, torchCudaChannel);
  await buildWorkerBinary(venvPython);
  const runtimeInfo = smokeTestWorkerExecutable(resolvedBackend);
  await writeMetadata({
    requested_backend: options.requestedBackend,
    resolved_backend: resolvedBackend,
    torch_cuda_channel: resolvedBackend === "cuda" ? torchCudaChannel : null,
    torch_version: runtimeInfo.torchVersion,
    torch_cuda_version: runtimeInfo.torchCudaVersion,
    cuda_available: runtimeInfo.cudaAvailable,
  });
}

main().catch((error) => {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exitCode = 1;
});
