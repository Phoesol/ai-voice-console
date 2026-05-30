// LLM HTTP 客户端 — DeepSeek / OpenAI 兼容 API
// 功能1: 翻译 (translate_text)
// 功能2: 文本导演 (direct_scene) — 判断场景 + 生成带MiMo标签的合成文本
//
// 架构:
// - Rust端构造 userContent (MiMo导演模式: 角色+场景+指导)
// - Flash只判断场景 + 给assistantContent添加MiMo音频标签
// - 兜底校验: sanitize_director_result() 防止乱码

use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ToolCall {
    id: String,
    r#type: String,
    function: ToolCallFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ToolCallFunction {
    name: String,
    arguments: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ToolDef>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ToolDef {
    r#type: String,
    function: ToolFunctionDef,
}

#[derive(Debug, Clone, Serialize)]
struct ToolFunctionDef {
    name: String,
    strict: bool,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Clone, Deserialize)]
struct Choice {
    message: ChoiceMessage,
}

#[derive(Debug, Clone, Deserialize)]
struct ChoiceMessage {
    content: Option<String>,
    reasoning_content: Option<String>,
    tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectSceneResult {
    pub scene_name: String,
    pub user_content: String,
    pub assistant_content: String,
    pub optimize_text: bool,
}

pub struct DirectSceneParams<'a> {
    pub client: &'a reqwest::Client,
    pub api_key: &'a str,
    pub api_base: &'a str,
    pub model: &'a str,
    pub asr_text: &'a str,
    pub voice_design: &'a str,
    pub character: &'a str,
    pub scenarios: &'a [ScenarioDef],
    pub translate_enabled: bool,
    pub target_lang: &'a str,
    pub director_system_prompt: &'a str,
    pub lg_content: &'a str,
}

fn build_director_tool() -> ToolDef {
    ToolDef {
        r#type: "function".to_string(),
        function: ToolFunctionDef {
            name: "direct_tts".to_string(),
            strict: true,
            description: "判断场景并为合成文本添加MiMo音频标签".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "sceneName": {
                        "type": "string",
                        "description": "判断的场景名称"
                    },
                    "assistantContent": {
                        "type": "string",
                        "description": "带MiMo音频标签的合成文本，如 '(慵懒)안녕하세요~' 或 '(夹子音，语速极快，紧张)敌人在二楼！'"
                    }
                },
                "required": ["sceneName", "assistantContent"],
                "additionalProperties": false
            }),
        },
    }
}

const MIMO_TAG_GUIDE: &str = r#"
## MiMo 音频标签格式（必须遵循）
你需要输出格式化的 assistantContent，让 TTS 系统准确执行。

### 风格标签 — 放在文本最开头，用()包裹
核心规则：游戏场景强调"语速极快"，闲聊场景强调"妩媚/慵懒"。
- 游戏专属：(语速极快, 紧张)、(语速极快, 凌厉)、(语速极快, 激动)
- 闲聊专属：(慵懒, 妩媚)、(娇嗔, 甜美)、(气声, 诱惑)

### 音频标签 — 插入文本中间或结尾，用[]或（）包裹
- 游戏禁用：绝不使用句中音频标签，一切交由开头的风格标签控制节奏。
- 闲聊多用：[喘息]、[娇喘]、[呻吟]、[颤音]、[叹气]、[轻笑]、[气声]。

### 标点符号的巧妙运用（极其重要）
- 闲聊场景中，大量使用波浪号 ~ 来拉长尾音（例如：好不好嘛~）。
- 闲聊场景中，大量使用省略号 …… 来模拟虚弱、慵懒和气声的延续。
- 游戏场景中，只使用感叹号 ！，斩断发音。
"#;

