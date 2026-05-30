// ============================================================
// settings-ui.js — 设置加载 / 保存 / UI 绑定
// ============================================================

// --- Load & Apply Settings ---

async function initSettings() {
  const settings = await tauriInvoke('get_settings');
  if (!settings) { log('[设置] 使用默认配置'); return; }
  cachedSettings = settings;

  // Volume & Speed
  $('volume').value = Math.round(settings.volume * 100);
  $('volume-label').value = Math.round(settings.volume * 100);
  $('playback-speed').value = Math.round(settings.playbackSpeed * 100);
  $('playback-speed-label').value = settings.playbackSpeed.toFixed(2);

  // Checkboxes
  $('bypass').checked = settings.bypassMode;
  $('auto-save').checked = settings.autoSaveAudio;
  $('ptt-enable').checked = settings.pttEnabled;
  $('translate-enable').checked = settings.translateEnabled;
  $('vad-enable').checked = settings.vadFilter;
  $('director-toggle').checked = settings.mimoDirectorEnabled;
  $('optimize-text').checked = settings.mimoOptimizeText;
  $('director-enable').checked = settings.textModelDirectorEnabled;
  $('tmd-enable').checked = settings.textModelDirectorEnabled;
  $('sexy-afterglow').checked = settings.sexyAfterglow;

  // Selects & inputs
  $('translate-lang').value = settings.translateTargetLang || 'Korean';
  $('deepseek-model').value = settings.deepseekModel || 'deepseek-v4-flash';
  $('deepseek-api-base').value = settings.deepseekApiBase || 'https://api.deepseek.com/v1';
  $('deepseek-api-key').value = settings.deepseekApiKey || '';
  $('mimo-api-base').value = settings.mimoApiBase || 'https://api.xiaomimimo.com/v1';
  $('mimo-api-key').value = settings.mimoApiKey || '';
  $('mimo-model').value = settings.mimoModel || 'mimo-v2.5-tts-voicedesign';
  $('mimo-style-prompt').value = settings.mimoStylePrompt || '';
  $('mimo-voice-design').value = settings.mimoVoiceDesign || '';
  $('mimo-clone-path').value = settings.mimoCloneAudioPath || '';
  $('tts-api-url').value = settings.ttsApiUrl || 'http://127.0.0.1:18084';
  $('asr-engine').value = settings.asrEngine || 'qwen3_asr';
  $('emotion-model').value = settings.emotionModel || 'none';
  $('director-character').value = settings.mimoCharacter || '';
  $('director-scene').value = settings.mimoScene || '';
  $('director-direction').value = settings.mimoDirection || '';

  // TTS params
  $('top-p').value = Math.round(settings.ttsTopP * 100);
  $('top-p-label').textContent = settings.ttsTopP.toFixed(2);
  $('temperature').value = Math.round(settings.ttsTemperature * 100);
  $('temp-label').textContent = settings.ttsTemperature.toFixed(2);
  $('rep-penalty').value = Math.round(settings.ttsRepetitionPenalty * 10);
  $('rep-label').textContent = settings.ttsRepetitionPenalty.toFixed(1);

  // VAD
  $('vad-silence').value = settings.vadMinSilence || 600;
  $('vad-pad').value = settings.vadSpeechPad || 400;

  // Theme & language
  const theme = settings.theme || 'dark';
  $('theme-select').value = theme;
  document.body.classList.toggle('light', theme === 'light');
  document.body.classList.toggle('dark', theme !== 'light');
  $('lang-select').value = settings.language || 'zh';

  // PTT keys
  const hk1Key = settings.pttKey1 ?? '';
  const hk1Mod = settings.hotkey1Modifier ?? 'none';
  if ($('hotkey1-key')) $('hotkey1-key').value = hk1Key;
  if ($('hotkey1-modifier')) $('hotkey1-modifier').value = hk1Mod;
  $('ptt-key2').value = settings.pttKey2 ?? 'none';

  // PTT status
  const pttStatus = $('ptt-status');
  if (pttStatus) {
    if (settings.pttEnabled) {
      pttStatus.textContent = '● 热键已启用';
      pttStatus.className = 'status-text success';
    } else {
      pttStatus.textContent = '○ 热键未启用';
      pttStatus.className = 'status-text stopped';
    }
  }

  // WASAPI Loopback
  const loopbackCb = $('wasapi-loopback');
  if (loopbackCb) loopbackCb.checked = settings.wasapiLoopback || false;
  if (settings.speakerDeviceId != null && $('speaker-device')) {
    $('speaker-device').value = settings.speakerDeviceId;
  }

  // Linked UI
  updateMimoModelUI(settings.mimoModel || 'mimo-v2.5-tts-voicedesign');
  updateDirectorUI();
  updateTmdUI();
  log('[设置] 配置已加载 ✓');
}

