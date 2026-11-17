//! Unit tests for the story library.

use super::*;

#[test]
fn sample_library_round_trips() {
    let library = Library::sample();
    let json = serde_json::to_string(&library).expect("serialize");
    let back: Library = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back.stories.len(), library.stories.len());
    assert_eq!(back.stories[0].title, "The Lantern Coast");
    assert!(!back.stories[0].chapters[0].summary.is_empty());
}

#[test]
fn the_writing_font_survives_a_round_trip_and_old_libraries_get_the_default() {
    let mut library = Library::sample();
    library.writing_font = "C:/Windows/Fonts/georgia.ttf".to_string();
    library.writing_size = 20.5;
    let json = serde_json::to_string(&library).expect("serialize");
    let back: Library = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back.writing_font, "C:/Windows/Fonts/georgia.ttf");
    assert_eq!(back.writing_size, 20.5);

    // Libraries from before the type bar have neither field.
    let old: Library =
        serde_json::from_str(r#"{"stories":[],"settings":{}}"#).expect("old library");
    assert!(
        old.writing_font.is_empty(),
        "empty means the system default"
    );
    assert_eq!(old.writing_size, DEFAULT_WRITING_SIZE);
}

#[test]
fn a_draft_survives_a_round_trip_and_old_libraries_still_load() {
    let mut library = Library::sample();
    let body = library.stories[0].chapters[0].body.clone();
    let draft = Draft {
        action: "continue".to_string(),
        text: "The wind turned.".to_string(),
        range: None,
        source: None,
        base_len: body.chars().count(),
        base_hash: content_hash(&body),
        stopped: true,
    };
    library.stories[0].chapters[0].draft = Some(draft.clone());

    let json = serde_json::to_string(&library).expect("serialize");
    let back: Library = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back.stories[0].chapters[0].draft, Some(draft));

    // A chapter without a draft writes no `draft` key, which is exactly the
    // shape of every library saved before drafts existed.
    library.stories[0].chapters[0].draft = None;
    let old = serde_json::to_string(&library).expect("serialize");
    assert!(!old.contains("\"draft\""));
    let back: Library = serde_json::from_str(&old).expect("old library");
    assert!(back.stories[0].chapters[0].draft.is_none());
}

#[test]
fn steering_survives_a_round_trip_and_old_libraries_still_load() {
    let mut library = Library::sample();
    library.stories[0].chapters[0].steering = "Bring the storm in from the west.".to_string();

    let json = serde_json::to_string(&library).expect("serialize");
    let back: Library = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(
        back.stories[0].chapters[0].steering,
        "Bring the storm in from the west."
    );

    // An empty note writes no `steering` key, which is exactly the shape of
    // every library saved before steering existed.
    library.stories[0].chapters[0].steering.clear();
    let old = serde_json::to_string(&library).expect("serialize");
    assert!(!old.contains("\"steering\""));
    let back: Library = serde_json::from_str(&old).expect("old library");
    assert!(back.stories[0].chapters[0].steering.is_empty());
}

#[test]
fn a_chapter_text_file_names_its_book_and_chapter() {
    let library = Library::sample();
    let story = &library.stories[0];
    let text = chapter_text(story, 1);
    assert!(text.starts_with("The Lantern Coast\r\nChapter 2 of 3 — A House With No Door\r\n\r\n"));
    assert!(text.contains("Her mother's house had no door."));
    // Windows line endings, so old editors show real paragraphs.
    assert!(!text.replace("\r\n", "").contains('\n'));
}

#[test]
fn file_names_are_safe_for_windows() {
    assert_eq!(
        file_name_part("The Lantern Coast", "Story"),
        "The Lantern Coast"
    );
    assert_eq!(file_name_part("A/B: C?", "Story"), "A B C");
    assert_eq!(file_name_part("  ...  ", "Story"), "Story");
    assert_eq!(file_name_part("Grüße & Küsse", "Story"), "Grüße & Küsse");
    // Long titles are cut without leaving a trailing dot or space.
    let long = "W".repeat(120);
    assert!(file_name_part(&long, "Story").chars().count() <= 70);
    let story = &Library::sample().stories[0];
    assert_eq!(
        chapter_file_name("The Lantern Coast", story, 0),
        "The Lantern Coast_01_The Tide Map.txt"
    );
    assert_eq!(
        chapter_file_name("Lantern - Draft", story, 2),
        "Lantern - Draft_03_Salt in the Wiring.txt"
    );
    assert_eq!(pdf_file_name("The Lantern Coast"), "The Lantern Coast.pdf");
    // An extension the author typed is not doubled.
    assert_eq!(pdf_file_name("Draft.pdf"), "Draft.pdf");
    assert_eq!(
        chapter_file_name("Draft.txt", story, 0),
        "Draft_01_The Tide Map.txt"
    );
    // A blank name falls back, so the export never ends up as ".pdf".
    assert_eq!(pdf_file_name("   "), "Story.pdf");
    assert_eq!(
        chapter_file_name("", story, 1),
        "Story_02_A House With No Door.txt"
    );
}

