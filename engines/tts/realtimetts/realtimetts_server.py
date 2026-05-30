"""RealtimeTTS API 服务器
引擎: mimo(小米云端TTS), edge(免费云端), kokoro(本地), system(系统语音)
启动: python realtimetts_server.py --port 18084 --engine mimo
"""

import argparse, io, os, time, numpy as np, soundfile as sf, uvicorn
import soxr
from contextlib import asynccontextmanager
from typing import Optional
from fastapi import FastAPI, HTTPException
from fastapi.responses import Response
from pydantic import BaseModel

from RealtimeTTS import (
    TextToAudioStream,
    MimoEngine,
)

_engine = None
_engine_name = None
_target_sr = 48000

MIMO_API_KEY = os.environ.get("MIMO_API_KEY", "")
MIMO_API_BASE = "https://api.xiaomimimo.com/v1"

ENGINES = {
    "mimo": MimoEngine,
}


class TTSRequest(BaseModel):
    text: str
    format: str = "wav"
    top_p: Optional[float] = None
    temperature: Optional[float] = None
    repetition_penalty: Optional[float] = None
    chunk_length: Optional[int] = None
    normalize: Optional[bool] = None
    streaming: Optional[bool] = None
    reference_id: Optional[str] = None
    references: Optional[list] = None
    voice: Optional[str] = None
    style_prompt: Optional[str] = None
    optimize_text_preview: Optional[bool] = None
    voice_design: Optional[str] = None
    clone_audio_path: Optional[str] = None
    mimo_model: Optional[str] = None


_startup_kwargs = {}


@asynccontextmanager
async def lifespan(application):
    global _engine, _engine_name
    _engine_name = _startup_kwargs.get("engine_name", "mimo")
    engine_kwargs = _startup_kwargs.get("engine_kwargs", {}) or {}
    cls = ENGINES.get(_engine_name)
    if cls is None:
        raise ValueError(f"Unknown engine: {_engine_name}")
    print(f"[RealtimeTTS] Loading {_engine_name} engine...")
    _engine = cls(**engine_kwargs)
    print(f"[RealtimeTTS] Engine {_engine_name} ready")
    yield


app = FastAPI(title="RealtimeTTS API", version="2.0", lifespan=lifespan)


@app.get("/v1/health")
def health():
    return {"status": "ok", "engine": _engine_name}


@app.get("/health")
def health_alt():
    return {"status": "ok", "engine": _engine_name}


@app.post("/v1/tts")
def tts(req: TTSRequest):
    if _engine is None:
        raise HTTPException(status_code=503, detail="Engine not loaded")

    if isinstance(_engine, MimoEngine):
        return _tts_mimo(req)

    return _tts_stream(req)


def _tts_mimo(req: TTSRequest):
    if req.mimo_model:
        _engine.model = req.mimo_model
    if _engine.model == "mimo-v2.5-tts":
        _engine.clone_audio_path = ""
        _engine.voice_design = ""
    elif _engine.model == "mimo-v2.5-tts-voicedesign":
        _engine.clone_audio_path = ""
    elif _engine.model == "mimo-v2.5-tts-voiceclone":
        _engine.voice_design = ""
    if req.voice:
        _engine.set_voice(req.voice)
    if req.style_prompt:
        _engine.style_prompt = req.style_prompt
    if req.voice_design:
        _engine.voice_design = req.voice_design
    if req.clone_audio_path:
        _engine.clone_audio_path = req.clone_audio_path
    if req.optimize_text_preview is not None:
        _engine.optimize_text_preview = req.optimize_text_preview

    text = req.text
    if not text or not text.strip():
        raise HTTPException(status_code=400, detail="text is required")

    import threading, queue as qmod
    audio_queue = qmod.Queue()

    _engine.stop_synthesis_event.clear()
    _engine.queue = audio_queue

    t0 = time.perf_counter()
    success = _engine.synthesize(text)

    if not success:
        last_err = getattr(_engine, '_last_api_error', '')
        detail = f"Synthesis failed{': ' + last_err if last_err else ''}"
        raise HTTPException(status_code=400, detail=detail)

    audio_chunks = []
    while True:
        try:
            chunk = audio_queue.get_nowait()
        except qmod.Empty:
            break
        audio_chunks.append(chunk)

    if not audio_chunks:
        raise HTTPException(status_code=400, detail="No audio data in queue")

    audio_data = b"".join(audio_chunks)
    elapsed = time.perf_counter() - t0

    try:
        audio_np = np.frombuffer(audio_data, dtype=np.int16).astype(np.float32) / 32768.0
        orig_sr = 24000
        target_sr = _target_sr
        if orig_sr != target_sr:
            audio_np = soxr.resample(audio_np, orig_sr, target_sr).astype(np.float32)
        if audio_np.ndim == 1:
            audio_np = audio_np.reshape(-1, 1)
        buf_out = io.BytesIO()
        sf.write(buf_out, audio_np, target_sr, format="WAV", subtype="PCM_16")
        buf_out.seek(0)
        result = buf_out.getvalue()
    except Exception:
        result = audio_data

    print(f"[RealtimeTTS/MiMo] {len(audio_data)} bytes in {elapsed:.1f}s, model={_engine.model}, {orig_sr}→{target_sr}Hz")
    return Response(content=result, media_type="audio/wav")


