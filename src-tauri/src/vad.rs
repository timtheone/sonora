#[derive(Debug, Clone)]
pub struct VadConfig {
    pub rms_threshold: f32,
    pub min_samples: usize,
    pub window_samples: usize,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            rms_threshold: 0.009,
            min_samples: 512,
            window_samples: 512,
        }
    }
}

pub fn has_speech(samples: &[f32], config: &VadConfig) -> bool {
    if samples.len() < config.min_samples {
        return false;
    }

    let window = config
        .window_samples
        .max(config.min_samples)
        .min(samples.len());
    samples
        .chunks(window)
        .any(|chunk| chunk_rms(chunk) >= config.rms_threshold)
}

fn chunk_rms(samples: &[f32]) -> f32 {
    let energy_sum = samples.iter().map(|value| value * value).sum::<f32>();
    (energy_sum / samples.len() as f32).sqrt()
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

    #[test]
    fn detects_quieter_signal_with_default_threshold() {
        let config = VadConfig::default();
        let quiet = create_chunk(0.015);
        assert!(has_speech(&quiet, &config));
    }

    #[test]
    fn detects_short_speech_burst_inside_long_chunk() {
        let config = VadConfig::default();
        let mut chunk = vec![0.0_f32; 16_000];
        for sample in chunk.iter_mut().skip(3_200).take(1_024) {
            *sample = 0.04;
        }

        assert!(has_speech(&chunk, &config));
    }

    #[test]
    fn rejects_low_noise_across_windows() {
        let config = VadConfig::default();
        let noise = vec![0.003_f32; 16_000];
        assert!(!has_speech(&noise, &config));
    }
}
