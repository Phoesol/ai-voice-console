// ============================================================
// audio.js — 录音 + 导演 + 翻译 + TTS 合成 + 音频播放
// 使用 Rust 端 cpal 直接录音（绕过 WebView2 的静音问题）
// ============================================================

const STOP_DELAY_MS = 300;
const MIN_RECORDING_MS = 300;
const FADE_DURATION_S = 0.015;
const PLAYBACK_TIMEOUT_PAD_MS = 2000;
const DEFAULT_VOLUME = 88;
const DEFAULT_SPEED = 98;

let isRecording = false;
let ttsPlaying = false;
let recordingStartTime = 0;
let stopTimer = null;

async function startRecording() {
  try {
    if (window.isAsrReady && !window.isAsrReady()) {
      addMessage('msg-system', 'ASR 还未加载完成，请稍等或点击“加载”重试');
      log('[录音] ASR 未就绪，跳过录音');
      return;
    }

    if (appState === States.SPEAKING) {
      if (currentAudio && currentAudio.source) {
        try { currentAudio.source.stop(); } catch (_) {}
        currentAudio = null;
      }
      ttsPlaying = false;
      setState(States.IDLE);
    }

    if (appState !== States.IDLE) {
      log(`[录音] 当前状态 ${appState}，无法录音`);
      return;
    }

    if (stopTimer) {
      clearTimeout(stopTimer);
      stopTimer = null;
    }

    const micSelect = $('mic-device');
    const deviceName = micSelect?.selectedOptions?.[0]?.dataset?.name || null;
    const monSelect = $('monitor-device');
    const monitorDeviceName = monSelect?.selectedOptions?.[0]?.dataset?.name || null;
    const sr = await tauriInvoke('start_recording', { deviceName, monitorDeviceName });

    isRecording = true;
    recordingStartTime = Date.now();
    setState(States.LISTENING);
    addMessage('msg-system', t('msg_recording'));

    log(`[录音] cpal started @ ${sr}Hz`);
  } catch (e) {
    log(`[录音] 启动失败: ${e}`);
    addMessage('msg-system', `录音启动失败: ${e}`);
    setState(States.IDLE);
  }
}

function stopRecording() {
  if (stopTimer) return;
  isRecording = false;
  stopTimer = setTimeout(() => {
    stopTimer = null;
    _finishRecording();
  }, STOP_DELAY_MS);
}

async function _finishRecording() {
  const durationMs = Date.now() - recordingStartTime;

  if (durationMs < MIN_RECORDING_MS) {
    log('[录音] 过短，忽略');
    addMessage('msg-system', t('msg_no_text'));
    try { await tauriInvoke('stop_recording_and_transcribe'); } catch (e) { log(`[录音] 停止录音失败: ${e}`); }
    setState(States.IDLE);
    return;
  }

  if (ttsPlaying) {
    addMessage('msg-system', '🛡️ 反环路: TTS播放中，丢弃ASR输入');
    try { await tauriInvoke('stop_recording_and_transcribe'); } catch (e) { log(`[录音] 停止录音失败: ${e}`); }
    setState(States.IDLE);
    return;
  }

  setState(States.THINKING);
  addMessage('msg-system', t('msg_asr_processing'));

  log(`[录音] 结束，${durationMs}ms`);

  try {
    const result = await tauriInvoke('stop_recording_and_transcribe');

    if (result && result.text && result.text.trim()) {
      const emotionInfo = result.emotionTags && result.emotionTags.length > 0
        ? ` [${result.emotionTags.join(', ')}]` : '';
      addMessage('msg-user', result.text + emotionInfo);
      log(`[ASR] ${result.text} (lang: ${result.language})${emotionInfo}`);
      processAsrText(result.text, result.emotionTags || []);
    } else {
      addMessage('msg-system', t('msg_no_text'));
      setState(States.IDLE);
    }
  } catch (err) {
    log(`[ASR] 错误: ${err}`);
    addMessage('msg-system', `ASR识别失败: ${err}`);
    if (window.loadAsrInBackground) {
      window.loadAsrInBackground('auto');
    }
    setState(States.IDLE);
  }
}

// --- Core Pipeline: ASR text → Director/Translate → TTS ---

async function processAsrText(text, emotionTags = []) {
  const directorEnabled = $('director-enable')?.checked;
  const translateEnabled = $('translate-enable')?.checked;

  if (directorEnabled) {
    await processWithDirector(text, translateEnabled, emotionTags);
  } else if (translateEnabled) {
    await processWithTranslate(text, emotionTags);
  } else {
    await synthesize(text, emotionTags);
  }
}

