pub mod config;
pub mod audio;
pub mod pipeline;
pub mod transcriber;
pub mod vad;

#[cfg(feature = "desktop")]
use config::AppSettings;
#[cfg(feature = "desktop")]
use config::DictationMode;
#[cfg(feature = "desktop")]
use pipeline::{DictationPipeline, PipelineStatus};
#[cfg(feature = "desktop")]
use serde::Serialize;
#[cfg(feature = "desktop")]
use std::sync::Mutex;
#[cfg(feature = "desktop")]
use tauri::Emitter;
#[cfg(feature = "desktop")]
use transcriber::StubTranscriber;

#[cfg(feature = "desktop")]
struct PipelineStore {
    pipeline: Mutex<DictationPipeline<StubTranscriber>>,
}

#[cfg(feature = "desktop")]
impl Default for PipelineStore {
    fn default() -> Self {
        Self {
            pipeline: Mutex::new(DictationPipeline::new(
                DictationMode::PushToToggle,
                StubTranscriber,
            )),
        }
    }
}

#[cfg(feature = "desktop")]
#[derive(Serialize)]
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
    tauri::Builder::default()
        .manage(PipelineStore::default())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_default_settings,
            health_check,
            phase1_get_status,
            phase1_set_mode,
            phase1_hotkey_down,
            phase1_hotkey_up,
            phase1_cancel,
            phase1_feed_audio
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(not(feature = "desktop"))]
pub fn run() {}
