# CLAP Python Server

HTTP server for CLAP audio-text embeddings, used by Asseteer for semantic audio search.

Uses `laion/clap-htsat-fused` model.

## Setup

```bash
# Create virtual environment
python -m venv venv

# Activate
venv\Scripts\activate      # Windows
source venv/bin/activate   # Mac/Linux

# Install dependencies
pip install -r requirements.txt
```

## HTTP Server

### Start

```bash
python clap_server.py

# Custom port
python clap_server.py --port 8080
```

### Endpoints

**Health Check**
```
GET /health

Response: { "status": "ok", "model": "...", "device": "...", "embedding_dim": 512 }
```

**Text Embedding**
```
POST /embed/text
Content-Type: application/json

{ "text": "footsteps on wood" }

Response: { "embedding": [0.123, -0.456, ...] }  // 512 floats
```

**Audio Embedding (File Path)**
```
POST /embed/audio
Content-Type: application/json

{ "audio_path": "C:/path/to/audio.wav" }

Response: { "embedding": [0.123, -0.456, ...] }
```

**Audio Embedding (Binary Upload)**

For audio from ZIP archives or in-memory sources:

```
POST /embed/audio/upload
Content-Type: multipart/form-data
Form field: audio (binary file)

Response: { "embedding": [0.123, -0.456, ...] }
```

## CLI Tool

For testing individual files:

```bash
# Single file + query
python clap_test.py audio.wav "footsteps on wood"

# Multiple queries
python clap_test.py audio.wav "footsteps" "walking" "explosion"

# Directory of files
python clap_test.py ./sounds/ "gunshot" "explosion"
```
