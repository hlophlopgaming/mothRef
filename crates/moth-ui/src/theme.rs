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
    /// Original dark palette.
    pub const DARK: Self = Self {
        dark: true,
        canvas: Color32::from_rgb(17, 19, 23),
        accent: Color32::from_rgb(181, 206, 143),
        accent_text: Color32::from_rgb(181, 206, 143),
        surface: Color32::from_rgb(31, 34, 40),
        text_muted: Color32::from_rgb(120, 125, 136),
        border: Color32::from_rgb(48, 52, 60),
        text: Color32::from_rgb(224, 226, 231),
        surface_raised: Color32::from_rgb(37, 41, 48),
        hover: Color32::from_rgb(49, 55, 61),
        selection: Color32::from_rgb(61, 76, 45),
        divider: Color32::from_rgb(47, 51, 59),
        empty_border: Color32::from_rgb(59, 63, 72),
        empty_icon: Color32::from_rgb(113, 119, 132),
        sidebar: Color32::from_rgb(23, 25, 30),
        minimap_item: Color32::from_rgb(69, 75, 82),
        danger: Color32::from_rgb(209, 133, 123),
        note_fill: Color32::from_rgb(39, 42, 50),
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
    /// Paper light palette.
    pub const LIGHT: Self = Self {
        dark: false,
        canvas: Color32::from_rgb(243, 239, 228),
        accent: Color32::from_rgb(166, 196, 122),
        accent_text: Color32::from_rgb(85, 112, 47),
        surface: Color32::from_rgb(249, 247, 240),
        text_muted: Color32::from_rgb(105, 107, 97),
        border: Color32::from_rgb(196, 195, 181),
        text: Color32::from_rgb(35, 36, 31),
        surface_raised: Color32::from_rgb(236, 233, 220),
        hover: Color32::from_rgb(225, 230, 212),
        selection: Color32::from_rgb(218, 232, 194),
        divider: Color32::from_rgb(209, 208, 194),
        empty_border: Color32::from_rgb(182, 184, 166),
        empty_icon: Color32::from_rgb(85, 112, 47),
        sidebar: Color32::from_rgb(235, 232, 220),
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
    style.visuals.selection.stroke = egui::Stroke::new(1., tokens.accent);
    style.spacing.button_padding = egui::vec2(tokens.space_m as f32, tokens.space_s as f32);
    style.spacing.item_spacing = egui::Vec2::splat(tokens.space_s as f32);
    style.spacing.interact_size = egui::vec2(36., 36.);
    style.visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1., tokens.divider);
    for widget in [
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
    ] {
        widget.corner_radius = egui::CornerRadius::same(tokens.radius_m);
    }
    style.animation_time = 0.12;
    ctx.data_mut(|data| data.insert_temp(egui::Id::new("house_tokens"), *tokens));
    ctx.set_style_of(theme, style);
    ctx.set_theme(theme);
}
#[cfg(test)]
mod tests {
    use super::*;
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
