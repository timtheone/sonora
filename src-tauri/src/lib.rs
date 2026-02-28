pub mod audio;
pub mod config;
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
use profile::{
    build_model_status, detect_hardware_tier, recommended_profile_for_tier, HardwareTier,
    ModelStatus,
};
#[cfg(feature = "desktop")]
use recovery::RecoveryCheckpoint;
#[cfg(feature = "desktop")]
use runtime_log as log_store;
#[cfg(feature = "desktop")]
use serde::Serialize;
#[cfg(feature = "desktop")]
use settings_store::AppSettingsPatch;
#[cfg(feature = "desktop")]
use std::collections::VecDeque;
#[cfg(feature = "desktop")]
use std::path::{Path, PathBuf};
#[cfg(feature = "desktop")]
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
#[cfg(feature = "desktop")]
use std::sync::{Arc, Mutex};
#[cfg(feature = "desktop")]
use std::thread;
#[cfg(feature = "desktop")]
use std::time::{Duration, Instant};
#[cfg(feature = "desktop")]
use tauri::Emitter;
#[cfg(feature = "desktop")]
use tauri::Manager;
#[cfg(feature = "desktop")]
use transcriber::{
    build_runtime_transcriber, resolve_binary_candidates, resolve_binary_path, RuntimeTranscriber,
};

#[cfg(feature = "desktop")]
struct PipelineStore {
    pipeline: Arc<Mutex<DictationPipeline<RuntimeTranscriber>>>,
    last_transcript: Arc<Mutex<Option<String>>>,
    live_capture: Mutex<Option<LiveCaptureSession>>,
}

#[cfg(feature = "desktop")]
impl PipelineStore {
    fn new(mode: DictationMode, model_profile: ModelProfile) -> Self {
        Self {
            pipeline: Arc::new(Mutex::new(DictationPipeline::new(
                mode,
                model_profile,
                RuntimeTranscriber::Unavailable {
                    reason: "transcriber not initialized".to_string(),
                },
            ))),
            last_transcript: Arc::new(Mutex::new(None)),
            live_capture: Mutex::new(None),
        }
    }
}

#[cfg(feature = "desktop")]
struct LiveCaptureSession {
    stop_tx: Sender<()>,
    worker: Option<thread::JoinHandle<()>>,
}

