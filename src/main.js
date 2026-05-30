// ============================================================
// AI Voice Console — Tauri v2 Frontend  (main.js)
// 入口 + 初始化 + 工具函数 + 事件监听
// 依赖：state.js / audio.js / settings-ui.js  (按此顺序在 index.html 中加载)
// ============================================================

const __T = window.__TAURI__;
const invoke = __T?.core?.invoke;
const listen = __T?.event?.listen;
const getCurrentWindow = __T?.window?.getCurrentWindow;

const PTT_DEBOUNCE_MS = 150;
const STATS_REFRESH_MS = 5000;

// JS 层 PTT 热键兜底 — 当全局 Hook 不触发时（WebView2 焦点场景），由前端键盘事件直接处理
let _lastJsPttDown = 0;
let _lastJsPttUp = 0;
let _asrLoading = false;
let _asrLoaded = false;
const _activePttSources = new Set();

// ============================================================
// Waveform — 动态生成 animation-delay
// ============================================================
function initWaveBars() {
  const bars = document.querySelectorAll('.wave-bar');
  const step = 0.05;
  bars.forEach((bar, i) => {
    bar.style.animationDelay = `${i * step}s`;
  });
}

// ============================================================
// Utils (shared)
// ============================================================
function $(id) { return document.getElementById(id); }

function log(msg) {
  const box = $('log-box');
  const ts = new Date().toLocaleTimeString();
  const div = document.createElement('div');
  div.textContent = `[${ts}] ${msg}`;
  box.appendChild(div);
  box.scrollTop = box.scrollHeight;
}

function addMessage(type, text, extra = '') {
  const stage = $('messages');
  const scrollStage = $('chat-stage');
  const empty = $('empty-state');
  if (empty) empty.style.display = 'none';

  const msg = document.createElement('div');
  msg.className = `msg ${type}`;

  const label = document.createElement('div');
  label.className = 'msg-label';
  label.textContent = type === 'msg-user' ? 'YOU' : type === 'msg-ai' ? 'AI' : 'SYS';
  msg.appendChild(label);

  const content = document.createElement('div');
  content.className = 'msg-text';
  content.textContent = text;
  msg.appendChild(content);

  if (extra) {
    const ex = document.createElement('div');
    ex.className = 'msg-time';
    ex.textContent = extra;
    msg.appendChild(ex);
  } else {
    const time = document.createElement('div');
    time.className = 'msg-time';
    time.textContent = new Date().toLocaleTimeString();
    msg.appendChild(time);
  }

  stage.appendChild(msg);
  requestAnimationFrame(() => {
    if (scrollStage) {
      scrollStage.scrollTo({ top: scrollStage.scrollHeight, behavior: 'smooth' });
    }
    msg.scrollIntoView({ block: 'end', behavior: 'smooth' });
  });
  return msg;
}

function beginPtt(source) {
  const wasIdle = _activePttSources.size === 0;
  _activePttSources.add(source);
  if (wasIdle && appState === States.IDLE) {
    startRecording();
  }
}

function endPtt(source) {
  _activePttSources.delete(source);
  if (_activePttSources.size === 0 && getIsRecording()) {
    stopRecording();
  }
}

function resetPttSources() {
  _activePttSources.clear();
}

async function tauriInvoke(command, args = {}) {
  if (!invoke) { log(`[IPC Error] invoke 不可用，无法调用 ${command}`); return null; }
  try {
    const result = await invoke(command, args);
    return result;
  } catch (e) {
    log(`[IPC Error] ${command}: ${e}`);
    return null;
  }
}

