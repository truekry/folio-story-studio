//! Unit tests for the whole window.

use super::summaries::summary_queue;
use super::*;

fn prose_draft(
    action: &str,
    text: &str,
    body: &str,
    range: Option<Range<usize>>,
    source: Option<String>,
) -> Draft {
    Draft {
        action: action.to_string(),
        text: text.to_string(),
        range: range.map(|r| (r.start, r.end)),
        source,
        base_len: body.chars().count(),
        base_hash: crate::model::content_hash(body),
        stopped: false,
    }
}

#[test]
fn a_continue_draft_ends_up_as_one_new_paragraph() {
    assert_eq!(append_draft("", "  First line.  "), "First line.");
    assert_eq!(
        append_draft("The door closed.\n", "The hall was empty."),
        "The door closed.\n\nThe hall was empty."
    );
    // Never more than one blank line, however the chapter ended.
    let body = "The door closed.\n\n\n";
    let joined = append_draft(body, "The hall was empty.");
    assert_eq!(joined, "The door closed.\n\nThe hall was empty.");
    assert!(!joined.contains("\n\n\n"));
}

#[test]
fn a_rewrite_touches_only_its_passage() {
    let body = "The door closed.\n\nThe hall was empty.";
    let range = last_paragraph_range(body).expect("last paragraph");
    let source = slice_chars(body, &range).to_string();
    let draft = prose_draft(
        "rewrite",
        "The hall waited.",
        body,
        Some(range),
        Some(source),
    );
    assert_eq!(
        apply_draft(&draft, body).expect("apply the rewrite"),
        "The door closed.\n\nThe hall waited."
    );
}

#[test]
fn a_draft_refuses_a_chapter_that_moved() {
    let body = "The door closed.";
    let draft = prose_draft("continue", "The hall waited.", body, None, None);
    let changed = format!("{body} She turned.");
    let error = apply_draft(&draft, &changed).expect_err("the chapter moved");
    assert!(error.contains("changed since this draft"), "{error}");
}

#[test]
fn a_rewrite_refuses_a_passage_that_no_longer_matches() {
    let body = "The door closed.\n\nThe hall was empty.";
    let range = last_paragraph_range(body).expect("last paragraph");
    // The hash still matches, but the passage the model saw does not.
    let draft = prose_draft(
        "rewrite",
        "The hall waited.",
        body,
        Some(range),
        Some("Something else.".to_string()),
    );
    let error = apply_draft(&draft, body).expect_err("the passage moved");
    assert!(error.contains("passage changed"), "{error}");
}

#[test]
fn accepting_a_draft_writes_it_once_and_leaves_the_chat_alone() {
    let mut chapter = Library::sample().stories[0].chapters[0].clone();
    let body = chapter.body.clone();
    let chat = chapter.chat.len();
    chapter.draft = Some(prose_draft(
        "continue",
        "A new last line.",
        &body,
        None,
        None,
    ));

    // The same take-then-write order the Accept button uses.
    let taken = chapter.draft.take().expect("the draft");
    chapter.body = apply_draft(&taken, &chapter.body).expect("accept");
    assert!(
        chapter.draft.take().is_none(),
        "a second click finds nothing"
    );
    assert_eq!(chapter.body, append_draft(&body, "A new last line."));
    assert_eq!(
        chapter.chat.len(),
        chat,
        "Continue never writes to the chat"
    );
}

#[test]
fn accepting_retires_the_steering_note_and_a_failed_accept_keeps_it() {
    let mut chapter = Library::sample().stories[0].chapters[0].clone();
    chapter.steering = "Bring the storm in from the west.".to_string();
    let body = chapter.body.clone();
    let draft = prose_draft("continue", "The hail came first.", &body, None, None);

    accept_into(&mut chapter, &draft).expect("accept");
    assert!(
        chapter.steering.is_empty(),
        "the note retires with the accepted draft"
    );

    // A draft that no longer fits changes neither the chapter nor the note.
    let mut chapter = Library::sample().stories[0].chapters[0].clone();
    chapter.steering = "Bring the storm in from the west.".to_string();
    let stale = prose_draft(
        "continue",
        "The hail came first.",
        "another chapter entirely",
        None,
        None,
    );
    assert!(accept_into(&mut chapter, &stale).is_err());
    assert_eq!(chapter.steering, "Bring the storm in from the west.");
}

#[test]
fn an_elaborate_draft_replaces_its_passage_too() {
    let body = "The door closed.\n\nThe hall was empty.";
    let range = last_paragraph_range(body).expect("last paragraph");
    let source = slice_chars(body, &range).to_string();
    let draft = prose_draft(
        "elaborate",
        "The hall waited, long and dark.",
        body,
        Some(range),
        Some(source),
    );
    assert_eq!(
        apply_draft(&draft, body).expect("apply the elaboration"),
        "The door closed.\n\nThe hall waited, long and dark."
    );
}

#[test]
fn lore_facts_are_appended_without_losing_the_old_text() {
    assert_eq!(append_lore("", "New."), "New.");
    assert_eq!(append_lore("Old.", ""), "Old.");
    assert_eq!(append_lore("Old.", "New."), "Old.\n\nNew.");
}

