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
    hovered_band: Option<usize>,
    // Store the band we started dragging to keep it during the drag
    drag_start_band: Option<usize>,
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
        hovered_band: None,
        drag_start_band: None,
    }));

    create_egui_editor(
        editor_state,
        editor_data,
        |_, _| {},
        move |egui_ctx, setter, state| {
            let mut data = state.lock().unwrap();
            let sample_rate = 44100.0;

            // Dark background like Pro-Q
            egui_ctx.set_visuals(egui::Visuals::dark());

            egui::CentralPanel::default().show(egui_ctx, |ui| {
                // Get input state BEFORE the plot consumes it
                let scroll_delta = ui.input(|i| i.raw_scroll_delta.y);
                let modifiers = ui.input(|i| i.modifiers);
                let _primary_down = ui.input(|i| i.pointer.primary_down());

                // 1. MAIN PLOT AREA (full height, no bottom panel needed)
                Plot::new("my_eq_plot")
                    .view_aspect(2.0)
                    .x_axis_label("Frequency (Hz)")
                    .y_axis_label("Gain (dB)")
                    .x_grid_spacer(egui_plot::log_grid_spacer(10))
                    .include_y(-18.0)
                    .include_y(18.0)
                    .include_x(20.0)
                    .include_x(20000.0)
                    // DISABLE zoom and pan for Pro-Q style direct control
                    .allow_zoom(false)
                    .allow_drag(false)
                    .allow_scroll(false)
                    .allow_boxed_zoom(false)
                    .show(ui, |plot_ui| {
                        let bands = [
                            &params.band1, &params.band2, &params.band3,
                            &params.band4, &params.band5, &params.band6
                        ];

                        // --- A. Draw individual band response curves (Q visualization) ---
                        for (idx, band) in bands.iter().enumerate() {
                            if !band.active.value() { continue; }

                            let is_hovered = data.hovered_band == Some(idx);
                            let is_dragging = data.dragging_band == Some(idx);

                            // Draw individual band curve when hovered or dragging
                            if is_hovered || is_dragging {
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

                                let alpha = if is_dragging { 100 } else { 60 };
                                let color = egui::Color32::from_rgba_unmultiplied(
                                    BAND_COLORS[idx].r(),
                                    BAND_COLORS[idx].g(),
                                    BAND_COLORS[idx].b(),
                                    alpha
                                );
                                plot_ui.line(Line::new(band_curve).width(2.0).color(color).fill(0.0));
                            }
                        }

                        // --- B. Draw Combined Response Curve ---
                        let curve: PlotPoints = (0..600).map(|i| {
                            let x = i as f32;
                            let f = 20.0 * (1000.0f32).powf(x / 600.0);

                            let mut mag = 1.0;
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

                        plot_ui.line(Line::new(curve).width(2.5).color(egui::Color32::WHITE));

                        // --- C. Hit Testing ---
                        let pointer_pos = plot_ui.pointer_coordinate();
                        let mut new_hovered_band: Option<usize> = None;

                        if let Some(pos) = pointer_pos {
                            let mut min_dist_sq = f64::MAX;

                            for (idx, band) in bands.iter().enumerate() {
                                if !band.active.value() { continue; }

                                let freq = band.freq.value() as f64;
                                let gain = band.gain.value() as f64;

                                // Distance in normalized log-frequency and gain space
                                let dx = (freq.log10() - pos.x.max(1.0).log10()) / 0.4;
                                let dy = (gain - pos.y) / 6.0;

                                let dist_sq = dx * dx + dy * dy;

                                // Large threshold for easy grabbing
                                if dist_sq < 0.2 {
                                    if dist_sq < min_dist_sq {
                                        min_dist_sq = dist_sq;
                                        new_hovered_band = Some(idx);
                                    }
                                }
                            }
                        }

                        // Keep tracking the band we're dragging even if pointer moves away
                        if data.dragging_band.is_none() {
                            data.hovered_band = new_hovered_band;
                        }

                        // --- D. Handle Input (Pro-Q 3 Style) ---

                        // DOUBLE CLICK: Create or Delete
                        if plot_ui.response().double_clicked() {
                            if let Some(idx) = new_hovered_band {
                                // Double-click on band -> DELETE
                                setter.begin_set_parameter(&bands[idx].active);
                                setter.set_parameter(&bands[idx].active, false);
                                setter.end_set_parameter(&bands[idx].active);
                                data.dragging_band = None;
                                data.hovered_band = None;
                            } else if let Some(pos) = pointer_pos {
                                // Double-click on empty -> CREATE new band
                                if let Some((idx, band)) = bands.iter().enumerate().find(|(_, b)| !b.active.value()) {
                                    setter.begin_set_parameter(&band.active);
                                    setter.set_parameter(&band.active, true);
                                    setter.end_set_parameter(&band.active);

                                    setter.begin_set_parameter(&band.filter_type);
                                    setter.set_parameter(&band.filter_type, 2); // Peaking
                                    setter.end_set_parameter(&band.filter_type);

                                    let freq = (pos.x as f32).clamp(20.0, 20000.0);
                                    setter.begin_set_parameter(&band.freq);
                                    setter.set_parameter(&band.freq, freq);
                                    setter.end_set_parameter(&band.freq);

                                    let gain = (pos.y as f32).clamp(-24.0, 24.0);
                                    setter.begin_set_parameter(&band.gain);
                                    setter.set_parameter(&band.gain, gain);
                                    setter.end_set_parameter(&band.gain);

                                    setter.begin_set_parameter(&band.q);
                                    setter.set_parameter(&band.q, 1.0);
                                    setter.end_set_parameter(&band.q);
                                }
                            }
                        }

                        // DRAG: Immediate grab and move (no click to select first!)
                        if plot_ui.response().drag_started() {
                            if let Some(idx) = new_hovered_band {
                                data.dragging_band = Some(idx);
                                data.drag_start_band = Some(idx);
                                setter.begin_set_parameter(&bands[idx].freq);
                                setter.begin_set_parameter(&bands[idx].gain);
                            }
                        }

                        // While dragging, update position continuously
                        if let Some(idx) = data.dragging_band {
                            if let Some(pos) = pointer_pos {
                                let freq = (pos.x as f32).clamp(20.0, 20000.0);
                                let gain = (pos.y as f32).clamp(-24.0, 24.0);

                                setter.set_parameter(&bands[idx].freq, freq);
                                setter.set_parameter(&bands[idx].gain, gain);
                            }
                        }

                        if plot_ui.response().drag_stopped() {
                            if let Some(idx) = data.dragging_band {
                                setter.end_set_parameter(&bands[idx].freq);
                                setter.end_set_parameter(&bands[idx].gain);
                            }
                            data.dragging_band = None;
                            data.drag_start_band = None;
                        }

                        // --- E. Draw Handles ---
                        for (idx, band) in bands.iter().enumerate() {
                            if !band.active.value() { continue; }

                            let freq = band.freq.value();
                            let gain = band.gain.value();

                            let is_hovered = data.hovered_band == Some(idx) || new_hovered_band == Some(idx);
                            let is_dragging = data.dragging_band == Some(idx);

                            // Determine appearance based on state
                            let base_color = BAND_COLORS[idx];
                            let (color, radius) = if is_dragging {
                                (egui::Color32::WHITE, 14.0)
                            } else if is_hovered {
                                (base_color, 11.0)
                            } else {
                                (base_color, 8.0)
                            };

                            // Draw glow ring for hovered/dragging
                            if is_hovered || is_dragging {
                                let glow_alpha = if is_dragging { 150 } else { 80 };
                                let ring = Points::new(vec![[freq as f64, gain as f64]])
                                    .shape(MarkerShape::Circle)
                                    .radius(radius + 4.0)
                                    .color(egui::Color32::from_rgba_unmultiplied(
                                        base_color.r(), base_color.g(), base_color.b(), glow_alpha
                                    ));
                                plot_ui.points(ring);
                            }

                            // Draw main handle
                            let point = Points::new(vec![[freq as f64, gain as f64]])
                                .shape(MarkerShape::Circle)
                                .radius(radius)
                                .filled(true)
                                .color(color);
                            plot_ui.points(point);
                        }
                    });

                // --- SCROLL WHEEL FOR Q ADJUSTMENT ---
                if scroll_delta.abs() > 0.0 {
                    let target_band = data.dragging_band.or(data.hovered_band);
                    if let Some(idx) = target_band {
                        let bands = [
                            &params.band1, &params.band2, &params.band3,
                            &params.band4, &params.band5, &params.band6
                        ];
                        let band = bands[idx];

                        if band.active.value() {
                            let current_q = band.q.value();
                            let q_factor = if modifiers.shift { 1.02 } else { 1.1 };

                            let new_q = if scroll_delta > 0.0 {
                                (current_q * q_factor).min(10.0)
                            } else {
                                (current_q / q_factor).max(0.1)
                            };

                            setter.begin_set_parameter(&band.q);
                            setter.set_parameter(&band.q, new_q);
                            setter.end_set_parameter(&band.q);
                        }
                    }
                }

                // 2. MINIMAL INFO BAR (only shows when interacting)
                let active_band = data.dragging_band.or(data.hovered_band);
                if let Some(idx) = active_band {
                    let bands = [
                        &params.band1, &params.band2, &params.band3,
                        &params.band4, &params.band5, &params.band6
                    ];
                    let band = bands[idx];

                    if band.active.value() {
                        egui::TopBottomPanel::bottom("info")
                            .frame(egui::Frame::none().fill(egui::Color32::from_rgba_unmultiplied(30, 30, 30, 220)))
                            .show_inside(ui, |ui| {
                                ui.add_space(6.0);
                                ui.horizontal(|ui| {
                                    ui.add_space(10.0);

                                    // Color indicator
                                    let color = BAND_COLORS[idx];
                                    let (rect, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                                    ui.painter().circle_filled(rect.center(), 6.0, color);

                                    // Filter type selector
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
                                            for (val, name) in [
                                                (0, "Low Shelf"), (1, "High Shelf"), (2, "Bell"),
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

                                    // Frequency display/slider
                                    ui.label("Freq:");
                                    ui.add(widgets::ParamSlider::for_param(&band.freq, setter).with_width(100.0));

                                    // Gain display/slider
                                    ui.label("Gain:");
                                    ui.add(widgets::ParamSlider::for_param(&band.gain, setter).with_width(80.0));

                                    // Q display/slider
                                    ui.label("Q:");
                                    ui.add(widgets::ParamSlider::for_param(&band.q, setter).with_width(70.0));
                                });
                                ui.add_space(4.0);
                            });
                    }
                }
            });
        },
    )
}
