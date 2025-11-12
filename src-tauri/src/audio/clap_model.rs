use ort::{
    session::{Session, builder::GraphOptimizationLevel},
    value::Value,
    tensor::TensorElementType,
};
use ndarray::Array1;
use tokenizers::Tokenizer;
use std::path::Path;

/// CLAP (Contrastive Language-Audio Pretraining) model for audio-text embedding
pub struct ClapModel {
    audio_encoder: Session,
    text_encoder: Session,
    tokenizer: Tokenizer,
}

impl ClapModel {
    /// Load CLAP ONNX models and tokenizer from model directory
    ///
    /// Expected files:
    /// - `clap_audio_encoder.onnx` (~300MB)
    /// - `clap_text_encoder.onnx` (~330MB)
    /// - `tokenizer.json` (CLIP BPE tokenizer)
    pub fn new(model_dir: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let audio_path = model_dir.join("clap_audio_encoder.onnx");
        let text_path = model_dir.join("clap_text_encoder.onnx");
        let tokenizer_path = model_dir.join("tokenizer.json");

        // Check that all required files exist
        if !audio_path.exists() {
            return Err(format!("Audio encoder not found: {:?}", audio_path).into());
        }
        if !text_path.exists() {
            return Err(format!("Text encoder not found: {:?}", text_path).into());
        }
        if !tokenizer_path.exists() {
            return Err(format!("Tokenizer not found: {:?}", tokenizer_path).into());
        }

        // Load ONNX models with optimization
        let audio_encoder = Session::builder()?
            .with_optimization_level(GraphOptimizationLevel::Level3)?
            .with_intra_threads(2)?
            .commit_from_file(audio_path)?;

        let text_encoder = Session::builder()?
            .with_optimization_level(GraphOptimizationLevel::Level3)?
            .with_intra_threads(2)?
            .commit_from_file(text_path)?;

        // Load tokenizer
        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| format!("Failed to load tokenizer: {}", e))?;

        Ok(Self {
            audio_encoder,
            text_encoder,
            tokenizer,
        })
    }

    /// Generate 512-dimensional audio embedding from mel spectrogram
    ///
    /// Input: Mel spectrogram with shape [mels=64, time_frames]
    /// Output: 512-dimensional embedding vector
    pub fn encode_audio(&mut self, mel_spec: &ndarray::Array2<f32>) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
        let (n_mels, time_frames) = mel_spec.dim();

        // Convert to flat vec for ONNX input: [batch=1, mels=64, time_frames]
        let data: Vec<f32> = mel_spec.iter().cloned().collect();
        let shape = vec![1, n_mels, time_frames];

        // Create tensor from shape + data
        let input_value = Value::from_array((shape.as_slice(), data))?;
        let outputs = self.audio_encoder.run(ort::inputs![input_value])?;

        // Extract embedding (512-dim vector)
        let (_shape, data) = outputs[0].try_extract_tensor::<f32>()?;

        Ok(data.to_vec())
    }

    /// Generate 512-dimensional text embedding from query string
    ///
    /// Input: Text query (e.g., "footsteps on wood")
    /// Output: 512-dimensional embedding vector
    pub fn encode_text(&mut self, text: &str) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
        // Tokenize text using CLIP tokenizer
        let encoding = self.tokenizer.encode(text, false)
            .map_err(|e| format!("Tokenization failed: {}", e))?;
        let token_ids = encoding.get_ids();

        // Convert to i64 array and pad/truncate to 77 tokens (CLIP standard)
        let mut tokens = vec![0i64; 77];
        for (i, &id) in token_ids.iter().enumerate().take(77) {
            tokens[i] = id as i64;
        }

        // Create tensor from shape + data: [batch=1, seq_len=77]
        let shape = vec![1, 77];
        let input_value = Value::from_array((shape.as_slice(), tokens))?;
        let outputs = self.text_encoder.run(ort::inputs![input_value])?;

        // Extract embedding (512-dim vector)
        let (_shape, data) = outputs[0].try_extract_tensor::<f32>()?;

        Ok(data.to_vec())
    }

    /// Check if CLAP models exist at the given path
    pub fn models_exist(model_dir: &Path) -> bool {
        let audio_path = model_dir.join("clap_audio_encoder.onnx");
        let text_path = model_dir.join("clap_text_encoder.onnx");
        let tokenizer_path = model_dir.join("tokenizer.json");

        audio_path.exists() && text_path.exists() && tokenizer_path.exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn get_test_model_dir() -> Option<PathBuf> {
        // Check if test models exist in project directory
        let test_dirs = vec![
            PathBuf::from("test_models"),
            PathBuf::from("models/clap"),
            PathBuf::from("../models/clap"),
        ];

        for dir in test_dirs {
            if ClapModel::models_exist(&dir) {
                return Some(dir);
            }
        }

        None
    }

    #[test]
    fn test_models_exist_check() {
        // This test just checks the models_exist function works
        let fake_dir = Path::new("nonexistent_directory");
        assert!(!ClapModel::models_exist(fake_dir));
    }

    #[test]
    #[ignore] // Ignore by default - requires CLAP models to be downloaded
    fn test_clap_model_loading() {
        let model_dir = get_test_model_dir().expect("CLAP models not found for testing");
        let result = ClapModel::new(&model_dir);
        assert!(result.is_ok(), "Failed to load CLAP model: {:?}", result.err());
    }

    #[test]
    #[ignore] // Ignore by default - requires CLAP models to be downloaded
    fn test_text_encoding() {
        let model_dir = get_test_model_dir().expect("CLAP models not found for testing");
        let clap = ClapModel::new(&model_dir).expect("Failed to load model");

        let embedding = clap.encode_text("footsteps on wood").unwrap();

        assert_eq!(embedding.len(), 512, "Embedding should be 512-dimensional");
        assert!(embedding.iter().all(|&x| x.is_finite()), "Embedding should not contain NaN/Inf");
    }

    #[test]
    #[ignore] // Ignore by default - requires CLAP models to be downloaded
    fn test_audio_encoding() {
        use ndarray::Array2;

        let model_dir = get_test_model_dir().expect("CLAP models not found for testing");
        let clap = ClapModel::new(&model_dir).expect("Failed to load model");

        // Create dummy mel spectrogram (64 mels, 100 time frames)
        let mel_spec = Array2::<f32>::zeros((64, 100));

        let embedding = clap.encode_audio(&mel_spec).unwrap();

        assert_eq!(embedding.len(), 512, "Embedding should be 512-dimensional");
        assert!(embedding.iter().all(|&x| x.is_finite()), "Embedding should not contain NaN/Inf");
    }

    #[test]
    #[ignore] // Ignore by default - requires CLAP models to be downloaded
    fn test_semantic_similarity() {
        let model_dir = get_test_model_dir().expect("CLAP models not found for testing");
        let clap = ClapModel::new(&model_dir).expect("Failed to load model");

        // Test that semantically similar queries produce similar embeddings
        let emb1 = clap.encode_text("footsteps").unwrap();
        let emb2 = clap.encode_text("walking sounds").unwrap();
        let emb3 = clap.encode_text("explosion").unwrap();

        // Calculate cosine similarities
        let sim_footsteps_walking = cosine_similarity(&emb1, &emb2);
        let sim_footsteps_explosion = cosine_similarity(&emb1, &emb3);

        // Footsteps and walking should be more similar than footsteps and explosion
        assert!(
            sim_footsteps_walking > sim_footsteps_explosion,
            "Semantic similarity should be higher for related concepts"
        );
    }

    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            0.0
        } else {
            dot / (norm_a * norm_b)
        }
    }
}
