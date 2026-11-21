//! What the partner is told, and what it writes offline.

use super::*;

fn single_summary(story: &Story, chapter: &Chapter, settings: &AiSettings) -> String {
    summary_block_prompt(story, chapter, &chapter.body, 0, 1, settings)
}

#[test]
fn local_summary_uses_the_chapter() {
    let library = Library::sample();
    let chapter = &library.stories[0].chapters[0];
    let summary = local_summary(chapter);
    assert!(summary.len() > 60, "summary was {summary:?}");
    assert!(summary.contains("Mira"), "summary was {summary:?}");
}

#[test]
fn demo_writer_uses_the_lorebook() {
    let library = Library::sample();
    let story = &library.stories[0];
    let chapter = &story.chapters[1];
    let draft = demo_reply(Action::Continue, story, chapter, "", "");
    assert!(draft.contains("Mira"));
    assert!(draft.split_whitespace().count() > 80);
}

#[test]
fn the_demo_writer_answers_a_brainstorm_question() {
    let library = Library::sample();
    let story = &library.stories[0];
    let chapter = &story.chapters[1];
    let asked = demo_reply(
        Action::Brainstorm,
        story,
        chapter,
        "What should Wren do?",
        "",
    );
    assert!(asked.contains("What should Wren do?"), "{asked}");
    let five = demo_reply(Action::Brainstorm, story, chapter, "", "");
    assert!(five.contains("Five directions worth trying"), "{five}");
}

#[test]
fn the_prompt_lays_out_recap_recent_and_current() {
    let library = Library::sample();
    let story = &library.stories[0];
    let chapter = &story.chapters[2];
    let context = story_context(story, chapter, 1, 6000);
    let recap_at = context.find("STORY SO FAR").expect("recap block");
    let recent_at = context
        .find("RECENT CHAPTER SUMMARIES")
        .expect("recent block");
    let current_at = context.find("CURRENT CHAPTER").expect("current chapter");
    let lore_at = context.find("LOREBOOK").expect("lorebook");
    assert!(recap_at < recent_at, "the recap must come first");
    assert!(
        recent_at < current_at,
        "the recent summaries must come first"
    );
    assert!(current_at < lore_at, "the lorebook travels last");
}

#[test]
fn a_narrow_window_moves_more_into_the_recap() {
    let library = Library::sample();
    let story = &library.stories[0];
    let chapter = &story.chapters[2];

    let wide = story_context(story, chapter, 5, 6000);
    assert!(!wide.contains("STORY SO FAR"));
    assert!(wide.contains("Chapter 1 — The Tide Map"));

    let narrow = story_context(story, chapter, 0, 6000);
    assert!(narrow.contains("STORY SO FAR (chapters 1–2, condensed)"));
    assert!(!narrow.contains("RECENT CHAPTER SUMMARIES"));
    // Chapter one is condensed into the recap, without its label.
    assert!(!narrow.contains("Chapter 1 — The Tide Map"));
    assert!(narrow.contains("Mira returns to Brinehouse"));
}

#[test]
fn steering_joins_the_prompt_only_when_it_is_written() {
    let library = Library::sample();
    let story = &library.stories[0];
    let mut chapter = story.chapters[1].clone();
    let settings = AiSettings::default();

    // Empty steering reproduces the old request byte for byte.
    let plain = chat_prompt(story, &chapter, Action::Continue, "", "", &settings);
    chapter.steering = "   ".to_string();
    let padded = chat_prompt(story, &chapter, Action::Continue, "", "", &settings);
    assert_eq!(plain, padded);
    assert!(!plain.contains("STEERING"));

    chapter.steering = "Bring the storm in from the west.".to_string();
    let steered = chat_prompt(story, &chapter, Action::Continue, "", "", &settings);
    assert!(
        steered.contains(
            "STEERING — the author wants the next part to go this way:\nBring the storm in from the west."
        ),
        "{steered}"
    );
    // Everything before the steering block is unchanged.
    assert!(steered.starts_with("TASK:"));
    assert!(plain.starts_with("TASK:"));

    // Brainstorming is not prose: its note stays out of the request.
    let chat = chat_prompt(
        story,
        &chapter,
        Action::Brainstorm,
        "What next?",
        "",
        &settings,
    );
    assert!(!chat.contains("STEERING"), "{chat}");
}

