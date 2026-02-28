pub mod config;
pub mod audio;
pub mod environment;
pub mod insertion;
pub mod pipeline;
pub mod postprocess;
pub mod profile;
pub mod recovery;
pub mod runtime_log;
pub mod settings_store;
pub mod transcriber;
pub mod vad;

#[cfg(feature = "desktop")]
use config::AppSettings;
#[cfg(feature = "desktop")]
use config::{DictationMode, ModelProfile};
#[cfg(feature = "desktop")]
use environment::EnvironmentHealth;
#[cfg(feature = "desktop")]
use insertion::{append_recent, resolve_status, InsertionRecord};
#[cfg(feature = "desktop")]
use pipeline::{DictationPipeline, PipelineStatus};
#[cfg(feature = "desktop")]
use postprocess::{is_duplicate_transcript, normalize_transcript};
#[cfg(feature = "desktop")]
use profile::{build_model_status, detect_hardware_tier, recommended_profile_for_tier, HardwareTier, ModelStatus};
#[cfg(feature = "desktop")]
use recovery::RecoveryCheckpoint;
#[cfg(feature = "desktop")]
use runtime_log as log_store;
#[cfg(feature = "desktop")]
use serde::Serialize;
#[cfg(feature = "desktop")]
use settings_store::AppSettingsPatch;
#[cfg(feature = "desktop")]
use std::path::PathBuf;
#[cfg(feature = "desktop")]
use std::sync::Mutex;
#[cfg(feature = "desktop")]
use tauri::Emitter;
#[cfg(feature = "desktop")]
use tauri::Manager;
#[cfg(feature = "desktop")]
use transcriber::{build_runtime_transcriber, resolve_binary_candidates, resolve_binary_path, RuntimeTranscriber};

#[cfg(feature = "desktop")]
struct PipelineStore {
    pipeline: Mutex<DictationPipeline<RuntimeTranscriber>>,
    last_transcript: Mutex<Option<String>>,
}

#[cfg(feature = "desktop")]
impl PipelineStore {
    fn new(mode: DictationMode, model_profile: ModelProfile) -> Self {
        Self {
            pipeline: Mutex::new(DictationPipeline::new(
                mode,
                model_profile,
                RuntimeTranscriber::Unavailable {
                    reason: "transcriber not initialized".to_string(),
                },
            )),
            last_transcript: Mutex::new(None),
        }
    }
}

#[cfg(feature = "desktop")]
#[derive(Clone, Serialize)]
struct HardwareProfileStatus {
    logical_cores: usize,
    hardware_tier: HardwareTier,
    recommended_profile: ModelProfile,
}

