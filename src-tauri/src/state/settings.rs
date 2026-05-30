// Settings — 配置结构体
// 与 Python 版 config.json 保持向后兼容
// 注意: 所有 serde 序列化使用 camelCase, 与前端 JS 保持一致
// 配置文件读写时自动转换 snake_case ↔ camelCase (见 app_state.rs)

use serde::{Deserialize, Serialize};

/// 宏：将 Option<T> 字段应用到 Settings（数值/ Copy 类型）
/// 用法：apply_opt!(self, settings, field_name, T);
/// 生成：if let Some(v) = self.field_name { settings.field_name = v; }
macro_rules! apply_opt {
    ($self_:ident, $dst:ident, $field:ident, $($_ty:ty)*) => {
        if let Some(v) = $self_.$field {
            $dst.$field = v;
        }
    };
}

/// 宏：将 Option<bool> 字段应用到 Settings（特殊处理 bool）
macro_rules! apply_opt_bool {
    ($self_:ident, $dst:ident, $field:ident) => {
        if let Some(v) = $self_.$field {
            $dst.$field = v;
        }
    };
}

/// 宏：将 Option<String> 字段应用到 Settings（直接覆盖，含 CLEAR_MARKER 支持）
/// 对于含 CLEAR_MARKER 逻辑的字段，改用 apply_opt_option_str!
macro_rules! apply_opt_str {
    ($self_:ident, $dst:ident, $field:ident) => {
        if let Some(ref v) = $self_.$field {
            $dst.$field = v.clone();
        }
    };
}

/// 宏：将 Option<String> 字段应用到 Settings（CLEAR_MARKER 支持 → 设为 None）
macro_rules! apply_opt_option_str {
    ($self_:ident, $dst:ident, $field:ident) => {
        if let Some(ref v) = $self_.$field {
            if v == $crate::state::settings::CLEAR_MARKER {
                $dst.$field = None;
            } else {
                $dst.$field = Some(v.clone());
            }
        }
    };
}

/// 宏：将 Option<i64> 字段应用到 Settings（负值 → 设为 None）
macro_rules! apply_opt_option_i64 {
    ($self_:ident, $dst:ident, $field:ident) => {
        if let Some(v) = $self_.$field {
            $dst.$field = if v < 0 { None } else { Some(v) };
        }
    };
}

/// 清空标记：当 Option<T> 字段收到此特殊字符串时，表示应设为 None
/// 用于前端发送 `{ "voiceReferenceId": "__clear__" }` 来清空字段
const CLEAR_MARKER: &str = "__clear__";

