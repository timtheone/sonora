use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde::Serialize;
use sonora_dictation_lib::audio;
use sonora_dictation_lib::config::{
    DictationMode, FasterWhisperComputeType, ModelProfile, SttEngine, WhisperBackendPreference,
};
use sonora_dictation_lib::pipeline::DictationPipeline;
use sonora_dictation_lib::postprocess::{merge_transcript_segments, normalize_transcript};
use sonora_dictation_lib::profile::ProfileTuning;
use sonora_dictation_lib::transcriber::{build_runtime_engine, EngineSpec};
use sonora_dictation_lib::vad::VadConfig;

const SAMPLE_RATE_HZ: usize = 16_000;
const DEFAULT_CHUNK_MS: u64 = 1_800;
const DEFAULT_PARTIAL_CADENCE_MS: u64 = 900;
const DEFAULT_SESSION_GAP_MS: u64 = 2_000;

#[derive(Debug, Clone)]
struct RunOptions {
    audio_path: PathBuf,
    reference_path: Option<PathBuf>,
    resource_dir: PathBuf,
    backend: WhisperBackendPreference,
    chunk_ms: u64,
    partial_cadence_ms: u64,
    session_gap_ms: u64,
    vad_disabled: bool,
    vad_threshold_milli: u16,
    runs: usize,
    case_names: Vec<String>,
}

#[derive(Debug, Clone)]
struct RecordOptions {
    out_path: PathBuf,
    seconds: u64,
    microphone_id: Option<String>,
    sensitivity_percent: u16,
}

#[derive(Debug, Clone)]
struct BenchCase {
    name: &'static str,
    engine: SttEngine,
    model_reference: &'static str,
    compute_type: FasterWhisperComputeType,
    beam_size: u8,
}

#[derive(Debug, Clone, Serialize)]
struct CaseResult {
    case_name: String,
    engine: String,
    model: String,
    backend: String,
    using_gpu: bool,
    ready: bool,
    error: Option<String>,
    audio_ms: u64,
    wall_ms: u64,
    prepare_ms: u64,
    real_time_factor: f64,
    chunks_total: usize,
    speech_chunks: usize,
    speech_ratio: f64,
    transcript_updates: usize,
    transcript: String,
    inference_ms_p50: u64,
    inference_ms_p95: u64,
    inference_ms_total: u64,
    wer: Option<f64>,
    cer: Option<f64>,
    self_peak_rss_kb: Option<u64>,
}

#[derive(Debug, Clone)]
struct RunMetrics {
    transcript: String,
    wall_ms: u64,
    prepare_ms: u64,
    chunks_total: usize,
    speech_chunks: usize,
    transcript_updates: usize,
    inference_values: Vec<u64>,
    inference_total: u64,
}

#[derive(Debug)]
struct SessionState {
    current: Option<String>,
    last_speech_elapsed_ms: u64,
    utterances: Vec<String>,
    updates: usize,
}

impl SessionState {
    fn new() -> Self {
        Self {
            current: None,
            last_speech_elapsed_ms: 0,
            utterances: Vec::new(),
            updates: 0,
        }
    }

    fn on_transcript(&mut self, raw: &str, elapsed_ms: u64) {
        let normalized = normalize_transcript(raw);
        if normalized.is_empty() {
            return;
        }

        if let Some(existing) = self.current.as_ref() {
            let merged = merge_transcript_segments(existing, &normalized);
            if merged != *existing {
                self.current = Some(merged);
                self.updates = self.updates.saturating_add(1);
            }
        } else {
            self.current = Some(normalized);
            self.updates = self.updates.saturating_add(1);
        }

        self.last_speech_elapsed_ms = elapsed_ms;
    }

    fn maybe_flush(&mut self, elapsed_ms: u64, session_gap_ms: u64) {
        if self.current.is_none() {
            return;
        }
        if elapsed_ms.saturating_sub(self.last_speech_elapsed_ms) < session_gap_ms {
            return;
        }
        self.flush();
    }

