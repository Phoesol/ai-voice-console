// Pipeline Commands — 管道控制

use serde::{Deserialize, Serialize};
use tauri::{Emitter, State};

use crate::state::app_state::AppState;
use crate::state::app_state::PipelineStats;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineConfig {
    pub wasapi_device: Option<String>,
    pub vad_threshold: f64,
    pub vad_mode: Option<String>,
    pub min_speech_duration: f64,
    pub max_speech_duration: f64,
    pub translate_enabled: bool,
    pub translate_target_lang: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineStatus {
    pub running: bool,
    pub asr_loaded: bool,
    pub asr_engine: String,
    pub tts_busy: bool,
    pub ptt_active: bool,
}

/// 启动系统管道 (WASAPI 内录 → VAD → ASR → LLM 翻译 → 字幕)
#[tauri::command]
pub async fn start_pipeline(
    config: PipelineConfig,
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let mut running = state.pipeline_running.write().await;
    if *running {
        return Ok(()); // 已运行
    }
    *running = true;

    // 更新配置
    {
        let mut settings = state.settings.write().await;
        settings.vad_threshold = config.vad_threshold;
        settings.translate_enabled = config.translate_enabled;
        if let Some(lang) = config.translate_target_lang {
            settings.translate_target_lang = lang;
        }
    }

    // 确保 ASR Sidecar 在运行
    {
        let mut sidecar = state.asr_sidecar.write().await;
        if !sidecar.is_running() {
            let engine = state.asr_engine.read().await.clone();
            sidecar.start(&app, &engine, &state.http_client).await?;
        }
    }

    app.emit("pipeline_status", "running").ok();
    log::info!("Pipeline started");

    // TODO: 启动管道线程:
    // 1. cpal WASAPI loopback 采集
    // 2. VAD 检测
    // 3. 音频发送到 ASR Sidecar
    // 4. ASR 结果 → 翻译（可选）
    // 5. 通过 event 推送字幕到前端

    Ok(())
}

/// 停止系统管道
#[tauri::command]
pub async fn stop_pipeline(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let mut running = state.pipeline_running.write().await;
    *running = false;
    drop(running);

    app.emit("pipeline_status", "stopped").ok();
    log::info!("Pipeline stopped");
    Ok(())
}

/// 获取管道状态
#[tauri::command]
pub async fn get_pipeline_status(state: State<'_, AppState>) -> Result<PipelineStatus, String> {
    let running = *state.pipeline_running.read().await;
    let asr_loaded = *state.asr_loaded.read().await;
    let asr_engine = state.asr_engine.read().await.clone();
    let tts_busy = *state.tts_busy.read().await;
    let ptt_active = *state.ptt_active.read().await;

    Ok(PipelineStatus {
        running,
        asr_loaded,
        asr_engine,
        tts_busy,
        ptt_active,
    })
}

#[tauri::command]
pub async fn get_pipeline_stats(state: State<'_, AppState>) -> Result<PipelineStats, String> {
    let stats = state.stats.read().await;
    Ok((*stats).clone())
}