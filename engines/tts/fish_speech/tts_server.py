"""
tts_server.py —— Fish Speech TTS API 服务器
使用 fish_speech 库加载模型并提供 HTTP API 接口。
支持 openaudio-s1-mini (dual_ar) 和 s2-pro (fish_qwen3_omni) 模型。

启动方式:
  python tts_server.py --model checkpoints/openaudio-s1-mini --port 8080
  python tts_server.py --model checkpoints/s2-pro --port 8080

API 端点:
  GET  /v1/health  — 健康检查
  POST /v1/tts     — 文本转语音

请求体 (ServeTTSRequest):
  {
    "text": "要合成的文本",
    "format": "wav",
    "top_p": 0.7,
    "temperature": 0.7,
    "repetition_penalty": 1.2,
    "chunk_length": 200,
    "normalize": true,
    "streaming": false
  }
"""

import argparse
import io
import os
import queue
import sys
import threading
import time
from pathlib import Path
from typing import BinaryIO, Optional, Tuple, Union

import numpy as np
import torch
import uvicorn
from fastapi import FastAPI
from fastapi.responses import Response, StreamingResponse
from loguru import logger


def _patch_torchaudio_load():
    """Monkey-patch torchaudio.load to use soundfile when torchcodec is unavailable.
    torchaudio 2.9+ defaults to torchcodec which requires FFmpeg shared libraries.
    On Windows without FFmpeg, this falls back to soundfile."""
    try:
        import torchaudio
    except ImportError:
        return

    try:
        import torchcodec
        torchcodec.decoders
        return
    except (ImportError, RuntimeError):
        pass

    import soundfile as sf

    def _patched_load(
        uri: Union[BinaryIO, str, os.PathLike],
        frame_offset: int = 0,
        num_frames: int = -1,
        normalize: bool = True,
        channels_first: bool = True,
        format: Optional[str] = None,
        buffer_size: int = 4096,
        backend: Optional[str] = None,
    ) -> Tuple[torch.Tensor, int]:
        if hasattr(uri, "read"):
            uri.seek(0)
        data, sr = sf.read(uri, dtype="float32", frames=num_frames if num_frames > 0 else -1,
                           start=frame_offset)
        if data.ndim == 1:
            data = data.reshape(-1, 1)
        if channels_first:
            data = data.T
        else:
            data = data
        return torch.from_numpy(data.copy()), sr

    torchaudio.load = _patched_load
    logger.info("torchaudio.load 已切换为 soundfile 后端（torchcodec 不可用）")


_patch_torchaudio_load()


from fish_speech.inference_engine import TTSInferenceEngine
from fish_speech.models.dac.inference import load_model as load_decoder
from fish_speech.models.text2semantic.inference import (
    launch_thread_safe_queue,
    GenerateRequest,
)
from fish_speech.utils.schema import ServeTTSRequest

app = FastAPI(title="Fish Speech TTS Server")

llama_queue: queue.Queue | None = None
decoder_model = None
tts_engine: TTSInferenceEngine | None = None
SAMPLE_RATE = 24000


@app.get("/v1/health")
async def health():
    return {"status": "ok"}


@app.get("/health")
async def health_alt():
    return {"status": "ok"}


@app.post("/v1/tts")
async def text_to_speech(req: ServeTTSRequest):
    global tts_engine, SAMPLE_RATE

    if tts_engine is None:
        return Response(content=b"TTS engine not loaded", status_code=503)

    try:
        segments = []
        for result in tts_engine.inference(req):
            if result.code == "error":
                logger.error(f"TTS error: {result.error}")
                return Response(
                    content=str(result.error).encode(),
                    status_code=500,
                )
            if result.code == "final":
                sr, audio_data = result.audio
                SAMPLE_RATE = sr
                segments.append(audio_data)

        if not segments:
            return Response(content=b"No audio generated", status_code=500)

        audio = np.concatenate(segments, axis=0)

        if req.format == "wav":
            import soundfile as sf
            buf = io.BytesIO()
            sf.write(buf, audio, SAMPLE_RATE, format="WAV", subtype="PCM_16")
            buf.seek(0)
            return Response(
                content=buf.read(),
                media_type="audio/wav",
            )
        elif req.format == "mp3":
            import soundfile as sf
            buf = io.BytesIO()
            sf.write(buf, audio, SAMPLE_RATE, format="MP3")
            buf.seek(0)
            return Response(
                content=buf.read(),
                media_type="audio/mpeg",
            )
        else:
            audio_int16 = (audio * 32767).astype(np.int16)
            return Response(
                content=audio_int16.tobytes(),
                media_type="audio/pcm",
            )

    except Exception as e:
        logger.error(f"TTS inference error: {e}")
        return Response(content=str(e).encode(), status_code=500)


def load_models(checkpoint_path: str, device: str = "auto", precision: str = "bfloat16"):
    """加载 TTS 模型（文本语义模型 + 音频解码器）。"""
    global llama_queue, decoder_model, tts_engine, SAMPLE_RATE

    if device == "auto":
        device = "cuda:0" if torch.cuda.is_available() else "cpu"

    precision_map = {
        "bfloat16": torch.bfloat16,
        "float16": torch.float16,
        "float32": torch.float32,
    }
    dtype = precision_map.get(precision, torch.bfloat16)

    if device == "cpu" and dtype == torch.bfloat16:
        dtype = torch.float32
        logger.info("CPU 模式下使用 float32 精度")

    logger.info(f"加载文本语义模型: {checkpoint_path} ({device}, {dtype})")
    llama_queue = launch_thread_safe_queue(
        checkpoint_path=checkpoint_path,
        device=device,
        precision=dtype,
    )

    logger.info("加载音频解码器...")
    ckpt_dir = Path(checkpoint_path)
    codec_path = ckpt_dir / "codec.pth"
    if not codec_path.exists():
        raise FileNotFoundError(f"解码器文件不存在: {codec_path}")

    decoder_model = load_decoder("modded_dac_vq", str(codec_path), device=device)
    if hasattr(decoder_model, "spec_transform"):
        SAMPLE_RATE = decoder_model.spec_transform.sample_rate
    elif hasattr(decoder_model, "sample_rate"):
        SAMPLE_RATE = decoder_model.sample_rate
    logger.info(f"解码器采样率: {SAMPLE_RATE}")

    tts_engine = TTSInferenceEngine(
        llama_queue=llama_queue,
        decoder_model=decoder_model,
        precision=dtype,
        compile=False,
    )

    logger.info("所有模型加载完成")


def main():
    parser = argparse.ArgumentParser(description="Fish Speech TTS API Server")
    parser.add_argument(
        "--model", type=str, default="checkpoints/openaudio-s1-mini",
        help="模型 checkpoint 路径"
    )
    parser.add_argument("--port", type=int, default=18080, help="服务端口")
    parser.add_argument("--host", type=str, default="127.0.0.1", help="绑定地址")
    parser.add_argument("--device", type=str, default="auto", help="推理设备 (auto/cpu/cuda:0)")
    parser.add_argument("--precision", type=str, default="bfloat16", help="精度 (bfloat16/float16/float32)")
    args = parser.parse_args()

    logger.info(f"启动 Fish Speech TTS 服务器")
    logger.info(f"  模型: {args.model}")
    logger.info(f"  地址: http://{args.host}:{args.port}")

    load_models(args.model, args.device, args.precision)

    logger.info(f"API 服务就绪: http://{args.host}:{args.port}")
    uvicorn.run(app, host=args.host, port=args.port, log_level="warning")


if __name__ == "__main__":
    main()
