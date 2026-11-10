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
    /// Near-black stage of drawing sessions and other full-screen image views.
    pub theater: Color32,
    /// Text and marks drawn on `theater`.
    pub on_theater: Color32,
    /// Wing: the last seconds of a timed image.
    pub wing: Color32,
    /// Group swatches by palette code: neutral, sage, amber, sky, wing, lilac.
    pub groups: [Color32; 6],
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
    /// A group's swatch; unknown codes fall back to neutral.
    pub fn group(self, code: u32) -> Color32 {
        let [tag, r, g, b] = code.to_be_bytes();
        if tag == 1 {
            return Color32::from_rgb(r, g, b);
        }
        self.groups
            .get(code as usize)
            .copied()
            .unwrap_or(self.groups[0])
    }
    /// Backdrop fill and border of a group, translucent so the canvas reads through.
    pub fn group_backdrop(self, code: u32) -> [Color32; 2] {
        let [r, g, b, _] = self.group(code).to_array();
        let (fill, border) = if self.dark { (34, 120) } else { (38, 120) };
        [
            Color32::from_rgba_unmultiplied(r, g, b, fill),
            Color32::from_rgba_unmultiplied(r, g, b, border),
        ]
    }
    /// Text on a filled group swatch (a selected title chip): dark on light swatches.
    pub fn on_group(self, code: u32) -> Color32 {
        let [r, g, b, _] = self.group(code).to_array();
        let luma = 0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32;
        if luma > 140. {
            Self::DARK.on_accent
        } else {
            Self::LIGHT.surface
        }
    }
    /// Title chip fill of a group: the canvas tinted towards its swatch.
    pub fn group_chip(self, code: u32) -> Color32 {
        self.surface.lerp_to_gamma(self.group(code), 0.14)
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
        theater: Color32::from_rgb(9, 11, 10),
        on_theater: Color32::from_rgb(232, 228, 214),
        wing: Color32::from_rgb(229, 143, 174),
        groups: [
            Color32::from_rgb(154, 159, 148),
            Color32::from_rgb(166, 196, 122),
            Color32::from_rgb(224, 180, 108),
            Color32::from_rgb(143, 184, 222),
            Color32::from_rgb(229, 143, 174),
            Color32::from_rgb(180, 160, 216),
        ],
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
        theater: Color32::from_rgb(20, 22, 21),
        on_theater: Color32::from_rgb(232, 228, 214),
        wing: Color32::from_rgb(229, 143, 174),
        groups: [
            Color32::from_rgb(105, 107, 97),
            Color32::from_rgb(85, 112, 47),
            Color32::from_rgb(166, 112, 28),
            Color32::from_rgb(46, 104, 158),
            Color32::from_rgb(172, 64, 104),
            Color32::from_rgb(108, 84, 160),
        ],
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
/// No fill: idle ghost buttons and inactive tabs.
pub const CLEAR: Color32 = Color32::TRANSPARENT;
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
    #[test]
    fn text_stays_readable_on_every_background() {
        // Every palette, light and dark, with its own accent and with each quick pick.
        let palettes = Palette::ALL.into_iter().flat_map(|palette| {
            [true, false].into_iter().flat_map(move |dark| {
                let base = Tokens::palette(palette, dark);
                std::iter::once(base).chain(ACCENTS.into_iter().map(move |[r, g, b]| {
                    let mut tokens = base;
                    tokens.set_accent(Color32::from_rgb(r, g, b));
                    tokens
                }))
            })
        });
        for palette in palettes {
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

/// Stable, optional RGB preference; an empty value restores the active palette.
pub fn encode_color(value: Option<[u8; 3]>) -> String {
    value.map_or_else(String::new, |[r, g, b]| format!("{r:02X}{g:02X}{b:02X}"))
}
/// Rejects malformed preferences before they can reach the theme.
pub fn decode_color(value: &str) -> Option<[u8; 3]> {
    if value.len() != 6 || !value.is_ascii() {
        return None;
    }
    Some([
        u8::from_str_radix(&value[0..2], 16).ok()?,
        u8::from_str_radix(&value[2..4], 16).ok()?,
        u8::from_str_radix(&value[4..6], 16).ok()?,
    ])
}
impl Tokens {
    /// Applies user colors to the semantic roles used by both palettes.
    pub fn custom_colors(
        &mut self,
        background: Option<[u8; 3]>,
        canvas: Option<[u8; 3]>,
        selection: Option<[u8; 3]>,
    ) {
        if let Some([r, g, b]) = background {
            self.surface = Color32::from_rgb(r, g, b);
        }
        if let Some([r, g, b]) = canvas {
            self.canvas = Color32::from_rgb(r, g, b);
        }
        if let Some([r, g, b]) = selection {
            self.set_accent(Color32::from_rgb(r, g, b));
        }
    }
    /// Makes `chosen` the accent. Fills under selected rows and buttons get a calm
    /// tone of it, so a saturated pick never turns lists into solid color, and text
    /// in it stays readable on the surface.
    pub fn set_accent(&mut self, chosen: Color32) {
        self.accent = chosen;
        self.selection = self
            .surface
            .lerp_to_gamma(chosen, if self.dark { 0.22 } else { 0.3 });
        // Text in the accent color is lightened (dark theme) or darkened (light theme)
        // just enough to read on the canvas.
        let toward = if self.dark {
            Color32::WHITE
        } else {
            Color32::BLACK
        };
        self.accent_text = (0..=20)
            .map(|step| chosen.lerp_to_gamma(toward, step as f32 / 20.))
            .find(|color| contrast(*color, self.canvas) >= 4.5)
            .unwrap_or(toward);
        // Text on an accent fill: whichever of dark and light reads better.
        self.on_accent = [Self::DARK.on_accent, Self::LIGHT.surface]
            .into_iter()
            .max_by(|a, b| contrast(*a, chosen).total_cmp(&contrast(*b, chosen)))
            .unwrap_or(Self::DARK.on_accent);
    }
    /// The base light or dark tokens in one of the palettes.
    pub fn palette(palette: Palette, dark: bool) -> Self {
        let mut tokens = if dark { Self::DARK } else { Self::LIGHT };
        let Some(neutrals) = palette.neutrals(dark) else {
            return tokens;
        };
        let c = |[r, g, b]: [u8; 3]| Color32::from_rgb(r, g, b);
        tokens.canvas = c(neutrals.canvas);
        tokens.surface = c(neutrals.surface);
        tokens.surface_raised = c(neutrals.raised);
        tokens.sidebar = c(neutrals.sidebar);
        tokens.border = c(neutrals.border);
        tokens.divider = c(neutrals.divider);
        tokens.hover = c(neutrals.hover);
        tokens.text = c(neutrals.text);
        tokens.text_muted = c(neutrals.muted);
        tokens.empty_border = c(neutrals.empty_border);
        tokens.empty_icon = c(neutrals.empty_icon);
        tokens.minimap_item = c(neutrals.minimap_item);
        tokens.note_fill = c(neutrals.surface);
        tokens.groups[0] = c(neutrals.muted);
        tokens.set_accent(c(palette.accent()));
        tokens
    }
}

/// WCAG relative luminance of an sRGB color.
fn luminance(color: Color32) -> f32 {
    let channel = |value: u8| {
        let v = value as f32 / 255.;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    let [r, g, b, _] = color.to_array();
    0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
}
/// WCAG contrast ratio between two colors, 1 to 21.
fn contrast(a: Color32, b: Color32) -> f32 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}
/// The base colors of the interface. Sage is MothRef's original green-tinted
/// palette; the others keep the same structure with other neutrals.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Palette {
    Sage,
    /// Plain gray, with no tint.
    #[default]
    Graphite,
    /// Cool blue-gray.
    Ink,
    /// Warm brown-gray.
    Umber,
}
/// Neutral colors of a palette: everything that is not an accent.
struct Neutrals {
    canvas: [u8; 3],
    surface: [u8; 3],
    raised: [u8; 3],
    sidebar: [u8; 3],
    border: [u8; 3],
    divider: [u8; 3],
    hover: [u8; 3],
    text: [u8; 3],
    muted: [u8; 3],
    empty_border: [u8; 3],
    empty_icon: [u8; 3],
    minimap_item: [u8; 3],
}
impl Palette {
    pub const ALL: [Self; 4] = [Self::Sage, Self::Graphite, Self::Ink, Self::Umber];
    pub fn from_code(code: u8) -> Self {
        Self::ALL
            .get(usize::from(code))
            .copied()
            .unwrap_or_default()
    }
    /// The accent a palette starts with; a chosen accent replaces it.
    fn accent(self) -> [u8; 3] {
        match self {
            Self::Sage => [166, 196, 122],
            Self::Graphite => [143, 184, 222],
            Self::Ink => [143, 184, 222],
            Self::Umber => [224, 180, 108],
        }
    }
    fn neutrals(self, dark: bool) -> Option<Neutrals> {
        let n = |canvas,
                 surface,
                 raised,
                 sidebar,
                 border,
                 divider,
                 hover,
                 text,
                 muted,
                 empty_border,
                 empty_icon,
                 minimap_item| Neutrals {
            canvas,
            surface,
            raised,
            sidebar,
            border,
            divider,
            hover,
            text,
            muted,
            empty_border,
            empty_icon,
            minimap_item,
        };
        Some(match (self, dark) {
            (Self::Sage, _) => return None,
            (Self::Graphite, true) => n(
                [28, 28, 30],
                [36, 36, 39],
                [44, 44, 48],
                [22, 22, 24],
                [58, 58, 63],
                [50, 50, 54],
                [52, 52, 57],
                [232, 232, 236],
                [152, 152, 160],
                [70, 70, 76],
                [138, 138, 146],
                [74, 74, 80],
            ),
            (Self::Graphite, false) => n(
                [240, 240, 242],
                [248, 248, 250],
                [228, 228, 232],
                [233, 233, 237],
                [196, 196, 202],
                [210, 210, 216],
                [226, 227, 232],
                [32, 32, 36],
                [104, 104, 112],
                [182, 182, 190],
                [90, 90, 100],
                [178, 178, 186],
            ),
            (Self::Ink, true) => n(
                [24, 28, 36],
                [31, 36, 46],
                [38, 44, 56],
                [19, 22, 29],
                [52, 60, 76],
                [45, 51, 64],
                [46, 53, 67],
                [226, 232, 242],
                [148, 158, 176],
                [62, 72, 90],
                [132, 144, 166],
                [66, 76, 94],
            ),
            (Self::Ink, false) => n(
                [236, 240, 246],
                [246, 248, 252],
                [224, 230, 240],
                [229, 234, 242],
                [190, 198, 212],
                [206, 213, 226],
                [222, 229, 240],
                [28, 34, 46],
                [98, 108, 126],
                [176, 186, 204],
                [70, 90, 130],
                [170, 180, 198],
            ),
            (Self::Umber, true) => n(
                [34, 30, 27],
                [42, 37, 33],
                [50, 44, 39],
                [27, 24, 21],
                [66, 58, 51],
                [56, 49, 43],
                [60, 52, 46],
                [236, 228, 216],
                [164, 154, 140],
                [78, 69, 61],
                [150, 138, 122],
                [84, 74, 64],
            ),
            (Self::Umber, false) => n(
                [244, 238, 230],
                [250, 246, 240],
                [232, 224, 212],
                [238, 231, 221],
                [200, 190, 176],
                [214, 204, 190],
                [230, 222, 210],
                [40, 34, 28],
                [112, 102, 90],
                [190, 178, 162],
                [120, 90, 60],
                [184, 172, 156],
            ),
        })
    }
}
/// Quick picks for the accent: the group colors plus coral, teal and graphite.
pub const ACCENTS: [[u8; 3]; 8] = [
    [166, 196, 122],
    [224, 180, 108],
    [143, 184, 222],
    [229, 143, 174],
    [180, 160, 216],
    [232, 128, 104],
    [110, 196, 184],
    [176, 176, 184],
];
/// Quick picks for the canvas: neutral grays from near black to near white.
pub const CANVASES: [[u8; 3]; 7] = [
    [18, 18, 20],
    [28, 28, 30],
    [40, 40, 44],
    [64, 64, 68],
    [128, 128, 132],
    [204, 204, 208],
    [238, 238, 240],
];

#[cfg(test)]
mod custom_color_tests {
    use super::*;
    #[test]
    fn colors_validate_and_override_both_palettes() {
        for tokens in [Tokens::DARK, Tokens::LIGHT] {
            for rgb in [[0, 0, 0], [255, 255, 255], [12, 128, 240]] {
                let code = u32::from_be_bytes([1, rgb[0], rgb[1], rgb[2]]);
                assert_eq!(tokens.group(code).to_array(), [rgb[0], rgb[1], rgb[2], 255]);
                assert!(tokens.group_backdrop(code)[0].a() > 0);
            }
        }
        let color = Some([16, 128, 255]);
        assert_eq!(decode_color(&encode_color(color)), color);
        for bad in ["", "#FFFFFF", "GGFFFF", "ＦＦ", "00000", "0000000"] {
            assert_eq!(decode_color(bad), None);
        }
        for mut tokens in [Tokens::LIGHT, Tokens::DARK] {
            tokens.custom_colors(color, color, color);
            assert_eq!(tokens.canvas.to_array(), [16, 128, 255, 255]);
            assert_eq!(tokens.surface, tokens.canvas);
            assert_eq!(tokens.accent, tokens.canvas);
            // Selection fills are a tone of the chosen color, never the raw color on
            // another surface.
            let mut quiet = Tokens::DARK;
            quiet.custom_colors(None, None, Some([232, 80, 74]));
            assert_eq!(quiet.accent.to_array(), [232, 80, 74, 255]);
            assert_ne!(quiet.selection, quiet.accent);
        }
    }
}
