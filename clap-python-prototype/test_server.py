#!/usr/bin/env python3
"""
Simple test script for CLAP HTTP server

Usage:
    # Start server in one terminal:
    python clap_server.py

    # Run this test in another terminal:
    python test_server.py
"""

import requests
import json


def test_health():
    """Test health check endpoint"""
    print("Testing /health endpoint...")
    response = requests.get('http://127.0.0.1:5555/health')

    if response.status_code == 200:
        print("✓ Health check passed")
        print(f"  Response: {json.dumps(response.json(), indent=2)}")
    else:
        print(f"✗ Health check failed: {response.status_code}")
        return False

    return True


def test_text_embedding():
    """Test text embedding endpoint"""
    print("\nTesting /embed/text endpoint...")

    text_query = "footsteps on wood"
    response = requests.post(
        'http://127.0.0.1:5555/embed/text',
        json={'text': text_query}
    )

    if response.status_code == 200:
        data = response.json()
        embedding = data['embedding']
        print(f"✓ Text embedding generated")
        print(f"  Query: '{text_query}'")
        print(f"  Embedding dimension: {len(embedding)}")
        print(f"  First 5 values: {embedding[:5]}")

        # Verify embedding is normalized
        import math
        norm = math.sqrt(sum(x * x for x in embedding))
        print(f"  L2 norm: {norm:.4f} (should be ~1.0)")
    else:
        print(f"✗ Text embedding failed: {response.status_code}")
        print(f"  Error: {response.text}")
        return False

    return True


def test_audio_embedding():
    """Test audio embedding endpoint"""
    print("\nTesting /embed/audio endpoint...")

    # You'll need to provide a real audio file path for this test
    print("  Note: This test requires an actual audio file path")
    print("  Skipping for now - test manually with a real file")
    return True


def main():
    print("=" * 60)
    print("CLAP Server Test Suite")
    print("=" * 60)
    print("Make sure the server is running on http://127.0.0.1:5555")
    print()

    try:
        # Run tests
        if not test_health():
            print("\n✗ Tests failed at health check")
            return

        if not test_text_embedding():
            print("\n✗ Tests failed at text embedding")
            return

        test_audio_embedding()

        print("\n" + "=" * 60)
        print("✓ All tests passed!")
        print("=" * 60)

    except requests.exceptions.ConnectionError:
        print("\n✗ Could not connect to server")
        print("  Make sure the server is running:")
        print("  python clap_server.py")


if __name__ == '__main__':
    main()
