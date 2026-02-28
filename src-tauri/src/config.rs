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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppSettings {
    pub hotkey: String,
    pub mode: DictationMode,
    pub language: String,
    pub model_profile: ModelProfile,
    pub model_path: Option<String>,
    pub microphone_id: Option<String>,
    #[serde(default = "default_mic_sensitivity_percent")]
    pub mic_sensitivity_percent: u16,
    pub clipboard_fallback: bool,
    pub launch_at_startup: bool,
}

fn default_mic_sensitivity_percent() -> u16 {
    140
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            hotkey: "CtrlOrCmd+Shift+U".to_string(),
            mode: DictationMode::PushToToggle,
            language: "en".to_string(),
            model_profile: ModelProfile::Balanced,
            model_path: None,
            microphone_id: None,
            mic_sensitivity_percent: default_mic_sensitivity_percent(),
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
        assert!(settings.model_path.is_none());
        assert!(settings.clipboard_fallback);
        assert!(!settings.launch_at_startup);
        assert!(settings.microphone_id.is_none());
        assert_eq!(settings.mic_sensitivity_percent, 140);
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
        assert_eq!(parsed.mic_sensitivity_percent, 140);
    }
}
