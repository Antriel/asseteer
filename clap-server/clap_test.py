#!/usr/bin/env python3
"""
CLAP (Contrastive Language-Audio Pretraining) Prototype
Tests laion/clap-htsat-fused model for SFX audio search

Usage:
    python clap_test.py <audio_file.wav> "text query"
    python clap_test.py --batch <audio_dir> --queries "query1" "query2" ...
"""

import json
import argparse
from pathlib import Path
from typing import List, Dict, Tuple
import numpy as np

import torch
from transformers import ClapModel, ClapProcessor
import librosa


class ClapTester:
    """Wrapper for CLAP model testing and embedding generation"""

    def __init__(self, model_name: str = "laion/clap-htsat-fused"):
        """Load pretrained CLAP model and processor"""
        print(f"Loading CLAP model: {model_name}")
        self.device = "cuda" if torch.cuda.is_available() else "cpu"
        print(f"Using device: {self.device}")

        self.model = ClapModel.from_pretrained(model_name).to(self.device)
        self.processor = ClapProcessor.from_pretrained(model_name)
        self.model.eval()  # Set to evaluation mode

        print(f"OK: Model loaded successfully")
        print(f"  - Audio embedding dim: {self.model.config.projection_dim}")
        print(f"  - Text embedding dim: {self.model.config.projection_dim}")

    def load_audio(self, audio_path: str, duration: float = None) -> np.ndarray:
        """
        Load audio file and resample to model's expected sample rate

        Args:
            audio_path: Path to audio file
            duration: Optional duration to load (in seconds)

        Returns:
            Audio waveform as numpy array
        """
        target_sr = self.processor.feature_extractor.sampling_rate

        # Load audio with librosa (handles multiple formats)
        audio, sr = librosa.load(audio_path, sr=target_sr, duration=duration, mono=True)

        return audio

    def encode_audio(self, audio: np.ndarray) -> np.ndarray:
        """
        Generate embedding for audio waveform

        Args:
            audio: Audio waveform (numpy array)

        Returns:
            Normalized embedding vector (512-dim)
        """
        # Preprocess audio (mel spectrogram, normalization)
        inputs = self.processor(
            audios=audio,
            sampling_rate=self.processor.feature_extractor.sampling_rate,
            return_tensors="pt"
        )

        # Move to device
        inputs = {k: v.to(self.device) for k, v in inputs.items()}

        # Generate embedding
        with torch.no_grad():
            audio_embeds = self.model.get_audio_features(**inputs)

        # Convert to numpy and normalize
        embedding = audio_embeds.cpu().numpy()[0]
        embedding = embedding / np.linalg.norm(embedding)  # L2 normalize

        return embedding

    def encode_text(self, text: str) -> np.ndarray:
        """
        Generate embedding for text query

        Args:
            text: Text query string

        Returns:
            Normalized embedding vector (512-dim)
        """
        # Tokenize text
        inputs = self.processor(
            text=[text],
            return_tensors="pt",
            padding=True
        )

        # Move to device
        inputs = {k: v.to(self.device) for k, v in inputs.items()}

        # Generate embedding
        with torch.no_grad():
            text_embeds = self.model.get_text_features(**inputs)

        # Convert to numpy and normalize
        embedding = text_embeds.cpu().numpy()[0]
        embedding = embedding / np.linalg.norm(embedding)  # L2 normalize

        return embedding

    def compute_similarity(self, embedding1: np.ndarray, embedding2: np.ndarray) -> float:
        """
        Compute cosine similarity between two embeddings

        Args:
            embedding1: First embedding vector
            embedding2: Second embedding vector

        Returns:
            Similarity score (0-1 range, higher is more similar)
        """
        # Cosine similarity (embeddings already normalized)
        similarity = np.dot(embedding1, embedding2)
        return float(similarity)

    def test_audio_text_pair(self, audio_path: str, text_query: str) -> Dict:
        """
        Test a single audio-text pair and return results

        Args:
            audio_path: Path to audio file
            text_query: Text query to compare

        Returns:
            Dictionary with embeddings and similarity score
        """
        # Load and encode audio
        audio = self.load_audio(audio_path)
        audio_embedding = self.encode_audio(audio)

        # Encode text
        print(f"\nEncoding text query: '{text_query}'")
        text_embedding = self.encode_text(text_query)

        # Compute similarity
        similarity = self.compute_similarity(audio_embedding, text_embedding)

        return {
            "audio_path": str(audio_path),
            "text_query": text_query,
            "audio_embedding": audio_embedding.tolist(),
            "text_embedding": text_embedding.tolist(),
            "similarity": similarity
        }


def main():
    parser = argparse.ArgumentParser(description="Test CLAP model on audio files")
    parser.add_argument("audio", help="Path to audio file or directory")
    parser.add_argument("queries", nargs="+", help="Text queries to test")
    parser.add_argument("--model", default="laion/clap-htsat-fused", help="Model name")
    parser.add_argument("--output", "-o", help="Output JSON file for embeddings")
    parser.add_argument("--duration", type=float, help="Max audio duration (seconds)")

    args = parser.parse_args()

    # Initialize tester
    tester = ClapTester(model_name=args.model)

    # Get audio files
    audio_path = Path(args.audio)
    if audio_path.is_file():
        audio_files = [audio_path]
    elif audio_path.is_dir():
        # Find all audio files in directory
        audio_exts = [".wav", ".mp3", ".flac", ".ogg", ".m4a"]
        audio_files = []
        for ext in audio_exts:
            audio_files.extend(audio_path.glob(f"*{ext}"))
    else:
        print(f"Error: {audio_path} not found")
        return

    print(f"\nFound {len(audio_files)} audio file(s)")
    print(f"Testing with {len(args.queries)} text query/queries\n")
    print("=" * 80)

    # Test all combinations
    results = []
    for audio_file in audio_files:
        for query in args.queries:
            print(f"\n{'=' * 80}")
            print(f"Audio: {audio_file.name}")
            print(f"Query: '{query}'")
            print("=" * 80)

            result = tester.test_audio_text_pair(str(audio_file), query)
            results.append(result)

            print(f"\nOK: Similarity: {result['similarity']:.4f}")

    # Print summary
    print(f"\n{'=' * 80}")
    print("SUMMARY")
    print("=" * 80)
    for result in results:
        audio_name = Path(result["audio_path"]).name
        print(f"{audio_name:40s} | {result['text_query']:30s} | {result['similarity']:.4f}")

    # Save embeddings to JSON if requested
    if args.output:
        output_path = Path(args.output)
        print(f"\nSaving embeddings to: {output_path}")
        with open(output_path, "w") as f:
            json.dump(results, f, indent=2)
        print("OK: Saved")

    print("\nOK: Done!")


if __name__ == "__main__":
    main()
