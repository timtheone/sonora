use std::path::{Path, PathBuf};

pub trait Transcriber: Send + Sync {
    fn transcribe(&self, _samples: &[f32]) -> Result<String, String>;
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
    pub fn command_args(&self, audio_file: &Path) -> Vec<String> {
        vec![
            "-m".to_string(),
            self.model_path.to_string_lossy().to_string(),
            "-f".to_string(),
            audio_file.to_string_lossy().to_string(),
            "-l".to_string(),
            self.language.clone(),
            "--no-timestamps".to_string(),
        ]
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
        let args = config.command_args(Path::new("./tmp/chunk.wav"));

        assert!(args.iter().any(|arg| arg == "-m"));
        assert!(args.iter().any(|arg| arg == "-f"));
        assert!(args.iter().any(|arg| arg == "-l"));
        assert!(args.iter().any(|arg| arg == "en"));
    }
}
