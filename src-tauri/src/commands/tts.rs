// TTS Commands — 语音合成相关 IPC 命令

use serde::{Deserialize, Serialize};
use tauri::{Emitter, State};

use crate::state::app_state::AppState;
use crate::http::tts_client;
use crate::http::tts_client::TTS_DEFAULT_HOST;
use crate::audio::output::AudioOutput;
use crate::audio::resample::{resample_pcm, get_device_sample_rate};
use cpal::traits::{DeviceTrait, HostTrait};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TtsEngineInfo {
    pub id: String,
    pub name: String,
    pub port: u16,
    pub desc: String,
    pub healthy: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SynthesizeResult {
    pub success: bool,
    pub audio_base64: Option<String>,
    pub text: String,
    pub engine: String,
    pub error: Option<String>,
}

/// TTS 合成 (文本 → 音频) — 普通模式
#[tauri::command]
pub async fn synthesize(
    text: String,
    tts_engine: String,
    emotion_tags: Option<Vec<String>>,
    _output_device_id: Option<i64>,
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<SynthesizeResult, String> {
    let settings = state.settings.read().await;
    let client = &state.http_client;
    {
        let mut busy = state.tts_busy.write().await;
        *busy = true;
    }
    app.emit("pipeline_status", "tts").ok();

    let mut text = text;
    if settings.sexy_afterglow {
        text = format!("{} 야~", text.trim_end());
    }

    let start = std::time::Instant::now();
    let result = match tts_engine.as_str() {
        "mimo_tts" => {
            let model = &settings.mimo_model;
            let style_prompt = if settings.mimo_director_enabled && !settings.mimo_style_prompt.is_empty() {
                Some(settings.mimo_style_prompt.as_str())
            } else {
                None
            };
            let voice_design = if model == "mimo-v2.5-tts-voicedesign" && !settings.mimo_voice_design.is_empty() {
                Some(settings.mimo_voice_design.as_str())
            } else {
                None
            };
            let voice = if model == "mimo-v2.5-tts" {
                Some("mimo_default")
            } else {
                None
            };

            let mut final_text = text.clone();
            if let Some(ref tags) = emotion_tags {
                if !tags.is_empty() {
                    let emotion_style = tts_client::get_mimo_style_from_emotion(&tags[0]);
                    if !emotion_style.is_empty() {
                        final_text = format!("{}{}", emotion_style, final_text);
                    }
                }
            }

            let clone_path = if model == "mimo-v2.5-tts-voiceclone" && !settings.mimo_clone_audio_path.is_empty() {
                Some(settings.mimo_clone_audio_path.as_str())
            } else {
                None
            };

            tts_client::mimo_synthesize(tts_client::MimoSynthParams {
                client,
                api_key: &settings.mimo_api_key,
                api_base: &settings.mimo_api_base,
                text: &final_text,
                model,
                style_prompt,
                voice_design,
                voice,
                optimize_text: settings.mimo_optimize_text,
                clone_audio_path: clone_path,
            })
            .await
        }
        engine_name => {
            let port = tts_client::TTS_ENGINE_PORTS
                .iter()
                .find(|(name, _)| *name == engine_name)
                .map(|(_, port)| *port)
                .ok_or_else(|| format!("Unknown TTS engine: {}", engine_name))?;

            tts_client::fish_speech_synthesize(tts_client::FishSynthParams {
                client,
                host: TTS_DEFAULT_HOST,
                port,
                text: &text,
                reference_id: settings.voice_reference_id.as_deref(),
                top_p: settings.tts_top_p,
                temperature: settings.tts_temperature,
                repetition_penalty: settings.tts_repetition_penalty,
                chunk_length: settings.tts_chunk_length,
            })
            .await
        }
    };

    {
        let mut busy = state.tts_busy.write().await;
        *busy = false;
    }

    match result {
        Ok(audio_bytes) => {
            let elapsed_ms = start.elapsed().as_millis() as u64;
            {
                let mut stats = state.stats.write().await;
                stats.tts_count += 1;
                stats.total_tts_ms += elapsed_ms;
            }
            if settings.auto_save_audio {
                let output_dir = resolve_output_dir(&app);
                let _ = std::fs::create_dir_all(&output_dir);
                let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
                let path = output_dir.join(format!("tts_{}.wav", timestamp));
                if let Err(e) = std::fs::write(&path, &audio_bytes) {
                    log::warn!("Auto-save failed: {}", e);
                } else {
                    log::info!("Auto-saved to {}", path.display());
                }
            }
            log::info!("───────────────────────────────────────────────────");
            log::info!("【Step 5】TTS 合成完成 ({}ms)", elapsed_ms);
            log::info!("  音频大小: {} bytes WAV", audio_bytes.len());
            log::info!("  引擎: {}", tts_engine);
            let audio_b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &audio_bytes);
            app.emit("pipeline_status", "playing").ok();
            Ok(SynthesizeResult {
                success: true,
                audio_base64: Some(audio_b64),
                text: text.clone(),
                engine: tts_engine.clone(),
                error: None,
            })
        }
        Err(e) => {
            app.emit("pipeline_status", "idle").ok();
            Ok(SynthesizeResult {
                success: false,
                audio_base64: None,
                text,
                engine: tts_engine,
                error: Some(e),
            })
        }
    }
}

/// TTS 合成 — 导演模式（使用导演生成的 MiMo 消息）
#[tauri::command]
pub async fn synthesize_directed(
    user_content: String,
    assistant_content: String,
    optimize_text: bool,
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<SynthesizeResult, String> {
    let settings = state.settings.read().await;
    let auto_save = settings.auto_save_audio;
    let client = &state.http_client;
    {
        let mut busy = state.tts_busy.write().await;
        *busy = true;
    }
    app.emit("pipeline_status", "tts").ok();

    let mut assistant_content = assistant_content;
    if settings.sexy_afterglow {
        assistant_content = format!("{} 야~", assistant_content.trim_end());
    }

    let start = std::time::Instant::now();
    let result = tts_client::mimo_synthesize_directed(
        client,
        &settings.mimo_api_key,
        &settings.mimo_api_base,
        &settings.mimo_model,
        &user_content,
        &assistant_content,
        optimize_text,
    )
    .await;

    {
        let mut busy = state.tts_busy.write().await;
        *busy = false;
    }

    match result {
        Ok(audio_bytes) => {
            let elapsed_ms = start.elapsed().as_millis() as u64;
            {
                let mut stats = state.stats.write().await;
                stats.tts_count += 1;
                stats.total_tts_ms += elapsed_ms;
            }
            if auto_save {
                let output_dir = resolve_output_dir(&app);
                let _ = std::fs::create_dir_all(&output_dir);
                let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
                let path = output_dir.join(format!("tts_{}.wav", timestamp));
                if let Err(e) = std::fs::write(&path, &audio_bytes) {
                    log::warn!("Auto-save failed: {}", e);
                } else {
                    log::info!("Auto-saved to {}", path.display());
                }
            }
            log::info!("───────────────────────────────────────────────────");
            log::info!("【Step 5】TTS 导演模式合成完成 ({}ms)", elapsed_ms);
            log::info!("  音频大小: {} bytes WAV", audio_bytes.len());
            let audio_b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &audio_bytes);
            app.emit("pipeline_status", "playing").ok();
            Ok(SynthesizeResult {
                success: true,
                audio_base64: Some(audio_b64),
                text: assistant_content.clone(),
                engine: "mimo_tts_directed".to_string(),
                error: None,
            })
        }
        Err(e) => {
            app.emit("pipeline_status", "idle").ok();
            Ok(SynthesizeResult {
                success: false,
                audio_base64: None,
                text: assistant_content,
                engine: "mimo_tts_directed".to_string(),
                error: Some(e),
            })
        }
    }
}

#[tauri::command]
pub async fn get_tts_engines(
    state: State<'_, AppState>,
) -> Result<Vec<TtsEngineInfo>, String> {
    let client = &state.http_client;

    let mut engines = vec![TtsEngineInfo {
        id: "mimo_tts".to_string(),
        name: "MiMo TTS (云端)".to_string(),
        port: 0,
        desc: "小米云端 TTS，预置音色/音色设计/音色复刻".to_string(),
        healthy: !state.settings.read().await.mimo_api_key.is_empty(),
    }];

    for (id, port) in tts_client::TTS_ENGINE_PORTS {
        let healthy = tts_client::check_tts_health(client, TTS_DEFAULT_HOST, *port).await;
        engines.push(TtsEngineInfo {
            id: id.to_string(),
            name: match *id {
                "fish_speech_s2" => "Fish Speech S2",
                "f5_tts" => "F5-TTS",
                "kokoro" => "Kokoro",
                "chat_tts" => "ChatTTS",
                _ => id,
            }.to_string(),
            port: *port,
            desc: match *id {
                "fish_speech_s2" => "DualAR 架构，情绪表现力强",
                "f5_tts" => "零样本克隆，Flow Matching",
                "kokoro" => "82M 轻量引擎，多语言",
                "chat_tts" => "中文对话优化，韵律控制",
                _ => "",
            }.to_string(),
            healthy,
        });
    }
    Ok(engines)
}

#[tauri::command]
pub async fn check_tts_health(
    engine: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    if engine == "mimo_tts" {
        return Ok(!state.settings.read().await.mimo_api_key.is_empty());
    }

    let client = &state.http_client;
    let port = tts_client::TTS_ENGINE_PORTS
        .iter()
        .find(|(name, _)| *name == engine)
        .map(|(_, port)| *port)
        .ok_or_else(|| format!("Unknown TTS engine: {}", engine))?;

    Ok(tts_client::check_tts_health(client, TTS_DEFAULT_HOST, port).await)
}

#[tauri::command]
pub async fn play_to_device(
    audio_base64: String,
    device_name: Option<String>,
) -> Result<(), String> {
    let audio_data = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &audio_base64)
        .map_err(|e| format!("Failed to decode audio base64: {}", e))?;

    AudioOutput::play_wav_to_device(&audio_data, device_name.as_deref())?;

    Ok(())
}

