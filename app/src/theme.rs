//! Folio's three visual languages.
//!
//! One set of job-named colours, three palettes: a neutral Windows-like white,
//! the warm paper the app grew up with, and a graphite "Corpo" look with light
//! text on dark surfaces. Which one is live is decided once per change by
//! [`Look`], and everything that paints reads it through [`C`].
//!
//! The writing face is installed once under a fixed family name. [`serif`]
//! only chooses which family the text styles point at, so switching looks can
//! never rewrite egui's own proportional family.

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke, TextStyle,
    Visuals,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::path::Path;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

/// The three faces Folio can wear.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Look {
    /// Neutral greys and a light navigation column, with the brand orange as
    /// accent.
    White,
    /// Warm paper, dark ink, terracotta — the original look.
    Paper,
    /// Graphite and near-black with light text, accented in the brand orange.
    Corpo,
}

impl Look {
    pub const ALL: [Look; 3] = [Look::White, Look::Paper, Look::Corpo];

    /// The name stored in `library.json` and accepted by `FOLIO_THEME`.
    pub fn name(self) -> &'static str {
        match self {
            Look::White => "white",
            Look::Paper => "paper",
            Look::Corpo => "corpo",
        }
    }

    /// The label in the settings drop-down.
    pub fn label(self) -> &'static str {
        match self {
            Look::White => "White",
            Look::Paper => "Paper",
            Look::Corpo => "Corpo",
        }
    }

    /// One line under the drop-down, so the names mean something.
    pub fn note(self) -> &'static str {
        match self {
            Look::White => "Clean and neutral, close to Windows itself.",
            Look::Paper => "Warm paper and ink — Folio's original writing desk.",
            Look::Corpo => "Graphite and white, tight and modern.",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "white" => Some(Look::White),
            "paper" => Some(Look::Paper),
            "corpo" => Some(Look::Corpo),
            _ => None,
        }
    }

    /// True for a palette meant to be read in the dark.
    pub fn is_dark(self) -> bool {
        matches!(self, Look::Corpo)
    }
}

impl Default for Look {
    fn default() -> Self {
        Look::Paper
    }
}

impl Serialize for Look {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.name())
    }
}

impl<'de> Deserialize<'de> for Look {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // A hand-edited or older file may name anything; falling back to Paper
        // keeps a working window instead of refusing to load the library.
        let name = String::deserialize(deserializer)?;
        Ok(Look::from_name(&name).unwrap_or_default())
    }
}

/// Every colour the app uses, named by its job rather than its hex value.
#[derive(Clone, Copy)]
pub struct Palette {
    pub paper: Color32,
    pub panel: Color32,
    pub card: Color32,
    pub soft: Color32,
    pub summary_bg: Color32,
    /// The AI draft block under the chapter text. Cooler than `summary_bg`,
    /// so it reads as a proposal rather than part of the page.
    pub draft_bg: Color32,
    pub draft_line: Color32,
    /// Background behind a find match in the editor.
    pub find_bg: Color32,
    /// The match Find next/previous last jumped to.
    pub find_active_bg: Color32,
    pub ink: Color32,
    pub ink_soft: Color32,
    pub muted: Color32,
    /// For small hints that still need to be readable.
    pub hint: Color32,
    pub line: Color32,
    pub accent: Color32,
    pub accent_deep: Color32,
    pub accent_soft: Color32,
    pub green: Color32,
    pub green_soft: Color32,
    /// Deep enough for a white letter, e.g. the partner's avatar.
    pub green_deep: Color32,
    pub amber: Color32,
    pub amber_soft: Color32,
    pub sidebar: Color32,
    pub sidebar_card: Color32,
    pub sidebar_hover: Color32,
    pub sidebar_text: Color32,
    pub sidebar_muted: Color32,
    pub red: Color32,
    /// Deep enough for white text, e.g. a delete button or an error toast.
    pub red_deep: Color32,
    /// The toast card: near-black in daylight, raised graphite in Corpo.
    pub toast: Color32,
    pub on_toast: Color32,
}

/// The logo's orange. It is the mark, not the accent, so it stays the same in
/// every look.
pub const BRAND_ORANGE: Color32 = Color32::from_rgb(174, 82, 48);

