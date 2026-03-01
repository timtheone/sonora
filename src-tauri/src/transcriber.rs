use std::collections::HashSet;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::{
    FasterWhisperComputeType, ModelProfile, ParakeetComputeType, SttEngine,
    WhisperBackendPreference,
};
use serde::{Deserialize, Serialize};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;
#[cfg(target_os = "windows")]
const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x00004000;

pub trait Transcriber: Send + Sync {
    fn transcribe(&self, samples: &[f32]) -> Result<String, String>;

    fn set_stream_context(&self, _context: Option<&str>) {}

    fn prepare(&self) -> Result<(), String> {
        Ok(())
    }

    fn engine_label(&self) -> &'static str {
        "unknown"
    }

    fn model_label(&self) -> String {
        "unknown".to_string()
    }

    fn backend_label(&self) -> String {
        "unknown".to_string()
    }
}

#[derive(Debug, Clone, Default)]
pub struct StubTranscriber;

impl Transcriber for StubTranscriber {
    fn transcribe(&self, _samples: &[f32]) -> Result<String, String> {
        Ok("phase-1 transcript".to_string())
    }

    fn engine_label(&self) -> &'static str {
        "stub"
    }

    fn backend_label(&self) -> String {
        "stub".to_string()
    }
}

#[derive(Debug, Clone)]
pub struct WhisperSidecarConfig {
    pub binary_path: PathBuf,
    pub model_path: PathBuf,
    pub language: String,
    pub threads: usize,
    pub compute_backend: WhisperComputeBackend,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhisperComputeBackend {
    Cpu,
    Cuda,
}

impl WhisperComputeBackend {
    fn as_label(self) -> &'static str {
        match self {
            WhisperComputeBackend::Cpu => "cpu",
            WhisperComputeBackend::Cuda => "cuda",
        }
    }
}

#[derive(Debug, Deserialize)]
struct WhisperSidecarMetadata {
    backend: Option<String>,
}

const SIDECAR_METADATA_FILE_NAME: &str = "whisper-sidecar.json";
const BACKEND_ENV_NAME: &str = "SONORA_WHISPER_BACKEND";
const FASTER_WHISPER_BIN_ENV_NAME: &str = "SONORA_FASTER_WHISPER_BIN";
const PARAKEET_BIN_ENV_NAME: &str = "SONORA_PARAKEET_BIN";
const WHISPER_EXTRA_PATH_ENV_NAME: &str = "SONORA_WHISPER_EXTRA_PATH";
const FASTER_WHISPER_EXTRA_PATH_ENV_NAME: &str = "SONORA_FASTER_WHISPER_EXTRA_PATH";
const FASTER_WHISPER_DEFAULT_MODEL_FAST: &str = "tiny.en";
const FASTER_WHISPER_DEFAULT_MODEL_BALANCED: &str = "small.en";
const PARAKEET_DEFAULT_MODEL_FAST: &str = "nvidia/parakeet-ctc-0.6b";
const PARAKEET_DEFAULT_MODEL_BALANCED: &str = "nvidia/parakeet-ctc-1.1b";

#[derive(Debug, Clone)]
pub struct EngineSpec {
    pub engine: SttEngine,
    pub language: String,
    pub model_profile: ModelProfile,
    pub model_path: PathBuf,
    pub whisper_backend_preference: WhisperBackendPreference,
    pub faster_whisper_compute_type: FasterWhisperComputeType,
    pub faster_whisper_beam_size: u8,
    pub parakeet_compute_type: ParakeetComputeType,
    pub resource_dir: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct RuntimeEngineDiagnostics {
    pub ready: bool,
    pub active_engine: String,
    pub description: String,
    pub compute_backend: String,
    pub using_gpu: bool,
    pub resolved_binary_path: Option<String>,
    pub checked_binary_paths: Vec<String>,
    pub resolved_model_path: String,
    pub model_exists: bool,
}

#[derive(Debug, Clone)]
pub struct RuntimeEngine {
    pub transcriber: RuntimeTranscriber,
    pub diagnostics: RuntimeEngineDiagnostics,
}

impl WhisperSidecarConfig {
    pub fn command_args(&self, audio_file: &Path, output_prefix: &Path) -> Vec<String> {
        let mut args = vec![
            "-m".to_string(),
            self.model_path.to_string_lossy().to_string(),
            "-f".to_string(),
            audio_file.to_string_lossy().to_string(),
            "-l".to_string(),
            self.language.clone(),
            "-t".to_string(),
            self.threads.to_string(),
            "-np".to_string(),
            "--no-timestamps".to_string(),
            "-otxt".to_string(),
            "-of".to_string(),
            output_prefix.to_string_lossy().to_string(),
        ];

        if self.compute_backend == WhisperComputeBackend::Cpu {
            args.push("-ng".to_string());
        }

        args
    }
}

#[derive(Debug, Clone)]
pub struct WhisperSidecarTranscriber {
    pub config: WhisperSidecarConfig,
}

impl WhisperSidecarTranscriber {
    fn transcribe_impl(&self, samples: &[f32]) -> Result<String, String> {
        if samples.is_empty() {
            return Err("cannot transcribe empty audio chunk".to_string());
        }

        let token = temporary_token();
        let temp_dir = std::env::temp_dir();
        let wav_path = temp_dir.join(format!("sonora-{token}.wav"));
        let output_prefix = temp_dir.join(format!("sonora-{token}-out"));
        let txt_path = output_prefix.with_extension("txt");

        write_wav_file(&wav_path, samples)?;

        let args = self.config.command_args(&wav_path, &output_prefix);
        let mut command = Command::new(&self.config.binary_path);
        command.args(args);

        if self.config.compute_backend == WhisperComputeBackend::Cuda {
            let extra_paths = extra_path_entries_from_env(WHISPER_EXTRA_PATH_ENV_NAME);
            prepend_process_path(&mut command, &extra_paths);
        }

        #[cfg(target_os = "windows")]
        {
            command.creation_flags(CREATE_NO_WINDOW | BELOW_NORMAL_PRIORITY_CLASS);
        }

        let output = command.output().map_err(|error| {
            format!(
                "failed to execute whisper sidecar at '{}': {}",
                self.config.binary_path.to_string_lossy(),
                error
            )
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            cleanup_temp_files(&[&wav_path, &txt_path]);
            return Err(format!(
                "whisper sidecar exited with status {}: {}",
                output.status,
                stderr.trim()
            ));
        }

        let transcript = if txt_path.exists() {
            fs::read_to_string(&txt_path)
                .map_err(|error| format!("failed to read transcription output: {}", error))?
        } else {
            String::from_utf8_lossy(&output.stdout).to_string()
        };

        cleanup_temp_files(&[&wav_path, &txt_path]);

        let normalized = transcript.trim().to_string();
        if normalized.is_empty() {
            return Err("whisper sidecar returned empty transcript".to_string());
        }

        Ok(normalized)
    }
}

impl Transcriber for WhisperSidecarTranscriber {
    fn transcribe(&self, samples: &[f32]) -> Result<String, String> {
        self.transcribe_impl(samples)
    }

    fn engine_label(&self) -> &'static str {
        "whisper_cpp"
    }

    fn model_label(&self) -> String {
        self.config.model_path.to_string_lossy().to_string()
    }

    fn backend_label(&self) -> String {
        self.config.compute_backend.as_label().to_string()
    }
}

#[derive(Debug, Clone)]
pub struct FasterWhisperSidecarConfig {
    pub binary_path: PathBuf,
    pub model: String,
    pub model_cache_dir: PathBuf,
    pub language: String,
    pub device: String,
    pub compute_type: String,
    pub beam_size: u8,
    pub condition_on_previous_text: bool,
}

#[derive(Debug)]
struct FasterWhisperWorker {
    _child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

#[derive(Debug, Clone)]
pub struct FasterWhisperSidecarTranscriber {
    pub config: FasterWhisperSidecarConfig,
    worker: Arc<Mutex<Option<FasterWhisperWorker>>>,
    preloaded: Arc<Mutex<bool>>,
    context_prompt: Arc<Mutex<Option<String>>>,
}

#[derive(Debug, Clone)]
pub struct ParakeetSidecarConfig {
    pub binary_path: PathBuf,
    pub model: String,
    pub model_cache_dir: PathBuf,
    pub language: String,
    pub device: String,
    pub compute_type: String,
}

#[derive(Debug)]
struct ParakeetWorker {
    _child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

#[derive(Debug, Clone)]
pub struct ParakeetSidecarTranscriber {
    pub config: ParakeetSidecarConfig,
    worker: Arc<Mutex<Option<ParakeetWorker>>>,
    preloaded: Arc<Mutex<bool>>,
}

impl FasterWhisperSidecarTranscriber {
    pub fn new(config: FasterWhisperSidecarConfig) -> Self {
        Self {
            config,
            worker: Arc::new(Mutex::new(None)),
            preloaded: Arc::new(Mutex::new(false)),
            context_prompt: Arc::new(Mutex::new(None)),
        }
    }

