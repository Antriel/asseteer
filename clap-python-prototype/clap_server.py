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
async def embed_text(request: TextRequest):
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
async def embed_audio(request: AudioPathRequest):
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

    logger.info(f"Encoding uploaded audio: {audio.filename} ({len(content)} bytes)")

    # Load audio from bytes
    target_sr = clap_model.processor.feature_extractor.sampling_rate
    audio_data, sr = librosa.load(io.BytesIO(content), sr=target_sr, mono=True)
    logger.info(f"Loaded {len(audio_data) / sr:.2f} seconds of audio")

    embedding = clap_model.encode_audio(audio_data)
    logger.info(f"Audio embedding generated (dim: {len(embedding)})")

    return EmbeddingResponse(embedding=embedding.tolist())


if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="127.0.0.1", port=5555)
