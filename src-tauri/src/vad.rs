#[derive(Debug, Clone)]
pub struct VadConfig {
    pub rms_threshold: f32,
    pub min_samples: usize,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            rms_threshold: 0.015,
            min_samples: 512,
        }
    }
}

pub fn has_speech(samples: &[f32], config: &VadConfig) -> bool {
    if samples.len() < config.min_samples {
        return false;
    }

    let energy_sum = samples.iter().map(|value| value * value).sum::<f32>();
    let rms = (energy_sum / samples.len() as f32).sqrt();
    rms >= config.rms_threshold
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_chunk(amplitude: f32) -> Vec<f32> {
        (0..1024)
            .map(|i| {
                let angle = i as f32 * 0.1;
                angle.sin() * amplitude
            })
            .collect()
    }

    #[test]
    fn rejects_short_chunks() {
        let config = VadConfig::default();
        let short = vec![0.2; 32];
        assert!(!has_speech(&short, &config));
    }

    #[test]
    fn rejects_silence() {
        let config = VadConfig::default();
        let silence = vec![0.0; 1024];
        assert!(!has_speech(&silence, &config));
    }

    #[test]
    fn detects_speech_like_signal() {
        let config = VadConfig::default();
        let speech = create_chunk(0.12);
        assert!(has_speech(&speech, &config));
    }
}
