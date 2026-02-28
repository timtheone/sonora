#!/usr/bin/env python3
# pyright: reportMissingImports=false

import json
import os
import sys
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

    started_at = perf_counter()
    model = runtime.get_model(model_name, device, compute_type)
    segments, _info = model.transcribe(
        audio_path,
        language=language,
        beam_size=beam_size,
        condition_on_previous_text=False,
        without_timestamps=True,
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

    started_at = perf_counter()
    runtime.get_model(model_name, device, compute_type)
    duration_ms = int((perf_counter() - started_at) * 1000)
    write_response(
        {
            "id": request_id,
            "ok": True,
            "model": model_name,
            "device": device,
            "compute_type": compute_type,
            "load_ms": duration_ms,
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

        op = str(request.get("op", "")).strip().lower()
        try:
            if op == "transcribe":
                handle_transcribe(runtime, request)
            elif op == "preload":
                handle_preload(runtime, request)
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
