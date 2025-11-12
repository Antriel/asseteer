/// CLAP (Contrastive Language-Audio Pretraining) model configuration
/// Based on laion/clap-htsat-fused

#[derive(Debug, Clone)]
pub struct ClapConfig {
    pub text_config: ClapTextConfig,
    pub audio_config: ClapAudioConfig,
    pub projection_dim: usize,
    pub logit_scale_init_value: f32,
}

impl ClapConfig {
    pub fn htsat_fused() -> Self {
        Self {
            text_config: ClapTextConfig::default(),
            audio_config: ClapAudioConfig::default(),
            projection_dim: 512,
            logit_scale_init_value: 14.29,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ClapTextConfig {
    pub vocab_size: usize,
    pub hidden_size: usize,
    pub num_hidden_layers: usize,
    pub num_attention_heads: usize,
    pub intermediate_size: usize,
    pub max_position_embeddings: usize,
    pub layer_norm_eps: f64,
}

impl Default for ClapTextConfig {
    fn default() -> Self {
        // Config from laion/clap-htsat-fused
        Self {
            vocab_size: 50265,
            hidden_size: 768,
            num_hidden_layers: 12,
            num_attention_heads: 12,
            intermediate_size: 3072,
            max_position_embeddings: 514,
            layer_norm_eps: 1e-5,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ClapAudioConfig {
    pub hidden_size: usize,
    pub num_hidden_layers: usize,
    pub num_attention_heads: usize,
    pub intermediate_size: usize,
    pub num_mel_bins: usize,
    pub spec_size: usize, // Spectrogram time dimension
    pub patch_size: usize,
    pub layer_norm_eps: f64,
}

impl Default for ClapAudioConfig {
    fn default() -> Self {
        // Simplified config (not hierarchical)
        Self {
            hidden_size: 768,
            num_hidden_layers: 12,
            num_attention_heads: 12,
            intermediate_size: 3072,
            num_mel_bins: 64,
            spec_size: 1024, // ~10 seconds at 16kHz with hop_length=160
            patch_size: 4,
            layer_norm_eps: 1e-5,
        }
    }
}

// Audio preprocessing constants
pub const SAMPLE_RATE: usize = 48000;
pub const N_FFT: usize = 1024;
pub const HOP_LENGTH: usize = 480;
pub const N_MELS: usize = 64;
