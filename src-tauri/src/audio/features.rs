use ndarray::{Array1, Array2};
use num_complex::Complex;
use rustfft::{FftPlanner, num_complex};
use std::f32::consts::PI;

/// Configuration for mel spectrogram generation
pub struct MelConfig {
    pub sample_rate: u32,
    pub n_fft: usize,
    pub hop_length: usize,
    pub n_mels: usize,
    pub fmin: f32,
    pub fmax: f32,
}

impl Default for MelConfig {
    fn default() -> Self {
        Self {
            sample_rate: 32000,  // CLAP expects 32kHz
            n_fft: 1024,
            hop_length: 320,
            n_mels: 64,
            fmin: 0.0,
            fmax: 16000.0,
        }
    }
}

/// Generate mel spectrogram from audio samples
pub fn create_mel_spectrogram(
    samples: &[f32],
    config: &MelConfig,
) -> Result<Array2<f32>, Box<dyn std::error::Error>> {
    // 1. Apply STFT (Short-Time Fourier Transform)
    let stft = compute_stft(samples, config.n_fft, config.hop_length)?;

    // 2. Compute power spectrogram
    let power_spec = stft.mapv(|c| c.norm_sqr());

    // 3. Create mel filterbank
    let mel_filters = create_mel_filterbank(
        config.n_mels,
        config.n_fft,
        config.sample_rate,
        config.fmin,
        config.fmax,
    )?;

    // 4. Apply mel filters
    let mel_spec = mel_filters.dot(&power_spec);

    // 5. Convert to log scale
    let log_mel = mel_spec.mapv(|x| (x + 1e-10).ln());

    Ok(log_mel)
}

/// Compute Short-Time Fourier Transform (STFT)
fn compute_stft(
    samples: &[f32],
    n_fft: usize,
    hop_length: usize,
) -> Result<Array2<Complex<f32>>, Box<dyn std::error::Error>> {
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(n_fft);

    if samples.len() < n_fft {
        return Err("Audio too short for FFT window".into());
    }

    let num_frames = (samples.len() - n_fft) / hop_length + 1;
    let mut stft_result = Array2::zeros((n_fft / 2 + 1, num_frames));

    // Apply Hann window
    let window: Vec<f32> = (0..n_fft)
        .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f32 / n_fft as f32).cos()))
        .collect();

    for frame_idx in 0..num_frames {
        let start = frame_idx * hop_length;
        let mut buffer: Vec<Complex<f32>> = samples[start..start + n_fft]
            .iter()
            .zip(window.iter())
            .map(|(s, w)| Complex::new(s * w, 0.0))
            .collect();

        fft.process(&mut buffer);

        // Keep only positive frequencies
        for (freq_idx, value) in buffer[..(n_fft / 2 + 1)].iter().enumerate() {
            stft_result[[freq_idx, frame_idx]] = *value;
        }
    }

    Ok(stft_result)
}

/// Create mel filterbank for converting spectrogram to mel scale
fn create_mel_filterbank(
    n_mels: usize,
    n_fft: usize,
    sample_rate: u32,
    fmin: f32,
    fmax: f32,
) -> Result<Array2<f32>, Box<dyn std::error::Error>> {
    // Helper: Convert Hz to mel scale
    let hz_to_mel = |hz: f32| 2595.0 * (1.0 + hz / 700.0).log10();
    let mel_to_hz = |mel: f32| 700.0 * (10.0_f32.powf(mel / 2595.0) - 1.0);

    let mel_min = hz_to_mel(fmin);
    let mel_max = hz_to_mel(fmax);
    let mel_points: Vec<f32> = (0..=n_mels + 1)
        .map(|i| mel_to_hz(mel_min + (mel_max - mel_min) * i as f32 / (n_mels + 1) as f32))
        .collect();

    let n_freqs = n_fft / 2 + 1;
    let fft_freqs: Vec<f32> = (0..n_freqs)
        .map(|i| i as f32 * sample_rate as f32 / n_fft as f32)
        .collect();

    let mut filterbank = Array2::zeros((n_mels, n_freqs));

    for mel_idx in 0..n_mels {
        let left = mel_points[mel_idx];
        let center = mel_points[mel_idx + 1];
        let right = mel_points[mel_idx + 2];

        for (freq_idx, &freq) in fft_freqs.iter().enumerate() {
            if freq >= left && freq <= center {
                filterbank[[mel_idx, freq_idx]] = (freq - left) / (center - left);
            } else if freq > center && freq <= right {
                filterbank[[mel_idx, freq_idx]] = (right - freq) / (right - center);
            }
        }
    }

    Ok(filterbank)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mel_config_default() {
        let config = MelConfig::default();
        assert_eq!(config.sample_rate, 32000);
        assert_eq!(config.n_fft, 1024);
        assert_eq!(config.hop_length, 320);
        assert_eq!(config.n_mels, 64);
    }

    #[test]
    fn test_stft_shape() {
        // Generate 1 second of silence at 32kHz
        let samples = vec![0.0f32; 32000];
        let result = compute_stft(&samples, 1024, 320);
        assert!(result.is_ok());

        let stft = result.unwrap();
        // Check shape: (n_fft/2 + 1, num_frames)
        assert_eq!(stft.nrows(), 513); // 1024/2 + 1
        assert_eq!(stft.ncols(), (32000 - 1024) / 320 + 1);
    }

    #[test]
    fn test_mel_filterbank_shape() {
        let result = create_mel_filterbank(64, 1024, 32000, 0.0, 16000.0);
        assert!(result.is_ok());

        let filterbank = result.unwrap();
        // Check shape: (n_mels, n_freqs)
        assert_eq!(filterbank.nrows(), 64);
        assert_eq!(filterbank.ncols(), 513); // 1024/2 + 1
    }

    #[test]
    fn test_mel_spectrogram_generation() {
        // Generate 1 second of 440Hz sine wave at 32kHz
        let duration = 1.0;
        let sample_rate = 32000;
        let frequency = 440.0;
        let samples: Vec<f32> = (0..sample_rate)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                (2.0 * PI * frequency * t).sin()
            })
            .collect();

        let config = MelConfig::default();
        let result = create_mel_spectrogram(&samples, &config);
        assert!(result.is_ok());

        let mel_spec = result.unwrap();
        // Check shape: (n_mels, num_frames)
        assert_eq!(mel_spec.nrows(), 64);
        assert!(mel_spec.ncols() > 0);

        // Check that values are finite
        assert!(mel_spec.iter().all(|&x| x.is_finite()));
    }
}