def _tts_stream(req: TTSRequest):
    import threading, queue as qmod
    audio_queue = qmod.Queue()

    def on_chunk(chunk):
        audio_queue.put(chunk)

    def on_stop():
        audio_queue.put(None)

    stream = TextToAudioStream(_engine, on_audio_stream_stop=on_stop, muted=True)
    stream.feed(req.text)

    t0 = time.perf_counter()
    stream.play_async(on_audio_chunk=on_chunk, muted=True)

    chunks = []
    while True:
        try:
            chunk = audio_queue.get(timeout=120)
        except qmod.Empty:
            break
        if chunk is None:
            break
        chunks.append(chunk)

    if not chunks:
        raise HTTPException(status_code=400, detail="No audio generated")

    audio_data = b"".join(chunks)
    elapsed = time.perf_counter() - t0

    try:
        audio_np = np.frombuffer(audio_data, dtype=np.int16).astype(np.float32) / 32768.0
        orig_sr = 24000
        target_sr = _target_sr
        if orig_sr != target_sr:
            audio_np = soxr.resample(audio_np, orig_sr, target_sr).astype(np.float32)
        if audio_np.ndim == 1:
            audio_np = audio_np.reshape(-1, 1)
        buf_out = io.BytesIO()
        sf.write(buf_out, audio_np, target_sr, format="WAV", subtype="PCM_16")
        buf_out.seek(0)
        result = buf_out.getvalue()
    except Exception:
        result = audio_data

    print(f"[RealtimeTTS] {len(audio_data)} bytes in {elapsed:.1f}s, engine={_engine_name}, {orig_sr}→{target_sr}Hz")
    return Response(content=result, media_type="audio/wav")


def main():
    global _target_sr
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, default=18084)
    parser.add_argument("--host", type=str, default="127.0.0.1")
    parser.add_argument("--engine", type=str, default="mimo",
                        help="mimo|edge|kokoro|system")
    parser.add_argument("--voice", type=str, default="冰糖",
                        help="MiMo voice: 冰糖/茉莉/苏打/白桦/Mia/Chloe/Milo/Dean")
    parser.add_argument("--model", type=str, default="mimo-v2.5-tts",
                        help="MiMo model: mimo-v2.5-tts/mimo-v2.5-tts-voicedesign/mimo-v2.5-tts-voiceclone")
    parser.add_argument("--api-key", type=str, default=MIMO_API_KEY,
                        help="MiMo API key")
    parser.add_argument("--api-base", type=str, default=MIMO_API_BASE,
                        help="MiMo API base URL")
    parser.add_argument("--target-sr", type=int, default=48000,
                        help="Target sample rate for resampling (default: 48000)")
    args = parser.parse_args()
    _target_sr = args.target_sr

    engine_kwargs = {}
    if args.engine == "mimo":
        engine_kwargs = {
            "api_key": args.api_key,
            "api_base": args.api_base,
            "voice": args.voice,
            "model": args.model,
            "audio_format": "wav",
            "debug": True,
        }

    app.state.engine_name = args.engine
    app.state.engine_kwargs = engine_kwargs
    _startup_kwargs["engine_name"] = args.engine
    _startup_kwargs["engine_kwargs"] = engine_kwargs
    print(f"[RealtimeTTS] Engine: {args.engine}, Port: {args.port}")
    if args.engine == "mimo":
        print(f"[RealtimeTTS] MiMo model: {args.model}, voice: {args.voice}")
    uvicorn.run(app, host=args.host, port=args.port, log_level="info")


if __name__ == "__main__":
    main()
