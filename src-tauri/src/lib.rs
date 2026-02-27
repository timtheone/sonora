pub mod config;
pub mod audio;
pub mod insertion;
pub mod pipeline;
pub mod settings_store;
pub mod transcriber;
pub mod vad;

#[cfg(feature = "desktop")]
use config::AppSettings;
#[cfg(feature = "desktop")]
use config::DictationMode;
#[cfg(feature = "desktop")]
use insertion::{append_recent, resolve_status, InsertionRecord};
#[cfg(feature = "desktop")]
use pipeline::{DictationPipeline, PipelineStatus};
#[cfg(feature = "desktop")]
use serde::Serialize;
#[cfg(feature = "desktop")]
use settings_store::AppSettingsPatch;
#[cfg(feature = "desktop")]
use std::sync::Mutex;
#[cfg(feature = "desktop")]
use tauri::Emitter;
#[cfg(feature = "desktop")]
use transcriber::StubTranscriber;
#[cfg(feature = "desktop")]
use std::path::PathBuf;

#[cfg(feature = "desktop")]
struct PipelineStore {
    pipeline: Mutex<DictationPipeline<StubTranscriber>>,
}

#[cfg(feature = "desktop")]
impl PipelineStore {
    fn new(mode: DictationMode) -> Self {
        Self {
            pipeline: Mutex::new(DictationPipeline::new(mode, StubTranscriber)),
        }
    }
}

#[cfg(feature = "desktop")]
struct SettingsState {
    settings: Mutex<AppSettings>,
    settings_path: PathBuf,
}

#[cfg(feature = "desktop")]
impl SettingsState {
    fn new(settings: AppSettings, settings_path: PathBuf) -> Self {
        Self {
            settings: Mutex::new(settings),
            settings_path,
        }
    }
}

#[cfg(feature = "desktop")]
#[derive(Default)]
struct InsertionState {
    records: Mutex<Vec<InsertionRecord>>,
}

#[cfg(feature = "desktop")]
#[derive(Clone, Serialize)]
struct TranscriptPayload {
    text: String,
}

#[cfg(feature = "desktop")]
#[tauri::command]
fn get_default_settings() -> AppSettings {
    AppSettings::default()
}

#[cfg(feature = "desktop")]
#[tauri::command]
fn health_check() -> &'static str {
    "ok"
}

#[cfg(feature = "desktop")]
#[tauri::command]
fn phase2_get_settings(state: tauri::State<'_, SettingsState>) -> Result<AppSettings, String> {
    let settings = state
        .settings
        .lock()
        .map_err(|_| "failed to acquire settings state".to_string())?;
    Ok(settings.clone())
}

#[cfg(feature = "desktop")]
#[tauri::command]
fn phase2_update_settings(
    settings_state: tauri::State<'_, SettingsState>,
    pipeline_state: tauri::State<'_, PipelineStore>,
    patch: AppSettingsPatch,
) -> Result<AppSettings, String> {
    let mut settings = settings_state
        .settings
        .lock()
        .map_err(|_| "failed to acquire settings state".to_string())?;
    let updated = settings_store::apply_patch(&settings, patch);
    settings_store::save(&settings_state.settings_path, &updated)?;
    *settings = updated.clone();

    let mut pipeline = pipeline_state
        .pipeline
        .lock()
        .map_err(|_| "failed to acquire pipeline state".to_string())?;
    pipeline.set_mode(updated.mode);

    Ok(updated)
}

#[cfg(feature = "desktop")]
#[tauri::command]
fn phase2_get_recent_insertions(
    insertion_state: tauri::State<'_, InsertionState>,
) -> Result<Vec<InsertionRecord>, String> {
    let records = insertion_state
        .records
        .lock()
        .map_err(|_| "failed to acquire insertion state".to_string())?;
    Ok(records.clone())
}