    fn transcribe_impl(&self, samples: &[f32]) -> Result<String, String> {
        if samples.is_empty() {
            return Err("cannot transcribe empty audio chunk".to_string());
        }

        self.prepare_impl()?;

        let token = temporary_token();
        let temp_dir = std::env::temp_dir();
        let wav_path = temp_dir.join(format!("sonora-faster-{token}.wav"));
        let initial_prompt = self
            .context_prompt
            .lock()
            .map_err(|_| "failed to acquire faster-whisper context lock".to_string())?
            .clone();

        write_wav_file(&wav_path, samples)?;
        let request = FasterWhisperRequest {
            op: "transcribe".to_string(),
            id: token,
            audio_path: path_to_sidecar_string(&wav_path),
            language: self.config.language.clone(),
            model: self.config.model.clone(),
            device: self.config.device.clone(),
            compute_type: self.config.compute_type.clone(),
            beam_size: self.config.beam_size,
            condition_on_previous_text: self.config.condition_on_previous_text,
            initial_prompt,
        };

        let result = self.send_request(request);
        cleanup_temp_files(&[&wav_path]);
        result
    }

    fn send_request(&self, request: FasterWhisperRequest) -> Result<String, String> {
        let request_id = request.id.clone();
        let mut guard = self
            .worker
            .lock()
            .map_err(|_| "failed to acquire faster-whisper worker lock".to_string())?;

        ensure_faster_whisper_worker(&mut guard, &self.config)?;

        let payload = serde_json::to_string(&request)
            .map_err(|error| format!("failed to serialize faster-whisper request: {error}"))?;

        let worker = match guard.as_mut() {
            Some(worker) => worker,
            None => return Err("faster-whisper worker was not initialized".to_string()),
        };

        worker
            .stdin
            .write_all(payload.as_bytes())
            .map_err(|error| format!("failed to write faster-whisper request: {error}"))?;
        worker
            .stdin
            .write_all(b"\n")
            .map_err(|error| format!("failed to finalize faster-whisper request: {error}"))?;
        worker
            .stdin
            .flush()
            .map_err(|error| format!("failed to flush faster-whisper request: {error}"))?;

        let mut response = None;
        let mut non_json_lines = Vec::<String>::new();
        for _ in 0..64 {
            let mut line = String::new();
            let bytes_read = worker
                .stdout
                .read_line(&mut line)
                .map_err(|error| format!("failed to read faster-whisper response: {error}"))?;

            if bytes_read == 0 {
                *guard = None;
                return Err("faster-whisper worker closed stdout unexpectedly".to_string());
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            match serde_json::from_str::<FasterWhisperResponse>(trimmed) {
                Ok(parsed) => {
                    if parsed.id.as_deref() == Some(request_id.as_str()) {
                        response = Some(parsed);
                        break;
                    }
                }
                Err(_) => {
                    if non_json_lines.len() < 3 {
                        non_json_lines.push(trimmed.to_string());
                    }
                }
            }
        }

        let response = response.ok_or_else(|| {
            if non_json_lines.is_empty() {
                "did not receive matching faster-whisper JSON response".to_string()
            } else {
                format!(
                    "did not receive matching faster-whisper JSON response (worker output: {})",
                    non_json_lines.join(" | ")
                )
            }
        })?;

        if !response.ok {
            return Err(response
                .error
                .unwrap_or_else(|| "unknown faster-whisper worker error".to_string()));
        }

        let normalized = response.text.unwrap_or_default().trim().to_string();
        Ok(normalized)
    }

    fn prepare_impl(&self) -> Result<(), String> {
        {
            let preloaded = self
                .preloaded
                .lock()
                .map_err(|_| "failed to acquire faster-whisper preloaded lock".to_string())?;
            if *preloaded {
                return Ok(());
            }
        }

        let mut guard = self
            .worker
            .lock()
            .map_err(|_| "failed to acquire faster-whisper worker lock".to_string())?;
        ensure_faster_whisper_worker(&mut guard, &self.config)?;

        let preload_request = FasterWhisperPreloadRequest {
            op: "preload".to_string(),
            id: "preload-runtime".to_string(),
            model: self.config.model.clone(),
            language: self.config.language.clone(),
            device: self.config.device.clone(),
            compute_type: self.config.compute_type.clone(),
            warmup: self.config.device == "cuda",
        };
        let payload = serde_json::to_string(&preload_request).map_err(|error| {
            format!("failed to serialize faster-whisper preload request: {error}")
        })?;

        let worker = match guard.as_mut() {
            Some(worker) => worker,
            None => return Err("faster-whisper worker was not initialized".to_string()),
        };

        worker
            .stdin
            .write_all(payload.as_bytes())
            .map_err(|error| format!("failed to write faster-whisper preload request: {error}"))?;
        worker.stdin.write_all(b"\n").map_err(|error| {
            format!("failed to finalize faster-whisper preload request: {error}")
        })?;
        worker
            .stdin
            .flush()
            .map_err(|error| format!("failed to flush faster-whisper preload request: {error}"))?;

        let mut response = None;
        for _ in 0..64 {
            let mut line = String::new();
            let bytes_read = worker.stdout.read_line(&mut line).map_err(|error| {
                format!("failed to read faster-whisper preload response: {error}")
            })?;
            if bytes_read == 0 {
                *guard = None;
                return Err("faster-whisper worker closed stdout during preload".to_string());
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            if let Ok(parsed) = serde_json::from_str::<FasterWhisperResponse>(trimmed) {
                if parsed.id.as_deref() == Some("preload-runtime") {
                    response = Some(parsed);
                    break;
                }
            }
        }

        let response = response
            .ok_or_else(|| "did not receive faster-whisper preload response".to_string())?;
        if !response.ok {
            return Err(response
                .error
                .unwrap_or_else(|| "faster-whisper preload failed".to_string()));
        }

        let mut preloaded = self
            .preloaded
            .lock()
            .map_err(|_| "failed to acquire faster-whisper preloaded lock".to_string())?;
        *preloaded = true;

        Ok(())
    }
}

impl Transcriber for FasterWhisperSidecarTranscriber {
    fn transcribe(&self, samples: &[f32]) -> Result<String, String> {
        self.transcribe_impl(samples)
    }

    fn set_stream_context(&self, context: Option<&str>) {
        if let Ok(mut guard) = self.context_prompt.lock() {
            *guard = trim_context_prompt(context);
        }
    }

    fn prepare(&self) -> Result<(), String> {
        self.prepare_impl()
    }

    fn engine_label(&self) -> &'static str {
        "faster_whisper"
    }

    fn model_label(&self) -> String {
        self.config.model.clone()
    }

    fn backend_label(&self) -> String {
        self.config.device.clone()
    }
}

impl ParakeetSidecarTranscriber {
    pub fn new(config: ParakeetSidecarConfig) -> Self {
        Self {
            config,
            worker: Arc::new(Mutex::new(None)),
            preloaded: Arc::new(Mutex::new(false)),
        }
    }

