use tauri::{Emitter, State};

use crate::state::app_state::AppState;

#[tauri::command]
pub async fn start_listening(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    {
        let mut running = state.pipeline_running.write().await;
        if *running {
            return Err("Pipeline already running".to_string());
        }
        *running = true;
    }

    {
        let mut sidecar = state.asr_sidecar.write().await;
        if !sidecar.is_running() {
            let engine = state.asr_engine.read().await.clone();
            sidecar.start(&app, &engine, &state.http_client).await?;
        }
    }

    app.emit("pipeline_status", "listening").ok();
    log::info!("[Voice] Listening started");
    Ok(())
}

#[tauri::command]
pub async fn stop_listening(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    {
        let mut running = state.pipeline_running.write().await;
        *running = false;
    }
    app.emit("pipeline_status", "idle").ok();
    log::info!("[Voice] Listening stopped");
    Ok(())
}
