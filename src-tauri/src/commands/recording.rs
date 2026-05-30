use tauri::State;

use crate::audio::capture;
use crate::audio::resample::resample_pcm;
use crate::constants::AUDIO_TARGET_SAMPLE_RATE;
use crate::http::asr_client;
use crate::state::app_state::AppState;

#[tauri::command]
pub async fn start_recording(
    device_name: Option<String>,
    monitor_device_name: Option<String>,
) -> Result<u32, String> {
    log::info!("═══════════════════════════════════════════════════");
    log::info!("【Step 1】开始录音");
    log::info!("  设备: {}", device_name.as_deref().unwrap_or("默认"));
    log::info!("  监听: {}", monitor_device_name.as_deref().unwrap_or("无"));
    let sr = capture::start_capture(device_name.as_deref(), monitor_device_name.as_deref())?;
    log::info!("  采样率: {}Hz", sr);
    Ok(sr)
}

#[tauri::command]
pub async fn stop_recording_and_transcribe(
    state: State<'_, AppState>,
) -> Result<asr_client::AsrResult, String> {
    log::info!("【Step 1】停止录音");
    let (waveform, device_sr) = capture::stop_capture()
        .ok_or_else(|| "No audio data captured".to_string())?;

    if waveform.is_empty() {
        return Err("Empty audio after recording".to_string());
    }

    let peak = waveform.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    let rms = (waveform.iter().map(|s| s * s).sum::<f32>() / waveform.len() as f32).sqrt();
    let duration = waveform.len() as f64 / device_sr as f64;
    log::info!("  原始数据: {} samples @ {}Hz, {:.2}s", waveform.len(), device_sr, duration);
    log::info!("  波形统计: peak={:.4}, rms={:.4}", peak, rms);

    if duration < 0.6 {
        log::warn!("  ⚠️ 录音过短 ({:.2}s)，跳过 ASR", duration);
        return Ok(asr_client::AsrResult {
            text: String::new(),
            language: "zh".to_string(),
            emotion_tags: Vec::new(),
            confidence: 0.0,
            latency_ms: 0,
        });
    }

    if peak < 0.001 || rms < 0.0005 {
        log::warn!("  ⚠️ 音频可能为静音 (peak={:.4}, rms={:.4})，跳过 ASR", peak, rms);
        return Ok(asr_client::AsrResult {
            text: String::new(),
            language: "zh".to_string(),
            emotion_tags: Vec::new(),
            confidence: 0.0,
            latency_ms: 0,
        });
    }

    let target_sr = AUDIO_TARGET_SAMPLE_RATE;
    let pcm_i16: Vec<i16> = waveform.iter()
        .map(|&s| (s.clamp(-1.0, 1.0) * 32767.0) as i16)
        .collect();
    let pcm_bytes: Vec<u8> = pcm_i16.iter().flat_map(|&s| s.to_le_bytes()).collect();

    let resampled_bytes = if device_sr != target_sr {
        log::info!("  重采样: {}Hz → {}Hz", device_sr, target_sr);
        resample_pcm(&pcm_bytes, device_sr, target_sr)
    } else {
        pcm_bytes
    };

    let wav_data = crate::audio::pcm_to_wav(&resampled_bytes, target_sr, 1);
    log::info!("  WAV 封装: {} bytes", wav_data.len());

    if !*state.asr_loaded.read().await {
        let asr_url = {
            let sidecar = state.asr_sidecar.read().await;
            sidecar.api_url().to_string()
        };
        let alive = asr_client::health_check(&state.http_client, &asr_url).await;
        if alive {
            log::info!("[ASR] ASR was marked dead but health check passed, recovering...");
            {
                let mut asr_loaded = state.asr_loaded.write().await;
                *asr_loaded = true;
            }
        } else {
            return Err("ASR engine not loaded".to_string());
        }
    }

    let asr_url = {
        let sidecar = state.asr_sidecar.read().await;
        sidecar.api_url().to_string()
    };

    let engine = state.asr_engine.read().await.clone();

    log::info!("───────────────────────────────────────────────────");
    log::info!("【Step 2】发送到 ASR 服务器");
    log::info!("  URL: {}/v1/audio/transcriptions", asr_url);
    log::info!("  音频: {} bytes WAV @ {}Hz", wav_data.len(), target_sr);
    log::info!("  模型: {}", engine);

    let start = std::time::Instant::now();
    let result = asr_client::transcribe(
        &state.http_client,
        &asr_url,
        &wav_data,
        target_sr,
        &engine,
        "cuda",
    )
    .await;

    match result {
        Ok(asr_result) => {
            let elapsed_ms = start.elapsed().as_millis() as u64;
            {
                let mut stats = state.stats.write().await;
                stats.asr_count += 1;
                stats.total_asr_ms += elapsed_ms;
            }
            log::info!("【Step 2】ASR 结果 ({}ms)", elapsed_ms);
            log::info!("  文本: {}", asr_result.text);
            log::info!("  语言: {}", asr_result.language);
            log::info!("  情绪标签: {:?}", asr_result.emotion_tags);
            Ok(asr_result)
        }
        Err(e) => {
            log::warn!("【Step 2】ASR 请求失败: {}", e);
            let asr_url_check = {
                let sidecar = state.asr_sidecar.read().await;
                sidecar.api_url().to_string()
            };
            let alive = asr_client::health_check(&state.http_client, &asr_url_check).await;
            if alive {
                log::info!("[ASR] ASR request failed but server is still alive, keeping asr_loaded=true");
            } else {
                log::warn!("[ASR] ASR server is truly dead, marking as dead");
                {
                    let mut sidecar = state.asr_sidecar.write().await;
                    sidecar.mark_dead();
                }
                {
                    let mut asr_loaded = state.asr_loaded.write().await;
                    *asr_loaded = false;
                }
            }
            Err(e)
        }
    }
}