#[test]
fn clearing_a_chat_drops_messages_and_memory() {
    let mut chapter = Library::sample().stories[0].chapters[1].clone();
    chapter.chat = vec![ChatMessage {
        role: "user".to_string(),
        text: "What should Wren do?".to_string(),
        demo: false,
    }];
    chapter.chat_summary = "Earlier turns, condensed.".to_string();
    clear_chapter_chat(&mut chapter);
    assert!(chapter.chat.is_empty());
    assert!(chapter.chat_summary.is_empty());
    assert!(
        !chapter.body.is_empty(),
        "clearing the chat must leave the chapter text alone"
    );
}

#[test]
fn a_blank_brainstorm_field_sends_nothing() {
    assert_eq!(brainstorm_send_text("   "), None);
    assert_eq!(
        brainstorm_send_text("  What should Wren do? "),
        Some("What should Wren do?".to_string())
    );
}

#[test]
fn the_demo_writer_streams_in_pieces() {
    let text = "One two three. Four five six.\n\nSeven eight nine ten.";
    let (rx, _cancel) = start_demo_stream(text.to_string());
    let mut out = String::new();
    let mut pieces = 0;
    let mut done = false;
    while !done {
        match rx.recv_timeout(Duration::from_secs(5)).expect("an event") {
            StreamEvent::Delta(delta) => {
                pieces += 1;
                out.push_str(&delta);
            }
            StreamEvent::Done { .. } => done = true,
            StreamEvent::Failed(error) => panic!("{error}"),
            StreamEvent::Progress { .. } => {}
        }
    }
    assert_eq!(out, text, "the pieces must add up to the whole reply");
    assert!(pieces > 1, "the demo reply must grow in several pieces");
}

#[test]
fn a_stream_folds_into_one_reply() {
    let mut fold = StreamFold::default();
    assert!(!fold.fold(StreamEvent::Delta("Hel".to_string())));
    assert!(!fold.fold(StreamEvent::Delta("lo".to_string())));
    assert!(!fold.fold(StreamEvent::Delta(" world".to_string())));
    assert_eq!(fold.text, "Hello world");
    assert!(!fold.done);
    assert!(!fold.fold(StreamEvent::Progress { done: 2, total: 5 }));
    assert_eq!(fold.progress, Some((2, 5)));

    assert!(fold.fold(StreamEvent::Done { truncated: false }));
    assert!(fold.done);
    assert_eq!(fold.text, "Hello world");

    // A second end marker must not append anything or reset the text.
    assert!(fold.fold(StreamEvent::Done { truncated: true }));
    assert_eq!(fold.text, "Hello world");
}

#[test]
fn a_failed_stream_keeps_what_arrived() {
    let mut fold = StreamFold::default();
    fold.fold(StreamEvent::Delta("First.".to_string()));
    assert!(fold.fold(StreamEvent::Failed("the stream broke".to_string())));
    assert_eq!(fold.text, "First.");
    assert_eq!(fold.error.as_deref(), Some("the stream broke"));
    assert!(fold.done);
}

#[test]
fn the_lamp_turns_green_only_for_a_named_model() {
    let note = "Connected (HTTP 200). 1 model is listed.".to_string();
    let listed = vec!["mock-model".to_string()];

    // A good answer with a model: green, with the window it reported.
    assert_eq!(
        judge_probe((true, note.clone()), &listed, None, "mock-model", 8192),
        Probe::Online {
            model: "mock-model".to_string(),
            context: Some(8192),
            note: note.clone(),
        }
    );

    // The configured name is not in the list: the listed one is the truth.
    assert_eq!(
        judge_probe((true, note.clone()), &listed, None, "typed-by-hand", 0),
        Probe::Online {
            model: "mock-model".to_string(),
            context: None,
            note: note.clone(),
        }
    );

    // Reached, but no model: red. This is the case the person asked about.
    match judge_probe((true, note.clone()), &[], None, "mock-model", 8192) {
        Probe::Offline(why) => assert!(why.contains("named no model"), "{why}"),
        other => panic!("expected offline, got {other:?}"),
    }

    // The model list itself failed: red, and the reason is kept.
    match judge_probe(
        (true, note.clone()),
        &[],
        Some("This endpoint does not list its models"),
        "mock-model",
        8192,
    ) {
        Probe::Offline(why) => {
            assert!(why.contains("does not list its models"), "{why}")
        }
        other => panic!("expected offline, got {other:?}"),
    }

    // The test itself failed: red with its own sentence.
    match judge_probe(
        (false, "Could not reach the endpoint.".to_string()),
        &listed,
        None,
        "mock-model",
        8192,
    ) {
        Probe::Offline(why) => assert_eq!(why, "Could not reach the endpoint."),
        other => panic!("expected offline, got {other:?}"),
    }
}

