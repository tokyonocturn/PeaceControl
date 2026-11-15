//! PeaceControl design system.
//!
//! One palette drives every surface so the application looks deliberate in
//! both light and dark mode. Colours, radii and spacing live here and nowhere
//! else.

use egui::{
    Color32, Context, Rounding, FontFamily, FontId, Stroke, TextStyle, Visuals,
};

/// Colour roles used by the interface.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub dark: bool,
    pub bg: Color32,
    pub panel: Color32,
    pub card: Color32,
    pub card_hover: Color32,
    pub border: Color32,
    pub border_strong: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub accent_dim: Color32,
    pub accent_text: Color32,
    pub warn: Color32,
    pub danger: Color32,
    pub ok: Color32,
    pub grid: Color32,
}

impl Palette {
    pub fn dark() -> Palette {
        Palette {
            dark: true,
            bg: Color32::from_rgb(9, 12, 17),
            panel: Color32::from_rgb(15, 20, 27),
            card: Color32::from_rgb(21, 27, 36),
            card_hover: Color32::from_rgb(28, 36, 47),
            border: Color32::from_rgb(35, 44, 57),
            border_strong: Color32::from_rgb(52, 65, 82),
            text: Color32::from_rgb(232, 239, 246),
            muted: Color32::from_rgb(137, 150, 167),
            accent: Color32::from_rgb(94, 234, 212),
            accent_dim: Color32::from_rgb(31, 78, 76),
            accent_text: Color32::from_rgb(6, 24, 24),
            warn: Color32::from_rgb(245, 178, 66),
            danger: Color32::from_rgb(248, 113, 113),
            ok: Color32::from_rgb(104, 211, 145),
            grid: Color32::from_rgb(31, 39, 51),
        }
    }

    pub fn light() -> Palette {
        Palette {
            dark: false,
            bg: Color32::from_rgb(244, 246, 249),
            panel: Color32::from_rgb(255, 255, 255),
            card: Color32::from_rgb(248, 250, 252),
            card_hover: Color32::from_rgb(238, 243, 248),
            border: Color32::from_rgb(222, 228, 236),
            border_strong: Color32::from_rgb(198, 208, 220),
            text: Color32::from_rgb(23, 31, 41),
            muted: Color32::from_rgb(100, 114, 131),
            accent: Color32::from_rgb(13, 148, 136),
            accent_dim: Color32::from_rgb(204, 245, 240),
            accent_text: Color32::from_rgb(255, 255, 255),
            warn: Color32::from_rgb(184, 108, 0),
            danger: Color32::from_rgb(200, 52, 52),
            ok: Color32::from_rgb(22, 130, 80),
            grid: Color32::from_rgb(228, 233, 240),
        }
    }

    pub fn for_dark(dark: bool) -> Palette {
        if dark {
            Palette::dark()
        } else {
            Palette::light()
        }
    }

    /// The accent with an alpha, for soft fills.
    pub fn accent_alpha(&self, alpha: u8) -> Color32 {
        Color32::from_rgba_unmultiplied(self.accent.r(), self.accent.g(), self.accent.b(), alpha)
    }

    pub fn muted_alpha(&self, alpha: u8) -> Color32 {
        Color32::from_rgba_unmultiplied(self.muted.r(), self.muted.g(), self.muted.b(), alpha)
    }
}

pub const CARD_RADIUS: Rounding = Rounding::same(10.0);
pub const SMALL_RADIUS: Rounding = Rounding::same(6.0);

/// Installs the system fonts when they are available, then the egui defaults.
pub fn install_fonts(ctx: &Context) {
    let mut fonts = egui::FontDefinitions::default();

    let candidates = [
        ("segoe-ui", r"C:\Windows\Fonts\segoeui.ttf"),
        ("segoe-ui-semibold", r"C:\Windows\Fonts\seguisb.ttf"),
        ("consolas", r"C:\Windows\Fonts\consola.ttf"),
    ];

    for (name, path) in candidates {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        fonts
            .font_data
            .insert(name.to_owned(), egui::FontData::from_owned(bytes));
        let family = if name == "consolas" {
            FontFamily::Monospace
        } else {
            FontFamily::Proportional
        };
        fonts
            .families
            .entry(family)
            .or_default()
            .insert(0, name.to_owned());
    }

    ctx.set_fonts(fonts);
}

/// Applies the palette and the spacing rhythm to a context.
pub fn apply(ctx: &Context, palette: &Palette) {
    let mut style = (*ctx.style()).clone();
    style.visuals = if palette.dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };

    let visuals = &mut style.visuals;
    visuals.override_text_color = Some(palette.text);
    visuals.panel_fill = palette.bg;
    visuals.window_fill = palette.panel;
    visuals.extreme_bg_color = palette.card;
    visuals.faint_bg_color = palette.card;
    visuals.window_stroke = Stroke::new(1.0_f32, palette.border);
    visuals.hyperlink_color = palette.accent;

    visuals.widgets.noninteractive.bg_fill = palette.panel;
    visuals.widgets.noninteractive.weak_bg_fill = palette.panel;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, palette.border);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, palette.text);

    visuals.widgets.inactive.bg_fill = palette.card;
    visuals.widgets.inactive.weak_bg_fill = palette.card;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, palette.border);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, palette.text);

    visuals.widgets.hovered.bg_fill = palette.card_hover;
    visuals.widgets.hovered.weak_bg_fill = palette.card_hover;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, palette.border_strong);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, palette.text);

    visuals.widgets.active.bg_fill = palette.accent_dim;
    visuals.widgets.active.weak_bg_fill = palette.accent_dim;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, palette.accent);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, palette.text);

    visuals.selection.bg_fill = palette.accent_dim;
    visuals.selection.stroke = Stroke::new(1.0_f32, palette.accent);

    style.spacing.item_spacing = egui::vec2(10.0, 8.0);
    style.spacing.button_padding = egui::vec2(12.0, 6.0);
    style.spacing.interact_size = egui::vec2(32.0, 26.0);

    let mut text_styles = std::collections::BTreeMap::new();
    text_styles.insert(
        TextStyle::Heading,
        FontId::new(21.0, FontFamily::Proportional),
    );
    text_styles.insert(TextStyle::Body, FontId::new(13.5, FontFamily::Proportional));
    text_styles.insert(
        TextStyle::Button,
        FontId::new(13.0, FontFamily::Proportional),
    );
    text_styles.insert(
        TextStyle::Small,
        FontId::new(11.5, FontFamily::Proportional),
    );
    text_styles.insert(
        TextStyle::Monospace,
        FontId::new(12.5, FontFamily::Monospace),
    );
    style.text_styles = text_styles;

    ctx.set_style(style);
}