    fn flush(&mut self) {
        if let Some(text) = self.current.take() {
            self.utterances.push(text);
        }
    }

    fn final_transcript(mut self) -> String {
        self.flush();
        self.utterances.join(" ")
    }
}

fn main() {
    if let Err(error) = run_main() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run_main() -> Result<(), String> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let Some(command) = args.first().cloned() else {
        return Err(usage());
    };

    match command.as_str() {
        "record" => {
            let options = parse_record_options(&args[1..])?;
            record_sample(options)
        }
        "run" => {
            let options = parse_run_options(&args[1..])?;
            run_benchmark(options)
        }
        _ => Err(usage()),
    }
}

fn parse_record_options(args: &[String]) -> Result<RecordOptions, String> {
    let args = if args.first().map(|value| value.as_str()) == Some("--") {
        &args[1..]
    } else {
        args
    };

    let mut out_path: Option<PathBuf> = None;
    let mut seconds = 20u64;
    let mut microphone_id = None;
    let mut sensitivity_percent = 170u16;

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--out" => {
                out_path = Some(PathBuf::from(next_arg(args, &mut index, "--out")?));
            }
            "--seconds" => {
                seconds = parse_u64(next_arg(args, &mut index, "--seconds")?, "seconds")?;
            }
            "--microphone-id" => {
                microphone_id = Some(next_arg(args, &mut index, "--microphone-id")?.to_string());
            }
            "--sensitivity" => {
                let parsed =
                    parse_u64(next_arg(args, &mut index, "--sensitivity")?, "sensitivity")?;
                sensitivity_percent = parsed.clamp(50, 300) as u16;
            }
            unexpected => {
                return Err(format!("unknown record option: {unexpected}"));
            }
        }
        index += 1;
    }

    let out_path = out_path.ok_or_else(|| "record requires --out <path>".to_string())?;

    Ok(RecordOptions {
        out_path,
        seconds: seconds.max(1),
        microphone_id,
        sensitivity_percent,
    })
}

fn parse_run_options(args: &[String]) -> Result<RunOptions, String> {
    let args = if args.first().map(|value| value.as_str()) == Some("--") {
        &args[1..]
    } else {
        args
    };

    let mut audio_path: Option<PathBuf> = None;
    let mut reference_path: Option<PathBuf> = None;
    let mut resource_dir = PathBuf::from("src-tauri/resources");
    let mut backend = WhisperBackendPreference::Auto;
    let mut chunk_ms = DEFAULT_CHUNK_MS;
    let mut partial_cadence_ms = DEFAULT_PARTIAL_CADENCE_MS;
    let mut session_gap_ms = DEFAULT_SESSION_GAP_MS;
    let mut vad_disabled = false;
    let mut vad_threshold_milli = 9u16;
    let mut runs = 1usize;
    let mut case_names = Vec::<String>::new();

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--audio" => {
                audio_path = Some(PathBuf::from(next_arg(args, &mut index, "--audio")?));
            }
            "--reference" => {
                reference_path = Some(PathBuf::from(next_arg(args, &mut index, "--reference")?));
            }
            "--resource-dir" => {
                resource_dir = PathBuf::from(next_arg(args, &mut index, "--resource-dir")?);
            }
            "--backend" => {
                backend = parse_backend(next_arg(args, &mut index, "--backend")?)?;
            }
            "--chunk-ms" => {
                chunk_ms = parse_u64(next_arg(args, &mut index, "--chunk-ms")?, "chunk-ms")?;
            }
            "--cadence-ms" => {
                partial_cadence_ms =
                    parse_u64(next_arg(args, &mut index, "--cadence-ms")?, "cadence-ms")?;
            }
            "--session-gap-ms" => {
                session_gap_ms = parse_u64(
                    next_arg(args, &mut index, "--session-gap-ms")?,
                    "session-gap-ms",
                )?;
            }
            "--disable-vad" => {
                vad_disabled = true;
            }
            "--vad-threshold-milli" => {
                let parsed = parse_u64(
                    next_arg(args, &mut index, "--vad-threshold-milli")?,
                    "vad-threshold-milli",
                )?;
                vad_threshold_milli = parsed.clamp(1, 80) as u16;
            }
            "--runs" => {
                let parsed = parse_u64(next_arg(args, &mut index, "--runs")?, "runs")?;
                runs = parsed.max(1) as usize;
            }
            "--case" => {
                case_names.push(next_arg(args, &mut index, "--case")?.to_string());
            }
            unexpected => {
                return Err(format!("unknown run option: {unexpected}"));
            }
        }
        index += 1;
    }

    let audio_path = audio_path.ok_or_else(|| "run requires --audio <path>".to_string())?;
    if case_names.is_empty() {
        case_names = default_case_names()
            .iter()
            .map(|value| value.to_string())
            .collect();
    }

    Ok(RunOptions {
        audio_path,
        reference_path,
        resource_dir,
        backend,
        chunk_ms: chunk_ms.clamp(500, 4_000),
        partial_cadence_ms: partial_cadence_ms.clamp(300, 2_500),
        session_gap_ms: session_gap_ms.max(500),
        vad_disabled,
        vad_threshold_milli,
        runs,
        case_names,
    })
}

