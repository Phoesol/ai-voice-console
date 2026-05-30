<p align="center">
  <img src="docs/banner.png" alt="AI Voice Console Banner" width="720" />
</p>

<h1 align="center">🎙️ 赛博声带 — AI Voice Console</h1>

<p align="center">
  <b>实时跨语种 AI 语音重构管线</b><br/>
  ASR 语音识别 → LLM 翻译与情绪扩写 → TTS 拟真发声
</p>

<p align="center">
  <a href="./README.md">English</a> | <b>中文</b>
</p>

<p align="center">
  <a href="#-它是如何工作的">工作原理</a> •
  <a href="#-核心特性">核心特性</a> •
  <a href="#-快速开始">快速开始</a> •
  <a href="#-应用场景">应用场景</a> •
  <a href="#-致谢">致谢</a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Tauri-v2-blue?logo=tauri" alt="Tauri v2" />
  <img src="https://img.shields.io/badge/Rust-2021-orange?logo=rust" alt="Rust 2021" />
  <img src="https://img.shields.io/badge/License-MIT-green" alt="MIT License" />
  <img src="https://img.shields.io/badge/Platform-Windows-blue?logo=windows" alt="Windows" />
</p>

---

## 📖 这是什么？

**赛博声带（AI Voice Console）** 是一款基于 `ASR → LLM → TTS` 架构的系统级实时语音重构与输出工具。

传统变声器仅仅改变音频频率，而赛博声带则是**语义级别的重构**。它不仅能将你的母语实时翻译并转换为任意语种的语音，还能通过中间层的 LLM 脚本，对你的发言进行润色、扩写和情绪注入，最终输出高拟真的定制音色。

> 如果说同传字幕引擎帮你"听懂世界"，那么赛博声带就是帮你"向世界发声"的跨语种数字声带。两者结合，即可构建一套完整的跨语种、跨身份实时交互闭环。

---

## ⚙️ 它是如何工作的？

本软件彻底打破了"原声直出"的逻辑，采用物理级隔离的重构链路：

```
🎤 你说话（中文）                  🔊 输出（任意语言）
     │                                  ▲
     ▼                                  │
┌─────────┐    ┌─────────┐    ┌─────────┐
│  👂 听   │ ──▶│  🧠 想   │ ──▶│  👄 说   │
│ 本地 ASR │    │ 云端 LLM │    │ 云端 TTS │
│ Qwen3    │    │ DeepSeek │    │  MiMo    │
└─────────┘    └─────────┘    └─────────┘
  极速识别        翻译/扩写/       拟真发声
  支持方言        情绪注入         音频标签
```

- **👂 听（本地 ASR）**：极速识别你的母语语音（支持方言），在本地运行，隐私安全。
- **🧠 想（云端 LLM）**：这是核心。通过自定义 Prompt，模型不仅负责精准翻译，还能根据你设定的"人设"改写台词——例如将普通问候扩写为高情绪价值的闲聊，或将简单词汇转化为地道的战术黑话。
- **👄 说（云端 TTS）**：携带丰富的音频标签（语速极快、慵懒、喘息、颤音等），通过虚拟麦克风无延迟输出给其他玩家或观众。

---

## ✨ 核心特性

### 管线能力
| 功能 | 说明 |
|------|------|
| 🎤 本地 ASR | Qwen3-ASR 极速语音识别，支持方言，本地推理 |
| 🧠 LLM 导演 | 自定义 Prompt 驱动的翻译、改写、情绪注入 |
| 🔊 云端 TTS | 高拟真语音合成，支持丰富音频标签与情绪控制 |
| 🎭 场景导演 (TMD) | 智能多场景路由，自动切换"报点模式"和"闲聊模式" |
| 🎵 声音克隆 | 基于参考音频的声线克隆 |
| 🌍 跨语种翻译 | 实时翻译为任意目标语言 |

### 音频与系统
| 功能 | 说明 |
|------|------|
| 🎚️ 虚拟麦克风输出 | 合成音频直接路由到虚拟麦克风，可在任意应用中使用 |
| 🔑 按键说话 (PTT) | 全局热键支持，系统级键盘 Hook + JS 兜底双保险 |
| 🔄 WASAPI 内录 | 捕获系统音频进行实时处理 |
| 💾 自动保存 | TTS 输出音频自动录制 |

### 性能优化
| 技术 | 说明 |
|------|------|
| ⚡ Rust / Tauri v2 | 原生级性能，极低资源占用，榨干底层性能 |
| 🚀 零拷贝 IPC | 进程间音频传输采用标准管道流，彻底摒弃磁盘临时文件 |
| 🌐 TCP Keep-Alive | 全局单例 HTTP 客户端，免去频繁 TLS 握手开销 |
| 🔒 单实例锁 | 防止应用重复启动 |

