#!/usr/bin/env python3
"""
CLAP HTTP Server (FastAPI)

Provides HTTP endpoints for generating CLAP embeddings.
Used by Asseteer Tauri app for audio search functionality.

Endpoints:
    POST /embed/text         - Generate text embedding
    POST /embed/audio        - Generate audio embedding from file path
    POST /embed/audio/upload - Generate audio embedding from raw bytes
    GET  /health             - Health check

Run with:
    python clap_server.py
    # or
    uvicorn clap_server:app --host 127.0.0.1 --port 5555
"""

import io
import logging
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
    path = Path(request.audio_path)
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


def _process_audio_bytes(content: bytes, filename: str) -> list[float]:
    """Process audio bytes into embedding (blocking, runs in thread pool)."""
    logger.info(f"Encoding uploaded audio: {filename} ({len(content)} bytes)")

    target_sr = clap_model.processor.feature_extractor.sampling_rate
    audio_data, sr = librosa.load(io.BytesIO(content), sr=target_sr, mono=True)
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
        path = Path(audio_path)
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
        audios=audio_arrays,
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
            audio_data, sr = librosa.load(io.BytesIO(content), sr=target_sr, mono=True)
            loaded.append((i, audio_data))
        except Exception as e:
            results[i] = BatchEmbeddingItem(path=filename, error=str(e))

    if not loaded:
        return BatchEmbeddingResponse(results=[r for r in results if r is not None])

    # Batch inference
    audio_arrays = [audio for _, audio in loaded]
    inputs = clap_model.processor(
        audios=audio_arrays,
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