fn record_sample(options: RecordOptions) -> Result<(), String> {
    let parent = options
        .out_path
        .parent()
        .ok_or_else(|| "output path has no parent directory".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("failed to create output directory: {error}"))?;

    let (frame_tx, frame_rx) = mpsc::sync_channel::<Vec<f32>>(64);
    let stream = audio::build_live_input_stream(options.microphone_id.as_deref(), frame_tx)?;
    let gain = (options.sensitivity_percent.clamp(50, 300) as f32 / 100.0).clamp(0.5, 3.0);

    eprintln!(
        "recording benchmark sample for {}s (source={} Hz)...",
        options.seconds, stream.sample_rate_hz
    );

    let started_at = Instant::now();
    let mut captured_16k = Vec::<f32>::new();
    while started_at.elapsed() < Duration::from_secs(options.seconds) {
        match frame_rx.recv_timeout(Duration::from_millis(80)) {
            Ok(mut frame) => {
                apply_gain(&mut frame, gain);
                let downsampled = audio::downsample_to_16k(&frame, stream.sample_rate_hz);
                if !downsampled.is_empty() {
                    captured_16k.extend(downsampled);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err("microphone stream disconnected while recording".to_string());
            }
        }
    }
    drop(stream);

    write_wav_f32(&options.out_path, &captured_16k)?;
    let duration_ms = (captured_16k.len() as u64 * 1000) / SAMPLE_RATE_HZ as u64;
    eprintln!(
        "saved benchmark sample: {} ({} ms, {} samples)",
        options.out_path.to_string_lossy(),
        duration_ms,
        captured_16k.len()
    );
    Ok(())
}

fn run_benchmark(options: RunOptions) -> Result<(), String> {
    if !options.audio_path.exists() {
        return Err(format!(
            "audio file not found: {}",
            options.audio_path.to_string_lossy()
        ));
    }

    let reference_text = options
        .reference_path
        .as_ref()
        .map(|path| {
            fs::read_to_string(path).map_err(|error| {
                format!(
                    "failed to read reference text '{}': {error}",
                    path.to_string_lossy()
                )
            })
        })
        .transpose()?;

    let samples = read_audio_16k_mono(&options.audio_path)?;
    if samples.is_empty() {
        return Err("audio file decoded to empty sample buffer".to_string());
    }
    let audio_ms = (samples.len() as u64 * 1000) / SAMPLE_RATE_HZ as u64;

    let mut results = Vec::<CaseResult>::new();
    for case_name in &options.case_names {
        let case = resolve_case(case_name)?;
        let result = run_single_case(
            case,
            &options,
            &samples,
            audio_ms,
            reference_text.as_deref(),
        )?;
        results.push(result);
    }

    print_report(&options, &results);
    let payload = serde_json::to_string_pretty(&results)
        .map_err(|error| format!("failed to serialize benchmark results: {error}"))?;
    println!("\n{payload}");
    Ok(())
}

fn run_single_case(
    case: BenchCase,
    options: &RunOptions,
    samples: &[f32],
    audio_ms: u64,
    reference_text: Option<&str>,
) -> Result<CaseResult, String> {
    let mut run_metrics = Vec::<RunMetrics>::new();
    let mut ready = false;
    let mut backend = "unavailable".to_string();
    let mut using_gpu = false;
    let mut error = None;

    for _ in 0..options.runs {
        let spec = build_case_spec(&case, options)?;
        let runtime = build_runtime_engine(spec);
        ready = runtime.diagnostics.ready;
        backend = runtime.diagnostics.compute_backend.clone();
        using_gpu = runtime.diagnostics.using_gpu;

        if !runtime.diagnostics.ready {
            error = Some(runtime.diagnostics.description);
            break;
        }

        let run = benchmark_runtime_once(runtime.transcriber, options, samples)?;
        run_metrics.push(run);
    }

    if !ready {
        return Ok(CaseResult {
            case_name: case.name.to_string(),
            engine: case_engine_label(case.engine).to_string(),
            model: case.model_reference.to_string(),
            backend,
            using_gpu,
            ready,
            error,
            audio_ms,
            wall_ms: 0,
            prepare_ms: 0,
            real_time_factor: 0.0,
            chunks_total: 0,
            speech_chunks: 0,
            speech_ratio: 0.0,
            transcript_updates: 0,
            transcript: String::new(),
            inference_ms_p50: 0,
            inference_ms_p95: 0,
            inference_ms_total: 0,
            wer: None,
            cer: None,
            self_peak_rss_kb: self_peak_rss_kb(),
        });
    }

    let wall_ms = average_u64(run_metrics.iter().map(|value| value.wall_ms));
    let prepare_ms = average_u64(run_metrics.iter().map(|value| value.prepare_ms));
    let chunks_total =
        average_u64(run_metrics.iter().map(|value| value.chunks_total as u64)) as usize;
    let speech_chunks =
        average_u64(run_metrics.iter().map(|value| value.speech_chunks as u64)) as usize;
    let transcript_updates = average_u64(
        run_metrics
            .iter()
            .map(|value| value.transcript_updates as u64),
    ) as usize;
    let inference_total = average_u64(run_metrics.iter().map(|value| value.inference_total));

    let mut all_infer = Vec::<u64>::new();
    for metric in &run_metrics {
        all_infer.extend(
            metric
                .inference_values
                .iter()
                .copied()
                .filter(|value| *value > 0),
        );
    }

    let transcript = run_metrics
        .first()
        .map(|value| value.transcript.clone())
        .unwrap_or_default();
    let (wer, cer) = reference_text
        .map(|expected| {
            (
                word_error_rate(expected, &transcript),
                char_error_rate(expected, &transcript),
            )
        })
        .unwrap_or((None, None));

    Ok(CaseResult {
        case_name: case.name.to_string(),
        engine: case_engine_label(case.engine).to_string(),
        model: case.model_reference.to_string(),
        backend,
        using_gpu,
        ready,
        error,
        audio_ms,
        wall_ms,
        prepare_ms,
        real_time_factor: if audio_ms == 0 {
            0.0
        } else {
            wall_ms as f64 / audio_ms as f64
        },
        chunks_total,
        speech_chunks,
        speech_ratio: if chunks_total == 0 {
            0.0
        } else {
            speech_chunks as f64 / chunks_total as f64
        },
        transcript_updates,
        transcript,
        inference_ms_p50: percentile(&all_infer, 50),
        inference_ms_p95: percentile(&all_infer, 95),
        inference_ms_total: inference_total,
        wer,
        cer,
        self_peak_rss_kb: self_peak_rss_kb(),
    })
}

fn benchmark_runtime_once(
    transcriber: sonora_dictation_lib::transcriber::RuntimeTranscriber,
    options: &RunOptions,
    samples: &[f32],
) -> Result<RunMetrics, String> {
    let mut pipeline = DictationPipeline::new(
        DictationMode::PushToToggle,
        ModelProfile::Balanced,
        transcriber,
    );
    pipeline.set_tuning(ProfileTuning {
        min_chunk_samples: ((SAMPLE_RATE_HZ as u64 * options.chunk_ms) / 1000).max(8_000) as usize,
        partial_cadence_ms: options.partial_cadence_ms,
    });

    let mut vad_config = VadConfig::default();
    vad_config.enabled = !options.vad_disabled;
    vad_config.rms_threshold = options.vad_threshold_milli.clamp(1, 80) as f32 / 1000.0;
    pipeline.set_vad_config(vad_config);

    pipeline.on_hotkey_down();

    let prepare_started_at = Instant::now();
    pipeline.prepare_transcriber()?;
    let prepare_ms = duration_ms_u64(prepare_started_at.elapsed());

    let run_started_at = Instant::now();
    let chunk_samples = ((SAMPLE_RATE_HZ as u64 * options.chunk_ms) / 1000).max(8_000) as usize;

    let mut chunks_total = 0usize;
    let mut speech_chunks = 0usize;
    let mut inference_values = Vec::<u64>::new();
    let mut inference_total = 0u64;
    let mut session = SessionState::new();
    let mut elapsed_audio_ms = 0u64;

    for chunk in samples.chunks(chunk_samples) {
        chunks_total = chunks_total.saturating_add(1);
        let metrics = pipeline.process_audio_chunk_profiled(chunk)?;
        let chunk_audio_ms = (chunk.len() as u64 * 1000) / SAMPLE_RATE_HZ as u64;
        elapsed_audio_ms = elapsed_audio_ms.saturating_add(chunk_audio_ms);

        if metrics.had_speech {
            speech_chunks = speech_chunks.saturating_add(1);
        }
        if metrics.inference_ms > 0 {
            inference_values.push(metrics.inference_ms);
            inference_total = inference_total.saturating_add(metrics.inference_ms);
        }
        if let Some(text) = metrics.transcript {
            session.on_transcript(&text, elapsed_audio_ms);
        } else {
            session.maybe_flush(elapsed_audio_ms, options.session_gap_ms);
        }
    }

    let transcript_updates = session.updates;
    let transcript = session.final_transcript();

    Ok(RunMetrics {
        transcript,
        wall_ms: duration_ms_u64(run_started_at.elapsed()),
        prepare_ms,
        chunks_total,
        speech_chunks,
        transcript_updates,
        inference_values,
        inference_total,
    })
}

fn build_case_spec(case: &BenchCase, options: &RunOptions) -> Result<EngineSpec, String> {
    let model_path = resolve_case_model_path(case, &options.resource_dir);

    Ok(EngineSpec {
        engine: case.engine,
        language: "en".to_string(),
        model_profile: ModelProfile::Balanced,
        model_path,
        whisper_backend_preference: options.backend,
        faster_whisper_compute_type: case.compute_type,
        faster_whisper_beam_size: case.beam_size,
        resource_dir: Some(options.resource_dir.clone()),
    })
}

fn resolve_case_model_path(case: &BenchCase, resource_dir: &Path) -> PathBuf {
    match case.engine {
        SttEngine::WhisperCpp => resource_dir.join(case.model_reference),
        SttEngine::FasterWhisper => PathBuf::from(case.model_reference),
    }
}

fn resolve_case(name: &str) -> Result<BenchCase, String> {
    match name {
        "whisper-large-v3-turbo-q8" => Ok(BenchCase {
            name: "whisper-large-v3-turbo-q8",
            engine: SttEngine::WhisperCpp,
            model_reference: "models/ggml-large-v3-turbo-q8_0.bin",
            compute_type: FasterWhisperComputeType::Auto,
            beam_size: 1,
        }),
        "whisper-base-en-q8" => Ok(BenchCase {
            name: "whisper-base-en-q8",
            engine: SttEngine::WhisperCpp,
            model_reference: "models/ggml-base.en-q8_0.bin",
            compute_type: FasterWhisperComputeType::Auto,
            beam_size: 1,
        }),
        "faster-large-v3" => Ok(BenchCase {
            name: "faster-large-v3",
            engine: SttEngine::FasterWhisper,
            model_reference: "large-v3",
            compute_type: FasterWhisperComputeType::Float16,
            beam_size: 5,
        }),
        "faster-distil-large-v3" => Ok(BenchCase {
            name: "faster-distil-large-v3",
            engine: SttEngine::FasterWhisper,
            model_reference: "distil-large-v3",
            compute_type: FasterWhisperComputeType::Float16,
            beam_size: 5,
        }),
        "faster-small-en" => Ok(BenchCase {
            name: "faster-small-en",
            engine: SttEngine::FasterWhisper,
            model_reference: "small.en",
            compute_type: FasterWhisperComputeType::Float16,
            beam_size: 3,
        }),
        other => Err(format!("unknown --case '{other}'")),
    }
}

fn default_case_names() -> [&'static str; 5] {
    [
        "whisper-large-v3-turbo-q8",
        "whisper-base-en-q8",
        "faster-large-v3",
        "faster-distil-large-v3",
        "faster-small-en",
    ]
}

fn case_engine_label(engine: SttEngine) -> &'static str {
    match engine {
        SttEngine::WhisperCpp => "whisper_cpp",
        SttEngine::FasterWhisper => "faster_whisper",
    }
}

