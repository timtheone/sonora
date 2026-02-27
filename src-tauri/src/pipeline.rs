use serde::Serialize;

use crate::config::DictationMode;
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
}

pub struct DictationPipeline<T: Transcriber> {
    mode: DictationMode,
    state: DictationState,
    vad_config: VadConfig,
    transcriber: T,
}

impl<T: Transcriber> DictationPipeline<T> {
    pub fn new(mode: DictationMode, transcriber: T) -> Self {
        Self {
            mode,
            state: DictationState::Idle,
            vad_config: VadConfig::default(),
            transcriber,
        }
    }

    pub fn status(&self) -> PipelineStatus {
        PipelineStatus {
            mode: self.mode,
            state: self.state,
        }
    }

    pub fn set_mode(&mut self, mode: DictationMode) {
        self.mode = mode;
        self.state = DictationState::Idle;
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
        if self.state != DictationState::Listening {
            return Ok(None);
        }

        if !has_speech(samples, &self.vad_config) {
            return Ok(None);
        }

        self.state = DictationState::Transcribing;
        let transcript = self.transcriber.transcribe(samples)?;
        self.state = DictationState::Listening;
        Ok(Some(transcript))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcriber::StubTranscriber;

    fn speech_chunk() -> Vec<f32> {
        (0..1024)
            .map(|i| {
                let angle = i as f32 * 0.1;
                angle.sin() * 0.2
            })
            .collect()
    }

    #[test]
    fn starts_listening_on_hotkey_down() {
        let mut pipeline = DictationPipeline::new(DictationMode::PushToToggle, StubTranscriber);
        pipeline.on_hotkey_down();
        assert_eq!(pipeline.status().state, DictationState::Listening);
    }

    #[test]
    fn push_to_toggle_stops_with_second_press() {
        let mut pipeline = DictationPipeline::new(DictationMode::PushToToggle, StubTranscriber);
        pipeline.on_hotkey_down();
        pipeline.on_hotkey_down();
        assert_eq!(pipeline.status().state, DictationState::Idle);
    }

    #[test]
    fn push_to_talk_stops_on_release() {
        let mut pipeline = DictationPipeline::new(DictationMode::PushToTalk, StubTranscriber);
        pipeline.on_hotkey_down();
        pipeline.on_hotkey_up();
        assert_eq!(pipeline.status().state, DictationState::Idle);
    }

    #[test]
    fn silent_chunk_does_not_transcribe() {
        let mut pipeline = DictationPipeline::new(DictationMode::PushToToggle, StubTranscriber);
        pipeline.on_hotkey_down();

        let result = pipeline
            .process_audio_chunk(&vec![0.0; 1024])
            .expect("silence should not fail processing");
        assert!(result.is_none());
    }

    #[test]
    fn speech_chunk_transcribes() {
        let mut pipeline = DictationPipeline::new(DictationMode::PushToToggle, StubTranscriber);
        pipeline.on_hotkey_down();

        let result = pipeline
            .process_audio_chunk(&speech_chunk())
            .expect("speech chunk should be transcribed");

        assert_eq!(result.as_deref(), Some("phase-1 transcript"));
        assert_eq!(pipeline.status().state, DictationState::Listening);
    }
}