async function processWithDirector(text, translateEnabled, emotionTags = []) {
  setState(States.THINKING);
  addMessage('msg-system', t('msg_director_analyzing'));
  log('[导演] 文本模型导演分析场景...');

  try {
    const directorResult = await tauriInvoke('direct_scene', { asrText: text, translateEnabled });

    if (directorResult) {
      log(`[导演] 场景: ${directorResult.sceneName}`);
      log(`[导演] userContent: ${directorResult.userContent.substring(0, 80)}...`);
      log(`[导演] assistantContent: ${directorResult.assistantContent}`);
      addMessage('msg-system', `🎬 场景: ${directorResult.sceneName}`);

      const ttsResult = await tauriInvoke('synthesize_directed', {
        userContent: directorResult.userContent,
        assistantContent: directorResult.assistantContent,
        optimizeText: directorResult.optimizeText,
      });

      if (ttsResult && ttsResult.success && ttsResult.audioBase64) {
        setState(States.SPEAKING);
        const displayText = translateEnabled
          ? `${text} → ${directorResult.assistantContent}`
          : directorResult.assistantContent;
        addMessage('msg-ai', displayText, `导演:${directorResult.sceneName}`);
        playAudio(ttsResult.audioBase64);
      } else {
        addMessage('msg-system', `导演TTS失败: ${ttsResult?.error || '未知错误'}`);
        setState(States.IDLE);
      }
    } else {
      addMessage('msg-system', '导演分析失败，使用普通模式');
      if (translateEnabled) {
        await processWithTranslate(text, emotionTags);
      } else {
        await synthesize(text, emotionTags);
      }
    }
  } catch (e) {
    log(`[导演] 错误: ${e}`);
    addMessage('msg-system', `导演错误: ${e}，回退普通模式`);
    if (translateEnabled) {
      await processWithTranslate(text, emotionTags);
    } else {
      await synthesize(text, emotionTags);
    }
  }
}

async function processWithTranslate(text, emotionTags = []) {
  setState(States.THINKING);
  const targetLang = $('translate-lang')?.value || 'Korean';
  log(`[翻译] → ${targetLang}`);

  try {
    const translated = await tauriInvoke('translate', { text, targetLang });
    if (translated) {
      addMessage('msg-ai', translated, `翻译 → ${targetLang}`);
      await synthesize(translated, emotionTags);
    } else {
      addMessage('msg-system', '翻译失败，使用原文合成');
      await synthesize(text, emotionTags);
    }
  } catch (e) {
    log(`[翻译] 错误: ${e}`);
    addMessage('msg-system', `翻译错误: ${e}，使用原文`);
    await synthesize(text, emotionTags);
  }
}

async function synthesize(text, emotionTags = []) {
  if (!text || !text.trim()) { setState(States.IDLE); return; }

  setState(States.THINKING);
  const engine = $('tts-engine')?.value || 'mimo_tts';
  log(`[TTS] 合成中... 引擎: ${engine}`);

  try {
    const result = await tauriInvoke('synthesize', { text, ttsEngine: engine, emotionTags });
    if (result && result.success && result.audioBase64) {
      setState(States.SPEAKING);
      addMessage('msg-ai', text, `引擎: ${result.engine}`);
      playAudio(result.audioBase64);
    } else {
      addMessage('msg-system', `TTS 合成失败: ${result?.error || '未知错误'}`);
      setState(States.IDLE);
    }
  } catch (e) {
    log(`[TTS] 错误: ${e}`);
    addMessage('msg-system', `TTS 合成失败: ${e}`);
    setState(States.IDLE);
  }
}

let currentAudio = null;
let audioCtx = null;
let currentSinkId = '';
let currentMonitorSinkId = '';

function getAudioContext() {
  if (!audioCtx || audioCtx.state === 'closed') {
    audioCtx = new (window.AudioContext || window.webkitAudioContext)();
  }
  return audioCtx;
}

