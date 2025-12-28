use nih_plug::prelude::*;
use std::sync::Arc;

pub mod biquad;
pub use biquad::{Biquad, FilterType};

struct MyEq {
    params: Arc<MyEqParams>,

    // Per-channel filters
    low_shelf: Vec<Biquad>,
    mid_peak: Vec<Biquad>,
    high_shelf: Vec<Biquad>,
}

#[derive(Params)]
struct MyEqParams {
    // Low Shelf
    #[id = "low_gain"]
    pub low_gain: FloatParam,
    #[id = "low_freq"]
    pub low_freq: FloatParam,

    // Mid Peak
    #[id = "mid_gain"]
    pub mid_gain: FloatParam,
    #[id = "mid_freq"]
    pub mid_freq: FloatParam,
    #[id = "mid_q"]
    pub mid_q: FloatParam,

    // High Shelf
    #[id = "high_gain"]
    pub high_gain: FloatParam,
    #[id = "high_freq"]
    pub high_freq: FloatParam,
}

impl Default for MyEq {
    fn default() -> Self {
        Self {
            params: Arc::new(MyEqParams::default()),
            low_shelf: Vec::new(),
            mid_peak: Vec::new(),
            high_shelf: Vec::new(),
        }
    }
}

impl Default for MyEqParams {
    fn default() -> Self {
        Self {
            low_gain: FloatParam::new(
                "Low Gain",
                0.0,
                FloatRange::Linear { min: -18.0, max: 18.0 },
            )
            .with_unit(" dB"),

            low_freq: FloatParam::new(
                "Low Freq",
                100.0,
                FloatRange::Skewed { min: 20.0, max: 2000.0, factor: FloatRange::skew_factor(-1.0) },
            )
            .with_unit(" Hz"),

            mid_gain: FloatParam::new(
                "Mid Gain",
                0.0,
                FloatRange::Linear { min: -18.0, max: 18.0 },
            )
            .with_unit(" dB"),

            mid_freq: FloatParam::new(
                "Mid Freq",
                1000.0,
                FloatRange::Skewed { min: 100.0, max: 10000.0, factor: FloatRange::skew_factor(0.0) },
            )
            .with_unit(" Hz"),

            mid_q: FloatParam::new(
                "Mid Q",
                0.707,
                FloatRange::Linear { min: 0.1, max: 10.0 },
            ),

            high_gain: FloatParam::new(
                "High Gain",
                0.0,
                FloatRange::Linear { min: -18.0, max: 18.0 },
            )
            .with_unit(" dB"),

            high_freq: FloatParam::new(
                "High Freq",
                5000.0,
                FloatRange::Skewed { min: 2000.0, max: 20000.0, factor: FloatRange::skew_factor(1.0) },
            )
            .with_unit(" Hz"),
        }
    }
}

impl Plugin for MyEq {
    const NAME: &'static str = "My Equalizer";
    const VENDOR: &'static str = "MyVendor";
    const URL: &'static str = "https://youtu.be/dQw4w9WgXcQ";
    const EMAIL: &'static str = "info@example.com";

    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    // The first audio IO layout is the default
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(1),
            main_output_channels: NonZeroU32::new(1),
            ..AudioIOLayout::const_default()
        },
    ];

    const MIDI_INPUT: MidiConfig = MidiConfig::None;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::None;

    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        _buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        // Resize filters to match channel count
        // buffer_config.input_channels was removed, we use audio_io_layout or assume based on buffer in process
        // Actually, we can get channel count from _audio_io_layout

        // Wait, AudioIOLayout is a struct describing capability, not the *active* layout necessarily in initialize?
        // Ah, initialize is called with the *selected* layout.

        let num_channels = _audio_io_layout.main_input_channels.map(|n| n.get()).unwrap_or(2) as usize;

        self.low_shelf = vec![Biquad::new(); num_channels];
        self.mid_peak = vec![Biquad::new(); num_channels];
        self.high_shelf = vec![Biquad::new(); num_channels];

        true
    }

    fn reset(&mut self) {
        for filter in &mut self.low_shelf {
            filter.reset();
        }
        for filter in &mut self.mid_peak {
            filter.reset();
        }
        for filter in &mut self.high_shelf {
            filter.reset();
        }
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let sample_rate = context.transport().sample_rate;

        for (channel_idx, channel_samples) in buffer.as_slice().iter_mut().enumerate() {
            // Safety check for channel index
            if channel_idx >= self.low_shelf.len() {
                break;
            }

            let low_filter = &mut self.low_shelf[channel_idx];
            let mid_filter = &mut self.mid_peak[channel_idx];
            let high_filter = &mut self.high_shelf[channel_idx];

            for sample in channel_samples.iter_mut() {
                let low_g = self.params.low_gain.value();
                let low_f = self.params.low_freq.value();
                let mid_g = self.params.mid_gain.value();
                let mid_f = self.params.mid_freq.value();
                let mid_q = self.params.mid_q.value();
                let high_g = self.params.high_gain.value();
                let high_f = self.params.high_freq.value();

                low_filter.update(FilterType::LowShelf, sample_rate, low_f, 0.707, low_g);
                mid_filter.update(FilterType::Peaking, sample_rate, mid_f, mid_q, mid_g);
                high_filter.update(FilterType::HighShelf, sample_rate, high_f, 0.707, high_g);

                let s = *sample;
                let s = low_filter.process(s);
                let s = mid_filter.process(s);
                let s = high_filter.process(s);

                *sample = s;
            }
        }

        ProcessStatus::Normal
    }
}

impl ClapPlugin for MyEq {
    const CLAP_ID: &'static str = "com.myvendor.myeq";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("A simple 3-band equalizer");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::AudioEffect, ClapFeature::Equalizer];
}

impl Vst3Plugin for MyEq {
    const VST3_CLASS_ID: [u8; 16] = *b"MyEqForJulesUser";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Eq];
}

nih_export_clap!(MyEq);
nih_export_vst3!(MyEq);

#[cfg(test)]
mod dsp_tests;
