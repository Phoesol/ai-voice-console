use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use std::time::Duration;

fn emotion_regex() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"<\|([A-Z_]+)\|>").unwrap_or_else(|_| regex::Regex::new("").unwrap()))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AsrResult {
    pub text: String,
    pub language: String,
    pub emotion_tags: Vec<String>,
    pub confidence: f64,
    pub latency_ms: u64,
}

pub async fn transcribe(
    client: &reqwest::Client,
    asr_url: &str,
    audio_data: &[u8],
    sample_rate: u32,
    engine: &str,
    _device: &str,
) -> Result<AsrResult, String> {
    let start = std::time::Instant::now();

    let url = format!("{}/v1/audio/transcriptions", asr_url.trim_end_matches('/'));

    let model_name = match engine {
        "sensevoice" => "iic/SenseVoiceSmall",
        "faster_whisper" => "large-v3",
        "paraformer" => "iic/speech_paraformer-large_asr_nat-zh-cn-16k-common-vocab8404-pytorch",
        _ => "Qwen3-ASR-1.7B",
    };

    let (file_bytes, file_name, mime_type) = if is_wav_audio(audio_data) {
        log::info!("[ASR] Detected WAV format, sending as-is ({} bytes)", audio_data.len());
        (audio_data.to_vec(), "audio.wav", "audio/wav")
    } else if is_webm_audio(audio_data) {
        log::info!("[ASR] Detected WebM format, sending as-is ({} bytes)", audio_data.len());
        (audio_data.to_vec(), "audio.webm", "audio/webm")
    } else if is_mp4_audio(audio_data) {
        log::info!("[ASR] Detected MP4 format, sending as-is ({} bytes)", audio_data.len());
        (audio_data.to_vec(), "audio.mp4", "audio/mp4")
    } else {
        let wav_bytes = crate::audio::pcm_to_wav(audio_data, sample_rate, 1);
        log::info!("[ASR] Wrapping PCM as WAV: {} raw bytes → {} WAV bytes, sr={}", audio_data.len(), wav_bytes.len(), sample_rate);
        (wav_bytes, "audio.wav", "audio/wav")
    };

    let part = reqwest::multipart::Part::bytes(file_bytes)
        .file_name(file_name.to_string())
        .mime_str(mime_type)
        .map_err(|e| format!("MIME error: {}", e))?;

    let form = reqwest::multipart::Form::new()
        .text("model", model_name.to_string())
        .text("response_format", "json")
        .part("file", part);

    let resp = client
        .post(&url)
        .multipart(form)
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| format!("ASR request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        let err_msg = if body.starts_with("<!") || body.starts_with("<html") {
            format!("HTTP {}", status)
        } else {
            body.chars().take(200).collect()
        };
        return Err(format!("ASR server error ({}): {}", status, err_msg));
    }

    let vllm_result: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse ASR response: {}", e))?;

    log::info!("  ASR 原始响应: {}", vllm_result);

    let raw_text = vllm_result
        .get("text")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let emotion_pattern = emotion_regex();
    let emotion_tags: Vec<String> = emotion_pattern.captures_iter(&raw_text)
        .filter_map(|c| c.get(1).map(|m| format!("<|{}|>", m.as_str())))
        .collect();
    let text = emotion_pattern.replace_all(&raw_text, "").trim().to_string();

    let language = vllm_result
        .get("language")
        .and_then(|v| v.as_str())
        .unwrap_or("zh")
        .to_string();

    let latency_ms = start.elapsed().as_millis() as u64;

    Ok(AsrResult {
        text,
        language,
        emotion_tags,
        confidence: 1.0,
        latency_ms,
    })
}

pub async fn health_check(
    client: &reqwest::Client,
    asr_url: &str,
) -> bool {
    let url = format!("{}/health", asr_url.trim_end_matches('/'));
    client
        .get(&url)
        .timeout(Duration::from_secs(3))
        .send()
        .await
        .map(|resp| resp.status().is_success())
        .unwrap_or(false)
}


fn is_wav_audio(data: &[u8]) -> bool {
    data.len() >= 4 && &data[0..4] == b"RIFF"
}

fn is_webm_audio(data: &[u8]) -> bool {
    data.len() >= 4 && &data[0..4] == b"\x1a\x45\xdf\xa3"
}

fn is_mp4_audio(data: &[u8]) -> bool {
    if data.len() < 8 {
        return false;
    }
    let ftyp_offset = u32::from_be_bytes([data[4], data[5], data[6], data[7]]) as usize;
    if ftyp_offset + 4 < data.len() && &data[ftyp_offset..ftyp_offset + 4] == b"ftyp" {
        return true;
    }
    if data.len() >= 12 && &data[4..8] == b"ftyp" {
        return true;
    }
    false
}
