/// Simplified audio encoder for CLAP
/// Uses standard transformer instead of hierarchical HTSAT

use candle_core::{Result, Tensor, D, IndexOp};
use candle_nn::{embedding, layer_norm, linear, Embedding, LayerNorm, Linear, Module, VarBuilder};

use super::config::ClapAudioConfig;

#[derive(Debug, Clone)]
pub struct ClapAudioEncoder {
    patch_embed: AudioPatchEmbedding,
    position_embedding: Embedding,
    encoder: AudioTransformer,
    layernorm: LayerNorm,
}

impl ClapAudioEncoder {
    pub fn new(vb: VarBuilder, config: &ClapAudioConfig) -> Result<Self> {
        let patch_embed = AudioPatchEmbedding::new(vb.pp("patch_embed"), config)?;

        // Calculate number of patches
        let num_patches = (config.num_mel_bins / config.patch_size)
            * (config.spec_size / config.patch_size);

        let position_embedding = embedding(
            num_patches + 1, // +1 for CLS token
            config.hidden_size,
            vb.pp("position_embedding"),
        )?;

        let encoder = AudioTransformer::new(vb.pp("encoder"), config)?;
        let layernorm = layer_norm(config.hidden_size, config.layer_norm_eps, vb.pp("layernorm"))?;

        Ok(Self {
            patch_embed,
            position_embedding,
            encoder,
            layernorm,
        })
    }

    pub fn forward(&self, mel_spectrogram: &Tensor) -> Result<Tensor> {
        // mel_spectrogram: (batch, n_mels, time_steps)

        // Patch embedding
        let mut x = self.patch_embed.forward(mel_spectrogram)?;
        // x: (batch, num_patches, hidden_size)

        let (batch_size, num_patches, _) = x.dims3()?;

        // Add CLS token
        let cls_token = self.position_embedding.embeddings()
            .i(0)?
            .unsqueeze(0)?
            .unsqueeze(0)?
            .broadcast_as((batch_size, 1, x.dim(D::Minus1)?))?;
        x = Tensor::cat(&[cls_token, x], 1)?;

        // Add position embeddings
        let position_ids = Tensor::arange(0u32, (num_patches + 1) as u32, x.device())?;
        let position_embeds = self.position_embedding.forward(&position_ids)?;
        x = x.broadcast_add(&position_embeds.unsqueeze(0)?)?;

        // Transformer encoder
        x = self.encoder.forward(&x)?;

        // Layer norm
        x = self.layernorm.forward(&x)?;

        // Return CLS token embedding
        x.i((.., 0, ..))
    }
}

#[derive(Debug, Clone)]
struct AudioPatchEmbedding {
    proj: Linear,
    patch_size: usize,
    num_mel_bins: usize,
}

impl AudioPatchEmbedding {
    fn new(vb: VarBuilder, config: &ClapAudioConfig) -> Result<Self> {
        let patch_dim = config.patch_size * config.patch_size;
        let proj = linear(patch_dim, config.hidden_size, vb.pp("proj"))?;

        Ok(Self {
            proj,
            patch_size: config.patch_size,
            num_mel_bins: config.num_mel_bins,
        })
    }

    fn forward(&self, x: &Tensor) -> Result<Tensor> {
        // x: (batch, n_mels, time_steps)
        let (batch_size, n_mels, time_steps) = x.dims3()?;

        let patch_size = self.patch_size;
        let n_mel_patches = n_mels / patch_size;
        let n_time_patches = time_steps / patch_size;

        // Reshape into patches
        // (batch, n_mel_patches, n_time_patches, patch_size, patch_size)
        let x = x
            .reshape((batch_size, n_mel_patches, patch_size, n_time_patches, patch_size))?
            .permute((0, 1, 3, 2, 4))?  // (batch, n_mel_patches, n_time_patches, patch_size, patch_size)
            .reshape((batch_size, n_mel_patches * n_time_patches, patch_size * patch_size))?;

        // Project to hidden_size
        self.proj.forward(&x)
    }
}

#[derive(Debug, Clone)]
struct AudioTransformer {
    layers: Vec<TransformerLayer>,
}

impl AudioTransformer {
    fn new(vb: VarBuilder, config: &ClapAudioConfig) -> Result<Self> {
        let mut layers = Vec::new();
        let vb_l = vb.pp("layers");

        for i in 0..config.num_hidden_layers {
            let layer = TransformerLayer::new(vb_l.pp(&i.to_string()), config)?;
            layers.push(layer);
        }

        Ok(Self { layers })
    }

    fn forward(&self, x: &Tensor) -> Result<Tensor> {
        let mut x = x.clone();
        for layer in &self.layers {
            x = layer.forward(&x)?;
        }
        Ok(x)
    }
}

