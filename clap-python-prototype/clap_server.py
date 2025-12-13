#!/usr/bin/env python3
"""
CLAP HTTP Server

Provides HTTP endpoints for generating CLAP embeddings.
Used by Asseteer Tauri app for audio search functionality.

Endpoints:
    POST /embed/text   - Generate text embedding
    POST /embed/audio  - Generate audio embedding from file path
    GET  /health       - Health check
"""

import sys
import logging
from pathlib import Path
from flask import Flask, request, jsonify

# Import the existing ClapTester class
from clap_test import ClapTester

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

# Create Flask app
app = Flask(__name__)

# Global model instance (loaded once at startup)
clap_model = None


def initialize_model():
    """Load CLAP model at startup"""
    global clap_model

    logger.info("Initializing CLAP model...")
    try:
        clap_model = ClapTester(model_name="laion/clap-htsat-fused")
        logger.info("✓ CLAP model loaded successfully")
        logger.info(f"  - Device: {clap_model.device}")
        logger.info(f"  - Embedding dimension: {clap_model.model.config.projection_dim}")
    except Exception as e:
        logger.error(f"Failed to load CLAP model: {e}")
        sys.exit(1)


@app.route('/health', methods=['GET'])
def health():
    """Health check endpoint"""
    return jsonify({
        'status': 'ok',
        'model': 'laion/clap-htsat-fused',
        'device': clap_model.device if clap_model else 'unknown',
        'embedding_dim': clap_model.model.config.projection_dim if clap_model else 512
    })


@app.route('/embed/text', methods=['POST'])
def embed_text():
    """
    Generate text embedding

    Request body:
        {
            "text": "footsteps on wood"
        }

    Response:
        {
            "embedding": [0.123, -0.456, ...]  // 512-dim array
        }
    """
    try:
        # Parse request
        data = request.get_json()
        if not data or 'text' not in data:
            return jsonify({'error': 'Missing "text" field in request body'}), 400

        text = data['text']
        if not text or not text.strip():
            return jsonify({'error': '"text" field cannot be empty'}), 400

        logger.info(f"Encoding text: '{text}'")

        # Generate embedding
        embedding = clap_model.encode_text(text)

        # Convert to list for JSON serialization
        embedding_list = embedding.tolist()

        logger.info(f"✓ Text embedding generated (dim: {len(embedding_list)})")

        return jsonify({
            'embedding': embedding_list
        })

    except Exception as e:
        logger.error(f"Error encoding text: {e}", exc_info=True)
        return jsonify({'error': str(e)}), 500


@app.route('/embed/audio', methods=['POST'])
def embed_audio():
    """
    Generate audio embedding from file path

    Request body:
        {
            "audio_path": "/path/to/audio.wav"
        }

    Response:
        {
            "embedding": [0.123, -0.456, ...]  // 512-dim array
        }
    """
    try:
        # Parse request
        data = request.get_json()
        if not data or 'audio_path' not in data:
            return jsonify({'error': 'Missing "audio_path" field in request body'}), 400

        audio_path = data['audio_path']
        if not audio_path:
            return jsonify({'error': '"audio_path" field cannot be empty'}), 400

        # Check file exists
        audio_file = Path(audio_path)
        if not audio_file.exists():
            return jsonify({'error': f'Audio file not found: {audio_path}'}), 404

        logger.info(f"Encoding audio: {audio_path}")

        # Load and encode audio
        audio = clap_model.load_audio(str(audio_path))
        embedding = clap_model.encode_audio(audio)

        # Convert to list for JSON serialization
        embedding_list = embedding.tolist()

        logger.info(f"✓ Audio embedding generated (dim: {len(embedding_list)})")

        return jsonify({
            'embedding': embedding_list
        })

    except Exception as e:
        logger.error(f"Error encoding audio: {e}", exc_info=True)
        return jsonify({'error': str(e)}), 500


@app.route('/embed/audio/upload', methods=['POST'])
def embed_audio_upload():
    """
    Generate audio embedding from uploaded binary data

    Use this endpoint for audio files from zip archives or in-memory sources.
    Accepts multipart/form-data with binary audio file.

    Request:
        Content-Type: multipart/form-data
        Field name: 'audio'
        File data: Raw audio bytes (WAV, MP3, FLAC, etc.)

    Response:
        {
            "embedding": [0.123, -0.456, ...]  // 512-dim array
        }
    """
    try:
        # Check if audio file was provided
        if 'audio' not in request.files:
            return jsonify({'error': 'Missing "audio" file in multipart form data'}), 400

        audio_file = request.files['audio']

        if audio_file.filename == '':
            return jsonify({'error': 'Empty filename'}), 400

        logger.info(f"Encoding uploaded audio: {audio_file.filename}")

        # Load audio directly from file-like object (no temp file needed!)
        # librosa.load() accepts file-like objects
        target_sr = clap_model.processor.feature_extractor.sampling_rate

        import librosa
        audio, sr = librosa.load(
            audio_file,  # File-like object from Flask
            sr=target_sr,
            mono=True
        )

        logger.info(f"  - Loaded {len(audio) / sr:.2f} seconds")

        # Encode audio
        embedding = clap_model.encode_audio(audio)

        # Convert to list for JSON serialization
        embedding_list = embedding.tolist()

        logger.info(f"✓ Audio embedding generated (dim: {len(embedding_list)})")

        return jsonify({
            'embedding': embedding_list
        })

    except Exception as e:
        logger.error(f"Error encoding uploaded audio: {e}", exc_info=True)
        return jsonify({'error': str(e)}), 500


@app.errorhandler(404)
def not_found(error):
    """Handle 404 errors"""
    return jsonify({'error': 'Endpoint not found'}), 404


@app.errorhandler(500)
def internal_error(error):
    """Handle 500 errors"""
    return jsonify({'error': 'Internal server error'}), 500


def main():
    """Start the HTTP server"""
    import argparse

    parser = argparse.ArgumentParser(description='CLAP HTTP Server')
    parser.add_argument('--host', default='127.0.0.1', help='Host to bind to')
    parser.add_argument('--port', type=int, default=5555, help='Port to bind to')
    parser.add_argument('--debug', action='store_true', help='Enable debug mode')

    args = parser.parse_args()

    # Initialize model
    initialize_model()

    # Start server
    logger.info(f"Starting CLAP server on {args.host}:{args.port}")
    logger.info(f"Health check: http://{args.host}:{args.port}/health")

    app.run(
        host=args.host,
        port=args.port,
        debug=args.debug,
        threaded=True  # Handle multiple requests concurrently
    )


if __name__ == '__main__':
    main()
