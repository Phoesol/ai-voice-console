use serde::{Deserialize, Serialize};
use cpal::traits::{DeviceTrait, HostTrait};
use crate::audio::output::AudioOutput;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDevice {
    pub id: u32,
    pub name: String,
    pub api: String,
    pub api_index: u32,
    pub channels: u16,
    pub is_input: bool,
    pub is_output: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDevices {
    pub input: Vec<AudioDevice>,
    pub output: Vec<AudioDevice>,
    pub host_apis: Vec<HostApi>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostApi {
    pub index: u32,
    pub name: String,
    pub device_count: u32,
}

#[tauri::command]
pub async fn list_audio_devices() -> Result<AudioDevices, String> {
    let host = cpal::default_host();

    let mut input_devices: Vec<AudioDevice> = Vec::new();

    if let Ok(devices) = host.input_devices() {
        for (i, device) in devices.enumerate() {
            let name = device.name().unwrap_or_else(|_| "Unknown".to_string());
            let channels = device
                .default_input_config()
                .map(|c| c.channels())
                .unwrap_or(1);
            input_devices.push(AudioDevice {
                id: i as u32,
                name,
                api: "Windows WASAPI".to_string(),
                api_index: 2,
                channels,
                is_input: true,
                is_output: false,
            });
        }
    }

    let output_devices = AudioOutput::list_output_devices();

    let total_devices = (input_devices.len() + output_devices.len()) as u32;

    let host_apis = vec![
        HostApi {
            index: 2,
            name: "Windows WASAPI".to_string(),
            device_count: total_devices,
        },
        HostApi {
            index: 0,
            name: "MME".to_string(),
            device_count: 0,
        },
        HostApi {
            index: 1,
            name: "Windows DirectSound".to_string(),
            device_count: 0,
        },
        HostApi {
            index: 3,
            name: "ASIO".to_string(),
            device_count: 0,
        },
    ];

    Ok(AudioDevices {
        input: input_devices,
        output: output_devices,
        host_apis,
    })
}