// --- Audio Devices ---

let _allInputDevices = [];
let _allOutputDevices = [];

async function initDevices() {
  const devices = await tauriInvoke('list_audio_devices');
  if (!devices) { log('[设备] 无设备数据'); return; }

  _allInputDevices = devices.input || [];
  _allOutputDevices = devices.output || [];

  const hostSel = $('host-api');
  hostSel.replaceChildren();
  const hostApis = devices.hostApis || [];
  if (hostApis.length === 0) {
    ['WASAPI', 'MME', 'Windows DirectSound', 'ASIO'].forEach(name => {
      const opt = document.createElement('option');
      opt.value = name; opt.textContent = name;
      hostSel.appendChild(opt);
    });
  } else {
    hostApis.forEach(h => {
      const opt = document.createElement('option');
      opt.value = h.name; opt.textContent = h.name;
      hostSel.appendChild(opt);
    });
  }

  populateDeviceLists();

  if (cachedSettings) {
    if (cachedSettings.hostApi) $('host-api').value = cachedSettings.hostApi;
    populateDeviceLists();
    if (cachedSettings.micDeviceId != null) $('mic-device').value = cachedSettings.micDeviceId;
    if (cachedSettings.outputDeviceId != null) $('output-device').value = cachedSettings.outputDeviceId;
    if (cachedSettings.monitorDeviceId != null) {
      $('monitor-device').value = cachedSettings.monitorDeviceId;
    } else {
      $('monitor-device').value = '';
    }
  }
  log('[设备] 音频设备已加载 ✓');
}

function populateDeviceLists() {
  const selectedApi = $('host-api')?.value || 'Windows WASAPI';
  const apiName = selectedApi.startsWith('Windows') ? selectedApi : `Windows ${selectedApi}`;

  const micSel = $('mic-device');
  micSel.replaceChildren();
  const filteredInput = _allInputDevices.filter(d => d.api === apiName || d.api === selectedApi);
  const inputList = filteredInput.length > 0 ? filteredInput : _allInputDevices;
  inputList.forEach(d => {
    const opt = document.createElement('option');
    opt.value = d.id; opt.textContent = `${d.name} (${d.channels}ch)`;
    opt.dataset.name = d.name;
    micSel.appendChild(opt);
  });

  const outSel = $('output-device');
  const monSel = $('monitor-device');
  const speakerSel = $('speaker-device');
  outSel.replaceChildren(); monSel.replaceChildren();
  if (speakerSel) speakerSel.replaceChildren();
  const monNoneOpt = document.createElement('option');
  monNoneOpt.value = ''; monNoneOpt.textContent = '不试听（TTS仅输出到输出设备）'; monNoneOpt.dataset.name = '';
  monSel.appendChild(monNoneOpt);
  const filteredOutput = _allOutputDevices.filter(d => d.api === apiName || d.api === selectedApi);
  const outputList = filteredOutput.length > 0 ? filteredOutput : _allOutputDevices;
  outputList.forEach(d => {
    const opt = document.createElement('option');
    opt.value = d.id; opt.textContent = `${d.name} (${d.channels}ch)`;
    opt.dataset.name = d.name;
    outSel.appendChild(opt);
    const opt2 = opt.cloneNode(true);
    monSel.appendChild(opt2);
    if (speakerSel) speakerSel.appendChild(opt.cloneNode(true));
  });
}

// --- TTS Engines ---

async function initTtsEngines() {
  const engines = await tauriInvoke('get_tts_engines');
  if (!engines) return;
  const sel = $('tts-engine');
  sel.replaceChildren();
  engines.forEach(eng => {
    const opt = document.createElement('option');
    opt.value = eng.id;
    opt.textContent = `${eng.name} ${eng.healthy ? '✅' : '❌'}`;
    sel.appendChild(opt);
  });
  if (cachedSettings && cachedSettings.ttsEngine) sel.value = cachedSettings.ttsEngine;
  log('[TTS] 引擎列表已加载 ✓');
}

