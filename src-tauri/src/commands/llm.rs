// LLM Commands — 翻译 + 文本导演

use tauri::State;
use crate::state::app_state::AppState;
use crate::http::llm_client;
use crate::http::llm_client::ScenarioDef;

/// 翻译文本（独立功能）
#[tauri::command]
pub async fn translate(
    text: String,
    target_lang: String,
    system_prompt: Option<String>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let settings = state.settings.read().await;

    if settings.deepseek_api_key.is_empty() {
        return Err("DeepSeek API Key not configured".to_string());
    }

    let sp = system_prompt.unwrap_or_default();

    let api_key = settings.deepseek_api_key.clone();
    let api_base = settings.deepseek_api_base.clone();
    let model = settings.deepseek_model.clone();
    drop(settings);

    let start = std::time::Instant::now();
    let result = llm_client::translate_text(
        &state.http_client,
        &api_key,
        &api_base,
        &model,
        &text,
        &target_lang,
        &sp,
    )
    .await;

    match result {
        Ok(translated) => {
            let elapsed_ms = start.elapsed().as_millis() as u64;
            let mut stats = state.stats.write().await;
            stats.translate_count += 1;
            stats.total_translate_ms += elapsed_ms;
            Ok(translated)
        }
        Err(e) => Err(e),
    }
}

/// 文本导演：判断场景 + 生成 MiMo TTS 消息
#[tauri::command]
pub async fn direct_scene(
    asr_text: String,
    translate_enabled: Option<bool>,
    state: State<'_, AppState>,
) -> Result<llm_client::DirectSceneResult, String> {
    let settings = state.settings.read().await;

    if settings.deepseek_api_key.is_empty() {
        return Err("DeepSeek API Key not configured".to_string());
    }

    if !settings.text_model_director_enabled {
        return Err("Text model director is disabled".to_string());
    }

    let do_translate = translate_enabled.unwrap_or(settings.translate_enabled);

    let scenarios: Vec<ScenarioDef> = settings
        .text_model_director_scenarios
        .iter()
        .map(|s| ScenarioDef {
            name: s.name.clone(),
            trigger: s.trigger.clone(),
            prompt: s.prompt.clone(),
            character: s.character.clone(),
            scene: s.scene.clone(),
            direction: s.direction.clone(),
        })
        .collect();

    if scenarios.is_empty() {
        return Err("No scenarios configured for text model director".to_string());
    }

    let director_system_prompt = settings.director_system_prompt.clone();

    let lg_content: String = settings
        .llm_style_guides
        .iter()
        .map(|g| {
            let mut parts = Vec::new();
            parts.push(format!("【{}】", g.name));
            parts.push(g.content.clone());
            parts.join("\n")
        })
        .collect::<Vec<String>>()
        .join("\n\n");

    llm_client::direct_scene(llm_client::DirectSceneParams {
        client: &state.http_client,
        api_key: &settings.deepseek_api_key,
        api_base: &settings.deepseek_api_base,
        model: &settings.deepseek_model,
        asr_text: &asr_text,
        voice_design: &settings.mimo_voice_design,
        character: &settings.mimo_character,
        scenarios: &scenarios,
        translate_enabled: do_translate,
        target_lang: &settings.translate_target_lang,
        director_system_prompt: &director_system_prompt,
        lg_content: &lg_content,
    })
    .await
}

#[tauri::command]
pub async fn test_llm_connection(
    api_key: String,
    api_base: String,
    model: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let client = &state.http_client;
    let url = format!("{}/chat/completions", api_base.trim_end_matches('/'));

    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "model": model,
            "messages": [{"role": "user", "content": "hi"}],
            "max_tokens": 1
        }))
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| format!("Connection failed: {}", e))?;

    Ok(resp.status().is_success())
}

/// 模块3: 导演提示词生成 — 调用 LLM 根据上下文生成新的识别指导
#[tauri::command]
pub async fn generate_director_prompt(
    prompt: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let settings = state.settings.read().await;

    if settings.deepseek_api_key.is_empty() {
        return Err("DeepSeek API Key not configured".to_string());
    }

    let api_key = settings.deepseek_api_key.clone();
    let api_base = settings.deepseek_api_base.clone();
    let model = settings.deepseek_model.clone();
    drop(settings);

    let client = &state.http_client;
    let url = format!("{}/chat/completions", api_base.trim_end_matches('/'));

    let system_msg = "你是一位资深的语音合成（TTS）系统提示词工程师。根据用户提供的多种语音配置参考，生成一份结构清晰、指令明确的新版\"文本模型导演\"系统提示词。输出应为纯 Markdown 格式的提示词全文，可直接用于指导 LLM 进行语音标签生成。";

    let request = serde_json::json!({
        "model": model,
        "messages": [
            {"role": "system", "content": system_msg},
            {"role": "user", "content": format!("请基于以下配置生成新的文本模型导演系统提示词：\n\n{}", prompt)}
        ],
        "temperature": 0.4,
        "max_tokens": 4096
    });

    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request)
        .timeout(std::time::Duration::from_secs(120))
        .send()
        .await
        .map_err(|e| format!("LLM request failed: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("LLM API error ({}): {}", status, body));
    }

    let result: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse LLM response: {}", e))?;

    let content = result["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();

    Ok(content)
}