#[test]
fn the_lamp_hint_names_the_endpoint_the_model_and_the_window() {
    let online = Probe::Online {
        model: "local-model".to_string(),
        context: Some(32768),
        note: "Connected (HTTP 200). 1 model is listed.".to_string(),
    };
    let hint = online.hint("192.168.10.20:5001/v1", false);
    assert!(hint.contains("Endpoint online"), "{hint}");
    assert!(hint.contains("192.168.10.20:5001/v1"), "{hint}");
    assert!(hint.contains("local-model"), "{hint}");
    assert!(hint.contains("32768 tokens"), "{hint}");
    assert!(hint.contains("Click to check again"), "{hint}");

    let offline = Probe::Offline("Could not reach the endpoint.".to_string());
    let hint = offline.hint("127.0.0.1:5099/v1", false);
    assert!(hint.contains("Endpoint offline"), "{hint}");
    assert!(hint.contains("Could not reach the endpoint."), "{hint}");

    // No endpoint at all: the demo writer, and where to change that.
    let hint = Probe::Idle.hint("https://api.openai.com/v1", true);
    assert!(hint.contains("demo writer"), "{hint}");
    assert!(hint.contains("Click to open settings"), "{hint}");

    // A screenshot state with no real answer stays quiet about details.
    assert!(Probe::Checking
        .hint("", false)
        .contains("Asking the endpoint"));
}

