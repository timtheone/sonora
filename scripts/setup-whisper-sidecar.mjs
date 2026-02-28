import { existsSync } from "node:fs";
import { promises as fs } from "node:fs";
import path from "node:path";
import process from "node:process";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const REPO_URL = "https://github.com/ggml-org/whisper.cpp.git";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(__dirname, "..");
const cacheRoot = path.join(projectRoot, ".cache", "whisper-sidecar");
const sourceDir = path.join(cacheRoot, "whisper.cpp");
const buildDir = path.join(cacheRoot, "build");
const outputDir = path.join(projectRoot, "src-tauri", "resources", "bin");

const platform = process.platform;
const executableName = platform === "win32" ? "whisper-cli.exe" : "whisper-cli";
const metadataFileName = "whisper-sidecar.json";

function missingCommandHelp(command) {
  const base = `Missing required command: '${command}'.`;

  if (platform === "linux") {
    if (command === "cmake") {
      return `${base}\nInstall it with: sudo apt-get update && sudo apt-get install -y cmake build-essential`;
    }
    if (command === "git") {
      return `${base}\nInstall it with: sudo apt-get update && sudo apt-get install -y git`;
    }
    return `${base}\nInstall required build tools with: sudo apt-get update && sudo apt-get install -y git cmake build-essential`;
  }

  if (platform === "darwin") {
    return `${base}\nInstall prerequisites with: brew install git cmake && xcode-select --install`;
  }

  if (platform === "win32") {
    return `${base}\nInstall Git + CMake and Visual Studio C++ Build Tools, then reopen your shell.`;
  }

  return base;
}

function commandExists(command, args = ["--version"]) {
  const result = spawnSync(command, args, {
    stdio: "ignore",
  });
  if (result.error && result.error.code === "ENOENT") {
    return false;
  }
  return true;
}

function ensureRequirements() {
  const required = ["git", "cmake"];
  for (const command of required) {
    if (!commandExists(command)) {
      throw new Error(missingCommandHelp(command));
    }
  }
}

function hasCudaToolchain() {
  const result = spawnSync("nvcc", ["--version"], {
    stdio: "ignore",
  });
  if (result.error && result.error.code === "ENOENT") {
    return false;
  }
  return result.status === 0;
}

function hasNvidiaDriver() {
  const result = spawnSync("nvidia-smi", ["-L"], {
    encoding: "utf8",
  });
  if (result.error && result.error.code === "ENOENT") {
    return false;
  }
  return result.status === 0 && Boolean(result.stdout?.trim());
}

function parseBackend(value) {
  const normalized = (value ?? "").trim().toLowerCase();
  if (normalized === "") {
    return null;
  }
  if (normalized === "auto" || normalized === "cpu" || normalized === "cuda") {
    return normalized;
  }
  throw new Error(`Unsupported backend '${value}'. Use auto, cpu, or cuda.`);
}

function resolveBuildBackend(requested) {
  if (requested === "cpu") {
    return "cpu";
  }

  if (requested === "cuda") {
    if (!hasCudaToolchain()) {
      throw new Error(
        "CUDA backend requested but 'nvcc' was not found. Install NVIDIA CUDA Toolkit and retry.",
      );
    }
    return "cuda";
  }

  if (hasCudaToolchain() && hasNvidiaDriver()) {
    return "cuda";
  }

  return "cpu";
}

function parseArgs(argv) {
  const args = argv.slice(2);
  if (args.includes("--help") || args.includes("-h")) {
    return { help: true, forceClone: false, requestedBackend: "auto" };
  }

  let requestedBackend = parseBackend(process.env.SONORA_WHISPER_BACKEND) ?? "auto";
  for (let i = 0; i < args.length; i += 1) {
    const arg = args[i];
    if (arg === "--backend") {
      const value = args[i + 1];
      if (!value) {
        throw new Error("Missing value for --backend. Use: --backend auto|cpu|cuda");
      }
      requestedBackend = parseBackend(value) ?? "auto";
      i += 1;
      continue;
    }

    if (arg.startsWith("--backend=")) {
      requestedBackend = parseBackend(arg.slice("--backend=".length)) ?? "auto";
      continue;
    }

    if (arg === "--cuda") {
      requestedBackend = "cuda";
      continue;
    }

    if (arg === "--cpu") {
      requestedBackend = "cpu";
      continue;
    }
  }

  return {
    help: false,
    forceClone: args.includes("--force-clone"),
    requestedBackend,
  };
}

