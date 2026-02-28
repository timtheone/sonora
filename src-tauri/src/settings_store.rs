use crate::config::{AppSettings, DictationMode, ModelProfile};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppSettingsPatch {
    pub hotkey: Option<String>,
    pub mode: Option<DictationMode>,
    pub model_profile: Option<ModelProfile>,
    pub model_path: Option<Option<String>>,
    pub microphone_id: Option<Option<String>>,
    pub mic_sensitivity_percent: Option<u16>,
    pub clipboard_fallback: Option<bool>,
    pub launch_at_startup: Option<bool>,
}

pub fn default_settings_path() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("sonora-dictation").join("settings.json")
}

pub fn load_or_default(path: &Path) -> AppSettings {
    match fs::read_to_string(path) {
        Ok(contents) => serde_json::from_str::<AppSettings>(&contents)
            .map(normalize_settings)
            .unwrap_or_default(),
        Err(_) => AppSettings::default(),
    }
}

pub fn save(path: &Path, settings: &AppSettings) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "settings path has no parent directory".to_string())?;
    fs::create_dir_all(parent).map_err(io_to_string)?;
    let contents = serde_json::to_string_pretty(settings).map_err(|error| error.to_string())?;
    fs::write(path, contents).map_err(io_to_string)
}

pub fn apply_patch(settings: &AppSettings, patch: AppSettingsPatch) -> AppSettings {
    normalize_settings(AppSettings {
        hotkey: patch
            .hotkey
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| settings.hotkey.clone()),
        mode: patch.mode.unwrap_or(settings.mode),
        language: settings.language.clone(),
        model_profile: patch.model_profile.unwrap_or(settings.model_profile),
        model_path: patch
            .model_path
            .unwrap_or_else(|| settings.model_path.clone()),
        microphone_id: patch
            .microphone_id
            .unwrap_or_else(|| settings.microphone_id.clone()),
        mic_sensitivity_percent: patch
            .mic_sensitivity_percent
            .map(|value| value.clamp(50, 300))
            .unwrap_or(settings.mic_sensitivity_percent),
        clipboard_fallback: patch
            .clipboard_fallback
            .unwrap_or(settings.clipboard_fallback),
        launch_at_startup: patch
            .launch_at_startup
            .unwrap_or(settings.launch_at_startup),
    })
}

fn normalize_settings(mut settings: AppSettings) -> AppSettings {
    settings.mic_sensitivity_percent = settings.mic_sensitivity_percent.clamp(50, 300);
    settings
}

fn io_to_string(error: io::Error) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_file(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be set")
            .as_nanos();
        std::env::temp_dir().join(format!("sonora-{name}-{nanos}.json"))
    }

    #[test]
    fn applies_partial_settings_patch() {
        let defaults = AppSettings::default();
        let updated = apply_patch(
            &defaults,
            AppSettingsPatch {
                hotkey: Some("CtrlOrCmd+Shift+Y".to_string()),
                mode: Some(DictationMode::PushToTalk),
                model_profile: Some(ModelProfile::Fast),
                model_path: Some(Some("models/custom.bin".to_string())),
                microphone_id: Some(Some("mic-2".to_string())),
                mic_sensitivity_percent: Some(185),
                clipboard_fallback: Some(false),
                launch_at_startup: Some(true),
            },
        );

        assert_eq!(updated.hotkey, "CtrlOrCmd+Shift+Y");
        assert_eq!(updated.mode, DictationMode::PushToTalk);
        assert_eq!(updated.language, "en");
        assert_eq!(updated.model_profile, ModelProfile::Fast);
        assert_eq!(updated.model_path.as_deref(), Some("models/custom.bin"));
        assert_eq!(updated.microphone_id, Some("mic-2".to_string()));
        assert_eq!(updated.mic_sensitivity_percent, 185);
        assert!(!updated.clipboard_fallback);
        assert!(updated.launch_at_startup);
    }

    #[test]
    fn clamps_mic_sensitivity_patch() {
        let defaults = AppSettings::default();
        let updated = apply_patch(
            &defaults,
            AppSettingsPatch {
                mic_sensitivity_percent: Some(255),
                ..AppSettingsPatch::default()
            },
        );
        assert_eq!(updated.mic_sensitivity_percent, 255);

        let clamped_low = apply_patch(
            &updated,
            AppSettingsPatch {
                mic_sensitivity_percent: Some(2),
                ..AppSettingsPatch::default()
            },
        );
        assert_eq!(clamped_low.mic_sensitivity_percent, 50);

        let clamped_high = apply_patch(
            &clamped_low,
            AppSettingsPatch {
                mic_sensitivity_percent: Some(355),
                ..AppSettingsPatch::default()
            },
        );
        assert_eq!(clamped_high.mic_sensitivity_percent, 300);
    }

    #[test]
    fn persists_and_loads_settings() {
        let path = temp_file("settings");
        let settings = AppSettings {
            hotkey: "CtrlOrCmd+Shift+P".to_string(),
            mode: DictationMode::PushToTalk,
            language: "en".to_string(),
            model_profile: ModelProfile::Fast,
            model_path: Some("models/ggml-tiny.en-q8_0.bin".to_string()),
            microphone_id: None,
            mic_sensitivity_percent: 165,
            clipboard_fallback: true,
            launch_at_startup: false,
        };

        save(&path, &settings).expect("settings should be saved");
        let loaded = load_or_default(&path);
        assert_eq!(loaded, settings);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn falls_back_to_defaults_for_missing_file() {
        let path = temp_file("missing");
        let loaded = load_or_default(&path);
        assert_eq!(loaded, AppSettings::default());
    }

    #[test]
    fn load_normalizes_mic_sensitivity_from_file() {
        let path = temp_file("normalize");
        let mut settings = AppSettings::default();
        settings.mic_sensitivity_percent = 999;

        save(&path, &settings).expect("settings should be saved");
        let loaded = load_or_default(&path);
        assert_eq!(loaded.mic_sensitivity_percent, 300);

        let _ = fs::remove_file(path);
    }
}
