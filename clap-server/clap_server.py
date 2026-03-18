#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11,<3.14"
# dependencies = [
#     "torch>=2.0.0",
#     "transformers>=4.30.0",
#     "librosa>=0.10.0",
#     "soundfile>=0.13.0",
#     "numpy>=1.24.0",
#     "fastapi>=0.109.0",
#     "uvicorn[standard]>=0.27.0",
#     "python-multipart>=0.0.6",
#     "miniaudio>=1.2",
#     "imageio-ffmpeg>=0.5.1",
# ]
# ///
"""
CLAP HTTP Server (FastAPI)

Provides HTTP endpoints for generating CLAP embeddings.
Used by Asseteer Tauri app for audio search functionality.

Endpoints:
    POST /embed/text         - Generate text embedding
    POST /embed/audio        - Generate audio embedding from file path
    POST /embed/audio/upload - Generate audio embedding from raw bytes
    POST /preload            - Trigger model download/loading
    GET  /health             - Health check

Run with:
    uv run clap_server.py
    # or
    uvicorn clap_server:app --host 127.0.0.1 --port 5555
"""

import io
import logging
import sys
from pathlib import Path
from contextlib import asynccontextmanager

import librosa
import numpy as np
from fastapi import FastAPI, HTTPException, UploadFile, File
from pydantic import BaseModel

from clap_test import ClapTester

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)


def _win_path(path_str: str) -> str:
    """On Windows, prefix with \\?\\ to support paths longer than MAX_PATH (260 chars)."""
    if sys.platform != 'win32' or path_str.startswith('\\\\?\\'):
        return path_str
    return '\\\\?\\' + path_str.replace('/', '\\')


# Global model instance (loaded once at startup)
clap_model: ClapTester | None = None


@asynccontextmanager
async def lifespan(app: FastAPI):
    """Load model on startup, cleanup on shutdown"""
    global clap_model
    logger.info("Loading CLAP model...")
    clap_model = ClapTester(model_name="laion/clap-htsat-fused")
    logger.info(f"Model loaded on {clap_model.device}")
    yield
    logger.info("Shutting down CLAP server...")


app = FastAPI(
    title="CLAP Embedding Server",
    description="HTTP API for CLAP audio-text embeddings",
    version="1.0.0",
    lifespan=lifespan
)


# Request/Response models
class TextRequest(BaseModel):
    text: str


class AudioPathRequest(BaseModel):
    audio_path: str


class EmbeddingResponse(BaseModel):
    embedding: list[float]


class BatchAudioPathRequest(BaseModel):
    audio_paths: list[str]


class BatchEmbeddingItem(BaseModel):
    path: str
    embedding: list[float] | None = None
    error: str | None = None


class BatchEmbeddingResponse(BaseModel):
    results: list[BatchEmbeddingItem]


class HealthResponse(BaseModel):
    status: str
    model: str
    device: str
    embedding_dim: int


@app.get("/health", response_model=HealthResponse)
async def health():
    """Health check endpoint"""
    return HealthResponse(
        status="ok",
        model="laion/clap-htsat-fused",
        device=clap_model.device if clap_model else "unknown",
        embedding_dim=512
    )


class PreloadResponse(BaseModel):
    status: str
    model: str
    device: str
    message: str


@app.post("/preload", response_model=PreloadResponse)
def preload():
    """
    Trigger model loading if not already loaded.

    Call this after the server starts to ensure the HuggingFace model
    is downloaded and loaded into memory before the first inference request.
    On first run this triggers a ~1-2GB model download from HuggingFace.
    """
    if clap_model is not None:
        return PreloadResponse(
            status="ready",
            model="laion/clap-htsat-fused",
            device=str(clap_model.device),
            message="Model already loaded",
        )
    # Model should be loaded by lifespan handler; if not, something went wrong
    raise HTTPException(status_code=503, detail="Model not loaded — server may still be starting")