// --- ASR & Emotion Options ---

function initAsrOptions() {
  const sel = $('asr-engine');
  sel.replaceChildren();
  [
    { value: 'qwen3_asr', text: 'Qwen3-ASR-1.7B (最准中文)' },
    { value: 'sensevoice', text: 'SenseVoice-Small (支持情绪)' },
    { value: 'faster_whisper', text: 'Faster-Whisper (速度快)' },
    { value: 'paraformer', text: 'Paraformer-Large (中文最准)' },
  ].forEach(o => {
    const opt = document.createElement('option');
    opt.value = o.value; opt.textContent = o.text;
    sel.appendChild(opt);
  });
}

function initEmotionOptions() {
  const sel = $('emotion-model');
  sel.replaceChildren();
  [
    { value: 'none', text: '无 (不检测情绪)' },
    { value: 'sensevoice', text: 'SenseVoice-Small (情绪检测)' },
  ].forEach(o => {
    const opt = document.createElement('option');
    opt.value = o.value; opt.textContent = o.text;
    sel.appendChild(opt);
  });
}

// --- MiMo & Director UI Helpers ---

function updateMimoModelUI(model) {
  const vdRow = $('vd-row');
  const cloneRow = $('clone-row');
  if (vdRow) vdRow.style.display = model === 'mimo-v2.5-tts-voicedesign' ? 'flex' : 'none';
  if (cloneRow) cloneRow.style.display = model === 'mimo-v2.5-tts-voiceclone' ? 'flex' : 'none';
}

function updateDirectorUI() {
  const fields = $('director-fields');
  const toggle = $('director-toggle');
  if (fields && toggle) fields.classList.toggle('hidden', !toggle.checked);
}

function updateTmdUI() {
  const section = $('tmd-section');
  const toggle = $('tmd-enable');
  if (section && toggle) section.classList.toggle('hidden', !toggle.checked);
}

function syncDirectorToggle(source) {
  const directorEnable = $('director-enable');
  const tmdEnable = $('tmd-enable');
  if (source === 'quick' && directorEnable && tmdEnable) {
    tmdEnable.checked = directorEnable.checked;
  } else if (source === 'settings' && directorEnable && tmdEnable) {
    directorEnable.checked = tmdEnable.checked;
  }
  updateTmdUI();
}

// --- TMD Scenarios (场景数据由 Rust 端管理) ---

const directorLists = {
  tmd: {
    activeKey: 'textModelDirectorScenarios',
    standbyKey: 'tmdStandbyScenarios',
    active: [],
    standby: [],
    selectedZone: 'active',
    selectedIndex: -1,
    ids: {
      activeList: 'tmd-active-list',
      standbyList: 'tmd-standby-list',
      activeCount: 'tmd-active-count',
      standbyCount: 'tmd-standby-count',
    },
    fields: {
      name: 'tmd-name',
      trigger: 'tmd-trigger',
      prompt: 'tmd-prompt',
      character: 'tmd-character',
      scene: 'tmd-scene',
      direction: 'tmd-direction',
    },
    emptyItem: () => ({ name: '新情景', trigger: '', prompt: '', character: '', scene: '', direction: '' }),
  },
  ts: {
    activeKey: 'ttsStandards',
    standbyKey: 'ttsStandbyStandards',
    active: [],
    standby: [],
    selectedZone: 'active',
    selectedIndex: -1,
    ids: {
      activeList: 'ts-active-list',
      standbyList: 'ts-standby-list',
      activeCount: 'ts-active-count',
      standbyCount: 'ts-standby-count',
    },
    fields: {
      name: 'ts-name',
      voiceDesignPrompt: 'ts-voice-design',
      audioTagControl: 'ts-tag-control',
      styleControl: 'ts-style-control',
    },
    emptyItem: () => ({ name: '新标准', voiceDesignPrompt: '', audioTagControl: '', styleControl: '' }),
  },
  lg: {
    activeKey: 'llmStyleGuides',
    standbyKey: 'llmStandbyStyleGuides',
    active: [],
    standby: [],
    selectedZone: 'active',
    selectedIndex: -1,
    ids: {
      activeList: 'lg-active-list',
      standbyList: 'lg-standby-list',
      activeCount: 'lg-active-count',
      standbyCount: 'lg-standby-count',
    },
    fields: {
      name: 'lg-name',
      content: 'lg-content',
    },
    emptyItem: () => ({ name: '新指导', content: '' }),
  },
};

