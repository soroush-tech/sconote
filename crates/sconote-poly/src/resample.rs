//! Windowed-sinc resampling, so recordings at device rates (44.1/48 kHz)
//! can feed the 22.05 kHz model.

use std::f64::consts::PI;

use crate::wav::MonoAudio;

/// Sinc lobes on each side of the kernel center - 12 gives ~80 dB of alias
/// rejection with a Hann window, far below the noise floor of a room
/// recording.
const KERNEL_LOBES: f64 = 12.0;

/// Resample to `target_rate` (up or down). Returns a clone when the rate
/// already matches.
pub fn resample(audio: &MonoAudio, target_rate: u32) -> MonoAudio {
    if audio.sample_rate == target_rate {
        return audio.clone();
    }
    let ratio = f64::from(target_rate) / f64::from(audio.sample_rate);
    // Anti-aliasing cutoff in cycles per *input* sample: the lower of the
    // two Nyquist frequencies.
    let cutoff = 0.5 * ratio.min(1.0);
    let half_width = (KERNEL_LOBES / (2.0 * cutoff)).ceil();
    let input = &audio.samples;
    let output_len = (input.len() as f64 * ratio).floor() as usize;
    let mut output = Vec::with_capacity(output_len);
    for j in 0..output_len {
        let center = j as f64 / ratio;
        let lo = (center - half_width).ceil().max(0.0) as usize;
        let hi = ((center + half_width).floor() as usize).min(input.len().saturating_sub(1));
        let mut acc = 0.0;
        for (k, &sample) in input[lo..=hi].iter().enumerate() {
            acc += f64::from(sample) * kernel((lo + k) as f64 - center, cutoff, half_width);
        }
        output.push(acc as f32);
    }
    MonoAudio {
        samples: output,
        sample_rate: target_rate,
    }
}

/// Hann-windowed sinc low-pass kernel, unit DC gain.
fn kernel(x: f64, cutoff: f64, half_width: f64) -> f64 {
    let sinc = if x == 0.0 {
        2.0 * cutoff
    } else {
        (2.0 * PI * cutoff * x).sin() / (PI * x)
    };
    let window = 0.5 * (1.0 + (PI * x / half_width).cos());
    sinc * window
}

#[cfg(test)]
#[path = "resample_test.rs"]
mod resample_test;
