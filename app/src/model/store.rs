//! Where the library lives and how it is read and written.
//!
//! Settings live in `config.json`. Every story has its own `stories\<id>.json`.
//! The old single `library.json` is migrated once and kept as a backup.

use super::*;
use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

/// Settings and UI state, separate from the books.
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Config {
    #[serde(default = "one")]
    pub version: u64,
    #[serde(default)]
    pub settings: AiSettings,
    #[serde(default)]
    pub writing_font: String,
    #[serde(default = "default_writing_size")]
    pub writing_size: f32,
    #[serde(default)]
    pub export_dir: String,
    #[serde(default = "yes")]
    pub export_pdf: bool,
    /// The look chosen in the settings. Older config files have none and open
    /// in Paper.
    #[serde(default)]
    pub look: crate::theme::Look,
    /// The open story by key, not by position.
    #[serde(default)]
    pub active_story_key: String,
}

impl Config {
    fn from_library(library: &Library) -> Self {
        Self {
            version: 1,
            settings: library.settings.clone(),
            writing_font: library.writing_font.clone(),
            writing_size: library.writing_size,
            export_dir: library.export_dir.clone(),
            export_pdf: library.export_pdf,
            look: library.look,
            active_story_key: library
                .stories
                .get(library.active_story)
                .map(|story| story.key.clone())
                .unwrap_or_default(),
        }
    }

    fn into_library(self, stories: Vec<Story>) -> Library {
        let active_story = stories
            .iter()
            .position(|story| story.key == self.active_story_key)
            .unwrap_or(0);
        let mut library = Library {
            stories,
            settings: self.settings,
            active_story,
            writing_font: self.writing_font,
            writing_size: self.writing_size,
            export_dir: self.export_dir,
            export_pdf: self.export_pdf,
            look: self.look,
        };
        normalize(&mut library);
        library
    }
}

/// What happened while opening the data folder, for the one-time notice.
#[derive(Default)]
pub(crate) struct LoadReport {
    pub migrated: bool,
    pub backup: Option<PathBuf>,
    pub warnings: Vec<String>,
}

/// The data folder, its last-written fingerprints, and the migration state.
pub(crate) struct Store {
    dir: PathBuf,
    last_config: Option<u64>,
    last_stories: HashMap<String, u64>,
    /// Files we loaded or wrote ourselves. Only these may be removed by the
    /// delete sweep; a file someone drops into `stories\` later is left alone.
    known_files: HashSet<String>,
}

impl Store {
    /// Open the new layout. If only the old `library.json` exists, it is
    /// converted first and then kept as a `.migrated-*` backup.
    pub(crate) fn open(dir: &Path) -> (Library, Store, LoadReport) {
        let config_path = dir.join("config.json");
        let legacy_path = dir.join("library.json");
        let stories_dir = dir.join("stories");
        let mut report = LoadReport::default();
        let mut cache = HashMap::new();
        let mut last_config = None;

        let config = if config_path.exists() {
            match read_text(&config_path) {
                Ok(text) => {
                    let hash = content_hash(&text);
                    match serde_json::from_str::<Config>(&text) {
                        Ok(mut config) => {
                            last_config = Some(hash);
                            if let Some(warning) = secrets::unlock(dir, &mut config.settings) {
                                report.warnings.push(warning);
                            }
                            Some(config)
                        }
                        Err(error) => {
                            let backup = backup_file(&config_path, "broken", &mut report);
                            report.warnings.push(format!(
                                "config.json could not be read: {error}. The file was kept as {}.",
                                backup
                                    .map(|path| path.display().to_string())
                                    .unwrap_or_else(|| config_path.display().to_string())
                            ));
                            None
                        }
                    }
                }
                Err(error) => {
                    report
                        .warnings
                        .push(format!("config.json could not be read: {error}"));
                    None
                }
            }
        } else {
            None
        };

        // No usable config, but an old library: migrate it now.
        if config.is_none() && legacy_path.exists() {
            match read_text(&legacy_path)
                .map_err(|error| error.to_string())
                .and_then(|text| {
                    serde_json::from_str::<Library>(&text).map_err(|error| error.to_string())
                }) {
                Ok(mut library) => {
                    normalize(&mut library);
                    match write_migration(dir, &library) {
                        Ok((config_hash, story_hashes)) => {
                            last_config = Some(config_hash);
                            cache = story_hashes;
                            report.migrated = true;
                            match rename_migrated(&legacy_path) {
                                Ok(backup) => report.backup = Some(backup),
                                Err(error) => report.warnings.push(format!(
                                    "The old library was migrated but could not be moved aside: {error}"
                                )),
                            }
                            let known_files = cache.keys().cloned().collect();
                            let store = Store {
                                dir: dir.to_path_buf(),
                                last_config,
                                last_stories: cache,
                                known_files,
                            };
                            return (library, store, report);
                        }
                        Err(error) => report.warnings.push(format!(
                            "The old library could not be migrated, so it was left untouched: {error}"
                        )),
                    }
                    let store = Store {
                        dir: dir.to_path_buf(),
                        last_config: None,
                        last_stories: HashMap::new(),
                        known_files: HashSet::new(),
                    };
                    return (library, store, report);
                }
                Err(error) => {
                    let backup = backup_file(&legacy_path, "broken", &mut report);
                    report.warnings.push(format!(
                        "library.json could not be read: {error}. It was kept as {}.",
                        backup
                            .map(|path| path.display().to_string())
                            .unwrap_or_else(|| legacy_path.display().to_string())
                    ));
                }
            }
        }

        let stories = read_stories(&stories_dir, &mut report, &mut cache);
        let has_config = config.is_some();
        if !has_config && !stories.is_empty() {
            report.warnings.push(
                "config.json was missing; the books were loaded with default settings.".to_string(),
            );
        }
        if has_config && stories.is_empty() {
            report
                .warnings
                .push("config.json exists, but no books were found in stories\\.".to_string());
        }

        let mut library = match config {
            Some(config) => config.into_library(stories),
            None => Config::default().into_library(stories),
        };
        let known_files = cache.keys().cloned().collect();
        let mut store = Store {
            dir: dir.to_path_buf(),
            last_config,
            last_stories: cache,
            known_files,
        };
        // A key from an older version is sealed right now, so the plain text
        // does not survive this start, even if the author changes nothing.
        if !library.settings.api_key.trim().is_empty()
            && library.settings.api_key_sealed.trim().is_empty()
        {
            match secrets::seal(store.dir(), &library.settings.api_key) {
                Ok(sealed) => {
                    library.settings.api_key_sealed = sealed;
                    if let Err(errors) = store.save(&library) {
                        report.warnings.extend(errors);
                    }
                }
                Err(error) => report
                    .warnings
                    .push(format!("The API key could not be sealed: {error}.")),
            }
        }
        if secrets::machine_id().is_none() {
            report.warnings.push(
                "No machine id was available, so the API key is bound to this data folder only."
                    .to_string(),
            );
        }
        (library, store, report)
    }