    fn transcribe_impl(&self, samples: &[f32]) -> Result<String, String> {
        if samples.is_empty() {
            return Err("cannot transcribe empty audio chunk".to_string());
        }

        self.prepare_impl()?;

        let token = temporary_token();
        let temp_dir = std::env::temp_dir();
        let wav_path = temp_dir.join(format!("sonora-parakeet-{token}.wav"));

        write_wav_file(&wav_path, samples)?;
        let request = ParakeetRequest {
            op: "transcribe".to_string(),
            id: token,
            audio_path: path_to_sidecar_string(&wav_path),
            language: self.config.language.clone(),
            model: self.config.model.clone(),
            device: self.config.device.clone(),
            compute_type: self.config.compute_type.clone(),
        };

        let result = self.send_request(request);
        cleanup_temp_files(&[&wav_path]);
        result
    }

    fn send_request(&self, request: ParakeetRequest) -> Result<String, String> {
        let request_id = request.id.clone();
        let mut guard = self
            .worker
            .lock()
            .map_err(|_| "failed to acquire parakeet worker lock".to_string())?;

        ensure_parakeet_worker(&mut guard, &self.config)?;

        let payload = serde_json::to_string(&request)
            .map_err(|error| format!("failed to serialize parakeet request: {error}"))?;

        let worker = match guard.as_mut() {
            Some(worker) => worker,
            None => return Err("parakeet worker was not initialized".to_string()),
        };

        worker
            .stdin
            .write_all(payload.as_bytes())
            .map_err(|error| format!("failed to write parakeet request: {error}"))?;
        worker
            .stdin
            .write_all(b"\n")
            .map_err(|error| format!("failed to finalize parakeet request: {error}"))?;
        worker
            .stdin
            .flush()
            .map_err(|error| format!("failed to flush parakeet request: {error}"))?;

        let mut response = None;
        let mut non_json_lines = Vec::<String>::new();
        for _ in 0..64 {
            let mut line = String::new();
            let bytes_read = worker
                .stdout
                .read_line(&mut line)
                .map_err(|error| format!("failed to read parakeet response: {error}"))?;

            if bytes_read == 0 {
                *guard = None;
                return Err("parakeet worker closed stdout unexpectedly".to_string());
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            match serde_json::from_str::<ParakeetResponse>(trimmed) {
                Ok(parsed) => {
                    if parsed.id.as_deref() == Some(request_id.as_str()) {
                        response = Some(parsed);
                        break;
                    }
                }
                Err(_) => {
                    if non_json_lines.len() < 3 {
                        non_json_lines.push(trimmed.to_string());
                    }
                }
            }
        }

        let response = response.ok_or_else(|| {
            if non_json_lines.is_empty() {
                "did not receive matching parakeet JSON response".to_string()
            } else {
                format!(
                    "did not receive matching parakeet JSON response (worker output: {})",
                    non_json_lines.join(" | ")
                )
            }
        })?;

        if !response.ok {
            return Err(response
                .error
                .unwrap_or_else(|| "unknown parakeet worker error".to_string()));
        }

        Ok(response.text.unwrap_or_default().trim().to_string())
    }

    fn prepare_impl(&self) -> Result<(), String> {
        {
            let preloaded = self
                .preloaded
                .lock()
                .map_err(|_| "failed to acquire parakeet preloaded lock".to_string())?;
            if *preloaded {
                return Ok(());
            }
        }

        let mut guard = self
            .worker
            .lock()
            .map_err(|_| "failed to acquire parakeet worker lock".to_string())?;
        ensure_parakeet_worker(&mut guard, &self.config)?;

        let preload_request = ParakeetPreloadRequest {
            op: "preload".to_string(),
            id: "preload-runtime".to_string(),
            model: self.config.model.clone(),
            language: self.config.language.clone(),
            device: self.config.device.clone(),
            compute_type: self.config.compute_type.clone(),
        };
        let payload = serde_json::to_string(&preload_request)
            .map_err(|error| format!("failed to serialize parakeet preload request: {error}"))?;

        let worker = match guard.as_mut() {
            Some(worker) => worker,
            None => return Err("parakeet worker was not initialized".to_string()),
        };

        worker
            .stdin
            .write_all(payload.as_bytes())
            .map_err(|error| format!("failed to write parakeet preload request: {error}"))?;
        worker
            .stdin
            .write_all(b"\n")
            .map_err(|error| format!("failed to finalize parakeet preload request: {error}"))?;
        worker
            .stdin
            .flush()
            .map_err(|error| format!("failed to flush parakeet preload request: {error}"))?;

        let mut response = None;
        for _ in 0..64 {
            let mut line = String::new();
            let bytes_read = worker
                .stdout
                .read_line(&mut line)
                .map_err(|error| format!("failed to read parakeet preload response: {error}"))?;

            if bytes_read == 0 {
                *guard = None;
                return Err("parakeet worker closed stdout during preload".to_string());
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            if let Ok(parsed) = serde_json::from_str::<ParakeetResponse>(trimmed) {
                if parsed.id.as_deref() == Some("preload-runtime") {
                    response = Some(parsed);
                    break;
                }
            }
        }

        let response =
            response.ok_or_else(|| "did not receive parakeet preload response".to_string())?;
        if !response.ok {
            return Err(response
                .error
                .unwrap_or_else(|| "parakeet preload failed".to_string()));
        }

        let mut preloaded = self
            .preloaded
            .lock()
            .map_err(|_| "failed to acquire parakeet preloaded lock".to_string())?;
        *preloaded = true;
        Ok(())
    }
}

impl Transcriber for ParakeetSidecarTranscriber {
    fn transcribe(&self, samples: &[f32]) -> Result<String, String> {
        self.transcribe_impl(samples)
    }

    fn prepare(&self) -> Result<(), String> {
        self.prepare_impl()
    }

    fn engine_label(&self) -> &'static str {
        "parakeet"
    }

    fn model_label(&self) -> String {
        self.config.model.clone()
    }

    fn backend_label(&self) -> String {
        self.config.device.clone()
    }
}

#[derive(Debug, Serialize)]
struct FasterWhisperRequest {
    op: String,
    id: String,
    audio_path: String,
    language: String,
    model: String,
    device: String,
    compute_type: String,
    beam_size: u8,
    condition_on_previous_text: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    initial_prompt: Option<String>,
}

#[derive(Debug, Serialize)]
struct FasterWhisperPreloadRequest {
    op: String,
    id: String,
    model: String,
    language: String,
    device: String,
    compute_type: String,
    warmup: bool,
}

#[derive(Debug, Deserialize)]
struct FasterWhisperResponse {
    id: Option<String>,
    ok: bool,
    text: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct ParakeetRequest {
    op: String,
    id: String,
    audio_path: String,
    language: String,
    model: String,
    device: String,
    compute_type: String,
}

#[derive(Debug, Serialize)]
struct ParakeetPreloadRequest {
    op: String,
    id: String,
    model: String,
    language: String,
    device: String,
    compute_type: String,
}

#[derive(Debug, Deserialize)]
struct ParakeetResponse {
    id: Option<String>,
    ok: bool,
    text: Option<String>,
    error: Option<String>,
}

fn ensure_faster_whisper_worker(
    worker: &mut Option<FasterWhisperWorker>,
    config: &FasterWhisperSidecarConfig,
) -> Result<(), String> {
    if worker.is_some() {
        return Ok(());
    }

    let mut command = Command::new(&config.binary_path);
    command
        .arg("--stdio")
        .env(
            "SONORA_FASTER_WHISPER_MODEL_CACHE",
            path_to_sidecar_string(&config.model_cache_dir),
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    let extra_paths = extra_path_entries_from_env(FASTER_WHISPER_EXTRA_PATH_ENV_NAME);
    prepend_process_path(&mut command, &extra_paths);

    #[cfg(target_os = "windows")]
    {
        command.creation_flags(CREATE_NO_WINDOW | BELOW_NORMAL_PRIORITY_CLASS);
    }

    let mut child = command.spawn().map_err(|error| {
        format!(
            "failed to launch faster-whisper worker at '{}': {}",
            config.binary_path.to_string_lossy(),
            error
        )
    })?;

    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| "faster-whisper worker stdin not available".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "faster-whisper worker stdout not available".to_string())?;

    *worker = Some(FasterWhisperWorker {
        _child: child,
        stdin,
        stdout: BufReader::new(stdout),
    });

    Ok(())
}

fn ensure_parakeet_worker(
    worker: &mut Option<ParakeetWorker>,
    config: &ParakeetSidecarConfig,
) -> Result<(), String> {
    if worker.is_some() {
        return Ok(());
    }

    let mut command = Command::new(&config.binary_path);
    command
        .arg("--stdio")
        .env(
            "SONORA_PARAKEET_MODEL_CACHE",
            path_to_sidecar_string(&config.model_cache_dir),
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    #[cfg(target_os = "windows")]
    {
        command.creation_flags(CREATE_NO_WINDOW | BELOW_NORMAL_PRIORITY_CLASS);
    }

    let mut child = command.spawn().map_err(|error| {
        format!(
            "failed to launch parakeet worker at '{}': {}",
            config.binary_path.to_string_lossy(),
            error
        )
    })?;

    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| "parakeet worker stdin not available".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "parakeet worker stdout not available".to_string())?;

    *worker = Some(ParakeetWorker {
        _child: child,
        stdin,
        stdout: BufReader::new(stdout),
    });

    Ok(())
}

#[derive(Debug, Clone)]
pub enum RuntimeTranscriber {
    Stub(StubTranscriber),
    Unavailable { reason: String },
    Whisper(WhisperSidecarTranscriber),
    FasterWhisper(FasterWhisperSidecarTranscriber),
    Parakeet(ParakeetSidecarTranscriber),
}

impl RuntimeTranscriber {
    pub fn description(&self) -> String {
        match self {
            RuntimeTranscriber::Stub(_) => "stub".to_string(),
            RuntimeTranscriber::Unavailable { reason } => {
                format!("unavailable: {reason}")
            }
            RuntimeTranscriber::Whisper(config) => {
                format!(
                    "whisper sidecar ({}, backend {})",
                    config.config.binary_path.to_string_lossy(),
                    config.config.compute_backend.as_label()
                )
            }
            RuntimeTranscriber::FasterWhisper(config) => {
                format!(
                    "faster-whisper sidecar ({}, device {})",
                    config.config.binary_path.to_string_lossy(),
                    config.config.device
                )
            }
            RuntimeTranscriber::Parakeet(config) => {
                format!(
                    "parakeet sidecar ({}, device {})",
                    config.config.binary_path.to_string_lossy(),
                    config.config.device
                )
            }
        }
    }