#[tauri::command]
pub async fn browse_clone_audio(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let path = app.dialog().file()
        .add_filter("Audio", &["wav", "mp3", "flac", "ogg"])
        .blocking_pick_file();
    Ok(path.and_then(|p| p.as_path().map(|pp| pp.to_string_lossy().to_string())))
}

#[tauri::command]
pub async fn test_mimo_connection(
    api_key: String,
    api_base: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let client = &state.http_client;
    let url = format!("{}/models", api_base.trim_end_matches('/'));

    let resp = client
        .get(&url)
        .header("api-key", api_key)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| format!("Connection failed: {}", e))?;

    Ok(resp.status().is_success())
}

#[tauri::command]
pub async fn resample_audio(
    audio_base64: String,
    source_sample_rate: u32,
    device_name: Option<String>,
) -> Result<String, String> {
    let audio_data = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &audio_base64)
        .map_err(|e| format!("Failed to decode audio base64: {}", e))?;

    let target_rate = if let Some(ref name) = device_name {
        get_device_sample_rate(name).unwrap_or(source_sample_rate)
    } else {
        let host = cpal::default_host();
        host.default_output_device()
            .and_then(|d| d.default_output_config().ok())
            .map(|c| c.sample_rate().0)
            .unwrap_or(source_sample_rate)
    };

    let resampled = resample_pcm(&audio_data, source_sample_rate, target_rate);

    Ok(base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &resampled))
}

fn resolve_output_dir(_app: &tauri::AppHandle) -> std::path::PathBuf {
    let exe_dir = std::env::current_exe().ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    exe_dir.join("output")
}