### 界面
| 功能 | 说明 |
|------|------|
| 🌓 明暗主题 | 优雅的毛玻璃 UI，支持主题切换 |
| 🌍 国际化 | 中文 / 英文界面 |
| 📊 实时统计 | 管线延迟实时监控（ASR / LLM / TTS） |
| 💬 对话舞台 | 用户 / AI 消息气泡可视化对话历史 |

---

## 🏗️ 系统架构

```
┌──────────────────────────────────────────────────────────────┐
│                     Tauri v2 应用程序                         │
├───────────────────────┬──────────────────────────────────────┤
│    前端 (WebView)      │          Rust 后端                   │
│                       │                                      │
│  index.html           │  main.rs          ← 应用入口         │
│  main.js              │  ├─ audio/        ← WASAPI 音频      │
│  audio.js             │  │   ├─ capture      采集 & 输出     │
│  state.js             │  │   ├─ loopback     内录            │
│  settings-ui.js       │  │   ├─ output       播放            │
│  i18n.js              │  │   └─ resample     重采样          │
│  styles.css           │  ├─ commands/     ← IPC 命令处理      │
│                       │  │   ├─ asr / llm / tts              │
│                       │  │   ├─ hotkey / pipeline             │
│                       │  │   └─ recording / ...               │
│                       │  ├─ http/         ← HTTP 客户端       │
│                       │  │   ├─ asr_client                   │
│                       │  │   ├─ llm_client                   │
│                       │  │   └─ tts_client                   │
│                       │  ├─ engine/       ← ASR 服务管理      │
│                       │  └─ state/        ← 配置 & 状态       │
└───────────────────────┴──────────────────────────────────────┘
          │                           │
          ▼                           ▼
┌──────────────┐   ┌──────────────────────────────────┐
│  ASR 服务器   │   │           云端 API                 │
│  (Python)    │   │  ┌────────────┐ ┌──────────────┐  │
│  Qwen3-ASR   │   │  │ DeepSeek   │ │  MiMo TTS    │  │
│  Flask HTTP  │   │  │ LLM 大模型  │ │  语音合成     │  │
└──────────────┘   │  └────────────┘ └──────────────┘  │
                   └──────────────────────────────────┘
```

---

## 🚀 快速开始

### 环境要求