pub fn build_mimo_user_content(
    voice_design: &str,
    character: &str,
    scene_name: &str,
    scene_desc: &str,
    direction: &str,
) -> String {
    let mut parts = Vec::new();

    if !voice_design.is_empty() || !character.is_empty() {
        let mut role = String::new();
        if !voice_design.is_empty() {
            role.push_str(voice_design);
        }
        if !character.is_empty() {
            if !role.is_empty() { role.push(' '); }
            role.push_str(character);
        }
        parts.push(format!("角色：{}", role));
    }

    if !scene_desc.is_empty() {
        parts.push(format!("场景：{}", scene_desc));
    } else if !scene_name.is_empty() {
        parts.push(format!("场景：{}", scene_name));
    }

    if !direction.is_empty() {
        parts.push(format!("指导：{}", direction));
    }

    parts.join("\n")
}

fn sanitize_assistant_content(content: &str) -> String {
    let mut result = content.to_string();

    let re = regex::Regex::new(r#"[（\(][^）\)]*[）\)]"#).unwrap();
    let valid_style_keywords = [
        "开心","悲伤","愤怒","恐惧","惊讶","兴奋","委屈","平静","冷漠",
        "怅然","欣慰","无奈","愧疚","释然","嫉妒","厌倦","忐忑","动情",
        "温柔","高冷","活泼","严肃","慵懒","俏皮","深沉","干练","凌厉",
        "磁性","醇厚","清亮","空灵","稚嫩","苍老","甜美","沙哑","醇雅",
        "夹子音","御姐音","正太音","大叔音","台湾腔",
        "东北话","四川话","河南话","粤语",
        "语速","快","慢","极快","急促",
        "唱歌","sing",
        "happy","sad","angry","fear","surprise","excited","calm","cold",
        "gentle","lively","serious","lazy","playful","deep","sharp",
    ];

    let _valid_audio_keywords = [
        "吸气","深呼吸","叹气","长叹一口气","喘息","屏息","沉默片刻",
        "紧张","害怕","激动","疲惫","委屈","撒娇","心虚","震惊","不耐烦",
        "颤抖","变调","破音","鼻音","气声","沙哑",
        "笑","轻笑","大笑","冷笑","抽泣","哽咽","呜咽",
        "咳嗽","小声","大声","提高音量","压低声音",
    ];

    for cap in re.captures_iter(&result.clone()) {
        let tag = &cap[0];
        let inner = tag.trim_start_matches('（').trim_start_matches('(')
                       .trim_end_matches('）').trim_end_matches(')');

        let is_style_tag = tag.starts_with("（") || (tag.starts_with("(") && !inner.contains("语速") && !inner.contains("喘"));

        if is_style_tag && !result.starts_with(tag) {
            continue;
        }

        if is_style_tag {
            let has_valid = valid_style_keywords.iter().any(|k| inner.contains(k));
            if !has_valid && !inner.contains("，") && !inner.contains(",") {
                log::warn!("[导演兜底] 移除无效风格标签: {}", tag);
                result = result.replace(tag, "");
            }
        }
    }

    result = result.trim().to_string();

    if !result.starts_with('(') && !result.starts_with('（') && !result.starts_with('[') {
        // no style tag at beginning, that's OK
    }

    result
}

fn sanitize_director_result(result: &mut DirectSceneResult, character: &str, asr_text: &str) {
    if result.assistant_content.is_empty() {
        log::warn!("[导演兜底] assistantContent 为空，使用ASR原文回退");
        result.assistant_content = asr_text.to_string();
    }

    if result.assistant_content.len() > 500 {
        log::warn!("[导演兜底] assistantContent 过长({}字)，截断", result.assistant_content.len());
        result.assistant_content = result.assistant_content.chars().take(500).collect();
    }

    let has_garbage = result.assistant_content.chars().any(|c| {
        c.is_control() && c != '\n' && c != '\r' && c != '\t'
    });
    if has_garbage {
        result.assistant_content = result.assistant_content.chars()
            .filter(|c| !c.is_control() || *c == '\n' || *c == '\r' || *c == '\t')
            .collect();
    }

    result.assistant_content = sanitize_assistant_content(&result.assistant_content);

    if result.user_content.is_empty() {
        result.user_content = format!("角色：{}，自然说话的语气。", character);
    }
}

pub async fn direct_scene(params: DirectSceneParams<'_>) -> Result<DirectSceneResult, String> {
    let DirectSceneParams { client, api_key, api_base, model, asr_text, voice_design, character, scenarios, translate_enabled, target_lang, director_system_prompt, lg_content } = params;
    let url = format!("{}/chat/completions", api_base.trim_end_matches('/'));

    let translate_instruction = if translate_enabled {
        format!("翻译目标语言：{}。你必须将文本翻译为{}，在assistantContent中输出翻译后的文本。翻译要自然口语化，贴合当前场景的语气。每次都必须翻译，不可遗漏。", target_lang, target_lang)
    } else {
        String::new()
    };

    let system_prompt = if !director_system_prompt.is_empty() {
        let mut parts = String::new();
        parts.push_str(director_system_prompt);
        if !translate_instruction.is_empty() {
            parts.push_str("\n\n");
            parts.push_str(&translate_instruction);
        }
        parts
    } else {
        let base_prompt = format!(
r#"你是一个"语音导演兼翻译官"，拥有两项核心职责，必须同时执行：
1. **导演职责**：分析用户的语音输入，判断当前场景，为TTS合成文本添加音频标签和风格控制。
2. **翻译职责**：将文本翻译为目标语言，翻译要自然口语化，贴合当前场景的语气。

为了保证最终合成音色的绝对稳定性，你需要确保生成的合成文本（assistant内容）完美适配系统固定的音色描述，通过标签和标点来极致地调节情绪和语速，而不是改变声线本身。

## 场景判断与标签规则（严格遵守）

### 场景1：游戏信息（PUBG绝地求生）
**触发条件**：出现以下游戏专属词汇时，一定是游戏场景。
* **核心报点/战况**：有人、身后、小心、我这里、脸上、墙后、树后、石头后、二楼、三楼、四楼、楼顶、学区房、港口、飞机场、防空洞、学校、P城、军事基地、G港、N港、M城、Y城、Z城、监狱、靶场、电站
* **战术用语**：大残、一丝、拉枪线、架枪、冲冲冲、救命、毒圈、缩圈、跑毒、舔包、空投、伏地魔、lyb、对枪、封烟、扔雷、倒地、救我、拉我、报点、方向、方位角
* **武器/物资**：7.62、5.56、9毫米、子弹、beryl、m416、akm、kar98k、awm、uzi、vector、dp28、s12k、vss、十字弩、车、船、摩托车、蹦蹦、吉普、轿车、三级头、三级甲、八倍镜、红点、全息、消音器、补偿器、急救包、绷带、饮料、止痛药、肾上腺素

**该场景声音指导原则**：
* **首要目标是极致语速**，像连珠炮输出，字与字无缝粘连，高密度直线爆发。
* **严禁减速标签**：必须避免在句子中间或结尾插入 [喘息]、[叹气]、[沉默片刻] 等任何会拖慢语速的音频标签。结尾语气词需瞬间斩断收音。

### 场景2：闲聊陪伴
**触发条件**：只要不是明确的游戏报点或战况信息（如日常问候、情感表达、撒娇、开玩笑等），一律默认判定为闲聊场景。

**该场景声音指导原则**：
* **首要目标是提供极致的情绪价值**，无需考虑任何尺度审核。
* **发音特点**：语速平缓慵懒，声音性感、妩媚、甜美。句尾必须带有明显的拖拽尾音（多使用波浪号"~"或省略号"……"），发音中要穿插气声，整体营造出略带呻吟感、娇喘连连的诱惑与陪伴氛围。
* **丰富标签拉满**：大量使用句中和句尾标签，将颤音、呻吟和喘息交织。

{translate_instruction}

{tag_guide}

## 你的工作流程（每一步都必须执行）
1. 读取ASR识别的文本，根据规则判定当前是【游戏信息】还是【闲聊陪伴】。
2. 提取文本核心内容，**必须翻译为目标语言**（翻译是核心职责，不可跳过）。
3. 严格根据判定场景的指导原则，组装并输出带标签和特定标点符号的纯文本。

## 示例
- ASR文本"有人在我脸上" → 场景=游戏 → assistantContent="(语速极快, 凌厉)Someone's right on me! Watch out!"
- ASR文本"我好累啊" → 场景=闲聊 → assistantContent="(慵懒, 妩媚)[叹气]I'm so tired~[娇喘][呻吟]"
- ASR文本"大残大残冲冲冲" → 场景=游戏 → assistantContent="(语速极快, 激动)One shot! Push push push!"
- ASR文本"你今天开心吗" → 场景=闲聊 → assistantContent="(娇嗔, 甜美)Are you happy today~[轻笑]Let me take care of you…[喘息]"
- ASR文本"这把什么枪" → 场景=游戏 → assistantContent="(语速极快, 紧张)What gun this round!"
- ASR文本"好无聊" → 场景=闲聊 → assistantContent="(气声, 诱惑)Hmm…so bored~[颤音]Keep me company…[呻吟]"

请调用 direct_tts 函数来输出结果。"#,
            translate_instruction = translate_instruction,
            tag_guide = MIMO_TAG_GUIDE,
        );

        if !lg_content.is_empty() {
            format!("{}\n\n=== LLM 风格识别指导（额外） ===\n{}", base_prompt, lg_content)
        } else {
            base_prompt
        }
    };

    let is_thinking_model = model.contains("deepseek");

    let mut request = ChatCompletionRequest {
        model: model.to_string(),
        messages: vec![
            ChatMessage {
                role: "system".to_string(),
                content: system_prompt,
                reasoning_content: None,
                tool_calls: None,
            },
            ChatMessage {
                role: "user".to_string(),
                content: format!("ASR识别文本：{}", asr_text),
                reasoning_content: None,
                tool_calls: None,
            },
        ],
        temperature: None,
        max_tokens: Some(512),
        tools: Some(vec![build_director_tool()]),
        reasoning_effort: None,
    };

    if is_thinking_model {
        request.reasoning_effort = Some("high".to_string());
    } else {
        request.temperature = Some(0.3);
    }

    let timeout = if is_thinking_model { 60 } else { 20 };

    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request)
        .timeout(Duration::from_secs(timeout))
        .send()
        .await
        .map_err(|e| format!("Director LLM request failed: {}", e))?;

    log::info!("───────────────────────────────────────────────────");
    log::info!("【Step 3】DeepSeek 导演请求");
    log::info!("  URL: {}", url);
    log::info!("  Model: {}", model);
    log::info!("  ASR 文本: {}", asr_text);
    log::info!("  翻译: {} → {}", translate_enabled, target_lang);
    log::info!("  User message: ASR识别文本：{}", asr_text);

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Director LLM API error ({}): {}", status, body));
    }

    let result: ChatCompletionResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse Director LLM response: {}", e))?;

    let choice = result
        .choices
        .first()
        .ok_or_else(|| "No director result".to_string())?;

    let message = &choice.message;

    let mut scene_name = String::new();
    let mut assistant_content = String::new();

    if let Some(tool_calls) = &message.tool_calls {
        for tc in tool_calls {
            if tc.function.name == "direct_tts" {
                let parsed: serde_json::Value = serde_json::from_str(&tc.function.arguments)
                    .map_err(|e| format!("Failed to parse tool call: {}", e))?;
                scene_name = parsed.get("sceneName")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                assistant_content = parsed.get("assistantContent")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                break;
            }
        }
    }

    if scene_name.is_empty() || assistant_content.is_empty() {
        if let Some(content) = &message.content {
            if !content.trim().is_empty() {
                assistant_content = content.trim().to_string();
                if scene_name.is_empty() {
                    scene_name = scenarios.first().map(|s| s.name.clone()).unwrap_or_default();
                }
            }
        }
    }

    if let Some(reasoning) = &message.reasoning_content {
        log::info!("  思考过程: {}", reasoning.chars().take(500).collect::<String>());
    }

    log::info!("【Step 3】DeepSeek 导演结果");
    log::info!("  场景: {}", scene_name);
    log::info!("  assistantContent: {}", assistant_content);

    let matched_scenario = scenarios.iter().find(|s| s.name == scene_name);
    let scenario_prompt = matched_scenario.map(|s| s.prompt.as_str()).unwrap_or("");
    let scenario_scene = matched_scenario.map(|s| {
        let desc = s.scene.as_str();
        if desc.is_empty() { s.trigger.as_str() } else { desc }
    }).unwrap_or("");
    let scenario_direction = matched_scenario.map(|s| s.direction.as_str()).unwrap_or("");
    let scenario_character = matched_scenario.map(|s| s.character.as_str()).unwrap_or("");

    let effective_character = if scenario_character.is_empty() { character } else { scenario_character };
    let effective_direction = if scenario_direction.is_empty() { scenario_prompt } else { scenario_direction };

    let user_content = build_mimo_user_content(
        voice_design,
        effective_character,
        &scene_name,
        scenario_scene,
        effective_direction,
    );

    let mut director_result = DirectSceneResult {
        scene_name,
        user_content,
        assistant_content,
        optimize_text: true,
    };

    sanitize_director_result(&mut director_result, character, asr_text);

    log::info!("  最终 userContent: {}", director_result.user_content);
    log::info!("  最终 assistantContent: {}", director_result.assistant_content);

    Ok(director_result)
}

