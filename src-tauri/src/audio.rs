pub const SAMPLE_RATE_HZ: u32 = 16_000;
pub const CHANNELS: u16 = 1;

#[cfg(feature = "desktop")]
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
#[cfg(feature = "desktop")]
use cpal::{SampleFormat, Stream};
#[cfg(feature = "desktop")]
use serde::Serialize;
#[cfg(feature = "desktop")]
use std::sync::mpsc::SyncSender;

pub fn validate_audio_format(sample_rate_hz: u32, channels: u16) -> Result<(), String> {
    if sample_rate_hz != SAMPLE_RATE_HZ {
        return Err(format!(
            "invalid sample rate: expected {SAMPLE_RATE_HZ}, got {sample_rate_hz}"
        ));
    }
    if channels != CHANNELS {
        return Err(format!(
            "invalid channel count: expected {CHANNELS}, got {channels}"
        ));
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

#[cfg(feature = "desktop")]
#[derive(Debug, Clone, Serialize)]
pub struct InputMicrophone {
    pub id: String,
    pub label: String,
    pub is_default: bool,
}

#[cfg(feature = "desktop")]
#[derive(Debug, Clone, Copy, Serialize)]
pub struct MicLevel {
    pub level: f32,
    pub peak: f32,
    pub active: bool,
}

#[cfg(feature = "desktop")]
pub struct LiveInputStream {
    pub stream: Stream,
    pub sample_rate_hz: u32,
}

#[cfg(feature = "desktop")]
pub fn list_input_microphones() -> Result<Vec<InputMicrophone>, String> {
    let host = cpal::default_host();
    let default_name = host
        .default_input_device()
        .and_then(|device| device.name().ok());
    let devices = host
        .input_devices()
        .map_err(|error| format!("failed to enumerate input devices: {error}"))?;

    let mut microphones = Vec::new();
    for (index, device) in devices.enumerate() {
        let label = device
            .name()
            .unwrap_or_else(|_| format!("Microphone {}", index + 1));
        let is_default = default_name.as_deref() == Some(label.as_str());
        microphones.push(InputMicrophone {
            id: index.to_string(),
            label,
            is_default,
        });
    }

    Ok(microphones)
}

#[cfg(feature = "desktop")]
pub fn build_live_input_stream(
    microphone_id: Option<&str>,
    frame_tx: SyncSender<Vec<f32>>,
) -> Result<LiveInputStream, String> {
    let host = cpal::default_host();
    let device = resolve_input_device(&host, microphone_id)?;
    let supported = device
        .default_input_config()
        .map_err(|error| format!("failed to get default input config: {error}"))?;

    let sample_format = supported.sample_format();
    let stream_config = supported.config();
    let sample_rate_hz = stream_config.sample_rate.0;
    let channels = usize::from(stream_config.channels.max(1));

    let error_callback = move |error| {
        eprintln!("live input stream error: {error}");
    };

    let stream = match sample_format {
        SampleFormat::F32 => {
            let tx = frame_tx.clone();
            device
                .build_input_stream(
                    &stream_config,
                    move |data: &[f32], _| {
                        let mono = interleaved_f32_to_mono(data, channels);
                        let _ = tx.try_send(mono);
                    },
                    error_callback,
                    None,
                )
                .map_err(|error| format!("failed to build f32 input stream: {error}"))?
        }
        SampleFormat::I16 => {
            let tx = frame_tx.clone();
            device
                .build_input_stream(
                    &stream_config,
                    move |data: &[i16], _| {
                        let mono = interleaved_i16_to_mono(data, channels);
                        let _ = tx.try_send(mono);
                    },
                    error_callback,
                    None,
                )
                .map_err(|error| format!("failed to build i16 input stream: {error}"))?
        }
        SampleFormat::U16 => {
            let tx = frame_tx.clone();
            device
                .build_input_stream(
                    &stream_config,
                    move |data: &[u16], _| {
                        let mono = interleaved_u16_to_mono(data, channels);
                        let _ = tx.try_send(mono);
                    },
                    error_callback,
                    None,
                )
                .map_err(|error| format!("failed to build u16 input stream: {error}"))?
        }
        _ => {
            return Err(format!(
                "unsupported input sample format: {:?}",
                sample_format
            ));
        }
    };

    stream
        .play()
        .map_err(|error| format!("failed to start input stream: {error}"))?;

    Ok(LiveInputStream {
        stream,
        sample_rate_hz,
    })
}

#[cfg(feature = "desktop")]
pub fn downsample_to_16k(input: &[f32], source_sample_rate_hz: u32) -> Vec<f32> {
    if source_sample_rate_hz == SAMPLE_RATE_HZ {
        return input.to_vec();
    }

    if source_sample_rate_hz < SAMPLE_RATE_HZ {
        return Vec::new();
    }

    let ratio = source_sample_rate_hz as f32 / SAMPLE_RATE_HZ as f32;
    let output_length = (input.len() as f32 / ratio).floor() as usize;
    let mut output = Vec::with_capacity(output_length);

    let mut position = 0usize;
    for index in 0..output_length {
        let next_position = (((index + 1) as f32 * ratio).floor() as usize).min(input.len());
        let mut sum = 0f32;
        let mut count = 0usize;
        for sample in &input[position..next_position] {
            sum += *sample;
            count += 1;
        }
        output.push(if count > 0 { sum / count as f32 } else { 0.0 });
        position = next_position;
    }

    output
}

#[cfg(feature = "desktop")]
pub fn measure_mic_level(samples: &[f32], previous_level: f32, previous_peak: f32) -> MicLevel {
    if samples.is_empty() {
        return MicLevel {
            level: 0.0,
            peak: previous_peak * 0.96,
            active: false,
        };
    }

    let mut energy_sum = 0f32;
    let mut peak = 0f32;
    for sample in samples {
        let absolute = sample.abs();
        energy_sum += sample * sample;
        if absolute > peak {
            peak = absolute;
        }
    }

    let rms = (energy_sum / samples.len() as f32).sqrt();
    let scaled_level = (rms * 14.0).clamp(0.0, 1.0);
    let level = if scaled_level >= previous_level {
        scaled_level
    } else {
        previous_level * 0.84 + scaled_level * 0.16
    };
    let combined_peak = (previous_peak * 0.96).max(peak);
    let active = level > 0.08 || peak > 0.12;

    MicLevel {
        level,
        peak: combined_peak,
        active,
    }
}

#[cfg(feature = "desktop")]
fn resolve_input_device(
    host: &cpal::Host,
    microphone_id: Option<&str>,
) -> Result<cpal::Device, String> {
    if let Some(raw_id) = microphone_id {
        let trimmed = raw_id.trim();
        if !trimmed.is_empty() {
            let index = trimmed
                .parse::<usize>()
                .map_err(|_| format!("invalid microphone id: {trimmed}"))?;
            let devices = host
                .input_devices()
                .map_err(|error| format!("failed to enumerate input devices: {error}"))?
                .collect::<Vec<_>>();
            if let Some(device) = devices.into_iter().nth(index) {
                return Ok(device);
            }
            return Err(format!("microphone not found for id {trimmed}"));
        }
    }

    if let Some(default) = host.default_input_device() {
        return Ok(default);
    }

    host.input_devices()
        .map_err(|error| format!("failed to enumerate input devices: {error}"))?
        .next()
        .ok_or_else(|| "no input microphone is available".to_string())
}

#[cfg(feature = "desktop")]
fn interleaved_f32_to_mono(input: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return input.to_vec();
    }

    let mut output = Vec::with_capacity(input.len() / channels);
    for frame in input.chunks_exact(channels) {
        let sum = frame.iter().copied().sum::<f32>();
        output.push(sum / channels as f32);
    }
    output
}

#[cfg(feature = "desktop")]
fn interleaved_i16_to_mono(input: &[i16], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return pcm_i16_to_f32(input);
    }

    let scale = i16::MAX as f32;
    let mut output = Vec::with_capacity(input.len() / channels);
    for frame in input.chunks_exact(channels) {
        let mut sum = 0f32;
        for sample in frame {
            sum += *sample as f32 / scale;
        }
        output.push(sum / channels as f32);
    }
    output
}

