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
            story.chapters[0].id,
            story.chapters[1].id,
            story.chapters[2].id
        ]
    );

    // With every older summary in place, only the open chapter is left.
    let library = crate::model::Library::sample();
    let story = &library.stories[0];
    let ids = summary_queue(story, 2, 2);
    assert_eq!(ids, vec![story.chapters[2].id]);
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

/// A lore entry has to accept a right-click, or its context menu (edit, pin,
/// delete) can never open. Drawing it head-lessly and clicking with the
/// secondary button is the only honest way to check that.
#[test]
fn a_lore_card_answers_a_right_click() {
    use eframe::egui::{self, Event, Modifiers, PointerButton, Pos2, RawInput, Rect, Vec2};

    let entry = crate::model::Library::sample().stories[0].lore[0].clone();
    let ctx = egui::Context::default();
    // The card draws in the app's serif family, which only `install` binds.
    crate::theme::install(&ctx);
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
    crate::theme::install(&ctx);
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
