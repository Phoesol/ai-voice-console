<p align="center">
  <img src="docs/banner.png" alt="AI Voice Console Banner" width="720" />
</p>

<h1 align="center">🎙️ AI Voice Console — 赛博声带</h1>

<p align="center">
  <b>Real-time Cross-lingual AI Voice Reconstruction Pipeline</b><br/>
  实时跨语种 AI 语音重构管线
</p>

<p align="center">
  <b>English</b> | <a href="./README_CN.md">中文</a>
</p>

<p align="center">
  <a href="#-features">Features</a> •
  <a href="#-architecture">Architecture</a> •
  <a href="#-quick-start">Quick Start</a> •
  <a href="#-configuration">Configuration</a> •
  <a href="#-use-cases">Use Cases</a> •
  <a href="#-acknowledgements">Acknowledgements</a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Tauri-v2-blue?logo=tauri" alt="Tauri v2" />
  <img src="https://img.shields.io/badge/Rust-2021-orange?logo=rust" alt="Rust 2021" />
  <img src="https://img.shields.io/badge/License-MIT-green" alt="MIT License" />
  <img src="https://img.shields.io/badge/Platform-Windows-blue?logo=windows" alt="Windows" />
</p>

---

## 📖 What is AI Voice Console?

**AI Voice Console（赛博声带）** is a system-level, real-time voice reconstruction and output tool built on the `ASR → LLM → TTS` pipeline architecture.

Unlike traditional voice changers that merely shift audio frequencies, AI Voice Console performs **semantic-level reconstruction**: it listens to your native language, processes your speech through an LLM for translation, rewriting, and emotional injection, then outputs high-fidelity synthesized speech in any target language with a customized voice persona.

> 如果说同传字幕引擎帮你"听懂世界"，那么 AI Voice Console 就是帮你"向世界发声"的跨语种数字声带。

## ✨ Features

### Core Pipeline
- 🎤 **Local ASR** — Blazing-fast speech recognition powered by Qwen3-ASR (supports dialects)
- 🧠 **LLM Director** — Customizable prompt-driven text rewriting, translation, and emotional scripting via DeepSeek
- 🔊 **Cloud TTS** — High-fidelity voice synthesis with rich audio tags (speed, breath, tremolo, etc.) via MiMo TTS
- 🎭 **Scene Director (TMD)** — Intelligent multi-scenario routing (e.g., auto-switch between "game callout mode" and "casual chat mode")

### Audio & System
- 🎚️ **Virtual Mic Output** — Routes synthesized audio directly to virtual microphone devices for use in any application
- 🔑 **Push-to-Talk (PTT)** — Global hotkey support with system-wide keyboard hooks + JS fallback
- 🔄 **WASAPI Loopback** — Capture system audio for real-time processing
- 🎵 **Voice Cloning** — Reference audio-based voice cloning support
- 💾 **Auto-save** — Automatic TTS output recording

### Performance
- ⚡ **Rust / Tauri v2 Core** — Native-level performance with minimal resource footprint
- 🚀 **Zero-copy IPC** — Pipe-based inter-process audio transfer, no temp files on disk
- 🌐 **TCP Keep-Alive** — Global singleton HTTP client with persistent connections, minimizing TLS handshake overhead
- 🔒 **Single Instance Lock** — Prevents duplicate application instances

### UI / UX
- 🌓 **Dark / Light Theme** — Elegant glassmorphism UI with theme switching
- 🌍 **i18n** — Chinese / English interface localization
- 📊 **Real-time Stats** — Live pipeline latency monitoring (ASR / LLM / TTS)
- 💬 **Chat Stage** — Visual conversation history with user / AI message bubbles

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Tauri v2 Application                     │
├──────────────────────┬──────────────────────────────────────┤
│   Frontend (WebView) │         Rust Backend                 │
│                      │                                      │
│  index.html          │  main.rs          ← App entry        │
│  main.js             │  ├─ audio/        ← WASAPI capture   │
│  audio.js            │  │   ├─ capture   │  & output        │
│  state.js            │  │   ├─ loopback  │                  │
│  settings-ui.js      │  │   ├─ output    │                  │
│  i18n.js             │  │   └─ resample  │                  │
│  styles.css          │  ├─ commands/     ← IPC handlers     │
│                      │  │   ├─ asr       │                  │
│                      │  │   ├─ llm       │                  │
│                      │  │   ├─ tts       │                  │
│                      │  │   ├─ hotkey    │                  │
│                      │  │   ├─ pipeline  │                  │
│                      │  │   ├─ recording │                  │
│                      │  │   └─ ...       │                  │
│                      │  ├─ http/         ← API clients      │
│                      │  │   ├─ asr_client│                  │
│                      │  │   ├─ llm_client│                  │
│                      │  │   └─ tts_client│                  │
│                      │  ├─ engine/       ← ASR server mgmt  │
│                      │  └─ state/        ← Config & state   │
└──────────────────────┴──────────────────────────────────────┘
         │                          │
         ▼                          ▼
