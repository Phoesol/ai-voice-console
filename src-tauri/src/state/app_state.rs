// AppState — 全局共享状态
// 管理 ASR Sidecar 进程、TTS 客户端连接、管道状态等

use std::sync::Arc;
use tokio::sync::RwLock;
use std::path::PathBuf;

use crate::state::settings::Settings;
use crate::engine::asr_server::AsrSidecar;
use crate::audio::loopback::LoopbackCapture;

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineStats {
    pub asr_count: u64,
    pub translate_count: u64,
    pub total_asr_ms: u64,
    pub total_translate_ms: u64,
    pub total_tts_ms: u64,
    pub tts_count: u64,
}

// ============================================================
// JSON Key 命名转换: camelCase ↔ snake_case
// 确保 Tauri IPC 用 camelCase, config.json 用 snake_case (Python兼容)
// ============================================================

/// 将 JSON Value 中的所有 key 从 snake_case 转换为 camelCase
/// 用于读取 config.json 时转换, 使 serde (rename_all = "camelCase") 能正确反序列化
fn convert_keys_snake_to_camel(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            // 收集旧键值对, 用新键重建 map
            let old: Vec<(String, serde_json::Value)> = map
                .iter_mut()
                .map(|(k, v)| {
                    convert_keys_snake_to_camel(v);
                    (snake_to_camel(k), std::mem::take(v))
                })
                .collect();
            map.clear();
            for (k, v) in old {
                map.insert(k, v);
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr.iter_mut() {
                convert_keys_snake_to_camel(v);
            }
        }
        _ => {}
    }
}

/// 将 JSON Value 中的所有 key 从 camelCase 转换为 snake_case
/// 用于保存 config.json 时转换, 保持与 Python 端的兼容性
fn convert_keys_camel_to_snake(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            let old: Vec<(String, serde_json::Value)> = map
                .iter_mut()
                .map(|(k, v)| {
                    convert_keys_camel_to_snake(v);
                    (camel_to_snake(k), std::mem::take(v))
                })
                .collect();
            map.clear();
            for (k, v) in old {
                map.insert(k, v);
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr.iter_mut() {
                convert_keys_camel_to_snake(v);
            }
        }
        _ => {}
    }
}

/// snake_case → camelCase (例: tts_engine → ttsEngine)
fn snake_to_camel(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize = false;
    for c in s.chars() {
        if c == '_' {
            capitalize = true;
        } else if capitalize {
            result.push(c.to_ascii_uppercase());
            capitalize = false;
        } else {
            result.push(c);
        }
    }
    result
}

/// camelCase → snake_case (例: ttsEngine → tts_engine)
fn camel_to_snake(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                result.push('_');
            }
            result.push(c.to_ascii_lowercase());
        } else {
            result.push(c);
        }
    }
    result
}

/// 应用全局状态, 通过 tauri::State 在各 command 间共享
pub struct AppState {
    /// 运行时配置 (读写锁, 允许并发读)
    pub settings: Arc<RwLock<Settings>>,

    /// ASR Sidecar 管理器
    pub asr_sidecar: Arc<RwLock<AsrSidecar>>,

    /// HTTP 客户端 (复用连接池)
    pub http_client: reqwest::Client,

    /// 配置文件路径
    pub config_path: PathBuf,

    /// ASR 是否已加载
    pub asr_loaded: Arc<RwLock<bool>>,

    /// ASR 当前引擎名称
    pub asr_engine: Arc<RwLock<String>>,

    /// 管道是否运行中
    pub pipeline_running: Arc<RwLock<bool>>,

    /// PTT 是否激活
    pub ptt_active: Arc<RwLock<bool>>,

    /// TTS 是否工作中
    pub tts_busy: Arc<RwLock<bool>>,

    /// WASAPI Loopback 捕获器
    pub loopback_capture: Arc<RwLock<LoopbackCapture>>,

    /// 管道统计
    pub stats: Arc<RwLock<PipelineStats>>,
}

impl AppState {
    pub fn new() -> Self {
        let config_path = find_config_path();
        let settings = Settings::default();
        let default_asr_engine = settings.asr_engine.clone();
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .connect_timeout(std::time::Duration::from_secs(5))
            .pool_max_idle_per_host(8)
            .tcp_keepalive(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            settings: Arc::new(RwLock::new(settings)),
            asr_sidecar: Arc::new(RwLock::new(AsrSidecar::new())),
            http_client,
            config_path,
            asr_loaded: Arc::new(RwLock::new(false)),
            asr_engine: Arc::new(RwLock::new(default_asr_engine)),
            pipeline_running: Arc::new(RwLock::new(false)),
            ptt_active: Arc::new(RwLock::new(false)),
            tts_busy: Arc::new(RwLock::new(false)),
            loopback_capture: Arc::new(RwLock::new(LoopbackCapture::new())),
            stats: Arc::new(RwLock::new(PipelineStats::default())),
        }
    }

