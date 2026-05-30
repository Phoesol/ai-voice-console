use cpal::traits::{HostTrait, DeviceTrait};

pub fn resample_pcm(data: &[u8], from_rate: u32, to_rate: u32) -> Vec<u8> {
    if from_rate == to_rate || data.is_empty() {
        return data.to_vec();
    }

    let samples_i16 = data
        .chunks_exact(2)
        .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();

    let ratio = to_rate as f64 / from_rate as f64;
    let new_len = (samples_i16.len() as f64 * ratio) as usize;

    let mut result = Vec::with_capacity(new_len);
    for i in 0..new_len {
        let src_pos = i as f64 / ratio;
        let idx = src_pos as usize;
        let frac = src_pos - idx as f64;

        let s0 = samples_i16.get(idx).copied().unwrap_or(0);
        let s1 = samples_i16.get(idx + 1).copied().unwrap_or(0);
        let interpolated = s0 as f64 + (s1 as f64 - s0 as f64) * frac;
        let val = interpolated.clamp(i16::MIN as f64, i16::MAX as f64) as i16;
        result.extend_from_slice(&val.to_le_bytes());
    }

    result
}

pub fn get_device_sample_rate(device_name: &str) -> Option<u32> {
    let host = cpal::default_host();

    let mut output_devices = host.output_devices().ok()?;
    let device = output_devices.find(|d| {
        d.name()
            .map(|n| n.contains(device_name))
            .unwrap_or(false)
    })?;

    device.default_output_config().ok().map(|c| c.sample_rate().0)
}

