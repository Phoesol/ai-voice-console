from .base_engine import BaseEngine
from typing import Union
import pyaudio
import base64
import requests
import time
import io
import numpy as np
import soundfile as sf


MIMO_PRESET_VOICES = [
    {"voice_id": "冰糖", "name": "冰糖", "language": "zh", "gender": "female"},
    {"voice_id": "茉莉", "name": "茉莉", "language": "zh", "gender": "female"},
    {"voice_id": "苏打", "name": "苏打", "language": "zh", "gender": "male"},
    {"voice_id": "白桦", "name": "白桦", "language": "zh", "gender": "male"},
    {"voice_id": "Mia", "name": "Mia", "language": "en", "gender": "female"},
    {"voice_id": "Chloe", "name": "Chloe", "language": "en", "gender": "female"},
    {"voice_id": "Milo", "name": "Milo", "language": "en", "gender": "male"},
    {"voice_id": "Dean", "name": "Dean", "language": "en", "gender": "male"},
]

MIMO_MODELS = {
    "preset": "mimo-v2.5-tts",
    "voicedesign": "mimo-v2.5-tts-voicedesign",
    "voiceclone": "mimo-v2.5-tts-voiceclone",
}


class MimoVoice:
    def __init__(self, voice_id, name, language="zh", gender="unknown"):
        self.id = voice_id
        self.name = name
        self.language = language
        self.gender = gender

    def __repr__(self):
        return f"{self.name} ({self.language}, {self.gender})"

    def __str__(self):
        return self.name