#[test]
fn an_export_notices_files_that_are_already_there() {
    let story = Library::sample().stories[0].clone();
    let dir = std::env::temp_dir().join(format!("folio-existing-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("folder");

    let pdf = export_targets(&story, "Draft", &dir, ExportKind::Pdf);
    assert_eq!(pdf.len(), 1);
    assert_eq!(pdf[0].file_name().unwrap(), "Draft.pdf");
    assert!(existing_names(&pdf).is_empty(), "nothing written yet");

    let chapters = export_targets(&story, "Draft", &dir, ExportKind::Chapters);
    assert_eq!(chapters.len(), story.chapters.len());
    assert_eq!(
        chapters[1].file_name().unwrap(),
        "Draft_02_A House With No Door.txt"
    );
    assert!(existing_names(&chapters).is_empty());

    // A file left from an earlier export is spotted before anything runs.
    std::fs::write(&pdf[0], b"old pdf").expect("write a file");
    std::fs::write(&chapters[2], b"old chapter").expect("write a file");
    assert_eq!(existing_names(&pdf), vec!["Draft.pdf".to_string()]);
    assert_eq!(
        existing_names(&chapters),
        vec!["Draft_03_Salt in the Wiring.txt".to_string()]
    );

    // Another name touches nothing, and a folder counts only files.
    assert!(existing_names(&export_targets(
        &story,
        "Draft 2",
        &dir,
        ExportKind::Chapters
    ))
    .is_empty());
    std::fs::create_dir_all(&chapters[0]).expect("a folder with the file's name");
    assert!(
        existing_names(&chapters).len() == 1,
        "a folder is not a file"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn char_ranges_are_unicode_safe() {
    let text = "Mira walked to the harbour.\n\nThe bell rang — once.";
    let range = last_paragraph_range(text).expect("last paragraph");
    assert_eq!(slice_chars(text, &range), "The bell rang — once.");
    let replaced = replace_char_range(text, &range, "The bell answered.");
    assert!(replaced.ends_with("The bell answered."));
    assert!(replaced.starts_with("Mira walked"));
}

#[test]
fn empty_chapter_has_no_range() {
    assert!(last_paragraph_range("   \n\n  ").is_none());
}

#[test]
fn find_ignores_case_and_is_unicode_safe() {
    let text = "Mira met the Keeper. The keeper carried a lantern — and MIRA followed.";
    let ranges = find_char_ranges(text, "keeper", false);
    assert_eq!(ranges.len(), 2);
    assert_eq!(ranges[0], 13..19);
    assert_eq!(slice_chars(text, &ranges[1]), "keeper");

    let exact = find_char_ranges(text, "Mira", true);
    assert_eq!(exact.len(), 1);
    assert_eq!(slice_chars(text, &exact[0]), "Mira");
}

#[test]
fn replacing_every_match_keeps_the_words_around_them() {
    let text = "The door was red. Another door, then a third Door.";
    let (replaced, count) = replace_all_in_text(text, "door", "gate", false);
    assert_eq!(count, 3);
    assert_eq!(
        replaced,
        "The gate was red. Another gate, then a third gate."
    );

    let (deleted, count) = replace_all_in_text("one two one one", "one ", "", true);
    assert_eq!(count, 2);
    assert_eq!(deleted, "two one");
}

#[test]
fn the_desk_says_what_the_ai_can_see() {
    let both = context_caption(7, 3);
    assert!(
        both.contains("story so far (7 chapters)"),
        "caption was {both:?}"
    );
    assert!(
        both.contains("the last 3 summaries"),
        "caption was {both:?}"
    );

    let recap_only = context_caption(2, 0);
    assert!(recap_only.contains("story so far (2 chapters)"));
    assert!(!recap_only.contains("the last"));

    assert!(context_caption(0, 0).contains("No earlier chapter"));
}

#[test]
fn the_desk_says_how_full_the_chapter_conversation_is() {
    let line = conversation_line(1_200, 32_768);
    assert!(line.contains("1200 of 32768 tokens"), "line was {line:?}");
    assert!(
        line.contains("27852"),
        "the 85% mark must be named: {line:?}"
    );
    assert!(line.contains("85%"));
}

#[test]
fn the_settings_say_why_the_key_matters() {
    // No address: the demo writer answers.
    let empty = AiSettings {
        base_url: String::new(),
        ..Default::default()
    };
    assert!(endpoint_note(&empty).contains("No address yet"));

    // A cloud service without a key is the only case that asks for one.
    let cloud = AiSettings::default();
    assert!(
        endpoint_note(&cloud).contains("needs a key"),
        "{:?}",
        endpoint_note(&cloud)
    );

    // A server on the home network needs nothing: the key field can stay empty.
    let lan = AiSettings {
        base_url: "192.168.10.20:5001/v1".to_string(),
        ..Default::default()
    };
    assert!(
        endpoint_note(&lan).contains("no key needed"),
        "{:?}",
        endpoint_note(&lan)
    );

    // An https leftover is downgraded for the home network and said so.
    let tls = AiSettings {
        base_url: "https://192.168.10.20:5001/v1".to_string(),
        ..Default::default()
    };
    assert!(
        endpoint_note(&tls).contains("plain HTTP"),
        "{:?}",
        endpoint_note(&tls)
    );

    // With a key the same address is simply used.
    let keyed = AiSettings {
        base_url: "https://api.openai.com/v1".to_string(),
        api_key: "sk-test".to_string(),
        ..Default::default()
    };
    assert!(endpoint_note(&keyed).contains("used with the key"));
}

#[test]
fn the_settings_name_the_context_window() {
    assert!(context_line(0).contains("not reported"));
    let known = context_line(8192);
    assert!(known.contains("8192 tokens"), "line was {known:?}");
    assert!(known.contains("trimmed"));
}

#[test]
fn a_summary_press_covers_the_missing_older_chapters_first() {
    let mut story = crate::model::Library::sample().stories[0].clone();
    story.chapters[0].summary.clear();
    story.chapters[1].summary.clear();
    let ids = summary_queue(&story, 2, 1);
    assert_eq!(
        ids,
        vec![
            story.chapters[0].key.clone(),
            story.chapters[1].key.clone(),
            story.chapters[2].key.clone()
        ]
    );

    // With every older summary in place, only the open chapter is left.
    let library = crate::model::Library::sample();
    let story = &library.stories[0];
    let ids = summary_queue(story, 2, 2);
    assert_eq!(ids, vec![story.chapters[2].key.clone()]);
}

#[test]
fn paragraphs_are_split_at_blank_lines_and_trimmed() {
    let body = "  First line.\nStill first.\n\nSecond paragraph.\n\n\nThird.  ";
    let ranges = paragraph_ranges(body);
    assert_eq!(ranges.len(), 3);
    let texts: Vec<String> = ranges
        .iter()
        .map(|range| slice_chars(body, range).to_string())
        .collect();
    assert_eq!(
        texts,
        vec!["First line.\nStill first.", "Second paragraph.", "Third."]
    );
    assert!(paragraph_ranges("   \n\n  ").is_empty());
}

#[test]
fn stepping_through_matches_wraps_around() {
    assert_eq!(step_find_hit(None, 3, true), Some(0));
    assert_eq!(step_find_hit(Some(0), 3, true), Some(1));
    assert_eq!(step_find_hit(Some(2), 3, true), Some(0));
    assert_eq!(step_find_hit(None, 3, false), Some(2));
    assert_eq!(step_find_hit(Some(0), 3, false), Some(2));
    assert_eq!(step_find_hit(Some(1), 0, true), None);
}

/// The top-bar buttons behave like a radio group with an off switch: the same
/// view again closes the panel, another view replaces it.
#[test]
fn a_panel_button_toggles_only_its_own_view() {
    assert_eq!(
        next_tab(true, Tab::Settings, Tab::Settings),
        (false, Tab::Settings)
    );
    assert_eq!(
        next_tab(true, Tab::Ai, Tab::Settings),
        (true, Tab::Settings)
    );
    assert_eq!(next_tab(false, Tab::Find, Tab::Find), (true, Tab::Find));
    // Asking for a view that is not open only ever opens it.
    assert_eq!(
        next_tab(false, Tab::Summary, Tab::Summary),
        (true, Tab::Summary)
    );
}

/// The lamp under the chapter title says exactly one thing per state, and
/// green says nothing at all.
#[test]
fn the_chapter_lamp_speaks_three_states() {
    assert_eq!(summary_lamp(false, false, false).1, "No summary");
    assert_eq!(summary_lamp(false, true, false).1, "No summary");
    assert_eq!(summary_lamp(true, true, false).1, "Outdated summary");
    assert_eq!(summary_lamp(true, false, false).1, "");
    // A partial summary is never sold as current.
    assert_eq!(summary_lamp(true, false, true).1, "Partial summary");
    assert_eq!(summary_lamp(true, true, true).1, "Partial summary");
}

/// A lore card has to accept a right-click, or its context menu (edit, pin,
/// delete) can never open. Drawing it head-lessly and clicking with the
/// secondary button is the only honest way to check that.
#[test]
fn a_lore_card_answers_a_right_click() {
    use eframe::egui::{self, Event, Modifiers, PointerButton, Pos2, RawInput, Rect, Vec2};

    let entry = crate::model::Library::sample().stories[0].lore[0].clone();
    let ctx = egui::Context::default();
    // The card draws in the app's serif family, which only `install` binds.
    crate::theme::install(&ctx, crate::theme::Look::Paper, None);
    let mut response = None;

    let mut draw = |events: Vec<Event>| {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 700.0))),
            events,
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                response = Some(lore_card(ui, &entry, None));
            });
        });
    };

    let at = Pos2::new(120.0, 40.0);
    draw(vec![Event::PointerMoved(at)]);
    draw(vec![Event::PointerButton {
        pos: at,
        button: PointerButton::Secondary,
        pressed: true,
        modifiers: Modifiers::default(),
    }]);
    draw(vec![Event::PointerButton {
        pos: at,
        button: PointerButton::Secondary,
        pressed: false,
        modifiers: Modifiers::default(),
    }]);

    let response = response.expect("the card was drawn");
    assert!(
        response.secondary_clicked(),
        "the lore card must sense the right mouse button so its menu can open"
    );
}