async function loadAsrInBackground(source = 'manual') {
  if (_asrLoading) {
    log('[ASR] 已在加载中，跳过重复请求');
    return null;
  }

  _asrLoading = true;
  const engine = $('asr-engine')?.value || 'qwen3_asr';
  const statusEl = $('asr-status');
  if (statusEl) {
    statusEl.textContent = '⏳ 加载中...';
    statusEl.className = 'status-text';
  }
  log(source === 'auto' ? '[ASR] 后台自动加载 ASR 引擎...' : `[ASR] 加载: ${engine}`);

  const result = await tauriInvoke('load_asr', { engine });
  _asrLoading = false;

  if (result) {
    _asrLoaded = true;
    if (statusEl) {
      statusEl.textContent = `✅ ${result.engine}`;
      statusEl.className = 'status-text success';
    }
    updateStatusBar(true);
    updateEmotionStatus();
    log(`[ASR] ${result.engine} 加载成功 ✓`);
  } else {
    _asrLoaded = false;
    if (statusEl) {
      statusEl.textContent = '❌ 加载失败，点击加载重试';
      statusEl.className = 'status-text danger';
    }
    updateStatusBar(false);
    updateEmotionStatus();
    log('[ASR] 加载失败，请手动点击加载按钮');
  }

  return result;
}

window.isAsrReady = () => _asrLoaded && !_asrLoading;
window.loadAsrInBackground = loadAsrInBackground;

// ============================================================
// Window Controls
// ============================================================
function initWindowControls() {
  if (!getCurrentWindow) { log('[窗口] getCurrentWindow 不可用，跳过窗口控制'); return; }
  const appWindow = getCurrentWindow();
  $('btn-minimize').addEventListener('click', () => appWindow.minimize());
  $('btn-maximize').addEventListener('click', () => appWindow.toggleMaximize());
  $('btn-close').addEventListener('click', () => appWindow.close());
}

// ============================================================
// Settings Drawer — 使用 inline style.transform 控制，避免 CSS 类冲突
// ============================================================
function initDrawer() {
  const overlay = $('settings-overlay');
  const drawer = $('settings-drawer');

  drawer.style.transform = 'translateX(100%)';

  const open = () => {
    overlay.classList.add('visible');
    drawer.style.transform = 'translateX(0)';
  };

  const close = () => {
    overlay.classList.remove('visible');
    drawer.style.transform = 'translateX(100%)';
  };

  $('btn-settings').addEventListener('click', open);
  $('btn-close-settings').addEventListener('click', close);
  overlay.addEventListener('click', close);

  document.querySelectorAll('.stab').forEach(btn => {
    btn.addEventListener('click', () => {
      document.querySelectorAll('.stab').forEach(b => b.classList.remove('active'));
      document.querySelectorAll('.stab-panel').forEach(p => p.classList.remove('active'));
      btn.classList.add('active');
      const tab = btn.dataset.stab;
      $(`stab-${tab}`).classList.add('active');
    });
  });
}

// ============================================================
// Log Panel Toggle
// ============================================================
function initLogPanel() {
  const panel = $('log-panel');
  let expanded = false;

  const toggle = () => {
    expanded = !expanded;
    panel.classList.toggle('log-collapsed', !expanded);
    panel.classList.toggle('log-expanded', expanded);
  };

  $('btn-log').addEventListener('click', toggle);
  $('btn-toggle-log').addEventListener('click', toggle);

  $('btn-copy-log').addEventListener('click', () => {
    try {
      navigator.clipboard.writeText($('log-box').textContent);
      log('[日志] 已复制到剪贴板');
    } catch (e) {
      log(`[日志] 复制失败: ${e.message || e}`);
    }
  });

  $('btn-clear-log').addEventListener('click', () => {
    $('log-box').replaceChildren();
  });
}

// ============================================================
// TTS Modal
// ============================================================
function initTtsModal() {
  $('btn-tts-input').addEventListener('click', () => {
    $('tts-modal').classList.add('visible');
  });

  $('btn-close-tts-modal').addEventListener('click', () => {
    $('tts-modal').classList.remove('visible');
  });

  $('btn-tts-generate').addEventListener('click', async () => {
    const text = $('tts-input').value.trim();
    if (!text) return;
    await synthesize(text);
  });
}

