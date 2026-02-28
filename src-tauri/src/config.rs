use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DictationMode {
    PushToToggle,
    PushToTalk,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelProfile {
    Fast,
    Balanced,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SttEngine {
    WhisperCpp,
    FasterWhisper,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WhisperBackendPreference {
    Auto,
    Cpu,
    Cuda,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FasterWhisperComputeType {
    Auto,
    Int8,
    Float16,
    Float32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppSettings {
    pub hotkey: String,
    pub mode: DictationMode,
    pub language: String,
    pub model_profile: ModelProfile,
    #[serde(default = "default_stt_engine")]
    pub stt_engine: SttEngine,
    pub model_path: Option<String>,
    pub microphone_id: Option<String>,
    #[serde(default = "default_mic_sensitivity_percent")]
    pub mic_sensitivity_percent: u16,
    #[serde(default)]
    pub chunk_duration_ms: Option<u16>,
    #[serde(default)]
    pub partial_cadence_ms: Option<u16>,
    #[serde(default = "default_whisper_backend_preference")]
    pub whisper_backend_preference: WhisperBackendPreference,
    #[serde(default)]
    pub faster_whisper_model: Option<String>,
    #[serde(default = "default_faster_whisper_compute_type")]
    pub faster_whisper_compute_type: FasterWhisperComputeType,
    #[serde(default = "default_faster_whisper_beam_size")]
    pub faster_whisper_beam_size: u8,
    pub clipboard_fallback: bool,
    pub launch_at_startup: bool,
}

fn default_mic_sensitivity_percent() -> u16 {
    170
}

fn default_whisper_backend_preference() -> WhisperBackendPreference {
    WhisperBackendPreference::Auto
}

fn default_stt_engine() -> SttEngine {
    SttEngine::WhisperCpp
}

fn default_faster_whisper_compute_type() -> FasterWhisperComputeType {
    FasterWhisperComputeType::Auto
}

fn default_faster_whisper_beam_size() -> u8 {
    1
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            hotkey: "CtrlOrCmd+Shift+U".to_string(),
            mode: DictationMode::PushToToggle,
            language: "en".to_string(),
            model_profile: ModelProfile::Balanced,
            stt_engine: default_stt_engine(),
            model_path: None,
            microphone_id: None,
            mic_sensitivity_percent: default_mic_sensitivity_percent(),
            chunk_duration_ms: None,
            partial_cadence_ms: None,
            whisper_backend_preference: default_whisper_backend_preference(),
            faster_whisper_model: None,
            faster_whisper_compute_type: default_faster_whisper_compute_type(),
            faster_whisper_beam_size: default_faster_whisper_beam_size(),
            clipboard_fallback: true,
            launch_at_startup: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_v1_plan() {
        let settings = AppSettings::default();
        assert_eq!(settings.hotkey, "CtrlOrCmd+Shift+U");
        assert_eq!(settings.mode, DictationMode::PushToToggle);
        assert_eq!(settings.language, "en");
        assert_eq!(settings.model_profile, ModelProfile::Balanced);
        assert_eq!(settings.stt_engine, SttEngine::WhisperCpp);
        assert!(settings.model_path.is_none());
        assert!(settings.clipboard_fallback);
        assert!(!settings.launch_at_startup);
        assert!(settings.microphone_id.is_none());
        assert_eq!(settings.mic_sensitivity_percent, 170);
        assert!(settings.chunk_duration_ms.is_none());
        assert!(settings.partial_cadence_ms.is_none());
        assert_eq!(
            settings.whisper_backend_preference,
            WhisperBackendPreference::Auto
        );
        assert!(settings.faster_whisper_model.is_none());
        assert_eq!(
            settings.faster_whisper_compute_type,
            FasterWhisperComputeType::Auto
        );
        assert_eq!(settings.faster_whisper_beam_size, 1);
    }

    #[test]
    fn missing_mic_sensitivity_deserializes_to_default() {
        let json = r#"{
  "hotkey": "CtrlOrCmd+Shift+U",
  "mode": "push_to_toggle",
  "language": "en",
  "model_profile": "balanced",
  "model_path": null,
  "microphone_id": null,
  "clipboard_fallback": true,
  "launch_at_startup": false
}"#;

        let parsed: AppSettings =
            serde_json::from_str(json).expect("older settings payload should deserialize");
        assert_eq!(parsed.mic_sensitivity_percent, 170);
        assert_eq!(parsed.stt_engine, SttEngine::WhisperCpp);
        assert!(parsed.chunk_duration_ms.is_none());
        assert!(parsed.partial_cadence_ms.is_none());
        assert_eq!(
            parsed.whisper_backend_preference,
            WhisperBackendPreference::Auto
        );
        assert!(parsed.faster_whisper_model.is_none());
        assert_eq!(
            parsed.faster_whisper_compute_type,
            FasterWhisperComputeType::Auto
        );
        assert_eq!(parsed.faster_whisper_beam_size, 1);
    }
}