function printHelp() {
  process.stdout.write(
    [
      "Download and build whisper.cpp sidecar binary for current OS.",
      "",
      "Usage:",
      "  pnpm sidecar:setup [--force-clone] [--backend auto|cpu|cuda]",
      "",
      "Options:",
      "  --force-clone   Delete cached source and clone fresh",
      "  --backend       Backend to build (default: auto)",
      "  --cuda          Shortcut for --backend cuda",
      "  --cpu           Shortcut for --backend cpu",
      "",
      "Output:",
      `  src-tauri/resources/bin/${executableName}`,
      `  src-tauri/resources/bin/${metadataFileName}`,
      "",
      "Requirements:",
      "  - git",
      "  - cmake",
      "  - C/C++ compiler toolchain for your OS",
      "  - NVIDIA CUDA Toolkit (for --backend cuda)",
      "",
    ].join("\n"),
  );
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

async function ensureDir(dir) {
  await fs.mkdir(dir, { recursive: true });
}

async function removeDir(dir) {
  if (existsSync(dir)) {
    await fs.rm(dir, { recursive: true, force: true });
  }
}

async function prepareSource(forceClone) {
  await ensureDir(cacheRoot);

  if (forceClone) {
    await removeDir(sourceDir);
  }

  if (!existsSync(sourceDir)) {
    process.stdout.write("Cloning whisper.cpp...\n");
    runCommand("git", ["clone", "--depth", "1", REPO_URL, sourceDir]);
    return;
  }

  process.stdout.write("Updating whisper.cpp...\n");
  runCommand("git", ["-C", sourceDir, "fetch", "--depth", "1", "origin"]);
  runCommand("git", ["-C", sourceDir, "pull", "--ff-only"]);
}

function resolveBuiltExecutable() {
  const candidates = [
    path.join(buildDir, "bin", executableName),
    path.join(buildDir, "bin", "Release", executableName),
    path.join(buildDir, "src", executableName),
    path.join(buildDir, "src", "Release", executableName),
    path.join(buildDir, executableName),
  ];

  return candidates.find((candidate) => existsSync(candidate));
}

async function resolveBuiltRuntimeLibraries() {
  const candidateDirs = [
    path.join(buildDir, "bin"),
    path.join(buildDir, "bin", "Release"),
    path.join(buildDir, "src"),
    path.join(buildDir, "src", "Release"),
  ];

  const patterns = platform === "win32" ? [".dll"] : platform === "darwin" ? [".dylib"] : [".so"];
  const files = [];

  for (const dir of candidateDirs) {
    if (!existsSync(dir)) {
      continue;
    }

    const entries = await fs.readdir(dir, { withFileTypes: true });
    for (const entry of entries) {
      if (!entry.isFile()) {
        continue;
      }
      if (!patterns.some((suffix) => entry.name.endsWith(suffix))) {
        continue;
      }
      if (!(entry.name.includes("whisper") || entry.name.includes("ggml"))) {
        continue;
      }
      files.push(path.join(dir, entry.name));
    }
  }

  return files;
}

async function copyExecutable(binaryPath) {
  await ensureDir(outputDir);
  const destination = path.join(outputDir, executableName);

  await fs.copyFile(binaryPath, destination);
  if (platform !== "win32") {
    await fs.chmod(destination, 0o755);
  }

  process.stdout.write(`Copied sidecar binary to ${destination}\n`);
}

async function copyRuntimeLibraries() {
  const libraries = await resolveBuiltRuntimeLibraries();
  if (libraries.length === 0) {
    return;
  }

  await ensureDir(outputDir);
  for (const library of libraries) {
    const destination = path.join(outputDir, path.basename(library));
    await fs.copyFile(library, destination);
    process.stdout.write(`Copied runtime library to ${destination}\n`);
  }
}

async function writeBackendMetadata(backend) {
  await ensureDir(outputDir);
  const destination = path.join(outputDir, metadataFileName);
  const payload = {
    backend,
    platform,
    generated_at: new Date().toISOString(),
  };
  await fs.writeFile(destination, `${JSON.stringify(payload, null, 2)}\n`, "utf8");
  process.stdout.write(`Wrote sidecar metadata to ${destination}\n`);
}

async function main() {
  const options = parseArgs(process.argv);
  if (options.help) {
    printHelp();
    return;
  }

  ensureRequirements();

  await prepareSource(options.forceClone);

  const backend = resolveBuildBackend(options.requestedBackend);
  process.stdout.write(
    `Selected backend: ${backend} (requested: ${options.requestedBackend})\n`,
  );

  process.stdout.write("Configuring whisper.cpp build...\n");
  const cmakeArgs = [
    "-S",
    sourceDir,
    "-B",
    buildDir,
    "-DCMAKE_BUILD_TYPE=Release",
    "-DBUILD_SHARED_LIBS=OFF",
    "-DWHISPER_BUILD_EXAMPLES=ON",
    "-DWHISPER_BUILD_TESTS=OFF",
    "-DWHISPER_BUILD_SERVER=OFF",
    "-DGGML_BACKEND_DL=OFF",
  ];

  if (backend === "cuda") {
    cmakeArgs.push("-DGGML_CUDA=ON");
  }

  runCommand("cmake", cmakeArgs);

  process.stdout.write("Building whisper.cpp sidecar...\n");
  runCommand("cmake", ["--build", buildDir, "--config", "Release"]);

  const built = resolveBuiltExecutable();
  if (!built) {
    throw new Error(
      `Could not locate built executable '${executableName}' under ${buildDir}`,
    );
  }

  await copyExecutable(built);
  await copyRuntimeLibraries();
  await writeBackendMetadata(backend);
}

main().catch((error) => {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exitCode = 1;
});
