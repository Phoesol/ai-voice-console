// ============================================================
// state.js — 状态机 + 全局状态 + 状态守卫
// ============================================================

// --- State Machine ---
const States = { IDLE: 'idle', LISTENING: 'listening', THINKING: 'thinking', SPEAKING: 'speaking' };
let appState = States.IDLE;

/** 合法状态转换表：key=当前状态，value=允许直接转换到的状态集合 */
const VALID_TRANSITIONS = {
  [States.IDLE]:      [States.LISTENING, States.IDLE],
  [States.LISTENING]: [States.THINKING, States.IDLE],
  [States.THINKING]:  [States.SPEAKING, States.IDLE],
  [States.SPEAKING]:  [States.IDLE, States.LISTENING],
};

/**
 * 状态转换守卫：非法转换会被拒绝并 log 警告
 * 禁止的转换：SPEAKING→LISTENING（不经 IDLE）
 */
function setState(newState) {
  if (newState === appState) return;

  if (!VALID_TRANSITIONS[appState]?.includes(newState)) {
    log(`[状态机] 非法转换: ${appState} → ${newState} (已拒绝)`);
    return;
  }

  appState = newState;
  const mic = $('mic-btn');
  const label = $('mic-label');
  const waveform = $('waveform');
  const badge = $('pipeline-badge');

  mic.classList.remove('mic-idle', 'mic-listening', 'mic-thinking', 'mic-speaking');
  waveform.classList.remove('listening', 'thinking', 'speaking');
  label.classList.remove('mic-label-listening', 'mic-label-thinking', 'mic-label-speaking');
  badge.classList.remove('badge-idle', 'badge-running', 'badge-listening', 'badge-error');

  switch (newState) {
    case States.IDLE:
      mic.classList.add('mic-idle');
      waveform.classList.remove('active');
      label.textContent = t('btn_mic');
      badge.classList.add('badge-idle');
      badge.textContent = 'IDLE';
      break;
    case States.LISTENING:
      mic.classList.add('mic-listening');
      waveform.classList.add('active', 'listening');
      label.textContent = t('state_listening');
      label.classList.add('mic-label-listening');
      badge.classList.add('badge-listening');
      badge.textContent = 'REC';
      break;
    case States.THINKING:
      mic.classList.add('mic-thinking');
      waveform.classList.add('active', 'thinking');
      label.textContent = t('state_thinking');
      label.classList.add('mic-label-thinking');
      badge.classList.add('badge-running');
      badge.textContent = 'AI';
      break;
    case States.SPEAKING:
      mic.classList.add('mic-speaking');
      waveform.classList.add('active', 'speaking');
      label.textContent = t('state_speaking');
      label.classList.add('mic-label-speaking');
      badge.classList.add('badge-running');
      badge.textContent = 'TTS';
      break;
  }
}

// --- Cached Settings ---
let cachedSettings = null;

// --- Exports (for other modules) ---
// 在 Tauri 前端，所有 <script> 共享全局作用域，
// 直接赋值全局变量即可，无需挂在 window 上（同一 window 上下文）
// 注意：基础类型按值复制，所以 appState 用全局变量即可（所有脚本共享同一个全局作用域）
window.States = States;
window.setState = setState;
// appState 是 let 全局变量，其他模块直接访问 appState 变量（同一作用域）
// 为方便外部访问，提供 getter
window.getAppState = () => appState;
