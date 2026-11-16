//! Folio's paper-and-ink visual language, in two lights.
//!
//! One set of names, two palettes: warm paper with dark ink for a bright room,
//! and the same paper at night — near-black, but still warm, so the orange
//! stays the same orange. Which one is live is decided once per change by
//! [`follow_system`], and everything that paints reads it through [`C`].

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke, TextStyle,
    Visuals,
};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

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
    /// The toast card: near-black in daylight, raised warm grey at night.
    pub toast: Color32,
    pub on_toast: Color32,
}

/// Day: warm paper, dark ink, the accent the logo and the .exe icon use.
pub const LIGHT: Palette = Palette {
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

/// Night: the same hue family, turned inside out. The accent stays exactly the
/// logo's orange, so the window, the taskbar and the .exe keep one face.
pub const DARK: Palette = Palette {
    paper: Color32::from_rgb(27, 24, 21),
    panel: Color32::from_rgb(34, 31, 26),
    card: Color32::from_rgb(42, 37, 31),
    soft: Color32::from_rgb(51, 45, 37),
    summary_bg: Color32::from_rgb(45, 40, 33),
    draft_bg: Color32::from_rgb(32, 41, 35),
    draft_line: Color32::from_rgb(57, 74, 61),
    find_bg: Color32::from_rgb(78, 64, 34),
    find_active_bg: Color32::from_rgb(122, 88, 38),
    ink: Color32::from_rgb(239, 231, 217),
    ink_soft: Color32::from_rgb(205, 191, 174),
    muted: Color32::from_rgb(154, 141, 124),
    hint: Color32::from_rgb(172, 159, 141),
    line: Color32::from_rgb(69, 61, 51),
    accent: Color32::from_rgb(174, 82, 48),
    accent_deep: Color32::from_rgb(233, 164, 127),
    accent_soft: Color32::from_rgb(58, 36, 27),
    green: Color32::from_rgb(143, 191, 159),
    green_soft: Color32::from_rgb(34, 50, 42),
    green_deep: Color32::from_rgb(60, 107, 85),
    amber: Color32::from_rgb(214, 174, 107),
    amber_soft: Color32::from_rgb(56, 46, 30),
    sidebar: Color32::from_rgb(19, 26, 23),
    sidebar_card: Color32::from_rgb(33, 44, 39),
    sidebar_hover: Color32::from_rgb(26, 36, 32),
    sidebar_text: Color32::from_rgb(237, 230, 216),
    sidebar_muted: Color32::from_rgb(148, 160, 146),
    red: Color32::from_rgb(208, 106, 91),
    red_deep: Color32::from_rgb(158, 66, 52),
    toast: Color32::from_rgb(59, 51, 42),
    on_toast: Color32::from_rgb(245, 238, 226),
};

static NIGHT: AtomicBool = AtomicBool::new(false);

/// The colours in use right now.
pub fn palette() -> &'static Palette {
    if NIGHT.load(Ordering::Relaxed) {
        &DARK
    } else {
        &LIGHT
    }
}

pub fn is_dark() -> bool {
    NIGHT.load(Ordering::Relaxed)
}