let _directorEventsBound = false;

function cloneDirectorItem(item) {
  return JSON.parse(JSON.stringify(item || {}));
}

function getDirectorConfig(prefix) {
  return directorLists[prefix];
}

function getSelectedDirectorItem(prefix) {
  const cfg = getDirectorConfig(prefix);
  if (!cfg) return null;
  return cfg[cfg.selectedZone]?.[cfg.selectedIndex] || null;
}

function readDirectorForm(prefix) {
  const cfg = getDirectorConfig(prefix);
  const item = {};
  Object.entries(cfg.fields).forEach(([key, id]) => {
    item[key] = $(id)?.value || '';
  });
  return item;
}

function writeDirectorForm(prefix, item = {}) {
  const cfg = getDirectorConfig(prefix);
  Object.entries(cfg.fields).forEach(([key, id]) => {
    const el = $(id);
    if (el) el.value = item[key] || '';
  });
}

async function persistDirectorLists(prefix, quiet = false) {
  const cfg = getDirectorConfig(prefix);
  if (!cfg) return;
  const update = {
    [cfg.activeKey]: cfg.active,
    [cfg.standbyKey]: cfg.standby,
  };
  await tauriInvoke('save_settings', { update });
  if (cachedSettings) {
    cachedSettings[cfg.activeKey] = cloneDirectorItem(cfg.active);
    cachedSettings[cfg.standbyKey] = cloneDirectorItem(cfg.standby);
  }
  if (!quiet) log('[导演] 列表已保存 ✓');
}

function selectDirectorItem(prefix, zone, index) {
  const cfg = getDirectorConfig(prefix);
  if (!cfg) return;
  cfg.selectedZone = zone;
  cfg.selectedIndex = index;
  writeDirectorForm(prefix, cfg[zone]?.[index] || cfg.emptyItem());
  renderDirectorList(prefix);
}

function renderDirectorList(prefix) {
  const cfg = getDirectorConfig(prefix);
  if (!cfg) return;

  const renderZone = (zone, listId) => {
    const list = $(listId);
    if (!list) return;
    list.replaceChildren();
    cfg[zone].forEach((item, index) => {
      const li = document.createElement('li');
      li.className = 'tmd-item';
      li.draggable = true;
      li.dataset.prefix = prefix;
      li.dataset.zone = zone;
      li.dataset.index = String(index);
      li.title = item.name || '(未命名)';
      li.textContent = item.name || '(未命名)';
      if (cfg.selectedZone === zone && cfg.selectedIndex === index) li.classList.add('selected');

      li.addEventListener('click', () => selectDirectorItem(prefix, zone, index));
      li.addEventListener('dragstart', (e) => {
        e.dataTransfer.setData('text/plain', JSON.stringify({ prefix, zone, index }));
        e.dataTransfer.effectAllowed = 'move';
        li.classList.add('tmd-chosen');
      });
      li.addEventListener('dragend', () => li.classList.remove('tmd-chosen'));
      list.appendChild(li);
    });
  };

  renderZone('active', cfg.ids.activeList);
  renderZone('standby', cfg.ids.standbyList);

  if ($(cfg.ids.activeCount)) $(cfg.ids.activeCount).textContent = `${cfg.active.length}个`;
  if ($(cfg.ids.standbyCount)) $(cfg.ids.standbyCount).textContent = `${cfg.standby.length}个`;
}

async function moveDirectorItem(prefix, targetZone) {
  const cfg = getDirectorConfig(prefix);
  if (!cfg) return;
  const sourceZone = targetZone === 'active' ? 'standby' : 'active';
  const index = cfg.selectedZone === sourceZone ? cfg.selectedIndex : -1;
  if (index < 0 || !cfg[sourceZone][index]) return;
  const [item] = cfg[sourceZone].splice(index, 1);
  cfg[targetZone].push(item);
  cfg.selectedZone = targetZone;
  cfg.selectedIndex = cfg[targetZone].length - 1;
  writeDirectorForm(prefix, item);
  renderDirectorList(prefix);
  await persistDirectorLists(prefix);
}