#[cfg(feature = "desktop")]
#[derive(Clone, Serialize)]
struct TranscriberStatus {
    ready: bool,
    description: String,
    resolved_binary_path: Option<String>,
    checked_binary_paths: Vec<String>,
    resolved_model_path: String,
    model_exists: bool,
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
struct RuntimeLogState {
    path: PathBuf,
}

#[cfg(feature = "desktop")]
impl RuntimeLogState {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

#[cfg(feature = "desktop")]
struct RecoveryState {
    path: PathBuf,
    checkpoint: Mutex<RecoveryCheckpoint>,
}

#[cfg(feature = "desktop")]
impl RecoveryState {
    fn new(path: PathBuf, checkpoint: RecoveryCheckpoint) -> Self {
        Self {
            path,
            checkpoint: Mutex::new(checkpoint),
        }
    }
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
fn phase4_get_environment_health() -> EnvironmentHealth {
    environment::detect_environment_health()
}

#[cfg(feature = "desktop")]
#[tauri::command]
fn phase4_get_runtime_logs(
    logs: tauri::State<'_, RuntimeLogState>,
    limit: Option<usize>,
) -> Result<Vec<String>, String> {
    let normalized_limit = limit.unwrap_or(40).clamp(1, 200);
    log_store::read_recent(&logs.path, normalized_limit)
}

#[cfg(feature = "desktop")]
#[tauri::command]
fn phase4_clear_runtime_logs(logs: tauri::State<'_, RuntimeLogState>) -> Result<(), String> {
    log_store::clear(&logs.path)
}

#[cfg(feature = "desktop")]
#[tauri::command]
fn phase4_get_transcriber_status(
    app: tauri::AppHandle,
    settings: tauri::State<'_, SettingsState>,
) -> Result<TranscriberStatus, String> {
    let current = settings
        .settings
        .lock()
        .map_err(|_| "failed to acquire settings state".to_string())?
        .clone();
    Ok(build_transcriber_status(&app, &current))
}

#[cfg(feature = "desktop")]
#[tauri::command]
fn phase4_get_recovery_checkpoint(
    recovery: tauri::State<'_, RecoveryState>,
) -> Result<RecoveryCheckpoint, String> {
    let checkpoint = recovery
        .checkpoint
        .lock()
        .map_err(|_| "failed to acquire recovery state".to_string())?;
    Ok(checkpoint.clone())
}

#[cfg(feature = "desktop")]
#[tauri::command]
fn phase4_acknowledge_recovery_notice(
    recovery: tauri::State<'_, RecoveryState>,
) -> Result<RecoveryCheckpoint, String> {
    let mut checkpoint = recovery
        .checkpoint
        .lock()
        .map_err(|_| "failed to acquire recovery state".to_string())?;
    let updated = recovery::acknowledge_recovery_notice(&checkpoint);
    recovery::save(&recovery.path, &updated)?;
    *checkpoint = updated.clone();
    Ok(updated)
}

#[cfg(feature = "desktop")]
#[tauri::command]
fn phase4_mark_clean_shutdown(
    recovery: tauri::State<'_, RecoveryState>,
) -> Result<RecoveryCheckpoint, String> {
    mark_clean_shutdown_state(&recovery)
}

#[cfg(feature = "desktop")]
fn current_logical_cores() -> usize {
    std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(4)
}

#[cfg(feature = "desktop")]
fn build_transcriber_status(app: &tauri::AppHandle, settings: &AppSettings) -> TranscriberStatus {
    let resource_dir = app.path().resource_dir().ok();
    let model_path = profile::resolve_model_path(settings, resource_dir.as_deref());
    let binary_path = resolve_binary_path(resource_dir.as_deref());
    let checked_binary_paths = resolve_binary_candidates(resource_dir.as_deref())
        .into_iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect::<Vec<_>>();

    let runtime = build_runtime_transcriber(
        &settings.language,
        model_path.clone(),
        resource_dir.as_deref(),
    );

    let ready = matches!(runtime, RuntimeTranscriber::Whisper(_));
    TranscriberStatus {
        ready,
        description: runtime.description(),
        resolved_binary_path: binary_path.map(|path| path.to_string_lossy().to_string()),
        checked_binary_paths,
        resolved_model_path: model_path.to_string_lossy().to_string(),
        model_exists: model_path.exists(),
    }
}

#[cfg(feature = "desktop")]
fn apply_runtime_transcriber_from_settings(
    app: &tauri::AppHandle,
    settings: &AppSettings,
    pipeline_store: &tauri::State<'_, PipelineStore>,
) -> Result<TranscriberStatus, String> {
    let resource_dir = app.path().resource_dir().ok();
    let model_path = profile::resolve_model_path(settings, resource_dir.as_deref());
    let runtime = build_runtime_transcriber(
        &settings.language,
        model_path,
        resource_dir.as_deref(),
    );

    let mut pipeline = pipeline_store
        .pipeline
        .lock()
        .map_err(|_| "failed to acquire pipeline state".to_string())?;
    pipeline.set_transcriber(runtime);

    Ok(build_transcriber_status(app, settings))
}

#[cfg(feature = "desktop")]
fn mark_clean_shutdown_state(
    recovery: &tauri::State<'_, RecoveryState>,
) -> Result<RecoveryCheckpoint, String> {
    let mut checkpoint = recovery
        .checkpoint
        .lock()
        .map_err(|_| "failed to acquire recovery state".to_string())?;
    let now = recovery::current_unix_ms()?;
    let updated = recovery::mark_clean_shutdown(&checkpoint, now);
    recovery::save(&recovery.path, &updated)?;
    *checkpoint = updated.clone();
    Ok(updated)
}

#[cfg(feature = "desktop")]
#[tauri::command]
fn phase3_get_hardware_profile() -> HardwareProfileStatus {
    let logical_cores = current_logical_cores();
    let hardware_tier = detect_hardware_tier(logical_cores);
    let recommended_profile = recommended_profile_for_tier(hardware_tier);

    HardwareProfileStatus {
        logical_cores,
        hardware_tier,
        recommended_profile,
    }
}

#[cfg(feature = "desktop")]
#[tauri::command]
fn phase3_auto_select_profile(
    app: tauri::AppHandle,
    settings_state: tauri::State<'_, SettingsState>,
    pipeline_state: tauri::State<'_, PipelineStore>,
    logs: tauri::State<'_, RuntimeLogState>,
) -> Result<AppSettings, String> {
    let hardware = phase3_get_hardware_profile();
    let patch = AppSettingsPatch {
        model_profile: Some(hardware.recommended_profile),
        ..AppSettingsPatch::default()
    };

    let mut settings = settings_state
        .settings
        .lock()
        .map_err(|_| "failed to acquire settings state".to_string())?;
    let updated = settings_store::apply_patch(&settings, patch);
    settings_store::save(&settings_state.settings_path, &updated)?;
    *settings = updated.clone();

    apply_runtime_transcriber_from_settings(&app, &updated, &pipeline_state)?;

    let _ = log_store::append(
        &logs.path,
        "info",
        "profile.auto_select",
        &format!(
            "auto-selected model profile {:?} for tier {:?}",
            hardware.recommended_profile, hardware.hardware_tier
        ),
    );

    Ok(updated)
}

#[cfg(feature = "desktop")]
#[tauri::command]
fn phase3_get_model_status(
    app: tauri::AppHandle,
    state: tauri::State<'_, SettingsState>,
) -> Result<ModelStatus, String> {
    let settings = state
        .settings
        .lock()
        .map_err(|_| "failed to acquire settings state".to_string())?;
    let resource_dir = app.path().resource_dir().ok();
    Ok(build_model_status(
        &settings,
        current_logical_cores(),
        resource_dir.as_deref(),
    ))
}

#[cfg(feature = "desktop")]
#[tauri::command]
fn phase3_set_model_path(
    app: tauri::AppHandle,
    settings_state: tauri::State<'_, SettingsState>,
    pipeline_state: tauri::State<'_, PipelineStore>,
    logs: tauri::State<'_, RuntimeLogState>,
    path: Option<String>,
) -> Result<AppSettings, String> {
    let mut settings = settings_state
        .settings
        .lock()
        .map_err(|_| "failed to acquire settings state".to_string())?;

    let normalized = path
        .map(|value| value.trim().to_string())
        .and_then(|value| if value.is_empty() { None } else { Some(value) });

    let patch = AppSettingsPatch {
        model_path: Some(normalized),
        ..AppSettingsPatch::default()
    };
    let updated = settings_store::apply_patch(&settings, patch);
    settings_store::save(&settings_state.settings_path, &updated)?;
    *settings = updated.clone();

    apply_runtime_transcriber_from_settings(&app, &updated, &pipeline_state)?;

    let _ = log_store::append(
        &logs.path,
        "info",
        "model.path",
        &format!("set model path to {:?}", updated.model_path),
    );

    Ok(updated)
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
    app: tauri::AppHandle,
    settings_state: tauri::State<'_, SettingsState>,
    pipeline_state: tauri::State<'_, PipelineStore>,
    logs: tauri::State<'_, RuntimeLogState>,
    patch: AppSettingsPatch,
) -> Result<AppSettings, String> {
    let mut settings = settings_state
        .settings
        .lock()
        .map_err(|_| "failed to acquire settings state".to_string())?;
    let updated = settings_store::apply_patch(&settings, patch);
    settings_store::save(&settings_state.settings_path, &updated)?;
    *settings = updated.clone();

    {
        let mut pipeline = pipeline_state
            .pipeline
            .lock()
            .map_err(|_| "failed to acquire pipeline state".to_string())?;
        pipeline.set_mode(updated.mode);
        pipeline.set_model_profile(updated.model_profile);
    }
    apply_runtime_transcriber_from_settings(&app, &updated, &pipeline_state)?;

    let _ = log_store::append(
        &logs.path,
        "info",
        "settings.update",
        "updated runtime settings",
    );

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
    logs: tauri::State<'_, RuntimeLogState>,
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

    let _ = log_store::append(
        &logs.path,
        "info",
        "insertion.attempt",
        &format!("insertion status {:?}", record.status),
    );

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

    let mut last_transcript = store
        .last_transcript
        .lock()
        .map_err(|_| "failed to acquire transcript state".to_string())?;
    *last_transcript = None;

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

    let mut last_transcript = store
        .last_transcript
        .lock()
        .map_err(|_| "failed to acquire transcript state".to_string())?;
    *last_transcript = None;

    Ok(pipeline.status())
}

#[cfg(feature = "desktop")]
#[tauri::command]
fn phase1_feed_audio(
    app: tauri::AppHandle,
    store: tauri::State<'_, PipelineStore>,
    logs: tauri::State<'_, RuntimeLogState>,
    samples: Vec<f32>,
) -> Result<Option<String>, String> {
    let mut pipeline = store
        .pipeline
        .lock()
        .map_err(|_| "failed to acquire pipeline state".to_string())?;

    let raw_transcript = pipeline.process_audio_chunk(&samples)?;
    let normalized = raw_transcript.map(|value| normalize_transcript(&value));

    let mut last_transcript = store
        .last_transcript
        .lock()
        .map_err(|_| "failed to acquire transcript state".to_string())?;

    let transcript = normalized.and_then(|value| {
        if value.is_empty() || is_duplicate_transcript(last_transcript.as_deref(), &value) {
            None
        } else {
            *last_transcript = Some(value.clone());
            Some(value)
        }
    });

    if let Some(text) = &transcript {
        app.emit(
            "dictation:transcript",
            TranscriptPayload { text: text.clone() },
        )
        .map_err(|error| error.to_string())?;

        let _ = log_store::append(
            &logs.path,
            "info",
            "transcript.emit",
            &format!("emitted transcript length={}", text.len()),
        );
    }

    Ok(transcript)
}

#[cfg(feature = "desktop")]
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let settings_path = settings_store::default_settings_path();
    let settings = settings_store::load_or_default(&settings_path);
    let logs_path = log_store::default_log_path();
    let recovery_path = recovery::default_checkpoint_path();
    let previous_checkpoint = recovery::load_or_default(&recovery_path);
    let now = recovery::current_unix_ms().unwrap_or(0);
    let current_checkpoint = recovery::mark_start(&previous_checkpoint, now);
    let _ = recovery::save(&recovery_path, &current_checkpoint);