#[cfg(feature = "desktop")]
#[tauri::command]
fn phase2_insert_text(
    app: tauri::AppHandle,
    settings_state: tauri::State<'_, SettingsState>,
    insertion_state: tauri::State<'_, InsertionState>,
    text: String,
) -> Result<InsertionRecord, String> {
    if text.trim().is_empty() {
        return Err("cannot insert empty text".to_string());
    }

    let fallback_enabled = settings_state
        .settings
        .lock()
        .map_err(|_| "failed to acquire settings state".to_string())?
        .clipboard_fallback;

    let status = resolve_status(
        try_direct_insertion(&text),
        fallback_enabled,
        try_clipboard_fallback(&text),
    );
    let record = InsertionRecord {
        text,
        status,
    };

    let mut records = insertion_state
        .records
        .lock()
        .map_err(|_| "failed to acquire insertion state".to_string())?;
    append_recent(&mut records, record.clone(), 3);

    app.emit("dictation:insertion", record.clone())
        .map_err(|error| error.to_string())?;
    Ok(record)
}

#[cfg(feature = "desktop")]
fn try_direct_insertion(_text: &str) -> Result<(), String> {
    Err("direct insertion adapters are not wired yet".to_string())
}

#[cfg(feature = "desktop")]
fn try_clipboard_fallback(_text: &str) -> Result<(), String> {
    Ok(())
}

#[cfg(feature = "desktop")]
#[tauri::command]
fn phase1_get_status(store: tauri::State<'_, PipelineStore>) -> Result<PipelineStatus, String> {
    let pipeline = store
        .pipeline
        .lock()
        .map_err(|_| "failed to acquire pipeline state".to_string())?;
    Ok(pipeline.status())
}

#[cfg(feature = "desktop")]
#[tauri::command]
fn phase1_set_mode(
    store: tauri::State<'_, PipelineStore>,
    mode: DictationMode,
) -> Result<PipelineStatus, String> {
    let mut pipeline = store
        .pipeline
        .lock()
        .map_err(|_| "failed to acquire pipeline state".to_string())?;
    pipeline.set_mode(mode);
    Ok(pipeline.status())
}

#[cfg(feature = "desktop")]
#[tauri::command]
fn phase1_hotkey_down(store: tauri::State<'_, PipelineStore>) -> Result<PipelineStatus, String> {
    let mut pipeline = store
        .pipeline
        .lock()
        .map_err(|_| "failed to acquire pipeline state".to_string())?;
    pipeline.on_hotkey_down();
    Ok(pipeline.status())
}

#[cfg(feature = "desktop")]
#[tauri::command]
fn phase1_hotkey_up(store: tauri::State<'_, PipelineStore>) -> Result<PipelineStatus, String> {
    let mut pipeline = store
        .pipeline
        .lock()
        .map_err(|_| "failed to acquire pipeline state".to_string())?;
    pipeline.on_hotkey_up();
    Ok(pipeline.status())
}

#[cfg(feature = "desktop")]
#[tauri::command]
fn phase1_cancel(store: tauri::State<'_, PipelineStore>) -> Result<PipelineStatus, String> {
    let mut pipeline = store
        .pipeline
        .lock()
        .map_err(|_| "failed to acquire pipeline state".to_string())?;
    pipeline.cancel();
    Ok(pipeline.status())
}

#[cfg(feature = "desktop")]
#[tauri::command]
fn phase1_feed_audio(
    app: tauri::AppHandle,
    store: tauri::State<'_, PipelineStore>,
    samples: Vec<f32>,
) -> Result<Option<String>, String> {
    let mut pipeline = store
        .pipeline
        .lock()
        .map_err(|_| "failed to acquire pipeline state".to_string())?;

    let transcript = pipeline.process_audio_chunk(&samples)?;
    if let Some(text) = &transcript {
        app.emit(
            "dictation:transcript",
            TranscriptPayload { text: text.clone() },
        )
        .map_err(|error| error.to_string())?;
    }

    Ok(transcript)
}

#[cfg(feature = "desktop")]
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let settings_path = settings_store::default_settings_path();
    let settings = settings_store::load_or_default(&settings_path);
    let initial_mode = settings.mode;

    tauri::Builder::default()
        .manage(PipelineStore::new(initial_mode))
        .manage(SettingsState::new(settings, settings_path))
        .manage(InsertionState::default())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_default_settings,
            health_check,
            phase1_get_status,
            phase1_set_mode,
            phase1_hotkey_down,
            phase1_hotkey_up,
            phase1_cancel,
            phase1_feed_audio,
            phase2_get_settings,
            phase2_update_settings,
            phase2_get_recent_insertions,
            phase2_insert_text
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(not(feature = "desktop"))]
pub fn run() {}
