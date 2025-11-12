/// CLAP text encoder (adapted from CLIP)

use candle_core::{Result, Tensor, D, Device, IndexOp};
use candle_nn::{embedding, layer_norm, linear, Embedding, LayerNorm, Linear, Module, VarBuilder};

use super::config::ClapTextConfig;

#[derive(Debug, Clone)]
pub struct ClapTextEncoder {
    embeddings: TextEmbeddings,
    encoder: TextTransformer,
    final_layer_norm: LayerNorm,
}

impl ClapTextEncoder {
    pub fn new(vb: VarBuilder, config: &ClapTextConfig) -> Result<Self> {
        let embeddings = TextEmbeddings::new(vb.pp("embeddings"), config)?;
        let encoder = TextTransformer::new(vb.pp("encoder"), config)?;
        let final_layer_norm = layer_norm(
            config.hidden_size,
            config.layer_norm_eps,
            vb.pp("final_layer_norm"),
        )?;

        Ok(Self {
            embeddings,
            encoder,
            final_layer_norm,
        })
    }

    pub fn forward(&self, input_ids: &Tensor) -> Result<Tensor> {
        let x = self.embeddings.forward(input_ids)?;
        let x = self.encoder.forward(&x)?;
        let x = self.final_layer_norm.forward(&x)?;

        // Return the embedding for the EOS token (last token)
        let seq_len = x.dim(1)?;
        x.i((.., seq_len - 1, ..))
    }
}

#[derive(Debug, Clone)]
struct TextEmbeddings {
    token_embedding: Embedding,
    position_embedding: Embedding,
}

impl TextEmbeddings {
    fn new(vb: VarBuilder, config: &ClapTextConfig) -> Result<Self> {
        let token_embedding = embedding(
            config.vocab_size,
            config.hidden_size,
            vb.pp("token_embedding"),
        )?;
        let position_embedding = embedding(
            config.max_position_embeddings,
            config.hidden_size,
            vb.pp("position_embedding"),
        )?;

        Ok(Self {
            token_embedding,
            position_embedding,
        })
    }

    fn forward(&self, input_ids: &Tensor) -> Result<Tensor> {
        let seq_len = input_ids.dim(D::Minus1)?;
        let token_embeds = self.token_embedding.forward(input_ids)?;
        let position_ids = Tensor::arange(0u32, seq_len as u32, input_ids.device())?;
        let position_embeds = self.position_embedding.forward(&position_ids)?;

        token_embeds.broadcast_add(&position_embeds.unsqueeze(0)?)
    }
}

#[derive(Debug, Clone)]
struct TextTransformer {
    layers: Vec<TextTransformerLayer>,
}

impl TextTransformer {
    fn new(vb: VarBuilder, config: &ClapTextConfig) -> Result<Self> {
        let mut layers = Vec::new();
        let vb_l = vb.pp("layers");

        for i in 0..config.num_hidden_layers {
            let layer = TextTransformerLayer::new(vb_l.pp(&i.to_string()), config)?;
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
struct TextTransformerLayer {
    self_attn: TextAttention,
    self_attn_layer_norm: LayerNorm,
    mlp: TextMLP,
    mlp_layer_norm: LayerNorm,
}

impl TextTransformerLayer {
    fn new(vb: VarBuilder, config: &ClapTextConfig) -> Result<Self> {
        let self_attn = TextAttention::new(vb.pp("self_attn"), config)?;
        let self_attn_layer_norm = layer_norm(
            config.hidden_size,
            config.layer_norm_eps,
            vb.pp("self_attn_layer_norm"),
        )?;
        let mlp = TextMLP::new(vb.pp("mlp"), config)?;
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
        // Pre-norm architecture
        let attn_output = self.self_attn.forward(&self.self_attn_layer_norm.forward(x)?)?;
        let x = (x + attn_output)?;

        let mlp_output = self.mlp.forward(&self.mlp_layer_norm.forward(&x)?)?;
        x + mlp_output
    }
}

#[derive(Debug, Clone)]
struct TextAttention {
    q_proj: Linear,
    k_proj: Linear,
    v_proj: Linear,
    out_proj: Linear,
    num_heads: usize,
    head_dim: usize,
}

impl TextAttention {
    fn new(vb: VarBuilder, config: &ClapTextConfig) -> Result<Self> {
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

        let q = self.q_proj.forward(x)?;
        let k = self.k_proj.forward(x)?;
        let v = self.v_proj.forward(x)?;

        let q = q
            .reshape((batch_size, seq_len, self.num_heads, self.head_dim))?
            .transpose(1, 2)?;
        let k = k
            .reshape((batch_size, seq_len, self.num_heads, self.head_dim))?
            .transpose(1, 2)?;
        let v = v
            .reshape((batch_size, seq_len, self.num_heads, self.head_dim))?
            .transpose(1, 2)?;

        // Causal attention mask for autoregressive text
        let scale = (self.head_dim as f64).sqrt();
        let attn_weights = (q.matmul(&k.transpose(2, 3)?)? / scale)?;

        // Apply causal mask
        let mask = self.get_causal_mask(seq_len, x.device())?;
        let attn_weights = attn_weights.broadcast_add(&mask)?;

        let attn_weights = candle_nn::ops::softmax_last_dim(&attn_weights)?;
        let attn_output = attn_weights.matmul(&v)?;

        let attn_output = attn_output
            .transpose(1, 2)?
            .reshape((batch_size, seq_len, self.num_heads * self.head_dim))?;

        self.out_proj.forward(&attn_output)
    }

    fn get_causal_mask(&self, seq_len: usize, device: &Device) -> Result<Tensor> {
        let mask: Vec<_> = (0..seq_len)
            .flat_map(|i| {
                (0..seq_len).map(move |j| {
                    if j > i {
                        f32::NEG_INFINITY
                    } else {
                        0.0
                    }
                })
            })
            .collect();

        Tensor::from_vec(mask, (seq_len, seq_len), device)?
            .unsqueeze(0)?
            .unsqueeze(0)
    }
}

#[derive(Debug, Clone)]
struct TextMLP {
    fc1: Linear,
    fc2: Linear,
}

impl TextMLP {
    fn new(vb: VarBuilder, config: &ClapTextConfig) -> Result<Self> {
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

// Implement Module trait for ClapTextEncoder
impl Module for ClapTextEncoder {
    fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        self.forward(xs)
    }
}