class MimoEngine(BaseEngine):
    def __init__(
        self,
        api_key: str = "",
        api_base: str = "https://api.xiaomimimo.com/v1",
        voice: str = "冰糖",
        model: str = "mimo-v2.5-tts",
        style_prompt: str = "",
        audio_format: str = "wav",
        timeout: float = 120.0,
        debug: bool = False,
    ):
        self.api_key = api_key
        self.api_base = api_base
        self.voice = voice
        self.model = model
        self.style_prompt = style_prompt
        self.audio_format = audio_format
        self.timeout = timeout
        self.debug = debug
        self.current_voice = None
        self.voice_design: str = ""
        self.clone_audio_path: str = ""
        self._session = requests.Session()

        for v in MIMO_PRESET_VOICES:
            if v["voice_id"] == voice:
                self.current_voice = MimoVoice(**v)
                break
        if self.current_voice is None:
            self.current_voice = MimoVoice(voice, voice)

    def post_init(self):
        self.engine_name = "mimo"

    def get_stream_info(self):
        return pyaudio.paInt16, 1, 24000

    def synthesize(self, text: str, sentence_count: int = 0) -> bool:
        super().synthesize(text, sentence_count)

        if not text or not text.strip():
            return False

        if self.debug:
            t0 = time.time()
            print(f"[MimoEngine] Synthesizing: {text[:60]}...")

        messages = []
        model = self.model
        if model == MIMO_MODELS["voicedesign"] and self.voice_design:
            user_content = self.voice_design
        else:
            user_content = self.style_prompt or "用自然流畅的语气朗读以下内容"
        messages.append({"role": "user", "content": user_content})
        messages.append({"role": "assistant", "content": text})

        audio_config = {
            "format": self.audio_format,
        }
        if model == MIMO_MODELS["voicedesign"]:
            pass
        elif model == MIMO_MODELS["voiceclone"]:
            if self.clone_audio_path:
                try:
                    with open(self.clone_audio_path, "rb") as f:
                        audio_b64 = base64.b64encode(f.read()).decode("utf-8")
                    ext = self.clone_audio_path.rsplit(".", 1)[-1].lower()
                    if ext == "mp3":
                        mime_type = "audio/mpeg"
                    else:
                        mime_type = "audio/wav"
                    audio_config["voice"] = f"data:{mime_type};base64,{audio_b64}"
                except Exception as e:
                    if self.debug:
                        print(f"[MimoEngine] Clone audio read error: {e}")
        else:
            audio_config["voice"] = self.current_voice.id if self.current_voice else self.voice

        payload = {
            "model": model,
            "messages": messages,
            "audio": audio_config,
        }
        if self.debug:
            print(f"[MimoEngine] model={model}, clone_path={self.clone_audio_path}, voice_key={list(audio_config.keys())}")
            if "voice" in audio_config:
                v = audio_config["voice"]
                print(f"[MimoEngine] audio.voice type={'DataURL' if v.startswith('data:') else 'preset_id'}, value={v[:60] if len(v) > 60 else v}")
        # optimize_text_preview: 让 MiMo 服务端优化文本（分句、韵律），增强表现力
        if getattr(self, "optimize_text_preview", False):
            payload["audio"]["optimize_text_preview"] = True

        try:
            resp = self._session.post(
                f"{self.api_base}/chat/completions",
                json=payload,
                headers={
                    "api-key": self.api_key,
                    "Content-Type": "application/json",
                },
                timeout=self.timeout,
            )
            if resp.status_code != 200:
                if self.debug:
                    print(f"[MimoEngine] API error: {resp.status_code} {resp.text[:200]}")
                return False

            data = resp.json()
            audio_b64 = data["choices"][0]["message"]["audio"]["data"]
            audio_bytes = base64.b64decode(audio_b64)

            if self.audio_format in ("pcm", "pcm16"):
                audio_np = np.frombuffer(audio_bytes, dtype=np.int16)
                chunk_size = 4096
                for i in range(0, len(audio_np), chunk_size):
                    if self.stop_synthesis_event.is_set():
                        return True
                    chunk = audio_np[i:i + chunk_size].tobytes()
                    self.queue.put(chunk)
            else:
                buf = io.BytesIO(audio_bytes)
                wav_data, sr = sf.read(buf, dtype="float32")
                if wav_data.ndim > 1:
                    wav_data = wav_data[:, 0]
                pcm = (wav_data * 32767).astype(np.int16)
                chunk_size = 4096
                for i in range(0, len(pcm), chunk_size):
                    if self.stop_synthesis_event.is_set():
                        return True
                    chunk = pcm[i:i + chunk_size].tobytes()
                    self.queue.put(chunk)

            if self.debug:
                elapsed = time.time() - t0
                print(f"[MimoEngine] Done: {len(audio_bytes)} bytes in {elapsed:.1f}s")

            return True

        except requests.exceptions.Timeout:
            if self.debug:
                print("[MimoEngine] API timeout")
            return False
        except requests.exceptions.ConnectionError:
            if self.debug:
                print("[MimoEngine] Connection error")
            return False
        except Exception as e:
            if self.debug:
                print(f"[MimoEngine] Error: {e}")
            return False

    def get_voices(self):
        return [MimoVoice(**v) for v in MIMO_PRESET_VOICES]

    def set_voice(self, voice: Union[str, MimoVoice]):
        if isinstance(voice, MimoVoice):
            self.current_voice = voice
            self.voice = voice.id
        else:
            for v in MIMO_PRESET_VOICES:
                if voice == v["voice_id"] or voice == v["name"]:
                    self.current_voice = MimoVoice(**v)
                    self.voice = v["voice_id"]
                    return
            self.current_voice = MimoVoice(voice, voice)
            self.voice = voice

    def set_voice_parameters(self, **params):
        if "style_prompt" in params:
            self.style_prompt = params["style_prompt"]
        if "model" in params:
            self.model = params["model"]
        if "audio_format" in params:
            self.audio_format = params["audio_format"]
        if "voice_design" in params:
            self.voice_design = params["voice_design"]
        if "clone_audio_path" in params:
            self.clone_audio_path = params["clone_audio_path"]

    def shutdown(self):
        self._session.close()