fn set_dark(dark: bool) {
    NIGHT.store(dark, Ordering::Relaxed);
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

pub fn serif() -> FontFamily {
    FontFamily::Name(Arc::from("folio-serif"))
}

pub fn serif_id(size: f32) -> FontId {
    FontId::new(size, serif())
}

pub fn install(ctx: &egui::Context) {
    use_writing_font(ctx, None);
    // Both themes get one of our styles, so egui never falls back to its own
    // colours — not even for a single frame while the system theme changes.
    ctx.set_style_of(egui::Theme::Light, style_for(&LIGHT, false));
    ctx.set_style_of(egui::Theme::Dark, style_for(&DARK, true));
    set_dark(wants_dark(ctx));
    ctx.set_theme(preference());
}

/// Follow the operating system, called once per frame. Cheap when nothing
/// changed; when the person flips Windows to dark, the window follows on the
/// next frame.
pub fn follow_system(ctx: &egui::Context) {
    let dark = wants_dark(ctx);
    if dark != is_dark() {
        set_dark(dark);
        ctx.set_theme(preference());
        ctx.request_repaint();
    }
}

fn preference() -> egui::Theme {
    if is_dark() {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    }
}

/// What the OS asks for, unless `FOLIO_THEME=dark` or `FOLIO_THEME=light`
/// overrides it. The override exists for screenshots and for checking both
/// looks on one machine.
fn wants_dark(ctx: &egui::Context) -> bool {
    resolve(ctx.system_theme(), std::env::var("FOLIO_THEME").ok().as_deref())
}

fn resolve(system: Option<egui::Theme>, forced: Option<&str>) -> bool {
    match forced.map(|value| value.trim().to_ascii_lowercase()).as_deref() {
        Some("dark") => true,
        Some("light") => false,
        // No report from the OS: stay with the daylight palette the app grew
        // up with, rather than guessing.
        _ => matches!(system, Some(egui::Theme::Dark)),
    }
}

/// The whole egui styling for one palette: panel colours, widget states,
/// spacing and type sizes.
fn style_for(palette: &Palette, dark: bool) -> egui::Style {
    let mut style = egui::Style::default();
    let mut visuals = if dark { Visuals::dark() } else { Visuals::light() };
    visuals.dark_mode = dark;
    visuals.override_text_color = Some(palette.ink);
    visuals.panel_fill = palette.paper;
    visuals.window_fill = palette.panel;
    visuals.extreme_bg_color = palette.card;
    visuals.faint_bg_color = palette.soft;
    visuals.code_bg_color = palette.soft;
    visuals.window_corner_radius = CornerRadius::same(14);
    visuals.menu_corner_radius = CornerRadius::same(10);
    visuals.window_stroke = Stroke::new(1.0_f32, palette.line);
    visuals.selection.bg_fill = palette.accent;
    visuals.selection.stroke = Stroke::new(1.0_f32, Color32::WHITE);
    visuals.hyperlink_color = palette.accent_deep;
    visuals.warn_fg_color = palette.amber;
    visuals.error_fg_color = palette.red;

    visuals.widgets.noninteractive.bg_fill = palette.panel;
    visuals.widgets.noninteractive.weak_bg_fill = palette.panel;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, palette.line);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, palette.ink_soft);
    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(9);
    visuals.widgets.noninteractive.expansion = 0.0;

    visuals.widgets.inactive.bg_fill = palette.soft;
    visuals.widgets.inactive.weak_bg_fill = palette.soft;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, palette.line);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, palette.ink);
    visuals.widgets.inactive.corner_radius = CornerRadius::same(9);
    visuals.widgets.inactive.expansion = 0.0;

    visuals.widgets.hovered.bg_fill = palette.accent_soft;
    visuals.widgets.hovered.weak_bg_fill = palette.accent_soft;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, palette.accent);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.2_f32, palette.accent_deep);
    visuals.widgets.hovered.corner_radius = CornerRadius::same(9);
    visuals.widgets.hovered.expansion = 0.0;

    visuals.widgets.active.bg_fill = palette.accent_soft;
    visuals.widgets.active.weak_bg_fill = palette.accent_soft;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, palette.accent);
    visuals.widgets.active.fg_stroke = Stroke::new(1.2_f32, palette.accent_deep);
    visuals.widgets.active.corner_radius = CornerRadius::same(9);
    visuals.widgets.active.expansion = 0.0;

    visuals.widgets.open.bg_fill = palette.card;
    visuals.widgets.open.weak_bg_fill = palette.card;
    visuals.widgets.open.bg_stroke = Stroke::new(1.0_f32, palette.line);
    visuals.widgets.open.fg_stroke = Stroke::new(1.0_f32, palette.ink);
    visuals.widgets.open.corner_radius = CornerRadius::same(9);

    style.visuals = visuals;
    style.spacing.item_spacing = egui::vec2(8.0, 7.0);
    style.spacing.button_padding = egui::vec2(12.0, 7.0);
    style.spacing.menu_margin = egui::Margin::same(8);
    style.spacing.scroll = egui::style::ScrollStyle::solid();
    style.text_styles = [
        (TextStyle::Heading, serif_id(26.0)),
        (
            TextStyle::Body,
            FontId::new(14.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Button,
            FontId::new(13.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Small,
            FontId::new(11.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(12.5, FontFamily::Monospace),
        ),
    ]
    .into();
    style
}

/// Install the writing face. `None` searches the usual system serif paths;
/// `Some(path)` loads exactly that installed font. Folio ships no font files.
pub fn use_writing_font(ctx: &egui::Context, path: Option<&Path>) {
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
    if let Some(bytes) = bytes {
        fonts.font_data.insert(
            "folio-serif-data".to_owned(),
            Arc::new(FontData::from_owned(bytes)),
        );
        let mut family = vec!["folio-serif-data".to_owned()];
        if let Some(fallback) = fonts.families.get(&FontFamily::Proportional) {
            family.extend(fallback.iter().cloned());
        }
        fonts.families.insert(serif(), family);
    } else {
        let fallback = fonts
            .families
            .get(&FontFamily::Proportional)
            .cloned()
            .unwrap_or_default();
        fonts.families.insert(serif(), fallback);
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

    /// The live palette is process-wide, so tests that switch it take turns.
    static TURN: Mutex<()> = Mutex::new(());

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
        let _turn = TURN.lock().unwrap();
        set_dark(false);
        assert_eq!(C::paper(), LIGHT.paper);
        assert_eq!(C::accent(), LIGHT.accent);

        set_dark(true);
        assert_eq!(C::paper(), DARK.paper);
        assert_eq!(C::ink(), DARK.ink);
        assert!(is_dark());

        set_dark(false);
        assert_eq!(C::paper(), LIGHT.paper);
        assert!(!is_dark());
    }

    #[test]
    fn daylight_keeps_the_colours_the_logo_is_drawn_with() {
        // tools/make_icon.py uses these two numbers; if one moves, the icon
        // and the window would no longer be the same mark.
        assert_eq!(LIGHT.accent, Color32::from_rgb(174, 82, 48));
        assert_eq!(LIGHT.paper, Color32::from_rgb(246, 241, 231));
        // Night keeps the orange exactly.
        assert_eq!(DARK.accent, LIGHT.accent);
    }

    #[test]
    fn the_system_theme_decides_unless_an_override_says_otherwise() {
        use egui::Theme;
        // The OS answer wins when there is no override …
        assert!(resolve(Some(Theme::Dark), None));
        assert!(!resolve(Some(Theme::Light), None));
        // … and an unknown OS (a Linux desktop without a portal, say) stays light.
        assert!(!resolve(None, None));
        // The override wins over the OS, and tolerates stray spaces and case.
        assert!(resolve(Some(Theme::Light), Some("dark")));
        assert!(resolve(Some(Theme::Light), Some(" DARK ")));
        assert!(!resolve(Some(Theme::Dark), Some("light")));
        // Anything else is not an override.
        assert!(!resolve(None, Some("sepia")));
    }

    #[test]
    fn night_is_dark_and_its_text_stays_readable() {
        let night = DARK;
        assert!(luminance(night.paper) < 0.02, "the paper really is dark");
        assert!(luminance(LIGHT.paper) > 0.8, "the day paper stays bright");

        let pairs = [
            ("ink on paper", night.ink, night.paper, 12.0),
            ("soft ink on paper", night.ink_soft, night.paper, 8.0),
            ("muted on paper", night.muted, night.paper, 4.5),
            ("hint on card", night.hint, night.card, 4.5),
            ("accent text on paper", night.accent_deep, night.paper, 6.0),
            ("green text on paper", night.green, night.paper, 6.0),
            ("amber text on paper", night.amber, night.paper, 6.0),
            ("red text on paper", night.red, night.paper, 4.5),
            ("toast text", night.on_toast, night.toast, 7.0),
            ("white on the accent", Color32::WHITE, night.accent, 4.5),
            ("white on a delete button", Color32::WHITE, night.red_deep, 4.5),
            ("white on the avatar", Color32::WHITE, night.green_deep, 4.5),
            ("sidebar text", night.sidebar_text, night.sidebar, 9.0),
            ("sidebar muted", night.sidebar_muted, night.sidebar, 4.5),
            ("pinned pill", night.green, night.green_soft, 4.5),
            ("warning pill", night.amber, night.amber_soft, 4.5),
            ("selected chip", Color32::WHITE, night.accent, 4.5),
        ];
        for (what, foreground, background, wanted) in pairs {
            let got = contrast(foreground, background);
            assert!(got >= wanted, "{what}: {got:.2}:1 is below {wanted}:1");
        }
    }
}
