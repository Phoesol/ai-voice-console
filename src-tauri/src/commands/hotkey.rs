use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::collections::HashSet;
use tauri::AppHandle;
use tauri::Emitter;

pub struct HotkeyState {
    pub enabled: AtomicBool,
    pub hk1_key: parking_lot::Mutex<String>,
    pub hk1_mod: parking_lot::Mutex<String>,
    pub hk2_key: parking_lot::Mutex<String>,
    pressed: AtomicBool,
    active_sources: parking_lot::Mutex<HashSet<String>>,
    pub app: parking_lot::Mutex<Option<AppHandle>>,
}

impl HotkeyState {
    pub fn new() -> Self {
        Self {
            enabled: AtomicBool::new(false),
            hk1_key: parking_lot::Mutex::new(String::new()),
            hk1_mod: parking_lot::Mutex::new("none".to_string()),
            hk2_key: parking_lot::Mutex::new("none".to_string()),
            pressed: AtomicBool::new(false),
            active_sources: parking_lot::Mutex::new(HashSet::new()),
            app: parking_lot::Mutex::new(None),
        }
    }
}

static HOTKEY_STATE: once_cell::sync::Lazy<HotkeyState> = once_cell::sync::Lazy::new(HotkeyState::new);

#[cfg(target_os = "windows")]
mod win_hook {
    use super::*;
    use windows::Win32::Foundation::*;
    use windows::Win32::UI::WindowsAndMessaging::*;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;

    static mut KEYBOARD_HOOK: Option<HHOOK> = None;
    static mut MOUSE_HOOK: Option<HHOOK> = None;
    static HOOK_INSTALLED: AtomicBool = AtomicBool::new(false);
    static HK_EVENT_COUNT: AtomicUsize = AtomicUsize::new(0);

    fn is_valid_key(key: &str) -> bool {
        !key.is_empty() && key != "none"
    }

    fn matches_hotkey(key_name: &str, hk_key: &str, hk_mod: &str, check_mod: bool) -> bool {
        if !is_valid_key(hk_key) { return false; }
        if key_name != hk_key { return false; }
        if check_mod { super::check_modifier(hk_mod) } else { true }
    }

    fn update_active_sources(sources: Vec<&'static str>, is_down: bool) {
        let mut active = HOTKEY_STATE.active_sources.lock();
        let was_idle = active.is_empty();

        if is_down {
            for source in sources {
                active.insert(source.to_string());
            }
            HOTKEY_STATE.pressed.store(!active.is_empty(), Ordering::Relaxed);
            if was_idle && !active.is_empty() {
                HK_EVENT_COUNT.fetch_add(1, Ordering::Relaxed);
                if let Some(ref app) = *HOTKEY_STATE.app.lock() {
                    let _ = app.emit("hotkey_down", "any");
                }
            }
        } else {
            for source in sources {
                active.remove(source);
            }
            let now_idle = active.is_empty();
            HOTKEY_STATE.pressed.store(!now_idle, Ordering::Relaxed);
            if !was_idle && now_idle {
                if let Some(ref app) = *HOTKEY_STATE.app.lock() {
                    let _ = app.emit("hotkey_up", "any");
                }
            }
        }
    }

    unsafe extern "system" fn keyboard_proc(n_code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
        if n_code >= 0 && HOTKEY_STATE.enabled.load(Ordering::Relaxed) {
            let kb = &*(l_param.0 as *const KBDLLHOOKSTRUCT);
            let is_down = w_param.0 as u32 == WM_KEYDOWN || w_param.0 as u32 == WM_SYSKEYDOWN;
            let is_up = w_param.0 as u32 == WM_KEYUP || w_param.0 as u32 == WM_SYSKEYUP;

            if is_down || is_up {
                let key_name = super::vk_to_name(kb.vkCode);
                let hk1_key = HOTKEY_STATE.hk1_key.lock().clone();
                let hk1_mod = HOTKEY_STATE.hk1_mod.lock().clone();
                let hk2_key = HOTKEY_STATE.hk2_key.lock().clone();

                let hit1 = matches_hotkey(&key_name, &hk1_key, &hk1_mod, true);
                let hit2 = matches_hotkey(&key_name, &hk2_key, "", false);
                let mut sources = Vec::new();
                if hit1 { sources.push("hk1"); }
                if hit2 { sources.push("hk2"); }

                if !sources.is_empty() {
                    update_active_sources(sources, is_down);
                }
            }
        }
        let hook = unsafe { KEYBOARD_HOOK.unwrap_or_default() };
        CallNextHookEx(hook, n_code, w_param, l_param)
    }

