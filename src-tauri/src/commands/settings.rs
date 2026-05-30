// Settings Commands — 配置读写

use tauri::State;

use crate::state::app_state::AppState;
use crate::state::settings::{Settings, SettingsUpdate};

/// 获取当前配置
#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<Settings, String> {
    let settings = state.settings.read().await;
    Ok(settings.clone())
}

/// 保存配置 (部分更新)
#[tauri::command]
pub async fn save_settings(
    update: SettingsUpdate,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut settings = state.settings.write().await;
    update.apply_to(&mut settings);
    drop(settings); // 释放写锁

    // 持久化到 config.json
    state.save_config()?;

    Ok(())
}

/// 重置配置为默认值
#[tauri::command]
pub async fn reset_settings(state: State<'_, AppState>) -> Result<(), String> {
    let mut settings = state.settings.write().await;
    *settings = Settings::default();
    drop(settings);

    state.save_config()?;

    Ok(())
}