fn print_report(options: &RunOptions, results: &[CaseResult]) {
    println!("Sonora Benchmark Report");
    println!("audio: {}", options.audio_path.to_string_lossy());
    println!(
        "chunk={}ms cadence={}ms session_gap={}ms vad_disabled={} vad_threshold={:.3}",
        options.chunk_ms,
        options.partial_cadence_ms,
        options.session_gap_ms,
        options.vad_disabled,
        options.vad_threshold_milli as f32 / 1000.0,
    );
    println!("runs per case: {}", options.runs);
    println!();
    println!(
        "{:<26} {:<15} {:<6} {:>7} {:>7} {:>6} {:>7} {:>7} {:>7} {:>7}",
        "case", "engine", "backnd", "wall", "prep", "rtf", "p50inf", "p95inf", "wer", "cer"
    );
    println!("{}", "-".repeat(108));

    for result in results {
        let wer = result
            .wer
            .map(|value| format!("{:.3}", value))
            .unwrap_or_else(|| "-".to_string());
        let cer = result
            .cer
            .map(|value| format!("{:.3}", value))
            .unwrap_or_else(|| "-".to_string());
        let rtf = if result.real_time_factor > 0.0 {
            format!("{:.2}", result.real_time_factor)
        } else {
            "-".to_string()
        };

        println!(
            "{:<26} {:<15} {:<6} {:>7} {:>7} {:>6} {:>7} {:>7} {:>7} {:>7}",
            truncate(&result.case_name, 26),
            truncate(&result.engine, 15),
            truncate(&result.backend, 6),
            result.wall_ms,
            result.prepare_ms,
            rtf,
            result.inference_ms_p50,
            result.inference_ms_p95,
            wer,
            cer,
        );

        if let Some(error) = &result.error {
            println!("  error: {error}");
            continue;
        }

        println!(
            "  chunks={}/{} speech_ratio={:.1}% gpu={} rss_kb={} updates={} infer_total={}ms",
            result.speech_chunks,
            result.chunks_total,
            result.speech_ratio * 100.0,
            result.using_gpu,
            result
                .self_peak_rss_kb
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            result.transcript_updates,
            result.inference_ms_total,
        );
    }
}

