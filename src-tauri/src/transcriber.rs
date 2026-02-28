use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

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
}

impl WhisperSidecarConfig {
    pub fn command_args(&self, audio_file: &Path, output_prefix: &Path) -> Vec<String> {
        vec![
            "-m".to_string(),
            self.model_path.to_string_lossy().to_string(),
            "-f".to_string(),
            audio_file.to_string_lossy().to_string(),
            "-l".to_string(),
            self.language.clone(),
            "--no-timestamps".to_string(),
            "-otxt".to_string(),
            "-of".to_string(),
            output_prefix.to_string_lossy().to_string(),
        ]
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
        let output = Command::new(&self.config.binary_path)
            .args(args)
            .output()
            .map_err(|error| {
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
            RuntimeTranscriber::Whisper(config) => format!(
                "whisper sidecar ({})",
                config.config.binary_path.to_string_lossy()
            ),
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

    RuntimeTranscriber::Whisper(WhisperSidecarTranscriber {
        config: WhisperSidecarConfig {
            binary_path,
            model_path,
            language: language.to_string(),
        },
    })
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
        };
        let args = config.command_args(Path::new("./tmp/chunk.wav"), Path::new("./tmp/out"));

        assert!(args.iter().any(|arg| arg == "-m"));
        assert!(args.iter().any(|arg| arg == "-f"));
        assert!(args.iter().any(|arg| arg == "-l"));
        assert!(args.iter().any(|arg| arg == "-otxt"));
        assert!(args.iter().any(|arg| arg == "-of"));
        assert!(args.iter().any(|arg| arg == "en"));
    }

    #[test]
    fn resolves_runtime_transcriber_as_unavailable_for_missing_model() {
        let transcriber = build_runtime_transcriber(
            "en",
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
