//! The interactive frequency response graph.
//!
//! Draws the predicted sum of every enabled band on a logarithmic frequency
//! axis and lets the user drag band handles: vertically for gain, and
//! horizontally while holding Shift for frequency.

use crate::theme::Palette;
use egui::{Align2, Color32, FontId, Pos2, Rect, Rounding, Sense, Stroke, Ui, Vec2};
use peace_control_core::dsp;
use peace_control_core::model::Band;
use peace_control_core::render::num;

const MIN_HZ: f32 = 20.0;
const MAX_HZ: f32 = 20_000.0;

/// What happened in the graph during this frame.
#[derive(Debug, Default, Clone)]
pub struct GraphOutcome {
    pub changed: bool,
    pub started_edit: bool,
    pub clicked_band: Option<usize>,
}

fn freq_to_x(rect: Rect, freq: f32) -> f32 {
    let lo = MIN_HZ.log10();
    let hi = MAX_HZ.log10();
    let t = (freq.clamp(MIN_HZ, MAX_HZ).log10() - lo) / (hi - lo);
    rect.left() + rect.width() * t
}

fn x_to_freq(rect: Rect, x: f32) -> f32 {
    let lo = MIN_HZ.log10();
    let hi = MAX_HZ.log10();
    let t = ((x - rect.left()) / rect.width()).clamp(0.0, 1.0);
    10f32.powf(lo + (hi - lo) * t)
}

fn db_to_y(rect: Rect, db: f32, range: f32) -> f32 {
    let t = ((db.clamp(-range, range) + range) / (range * 2.0)).clamp(0.0, 1.0);
    rect.bottom() - rect.height() * t
}

fn y_to_db(rect: Rect, y: f32, range: f32) -> f32 {
    let t = ((rect.bottom() - y) / rect.height()).clamp(0.0, 1.0);
    -range + t * range * 2.0
}

/// Formats a frequency for an axis label.
pub fn freq_label(freq: f32) -> String {
    if freq >= 1000.0 {
        let k = freq / 1000.0;
        if (k - k.round()).abs() < 0.05 {
            format!("{}k", k.round() as i32)
        } else {
            format!("{k:.1}k")
        }
    } else {
        format!("{}", freq.round() as i32)
    }
}

