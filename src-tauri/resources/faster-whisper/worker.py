#!/usr/bin/env python3
# pyright: reportMissingImports=false

import json
import os
import sys
import tempfile
import wave
from time import perf_counter

from faster_whisper import WhisperModel


class ModelRuntime:
    def __init__(self):
        self._key = None
        self._model = None

    def get_model(self, model_name: str, device: str, compute_type: str):
        key = (model_name, device, compute_type)
        if self._model is not None and self._key == key:
            return self._model

        download_root = os.environ.get("SONORA_FASTER_WHISPER_MODEL_CACHE", "")
        model_options = {
            "device": device,
            "compute_type": compute_type,
        }
        if download_root:
            model_options["download_root"] = download_root

        self._model = WhisperModel(model_name, **model_options)
        self._key = key
        return self._model


def write_response(payload: dict):
    sys.stdout.write(json.dumps(payload, ensure_ascii=True) + "\n")
    sys.stdout.flush()


def handle_transcribe(runtime: ModelRuntime, request: dict):
    request_id = str(request.get("id", ""))
    audio_path = str(request.get("audio_path", "")).strip()
    if not audio_path:
        write_response(
            {
                "id": request_id,
                "ok": False,
                "error": "missing audio_path",
            }
        )
        return

    model_name = str(request.get("model", "small.en")).strip() or "small.en"
    device = str(request.get("device", "cpu")).strip() or "cpu"
    compute_type = str(request.get("compute_type", "int8")).strip() or "int8"
    language = str(request.get("language", "en")).strip() or "en"
    beam_size = int(request.get("beam_size", 1))
    condition_on_previous_text = bool(request.get("condition_on_previous_text", True))
    initial_prompt = request.get("initial_prompt", None)
    if initial_prompt is not None:
        initial_prompt = str(initial_prompt).strip() or None

    started_at = perf_counter()
    model = runtime.get_model(model_name, device, compute_type)
    segments, _info = model.transcribe(
        audio_path,
        language=language,
        beam_size=beam_size,
        condition_on_previous_text=condition_on_previous_text,
        initial_prompt=initial_prompt,
        vad_filter=False,
    )
    pieces = []
    for segment in segments:
        text = (segment.text or "").strip()
        if text:
            pieces.append(text)

    duration_ms = int((perf_counter() - started_at) * 1000)
    write_response(
        {
            "id": request_id,
            "ok": True,
            "text": " ".join(pieces).strip(),
            "inference_ms": duration_ms,
        }
    )


def handle_preload(runtime: ModelRuntime, request: dict):
    request_id = str(request.get("id", ""))
    model_name = str(request.get("model", "small.en")).strip() or "small.en"
    device = str(request.get("device", "cpu")).strip() or "cpu"
    compute_type = str(request.get("compute_type", "int8")).strip() or "int8"
    language = str(request.get("language", "en")).strip() or "en"
    warmup = bool(request.get("warmup", False))

    started_at = perf_counter()
    model = runtime.get_model(model_name, device, compute_type)
    load_ms = int((perf_counter() - started_at) * 1000)

    warmup_ms = 0
    if warmup:
        warmup_started_at = perf_counter()
        run_warmup_inference(model, language)
        warmup_ms = int((perf_counter() - warmup_started_at) * 1000)

    write_response(
        {
            "id": request_id,
            "ok": True,
            "model": model_name,
            "device": device,
            "compute_type": compute_type,
            "load_ms": load_ms,
            "warmup_ms": warmup_ms,
        }
    )


def run_warmup_inference(model, language: str):
    warmup_samples = 16000
    handle, warmup_path = tempfile.mkstemp(prefix="sonora-fw-warmup-", suffix=".wav")
    os.close(handle)
    try:
        with wave.open(warmup_path, "wb") as writer:
            writer.setnchannels(1)
            writer.setsampwidth(2)
            writer.setframerate(16000)
            writer.writeframes(b"\x00\x00" * warmup_samples)

        segments, _ = model.transcribe(
            warmup_path,
            language=language,
            beam_size=1,
            condition_on_previous_text=True,
            vad_filter=False,
        )
        for _ in segments:
            pass
    finally:
        try:
            os.remove(warmup_path)
        except OSError:
            pass


def handle_ping(request: dict):
    request_id = str(request.get("id", ""))
    write_response({"id": request_id, "ok": True, "pong": True})


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

        op = str(request.get("op", "")).strip().lower()
        try:
            if op == "transcribe":
                handle_transcribe(runtime, request)
            elif op == "preload":
                handle_preload(runtime, request)
            elif op == "ping":
                handle_ping(request)
            else:
                write_response(
                    {
                        "id": str(request.get("id", "")),
                        "ok": False,
                        "error": f"unsupported op: {op}",
                    }
                )
        except Exception as error:
            write_response(
                {
                    "id": str(request.get("id", "")),
                    "ok": False,
                    "error": str(error),
                }
            )


if __name__ == "__main__":
    main()
