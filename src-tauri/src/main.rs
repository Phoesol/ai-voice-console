#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod constants;
mod commands;
mod http;
mod engine;
mod state;
mod audio;

use tauri::Manager;
use state::app_state::AppState;
use std::net::TcpListener;

#[cfg(target_os = "windows")]
fn kill_all_child_processes() {
    let my_pid = std::process::id();
    log::info!("Window destroyed (PID {}), killing child processes", my_pid);

    std::thread::spawn(move || {
        let output = std::process::Command::new("wmic")
            .args([
                "process",
                "where",
                &format!("ParentProcessId={}", my_pid),
                "get",
                "ProcessId",
                "/format:list",
            ])
            .output();

        if let Ok(out) = output {
            let text = String::from_utf8_lossy(&out.stdout);
            for line in text.lines() {
                if let Some(pid_str) = line.strip_prefix("ProcessId=") {
                    if let Ok(pid) = pid_str.trim().parse::<u32>() {
                        log::info!("Killing child process tree (PID {})", pid);
                        let _ = std::process::Command::new("taskkill")
                            .args(["/F", "/T", "/PID", &pid.to_string()])
                            .output();
                    }
                }
            }
        }
    });
}

#[cfg(not(target_os = "windows"))]
fn kill_all_child_processes() {}

fn setup_logging() {
    let app_root = state::app_state::resolve_app_root();
    let log_dir = app_root.join(constants::LOG_DIR);
    let _ = std::fs::create_dir_all(&log_dir);

    let date_str = chrono::Local::now().format("%Y-%m-%d").to_string();
    let log_path = log_dir.join(format!("{}.log", date_str));

    let file_logger = fern::log_file(&log_path).unwrap_or_else(|e| {
        eprintln!("Failed to create log file {:?}: {}", log_path, e);
        let fallback = app_root.join("app.log");
        fern::log_file(&fallback).unwrap_or_else(|e2| {
            eprintln!("Fallback log also failed: {}", e2);
            fern::log_file(app_root.join("fallback.log")).unwrap()
        })
    });

    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{} {} {}] {}",
                chrono::Local::now().format("%H:%M:%S%.3f"),
                record.level(),
                record.target(),
                message
            ))
        })
        .level(log::LevelFilter::Info)
        .chain(std::io::stderr())
        .chain(file_logger)
        .apply()
        .unwrap_or_else(|e| eprintln!("Failed to init logging: {}", e));

    log::info!("Log file: {}", log_path.display());
}

fn main() {
    setup_logging();

    let _instance_lock = match TcpListener::bind(("127.0.0.1", constants::APP_INSTANCE_LOCK_PORT)) {
        Ok(listener) => listener,
        Err(e) => {
            log::warn!(
                "Another AI Voice Console instance appears to be running (lock port {} unavailable: {}). Exiting.",
                constants::APP_INSTANCE_LOCK_PORT,
                e
            );
            return;
        }
    };

    #[cfg(target_os = "windows")]
    {
        let existing = std::env::var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS")
            .unwrap_or_default();
        let args = format!(
            "{} --use-fake-ui-for-media-stream",
            existing
        );
        std::env::set_var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", args);
    }

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::voice::start_listening,
            commands::voice::stop_listening,
            commands::settings::get_settings,
            commands::settings::save_settings,
            commands::settings::reset_settings,
            commands::devices::list_audio_devices,
            commands::pipeline::start_pipeline,
            commands::pipeline::stop_pipeline,
            commands::pipeline::get_pipeline_status,
            commands::pipeline::get_pipeline_stats,
            commands::sidecar_ctl::start_asr_sidecar,
            commands::sidecar_ctl::stop_asr_sidecar,
            commands::sidecar_ctl::sidecar_status,
            commands::asr::load_asr,
            commands::asr::transcribe_audio,
            commands::asr::get_asr_status,
            commands::asr::stop_asr,
            commands::tts::synthesize,
            commands::tts::synthesize_directed,
            commands::tts::get_tts_engines,
            commands::tts::check_tts_health,
            commands::tts::play_to_device,
            commands::tts::resample_audio,
            commands::tts::browse_clone_audio,
            commands::tts::test_mimo_connection,
            commands::llm::translate,
            commands::llm::direct_scene,
            commands::llm::test_llm_connection,
            commands::llm::generate_director_prompt,
            commands::mimo::get_mimo_settings,
            commands::mimo::update_mimo_settings,
            commands::mimo::list_mimo_models,
            commands::mimo::list_style_presets,
            commands::ptt::start_ptt,
            commands::ptt::stop_ptt,
            commands::ptt::ptt_status,
            commands::loopback::start_loopback_capture,
            commands::loopback::stop_loopback_capture,
            commands::loopback::get_loopback_audio,
            commands::loopback::loopback_status,
            commands::recording::start_recording,
            commands::recording::stop_recording_and_transcribe,
            commands::hotkey::configure_hotkey,
            commands::hotkey::get_hotkey_diag,
        ])
        .setup(|app| {
            let state = app.state::<AppState>();

            if let Err(e) = state.load_config() {
                log::warn!("Failed to load config.json: {}", e);
            }

            #[cfg(debug_assertions)]
            {
                if let Some(window) = app.get_webview_window("main") {
                    window.open_devtools();
                }
            }

            log::info!("AI Voice Console started (Tauri v2)");
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                let state = window.state::<AppState>();
                if let Err(e) = state.cleanup() {
                    log::error!("Cleanup error: {}", e);
                }
                kill_all_child_processes();
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building AI Voice Console");

    app.run(|_app_handle, event| {
        if let tauri::RunEvent::Exit = event {
            kill_all_child_processes();
        }
    });
}