/// Draws the graph and handles interaction. Returns what changed.
#[allow(clippy::too_many_arguments)]
pub fn show(
    ui: &mut Ui,
    bands: &mut [Band],
    selected: &mut Option<usize>,
    preamp_db: f32,
    show_preamp: bool,
    range_db: f32,
    height: f32,
    palette: &Palette,
) -> GraphOutcome {
    let width = ui.available_width().max(120.0);
    let (outer, response) =
        ui.allocate_exact_size(Vec2::new(width, height), Sense::click_and_drag());
    let rect = outer.shrink2(Vec2::new(34.0, 12.0));
    let painter = ui.painter_at(outer);

    painter.rect_filled(outer, Rounding::same(10.0), palette.bg);
    painter.rect_stroke(
        outer,
        Rounding::same(10.0),
        Stroke::new(1.0_f32, palette.border),
    );

    // Grid: horizontal dB lines and vertical frequency lines.
    let step = if range_db <= 12.0 {
        3.0
    } else if range_db <= 24.0 {
        6.0
    } else {
        12.0
    };
    let mut db = -range_db;
    while db <= range_db + 0.01 {
        let y = db_to_y(rect, db, range_db);
        let is_zero = db.abs() < 0.01;
        painter.line_segment(
            [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
            Stroke::new(
                if is_zero { 1.2_f32 } else { 0.6_f32 },
                if is_zero {
                    palette.border_strong
                } else {
                    palette.grid
                },
            ),
        );
        painter.text(
            Pos2::new(rect.left() - 8.0, y),
            Align2::RIGHT_CENTER,
            format!("{}{}", if db > 0.0 { "+" } else { "" }, db.round() as i32),
            FontId::proportional(9.5),
            palette.muted,
        );
        db += step;
    }

    for freq in [
        20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10_000.0, 20_000.0,
    ] {
        let x = freq_to_x(rect, freq);
        painter.line_segment(
            [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
            Stroke::new(0.6_f32, palette.grid),
        );
        painter.text(
            Pos2::new(x, outer.bottom() - 1.0),
            Align2::CENTER_BOTTOM,
            freq_label(freq),
            FontId::proportional(9.0),
            palette.muted,
        );
    }

    // Response curve.
    let offset = if show_preamp { preamp_db } else { 0.0 };
    let curve = dsp::response_curve(bands, MIN_HZ, MAX_HZ, 260);
    let points: Vec<Pos2> = curve
        .iter()
        .map(|(freq, gain)| {
            Pos2::new(
                freq_to_x(rect, *freq),
                db_to_y(rect, (*gain as f32) + offset, range_db),
            )
        })
        .collect();

    if points.len() >= 2 {
        let mut mesh = egui::Mesh::default();
        let base = db_to_y(rect, 0.0, range_db);
        let top_color = palette.accent_alpha(58);
        let bottom_color = palette.accent_alpha(6);
        for point in &points {
            mesh.vertices.push(egui::epaint::Vertex {
                pos: *point,
                uv: egui::epaint::WHITE_UV,
                color: top_color,
            });
        }
        let base_start = mesh.vertices.len() as u32;
        for point in &points {
            mesh.vertices.push(egui::epaint::Vertex {
                pos: Pos2::new(point.x, base),
                uv: egui::epaint::WHITE_UV,
                color: bottom_color,
            });
        }
        for index in 0..points.len().saturating_sub(1) {
            let a = index as u32;
            let b = a + 1;
            let c = b + base_start;
            let d = a + base_start;
            mesh.add_triangle(a, b, c);
            mesh.add_triangle(a, c, d);
        }
        painter.add(egui::Shape::mesh(mesh));
        painter.add(egui::Shape::line(
            points.clone(),
            Stroke::new(2.0_f32, palette.accent),
        ));
    }

    // Band handles.
    let pointer = response.hover_pos();

    let mut nearest: Option<(usize, f32)> = None;
    for (index, band) in bands.iter().enumerate() {
        let pos = Pos2::new(
            freq_to_x(rect, band.freq_hz),
            db_to_y(rect, band.gain_db + offset, range_db),
        );
        if let Some(pointer) = pointer {
            let distance = pos.distance(pointer);
            if distance < 16.0 && nearest.map(|(_, d)| distance < d).unwrap_or(true) {
                nearest = Some((index, distance));
            }
        }
        let is_selected = *selected == Some(index);
        let color = if !band.enabled {
            palette.muted_alpha(120)
        } else if is_selected {
            palette.accent
        } else {
            palette.accent_alpha(190)
        };
        if is_selected {
            painter.circle_filled(pos, 9.0, palette.accent_alpha(40));
        }
        painter.circle_filled(pos, if is_selected { 5.5 } else { 4.0 }, color);
        painter.circle_stroke(
            pos,
            if is_selected { 5.5 } else { 4.0 },
            Stroke::new(1.0_f32, palette.bg),
        );
    }
    let hovered: Option<usize> = nearest.map(|(index, _)| index);
    if hovered.is_some() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    let mut outcome = GraphOutcome::default();

    if response.clicked() {
        outcome.clicked_band = nearest.map(|(index, _)| index);
        *selected = outcome.clicked_band;
    }

    if response.drag_started() {
        if let Some((index, _)) = nearest {
            *selected = Some(index);
            outcome.started_edit = true;
        }
    }

    if response.dragged() {
        if let (Some(index), Some(pointer)) = (*selected, response.interact_pointer_pos()) {
            if let Some(band) = bands.get_mut(index) {
                let shift = ui.input(|input| input.modifiers.shift);
                if shift {
                    band.freq_hz =
                        peace_control_core::model::round_frequency(x_to_freq(rect, pointer.x));
                } else {
                    band.gain_db = y_to_db(rect, pointer.y, range_db).clamp(-30.0, 30.0);
                }
                outcome.changed = true;
            }
        }
    }

    if response.secondary_clicked() {
        if let Some((index, _)) = nearest {
            if let Some(band) = bands.get_mut(index) {
                band.gain_db = 0.0;
                outcome.changed = true;
                outcome.started_edit = true;
            }
        }
    }

    // Hover tooltip drawn manually so it follows the handle.
    if let Some(index) = hovered {
        if let Some(band) = bands.get(index) {
            let text = format!(
                "{} Hz   {:+.1} dB   Q {:.2}",
                num(band.freq_hz),
                band.gain_db,
                band.q
            );
            let pos = Pos2::new(
                freq_to_x(rect, band.freq_hz),
                db_to_y(rect, band.gain_db + offset, range_db) - 14.0,
            );
            let galley = painter.layout_no_wrap(text, FontId::proportional(10.5), palette.text);
            let size = galley.size() + Vec2::new(12.0, 6.0);
            let mut rect = Rect::from_center_size(pos - Vec2::new(0.0, size.y * 0.5), size);
            rect = rect.translate(Vec2::new(
                (outer.right() - rect.right()).min(0.0),
                (outer.top() - rect.top()).max(0.0),
            ));
            painter.rect_filled(rect, Rounding::same(6.0), palette.card);
            painter.rect_stroke(
                rect,
                Rounding::same(6.0),
                Stroke::new(1.0_f32, palette.border_strong),
            );
            painter.galley(rect.min + Vec2::new(6.0, 3.0), galley, palette.text);
        }
    }

    // Corner hint.
    if hovered.is_none() {
        painter.text(
            Pos2::new(outer.right() - 10.0, outer.top() + 8.0),
            Align2::RIGHT_TOP,
            "drag: gain   shift+drag: frequency",
            FontId::proportional(9.5),
            palette.muted_alpha(170),
        );
    }

    let _ = Color32::TRANSPARENT;
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect() -> Rect {
        Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(400.0, 200.0))
    }

    #[test]
    fn frequency_mapping_round_trips() {
        let rect = rect();
        for freq in [20.0, 100.0, 1000.0, 20_000.0] {
            let x = freq_to_x(rect, freq);
            let back = x_to_freq(rect, x);
            assert!((back - freq).abs() / freq < 0.02, "{freq} -> {back}");
        }
    }

    #[test]
    fn zero_db_sits_in_the_middle() {
        let rect = rect();
        let y = db_to_y(rect, 0.0, 24.0);
        assert!((y - rect.center().y).abs() < 0.001);
    }

    #[test]
    fn db_mapping_round_trips() {
        let rect = rect();
        for db in [-24.0, -6.0, 0.0, 6.0, 24.0] {
            let y = db_to_y(rect, db, 24.0);
            let back = y_to_db(rect, y, 24.0);
            assert!((back - db).abs() < 0.001);
        }
    }

    #[test]
    fn labels_are_compact() {
        assert_eq!(freq_label(1000.0), "1k");
        assert_eq!(freq_label(20_000.0), "20k");
        assert_eq!(freq_label(250.0), "250");
        assert_eq!(freq_label(1500.0), "1.5k");
    }
}