// ============================================================
// Tauri Event Listeners
// ============================================================
function initEventListeners() {
  if (!listen) { log('[事件] listen 不可用，跳过 Tauri 事件监听'); return; }
  listen('pipeline_status', (event) => {
    const status = event.payload;
    log(`[管道] ${status}`);
    if (status === 'running') {
      $('pipeline-badge').textContent = 'LIVE';
      $('pipeline-badge').className = 'badge badge-running';
    } else if (status === 'stopped') {
      setState(States.IDLE);
    }
  });

  listen('sidecar_log', (event) => {
    log(`[Sidecar] ${event.payload}`);
  });

  listen('ptt_status', (event) => {
    log(`[PTT] ${event.payload}`);
  });

  listen('asr_process_died', async () => {
    log('[ASR] ⚠️ ASR 进程意外退出，请点击“加载”重启');
    _asrLoaded = false;
    $('asr-status').textContent = '❌ 已退出，点击加载重启';
    $('asr-status').className = 'status-text danger';
    updateStatusBar(false);
  });

  listen('hotkey_down', (event) => {
    const which = event.payload;
    // JS 层已处理（100ms内），跳过避免双重触发
    if (Date.now() - _lastJsPttDown < PTT_DEBOUNCE_MS) {
      log(`[热键] 热键${which}按下 (JS已处理，跳过Hook事件)`);
      return;
    }
    log(`[热键] 热键${which}按下 (Hook)`);
    beginPtt(`hook:${which}`);
  });

  listen('hotkey_up', (event) => {
    const which = event.payload;
    if (Date.now() - _lastJsPttUp < PTT_DEBOUNCE_MS) {
      log(`[热键] 热键${which}释放 (JS已处理，跳过Hook事件)`);
      return;
    }
    log(`[热键] 热键${which}释放 (Hook)`);
    endPtt(`hook:${which}`);
  });
}