    pub(crate) fn dir(&self) -> &Path {
        &self.dir
    }

    /// Write the books that changed, remove files for deleted books, and write
    /// the config last. A failed file keeps its old fingerprint so the next
    /// save retries it.
    pub(crate) fn save(&mut self, library: &Library) -> Result<(), Vec<String>> {
        let stories_dir = self.dir.join("stories");
        let mut errors = Vec::new();
        if let Err(error) = fs::create_dir_all(&stories_dir) {
            errors.push(format!("Could not create the stories folder: {error}"));
            return Err(errors);
        }

        let mut keys = HashSet::new();
        for story in &library.stories {
            if story.key.is_empty() {
                errors.push(format!(
                    "Could not save “{}”: the story has no key.",
                    story.title
                ));
                continue;
            }
            keys.insert(story.key.clone());
            let text = match serde_json::to_string_pretty(story) {
                Ok(text) => text,
                Err(error) => {
                    errors.push(format!(
                        "Could not prepare story “{}”: {error}",
                        story.title
                    ));
                    continue;
                }
            };
            let hash = content_hash(&text);
            if self.last_stories.get(&story.key) == Some(&hash) {
                continue;
            }
            let path = story_path(&stories_dir, &story.key);
            match write_atomic(&path, &text) {
                Ok(()) => {
                    self.last_stories.insert(story.key.clone(), hash);
                    self.known_files.insert(story.key.clone());
                }
                Err(error) => errors.push(format!("Could not save “{}”: {error}", story.title)),
            }
        }

        // Files for books that no longer exist, including old numeric file
        // names from the first split layout. A temporary or backup file is
        // left alone; only a <key>.json is a live book.
        if let Ok(entries) = fs::read_dir(&stories_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                    continue;
                }
                let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
                    continue;
                };
                if keys.contains(stem) || !self.known_files.contains(stem) {
                    continue;
                }
                if let Err(error) = fs::remove_file(&path) {
                    errors.push(format!(
                        "Could not remove the old book file {}: {error}",
                        path.display()
                    ));
                } else {
                    self.last_stories.remove(stem);
                    self.known_files.remove(stem);
                }
            }
        }

        let mut config = Config::from_library(library);
        if let Some(warning) = secrets::lock(&self.dir, &mut config.settings) {
            errors.push(warning);
        }
        match serde_json::to_string_pretty(&config) {
            Ok(text) => {
                let hash = content_hash(&text);
                if self.last_config != Some(hash) {
                    let path = self.dir.join("config.json");
                    match write_atomic(&path, &text) {
                        Ok(()) => self.last_config = Some(hash),
                        Err(error) => errors.push(format!("Could not save config.json: {error}")),
                    }
                }
            }
            Err(error) => errors.push(format!("Could not prepare config.json: {error}")),
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::from_library(&Library::empty())
    }
}

