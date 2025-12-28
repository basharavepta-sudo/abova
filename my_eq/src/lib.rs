use nih_plug::prelude::*;
use nih_plug_egui::{EguiState, create_egui_editor}; // Check imports
use std::sync::Arc;

pub mod biquad;
pub use biquad::{Biquad, FilterType};

mod editor;

struct MyEq {
    params: Arc<MyEqParams>,

    // Per-channel filters. Outer Vec = Channels, Inner Vec = Bands
    filters: Vec<Vec<Biquad>>,
}

#[derive(Params)]
pub struct MyEqParams {
    /// The editor state, saved together with the parameter state so the custom
    /// scaling can be restored.
    #[persist = "editor-state"]
    editor_state: Arc<EguiState>,

    #[nested(group = "Band 1")]
    pub band1: BandParams,
    #[nested(group = "Band 2")]
    pub band2: BandParams,
    #[nested(group = "Band 3")]
    pub band3: BandParams,
    #[nested(group = "Band 4")]
    pub band4: BandParams,
    #[nested(group = "Band 5")]
    pub band5: BandParams,
    #[nested(group = "Band 6")]
    pub band6: BandParams,
}

#[derive(Params)]
pub struct BandParams {
    #[id = "active"]
    pub active: BoolParam,

    #[id = "type"]
    pub filter_type: IntParam, // 0: LowShelf, 1: HighShelf, 2: Peaking, 3: LowPass, 4: HighPass

    #[id = "freq"]
    pub freq: FloatParam,

    #[id = "gain"]
    pub gain: FloatParam,

    #[id = "q"]
    pub q: FloatParam,
}

impl Default for MyEq {
    fn default() -> Self {
        Self {
            params: Arc::new(MyEqParams::default()),
            filters: Vec::new(),
        }
    }
}

impl Default for MyEqParams {
    fn default() -> Self {
        Self {
            editor_state: editor::default_state(),
            band1: BandParams::new(true, 0, 100.0, 0.0, 0.707),
            band2: BandParams::new(true, 2, 200.0, 0.0, 0.707),
            band3: BandParams::new(true, 2, 500.0, 0.0, 0.707),
            band4: BandParams::new(true, 2, 1000.0, 0.0, 0.707),
            band5: BandParams::new(true, 2, 5000.0, 0.0, 0.707),
            band6: BandParams::new(true, 1, 10000.0, 0.0, 0.707),
        }
    }
}

impl BandParams {
    fn new(active: bool, type_val: i32, freq: f32, gain: f32, q: f32) -> Self {
        Self {
            active: BoolParam::new("Active", active),
            filter_type: IntParam::new(
                "Type",
                type_val,
                IntRange::Linear { min: 0, max: 4 },
            ),
            freq: FloatParam::new(
                "Frequency",
                freq,
                FloatRange::Skewed { min: 20.0, max: 20000.0, factor: FloatRange::skew_factor(1000.0) },
            )
            .with_unit(" Hz")
            .with_value_to_string(formatters::v2s_f32_hz_then_khz(1)),

            gain: FloatParam::new(
                "Gain",
                gain,
                FloatRange::Linear { min: -24.0, max: 24.0 },
            )
            .with_unit(" dB"),

            q: FloatParam::new(
                "Q",
                q,
                FloatRange::Skewed { min: 0.1, max: 10.0, factor: FloatRange::skew_factor(1.0) },
            ),
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

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        editor::create(
            self.params.clone(),
            self.params.editor_state.clone(),
        )
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        _buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        let num_channels = _audio_io_layout.main_input_channels.map(|n| n.get()).unwrap_or(2) as usize;

        // 6 bands per channel
        self.filters = vec![vec![Biquad::new(); 6]; num_channels];

        true
    }

    fn reset(&mut self) {
        for channel_filters in &mut self.filters {
            for filter in channel_filters {
                filter.reset();
            }
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
            if channel_idx >= self.filters.len() {
                break;
            }

            let channel_filters = &mut self.filters[channel_idx];

            for sample in channel_samples.iter_mut() {
                // Collect parameters for 6 bands
                // This is redundant to do per sample, but required for sample accurate automation.
                // We will iterate 6 bands.

                // Helper closure to process one band
                let mut process_band = |band_idx: usize, active: bool, type_idx: i32, freq: f32, q: f32, gain: f32, s: f32| -> f32 {
                    if !active {
                        return s;
                    }
                    let ft = match type_idx {
                        0 => FilterType::LowShelf,
                        1 => FilterType::HighShelf,
                        2 => FilterType::Peaking,
                        3 => FilterType::LowPass,
                        4 => FilterType::HighPass,
                        _ => FilterType::Peaking,
                    };

                    let filter = &mut channel_filters[band_idx];
                    filter.update(ft, sample_rate, freq, q, gain);
                    filter.process(s)
                };

                let mut s = *sample;
                s = process_band(0, self.params.band1.active.value(), self.params.band1.filter_type.value(), self.params.band1.freq.value(), self.params.band1.q.value(), self.params.band1.gain.value(), s);
                s = process_band(1, self.params.band2.active.value(), self.params.band2.filter_type.value(), self.params.band2.freq.value(), self.params.band2.q.value(), self.params.band2.gain.value(), s);
                s = process_band(2, self.params.band3.active.value(), self.params.band3.filter_type.value(), self.params.band3.freq.value(), self.params.band3.q.value(), self.params.band3.gain.value(), s);
                s = process_band(3, self.params.band4.active.value(), self.params.band4.filter_type.value(), self.params.band4.freq.value(), self.params.band4.q.value(), self.params.band4.gain.value(), s);
                s = process_band(4, self.params.band5.active.value(), self.params.band5.filter_type.value(), self.params.band5.freq.value(), self.params.band5.q.value(), self.params.band5.gain.value(), s);
                s = process_band(5, self.params.band6.active.value(), self.params.band6.filter_type.value(), self.params.band6.freq.value(), self.params.band6.q.value(), self.params.band6.gain.value(), s);

                *sample = s;
            }
        }

        ProcessStatus::Normal
    }
}

impl ClapPlugin for MyEq {
    const CLAP_ID: &'static str = "com.myvendor.myeq";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("A simple 6-band equalizer");
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