#[test]
fn a_typed_brainstorm_question_is_answered_as_a_partner() {
    let library = Library::sample();
    let story = &library.stories[0];
    let chapter = &story.chapters[1];
    let mut settings = AiSettings::default();

    let prompt = chat_prompt(
        story,
        chapter,
        Action::Brainstorm,
        "  What should Wren do?  ",
        "",
        &settings,
    );
    // The assistant's role comes first, the question is appended to it.
    assert!(
        prompt.contains("TASK:\nAnswer as the author's assistant for this chapter"),
        "{prompt}"
    );
    assert!(
        prompt.contains("AUTHOR'S QUESTION:\nWhat should Wren do?\n"),
        "{prompt}"
    );
    assert!(
        prompt.contains("Do not continue, rewrite or draft the scene"),
        "{prompt}"
    );
    // It sees the page it is talking about.
    assert!(prompt.contains("CURRENT CHAPTER — A House With No Door"));
    assert!(prompt.contains("LOREBOOK"));
    assert!(prompt.contains("RECENT CHAPTER SUMMARIES"), "{prompt}");

    // Without a question only the role is sent.
    let empty = chat_prompt(story, chapter, Action::Brainstorm, "", "", &settings);
    assert!(!empty.contains("AUTHOR'S QUESTION"), "{empty}");
    assert!(empty.contains("Answer as the author's assistant for this chapter"));

    // An edited role is kept, and the question follows it.
    settings.prompts.brainstorm.text = "Be blunt.".to_string();
    let own = chat_prompt(
        story,
        chapter,
        Action::Brainstorm,
        "Is the boy lying?",
        "",
        &settings,
    );
    assert!(
        own.contains("TASK:\nBe blunt.\n\nAUTHOR'S QUESTION:\nIs the boy lying?\n"),
        "{own}"
    );
}

#[test]
fn summary_prompt_sees_the_same_memory_as_the_writer() {
    let library = Library::sample();
    let story = &library.stories[0];
    let chapter = &story.chapters[2];
    let mut settings = AiSettings {
        summary_window: 0,
        ..Default::default()
    };
    let condensed = single_summary(story, chapter, &settings);
    assert!(condensed.contains("STORY SO FAR"));
    assert!(!condensed.contains("RECENT CHAPTER SUMMARIES"));
    settings.summary_window = 2;
    let windowed = single_summary(story, chapter, &settings);
    assert!(windowed.contains("RECENT CHAPTER SUMMARIES"));
}

/// A summary asks for a hard word limit, so a chatty local model knows where
/// to stop.
#[test]
fn the_summary_asks_for_a_length_and_a_finished_sentence() {
    let library = Library::sample();
    let story = &library.stories[0];
    let chapter = &story.chapters[2];
    let settings = AiSettings::default();
    let prompt = single_summary(story, chapter, &settings);
    assert!(prompt.contains("at most 90 words"), "{prompt}");
    assert!(prompt.contains("Do not exceed 90"), "{prompt}");

    // The author may want a longer summary; the number follows.
    let mut longer = AiSettings::default();
    longer.prompts.summary.words = Some(140);
    let prompt = single_summary(story, chapter, &longer);
    assert!(prompt.contains("at most 140 words"), "{prompt}");
    assert!(!prompt.contains("at most 90 words"));

    // Zero takes the length request out entirely.
    let mut no_target = AiSettings::default();
    no_target.prompts.summary.words = Some(0);
    let prompt = single_summary(story, chapter, &no_target);
    assert!(!prompt.contains("Answer in at most"), "{prompt}");
}

#[test]
fn the_proofreader_keeps_the_authors_words() {
    let prompt = proofread_prompt("She walk to the see.");
    assert!(prompt.contains("Correct only spelling"), "{prompt}");
    assert!(prompt.contains("Do not rewrite"), "{prompt}");
    assert!(prompt.contains("OLD:"), "{prompt}");
    assert!(prompt.contains("NEW:"), "{prompt}");
    assert!(prompt.contains("She walk to the see."), "{prompt}");
}