    pub fn compute_backend_label(&self) -> String {
        match self {
            RuntimeTranscriber::Stub(_) => "stub".to_string(),
            RuntimeTranscriber::Unavailable { .. } => "unavailable".to_string(),
            RuntimeTranscriber::Whisper(runtime) => {
                runtime.config.compute_backend.as_label().to_string()
            }
            RuntimeTranscriber::FasterWhisper(runtime) => runtime.config.device.clone(),
            RuntimeTranscriber::Parakeet(runtime) => runtime.config.device.clone(),
        }
    }

    pub fn uses_gpu(&self) -> bool {
        match self {
            RuntimeTranscriber::Whisper(runtime) => {
                runtime.config.compute_backend == WhisperComputeBackend::Cuda
            }
            RuntimeTranscriber::FasterWhisper(runtime) => runtime.config.device == "cuda",
            RuntimeTranscriber::Parakeet(runtime) => runtime.config.device == "cuda",
            _ => false,
        }
    }

    pub fn active_engine_label(&self) -> &'static str {
        match self {
            RuntimeTranscriber::Whisper(_) => "whisper_cpp",
            RuntimeTranscriber::FasterWhisper(_) => "faster_whisper",
            RuntimeTranscriber::Parakeet(_) => "parakeet",
            RuntimeTranscriber::Stub(_) | RuntimeTranscriber::Unavailable { .. } => "unknown",
        }
    }

    pub fn model_label(&self) -> String {
        match self {
            RuntimeTranscriber::Whisper(runtime) => {
                runtime.config.model_path.to_string_lossy().to_string()
            }
            RuntimeTranscriber::FasterWhisper(runtime) => runtime.config.model.clone(),
            RuntimeTranscriber::Parakeet(runtime) => runtime.config.model.clone(),
            RuntimeTranscriber::Stub(_) => "stub".to_string(),
            RuntimeTranscriber::Unavailable { .. } => "unknown".to_string(),
        }
    }
}

impl Transcriber for RuntimeTranscriber {
    fn transcribe(&self, samples: &[f32]) -> Result<String, String> {
        match self {
            RuntimeTranscriber::Stub(stub) => stub.transcribe(samples),
            RuntimeTranscriber::Unavailable { reason } => Err(reason.clone()),
            RuntimeTranscriber::Whisper(runtime) => runtime.transcribe(samples),
            RuntimeTranscriber::FasterWhisper(runtime) => runtime.transcribe(samples),
            RuntimeTranscriber::Parakeet(runtime) => runtime.transcribe(samples),
        }
    }

    fn set_stream_context(&self, context: Option<&str>) {
        match self {
            RuntimeTranscriber::Whisper(runtime) => runtime.set_stream_context(context),
            RuntimeTranscriber::FasterWhisper(runtime) => runtime.set_stream_context(context),
            RuntimeTranscriber::Parakeet(runtime) => runtime.set_stream_context(context),
            RuntimeTranscriber::Stub(stub) => stub.set_stream_context(context),
            RuntimeTranscriber::Unavailable { .. } => {}
        }
    }

    fn prepare(&self) -> Result<(), String> {
        match self {
            RuntimeTranscriber::Stub(stub) => stub.prepare(),
            RuntimeTranscriber::Unavailable { reason } => Err(reason.clone()),
            RuntimeTranscriber::Whisper(runtime) => runtime.prepare(),
            RuntimeTranscriber::FasterWhisper(runtime) => runtime.prepare(),
            RuntimeTranscriber::Parakeet(runtime) => runtime.prepare(),
        }
    }

    fn engine_label(&self) -> &'static str {
        match self {
            RuntimeTranscriber::Whisper(runtime) => runtime.engine_label(),
            RuntimeTranscriber::FasterWhisper(runtime) => runtime.engine_label(),
            RuntimeTranscriber::Parakeet(runtime) => runtime.engine_label(),
            RuntimeTranscriber::Stub(stub) => stub.engine_label(),
            RuntimeTranscriber::Unavailable { .. } => "unavailable",
        }
    }

    fn model_label(&self) -> String {
        match self {
            RuntimeTranscriber::Whisper(runtime) => runtime.model_label(),
            RuntimeTranscriber::FasterWhisper(runtime) => runtime.model_label(),
            RuntimeTranscriber::Parakeet(runtime) => runtime.model_label(),
            RuntimeTranscriber::Stub(stub) => stub.model_label(),
            RuntimeTranscriber::Unavailable { .. } => "unknown".to_string(),
        }
    }