/// 全局配置 (与 config.json 一一对应, 可反序列化旧配置)
/// 使用 camelCase 与前端 JS 交互
/// 读写 config.json 时自动进行 snake_case ↔ camelCase 转换
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    // --- 语言/主题 ---
    pub language: String,
    pub theme: String,

    // --- 音量/速度 ---
    pub volume: f64,
    pub playback_speed: f64,
    pub bypass_mode: bool,
    pub auto_save_audio: bool,

    // --- PTT ---
    pub ptt_enabled: bool,
    pub ptt_key1: String,
    pub hotkey1_modifier: String,
    pub ptt_key2: String,

    // --- ASR ---
    pub asr_engine: String,
    pub emotion_model: String,
    pub emotion_intensity: f64,

    // --- TTS ---
    pub tts_model: String,
    pub tts_engine: String,
    pub tts_prefix_tag: String,
    pub tts_suffix_tag: String,
    pub tts_streaming: bool,
    pub tts_chunk_length: u32,
    pub tts_top_p: f64,
    pub tts_temperature: f64,
    pub tts_repetition_penalty: f64,

    // --- 语音模式 ---
    pub voice_mode: String,
    pub voice_reference_id: Option<String>,
    pub voice_name: Option<String>,

    // --- 音频设备 ---
    pub host_api: String,
    pub mic_device_id: Option<i64>,
    pub output_device_id: Option<i64>,
    pub monitor_device_id: Option<i64>,

    // --- WASAPI Loopback ---
    pub wasapi_loopback: bool,
    pub speaker_device_id: Option<i64>,

    // --- VAD ---
    pub vad_filter: bool,
    pub vad_min_silence: u32,
    pub vad_speech_pad: u32,

    // --- MiMo TTS ---
    pub mimo_style_prompt: String,
    pub mimo_voice_design: String,
    pub mimo_clone_audio_path: String,
    pub mimo_model: String,
    pub mimo_api_key: String,
    pub mimo_api_base: String,
    pub mimo_director_enabled: bool,
    pub mimo_optimize_text: bool,

    // --- 文本导演 ---
    pub text_model_director_enabled: bool,
    pub text_model_director_scenarios: Vec<ScenarioConfig>,
    #[serde(default)]
    pub tmd_standby_scenarios: Vec<ScenarioConfig>,

    // --- MiMo 角色 ---
    pub mimo_character: String,
    pub mimo_scene: String,
    pub mimo_direction: String,

    // --- 反差尾音 ---
    pub sexy_afterglow: bool,

    // --- 翻译 ---
    pub translate_enabled: bool,
    pub translate_target_lang: String,

    // --- DeepSeek LLM ---
    pub deepseek_api_key: String,
    pub deepseek_api_base: String,
    pub deepseek_model: String,

    // --- 系统管道 ---
    #[serde(default = "default_tts_api_url")]
    pub tts_api_url: String,

    // --- 管道VAD设置 ---
    #[serde(default)]
    pub vad_threshold: f64,
    #[serde(default = "default_min_speech_duration")]
    pub min_speech_duration: f64,
    #[serde(default = "default_max_speech_duration")]
    pub max_speech_duration: f64,

    // === 导演模式新增模块 ===

    // --- 模块1: TTS 识别标准 ---
    #[serde(default)]
    pub tts_standards: Vec<TtsStandard>,
    #[serde(default)]
    pub tts_standby_standards: Vec<TtsStandard>,

    // --- 模块2: LLM 风格指导 ---
    #[serde(default)]
    pub llm_style_guides: Vec<LlmStyleGuide>,
    #[serde(default)]
    pub llm_standby_style_guides: Vec<LlmStyleGuide>,

    // --- 模块3: 可替换的 LLM 系统提示词 (direct_scene 使用) ---
    #[serde(default)]
    pub director_system_prompt: String,

    // --- 模块3: 提示词生成辅助 ---
    #[serde(default)]
    pub merged_context: String,
    #[serde(default)]
    pub generated_prompt: String,
}

fn default_tts_api_url() -> String {
    "http://127.0.0.1:18084".to_string()
}

fn default_min_speech_duration() -> f64 { 1.0 }
fn default_max_speech_duration() -> f64 { 15.0 }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioConfig {
    pub name: String,
    pub trigger: String,
    pub prompt: String,
    #[serde(default)]
    pub character: String,
    #[serde(default)]
    pub scene: String,
    #[serde(default)]
    pub direction: String,
}

/// 模块1: TTS 识别标准
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TtsStandard {
    pub name: String,
    #[serde(default)]
    pub voice_design_prompt: String,
    #[serde(default)]
    pub audio_tag_control: String,
    #[serde(default)]
    pub style_control: String,
}

