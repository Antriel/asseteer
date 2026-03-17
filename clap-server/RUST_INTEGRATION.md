# Rust Integration Guide

## Using the Binary Upload Endpoint

The `/embed/audio/upload` endpoint is perfect for files from zip archives since you can send raw bytes without saving to disk.

### Rust Dependencies

Add to `Cargo.toml`:
```toml
[dependencies]
reqwest = { version = "0.11", features = ["blocking", "multipart"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
zip = "0.6"  # For reading zip files
```

### Example: Embed Audio from Zip File

```rust
use reqwest::blocking::multipart;
use std::io::Read;
use std::path::Path;

#[derive(serde::Deserialize)]
struct EmbedResponse {
    embedding: Vec<f32>,
}

/// Generate embedding for audio file inside a zip archive
pub fn embed_audio_from_zip(
    zip_path: &Path,
    audio_filename: &str,
) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
    // 1. Open zip archive
    let file = std::fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    // 2. Find and read audio file
    let mut zip_file = archive.by_name(audio_filename)?;
    let mut audio_bytes = Vec::new();
    zip_file.read_to_end(&mut audio_bytes)?;

    println!("Read {} bytes from zip: {}", audio_bytes.len(), audio_filename);

    // 3. Create multipart form with audio bytes
    let part = multipart::Part::bytes(audio_bytes)
        .file_name(audio_filename.to_string())
        .mime_str("audio/wav")?;  // or detect from extension

    let form = multipart::Form::new()
        .part("audio", part);

    // 4. Send to Python server
    let client = reqwest::blocking::Client::new();
    let response = client
        .post("http://127.0.0.1:5555/embed/audio/upload")
        .multipart(form)
        .send()?;

    // 5. Parse response
    if !response.status().is_success() {
        let error_text = response.text()?;
        return Err(format!("Server error: {}", error_text).into());
    }

    let embed_response: EmbedResponse = response.json()?;

    println!("Generated embedding: {} dimensions", embed_response.embedding.len());
    Ok(embed_response.embedding)
}

/// Generate embedding for regular file (not in zip)
pub fn embed_audio_from_file(
    audio_path: &Path,
) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
    // Read file into memory
    let audio_bytes = std::fs::read(audio_path)?;

    // Create multipart form
    let part = multipart::Part::bytes(audio_bytes)
        .file_name(audio_path.file_name().unwrap().to_string_lossy().to_string())
        .mime_str("audio/wav")?;

    let form = multipart::Form::new()
        .part("audio", part);

    // Send to server
    let client = reqwest::blocking::Client::new();
    let response = client
        .post("http://127.0.0.1:5555/embed/audio/upload")
        .multipart(form)
        .send()?;

    let embed_response: EmbedResponse = response.json()?;
    Ok(embed_response.embedding)
}
```

### Usage Example

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Example 1: Audio from zip archive
    let embedding = embed_audio_from_zip(
        Path::new("sounds.zip"),
        "footsteps.wav"
    )?;

    println!("Embedding from zip: {:?}", &embedding[..5]);

    // Example 2: Regular audio file
    let embedding2 = embed_audio_from_file(
        Path::new("regular_audio.wav")
    )?;

    println!("Embedding from file: {:?}", &embedding2[..5]);

    Ok(())
}
```

### Integration with Asset Processing

```rust
// In your asset processing pipeline
async fn process_audio_asset(asset: &Asset) -> Result<Vec<f32>, String> {
    let embedding = if asset.is_in_zip {
        // Audio is inside a zip file
        embed_audio_from_zip(
            &asset.zip_path,
            &asset.filename_in_zip
        )?
    } else {
        // Regular file on disk
        embed_audio_from_file(&asset.file_path)?
    };

    // Store embedding in database
    store_embedding_in_db(asset.id, &embedding).await?;

    Ok(embedding)
}
```

### Performance Notes

- **No base64 overhead**: Binary multipart is ~33% more efficient than base64
- **No temp files needed**: Send bytes directly from memory
- **Works with any audio format**: Server uses librosa which supports WAV, MP3, FLAC, OGG, etc.
- **Typical latency**: 50-100ms for upload + embedding generation (localhost)

### Error Handling

```rust
match embed_audio_from_zip(zip_path, filename) {
    Ok(embedding) => {
        println!("✓ Embedding generated: {} dims", embedding.len());
    }
    Err(e) => {
        eprintln!("✗ Failed to generate embedding: {}", e);
        // Could be:
        // - Zip file not found
        // - Audio file not in zip
        // - Server not running
        // - Invalid audio format
        // - Server processing error
    }
}
```

### Detecting MIME Type from Extension

```rust
fn get_audio_mime_type(filename: &str) -> &'static str {
    match Path::new(filename)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .as_deref()
    {
        Some("wav") => "audio/wav",
        Some("mp3") => "audio/mpeg",
        Some("flac") => "audio/flac",
        Some("ogg") => "audio/ogg",
        Some("m4a") => "audio/mp4",
        Some("aac") => "audio/aac",
        _ => "application/octet-stream",
    }
}

// Use it:
let mime_type = get_audio_mime_type(audio_filename);
let part = multipart::Part::bytes(audio_bytes)
    .file_name(audio_filename.to_string())
    .mime_str(mime_type)?;
```
