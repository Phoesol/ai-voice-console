#!/usr/bin/env python3
"""Qwen3-ASR HTTP Server — Flask backend with reliable multipart parsing

Uses Flask instead of manual HTTP server + manual multipart parsing.
Flask's Werkzeug parser correctly handles binary data in multipart forms,
which the previous manual split-based parser could corrupt.

参考: https://huggingface.co/Qwen/Qwen3-ASR-1.7B
"""

import sys
import os
import re
import wave
import tempfile
import traceback
import time
import logging

sys.stdout.reconfigure(line_buffering=True)
sys.stderr.reconfigure(line_buffering=True)

log = logging.getLogger('werkzeug')
log.setLevel(logging.WARNING)

import torch
import numpy as np
from flask import Flask, request, jsonify

MODEL = None
DEVICE = "cuda" if torch.cuda.is_available() else "cpu"
DEBUG_DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)), "debug_audio")
DEBUG_AUDIO = os.environ.get("ASR_DEBUG_AUDIO", "0") == "1"
TEMP_DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)), "data", "tmp")

app = Flask(__name__)


def load_model(model_path):
    global MODEL
    from qwen_asr import Qwen3ASRModel

    dtype = torch.bfloat16 if DEVICE.startswith("cuda") else torch.float32
    device_map = DEVICE if DEVICE.startswith("cuda") else "cpu"

    print(f"[ASR] Loading model from {model_path} on {device_map} ({dtype})...", flush=True)
    MODEL = Qwen3ASRModel.from_pretrained(
        model_path,
        dtype=dtype,
        device_map=device_map,
        max_inference_batch_size=8,
        max_new_tokens=512,
    )
    print(f"[ASR] Model loaded successfully on {device_map}", flush=True)


def _detect_audio_format(data):
    if len(data) >= 4 and data[:4] == b"RIFF":
        return "wav"
    if len(data) >= 4 and data[:4] == b"\x1a\x45\xdf\xa3":
        return "webm"
    if len(data) >= 12 and data[4:8] == b"ftyp":
        return "mp4"
    if len(data) >= 3 and data[:3] == b"ID3":
        return "mp3"
    if len(data) >= 4 and data[:4] == b"OggS":
        return "ogg"
    if len(data) >= 2 and data[:2] == b"\xff\xfb":
        return "mp3"
    return "wav"