/// 模块2: LLM 风格识别指导
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LlmStyleGuide {
    pub name: String,
    #[serde(default)]
    pub content: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            // --- 语言/主题 ---
            language: "zh".to_string(),
            theme: "dark".to_string(),

            // --- 音量/速度 ---
            volume: 0.88,
            playback_speed: 1.0,
            bypass_mode: false,
            auto_save_audio: true,

            // --- PTT ---
            ptt_enabled: true,
            ptt_key1: String::new(),
            hotkey1_modifier: "none".to_string(),
            ptt_key2: "none".to_string(),

            // --- ASR ---
            asr_engine: "qwen3_asr".to_string(),
            emotion_model: "none".to_string(),
            emotion_intensity: 0.9,

            // --- TTS ---
            tts_model: "openaudio-s1-mini".to_string(),
            tts_engine: "mimo_tts".to_string(),
            tts_prefix_tag: String::new(),
            tts_suffix_tag: String::new(),
            tts_streaming: false,            // 用户实际偏好
            tts_chunk_length: 200,
            tts_top_p: 0.95,
            tts_temperature: 0.7,
            tts_repetition_penalty: 1.2,

            // --- 语音模式 ---
            voice_mode: "asr_tts".to_string(),
            voice_reference_id: None,
            voice_name: None,

            // --- 音频设备 ---
            host_api: "Windows WASAPI".to_string(),
            mic_device_id: None,
            output_device_id: None,
            monitor_device_id: None,

            // --- WASAPI Loopback ---
            wasapi_loopback: false,
            speaker_device_id: None,

            // --- VAD ---
            vad_filter: false,
            vad_min_silence: 600,           // 用户实际偏好
            vad_speech_pad: 400,

            // --- MiMo TTS ---
            mimo_style_prompt: "角色：正在进行高强度 FPS 电竞的19岁、女大学生玩家。会中英韩日4国语言。\n\n场景：FPS 游戏决赛圈，战况十万火急。正在进行极度紧张、快节奏的与队友沟通，必须像机关枪一样飞速说话。\n\n指导：绝对语速控制（最高优先级）：极限语速（Fastest rate）。像连珠炮一样输出，字与字之间必须无缝粘连，彻底消除所有标点符号带来的物理停顿，绝不允许有任何空白间隙。\n音色特征维持：在极高语速下，全程保持萝莉音与夹子音的特征。喘息必须是\u{201C}战况激烈导致的急促呼吸\u{201D}，颤音必须是\u{201C}高频快速的颤抖\u{201D}，绝对不能因为喘息或颤音而拖慢说话的节奏。\n极速收音：严禁任何形式的拖音、拉长音。遇到所有结尾语气词时，必须瞬间斩断收音，保持高密度的直线爆发输出。".to_string(),
            mimo_voice_design: "19岁年轻女性，嗓音甜美略带气声，有着自然的萝莉音和夹子音质感。声线极具张力，能驾驭机关枪般的极速爆发，也能无缝切换到慵懒妩媚的轻声细语。".to_string(),
            mimo_clone_audio_path: String::new(),
            mimo_model: "mimo-v2.5-tts-voicedesign".to_string(),   // 用户实际使用的模型
            mimo_api_key: String::new(),
            mimo_api_base: "https://api.xiaomimimo.com/v1".to_string(),
            mimo_director_enabled: false,
            mimo_optimize_text: false,

            // --- 文本导演 ---
            text_model_director_enabled: true,
            text_model_director_scenarios: vec![
                ScenarioConfig {
                    name: "游戏报点".to_string(),
                    trigger: "报点、喊救命、报物资、战术沟通等游戏相关信息".to_string(),
                    prompt: "19岁年轻女性，嗓音甜美略带气声，有着自然的萝莉音和夹子音质感。声线极具张力，能驾驭机关枪般的极速爆发，也能无缝切换到慵懒妩媚的轻声细语。".to_string(),
                    character: "你是一位精通中英韩日4国语言的19岁女大学生玩家。在进行高强度 FPS 电竞时如同机关枪一般暴躁干练；在闲聊时则卸下防备，提供极致情绪价值，充满反差萌。".to_string(),
                    scene: "**游戏场景**：FPS 游戏决赛圈，战况十万火急（触发词：有人、脸上、大残、报点、缩圈、架枪、以及各类武器名等），提供专业报点。".to_string(),
                    direction: "**【游戏场景指导原则】**\n* **标签前缀**：必须在句首加上 `(萝莉音, 语速极快, 紧张)` 或 `(夹子音, 语速极快, 凌厉)`。\n* **发音控制**：极限语速像连珠炮输出，字与字无缝粘连。\n* **严格禁忌**：严禁在句中或句尾插入任何会拖慢语速的标签（如喘息、叹气、沉默）。如果要有喘息，只能放在句首且必须是 `[急促喘息]`（战况激烈导致）。\n* **极速收音**：严禁拖音拉长音。遇到结尾语气词瞬间斩断，只能使用感叹号 `！` 结尾，保持高密度直线爆发输出。".to_string(),
                },
                ScenarioConfig {
                    name: "闲聊".to_string(),
                    trigger: "日常闲聊、情感交流、撒娇、开玩笑等非游戏信息".to_string(),
                    prompt: "19岁年轻女性，嗓音甜美略带气声，有着自然的萝莉音和夹子音质感。声线极具张力，能驾驭机关枪般的极速爆发，也能无缝切换到慵懒妩媚的轻声细语。".to_string(),
                    character: "你是一位精通中英韩日4国语言的19岁女大学生玩家。在进行高强度 FPS 电竞时如同机关枪一般暴躁干练；在闲聊时则卸下防备，提供极致情绪价值，充满反差萌。".to_string(),
                    scene: "**闲聊场景**：非激烈交战时的日常交流、情感表达、陪伴闲聊。提供情绪价值。".to_string(),
                    direction: "**【闲聊场景指导原则】**\n* **标签前缀**：必须在句首加上 `(慵懒, 妩媚)` 或 `(气声, 诱惑, 甜美)`。\n* **发音控制**：语速平缓慵懒，声音性感妩媚甜美，提供极致的情绪价值。\n* **尾音拉长**：句尾必须大量使用波浪号 `~` 或省略号 `……` 来明显拖拽尾音。\n* **丰富修饰**：必须在句中和句尾大量穿插 `[喘息]`、`[娇喘]`、`[呻吟]`、`[轻笑]` 等标签，让轻柔颤音和急促喘息交织，营造反差萌。".to_string(),
                },
            ],
            tmd_standby_scenarios: vec![],

            // --- MiMo 角色 ---
            mimo_character: "正在进行高强度 FPS 电竞的19岁、女大学生玩家。会中英韩日4国语言。".to_string(),
            mimo_scene: "FPS 游戏决赛圈，战况十万火急。正在进行极度紧张、快节奏的与队友沟通，必须像机关枪一样飞速说话。".to_string(),
            mimo_direction: "绝对语速控制（最高优先级）：极限语速（Fastest rate）。像连珠炮一样输出，字与字之间必须无缝粘连，彻底消除所有标点符号带来的物理停顿，绝不允许有任何空白间隙。\n音色特征维持：在极高语速下，全程保持萝莉音与夹子音的特征。喘息必须是\u{201C}战况激烈导致的急促呼吸\u{201D}，颤音必须是\u{201C}高频快速的颤抖\u{201D}，绝对不能因为喘息或颤音而拖慢说话的节奏。\n极速收音：严禁任何形式的拖音、拉长音。遇到所有结尾语气词时，必须瞬间斩断收音，保持高密度的直线爆发输出。".to_string(),

            // --- 反差尾音 ---
            sexy_afterglow: true,            // 用户实际偏好

            // --- 翻译 ---
            translate_enabled: true,          // 用户实际偏好
            translate_target_lang: "Korean".to_string(),

            // --- DeepSeek LLM ---
            deepseek_api_key: String::new(),
            deepseek_api_base: "https://api.deepseek.com/v1".to_string(),
            deepseek_model: "deepseek-v4-flash".to_string(),

            // --- 系统管道 ---
            tts_api_url: "http://127.0.0.1:18084".to_string(),

            // --- 管道VAD设置 ---
            vad_threshold: 0.5,
            min_speech_duration: 1.0,
            max_speech_duration: 15.0,

            // === 导演模式新增模块 ===
            tts_standards: vec![],
            tts_standby_standards: vec![],
            llm_style_guides: vec![],
            llm_standby_style_guides: vec![],
            director_system_prompt: String::new(),
            merged_context: String::new(),
            generated_prompt: String::new(),
        }
    }
}