@app.post("/embed/text", response_model=EmbeddingResponse)
def embed_text(request: TextRequest):
    """
    Generate text embedding.

    Request body:
        {"text": "footsteps on wood"}

    Response:
        {"embedding": [0.123, -0.456, ...]}  // 512-dim array
    """
    if not request.text.strip():
        raise HTTPException(status_code=400, detail="Text cannot be empty")

    logger.info(f"Encoding text: '{request.text}'")
    embedding = clap_model.encode_text(request.text)
    logger.info(f"Text embedding generated (dim: {len(embedding)})")

    return EmbeddingResponse(embedding=embedding.tolist())


@app.post("/embed/audio", response_model=EmbeddingResponse)
def embed_audio(request: AudioPathRequest):
    """
    Generate audio embedding from file path.

    Request body:
        {"audio_path": "/path/to/audio.wav"}

    Response:
        {"embedding": [0.123, -0.456, ...]}  // 512-dim array
    """
    path = Path(_win_path(request.audio_path))
    if not path.exists():
        raise HTTPException(status_code=404, detail=f"File not found: {request.audio_path}")

    logger.info(f"Encoding audio: {request.audio_path}")
    audio = clap_model.load_audio(str(path))
    embedding = clap_model.encode_audio(audio)
    logger.info(f"Audio embedding generated (dim: {len(embedding)})")

    return EmbeddingResponse(embedding=embedding.tolist())


@app.post("/embed/audio/upload", response_model=EmbeddingResponse)
async def embed_audio_upload(audio: UploadFile = File(...)):
    """
    Generate audio embedding from uploaded binary data.

    Use this endpoint for audio files from zip archives or in-memory sources.
    Accepts multipart/form-data with binary audio file.

    Request:
        Content-Type: multipart/form-data
        Field name: 'audio'
        File data: Raw audio bytes (WAV, MP3, FLAC, etc.)

    Response:
        {"embedding": [0.123, -0.456, ...]}  // 512-dim array
    """
    content = await audio.read()
    filename = audio.filename

    import asyncio
    embedding = await asyncio.get_event_loop().run_in_executor(
        None, _process_audio_bytes, content, filename
    )

    return EmbeddingResponse(embedding=embedding)


