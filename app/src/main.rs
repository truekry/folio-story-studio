#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ai;
mod app;
mod folder;
mod fonts;
mod model;
mod pdf;
mod theme;

use eframe::egui;
use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    let (screenshot, help) = parse_args();
    if help {
        println!("Folio — a story studio with an AI writing partner");
        println!();
        println!("Usage: folio [--screenshot <file.png>]");
        println!("  --screenshot  draw the window for a moment, save a PNG, then exit");
        println!();
        println!("Light or dark follows Windows. To force one of the two:");
        println!("  FOLIO_THEME=dark   or   FOLIO_THEME=light");
        return Ok(());
    }

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Folio — Story Studio")
        .with_inner_size([1440.0, 900.0])
        .with_min_inner_size([1040.0, 680.0]);
    if let Some(icon) = app_icon() {
        viewport = viewport.with_icon(icon);
    }

    if screenshot.is_some() {
        // A check must never pull focus away from whatever the person is doing.
        viewport = viewport.with_active(false);
        #[cfg(target_os = "windows")]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    }

    let options = eframe::NativeOptions {
        viewport,
        vsync: true,
        centered: true,
        ..Default::default()
    };

    let shot = screenshot.clone();
    eframe::run_native(
        "Folio",
        options,
        Box::new(move |cc| Ok(Box::new(app::FolioApp::new(cc, shot.clone())))),
    )
}

/// The same logo the window draws top-left: an orange rounded square with a
/// white serif "F". `assets/icon.png` is rendered from this by
/// `tools/make_icon.py`; here it becomes the window and taskbar icon.
fn app_icon() -> Option<egui::IconData> {
    let bytes = include_bytes!("../assets/icon.png");
    let image = image::load_from_memory(bytes).ok()?.into_rgba8();
    let (width, height) = image.dimensions();
    Some(egui::IconData {
        rgba: image.into_raw(),
        width,
        height,
    })
}

fn parse_args() -> (Option<PathBuf>, bool) {
    let mut screenshot = None;
    let mut help = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--screenshot" {
            match args.next() {
                Some(path) if !path.starts_with('-') => screenshot = Some(PathBuf::from(path)),
                _ => {
                    eprintln!("--screenshot needs a file path, e.g. --screenshot shots/now.png");
                    std::process::exit(2);
                }
            }
        } else if let Some(path) = arg.strip_prefix("--screenshot=") {
            screenshot = Some(PathBuf::from(path));
        } else if arg == "--help" || arg == "-h" {
            help = true;
        }
    }
    (screenshot, help)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_window_icon_decodes_to_a_square_image() {
        let icon = app_icon().expect("assets/icon.png must decode");
        assert_eq!(icon.width, 256);
        assert_eq!(icon.height, 256);
        assert_eq!(icon.rgba.len(), 256 * 256 * 4);
        // The logo is an orange square with a white "F", not an empty canvas.
        assert!(icon.rgba.chunks(4).any(|pixel| pixel[..3] == [174, 82, 48]));
        assert!(icon.rgba.chunks(4).any(|pixel| pixel[3] > 200 && pixel[0] > 240));
    }

    #[test]
    fn the_exe_icon_holds_every_size_windows_asks_for() {
        let bytes = include_bytes!("../assets/icon.ico");
        assert_eq!(&bytes[..4], &[0, 0, 1, 0], "an .ico header");
        let count = u16::from_le_bytes([bytes[4], bytes[5]]) as usize;
        let mut sizes = Vec::new();
        for index in 0..count {
            let width = bytes[6 + index * 16];
            sizes.push(if width == 0 { 256 } else { width as usize });
        }
        for wanted in [16, 32, 48, 256] {
            assert!(sizes.contains(&wanted), "the .ico needs a {wanted}px frame: {sizes:?}");
        }
    }
}