async function syncOutputDevice() {
  const outSel = $('output-device');
  if (!outSel) return;
  const selectedName = outSel.selectedOptions?.[0]?.dataset?.name || outSel.selectedOptions?.[0]?.textContent?.split(' (')?.[0] || '';
  if (!selectedName || !audioCtx) return;

  try {
    const devices = await navigator.mediaDevices.enumerateDevices();
    const audioOutputs = devices.filter(d => d.kind === 'audiooutput');
    if (audioOutputs.length > 0 && !audioOutputs[0].label) {
      try {
        const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
        stream.getTracks().forEach(t => t.stop());
      } catch (_) {}
      return syncOutputDevice();
    }

    let matched = audioOutputs.find(d => d.label && d.label.includes(selectedName));
    if (!matched && audioOutputs.length > 0) {
      matched = audioOutputs.find(d => d.label && selectedName.includes(d.label));
    }

    const sinkId = matched ? matched.deviceId : '';
    if (sinkId !== currentSinkId && typeof audioCtx.setSinkId === 'function') {
      await audioCtx.setSinkId(sinkId);
      currentSinkId = sinkId;
      log(`[播放] 输出设备: ${matched?.label || '系统默认'}`);
    }
  } catch (e) {
    log(`[播放] 设置输出设备失败: ${e}`);
  }
}

// 获取监听（试听）设备名 — 用于 TTS 输出同时播放到此设备
function getMonitorDeviceName() {
  const monSel = $('monitor-device');
  if (!monSel) return null;
  const val = monSel.value;
  if (!val || val === '') return null;  // "无（不监听）"
  return monSel.selectedOptions?.[0]?.dataset?.name || null;
}

function getOutputDeviceName() {
  const outSel = $('output-device');
  if (!outSel) return null;
  return outSel.selectedOptions?.[0]?.dataset?.name || null;
}

function playAudio(base64) {
  try {
    if (currentAudio && currentAudio.source) {
      try { currentAudio.source.stop(); } catch (_) {}
      currentAudio = null;
    }

    ttsPlaying = true;
    const binaryStr = atob(base64);
    const bytes = new Uint8Array(binaryStr.length);
    for (let i = 0; i < binaryStr.length; i++) bytes[i] = binaryStr.charCodeAt(i);

    log(`[播放] 音频数据 ${bytes.length} bytes, 开始解码`);
    const ctx = getAudioContext();
    const doPlay = async () => {
      await syncOutputDevice();
      ctx.decodeAudioData(bytes.buffer.slice(0), (audioBuffer) => {
        log(`[播放] 解码成功: ${audioBuffer.duration.toFixed(2)}s, ${audioBuffer.numberOfChannels}ch, ${audioBuffer.sampleRate}Hz`);
        const source = ctx.createBufferSource();
        source.buffer = audioBuffer;

        const gainNode = ctx.createGain();
        const volume = ($('volume')?.value ?? DEFAULT_VOLUME) / 100;
        const playbackRate = ($('playback-speed')?.value ?? DEFAULT_SPEED) / 100;
        const fadeDuration = FADE_DURATION_S;
        const duration = audioBuffer.duration;
        const now = ctx.currentTime;

        if (duration > fadeDuration * 2) {
          gainNode.gain.setValueAtTime(0, now);
          gainNode.gain.linearRampToValueAtTime(volume, now + fadeDuration);
          gainNode.gain.setValueAtTime(volume, now + duration - fadeDuration);
          gainNode.gain.linearRampToValueAtTime(0, now + duration);
        } else {
          gainNode.gain.setValueAtTime(volume, now);
        }

        source.connect(gainNode);
        gainNode.connect(ctx.destination);
        source.playbackRate.value = playbackRate;

        log(`[播放] 开始播放 rate=${playbackRate.toFixed(2)} vol=${volume.toFixed(2)}`);
        let ended = false;
        const onEnded = () => {
          if (ended) return; ended = true;
          log('[播放] 播放完成');
          ttsPlaying = false;
          setState(States.IDLE);
          addMessage('msg-system', t('msg_playback_done'));
        };
        source.onended = onEnded;
        // 安全兜底：超时自动结束（防止 onended 不触发）
        setTimeout(() => onEnded(), (duration * 1000) + PLAYBACK_TIMEOUT_PAD_MS);

        source.start(0);
        currentAudio = { source, gainNode };

        // 同时输出到监听设备（试听）
        const monDevName = getMonitorDeviceName();
        const outDevName = getOutputDeviceName();
        if (monDevName && monDevName !== outDevName) {
          playAudioToMonitor(bytes.buffer.slice(0), monDevName, volume, playbackRate);
        }
      }, (e) => {
        log(`[播放] 解码失败: ${e}, 回退 Audio 元素`);
        playAudioFallback(base64);
      });
    };

    if (ctx.state === 'suspended') {
      ctx.resume().then(doPlay).catch(e => {
        log(`[播放] AudioContext resume 失败: ${e}, 回退 Audio 元素`);
        playAudioFallback(base64);
      });
    } else {
      doPlay();
    }
  } catch (e) {
    log(`[播放] 失败: ${e}`);
    ttsPlaying = false;
    setState(States.IDLE);
  }
}