/// 部分更新 (前端可能只发部分字段)
/// 所有字段使用 camelCase, 与前端 JS 一致
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SettingsUpdate {
    pub volume: Option<f64>,
    pub playback_speed: Option<f64>,
    pub tts_engine: Option<String>,
    pub tts_model: Option<String>,
    pub tts_streaming: Option<bool>,
    pub tts_chunk_length: Option<u32>,
    pub tts_top_p: Option<f64>,
    pub tts_temperature: Option<f64>,
    pub tts_repetition_penalty: Option<f64>,
    pub tts_prefix_tag: Option<String>,
    pub tts_suffix_tag: Option<String>,
    pub emotion_intensity: Option<f64>,
    pub sexy_afterglow: Option<bool>,
    pub voice_reference_id: Option<String>,
    pub voice_name: Option<String>,
    pub voice_mode: Option<String>,
    pub bypass_mode: Option<bool>,
    pub auto_save_audio: Option<bool>,
    pub mic_device_id: Option<i64>,
    pub output_device_id: Option<i64>,
    pub monitor_device_id: Option<i64>,
    pub ptt_key1: Option<String>,
    pub hotkey1_modifier: Option<String>,
    pub ptt_key2: Option<String>,
    pub emotion_model: Option<String>,
    pub vad_filter: Option<bool>,
    pub vad_min_silence: Option<u32>,
    pub vad_speech_pad: Option<u32>,
    pub mimo_voice: Option<String>,
    pub mimo_model: Option<String>,
    pub mimo_api_key: Option<String>,
    pub mimo_api_base: Option<String>,
    pub mimo_style_prompt: Option<String>,
    pub mimo_voice_design: Option<String>,
    pub mimo_clone_audio_path: Option<String>,
    pub mimo_optimize_text: Option<bool>,
    pub translate_enabled: Option<bool>,
    pub translate_target_lang: Option<String>,
    pub deepseek_api_key: Option<String>,
    pub deepseek_api_base: Option<String>,
    pub deepseek_model: Option<String>,
    pub language: Option<String>,
    pub theme: Option<String>,
    pub host_api: Option<String>,
    // --- 新增字段 ---
    pub ptt_enabled: Option<bool>,
    pub asr_engine: Option<String>,
    pub mimo_director_enabled: Option<bool>,
    pub text_model_director_enabled: Option<bool>,
    pub text_model_director_scenarios: Option<Vec<ScenarioConfig>>,
    pub tmd_standby_scenarios: Option<Vec<ScenarioConfig>>,
    pub mimo_character: Option<String>,
    pub mimo_scene: Option<String>,
    pub mimo_direction: Option<String>,
    pub vad_threshold: Option<f64>,
    pub tts_api_url: Option<String>,
    pub wasapi_loopback: Option<bool>,
    pub speaker_device_id: Option<i64>,

    // === 导演模式新增 ===
    pub tts_standards: Option<Vec<TtsStandard>>,
    pub tts_standby_standards: Option<Vec<TtsStandard>>,
    pub llm_style_guides: Option<Vec<LlmStyleGuide>>,
    pub llm_standby_style_guides: Option<Vec<LlmStyleGuide>>,
    pub director_system_prompt: Option<String>,
    pub merged_context: Option<String>,
    pub generated_prompt: Option<String>,
}