#[test]
fn the_summary_prompt_is_a_plain_text_memory_keeper() {
    let library = Library::sample();
    let story = &library.stories[0];
    let chapter = &story.chapters[1];
    let settings = AiSettings::default();
    let prompt = single_summary(story, chapter, &settings);
    assert!(prompt.contains("long-term memory"), "{prompt}");
    assert!(prompt.contains("Write plain prose only"), "{prompt}");
    assert!(!prompt.contains("STRICT JSON"), "{prompt}");
    assert!(!prompt.contains("Output shape"), "{prompt}");
    assert!(prompt.contains("past-tense, third-person"), "{prompt}");
    assert!(prompt.contains("Summarize the passage below"), "{prompt}");
    assert!(prompt.contains("Keep every name, number"), "{prompt}");
    assert!(
        prompt.contains("do not resolve anything it leaves open"),
        "{prompt}"
    );
    assert!(prompt.contains("PASSAGE:\n"), "{prompt}");
}

#[test]
fn an_authors_instruction_replaces_the_built_in_one() {
    let library = Library::sample();
    let story = &library.stories[0];
    let chapter = &story.chapters[1];
    let mut settings = AiSettings::default();
    settings.prompts.rewrite.text =
        "Make the passage colder and shorter, keep every fact.".to_string();
    let prompt = chat_prompt(
        story,
        chapter,
        Action::Rewrite,
        "",
        "The door gave way.",
        &settings,
    );
    assert!(
        prompt.contains("Make the passage colder and shorter"),
        "{prompt}"
    );
    assert!(!prompt.contains("sharper, more concrete"), "{prompt}");
    // The passage is added by Folio, so an edited prompt cannot lose it.
    assert!(prompt.contains("PASSAGE:\nThe door gave way."), "{prompt}");
    // The instruction leads, the source sits in the middle, a reminder closes.
    assert!(prompt.starts_with("TASK:"), "{prompt}");
    assert!(prompt.find("TASK:") < prompt.find("PASSAGE:"), "{prompt}");
    assert!(prompt.find("PASSAGE:") < prompt.find("NOW:"), "{prompt}");

    // Reset means: back to the built-in text.
    settings.prompts.rewrite = Default::default();
    let prompt = chat_prompt(
        story,
        chapter,
        Action::Rewrite,
        "",
        "The door gave way.",
        &settings,
    );
    assert!(prompt.contains("sharper, more concrete"), "{prompt}");
    assert!(prompt.contains("PASSAGE:\nThe door gave way."), "{prompt}");
}

#[test]
fn elaborating_uses_its_own_text_and_keeps_the_passage() {
    let library = Library::sample();
    let story = &library.stories[0];
    let chapter = &story.chapters[1];
    let mut settings = AiSettings::default();
    let prompt = chat_prompt(
        story,
        chapter,
        Action::Elaborate,
        "",
        "The door gave way.",
        &settings,
    );
    assert!(prompt.contains("Expand the text under PASSAGE"), "{prompt}");
    assert!(prompt.contains("PASSAGE:\nThe door gave way."), "{prompt}");

    settings.prompts.elaborate.text = "Slow the moment down.".to_string();
    let own = chat_prompt(
        story,
        chapter,
        Action::Elaborate,
        "",
        "The door gave way.",
        &settings,
    );
    assert!(own.contains("Slow the moment down."), "{own}");
    assert!(own.starts_with("TASK:"), "{own}");
    assert!(own.contains("PASSAGE:\nThe door gave way."), "{own}");
}

#[test]
fn the_lore_prompt_lists_the_entries_and_the_passage() {
    let library = Library::sample();
    let story = &library.stories[0];
    let prompt = lore_prompt(story, "The key was stamped E. VALE.");
    assert!(prompt.contains("EXISTING LOREBOOK ENTRIES"), "{prompt}");
    assert!(prompt.contains("Brinehouse"), "{prompt}");
    assert!(
        prompt.contains("PASSAGE:\nThe key was stamped E. VALE."),
        "{prompt}"
    );
    assert!(prompt.contains("TARGET:"), "{prompt}");
}

#[test]
fn the_demo_lore_pass_uses_a_known_name() {
    let library = Library::sample();
    let story = &library.stories[0];
    let reply = demo_lore_reply(story, "Mira put the key in the lock at Brinehouse.");
    let suggestion = parse_lore_reply(&reply, story);
    assert!(
        suggestion.target.is_some(),
        "a known alias must resolve: {suggestion:?}"
    );
    assert!(
        suggestion.text.contains("Mira put the key"),
        "{suggestion:?}"
    );
}

