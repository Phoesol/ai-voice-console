// Sidecar Control Commands — ASR Sidecar 进程管理

use tauri::State;
use crate::state::app_state::AppState;

/// 启动 ASR Sidecar
#[tauri::command]
pub async fn start_asr_sidecar(
    port: Option<u16>,
    engine: Option<String>,
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<String, String> {
    let mut sidecar = state.asr_sidecar.write().await;

    if let Some(p) = port {
        sidecar.set_port(p);
    }

    let engine = engine.unwrap_or_else(|| "qwen3_asr".to_string());
    sidecar.start(&app, &engine, &state.http_client).await?;
    Ok(format!("ASR Sidecar started on port {}", sidecar.port()))
}

/// 停止 ASR Sidecar
#[tauri::command]
pub async fn stop_asr_sidecar(state: State<'_, AppState>) -> Result<String, String> {
    let mut sidecar = state.asr_sidecar.write().await;
    sidecar.stop()?;
    Ok("ASR Sidecar stopped".to_string())
}

/// 获取 Sidecar 状态
#[tauri::command]
pub async fn sidecar_status(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let sidecar = state.asr_sidecar.read().await;
    Ok(serde_json::json!({
        "running": sidecar.is_running(),
        "port": sidecar.port(),
        "api_url": sidecar.api_url(),
    }))
}