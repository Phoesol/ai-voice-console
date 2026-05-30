// PTT Commands — Push-to-Talk 控制

use tauri::State;
use crate::state::app_state::AppState;

/// 启动 PTT 监听
#[tauri::command]
pub async fn start_ptt(state: State<'_, AppState>) -> Result<bool, String> {
    let mut ptt_active = state.ptt_active.write().await;
    *ptt_active = true;
    log::info!("PTT started");
    Ok(true)
}

/// 停止 PTT 监听
#[tauri::command]
pub async fn stop_ptt(state: State<'_, AppState>) -> Result<bool, String> {
    let mut ptt_active = state.ptt_active.write().await;
    *ptt_active = false;
    log::info!("PTT stopped");
    Ok(false)
}

/// 获取 PTT 状态
#[tauri::command]
pub async fn ptt_status(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(*state.ptt_active.read().await)
}

// 注意: 全局按键钩子 (鼠标中键/键盘) 需要在前端 webview 中通过
// mousedown/mouseup/keydown/keyup 事件监听, 或者使用 Rust 端的
// rdev crate 进行全局钩子 (用于游戏内 PTT).
//
// 游戏内 PTT 的完整实现:
// 1. 前端: 页面可见时, 监听 document 的 keydown/keyup 事件
// 2. Rust 后端: 使用 rdev crate 注册全局快捷键 (用于游戏内不可见时)
//
// 当前骨架仅管理 PTT 状态, 实际按键监听在前端实现