#[derive(Debug, Clone)]
struct TransformerLayer {
    self_attn: MultiHeadAttention,
    self_attn_layer_norm: LayerNorm,
    mlp: MLP,
    mlp_layer_norm: LayerNorm,
}

impl TransformerLayer {
    fn new(vb: VarBuilder, config: &ClapAudioConfig) -> Result<Self> {
        let self_attn = MultiHeadAttention::new(vb.pp("self_attn"), config)?;
        let self_attn_layer_norm = layer_norm(
            config.hidden_size,
            config.layer_norm_eps,
            vb.pp("self_attn_layer_norm"),
        )?;
        let mlp = MLP::new(vb.pp("mlp"), config)?;
        let mlp_layer_norm = layer_norm(
            config.hidden_size,
            config.layer_norm_eps,
            vb.pp("mlp_layer_norm"),
        )?;

        Ok(Self {
            self_attn,
            self_attn_layer_norm,
            mlp,
            mlp_layer_norm,
        })
    }

    fn forward(&self, x: &Tensor) -> Result<Tensor> {
        // Self-attention with residual
        let attn_output = self.self_attn.forward(&self.self_attn_layer_norm.forward(x)?)?;
        let x = (x + attn_output)?;

        // MLP with residual
        let mlp_output = self.mlp.forward(&self.mlp_layer_norm.forward(&x)?)?;
        x + mlp_output
    }
}

#[derive(Debug, Clone)]
struct MultiHeadAttention {
    q_proj: Linear,
    k_proj: Linear,
    v_proj: Linear,
    out_proj: Linear,
    num_heads: usize,
    head_dim: usize,
}

impl MultiHeadAttention {
    fn new(vb: VarBuilder, config: &ClapAudioConfig) -> Result<Self> {
        let hidden_size = config.hidden_size;
        let num_heads = config.num_attention_heads;
        let head_dim = hidden_size / num_heads;

        let q_proj = linear(hidden_size, hidden_size, vb.pp("q_proj"))?;
        let k_proj = linear(hidden_size, hidden_size, vb.pp("k_proj"))?;
        let v_proj = linear(hidden_size, hidden_size, vb.pp("v_proj"))?;
        let out_proj = linear(hidden_size, hidden_size, vb.pp("out_proj"))?;

        Ok(Self {
            q_proj,
            k_proj,
            v_proj,
            out_proj,
            num_heads,
            head_dim,
        })
    }

    fn forward(&self, x: &Tensor) -> Result<Tensor> {
        let (batch_size, seq_len, _) = x.dims3()?;

        // Project Q, K, V
        let q = self.q_proj.forward(x)?;
        let k = self.k_proj.forward(x)?;
        let v = self.v_proj.forward(x)?;

        // Reshape for multi-head attention
        // (batch, seq_len, hidden) -> (batch, num_heads, seq_len, head_dim)
        let q = q
            .reshape((batch_size, seq_len, self.num_heads, self.head_dim))?
            .transpose(1, 2)?;
        let k = k
            .reshape((batch_size, seq_len, self.num_heads, self.head_dim))?
            .transpose(1, 2)?;
        let v = v
            .reshape((batch_size, seq_len, self.num_heads, self.head_dim))?
            .transpose(1, 2)?;

        // Scaled dot-product attention
        let scale = (self.head_dim as f64).sqrt();
        let attn_weights = (q.matmul(&k.transpose(2, 3)?)? / scale)?;
        let attn_weights = candle_nn::ops::softmax_last_dim(&attn_weights)?;

        // Apply attention to values
        let attn_output = attn_weights.matmul(&v)?;

        // Reshape back
        // (batch, num_heads, seq_len, head_dim) -> (batch, seq_len, hidden)
        let attn_output = attn_output
            .transpose(1, 2)?
            .reshape((batch_size, seq_len, self.num_heads * self.head_dim))?;

        // Output projection
        self.out_proj.forward(&attn_output)
    }
}

#[derive(Debug, Clone)]
struct MLP {
    fc1: Linear,
    fc2: Linear,
}

impl MLP {
    fn new(vb: VarBuilder, config: &ClapAudioConfig) -> Result<Self> {
        let fc1 = linear(config.hidden_size, config.intermediate_size, vb.pp("fc1"))?;
        let fc2 = linear(config.intermediate_size, config.hidden_size, vb.pp("fc2"))?;

        Ok(Self { fc1, fc2 })
    }

    fn forward(&self, x: &Tensor) -> Result<Tensor> {
        let x = self.fc1.forward(x)?;
        let x = x.gelu()?;
        self.fc2.forward(&x)
    }
}

// Implement Module trait for ClapAudioEncoder
impl Module for ClapAudioEncoder {
    fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        self.forward(xs)
    }
}