- **操作系统**：Windows 10/11 (x64)
- **Python**：3.12+（用于 ASR 服务器）
- **GPU**：NVIDIA GPU + CUDA（推荐，用于 ASR 推理加速）
- **虚拟音频**：[VB-CABLE](https://vb-audio.com/Cable/) 或类似虚拟音频设备

### 1. 克隆仓库

```bash
git clone https://github.com/Phoesol/ai-voice-console.git
cd ai-voice-console
```

### 2. 下载 ASR 模型

下载 [Qwen3-ASR-1.7B](https://huggingface.co/Qwen/Qwen3-ASR-1.7B) 模型，放置到：

```
data/checkpoints/Qwen3-ASR-1.7B/
```

### 3. 安装 Python 依赖

```bash
pip install torch flask numpy
pip install qwen-asr  # Qwen3 ASR 推理包
```

### 4. 获取 API 密钥

| 服务 | 用途 | 链接 |
|------|------|------|
| **DeepSeek** | LLM 文本处理（翻译/改写） | [platform.deepseek.com](https://platform.deepseek.com/) |
| **MiMo TTS** | 语音合成 | [xiaomimimo.com](https://api.xiaomimimo.com/) |

> 💡 DeepSeek V4 Flash 高强度使用一天几毛钱，几乎免费。MiMo TTS 目前也免费（截止发布时）。

### 5. 编译 & 运行

```bash
# 安装 Rust 和 Tauri CLI
cargo install tauri-cli

# 开发模式运行
cd src-tauri
cargo tauri dev
```

### 6. 首次配置

启动后，打开**设置面板**（⚙️），配置以下内容：

1. 填入 **DeepSeek API Key**
2. 填入 **MiMo TTS API Key**
3. 选择**麦克风**和**输出设备**（虚拟麦克风）
4. 设置 **PTT 热键**（默认：`T` 键）
5. 自定义你的**声音人设**和 **LLM 提示词**

> 📋 可参考 `config.example.json` 了解全部配置字段。

---

## 🎮 应用场景

### 🕹️ VRChat & 虚拟主播：跨次元多语种分身

**痛点**：拥有精美的 3D 皮套和面捕，却受限于个人的外语水平和真实声线。

**解决方案**：用母语自然交流，系统实时输出完美贴合虚拟人设的纯正外语（如日语萝莉音、英语御姐音）。配合同传字幕捕获，轻松实现零门槛的全球跨区整活。

---

### 🎯 外服游戏竞技：无障碍战术沟通

**痛点**：在美服、欧服、亚服游玩时（如 PUBG、DOTA），面临语言壁垒，遭遇嘴臭队友时只能吃哑巴亏。

**解决方案**：开启游戏模式。用中文快速报点，系统以连珠炮般的语速输出外语战术指令；面对恶意挑衅，用母语自然反击，LLM 会自动润色为地道的本地黑话，并用粗犷霸气的音色回敬对方。

---

### 💕 高情绪价值社交 & 跨区陪玩

**痛点**：语言和语气的表现力不足，难以跨越文化背景提供情绪价值。

**解决方案**：利用 LLM 的文本加工能力，用最普通的语气说话，软件则根据脚本自动输出带有特定尾音、情绪起伏的高情商回复，跨越语种接单，主打反差与沉浸感。

---

### 🎭 匿名直播：物理级防掉马

**痛点**：传统变声器容易因现实中的杂音、咳嗽或情绪激动导致破音"掉马"。

**解决方案**：由于是 `文本 → AI 发声` 的单向链路，真实麦克风的物理环境音被绝对隔离。现实中你大吼大叫或喝水咳嗽，观众听到的依然是吐字优雅、声线稳定的完美人设。

---

## 📁 项目结构

```
ai-voice-console/
├── src/                    # 前端（HTML/JS/CSS）
│   ├── index.html          # 主界面
│   ├── main.js             # 应用初始化 & 事件绑定
│   ├── audio.js            # 录音 & 播放
│   ├── state.js            # 状态机
│   ├── settings-ui.js      # 设置面板逻辑
│   ├── i18n.js             # 国际化
│   └── styles.css          # 样式（明暗主题）
├── src-tauri/              # Rust 后端（Tauri v2）
│   ├── src/
│   │   ├── main.rs         # 应用入口
│   │   ├── audio/          # WASAPI 音频采集 & 输出
│   │   ├── commands/       # Tauri IPC 命令处理器
│   │   ├── http/           # HTTP 客户端（ASR/LLM/TTS）
│   │   ├── engine/         # ASR 服务生命周期管理
│   │   └── state/          # 应用状态 & 设置管理
│   ├── Cargo.toml          # Rust 依赖
│   └── tauri.conf.json     # Tauri 配置
├── engines/                # 引擎脚本
│   ├── asr/                # ASR 引擎实现
│   └── tts/                # TTS 引擎实现
├── asr_server.py           # 独立 ASR HTTP 服务器（Flask）
├── config.example.json     # 配置模板
└── data/                   # 运行时数据（模型、日志等）
```

---

## 🛠️ 技术栈

| 层级 | 技术 | 用途 |
|------|------|------|
| **外壳** | Tauri v2 | 原生窗口、IPC、系统集成 |
| **后端** | Rust | 音频采集、HTTP 客户端、热键 Hook |
| **前端** | HTML / JS / CSS | 界面、状态管理、设置 |
| **语音识别** | Qwen3-ASR-1.7B + Python/Flask | 本地 ASR 服务器 |
| **大语言模型** | DeepSeek V4 Flash | 文本翻译 & 改写 |
| **语音合成** | MiMo v2.5 TTS VoiceDesign | 高拟真语音输出 |
| **音频接口** | WASAPI (Windows) | 低延迟音频 I/O |

---

## 🤝 参与贡献

欢迎贡献！这是一个非盈利、完全开源的项目（纯 AI 创作）。你可以：

- 🐛 通过 [Issues](https://github.com/Phoesol/ai-voice-console/issues) 报告 Bug
- 🔀 提交 Pull Request
- 💡 提出新功能建议
- 🌍 帮助翻译
- 🔧 优化性能或架构

**欢迎大佬拿去优化，二创。**

---

## ❤️ 致谢

本项目的丰富功能与极速体验，完全建立在以下优秀 AI 模型的基础之上，特此致谢：

- **[Qwen3-ASR](https://huggingface.co/Qwen/Qwen3-ASR-1.7B)**（通义千问）— 感谢其出色的本地推理优化，以极低的系统性能消耗提供了精准无误的语音识别与语言理解能力，构筑了本引擎的"**听觉**"。
- **[DeepSeek-V4-Flash](https://platform.deepseek.com/)**（深度求索）— 感谢其令人惊叹的开源精神与极致性价比（高强度使用一天几毛钱，几乎免费），其强悍的文本指令遵循与翻译重构能力，赋予了本引擎最核心的"**大脑**"。
- **[MiMo-v2.5-TTS-VoiceDesign](https://api.xiaomimimo.com/)**（小米）— 感谢其卓越的声色设计能力与丰富的情绪标签解析，免费且强大的语言生成表现让冰冷的文字转化为了极具情绪张力的完美"**声带**"。

---

## 📄 开源协议

本项目采用 [MIT 协议](LICENSE) 开源。

---

<p align="center">
  Made with 🤖 by AI, for humans.<br/>
  <b>纯 AI 创作，欢迎二创。</b>
</p>
