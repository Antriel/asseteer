# CLAP Python Prototype

Provides two modes:
1. **CLI Tool** (`clap_test.py`) - For testing and validation
2. **HTTP Server** (`clap_server.py`) - For production use with Asseteer

Uses `laion/clap-htsat-fused` model for audio-text similarity search.

## Setup

```bash
# Create virtual environment (isolates from global Python)
python -m venv venv

# Activate virtual environment
# Windows:
venv\Scripts\activate
# Mac/Linux:
source venv/bin/activate

# Install dependencies
pip install -r requirements.txt
```

## Usage

### Single Audio File + Single Query

```bash
python clap_test.py path/to/audio.wav "footsteps on wood"
```

### Single Audio File + Multiple Queries

```bash
python clap_test.py audio.wav "footsteps" "walking" "wood floor" "creaking"
```

### Directory of Audio Files + Multiple Queries

```bash
python clap_test.py ./sfx_samples/ "explosion" "gunshot" "footsteps"
```

### Save Embeddings for Rust Comparison

```bash
python clap_test.py audio.wav "footsteps" --output embeddings.json
```

This will save:
- Audio embeddings (512-dim vectors)
- Text embeddings (512-dim vectors)
- Similarity scores

The JSON output can be used to validate the Rust implementation later.

## Example Output

```
Loading CLAP model: laion/clap-htsat-fused
Using device: cpu
✓ Model loaded successfully
  - Audio embedding dim: 512
  - Text embedding dim: 512

Found 1 audio file(s)
Testing with 2 text query/queries

================================================================================
Audio: footstep.wav
Query: 'footsteps on wood'
================================================================================
Loading audio: footstep.wav
  - Target sample rate: 48000 Hz
  - Loaded 2.50 seconds

Encoding text query: 'footsteps on wood'

✓ Similarity: 0.8234

================================================================================
Audio: footstep.wav
Query: 'explosion'
================================================================================
Loading audio: footstep.wav
  - Target sample rate: 48000 Hz
  - Loaded 2.50 seconds

Encoding text query: 'explosion'

✓ Similarity: 0.1523

================================================================================
SUMMARY
================================================================================
footstep.wav                             | footsteps on wood              | 0.8234
footstep.wav                             | explosion                      | 0.1523

✓ Done!
```

## What This Tests

1. **Model Loading**: Can we load pretrained weights from HuggingFace?
2. **Audio Processing**: Does the audio preprocessing work correctly?
3. **Embedding Quality**: Do similar concepts get high similarity scores?
4. **Reference Embeddings**: Generate ground truth for Rust validation

## HTTP Server (Production Use)

### Start the Server

```bash
# Activate venv first!
venv\Scripts\activate  # Windows
# source venv/bin/activate  # Mac/Linux

# Start server on default port (5555)
python clap_server.py

# Custom port
python clap_server.py --port 8080

# Enable debug mode (auto-reload on code changes)
python clap_server.py --debug
```

### Test the Server

```bash
# In another terminal (with venv activated)
python test_server.py
```

### API Endpoints

**Health Check**
```bash
GET http://127.0.0.1:5555/health

Response:
{
  "status": "ok",
  "model": "laion/clap-htsat-fused",
  "device": "cpu",
  "embedding_dim": 512
}
```

**Text Embedding**
```bash
POST http://127.0.0.1:5555/embed/text
Content-Type: application/json

{
  "text": "footsteps on wood"
}

Response:
{
  "embedding": [0.123, -0.456, ...]  // 512 floats
}
```

**Audio Embedding (File Path)**
```bash
POST http://127.0.0.1:5555/embed/audio
Content-Type: application/json

{
  "audio_path": "C:/path/to/audio.wav"
}

Response:
{
  "embedding": [0.123, -0.456, ...]  // 512 floats
}
```

**Audio Embedding (Binary Upload)**

Use this for audio files from zip archives or in-memory sources:

```bash
POST http://127.0.0.1:5555/embed/audio/upload
Content-Type: multipart/form-data

Form field: audio (binary file data)

Response:
{
  "embedding": [0.123, -0.456, ...]  // 512 floats
}
```

Example with curl:
```bash
curl -X POST http://127.0.0.1:5555/embed/audio/upload \
  -F "audio=@path/to/audio.wav"
```

## Next Steps

1. ✅ HTTP server implementation complete
2. Test with real audio files from your library
3. Integrate with Rust backend (HTTP client)
4. Add database storage for embeddings