async function reorderDirectorItem(prefix, delta) {
  const cfg = getDirectorConfig(prefix);
  if (!cfg) return;
  const list = cfg[cfg.selectedZone];
  const index = cfg.selectedIndex;
  const nextIndex = index + delta;
  if (index < 0 || nextIndex < 0 || nextIndex >= list.length) return;
  [list[index], list[nextIndex]] = [list[nextIndex], list[index]];
  cfg.selectedIndex = nextIndex;
  renderDirectorList(prefix);
  await persistDirectorLists(prefix);
}

async function addDirectorItem(prefix) {
  const cfg = getDirectorConfig(prefix);
  if (!cfg) return;
  const item = { ...cfg.emptyItem(), ...readDirectorForm(prefix) };
  if (!item.name?.trim()) item.name = cfg.emptyItem().name;
  cfg.active.push(item);
  cfg.selectedZone = 'active';
  cfg.selectedIndex = cfg.active.length - 1;
  writeDirectorForm(prefix, item);
  renderDirectorList(prefix);
  await persistDirectorLists(prefix);
}

async function copyDirectorItem(prefix) {
  const cfg = getDirectorConfig(prefix);
  const selected = getSelectedDirectorItem(prefix);
  if (!cfg || !selected) return;
  const item = cloneDirectorItem(selected);
  item.name = `${item.name || cfg.emptyItem().name} 副本`;
  cfg[cfg.selectedZone].splice(cfg.selectedIndex + 1, 0, item);
  cfg.selectedIndex += 1;
  writeDirectorForm(prefix, item);
  renderDirectorList(prefix);
  await persistDirectorLists(prefix);
}

async function deleteDirectorItem(prefix) {
  const cfg = getDirectorConfig(prefix);
  if (!cfg || cfg.selectedIndex < 0) return;
  const list = cfg[cfg.selectedZone];
  if (!list[cfg.selectedIndex]) return;
  list.splice(cfg.selectedIndex, 1);
  cfg.selectedIndex = Math.min(cfg.selectedIndex, list.length - 1);
  writeDirectorForm(prefix, list[cfg.selectedIndex] || cfg.emptyItem());
  renderDirectorList(prefix);
  await persistDirectorLists(prefix);
}

async function saveDirectorItem(prefix) {
  const cfg = getDirectorConfig(prefix);
  if (!cfg) return;
  if (cfg.selectedIndex < 0 || !cfg[cfg.selectedZone][cfg.selectedIndex]) {
    await addDirectorItem(prefix);
    return;
  }
  const item = readDirectorForm(prefix);
  if (!item.name?.trim()) item.name = cfg.emptyItem().name;
  cfg[cfg.selectedZone][cfg.selectedIndex] = item;
  renderDirectorList(prefix);
  await persistDirectorLists(prefix);
}

function handleDirectorAction(action) {
  const map = {
    tmdAddScenario: () => addDirectorItem('tmd'),
    tmdCopyScenario: () => copyDirectorItem('tmd'),
    tmdDelScenario: () => deleteDirectorItem('tmd'),
    tmdSaveScenario: () => saveDirectorItem('tmd'),
    tmdMoveToStandby: () => moveDirectorItem('tmd', 'standby'),
    tmdMoveToActive: () => moveDirectorItem('tmd', 'active'),
    tmdMoveUp: () => reorderDirectorItem('tmd', -1),
    tmdMoveDown: () => reorderDirectorItem('tmd', 1),
    tsAddStandard: () => addDirectorItem('ts'),
    tsCopyStandard: () => copyDirectorItem('ts'),
    tsDelStandard: () => deleteDirectorItem('ts'),
    tsSaveStandard: () => saveDirectorItem('ts'),
    tsMoveToStandby: () => moveDirectorItem('ts', 'standby'),
    tsMoveToActive: () => moveDirectorItem('ts', 'active'),
    tsMoveUp: () => reorderDirectorItem('ts', -1),
    tsMoveDown: () => reorderDirectorItem('ts', 1),
    lgAddGuide: () => addDirectorItem('lg'),
    lgCopyGuide: () => copyDirectorItem('lg'),
    lgDelGuide: () => deleteDirectorItem('lg'),
    lgSaveGuide: () => saveDirectorItem('lg'),
    lgMoveToStandby: () => moveDirectorItem('lg', 'standby'),
    lgMoveToActive: () => moveDirectorItem('lg', 'active'),
    lgMoveUp: () => reorderDirectorItem('lg', -1),
    lgMoveDown: () => reorderDirectorItem('lg', 1),
  };
  return map[action]?.();
}

