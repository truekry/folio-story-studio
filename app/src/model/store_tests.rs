//! Tests for config.json, one file per book, and the old-library migration.

use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(1);

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "folio-store-{}-{}-{}",
        std::process::id(),
        name,
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("temporary data folder");
    dir
}

fn legacy_text(library: &Library) -> String {
    serde_json::to_string_pretty(library).expect("legacy library")
}

fn migrated(name: &str) -> (PathBuf, Library, Store) {
    let dir = temp_dir(name);
    fs::write(dir.join("library.json"), legacy_text(&Library::sample())).expect("legacy file");
    let (library, store, report) = Store::open(&dir);
    assert!(report.migrated, "the old library must be converted");
    (dir, library, store)
}

#[test]
fn a_first_start_is_empty_and_writes_config_without_sample_books() {
    let dir = temp_dir("first-start");
    let (library, mut store, report) = Store::open(&dir);
    assert!(library.stories.is_empty(), "no sample books on first start");
    assert!(!report.migrated);
    store.save(&library).expect("save the empty library");
    assert!(dir.join("config.json").exists());
    assert!(dir.join("stories").is_dir());
    assert_eq!(
        fs::read_dir(dir.join("stories"))
            .expect("stories folder")
            .count(),
        0,
        "an empty library writes no book files"
    );

    let (again, _, _) = Store::open(&dir);
    assert!(again.stories.is_empty(), "the empty library stays empty");
}

#[test]
fn an_old_library_is_migrated_once_and_kept_as_backup() {
    let (dir, library, mut store) = migrated("migrate");
    assert_eq!(library.stories.len(), 2);
    assert!(library.stories.iter().all(|story| is_story_key(&story.key)));
    assert!(library.stories[0].chapters.iter().all(|chapter| chapter
        .key
        .starts_with(&format!("{}-", library.stories[0].key))));
    assert!(dir.join("config.json").exists());
    for story in &library.stories {
        assert!(dir
            .join("stories")
            .join(format!("{}.json", story.key))
            .exists());
    }
    assert!(!dir.join("library.json").exists());
    let backups: Vec<String> = fs::read_dir(&dir)
        .expect("data folder")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("library.json.migrated-"))
        .collect();
    assert_eq!(backups.len(), 1, "the old file is kept once");

    store.save(&library).expect("save migrated data");
    let (again, _, report) = Store::open(&dir);
    assert!(!report.migrated, "the new format is not migrated again");
    assert_eq!(again.stories.len(), 2);
    assert_eq!(again.stories[0].title, "The Lantern Coast");
}

#[test]
fn saved_files_use_keys_instead_of_numeric_ids() {
    let (dir, library, _) = migrated("keys-only");
    let story = &library.stories[0];
    let story_text = fs::read_to_string(dir.join("stories").join(format!("{}.json", story.key)))
        .expect("story file");
    assert!(story_text.contains("\"key\""), "{story_text}");
    assert!(
        !story_text.contains("\"id\""),
        "numeric id leaked: {story_text}"
    );
    let config = fs::read_to_string(dir.join("config.json")).expect("config");
    assert!(!config.contains("\"next_id\""), "{config}");
    assert!(!config.contains("\"active_story_id\""), "{config}");
    assert!(config.contains("\"active_story_key\""), "{config}");
}

#[test]
fn an_empty_old_library_stays_empty() {
    let dir = temp_dir("empty-legacy");
    fs::write(dir.join("library.json"), legacy_text(&Library::empty())).expect("empty legacy");
    let (library, _, report) = Store::open(&dir);
    assert!(report.migrated);
    assert!(library.stories.is_empty(), "no sample books are added");
}

#[test]
fn a_broken_old_library_is_backed_up_and_does_not_become_sample_books() {
    let dir = temp_dir("broken-legacy");
    fs::write(dir.join("library.json"), "{not json").expect("broken legacy");
    let (library, _, report) = Store::open(&dir);
    assert!(library.stories.is_empty());
    assert!(!report.warnings.is_empty());
    let backups: Vec<String> = fs::read_dir(&dir)
        .expect("data folder")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("library.json.broken-"))
        .collect();
    assert_eq!(backups.len(), 1);
}

#[test]
fn saving_writes_only_the_changed_book() {
    let (dir, mut library, mut store) = migrated("changed");
    let first = dir
        .join("stories")
        .join(format!("{}.json", library.stories[0].key));
    let second = dir
        .join("stories")
        .join(format!("{}.json", library.stories[1].key));
    let second_before = fs::read(&second).expect("second book");
    let first_before = fs::read(&first).expect("first book");

    library.stories[0].chapters[0]
        .body
        .push_str("\n\nA new paragraph.");
    store.save(&library).expect("save changed book");
    assert_ne!(
        fs::read(&first).expect("first book"),
        first_before,
        "the changed book was written"
    );
    assert_eq!(
        fs::read(&second).expect("second book"),
        second_before,
        "the untouched book was not rewritten"
    );
}

#[test]
fn a_broken_book_file_is_kept_aside_and_the_others_still_load() {
    let (dir, library, _) = migrated("broken-book");
    let broken_key = library.stories[1].key.clone();
    let broken_name = format!("{broken_key}.json");
    fs::write(dir.join("stories").join(&broken_name), "{broken").expect("broken book");
    let (loaded, mut store, report) = Store::open(&dir);
    assert_eq!(loaded.stories.len(), 1, "the healthy book still loads");
    assert!(!report.warnings.is_empty());
    let backups: Vec<String> = fs::read_dir(dir.join("stories"))
        .expect("stories folder")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with(&format!("{broken_name}.broken-")))
        .collect();
    assert_eq!(backups.len(), 1, "the broken file is preserved");

    // A save must not invent the missing book.
    store.save(&loaded).expect("save healthy library");
    assert!(!dir.join("stories").join(&broken_name).exists());
}