/// The chapter editor must survive a right click on marked words: egui
/// treats the press like a new click and collapses the selection, which is
/// exactly what hid the selection and the menu.
#[test]
fn a_right_click_keeps_the_text_selection_and_opens_the_menu() {
    use eframe::egui::{self, Event, Modifiers, PointerButton, Pos2, RawInput, Rect, Vec2};
    use std::cell::{Cell, RefCell};

    fn press(pos: Pos2, button: PointerButton, pressed: bool) -> Event {
        Event::PointerButton {
            pos,
            button,
            pressed,
            modifiers: Modifiers::default(),
        }
    }

    let ctx = egui::Context::default();
    crate::theme::install(&ctx, crate::theme::Look::Paper, None);
    let mut text = "The tide came back at dawn.".to_string();
    let id = Id::new("test-editor");
    let kept: RefCell<Option<Range<usize>>> = RefCell::new(None);
    let menu_opened = Cell::new(false);
    let cursor_after: RefCell<Option<egui::text::CCursorRange>> = RefCell::new(None);

    let mut draw = |events: Vec<Event>| {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 400.0))),
            events,
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let had_focus = ui.memory(|memory| memory.has_focus(id));
                let output = egui::TextEdit::multiline(&mut text)
                    .id(id)
                    .font(egui::FontId::proportional(16.0))
                    .desired_width(400.0)
                    .show(ui);
                {
                    let mut selection = kept.borrow_mut();
                    let secondary = keep_selection_on_secondary(ui, id, had_focus, &mut selection);
                    if output.response.has_focus() && !secondary {
                        if let Some(cursor) = output.cursor_range {
                            *selection = Some(cursor.as_sorted_char_range());
                        }
                    }
                }
                if kept
                    .borrow()
                    .as_ref()
                    .is_some_and(|range| range.start < range.end)
                {
                    output.response.context_menu(|ui| {
                        menu_opened.set(true);
                        if ui.button("Rewrite").clicked() {
                            ui.close();
                        }
                    });
                }
                *cursor_after.borrow_mut() = output.cursor_range;
            });
        });
    };

    // Focus the page and drag across the first words.
    let start = Pos2::new(12.0, 16.0);
    let end = Pos2::new(140.0, 16.0);
    draw(vec![Event::PointerMoved(start)]);
    draw(vec![press(start, PointerButton::Primary, true)]);
    draw(vec![Event::PointerMoved(end)]);
    draw(vec![Event::PointerMoved(end)]);
    draw(vec![press(end, PointerButton::Primary, false)]);
    draw(vec![]);

    let marked = kept.borrow().clone().expect("a selection");
    assert!(marked.start < marked.end, "the drag must select words");

    // Right click on the marked words: press, release, settle.
    draw(vec![Event::PointerMoved(end)]);
    draw(vec![press(end, PointerButton::Secondary, true)]);
    draw(vec![press(end, PointerButton::Secondary, false)]);
    draw(vec![]);

    assert_eq!(
        *kept.borrow(),
        Some(marked),
        "the marked words must survive the right click"
    );
    assert!(
        menu_opened.get(),
        "the context menu must open on the marked words"
    );
    assert!(
        cursor_after
            .borrow()
            .is_some_and(|range| range.primary.index != range.secondary.index),
        "the highlight must still be visible"
    );
}

#[test]
fn a_rewrite_that_only_repeats_the_passage_is_recognised() {
    let source = "The sea was very beautiful, and the light on it was gold.";
    assert_eq!(echo_similarity(source, source), 1.0);
    assert!(
        echo_similarity(
            "the sea was very beautiful and the light on it was gold",
            source
        ) > 0.95,
        "case and punctuation do not make a rewrite"
    );
    assert!(
        echo_similarity(
            "Gold light lay across the water; the sea itself was lovely.",
            source
        ) < 0.5,
        "a real rewrite keeps some nouns, not the wording"
    );
    assert_eq!(echo_similarity("", ""), 0.0);
}

#[test]
fn proofreading_changes_only_the_corrected_spans() {
    let body = "Mira sat down.\n\nShe begun to drew it again.\n\nShe did not made it beautiful.";
    let ranges = paragraph_ranges(body);
    assert_eq!(ranges.len(), 3);
    let second = slice_chars(body, &ranges[1]).to_string();
    let third = slice_chars(body, &ranges[2]).to_string();
    let corrections = vec![
        (ranges[1].clone(), "She began to draw it again.".to_string()),
        (
            ranges[2].clone(),
            "She did not make it beautiful.".to_string(),
        ),
    ];
    let fixed = apply_corrections(body, &corrections);
    assert!(fixed.contains("She began to draw it again."));
    assert!(fixed.contains("She did not make it beautiful."));
    assert!(!fixed.contains("begun"));
    assert!(!fixed.contains("did not made"));
    // Everything outside the two paragraphs is byte-identical.
    assert!(fixed.starts_with("Mira sat down.\n\n"));
    assert_eq!(
        fixed,
        "Mira sat down.\n\nShe began to draw it again.\n\nShe did not make it beautiful."
    );
    assert_ne!(second, "She began to draw it again.");
    assert_ne!(third, "She did not make it beautiful.");

    // A stale range is clamped, not fatal.
    let clamped = apply_corrections("short", &[(3..99, "er".to_string())]);
    assert_eq!(clamped, "shoer");
}