    unsafe extern "system" fn mouse_proc(n_code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
        if n_code >= 0 && HOTKEY_STATE.enabled.load(Ordering::Relaxed) {
            let msg = w_param.0 as u32;
            let mouse_key = match msg {
                WM_MBUTTONDOWN | WM_MBUTTONUP => Some("middle_mouse"),
                WM_LBUTTONDOWN | WM_LBUTTONUP => Some("mouse_left"),
                WM_RBUTTONDOWN | WM_RBUTTONUP => Some("mouse_right"),
                _ => None,
            };
            let is_down = matches!(msg, WM_MBUTTONDOWN | WM_LBUTTONDOWN | WM_RBUTTONDOWN);
            let is_up = matches!(msg, WM_MBUTTONUP | WM_LBUTTONUP | WM_RBUTTONUP);

            if let Some(mouse_key) = mouse_key {
                let hk1_key = HOTKEY_STATE.hk1_key.lock().clone();
                let hk1_mod = HOTKEY_STATE.hk1_mod.lock().clone();
                let hk2_key = HOTKEY_STATE.hk2_key.lock().clone();

                let hit1 = matches_hotkey(mouse_key, &hk1_key, &hk1_mod, true);
                let hit2 = matches_hotkey(mouse_key, &hk2_key, "", false);
                let mut sources = Vec::new();
                if hit1 { sources.push("hk1"); }
                if hit2 { sources.push("hk2"); }

                if !sources.is_empty() && (is_down || is_up) {
                    update_active_sources(sources, is_down);
                }
            }
        }
        let hook = unsafe { MOUSE_HOOK.unwrap_or_default() };
        CallNextHookEx(hook, n_code, w_param, l_param)
    }

    pub fn is_installed() -> bool {
        HOOK_INSTALLED.load(Ordering::Relaxed)
    }

    pub fn event_count() -> usize {
        HK_EVENT_COUNT.load(Ordering::Relaxed)
    }

    pub fn reset_event_count() -> usize {
        HK_EVENT_COUNT.swap(0, Ordering::Relaxed)
    }

    pub fn install_hooks(app: AppHandle) -> Result<(), String> {
        if HOOK_INSTALLED.load(Ordering::Relaxed) {
            *HOTKEY_STATE.app.lock() = Some(app);
            return Ok(());
        }

        *HOTKEY_STATE.app.lock() = Some(app);

        std::thread::spawn(move || {
            unsafe {
                let module = GetModuleHandleW(None).unwrap_or_default();

                let kb_hook = SetWindowsHookExW(
                    WH_KEYBOARD_LL,
                    Some(keyboard_proc),
                    module,
                    0,
                );
                let ms_hook = SetWindowsHookExW(
                    WH_MOUSE_LL,
                    Some(mouse_proc),
                    module,
                    0,
                );

                match (&kb_hook, &ms_hook) {
                    (Ok(kh), Ok(mh)) => {
                        KEYBOARD_HOOK = Some(*kh);
                        MOUSE_HOOK = Some(*mh);
                        HOOK_INSTALLED.store(true, Ordering::Relaxed);
                        log::info!("[热键] Global hooks installed");
                    }
                    _ => {
                        log::error!("[热键] Failed to install hooks");
                        if let Ok(kh) = kb_hook { let _ = UnhookWindowsHookEx(kh); }
                        if let Ok(mh) = ms_hook { let _ = UnhookWindowsHookEx(mh); }
                        return;
                    }
                }

                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }

                if let Some(kh) = KEYBOARD_HOOK { let _ = UnhookWindowsHookEx(kh); }
                if let Some(mh) = MOUSE_HOOK { let _ = UnhookWindowsHookEx(mh); }
                KEYBOARD_HOOK = None;
                MOUSE_HOOK = None;
                HOOK_INSTALLED.store(false, Ordering::Relaxed);
                log::info!("[热键] Hooks removed");
            }
        });

        Ok(())
    }
}

#[cfg(not(target_os = "windows"))]
mod win_hook {
    use super::*;
    pub fn install_hooks(_app: AppHandle) -> Result<(), String> {
        Err("Global hotkeys not supported on this platform".to_string())
    }
}

