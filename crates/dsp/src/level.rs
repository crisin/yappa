/// Floor for [`gain_to_db`], so silence does not become `-inf`.
pub const MIN_DB: f32 = -120.0;

pub fn db_to_gain(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

pub fn gain_to_db(gain: f32) -> f32 {
    if gain <= 0.0 {
        MIN_DB
    } else {
        (20.0 * gain.log10()).max(MIN_DB)
    }
}

/// Root mean square of a block; 0 for an empty block.
pub fn rms(block: &[f32]) -> f32 {
    if block.is_empty() {
        return 0.0;
    }
    let sum: f32 = block.iter().map(|s| s * s).sum();
    (sum / block.len() as f32).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_gain_round_trip() {
        for db in [-60.0, -6.0, 0.0, 6.0] {
            assert!((gain_to_db(db_to_gain(db)) - db).abs() < 1e-4);
        }
        assert!((db_to_gain(-6.0206) - 0.5).abs() < 1e-4);
    }

    #[test]
    fn silence_is_floored() {
        assert_eq!(gain_to_db(0.0), MIN_DB);
        assert_eq!(rms(&[]), 0.0);
    }

    #[test]
    fn rms_of_full_scale_sine_is_one_over_sqrt2() {
        let block: Vec<f32> = (0..4800)
            .map(|i| (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48_000.0).sin())
            .collect();
        assert!((rms(&block) - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-3);
    }
}