fn read_audio_16k_mono(path: &Path) -> Result<Vec<f32>, String> {
    let mut reader = hound::WavReader::open(path)
        .map_err(|error| format!("failed to open wav '{}': {error}", path.to_string_lossy()))?;
    let spec = reader.spec();
    if spec.channels == 0 {
        return Err("wav has invalid channel count 0".to_string());
    }

    let mut interleaved = Vec::<f32>::new();
    match spec.sample_format {
        hound::SampleFormat::Int => {
            if spec.bits_per_sample <= 16 {
                for sample in reader.samples::<i16>() {
                    let value =
                        sample.map_err(|error| format!("failed to read i16 sample: {error}"))?;
                    interleaved.push(value as f32 / i16::MAX as f32);
                }
            } else {
                let scale = ((1_i64 << (spec.bits_per_sample - 1)) - 1) as f32;
                for sample in reader.samples::<i32>() {
                    let value =
                        sample.map_err(|error| format!("failed to read i32 sample: {error}"))?;
                    interleaved.push((value as f32 / scale).clamp(-1.0, 1.0));
                }
            }
        }
        hound::SampleFormat::Float => {
            for sample in reader.samples::<f32>() {
                let value =
                    sample.map_err(|error| format!("failed to read f32 sample: {error}"))?;
                interleaved.push(value.clamp(-1.0, 1.0));
            }
        }
    }

    let mono = if spec.channels == 1 {
        interleaved
    } else {
        let channels = spec.channels as usize;
        let mut output = Vec::<f32>::with_capacity(interleaved.len() / channels);
        for frame in interleaved.chunks_exact(channels) {
            output.push(frame.iter().copied().sum::<f32>() / channels as f32);
        }
        output
    };

    if spec.sample_rate < SAMPLE_RATE_HZ as u32 {
        return Err(format!(
            "unsupported sample rate {} Hz (minimum is 16000 Hz)",
            spec.sample_rate
        ));
    }

    if spec.sample_rate == SAMPLE_RATE_HZ as u32 {
        return Ok(mono);
    }

    Ok(audio::downsample_to_16k(&mono, spec.sample_rate))
}

