use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, egui, EguiState, widgets};
use std::sync::{Arc, Mutex};
use crate::MyEqParams;
use egui_plot::{Line, Plot, PlotPoints, Points, MarkerShape};

pub fn default_state() -> Arc<EguiState> {
    EguiState::from_size(1000, 600)
}

struct EditorData {
    dragging_band: Option<usize>,
    was_dragging: bool,
}

// Band colors similar to Pro-Q (distinct colors for each band)
const BAND_COLORS: [egui::Color32; 6] = [
    egui::Color32::from_rgb(255, 100, 100),  // Red
    egui::Color32::from_rgb(255, 180, 80),   // Orange
    egui::Color32::from_rgb(255, 255, 100),  // Yellow
    egui::Color32::from_rgb(100, 255, 100),  // Green
    egui::Color32::from_rgb(100, 180, 255),  // Blue
    egui::Color32::from_rgb(200, 120, 255),  // Purple
];

pub fn create(
    params: Arc<MyEqParams>,
    editor_state: Arc<EguiState>,
) -> Option<Box<dyn Editor>> {
    let editor_data = Arc::new(Mutex::new(EditorData {
        dragging_band: None,
        was_dragging: false,
    }));

    create_egui_editor(
        editor_state,
        editor_data,
        |_, _| {},
        move |egui_ctx, setter, state| {
            let mut data = state.lock().unwrap();
            let sample_rate = 44100.0;

            egui_ctx.set_visuals(egui::Visuals::dark());

            // Get global input state
            let primary_down = egui_ctx.input(|i| i.pointer.primary_down());
            let primary_pressed = egui_ctx.input(|i| i.pointer.primary_pressed());
            let primary_released = egui_ctx.input(|i| i.pointer.primary_released());
            let scroll_delta = egui_ctx.input(|i| i.raw_scroll_delta.y);
            let modifiers = egui_ctx.input(|i| i.modifiers);
            let double_clicked = egui_ctx.input(|i| i.pointer.button_double_clicked(egui::PointerButton::Primary));

            egui::CentralPanel::default().show(egui_ctx, |ui| {
                let bands = [
                    &params.band1, &params.band2, &params.band3,
                    &params.band4, &params.band5, &params.band6
                ];

                // Store hovered band for use after plot
                let mut current_hovered: Option<usize> = None;
                let mut current_pointer_pos: Option<[f64; 2]> = None;

                Plot::new("my_eq_plot")
                    .view_aspect(2.0)
                    .x_axis_label("Frequency (Hz)")
                    .y_axis_label("Gain (dB)")
                    .x_grid_spacer(egui_plot::log_grid_spacer(10))
                    .include_y(-18.0)
                    .include_y(18.0)
                    .include_x(20.0)
                    .include_x(20000.0)
                    .allow_zoom(false)
                    .allow_drag(false)
                    .allow_scroll(false)
                    .allow_boxed_zoom(false)
                    .show(ui, |plot_ui| {
                        // --- A. Draw individual band curves for hovered/dragged ---
                        for (idx, band) in bands.iter().enumerate() {
                            if !band.active.value() { continue; }

                            let is_dragging = data.dragging_band == Some(idx);

                            if is_dragging {
                                let band_curve: PlotPoints = (0..300).map(|i| {
                                    let x = i as f32;
                                    let f = 20.0 * (1000.0f32).powf(x / 300.0);
                                    use crate::biquad::{Biquad, FilterType};
                                    let ft = match band.filter_type.value() {
                                        0 => FilterType::LowShelf,
                                        1 => FilterType::HighShelf,
                                        2 => FilterType::Peaking,
                                        3 => FilterType::LowPass,
                                        4 => FilterType::HighPass,
                                        _ => FilterType::Peaking,
                                    };
                                    let mut b = Biquad::new();
                                    b.update(ft, sample_rate, band.freq.value(), band.q.value(), band.gain.value());
                                    let mag = b.magnitude(f, sample_rate);
                                    let db = 20.0 * mag.log10();
                                    [f as f64, db as f64]
                                }).collect();

                                let color = egui::Color32::from_rgba_unmultiplied(
                                    BAND_COLORS[idx].r(),
                                    BAND_COLORS[idx].g(),
                                    BAND_COLORS[idx].b(),
                                    100
                                );
                                plot_ui.line(Line::new(band_curve).width(2.0).color(color).fill(0.0));
                            }
                        }

                        // --- B. Draw Combined Response Curve ---
                        let curve: PlotPoints = (0..600).map(|i| {
                            let x = i as f32;
                            let f = 20.0 * (1000.0f32).powf(x / 600.0);
                            let mut mag = 1.0;

                            for band in bands {
                                if !band.active.value() { continue; }
                                use crate::biquad::{Biquad, FilterType};
                                let ft = match band.filter_type.value() {
                                    0 => FilterType::LowShelf,
                                    1 => FilterType::HighShelf,
                                    2 => FilterType::Peaking,
                                    3 => FilterType::LowPass,
                                    4 => FilterType::HighPass,
                                    _ => FilterType::Peaking,
                                };
                                let mut b = Biquad::new();
                                b.update(ft, sample_rate, band.freq.value(), band.q.value(), band.gain.value());
                                mag *= b.magnitude(f, sample_rate);
                            }

                            let db = 20.0 * mag.log10();
                            [f as f64, db as f64]
                        }).collect();

                        plot_ui.line(Line::new(curve).width(2.5).color(egui::Color32::WHITE));

                        // --- C. Hit Testing ---
                        if let Some(pos) = plot_ui.pointer_coordinate() {
                            current_pointer_pos = Some([pos.x, pos.y]);
                            let mut min_dist_sq = f64::MAX;

                            for (idx, band) in bands.iter().enumerate() {
                                if !band.active.value() { continue; }

                                let freq = band.freq.value() as f64;
                                let gain = band.gain.value() as f64;

                                let dx = (freq.log10() - pos.x.max(1.0).log10()) / 0.3;
                                let dy = (gain - pos.y) / 5.0;
                                let dist_sq = dx * dx + dy * dy;

                                if dist_sq < 0.25 && dist_sq < min_dist_sq {
                                    min_dist_sq = dist_sq;
                                    current_hovered = Some(idx);
                                }
                            }
                        }

                        // --- D. Draw Handles (ALWAYS for active bands) ---
                        for (idx, band) in bands.iter().enumerate() {
                            if !band.active.value() { continue; }

                            let freq = band.freq.value();
                            let gain = band.gain.value();
                            let is_hovered = current_hovered == Some(idx);
                            let is_dragging = data.dragging_band == Some(idx);

                            let base_color = BAND_COLORS[idx];
                            let (color, radius) = if is_dragging {
                                (egui::Color32::WHITE, 14.0)
                            } else if is_hovered {
                                (base_color, 12.0)
                            } else {
                                (base_color, 9.0)
                            };

                            // Glow ring
                            if is_hovered || is_dragging {
                                let glow = Points::new(vec![[freq as f64, gain as f64]])
                                    .shape(MarkerShape::Circle)
                                    .radius(radius + 5.0)
                                    .color(egui::Color32::from_rgba_unmultiplied(
                                        base_color.r(), base_color.g(), base_color.b(),
                                        if is_dragging { 180 } else { 100 }
                                    ));
                                plot_ui.points(glow);
                            }

                            // Main dot
                            let point = Points::new(vec![[freq as f64, gain as f64]])
                                .shape(MarkerShape::Circle)
                                .radius(radius)
                                .filled(true)
                                .color(color);
                            plot_ui.points(point);
                        }
                    });

                // --- Handle mouse interactions OUTSIDE the plot closure ---

                // DOUBLE CLICK: Create or Delete
                if double_clicked {
                    if let Some(idx) = current_hovered {
                        // Delete band
                        setter.begin_set_parameter(&bands[idx].active);
                        setter.set_parameter(&bands[idx].active, false);
                        setter.end_set_parameter(&bands[idx].active);
                        data.dragging_band = None;
                    } else if let Some(pos) = current_pointer_pos {
                        // Create new band
                        if let Some((idx, band)) = bands.iter().enumerate().find(|(_, b)| !b.active.value()) {
                            setter.begin_set_parameter(&band.active);
                            setter.set_parameter(&band.active, true);
                            setter.end_set_parameter(&band.active);

                            setter.begin_set_parameter(&band.filter_type);
                            setter.set_parameter(&band.filter_type, 2);
                            setter.end_set_parameter(&band.filter_type);

                            setter.begin_set_parameter(&band.freq);
                            setter.set_parameter(&band.freq, (pos[0] as f32).clamp(20.0, 20000.0));
                            setter.end_set_parameter(&band.freq);

                            setter.begin_set_parameter(&band.gain);
                            setter.set_parameter(&band.gain, (pos[1] as f32).clamp(-24.0, 24.0));
                            setter.end_set_parameter(&band.gain);

                            setter.begin_set_parameter(&band.q);
                            setter.set_parameter(&band.q, 1.0);
                            setter.end_set_parameter(&band.q);

                            data.dragging_band = Some(idx);
                        }
                    }
                }

                // START DRAG: Mouse pressed on a band
                if primary_pressed && !double_clicked {
                    if let Some(idx) = current_hovered {
                        data.dragging_band = Some(idx);
                        data.was_dragging = false;
                        setter.begin_set_parameter(&bands[idx].freq);
                        setter.begin_set_parameter(&bands[idx].gain);
                    }
                }

                // DURING DRAG: Update position
                if primary_down {
                    if let Some(idx) = data.dragging_band {
                        if let Some(pos) = current_pointer_pos {
                            data.was_dragging = true;
                            setter.set_parameter(&bands[idx].freq, (pos[0] as f32).clamp(20.0, 20000.0));
                            setter.set_parameter(&bands[idx].gain, (pos[1] as f32).clamp(-24.0, 24.0));
                        }
                    }
                }

                // END DRAG: Mouse released
                if primary_released {
                    if let Some(idx) = data.dragging_band {
                        setter.end_set_parameter(&bands[idx].freq);
                        setter.end_set_parameter(&bands[idx].gain);
                    }
                    data.dragging_band = None;
                }

                // SCROLL: Adjust Q
                if scroll_delta.abs() > 0.0 {
                    let target = data.dragging_band.or(current_hovered);
                    if let Some(idx) = target {
                        let band = bands[idx];
                        if band.active.value() {
                            let q = band.q.value();
                            let factor = if modifiers.shift { 1.02 } else { 1.1 };
                            let new_q = if scroll_delta > 0.0 {
                                (q * factor).min(10.0)
                            } else {
                                (q / factor).max(0.1)
                            };
                            setter.begin_set_parameter(&band.q);
                            setter.set_parameter(&band.q, new_q);
                            setter.end_set_parameter(&band.q);
                        }
                    }
                }

                // INFO BAR (only when dragging)
                if let Some(idx) = data.dragging_band {
                    let band = bands[idx];
                    if band.active.value() {
                        egui::TopBottomPanel::bottom("info")
                            .frame(egui::Frame::none().fill(egui::Color32::from_rgba_unmultiplied(20, 20, 20, 240)))
                            .show_inside(ui, |ui| {
                                ui.add_space(8.0);
                                ui.horizontal(|ui| {
                                    ui.add_space(15.0);

                                    let color = BAND_COLORS[idx];
                                    let (rect, _) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
                                    ui.painter().circle_filled(rect.center(), 7.0, color);

                                    egui::ComboBox::from_id_salt("type")
                                        .width(90.0)
                                        .selected_text(match band.filter_type.value() {
                                            0 => "Low Shelf",
                                            1 => "High Shelf",
                                            2 => "Bell",
                                            3 => "Low Pass",
                                            4 => "High Pass",
                                            _ => "Bell",
                                        })
                                        .show_ui(ui, |ui| {
                                            for (val, name) in [(0, "Low Shelf"), (1, "High Shelf"), (2, "Bell"), (3, "Low Pass"), (4, "High Pass")] {
                                                if ui.selectable_label(band.filter_type.value() == val, name).clicked() {
                                                    setter.begin_set_parameter(&band.filter_type);
                                                    setter.set_parameter(&band.filter_type, val);
                                                    setter.end_set_parameter(&band.filter_type);
                                                }
                                            }
                                        });

                                    ui.separator();
                                    ui.label("Freq:");
                                    ui.add(widgets::ParamSlider::for_param(&band.freq, setter).with_width(100.0));
                                    ui.label("Gain:");
                                    ui.add(widgets::ParamSlider::for_param(&band.gain, setter).with_width(80.0));
                                    ui.label("Q:");
                                    ui.add(widgets::ParamSlider::for_param(&band.q, setter).with_width(70.0));
                                });
                                ui.add_space(6.0);
                            });
                    }
                }
            });
        },
    )
}
