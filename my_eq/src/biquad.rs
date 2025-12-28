use std::f32::consts::PI;

#[derive(Debug, Clone, Copy)]
pub enum FilterType {
    LowShelf,
    HighShelf,
    Peaking,
}

#[derive(Debug, Clone, Default)]
pub struct Biquad {
    // Coefficients
    a0: f32, // Not strictly needed if normalized, but kept for clarity/structure
    a1: f32,
    a2: f32,
    b0: f32,
    b1: f32,
    b2: f32,

    // State (Direct Form II Transposed)
    s1: f32,
    s2: f32,
}

impl Biquad {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.s1 = 0.0;
        self.s2 = 0.0;
    }

    pub fn update(&mut self, filter_type: FilterType, sample_rate: f32, frequency: f32, q: f32, gain_db: f32) {
        let a = 10.0f32.powf(gain_db / 40.0);
        let w0 = 2.0 * PI * frequency / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);

        let b0;
        let b1;
        let b2;
        let a0;
        let a1;
        let a2;

        match filter_type {
            FilterType::LowShelf => {
                b0 = a * ((a + 1.0) - (a - 1.0) * cos_w0 + 2.0 * a.sqrt() * alpha);
                b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * cos_w0);
                b2 = a * ((a + 1.0) - (a - 1.0) * cos_w0 - 2.0 * a.sqrt() * alpha);
                a0 = (a + 1.0) + (a - 1.0) * cos_w0 + 2.0 * a.sqrt() * alpha;
                a1 = -2.0 * ((a - 1.0) + (a + 1.0) * cos_w0);
                a2 = (a + 1.0) + (a - 1.0) * cos_w0 - 2.0 * a.sqrt() * alpha;
            }
            FilterType::HighShelf => {
                b0 = a * ((a + 1.0) + (a - 1.0) * cos_w0 + 2.0 * a.sqrt() * alpha);
                b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_w0);
                b2 = a * ((a + 1.0) + (a - 1.0) * cos_w0 - 2.0 * a.sqrt() * alpha);
                a0 = (a + 1.0) - (a - 1.0) * cos_w0 + 2.0 * a.sqrt() * alpha;
                a1 = 2.0 * ((a - 1.0) - (a + 1.0) * cos_w0);
                a2 = (a + 1.0) - (a - 1.0) * cos_w0 - 2.0 * a.sqrt() * alpha;
            }
            FilterType::Peaking => {
                b0 = 1.0 + alpha * a;
                b1 = -2.0 * cos_w0;
                b2 = 1.0 - alpha * a;
                a0 = 1.0 + alpha / a;
                a1 = -2.0 * cos_w0;
                a2 = 1.0 - alpha / a;
            }
        }

        // Normalize
        let inv_a0 = 1.0 / a0;
        self.b0 = b0 * inv_a0;
        self.b1 = b1 * inv_a0;
        self.b2 = b2 * inv_a0;
        self.a1 = a1 * inv_a0;
        self.a2 = a2 * inv_a0;
        self.a0 = 1.0;
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        // Direct Form II Transposed
        // y[n] = b0 * x[n] + s1[n-1]
        // s1[n] = s2[n-1] + b1 * x[n] - a1 * y[n]
        // s2[n] = b2 * x[n] - a2 * y[n]

        let out = self.b0 * sample + self.s1;

        self.s1 = self.s2 + self.b1 * sample - self.a1 * out;
        self.s2 = self.b2 * sample - self.a2 * out;

        out
    }
}