fn vk_to_name(vk_code: u32) -> String {
    match vk_code {
        0x08 => "backspace".to_string(),
        0x09 => "tab".to_string(),
        0x0D => "enter".to_string(),
        0x10 => "shift".to_string(),
        0x11 => "ctrl".to_string(),
        0x12 => "alt".to_string(),
        0x13 => "pause".to_string(),
        0x14 => "capslock".to_string(),
        0x1B => "escape".to_string(),
        0x20 => "space".to_string(),
        0x21 => "pageup".to_string(),
        0x22 => "pagedown".to_string(),
        0x23 => "end".to_string(),
        0x24 => "home".to_string(),
        0x25 => "arrowleft".to_string(),
        0x26 => "arrowup".to_string(),
        0x27 => "arrowright".to_string(),
        0x28 => "arrowdown".to_string(),
        0x2C => "printscreen".to_string(),
        0x2D => "insert".to_string(),
        0x2E => "delete".to_string(),
        0x70..=0x87 => format!("f{}", vk_code - 0x70 + 1),
        vk if (0x30..=0x39).contains(&vk) => {
            let c = ((vk - 0x30) as u8 + b'0') as char;
            c.to_lowercase().to_string()
        }
        vk if (0x41..=0x5A).contains(&vk) => {
            let c = ((vk - 0x41) as u8 + b'a') as char;
            c.to_string()
        }
        _ => format!("vk_{}", vk_code),
    }
}

fn check_modifier(mod_str: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
        match mod_str {
            "ctrl" => (unsafe { GetAsyncKeyState(0x11) } as u16) & 0x8000 != 0,
            "shift" => (unsafe { GetAsyncKeyState(0x10) } as u16) & 0x8000 != 0,
            "alt" => (unsafe { GetAsyncKeyState(0x12) } as u16) & 0x8000 != 0,
            _ => true,
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = mod_str;
        true
    }
}

pub fn install_global_hotkey(app: AppHandle) -> Result<(), String> {
    win_hook::install_hooks(app)
}

pub fn update_hotkey_config(hk1_key: &str, hk1_mod: &str, hk2_key: &str) {
    *HOTKEY_STATE.hk1_key.lock() = hk1_key.to_lowercase();
    *HOTKEY_STATE.hk1_mod.lock() = hk1_mod.to_lowercase();
    *HOTKEY_STATE.hk2_key.lock() = hk2_key.to_lowercase();
    HOTKEY_STATE.pressed.store(false, Ordering::Relaxed);
    HOTKEY_STATE.active_sources.lock().clear();
    log::info!("[热键] Config updated: hk1={} (mod={}), hk2={}", hk1_key, hk1_mod, hk2_key);
}

pub fn set_hotkey_enabled(enabled: bool) {
    HOTKEY_STATE.enabled.store(enabled, Ordering::Relaxed);
    log::info!("[热键] Enabled: {}", enabled);
}

#[tauri::command]
pub async fn configure_hotkey(
    hk1_key: String,
    hk1_mod: String,
    hk2_key: String,
    enabled: bool,
    _state: tauri::State<'_, crate::state::app_state::AppState>,
    app: AppHandle,
) -> Result<bool, String> {
    let prev_count = win_hook::reset_event_count();
    log::info!("[热键] configure: hk1={} mod={}, hk2={}, enabled={}, prev_events={}", hk1_key, hk1_mod, hk2_key, enabled, prev_count);
    update_hotkey_config(&hk1_key, &hk1_mod, &hk2_key);
    set_hotkey_enabled(enabled);
    if enabled {
        install_global_hotkey(app)?;
        log::info!("[热键] 全局钩子已安装 (hk1={}, hk2={})", hk1_key, hk2_key);
    }
    Ok(true)
}

#[tauri::command]
pub fn get_hotkey_diag() -> serde_json::Value {
    let count = win_hook::event_count();
    let enabled = HOTKEY_STATE.enabled.load(Ordering::Relaxed);
    let hk1 = HOTKEY_STATE.hk1_key.lock().clone();
    let hk2 = HOTKEY_STATE.hk2_key.lock().clone();
    serde_json::json!({
        "enabled": enabled,
        "hk1_key": hk1,
        "hk2_key": hk2,
        "event_count": count,
        "hook_installed": win_hook::is_installed(),
    })
}