#[test]
fn a_lore_reply_resolves_to_an_entry_or_a_new_one() {
    let library = Library::sample();
    let story = &library.stories[0];
    let brinehouse = story
        .lore
        .iter()
        .find(|entry| entry.name == "Brinehouse")
        .expect("sample entry");

    let update = parse_lore_reply(
        "TARGET: Brinehouse\nTEXT: The harbour bell rings only when the tide turns.",
        story,
    );
    assert_eq!(update.target, Some(brinehouse.key.clone()));
    assert_eq!(update.name, "Brinehouse");
    assert_eq!(update.category, "Place");
    assert!(update.text.contains("harbour bell"), "{update:?}");

    let new = parse_lore_reply(
        "TARGET: NEW\nNAME: The Brass Key\nCATEGORY: Item\nALIASES: the key, E. Vale's key\nTRIGGERS: brass, salt\nTEXT: A key stamped with E. VALE.\nSecond line of facts.",
        story,
    );
    assert_eq!(new.target, None);
    assert_eq!(new.name, "The Brass Key");
    assert_eq!(new.category, "Item");
    assert!(new.text.contains("Second line of facts."), "{new:?}");
    assert!(new.well_formed);

    // Bold labels and list dashes are tolerated.
    let sloppy = parse_lore_reply(
        "- **TARGET:** Brinehouse\n- **TEXT:** Salt crusts the windows.",
        story,
    );
    assert_eq!(sloppy.target, Some(brinehouse.key.clone()));
    assert_eq!(sloppy.text, "Salt crusts the windows.");

    // No labels at all: the whole reply becomes the text of a new entry.
    let raw = parse_lore_reply("The bell is older than the town.", story);
    assert_eq!(raw.target, None);
    assert!(!raw.well_formed);
    assert_eq!(raw.text, "The bell is older than the town.");
}

#[test]
fn a_first_chapter_in_a_new_book_is_named_as_such() {
    let library = Library::sample();
    let mut story = library.stories[0].clone();
    let mut chapter = story.chapters[0].clone();
    chapter.body.clear();
    chapter.summary.clear();
    story.chapters = vec![chapter.clone()];
    let prompt = chat_prompt(
        &story,
        &chapter,
        Action::Continue,
        "",
        "",
        &AiSettings::default(),
    );
    assert!(
        prompt.contains("CHAPTER POSITION: chapter 1 of 1"),
        "{prompt}"
    );
    assert!(
        prompt.contains("first chapter of a new book"),
        "the first chapter must not look like a continuation: {prompt}"
    );
    assert!(
        prompt.contains("No earlier chapter or chapter summary exists"),
        "{prompt}"
    );
    assert!(prompt.contains("The current chapter is empty"), "{prompt}");
}

#[test]
fn a_new_empty_chapter_does_not_continue_the_previous_sentence() {
    let library = Library::sample();
    let story = library.stories[0].clone();
    let mut chapter = story.chapters[1].clone();
    chapter.body.clear();
    let prompt = chat_prompt(
        &story,
        &chapter,
        Action::Continue,
        "",
        "",
        &AiSettings::default(),
    );
    assert!(
        prompt.contains("CHAPTER POSITION: chapter 2 of 3"),
        "{prompt}"
    );
    assert!(prompt.contains("NEW CHAPTER after chapter 1"), "{prompt}");
    assert!(
        prompt.contains("do not continue chapter 1's last sentence"),
        "{prompt}"
    );
    assert!(prompt.contains("The current chapter is empty"), "{prompt}");
}

#[test]
fn an_ongoing_chapter_is_not_mistaken_for_a_new_one() {
    let library = Library::sample();
    let story = library.stories[0].clone();
    let chapter = story.chapters[1].clone();
    let prompt = chat_prompt(
        &story,
        &chapter,
        Action::Continue,
        "",
        "",
        &AiSettings::default(),
    );
    assert!(
        prompt.contains("CHAPTER POSITION: chapter 2 of 3"),
        "{prompt}"
    );
    assert!(
        prompt.contains("following chapter 1"),
        "an ongoing chapter still knows where it stands: {prompt}"
    );
    assert!(
        !prompt.contains("NEW CHAPTER"),
        "a chapter with prose is not a fresh new chapter: {prompt}"
    );
}