┌─────────────┐  ┌─────────────────────────────────┐
│  ASR Server │  │        Cloud APIs                │
│  (Python)   │  │  ┌───────────┐ ┌──────────────┐  │
│  Qwen3-ASR  │  │  │ DeepSeek  │ │  MiMo TTS    │  │
│  Flask HTTP │  │  │ LLM API   │ │  Voice API   │  │
└─────────────┘  │  └───────────┘ └──────────────┘  │
                 └─────────────────────────────────┘
```

## 🚀 Quick Start

### Prerequisites

- **OS**: Windows 10/11 (x64)
- **Runtime**: Python 3.12+ (for ASR server)
- **GPU**: NVIDIA GPU with CUDA support (recommended for ASR)
- **Virtual Audio**: [VB-CABLE](https://vb-audio.com/Cable/) or similar virtual audio device

### 1. Clone the Repository

```bash
git clone https://github.com/Phoesol/ai-voice-console.git
cd ai-voice-console
```

### 2. Install ASR Model

Download the [Qwen3-ASR-1.7B](https://huggingface.co/Qwen/Qwen3-ASR-1.7B) model and place it in:

```
data/checkpoints/Qwen3-ASR-1.7B/
```

### 3. Set Up Python Environment

```bash
pip install torch flask numpy
pip install qwen-asr  # Qwen3 ASR inference package
```

### 4. Get API Keys

| Service | Purpose | Link |
|---------|---------|------|
| **DeepSeek** | LLM text processing | [platform.deepseek.com](https://platform.deepseek.com/) |
| **MiMo TTS** | Voice synthesis | [xiaomimimo.com](https://api.xiaomimimo.com/) |

### 5. Build & Run (Development)

```bash
# Install Rust & Tauri CLI
cargo install tauri-cli

