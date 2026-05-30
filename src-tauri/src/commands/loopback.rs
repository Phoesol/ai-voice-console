use tauri::{Emitter, State};
use crate::state::app_state::AppState;

#[tauri::command]
pub async fn start_loopback_capture(
    device_name: Option<String>,
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<String, String> {
    let capture = state.loopback_capture.read().await;
    if capture.is_running() {
        return Err("Loopback capture already running".to_string());
    }
    drop(capture);

    let capture = state.loopback_capture.read().await;
    let device = device_name.unwrap_or_default();

    let app_handle = app.clone();
    capture.start(&device, Box::new(move |samples: &[f32], channels: u32| {
        let _ = app_handle.emit("loopback_audio", serde_json::json!({
            "samples": samples.len(),
            "channels": channels,
        }));
    }))?;

    Ok("Loopback capture started".to_string())
}

#[tauri::command]
pub async fn stop_loopback_capture(
    state: State<'_, AppState>,
) -> Result<String, String> {
    let capture = state.loopback_capture.read().await;
    capture.stop()?;
    Ok("Loopback capture stopped".to_string())
}

#[tauri::command]
pub async fn get_loopback_audio(
    state: State<'_, AppState>,
) -> Result<Vec<f32>, String> {
    let capture = state.loopback_capture.read().await;
    Ok(capture.get_buffered_audio())
}

#[tauri::command]
pub async fn loopback_status(
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let capture = state.loopback_capture.read().await;
    Ok(serde_json::json!({
        "running": capture.is_running(),
        "sampleRate": capture.sample_rate(),
        "channels": capture.channels(),
    }))
}