    let _ = log_store::append(&logs_path, "info", "app.start", "application startup");
    if current_checkpoint.recovery_notice_pending {
        let _ = log_store::append(
            &logs_path,
            "warn",
            "recovery.pending",
            "previous session ended unexpectedly; recovery notice is pending",
        );
    }

    let initial_mode = settings.mode;
    let initial_profile = settings.model_profile;

    tauri::Builder::default()
        .manage(PipelineStore::new(initial_mode, initial_profile))
        .manage(SettingsState::new(settings, settings_path))
        .manage(InsertionState::default())
        .manage(RuntimeLogState::new(logs_path))
        .manage(RecoveryState::new(recovery_path, current_checkpoint))
        .setup(|app| {
            let settings_state = app.state::<SettingsState>();
            let pipeline_state = app.state::<PipelineStore>();
            if let Ok(current_settings) = settings_state.settings.lock().map(|value| value.clone()) {
                let status = apply_runtime_transcriber_from_settings(
                    &app.handle(),
                    &current_settings,
                    &pipeline_state,
                );

                let logs_state = app.state::<RuntimeLogState>();
                match status {
                    Ok(status) => {
                        let _ = log_store::append(
                            &logs_state.path,
                            if status.ready { "info" } else { "warn" },
                            "transcriber.setup",
                            &status.description,
                        );
                    }
                    Err(error) => {
                        let _ = log_store::append(
                            &logs_state.path,
                            "error",
                            "transcriber.setup",
                            &error,
                        );
                    }
                }
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
                let recovery = window.app_handle().state::<RecoveryState>();
                let _ = mark_clean_shutdown_state(&recovery);
            }
        })
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
            phase2_insert_text,
            phase3_get_hardware_profile,
            phase3_auto_select_profile,
            phase3_get_model_status,
            phase3_set_model_path,
            phase4_get_environment_health,
            phase4_get_runtime_logs,
            phase4_clear_runtime_logs,
            phase4_get_transcriber_status,
            phase4_get_recovery_checkpoint,
            phase4_acknowledge_recovery_notice,
            phase4_mark_clean_shutdown
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(not(feature = "desktop"))]
pub fn run() {}
