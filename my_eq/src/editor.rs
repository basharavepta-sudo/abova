use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, egui, EguiState, widgets};
use std::sync::{Arc, Mutex};
use crate::MyEqParams;
use egui_plot::{Line, Plot, PlotPoints, Points, PlotPoint, MarkerShape};

pub fn default_state() -> Arc<EguiState> {
    EguiState::from_size(1000, 600)
}

struct EditorData {
    selected_band: Option<usize>,
    dragging_band: Option<usize>,
    last_mouse_pos: Option<PlotPoint>,
}

pub fn create(
    params: Arc<MyEqParams>,
    editor_state: Arc<EguiState>,
) -> Option<Box<dyn Editor>> {
    // We use a Mutex for internal UI state that doesn't need persistence across sessions
    // (though selection could be persisted, it's fine to reset).
    let editor_data = Arc::new(Mutex::new(EditorData {
        selected_band: None,
        dragging_band: None,
        last_mouse_pos: None,
    }));

    create_egui_editor(
        editor_state,
        editor_data,
        |_, _| {},
        move |egui_ctx, setter, state| {
            let mut data = state.lock().unwrap();
            let sample_rate = 44100.0; // Assume 44.1k for viz

            egui::CentralPanel::default().show(egui_ctx, |ui| {
                // 1. TOP PLOT AREA
                let plot_response = Plot::new("my_eq_plot")
                    .view_aspect(2.0)
                    .x_axis_label("Frequency (Hz)")
                    .y_axis_label("Gain (dB)")
                    .x_grid_spacer(egui_plot::log_grid_spacer(10))
                    .include_y(-15.0)
                    .include_y(15.0)
                    .include_x(20.0)
                    .include_x(20000.0)
                    .show(ui, |plot_ui| {
                        // --- A. Draw Response Curve ---
                        let curve: PlotPoints = (0..600).map(|i| {
                            let x = i as f32;
                            // Log mapping 20Hz -> 20kHz
                            let f = 20.0 * (1000.0f32).powf(x / 600.0);

                            let mut mag = 1.0;
                            // Calculate response
                            let bands = [
                                &params.band1, &params.band2, &params.band3,
                                &params.band4, &params.band5, &params.band6
                            ];
                             // Helper (same as before)
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

                        plot_ui.line(Line::new(curve).width(2.0).color(egui::Color32::LIGHT_BLUE));

                        // --- B. Draw Handles & Hit Testing ---
                        let bands = [
                            &params.band1, &params.band2, &params.band3,
                            &params.band4, &params.band5, &params.band6
                        ];

                        let pointer_pos = plot_ui.pointer_coordinate();
                        let mut hovered_band = None;

                        // Check for hover/interaction
                        if let Some(pos) = pointer_pos {
                            let mut min_dist_sq = f64::MAX;

                            for (idx, band) in bands.iter().enumerate() {
                                if !band.active.value() { continue; }

                                let freq = band.freq.value() as f64;
                                let gain = band.gain.value() as f64;

                                // Calculate distance in "screen" logic roughly
                                // Log frequency distance is better for feeling
                                let dx = (freq.log10() - pos.x.log10()).abs();
                                // gain distance
                                let dy = (gain - pos.y).abs() / 10.0; // scale gain diff to be comparable

                                let dist_sq = dx*dx + dy*dy;
                                // Threshold: 0.1 log-freq units is reasonable?
                                if dist_sq < 0.005 { // Tuning needed
                                    if dist_sq < min_dist_sq {
                                        min_dist_sq = dist_sq;
                                        hovered_band = Some(idx);
                                    }
                                }
                            }
                        }

                        // Handle Input
                        if plot_ui.response().clicked() {
                            if let Some(idx) = hovered_band {
                                data.selected_band = Some(idx);
                            } else {
                                // Clicked background -> Deselect? Or Create?
                                // Let's deselect if single click background
                                // But prevent deselect if we just created
                                // data.selected_band = None;
                            }
                        }

                        if plot_ui.response().double_clicked() {
                            if let Some(idx) = hovered_band {
                                // Deactivate band
                                setter.begin_set_parameter(&bands[idx].active);
                                setter.set_parameter(&bands[idx].active, false);
                                setter.end_set_parameter(&bands[idx].active);
                                data.selected_band = None;
                            } else if let Some(pos) = pointer_pos {
                                // Create new band
                                // Find first inactive
                                if let Some((idx, band)) = bands.iter().enumerate().find(|(_, b)| !b.active.value()) {
                                    setter.begin_set_parameter(&band.active);
                                    setter.set_parameter(&band.active, true);
                                    setter.end_set_parameter(&band.active);

                                    setter.begin_set_parameter(&band.freq);
                                    setter.set_parameter(&band.freq, pos.x as f32);
                                    setter.end_set_parameter(&band.freq);

                                    setter.begin_set_parameter(&band.gain);
                                    setter.set_parameter(&band.gain, pos.y as f32);
                                    setter.end_set_parameter(&band.gain);

                                    data.selected_band = Some(idx);
                                }
                            }
                        }

                        // Dragging Logic
                        if plot_ui.response().drag_started() {
                            if let Some(idx) = hovered_band {
                                data.dragging_band = Some(idx);
                                data.selected_band = Some(idx);
                                // Start param change?
                                setter.begin_set_parameter(&bands[idx].freq);
                                setter.begin_set_parameter(&bands[idx].gain);
                            }
                        }

                        if plot_ui.response().drag_stopped() {
                             if let Some(idx) = data.dragging_band {
                                setter.end_set_parameter(&bands[idx].freq);
                                setter.end_set_parameter(&bands[idx].gain);
                             }
                             data.dragging_band = None;
                        }

                        if let Some(idx) = data.dragging_band {
                            if let Some(pos) = pointer_pos {
                                // Update Params
                                setter.set_parameter(&bands[idx].freq, pos.x as f32);
                                setter.set_parameter(&bands[idx].gain, pos.y as f32);
                            }
                        }

                        // Scroll Logic for Q
                        if let Some(idx) = hovered_band {
                             // This requires access to input events, plot_ui doesn't expose scroll easily directly?
                             // plot_ui.response().hovered() is true.
                             // We can check ui.input().scroll_delta
                        }
                        // Note: capturing scroll inside Plot is tricky as Plot consumes it for zoom/pan.
                        // We disabled zoom/pan? Not yet. Plot allows scrolling.
                        // If we want Q control, we might need Ctrl+Scroll or similar, or disable zoom.

                        // Draw Handles
                        for (idx, band) in bands.iter().enumerate() {
                            if !band.active.value() { continue; }

                            let freq = band.freq.value();
                            let gain = band.gain.value();

                            let is_selected = data.selected_band == Some(idx);
                            let color = if is_selected { egui::Color32::WHITE } else { egui::Color32::YELLOW };

                            let point = Points::new(vec![[freq as f64, gain as f64]])
                                .shape(MarkerShape::Circle)
                                .radius(if is_selected { 6.0 } else { 4.0 })
                                .color(color);

                            plot_ui.points(point);
                        }
                    });

                // 2. BOTTOM CONTROL PANEL (Context Sensitive)
                egui::TopBottomPanel::bottom("controls").show_inside(ui, |ui| {
                     ui.add_space(5.0);
                     if let Some(idx) = data.selected_band {
                        let bands = [
                            &params.band1, &params.band2, &params.band3,
                            &params.band4, &params.band5, &params.band6
                        ];
                        let band = bands[idx];

                        ui.horizontal(|ui| {
                            ui.label(format!("Band {}", idx + 1));

                            ui.separator();

                            // Type Selector
                            // Ideally a ComboBox
                            egui::ComboBox::from_label("Type")
                                .selected_text(match band.filter_type.value() {
                                    0 => "Low Shelf",
                                    1 => "High Shelf",
                                    2 => "Peaking",
                                    3 => "Low Pass",
                                    4 => "High Pass",
                                    _ => "Unknown",
                                })
                                .show_ui(ui, |ui| {
                                    for (val, name) in [
                                        (0, "Low Shelf"), (1, "High Shelf"), (2, "Peaking"),
                                        (3, "Low Pass"), (4, "High Pass")
                                    ] {
                                        if ui.selectable_label(band.filter_type.value() == val, name).clicked() {
                                            setter.begin_set_parameter(&band.filter_type);
                                            setter.set_parameter(&band.filter_type, val);
                                            setter.end_set_parameter(&band.filter_type);
                                        }
                                    }
                                });

                            ui.separator();
                            ui.add(widgets::ParamSlider::for_param(&band.freq, setter).with_width(100.0));
                            ui.add(widgets::ParamSlider::for_param(&band.gain, setter).with_width(80.0));
                            ui.add(widgets::ParamSlider::for_param(&band.q, setter).with_width(80.0));

                            ui.separator();
                            if ui.button("Delete").clicked() {
                                setter.begin_set_parameter(&band.active);
                                setter.set_parameter(&band.active, false);
                                setter.end_set_parameter(&band.active);
                                data.selected_band = None;
                            }
                        });
                     } else {
                         ui.centered_and_justified(|ui| {
                             ui.label("Double-click graph to add band. Select point to edit.");
                         });
                     }
                     ui.add_space(5.0);
                });
            });
        },
    )
}
