#!/usr/bin/env python3
"""
Export CLAP model to ONNX format for Rust inference

This exports the audio and text encoders separately as two ONNX files:
- clap_audio_encoder.onnx (audio → 512-dim embedding)
- clap_text_encoder.onnx (text → 512-dim embedding)

Usage:
    python export_to_onnx.py --model laion/clap-htsat-fused --output ./onnx_models/
"""

import argparse
from pathlib import Path
import torch
from transformers import ClapModel, ClapProcessor


def export_clap_to_onnx(model_name: str, output_dir: str):
    """Export CLAP model components to ONNX format"""

    output_path = Path(output_dir)
    output_path.mkdir(parents=True, exist_ok=True)

    print(f"Loading model: {model_name}")
    model = ClapModel.from_pretrained(model_name)
    processor = ClapProcessor.from_pretrained(model_name)
    model.eval()

    device = "cpu"  # Export on CPU for compatibility
    model = model.to(device)

    print(f"Model loaded successfully")
    print(f"  - Projection dimension: {model.config.projection_dim}")
    print(f"  - Audio config: {model.config.audio_config.hidden_size}D hidden")
    print(f"  - Text config: {model.config.text_config.hidden_size}D hidden")
    print()

    # ========================================================================
    # Export Audio Encoder
    # ========================================================================
    print("=" * 80)
    print("Exporting Audio Encoder")
    print("=" * 80)

    audio_output_path = output_path / "clap_audio_encoder.onnx"

    # Create dummy audio input by processing silence through the feature extractor
    # This ensures we get the exact shape the model expects
    sample_rate = processor.feature_extractor.sampling_rate
    duration = 10  # seconds
    dummy_audio_waveform = torch.zeros(1, sample_rate * duration)  # Silence

    print(f"Creating dummy audio input via feature extractor...")
    print(f"  - Sample rate: {sample_rate} Hz")
    print(f"  - Duration: {duration} seconds")

    # Process through feature extractor to get mel spectrogram
    inputs = processor(
        audio=dummy_audio_waveform.numpy()[0],
        sampling_rate=sample_rate,
        return_tensors="pt"
    )
    dummy_audio_input = inputs["input_features"].to(device)
    print(f"  - Mel spectrogram shape: {dummy_audio_input.shape}")

    # Test forward pass using the full audio model
    print("Testing forward pass...")
    with torch.no_grad():
        test_output = model.get_audio_features(**inputs)
    print(f"  ✓ Output shape: {test_output.shape}")

    # Create wrapper model that uses the full audio model forward
    class AudioEncoderWithProjection(torch.nn.Module):
        def __init__(self, clap_audio_model):
            super().__init__()
            self.audio_model = clap_audio_model

        def forward(self, input_features, is_longer=None):
            # Use the audio model's forward method which handles fusion
            audio_embeds = self.audio_model(input_features=input_features, is_longer=is_longer)
            # Already includes projection and normalization
            return audio_embeds

    audio_encoder = AudioEncoderWithProjection(model.audio_model)
    audio_encoder.eval()

    print(f"Exporting to: {audio_output_path}")
    print("  - Using legacy ONNX exporter for compatibility...")
    torch.onnx.export(
        audio_encoder,
        (dummy_audio_input,),  # Tuple of inputs (input_features,)
        audio_output_path,
        input_names=["input_features"],
        output_names=["audio_embedding"],
        dynamic_axes={
            "input_features": {0: "batch_size", 1: "num_audio_clips", 2: "time_steps"},
            "audio_embedding": {0: "batch_size"}
        },
        opset_version=14,
        do_constant_folding=True,
        verbose=False,
        dynamo=False  # Use legacy exporter for stability
    )

    print(f"  ✓ Audio encoder exported")
    print(f"  - Size: {audio_output_path.stat().st_size / 1024 / 1024:.1f} MB")
    print()

    # ========================================================================
    # Export Text Encoder
    # ========================================================================
    print("=" * 80)
    print("Exporting Text Encoder")
    print("=" * 80)

    text_output_path = output_path / "clap_text_encoder.onnx"

    # Create dummy text input
    # Shape: (batch_size, sequence_length)
    max_length = model.config.text_config.max_position_embeddings  # 77

    print(f"Creating dummy text input: (1, {max_length})")
    dummy_text_input = torch.randint(0, 1000, (1, max_length), device=device)
    dummy_attention_mask = torch.ones(1, max_length, dtype=torch.long, device=device)

    # Test forward pass
    print("Testing forward pass...")
    with torch.no_grad():
        test_output = model.text_model.text_encoder(
            input_ids=dummy_text_input,
            attention_mask=dummy_attention_mask
        )
        text_projection = model.text_model.text_projection(test_output)
    print(f"  ✓ Output shape: {text_projection.shape}")

    # Create wrapper model that includes projection
    class TextEncoderWithProjection(torch.nn.Module):
        def __init__(self, text_model):
            super().__init__()
            self.text_encoder = text_model.text_encoder
            self.text_projection = text_model.text_projection

        def forward(self, input_ids, attention_mask):
            # Encode text
            text_features = self.text_encoder(
                input_ids=input_ids,
                attention_mask=attention_mask
            )
            # Project to embedding space
            text_embeds = self.text_projection(text_features)
            # L2 normalize
            text_embeds = text_embeds / text_embeds.norm(p=2, dim=-1, keepdim=True)
            return text_embeds

    text_encoder = TextEncoderWithProjection(model.text_model)
    text_encoder.eval()

    print(f"Exporting to: {text_output_path}")
    print("  - Using legacy ONNX exporter for compatibility...")
    torch.onnx.export(
        text_encoder,
        (dummy_text_input, dummy_attention_mask),
        text_output_path,
        input_names=["input_ids", "attention_mask"],
        output_names=["text_embedding"],
        dynamic_axes={
            "input_ids": {0: "batch_size", 1: "sequence_length"},
            "attention_mask": {0: "batch_size", 1: "sequence_length"},
            "text_embedding": {0: "batch_size"}
        },
        opset_version=14,
        do_constant_folding=True,
        verbose=False,
        dynamo=False  # Use legacy exporter for stability
    )

    print(f"  ✓ Text encoder exported")
    print(f"  - Size: {text_output_path.stat().st_size / 1024 / 1024:.1f} MB")
    print()

    # ========================================================================
    # Summary
    # ========================================================================
    print("=" * 80)
    print("Export Complete!")
    print("=" * 80)
    print(f"Output directory: {output_path.absolute()}")
    print()
    print("Files created:")
    print(f"  - {audio_output_path.name} ({audio_output_path.stat().st_size / 1024 / 1024:.1f} MB)")
    print(f"  - {text_output_path.name} ({text_output_path.stat().st_size / 1024 / 1024:.1f} MB)")
    print()
    print("Total size:", (audio_output_path.stat().st_size + text_output_path.stat().st_size) / 1024 / 1024, "MB")
    print()
    print("Next steps:")
    print("  1. Integrate ONNX Runtime in Rust")
    print("  2. Load these models with ort crate")
    print("  3. Validate embeddings match Python implementation")


def main():
    parser = argparse.ArgumentParser(description="Export CLAP model to ONNX")
    parser.add_argument("--model", default="laion/clap-htsat-fused", help="Model name")
    parser.add_argument("--output", default="./onnx_models", help="Output directory")

    args = parser.parse_args()

    export_clap_to_onnx(args.model, args.output)


if __name__ == "__main__":
    main()