function mergeDirectorContext() {
  const section = (title, items, format) => {
    if (!items.length) return '';
    return `## ${title}\n\n${items.map(format).join('\n\n')}`;
  };
  const tmd = section('情景管理', directorLists.tmd.active, (s, i) =>
    `${i + 1}. ${s.name}\n触发条件：${s.trigger || ''}\n音色描述：${s.prompt || ''}\n角色：${s.character || ''}\n场景：${s.scene || ''}\n指导：${s.direction || ''}`
  );
  const ts = section('TTS 识别标准', directorLists.ts.active, (s, i) =>
    `${i + 1}. ${s.name}\n音色描述：${s.voiceDesignPrompt || ''}\n标签控制：${s.audioTagControl || ''}\n风格控制：${s.styleControl || ''}`
  );
  const lg = section('LLM 风格识别指导', directorLists.lg.active, (s, i) =>
    `${i + 1}. ${s.name}\n${s.content || ''}`
  );
  return [tmd, ts, lg].filter(Boolean).join('\n\n');
}

function bindDirectorDropTargets() {
  document.querySelectorAll('.tmd-drag-list').forEach(list => {
    list.addEventListener('dragover', (e) => {
      e.preventDefault();
      e.dataTransfer.dropEffect = 'move';
    });
    list.addEventListener('drop', async (e) => {
      e.preventDefault();
      const targetPrefix = list.id.split('-')[0];
      const targetZone = list.dataset.zone;
      const data = JSON.parse(e.dataTransfer.getData('text/plain') || '{}');
      if (data.prefix !== targetPrefix) return;
      const cfg = getDirectorConfig(data.prefix);
      const source = cfg?.[data.zone];
      const target = cfg?.[targetZone];
      if (!source || !target || !source[data.index]) return;
      const [item] = source.splice(data.index, 1);
      target.push(item);
      cfg.selectedZone = targetZone;
      cfg.selectedIndex = target.length - 1;
      writeDirectorForm(data.prefix, item);
      renderDirectorList(data.prefix);
      await persistDirectorLists(data.prefix);
    });
  });
}

function bindTmdEvents() {
  if (_directorEventsBound) return;
  _directorEventsBound = true;

  document.querySelectorAll('[data-director-action]').forEach(btn => {
    btn.addEventListener('click', () => handleDirectorAction(btn.dataset.directorAction));
  });
  bindDirectorDropTargets();

  $('btn-merge-context')?.addEventListener('click', () => {
    $('merged-context').value = mergeDirectorContext();
    log('[导演] 已合并启用配置');
  });
  $('btn-save-merged')?.addEventListener('click', async () => {
    await tauriInvoke('save_settings', { update: { mergedContext: $('merged-context').value } });
    log('[导演] 上下文已保存 ✓');
  });
  $('btn-generate-prompt')?.addEventListener('click', async () => {
    const prompt = $('merged-context').value || mergeDirectorContext();
    if (!prompt.trim()) return;
    $('btn-generate-spinner')?.classList.remove('hidden');
    const generated = await tauriInvoke('generate_director_prompt', { prompt });
    $('btn-generate-spinner')?.classList.add('hidden');
    if (generated) $('new-prompt-preview').value = generated;
  });
  $('btn-save-generated')?.addEventListener('click', async () => {
    await tauriInvoke('save_settings', { update: { generatedPrompt: $('new-prompt-preview').value } });
    log('[导演] 生成结果已保存 ✓');
  });
  $('btn-preview-current')?.addEventListener('click', () => {
    $('system-prompt-editor').value = cachedSettings?.directorSystemPrompt || cachedSettings?.generatedPrompt || '';
  });
  $('btn-save-system-prompt')?.addEventListener('click', async () => {
    const directorSystemPrompt = $('system-prompt-editor').value;
    await tauriInvoke('save_settings', { update: { directorSystemPrompt } });
    if (cachedSettings) cachedSettings.directorSystemPrompt = directorSystemPrompt;
    log('[导演] System Prompt 已保存 ✓');
  });
}

