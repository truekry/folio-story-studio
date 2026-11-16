//! The fonts installed on this computer. Folio ships none of its own: the
//! writing font is always one of these files.

use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemFont {
    pub name: String,
    pub path: PathBuf,
}

/// Every installed font Folio can load, sorted by name. Registry entries give
/// the pretty family name; a folder scan stands in when the registry is not
/// available. `.ttc` collections are skipped: egui loads one face per file.
pub fn system_fonts() -> Vec<SystemFont> {
    let mut fonts = platform_fonts();
    fonts.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.path.cmp(&b.path))
    });
    fonts.dedup_by(|a, b| a.path == b.path || a.name.eq_ignore_ascii_case(&b.name));
    fonts
}

#[cfg(windows)]
fn platform_fonts() -> Vec<SystemFont> {
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegEnumValueW, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE,
        KEY_READ, REG_EXPAND_SZ, REG_SZ,
    };

    const SUBKEY: &str = "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Fonts";
    let mut fonts = Vec::new();
    for root in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
        let subkey: Vec<u16> = SUBKEY.encode_utf16().chain(std::iter::once(0)).collect();
        let mut key: HKEY = std::ptr::null_mut();
        // SAFETY: `subkey` is a NUL-terminated UTF-16 string and `key` is a
        // valid out pointer; the key is closed below.
        let opened = unsafe { RegOpenKeyExW(root, subkey.as_ptr(), 0, KEY_READ, &mut key) };
        if opened != 0 {
            continue;
        }
        let mut index = 0;
        loop {
            let mut name = [0u16; 512];
            let mut name_len = name.len() as u32;
            let mut data = [0u8; 4096];
            let mut data_len = data.len() as u32;
            let mut kind = 0u32;
            // SAFETY: every pointer belongs to a live buffer whose length is
            // passed alongside it; the call writes at most that many units.
            let rc = unsafe {
                RegEnumValueW(
                    key,
                    index,
                    name.as_mut_ptr(),
                    &mut name_len,
                    std::ptr::null(),
                    &mut kind,
                    data.as_mut_ptr(),
                    &mut data_len,
                )
            };
            if rc != 0 {
                break;
            }
            index += 1;
            if kind != REG_SZ && kind != REG_EXPAND_SZ {
                continue;
            }
            let raw_name = String::from_utf16_lossy(&name[..name_len as usize]);
            let name = clean_name(&raw_name);
            if name.is_empty() {
                continue;
            }
            let raw_value = decode_utf16(&data[..data_len as usize]);
            let raw_value = if kind == REG_EXPAND_SZ {
                expand_env(&raw_value)
            } else {
                raw_value
            };
            let Some(path) = resolve_font_path(&raw_value) else {
                continue;
            };
            fonts.push(SystemFont { name, path });
        }
        // SAFETY: `key` came from a successful RegOpenKeyExW.
        unsafe { RegCloseKey(key) };
    }
    if fonts.is_empty() {
        folder_fonts()
    } else {
        fonts
    }
}

#[cfg(not(windows))]
fn platform_fonts() -> Vec<SystemFont> {
    folder_fonts()
}

/// The last resort when the registry cannot be read: look in the usual font
/// folders and use the file stem as the name.
fn folder_fonts() -> Vec<SystemFont> {
    let mut dirs = Vec::new();
    #[cfg(windows)]
    {
        if let Some(windir) = std::env::var_os("WINDIR") {
            dirs.push(PathBuf::from(windir).join("Fonts"));
        }
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            dirs.push(PathBuf::from(local).join("Microsoft\\Windows\\Fonts"));
        }
    }
    #[cfg(not(windows))]
    {
        dirs.push(PathBuf::from("/usr/share/fonts"));
        dirs.push(PathBuf::from("/usr/local/share/fonts"));
        if let Some(home) = std::env::var_os("HOME") {
            let home = PathBuf::from(home);
            dirs.push(home.join(".fonts"));
            dirs.push(home.join(".local/share/fonts"));
        }
        dirs.push(PathBuf::from("/System/Library/Fonts"));
        dirs.push(PathBuf::from("/Library/Fonts"));
    }

    let mut fonts = Vec::new();
    for dir in dirs {
        collect_fonts(&dir, 0, &mut fonts);
    }
    fonts
}

fn collect_fonts(dir: &Path, depth: usize, out: &mut Vec<SystemFont>) {
    if depth > 2 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_fonts(&path, depth + 1, out);
            continue;
        }
        let Some(ext) = path.extension().and_then(|ext| ext.to_str()) else {
            continue;
        };
        if !ext.eq_ignore_ascii_case("ttf") && !ext.eq_ignore_ascii_case("otf") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        let name = stem.replace(['_', '-'], " ");
        if name.trim().is_empty() {
            continue;
        }
        out.push(SystemFont {
            name: name.trim().to_string(),
            path,
        });
    }
}

#[cfg(windows)]
fn decode_utf16(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .take_while(|unit| *unit != 0)
        .collect();
    String::from_utf16_lossy(&units)
}

/// Registry names look like "Georgia (TrueType)" or "Arial Bold (TrueType)".
#[cfg(windows)]
fn clean_name(raw: &str) -> String {
    raw.split(" (")
        .next()
        .unwrap_or(raw)
        .trim()
        .to_string()
}

/// A font value is either an absolute path or a file name inside a font
/// folder. Values can contain `%WINDIR%`-style variables.
#[cfg(windows)]
fn resolve_font_path(raw: &str) -> Option<PathBuf> {
    let raw = raw.trim().trim_matches('"');
    if raw.is_empty() {
        return None;
    }
    let extension = Path::new(raw)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default();
    if !extension.eq_ignore_ascii_case("ttf") && !extension.eq_ignore_ascii_case("otf") {
        return None;
    }
    let path = PathBuf::from(raw);
    if path.is_absolute() && path.exists() {
        return Some(path);
    }
    let mut bases = Vec::new();
    if let Some(windir) = std::env::var_os("WINDIR") {
        bases.push(PathBuf::from(windir).join("Fonts"));
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        bases.push(PathBuf::from(local).join("Microsoft\\Windows\\Fonts"));
    }
    for base in bases {
        let candidate = base.join(raw);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

#[cfg(windows)]
fn expand_env(value: &str) -> String {
    let mut out = String::new();
    let mut rest = value;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let Some(end) = after.find('%') else {
            out.push('%');
            rest = after;
            continue;
        };
        let variable = &after[..end];
        match std::env::var_os(variable) {
            Some(value) => out.push_str(&value.to_string_lossy()),
            None => {
                out.push('%');
                out.push_str(variable);
                out.push('%');
            }
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_font_list_is_made_of_real_files() {
        let fonts = system_fonts();
        assert!(
            !fonts.is_empty(),
            "the system must offer at least one font"
        );
        assert!(
            fonts.iter().all(|font| font.path.exists()),
            "every listed font must exist on disk"
        );
        assert!(
            fonts.iter().all(|font| !font.name.trim().is_empty()),
            "every listed font needs a name"
        );
        // The same family must not appear twice just because two files exist.
        let mut names: Vec<String> = fonts.iter().map(|font| font.name.to_lowercase()).collect();
        names.sort();
        let before = names.len();
        names.dedup();
        assert_eq!(before, names.len(), "font names must be unique");
    }
}
