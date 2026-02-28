use crate::config::{
    AppSettings, DictationMode, FasterWhisperComputeType, ModelProfile, SttEngine,
    WhisperBackendPreference,
};
use crate::profile::{clamp_chunk_duration_ms, clamp_partial_cadence_ms};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppSettingsPatch {
    pub hotkey: Option<String>,
    pub mode: Option<DictationMode>,
    pub model_profile: Option<ModelProfile>,
    pub stt_engine: Option<SttEngine>,
    pub model_path: Option<Option<String>>,
    pub microphone_id: Option<Option<String>>,
    pub mic_sensitivity_percent: Option<u16>,
    pub chunk_duration_ms: Option<u16>,
    pub partial_cadence_ms: Option<u16>,
    pub whisper_backend_preference: Option<WhisperBackendPreference>,
    pub faster_whisper_model: Option<Option<String>>,
    pub faster_whisper_compute_type: Option<FasterWhisperComputeType>,
    pub faster_whisper_beam_size: Option<u8>,
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
        stt_engine: patch.stt_engine.unwrap_or(settings.stt_engine),
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
        chunk_duration_ms: patch.chunk_duration_ms.or(settings.chunk_duration_ms),
        partial_cadence_ms: patch.partial_cadence_ms.or(settings.partial_cadence_ms),
        whisper_backend_preference: patch
            .whisper_backend_preference
            .unwrap_or(settings.whisper_backend_preference),
        faster_whisper_model: patch
            .faster_whisper_model
            .unwrap_or_else(|| settings.faster_whisper_model.clone()),
        faster_whisper_compute_type: patch
            .faster_whisper_compute_type
            .unwrap_or(settings.faster_whisper_compute_type),
        faster_whisper_beam_size: patch
            .faster_whisper_beam_size
            .unwrap_or(settings.faster_whisper_beam_size),
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
    settings.chunk_duration_ms = settings.chunk_duration_ms.map(clamp_chunk_duration_ms);
    settings.partial_cadence_ms = settings.partial_cadence_ms.map(clamp_partial_cadence_ms);
    settings.faster_whisper_model = settings
        .faster_whisper_model
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    settings.faster_whisper_beam_size = settings.faster_whisper_beam_size.clamp(1, 8);
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
                stt_engine: Some(SttEngine::WhisperCpp),
                model_path: Some(Some("models/custom.bin".to_string())),
                microphone_id: Some(Some("mic-2".to_string())),
                mic_sensitivity_percent: Some(185),
                chunk_duration_ms: Some(1_600),
                partial_cadence_ms: Some(700),
                whisper_backend_preference: Some(WhisperBackendPreference::Cuda),
                faster_whisper_model: Some(Some("small.en".to_string())),
                faster_whisper_compute_type: Some(FasterWhisperComputeType::Float16),
                faster_whisper_beam_size: Some(2),
                clipboard_fallback: Some(false),
                launch_at_startup: Some(true),
            },
        );

        assert_eq!(updated.hotkey, "CtrlOrCmd+Shift+Y");
        assert_eq!(updated.mode, DictationMode::PushToTalk);
        assert_eq!(updated.language, "en");
        assert_eq!(updated.model_profile, ModelProfile::Fast);
        assert_eq!(updated.stt_engine, SttEngine::WhisperCpp);
        assert_eq!(updated.model_path.as_deref(), Some("models/custom.bin"));
        assert_eq!(updated.microphone_id, Some("mic-2".to_string()));
        assert_eq!(updated.mic_sensitivity_percent, 185);
        assert_eq!(updated.chunk_duration_ms, Some(1_600));
        assert_eq!(updated.partial_cadence_ms, Some(700));
        assert_eq!(
            updated.whisper_backend_preference,
            WhisperBackendPreference::Cuda
        );
        assert_eq!(updated.faster_whisper_model.as_deref(), Some("small.en"));
        assert_eq!(
            updated.faster_whisper_compute_type,
            FasterWhisperComputeType::Float16
        );
        assert_eq!(updated.faster_whisper_beam_size, 2);
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
    fn clamps_chunk_and_cadence_patch() {
        let defaults = AppSettings::default();
        let updated = apply_patch(
            &defaults,
            AppSettingsPatch {
                chunk_duration_ms: Some(100),
                partial_cadence_ms: Some(9_000),
                ..AppSettingsPatch::default()
            },
        );

        assert_eq!(updated.chunk_duration_ms, Some(500));
        assert_eq!(updated.partial_cadence_ms, Some(2_500));
    }

    #[test]
    fn persists_and_loads_settings() {
        let path = temp_file("settings");
        let settings = AppSettings {
            hotkey: "CtrlOrCmd+Shift+P".to_string(),
            mode: DictationMode::PushToTalk,
            language: "en".to_string(),
            model_profile: ModelProfile::Fast,
            stt_engine: SttEngine::WhisperCpp,
            model_path: Some("models/ggml-tiny.en-q8_0.bin".to_string()),
            microphone_id: None,
            mic_sensitivity_percent: 165,
            chunk_duration_ms: Some(1_200),
            partial_cadence_ms: Some(600),
            whisper_backend_preference: WhisperBackendPreference::Cpu,
            faster_whisper_model: Some("small.en".to_string()),
            faster_whisper_compute_type: FasterWhisperComputeType::Int8,
            faster_whisper_beam_size: 3,
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
        settings.chunk_duration_ms = Some(100);
        settings.partial_cadence_ms = Some(9_000);
        settings.faster_whisper_model = Some("   ".to_string());
        settings.faster_whisper_beam_size = 90;

        save(&path, &settings).expect("settings should be saved");
        let loaded = load_or_default(&path);
        assert_eq!(loaded.mic_sensitivity_percent, 300);
        assert_eq!(loaded.chunk_duration_ms, Some(500));
        assert_eq!(loaded.partial_cadence_ms, Some(2_500));
        assert!(loaded.faster_whisper_model.is_none());
        assert_eq!(loaded.faster_whisper_beam_size, 8);

        let _ = fs::remove_file(path);
    }
}