async function loadTmdScenarios() {
  const settings = await tauriInvoke('get_settings');
  if (!settings) return;
  cachedSettings = settings;

  Object.values(directorLists).forEach(cfg => {
    cfg.active = cloneDirectorItem(settings[cfg.activeKey] || []);
    cfg.standby = cloneDirectorItem(settings[cfg.standbyKey] || []);
    cfg.selectedZone = cfg.active.length > 0 ? 'active' : 'standby';
    cfg.selectedIndex = cfg[cfg.selectedZone].length > 0 ? 0 : -1;
  });

  Object.keys(directorLists).forEach(prefix => {
    const selected = getSelectedDirectorItem(prefix) || directorLists[prefix].emptyItem();
    writeDirectorForm(prefix, selected);
    renderDirectorList(prefix);
  });

  if ($('merged-context')) $('merged-context').value = settings.mergedContext || '';
  if ($('new-prompt-preview')) $('new-prompt-preview').value = settings.generatedPrompt || '';
  if ($('system-prompt-editor')) $('system-prompt-editor').value = settings.directorSystemPrompt || '';

  bindTmdEvents();
  log('[导演] 情景/标准/指导已加载 ✓');
}

async function initDirector() {
  bindTmdEvents();
  await loadTmdScenarios();
}

// --- Save Settings ---

async function saveSettings() {
  const update = {
    volume: $('volume').value / 100,
    playbackSpeed: $('playback-speed').value / 100,
    bypassMode: $('bypass').checked,
    autoSaveAudio: $('auto-save').checked,
    pttEnabled: $('ptt-enable').checked,
    pttKey1: $('hotkey1-key')?.value ?? '',
    hotkey1Modifier: $('hotkey1-modifier')?.value ?? 'none',
    pttKey2: $('ptt-key2').value,
    asrEngine: $('asr-engine').value,
    ttsEngine: $('tts-engine').value,
    ttsTopP: $('top-p').value / 100,
    ttsTemperature: $('temperature').value / 100,
    ttsRepetitionPenalty: $('rep-penalty').value / 10,
    mimoModel: $('mimo-model').value,
    mimoStylePrompt: $('mimo-style-prompt').value,
    mimoVoiceDesign: $('mimo-voice-design').value,
    mimoCloneAudioPath: $('mimo-clone-path').value,
    mimoDirectorEnabled: $('director-toggle').checked,
    mimoOptimizeText: $('optimize-text').checked,
    mimoApiKey: $('mimo-api-key').value,
    mimoApiBase: $('mimo-api-base').value,
    mimoCharacter: $('director-character').value,
    mimoScene: $('director-scene').value,
    mimoDirection: $('director-direction').value,
    translateEnabled: $('translate-enable').checked,
    translateTargetLang: $('translate-lang').value,
    textModelDirectorEnabled: $('director-enable').checked,
    deepseekModel: $('deepseek-model').value,
    deepseekApiBase: $('deepseek-api-base').value,
    deepseekApiKey: $('deepseek-api-key').value,
    vadFilter: $('vad-enable').checked,
    vadMinSilence: parseInt($('vad-silence').value),
    vadSpeechPad: parseInt($('vad-pad').value),
    sexyAfterglow: $('sexy-afterglow').checked,
    language: $('lang-select').value,
    theme: $('theme-select').value,
    hostApi: $('host-api').value,
    ttsApiUrl: $('tts-api-url').value,
    wasapiLoopback: $('wasapi-loopback')?.checked || false,
  };
  const micDevice = $('mic-device').value;
  const outDevice = $('output-device').value;
  const monDevice = $('monitor-device')?.value;
  const speakerDevice = $('speaker-device')?.value;
  if (micDevice) update.micDeviceId = parseInt(micDevice);
  if (outDevice) update.outputDeviceId = parseInt(outDevice);
  if (monDevice) update.monitorDeviceId = parseInt(monDevice);
  if (speakerDevice) update.speakerDeviceId = parseInt(speakerDevice);
  await tauriInvoke('save_settings', { update });
  addMessage('msg-system', '✅ 设置已保存');
  log('[设置] 已保存 ✓');
}

// --- Exports ---
window.initSettings = initSettings;
window.initDevices = initDevices;
window.initTtsEngines = initTtsEngines;
window.initAsrOptions = initAsrOptions;
window.initEmotionOptions = initEmotionOptions;
window.updateMimoModelUI = updateMimoModelUI;
window.updateDirectorUI = updateDirectorUI;
window.updateTmdUI = updateTmdUI;
window.syncDirectorToggle = syncDirectorToggle;
window.loadTmdScenarios = loadTmdScenarios;
window.bindTmdEvents = bindTmdEvents;
window.initDirector = initDirector;
window.saveSettings = saveSettings;
