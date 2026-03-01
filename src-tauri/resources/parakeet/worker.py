#!/usr/bin/env python3
# pyright: reportMissingImports=false

import json
import os
import sys
import wave
from time import perf_counter

import numpy as np
import torch
from transformers import AutoModelForCTC, AutoProcessor


def write_response(payload: dict):
    sys.stdout.write(json.dumps(payload, ensure_ascii=True) + "\n")
    sys.stdout.flush()


def normalize_device(device: str) -> str:
    value = (device or "cpu").strip().lower()
    if value not in {"cpu", "cuda"}:
        return "cpu"
    return value


def resolve_dtype(device: str, compute_type: str):
    normalized = (compute_type or "auto").strip().lower()
    if normalized == "auto":
        return torch.float16 if device == "cuda" else torch.float32
    if normalized == "float16":
        return torch.float16
    if normalized == "float32":
        return torch.float32
    raise ValueError("unsupported compute_type; use auto, float16, or float32")


def read_wav_mono_16k(audio_path: str) -> np.ndarray:
    with wave.open(audio_path, "rb") as reader:
        channels = reader.getnchannels()
        sample_width = reader.getsampwidth()
        sample_rate = reader.getframerate()
        frame_count = reader.getnframes()
        raw = reader.readframes(frame_count)

    if sample_rate != 16000:
        raise ValueError(f"expected 16000 Hz audio, got {sample_rate} Hz")
    if channels <= 0:
        raise ValueError("invalid channel count")

    if sample_width == 2:
        audio = np.frombuffer(raw, dtype=np.int16).astype(np.float32) / 32768.0
    elif sample_width == 4:
        audio = np.frombuffer(raw, dtype=np.int32).astype(np.float32) / 2147483648.0
    elif sample_width == 1:
        audio = (np.frombuffer(raw, dtype=np.uint8).astype(np.float32) - 128.0) / 128.0
    else:
        raise ValueError(f"unsupported sample width {sample_width * 8} bits")

    if channels > 1:
        audio = audio.reshape(-1, channels).mean(axis=1)

    return np.clip(audio, -1.0, 1.0)


class ModelRuntime:
    def __init__(self):
        self._key = None
        self._processor = None
        self._model = None

    def get_model_bundle(self, model_name: str, device: str, compute_type: str):
        key = (model_name, device, compute_type)
        if self._model is not None and self._processor is not None and self._key == key:
            return self._processor, self._model

        if device == "cuda" and not torch.cuda.is_available():
            raise RuntimeError("CUDA requested but torch.cuda.is_available() is false")

        dtype = resolve_dtype(device, compute_type)
        cache_root = os.environ.get("SONORA_PARAKEET_MODEL_CACHE", "").strip() or None

        processor = AutoProcessor.from_pretrained(model_name, cache_dir=cache_root)
        model = AutoModelForCTC.from_pretrained(
            model_name,
            cache_dir=cache_root,
            torch_dtype=dtype,
        )
        model.to(device)
        model.eval()

        self._processor = processor
        self._model = model
        self._key = key
        return processor, model


def handle_ping(request: dict):
    request_id = str(request.get("id", ""))
    write_response(
        {
            "id": request_id,
            "ok": True,
            "pong": True,
            "cuda_available": bool(torch.cuda.is_available()),
            "torch_version": str(getattr(torch, "__version__", "unknown")),
            "torch_cuda_version": str(getattr(torch.version, "cuda", "none") or "none"),
        }
    )


def handle_preload(runtime: ModelRuntime, request: dict):
    request_id = str(request.get("id", ""))
    model_name = str(request.get("model", "nvidia/parakeet-ctc-0.6b")).strip()
    device = normalize_device(str(request.get("device", "cpu")))
    compute_type = str(request.get("compute_type", "auto")).strip() or "auto"

    started_at = perf_counter()
    runtime.get_model_bundle(model_name, device, compute_type)
    load_ms = int((perf_counter() - started_at) * 1000)

    write_response(
        {
            "id": request_id,
            "ok": True,
            "model": model_name,
            "device": device,
            "compute_type": compute_type,
            "load_ms": load_ms,
        }
    )


def handle_transcribe(runtime: ModelRuntime, request: dict):
    request_id = str(request.get("id", ""))
    audio_path = str(request.get("audio_path", "")).strip()
    if not audio_path:
        write_response({"id": request_id, "ok": False, "error": "missing audio_path"})
        return

    model_name = str(request.get("model", "nvidia/parakeet-ctc-0.6b")).strip()
    language = str(request.get("language", "en")).strip() or "en"
    device = normalize_device(str(request.get("device", "cpu")))
    compute_type = str(request.get("compute_type", "auto")).strip() or "auto"

    started_at = perf_counter()
    processor, model = runtime.get_model_bundle(model_name, device, compute_type)
    model_dtype = next(model.parameters()).dtype

    audio = read_wav_mono_16k(audio_path)
    inputs = processor(audio, sampling_rate=16000, return_tensors="pt")

    prepared_inputs = {}
    for name, value in inputs.items():
        moved = value.to(model.device)
        if torch.is_floating_point(moved):
            moved = moved.to(model_dtype)
        prepared_inputs[name] = moved

    with torch.inference_mode():
        logits = model(**prepared_inputs).logits
        token_ids = torch.argmax(logits, dim=-1)

    _ = language
    text = processor.batch_decode(token_ids, skip_special_tokens=True)[0].strip()
    text = " ".join(text.split())

    inference_ms = int((perf_counter() - started_at) * 1000)
    write_response(
        {
            "id": request_id,
            "ok": True,
            "text": text,
            "inference_ms": inference_ms,
        }
    )


def main():
    runtime = ModelRuntime()
    for raw in sys.stdin:
        line = raw.strip()
        if not line:
            continue

        try:
            request = json.loads(line)
        except Exception as error:
            write_response({"id": "", "ok": False, "error": f"invalid json: {error}"})
            continue

        request_id = str(request.get("id", ""))
        op = str(request.get("op", "")).strip().lower()
        try:
            if op == "ping":
                handle_ping(request)
            elif op == "preload":
                handle_preload(runtime, request)
            elif op == "transcribe":
                handle_transcribe(runtime, request)
            else:
                write_response({"id": request_id, "ok": False, "error": f"unsupported op: {op}"})
        except Exception as error:
            write_response({"id": request_id, "ok": False, "error": str(error)})


if __name__ == "__main__":
    main()
