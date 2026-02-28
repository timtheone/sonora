import os from "node:os";
import path from "node:path";
import process from "node:process";
import { existsSync } from "node:fs";
import { promises as fs } from "node:fs";

function defaultRuntimeLogPath() {
  if (process.platform === "win32") {
    const appData = process.env.APPDATA || path.join(os.homedir(), "AppData", "Roaming");
    return path.join(appData, "sonora-dictation", "runtime.log");
  }
  if (process.platform === "darwin") {
    return path.join(os.homedir(), "Library", "Application Support", "sonora-dictation", "runtime.log");
  }

  const xdg = process.env.XDG_CONFIG_HOME || path.join(os.homedir(), ".config");
  return path.join(xdg, "sonora-dictation", "runtime.log");
}

function parseArgs(argv) {
  const args = argv.slice(2);
  let logPath = defaultRuntimeLogPath();

  for (let i = 0; i < args.length; i += 1) {
    const arg = args[i];
    if (arg === "--path") {
      const value = args[i + 1];
      if (!value) {
        throw new Error("Missing value for --path");
      }
      logPath = path.resolve(value);
      i += 1;
      continue;
    }
    if (arg.startsWith("--path=")) {
      logPath = path.resolve(arg.slice("--path=".length));
      continue;
    }
  }

  return { logPath };
}

function percentile(values, p) {
  if (values.length === 0) {
    return null;
  }
  const sorted = [...values].sort((a, b) => a - b);
  const index = Math.min(sorted.length - 1, Math.max(0, Math.ceil((p / 100) * sorted.length) - 1));
  return sorted[index];
}

function pad(value, width, align = "right") {
  const text = String(value);
  if (text.length >= width) {
    return text;
  }
  const missing = " ".repeat(width - text.length);
  return align === "left" ? `${text}${missing}` : `${missing}${text}`;
}

function formatMs(value) {
  if (value === null || value === undefined) {
    return "-";
  }
  return String(value);
}

function formatRow(row, widths) {
  return [
    pad(row.chunk, widths.chunk),
    pad(row.engine, widths.engine),
    pad(row.audio, widths.audio),
    pad(row.collect, widths.collect),
    pad(row.downsample, widths.downsample),
    pad(row.vad, widths.vad),
    pad(row.infer, widths.infer),
    pad(row.emitRust, widths.emitRust),
    pad(row.emitUi, widths.emitUi),
    pad(row.total, widths.total),
    pad(row.text, widths.text),
  ].join(" | ");
}

function renderTable(state) {
  const rows = [...state.chunksById.values()]
    .sort((a, b) => b._seq - a._seq)
    .slice(0, 16)
    .map((chunk) => {
      const emitUi = state.uiByChunk.get(chunk.chunk_id) ?? null;
      const total = emitUi === null ? chunk.total_worker_ms : chunk.total_worker_ms + emitUi;
      return {
        chunk: chunk.chunk_id,
        engine: chunk.engine ?? "-",
        audio: formatMs(chunk.chunk_audio_ms),
        collect: formatMs(chunk.collect_ms),
        downsample: formatMs(chunk.downsample_ms),
        vad: formatMs(chunk.vad_ms),
        infer: formatMs(chunk.inference_ms),
        emitRust: formatMs(chunk.emit_rust_ms),
        emitUi: formatMs(emitUi),
        total: formatMs(total),
        text: chunk.transcript_len,
      };
    });

  const widths = {
    chunk: 6,
    engine: 14,
    audio: 8,
    collect: 9,
    downsample: 9,
    vad: 6,
    infer: 8,
    emitRust: 9,
    emitUi: 7,
    total: 8,
    text: 4,
  };

  const statsWindow = [...state.chunksById.values()]
    .sort((a, b) => b._seq - a._seq)
    .slice(0, 120)
    .filter((chunk) => chunk.inference_ms > 0 || chunk.emitted_transcript);

  const inferValues = statsWindow
    .map((chunk) => chunk.inference_ms)
    .filter((value) => value > 0);
  const totalValues = statsWindow.map((chunk) => {
    const emitUi = state.uiByChunk.get(chunk.chunk_id) ?? 0;
    return chunk.total_worker_ms + emitUi;
  });

  const p50Infer = percentile(inferValues, 50);
  const p95Infer = percentile(inferValues, 95);
  const p50Total = percentile(totalValues, 50);
  const p95Total = percentile(totalValues, 95);

  process.stdout.write("\x1Bc");
  process.stdout.write(`Sonora Perf Watch\n`);
  process.stdout.write(`Log: ${state.logPath}\n`);
  process.stdout.write(
    `Chunks: ${state.chunksById.size}  UI-ack: ${state.uiByChunk.size}  Updated: ${new Date().toLocaleTimeString()}\n\n`,
  );

  const header = formatRow(
    {
      chunk: "chunk",
      engine: "engine",
      audio: "audio_ms",
      collect: "collect",
      downsample: "downsmpl",
      vad: "vad",
      infer: "infer",
      emitRust: "emit_rs",
      emitUi: "emit_ui",
      total: "total",
      text: "txt",
    },
    widths,
  );

  process.stdout.write(`${header}\n`);
  process.stdout.write(`${"-".repeat(header.length)}\n`);
  if (rows.length === 0) {
    process.stdout.write("(waiting for perf.chunk events)\n");
  } else {
    for (const row of rows) {
      process.stdout.write(`${formatRow(row, widths)}\n`);
    }
  }

  process.stdout.write("\n");
  process.stdout.write(
    `p50 infer=${formatMs(p50Infer)}ms  p95 infer=${formatMs(p95Infer)}ms  p50 total=${formatMs(p50Total)}ms  p95 total=${formatMs(p95Total)}ms\n`,
  );
}