#[test]
fn an_elaborate_draft_uses_the_marked_paragraph_not_the_last_one() {
    let body = "Opening line.\n\nThis middle paragraph is the one marked.\n\nClosing line.";
    let ranges = paragraph_ranges(body);
    let marked = ranges[1].clone();
    let source = slice_chars(body, &marked).to_string();
    let draft = prose_draft(
        "elaborate",
        "This middle paragraph is the one marked, now with more weather and a chair.",
        body,
        Some(marked),
        Some(source),
    );
    let result = apply_draft(&draft, body).expect("apply the elaboration");
    assert!(result.starts_with("Opening line.\n\nThis middle paragraph"));
    assert!(result.ends_with("\n\nClosing line."));
    assert_eq!(result.matches("Closing line.").count(), 1);
}

#[test]
fn the_side_panel_never_squeezes_the_editor_away() {
    // At the smallest window a full-width panel would leave no page at all.
    let (min, max) = desk_width_range(1040.0);
    assert_eq!(min, 320.0);
    assert!(max <= 360.0, "1040 px leaves about 354 px: {max}");
    assert!(max >= 320.0);
    assert_eq!(
        LIBRARY_WIDTH + max + 420.0,
        1040.0,
        "the editor keeps its 420 px column"
    );

    // A wide window allows the full 640 px the panel was designed for.
    assert_eq!(desk_width_range(1440.0), (320.0, 640.0));
    assert_eq!(desk_width_range(2560.0), (320.0, 640.0));

    // A window below the design minimum still offers a usable panel.
    let (min, max) = desk_width_range(900.0);
    assert_eq!(min, 320.0);
    assert_eq!(max, 320.0);
}

/// The full proofread of chapter 3 against a real endpoint. The two seeded
/// mistakes must be fixed and no name may be invented; a handful of other
/// small fixes is allowed, a rewrite is not.
#[test]
#[ignore = "needs FOLIO_LAN_URL"]
fn the_lan_proofread_of_chapter_three_changes_only_the_seeded_errors() {
    let Ok(base_url) = std::env::var("FOLIO_LAN_URL") else {
        panic!("set FOLIO_LAN_URL to an OpenAI-compatible endpoint");
    };
    let settings = AiSettings {
        base_url,
        api_key: std::env::var("FOLIO_LAN_API_KEY").unwrap_or_default(),
        model: std::env::var("FOLIO_LAN_MODEL").unwrap_or_default(),
        ..Default::default()
    };
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/bench_chapter_3.txt");
    let Ok(body) = std::fs::read_to_string(&path) else {
        eprintln!("skipped: local fixture tests/bench_chapter_3.txt is not in the repo");
        return;
    };
    let ranges = paragraph_ranges(&body);
    assert!(ranges.len() > 30, "the fixture is one long chapter");
    let cancel = AtomicBool::new(false);
    let started = Instant::now();
    let mut corrections = Vec::new();
    for range in &ranges {
        let original = slice_chars(&body, range).to_string();
        let corrected = ai::proofread_paragraph(&settings, &original, &cancel).expect("proofread");
        let corrected = corrected.trim().to_string();
        if !corrected.is_empty() && corrected != original {
            corrections.push((range.clone(), corrected));
        }
    }
    println!(
        "{} of {} paragraphs changed in {:.1}s",
        corrections.len(),
        ranges.len(),
        started.elapsed().as_secs_f32()
    );
    for (_, text) in &corrections {
        println!("CHANGED: {text}");
    }
    let fixed = apply_corrections(&body, &corrections);
    assert!(!fixed.contains("begun, slowly and carefully, to drew"));
    assert!(!fixed.contains("did not made"));
    assert!(
        fixed.contains("began, slowly and carefully, to draw"),
        "the first seeded mistake was not corrected"
    );
    assert!(
        corrections.len() <= 5,
        "the proofread must stay a correction, not a rewrite: {} paragraphs changed",
        corrections.len()
    );
    for invented in ["software office", "Cary", "amaryllis", "cutout"] {
        assert!(
            !fixed.contains(invented),
            "the proofreader invented “{invented}”"
        );
    }
}

#[test]
fn notices_stay_readable_on_a_narrow_window() {
    // The normal window is 1040 px at its narrowest; the notice may use a
    // comfortable column but must leave the edges alone.
    let width = toast_width(1040.0);
    assert!(width >= 320.0, "the text column must not collapse: {width}");
    assert!(
        width <= 620.0,
        "the notice must not grow with its text: {width}"
    );

    for screen in [360.0, 640.0, 1040.0, 1440.0, 2560.0] {
        let width = toast_width(screen);
        assert!(width > 0.0, "screen {screen}");
        assert!(
            width <= screen - 24.0,
            "toast {width} does not fit a {screen} px window"
        );
    }
}

