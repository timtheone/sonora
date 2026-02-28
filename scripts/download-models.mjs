import { createWriteStream } from "node:fs";
import { promises as fs } from "node:fs";
import { pipeline } from "node:stream/promises";
import { Readable } from "node:stream";
import path from "node:path";
import process from "node:process";
import { spawn, spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const WHISPER_MODELS = {
  balanced: {
    fileName: "ggml-base.en-q5_1.bin",
    url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en-q5_1.bin",
  },
  fast: {
    fileName: "ggml-tiny.en-q8_0.bin",
    url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.en-q8_0.bin",
  },
  base_q8: {
    fileName: "ggml-base.en-q8_0.bin",
    url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en-q8_0.bin",
  },
  small_q8: {
    fileName: "ggml-small.en-q8_0.bin",
    url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.en-q8_0.bin",
  },
  large_v3_turbo_q8: {
    fileName: "ggml-large-v3-turbo-q8_0.bin",
    url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q8_0.bin",
  },
};

const WHISPER_PROFILES = {
  default: ["balanced", "fast"],
  all: ["balanced", "fast", "base_q8", "small_q8", "large_v3_turbo_q8"],
  fast: ["fast"],
  balanced: ["balanced"],
  q8: ["base_q8", "small_q8"],
  quality: ["large_v3_turbo_q8"],
};

const FASTER_DEFAULT_MODELS = ["small.en", "distil-large-v3", "large-v3"];
const ENGINE_VALUES = ["whisper_cpp", "faster_whisper", "all"];

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(__dirname, "..");
const modelsDir = path.join(projectRoot, "src-tauri", "resources", "models");
const fasterCacheDir = path.join(modelsDir, "faster-whisper-cache");
const fasterWorkerPath = path.join(
  projectRoot,
  "src-tauri",
  "resources",
  "bin",
  process.platform === "win32" ? "faster-whisper-worker.exe" : "faster-whisper-worker",
);

function parseArgs(argv) {
  const values = argv.slice(2);
  if (values.includes("--help") || values.includes("-h")) {
    return {
      help: true,
      whisperSelection: "all",
      engine: "whisper_cpp",
      force: false,
      fasterModels: FASTER_DEFAULT_MODELS,
      device: "auto",
      computeType: "auto",
    };
  }

  const options = {
    help: false,
    whisperSelection: "all",
    engine: "whisper_cpp",
    force: false,
    fasterModels: FASTER_DEFAULT_MODELS,
    device: "auto",
    computeType: "auto",
  };

  let sawSelection = false;
  for (let i = 0; i < values.length; i += 1) {
    const value = values[i];
    if (value === "--force") {
      options.force = true;
      continue;
    }
    if (value === "--engine") {
      options.engine = values[i + 1] ?? options.engine;
      i += 1;
      continue;
    }
    if (value.startsWith("--engine=")) {
      options.engine = value.slice("--engine=".length);
      continue;
    }
    if (value === "--faster-models") {
      const raw = values[i + 1] ?? "";
      options.fasterModels = raw
        .split(",")
        .map((item) => item.trim())
        .filter(Boolean);
      i += 1;
      continue;
    }
    if (value.startsWith("--faster-models=")) {
      options.fasterModels = value
        .slice("--faster-models=".length)
        .split(",")
        .map((item) => item.trim())
        .filter(Boolean);
      continue;
    }
    if (value === "--device") {
      options.device = values[i + 1] ?? options.device;
      i += 1;
      continue;
    }
    if (value.startsWith("--device=")) {
      options.device = value.slice("--device=".length);
      continue;
    }
    if (value === "--compute-type") {
      options.computeType = values[i + 1] ?? options.computeType;
      i += 1;
      continue;
    }
    if (value.startsWith("--compute-type=")) {
      options.computeType = value.slice("--compute-type=".length);
      continue;
    }
    if (!value.startsWith("--") && !sawSelection) {
      options.whisperSelection = value;
      sawSelection = true;
      continue;
    }
    if (!value.startsWith("--")) {
      throw new Error(`Unexpected extra argument: '${value}'`);
    }
  }

  if (!Object.hasOwn(WHISPER_PROFILES, options.whisperSelection)) {
    throw new Error(
      `Invalid whisper profile '${options.whisperSelection}'. Use one of: ${Object.keys(WHISPER_PROFILES).join(", ")}.`,
    );
  }
  if (!ENGINE_VALUES.includes(options.engine)) {
    throw new Error(
      `Invalid engine '${options.engine}'. Use one of: ${ENGINE_VALUES.join(", ")}.`,
    );
  }
  if (options.fasterModels.length === 0) {
    options.fasterModels = FASTER_DEFAULT_MODELS;
  }

  return options;
}

function printHelp() {
  process.stdout.write(
    [
      "Download model bundles for whisper.cpp and/or faster-whisper.",
      "",
      "Usage:",
      "  pnpm model:download [default|all|balanced|fast|q8|quality] [--engine whisper_cpp|faster_whisper|all] [--force]",
      "",
      "Options:",
      "  --engine          Engine(s) to sync (default: whisper_cpp)",
      "  --force           Re-download whisper.cpp files even if present",
      "  --faster-models   Comma-separated faster-whisper model ids/paths",
      "  --device          faster-whisper device: auto|cpu|cuda",
      "  --compute-type    faster-whisper compute: auto|int8|float16|float32",
      "",
      "Whisper profiles:",
      "  default -> balanced + fast",
      "  q8      -> base_q8 + small_q8",
      "  quality -> large_v3_turbo_q8",
      "  all     -> default + q8 + quality",
      "  (default selection is: all)",
      "",
      "faster-whisper defaults:",
      `  ${FASTER_DEFAULT_MODELS.join(" ")}`,
      "",
      "Examples:",
      "  pnpm model:download",
      "  pnpm model:download default",
      "  pnpm model:download q8",
      "  pnpm model:download all --engine whisper_cpp",
      "  pnpm model:download -- --engine faster_whisper",
      "  pnpm model:download -- all --engine all",
      "  pnpm model:download balanced",
      "  pnpm model:download fast --force",
      "",
    ].join("\n"),
  );
}

async function ensureDir(target) {
  await fs.mkdir(target, { recursive: true });
}

async function pathExists(target) {
  try {
    await fs.access(target);
    return true;
  } catch {
    return false;
  }
}

async function downloadModel(key, force) {
  const model = WHISPER_MODELS[key];
  const destination = path.join(modelsDir, model.fileName);

  if (!force && (await pathExists(destination))) {
    process.stdout.write(`Skipping ${model.fileName} (already exists).\n`);
    return;
  }

  process.stdout.write(`Downloading ${model.fileName}...\n`);
  const response = await fetch(model.url);
  if (!response.ok || !response.body) {
    throw new Error(`Failed to download ${model.fileName}: HTTP ${response.status}`);
  }

  const temporary = `${destination}.download`;

  try {
    await pipeline(Readable.fromWeb(response.body), createWriteStream(temporary));
    await fs.rename(temporary, destination);

    const stat = await fs.stat(destination);
    const sizeMb = (stat.size / (1024 * 1024)).toFixed(1);
    process.stdout.write(`Saved ${model.fileName} (${sizeMb} MB).\n`);
  } catch (error) {
    await fs.rm(temporary, { force: true });
    throw error;
  }
}

async function main() {
  const options = parseArgs(process.argv);

  if (options.help) {
    printHelp();
    return;
  }

  await ensureDir(modelsDir);

  if (options.engine === "whisper_cpp" || options.engine === "all") {
    const targets = WHISPER_PROFILES[options.whisperSelection];
    for (const target of targets) {
      await downloadModel(target, options.force);
    }
  }

  if (options.engine === "faster_whisper" || options.engine === "all") {
    await prefetchFasterWhisperModels(
      options.fasterModels,
      options.device,
      options.computeType,
    );
  }
}

function hasNvidiaGpu() {
  const result = spawnSync("nvidia-smi", ["-L"], { stdio: "ignore" });
  return result.status === 0;
}

function resolveDevice(device) {
  const normalized = device.trim().toLowerCase();
  if (normalized === "cpu" || normalized === "cuda") {
    return normalized;
  }
  if (normalized !== "auto") {
    throw new Error("Invalid --device value. Use auto, cpu, or cuda.");
  }
  return hasNvidiaGpu() ? "cuda" : "cpu";
}

function resolveComputeType(computeType, device) {
  const normalized = computeType.trim().toLowerCase();
  if (normalized === "auto") {
    return device === "cuda" ? "float16" : "int8";
  }
  if (["int8", "float16", "float32"].includes(normalized)) {
    return normalized;
  }
  throw new Error("Invalid --compute-type value. Use auto, int8, float16, or float32.");
}

function sendJsonLine(stdin, payload) {
  return new Promise((resolve, reject) => {
    stdin.write(`${JSON.stringify(payload)}\n`, (error) => {
      if (error) {
        reject(error);
        return;
      }
      resolve();
    });
  });
}

async function prefetchFasterWhisperModels(models, deviceArg, computeTypeArg) {
  try {
    await fs.access(fasterWorkerPath);
  } catch {
    throw new Error(
      `faster-whisper worker not found at ${fasterWorkerPath}. Run: pnpm sidecar:setup:faster-whisper`,
    );
  }

  await ensureDir(fasterCacheDir);
  const device = resolveDevice(deviceArg);
  const computeType = resolveComputeType(computeTypeArg, device);
  process.stdout.write(
    `Prefetching faster-whisper models [${models.join(", ")}] with device=${device}, compute_type=${computeType}\n`,
  );

  const child = spawn(fasterWorkerPath, ["--stdio"], {
    stdio: ["pipe", "pipe", "inherit"],
    env: {
      ...process.env,
      SONORA_FASTER_WHISPER_MODEL_CACHE: fasterCacheDir,
    },
  });

  if (!child.stdin || !child.stdout) {
    throw new Error("Failed to initialize faster-whisper worker stdio streams.");
  }

  child.stdout.setEncoding("utf8");
  let buffer = "";
  const responses = [];
  child.stdout.on("data", (chunk) => {
    buffer += chunk;
    const lines = buffer.split(/\r?\n/);
    buffer = lines.pop() ?? "";
    for (const line of lines) {
      if (!line.trim()) {
        continue;
      }
      try {
        responses.push(JSON.parse(line));
      } catch {
        // ignore malformed worker lines
      }
    }
  });

  for (const model of models) {
    const requestId = `prefetch-${model}`;
    await sendJsonLine(child.stdin, {
      op: "preload",
      id: requestId,
      model,
      device,
      compute_type: computeType,
    });

    const startedAt = Date.now();
    let matched = null;
    while (!matched) {
      if (Date.now() - startedAt > 300_000) {
        throw new Error(`Timed out while prefetching '${model}'.`);
      }
      await new Promise((resolve) => setTimeout(resolve, 100));
      matched = responses.find((entry) => entry.id === requestId);
    }

    if (!matched.ok) {
      throw new Error(`Failed to prefetch '${model}': ${matched.error ?? "unknown error"}`);
    }
    process.stdout.write(`Prefetched ${model} (${matched.load_ms} ms)\n`);
  }

  child.stdin.end();
  child.kill();
  process.stdout.write(`Model cache ready: ${fasterCacheDir}\n`);
}

main().catch((error) => {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exitCode = 1;
});