#[cfg(feature = "desktop")]
impl LiveCaptureSession {
    fn stop(mut self) {
        let _ = self.stop_tx.send(());
        if let Some(handle) = self.worker.take() {
            let _ = handle.join();
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
    compute_backend: String,
    using_gpu: bool,
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
#[derive(Clone, Serialize)]
struct LiveMicPayload {
    active: bool,
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
        settings.model_profile,
        model_path.clone(),
        resource_dir.as_deref(),
    );

    let ready = matches!(runtime, RuntimeTranscriber::Whisper(_));
    TranscriberStatus {
        ready,
        description: runtime.description(),
        compute_backend: runtime.compute_backend_label(),
        using_gpu: runtime.uses_gpu(),
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
        settings.model_profile,
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
    let record = InsertionRecord { text, status };

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
fn emit_live_mic_state(app: &tauri::AppHandle, active: bool) {
    let _ = app.emit("dictation:live-mic", LiveMicPayload { active });
}

#[cfg(feature = "desktop")]
fn select_fresh_transcript(
    last_transcript: &mut Option<String>,
    raw_transcript: Option<String>,
) -> Option<String> {
    let normalized = raw_transcript.map(|value| normalize_transcript(&value));
    normalized.and_then(|value| {
        if value.is_empty() || is_duplicate_transcript(last_transcript.as_deref(), &value) {
            None
        } else {
            *last_transcript = Some(value.clone());
            Some(value)
        }
    })
}

#[cfg(feature = "desktop")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LiveCaptureChunkPlan {
    next_chunk_size: usize,
    max_chunk_samples: usize,
}

#[cfg(feature = "desktop")]
fn plan_live_capture_chunk(
    status: &PipelineStatus,
    pending_samples: usize,
    elapsed_since_last_feed: Duration,
) -> Option<LiveCaptureChunkPlan> {
    if status.state != pipeline::DictationState::Listening {
        return None;
    }

    let min_chunk_samples = status.tuning.min_chunk_samples.max(8_000);
    if pending_samples < min_chunk_samples {
        return None;
    }

    let cadence = Duration::from_millis(status.tuning.partial_cadence_ms.max(300));
    if elapsed_since_last_feed < cadence {
        return None;
    }

    let max_chunk_samples = min_chunk_samples.saturating_mul(3);
    Some(LiveCaptureChunkPlan {
        next_chunk_size: pending_samples.min(max_chunk_samples),
        max_chunk_samples,
    })
}

#[cfg(feature = "desktop")]
fn trim_pending_backlog(pending_samples: &mut VecDeque<f32>, max_chunk_samples: usize) {
    while pending_samples.len() > max_chunk_samples.saturating_mul(5) {
        let _ = pending_samples.pop_front();
    }
}

#[cfg(feature = "desktop")]
fn mic_sensitivity_gain(mic_sensitivity_percent: u16) -> f32 {
    (mic_sensitivity_percent.clamp(50, 300) as f32 / 100.0).clamp(0.5, 3.0)
}

#[cfg(feature = "desktop")]
fn apply_mic_gain(samples: &mut [f32], gain: f32) {
    if (gain - 1.0).abs() < f32::EPSILON {
        return;
    }

    for sample in samples {
        *sample = (*sample * gain).clamp(-1.0, 1.0);
    }
}

#[cfg(feature = "desktop")]
const FRAME_RECV_TIMEOUT_MS: u64 = 60;

#[cfg(feature = "desktop")]
const METER_EMIT_INTERVAL_MS: u64 = 33;

#[cfg(feature = "desktop")]
fn should_emit_meter_update(elapsed: Duration) -> bool {
    elapsed >= Duration::from_millis(METER_EMIT_INTERVAL_MS)
}

#[cfg(feature = "desktop")]
fn emit_transcript_if_fresh(
    app: &tauri::AppHandle,
    logs_path: &Path,
    last_transcript: &Arc<Mutex<Option<String>>>,
    raw_transcript: Option<String>,
) -> Result<Option<String>, String> {
    let mut last = last_transcript
        .lock()
        .map_err(|_| "failed to acquire transcript state".to_string())?;
    let transcript = select_fresh_transcript(&mut last, raw_transcript);

    if let Some(text) = &transcript {
        app.emit(
            "dictation:transcript",
            TranscriptPayload { text: text.clone() },
        )
        .map_err(|error| error.to_string())?;

        let _ = log_store::append(
            logs_path,
            "info",
            "transcript.emit",
            &format!("emitted transcript length={}", text.len()),
        );
    }

    Ok(transcript)
}

#[cfg(feature = "desktop")]
fn run_transcription_worker(
    app: tauri::AppHandle,
    pipeline: Arc<Mutex<DictationPipeline<RuntimeTranscriber>>>,
    last_transcript: Arc<Mutex<Option<String>>>,
    logs_path: PathBuf,
    source_sample_rate_hz: u32,
    frame_rx: Receiver<Vec<f32>>,
) {
    let mut pending_samples = VecDeque::<f32>::new();
    let mut last_feed_at = Instant::now() - Duration::from_secs(8);

    loop {
        let frame = match frame_rx.recv_timeout(Duration::from_millis(FRAME_RECV_TIMEOUT_MS)) {
            Ok(samples) => samples,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        };

        let downsampled = audio::downsample_to_16k(&frame, source_sample_rate_hz);
        if downsampled.is_empty() {
            continue;
        }
        pending_samples.extend(downsampled);

        let status = match pipeline.lock() {
            Ok(locked) => locked.status(),
            Err(_) => {
                let _ = log_store::append(
                    &logs_path,
                    "error",
                    "mic.capture",
                    "failed to acquire pipeline state",
                );
                break;
            }
        };

        if status.state != pipeline::DictationState::Listening {
            pending_samples.clear();
            continue;
        }

        let Some(chunk_plan) =
            plan_live_capture_chunk(&status, pending_samples.len(), last_feed_at.elapsed())
        else {
            continue;
        };

        let mut chunk = Vec::<f32>::with_capacity(chunk_plan.next_chunk_size);
        for _ in 0..chunk_plan.next_chunk_size {
            if let Some(sample) = pending_samples.pop_front() {
                chunk.push(sample);
            }
        }

        trim_pending_backlog(&mut pending_samples, chunk_plan.max_chunk_samples);

        last_feed_at = Instant::now();

        let raw_transcript = match pipeline.lock() {
            Ok(mut locked) => match locked.process_audio_chunk(&chunk) {
                Ok(value) => value,
                Err(error) => {
                    let _ = log_store::append(&logs_path, "error", "mic.capture", &error);
                    continue;
                }
            },
            Err(_) => {
                let _ = log_store::append(
                    &logs_path,
                    "error",
                    "mic.capture",
                    "failed to lock pipeline for transcription",
                );
                break;
            }
        };

        if let Err(error) =
            emit_transcript_if_fresh(&app, &logs_path, &last_transcript, raw_transcript)
        {
            let _ = log_store::append(&logs_path, "error", "mic.capture", &error);
        }
    }
}

#[cfg(feature = "desktop")]
fn run_live_capture_session(
    app: tauri::AppHandle,
    pipeline: Arc<Mutex<DictationPipeline<RuntimeTranscriber>>>,
    last_transcript: Arc<Mutex<Option<String>>>,
    logs_path: PathBuf,
    microphone_id: Option<String>,
    mic_sensitivity_percent: u16,
    stop_rx: Receiver<()>,
) {
    let (capture_tx, capture_rx) = mpsc::sync_channel::<Vec<f32>>(48);
    let input_stream = match audio::build_live_input_stream(microphone_id.as_deref(), capture_tx) {
        Ok(stream) => stream,
        Err(error) => {
            let _ = log_store::append(&logs_path, "error", "mic.capture", &error);
            emit_live_mic_state(&app, false);
            return;
        }
    };

    let (transcribe_tx, transcribe_rx) = mpsc::sync_channel::<Vec<f32>>(24);
    let app_for_transcription = app.clone();
    let pipeline_for_transcription = Arc::clone(&pipeline);
    let transcripts_for_transcription = Arc::clone(&last_transcript);
    let logs_for_transcription = logs_path.clone();
    let source_sample_rate_hz = input_stream.sample_rate_hz;

    let transcription_worker = thread::spawn(move || {
        run_transcription_worker(
            app_for_transcription,
            pipeline_for_transcription,
            transcripts_for_transcription,
            logs_for_transcription,
            source_sample_rate_hz,
            transcribe_rx,
        );
    });

    let mic_gain = mic_sensitivity_gain(mic_sensitivity_percent);
    let mut last_meter_emit_at = Instant::now() - Duration::from_secs(1);
    let mut mic_level = 0f32;
    let mut mic_peak = 0f32;

    loop {
        if stop_rx.try_recv().is_ok() {
            break;
        }

        let mut frame = match capture_rx.recv_timeout(Duration::from_millis(FRAME_RECV_TIMEOUT_MS))
        {
            Ok(samples) => samples,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        };

        apply_mic_gain(&mut frame, mic_gain);

        let measured = audio::measure_mic_level(&frame, mic_level, mic_peak);
        mic_level = measured.level;
        mic_peak = measured.peak;

        if should_emit_meter_update(last_meter_emit_at.elapsed()) {
            let _ = app.emit("dictation:mic-level", measured);
            last_meter_emit_at = Instant::now();
        }

        let _ = transcribe_tx.try_send(frame);
    }

    drop(transcribe_tx);
    let _ = transcription_worker.join();

    let _ = app.emit(
        "dictation:mic-level",
        audio::MicLevel {
            level: 0.0,
            peak: 0.0,
            active: false,
        },
    );
}

#[cfg(feature = "desktop")]
fn reap_finished_live_capture(store: &tauri::State<'_, PipelineStore>) {
    let finished = {
        let mut active_capture = match store.live_capture.lock() {
            Ok(value) => value,
            Err(_) => return,
        };

        let is_finished = active_capture
            .as_ref()
            .and_then(|session| session.worker.as_ref())
            .map(thread::JoinHandle::is_finished)
            .unwrap_or(false);

        if is_finished {
            active_capture.take()
        } else {
            None
        }
    };

    if let Some(session) = finished {
        session.stop();
    }
}

#[cfg(feature = "desktop")]
fn stop_live_capture_internal(
    app: &tauri::AppHandle,
    store: &tauri::State<'_, PipelineStore>,
) -> Result<bool, String> {
    let session = {
        let mut active_capture = store
            .live_capture
            .lock()
            .map_err(|_| "failed to acquire live capture state".to_string())?;
        active_capture.take()
    };

    if let Some(session) = session {
        session.stop();
        emit_live_mic_state(app, false);
        Ok(true)
    } else {
        emit_live_mic_state(app, false);
        Ok(false)
    }
}

#[cfg(feature = "desktop")]
#[tauri::command]
fn phase1_list_microphones() -> Result<Vec<audio::InputMicrophone>, String> {
    audio::list_input_microphones()
}

#[cfg(feature = "desktop")]
#[tauri::command]
fn phase1_get_live_capture_active(store: tauri::State<'_, PipelineStore>) -> Result<bool, String> {
    reap_finished_live_capture(&store);

    let active_capture = store
        .live_capture
        .lock()
        .map_err(|_| "failed to acquire live capture state".to_string())?;
    Ok(active_capture.is_some())
}

#[cfg(feature = "desktop")]
#[tauri::command]
fn phase1_start_live_capture(
    app: tauri::AppHandle,
    store: tauri::State<'_, PipelineStore>,
    settings_state: tauri::State<'_, SettingsState>,
    logs: tauri::State<'_, RuntimeLogState>,
    microphone_id: Option<String>,
) -> Result<bool, String> {
    reap_finished_live_capture(&store);

    {
        let active_capture = store
            .live_capture
            .lock()
            .map_err(|_| "failed to acquire live capture state".to_string())?;
        if active_capture.is_some() {
            emit_live_mic_state(&app, true);
            return Ok(true);
        }
    }

    let pipeline = Arc::clone(&store.pipeline);
    let last_transcript = Arc::clone(&store.last_transcript);
    let logs_path = logs.path.clone();
    let app_for_worker = app.clone();
    let mic_sensitivity_percent = settings_state
        .settings
        .lock()
        .map_err(|_| "failed to acquire settings state".to_string())?
        .mic_sensitivity_percent;
    let selected_microphone = microphone_id
        .map(|value| value.trim().to_string())
        .and_then(|value| if value.is_empty() { None } else { Some(value) });

    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let worker = thread::spawn(move || {
        run_live_capture_session(
            app_for_worker,
            pipeline,
            last_transcript,
            logs_path,
            selected_microphone,
            mic_sensitivity_percent,
            stop_rx,
        );
    });

    {
        let mut active_capture = store
            .live_capture
            .lock()
            .map_err(|_| "failed to acquire live capture state".to_string())?;
        *active_capture = Some(LiveCaptureSession {
            stop_tx,
            worker: Some(worker),
        });
    }

    emit_live_mic_state(&app, true);
    let _ = log_store::append(&logs.path, "info", "mic.capture", "live capture started");
    Ok(true)
}

#[cfg(feature = "desktop")]
#[tauri::command]
fn phase1_stop_live_capture(
    app: tauri::AppHandle,
    store: tauri::State<'_, PipelineStore>,
    logs: tauri::State<'_, RuntimeLogState>,
) -> Result<bool, String> {
    let stopped = stop_live_capture_internal(&app, &store)?;
    if stopped {
        let _ = log_store::append(&logs.path, "info", "mic.capture", "live capture stopped");
    }
    Ok(stopped)
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
    drop(pipeline);

    emit_transcript_if_fresh(&app, &logs.path, &store.last_transcript, raw_transcript)
}

#[cfg(all(test, feature = "desktop"))]
mod tests {
    use super::*;
    use crate::config::{DictationMode, ModelProfile};
    use crate::pipeline::DictationState;
    use crate::profile::ProfileTuning;

    fn pipeline_status(
        state: DictationState,
        min_chunk_samples: usize,
        partial_cadence_ms: u64,
    ) -> PipelineStatus {
        PipelineStatus {
            mode: DictationMode::PushToToggle,
            state,
            model_profile: ModelProfile::Balanced,
            tuning: ProfileTuning {
                min_chunk_samples,
                partial_cadence_ms,
            },
        }
    }

    #[test]
    fn selects_fresh_transcript_once() {
        let mut last = None;

        let first = select_fresh_transcript(&mut last, Some("  hello   world  ".to_string()));
        assert_eq!(first.as_deref(), Some("Hello world."));
        assert_eq!(last.as_deref(), Some("Hello world."));

        let duplicate = select_fresh_transcript(&mut last, Some("hello world.".to_string()));
        assert!(duplicate.is_none());

        let empty = select_fresh_transcript(&mut last, Some("   ".to_string()));
        assert!(empty.is_none());

        let absent = select_fresh_transcript(&mut last, None);
        assert!(absent.is_none());
    }

    #[test]
    fn chunk_plan_requires_listening_state() {
        let status = pipeline_status(DictationState::Idle, 32_000, 1_400);
        let plan = plan_live_capture_chunk(&status, 64_000, Duration::from_secs(3));
        assert!(plan.is_none());
    }

    #[test]
    fn chunk_plan_respects_minimum_and_cadence() {
        let status = pipeline_status(DictationState::Listening, 32_000, 1_400);

        let too_small = plan_live_capture_chunk(&status, 31_999, Duration::from_secs(3));
        assert!(too_small.is_none());

        let too_soon = plan_live_capture_chunk(&status, 32_000, Duration::from_millis(1_000));
        assert!(too_soon.is_none());

        let ready = plan_live_capture_chunk(&status, 80_000, Duration::from_millis(1_600))
            .expect("chunk should be planned");
        assert_eq!(ready.max_chunk_samples, 96_000);
        assert_eq!(ready.next_chunk_size, 80_000);
    }

    #[test]
    fn chunk_plan_caps_chunk_size_by_maximum() {
        let status = pipeline_status(DictationState::Listening, 32_000, 1_400);
        let plan = plan_live_capture_chunk(&status, 150_000, Duration::from_secs(2))
            .expect("chunk should be planned");
        assert_eq!(plan.max_chunk_samples, 96_000);
        assert_eq!(plan.next_chunk_size, 96_000);
    }

    #[test]
    fn trims_pending_backlog_to_bounded_limit() {
        let mut pending = (0..80).map(|value| value as f32).collect::<VecDeque<_>>();
        trim_pending_backlog(&mut pending, 10);

        assert_eq!(pending.len(), 50);
        assert_eq!(pending.front().copied(), Some(30.0));
        assert_eq!(pending.back().copied(), Some(79.0));
    }

    #[test]
    fn mic_sensitivity_gain_is_clamped() {
        assert!((mic_sensitivity_gain(50) - 0.5).abs() < f32::EPSILON);
        assert!((mic_sensitivity_gain(140) - 1.4).abs() < f32::EPSILON);
        assert!((mic_sensitivity_gain(400) - 3.0).abs() < f32::EPSILON);
    }

    #[test]
    fn mic_gain_amplifies_and_clips_samples() {
        let mut samples = vec![0.1_f32, -0.3_f32, 0.9_f32];
        apply_mic_gain(&mut samples, 2.0);

        assert_eq!(samples[0], 0.2);
        assert_eq!(samples[1], -0.6);
        assert_eq!(samples[2], 1.0);
    }

    #[test]
    fn meter_emit_interval_matches_smooth_ui_target() {
        assert!(!should_emit_meter_update(Duration::from_millis(20)));
        assert!(should_emit_meter_update(Duration::from_millis(33)));
        assert!(should_emit_meter_update(Duration::from_millis(45)));
    }
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
            if let Ok(current_settings) = settings_state.settings.lock().map(|value| value.clone())
            {
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

                let pipeline = window.app_handle().state::<PipelineStore>();
                let _ = stop_live_capture_internal(&window.app_handle(), &pipeline);
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
            phase1_list_microphones,
            phase1_get_live_capture_active,
            phase1_start_live_capture,
            phase1_stop_live_capture,
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
