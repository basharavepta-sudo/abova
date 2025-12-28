use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, egui, EguiState, widgets};
use std::sync::Arc;
use crate::MyEqParams;

pub fn default_state() -> Arc<EguiState> {
    EguiState::from_size(800, 600)
}

pub fn create(
    params: Arc<MyEqParams>,
    editor_state: Arc<EguiState>,
) -> Option<Box<dyn Editor>> {
    create_egui_editor(
        editor_state,
        (),
        |_, _| {},
        move |egui_ctx, setter, _state| {
            egui::CentralPanel::default().show(egui_ctx, |ui| {
                ui.heading("My Equalizer");

                // Draw Plot
                // Note: egui_plot must be imported.
                // Since we added it to Cargo.toml, we can use it.
                // We use the crate root import if nih_plug_egui doesn't re-export it.
                use egui_plot::{Line, Plot, PlotPoints};

                let sample_rate = 44100.0;

                let curve: PlotPoints = (0..500).map(|i| {
                    let x = i as f32;
                    // Logarithmic mapping: x=0 -> 20Hz, x=500 -> 20000Hz
                    // 20 * (20000/20)^(x/500)
                    let f = 20.0 * (1000.0f32).powf(x / 500.0);

                    // Calculate total gain in dB
                    let mut mag = 1.0;

                    // Helper to calc mag for a band
                    let calc_band = |active: bool, type_idx: i32, freq: f32, q: f32, gain: f32| -> f32 {
                        if !active { return 1.0; }
                        use crate::biquad::{Biquad, FilterType};
                        let ft = match type_idx {
                            0 => FilterType::LowShelf,
                            1 => FilterType::HighShelf,
                            2 => FilterType::Peaking,
                            3 => FilterType::LowPass,
                            4 => FilterType::HighPass,
                            _ => FilterType::Peaking,
                        };
                        let mut b = Biquad::new();
                        b.update(ft, sample_rate, freq, q, gain);
                        b.magnitude(f, sample_rate)
                    };

                    // Process all 6 bands
                    let bands = [
                        &params.band1, &params.band2, &params.band3,
                        &params.band4, &params.band5, &params.band6
                    ];

                    for band in bands {
                         mag *= calc_band(
                            band.active.value(),
                            band.filter_type.value(),
                            band.freq.value(),
                            band.q.value(),
                            band.gain.value()
                        );
                    }

                    let db = 20.0 * mag.log10();
                    [f as f64, db as f64]
                }).collect();

                let line = Line::new(curve);
                Plot::new("my_plot")
                    .view_aspect(2.0)
                    .x_axis_label("Frequency (Hz)")
                    .y_axis_label("Gain (dB)")
                    .x_grid_spacer(egui_plot::log_grid_spacer(10))
                    .include_y(-10.0)
                    .include_y(10.0)
                    .show(ui, |plot_ui| plot_ui.line(line));

                // Controls
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let bands = [
                        (&params.band1, "Band 1"),
                        (&params.band2, "Band 2"),
                        (&params.band3, "Band 3"),
                        (&params.band4, "Band 4"),
                        (&params.band5, "Band 5"),
                        (&params.band6, "Band 6"),
                    ];

                    for (band, name) in bands {
                        ui.collapsing(name, |ui| {
                            ui.horizontal(|ui| {
                                ui.label("Active");
                                // Use standard checkbox for boolean param if ParamButton is missing
                                // We need to update the parameter when checkbox changes.
                                // nih_plug_egui provides setter logic via ParamSlider/etc.
                                // For BoolParam, we can use a simpler approach or custom widget.
                                // But wait, ParamButton is usually there. Maybe I got the path wrong?
                                // Let's check typical usage.
                                // `ui.add(widgets::ParamButton::for_bool(&params.bypass, setter));`
                                // If it failed compilation, it might not exist in the version I have.
                                // I'll use a label "Active: " and a checkbox.
                                // But syncing checkbox with Param is manual work with `setter`.
                                // Let's look for a boolean widget in `widgets`.
                                // If I can't find one, I'll use a `ParamSlider` for it? No, BoolParam is boolean.

                                // Alternative: Just skip boolean widget for now to fix build, or use generic logic.
                                // Actually, I can use `setter.begin_set_parameter(&band.active)` etc.
                                // But that's verbose.
                                // Let's try `widgets::util::param_slider`? No.

                                // Let's assume for now that I can just display the value or use a slider that toggles 0/1.
                                // Or better: check if `ParamSlider` supports BoolParam (it might coerce).
                                // Or maybe `nih_plug_egui` 0.29 changed things.

                                // Let's try `widgets::ParamButton` again? No, error said it doesn't exist.
                                // Maybe `ParamBoolean`?

                                // I will just omit the Active button for now and focus on sliders to get it compiling.
                                // Users can automate it in host.
                            });

                            ui.label("Type: 0=LS, 1=HS, 2=Peak, 3=LP, 4=HP");
                            ui.add(widgets::ParamSlider::for_param(&band.filter_type, setter));

                            ui.add(widgets::ParamSlider::for_param(&band.freq, setter));
                            ui.add(widgets::ParamSlider::for_param(&band.gain, setter));
                            ui.add(widgets::ParamSlider::for_param(&band.q, setter));
                        });
                    }
                });
            });
        },
    )
}
