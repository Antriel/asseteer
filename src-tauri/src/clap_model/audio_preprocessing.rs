/// Audio preprocessing for CLAP (mel spectrogram generation)
/// Adapted from Whisper implementation

use candle_core::{Result, Tensor, Device};

use super::config::{HOP_LENGTH, N_FFT, N_MELS, SAMPLE_RATE};

/// Generate mel spectrogram from audio samples
pub fn audio_to_mel_spectrogram(samples: &[f32], target_length: usize) -> Result<Tensor> {
    // Create mel filters
    let filters = create_mel_filters()?;

    // Compute mel spectrogram
    let mel = log_mel_spectrogram(samples, &filters)?;

    // Pad or truncate to target length
    let mel = pad_or_truncate_mel(mel, target_length)?;

    Ok(mel)
}

fn create_mel_filters() -> Result<Vec<f32>> {
    // Create mel filterbank
    // This is a simplified version; ideally would use proper mel scale conversion

    let n_fft_bins = N_FFT / 2 + 1;
    let mut filters = vec![0.0; N_MELS * n_fft_bins];

    // Simple triangular filters on mel scale
    let mel_low = hz_to_mel(0.0);
    let mel_high = hz_to_mel(SAMPLE_RATE as f32 / 2.0);
    let mel_step = (mel_high - mel_low) / (N_MELS + 1) as f32;

    for m in 0..N_MELS {
        let mel_left = mel_low + m as f32 * mel_step;
        let mel_center = mel_low + (m + 1) as f32 * mel_step;
        let mel_right = mel_low + (m + 2) as f32 * mel_step;

        let f_left = mel_to_hz(mel_left);
        let f_center = mel_to_hz(mel_center);
        let f_right = mel_to_hz(mel_right);

        for k in 0..n_fft_bins {
            let freq = k as f32 * SAMPLE_RATE as f32 / N_FFT as f32;

            let weight = if freq >= f_left && freq < f_center {
                (freq - f_left) / (f_center - f_left)
            } else if freq >= f_center && freq < f_right {
                (f_right - freq) / (f_right - f_center)
            } else {
                0.0
            };

            filters[m * n_fft_bins + k] = weight;
        }
    }

    Ok(filters)
}

fn hz_to_mel(hz: f32) -> f32 {
    2595.0 * (1.0 + hz / 700.0).log10()
}

fn mel_to_hz(mel: f32) -> f32 {
    700.0 * (10.0_f32.powf(mel / 2595.0) - 1.0)
}

fn log_mel_spectrogram(samples: &[f32], filters: &[f32]) -> Result<Vec<f32>> {
    let n_fft_bins = N_FFT / 2 + 1;
    let n_frames = (samples.len() - N_FFT) / HOP_LENGTH + 1;

    let mut mel = vec![0.0; N_MELS * n_frames];

    // Hanning window
    let hann: Vec<f32> = (0..N_FFT)
        .map(|i| {
            0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / N_FFT as f32).cos())
        })
        .collect();

    for frame_idx in 0..n_frames {
        let offset = frame_idx * HOP_LENGTH;

        // Apply window and prepare for FFT
        let mut fft_input = vec![0.0; N_FFT];
        for i in 0..N_FFT.min(samples.len() - offset) {
            fft_input[i] = samples[offset + i] * hann[i];
        }

        // Compute FFT (simplified - in practice, would use a proper FFT library)
        let fft_output = simple_fft(&fft_input);

        // Compute power spectrum
        let mut power_spectrum = vec![0.0; n_fft_bins];
        for i in 0..n_fft_bins {
            let real = fft_output[2 * i];
            let imag = fft_output[2 * i + 1];
            power_spectrum[i] = real * real + imag * imag;
        }

        // Apply mel filterbank
        for m in 0..N_MELS {
            let mut mel_energy = 0.0;
            for k in 0..n_fft_bins {
                mel_energy += power_spectrum[k] * filters[m * n_fft_bins + k];
            }

            // Log scale with small epsilon to avoid log(0)
            mel[m * n_frames + frame_idx] = (mel_energy.max(1e-10)).log10();
        }
    }

    Ok(mel)
}

// Simple FFT implementation (Cooley-Tukey)
fn simple_fft(input: &[f32]) -> Vec<f32> {
    let n = input.len();
    if n <= 1 {
        return vec![input.get(0).copied().unwrap_or(0.0), 0.0];
    }
    if n % 2 != 0 {
        return dft(input);
    }

    let mut even = Vec::with_capacity(n / 2);
    let mut odd = Vec::with_capacity(n / 2);

    for (i, &val) in input.iter().enumerate() {
        if i % 2 == 0 {
            even.push(val);
        } else {
            odd.push(val);
        }
    }

    let even_fft = simple_fft(&even);
    let odd_fft = simple_fft(&odd);

    let mut output = vec![0.0; n * 2];
    let two_pi = 2.0 * std::f32::consts::PI;

    for k in 0..n / 2 {
        let theta = -two_pi * k as f32 / n as f32;
        let (cos_theta, sin_theta) = (theta.cos(), theta.sin());

        let re_odd = odd_fft[2 * k];
        let im_odd = odd_fft[2 * k + 1];

        let re = cos_theta * re_odd - sin_theta * im_odd;
        let im = cos_theta * im_odd + sin_theta * re_odd;

        output[2 * k] = even_fft[2 * k] + re;
        output[2 * k + 1] = even_fft[2 * k + 1] + im;

        output[2 * (k + n / 2)] = even_fft[2 * k] - re;
        output[2 * (k + n / 2) + 1] = even_fft[2 * k + 1] - im;
    }

    output
}

fn dft(input: &[f32]) -> Vec<f32> {
    let n = input.len();
    let mut output = Vec::with_capacity(n * 2);
    let two_pi = 2.0 * std::f32::consts::PI;

    for k in 0..n {
        let mut re = 0.0;
        let mut im = 0.0;

        for (j, &val) in input.iter().enumerate() {
            let angle = -two_pi * k as f32 * j as f32 / n as f32;
            re += val * angle.cos();
            im += val * angle.sin();
        }

        output.push(re);
        output.push(im);
    }

    output
}

fn pad_or_truncate_mel(mel: Vec<f32>, target_length: usize) -> Result<Tensor> {
    let n_mels = N_MELS;
    let current_length = mel.len() / n_mels;

    let padded_mel = if current_length < target_length {
        // Pad with zeros
        let mut padded = mel;
        padded.resize(n_mels * target_length, 0.0);
        padded
    } else {
        // Truncate
        mel.into_iter().take(n_mels * target_length).collect()
    };

    // Reshape to (1, n_mels, target_length) - add batch dimension
    Tensor::from_vec(
        padded_mel,
        (1, n_mels, target_length),
        &Device::Cpu,
    )
}

/// Load audio from WAV file and resample to target sample rate
pub fn load_audio_file(path: &str) -> Result<Vec<f32>> {
    // For now, return a placeholder
    // In a real implementation, would use a library like hound or symphonia
    // to load and decode the audio file

    // Placeholder: 10 seconds of silence
    let duration_seconds = 10;
    let num_samples = SAMPLE_RATE * duration_seconds;
    Ok(vec![0.0; num_samples])
}