/// White: neutral surfaces, the brand orange, and a navigation column that is a
/// little lighter than the page, as Explorer and Settings do it.
pub const WHITE: Palette = Palette {
    paper: Color32::from_rgb(243, 243, 243),
    panel: Color32::from_rgb(250, 250, 250),
    card: Color32::from_rgb(255, 255, 255),
    soft: Color32::from_rgb(235, 235, 235),
    summary_bg: Color32::from_rgb(240, 240, 240),
    draft_bg: Color32::from_rgb(240, 246, 252),
    draft_line: Color32::from_rgb(201, 221, 242),
    find_bg: Color32::from_rgb(255, 242, 168),
    find_active_bg: Color32::from_rgb(255, 210, 74),
    ink: Color32::from_rgb(26, 26, 26),
    ink_soft: Color32::from_rgb(61, 61, 61),
    muted: Color32::from_rgb(107, 107, 107),
    hint: Color32::from_rgb(92, 92, 92),
    line: Color32::from_rgb(224, 224, 224),
    accent: Color32::from_rgb(174, 82, 48),
    accent_deep: Color32::from_rgb(137, 59, 32),
    accent_soft: Color32::from_rgb(247, 227, 213),
    green: Color32::from_rgb(11, 106, 11),
    green_soft: Color32::from_rgb(230, 244, 230),
    green_deep: Color32::from_rgb(11, 106, 11),
    amber: Color32::from_rgb(122, 74, 0),
    amber_soft: Color32::from_rgb(255, 244, 206),
    sidebar: Color32::from_rgb(243, 243, 243),
    sidebar_card: Color32::from_rgb(232, 232, 232),
    sidebar_hover: Color32::from_rgb(229, 229, 229),
    sidebar_text: Color32::from_rgb(26, 26, 26),
    sidebar_muted: Color32::from_rgb(107, 107, 107),
    red: Color32::from_rgb(196, 43, 28),
    red_deep: Color32::from_rgb(196, 43, 28),
    toast: Color32::from_rgb(31, 31, 31),
    on_toast: Color32::WHITE,
};

/// Paper: warm paper, dark ink, the accent the logo and the .exe icon use.
/// This is the palette Folio 0.1.1 shipped, kept byte for byte.
pub const PAPER: Palette = Palette {
    paper: Color32::from_rgb(246, 241, 231),
    panel: Color32::from_rgb(252, 248, 240),
    card: Color32::from_rgb(255, 253, 247),
    soft: Color32::from_rgb(239, 232, 220),
    summary_bg: Color32::from_rgb(240, 233, 219),
    draft_bg: Color32::from_rgb(232, 238, 229),
    draft_line: Color32::from_rgb(197, 211, 193),
    find_bg: Color32::from_rgb(251, 231, 166),
    find_active_bg: Color32::from_rgb(244, 196, 110),
    ink: Color32::from_rgb(34, 30, 26),
    ink_soft: Color32::from_rgb(78, 70, 60),
    muted: Color32::from_rgb(136, 125, 109),
    hint: Color32::from_rgb(112, 101, 86),
    line: Color32::from_rgb(222, 212, 195),
    accent: Color32::from_rgb(174, 82, 48),
    accent_deep: Color32::from_rgb(137, 59, 32),
    accent_soft: Color32::from_rgb(247, 227, 213),
    green: Color32::from_rgb(61, 105, 86),
    green_soft: Color32::from_rgb(225, 237, 229),
    green_deep: Color32::from_rgb(61, 105, 86),
    amber: Color32::from_rgb(171, 128, 57),
    amber_soft: Color32::from_rgb(246, 234, 206),
    sidebar: Color32::from_rgb(27, 38, 34),
    sidebar_card: Color32::from_rgb(43, 57, 51),
    sidebar_hover: Color32::from_rgb(37, 50, 44),
    sidebar_text: Color32::from_rgb(238, 232, 220),
    sidebar_muted: Color32::from_rgb(158, 164, 150),
    red: Color32::from_rgb(158, 62, 52),
    red_deep: Color32::from_rgb(158, 62, 52),
    toast: Color32::from_rgb(34, 30, 26),
    on_toast: Color32::from_rgb(255, 255, 255),
};

