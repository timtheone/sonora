use serde::Serialize;
use std::time::Instant;

use crate::config::{DictationMode, ModelProfile};
use crate::profile::{tuning_for_profile, ProfileTuning};
use crate::transcriber::Transcriber;
use crate::vad::{has_speech, VadConfig};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DictationState {
    Idle,
    Listening,
    Transcribing,
}

#[derive(Debug, Clone, Serialize)]
pub struct PipelineStatus {
    pub mode: DictationMode,
    pub state: DictationState,
    pub model_profile: ModelProfile,
    pub tuning: ProfileTuning,
}

pub struct DictationPipeline<T: Transcriber> {
    mode: DictationMode,
    state: DictationState,
    model_profile: ModelProfile,
    tuning: ProfileTuning,
    vad_config: VadConfig,
    transcriber: T,
}

#[derive(Debug, Clone)]
pub struct ChunkProcessMetrics {
    pub listening: bool,
    pub enough_samples: bool,
    pub had_speech: bool,
    pub vad_ms: u64,
    pub inference_ms: u64,
    pub engine: String,
    pub model: String,
    pub transcript: Option<String>,
}

impl<T: Transcriber> DictationPipeline<T> {
    pub fn new(mode: DictationMode, model_profile: ModelProfile, transcriber: T) -> Self {
        Self {
            mode,
            state: DictationState::Idle,
            model_profile,
            tuning: tuning_for_profile(model_profile),
            vad_config: VadConfig::default(),
            transcriber,
        }
    }

    pub fn status(&self) -> PipelineStatus {
        PipelineStatus {
            mode: self.mode,
            state: self.state,
            model_profile: self.model_profile,
            tuning: self.tuning.clone(),
        }
    }

    pub fn set_mode(&mut self, mode: DictationMode) {
        self.mode = mode;
        self.state = DictationState::Idle;
    }

    pub fn set_model_profile(&mut self, model_profile: ModelProfile) {
        self.model_profile = model_profile;
    }

    pub fn set_tuning(&mut self, tuning: ProfileTuning) {
        self.tuning = tuning;
    }

    pub fn set_transcriber(&mut self, transcriber: T) {
        self.transcriber = transcriber;
    }

    pub fn on_hotkey_down(&mut self) {
        match self.state {
            DictationState::Idle => {
                self.state = DictationState::Listening;
            }
            DictationState::Listening => {
                if self.mode == DictationMode::PushToToggle {
                    self.state = DictationState::Idle;
                }
            }
            DictationState::Transcribing => {}
        }
    }

    pub fn on_hotkey_up(&mut self) {
        if self.mode == DictationMode::PushToTalk && self.state == DictationState::Listening {
            self.state = DictationState::Idle;
        }
    }

    pub fn cancel(&mut self) {
        self.state = DictationState::Idle;
    }

    pub fn process_audio_chunk(&mut self, samples: &[f32]) -> Result<Option<String>, String> {
        Ok(self.process_audio_chunk_profiled(samples)?.transcript)
    }

