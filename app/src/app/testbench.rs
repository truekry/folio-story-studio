//! Generates the local test bench: one book with three ~2,000-word chapters
//! and a small lorebook, written into `../testbench` as a separate `FOLIO_DATA_DIR`.
//!
//! The source of truth is the three text files under `tests/`; this test only
//! turns them into the library JSON the app reads. It is ignored on purpose:
//! run it by hand when the bench should be rebuilt.

use super::*;

#[test]
#[ignore = "reads the local test bench folder (../testbench)"]
fn the_test_bench_loads() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest.join("../testbench");
    if !root.exists() {
        eprintln!("skipped: the local test bench folder ../testbench is missing");
        return;
    }
    let (library, _, report) = crate::model::Store::open(&root);
    assert!(
        report.warnings.is_empty(),
        "the bench should load cleanly: {:?}",
        report.warnings
    );
    assert_eq!(library.stories.len(), 1);
    let story = &library.stories[0];
    assert_eq!(story.key, "BEN1");
    assert_eq!(story.title, "The Lantern Coast");
    assert_eq!(story.chapters.len(), 3);
    for (index, chapter) in story.chapters.iter().enumerate() {
        assert_eq!(chapter.key, format!("BEN1-{:03}", index + 1));
        assert!(
            chapter.words() > 1500,
            "chapter {} has only {} words",
            index + 1,
            chapter.words()
        );
    }
    assert_eq!(story.lore.len(), 6);
    assert!(story.lore.iter().any(|entry| entry.name == "Mira Vale"));
}

#[test]
#[ignore = "writes the local test bench folder (../testbench)"]
fn write_the_test_bench() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest.join("../testbench");
    let chapters = [
        ("The Survey Office", "bench_chapter_1.txt"),
        ("The Salt Room", "assistant_chapter.txt"),
        ("The Ninth Tide", "bench_chapter_3.txt"),
    ];
    if let Some((_, file)) = chapters
        .iter()
        .find(|(_, file)| !manifest.join("tests").join(file).is_file())
    {
        eprintln!("skipped: local fixture tests/{file} is not in the repo");
        return;
    }
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("test bench folder");

    let mut library = Library::sample();
    let template = library.stories[0].chapters[0].clone();
    let mut story = library.stories[0].clone();
    story.key = "BEN1".to_string();
    story.title = "The Lantern Coast".to_string();
    story.premise = "A cartographer returns to the drowned harbour of her childhood and finds her mother's last map pointing to an island that appears on no tide.".to_string();
    story.chapters.clear();
    story.lore.clear();
    story.chat.clear();
    story.open_chapter = 1;
    story.next_chapter = 4;
    story.next_lore = 1;

    for (index, (title, file)) in chapters.iter().enumerate() {
        let body = std::fs::read_to_string(manifest.join("tests").join(file))
            .unwrap_or_else(|error| panic!("read {file}: {error}"));
        let body = strip_leading_title(&body, title);
        let mut chapter = template.clone();
        chapter.key = format!("BEN1-{:03}", index + 1);
        chapter.title = title.to_string();
        chapter.body = body;
        chapter.summary.clear();
        chapter.summary_source.clear();
        chapter.summary_body_len = 0;
        chapter.summary_body_hash = 0;
        chapter.chat.clear();
        chapter.chat_summary.clear();
        chapter.steering.clear();
        chapter.draft = None;
        story.chapters.push(chapter);
    }

    let lore = [
        (
            "Mira Vale",
            "Character",
            "the cartographer, Mira",
            "Mira, cartographer",
            "The point-of-view character. A cartographer in her thirties, tall and angular, with dark hair cut short, grey-green eyes and ink-stained hands. She has a nervous habit of tapping her thumb against her knuckles.",
        ),
        (
            "Wren",
            "Character",
            "the lantern keeper",
            "Wren, the boy",
            "A boy of about twelve who keeps the house, lights the lamp and tends the marigolds. He knows more about the Vale family than he says.",
        ),
        (
            "Brinehouse",
            "Place",
            "the harbour, the town",
            "Brinehouse, harbour",
            "A small northern harbour town built into a wedge of black rock. Only four boats remain of the nine Mira remembers.",
        ),
        (
            "The Ninth Tide",
            "Lore",
            "the low tide, the emptying",
            "ninth tide, low tide",
            "A tide that arrives once in a generation and pulls the whole bay dry, exposing the seabed for one night.",
        ),
        (
            "Ada Vale",
            "Character",
            "Mira's mother, E. Vale",
            "Ada, E. Vale",
            "Mira's mother, a mapmaker who disappeared six years ago. Her notes are careful and her last map shows an island that should not exist.",
        ),
        (
            "The Key",
            "Item",
            "the green key, the salt key",
            "key, salt key",
            "A large, salt-crusted key, too large for any door in the house. It opens the salt room behind the pantry wall.",
        ),
    ];
    for (number, (name, category, aliases, triggers, description)) in lore.iter().enumerate() {
        story.lore.push(crate::model::LoreEntry {
            key: crate::model::lore_key("BEN1", (number + 1) as u32),
            name: name.to_string(),
            category: category.to_string(),
            aliases: aliases.to_string(),
            triggers: triggers.to_string(),
            description: description.to_string(),
            pinned: false,
            invented: false,
        });
    }
    story.next_lore = lore.len() as u32 + 1;

    library.stories = vec![story];
    library.active_story = 0;
    library.settings = crate::model::AiSettings::default();

    let (_, mut store, _) = crate::model::Store::open(&root);
    store.save(&library).expect("write the test bench");
    println!("test bench written to {}", root.display());
}

/// The fixtures start with their chapter title for readability; the app shows
/// the title separately. Strip that first line so it is not shown twice.
fn strip_leading_title(body: &str, title: &str) -> String {
    if let Some((first, rest)) = body.split_once('\n') {
        if first.trim().eq_ignore_ascii_case(title.trim()) {
            return rest.trim_start_matches(['\r', '\n']).to_string();
        }
    }
    body.to_string()
}