/// Dragging down lands after the target, dragging up lands before it; dropping
/// a block onto itself changes nothing.
#[test]
fn moving_a_block_by_drag_reorders_the_layout() {
    let mut order = PromptBlock::PROSE_ORDER.to_vec();
    assert!(move_block(
        &mut order,
        PromptBlock::StorySoFar,
        PromptBlock::Task
    ));
    assert_eq!(order[0], PromptBlock::StorySoFar);

    assert!(move_block(
        &mut order,
        PromptBlock::Task,
        PromptBlock::Story
    ));
    let task = order
        .iter()
        .position(|block| *block == PromptBlock::Task)
        .expect("task is here");
    let story = order
        .iter()
        .position(|block| *block == PromptBlock::Story)
        .expect("story is here");
    assert!(task > story, "dragging down lands after the target");

    assert!(!move_block(
        &mut order,
        PromptBlock::Task,
        PromptBlock::Task
    ));
    assert_eq!(order.len(), PromptBlock::PROSE_ORDER.len());
}

/// The layout repairs a hand-edited list instead of losing blocks.
#[test]
fn the_prompt_layout_keeps_every_block_and_drops_junk() {
    let standard = normalized_order(&[], &PromptBlock::PROSE_ORDER);
    assert_eq!(standard.as_slice(), PromptBlock::PROSE_ORDER.as_slice());

    let saved = vec![
        "current_chapter".to_string(),
        "story".to_string(),
        "current_chapter".to_string(),
        "assistant_rules".to_string(),
        "bogus".to_string(),
    ];
    let order = normalized_order(&saved, &PromptBlock::PROSE_ORDER);
    assert_eq!(order[0], PromptBlock::CurrentChapter);
    assert_eq!(order[1], PromptBlock::Story);
    assert_eq!(order.len(), PromptBlock::PROSE_ORDER.len());
    assert!(order.contains(&PromptBlock::Task));
}

/// A custom order changes the assembled prompt; blocks that do not belong to
/// the action stay out even when they are in the order.
#[test]
fn a_custom_order_rebuilds_the_prompt_and_skips_blocks_that_do_not_apply() {
    let library = Library::sample();
    let story = &library.stories[0];
    let chapter = &story.chapters[1];
    let mut settings = AiSettings::default();
    settings.prompts.order = vec![
        "current_chapter".to_string(),
        "story".to_string(),
        "task".to_string(),
    ];
    let prompt = chat_prompt(story, chapter, Action::Continue, "", "", &settings);
    assert!(
        prompt.find("CURRENT CHAPTER") < prompt.find("STORY:"),
        "{prompt}"
    );
    assert!(prompt.find("STORY:") < prompt.find("TASK:"), "{prompt}");
    // Normalization appends the blocks the saved list did not name.
    assert!(prompt.contains("LOREBOOK"), "{prompt}");

    let edit = chat_prompt(story, chapter, Action::Rewrite, "", "The door.", &settings);
    assert!(!edit.contains("CURRENT CHAPTER"), "{edit}");
    assert!(edit.contains("PASSAGE:\nThe door."), "{edit}");
}

/// The assistant frame names its two actions, states the honesty rule and can
/// never carry the API key.
#[test]
fn the_assistant_prompt_names_its_actions_and_never_carries_the_key() {
    let library = Library::sample();
    let story = &library.stories[0];
    let chapter = &story.chapters[1];
    let settings = AiSettings {
        api_key: "sk-do-not-leak".to_string(),
        ..Default::default()
    };
    let prompt = chat_prompt(
        story,
        chapter,
        Action::Brainstorm,
        "Add Mira's clothes.",
        "",
        &settings,
    );
    assert!(prompt.contains("[[FOLIO:LORE]]"), "{prompt}");
    assert!(prompt.contains("[[FOLIO:SUMMARY]]"), "{prompt}");
    assert!(prompt.contains("[[FOLIO:INVENT]]"), "{prompt}");
    assert!(prompt.contains("do not praise by default"), "{prompt}");
    assert!(prompt.contains("No settings or API keys"), "{prompt}");
    assert!(!prompt.contains("sk-do-not-leak"));
    // The fixed rules are appended after the sortable prompt, not part of it.
    let frame_at = prompt.find("THREE ACTIONS").expect("assistant frame");
    let chapter_at = prompt.find("CURRENT CHAPTER").expect("chapter block");
    assert!(frame_at > chapter_at, "{prompt}");

    // Prose writing is not the assistant and must not see the action rules.
    let prose = chat_prompt(story, chapter, Action::Continue, "", "", &settings);
    assert!(!prose.contains("[[FOLIO:LORE]]"));
}