    /// 从 config.json 加载配置
    /// config.json 使用 snake_case (Python兼容), 转换为 camelCase 后反序列化
    pub fn load_config(&self) -> Result<(), String> {
        let path = &self.config_path;
        if !path.exists() {
            log::info!("config.json not found, using defaults");
            return Ok(());
        }
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read config.json: {}", e))?;
        
        // 将 snake_case JSON keys 转换为 camelCase, 以匹配 Rust 的 serde(rename_all = "camelCase")
        let mut json_value: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse config.json as JSON: {}", e))?;
        convert_keys_snake_to_camel(&mut json_value);
        
        let loaded: Settings = serde_json::from_value(json_value)
            .map_err(|e| format!("Failed to deserialize config.json: {}", e))?;
        let mut settings = self.settings.blocking_write();
        *settings = loaded;
        log::info!("Config loaded from {}", path.display());
        Ok(())
    }

    /// 保存配置到 config.json
    /// 序列化为 camelCase 后转换为 snake_case, 保持 Python 端兼容性
    pub fn save_config(&self) -> Result<(), String> {
        let settings = self.settings.blocking_read();
        let mut json_value = serde_json::to_value(&*settings)
            .map_err(|e| format!("Failed to serialize settings: {}", e))?;
        // 将 camelCase JSON keys 转换为 snake_case, 保持与 Python 端的兼容性
        convert_keys_camel_to_snake(&mut json_value);
        let content = serde_json::to_string_pretty(&json_value)
            .map_err(|e| format!("Failed to format config.json: {}", e))?;
        std::fs::write(&self.config_path, content)
            .map_err(|e| format!("Failed to write config.json: {}", e))?;
        log::info!("Config saved to {}", self.config_path.display());
        Ok(())
    }

    /// 清理: 停止 Sidecar, 保存配置
    pub fn cleanup(&self) -> Result<(), String> {
        // 停止 ASR Sidecar
        let mut sidecar = self.asr_sidecar.blocking_write();
        sidecar.stop()?;

        // 保存配置
        self.save_config()?;

        log::info!("Cleanup complete");
        Ok(())
    }
}

/// 判断目录是否像应用根目录。
/// 依赖稳定的文件/目录特征，不依赖项目文件夹名称。
pub fn looks_like_app_root(dir: &std::path::Path) -> bool {
    dir.join(".project-root").exists()
        || (dir.join("config.json").exists()
            && (dir.join("asr_server.py").exists() || dir.join("data").exists()))
        || (dir.join("src-tauri").exists() && dir.join("src").exists())
}

/// 从当前工作目录和 exe 目录向上寻找应用根目录。
/// 不硬编码目录名，支持项目文件夹重命名、从快捷方式启动、开发构建目录启动。
pub fn resolve_app_root() -> PathBuf {
    let exe_path = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("ai-voice-console.exe"));
    let exe_dir = exe_path.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("."));
    let mut candidates = Vec::new();

    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd);
    }
    candidates.push(exe_dir.clone());

    for base in candidates {
        let mut current = Some(base.as_path());
        for _ in 0..8 {
            let Some(dir) = current else { break };
            if looks_like_app_root(dir) {
                let root = dunce::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
                log::info!("Resolved app root: {}", root.display());
                return root;
            }
            current = dir.parent();
        }
    }

    log::info!("App root not found by markers, falling back to exe dir: {}", exe_dir.display());
    exe_dir
}

/// 查找 config.json 路径。
/// 优先使用解析出的应用根目录，兜底才落到当前工作目录。
fn find_config_path() -> PathBuf {
    let app_root = resolve_app_root();
    let root_config = app_root.join("config.json");
    if root_config.exists() || looks_like_app_root(&app_root) {
        log::info!("Using config path: {}", root_config.display());
        return root_config;
    }

    if let Ok(cwd) = std::env::current_dir() {
        log::info!("Config not found, will create at CWD: {}", cwd.join("config.json").display());
        cwd.join("config.json")
    } else {
        root_config
    }
}