// 将音频同时播放到监听设备（独立 AudioContext）
async function playAudioToMonitor(arrayBuffer, deviceName, volume, playbackRate) {
  try {
    log(`[试听] 同时输出到监听设备: ${deviceName}`);
    const ctx = new (window.AudioContext || window.webkitAudioContext)();

    // 设置输出设备
    try {
      const devices = await navigator.mediaDevices.enumerateDevices();
      const audioOutputs = devices.filter(d => d.kind === 'audiooutput');
      const matched = audioOutputs.find(d => d.label && d.label.includes(deviceName))
        || audioOutputs.find(d => d.label && deviceName.includes(d.label));
      if (matched && typeof ctx.setSinkId === 'function') {
        await ctx.setSinkId(matched.deviceId);
        log(`[试听] 监听设备已设置: ${matched.label}`);
      }
    } catch (e) { log(`[试听] 设置监听设备失败: ${e}`); }

    ctx.decodeAudioData(arrayBuffer.slice(0), (audioBuffer) => {
      const source = ctx.createBufferSource();
      source.buffer = audioBuffer;
      const gainNode = ctx.createGain();
      const fadeDuration = FADE_DURATION_S;
      const duration = audioBuffer.duration;
      const now = ctx.currentTime;

      if (duration > fadeDuration * 2) {
        gainNode.gain.setValueAtTime(0, now);
        gainNode.gain.linearRampToValueAtTime(volume, now + fadeDuration);
        gainNode.gain.setValueAtTime(volume, now + duration - fadeDuration);
        gainNode.gain.linearRampToValueAtTime(0, now + duration);
      } else {
        gainNode.gain.setValueAtTime(volume, now);
      }

      source.connect(gainNode);
      gainNode.connect(ctx.destination);
      source.playbackRate.value = playbackRate;
      source.start(0);
      source.onended = () => { ctx.close().catch(() => {}); };
      setTimeout(() => { try { ctx.close(); } catch (_) {} }, (duration * 1000) + PLAYBACK_TIMEOUT_PAD_MS);
    }, (e) => {
      log(`[试听] 监听设备解码失败: ${e}`);
      try { ctx.close(); } catch (_) {}
    });

    if (ctx.state === 'suspended') {
      ctx.resume().catch(() => {});
    }
  } catch (e) {
    log(`[试听] 监听设备播放失败: ${e}`);
  }
}

function playAudioFallback(base64) {
  try {
    const binaryStr = atob(base64);
    const bytes = new Uint8Array(binaryStr.length);
    for (let i = 0; i < binaryStr.length; i++) bytes[i] = binaryStr.charCodeAt(i);
    const blob = new Blob([bytes], { type: 'audio/wav' });
    const url = URL.createObjectURL(blob);

    if (currentAudio && currentAudio.pause) { currentAudio.pause(); currentAudio = null; }
    const audio = new Audio();
    audio.volume = ($('volume')?.value ?? DEFAULT_VOLUME) / 100;
    audio.playbackRate = ($('playback-speed')?.value ?? DEFAULT_SPEED) / 100;
    audio.src = url;

    audio.onended = () => {
      ttsPlaying = false;
      setState(States.IDLE);
      addMessage('msg-system', t('msg_playback_done'));
      URL.revokeObjectURL(url);
    };
    audio.onerror = (e) => {
      ttsPlaying = false;
      setState(States.IDLE);
      log(`[播放] 错误: ${e}`);
      URL.revokeObjectURL(url);
    };
    audio.play().catch(e => {
      log(`[播放] play() 失败: ${e}`);
      ttsPlaying = false;
      setState(States.IDLE);
    });
    currentAudio = audio;
  } catch (e) {
    log(`[播放] 回退播放失败: ${e}`);
    ttsPlaying = false;
    setState(States.IDLE);
  }
}

window.startRecording = startRecording;
window.stopRecording = stopRecording;
window.synthesize = synthesize;
window.processAsrText = processAsrText;
window.playAudio = playAudio;
window.getIsRecording = () => isRecording;
window.getCurrentAudio = () => currentAudio;
window.getMonitorDeviceName = getMonitorDeviceName;
window.getOutputDeviceName = getOutputDeviceName;