#[cfg(feature = "desktop")]
fn interleaved_u16_to_mono(input: &[u16], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return input
            .iter()
            .map(|sample| (*sample as f32 / u16::MAX as f32) * 2.0 - 1.0)
            .collect();
    }

    let mut output = Vec::with_capacity(input.len() / channels);
    for frame in input.chunks_exact(channels) {
        let mut sum = 0f32;
        for sample in frame {
            sum += (*sample as f32 / u16::MAX as f32) * 2.0 - 1.0;
        }
        output.push(sum / channels as f32);
    }
    output
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

    #[cfg(feature = "desktop")]
    #[test]
    fn downsamples_from_48k_to_16k() {
        let input = vec![0.5_f32; 4_800];
        let output = downsample_to_16k(&input, 48_000);
        assert_eq!(output.len(), 1_600);
    }

    #[cfg(feature = "desktop")]
    #[test]
    fn returns_empty_when_source_rate_is_below_target() {
        let input = vec![0.5_f32; 2_400];
        let output = downsample_to_16k(&input, 8_000);
        assert!(output.is_empty());
    }

    #[cfg(feature = "desktop")]
    #[test]
    fn computes_mic_levels() {
        let samples = vec![0.2_f32; 1024];
        let level = measure_mic_level(&samples, 0.0, 0.0);
        assert!(level.level > 0.0);
        assert!(level.peak > 0.0);
        assert!(level.active);
    }

    #[cfg(feature = "desktop")]
    #[test]
    fn decays_peak_when_silent() {
        let silent = measure_mic_level(&[], 0.0, 0.75);
        assert!(!silent.active);
        assert!(silent.peak < 0.75);
        assert!(silent.peak > 0.70);
    }

    #[cfg(feature = "desktop")]
    #[test]
    fn averages_interleaved_f32_channels_to_mono() {
        let stereo = vec![0.2_f32, 0.6_f32, -0.2_f32, 0.2_f32];
        let mono = interleaved_f32_to_mono(&stereo, 2);
        assert_eq!(mono, vec![0.4_f32, 0.0_f32]);
    }
}
