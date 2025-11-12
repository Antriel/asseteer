/// CLAP (Contrastive Language-Audio Pretraining) implementation
/// Simplified version using standard transformer for audio encoder

pub mod audio_encoder;
pub mod audio_preprocessing;
pub mod config;
pub mod text_encoder;

use candle_core::{Result, Tensor, D, Device, Error as CandleError, DType};
use candle_nn::{linear_no_bias, Linear, Module, VarBuilder};

use audio_encoder::ClapAudioEncoder;
use config::ClapConfig;
use text_encoder::ClapTextEncoder;

#[derive(Debug, Clone)]
pub struct ClapModel {
    text_encoder: ClapTextEncoder,
    audio_encoder: ClapAudioEncoder,
    text_projection: Linear,
    audio_projection: Linear,
    logit_scale: Tensor,
}

impl ClapModel {
    pub fn new(vb: VarBuilder, config: &ClapConfig) -> Result<Self> {
        let text_encoder = ClapTextEncoder::new(vb.pp("text_model"), &config.text_config)?;
        let audio_encoder = ClapAudioEncoder::new(vb.pp("audio_model"), &config.audio_config)?;

        let text_projection = linear_no_bias(
            config.text_config.hidden_size,
            config.projection_dim,
            vb.pp("text_projection"),
        )?;

        let audio_projection = linear_no_bias(
            config.audio_config.hidden_size,
            config.projection_dim,
            vb.pp("audio_projection"),
        )?;

        let logit_scale = if vb.contains_tensor("logit_scale") {
            vb.get(&[], "logit_scale")?
        } else {
            Tensor::new(&[config.logit_scale_init_value], vb.device())?
        };

        Ok(Self {
            text_encoder,
            audio_encoder,
            text_projection,
            audio_projection,
            logit_scale,
        })
    }

    /// Encode text to embeddings
    pub fn encode_text(&self, input_ids: &Tensor) -> Result<Tensor> {
        let features = self.text_encoder.forward(input_ids)?;
        self.text_projection.forward(&features)
    }

    /// Encode audio (mel spectrogram) to embeddings
    pub fn encode_audio(&self, mel_spectrogram: &Tensor) -> Result<Tensor> {
        let features = self.audio_encoder.forward(mel_spectrogram)?;
        self.audio_projection.forward(&features)
    }

    /// Forward pass: compute similarity between audio and text
    pub fn forward(&self, mel_spectrogram: &Tensor, input_ids: &Tensor) -> Result<(Tensor, Tensor)> {
        let audio_features = self.encode_audio(mel_spectrogram)?;
        let text_features = self.encode_text(input_ids)?;

        let audio_features_normalized = normalize_l2(&audio_features)?;
        let text_features_normalized = normalize_l2(&text_features)?;

        let logits_per_text = text_features_normalized.matmul(&audio_features_normalized.t()?)?;
        let logit_scale = self.logit_scale.exp()?;
        let logits_per_text = logits_per_text.broadcast_mul(&logit_scale)?;
        let logits_per_audio = logits_per_text.t()?;

        Ok((logits_per_text, logits_per_audio))
    }

    /// Load pretrained model from HuggingFace Hub
    pub fn from_pretrained(model_id: &str, device: &Device) -> Result<Self> {
        use hf_hub::{api::sync::Api, Repo, RepoType};

        let api = Api::new().map_err(|e| CandleError::Msg(e.to_string()))?;
        let repo = Repo::with_revision(
            model_id.to_string(),
            RepoType::Model,
            "main".to_string(),
        );
        let api = api.repo(repo);

        let model_file = api
            .get("model.safetensors")
            .map_err(|e| CandleError::Msg(e.to_string()))?;

        let config = ClapConfig::htsat_fused();
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(
                &[model_file],
                DType::F32,
                device,
            )?
        };

        Self::new(vb, &config)
    }
}

/// L2 normalization
fn normalize_l2(tensor: &Tensor) -> Result<Tensor> {
    let l2_norm = tensor.sqr()?.sum_keepdim(D::Minus1)?.sqrt()?;
    tensor.broadcast_div(&l2_norm)
}

/// Compute cosine similarity between two normalized embeddings
pub fn cosine_similarity(a: &Tensor, b: &Tensor) -> Result<f32> {
    let similarity = (a * b)?.sum_all()?;
    similarity.to_scalar()
}
