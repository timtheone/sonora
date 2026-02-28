use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::ModelProfile;
use serde::Deserialize;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;
#[cfg(target_os = "windows")]
const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x00004000;

pub trait Transcriber: Send + Sync {
    fn transcribe(&self, samples: &[f32]) -> Result<String, String>;
}

#[derive(Debug, Clone, Default)]
pub struct StubTranscriber;

impl Transcriber for StubTranscriber {
    fn transcribe(&self, _samples: &[f32]) -> Result<String, String> {
        Ok("phase-1 transcript".to_string())
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WhisperBackendPreference {
    Auto,
    Cpu,
    Cuda,
}

#[derive(Debug, Deserialize)]
struct WhisperSidecarMetadata {
    backend: Option<String>,
}

const SIDECAR_METADATA_FILE_NAME: &str = "whisper-sidecar.json";
const BACKEND_ENV_NAME: &str = "SONORA_WHISPER_BACKEND";

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
}

#[derive(Debug, Clone)]
pub enum RuntimeTranscriber {
    Stub(StubTranscriber),
    Unavailable { reason: String },
    Whisper(WhisperSidecarTranscriber),
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
        }
    }

    pub fn compute_backend_label(&self) -> String {
        match self {
            RuntimeTranscriber::Stub(_) => "stub".to_string(),
            RuntimeTranscriber::Unavailable { .. } => "unavailable".to_string(),
            RuntimeTranscriber::Whisper(runtime) => {
                runtime.config.compute_backend.as_label().to_string()
            }
        }
    }

    pub fn uses_gpu(&self) -> bool {
        match self {
            RuntimeTranscriber::Whisper(runtime) => {
                runtime.config.compute_backend == WhisperComputeBackend::Cuda
            }
            _ => false,
        }
    }
}

impl Transcriber for RuntimeTranscriber {
    fn transcribe(&self, samples: &[f32]) -> Result<String, String> {
        match self {
            RuntimeTranscriber::Stub(stub) => stub.transcribe(samples),
            RuntimeTranscriber::Unavailable { reason } => Err(reason.clone()),
            RuntimeTranscriber::Whisper(runtime) => runtime.transcribe(samples),
        }
    }
}

pub fn build_runtime_transcriber(
    language: &str,
    model_profile: ModelProfile,
    model_path: PathBuf,
    resource_dir: Option<&Path>,
) -> RuntimeTranscriber {
    let binary = resolve_binary_path(resource_dir);

    if !model_path.exists() {
        return RuntimeTranscriber::Unavailable {
            reason: format!("model file not found: {}", model_path.to_string_lossy()),
        };
    }

    let Some(binary_path) = binary else {
        return RuntimeTranscriber::Unavailable {
            reason: "whisper sidecar binary not found".to_string(),
        };
    };

    let compute_backend = resolve_compute_backend(&binary_path);

    RuntimeTranscriber::Whisper(WhisperSidecarTranscriber {
        config: WhisperSidecarConfig {
            binary_path,
            model_path,
            language: language.to_string(),
            compute_backend,
            threads: recommended_threads(model_profile),
        },
    })
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

fn resolve_compute_backend(binary_path: &Path) -> WhisperComputeBackend {
    match parse_backend_preference(std::env::var(BACKEND_ENV_NAME).ok().as_deref()) {
        Some(WhisperBackendPreference::Cpu) => WhisperComputeBackend::Cpu,
        Some(WhisperBackendPreference::Cuda) => WhisperComputeBackend::Cuda,
        Some(WhisperBackendPreference::Auto) | None => {
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
    parse_backend_preference(parsed.backend.as_deref()).map(|preference| match preference {
        WhisperBackendPreference::Cuda => WhisperComputeBackend::Cuda,
        WhisperBackendPreference::Cpu | WhisperBackendPreference::Auto => {
            WhisperComputeBackend::Cpu
        }
    })
}

fn has_nvidia_gpu() -> bool {
    let output = Command::new("nvidia-smi").arg("-L").output();
    output
        .map(|result| result.status.success())
        .unwrap_or(false)
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
    fn resolves_runtime_transcriber_as_unavailable_for_missing_model() {
        let transcriber = build_runtime_transcriber(
            "en",
            ModelProfile::Balanced,
            PathBuf::from("./missing.bin"),
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
}