/// Corpo: near-black graphite, light text, and the brand orange — dark enough
/// for the white letters Folio puts on every primary button.
pub const CORPO: Palette = Palette {
    paper: Color32::from_rgb(14, 15, 17),
    panel: Color32::from_rgb(20, 22, 25),
    card: Color32::from_rgb(26, 29, 33),
    soft: Color32::from_rgb(38, 43, 49),
    summary_bg: Color32::from_rgb(28, 32, 36),
    draft_bg: Color32::from_rgb(22, 28, 36),
    draft_line: Color32::from_rgb(43, 58, 74),
    find_bg: Color32::from_rgb(58, 50, 22),
    find_active_bg: Color32::from_rgb(107, 90, 30),
    ink: Color32::from_rgb(242, 244, 247),
    ink_soft: Color32::from_rgb(198, 204, 212),
    muted: Color32::from_rgb(138, 145, 155),
    hint: Color32::from_rgb(154, 162, 172),
    line: Color32::from_rgb(52, 58, 66),
    accent: Color32::from_rgb(174, 82, 48),
    accent_deep: Color32::from_rgb(233, 164, 127),
    accent_soft: Color32::from_rgb(58, 36, 27),
    green: Color32::from_rgb(111, 191, 139),
    green_soft: Color32::from_rgb(20, 37, 28),
    green_deep: Color32::from_rgb(46, 107, 71),
    amber: Color32::from_rgb(217, 178, 106),
    amber_soft: Color32::from_rgb(42, 35, 24),
    sidebar: Color32::from_rgb(10, 11, 12),
    sidebar_card: Color32::from_rgb(22, 24, 27),
    sidebar_hover: Color32::from_rgb(29, 32, 36),
    sidebar_text: Color32::from_rgb(232, 235, 239),
    sidebar_muted: Color32::from_rgb(138, 145, 155),
    red: Color32::from_rgb(224, 108, 95),
    red_deep: Color32::from_rgb(142, 47, 39),
    toast: Color32::from_rgb(34, 38, 43),
    on_toast: Color32::from_rgb(242, 244, 247),
};

/// 0 = White, 1 = Paper, 2 = Corpo. Paper is the default so a fresh start and
/// an old library look exactly like Folio 0.1.1.
static LOOK: AtomicU8 = AtomicU8::new(1);

/// The look in use right now.
pub fn look() -> Look {
    match LOOK.load(Ordering::Relaxed) {
        0 => Look::White,
        2 => Look::Corpo,
        _ => Look::Paper,
    }
}

pub fn set_look(look: Look) {
    LOOK.store(
        match look {
            Look::White => 0,
            Look::Paper => 1,
            Look::Corpo => 2,
        },
        Ordering::Relaxed,
    );
}

/// The colours in use right now.
pub fn palette() -> &'static Palette {
    match look() {
        Look::White => &WHITE,
        Look::Paper => &PAPER,
        Look::Corpo => &CORPO,
    }
}

/// Short names for the live palette: `C::paper()` is whatever paper is today.
pub struct C;

macro_rules! colour_shortcuts {
    ($($field:ident),* $(,)?) => {
        #[allow(dead_code)]
        impl C {
            $(
                #[doc = concat!("The live `", stringify!($field), "` colour.")]
                pub fn $field() -> Color32 {
                    palette().$field
                }
            )*
        }
    };
}

colour_shortcuts! {
    paper, panel, card, soft, summary_bg, draft_bg, draft_line, ink, ink_soft, muted, hint, line,
    accent, accent_deep, accent_soft, green, green_soft, green_deep, amber,
    amber_soft, sidebar, sidebar_card, sidebar_hover, sidebar_text,
    sidebar_muted, red, red_deep, toast, on_toast, find_bg, find_active_bg,
}

/// The fixed name of the installed writing face. It never changes, so the
/// family can be filled once and simply pointed at (or not) by [`serif`].
const SERIF_FAMILY: &str = "folio-serif";
const SERIF_DATA: &str = "folio-serif-data";

/// The family the writing face is installed under, whatever the look.
pub fn writing_family() -> FontFamily {
    FontFamily::Name(Arc::from(SERIF_FAMILY))
}

/// The face story text and headings are set in: the installed writing face in
/// Paper, the system's own proportional face in White and Corpo.
pub fn serif() -> FontFamily {
    if look() == Look::Paper {
        writing_family()
    } else {
        FontFamily::Proportional
    }
}

pub fn serif_id(size: f32) -> FontId {
    FontId::new(size, serif())
}

/// The brand face: the "F" mark stays a serif in every look.
pub fn brand_id(size: f32) -> FontId {
    FontId::new(size, writing_family())
}

