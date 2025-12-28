#[cfg(test)]
mod tests {
    use crate::Biquad;
    use crate::FilterType;

    #[test]
    fn test_biquad_unity_gain() {
        let mut filter = Biquad::new();
        // 0dB gain should result in unity gain (no change)
        filter.update(FilterType::Peaking, 44100.0, 1000.0, 1.0, 0.0);

        let input = 1.0;
        let mut output = input;

        // Process a few samples to let state settle if needed, though with 0dB gain it should be instant
        for _ in 0..10 {
            output = filter.process(input);
        }

        assert!((output - input).abs() < 1e-6);
    }

    #[test]
    fn test_low_shelf_gain() {
        let mut filter = Biquad::new();
        // Low shelf at 100Hz, +6dB
        let gain_db = 6.0;
        filter.update(FilterType::LowShelf, 44100.0, 100.0, 0.707, gain_db);

        // At DC (0 Hz), gain should be approximately +6dB (linear ~2.0)
        // We can't easily test DC with just one sample, so let's check coefficients or response.
        // Or simpler: feed a DC signal (series of 1.0s) and check if output converges to ~2.0

        let target_gain = 10.0f32.powf(gain_db / 20.0); // Voltage gain
        let input = 1.0;
        let mut output = 0.0;

        for _ in 0..1000 {
            output = filter.process(input);
        }

        assert!((output - target_gain).abs() < 1e-2, "Expected {}, got {}", target_gain, output);
    }
}