#[test]
fn a_disabled_block_is_left_out_of_the_request() {
    let library = Library::sample();
    let story = &library.stories[0];
    let chapter = &story.chapters[0];
    let mut settings = AiSettings::default();
    let whole = chat_prompt(story, chapter, Action::Continue, "", "", &settings);
    assert!(whole.contains("STORY:"), "{whole}");

    settings.prompts.disabled = vec!["story".to_string()];
    let thinned = chat_prompt(story, chapter, Action::Continue, "", "", &settings);
    assert!(
        !thinned.contains("STORY:"),
        "a block switched off must not travel: {thinned}"
    );
    // The rest of the request is untouched.
    assert!(thinned.contains("TASK:"), "{thinned}");
    assert!(thinned.contains("CURRENT CHAPTER"), "{thinned}");
}

#[test]
fn the_task_block_cannot_be_switched_off() {
    let library = Library::sample();
    let story = &library.stories[0];
    let chapter = &story.chapters[0];
    let settings = AiSettings {
        prompts: crate::model::PromptSettings {
            disabled: vec![
                "task".to_string(),
                "Task".to_string(),
                "story".to_string(),
                "story".to_string(),
            ],
            ..Default::default()
        },
        ..Default::default()
    };
    let prompt = chat_prompt(story, chapter, Action::Continue, "", "", &settings);
    assert!(
        prompt.contains("TASK:"),
        "a request without its task has nothing to do: {prompt}"
    );
    assert!(!prompt.contains("STORY:"), "{prompt}");
    // Unknown names, duplicates and the task block are repaired away.
    assert_eq!(
        normalized_disabled(&settings.prompts.disabled, &PromptBlock::PROSE_ORDER),
        vec![PromptBlock::Story]
    );
}

#[test]
fn a_block_toggles_off_and_on_again() {
    let mut saved = Vec::new();
    assert!(toggle_disabled(
        &mut saved,
        PromptBlock::Lorebook,
        &PromptBlock::PROSE_ORDER
    ));
    assert_eq!(saved, vec!["lorebook".to_string()]);
    assert!(toggle_disabled(
        &mut saved,
        PromptBlock::Lorebook,
        &PromptBlock::PROSE_ORDER
    ));
    assert!(
        saved.is_empty(),
        "the second double-click switches it back on"
    );
    assert!(!toggle_disabled(
        &mut saved,
        PromptBlock::Task,
        &PromptBlock::PROSE_ORDER
    ));
    assert!(saved.is_empty(), "the task block stays on");
}

#[test]
fn reset_brings_every_block_back() {
    let mut order = vec!["lorebook".to_string(), "task".to_string()];
    let mut disabled = vec!["lorebook".to_string(), "steering".to_string()];
    reset_layout(&mut order, &mut disabled);
    assert!(order.is_empty(), "the standard order returns");
    assert!(disabled.is_empty(), "every block is switched on again");
}

#[test]
fn the_disabled_list_survives_a_saved_settings_file() {
    let settings = crate::model::PromptSettings {
        disabled: vec!["lorebook".to_string(), "steering".to_string()],
        ..Default::default()
    };
    let text = serde_json::to_string(&settings).expect("serialize the settings");
    let back: crate::model::PromptSettings =
        serde_json::from_str(&text).expect("read the settings back");
    assert_eq!(back.disabled, settings.disabled);

    // A file from before this field means: every block is on.
    let old: crate::model::PromptSettings = serde_json::from_str("{}").expect("old settings");
    assert!(old.disabled.is_empty());
}

#[test]
fn a_rewrite_gets_only_the_passage_and_its_neighbours() {
    let library = Library::sample();
    let story = &library.stories[0];
    let chapter = &story.chapters[1];
    let settings = AiSettings::default();
    let prompt = chat_prompt_with_neighbours(
        story,
        chapter,
        Action::Rewrite,
        "",
        "The door gave way.",
        Some("Before this line."),
        Some("After this line."),
        &settings,
    );
    assert!(
        prompt.contains("BEFORE (read-only context, do not return it):\nBefore this line."),
        "{prompt}"
    );
    assert!(prompt.contains("PASSAGE:\nThe door gave way."), "{prompt}");
    assert!(
        prompt.contains("AFTER (read-only context, do not return it):\nAfter this line."),
        "{prompt}"
    );
    assert!(
        prompt.contains("Rewrite only the text under PASSAGE"),
        "{prompt}"
    );
    // Nothing a rewrite must not see: no steering, no lore, no premise, no
    // chapter position, no summaries.
    for absent in [
        "STEERING",
        "LOREBOOK",
        "PREMISE",
        "STORY SO FAR",
        "RECENT CHAPTER SUMMARIES",
        "CURRENT CHAPTER",
        "CHAPTER POSITION",
        "CONTEXT",
    ] {
        assert!(
            !prompt.contains(absent),
            "{absent} reached the rewrite: {prompt}"
        );
    }
}