// ============================================================
// UI Event Bindings
// ============================================================
function bindUIEvents() {
  const isEditable = (el) => el && (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA' || el.tagName === 'SELECT');

  $('mic-btn').addEventListener('click', () => {
    if (appState === States.IDLE) {
      resetPttSources();
      startRecording();
    } else if (appState === States.LISTENING) {
      resetPttSources();
      stopRecording();
    }
  });

  const hotkey1Input = $('hotkey1-key');
  let hotkey1Listening = false;
  let hotkey1PrevValue = '';

  hotkey1Input.addEventListener('click', () => {
    hotkey1PrevValue = hotkey1Input.value;
    hotkey1Listening = true;
    hotkey1Input.value = '按下按键...';
    hotkey1Input.classList.add('listening');
  });

  document.addEventListener('mousedown', (e) => {
    if (hotkey1Listening) {
      e.preventDefault();
      if (e.button === 1) {
        hotkey1Input.value = 'middle_mouse';
      } else if (e.button === 0 || e.button === 2) {
        hotkey1Input.value = 'mouse_' + (e.button === 0 ? 'left' : 'right');
      } else {
        return;
      }
      hotkey1Input.classList.remove('listening');
      hotkey1Listening = false;
      syncHotkeyToBackend();
      return;
    }
  });

  document.addEventListener('keydown', (e) => {
    if (hotkey1Listening) {
      e.preventDefault();
      e.stopPropagation();
      let keyName = e.key.toLowerCase();
      if (keyName === 'backspace') {
        hotkey1Input.value = '';
        hotkey1Input.classList.remove('listening');
        hotkey1Listening = false;
        syncHotkeyToBackend();
        return;
      }
      if (keyName === 'escape') {
        hotkey1Input.value = hotkey1PrevValue;
        hotkey1Input.classList.remove('listening');
        hotkey1Listening = false;
        return;
      }
      if (keyName === 'control' || keyName === 'shift' || keyName === 'alt' || keyName === 'meta') return;
      if (keyName === ' ') keyName = 'space';
      hotkey1Input.value = keyName;
      hotkey1Input.classList.remove('listening');
      hotkey1Listening = false;
      syncHotkeyToBackend();
      return;
    }
  });

  function syncHotkeyToBackend() {
    resetPttSources();
    const hk1Key = hotkey1Input?.value || '';
    const hk1Mod = $('hotkey1-modifier')?.value || 'none';
    const hk2Key = $('ptt-key2')?.value || 'none';
    const enabled = $('ptt-enable')?.checked || false;
    tauriInvoke('configure_hotkey', { hk1Key, hk1Mod, hk2Key, enabled });
  }

  $('ptt-enable').addEventListener('change', async () => {
    syncHotkeyToBackend();
    const checked = $('ptt-enable').checked;
    const statusEl = $('ptt-status');
    if (checked) {
      log('[热键] 已启用');
      if (statusEl) { statusEl.textContent = '● 热键已启用'; statusEl.className = 'status-text success'; }
    } else {
      log('[热键] 已禁用');
      if (statusEl) { statusEl.textContent = '○ 热键未启用'; statusEl.className = 'status-text stopped'; }
    }
  });

  $('hotkey1-modifier').addEventListener('change', () => syncHotkeyToBackend());
  $('ptt-key2').addEventListener('change', () => syncHotkeyToBackend());

  $('volume').addEventListener('input', () => {
    $('volume-label').value = $('volume').value;
    const audio = window.getCurrentAudio?.();
    if (audio) audio.volume = $('volume').value / 100;
  });
  $('volume-label').addEventListener('change', () => {
    let v = parseInt($('volume-label').value) || 0;
    v = Math.max(0, Math.min(100, v));
    $('volume-label').value = v;
    $('volume').value = v;
    const audio = window.getCurrentAudio?.();
    if (audio) audio.volume = v / 100;
  });

  $('playback-speed').addEventListener('input', () => {
    const speed = $('playback-speed').value / 100;
    $('playback-speed-label').value = speed.toFixed(2);
    const audio = window.getCurrentAudio?.();
    if (audio) audio.playbackRate = speed;
  });
  $('playback-speed-label').addEventListener('change', () => {
    let s = parseFloat($('playback-speed-label').value) || 0.5;
    s = Math.max(0.50, Math.min(3.00, s));
    $('playback-speed-label').value = s.toFixed(2);
    $('playback-speed').value = Math.round(s * 100);
    const audio = window.getCurrentAudio?.();
    if (audio) audio.playbackRate = s;
  });

  $('btn-refresh-devices').addEventListener('click', initDevices);

  $('host-api').addEventListener('change', () => {
    if (typeof populateDeviceLists === 'function') populateDeviceLists();
  });

  $('emotion-model').addEventListener('change', updateEmotionStatus);

  $('wasapi-loopback').addEventListener('change', async () => {
    if ($('wasapi-loopback').checked) {
      const speakerName = $('speaker-device')?.selectedOptions[0]?.textContent || '';
      const result = await tauriInvoke('start_loopback_capture', { deviceName: speakerName });
      if (result) {
        log('[Loopback] 内录已启动');
      }
    } else {
      await tauriInvoke('stop_loopback_capture');
      log('[Loopback] 内录已停止');
    }
  });

  $('btn-load-asr').addEventListener('click', async () => {
    await loadAsrInBackground('manual');
  });

  $('btn-connect-api').addEventListener('click', async () => {
    log('[Sidecar] 启动 ASR 服务...');
    const port = parseInt($('tts-api-url').value.match(/:(\d+)/)?.[1] || '18765');
    const result = await tauriInvoke('start_asr_sidecar', { port });
    if (result) {
      $('tts-server-status').textContent = '运行中';
      $('tts-server-status').className = 'status-text success';
    }
  });

  $('btn-test-llm').addEventListener('click', async () => {
    const key = $('deepseek-api-key').value;
    const base = $('deepseek-api-base').value;
    const model = $('deepseek-model').value;
    if (!key) { $('llm-test-status').textContent = '❌ 请输入Key'; return; }
    $('llm-test-status').textContent = '测试中...';
    const result = await tauriInvoke('test_llm_connection', { apiKey: key, apiBase: base, model });
    if (result) {
      $('llm-test-status').textContent = '✅ 连接成功';
      $('llm-test-status').className = 'status-text success';
    } else {
      $('llm-test-status').textContent = '❌ 连接失败';
      $('llm-test-status').className = 'status-text danger';
    }
  });

  $('btn-test-mimo').addEventListener('click', async () => {
    const key = $('mimo-api-key').value;
    const base = $('mimo-api-base').value;
    if (!key) { $('mimo-test-status').textContent = '❌ 请输入Key'; return; }
    $('mimo-test-status').textContent = '测试中...';
    const result = await tauriInvoke('test_mimo_connection', { apiKey: key, apiBase: base });
    if (result) {
      $('mimo-test-status').textContent = '✅ 连接成功';
      $('mimo-test-status').className = 'status-text success';
    } else {
      $('mimo-test-status').textContent = '❌ 连接失败';
      $('mimo-test-status').className = 'status-text danger';
    }
  });

  let expandSourceEl = null;
  document.querySelectorAll('.btn-expand').forEach(btn => {
    btn.addEventListener('click', () => {
      const targetId = btn.dataset.target;
      expandSourceEl = document.getElementById(targetId) || btn.parentElement.querySelector('.form-input');
      if (!expandSourceEl) return;
      $('expand-modal-title').textContent = btn.parentElement.previousElementSibling?.textContent || '编辑';
      $('expand-modal-text').value = expandSourceEl.value || '';
      $('expand-modal').classList.add('visible');
    });
  });
  $('btn-close-expand').addEventListener('click', () => {
    $('expand-modal').classList.remove('visible');
  });
  $('btn-expand-confirm').addEventListener('click', () => {
    if (expandSourceEl) expandSourceEl.value = $('expand-modal-text').value;
    $('expand-modal').classList.remove('visible');
  });

  $('tts-engine').addEventListener('change', () => {
    const engine = $('tts-engine').value;
    const localRow = $('local-tts-url-row');
    if (localRow) localRow.style.display = (engine === 'fish_speech' || engine === 'f5_tts') ? 'flex' : 'none';
  });

  $('mimo-model').addEventListener('change', () => {
    updateMimoModelUI($('mimo-model').value);
  });
  $('director-toggle').addEventListener('change', updateDirectorUI);
  $('tmd-enable').addEventListener('change', () => {
    syncDirectorToggle('settings');
  });
  $('director-enable').addEventListener('change', () => {
    syncDirectorToggle('quick');
  });

  // TMD 双区域 — 事件绑定已移至 settings-ui.js bindTmdEvents()
  // (避免跨文件全局作用域引用问题)

  $('top-p').addEventListener('input', () => { $('top-p-label').textContent = ($('top-p').value / 100).toFixed(2); });
  $('temperature').addEventListener('input', () => { $('temp-label').textContent = ($('temperature').value / 100).toFixed(2); });
  $('rep-penalty').addEventListener('input', () => { $('rep-label').textContent = ($('rep-penalty').value / 10).toFixed(1); });
  $('btn-reset-params').addEventListener('click', () => {
    $('top-p').value = 95; $('top-p-label').textContent = '0.95';
    $('temperature').value = 70; $('temp-label').textContent = '0.70';
    $('rep-penalty').value = 12; $('rep-label').textContent = '1.2';
  });

  $('theme-select').addEventListener('change', () => {
    const theme = $('theme-select').value;
    document.body.classList.toggle('light', theme === 'light');
    document.body.classList.toggle('dark', theme !== 'light');
  });

  $('lang-select').addEventListener('change', () => {
    const lang = $('lang-select').value;
    setLanguage(lang);
    tauriInvoke('save_settings', { update: { language: lang } });
  });

  $('btn-save-settings').addEventListener('click', saveSettings);

  $('btn-browse-clone').addEventListener('click', async () => {
    const path = await tauriInvoke('browse_clone_audio');
    if (path) {
      $('mimo-clone-path').value = path;
    }
  });
}

// ============================================================
// Pipeline Stats Display
// ============================================================
function updateEmotionStatus() {
  const el = $('emotion-status');
  if (!el) return;
  const model = $('emotion-model')?.value || 'none';
  if (model === 'none') {
    el.textContent = '未启用';
    el.className = 'status-text';
  } else {
    const asrLoaded = $('asr-status')?.classList.contains('success');
    if (asrLoaded) {
      el.textContent = `✅ ${model}`;
      el.className = 'status-text success';
    } else {
      el.textContent = `❌ ${model} (需先加载ASR)`;
      el.className = 'status-text danger';
    }
  }
}

async function updateStatusBar(asrOk) {
  const engines = await tauriInvoke('get_tts_engines');
  const online = engines ? engines.filter(e => e.healthy) : [];
  $('status-line').textContent = `ASR: ${asrOk ? '✅' : '❌'} | LLM: Connected | TTS: ${online.length > 0 ? online.map(e => e.name).join(', ') : 'Offline'}`;
}

async function refreshPipelineStats() {
  const stats = await tauriInvoke('get_pipeline_stats');
  if (!stats) return;

  const avgAsr = stats.asrCount > 0 ? Math.round(stats.totalAsrMs / stats.asrCount) : 0;
  const avgTranslate = stats.translateCount > 0 ? Math.round(stats.totalTranslateMs / stats.translateCount) : 0;
  const avgTts = stats.ttsCount > 0 ? Math.round(stats.totalTtsMs / stats.ttsCount) : 0;

  const el = $('status-line');
  if (el) {
    const base = el.textContent.split('📊')[0].trim();
    el.textContent = `${base} 📊 ASR:${stats.asrCount}(${avgAsr}ms) 翻译:${stats.translateCount}(${avgTranslate}ms) TTS:${stats.ttsCount}(${avgTts}ms)`;
  }
}

// ============================================================
// JS PTT 热键兜底 — 当 WebView2 获得焦点时，全局 Hook 可能不触发
// 此时由前端键盘事件直接处理 T 键的按下/释放
// ============================================================
function isEditableFocused() {
  const el = document.activeElement;
  if (!el) return false;
  const tag = el.tagName;
  if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return true;
  if (el.isContentEditable) return true;
  return false;
}

function initPttJsFallback() {
  const normalizeKey = (key) => {
    if (!key) return '';
    const lower = key.toLowerCase();
    if (lower === ' ') return 'space';
    if (lower === 'control') return 'ctrl';
    if (lower === 'escape') return 'escape';
    return lower;
  };

  const modifierActive = (e, mod) => {
    if (!mod || mod === 'none') return true;
    if (mod === 'ctrl') return e.ctrlKey;
    if (mod === 'shift') return e.shiftKey;
    if (mod === 'alt') return e.altKey;
    return true;
  };

  const configuredKeyboardMatches = (e) => {
    const key = normalizeKey(e.key);
    const hk1 = normalizeKey($('hotkey1-key')?.value || '');
    const hk1Mod = $('hotkey1-modifier')?.value || 'none';
    const hk2 = normalizeKey($('ptt-key2')?.value || '');
    const hit1 = hk1 && hk1 !== 'none' && hk1 !== 'middle_mouse' && key === hk1 && modifierActive(e, hk1Mod);
    const hit2 = hk2 && hk2 !== 'none' && hk2 !== 'middle_mouse' && key === hk2;
    return hit1 || hit2;
  };

  document.addEventListener('keydown', (e) => {
    if (!$('ptt-enable')?.checked || isEditableFocused() || e.repeat) return;
    if (configuredKeyboardMatches(e)) {
      _lastJsPttDown = Date.now();
      log(`[热键] ${normalizeKey(e.key)}按下 (JS兜底)`);
      beginPtt(`js:key:${normalizeKey(e.key)}`);
    }
  });

  document.addEventListener('keyup', (e) => {
    if (!$('ptt-enable')?.checked || isEditableFocused()) return;
    if (configuredKeyboardMatches(e)) {
      _lastJsPttUp = Date.now();
      log(`[热键] ${normalizeKey(e.key)}释放 (JS兜底)`);
      endPtt(`js:key:${normalizeKey(e.key)}`);
    }
  });

  document.addEventListener('mousedown', (e) => {
    if (!$('ptt-enable')?.checked || isEditableFocused()) return;
    const mouseKey = e.button === 1 ? 'middle_mouse' : e.button === 0 ? 'mouse_left' : e.button === 2 ? 'mouse_right' : '';
    if (!mouseKey) return;
    const hk1 = $('hotkey1-key')?.value || '';
    const hk2 = $('ptt-key2')?.value || '';
    if (hk1 === mouseKey || hk2 === mouseKey) {
      _lastJsPttDown = Date.now();
      log(`[热键] ${mouseKey}按下 (JS兜底)`);
      beginPtt(`js:mouse:${mouseKey}`);
    }
  });

  document.addEventListener('mouseup', (e) => {
    if (!$('ptt-enable')?.checked || isEditableFocused()) return;
    const mouseKey = e.button === 1 ? 'middle_mouse' : e.button === 0 ? 'mouse_left' : e.button === 2 ? 'mouse_right' : '';
    if (!mouseKey) return;
    const hk1 = $('hotkey1-key')?.value || '';
    const hk2 = $('ptt-key2')?.value || '';
    if (hk1 === mouseKey || hk2 === mouseKey) {
      _lastJsPttUp = Date.now();
      log(`[热键] ${mouseKey}释放 (JS兜底)`);
      endPtt(`js:mouse:${mouseKey}`);
    }
  });

  log('[热键] JS动态兜底已启用 ✓');
}

// ============================================================
// App Initialization
// ============================================================
async function init() {
  log('🎤 AI Voice Console (Tauri v2) starting...');

  try { initWindowControls(); } catch (e) { log(`[初始化] 窗口控制失败: ${e}`); }
  try { initDrawer(); } catch (e) { log(`[初始化] 抽屉初始化失败: ${e}`); }
  try { initLogPanel(); } catch (e) { log(`[初始化] 日志面板失败: ${e}`); }
  try { initTtsModal(); } catch (e) { log(`[初始化] TTS弹窗失败: ${e}`); }
  try { initWaveBars(); } catch (e) { log(`[初始化] 波形动画失败: ${e}`); }
  try { initAsrOptions(); } catch (e) { log(`[初始化] ASR选项失败: ${e}`); }
  try { initEmotionOptions(); } catch (e) { log(`[初始化] 情绪选项失败: ${e}`); }
  try { initEventListeners(); } catch (e) { log(`[初始化] 事件监听失败: ${e}`); }
  try { initPttJsFallback(); } catch (e) { log(`[初始化] PTT兜底失败: ${e}`); }
  try { bindUIEvents(); } catch (e) { log(`[初始化] UI绑定失败: ${e}`); }

  try { await initSettings(); } catch (e) { log(`[初始化] 设置加载失败: ${e}`); }

  const savedLang = $('lang-select')?.value || 'zh';
  setLanguage(savedLang);
  try { await initDevices(); } catch (e) { log(`[初始化] 设备加载失败: ${e}`); }
  try { await initTtsEngines(); } catch (e) { log(`[初始化] TTS引擎加载失败: ${e}`); }
  try { await loadTmdScenarios(); } catch (e) { log(`[初始化] TMD场景加载失败: ${e}`); }
  try { await initDirector(); } catch (e) { log(`[初始化] 导演模式加载失败: ${e}`); }

  let asrLoaded = false;
  const asrStatus = await tauriInvoke('get_asr_status');
  if (asrStatus && asrStatus.loaded) {
    asrLoaded = true;
    _asrLoaded = true;
    $('asr-status').textContent = `✅ ${asrStatus.engine}`;
    $('asr-status').className = 'status-text success';
  } else {
    _asrLoaded = false;
    log('[ASR] 将在后台自动加载 ASR 引擎...');
    $('asr-status').textContent = '⏳ 加载中...';
    $('asr-status').className = 'status-text';
    loadAsrInBackground('auto');
  }
  updateEmotionStatus();

  const pipelineStatus = await tauriInvoke('get_pipeline_status');
  if (pipelineStatus && pipelineStatus.running) {
    $('pipeline-badge').textContent = 'LIVE';
    $('pipeline-badge').className = 'badge badge-running';
  }

  const engines = await tauriInvoke('get_tts_engines');
  if (engines) {
    const online = engines.filter(e => e.healthy);
    $('status-line').textContent = `ASR: ${asrLoaded ? '✅' : '⏳'} | LLM: Connected | TTS: ${online.length > 0 ? online.map(e => e.name).join(', ') : 'Offline'}`;
  }

  setState(States.IDLE);
  log('初始化完成 ✓');

  if (typeof syncHotkeyToBackend === 'function') {
    syncHotkeyToBackend();
  }

  setInterval(refreshPipelineStats, STATS_REFRESH_MS);
  refreshPipelineStats();
}

document.addEventListener('DOMContentLoaded', init);
