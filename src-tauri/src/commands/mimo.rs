// MiMo TTS Commands — MiMo 配置管理

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::state::app_state::AppState;
use crate::http::tts_client;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MimoSettings {
    pub mimo_voice: String,
    pub mimo_model: String,
    pub mimo_api_base: String,
    pub mimo_style_prompt: String,
    pub mimo_voice_design: String,
    pub mimo_clone_audio_path: String,
    pub mimo_optimize_text: bool,
    pub available_models: Vec<(String, String)>,
    pub style_presets: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MimoSettingsUpdate {
    pub mimo_voice: Option<String>,
    pub mimo_model: Option<String>,
    pub mimo_api_key: Option<String>,
    pub mimo_api_base: Option<String>,
    pub mimo_style_prompt: Option<String>,
    pub mimo_voice_design: Option<String>,
    pub mimo_clone_audio_path: Option<String>,
    pub mimo_optimize_text: Option<bool>,
}

/// 获取 MiMo 设置
#[tauri::command]
pub async fn get_mimo_settings(state: State<'_, AppState>) -> Result<MimoSettings, String> {
    let settings = state.settings.read().await;

    let models: Vec<(String, String)> = tts_client::MIMO_MODELS
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    let presets: Vec<(String, String)> = tts_client::MIMO_STYLE_PRESETS
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    Ok(MimoSettings {
        mimo_voice: settings.mimo_style_prompt.clone(), // 简化: 复用 style_prompt 字段
        mimo_model: settings.mimo_model.clone(),
        mimo_api_base: settings.mimo_api_base.clone(),
        mimo_style_prompt: settings.mimo_style_prompt.clone(),
        mimo_voice_design: settings.mimo_voice_design.clone(),
        mimo_clone_audio_path: settings.mimo_clone_audio_path.clone(),
        mimo_optimize_text: settings.mimo_optimize_text,
        available_models: models,
        style_presets: presets,
    })
}

/// 更新 MiMo 设置
#[tauri::command]
pub async fn update_mimo_settings(
    update: MimoSettingsUpdate,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut settings = state.settings.write().await;

    if let Some(v) = update.mimo_voice { settings.mimo_style_prompt = v; }
    if let Some(v) = update.mimo_model {
        // 验证模型名称
        let valid_models: Vec<String> = tts_client::MIMO_MODELS
            .iter()
            .map(|(_, v)| v.to_string())
            .collect();
        if valid_models.contains(&v) {
            settings.mimo_model = v;
        } else {
            return Err(format!("Invalid mimo model: {}", v));
        }
    }
    if let Some(v) = update.mimo_api_key { settings.mimo_api_key = v; }
    if let Some(v) = update.mimo_api_base { settings.mimo_api_base = v; }
    if let Some(v) = update.mimo_style_prompt { settings.mimo_style_prompt = v; }
    if let Some(v) = update.mimo_voice_design { settings.mimo_voice_design = v; }
    if let Some(v) = update.mimo_clone_audio_path { settings.mimo_clone_audio_path = v; }
    if let Some(v) = update.mimo_optimize_text { settings.mimo_optimize_text = v; }

    drop(settings); // 释放写锁

    // 自动保存配置
    state.save_config()?;

    Ok(())
}

/// 列出 MiMo 可用模型
#[tauri::command]
pub async fn list_mimo_models() -> Result<Vec<(String, String)>, String> {
    Ok(tts_client::MIMO_MODELS
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect())
}

/// 列出 MiMo 风格预设
#[tauri::command]
pub async fn list_style_presets() -> Result<Vec<(String, String)>, String> {
    Ok(tts_client::MIMO_STYLE_PRESETS
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect())
}