    pub fn process_audio_chunk_profiled(
        &mut self,
        samples: &[f32],
    ) -> Result<ChunkProcessMetrics, String> {
        let mut metrics = ChunkProcessMetrics {
            listening: self.state == DictationState::Listening,
            enough_samples: false,
            had_speech: false,
            vad_ms: 0,
            inference_ms: 0,
            engine: self.transcriber.engine_label().to_string(),
            model: self.transcriber.model_label(),
            transcript: None,
        };

        if !metrics.listening {
            return Ok(metrics);
        }

        if samples.len() < self.tuning.min_chunk_samples {
            return Ok(metrics);
        }
        metrics.enough_samples = true;

        let vad_started_at = Instant::now();
        let has_voice = has_speech(samples, &self.vad_config);
        metrics.vad_ms = vad_started_at.elapsed().as_millis() as u64;
        metrics.had_speech = has_voice;

        if !has_voice {
            return Ok(metrics);
        }

        self.state = DictationState::Transcribing;
        let inference_started_at = Instant::now();
        let transcript = self.transcriber.transcribe(samples)?;
        metrics.inference_ms = inference_started_at.elapsed().as_millis() as u64;
        self.state = DictationState::Listening;
        metrics.transcript = Some(transcript);
        Ok(metrics)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcriber::StubTranscriber;

    fn speech_chunk() -> Vec<f32> {
        (0..20_000)
            .map(|i| {
                let angle = i as f32 * 0.1;
                angle.sin() * 0.2
            })
            .collect()
    }

    #[test]
    fn starts_listening_on_hotkey_down() {
        let mut pipeline = DictationPipeline::new(
            DictationMode::PushToToggle,
            ModelProfile::Balanced,
            StubTranscriber,
        );
        pipeline.on_hotkey_down();
        assert_eq!(pipeline.status().state, DictationState::Listening);
    }

    #[test]
    fn push_to_toggle_stops_with_second_press() {
        let mut pipeline = DictationPipeline::new(
            DictationMode::PushToToggle,
            ModelProfile::Balanced,
            StubTranscriber,
        );
        pipeline.on_hotkey_down();
        pipeline.on_hotkey_down();
        assert_eq!(pipeline.status().state, DictationState::Idle);
    }

    #[test]
    fn push_to_talk_stops_on_release() {
        let mut pipeline = DictationPipeline::new(
            DictationMode::PushToTalk,
            ModelProfile::Balanced,
            StubTranscriber,
        );
        pipeline.on_hotkey_down();
        pipeline.on_hotkey_up();
        assert_eq!(pipeline.status().state, DictationState::Idle);
    }

    #[test]
    fn silent_chunk_does_not_transcribe() {
        let mut pipeline = DictationPipeline::new(
            DictationMode::PushToToggle,
            ModelProfile::Fast,
            StubTranscriber,
        );
        pipeline.on_hotkey_down();

        let result = pipeline
            .process_audio_chunk(&vec![0.0; 1024])
            .expect("silence should not fail processing");
        assert!(result.is_none());
    }

    #[test]
    fn speech_chunk_transcribes() {
        let mut pipeline = DictationPipeline::new(
            DictationMode::PushToToggle,
            ModelProfile::Fast,
            StubTranscriber,
        );
        pipeline.on_hotkey_down();

        let result = pipeline
            .process_audio_chunk(&speech_chunk())
            .expect("speech chunk should be transcribed");

        assert_eq!(result.as_deref(), Some("phase-1 transcript"));
        assert_eq!(pipeline.status().state, DictationState::Listening);
    }

    #[test]
    fn balanced_profile_ignores_short_chunks() {
        let mut pipeline = DictationPipeline::new(
            DictationMode::PushToToggle,
            ModelProfile::Balanced,
            StubTranscriber,
        );
        pipeline.on_hotkey_down();

        let short_chunk = vec![0.2_f32; 1024];
        let result = pipeline
            .process_audio_chunk(&short_chunk)
            .expect("short chunk should be ignored");
        assert!(result.is_none());
    }

    #[test]
    fn set_model_profile_updates_tuning() {
        let mut pipeline = DictationPipeline::new(
            DictationMode::PushToToggle,
            ModelProfile::Balanced,
            StubTranscriber,
        );

        let before = pipeline.status();
        pipeline.set_model_profile(ModelProfile::Fast);
        let after = pipeline.status();

        assert_eq!(before.model_profile, ModelProfile::Balanced);
        assert_eq!(after.model_profile, ModelProfile::Fast);
        assert_eq!(after.tuning, before.tuning);
    }

    #[test]
    fn set_tuning_overrides_chunk_timing() {
        let mut pipeline = DictationPipeline::new(
            DictationMode::PushToToggle,
            ModelProfile::Balanced,
            StubTranscriber,
        );

        pipeline.set_tuning(ProfileTuning {
            min_chunk_samples: 8_000,
            partial_cadence_ms: 450,
        });

        let status = pipeline.status();
        assert_eq!(status.tuning.min_chunk_samples, 8_000);
        assert_eq!(status.tuning.partial_cadence_ms, 450);
    }

    #[test]
    fn profiled_processing_reports_skipped_when_not_listening() {
        let mut pipeline = DictationPipeline::new(
            DictationMode::PushToToggle,
            ModelProfile::Fast,
            StubTranscriber,
        );

        let metrics = pipeline
            .process_audio_chunk_profiled(&speech_chunk())
            .expect("profiling call should succeed");

        assert!(!metrics.listening);
        assert!(!metrics.enough_samples);
        assert!(!metrics.had_speech);
        assert!(metrics.transcript.is_none());
    }

    #[test]
    fn profiled_processing_reports_vad_and_inference_for_speech() {
        let mut pipeline = DictationPipeline::new(
            DictationMode::PushToToggle,
            ModelProfile::Fast,
            StubTranscriber,
        );
        pipeline.on_hotkey_down();

        let metrics = pipeline
            .process_audio_chunk_profiled(&speech_chunk())
            .expect("speech chunk profiling should succeed");

        assert!(metrics.listening);
        assert!(metrics.enough_samples);
        assert!(metrics.had_speech);
        assert!(metrics.transcript.is_some());
    }
}