    fn backend_label(&self) -> String {
        match self {
            RuntimeTranscriber::Whisper(runtime) => runtime.backend_label(),
            RuntimeTranscriber::FasterWhisper(runtime) => runtime.backend_label(),
            RuntimeTranscriber::Parakeet(runtime) => runtime.backend_label(),
            RuntimeTranscriber::Stub(stub) => stub.backend_label(),
            RuntimeTranscriber::Unavailable { .. } => "unavailable".to_string(),
        }
    }
}

pub fn build_runtime_engine(spec: EngineSpec) -> RuntimeEngine {
    match spec.engine {
        SttEngine::WhisperCpp => build_whisper_runtime(spec),
        SttEngine::FasterWhisper => build_faster_whisper_runtime(spec),
        SttEngine::Parakeet => build_parakeet_runtime(spec),
    }
}

pub fn build_runtime_transcriber(
    language: &str,
    model_profile: ModelProfile,
    model_path: PathBuf,
    backend_preference: WhisperBackendPreference,
    resource_dir: Option<&Path>,
) -> RuntimeTranscriber {
    build_runtime_engine(EngineSpec {
        engine: SttEngine::WhisperCpp,
        language: language.to_string(),
        model_profile,
        model_path,
        whisper_backend_preference: backend_preference,
        faster_whisper_compute_type: FasterWhisperComputeType::Auto,
        faster_whisper_beam_size: 1,
        parakeet_compute_type: ParakeetComputeType::Auto,
        resource_dir: resource_dir.map(Path::to_path_buf),
    })
    .transcriber
}

fn build_whisper_runtime(spec: EngineSpec) -> RuntimeEngine {
    let model_exists = spec.model_path.exists();
    let resolved_model_path = spec.model_path.to_string_lossy().to_string();
    let checked_binary_paths = resolve_binary_candidates(spec.resource_dir.as_deref())
        .into_iter()
        .map(|value| value.to_string_lossy().to_string())
        .collect::<Vec<_>>();
    let binary_path = resolve_binary_path(spec.resource_dir.as_deref());

    let transcriber = if !model_exists {
        RuntimeTranscriber::Unavailable {
            reason: format!("model file not found: {resolved_model_path}"),
        }
    } else if let Some(binary_path) = &binary_path {
        let compute_backend = resolve_compute_backend(binary_path, spec.whisper_backend_preference);
        RuntimeTranscriber::Whisper(WhisperSidecarTranscriber {
            config: WhisperSidecarConfig {
                binary_path: binary_path.clone(),
                model_path: spec.model_path,
                language: spec.language,
                compute_backend,
                threads: recommended_threads(spec.model_profile),
            },
        })
    } else {
        RuntimeTranscriber::Unavailable {
            reason: "whisper sidecar binary not found".to_string(),
        }
    };

    RuntimeEngine {
        diagnostics: RuntimeEngineDiagnostics {
            ready: matches!(transcriber, RuntimeTranscriber::Whisper(_)),
            active_engine: "whisper_cpp".to_string(),
            description: transcriber.description(),
            compute_backend: transcriber.compute_backend_label(),
            using_gpu: transcriber.uses_gpu(),
            resolved_binary_path: binary_path.map(|value| value.to_string_lossy().to_string()),
            checked_binary_paths,
            resolved_model_path,
            model_exists,
        },
        transcriber,
    }
}

fn build_faster_whisper_runtime(spec: EngineSpec) -> RuntimeEngine {
    let resolved_model_path = spec.model_path.to_string_lossy().to_string();
    let checked_binary_paths =
        resolve_faster_whisper_binary_candidates(spec.resource_dir.as_deref())
            .into_iter()
            .map(|value| value.to_string_lossy().to_string())
            .collect::<Vec<_>>();
    let binary_path = resolve_faster_whisper_binary_path(spec.resource_dir.as_deref());
    let model_exists = is_resolvable_faster_whisper_model(&resolved_model_path);
    let resolved_model_reference = normalize_path_for_sidecar(&resolved_model_path);
    let cuda_runtime_ready = faster_whisper_cuda_runtime_ready();
    let device = resolve_faster_whisper_device(spec.whisper_backend_preference, cuda_runtime_ready)
        .to_string();
    let compute_type =
        resolve_faster_whisper_compute_type(device.as_str(), spec.faster_whisper_compute_type)
            .to_string();
    let model_cache_dir = resolve_faster_whisper_model_cache_dir(spec.resource_dir.as_deref());

    let transcriber = if !model_exists {
        RuntimeTranscriber::Unavailable {
            reason: format!("faster-whisper model target not found: {resolved_model_path}"),
        }
    } else if spec.whisper_backend_preference == WhisperBackendPreference::Cuda
        && !cuda_runtime_ready
    {
        RuntimeTranscriber::Unavailable {
            reason: "CUDA backend requested for faster-whisper, but CUDA runtime libraries were not found (missing cublas64_12.dll). Install CUDA runtime or switch backend to auto/cpu.".to_string(),
        }
    } else if let Some(binary_path) = &binary_path {
        RuntimeTranscriber::FasterWhisper(FasterWhisperSidecarTranscriber::new(
            FasterWhisperSidecarConfig {
                binary_path: binary_path.clone(),
                model: resolved_model_reference,
                model_cache_dir,
                language: spec.language,
                device: device.clone(),
                compute_type,
                beam_size: spec.faster_whisper_beam_size.clamp(1, 8),
                condition_on_previous_text: true,
            },
        ))
    } else {
        RuntimeTranscriber::Unavailable {
            reason:
                "faster-whisper worker binary not found (run pnpm sidecar:setup:faster-whisper)"
                    .to_string(),
        }
    };

    RuntimeEngine {
        diagnostics: RuntimeEngineDiagnostics {
            ready: matches!(transcriber, RuntimeTranscriber::FasterWhisper(_)),
            active_engine: "faster_whisper".to_string(),
            description: transcriber.description(),
            compute_backend: device,
            using_gpu: transcriber.uses_gpu(),
            resolved_binary_path: binary_path.map(|value| value.to_string_lossy().to_string()),
            checked_binary_paths,
            resolved_model_path,
            model_exists,
        },
        transcriber,
    }
}

fn build_parakeet_runtime(spec: EngineSpec) -> RuntimeEngine {
    let resolved_model_path = spec.model_path.to_string_lossy().to_string();
    let checked_binary_paths = resolve_parakeet_binary_candidates(spec.resource_dir.as_deref())
        .into_iter()
        .map(|value| value.to_string_lossy().to_string())
        .collect::<Vec<_>>();
    let binary_path = resolve_parakeet_binary_path(spec.resource_dir.as_deref());
    let model_exists = is_resolvable_parakeet_model(&resolved_model_path);
    let resolved_model_reference = normalize_path_for_sidecar(&resolved_model_path);
    let device = resolve_parakeet_device(spec.whisper_backend_preference).to_string();
    let compute_type =
        resolve_parakeet_compute_type(device.as_str(), spec.parakeet_compute_type).to_string();
    let model_cache_dir = resolve_parakeet_model_cache_dir(spec.resource_dir.as_deref());

    let transcriber = if !model_exists {
        RuntimeTranscriber::Unavailable {
            reason: format!("parakeet model target not found: {resolved_model_path}"),
        }
    } else if !is_transformers_parakeet_model_supported(&resolved_model_reference) {
        RuntimeTranscriber::Unavailable {
            reason: "parakeet model is not supported by the current Transformers sidecar (TDT/RNNT requires a NeMo-based worker). Use nvidia/parakeet-ctc-* models for now.".to_string(),
        }
    } else if spec.whisper_backend_preference == WhisperBackendPreference::Cuda && device != "cuda"
    {
        RuntimeTranscriber::Unavailable {
            reason: "CUDA backend requested for parakeet, but no NVIDIA GPU was detected. Switch backend to auto/cpu or verify your GPU setup.".to_string(),
        }
    } else if let Some(binary_path) = &binary_path {
        RuntimeTranscriber::Parakeet(ParakeetSidecarTranscriber::new(ParakeetSidecarConfig {
            binary_path: binary_path.clone(),
            model: resolved_model_reference,
            model_cache_dir,
            language: spec.language,
            device: device.clone(),
            compute_type,
        }))
    } else {
        RuntimeTranscriber::Unavailable {
            reason: "parakeet worker binary not found (run pnpm sidecar:setup:parakeet)"
                .to_string(),
        }
    };

    RuntimeEngine {
        diagnostics: RuntimeEngineDiagnostics {
            ready: matches!(transcriber, RuntimeTranscriber::Parakeet(_)),
            active_engine: "parakeet".to_string(),
            description: transcriber.description(),
            compute_backend: device,
            using_gpu: transcriber.uses_gpu(),
            resolved_binary_path: binary_path.map(|value| value.to_string_lossy().to_string()),
            checked_binary_paths,
            resolved_model_path,
            model_exists,
        },
        transcriber,
    }
}

pub fn default_faster_whisper_model(profile: ModelProfile) -> &'static str {
    match profile {
        ModelProfile::Fast => FASTER_WHISPER_DEFAULT_MODEL_FAST,
        ModelProfile::Balanced => FASTER_WHISPER_DEFAULT_MODEL_BALANCED,
    }
}

pub fn default_parakeet_model(profile: ModelProfile) -> &'static str {
    match profile {
        ModelProfile::Fast => PARAKEET_DEFAULT_MODEL_FAST,
        ModelProfile::Balanced => PARAKEET_DEFAULT_MODEL_BALANCED,
    }
}

fn resolve_faster_whisper_device(
    preference: WhisperBackendPreference,
    cuda_runtime_ready: bool,
) -> &'static str {
    match parse_backend_preference(std::env::var(BACKEND_ENV_NAME).ok().as_deref())
        .unwrap_or(preference)
    {
        WhisperBackendPreference::Cpu => "cpu",
        WhisperBackendPreference::Cuda => "cuda",
        WhisperBackendPreference::Auto => {
            if has_nvidia_gpu() && cuda_runtime_ready {
                "cuda"
            } else {
                "cpu"
            }
        }
    }
}

fn faster_whisper_cuda_runtime_ready() -> bool {
    #[cfg(target_os = "windows")]
    {
        let from_override_paths = extra_path_entries_from_env(FASTER_WHISPER_EXTRA_PATH_ENV_NAME)
            .into_iter()
            .any(|path| path.join("cublas64_12.dll").exists());
        if from_override_paths {
            return true;
        }

        if let Ok(cuda_path) = std::env::var("CUDA_PATH") {
            let candidate = PathBuf::from(cuda_path).join("bin").join("cublas64_12.dll");
            if candidate.exists() {
                return true;
            }
        }

        let output = Command::new("where").arg("cublas64_12.dll").output();
        return output
            .map(|result| result.status.success())
            .unwrap_or(false);
    }

    #[cfg(not(target_os = "windows"))]
    {
        true
    }
}

fn resolve_faster_whisper_compute_type(
    device: &str,
    preference: FasterWhisperComputeType,
) -> &'static str {
    match preference {
        FasterWhisperComputeType::Auto => {
            if device == "cuda" {
                "float16"
            } else {
                "int8"
            }
        }
        FasterWhisperComputeType::Int8 => "int8",
        FasterWhisperComputeType::Float16 => "float16",
        FasterWhisperComputeType::Float32 => "float32",
    }
}

