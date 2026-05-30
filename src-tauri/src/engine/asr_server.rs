use std::path::PathBuf;
use tauri::{Emitter, AppHandle};
use tauri_plugin_shell::ShellExt;
use tauri_plugin_shell::process::CommandChild;

use crate::constants::{ASR_DEFAULT_HOST, ASR_DEFAULT_PORT, ASR_HEALTH_TIMEOUT_SECS};
use crate::state::app_state::resolve_app_root;

pub struct AsrSidecar {
    child: Option<CommandChild>,
    attached_external: bool,
    port: u16,
    health_url: String,
    api_url: String,
}

impl AsrSidecar {
    pub fn new() -> Self {
        Self {
            child: None,
            attached_external: false,
            port: ASR_DEFAULT_PORT,
            health_url: format!("http://{}:{}/health", ASR_DEFAULT_HOST, ASR_DEFAULT_PORT),
            api_url: format!("http://{}:{}", ASR_DEFAULT_HOST, ASR_DEFAULT_PORT),
        }
    }

    fn find_python_exe() -> Option<PathBuf> {
        let exe_dir = std::env::current_exe().ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."));
        let app_root = resolve_app_root();

        let search_paths = vec![
            app_root.join("runtime").join("python312").join("python.exe"),
            exe_dir.join("runtime").join("python312").join("python.exe"),
            exe_dir.join("..").join("runtime").join("python312").join("python.exe"),
        ];

        for path in search_paths {
            if let Ok(canonical) = dunce::canonicalize(&path) {
                log::info!("Found python at: {}", canonical.display());
                return Some(canonical);
            }
        }

        if let Ok(output) = std::process::Command::new("where").arg("python").output() {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if let Some(line) = stdout.lines().next() {
                    let p = PathBuf::from(line.trim());
                    if p.exists() {
                        log::info!("Found python in PATH: {}", p.display());
                        return Some(p);
                    }
                }
            }
        }