#[test]
fn a_typed_name_reaches_the_written_files() {
    let story = Library::sample().stories[0].clone();
    let dir = std::env::temp_dir().join(format!("folio-export-name-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);

    let pdf = export_pdf(&story, "Moonlight Edition", &dir).expect("pdf export");
    assert_eq!(pdf.file_name().unwrap(), "Moonlight Edition.pdf");

    let files = export_chapters(&story, "Moonlight Edition", &dir).expect("chapter export");
    let names: Vec<String> = files
        .iter()
        .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        names,
        vec![
            "Moonlight Edition_01_The Tide Map.txt",
            "Moonlight Edition_02_A House With No Door.txt",
            "Moonlight Edition_03_Salt in the Wiring.txt",
        ]
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn exporting_writes_the_files_where_asked() {
    let story = Library::sample().stories[0].clone();
    let dir = std::env::temp_dir().join(format!("folio-export-test-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);

    let pdf = export_pdf(&story, &story.title, &dir).expect("pdf export");
    assert_eq!(pdf.file_name().unwrap(), "The Lantern Coast.pdf");
    let bytes = fs::read(&pdf).expect("read pdf");
    assert!(bytes.starts_with(b"%PDF-1.4"));

    let files = export_chapters(&story, &story.title, &dir).expect("chapter export");
    assert_eq!(files.len(), story.chapters.len());
    for (index, path) in files.iter().enumerate() {
        assert!(path.exists());
        let text = fs::read_to_string(path).expect("read chapter");
        assert!(text.contains(&format!(
            "Chapter {} of {}",
            index + 1,
            story.chapters.len()
        )));
        assert!(text.contains(&crlf(story.chapters[index].body.trim())));
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_second_export_replaces_what_was_there() {
    let story = Library::sample().stories[0].clone();
    let dir = std::env::temp_dir().join(format!("folio-export-replace-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("folder");
    let target = pdf_target("Draft", &dir);
    fs::write(&target, b"an older file").expect("plant a file");

    export_pdf(&story, "Draft", &dir).expect("pdf export");
    let bytes = fs::read(&target).expect("read pdf");
    assert!(bytes.starts_with(b"%PDF-1.4"), "the old file was replaced");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn exporting_a_story_without_chapters_says_so() {
    let mut story = Library::sample().stories[0].clone();
    story.chapters.clear();
    let dir = std::env::temp_dir();
    assert!(export_chapters(&story, "Draft", &dir).is_err());
}

#[test]
fn stale_summary_is_detected() {
    let mut chapter = Library::sample().stories[0].chapters[0].clone();
    assert!(!chapter.summary_is_stale());
    chapter.body.push_str(&" more words".repeat(30));
    assert!(chapter.summary_is_stale());
}

#[test]
fn a_same_length_edit_still_marks_the_summary_stale() {
    let mut chapter = Library::sample().stories[0].chapters[0].clone();
    assert!(!chapter.summary_is_stale());
    chapter.body = chapter.body.replacen("sea", "sky", 1);
    assert!(chapter.summary_is_stale());
}

#[test]
fn an_old_summary_without_a_fingerprint_uses_the_length_check() {
    let mut chapter = Library::sample().stories[0].chapters[0].clone();
    chapter.summary_body_hash = 0;
    assert!(!chapter.summary_is_stale());
    chapter.body.push_str(&" more words".repeat(30));
    assert!(chapter.summary_is_stale());
}

#[test]
fn deleting_a_story_keeps_the_library_consistent() {
    let mut library = Library::sample();
    library.active_story = 1;
    let second = library.stories[1].key.clone();
    let first = library.stories[0].key.clone();
    let removed = library.delete_story(&second).expect("story removed");
    assert_eq!(removed.title, "The Orchard at Night");
    assert_eq!(library.stories.len(), 1);
    assert_eq!(library.active_story, 0);

    let last = library.delete_story(&first).expect("last story removed");
    assert_eq!(last.chapters.len(), 3);
    assert!(library.stories.is_empty());
    assert_eq!(library.active_story, 0);
    assert!(library.delete_story(&first).is_none());
}

#[test]
fn deleting_another_story_keeps_the_open_one() {
    let mut library = Library::sample();
    let mut third = library.stories[1].clone();
    third.key = "ZZZZ".to_string();
    third.title = "A Third Shelf".to_string();
    library.stories.push(third);
    library.active_story = 2;
    let open_key = library.stories[2].key.clone();

    // Delete the first story while the last one is open.
    library
        .delete_story(&library.stories[0].key.clone())
        .expect("story removed");
    assert_eq!(
        library.stories[library.active_story].key, open_key,
        "deleting an earlier story must not switch the open one"
    );
}

#[test]
fn deleting_another_chapter_keeps_the_open_one() {
    let mut story = Library::sample().stories[0].clone();
    story.open_chapter = 2;
    let open_chapter_key = story.chapters[2].key.clone();

    // Delete the first chapter while the third one is open.
    story
        .delete_chapter(&story.chapters[0].key.clone())
        .expect("chapter removed");
    assert_eq!(
        story.chapters[story.open_chapter].key, open_chapter_key,
        "deleting an earlier chapter must not switch the open one"
    );
}

#[test]
fn deleting_a_chapter_keeps_the_rest_of_the_story() {
    let mut story = Library::sample().stories[0].clone();
    story.open_chapter = 1;
    let removed = story
        .delete_chapter(&story.chapters[1].key.clone())
        .expect("chapter removed");
    assert_eq!(removed.title, "A House With No Door");
    assert_eq!(story.chapters.len(), 2);
    assert_eq!(story.open_chapter, 1);
    assert!(story.chapter(&story.chapters[0].key).is_some());
    assert!(story.chapter(&removed.key).is_none());
    assert!(story.chapter(&story.chapters[1].key).is_some());

    let first = story.chapters[0].key.clone();
    let last = story.chapters[1].key.clone();
    story.delete_chapter(&first);
    story.delete_chapter(&last);
    assert!(story.chapters.is_empty());
    assert_eq!(story.open_chapter, 0);
}

#[test]
fn a_deleted_story_goes_back_where_it_was() {
    let mut library = Library::sample();
    let key = library.stories[1].key.clone();
    let removed = library.delete_story(&key).expect("story removed");
    let at = library.insert_story(1, removed);
    assert_eq!(at, 1);
    assert_eq!(library.stories.len(), 2);
    assert_eq!(library.stories[1].title, "The Orchard at Night");
    assert_eq!(library.stories[1].key, key);
    // Its chapters and lorebook came back too.
    assert_eq!(library.stories[1].chapters.len(), 1);
    assert!(!library.stories[1].lore.is_empty());
}

#[test]
fn a_deleted_chapter_goes_back_where_it_was() {
    let mut story = Library::sample().stories[0].clone();
    let before: Vec<String> = story.chapters.iter().map(|c| c.key.clone()).collect();
    let removed = story
        .delete_chapter(&story.chapters[1].key.clone())
        .expect("chapter removed");
    let at = story.insert_chapter(1, removed);
    assert_eq!(at, 1);
    let after: Vec<String> = story.chapters.iter().map(|c| c.key.clone()).collect();
    assert_eq!(before, after);
    assert_eq!(story.chapters[1].title, "A House With No Door");
}

#[test]
fn a_deleted_lore_entry_goes_back_where_it_was() {
    let mut story = Library::sample().stories[0].clone();
    let before: Vec<String> = story.lore.iter().map(|e| e.key.clone()).collect();
    let key = story.lore[2].key.clone();
    let removed = story.delete_lore(&key).expect("lore removed");
    assert_eq!(removed.name, "Brinehouse");
    assert!(story.lore.iter().all(|e| e.key != key));
    let at = story.insert_lore(2, removed);
    assert_eq!(at, 2);
    let after: Vec<String> = story.lore.iter().map(|e| e.key.clone()).collect();
    assert_eq!(before, after);
}

#[test]
fn the_session_trash_never_reaches_the_saved_library() {
    // Deleted items are kept by the open window, not the library file, so a
    // restart really forgets them.
    let library = Library::sample();
    let text = serde_json::to_string(&library).expect("serialize");
    assert!(!text.contains("trash"));
    assert!(!text.contains("deleted_at"));
}

#[test]
fn an_old_library_shows_the_summary_and_the_choice_is_remembered() {
    // Libraries written before the toggle existed have no such field.
    let old = r#"{"stories": [], "active_story": 0, "next_id": 1}"#;
    let old: Library = serde_json::from_str(old).expect("old library");
    assert!(old.show_summary);

    let mut library = Library::sample();
    library.show_summary = false;
    let json = serde_json::to_string(&library).expect("serialize");
    let back: Library = serde_json::from_str(&json).expect("deserialize");
    assert!(!back.show_summary);
    // Hiding the card never touches the summary itself.
    assert!(!back.stories[0].chapters[0].summary.is_empty());
}

#[test]
fn nothing_saves_an_automatic_summary_switch() {
    // A summary is written only when the author presses Summarize, so the
    // library no longer carries a setting that could turn one on by itself.
    let json = serde_json::to_string(&Library::sample()).expect("serialize");
    assert!(!json.contains("auto_summary"));

    // Libraries from before still load: the stray keys are simply ignored.
    let old = r#"{
        "stories": [],
        "active_story": 0,
        "next_id": 1,
        "settings": {"auto_summary": true, "include_summaries": false}
    }"#;
    let old: Library = serde_json::from_str(old).expect("old library");
    assert_eq!(old.settings.summary_window, DEFAULT_SUMMARY_WINDOW);
}

#[test]
fn a_server_on_the_home_network_needs_no_key() {
    let mut settings = AiSettings::default();
    // The shipped address is OpenAI without a key: the demo writer stands in.
    assert!(settings.is_demo());

    settings.base_url = "192.168.10.20:5001/v1".to_string();
    assert!(
        !settings.is_demo(),
        "a LAN address must not fall back to demo"
    );

    settings.base_url = "http://workstation:5001/v1".to_string();
    assert!(!settings.is_demo(), "a bare machine name is on the network");

    settings.base_url = "http://[::1]:8080/v1".to_string();
    assert!(!settings.is_demo());

    settings.base_url = "https://api.openai.com/v1".to_string();
    assert!(
        settings.is_demo(),
        "a cloud service without a key stays demo"
    );

    settings.api_key = "sk-test".to_string();
    assert!(!settings.is_demo());

    settings.base_url = String::new();
    assert!(settings.is_demo(), "no address means no endpoint");

    assert!(is_private_address("10.0.0.5:8000/v1"));
    assert!(!is_private_address("https://openrouter.ai/api/v1"));
    assert!(!is_private_address("example.com"));
}

#[test]
fn the_summary_window_decides_what_travels_alone() {
    let story = Library::sample().stories[0].clone();
    assert_eq!(story.recent_start(2, 1), 1);
    assert_eq!(story.recent_start(1, 3), 0);

    // Window of one on the third chapter: only chapter two is sent as itself.
    let recent: Vec<&str> = story
        .recent_summaries(2, 1)
        .iter()
        .map(|c| c.title.as_str())
        .collect();
    assert_eq!(recent, vec!["A House With No Door"]);

    // Everything before it is condensed, and nothing from the window leaks in.
    let recap = story.story_so_far(2, 1);
    assert!(
        recap.starts_with("Mira returns to Brinehouse"),
        "recap was {recap:?}"
    );
    assert!(
        !recap.contains("Her mother's house"),
        "the window leaked into the recap"
    );

    // A window wider than the story leaves no recap at all.
    assert!(story.story_so_far(2, 9).is_empty());
}

#[test]
fn a_long_story_is_condensed_but_keeps_every_chapter() {
    let mut story = Library::sample().stories[0].clone();
    let template = story.chapters[0].clone();
    for round in 0..12 {
        let mut extra = template.clone();
        extra.key = format!("{}-{:03}", story.key, 100 + round);
        extra.summary = format!(
            "Chapter {round} of the voyage: {}",
            "the ship held its course past the reef and the crew said nothing. ".repeat(4)
        );
        story.chapters.push(extra);
    }
    let recap = story.story_so_far(story.chapters.len(), 0);
    assert!(
        recap.chars().count() < 1600,
        "recap was {} characters",
        recap.chars().count()
    );
    for round in 0..12 {
        assert!(
            recap.contains(&format!("Chapter {round} of the voyage")),
            "chapter {round} vanished from the recap"
        );
    }
    // The same chapters are only repeated one by one when they are recent.
    assert!(story.recent_summaries(story.chapters.len(), 0).is_empty());
}

#[test]
fn a_summary_run_covers_the_chapters_the_recap_needs() {
    let mut story = Library::sample().stories[0].clone();
    story.chapters[0].summary.clear();
    story.chapters[1].summary.clear();
    // The second chapter is still inside the window, but it travels as
    // itself, so a missing summary there has to be written too.
    let ids: Vec<String> = story
        .chapters_needing_summary(2, 1)
        .iter()
        .map(|c| c.key.clone())
        .collect();
    assert_eq!(
        ids,
        vec![story.chapters[0].key.clone(), story.chapters[1].key.clone()]
    );

    // Inside the window a stale summary is still usable; outside it feeds the
    // recap and is refreshed.
    let mut second = story.chapters[1].clone();
    second.summary = "A summary from before the last edit.".to_string();
    second.summary_body_hash = 12345;
    story.chapters[1] = second;
    let ids: Vec<String> = story
        .chapters_needing_summary(2, 1)
        .iter()
        .map(|c| c.key.clone())
        .collect();
    assert_eq!(ids, vec![story.chapters[0].key.clone()]);

    let ids: Vec<String> = story
        .chapters_needing_summary(2, 0)
        .iter()
        .map(|c| c.key.clone())
        .collect();
    assert_eq!(
        ids,
        vec![story.chapters[0].key.clone(), story.chapters[1].key.clone()]
    );
}

#[test]
fn the_summary_window_is_remembered_and_clamped() {
    assert_eq!(AiSettings::default().summary_window, DEFAULT_SUMMARY_WINDOW);

    let mut library = Library::sample();
    library.settings.summary_window = 7;
    let json = serde_json::to_string(&library).expect("serialize");
    let mut back: Library = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back.settings.summary_window, 7);
    assert_eq!(back.settings.recent_window(), 7);

    // A hand-edited file cannot ask for more than the options offer.
    back.settings.summary_window = 99;
    assert_eq!(back.settings.recent_window(), MAX_SUMMARY_WINDOW);
}

fn talk(role: &str, text: &str) -> ChatMessage {
    ChatMessage {
        role: role.to_string(),
        text: text.to_string(),
        demo: false,
    }
}

#[test]
fn every_chapter_keeps_its_own_conversation() {
    let mut library = Library::sample();
    library.stories[0].chapters[0]
        .chat
        .push(talk("user", "Question about the first chapter"));
    library.stories[0].chapters[1]
        .chat
        .push(talk("user", "Question about the second chapter"));

    let json = serde_json::to_string(&library).expect("serialize");
    let back: Library = serde_json::from_str(&json).expect("deserialize");
    let story = &back.stories[0];
    assert_eq!(
        story.chapters[0].chat[0].text,
        "Question about the first chapter"
    );
    assert_eq!(
        story.chapters[1].chat[0].text,
        "Question about the second chapter"
    );
    assert!(story.chapters[2].chat.is_empty());
    // The story itself no longer carries a shared conversation.
    assert!(story.chat.is_empty());
}

#[test]
fn an_old_story_conversation_moves_to_the_open_chapter() {
    let old = r#"{
        "stories": [{
            "id": 1,
            "title": "Old Book",
            "open_chapter": 1,
            "chapters": [
                {"id": 10, "title": "One", "body": "a", "summary": ""},
                {"id": 11, "title": "Two", "body": "b", "summary": ""}
            ],
            "chat": [
                {"role": "user", "text": "Old question"},
                {"role": "assistant", "text": "Old answer"}
            ]
        }],
        "active_story": 0,
        "next_id": 20
    }"#;
    let mut library: Library = serde_json::from_str(old).expect("old library");
    library.adopt_story_chats();
    let story = &library.stories[0];
    assert!(story.chat.is_empty());
    assert!(story.chapters[0].chat.is_empty());
    assert_eq!(story.chapters[1].chat.len(), 2);
    assert_eq!(story.chapters[1].chat[0].text, "Old question");
}

#[test]
fn a_long_conversation_folds_its_oldest_turns_into_memory() {
    let mut chapter = Library::sample().stories[0].chapters[0].clone();
    for round in 0..12 {
        chapter.chat.push(talk(
            "user",
            &format!("Question {round}: {}", "detail ".repeat(40)),
        ));
        chapter.chat.push(talk(
            "assistant",
            &format!("Answer {round}: {}", "prose ".repeat(40)),
        ));
    }
    let before = chapter.conversation_tokens();
    assert!(chapter.compact_conversation(600));
    assert!(
        chapter.conversation_tokens() <= 600,
        "still {} tokens",
        chapter.conversation_tokens()
    );
    assert!(chapter.conversation_tokens() < before);

    // The newest exchange stays word for word.
    assert!(chapter.chat.last().unwrap().text.contains("Answer 11"));
    assert!(chapter.chat.len() >= 2);
    // The oldest turns left a memory behind, and it stays a paragraph.
    assert!(
        chapter.chat_summary.contains("Author:"),
        "memory was {:?}",
        chapter.chat_summary
    );
    assert!(chapter.chat_summary.contains("Question"));
    assert!(chapter.chat_summary.chars().count() <= 1201);
}

#[test]
fn compaction_never_drops_the_last_exchange() {
    let mut chapter = Library::sample().stories[0].chapters[0].clone();
    chapter.chat.push(talk("user", "One question"));
    chapter.chat.push(talk("assistant", "One answer"));
    assert!(!chapter.compact_conversation(100_000));
    assert_eq!(chapter.chat.len(), 2);
    // Even a tiny budget keeps the last question and answer.
    assert!(!chapter.compact_conversation(1));
    assert_eq!(chapter.chat.len(), 2);
}

#[test]
fn a_trigger_word_pulls_an_entry_into_the_context() {
    let story = Library::sample().stories[0].clone();
    let saltglass = story
        .lore
        .iter()
        .find(|e| e.name == "Saltglass")
        .expect("Saltglass");
    assert_eq!(
        saltglass.trigger_list(),
        vec!["the green shards", "the lantern light"]
    );

    // The chapter never says "Saltglass", only the words the author chose.
    let text = "A line of the green shards crossed the mud.";
    assert_eq!(
        saltglass
            .matched_term(&text.to_ascii_lowercase())
            .as_deref(),
        Some("the green shards")
    );
    assert!(story
        .lore_for_text(text)
        .iter()
        .any(|entry| entry.name == "Saltglass"));
}

#[test]
fn trigger_words_keep_quiet_when_they_are_not_written() {
    let story = Library::sample().stories[0].clone();
    let saltglass = story
        .lore
        .iter()
        .find(|e| e.name == "Saltglass")
        .expect("Saltglass");
    let text = "The bell rang twice in the empty street.";
    assert!(saltglass.matched_term(&text.to_ascii_lowercase()).is_none());
    assert!(!story
        .lore_for_text(text)
        .iter()
        .any(|entry| entry.name == "Saltglass"));
}

#[test]
fn blank_and_one_letter_triggers_are_ignored() {
    let entry = LoreEntry {
        key: String::new(),
        name: "The Bell".to_string(),
        category: "Item".to_string(),
        aliases: String::new(),
        triggers: " , a ,, the bell ,".to_string(),
        description: String::new(),
        pinned: false,
    };
    assert_eq!(entry.trigger_list(), vec!["the bell"]);
    assert!(entry.matches("they rang the bell"));
    assert!(!entry.matches("a"));
}

#[test]
fn lore_matching_ignores_case_for_umlauts() {
    let entry = LoreEntry {
        key: String::new(),
        name: "Über dem Hafen".to_string(),
        category: "Place".to_string(),
        aliases: "Ältere Boote".to_string(),
        triggers: "Öffnung".to_string(),
        description: String::new(),
        pinned: false,
    };
    // Matching is done against a lowercased page, as the context builder does.
    let page = "Zwei ältere Boote lagen still; die Öffnung war leer.".to_lowercase();
    assert_eq!(
        entry.matched_term(&page).as_deref(),
        Some("Ältere Boote"),
        "an umlaut alias must match its lowercase spelling"
    );
    let page = "über dem hafen hing nebel.".to_lowercase();
    assert_eq!(entry.matched_term(&page).as_deref(), Some("Über dem Hafen"));
    let page = "die öffnung kam mit der flut.".to_lowercase();
    assert_eq!(entry.matched_term(&page).as_deref(), Some("Öffnung"));
}

#[test]
fn triggers_round_trip_and_old_entries_still_load() {
    let library = Library::sample();
    let json = serde_json::to_string(&library).expect("serialize");
    let back: Library = serde_json::from_str(&json).expect("deserialize");
    let ada = back.stories[0]
        .lore
        .iter()
        .find(|e| e.name == "Ada Vale")
        .expect("Ada Vale");
    assert_eq!(ada.triggers, "her mother, the mapmaker");
    assert_eq!(
        ada.matched_term("mira read her mother's notes").as_deref(),
        Some("her mother")
    );

    // A library written before trigger words existed has no such field.
    let old = r#"{
        "stories": [{
            "id": 1,
            "title": "Old",
            "lore": [{
                "id": 7,
                "name": "Ada",
                "category": "Character",
                "aliases": "",
                "description": "x",
                "pinned": false
            }]
        }],
        "active_story": 0,
        "next_id": 8
    }"#;
    let old: Library = serde_json::from_str(old).expect("old library");
    assert_eq!(old.stories[0].lore[0].triggers, "");
    assert!(!old.stories[0].lore[0].matches("her mother"));
}

#[test]
fn an_old_library_without_prompts_uses_the_built_in_ones() {
    // Libraries written before the prompt editor have no such block.
    let old = r#"{
        "stories": [],
        "active_story": 0,
        "next_id": 1,
        "settings": {
            "base_url": "192.168.10.20:5001/v1",
            "model": "local-model",
            "max_tokens": 900
        }
    }"#;
    let old: Library = serde_json::from_str(old).expect("old library");
    assert_eq!(old.settings.max_tokens, 900);
    let prompts = &old.settings.prompts;
    assert!(
        prompts.summary.text.is_empty(),
        "no own text means built-in"
    );
    assert_eq!(prompts.summary.words, None, "no target means built-in");
    assert_eq!(prompts.continuing.words, None);
    // A function added later has no own text in an older file.
    assert!(prompts.elaborate.text.is_empty());
    assert_eq!(prompts.elaborate.words, None);
    // The built-in defaults still provide a length for the summary.
    assert_eq!(
        crate::ai::PromptKind::Summary.words(prompts),
        crate::ai::PromptKind::Summary.default_words()
    );
}

#[test]
fn a_written_prompt_survives_a_round_trip() {
    let mut library = Library::sample();
    library.settings.prompts.summary.text = "Keep it to the facts, in German.".to_string();
    library.settings.prompts.summary.words = Some(70);
    library.settings.prompts.continuing.text = "Stay in Mira's head.".to_string();
    library.settings.prompts.continuing.words = Some(0);
    library.settings.prompts.brainstorm.words = Some(140);
    library.settings.prompts.elaborate.text = "Let the weather do some of the work.".to_string();

    let json = serde_json::to_string(&library).expect("serialize");
    // The field is named "continue" in the file, as the rest of the app
    // speaks of it, though Rust has to call it something else.
    assert!(
        json.contains("\"continue\""),
        "the key must stay `continue`"
    );
    let back: Library = serde_json::from_str(&json).expect("deserialize");
    let prompts = &back.settings.prompts;
    assert_eq!(prompts.summary.text, "Keep it to the facts, in German.");
    assert_eq!(prompts.summary.words, Some(70));
    assert_eq!(prompts.continuing.text, "Stay in Mira's head.");
    // Zero means "no length wish" and must not be mistaken for "default".
    assert_eq!(prompts.continuing.words, Some(0));
    assert_eq!(prompts.brainstorm.words, Some(140));
    assert_eq!(
        prompts.elaborate.text,
        "Let the weather do some of the work."
    );
    assert_eq!(prompts.rewrite.words, None);
    assert!(prompts.rewrite.text.is_empty());
}

#[test]
fn a_prompt_for_the_removed_describe_action_is_ignored() {
    // Libraries written while "Describe a passage" existed carry a block
    // for it; the function is gone, the rest of the prompts must survive.
    let old = r#"{
        "stories": [],
        "active_story": 0,
        "next_id": 1,
        "settings": {
            "prompts": {
                "summary": {"text": "Keep it short.", "words": 70},
                "describe": {"text": "Old describe text", "words": 90}
            }
        }
    }"#;
    let old: Library = serde_json::from_str(old).expect("old library");
    assert_eq!(old.settings.prompts.summary.text, "Keep it short.");
    assert_eq!(old.settings.prompts.summary.words, Some(70));

    // Saving it again drops the stale block.
    let json = serde_json::to_string(&old).expect("serialize");
    assert!(!json.contains("describe"), "{json}");
}