def _decode_audio(data):
    fmt = _detect_audio_format(data)
    suffix = f".{fmt}"
    tmp_path = None

    try:
        os.makedirs(TEMP_DIR, exist_ok=True)
        with tempfile.NamedTemporaryFile(suffix=suffix, delete=False, dir=TEMP_DIR) as f:
            f.write(data)
            tmp_path = f.name

        print(f"[ASR] Decoding {fmt} audio ({len(data)} bytes)...", flush=True)

        if fmt == "wav":
            try:
                waveform, sr = _read_wav_simple(tmp_path)
                if sr != 16000:
                    waveform = _resample(waveform, sr, 16000)
                print(f"[ASR] WAV decoded: {len(waveform)} samples @ 16000Hz", flush=True)
                return waveform, 16000
            except Exception as e:
                print(f"[ASR] WAV decode failed: {e}", flush=True)

        try:
            import av
            container = av.open(tmp_path)
            stream = next((s for s in container.streams if s.type == 'audio'), None)
            if stream is None:
                raise RuntimeError("No audio stream")
            resampler = av.AudioResampler(format='flt', layout='mono', rate=16000)
            chunks = []
            for frame in container.decode(stream):
                frame = resampler.resample(frame)
                for f in frame:
                    arr = f.to_ndarray()
                    chunks.append(arr.flatten())
            container.close()
            if chunks:
                waveform = np.concatenate(chunks).astype(np.float32)
                print(f"[ASR] PyAV decoded: {len(waveform)} samples @ 16000Hz", flush=True)
                return waveform, 16000
        except Exception as e:
            print(f"[ASR] PyAV decode failed: {e}", flush=True)

        try:
            import librosa
            waveform, sr = librosa.load(tmp_path, sr=16000)
            print(f"[ASR] librosa decoded: {len(waveform)} samples @ 16000Hz", flush=True)
            return waveform, 16000
        except Exception as e:
            print(f"[ASR] librosa decode failed: {e}", flush=True)

        try:
            import soundfile as sf
            waveform, sr = sf.read(tmp_path, dtype="float32")
            if waveform.ndim > 1:
                waveform = waveform[:, 0]
            if sr != 16000:
                waveform = _resample(waveform, sr, 16000)
            print(f"[ASR] soundfile decoded: {len(waveform)} samples @ 16000Hz", flush=True)
            return waveform, 16000
        except Exception as e:
            print(f"[ASR] soundfile decode failed: {e}", flush=True)

        try:
            from pydub import AudioSegment
            seg = AudioSegment.from_file(tmp_path)
            seg = seg.set_frame_rate(16000).set_channels(1).set_sample_width(2)
            raw = np.frombuffer(seg.raw_data, dtype=np.int16).astype(np.float32) / 32768.0
            print(f"[ASR] pydub decoded: {len(raw)} samples @ 16000Hz", flush=True)
            return raw, 16000
        except Exception as e:
            print(f"[ASR] pydub decode failed: {e}", flush=True)

        try:
            import subprocess
            out_path = tmp_path + "_converted.wav"
            result = subprocess.run(
                ["ffmpeg", "-y", "-i", tmp_path, "-ar", "16000", "-ac", "1", "-f", "wav", out_path],
                capture_output=True, timeout=10,
            )
            if result.returncode == 0 and os.path.exists(out_path):
                waveform, sr = _read_wav_simple(out_path)
                os.unlink(out_path)
                print(f"[ASR] ffmpeg decoded: {len(waveform)} samples @ 16000Hz", flush=True)
                return waveform, 16000
        except Exception as e:
            print(f"[ASR] ffmpeg decode failed: {e}", flush=True)

        raise RuntimeError(f"Cannot decode audio format: {fmt}")

    finally:
        if tmp_path and os.path.exists(tmp_path):
            try:
                os.unlink(tmp_path)
            except Exception:
                pass


def _resample(waveform, orig_sr, target_sr):
    if orig_sr == target_sr or len(waveform) == 0:
        return waveform
    try:
        import soxr
        return soxr.resample(waveform, orig_sr, target_sr).astype(np.float32)
    except ImportError:
        pass
    try:
        from scipy.signal import resample as _scipy_resample
        num_samples = int(len(waveform) * target_sr / orig_sr)
        return _scipy_resample(waveform, num_samples).astype(np.float32)
    except ImportError:
        step = orig_sr / target_sr
        indices = np.arange(0, len(waveform), step).astype(int)
        return waveform[indices]


def _read_wav_simple(path):
    with wave.open(path, "rb") as wf:
        sr = wf.getframerate()
        n = wf.getnframes()
        raw = wf.readframes(n)
        dtype = np.int16 if wf.getsampwidth() == 2 else np.float32
        waveform = np.frombuffer(raw, dtype=dtype).astype(np.float32)
        if dtype == np.int16:
            waveform = waveform / 32768.0
        if wf.getnchannels() > 1:
            waveform = waveform[::wf.getnchannels()]
    return waveform, sr


def _save_debug_audio(data, fmt, waveform, sr):
    try:
        os.makedirs(DEBUG_DIR, exist_ok=True)
        ts = time.strftime("%Y%m%d_%H%M%S")
        raw_path = os.path.join(DEBUG_DIR, f"raw_{ts}.{fmt}")
        wav_path = os.path.join(DEBUG_DIR, f"decoded_{ts}.wav")
        with open(raw_path, "wb") as f:
            f.write(data)
        import soundfile as sf
        sf.write(wav_path, waveform, sr)
        print(f"[ASR] Debug: {raw_path} ({len(data)}B), {wav_path} ({len(waveform)} samples)", flush=True)
    except Exception as e:
        print(f"[ASR] Debug save failed: {e}", flush=True)