        None
    }

    fn find_asr_server_py() -> Option<PathBuf> {
        let exe_dir = std::env::current_exe().ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."));
        let app_root = resolve_app_root();

        let search_paths = vec![
            app_root.join("asr_server.py"),
            exe_dir.join("_up_").join("asr_server.py"),
            exe_dir.join("asr_server.py"),
            exe_dir.join("..").join("asr_server.py"),
        ];

        for path in search_paths {
            if let Ok(canonical) = dunce::canonicalize(&path) {
                log::info!("Found asr_server.py at: {}", canonical.display());
                return Some(canonical);
            }
        }

        None
    }

    pub fn find_model_path_for_engine(engine: &str) -> Option<PathBuf> {
        let exe_dir = std::env::current_exe().ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."));
        let app_root = resolve_app_root();

        let model_dir_name = match engine {
            "sensevoice" => "SenseVoiceSmall",
            "faster_whisper" => "faster-whisper-large-v3",
            "paraformer" => "speech_paraformer-large",
            _ => "Qwen3-ASR-1.7B",
        };

        let search_paths = vec![
            app_root.join("data").join("checkpoints").join(model_dir_name),
            exe_dir.join("data").join("checkpoints").join(model_dir_name),
            exe_dir.join("..").join("data").join("checkpoints").join(model_dir_name),
            exe_dir.join("checkpoints").join(model_dir_name),
        ];

        for path in search_paths {
            if let Ok(canonical) = dunce::canonicalize(&path) {
                log::info!("Found {} model at: {}", engine, canonical.display());
                return Some(canonical);
            }
        }

        None
    }

    pub async fn start(&mut self, app: &AppHandle, engine: &str, client: &reqwest::Client) -> Result<(), String> {
        if self.is_running() {
            return Ok(());
        }

        if let Ok(resp) = client.get(&self.health_url)
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await
        {
            let status = resp.status();
            if status.is_success() {
                self.attached_external = true;
                log::info!("Reusing existing healthy ASR server on port {}", self.port);
                return Ok(());
            } else if status.as_u16() == 503 {
                self.attached_external = true;
                log::info!("Found ASR server already loading on port {}, briefly waiting for readiness", self.port);
                match self.wait_until_ready(client, 30, true).await {
                    Ok(()) => return Ok(()),
                    Err(e) => {
                        log::warn!("Existing ASR did not become ready: {}. Starting a fresh sidecar.", e);
                        self.attached_external = false;
                    }
                }
            }
        }

        self.kill_port_users();

        let python_exe = Self::find_python_exe()
            .ok_or("python.exe not found. Check runtime/python312/")?;

        let asr_server_py = Self::find_asr_server_py()
            .ok_or("asr_server.py not found. Check project root/")?;

        let model_path = Self::find_model_path_for_engine(engine)
            .ok_or_else(|| format!("{} model not found. Check data/checkpoints/", engine))?;

        let port_str = self.port.to_string();

        let args = vec![
            "-s".to_string(),
            asr_server_py.to_string_lossy().to_string(),
            model_path.to_string_lossy().to_string(),
            "--host".to_string(),
            "127.0.0.1".to_string(),
            "--port".to_string(),
            port_str,
        ];

        log::info!("Starting ASR: {} {}", python_exe.display(), args.join(" "));

        let (mut rx, child) = app
            .shell()
            .command(python_exe.to_string_lossy().as_ref())
            .args(&args)
            .spawn()
            .map_err(|e| format!("Failed to spawn asr_server.py: {}", e))?;

        self.child = Some(child);
        self.attached_external = false;

        let app_handle = app.clone();
        tauri::async_runtime::spawn(async move {
            use tauri_plugin_shell::process::CommandEvent;
            while let Some(event) = rx.recv().await {
                match event {
                    CommandEvent::Stdout(line_bytes) => {
                        let line = String::from_utf8_lossy(&line_bytes);
                        let _ = app_handle.emit("sidecar_log", line.to_string());
                        log::info!("[ASR] {}", line);
                    }
                    CommandEvent::Stderr(line_bytes) => {
                        let line = String::from_utf8_lossy(&line_bytes);
                        let _ = app_handle.emit("sidecar_log", format!("[ERR] {}", line));
                        log::warn!("[ASR] {}", line);
                    }
                    CommandEvent::Terminated(status) => {
                        let msg = format!("ASR server exited with status: {:?}", status);
                        let _ = app_handle.emit("sidecar_log", msg.clone());
                        log::warn!("{}", msg);
                        let _ = app_handle.emit("asr_process_died", ());
                        break;
                    }
                    _ => {}
                }
            }
        });

        self.wait_until_ready(client, ASR_HEALTH_TIMEOUT_SECS, false).await
    }

    async fn wait_until_ready(&mut self, client: &reqwest::Client, timeout_secs: u64, external: bool) -> Result<(), String> {
        let mut refused_count = 0u32;
        for i in 0..timeout_secs {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            if let Ok(resp) = client.get(&self.health_url)
                .timeout(std::time::Duration::from_secs(2))
                .send()
                .await
            {
                refused_count = 0;
                let status = resp.status();
                if status.is_success() {
                    log::info!("ASR server is ready on port {} (waited {}s)", self.port, i + 1);
                    return Ok(());
                } else if i % 10 == 0 {
                    log::info!("ASR health check: {} (model still loading, waited {}s)", status, i + 1);
                }
            } else {
                refused_count += 1;
                if i % 10 == 0 {
                    log::info!("ASR health check: connection refused (waited {}s)", i + 1);
                }
                if external && refused_count >= 3 {
                    self.attached_external = false;
                    return Err("external ASR disappeared while loading".to_string());
                }
            }
        }

        if let Some(child) = self.child.take() {
            let _ = child.kill();
            log::warn!("Killed ASR sidecar after readiness timeout");
        }
        self.attached_external = false;
        Err(format!("ASR server health check timeout ({}s). Model loading may be slow on first run.", timeout_secs).to_string())
    }

    pub fn stop(&mut self) -> Result<(), String> {
        if let Some(child) = self.child.take() {
            child.kill().map_err(|e| format!("Failed to kill ASR server: {}", e))?;
            log::info!("ASR server stopped");
        } else if self.attached_external {
            log::info!("Detached from externally running ASR server");
        }
        self.attached_external = false;

        self.kill_orphaned_processes();

        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn kill_port_users(&self) {
        let port = self.port;
        log::info!("Checking for processes using port {}...", port);
        let output = std::process::Command::new("netstat")
            .args(["-ano", "-p", "TCP"])
            .output();

        if let Ok(out) = output {
            let text = String::from_utf8_lossy(&out.stdout);
            for line in text.lines() {
                if line.contains(&format!(":{}", port)) && line.contains("LISTENING") {
                    if let Some(pid_str) = line.split_whitespace().last() {
                        if let Ok(pid) = pid_str.parse::<u32>() {
                            if pid != std::process::id() {
                                log::warn!("Killing stale process {} on port {}", pid, port);
                                let _ = std::process::Command::new("taskkill")
                                    .args(["/F", "/PID", &pid.to_string()])
                                    .output();
                                std::thread::sleep(std::time::Duration::from_millis(500));
                            }
                        }
                    }
                }
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn kill_port_users(&self) {}

    #[cfg(target_os = "windows")]
    fn kill_orphaned_processes(&self) {
        let port = self.port;
        std::thread::spawn(move || {
            let output = std::process::Command::new("netstat")
                .args(["-ano", "-p", "TCP"])
                .output();
            if let Ok(out) = output {
                let text = String::from_utf8_lossy(&out.stdout);
                for line in text.lines() {
                    if line.contains(&format!("127.0.0.1:{}", port)) && line.contains("LISTENING") {
                        if let Some(pid_str) = line.split_whitespace().last() {
                            if let Ok(pid) = pid_str.parse::<u32>() {
                                log::info!("Killing orphaned ASR process tree (PID {})", pid);
                                let _ = std::process::Command::new("taskkill")
                                    .args(["/F", "/T", "/PID", &pid.to_string()])
                                    .output();
                            }
                        }
                    }
                }
            }
        });
    }

    #[cfg(not(target_os = "windows"))]
    fn kill_orphaned_processes(&self) {}

    pub fn is_running(&self) -> bool {
        self.child.is_some() || self.attached_external
    }

    pub fn mark_dead(&mut self) {
        if self.child.is_some() || self.attached_external {
            self.child = None;
            self.attached_external = false;
            log::warn!("ASR sidecar marked as dead (process exited)");
        }
    }

    pub fn api_url(&self) -> &str {
        &self.api_url
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn set_port(&mut self, port: u16) {
        self.port = port;
        self.api_url = format!("http://127.0.0.1:{}", port);
        self.health_url = format!("http://127.0.0.1:{}/health", port);
    }
}