#[test]
fn the_active_book_key_survives_a_restart() {
    let dir = temp_dir("active-key");
    let mut library = Library::sample();
    library.active_story = 1;
    let expected = library.stories[1].key.clone();
    fs::write(dir.join("library.json"), legacy_text(&library)).expect("legacy file");
    let (loaded, mut store, _) = Store::open(&dir);
    assert_eq!(
        loaded.stories[loaded.active_story].title,
        "The Orchard at Night"
    );
    assert_eq!(loaded.stories[loaded.active_story].key, expected);
    store.save(&loaded).expect("save");
    let (again, _, _) = Store::open(&dir);
    assert_eq!(again.stories[again.active_story].key, expected);
}

#[test]
fn the_chosen_look_survives_a_restart() {
    let dir = temp_dir("look");
    let mut library = Library::sample();
    library.look = crate::theme::Look::Corpo;
    fs::write(dir.join("library.json"), legacy_text(&library)).expect("legacy file");
    let (loaded, mut store, _) = Store::open(&dir);
    assert_eq!(loaded.look, crate::theme::Look::Corpo);
    store.save(&loaded).expect("save");
    let (again, _, _) = Store::open(&dir);
    assert_eq!(again.look, crate::theme::Look::Corpo);
}

#[test]
fn deleting_a_book_removes_its_file_and_undo_writes_it_back() {
    let (dir, mut library, mut store) = migrated("delete");
    let key = library.stories[0].key.clone();
    let file = dir.join("stories").join(format!("{key}.json"));
    let removed = library.delete_story(&key).expect("delete story");
    store.save(&library).expect("save after delete");
    assert!(!file.exists());

    library.insert_story(0, removed);
    store.save(&library).expect("save after undo");
    assert!(file.exists());
    let (again, _, _) = Store::open(&dir);
    assert_eq!(again.stories.len(), 2);
}

#[test]
fn books_without_a_config_still_load_and_get_one_on_save() {
    let dir = temp_dir("stories-only");
    fs::create_dir_all(dir.join("stories")).expect("stories folder");
    fs::write(
        dir.join("stories").join("7.json"),
        serde_json::to_string_pretty(&Library::sample().stories[0]).expect("story"),
    )
    .expect("story file");
    let (library, mut store, report) = Store::open(&dir);
    assert_eq!(library.stories.len(), 1);
    assert!(!report.warnings.is_empty());
    store.save(&library).expect("save");
    assert!(dir.join("config.json").exists());
}

#[test]
fn a_broken_config_keeps_story_files_and_uses_defaults() {
    let (dir, library, _) = migrated("broken-config");
    fs::write(dir.join("config.json"), "{not json").expect("broken config");
    let (loaded, mut store, report) = Store::open(&dir);
    assert_eq!(loaded.stories.len(), library.stories.len());
    assert!(!report.warnings.is_empty());
    assert!(fs::read_dir(&dir)
        .expect("data folder")
        .flatten()
        .any(|entry| entry
            .file_name()
            .to_string_lossy()
            .starts_with("config.json.broken-")));
    store.save(&loaded).expect("write a fresh config");
    assert!(dir.join("config.json").exists());
}

/// An editor on Windows may save JSON with a byte-order mark. The file must
/// load unchanged; Folio must not treat it as broken or rename it.
#[test]
fn a_config_with_a_utf8_bom_loads_without_a_backup() {
    let (dir, library, _) = migrated("bom-config");
    let text = fs::read_to_string(dir.join("config.json")).expect("config");
    fs::write(dir.join("config.json"), format!("\u{feff}{text}")).expect("write a BOM config");

    let (loaded, _store, report) = Store::open(&dir);
    assert!(
        !report
            .warnings
            .iter()
            .any(|warning| warning.contains("config.json")),
        "{:?}",
        report.warnings
    );
    assert_eq!(loaded.stories.len(), library.stories.len());
    assert!(
        !fs::read_dir(&dir)
            .expect("data folder")
            .flatten()
            .any(|entry| entry.file_name().to_string_lossy().contains(".broken-")),
        "a BOM is not a defect"
    );

    // The books are read the same way.
    let story_path = fs::read_dir(dir.join("stories"))
        .expect("stories")
        .flatten()
        .map(|entry| entry.path())
        .find(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
        .expect("a book file");
    let story_text = fs::read_to_string(&story_path).expect("book");
    fs::write(&story_path, format!("\u{feff}{story_text}")).expect("write a BOM book");
    let (loaded, _, report) = Store::open(&dir);
    assert_eq!(loaded.stories.len(), library.stories.len());
    assert!(
        !report
            .warnings
            .iter()
            .any(|warning| warning.contains("book file")),
        "{:?}",
        report.warnings
    );
}

#[test]
fn an_atomic_save_leaves_no_temporary_file() {
    let (dir, library, mut store) = migrated("atomic");
    store.save(&library).expect("save");
    let leftovers: Vec<String> = fs::read_dir(&dir)
        .expect("data folder")
        .flatten()
        .chain(
            fs::read_dir(dir.join("stories"))
                .expect("stories")
                .flatten(),
        )
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".tmp"))
        .collect();
    assert!(leftovers.is_empty(), "temporary files left: {leftovers:?}");
}