def _decode_audio_bytes(content: bytes, filename: str, target_sr: int) -> tuple:
    """Decode audio bytes to (samples, sample_rate).

    Tries four backends in order:
    1. soundfile via BytesIO (fast, handles WAV/FLAC/OGG/etc.)
    2. miniaudio (handles MP3 and other formats soundfile can't read from BytesIO)
    3. ffmpeg pipe via imageio-ffmpeg (handles M4A/AAC/anything ffmpeg supports)
    4. temp file + librosa (last resort for edge cases)
    """
    soundfile_err_msg = None
    miniaudio_err_msg = None
    ffmpeg_err_msg = None

    try:
        return librosa.load(io.BytesIO(content), sr=target_sr, mono=True)
    except Exception as e:
        soundfile_err_msg = str(e)
        logger.info(f"soundfile/BytesIO failed for '{filename}', trying miniaudio: {e}")

    try:
        import miniaudio
        decoded = miniaudio.decode(content, output_format=miniaudio.SampleFormat.FLOAT32, nchannels=1, sample_rate=target_sr)
        audio_data = np.frombuffer(decoded.samples, dtype=np.float32)
        return audio_data, target_sr
    except Exception as e:
        miniaudio_err_msg = str(e)
        logger.info(f"miniaudio failed for '{filename}', trying ffmpeg pipe: {e}")

    try:
        import subprocess
        import imageio_ffmpeg
        ffmpeg_exe = imageio_ffmpeg.get_ffmpeg_exe()
        cmd = [
            ffmpeg_exe, "-hide_banner", "-loglevel", "warning",
            "-i", "pipe:0",
            "-f", "f32le", "-acodec", "pcm_f32le",
            "-ar", str(target_sr), "-ac", "1",
            "-vn", "pipe:1",
        ]
        proc = subprocess.run(cmd, input=content, capture_output=True)
        stderr_out = proc.stderr.decode(errors='replace').strip()
        if proc.returncode != 0:
            raise RuntimeError(f"ffmpeg exited {proc.returncode}: {stderr_out}")
        audio_data = np.frombuffer(proc.stdout, dtype=np.float32)
        if len(audio_data) == 0:
            raise RuntimeError(f"ffmpeg produced no audio output (stderr: {stderr_out})")
        return audio_data, target_sr
    except Exception as e:
        ffmpeg_err_msg = str(e)
        logger.info(f"ffmpeg pipe failed for '{filename}', trying temp file: {e}")

    try:
        # MP4/M4A need seeking to read the moov atom — pipe stdin can't seek,
        # so write to a temp file and let ffmpeg read it by path.
        import os, tempfile
        import imageio_ffmpeg
        suffix = Path(filename).suffix or ".audio"
        with tempfile.NamedTemporaryFile(suffix=suffix, delete=False) as tf:
            tf.write(content)
            tmp_path = tf.name
        try:
            ffmpeg_exe = imageio_ffmpeg.get_ffmpeg_exe()
            cmd = [
                ffmpeg_exe, "-hide_banner", "-loglevel", "warning",
                "-i", tmp_path,
                "-f", "f32le", "-acodec", "pcm_f32le",
                "-ar", str(target_sr), "-ac", "1",
                "-vn", "pipe:1",
            ]
            proc = subprocess.run(cmd, capture_output=True)
            stderr_out = proc.stderr.decode(errors='replace').strip()
            if proc.returncode != 0:
                raise RuntimeError(f"ffmpeg exited {proc.returncode}: {stderr_out}")
            audio_data = np.frombuffer(proc.stdout, dtype=np.float32)
            if len(audio_data) == 0:
                raise RuntimeError(f"ffmpeg produced no audio output (stderr: {stderr_out})")
            return audio_data, target_sr
        finally:
            os.unlink(tmp_path)
    except Exception as e:
        ext = Path(filename).suffix.lower()
        raise RuntimeError(
            f"Failed to decode audio file '{filename}' ({ext}): all backends failed. "
            f"soundfile: {soundfile_err_msg}; miniaudio: {miniaudio_err_msg}; "
            f"ffmpeg pipe: {ffmpeg_err_msg}; ffmpeg file: {e}"
        ) from e


def _process_audio_bytes(content: bytes, filename: str) -> list[float]:
    """Process audio bytes into embedding (blocking, runs in thread pool)."""
    logger.info(f"Encoding uploaded audio: {filename} ({len(content)} bytes)")

    target_sr = clap_model.processor.feature_extractor.sampling_rate
    audio_data, sr = _decode_audio_bytes(content, filename, target_sr)
    logger.info(f"Loaded {len(audio_data) / sr:.2f} seconds of audio")

    embedding = clap_model.encode_audio(audio_data)
    logger.info(f"Audio embedding generated (dim: {len(embedding)})")

    return embedding.tolist()


