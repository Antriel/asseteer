use rubato::{Resampler, SincFixedIn, SincInterpolationType, SincInterpolationParameters, WindowFunction};

/// Resample audio to target sample rate
pub fn resample_audio(
    samples: &[f32],
    from_rate: u32,
    to_rate: u32,
) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
    if from_rate == to_rate {
        return Ok(samples.to_vec());
    }

    if samples.is_empty() {
        return Ok(Vec::new());
    }

    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };

    let mut resampler = SincFixedIn::<f32>::new(
        to_rate as f64 / from_rate as f64,
        2.0,
        params,
        samples.len(),
        1, // mono
    )?;

    let waves_in = vec![samples.to_vec()];
    let waves_out = resampler.process(&waves_in, None)?;

    Ok(waves_out[0].clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resample_no_change() {
        let samples = vec![0.5f32; 1000];
        let result = resample_audio(&samples, 44100, 44100);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1000);
    }

    #[test]
    fn test_resample_empty() {
        let samples: Vec<f32> = vec![];
        let result = resample_audio(&samples, 44100, 32000);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[test]
    fn test_resample_downsample() {
        // Downsample from 44.1kHz to 32kHz
        let samples = vec![0.5f32; 44100]; // 1 second at 44.1kHz
        let result = resample_audio(&samples, 44100, 32000);
        assert!(result.is_ok());

        let resampled = result.unwrap();
        // Should be approximately 32000 samples
        assert!(resampled.len() > 31000 && resampled.len() < 33000);
    }

    #[test]
    fn test_resample_upsample() {
        // Upsample from 22.05kHz to 32kHz
        let samples = vec![0.5f32; 22050]; // 1 second at 22.05kHz
        let result = resample_audio(&samples, 22050, 32000);
        assert!(result.is_ok());

        let resampled = result.unwrap();
        // Should be approximately 32000 samples
        assert!(resampled.len() > 31000 && resampled.len() < 33000);
    }
}
