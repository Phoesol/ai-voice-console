// TTS HTTP 客户端 — MiMo TTS (云端) + Fish Speech (本地)
// 所有 TTS API 调用均在 Rust 端完成, 避免 CORS 和 API Key 泄露

use serde::{Deserialize, Serialize};
use std::time::Duration;

const MIMO_TIMEOUT_SECS: u64 = 60;
const FISH_TIMEOUT_SECS: u64 = 60;
const HEALTH_CHECK_TIMEOUT_SECS: u64 = 3;
const FISH_DEFAULT_MAX_TOKENS: u32 = 512;
pub const TTS_DEFAULT_HOST: &str = "127.0.0.1";

// ============================================================
// MiMo TTS (云端 API — chat/completions 接口)
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct MimoTtsRequest {
    pub model: String,
    pub messages: Vec<MimoMessage>,
    pub audio: MimoAudioConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MimoMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MimoAudioConfig {
    pub format: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub voice: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub optimize_text_preview: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ref_audio: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct MimoTtsChatResponse {
    pub choices: Vec<MimoChoice>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct MimoChoice {
    pub message: MimoResponseMessage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct MimoResponseMessage {
    pub audio: Option<MimoResponseAudio>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct MimoResponseAudio {
    pub data: Option<String>,
    pub format: Option<String>,
}

pub const MIMO_EMOTION_MAP: &[(&str, &str)] = &[
    ("<|HAPPY|>", "(开心)"),
    ("<|SAD|>", "(悲伤)"),
    ("<|ANGRY|>", "(愤怒)"),
    ("<|NEUTRAL|>", ""),
    ("<|SURPRISED|>", "(惊讶)"),
    ("<|FEARFUL|>", "(恐惧)"),
    ("<|LAUGHTER|>", "(轻笑)"),
];

pub fn get_mimo_style_from_emotion(emotion_tag: &str) -> &'static str {
    MIMO_EMOTION_MAP
        .iter()
        .find(|(tag, _)| *tag == emotion_tag)
        .map(|(_, style)| *style)
        .unwrap_or("")
}

pub const MIMO_STYLE_PRESETS: &[(&str, &str)] = &[
    ("auto", "（自动，随情绪切换）"),
    ("natural", "用自然流畅的语气朗读以下内容"),
    ("lively", "用活泼可爱的语气说话，充满感染力"),
    ("gentle", "用温柔舒缓的语气说话，像在对好朋友倾诉"),
    ("narration", "用播音腔朗读以下内容，专业正式"),
    ("playful", "用调皮俏皮的语气说话，像在开玩笑"),
    ("emotional", "用富有感情的语气说话，情感充沛表达丰富"),
];

pub const MIMO_MODELS: &[(&str, &str)] = &[
    ("preset", "mimo-v2.5-tts"),
    ("voicedesign", "mimo-v2.5-tts-voicedesign"),
    ("voiceclone", "mimo-v2.5-tts-voiceclone"),
];



pub struct MimoSynthParams<'a> {
    pub client: &'a reqwest::Client,
    pub api_key: &'a str,
    pub api_base: &'a str,
    pub text: &'a str,
    pub model: &'a str,
    pub style_prompt: Option<&'a str>,
    pub voice_design: Option<&'a str>,
    pub voice: Option<&'a str>,
    pub optimize_text: bool,
    pub clone_audio_path: Option<&'a str>,
}

/// 调用 MiMo TTS API (chat/completions 接口)
pub async fn mimo_synthesize(params: MimoSynthParams<'_>) -> Result<Vec<u8>, String> {
    let MimoSynthParams { client, api_key, api_base, text, model, style_prompt, voice_design, voice, optimize_text, clone_audio_path } = params;
    let url = format!("{}/chat/completions", api_base.trim_end_matches('/'));

    let user_content = match model {
        "mimo-v2.5-tts-voicedesign" => {
            let vd = voice_design.unwrap_or("年轻女性，温柔甜美的声音");
            if let Some(sp) = style_prompt {
                format!("{}\n\n{}", vd, sp)
            } else {
                vd.to_string()
            }
        }
        _ => {
            style_prompt.unwrap_or("").to_string()
        }
    };

    let mut messages = Vec::new();

    if !user_content.is_empty() {
        messages.push(MimoMessage {
            role: "user".to_string(),
            content: user_content,
        });
    }

    messages.push(MimoMessage {
        role: "assistant".to_string(),
        content: text.to_string(),
    });

    let mut audio_config = MimoAudioConfig {
        format: "wav".to_string(),
        voice: None,
        optimize_text_preview: None,
        ref_audio: None,
    };

    if let Some(v) = voice {
        audio_config.voice = Some(v.to_string());
    }

    audio_config.optimize_text_preview = Some(false);

    if model == "mimo-v2.5-tts-voiceclone" {
        if let Some(path) = clone_audio_path {
            if !path.is_empty() {
                let audio_bytes = std::fs::read(path)
                    .map_err(|e| format!("Failed to read clone audio file: {}", e))?;
                use base64::Engine;
                audio_config.ref_audio = Some(base64::engine::general_purpose::STANDARD.encode(&audio_bytes));
            }
        }
    }

    let payload = MimoTtsRequest {
        model: model.to_string(),
        messages,
        audio: audio_config,
        stream: None,
    };

    log::info!("───────────────────────────────────────────────────");
    log::info!("【Step 4】MiMo TTS 普通模式请求");
    log::info!("  URL: {}", url);
    log::info!("  Model: {}", model);
    log::info!("  文本: {}", text);
    log::info!("  voice: {:?}", voice);
    log::info!("  optimizeText: {}", optimize_text);

    let resp = client
        .post(&url)
        .header("api-key", api_key.to_string())
        .header("Content-Type", "application/json")
        .json(&payload)
        .timeout(Duration::from_secs(MIMO_TIMEOUT_SECS))
        .send()
        .await
        .map_err(|e| format!("MiMo TTS request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("MiMo TTS API error ({}): {}", status, body));
    }

    let resp_json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse MiMo TTS response: {}", e))?;

    if let Some(audio_data) = resp_json
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("audio"))
        .and_then(|a| a.get("data"))
        .and_then(|d| d.as_str())
    {
        use base64::Engine;
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(audio_data)
            .map_err(|e| format!("Failed to decode MiMo audio base64: {}", e))?;
        log::info!("【Step 4】MiMo TTS 普通模式成功: {} bytes WAV", decoded.len());
        Ok(decoded)
    } else {
        Err(format!("MiMo TTS response missing audio data: {}", resp_json))
    }
}

/// 导演模式调用 MiMo TTS：直接使用导演生成的 user/assistant 消息
pub async fn mimo_synthesize_directed(
    client: &reqwest::Client,
    api_key: &str,
    api_base: &str,
    model: &str,
    user_content: &str,
    assistant_content: &str,
    optimize_text: bool,
) -> Result<Vec<u8>, String> {
    let url = format!("{}/chat/completions", api_base.trim_end_matches('/'));

    let mut messages = Vec::new();

    if !user_content.is_empty() {
        messages.push(MimoMessage {
            role: "user".to_string(),
            content: user_content.to_string(),
        });
    }

    messages.push(MimoMessage {
        role: "assistant".to_string(),
        content: assistant_content.to_string(),
    });

    let mut audio_config = MimoAudioConfig {
        format: "wav".to_string(),
        voice: None,
        optimize_text_preview: None,
        ref_audio: None,
    };

    audio_config.optimize_text_preview = Some(false);

    let payload = MimoTtsRequest {
        model: model.to_string(),
        messages,
        audio: audio_config,
        stream: None,
    };

    log::info!("───────────────────────────────────────────────────");
    log::info!("【Step 4】MiMo TTS 导演模式请求");
    log::info!("  URL: {}", url);
    log::info!("  Model: {}", model);
    log::info!("  userContent: {}", user_content);
    log::info!("  assistantContent: {}", assistant_content);
    log::info!("  optimizeText: {}", optimize_text);

    let resp = client
        .post(&url)
        .header("api-key", api_key.to_string())
        .header("Content-Type", "application/json")
        .json(&payload)
        .timeout(Duration::from_secs(MIMO_TIMEOUT_SECS))
        .send()
        .await
        .map_err(|e| format!("MiMo TTS directed request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("MiMo TTS directed API error ({}): {}", status, body));
    }

    let resp_json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse MiMo TTS directed response: {}", e))?;

    if let Some(audio_data) = resp_json
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("audio"))
        .and_then(|a| a.get("data"))
        .and_then(|d| d.as_str())
    {
        use base64::Engine;
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(audio_data)
            .map_err(|e| format!("Failed to decode MiMo audio base64: {}", e))?;
        log::info!("【Step 4】MiMo TTS 导演模式成功: {} bytes WAV", decoded.len());
        Ok(decoded)
    } else {
        Err(format!("MiMo TTS directed response missing audio data: {}", resp_json))
    }
}

// ============================================================
// Fish Speech TTS (本地 HTTP API)
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub struct FishSpeechTtsRequest {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub references: Option<serde_json::Value>,
    pub max_new_tokens: u32,
    pub chunk_length: u32,
    pub top_p: f64,
    pub temperature: f64,
    pub repetition_penalty: f64,
}

pub const TTS_ENGINE_PORTS: &[(&str, u16)] = &[
    ("fish_speech_s2", 18080),
    ("f5_tts", 18081),
    ("kokoro", 18082),
    ("chat_tts", 18083),
];

pub struct FishSynthParams<'a> {
    pub client: &'a reqwest::Client,
    pub host: &'a str,
    pub port: u16,
    pub text: &'a str,
    pub reference_id: Option<&'a str>,
    pub top_p: f64,
    pub temperature: f64,
    pub repetition_penalty: f64,
    pub chunk_length: u32,
}

pub async fn fish_speech_synthesize(params: FishSynthParams<'_>) -> Result<Vec<u8>, String> {
    let FishSynthParams { client, host, port, text, reference_id, top_p, temperature, repetition_penalty, chunk_length } = params;
    let url = format!("http://{}:{}/v1/tts", host, port);

    let payload = serde_json::json!({
        "text": text,
        "max_new_tokens": FISH_DEFAULT_MAX_TOKENS,
        "chunk_length": chunk_length,
        "top_p": top_p,
        "temperature": temperature,
        "repetition_penalty": repetition_penalty,
        "reference_id": reference_id,
    });

    let resp = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&payload)
        .timeout(Duration::from_secs(FISH_TIMEOUT_SECS))
        .send()
        .await
        .map_err(|e| format!("Fish Speech TTS request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Fish Speech TTS error ({}): {}", status, body));
    }

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("Failed to read Fish Speech response: {}", e))?;

    Ok(bytes.to_vec())
}

pub async fn check_tts_health(
    client: &reqwest::Client,
    host: &str,
    port: u16,
) -> bool {
    let url = format!("http://{}:{}/v1/health", host, port);
    client
        .get(&url)
        .timeout(Duration::from_secs(HEALTH_CHECK_TIMEOUT_SECS))
        .send()
        .await
        .map(|resp| resp.status().is_success())
        .unwrap_or(false)
}
