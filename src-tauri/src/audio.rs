pub const SAMPLE_RATE_HZ: u32 = 16_000;
pub const CHANNELS: u16 = 1;

pub fn validate_audio_format(sample_rate_hz: u32, channels: u16) -> Result<(), String> {
    if sample_rate_hz != SAMPLE_RATE_HZ {
        return Err(format!(
            "invalid sample rate: expected {SAMPLE_RATE_HZ}, got {sample_rate_hz}"
        ));
    }
    if channels != CHANNELS {
        return Err(format!("invalid channel count: expected {CHANNELS}, got {channels}"));
    }
    Ok(())
}

pub fn pcm_i16_to_f32(samples: &[i16]) -> Vec<f32> {
    const SCALE: f32 = i16::MAX as f32;
    samples
        .iter()
        .map(|sample| f32::from(*sample) / SCALE)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_target_audio_format() {
        assert!(validate_audio_format(16_000, 1).is_ok());
        assert!(validate_audio_format(48_000, 1).is_err());
        assert!(validate_audio_format(16_000, 2).is_err());
    }

    #[test]
    fn converts_pcm_i16_to_float_range() {
        let output = pcm_i16_to_f32(&[i16::MIN, 0, i16::MAX]);
        assert_eq!(output.len(), 3);
        assert!(output[0] < 0.0);
        assert_eq!(output[1], 0.0);
        assert!(output[2] > 0.99);
    }
}