impl SettingsUpdate {
    /// 将部分更新应用到完整 Settings
    /// 使用 apply_opt! 宏消除重复的 if-let-Some 分支
    pub fn apply_to(&self, settings: &mut Settings) {
        // --- 基础数值/布尔字段 ---
        apply_opt!(self, settings, volume, f64);
        apply_opt!(self, settings, playback_speed, f64);
        apply_opt!(self, settings, tts_chunk_length, u32);
        apply_opt!(self, settings, tts_top_p, f64);
        apply_opt!(self, settings, tts_temperature, f64);
        apply_opt!(self, settings, tts_repetition_penalty, f64);
        apply_opt!(self, settings, emotion_intensity, f64);
        apply_opt!(self, settings, vad_min_silence, u32);
        apply_opt!(self, settings, vad_speech_pad, u32);
        apply_opt!(self, settings, vad_threshold, f64);

        // --- 布尔字段 ---
        apply_opt_bool!(self, settings, tts_streaming);
        apply_opt_bool!(self, settings, sexy_afterglow);
        apply_opt_bool!(self, settings, bypass_mode);
        apply_opt_bool!(self, settings, auto_save_audio);
        apply_opt_bool!(self, settings, ptt_enabled);
        apply_opt_bool!(self, settings, vad_filter);
        apply_opt_bool!(self, settings, mimo_director_enabled);
        apply_opt_bool!(self, settings, text_model_director_enabled);
        apply_opt_bool!(self, settings, mimo_optimize_text);
        apply_opt_bool!(self, settings, translate_enabled);

        // --- String 字段 (直接覆盖) ---
        apply_opt_str!(self, settings, tts_engine);
        apply_opt_str!(self, settings, tts_model);
        apply_opt_str!(self, settings, voice_mode);
        apply_opt_str!(self, settings, emotion_model);
        apply_opt_str!(self, settings, language);
        apply_opt_str!(self, settings, theme);
        apply_opt_str!(self, settings, host_api);
        apply_opt_str!(self, settings, ptt_key1);
        apply_opt_str!(self, settings, hotkey1_modifier);
        apply_opt_str!(self, settings, ptt_key2);
        apply_opt_str!(self, settings, asr_engine);
        apply_opt_str!(self, settings, translate_target_lang);
        apply_opt_str!(self, settings, deepseek_api_base);
        apply_opt_str!(self, settings, deepseek_model);
        apply_opt_str!(self, settings, mimo_model);
        apply_opt_str!(self, settings, mimo_api_base);
        apply_opt_str!(self, settings, mimo_style_prompt);
        apply_opt_str!(self, settings, mimo_voice_design);
        apply_opt_str!(self, settings, mimo_clone_audio_path);
        apply_opt_str!(self, settings, mimo_character);
        apply_opt_str!(self, settings, mimo_scene);
        apply_opt_str!(self, settings, mimo_direction);
        apply_opt_str!(self, settings, tts_prefix_tag);
        apply_opt_str!(self, settings, tts_suffix_tag);
        apply_opt_str!(self, settings, tts_api_url);

        // --- ApiKey 特殊处理：空字符串=清空 ---
        if let Some(ref v) = self.mimo_api_key {
            settings.mimo_api_key = v.clone();
        }
        if let Some(ref v) = self.deepseek_api_key {
            settings.deepseek_api_key = v.clone();
        }

        // --- Option<String> 字段：CLEAR_MARKER 支持 ---
        apply_opt_option_str!(self, settings, voice_reference_id);
        apply_opt_option_str!(self, settings, voice_name);

        // --- Option<i64> 字段：负值=清空 ---
        apply_opt_option_i64!(self, settings, mic_device_id);
        apply_opt_option_i64!(self, settings, output_device_id);
        apply_opt_option_i64!(self, settings, monitor_device_id);
        apply_opt_option_i64!(self, settings, speaker_device_id);

        // --- WASAPI Loopback ---
        apply_opt_bool!(self, settings, wasapi_loopback);

        // --- Vec<ScenarioConfig> 字段 ---
        if let Some(ref v) = self.text_model_director_scenarios {
            settings.text_model_director_scenarios = v.clone();
        }
        if let Some(ref v) = self.tmd_standby_scenarios {
            settings.tmd_standby_scenarios = v.clone();
        }

        // === 导演模式新增 Vec + String 字段 ===
        if let Some(ref v) = self.tts_standards {
            settings.tts_standards = v.clone();
        }
        if let Some(ref v) = self.tts_standby_standards {
            settings.tts_standby_standards = v.clone();
        }
        if let Some(ref v) = self.llm_style_guides {
            settings.llm_style_guides = v.clone();
        }
        if let Some(ref v) = self.llm_standby_style_guides {
            settings.llm_standby_style_guides = v.clone();
        }
        apply_opt_str!(self, settings, director_system_prompt);
        apply_opt_str!(self, settings, merged_context);
        apply_opt_str!(self, settings, generated_prompt);
    }
}