/// A rounded corner for the live look. The number is the Paper value — the
/// value Folio 0.1.1 drew — and White and Corpo scale it down so the same card
/// looks right in every language.
pub fn radius(paper: u8) -> CornerRadius {
    let scaled = match look() {
        Look::Paper => paper,
        Look::White => (paper as f32 * 0.66).round() as u8,
        Look::Corpo => (paper as f32 * 0.35).round() as u8,
    };
    CornerRadius::same(scaled.max(2))
}

/// Install the look named by the library, or by `FOLIO_THEME` when a
/// screenshot or a check asks for one.
pub fn install(ctx: &egui::Context, requested: Look, writing_font: Option<&Path>) {
    let look = from_env().unwrap_or(requested);
    set_look(look);
    install_fonts(ctx, writing_font);
    apply_style(ctx, look);
}

/// Switch looks without touching the installed fonts.
pub fn apply_style(ctx: &egui::Context, look: Look) {
    set_look(look);
    let style = style_for(palette(), look);
    // Both themes get the same style, so flipping Windows to dark cannot
    // repaint a single pixel behind the author's back.
    ctx.set_style_of(egui::Theme::Light, style.clone());
    ctx.set_style_of(egui::Theme::Dark, style);
    ctx.set_theme(theme_of(look));
    ctx.request_repaint();
}

/// Keep egui on the look's theme if anything (the platform, a menu) tries to
/// follow the system again. Cheap: one comparison per frame.
pub fn keep(ctx: &egui::Context) {
    let wanted = theme_of(look());
    if ctx.theme() != wanted {
        ctx.set_theme(wanted);
        ctx.request_repaint();
    }
}

fn theme_of(look: Look) -> egui::Theme {
    if look.is_dark() {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    }
}

/// `FOLIO_THEME=white|paper|corpo` forces a look for screenshots. `light` and
/// `dark` stay accepted so older notes keep working.
pub fn from_env() -> Option<Look> {
    resolve_look(std::env::var("FOLIO_THEME").ok().as_deref())
}

fn resolve_look(value: Option<&str>) -> Option<Look> {
    match value
        .map(|value| value.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("white") => Some(Look::White),
        Some("paper") | Some("light") => Some(Look::Paper),
        Some("corpo") | Some("dark") => Some(Look::Corpo),
        _ => None,
    }
}

