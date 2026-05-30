// ASR Commands — 语音识别相关 IPC 命令
// qwen-asr-serve 是 vLLM 服务器，启动时自动加载模型，无需单独 load 步骤

use serde::{Deserialize, Serialize};
use tauri::{Emitter, State};
use base64::Engine;

use crate::state::app_state::AppState;
use crate::http::asr_client;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AsrStatus {
    pub loaded: bool,
    pub engine: String,
    pub sidecar_running: bool,
}

#[tauri::command]
pub async fn load_asr(
    engine: String,
    _device: Option<String>,
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<AsrStatus, String> {
    {
        let mut sidecar = state.asr_sidecar.write().await;
        if !sidecar.is_running() {
            sidecar.start(&app, &engine, &state.http_client).await?;
        }
    }

    let mut asr_engine = state.asr_engine.write().await;
    *asr_engine = engine.clone();
    let mut asr_loaded = state.asr_loaded.write().await;
    *asr_loaded = true;

    app.emit("pipeline_status", "asr_loaded").ok();

    let sidecar_running = state.asr_sidecar.read().await.is_running();
    Ok(AsrStatus {
        loaded: true,
        engine,
        sidecar_running,
    })
}

#[tauri::command]
pub async fn transcribe_audio(
    audio_base64: String,
    sample_rate: Option<u32>,
    _engine: Option<String>,
    _device: Option<String>,
    state: State<'_, AppState>,
) -> Result<asr_client::AsrResult, String> {
    let sample_rate = sample_rate.unwrap_or(16000);

    if !*state.asr_loaded.read().await {
        return Err("ASR engine not loaded. Please wait for auto-load or click Load button.".to_string());
    }

    let audio_data = base64::engine::general_purpose::STANDARD
        .decode(&audio_base64)
        .map_err(|e| format!("Failed to decode audio base64: {}", e))?;

    log::info!("[ASR] Received audio: {} bytes base64 → {} bytes raw, sample_rate={}", 
        audio_base64.len(), audio_data.len(), sample_rate);

    if audio_data.is_empty() {
        return Err("Audio data is empty after base64 decode".to_string());
    }

    if audio_data.len() < 100 {
        log::warn!("[ASR] Audio data too small ({} bytes), likely no speech", audio_data.len());
    }

    let asr_url = {
        let sidecar = state.asr_sidecar.read().await;
        sidecar.api_url().to_string()
    };

    let engine = state.asr_engine.read().await.clone();

    let start = std::time::Instant::now();
    let result = asr_client::transcribe(
        &state.http_client,
        &asr_url,
        &audio_data,
        sample_rate,
        &engine,
        "cuda",
    )
    .await?;
    let elapsed_ms = start.elapsed().as_millis() as u64;

    {
        let mut stats = state.stats.write().await;
        stats.asr_count += 1;
        stats.total_asr_ms += elapsed_ms;
    }

    log::info!("ASR result: {} (lang={})", result.text, result.language);

    Ok(result)
}

#[tauri::command]
pub async fn get_asr_status(state: State<'_, AppState>) -> Result<AsrStatus, String> {
    let loaded = *state.asr_loaded.read().await;
    let engine = state.asr_engine.read().await.clone();
    let sidecar_running = state.asr_sidecar.read().await.is_running();

    Ok(AsrStatus {
        loaded,
        engine,
        sidecar_running,
    })
}

#[tauri::command]
pub async fn stop_asr(state: State<'_, AppState>) -> Result<(), String> {
    let mut sidecar = state.asr_sidecar.write().await;
    sidecar.stop()?;

    let mut loaded = state.asr_loaded.write().await;
    *loaded = false;

    log::info!("ASR engine stopped");
    Ok(())
}