#[test]
fn confirmations_fit_the_window_they_open_in() {
    let width = confirm_width(1040.0);
    assert!(width >= 320.0, "the dialog text must have room: {width}");
    assert!(width <= 460.0, "the dialog must not become a page: {width}");

    for screen in [360.0, 640.0, 1040.0, 1440.0, 2560.0] {
        let width = confirm_width(screen);
        assert!(width > 0.0, "screen {screen}");
        assert!(
            width <= screen - 40.0,
            "dialog {width} does not fit a {screen} px window"
        );
    }
}

fn chat_line(role: &str, text: &str) -> ChatMessage {
    ChatMessage {
        role: role.to_string(),
        text: text.to_string(),
        demo: false,
    }
}

#[test]
fn the_bin_removes_exactly_one_entry_at_a_time() {
    let mut chapter = Library::sample().stories[0].chapters[1].clone();
    chapter.chat = vec![
        chat_line("user", "First question."),
        chat_line("assistant", "First answer."),
        chat_line("note", "A quiet note."),
    ];
    assert!(
        !removable_last(&chapter, false),
        "a system note has nothing to undo"
    );

    chapter.chat.pop();
    assert!(removable_last(&chapter, false));
    assert_eq!(
        pop_last_message(&mut chapter).map(|message| message.text),
        Some("First answer.".to_string()),
        "the answer goes first, not the whole turn"
    );
    assert_eq!(chapter.chat.len(), 1);
    assert_eq!(
        pop_last_message(&mut chapter).map(|message| message.text),
        Some("First question.".to_string())
    );
    assert!(chapter.chat.is_empty());
    assert!(pop_last_message(&mut chapter).is_none());
    assert!(!removable_last(&chapter, false), "an empty chat has no bin");
}

#[test]
fn the_bin_is_hidden_while_a_job_is_running() {
    let mut chapter = Library::sample().stories[0].chapters[1].clone();
    chapter.chat = vec![chat_line("user", "What should Wren do?")];
    assert!(removable_last(&chapter, false));
    assert!(
        !removable_last(&chapter, true),
        "a running answer is not part of the history yet"
    );
}

/// The layout block senses dragging only, so the double-click is read from the
/// pointer. Drawing the card head-lessly and clicking twice is the only honest
/// way to check that the block really switches off.
#[test]
fn a_double_click_switches_a_prompt_block_off() {
    use eframe::egui::{self, Event, Modifiers, PointerButton, Pos2, RawInput, Rect, Vec2};

    let ctx = egui::Context::default();
    crate::theme::install(&ctx, crate::theme::Look::Paper, None);
    let mut order: Vec<String> = Vec::new();
    let mut disabled: Vec<String> = Vec::new();

    let draw = |events: Vec<Event>, order: &mut Vec<String>, disabled: &mut Vec<String>| {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 700.0))),
            events,
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                prompt_layout_card(ui, LayoutKind::Writing, order, disabled, None);
            });
        });
    };
    let click = |at: Pos2, pressed: bool| Event::PointerButton {
        pos: at,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::default(),
    };

    draw(Vec::new(), &mut order, &mut disabled);
    let id = egui::Id::new(("prompt-block", "writing", "lorebook"));
    let at = ctx
        .read_response(id)
        .expect("the lorebook block was drawn")
        .rect
        .center();

    for _ in 0..2 {
        draw(vec![Event::PointerMoved(at)], &mut order, &mut disabled);
        draw(vec![click(at, true)], &mut order, &mut disabled);
        draw(vec![click(at, false)], &mut order, &mut disabled);
    }

    assert_eq!(
        disabled,
        vec!["lorebook".to_string()],
        "a double-click must switch the block off"
    );
    assert!(order.is_empty(), "switching a block off is not a reorder");
}

#[test]
fn the_neighbours_around_a_passage_are_read() {
    let body = "First.\n\nSecond.\n\nThird.\n\nFourth.";
    let ranges = paragraph_ranges(body);

    let (before, after) = neighbour_paragraphs(body, &ranges[1]);
    assert_eq!(before.as_deref(), Some("First."));
    assert_eq!(after.as_deref(), Some("Third."));

    let (before, after) = neighbour_paragraphs(body, &ranges[0]);
    assert!(
        before.is_none(),
        "the first paragraph has nothing before it"
    );
    assert_eq!(after.as_deref(), Some("Second."));

    let (before, after) = neighbour_paragraphs(body, &ranges[3]);
    assert_eq!(before.as_deref(), Some("Third."));
    assert!(after.is_none(), "the last paragraph has nothing after it");

    // A selection across two paragraphs skips both of them.
    let across = ranges[1].start..ranges[2].end;
    let (before, after) = neighbour_paragraphs(body, &across);
    assert_eq!(before.as_deref(), Some("First."));
    assert_eq!(after.as_deref(), Some("Fourth."));
}

#[test]
fn a_very_long_neighbour_is_shortened() {
    let body = format!("{}\n\nEnd.", "word ".repeat(300));
    let ranges = paragraph_ranges(&body);
    let (before, _) = neighbour_paragraphs(&body, &ranges[1]);
    let before = before.expect("a neighbour");
    assert!(before.chars().count() <= 601, "{}", before.chars().count());
    assert!(before.ends_with('…'));
}

