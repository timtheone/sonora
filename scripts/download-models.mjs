import { createWriteStream } from "node:fs";
import { promises as fs } from "node:fs";
import { pipeline } from "node:stream/promises";
import { Readable } from "node:stream";
import path from "node:path";
import { fileURLToPath } from "node:url";

const MODELS = {
  balanced: {
    fileName: "ggml-base.en-q5_1.bin",
    url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en-q5_1.bin",
  },
  fast: {
    fileName: "ggml-tiny.en-q8_0.bin",
    url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.en-q8_0.bin",
  },
};

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(__dirname, "..");
const modelsDir = path.join(projectRoot, "src-tauri", "resources", "models");

function parseArgs(argv) {
  const values = argv.slice(2);
  if (values.includes("--help") || values.includes("-h")) {
    return { help: true, selection: "all", force: false };
  }

  const force = values.includes("--force");
  const profile = values.find((value) => !value.startsWith("--")) ?? "all";

  if (!["all", "balanced", "fast"].includes(profile)) {
    throw new Error(
      `Invalid profile '${profile}'. Use one of: all, balanced, fast.`,
    );
  }

  return { help: false, selection: profile, force };
}

function printHelp() {
  process.stdout.write(
    [
      "Download default whisper.cpp model files for Sonora.",
      "",
      "Usage:",
      "  pnpm model:download [all|balanced|fast] [--force]",
      "",
      "Examples:",
      "  pnpm model:download",
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
  const model = MODELS[key];
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

  const targets =
    options.selection === "all"
      ? ["balanced", "fast"]
      : [options.selection];

  for (const target of targets) {
    await downloadModel(target, options.force);
  }
}

main().catch((error) => {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exitCode = 1;
});