pub fn data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("FOLIO_DATA_DIR") {
        let dir = dir.trim();
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        PathBuf::from(appdata).join("Folio")
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".folio")
    } else {
        PathBuf::from("folio-data")
    }
}

fn story_path(stories_dir: &Path, key: &str) -> PathBuf {
    stories_dir.join(format!("{key}.json"))
}

/// Read a JSON file the way Windows editors write it: an external tool may
/// prepend a UTF-8 byte-order mark, which `serde_json` rejects. The file itself
/// is never rewritten here; only the text handed to the parser is trimmed.
pub(crate) fn read_text(path: &Path) -> std::io::Result<String> {
    let text = fs::read_to_string(path)?;
    Ok(strip_bom(&text).to_string())
}

/// A leading UTF-8 BOM is not JSON and not part of the document.
pub(crate) fn strip_bom(text: &str) -> &str {
    text.strip_prefix('\u{feff}').unwrap_or(text)
}

fn normalize(library: &mut Library) {
    library.active_story = library
        .active_story
        .min(library.stories.len().saturating_sub(1));
    for story in &mut library.stories {
        story.open_chapter = story
            .open_chapter
            .min(story.chapters.len().saturating_sub(1));
    }
    library.adopt_story_chats();
    ensure_library_keys(library);
}

fn read_stories(
    stories_dir: &Path,
    report: &mut LoadReport,
    cache: &mut HashMap<String, u64>,
) -> Vec<Story> {
    let mut stories = Vec::new();
    let Ok(entries) = fs::read_dir(stories_dir) else {
        return stories;
    };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
    paths.sort();
    for path in paths {
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let text = match read_text(&path) {
            Ok(text) => text,
            Err(error) => {
                // Move it aside like an unreadable JSON file. Left in place it
                // would look like a deleted book to the next save's cleanup.
                let backup = backup_file(&path, "broken", report);
                report.warnings.push(format!(
                    "Could not read the book file {}: {error}. It was kept as {}.",
                    path.display(),
                    backup
                        .map(|backup| backup.display().to_string())
                        .unwrap_or_else(|| path.display().to_string())
                ));
                continue;
            }
        };
        match serde_json::from_str::<Story>(&text) {
            Ok(story) => {
                if !story.key.is_empty() {
                    cache.insert(story.key.clone(), content_hash(&text));
                }
                stories.push(story);
            }
            Err(error) => {
                let backup = backup_file(&path, "broken", report);
                report.warnings.push(format!(
                    "The book file {} could not be read: {error}. It was kept as {}.",
                    path.display(),
                    backup
                        .map(|backup| backup.display().to_string())
                        .unwrap_or_else(|| path.display().to_string())
                ));
            }
        }
    }
    stories
}

fn write_migration(dir: &Path, library: &Library) -> Result<(u64, HashMap<String, u64>), String> {
    let stories_dir = dir.join("stories");
    fs::create_dir_all(&stories_dir)
        .map_err(|error| format!("Could not create the stories folder: {error}"))?;
    let mut hashes = HashMap::new();
    for story in &library.stories {
        let text = serde_json::to_string_pretty(story)
            .map_err(|error| format!("Could not prepare “{}”: {error}", story.title))?;
        let path = story_path(&stories_dir, &story.key);
        write_atomic(&path, &text)
            .map_err(|error| format!("Could not write “{}”: {error}", story.title))?;
        hashes.insert(story.key.clone(), content_hash(&text));
    }
    let mut config = Config::from_library(library);
    if !config.settings.api_key.trim().is_empty() {
        let sealed = secrets::seal(dir, &config.settings.api_key)
            .map_err(|error| format!("The API key could not be sealed: {error}"))?;
        config.settings.api_key_sealed = sealed;
    }
    let text = serde_json::to_string_pretty(&config)
        .map_err(|error| format!("Could not prepare config.json: {error}"))?;
    // Config is the completion marker, so it is written last.
    write_atomic(&dir.join("config.json"), &text)
        .map_err(|error| format!("Could not write config.json: {error}"))?;
    Ok((content_hash(&text), hashes))
}

pub(crate) fn write_atomic(path: &Path, text: &str) -> Result<(), String> {
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, text).map_err(|error| format!("Could not write {}: {error}", tmp.display()))?;
    if let Err(error) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(format!("Could not replace {}: {error}", path.display()));
    }
    Ok(())
}

fn backup_file(path: &Path, kind: &str, _report: &mut LoadReport) -> Option<PathBuf> {
    let stamp = timestamp();
    let name = format!(
        "{}.{kind}-{stamp}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("file")
    );
    let backup = path.with_file_name(name);
    fs::rename(path, &backup).ok().map(|_| backup)
}

fn rename_migrated(path: &Path) -> Result<PathBuf, String> {
    let stamp = timestamp();
    let name = format!(
        "{}.migrated-{stamp}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("library.json")
    );
    let backup = path.with_file_name(name);
    fs::rename(path, &backup)
        .map_err(|error| format!("Could not rename {}: {error}", path.display()))?;
    Ok(backup)
}

fn timestamp() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}