@app.route("/health", methods=["GET"])
def health():
    if MODEL is None:
        return jsonify({"status": "loading", "model_loaded": False}), 503
    return jsonify({"status": "ok", "model_loaded": True})


@app.route("/v1/audio/transcriptions", methods=["POST"])
def transcribe():
    try:
        if MODEL is None:
            return jsonify({"error": "Model not loaded yet, please wait"}), 503

        audio_data = None
        language = None

        if request.files and "file" in request.files:
            file = request.files["file"]
            audio_data = file.read()
            print(f"[ASR] Received file: {file.filename}, {len(audio_data)} bytes", flush=True)
        else:
            audio_data = request.get_data()
            print(f"[ASR] Received raw body: {len(audio_data)} bytes", flush=True)

        language = request.form.get("language", None)

        if audio_data is None or len(audio_data) == 0:
            return jsonify({"error": "no audio data"}), 400

        fmt = _detect_audio_format(audio_data)
        print(f"[ASR] Detected format: {fmt}, size: {len(audio_data)} bytes", flush=True)

        waveform, sr = _decode_audio(audio_data)

        if len(waveform) == 0:
            return jsonify({"error": "empty audio after decoding"}), 400

        duration = len(waveform) / sr
        peak = float(np.max(np.abs(waveform)))
        rms = float(np.sqrt(np.mean(waveform ** 2)))
        print(f"[ASR] Waveform: {len(waveform)} samples, {duration:.2f}s, peak={peak:.4f}, rms={rms:.4f}", flush=True)

        if DEBUG_AUDIO:
            _save_debug_audio(audio_data, fmt, waveform, sr)

        results = MODEL.transcribe(
            audio=(waveform, sr),
            language=language if language else None,
        )

        if not results or len(results) == 0:
            print(f"[ASR] Model returned empty result", flush=True)
            return jsonify({"text": "", "language": language or "zh"})

        raw_text = results[0].text.strip() if results[0].text else ""
        detected_lang = results[0].language if hasattr(results[0], 'language') and results[0].language else (language or "zh")

        emotion_tags = []
        for m in re.finditer(r"<\|([A-Z_]+)\|>", raw_text):
            emotion_tags.append(f"<|{m.group(1)}|>")
        pure_text = re.sub(r"<\|([A-Z_]+)\|>", "", raw_text).strip()

        print(f"[ASR] Result: {pure_text} (lang={detected_lang}, emotions={emotion_tags})", flush=True)
        return jsonify({
            "text": pure_text,
            "language": detected_lang,
            "emotion_tags": emotion_tags,
        })

    except (ValueError, RuntimeError) as e:
        print(f"[ASR] ERROR in transcribe: {type(e).__name__}: {e}", flush=True)
        traceback.print_exc()
        return jsonify({"error": f"{type(e).__name__}: {e}"}), 500
    except Exception as e:
        print(f"[ASR] UNEXPECTED ERROR in transcribe: {type(e).__name__}: {e}", flush=True)
        traceback.print_exc()
        return jsonify({"error": f"Internal error: {type(e).__name__}"}), 500


def main():
    if len(sys.argv) < 2:
        print("Usage: asr_server.py <model_path> [--host HOST] [--port PORT]")
        sys.exit(1)

    model_path = sys.argv[1]
    host = "127.0.0.1"
    port = 18765

    i = 2
    while i < len(sys.argv):
        if sys.argv[i] == "--host" and i + 1 < len(sys.argv):
            host = sys.argv[i + 1]; i += 2
        elif sys.argv[i] == "--port" and i + 1 < len(sys.argv):
            port = int(sys.argv[i + 1]); i += 2
        else:
            i += 1

    load_model(model_path)

    print(f"[ASR] Flask server listening on http://{host}:{port}", flush=True)
    app.run(host=host, port=port, threaded=True, debug=False)


if __name__ == "__main__":
    main()