#[test]
fn an_elaborate_brings_premise_lore_and_the_chapter_summary() {
    let library = Library::sample();
    let story = &library.stories[0];
    let chapter = &story.chapters[1];
    let settings = AiSettings::default();
    let prompt = chat_prompt_with_neighbours(
        story,
        chapter,
        Action::Elaborate,
        "",
        "The door gave way.",
        None,
        None,
        &settings,
    );
    assert!(prompt.contains("PREMISE:"), "{prompt}");
    assert!(prompt.contains("LOREBOOK"), "{prompt}");
    assert!(prompt.contains("CHAPTER SUMMARY"), "{prompt}");
    assert!(
        prompt.contains("Expand only the text under PASSAGE"),
        "{prompt}"
    );
    // Even here the writing layout and the steering note stay out.
    assert!(!prompt.contains("STEERING"), "{prompt}");
    assert!(!prompt.contains("STORY SO FAR"), "{prompt}");
    assert!(!prompt.contains("CURRENT CHAPTER"), "{prompt}");
}

#[test]
fn a_missing_neighbour_leaves_its_block_out() {
    let library = Library::sample();
    let story = &library.stories[0];
    let chapter = &story.chapters[1];
    let settings = AiSettings::default();
    let prompt = chat_prompt_with_neighbours(
        story,
        chapter,
        Action::Rewrite,
        "",
        "The door gave way.",
        None,
        Some("After this line."),
        &settings,
    );
    assert!(!prompt.contains("BEFORE (read-only context"), "{prompt}");
    assert!(prompt.contains("AFTER (read-only context"), "{prompt}");
}

#[test]
fn the_two_layouts_do_not_touch_each_other() {
    let library = Library::sample();
    let story = &library.stories[0];
    let chapter = &story.chapters[1];
    let mut settings = AiSettings::default();

    // Off for writing, still on for the assistant.
    settings.prompts.disabled = vec!["lorebook".to_string()];
    let prose = chat_prompt(story, chapter, Action::Continue, "", "", &settings);
    assert!(!prose.contains("LOREBOOK"), "{prose}");
    let chat = chat_prompt(
        story,
        chapter,
        Action::Brainstorm,
        "Who is Mira?",
        "",
        &settings,
    );
    assert!(
        chat.contains("LOREBOOK"),
        "the chat keeps its layout: {chat}"
    );

    // Off for the assistant, still on for writing.
    settings.prompts.disabled.clear();
    settings.prompts.chat_disabled = vec!["current_chapter".to_string()];
    let chat = chat_prompt(
        story,
        chapter,
        Action::Brainstorm,
        "Who is Mira?",
        "",
        &settings,
    );
    assert!(!chat.contains("CURRENT CHAPTER"), "{chat}");
    let prose = chat_prompt(story, chapter, Action::Continue, "", "", &settings);
    assert!(prose.contains("CURRENT CHAPTER"), "{prose}");
}

#[test]
fn each_layout_drops_blocks_it_does_not_use() {
    assert_eq!(PromptBlock::PROSE_ORDER.len(), 10);
    assert_eq!(PromptBlock::CHAT_ORDER.len(), 7);
    assert!(!PromptBlock::CHAT_ORDER.contains(&PromptBlock::Steering));
    assert!(!PromptBlock::CHAT_ORDER.contains(&PromptBlock::Reminder));
    assert!(!PromptBlock::CHAT_ORDER.contains(&PromptBlock::ChapterPosition));

    let saved = vec![
        "steering".to_string(),
        "lorebook".to_string(),
        "passage".to_string(),
    ];
    let chat = normalized_order(&saved, &PromptBlock::CHAT_ORDER);
    assert_eq!(chat[0], PromptBlock::Lorebook);
    assert!(!chat.contains(&PromptBlock::Steering));
    assert_eq!(chat.len(), PromptBlock::CHAT_ORDER.len());

    let disabled = vec![
        "steering".to_string(),
        "task".to_string(),
        "lorebook".to_string(),
        "passage".to_string(),
    ];
    assert_eq!(
        normalized_disabled(&disabled, &PromptBlock::CHAT_ORDER),
        vec![PromptBlock::Lorebook]
    );
    assert_eq!(
        normalized_disabled(&disabled, &PromptBlock::PROSE_ORDER),
        vec![PromptBlock::Steering, PromptBlock::Lorebook]
    );
}

