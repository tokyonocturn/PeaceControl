//! Reusable interface pieces: the band slider strip, chips and section headers.

use crate::theme::{Palette, CARD_RADIUS, SMALL_RADIUS};
use egui::{Align2, Color32, Rounding, FontId, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2};
use peace_control_core::model::{Band, MAX_GAIN_DB, MIN_GAIN_DB};

/// Interaction result of the band strip.
#[derive(Debug, Default, Clone)]
pub struct StripOutcome {
    pub changed: bool,
    pub started_edit: bool,
    pub reset_band: Option<usize>,
}

fn slider_gain_from_pointer(rect: Rect, y: f32) -> f32 {
    let t = ((rect.bottom() - y) / rect.height()).clamp(0.0, 1.0);
    let range = 18.0;
    (-range + t * range * 2.0).clamp(MIN_GAIN_DB, MAX_GAIN_DB)
}

/// Draws one vertical gain slider per band.
pub fn band_strip(
    ui: &mut Ui,
    bands: &mut [Band],
    selected: &mut Option<usize>,
    palette: &Palette,
    height: f32,
) -> StripOutcome {
    let mut outcome = StripOutcome::default();
    if bands.is_empty() {
        return outcome;
    }

    let available = ui.available_width();
    let per_band = if bands.len() <= 3 {
        64.0
    } else {
        ((available - 8.0) / bands.len() as f32).clamp(30.0, 56.0)
    };

    egui::ScrollArea::horizontal()
        .id_salt("band-strip")
        .auto_shrink([false, true])
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for (index, band) in bands.iter_mut().enumerate() {
                    let (rect, response) = ui.allocate_exact_size(
                        Vec2::new(per_band, height),
                        Sense::click_and_drag(),
                    );
                    let painter = ui.painter_at(rect);
                    let is_selected = *selected == Some(index);
                    if is_selected {
                        painter.rect_filled(
                            rect.expand2(Vec2::new(0.0, 2.0)),
                            SMALL_RADIUS,
                            palette.accent_alpha(22),
                        );
                    }

                    let label_height = 13.0;
                    let value_height = 14.0;
                    let track = Rect::from_min_max(
                        Pos2::new(rect.center().x - 2.0, rect.top() + value_height + 6.0),
                        Pos2::new(
                            rect.center().x + 2.0,
                            rect.bottom() - label_height - 6.0,
                        ),
                    );
                    let center_y = track.center().y;
                    let half = track.height() / 2.0;
                    let value = band.gain_db.clamp(-18.0, 18.0);
                    let y = center_y - (value / 18.0) * half;

                    painter.text(
                        Pos2::new(rect.center().x, rect.top() + 1.0),
                        Align2::CENTER_TOP,
                        format!("{:+.1}", band.gain_db),
                        FontId::proportional(9.5),
                        if band.gain_db.abs() < 0.05 {
                            palette.muted
                        } else {
                            palette.text
                        },
                    );

                    painter.rect_filled(track, Rounding::same(2.0), palette.grid);
                    painter.line_segment(
                        [
                            Pos2::new(rect.center().x - 6.0, center_y),
                            Pos2::new(rect.center().x + 6.0, center_y),
                        ],
                        Stroke::new(1.0_f32, palette.border_strong),
                    );
                    if band.gain_db.abs() > 0.05 {
                        let fill = Rect::from_min_max(
                            Pos2::new(track.left(), y.min(center_y)),
                            Pos2::new(track.right(), y.max(center_y)),
                        );
                        painter.rect_filled(
                            fill,
                            Rounding::same(2.0),
                            if band.gain_db >= 0.0 {
                                palette.accent
                            } else {
                                palette.warn
                            },
                        );
                    }

                    let handle = Rect::from_center_size(
                        Pos2::new(rect.center().x, y),
                        Vec2::new(per_band.min(30.0) - 6.0, 10.0),
                    );
                    let handle_color = if !band.enabled {
                        palette.muted_alpha(150)
                    } else if is_selected {
                        palette.accent
                    } else {
                        palette.accent_alpha(200)
                    };
                    painter.rect_filled(handle, Rounding::same(3.0), handle_color);
                    if is_selected {
                        painter.rect_stroke(
                            handle.expand(1.5),
                            Rounding::same(4.0),
                            Stroke::new(1.0_f32, palette.accent),
                        );
                    }

                    painter.text(
                        Pos2::new(rect.center().x, rect.bottom() - 1.0),
                        Align2::CENTER_BOTTOM,
                        crate::graph::freq_label(band.freq_hz),
                        FontId::proportional(9.0),
                        if is_selected {
                            palette.accent
                        } else {
                            palette.muted
                        },
                    );

                    if response.clicked() {
                        *selected = Some(index);
                    }
                    if response.double_clicked() || response.secondary_clicked() {
                        outcome.reset_band = Some(index);
                    }
                    if response.drag_started() {
                        *selected = Some(index);
                        outcome.started_edit = true;
                    }
                    if response.dragged() {
                        if let Some(pointer) = response.interact_pointer_pos() {
                            band.gain_db =
                                (slider_gain_from_pointer(track, pointer.y) * 10.0).round() / 10.0;
                            outcome.changed = true;
                        }
                    }
                    if response.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
                    }
                }
            });
        });

    outcome
}

