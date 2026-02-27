use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DictationMode {
    PushToToggle,
    PushToTalk,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppSettings {
    pub hotkey: String,
    pub mode: DictationMode,
    pub language: String,
    pub model_profile: String,
    pub microphone_id: Option<String>,
    pub clipboard_fallback: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            hotkey: "CtrlOrCmd+Shift+U".to_string(),
            mode: DictationMode::PushToToggle,
            language: "en".to_string(),
            model_profile: "balanced".to_string(),
            microphone_id: None,
            clipboard_fallback: true,
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
        assert_eq!(settings.model_profile, "balanced");
        assert!(settings.clipboard_fallback);
        assert!(settings.microphone_id.is_none());
    }
}
