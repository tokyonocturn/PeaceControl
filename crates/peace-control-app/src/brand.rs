//! Original PeaceControl branding.
//!
//! The mark is three amplitude bars inside a softly rounded control plate,
//! drawn procedurally so the application ships no binary art assets and every
//! size stays crisp.

use crate::theme::Palette;
use egui::{Color32, Pos2, Rect, Response, Rounding, Sense, Stroke, Ui, Vec2};

/// Draws the logo mark at `size` points.
pub fn logo(ui: &mut Ui, size: f32, palette: &Palette) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    paint_logo(ui.painter(), rect, palette);
    response
}

/// Paints the mark into an existing rect.
pub fn paint_logo(painter: &egui::Painter, rect: Rect, palette: &Palette) {
    let radius = Rounding::same((rect.width() * 0.26).clamp(3.0, 22.0));
    painter.rect_filled(rect, radius, palette.accent_alpha(38));
    painter.rect_stroke(
        rect,
        radius,
        Stroke::new(1.2_f32, palette.accent_alpha(150)),
    );

    let pad = rect.width() * 0.2;
    let inner = rect.shrink(pad);
    let gap = inner.width() * 0.16;
    let bar_width = (inner.width() - gap * 2.0) / 3.0;
    let heights = [0.46f32, 0.94, 0.64];

    for (index, factor) in heights.iter().enumerate() {
        let height = inner.height() * factor;
        let x = inner.left() + index as f32 * (bar_width + gap);
        let bar = Rect::from_min_size(
            Pos2::new(x, inner.bottom() - height),
            Vec2::new(bar_width, height),
        );
        painter.rect_filled(bar, Rounding::same(bar_width * 0.42), palette.accent);
    }
}

/// The product wordmark with its tagline.
pub fn wordmark(ui: &mut Ui, palette: &Palette, compact: bool) {
    ui.horizontal(|ui| {
        let size = if compact { 26.0 } else { 32.0 };
        logo(ui, size, palette);
        ui.add_space(2.0);
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.label(
                    egui::RichText::new("Peace")
                        .size(if compact { 16.0 } else { 19.0 })
                        .strong()
                        .color(palette.text),
                );
                ui.label(
                    egui::RichText::new("Control")
                        .size(if compact { 16.0 } else { 19.0 })
                        .strong()
                        .color(palette.accent),
                );
            });
            if !compact {
                ui.label(
                    egui::RichText::new(crate::SHORT_TAGLINE)
                        .size(10.5)
                        .color(palette.muted),
                );
            }
        });
    });
}

/// Renders the logo into an RGBA buffer for the window icon.
pub fn icon_rgba(size: u32) -> (Vec<u8>, u32, u32) {
    let mut pixels = vec![0u8; (size * size * 4) as usize];
    let s = size as f32;
    let accent = (94u8, 234u8, 212u8);

    for y in 0..size {
        for x in 0..size {
            let fx = x as f32 / s;
            let fy = y as f32 / s;
            let mut color = (9u8, 12u8, 17u8, 0u8);

            // Rounded plate.
            let radius = 0.26f32;
            let inside = {
                let cx = fx.clamp(radius, 1.0 - radius);
                let cy = fy.clamp(radius, 1.0 - radius);
                let dx = fx - cx;
                let dy = fy - cy;
                dx * dx + dy * dy <= radius * radius
            };

            if inside {
                color = (9, 12, 17, 255);
                let heights = [0.46f32, 0.94, 0.64];
                let pad = 0.2;
                let inner_left = pad;
                let inner_width = 1.0 - pad * 2.0;
                let gap = inner_width * 0.16;
                let bar_width = (inner_width - gap * 2.0) / 3.0;
                for (index, factor) in heights.iter().enumerate() {
                    let left = inner_left + index as f32 * (bar_width + gap);
                    let right = left + bar_width;
                    let bottom = 1.0 - pad;
                    let top = bottom - (1.0 - pad * 2.0) * factor;
                    if fx >= left && fx <= right && fy >= top && fy <= bottom {
                        let edge = (bar_width * 0.35).min(0.03);
                        let dx = (fx - left).min(right - fx);
                        if dx > edge || (fy > top + edge && fy < bottom - edge) {
                            color = (accent.0, accent.1, accent.2, 255);
                        }
                    }
                }
            }

            let index = ((y * size + x) * 4) as usize;
            pixels[index] = color.0;
            pixels[index + 1] = color.1;
            pixels[index + 2] = color.2;
            pixels[index + 3] = color.3;
        }
    }
    (pixels, size, size)
}

/// A small dot used for status indicators.
pub fn status_dot(ui: &mut Ui, color: Color32, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    ui.painter()
        .circle_filled(rect.center(), size * 0.35, color);
}