@app.post("/embed/audio/batch", response_model=BatchEmbeddingResponse)
def embed_audio_batch(request: BatchAudioPathRequest):
    """
    Generate audio embeddings for multiple file paths in a single batched forward pass.

    Request body:
        {"audio_paths": ["/path/to/a.wav", "/path/to/b.wav", ...]}

    Response:
        {"results": [{"path": "...", "embedding": [...]} | {"path": "...", "error": "..."}]}
    """
    if not request.audio_paths:
        raise HTTPException(status_code=400, detail="audio_paths cannot be empty")

    logger.info(f"Batch encoding {len(request.audio_paths)} audio files")

    target_sr = clap_model.processor.feature_extractor.sampling_rate

    # Load all audio files, track which ones succeeded
    loaded = []  # (index, audio_array)
    results = [None] * len(request.audio_paths)

    for i, audio_path in enumerate(request.audio_paths):
        path = Path(_win_path(audio_path))
        if not path.exists():
            results[i] = BatchEmbeddingItem(path=audio_path, error=f"File not found: {audio_path}")
            continue
        try:
            audio_data, sr = librosa.load(str(path), sr=target_sr, mono=True)
            loaded.append((i, audio_data))
        except Exception as e:
            results[i] = BatchEmbeddingItem(path=audio_path, error=str(e))

    if not loaded:
        return BatchEmbeddingResponse(results=[r for r in results if r is not None])

    # Batch inference - pass all audio arrays to the processor at once
    audio_arrays = [audio for _, audio in loaded]
    inputs = clap_model.processor(
        audio=audio_arrays,
        sampling_rate=target_sr,
        return_tensors="pt",
        padding=True,
    )
    inputs = {k: v.to(clap_model.device) for k, v in inputs.items()}

    import torch
    with torch.no_grad():
        audio_embeds = clap_model.model.get_audio_features(**inputs)

    embeddings = audio_embeds.cpu().numpy()

    # Normalize and assign results
    for batch_idx, (orig_idx, _) in enumerate(loaded):
        emb = embeddings[batch_idx]
        emb = emb / np.linalg.norm(emb)
        results[orig_idx] = BatchEmbeddingItem(
            path=request.audio_paths[orig_idx],
            embedding=emb.tolist(),
        )

    logger.info(f"Batch embedding complete: {len(loaded)} succeeded, {len(request.audio_paths) - len(loaded)} failed")

    return BatchEmbeddingResponse(results=[r for r in results if r is not None])


@app.post("/embed/audio/batch/upload", response_model=BatchEmbeddingResponse)
async def embed_audio_batch_upload(files: list[UploadFile] = File(...)):
    """
    Generate audio embeddings for multiple uploaded files in a single batched forward pass.

    Request:
        Content-Type: multipart/form-data
        Multiple files with field name 'files'

    Response:
        {"results": [{"path": "filename", "embedding": [...]} | {"path": "filename", "error": "..."}]}
    """
    if not files:
        raise HTTPException(status_code=400, detail="No files provided")

    # Read all file contents while on the event loop (async I/O)
    file_data = []
    for f in files:
        content = await f.read()
        file_data.append((f.filename or f"file_{len(file_data)}", content))

    # Offload CPU-heavy work to thread pool
    import asyncio
    result = await asyncio.get_event_loop().run_in_executor(
        None, _process_batch_bytes, file_data
    )
    return result


def _process_batch_bytes(file_data: list[tuple[str, bytes]]) -> BatchEmbeddingResponse:
    """Process multiple audio byte buffers into embeddings (blocking)."""
    logger.info(f"Batch upload encoding {len(file_data)} audio files")

    target_sr = clap_model.processor.feature_extractor.sampling_rate

    loaded = []  # (index, audio_array)
    results = [None] * len(file_data)

    for i, (filename, content) in enumerate(file_data):
        try:
            audio_data, sr = _decode_audio_bytes(content, filename, target_sr)
            loaded.append((i, audio_data))
        except Exception as e:
            results[i] = BatchEmbeddingItem(path=filename, error=str(e))

    if not loaded:
        return BatchEmbeddingResponse(results=[r for r in results if r is not None])

    # Batch inference
    audio_arrays = [audio for _, audio in loaded]
    inputs = clap_model.processor(
        audio=audio_arrays,
        sampling_rate=target_sr,
        return_tensors="pt",
        padding=True,
    )
    inputs = {k: v.to(clap_model.device) for k, v in inputs.items()}

    import torch
    with torch.no_grad():
        audio_embeds = clap_model.model.get_audio_features(**inputs)

    embeddings = audio_embeds.cpu().numpy()

    for batch_idx, (orig_idx, _) in enumerate(loaded):
        emb = embeddings[batch_idx]
        emb = emb / np.linalg.norm(emb)
        results[orig_idx] = BatchEmbeddingItem(
            path=file_data[orig_idx][0],
            embedding=emb.tolist(),
        )

    logger.info(f"Batch upload complete: {len(loaded)} succeeded, {len(file_data) - len(loaded)} failed")

    return BatchEmbeddingResponse(results=[r for r in results if r is not None])


if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="127.0.0.1", port=5555)