/// The whole egui styling for one palette: panel colours, widget states,
/// spacing and type sizes.
fn style_for(palette: &Palette, look: Look) -> egui::Style {
    let mut style = egui::Style::default();
    let mut visuals = if look.is_dark() {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    visuals.dark_mode = look.is_dark();
    visuals.override_text_color = Some(palette.ink);
    visuals.panel_fill = palette.paper;
    visuals.window_fill = palette.panel;
    visuals.extreme_bg_color = palette.card;
    visuals.faint_bg_color = palette.soft;
    visuals.code_bg_color = palette.soft;
    visuals.window_corner_radius = radius(14);
    visuals.menu_corner_radius = radius(9);
    visuals.window_stroke = Stroke::new(1.0_f32, palette.line);
    visuals.selection.bg_fill = palette.accent;
    visuals.selection.stroke = Stroke::new(1.0_f32, Color32::WHITE);
    visuals.hyperlink_color = palette.accent_deep;
    visuals.warn_fg_color = palette.amber;
    visuals.error_fg_color = palette.red;

    let normal = radius(9);
    visuals.widgets.noninteractive.bg_fill = palette.panel;
    visuals.widgets.noninteractive.weak_bg_fill = palette.panel;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, palette.line);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, palette.ink_soft);
    visuals.widgets.noninteractive.corner_radius = normal;
    visuals.widgets.noninteractive.expansion = 0.0;

    visuals.widgets.inactive.bg_fill = palette.soft;
    visuals.widgets.inactive.weak_bg_fill = palette.soft;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, palette.line);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, palette.ink);
    visuals.widgets.inactive.corner_radius = normal;
    visuals.widgets.inactive.expansion = 0.0;

    visuals.widgets.hovered.bg_fill = palette.accent_soft;
    visuals.widgets.hovered.weak_bg_fill = palette.accent_soft;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, palette.accent);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.2_f32, palette.accent_deep);
    visuals.widgets.hovered.corner_radius = normal;
    visuals.widgets.hovered.expansion = 0.0;

    visuals.widgets.active.bg_fill = palette.accent_soft;
    visuals.widgets.active.weak_bg_fill = palette.accent_soft;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, palette.accent);
    visuals.widgets.active.fg_stroke = Stroke::new(1.2_f32, palette.accent_deep);
    visuals.widgets.active.corner_radius = normal;
    visuals.widgets.active.expansion = 0.0;

    visuals.widgets.open.bg_fill = palette.card;
    visuals.widgets.open.weak_bg_fill = palette.card;
    visuals.widgets.open.bg_stroke = Stroke::new(1.0_f32, palette.line);
    visuals.widgets.open.fg_stroke = Stroke::new(1.0_f32, palette.ink);
    visuals.widgets.open.corner_radius = normal;

    style.visuals = visuals;
    let (spacing_x, spacing_y, padding_x, padding_y) = match look {
        Look::White => (8.0, 7.0, 14.0, 6.0),
        Look::Paper => (8.0, 7.0, 12.0, 7.0),
        Look::Corpo => (7.0, 5.0, 11.0, 5.0),
    };
    style.spacing.item_spacing = egui::vec2(spacing_x, spacing_y);
    style.spacing.button_padding = egui::vec2(padding_x, padding_y);
    style.spacing.menu_margin = egui::Margin::same(if look == Look::Corpo { 6 } else { 8 });
    style.spacing.scroll = egui::style::ScrollStyle::solid();

    let (heading, body, button, small, mono) = match look {
        Look::White => (24.0, 14.0, 13.0, 11.0, 12.5),
        Look::Paper => (26.0, 14.0, 13.0, 11.0, 12.5),
        Look::Corpo => (23.0, 13.5, 12.5, 10.5, 12.0),
    };
    style.text_styles = [
        (TextStyle::Heading, serif_id(heading)),
        (TextStyle::Body, FontId::new(body, FontFamily::Proportional)),
        (
            TextStyle::Button,
            FontId::new(button, FontFamily::Proportional),
        ),
        (
            TextStyle::Small,
            FontId::new(small, FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(mono, FontFamily::Monospace),
        ),
    ]
    .into();
    style
}

/// Install the writing face. `None` searches the usual system serif paths;
/// `Some(path)` loads exactly that installed font. Folio ships no font files.
pub fn install_fonts(ctx: &egui::Context, path: Option<&Path>) {
    let mut fonts = FontDefinitions::default();
    let mut bytes = path
        .and_then(|path| std::fs::read(path).ok())
        .filter(|bytes| bytes.len() > 10_000);
    if bytes.is_none() {
        bytes = DEFAULT_SERIF_FONTS.iter().find_map(|candidate| {
            std::fs::read(candidate)
                .ok()
                .filter(|bytes| bytes.len() > 10_000)
        });
    }
    let family = writing_family();
    if let Some(bytes) = bytes {
        fonts
            .font_data
            .insert(SERIF_DATA.to_owned(), Arc::new(FontData::from_owned(bytes)));
        let mut family_list = vec![SERIF_DATA.to_owned()];
        if let Some(fallback) = fonts.families.get(&FontFamily::Proportional) {
            family_list.extend(fallback.iter().cloned());
        }
        fonts.families.insert(family, family_list);
    } else {
        let fallback = fonts
            .families
            .get(&FontFamily::Proportional)
            .cloned()
            .unwrap_or_default();
        fonts.families.insert(family, fallback);
    }
    ctx.set_fonts(fonts);
}

/// System serifs to try when the author has not chosen one. All of these are
/// installed by the operating system, never shipped with the app.
const DEFAULT_SERIF_FONTS: &[&str] = &[
    "C:/Windows/Fonts/georgia.ttf",
    "C:/Windows/Fonts/constan.ttf",
    "C:/Windows/Fonts/pala.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSerif.ttf",
    "/System/Library/Fonts/Supplemental/Georgia.ttf",
];