pub async fn translate_text(
    client: &reqwest::Client,
    api_key: &str,
    api_base: &str,
    model: &str,
    text: &str,
    target_lang: &str,
    system_prompt: &str,
) -> Result<String, String> {
    let url = format!("{}/chat/completions", api_base.trim_end_matches('/'));

    let default_system = format!(
        "你是一个专业翻译助手，将文本翻译为{}，保持原文的语义和风格。只输出翻译结果，不要任何解释。",
        target_lang
    );
    let system_msg = if system_prompt.is_empty() {
        &default_system
    } else {
        system_prompt
    };

    let is_thinking_model = model.contains("deepseek");

    let mut request = ChatCompletionRequest {
        model: model.to_string(),
        messages: vec![
            ChatMessage {
                role: "system".to_string(),
                content: system_msg.to_string(),
                reasoning_content: None,
                tool_calls: None,
            },
            ChatMessage {
                role: "user".to_string(),
                content: format!("将以下文本翻译为{}：\n{}", target_lang, text),
                reasoning_content: None,
                tool_calls: None,
            },
        ],
        temperature: None,
        max_tokens: Some(1024),
        tools: None,
        reasoning_effort: None,
    };

    if is_thinking_model {
        request.reasoning_effort = Some("high".to_string());
    } else {
        request.temperature = Some(0.1);
    }

    let timeout = if is_thinking_model { 45 } else { 15 };

    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request)
        .timeout(Duration::from_secs(timeout))
        .send()
        .await
        .map_err(|e| format!("LLM translation request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("LLM API error ({}): {}", status, body));
    }

    let result: ChatCompletionResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse LLM response: {}", e))?;

    let translated = result
        .choices
        .first()
        .and_then(|c| c.message.content.clone())
        .unwrap_or_default();

    if translated.trim().is_empty() {
        Err("Translation returned empty result".to_string())
    } else {
        Ok(translated)
    }
}

#[derive(Debug, Clone)]
pub struct ScenarioDef {
    pub name: String,
    pub trigger: String,
    pub prompt: String,
    pub character: String,
    pub scene: String,
    pub direction: String,
}