#[test]
fn accepting_a_rewrite_keeps_the_steering_note() {
    let mut chapter = Library::sample().stories[0].chapters[1].clone();
    chapter.steering = "Bring the storm in from the west.".to_string();
    let body = chapter.body.clone();
    let range = last_paragraph_range(&body).expect("last paragraph");
    let source = slice_chars(&body, &range).to_string();
    let draft = prose_draft(
        "rewrite",
        "A new last paragraph.",
        &body,
        Some(range),
        Some(source),
    );

    let mut rewritten = chapter.clone();
    accept_into(&mut rewritten, &draft).expect("accept the rewrite");
    assert_eq!(
        rewritten.steering, "Bring the storm in from the west.",
        "a rewrite must not use up the note for the next continuation"
    );

    // A continuation does use the note up.
    let continuation = prose_draft("continue", "More prose.", &body, None, None);
    accept_into(&mut chapter, &continuation).expect("accept the continuation");
    assert!(chapter.steering.is_empty());
}

#[test]
fn a_draft_that_hands_back_a_neighbour_is_noticed() {
    let before =
        "The harbour wall had been patched in her absence with stones of a different grey.";
    let echoed =
        "The harbour wall had been patched in her absence with stones, and the paint was new.";
    assert!(draft_repeats_neighbour(echoed, Some(before), None));
    assert!(!draft_repeats_neighbour(
        "She walked to the quay.",
        Some(before),
        None
    ));
    // A short neighbour is not proof of an echo.
    assert!(!draft_repeats_neighbour(
        "Short line.",
        Some("Short line."),
        None
    ));
}

/// The two cards share block names, so their drag ids must differ; otherwise a
/// double-click in one layout switches the other one off.
#[test]
fn the_two_layout_cards_keep_their_own_switches() {
    use eframe::egui::{self, Event, Modifiers, PointerButton, Pos2, RawInput, Rect, Vec2};

    let ctx = egui::Context::default();
    crate::theme::install(&ctx, crate::theme::Look::Paper, None);
    let mut order: Vec<String> = Vec::new();
    let mut disabled: Vec<String> = Vec::new();
    let mut chat_order: Vec<String> = Vec::new();
    let mut chat_disabled: Vec<String> = Vec::new();

    let draw = |events: Vec<Event>,
                order: &mut Vec<String>,
                disabled: &mut Vec<String>,
                chat_order: &mut Vec<String>,
                chat_disabled: &mut Vec<String>| {
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 1400.0))),
            events,
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                prompt_layout_card(ui, LayoutKind::Writing, order, disabled, None);
                prompt_layout_card(ui, LayoutKind::Chat, chat_order, chat_disabled, None);
            });
        });
    };

    draw(
        Vec::new(),
        &mut order,
        &mut disabled,
        &mut chat_order,
        &mut chat_disabled,
    );
    let id = egui::Id::new(("prompt-block", "chat", "lorebook"));
    let at = ctx
        .read_response(id)
        .expect("the chat's lorebook block was drawn")
        .rect
        .center();
    let click = |pressed: bool| Event::PointerButton {
        pos: at,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::default(),
    };
    for _ in 0..2 {
        draw(
            vec![Event::PointerMoved(at)],
            &mut order,
            &mut disabled,
            &mut chat_order,
            &mut chat_disabled,
        );
        draw(
            vec![click(true)],
            &mut order,
            &mut disabled,
            &mut chat_order,
            &mut chat_disabled,
        );
        draw(
            vec![click(false)],
            &mut order,
            &mut disabled,
            &mut chat_order,
            &mut chat_disabled,
        );
    }

    assert_eq!(chat_disabled, vec!["lorebook".to_string()]);
    assert!(
        disabled.is_empty(),
        "the writing layout must stay untouched"
    );
}

#[test]
fn the_bin_survives_a_restart() {
    let dir = std::env::temp_dir().join(format!("folio-bin-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("bin folder");

    let mut library = Library::sample();
    let mut story = library.stories.remove(0);
    let chapter = story.chapters.remove(0);
    let trash = vec![TrashItem::Chapter {
        id: 7,
        story_id: story.key.clone(),
        story_title: story.title.clone(),
        index: 0,
        chapter,
        deleted_at: crate::model::now_millis(),
    }];
    super::stories::save_trash(&dir, &trash);

    let loaded = super::stories::load_trash(&dir);
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].id(), 7);
    assert_eq!(loaded[0].title(), trash[0].title());
    assert_eq!(loaded[0].story_id(), trash[0].story_id());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_cached_find_matches_a_fresh_one_and_notices_edits() {
    let text = "Mira MIRA Mira";
    let mut cache = None;
    let first = find_ranges_cached(&mut cache, text, "mira", false);
    let second = find_ranges_cached(&mut cache, text, "mira", false);
    assert_eq!(first, second);
    assert_eq!(first, find_char_ranges(text, "mira", false));

    let edited = find_ranges_cached(&mut cache, "Mira", "mira", false);
    assert_eq!(edited.len(), 1);
    let exact = find_ranges_cached(&mut cache, text, "Mira", true);
    assert_eq!(exact, find_char_ranges(text, "Mira", true));
}