# Build the application
cd src-tauri
cargo tauri dev
```

### 6. Configure

On first launch, open **Settings** (⚙️) and enter:
- Your **DeepSeek API Key**
- Your **MiMo TTS API Key**
- Select your **Microphone** and **Output Device**
- Configure **PTT Hotkey** (default: `T`)

## ⚙️ Configuration

The app stores settings in `config.json`. Key configuration fields:

```jsonc
{
  // ASR Engine
  "asr_engine": "qwen3_asr",

  // LLM Settings
  "deepseek_api_base": "https://api.deepseek.com/v1",
  "deepseek_api_key": "YOUR_API_KEY_HERE",
  "deepseek_model": "deepseek-v4-flash",

  // TTS Settings
  "tts_engine": "mimo_tts",
  "mimo_api_base": "https://api.xiaomimimo.com/v1",
  "mimo_api_key": "YOUR_API_KEY_HERE",
  "mimo_model": "mimo-v2.5-tts-voicedesign",

  // Voice Persona
  "mimo_voice_design": "Description of your desired voice...",
  "mimo_character": "Character description...",
  "mimo_scene": "Scene context...",
  "mimo_direction": "Voice direction instructions...",

  // Audio
  "ptt_enabled": true,
  "ptt_key1": "t",
  "volume": 0.88,
  "playback_speed": 1.0
}
```

> ⚠️ **Never commit your `config.json` to version control** — it contains API keys.

A template `config.example.json` is provided for reference.

## 🎮 Use Cases

### 🕹️ VRChat & VTuber — Cross-dimensional Multilingual Avatar
Speak naturally in your native language while your virtual avatar outputs perfectly voiced foreign speech matching your character persona (e.g., Japanese loli voice, English queen voice).

### 🎯 Foreign Game Servers — Barrier-free Tactical Communication
Callout in Chinese, output rapid-fire tactical commands in English/Korean/Japanese. When facing toxic players, let the LLM craft culturally-appropriate roasts in native slang.

### 💕 High Emotional Value Social — Cross-region Companionship
Leverage LLM text enhancement to transform ordinary speech into emotionally rich, character-consistent responses with custom voice personas.

### 🎭 Anonymous Streaming — Physical-level Identity Protection
Unlike traditional voice changers that can "break character" on coughs or shouts, this `Text → AI Voice` pipeline physically isolates your real microphone, ensuring your persona never breaks.

## 📁 Project Structure

```
ai-voice-console/
├── src/                    # Frontend (HTML/JS/CSS)
│   ├── index.html          # Main UI
│   ├── main.js             # App initialization & events
│   ├── audio.js            # Audio recording & playback
│   ├── state.js            # State machine
│   ├── settings-ui.js      # Settings panel logic
│   ├── i18n.js             # Internationalization
│   └── styles.css          # Styles (dark/light themes)
├── src-tauri/              # Rust backend (Tauri v2)
│   ├── src/
│   │   ├── main.rs         # App entry point
│   │   ├── audio/          # WASAPI audio capture & output
│   │   ├── commands/       # Tauri IPC command handlers
│   │   ├── http/           # HTTP clients (ASR/LLM/TTS)
│   │   ├── engine/         # ASR server lifecycle management
│   │   └── state/          # App state & settings
│   ├── Cargo.toml          # Rust dependencies
│   └── tauri.conf.json     # Tauri configuration
├── engines/                # Engine scripts
│   ├── asr/                # ASR engine implementations
│   └── tts/                # TTS engine implementations
├── asr_server.py           # Standalone ASR HTTP server (Flask)
├── config.example.json     # Configuration template
└── data/                   # Runtime data (models, logs, etc.)
```

## 🛠️ Tech Stack

| Layer | Technology | Purpose |
|-------|-----------|---------|
| **Shell** | Tauri v2 | Native window, IPC, system integration |
| **Backend** | Rust | Audio capture, HTTP clients, hotkey hooks |
| **Frontend** | HTML/JS/CSS | UI, state management, settings |
| **ASR** | Qwen3-ASR-1.7B + Python/Flask | Local speech recognition |
| **LLM** | DeepSeek V4 Flash | Text translation & rewriting |
| **TTS** | MiMo v2.5 TTS VoiceDesign | Voice synthesis |
| **Audio** | WASAPI (Windows) | Low-latency audio I/O |

## 🤝 Contributing

Contributions are welcome! This is a non-profit, fully open-source project. Feel free to:

- 🐛 Report bugs via [Issues](https://github.com/Phoesol/ai-voice-console/issues)
- 🔀 Submit pull requests
- 💡 Suggest new features
- 🌍 Help with translations

## ❤️ Acknowledgements

This project's rich functionality and blazing-fast experience is built upon the following excellent AI models:

- **[Qwen3-ASR](https://huggingface.co/Qwen/Qwen3-ASR-1.7B)** (Alibaba Qwen) — Outstanding local inference optimization providing accurate speech recognition with minimal system overhead. The "ears" of this engine.
- **[DeepSeek-V4-Flash](https://platform.deepseek.com/)** — Incredible open-source spirit and extreme cost-effectiveness. Its powerful text instruction following and translation capabilities form the "brain" of this engine.
- **[MiMo-v2.5-TTS-VoiceDesign](https://api.xiaomimimo.com/)** (Xiaomi) — Exceptional voice design capabilities with rich emotion tag parsing, transforming cold text into emotionally expressive speech. The "vocal cords" of this engine.

## 📄 License

This project is licensed under the [MIT License](LICENSE).

---

<p align="center">
  Made with 🤖 by AI, for humans.<br/>
  <b>纯 AI 创作，欢迎二创。</b>
</p>