fn resolve_parakeet_device(preference: WhisperBackendPreference) -> &'static str {
    match parse_backend_preference(std::env::var(BACKEND_ENV_NAME).ok().as_deref())
        .unwrap_or(preference)
    {
        WhisperBackendPreference::Cpu => "cpu",
        WhisperBackendPreference::Cuda => {
            if has_nvidia_gpu() {
                "cuda"
            } else {
                "cpu"
            }
        }
        WhisperBackendPreference::Auto => {
            if has_nvidia_gpu() {
                "cuda"
            } else {
                "cpu"
            }
        }
    }
}

fn resolve_parakeet_compute_type(device: &str, preference: ParakeetComputeType) -> &'static str {
    match preference {
        ParakeetComputeType::Auto => {
            if device == "cuda" {
                "float16"
            } else {
                "float32"
            }
        }
        ParakeetComputeType::Float16 => {
            if device == "cuda" {
                "float16"
            } else {
                "float32"
            }
        }
        ParakeetComputeType::Float32 => "float32",
    }
}

fn resolve_faster_whisper_model_cache_dir(resource_dir: Option<&Path>) -> PathBuf {
    let mut candidates = Vec::<PathBuf>::new();

    if let Some(resources) = resource_dir {
        candidates.push(resources.join("models").join("faster-whisper-cache"));
        candidates.push(
            resources
                .join("resources")
                .join("models")
                .join("faster-whisper-cache"),
        );
        candidates.push(resources.join("faster-whisper-cache"));
    }
    candidates.push(
        PathBuf::from("src-tauri")
            .join("resources")
            .join("models")
            .join("faster-whisper-cache"),
    );

    for candidate in candidates {
        if candidate.exists() {
            return candidate;
        }
    }

    dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("sonora-dictation")
        .join("faster-whisper-cache")
}

fn resolve_parakeet_model_cache_dir(resource_dir: Option<&Path>) -> PathBuf {
    let mut candidates = Vec::<PathBuf>::new();

    if let Some(resources) = resource_dir {
        candidates.push(resources.join("models").join("parakeet-cache"));
        candidates.push(
            resources
                .join("resources")
                .join("models")
                .join("parakeet-cache"),
        );
        candidates.push(resources.join("parakeet-cache"));
    }
    candidates.push(
        PathBuf::from("src-tauri")
            .join("resources")
            .join("models")
            .join("parakeet-cache"),
    );

    for candidate in candidates {
        if candidate.exists() {
            return candidate;
        }
    }

    dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("sonora-dictation")
        .join("parakeet-cache")
}

fn is_resolvable_faster_whisper_model(model: &str) -> bool {
    let normalized = model.trim();
    if normalized.is_empty() {
        return false;
    }

    let as_path = Path::new(normalized);
    if as_path.exists() {
        return true;
    }

    is_known_faster_whisper_model_name(normalized)
        || normalized.starts_with("Systran/")
        || normalized.starts_with("openai/")
}

fn is_resolvable_parakeet_model(model: &str) -> bool {
    let normalized = model.trim();
    if normalized.is_empty() {
        return false;
    }

    let as_path = Path::new(normalized);
    if as_path.exists() {
        return true;
    }

    is_known_parakeet_model_name(normalized)
        || normalized.starts_with("nvidia/")
        || normalized.starts_with("NVIDIA/")
}

fn is_known_faster_whisper_model_name(name: &str) -> bool {
    matches!(
        name,
        "tiny"
            | "tiny.en"
            | "base"
            | "base.en"
            | "small"
            | "small.en"
            | "medium"
            | "medium.en"
            | "large-v1"
            | "large-v2"
            | "large-v3"
            | "distil-large-v2"
            | "distil-large-v3"
            | "distil-medium.en"
    )
}

fn is_known_parakeet_model_name(name: &str) -> bool {
    matches!(
        name,
        "nvidia/parakeet-ctc-0.6b"
            | "nvidia/parakeet-ctc-1.1b"
            | "nvidia/parakeet-ctc-0.6b-vietnamese"
            | "nvidia/parakeet-tdt-0.6b-v3"
    )
}

fn is_transformers_parakeet_model_supported(model: &str) -> bool {
    let normalized = model.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return false;
    }

    if Path::new(model).exists() {
        return true;
    }

    if normalized.starts_with("nvidia/parakeet-tdt-")
        || normalized.starts_with("nvidia/parakeet-rnnt-")
    {
        return false;
    }

    true
}

fn recommended_threads(profile: ModelProfile) -> usize {
    let logical = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(4);

    match profile {
        ModelProfile::Fast => logical.clamp(2, 6),
        ModelProfile::Balanced => logical.clamp(4, 8),
    }
}

fn resolve_compute_backend(
    binary_path: &Path,
    backend_preference: WhisperBackendPreference,
) -> WhisperComputeBackend {
    match parse_backend_preference(std::env::var(BACKEND_ENV_NAME).ok().as_deref())
        .unwrap_or(backend_preference)
    {
        WhisperBackendPreference::Cpu => WhisperComputeBackend::Cpu,
        WhisperBackendPreference::Cuda => WhisperComputeBackend::Cuda,
        WhisperBackendPreference::Auto => {
            if let Some(metadata_backend) = read_metadata_backend(binary_path) {
                metadata_backend
            } else if has_nvidia_gpu() {
                WhisperComputeBackend::Cuda
            } else {
                WhisperComputeBackend::Cpu
            }
        }
    }
}

fn parse_backend_preference(value: Option<&str>) -> Option<WhisperBackendPreference> {
    let normalized = value?.trim().to_ascii_lowercase();

    match normalized.as_str() {
        "" => None,
        "auto" => Some(WhisperBackendPreference::Auto),
        "cpu" => Some(WhisperBackendPreference::Cpu),
        "cuda" | "gpu" | "nvidia" => Some(WhisperBackendPreference::Cuda),
        _ => None,
    }
}

fn metadata_path_for_binary(binary_path: &Path) -> Option<PathBuf> {
    binary_path
        .parent()
        .map(|parent| parent.join(SIDECAR_METADATA_FILE_NAME))
}

fn read_metadata_backend(binary_path: &Path) -> Option<WhisperComputeBackend> {
    let metadata_path = metadata_path_for_binary(binary_path)?;
    let raw = fs::read_to_string(metadata_path).ok()?;
    let parsed = serde_json::from_str::<WhisperSidecarMetadata>(&raw).ok()?;
    parse_backend_preference(parsed.backend.as_deref()).map(map_preference_to_compute_backend)
}