/// A rounded pill label, used for channel targets and counters.
pub fn chip(ui: &mut Ui, text: &str, palette: &Palette, accent: bool) -> Response {
    let galley = ui.painter().layout_no_wrap(
        text.to_string(),
        FontId::proportional(10.5),
        if accent {
            palette.accent
        } else {
            palette.muted
        },
    );
    let size = galley.size() + Vec2::new(12.0, 5.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect_filled(
        rect,
        Rounding::same(5.0),
        if accent {
            palette.accent_alpha(26)
        } else {
            palette.card_hover
        },
    );
    ui.painter()
        .galley(rect.min + Vec2::new(6.0, 2.5), galley, palette.muted);
    response
}

/// A small uppercase section title with optional trailing widget.
pub fn section_title(ui: &mut Ui, text: &str, palette: &Palette) {
    ui.label(
        egui::RichText::new(text.to_uppercase())
            .size(10.0)
            .strong()
            .color(palette.muted),
    );
}

/// A toggle row: label on the left, switch on the right.
pub fn toggle_row(
    ui: &mut Ui,
    label: &str,
    hint: &str,
    value: &mut bool,
    palette: &Palette,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(egui::RichText::new(label).size(12.5).color(palette.text));
            if !hint.is_empty() {
                ui.label(egui::RichText::new(hint).size(10.0).color(palette.muted));
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if switch(ui, value, palette).clicked() {
                changed = true;
            }
        });
    });
    changed
}

/// A compact switch control.
pub fn switch(ui: &mut Ui, value: &mut bool, palette: &Palette) -> Response {
    let desired = Vec2::new(38.0, 20.0);
    let (rect, response) = ui.allocate_exact_size(desired, Sense::click());
    if response.clicked() {
        *value = !*value;
    }
    let on = *value;
    let track_color = if on {
        palette.accent_alpha(90)
    } else {
        palette.grid
    };
    ui.painter()
        .rect_filled(rect, Rounding::same(10.0), track_color);
    let knob_x = if on {
        rect.right() - 10.0
    } else {
        rect.left() + 10.0
    };
    ui.painter().circle_filled(
        Pos2::new(knob_x, rect.center().y),
        7.0,
        if on { palette.accent } else { palette.muted },
    );
    response
}

/// A filled primary button.
pub fn primary_button(ui: &mut Ui, label: &str, enabled: bool, palette: &Palette) -> Response {
    let text_color = if enabled {
        palette.accent_text
    } else {
        palette.muted
    };
    let fill = if enabled {
        palette.accent
    } else {
        palette.card_hover
    };
    ui.add_enabled(
        enabled,
        egui::Button::new(egui::RichText::new(label).size(12.5).strong().color(text_color))
            .fill(fill)
            .rounding(CARD_RADIUS)
            .min_size(Vec2::new(0.0, 30.0)),
    )
}

/// A secondary (outlined) button.
pub fn ghost_button(ui: &mut Ui, label: &str, palette: &Palette) -> Response {
    ui.add(
        egui::Button::new(egui::RichText::new(label).size(12.0).color(palette.text))
            .fill(palette.card)
            .stroke(Stroke::new(1.0_f32, palette.border))
            .rounding(CARD_RADIUS)
            .min_size(Vec2::new(0.0, 30.0)),
    )
}

/// A coloured badge with a dot, for the connection state.
pub fn badge(ui: &mut Ui, text: &str, color: Color32, palette: &Palette) {
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), FontId::proportional(10.5), color);
    let size = galley.size() + Vec2::new(20.0, 6.0);
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter()
        .rect_filled(rect, Rounding::same(9.0), palette.card_hover);
    ui.painter()
        .circle_filled(Pos2::new(rect.left() + 9.0, rect.center().y), 3.0, color);
    ui.painter()
        .galley(rect.min + Vec2::new(15.0, 3.0), galley, color);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pointer_maps_to_full_gain_range() {
        let rect = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(10.0, 100.0));
        assert!(slider_gain_from_pointer(rect, rect.top()) > 17.0);
        assert!(slider_gain_from_pointer(rect, rect.bottom()) < -17.0);
        assert!(slider_gain_from_pointer(rect, rect.center().y).abs() < 0.5);
    }
}
