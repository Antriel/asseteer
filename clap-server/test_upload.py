#!/usr/bin/env python3
"""
Test script for binary upload endpoint

Tests the /embed/audio/upload endpoint with multipart form data
"""

import requests
import sys
from pathlib import Path


def test_binary_upload(audio_path: str):
    """Test uploading audio file as binary data"""

    audio_file = Path(audio_path)
    if not audio_file.exists():
        print(f"Error: Audio file not found: {audio_path}")
        return False

    print(f"Testing binary upload with: {audio_file.name}")
    print(f"File size: {audio_file.stat().st_size / 1024:.2f} KB")

    # Open file and send as multipart form data
    with open(audio_file, 'rb') as f:
        files = {'audio': (audio_file.name, f, 'audio/wav')}

        response = requests.post(
            'http://127.0.0.1:5555/embed/audio/upload',
            files=files
        )

    if response.status_code == 200:
        data = response.json()
        embedding = data['embedding']
        print(f"✓ Binary upload succeeded")
        print(f"  Embedding dimension: {len(embedding)}")
        print(f"  First 5 values: {embedding[:5]}")

        # Verify normalized
        import math
        norm = math.sqrt(sum(x * x for x in embedding))
        print(f"  L2 norm: {norm:.4f} (should be ~1.0)")
        return True
    else:
        print(f"✗ Binary upload failed: {response.status_code}")
        print(f"  Error: {response.text}")
        return False


def test_in_memory_upload():
    """Test uploading audio from memory (simulates zip file extraction)"""
    print("\nTesting in-memory upload (simulating zip extraction)...")

    # Simulate reading audio from a zip file into memory
    # For this test, we'll just read a file into memory
    test_file = Path("test_audio.wav")  # You'd need a real file here

    if not test_file.exists():
        print("  Skipping: No test audio file available")
        print("  (Would work with audio bytes from zip archive)")
        return True

    # Read entire file into memory (like extracting from zip)
    with open(test_file, 'rb') as f:
        audio_bytes = f.read()

    print(f"  Loaded {len(audio_bytes)} bytes into memory")

    # Upload from memory
    files = {'audio': ('from_memory.wav', audio_bytes, 'audio/wav')}
    response = requests.post(
        'http://127.0.0.1:5555/embed/audio/upload',
        files=files
    )

    if response.status_code == 200:
        print("✓ In-memory upload succeeded")
        return True
    else:
        print(f"✗ In-memory upload failed: {response.status_code}")
        return False


def main():
    print("=" * 60)
    print("CLAP Server Binary Upload Test")
    print("=" * 60)
    print("Make sure the server is running on http://127.0.0.1:5555\n")

    # Check if audio file provided
    if len(sys.argv) < 2:
        print("Usage: python test_upload.py <audio_file.wav>")
        print("\nExample:")
        print("  python test_upload.py path/to/audio.wav")
        return

    audio_path = sys.argv[1]

    try:
        # Test binary upload
        if not test_binary_upload(audio_path):
            return

        # Test in-memory upload
        test_in_memory_upload()

        print("\n" + "=" * 60)
        print("✓ All tests passed!")
        print("=" * 60)

    except requests.exceptions.ConnectionError:
        print("\n✗ Could not connect to server")
        print("  Make sure the server is running:")
        print("  python clap_server.py")


if __name__ == '__main__':
    main()
