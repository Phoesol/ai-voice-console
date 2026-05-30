use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use super::resample::resample_pcm;

pub struct AudioOutput {
    _private: (),
}

impl AudioOutput {

    pub fn play_wav_to_device(wav_data: &[u8], device_name: Option<&str>) -> Result<(), String> {
        let (sample_rate, _channels, audio_data) = parse_wav(wav_data)?;

        let host = cpal::default_host();
        let device = if let Some(name) = device_name {
            host.output_devices()
                .map_err(|e| format!("Failed to enumerate output devices: {}", e))?
                .find(|d| d.name().map(|n| n.contains(name)).unwrap_or(false))
                .ok_or_else(|| format!("Output device '{}' not found", name))?
        } else {
            host.default_output_device()
                .ok_or("No default output device available")?
        };

        let config = device
            .default_output_config()
            .map_err(|e| format!("Failed to get output config: {}", e))?;

        let device_sample_rate = config.sample_rate().0;
        let audio_data = if device_sample_rate != sample_rate {
            let raw_pcm: Vec<u8> = audio_data
                .iter()
                .flat_map(|&s| s.to_le_bytes())
                .collect();
            let resampled = resample_pcm(&raw_pcm, sample_rate, device_sample_rate);
            resampled
                .chunks_exact(2)
                .map(|c| i16::from_le_bytes([c[0], c[1]]))
                .collect()
        } else {
            audio_data
        };

        let actual_rate = device_sample_rate;
        let samples_f32 = pcm_i16_to_f32(&audio_data);
        let num_channels = config.channels() as usize;

        let err_fn = |err: cpal::StreamError| {
            log::error!("Audio output stream error: {}", err);
        };

        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => {
                let data = samples_f32.clone();
                device.build_output_stream(
                    &config.into(),
                    move |output: &mut [f32], _: &cpal::OutputCallbackInfo| {
                        for (i, frame) in output.chunks_mut(num_channels).enumerate() {
                            let sample = data.get(i).copied().unwrap_or(0.0f32);
                            for ch in frame.iter_mut() {
                                *ch = sample;
                            }
                        }
                    },
                    err_fn,
                    None,
                )
            }
            cpal::SampleFormat::I16 => {
                let data = audio_data.clone();
                device.build_output_stream(
                    &config.into(),
                    move |output: &mut [i16], _: &cpal::OutputCallbackInfo| {
                        for (i, frame) in output.chunks_mut(num_channels).enumerate() {
                            let sample = data.get(i).copied().unwrap_or(0i16);
                            for ch in frame.iter_mut() {
                                *ch = sample;
                            }
                        }
                    },
                    err_fn,
                    None,
                )
            }
            cpal::SampleFormat::U16 => {
                let data: Vec<u16> = audio_data.iter().map(|&s| (s as i32 + 32768) as u16).collect();
                device.build_output_stream(
                    &config.into(),
                    move |output: &mut [u16], _: &cpal::OutputCallbackInfo| {
                        for (i, frame) in output.chunks_mut(num_channels).enumerate() {
                            let sample = data.get(i).copied().unwrap_or(32768u16);
                            for ch in frame.iter_mut() {
                                *ch = sample;
                            }
                        }
                    },
                    err_fn,
                    None,
                )
            }
            fmt => return Err(format!("Unsupported sample format: {:?}", fmt)),
        }
        .map_err(|e| format!("Failed to build output stream: {}", e))?;

        stream
            .play()
            .map_err(|e| format!("Failed to play stream: {}", e))?;

        let duration_secs = audio_data.len() as f64 / actual_rate as f64;
        let wait_ms = (duration_secs * 1000.0) as u64 + 500;
        std::thread::sleep(std::time::Duration::from_millis(wait_ms));

        Ok(())
    }

    pub fn list_output_devices() -> Vec<crate::commands::devices::AudioDevice> {
        let host = cpal::default_host();
        host.output_devices()
            .map(|iter| {
                iter.enumerate()
                    .filter_map(|(i, d)| {
                        d.name().ok().map(|name| crate::commands::devices::AudioDevice {
                            id: i as u32,
                            name,
                            api: "WASAPI".to_string(),
                            api_index: 2,
                            channels: d.default_output_config().map(|c| c.channels()).unwrap_or(2),
                            is_input: false,
                            is_output: true,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

}

fn parse_wav(data: &[u8]) -> Result<(u32, u16, Vec<i16>), String> {
    if data.len() < 44 {
        return Err("WAV data too short".to_string());
    }
    if &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        return Err("Invalid WAV header".to_string());
    }

    let sample_rate = u32::from_le_bytes([data[24], data[25], data[26], data[27]]);
    let num_channels = u16::from_le_bytes([data[22], data[23]]);
    let bits_per_sample = u16::from_le_bytes([data[34], data[35]]);

    let mut offset = 12usize;
    let mut audio_data = Vec::new();

    while offset + 8 <= data.len() {
        let chunk_id = &data[offset..offset + 4];
        let chunk_size = u32::from_le_bytes([
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ]) as usize;

        if chunk_id == b"data" {
            let start = offset + 8;
            let end = (start + chunk_size).min(data.len());
            let raw = &data[start..end];

            if bits_per_sample == 16 {
                audio_data = raw
                    .chunks_exact(2)
                    .map(|c| i16::from_le_bytes([c[0], c[1]]))
                    .collect();
            } else if bits_per_sample == 32 {
                audio_data = raw
                    .chunks_exact(4)
                    .map(|c| {
                        let f = f32::from_le_bytes([c[0], c[1], c[2], c[3]]);
                        (f * 32767.0).clamp(-32768.0, 32767.0) as i16
                    })
                    .collect();
            }
            break;
        }
        offset += 8 + chunk_size;
        if chunk_size % 2 == 1 {
            offset += 1;
        }
    }

    if audio_data.is_empty() {
        return Err("No audio data found in WAV".to_string());
    }

    Ok((sample_rate, num_channels, audio_data))
}

fn pcm_i16_to_f32(data: &[i16]) -> Vec<f32> {
    data.iter().map(|&s| s as f32 / 32768.0).collect()
}
