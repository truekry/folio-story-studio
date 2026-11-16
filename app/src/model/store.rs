//! Where the library file lives and how it is read and written.

use super::*;

pub fn data_dir() -> PathBuf {
    if let Ok(appdata) = std::env::var("APPDATA") {
        PathBuf::from(appdata).join("Folio")
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".folio")
    } else {
        PathBuf::from("folio-data")
    }
}

pub fn library_path() -> PathBuf {
    data_dir().join("library.json")
}

pub fn load_library() -> (Library, Option<PathBuf>) {
    let path = library_path();
    if let Ok(text) = fs::read_to_string(&path) {
        if let Ok(mut library) = serde_json::from_str::<Library>(&text) {
            library.active_story = library
                .active_story
                .min(library.stories.len().saturating_sub(1));
            for story in &mut library.stories {
                let max = story.chapters.len().saturating_sub(1);
                story.open_chapter = story.open_chapter.min(max);
            }
            // Older files kept one conversation per story.
            library.adopt_story_chats();
            return (library, Some(path));
        }
        // Keep a readable copy of data we could not parse rather than overwriting it.
        let backup = path.with_extension("json.bak");
        let _ = fs::copy(&path, backup);
    }
    (Library::sample(), Some(path))
}

pub fn save_library(path: &PathBuf, library: &Library) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Could not create data folder: {e}"))?;
    }
    let text = serde_json::to_string_pretty(library)
        .map_err(|e| format!("Could not prepare the library: {e}"))?;
    fs::write(path, text).map_err(|e| format!("Could not save the library: {e}"))
}