fn write_wav_f32(path: &Path, samples: &[f32]) -> Result<(), String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE_HZ as u32,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec)
        .map_err(|error| format!("failed to create wav: {error}"))?;

    for sample in samples {
        let clamped = sample.clamp(-1.0, 1.0);
        let value = (clamped * i16::MAX as f32) as i16;
        writer
            .write_sample(value)
            .map_err(|error| format!("failed to write wav sample: {error}"))?;
    }

    writer
        .finalize()
        .map_err(|error| format!("failed to finalize wav: {error}"))
}

fn apply_gain(samples: &mut [f32], gain: f32) {
    if (gain - 1.0).abs() < f32::EPSILON {
        return;
    }
    for sample in samples {
        *sample = (*sample * gain).clamp(-1.0, 1.0);
    }
}

fn parse_backend(value: &str) -> Result<WhisperBackendPreference, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "auto" => Ok(WhisperBackendPreference::Auto),
        "cpu" => Ok(WhisperBackendPreference::Cpu),
        "cuda" | "gpu" | "nvidia" => Ok(WhisperBackendPreference::Cuda),
        other => Err(format!(
            "unsupported backend '{other}', expected auto|cpu|cuda"
        )),
    }
}

fn parse_u64(value: &str, field: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .map_err(|_| format!("invalid {field}: {value}"))
}