pub fn save_png(path: &Path, image: &egui::ColorImage) -> Result<(), String> {
    let [width, height] = image.size;
    let mut bytes = Vec::with_capacity(width * height * 4);
    for pixel in &image.pixels {
        bytes.extend_from_slice(&[pixel.r(), pixel.g(), pixel.b(), pixel.a()]);
    }
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("could not create screenshot folder: {e}"))?;
        }
    }
    image::save_buffer_with_format(
        path,
        &bytes,
        width as u32,
        height as u32,
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    )
    .map_err(|e| format!("could not write PNG: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// The live look is process-wide, so tests that switch it take turns.
    static TURN: Mutex<()> = Mutex::new(());

    fn turn() -> std::sync::MutexGuard<'static, ()> {
        TURN.lock().unwrap_or_else(|poison| poison.into_inner())
    }

    fn luminance(color: Color32) -> f32 {
        let channel = |value: u8| {
            let value = value as f32 / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(color.r()) + 0.7152 * channel(color.g()) + 0.0722 * channel(color.b())
    }

    fn contrast(a: Color32, b: Color32) -> f32 {
        let (first, second) = (luminance(a), luminance(b));
        let (light, dark) = if first > second {
            (first, second)
        } else {
            (second, first)
        };
        (light + 0.05) / (dark + 0.05)
    }

    #[test]
    fn the_shortcuts_read_the_live_palette() {
        let _turn = turn();
        for chosen in Look::ALL {
            set_look(chosen);
            assert_eq!(look(), chosen);
            assert_eq!(C::paper(), palette().paper);
            assert_eq!(C::accent(), palette().accent);
            assert_eq!(C::ink(), palette().ink);
        }
        set_look(Look::Paper);
    }

    #[test]
    fn paper_keeps_the_palette_folio_0_1_1_shipped() {
        // The regression guard for "Paper must not move": these are the exact
        // values from 0.1.1's LIGHT palette.
        assert_eq!(PAPER.paper, Color32::from_rgb(246, 241, 231));
        assert_eq!(PAPER.panel, Color32::from_rgb(252, 248, 240));
        assert_eq!(PAPER.card, Color32::from_rgb(255, 253, 247));
        assert_eq!(PAPER.ink, Color32::from_rgb(34, 30, 26));
        assert_eq!(PAPER.ink_soft, Color32::from_rgb(78, 70, 60));
        assert_eq!(PAPER.muted, Color32::from_rgb(136, 125, 109));
        assert_eq!(PAPER.line, Color32::from_rgb(222, 212, 195));
        assert_eq!(PAPER.accent, Color32::from_rgb(174, 82, 48));
        assert_eq!(PAPER.accent_deep, Color32::from_rgb(137, 59, 32));
        assert_eq!(PAPER.sidebar, Color32::from_rgb(27, 38, 34));
        assert_eq!(PAPER.sidebar_text, Color32::from_rgb(238, 232, 220));
        assert_eq!(PAPER.green, Color32::from_rgb(61, 105, 86));
        assert_eq!(PAPER.amber, Color32::from_rgb(171, 128, 57));
        assert_eq!(PAPER.red, Color32::from_rgb(158, 62, 52));
    }

    #[test]
    fn paper_keeps_the_colours_the_logo_is_drawn_with() {
        // tools/make_icon.py uses these two numbers; if one moves, the icon
        // and the window would no longer be the same mark.
        assert_eq!(PAPER.accent, Color32::from_rgb(174, 82, 48));
        assert_eq!(PAPER.paper, Color32::from_rgb(246, 241, 231));
        // The mark's orange is the accent in every look, so a button, a
        // selected chip and the logo all speak with one colour.
        assert_eq!(BRAND_ORANGE, PAPER.accent);
        assert_eq!(WHITE.accent, BRAND_ORANGE);
        assert_eq!(CORPO.accent, BRAND_ORANGE);
    }

    #[test]
    fn only_an_explicit_name_chooses_a_look() {
        assert_eq!(resolve_look(Some("white")), Some(Look::White));
        assert_eq!(resolve_look(Some(" Paper ")), Some(Look::Paper));
        assert_eq!(resolve_look(Some("CORPO")), Some(Look::Corpo));
        // The old names keep old screenshot commands working.
        assert_eq!(resolve_look(Some("light")), Some(Look::Paper));
        assert_eq!(resolve_look(Some("dark")), Some(Look::Corpo));
        // No override, or a word that names nothing, leaves the library alone.
        assert_eq!(resolve_look(None), None);
        assert_eq!(resolve_look(Some("sepia")), None);
        assert_eq!(resolve_look(Some("")), None);
    }

    #[test]
    fn a_look_survives_a_round_trip_and_an_unknown_name_falls_back() {
        assert_eq!(serde_json::to_string(&Look::Corpo).unwrap(), "\"corpo\"");
        assert_eq!(
            serde_json::from_str::<Look>("\"white\"").unwrap(),
            Look::White
        );
        // A hand-edited library must not stop the app from opening.
        assert_eq!(
            serde_json::from_str::<Look>("\"sepia\"").unwrap(),
            Look::Paper
        );
        assert_eq!(Look::default(), Look::Paper);
    }

    #[test]
    fn the_writing_face_is_only_used_by_paper() {
        let _turn = turn();
        set_look(Look::Paper);
        assert_eq!(serif(), writing_family());
        for look in [Look::White, Look::Corpo] {
            set_look(look);
            assert_eq!(serif(), FontFamily::Proportional);
            // The brand mark stays serif in every look.
            assert_eq!(brand_id(20.0).family, writing_family());
        }
        set_look(Look::Paper);
    }

    #[test]
    fn every_look_stays_readable() {
        let _turn = turn();
        // Paper is frozen by construction and checked separately, so only the
        // two new looks have to argue for their contrast.
        for (look, palette) in [(Look::White, WHITE), (Look::Corpo, CORPO)] {
            let pairs = if look.is_dark() {
                vec![
                    ("ink on paper", palette.ink, palette.paper, 12.0),
                    ("soft ink on paper", palette.ink_soft, palette.paper, 8.0),
                    ("muted on paper", palette.muted, palette.paper, 4.5),
                    ("hint on card", palette.hint, palette.card, 4.5),
                    (
                        "accent text on paper",
                        palette.accent_deep,
                        palette.paper,
                        6.0,
                    ),
                    ("green text on paper", palette.green, palette.paper, 6.0),
                    ("amber text on paper", palette.amber, palette.paper, 6.0),
                    ("red text on paper", palette.red, palette.paper, 4.5),
                    ("toast text", palette.on_toast, palette.toast, 7.0),
                ]
            } else {
                vec![
                    ("ink on paper", palette.ink, palette.paper, 12.0),
                    ("soft ink on paper", palette.ink_soft, palette.paper, 8.0),
                    ("muted on paper", palette.muted, palette.paper, 4.5),
                    ("hint on card", palette.hint, palette.card, 4.5),
                    (
                        "accent text on paper",
                        palette.accent_deep,
                        palette.paper,
                        6.0,
                    ),
                    ("green text on paper", palette.green, palette.paper, 6.0),
                    ("amber text on paper", palette.amber, palette.paper, 6.0),
                    ("red text on paper", palette.red, palette.paper, 4.5),
                    ("toast text", palette.on_toast, palette.toast, 7.0),
                    ("sidebar text", palette.sidebar_text, palette.sidebar, 9.0),
                    ("sidebar muted", palette.sidebar_muted, palette.sidebar, 4.5),
                ]
            };
            for (what, foreground, background, wanted) in pairs {
                let got = contrast(foreground, background);
                assert!(
                    got >= wanted,
                    "{:?}: {what}: {got:.2}:1 is below {wanted}:1",
                    look
                );
            }
            // White letters sit on these three in every look.
            for (what, background, wanted) in [
                ("white on the accent", palette.accent, 4.5),
                ("white on a delete button", palette.red_deep, 4.5),
                ("white on the avatar", palette.green_deep, 4.5),
            ] {
                let got = contrast(Color32::WHITE, background);
                assert!(
                    got >= wanted,
                    "{:?}: {what}: {got:.2}:1 is below {wanted}:1",
                    look
                );
            }
            for (what, foreground, background, wanted) in [
                ("pinned pill", palette.green, palette.green_soft, 4.5),
                ("warning pill", palette.amber, palette.amber_soft, 4.5),
                ("selected chip", Color32::WHITE, palette.accent, 4.5),
            ] {
                let got = contrast(foreground, background);
                assert!(
                    got >= wanted,
                    "{:?}: {what}: {got:.2}:1 is below {wanted}:1",
                    look
                );
            }
        }
    }

    #[test]
    fn corpo_is_dark_and_white_is_light() {
        assert!(luminance(CORPO.paper) < 0.02, "Corpo really is dark");
        assert!(luminance(WHITE.paper) > 0.8, "White stays bright");
        assert!(luminance(PAPER.paper) > 0.8, "Paper stays bright");
        assert!(Look::Corpo.is_dark());
        assert!(!Look::White.is_dark());
        assert!(!Look::Paper.is_dark());
    }
}