fn map_preference_to_compute_backend(
    preference: WhisperBackendPreference,
) -> WhisperComputeBackend {
    match preference {
        WhisperBackendPreference::Cuda => WhisperComputeBackend::Cuda,
        WhisperBackendPreference::Cpu | WhisperBackendPreference::Auto => {
            WhisperComputeBackend::Cpu
        }
    }
}

fn has_nvidia_gpu() -> bool {
    let output = Command::new("nvidia-smi").arg("-L").output();
    output
        .map(|result| result.status.success())
        .unwrap_or(false)
}

fn resolve_faster_whisper_binary_candidates(resource_dir: Option<&Path>) -> Vec<PathBuf> {
    let binary_name = default_faster_whisper_binary_name();
    let mut candidates = Vec::<PathBuf>::new();

    if let Ok(override_path) = std::env::var(FASTER_WHISPER_BIN_ENV_NAME) {
        let normalized = override_path.trim();
        if !normalized.is_empty() {
            candidates.push(PathBuf::from(normalized));
        }
    }

    candidates.push(PathBuf::from("src-tauri/resources/bin").join(binary_name));
    candidates.push(PathBuf::from("resources/bin").join(binary_name));

    if let Some(resources) = resource_dir {
        candidates.push(resources.join("bin").join(binary_name));
        candidates.push(resources.join("resources").join("bin").join(binary_name));
        candidates.push(resources.join(binary_name));
    }

    candidates.push(PathBuf::from(binary_name));
    dedupe_paths(candidates)
}

fn resolve_parakeet_binary_candidates(resource_dir: Option<&Path>) -> Vec<PathBuf> {
    let binary_name = default_parakeet_binary_name();
    let mut candidates = Vec::<PathBuf>::new();

    if let Ok(override_path) = std::env::var(PARAKEET_BIN_ENV_NAME) {
        let normalized = override_path.trim();
        if !normalized.is_empty() {
            candidates.push(PathBuf::from(normalized));
        }
    }

    candidates.push(PathBuf::from("src-tauri/resources/bin").join(binary_name));
    candidates.push(PathBuf::from("resources/bin").join(binary_name));

    if let Some(resources) = resource_dir {
        candidates.push(resources.join("bin").join(binary_name));
        candidates.push(resources.join("resources").join("bin").join(binary_name));
        candidates.push(resources.join(binary_name));
    }

    candidates.push(PathBuf::from(binary_name));
    dedupe_paths(candidates)
}

fn resolve_faster_whisper_binary_path(resource_dir: Option<&Path>) -> Option<PathBuf> {
    let candidates = resolve_faster_whisper_binary_candidates(resource_dir);
    for candidate in &candidates {
        if candidate.components().count() == 1 {
            return Some(candidate.clone());
        }
        if candidate.exists() {
            return Some(candidate.clone());
        }
    }

    None
}

fn resolve_parakeet_binary_path(resource_dir: Option<&Path>) -> Option<PathBuf> {
    let candidates = resolve_parakeet_binary_candidates(resource_dir);
    for candidate in &candidates {
        if candidate.components().count() == 1 {
            return Some(candidate.clone());
        }
        if candidate.exists() {
            return Some(candidate.clone());
        }
    }

    None
}

pub fn resolve_binary_candidates(resource_dir: Option<&Path>) -> Vec<PathBuf> {
    let binary_name = default_binary_name();
    let mut candidates = Vec::<PathBuf>::new();

    if let Ok(override_path) = std::env::var("SONORA_WHISPER_BIN") {
        let normalized = override_path.trim();
        if !normalized.is_empty() {
            candidates.push(PathBuf::from(normalized));
        }
    }

    candidates.push(PathBuf::from("src-tauri/resources/bin").join(binary_name));
    candidates.push(PathBuf::from("resources/bin").join(binary_name));

    if let Some(resources) = resource_dir {
        candidates.push(resources.join("bin").join(binary_name));
        candidates.push(resources.join("resources").join("bin").join(binary_name));
        candidates.push(resources.join(binary_name));
    }

    candidates.push(PathBuf::from(binary_name));
    dedupe_paths(candidates)
}

pub fn resolve_binary_path(resource_dir: Option<&Path>) -> Option<PathBuf> {
    let candidates = resolve_binary_candidates(resource_dir);

    for candidate in &candidates {
        if candidate.components().count() == 1 {
            return Some(candidate.clone());
        }
        if candidate.exists() {
            return Some(candidate.clone());
        }
    }

    None
}

fn default_binary_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "whisper-cli.exe"
    } else {
        "whisper-cli"
    }
}

fn default_faster_whisper_binary_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "faster-whisper-worker.exe"
    } else {
        "faster-whisper-worker"
    }
}

fn default_parakeet_binary_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "parakeet-worker.exe"
    } else {
        "parakeet-worker"
    }
}