#[test]
fn an_old_writing_order_with_the_passage_block_still_loads() {
    let saved = vec!["lorebook".to_string(), "passage".to_string()];
    let order = normalized_order(&saved, &PromptBlock::PROSE_ORDER);
    assert_eq!(order[0], PromptBlock::Lorebook);
    assert_eq!(order.len(), PromptBlock::PROSE_ORDER.len());
}

#[test]
fn the_assistant_is_not_told_where_the_chapter_stands() {
    let library = Library::sample();
    let story = &library.stories[0];
    let chapter = &story.chapters[1];
    let settings = AiSettings::default();
    let chat = chat_prompt(
        story,
        chapter,
        Action::Brainstorm,
        "What could happen next?",
        "",
        &settings,
    );
    assert!(
        !chat.contains("CHAPTER POSITION"),
        "the assistant does not write into the chapter: {chat}"
    );
    // The writing prompt still knows its place in the book.
    let prose = chat_prompt(story, chapter, Action::Continue, "", "", &settings);
    assert!(prose.contains("CHAPTER POSITION"), "{prose}");
}

#[test]
fn the_premise_block_is_called_the_story_bible() {
    assert_eq!(PromptBlock::Premise.label(), "Story bible");
    assert_eq!(PromptBlock::Premise.card_label(), "Story bible");
}

#[test]
fn the_assistant_reads_more_of_the_chapter_than_a_draft() {
    let settings = AiSettings::default();
    assert_eq!(chapter_excerpt_chars(Action::Continue, &settings), 6_000);
    assert_eq!(chapter_excerpt_chars(Action::Brainstorm, &settings), 12_000);

    // A reported window gives the assistant half of it, within bounds.
    let small = AiSettings {
        context_limit: 8_000,
        ..Default::default()
    };
    assert_eq!(chapter_excerpt_chars(Action::Brainstorm, &small), 16_000);
    let huge = AiSettings {
        context_limit: 200_000,
        ..Default::default()
    };
    assert_eq!(chapter_excerpt_chars(Action::Brainstorm, &huge), 24_000);
}

#[test]
fn a_switched_off_block_shrinks_the_request() {
    let library = Library::sample();
    let story = &library.stories[0];
    let chapter = &story.chapters[1];
    let mut settings = AiSettings::default();
    let full = chat_prompt(story, chapter, Action::Continue, "", "", &settings);
    let full_tokens = prompt_tokens(&full, &[], "");

    settings.prompts.disabled = vec!["lorebook".to_string()];
    let thin = chat_prompt(story, chapter, Action::Continue, "", "", &settings);
    let thin_tokens = prompt_tokens(&thin, &[], "");
    assert!(
        thin_tokens < full_tokens,
        "switching the lorebook off must lower the size: {thin_tokens} vs {full_tokens}"
    );
}

#[test]
fn the_assistant_size_grows_with_its_conversation() {
    let library = Library::sample();
    let story = &library.stories[0];
    let mut chapter = story.chapters[1].clone();
    let settings = AiSettings::default();
    let bare = chat_prompt(story, &chapter, Action::Brainstorm, "Why?", "", &settings);
    let bare_tokens = prompt_tokens(&bare, &chapter.chat, &chapter.chat_summary);

    chapter.chat.push(crate::model::ChatMessage {
        role: "user".to_string(),
        text: "A long earlier question about the tide and the bell.".to_string(),
        demo: false,
    });
    chapter.chat.push(crate::model::ChatMessage {
        role: "assistant".to_string(),
        text: "A long earlier answer about the map room and the tea.".to_string(),
        demo: false,
    });
    let busy = chat_prompt(story, &chapter, Action::Brainstorm, "Why?", "", &settings);
    let busy_tokens = prompt_tokens(&busy, &chapter.chat, &chapter.chat_summary);
    assert!(
        busy_tokens > bare_tokens,
        "the assistant's conversation must count: {busy_tokens} vs {bare_tokens}"
    );
}
