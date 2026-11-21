//! File names, chapter text and the PDF/text export.

use super::*;

/// Where an export goes when the author has not picked a folder yet:
/// Documents\Folio, so the files are easy to find afterwards.
pub fn default_export_dir() -> PathBuf {
    if let Ok(profile) = std::env::var("USERPROFILE") {
        let documents = PathBuf::from(&profile).join("Documents");
        if documents.is_dir() {
            return documents.join("Folio");
        }
        return PathBuf::from(profile).join("Folio");
    }
    data_dir().join("exports")
}

/// A name that Windows accepts as a file name, keeping the author's spelling,
/// capitals and umlauts where it can.
pub fn file_name_part(text: &str, fallback: &str) -> String {
    let mut out = String::new();
    let mut space = false;
    for ch in text.trim().chars() {
        let bad =
            ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*');
        if bad || ch.is_whitespace() {
            space = !out.is_empty();
            continue;
        }
        if space {
            out.push(' ');
            space = false;
        }
        out.push(ch);
    }
    let mut out = out.trim_end_matches(['.', ' ']).to_string();
    if out.chars().count() > 70 {
        out = out.chars().take(70).collect();
        out = out.trim_end_matches(['.', ' ']).to_string();
    }
    if out.is_empty() || windows_reserved(&out) {
        fallback.to_string()
    } else {
        out
    }
}

/// Windows reserves these names even with an extension, so `CON.pdf` cannot be
/// written at all. A book with such a title exports under the fallback name.
fn windows_reserved(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or("").trim_end();
    let stem = stem.to_ascii_uppercase();
    if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL") {
        return true;
    }
    stem.len() == 4
        && (stem.starts_with("COM") || stem.starts_with("LPT"))
        && stem.as_bytes()[3].is_ascii_digit()
        && stem.as_bytes()[3] != b'0'
}

/// A name the author typed, minus an extension of the same kind, so
/// "Draft.pdf" does not become "Draft.pdf.pdf".
pub(crate) fn without_extension<'a>(name: &'a str, extension: &str) -> &'a str {
    let trimmed = name.trim_end();
    let start = trimmed.len().saturating_sub(extension.len());
    if start > 0
        && trimmed.is_char_boundary(start)
        && trimmed[start..].eq_ignore_ascii_case(extension)
    {
        trimmed[..start].trim_end()
    } else {
        trimmed
    }
}

pub fn pdf_file_name(name: &str) -> String {
    format!(
        "{}.pdf",
        file_name_part(without_extension(name, ".pdf"), "Story")
    )
}

pub fn chapter_file_name(name: &str, story: &Story, index: usize) -> String {
    let chapter = &story.chapters[index];
    format!(
        "{}_{:02}_{}.txt",
        file_name_part(without_extension(name, ".txt"), "Story"),
        index + 1,
        file_name_part(&chapter.title, "Chapter")
    )
}

/// One chapter as a plain text file: which book it belongs to, which chapter
/// it is, then the prose. Windows line endings, so Notepad and Word both see
/// proper paragraphs.
pub fn chapter_text(story: &Story, index: usize) -> String {
    let chapter = &story.chapters[index];
    format!(
        "{}\r\nChapter {} of {} — {}\r\n\r\n{}\r\n",
        story.title.trim(),
        index + 1,
        story.chapters.len(),
        chapter.title.trim(),
        crlf(chapter.body.trim())
    )
}

pub(crate) fn crlf(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\n', "\r\n")
}

/// Where the PDF for `name` goes inside `dir`.
pub fn pdf_target(name: &str, dir: &Path) -> PathBuf {
    dir.join(pdf_file_name(name))
}

/// Where each chapter file for `name` goes inside `dir`.
pub fn chapter_targets(name: &str, story: &Story, dir: &Path) -> Vec<PathBuf> {
    (0..story.chapters.len())
        .map(|index| dir.join(chapter_file_name(name, story, index)))
        .collect()
}

/// The whole story as one PDF, written into `dir` under `name`.
pub fn export_pdf(story: &Story, name: &str, dir: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(dir).map_err(|e| format!("Could not create the folder: {e}"))?;
    let path = pdf_target(name, dir);
    fs::write(&path, crate::pdf::story_pdf(story))
        .map_err(|e| format!("Could not write {}: {e}", path.display()))?;
    Ok(path)
}

/// One text file per chapter, written into `dir` under `name`, as
/// `name_01_chapter title.txt`. The caller asks before replacing files.
pub fn export_chapters(story: &Story, name: &str, dir: &Path) -> Result<Vec<PathBuf>, String> {
    if story.chapters.is_empty() {
        return Err("This story has no chapters yet.".to_string());
    }
    fs::create_dir_all(dir).map_err(|e| format!("Could not create the folder: {e}"))?;
    let mut written = Vec::new();
    for (index, path) in chapter_targets(name, story, dir).into_iter().enumerate() {
        fs::write(&path, chapter_text(story, index))
            .map_err(|e| format!("Could not write {}: {e}", path.display()))?;
        written.push(path);
    }
    Ok(written)
}
