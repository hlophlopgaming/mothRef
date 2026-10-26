// SPDX-License-Identifier: GPL-3.0-or-later
//! Semantic colors shared by every UI surface.
use egui::{Color32, Context};
/// Complete house palette; widgets read the active set from their context.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tokens {
    /// Chooses egui's matching base visuals.
    pub dark: bool,
    /// Semantic canvas color.
    pub canvas: Color32,
    /// Semantic accent color.
    pub accent: Color32,
    /// Accent suitable for text and thin lines.
    pub accent_text: Color32,
    /// Text and icons drawn on an `accent` fill, such as primary buttons.
    pub on_accent: Color32,
    /// Semantic surface color.
    pub surface: Color32,
    /// Semantic text muted color.
    pub text_muted: Color32,
    /// Semantic border color.
    pub border: Color32,
    /// Semantic text color.
    pub text: Color32,
    /// Semantic surface raised color.
    pub surface_raised: Color32,
    /// Semantic hover color.
    pub hover: Color32,
    /// Semantic selection color.
    pub selection: Color32,
    /// Semantic divider color.
    pub divider: Color32,
    /// Semantic empty border color.
    pub empty_border: Color32,
    /// Semantic empty icon color.
    pub empty_icon: Color32,
    /// Semantic sidebar color.
    pub sidebar: Color32,
    /// Semantic minimap item color.
    pub minimap_item: Color32,
    /// Maximum minimap side before adapting to the viewport.
    pub minimap_max: f32,
    /// Minimum aspect ratio of the minimap short side to its long side.
    pub minimap_aspect_min: f32,
    /// Semantic danger color.
    pub danger: Color32,
    /// Semantic note fill color.
    pub note_fill: Color32,
    /// Semantic shadow color.
    pub shadow: Color32,
    /// Semantic image tint color.
    pub image_tint: Color32,
    /// Shared radius none metric.
    pub radius_none: u8,
    /// Shared radius icon metric.
    pub radius_icon: u8,
    /// Shared radius placeholder metric.
    pub radius_placeholder: u8,
    /// Shared radius picture metric.
    pub radius_picture: u8,
    /// Shared radius badge metric.
    pub radius_badge: u8,
    /// Shared radius card metric.
    pub radius_card: u8,
    /// Shared radius panel metric.
    pub radius_panel: u8,
    /// Shared space none metric.
    pub space_none: u8,
    /// Shared space tight metric.
    pub space_tight: u8,
    /// Shared space inset metric.
    pub space_inset: u8,
    /// Shared space badge metric.
    pub space_badge: u8,
    /// Shared space header metric.
    pub space_header: u8,
    /// Shared space content metric.
    pub space_content: u8,
    /// Shared space section metric.
    pub space_section: u8,
    /// Shared space empty metric.
    pub space_empty: u8,
    /// Hover outline rounding.
    pub radius_hover: u8,
    /// Shared radius s metric.
    pub radius_s: u8,
    /// Shared radius m metric.
    pub radius_m: u8,
    /// Shared radius l metric.
    pub radius_l: u8,
    /// Shared space xs metric.
    pub space_xs: u8,
    /// Shared space s metric.
    pub space_s: u8,
    /// Shared space m metric.
    pub space_m: u8,
    /// Shared space l metric.
    pub space_l: u8,
    /// Shared space xl metric.
    pub space_xl: u8,
}
impl Tokens {
    /// Translucent note background preserves the original card opacity.
    pub fn note_fill(self) -> Color32 {
        let [r, g, b, _] = self.note_fill.to_array();
        Color32::from_rgba_unmultiplied(r, g, b, 235)
    }
    /// Dark palette from the «Pixel» exploration (docs/DESIGN.md): Screen
    /// background, Bone text, Sage accent, Ink shadows, Wing warm accent.
    pub const DARK: Self = Self {
        dark: true,
        canvas: Color32::from_rgb(30, 36, 32),
        accent: Color32::from_rgb(166, 196, 122),
        accent_text: Color32::from_rgb(166, 196, 122),
        on_accent: Color32::from_rgb(26, 26, 26),
        surface: Color32::from_rgb(37, 44, 39),
        text_muted: Color32::from_rgb(154, 159, 148),
        border: Color32::from_rgb(58, 68, 60),
        text: Color32::from_rgb(232, 228, 214),
        surface_raised: Color32::from_rgb(44, 52, 46),
        hover: Color32::from_rgb(52, 62, 54),
        selection: Color32::from_rgb(62, 77, 48),
        divider: Color32::from_rgb(51, 60, 53),
        empty_border: Color32::from_rgb(70, 81, 74),
        empty_icon: Color32::from_rgb(138, 146, 137),
        sidebar: Color32::from_rgb(24, 29, 26),
        minimap_max: 188.,
        minimap_aspect_min: 0.35,
        minimap_item: Color32::from_rgb(74, 86, 77),
        danger: Color32::from_rgb(229, 143, 174),
        note_fill: Color32::from_rgb(42, 50, 44),
        shadow: Color32::from_rgb(26, 26, 26),
        image_tint: Color32::from_rgb(255, 255, 255),
        radius_none: 0,
        radius_icon: 1,
        radius_placeholder: 2,
        radius_picture: 3,
        radius_badge: 6,
        radius_card: 10,
        radius_panel: 12,
        space_none: 0,
        space_tight: 2,
        space_inset: 5,
        space_badge: 10,
        space_header: 14,
        space_content: 20,
        space_section: 28,
        space_empty: 80,
        radius_hover: 5,
        radius_s: 4,
        radius_m: 8,
        radius_l: 14,
        space_xs: 4,
        space_s: 8,
        space_m: 12,
        space_l: 16,
        space_xl: 24,
    };
    /// «Album» light palette (docs/DESIGN.md): Paper, Graphite, Sage, Moss.
    pub const LIGHT: Self = Self {
        dark: false,
        canvas: Color32::from_rgb(243, 239, 228),
        accent: Color32::from_rgb(166, 196, 122),
        accent_text: Color32::from_rgb(65, 90, 30),
        on_accent: Color32::from_rgb(35, 36, 31),
        surface: Color32::from_rgb(249, 247, 240),
        text_muted: Color32::from_rgb(105, 107, 97),
        border: Color32::from_rgb(196, 195, 181),
        text: Color32::from_rgb(35, 36, 31),
        surface_raised: Color32::from_rgb(230, 226, 211),
        hover: Color32::from_rgb(225, 230, 212),
        selection: Color32::from_rgb(218, 232, 194),
        divider: Color32::from_rgb(209, 208, 194),
        empty_border: Color32::from_rgb(182, 184, 166),
        empty_icon: Color32::from_rgb(85, 112, 47),
        sidebar: Color32::from_rgb(235, 232, 220),
        minimap_max: 188.,
        minimap_aspect_min: 0.35,
        minimap_item: Color32::from_rgb(177, 181, 162),
        danger: Color32::from_rgb(156, 64, 50),
        note_fill: Color32::from_rgb(249, 247, 240),
        shadow: Color32::from_rgb(0, 0, 0),
        image_tint: Color32::from_rgb(255, 255, 255),
        radius_none: 0,
        radius_icon: 1,
        radius_placeholder: 2,
        radius_picture: 3,
        radius_badge: 6,
        radius_card: 10,
        radius_panel: 12,
        space_none: 0,
        space_tight: 2,
        space_inset: 5,
        space_badge: 10,
        space_header: 14,
        space_content: 20,
        space_section: 28,
        space_empty: 80,
        radius_hover: 5,
        radius_s: 4,
        radius_m: 8,
        radius_l: 14,
        space_xs: 4,
        space_s: 8,
        space_m: 12,
        space_l: 16,
        space_xl: 24,
    };
}
/// Active colors follow theme changes without retaining widget-local copies.
pub fn tokens(ctx: &Context) -> Tokens {
    ctx.data(|data| data.get_temp(egui::Id::new("house_tokens")))
        .unwrap_or(Tokens::DARK)
}
/// Applies colors and metrics without reinstalling fonts.
pub fn apply(ctx: &Context, tokens: &Tokens) {
    let theme = if tokens.dark {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    };
    let mut style = (*ctx.style_of(theme)).clone();
    style.visuals = if tokens.dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    style.visuals.panel_fill = tokens.surface;
    style.visuals.window_fill = tokens.surface;
    style.visuals.window_stroke = egui::Stroke::new(1., tokens.border);
    style.visuals.window_corner_radius = egui::CornerRadius::same(tokens.radius_l);
    style.visuals.override_text_color = Some(tokens.text);
    style.visuals.widgets.inactive.weak_bg_fill = tokens.surface_raised;
    style.visuals.widgets.hovered.weak_bg_fill = tokens.hover;
    style.visuals.selection.bg_fill = tokens.selection;
    style.visuals.selection.stroke = egui::Stroke::new(1., tokens.text);
    style.visuals.widgets.inactive.bg_fill = tokens.surface_raised;
    style.visuals.widgets.hovered.bg_fill = tokens.hover;
    style.visuals.widgets.active.bg_fill = tokens.selection;
    style.visuals.widgets.active.fg_stroke.color = tokens.text;
    style.spacing.button_padding = egui::vec2(tokens.space_m as f32, tokens.space_s as f32);
    style.spacing.item_spacing = egui::Vec2::splat(tokens.space_s as f32);
    style.spacing.interact_size = egui::vec2(36., 36.);
    style.visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1., tokens.divider);
    if !tokens.dark {
        // Raised paper is close to the page itself; an outline keeps idle
        // buttons visible in the light theme.
        style.visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1., tokens.border);
    }
    for widget in [
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
    ] {
        widget.corner_radius = egui::CornerRadius::same(tokens.radius_m);
    }
    style.text_styles.insert(
        egui::TextStyle::Heading,
        egui::FontId::new(24., egui::FontFamily::Name("Bricolage Grotesque".into())),
    );
    style.animation_time = 0.12;
    ctx.data_mut(|data| data.insert_temp(egui::Id::new("house_tokens"), *tokens));
    ctx.set_style_of(theme, style);
    ctx.set_theme(theme);
}
#[cfg(test)]
mod tests {
    use super::*;
    /// WCAG relative luminance.
    fn luminance(color: Color32) -> f32 {
        let channel = |v: u8| {
            let v = v as f32 / 255.;
            if v <= 0.039_28 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        let [r, g, b, _] = color.to_array();
        0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
    }
    fn contrast(a: Color32, b: Color32) -> f32 {
        let (a, b) = (luminance(a), luminance(b));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }
    #[test]
    fn text_stays_readable_on_every_background() {
        for palette in [Tokens::DARK, Tokens::LIGHT] {
            for background in [
                palette.canvas,
                palette.surface,
                palette.surface_raised,
                palette.sidebar,
                palette.note_fill,
                palette.hover,
                palette.selection,
            ] {
                assert!(contrast(palette.text, background) >= 4.5, "{palette:?}");
                assert!(
                    contrast(palette.text_muted, background) >= 3.,
                    "{palette:?}"
                );
            }
            assert!(contrast(palette.on_accent, palette.accent) >= 4.5);
            assert!(contrast(palette.accent_text, palette.canvas) >= 4.5);
            assert!(contrast(palette.danger, palette.surface) >= 4.5);
        }
    }
    #[test]
    fn light_text_and_accent_controls_meet_contrast() {
        let luminance = |color: Color32| {
            let linear = |value: u8| {
                let value = f64::from(value) / 255.;
                if value <= 0.04045 {
                    value / 12.92
                } else {
                    ((value + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * linear(color.r()) + 0.7152 * linear(color.g()) + 0.0722 * linear(color.b())
        };
        let palette = Tokens::LIGHT;
        for (foreground, background) in [
            (palette.on_accent, palette.accent),
            (palette.text, palette.surface),
            (palette.text_muted, palette.surface),
            (palette.text, palette.selection),
            (palette.text, palette.surface_raised),
            (palette.accent_text, palette.surface),
        ] {
            let (a, b) = (luminance(foreground), luminance(background));
            let contrast = (a.max(b) + 0.05) / (a.min(b) + 0.05);
            assert!(
                contrast >= 4.5,
                "{foreground:?} on {background:?}: {contrast}"
            );
        }
    }
    #[test]
    fn switching_updates_both_visuals_and_widget_tokens() {
        let ctx = Context::default();
        for palette in [Tokens::DARK, Tokens::LIGHT, Tokens::DARK] {
            apply(&ctx, &palette);
            assert_eq!(tokens(&ctx), palette);
            assert_eq!(
                ctx.style_of(if palette.dark {
                    egui::Theme::Dark
                } else {
                    egui::Theme::Light
                })
                .visuals
                .panel_fill,
                palette.surface
            );
            assert_eq!(
                ctx.style_of(if palette.dark {
                    egui::Theme::Dark
                } else {
                    egui::Theme::Light
                })
                .visuals
                .override_text_color,
                Some(palette.text)
            );
        }
    }
}