function parseLogLine(line) {
  if (!line.trim()) {
    return null;
  }

  try {
    return JSON.parse(line);
  } catch {
    return null;
  }
}

function handleEntry(entry, state) {
  if (!entry || typeof entry !== "object") {
    return false;
  }

  if (entry.event === "perf.chunk") {
    try {
      const parsed = JSON.parse(entry.message);
      if (typeof parsed.chunk_id !== "number") {
        return false;
      }
      state.seq += 1;
      parsed._seq = state.seq;
      state.chunksById.set(parsed.chunk_id, parsed);
      if (state.chunksById.size > 400) {
        const oldest = [...state.chunksById.values()]
          .sort((a, b) => a._seq - b._seq)
          .slice(0, state.chunksById.size - 400);
        for (const chunk of oldest) {
          state.chunksById.delete(chunk.chunk_id);
          state.uiByChunk.delete(chunk.chunk_id);
        }
      }
      return true;
    } catch {
      return false;
    }
  }

  if (entry.event === "perf.ui_transcript") {
    try {
      const parsed = JSON.parse(entry.message);
      if (typeof parsed.chunk_id !== "number" || typeof parsed.emit_to_ui_ms !== "number") {
        return false;
      }
      state.uiByChunk.set(parsed.chunk_id, parsed.emit_to_ui_ms);
      return true;
    } catch {
      return false;
    }
  }

  return false;
}

async function run() {
  const { logPath } = parseArgs(process.argv);
  const state = {
    logPath,
    offset: 0,
    tailRemainder: "",
    skipFirstPartialLine: false,
    seq: 0,
    chunksById: new Map(),
    uiByChunk: new Map(),
  };

  process.stdout.write(`Watching ${logPath}\n`);
  process.stdout.write("Start the app with: pnpm tauri:dev:profiling\n");

  if (existsSync(logPath)) {
    const stat = await fs.stat(logPath);
    const tailBytes = 512 * 1024;
    state.offset = Math.max(0, stat.size - tailBytes);
    state.skipFirstPartialLine = state.offset > 0;
  }

  const poll = async () => {
    if (!existsSync(logPath)) {
      return;
    }

    const stat = await fs.stat(logPath);
    if (stat.size < state.offset) {
      state.offset = 0;
      state.tailRemainder = "";
    }
    if (stat.size === state.offset) {
      return;
    }

    const length = stat.size - state.offset;
    const handle = await fs.open(logPath, "r");
    const buffer = Buffer.alloc(length);
    await handle.read(buffer, 0, length, state.offset);
    await handle.close();
    state.offset = stat.size;

    const chunkText = state.tailRemainder + buffer.toString("utf8");
    const lines = chunkText.split(/\r?\n/);
    state.tailRemainder = lines.pop() ?? "";
    if (state.skipFirstPartialLine) {
      lines.shift();
      state.skipFirstPartialLine = false;
    }

    let changed = false;
    for (const line of lines) {
      const entry = parseLogLine(line);
      if (handleEntry(entry, state)) {
        changed = true;
      }
    }

    if (changed) {
      renderTable(state);
    }
  };

  await poll();

  setInterval(() => {
    poll().catch((error) => {
      process.stderr.write(`perf-watch poll error: ${error.message}\n`);
    });
  }, 250);
}

run().catch((error) => {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exitCode = 1;
});
