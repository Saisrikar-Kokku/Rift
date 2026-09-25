pub fn calculate_level(samples: &[i16]) -> u32 {
    if samples.is_empty() {
        return 0;
    }

    let mut max_abs: i16 = 0;
    for &sample in samples {
        let abs = sample.saturating_abs();
        if abs > max_abs {
            max_abs = abs;
        }
    }

    if max_abs < 150 {
        0
    } else {
        let normalized = ((max_abs - 150) as f32 / 10000.0).min(1.0);
        let scaled = normalized.powf(0.55) * 100.0;
        (scaled as u32).min(100)
    }
}