fn next_arg<'a>(args: &'a [String], index: &mut usize, flag: &str) -> Result<&'a str, String> {
    let next = *index + 1;
    if next >= args.len() {
        return Err(format!("missing value for {flag}"));
    }
    *index = next;
    Ok(args[next].as_str())
}

fn duration_ms_u64(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

fn average_u64<I>(values: I) -> u64
where
    I: Iterator<Item = u64>,
{
    let collected = values.collect::<Vec<_>>();
    if collected.is_empty() {
        return 0;
    }
    let sum = collected.iter().copied().sum::<u64>();
    sum / collected.len() as u64
}

fn percentile(values: &[u64], p: usize) -> u64 {
    if values.is_empty() {
        return 0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let index = ((p as f64 / 100.0) * sorted.len() as f64).ceil() as usize;
    sorted[index.saturating_sub(1).min(sorted.len() - 1)]
}

fn tokenize_words(input: &str) -> Vec<String> {
    input
        .to_ascii_lowercase()
        .split_whitespace()
        .map(|token| {
            token
                .chars()
                .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '%' || *ch == '.')
                .collect::<String>()
        })
        .filter(|token| !token.is_empty())
        .collect()
}

fn levenshtein<T: PartialEq>(left: &[T], right: &[T]) -> usize {
    if left.is_empty() {
        return right.len();
    }
    if right.is_empty() {
        return left.len();
    }

    let mut prev = (0..=right.len()).collect::<Vec<_>>();
    let mut curr = vec![0usize; right.len() + 1];

    for (i, l) in left.iter().enumerate() {
        curr[0] = i + 1;
        for (j, r) in right.iter().enumerate() {
            let cost = if l == r { 0 } else { 1 };
            curr[j + 1] = (prev[j + 1] + 1).min(curr[j] + 1).min(prev[j] + cost);
        }
        prev.copy_from_slice(&curr);
    }

    prev[right.len()]
}

fn word_error_rate(reference: &str, hypothesis: &str) -> Option<f64> {
    let ref_tokens = tokenize_words(reference);
    if ref_tokens.is_empty() {
        return None;
    }
    let hyp_tokens = tokenize_words(hypothesis);
    let distance = levenshtein(&ref_tokens, &hyp_tokens);
    Some(distance as f64 / ref_tokens.len() as f64)
}

fn char_error_rate(reference: &str, hypothesis: &str) -> Option<f64> {
    let ref_chars = reference
        .to_ascii_lowercase()
        .chars()
        .filter(|ch| !ch.is_ascii_whitespace())
        .collect::<Vec<_>>();
    if ref_chars.is_empty() {
        return None;
    }
    let hyp_chars = hypothesis
        .to_ascii_lowercase()
        .chars()
        .filter(|ch| !ch.is_ascii_whitespace())
        .collect::<Vec<_>>();
    let distance = levenshtein(&ref_chars, &hyp_chars);
    Some(distance as f64 / ref_chars.len() as f64)
}

fn self_peak_rss_kb() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let raw = fs::read_to_string("/proc/self/status").ok()?;
        for line in raw.lines() {
            if !line.starts_with("VmHWM:") {
                continue;
            }
            let parts = line.split_whitespace().collect::<Vec<_>>();
            if parts.len() >= 2 {
                if let Ok(value) = parts[1].parse::<u64>() {
                    return Some(value);
                }
            }
        }
        None
    }

    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

fn truncate(value: &str, max_len: usize) -> String {
    if value.len() <= max_len {
        return value.to_string();
    }
    value[..max_len].to_string()
}

fn usage() -> String {
    [
        "usage:",
        "  transcription-bench record --out <path.wav> [--seconds 20] [--microphone-id 0] [--sensitivity 170]",
        "  transcription-bench run --audio <path.wav> [--reference <path.txt>] [--case <name>]...",
        "",
        "run options:",
        "  --resource-dir <path>         default: src-tauri/resources",
        "  --backend auto|cpu|cuda       default: auto",
        "  --chunk-ms <500..4000>        default: 1800",
        "  --cadence-ms <300..2500>      default: 900",
        "  --session-gap-ms <ms>         default: 2000",
        "  --runs <n>                    default: 1",
        "  --disable-vad                 disable VAD for benchmark-only runs",
        "  --vad-threshold-milli <1..80> default: 9 (0.009)",
        "",
        "available cases:",
        "  whisper-large-v3-turbo-q8",
        "  whisper-base-en-q8",
        "  faster-large-v3",
        "  faster-distil-large-v3",
        "  faster-small-en",
    ]
    .join("\n")
}