fn extra_path_entries_from_env(var_name: &str) -> Vec<PathBuf> {
    let separator = if cfg!(target_os = "windows") {
        ';'
    } else {
        ':'
    };
    std::env::var(var_name)
        .ok()
        .map(|raw| {
            raw.split(separator)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn prepend_process_path(command: &mut Command, extra_entries: &[PathBuf]) {
    if extra_entries.is_empty() {
        return;
    }

    let mut merged = extra_entries.to_vec();
    if let Some(existing) = std::env::var_os("PATH") {
        merged.extend(std::env::split_paths(&existing));
    }

    if let Ok(joined) = std::env::join_paths(merged) {
        command.env("PATH", joined);
    }
}

fn path_to_sidecar_string(path: &Path) -> String {
    normalize_path_for_sidecar(&path.to_string_lossy())
}

#[cfg(target_os = "windows")]
fn normalize_path_for_sidecar(raw: &str) -> String {
    if let Some(trimmed) = raw.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{trimmed}");
    }
    if let Some(trimmed) = raw.strip_prefix(r"\\?\") {
        return trimmed.to_string();
    }
    raw.to_string()
}

#[cfg(not(target_os = "windows"))]
fn normalize_path_for_sidecar(raw: &str) -> String {
    raw.to_string()
}

fn dedupe_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = HashSet::<String>::new();
    paths
        .into_iter()
        .filter(|path| {
            let key = path.to_string_lossy().to_string();
            seen.insert(key)
        })
        .collect()
}

fn trim_context_prompt(context: Option<&str>) -> Option<String> {
    const MAX_CONTEXT_CHARS: usize = 220;

    let normalized = context
        .map(|value| value.split_whitespace().collect::<Vec<_>>().join(" "))
        .unwrap_or_default();
    if normalized.is_empty() {
        return None;
    }

    if normalized.len() <= MAX_CONTEXT_CHARS {
        return Some(normalized);
    }

    let start = normalized.len().saturating_sub(MAX_CONTEXT_CHARS);
    let trimmed = normalized[start..].trim_start().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn temporary_token() -> String {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    format!("{}-{stamp}", std::process::id())
}

fn write_wav_file(path: &Path, samples: &[f32]) -> Result<(), String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = hound::WavWriter::create(path, spec)
        .map_err(|error| format!("failed to create wav file: {}", error))?;

    for sample in samples {
        let clamped = sample.clamp(-1.0, 1.0);
        let converted = (clamped * i16::MAX as f32) as i16;
        writer
            .write_sample(converted)
            .map_err(|error| format!("failed to write wav sample: {}", error))?;
    }

    writer
        .finalize()
        .map_err(|error| format!("failed to finalize wav file: {}", error))
}

fn cleanup_temp_files(paths: &[&Path]) {
    for path in paths {
        let _ = fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_transcriber_returns_text() {
        let text = StubTranscriber
            .transcribe(&vec![0.0; 2048])
            .expect("stub transcriber should return text");
        assert!(!text.is_empty());
    }

    #[test]
    fn builds_whisper_command_args() {
        let config = WhisperSidecarConfig {
            binary_path: PathBuf::from("./bin/whisper"),
            model_path: PathBuf::from("./models/ggml-base.en-q5_1.bin"),
            language: "en".to_string(),
            threads: 2,
            compute_backend: WhisperComputeBackend::Cpu,
        };
        let args = config.command_args(Path::new("./tmp/chunk.wav"), Path::new("./tmp/out"));

        assert!(args.iter().any(|arg| arg == "-m"));
        assert!(args.iter().any(|arg| arg == "-f"));
        assert!(args.iter().any(|arg| arg == "-l"));
        assert!(args.iter().any(|arg| arg == "-t"));
        assert!(args.iter().any(|arg| arg == "-np"));
        assert!(args.iter().any(|arg| arg == "-otxt"));
        assert!(args.iter().any(|arg| arg == "-of"));
        assert!(args.iter().any(|arg| arg == "en"));
        assert!(args.iter().any(|arg| arg == "-ng"));
    }

    #[test]
    fn whisper_command_args_do_not_disable_gpu_for_cuda_backend() {
        let config = WhisperSidecarConfig {
            binary_path: PathBuf::from("./bin/whisper"),
            model_path: PathBuf::from("./models/ggml-base.en-q5_1.bin"),
            language: "en".to_string(),
            threads: 6,
            compute_backend: WhisperComputeBackend::Cuda,
        };

        let args = config.command_args(Path::new("./tmp/chunk.wav"), Path::new("./tmp/out"));
        assert!(!args.iter().any(|arg| arg == "-ng"));
    }

    #[test]
    fn parses_backend_preference_variants() {
        assert_eq!(
            parse_backend_preference(Some("cuda")),
            Some(WhisperBackendPreference::Cuda)
        );
        assert_eq!(
            parse_backend_preference(Some("NVIDIA")),
            Some(WhisperBackendPreference::Cuda)
        );
        assert_eq!(
            parse_backend_preference(Some("cpu")),
            Some(WhisperBackendPreference::Cpu)
        );
        assert_eq!(
            parse_backend_preference(Some("auto")),
            Some(WhisperBackendPreference::Auto)
        );
        assert_eq!(parse_backend_preference(Some("")), None);
        assert_eq!(parse_backend_preference(Some("unknown")), None);
    }

    #[test]
    fn reads_sidecar_metadata_backend_hint() {
        let token = temporary_token();
        let dir = std::env::temp_dir().join(format!("sonora-sidecar-meta-{token}"));
        fs::create_dir_all(&dir).expect("temp metadata directory should be created");

        let binary = dir.join("whisper-cli");
        fs::write(&binary, "").expect("binary placeholder should be created");
        let metadata = dir.join(SIDECAR_METADATA_FILE_NAME);
        fs::write(&metadata, "{\"backend\":\"cuda\"}\n").expect("metadata file should be created");

        let backend = read_metadata_backend(&binary);
        assert_eq!(backend, Some(WhisperComputeBackend::Cuda));

        let _ = fs::remove_file(metadata);
        let _ = fs::remove_file(binary);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn maps_explicit_backend_preferences_without_auto_detection() {
        let cpu = map_preference_to_compute_backend(WhisperBackendPreference::Cpu);
        let cuda = map_preference_to_compute_backend(WhisperBackendPreference::Cuda);

        assert_eq!(cpu, WhisperComputeBackend::Cpu);
        assert_eq!(cuda, WhisperComputeBackend::Cuda);
    }

    #[test]
    fn resolves_runtime_transcriber_as_unavailable_for_missing_model() {
        let transcriber = build_runtime_transcriber(
            "en",
            ModelProfile::Balanced,
            PathBuf::from("./missing.bin"),
            WhisperBackendPreference::Auto,
            Some(Path::new("/tmp/resources")),
        );

        match transcriber {
            RuntimeTranscriber::Unavailable { reason } => {
                assert!(reason.contains("model file not found"));
            }
            _ => panic!("expected unavailable transcriber"),
        }
    }

    #[test]
    fn binary_candidates_include_path_binary_name() {
        let candidates = resolve_binary_candidates(None);
        let expected_name = if cfg!(target_os = "windows") {
            "whisper-cli.exe"
        } else {
            "whisper-cli"
        };

        assert!(candidates
            .iter()
            .any(|path| path == &PathBuf::from(expected_name)));
    }

    #[test]
    fn faster_whisper_runtime_reports_unavailable_engine() {
        let runtime = build_runtime_engine(EngineSpec {
            engine: SttEngine::FasterWhisper,
            language: "en".to_string(),
            model_profile: ModelProfile::Balanced,
            model_path: PathBuf::from("./missing-faster-model"),
            whisper_backend_preference: WhisperBackendPreference::Auto,
            faster_whisper_compute_type: FasterWhisperComputeType::Auto,
            faster_whisper_beam_size: 1,
            parakeet_compute_type: ParakeetComputeType::Auto,
            resource_dir: None,
        });

        assert!(!runtime.diagnostics.ready);
        assert_eq!(runtime.diagnostics.active_engine, "faster_whisper");
        assert!(runtime
            .diagnostics
            .description
            .contains("faster-whisper model target not found"));
    }

    #[test]
    fn faster_whisper_defaults_are_profile_aware() {
        assert_eq!(default_faster_whisper_model(ModelProfile::Fast), "tiny.en");
        assert_eq!(
            default_faster_whisper_model(ModelProfile::Balanced),
            "small.en"
        );
    }

    #[test]
    fn faster_whisper_binary_candidates_include_path_binary_name() {
        let candidates = resolve_faster_whisper_binary_candidates(None);
        let expected_name = if cfg!(target_os = "windows") {
            "faster-whisper-worker.exe"
        } else {
            "faster-whisper-worker"
        };

        assert!(candidates
            .iter()
            .any(|path| path == &PathBuf::from(expected_name)));
    }

    #[test]
    fn parakeet_runtime_reports_unavailable_engine() {
        let runtime = build_runtime_engine(EngineSpec {
            engine: SttEngine::Parakeet,
            language: "en".to_string(),
            model_profile: ModelProfile::Balanced,
            model_path: PathBuf::from("./missing-parakeet-model"),
            whisper_backend_preference: WhisperBackendPreference::Auto,
            faster_whisper_compute_type: FasterWhisperComputeType::Auto,
            faster_whisper_beam_size: 1,
            parakeet_compute_type: ParakeetComputeType::Auto,
            resource_dir: None,
        });

        assert!(!runtime.diagnostics.ready);
        assert_eq!(runtime.diagnostics.active_engine, "parakeet");
        assert!(runtime
            .diagnostics
            .description
            .contains("parakeet model target not found"));
    }

    #[test]
    fn parakeet_tdt_model_reports_transformers_unsupported() {
        let runtime = build_runtime_engine(EngineSpec {
            engine: SttEngine::Parakeet,
            language: "en".to_string(),
            model_profile: ModelProfile::Balanced,
            model_path: PathBuf::from("nvidia/parakeet-tdt-0.6b-v3"),
            whisper_backend_preference: WhisperBackendPreference::Auto,
            faster_whisper_compute_type: FasterWhisperComputeType::Auto,
            faster_whisper_beam_size: 1,
            parakeet_compute_type: ParakeetComputeType::Auto,
            resource_dir: None,
        });

        assert!(!runtime.diagnostics.ready);
        assert_eq!(runtime.diagnostics.active_engine, "parakeet");
        assert!(runtime
            .diagnostics
            .description
            .contains("requires a NeMo-based worker"));
    }

    #[test]
    fn parakeet_defaults_are_profile_aware() {
        assert_eq!(
            default_parakeet_model(ModelProfile::Fast),
            "nvidia/parakeet-ctc-0.6b"
        );
        assert_eq!(
            default_parakeet_model(ModelProfile::Balanced),
            "nvidia/parakeet-ctc-1.1b"
        );
    }

    #[test]
    fn parakeet_binary_candidates_include_path_binary_name() {
        let candidates = resolve_parakeet_binary_candidates(None);
        let expected_name = if cfg!(target_os = "windows") {
            "parakeet-worker.exe"
        } else {
            "parakeet-worker"
        };

        assert!(candidates
            .iter()
            .any(|path| path == &PathBuf::from(expected_name)));
    }
}
