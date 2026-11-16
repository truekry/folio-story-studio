//! What the partner is told, and what it writes offline.

use super::*;

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
    let context = story_context(story, chapter, 1);
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

    let wide = story_context(story, chapter, 5);
    assert!(!wide.contains("STORY SO FAR"));
    assert!(wide.contains("Chapter 1 — The Tide Map"));

    let narrow = story_context(story, chapter, 0);
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
    // Everything before the block is unchanged.
    assert!(steered.starts_with(&plain));

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
    // The partner's role comes first, the question is appended to it.
    assert!(
        prompt.contains("TASK:\nAnswer as the author's writing partner"),
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
    assert!(empty.contains("Answer as the author's writing partner"));

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
    let condensed = summary_prompt(story, chapter, &settings);
    assert!(condensed.contains("STORY SO FAR"));
    assert!(!condensed.contains("RECENT CHAPTER SUMMARIES"));
    settings.summary_window = 2;
    let windowed = summary_prompt(story, chapter, &settings);
    assert!(windowed.contains("RECENT CHAPTER SUMMARIES"));
}

/// A summary asks for a length, so a chatty local model knows where to
/// stop — and it is told, in the same breath, to finish its sentence.
#[test]
fn the_summary_asks_for_a_length_and_a_finished_sentence() {
    let library = Library::sample();
    let story = &library.stories[0];
    let chapter = &story.chapters[2];
    let settings = AiSettings::default();
    let prompt = summary_prompt(story, chapter, &settings);
    assert!(prompt.contains("about 90 words"), "{prompt}");
    assert!(prompt.contains("finish your last sentence"), "{prompt}");

    // The author may want a longer summary; the number follows.
    let mut longer = AiSettings::default();
    longer.prompts.summary.words = Some(140);
    let prompt = summary_prompt(story, chapter, &longer);
    assert!(prompt.contains("about 140 words"), "{prompt}");
    assert!(!prompt.contains("about 90 words"));

    // Zero takes the length request out entirely.
    let mut no_target = AiSettings::default();
    no_target.prompts.summary.words = Some(0);
    let prompt = summary_prompt(story, chapter, &no_target);
    assert!(!prompt.contains("Length:"), "{prompt}");
}

#[test]
fn the_proofreader_keeps_the_authors_words() {
    let prompt = proofread_prompt("She walk to the see.");
    assert!(prompt.contains("careful proofreader"), "{prompt}");
    assert!(prompt.contains("Correct only spelling"), "{prompt}");
    assert!(prompt.contains("do not rewrite"), "{prompt}");
    assert!(prompt.contains("She walk to the see."), "{prompt}");
}

#[test]
fn the_summary_prompt_is_a_plain_text_memory_keeper() {
    let library = Library::sample();
    let story = &library.stories[0];
    let chapter = &story.chapters[1];
    let settings = AiSettings::default();
    let prompt = summary_prompt(story, chapter, &settings);
    assert!(prompt.contains("long-term memory"), "{prompt}");
    assert!(prompt.contains("Write plain prose only"), "{prompt}");
    assert!(!prompt.contains("STRICT JSON"), "{prompt}");
    assert!(!prompt.contains("Output shape"), "{prompt}");
    assert!(prompt.contains("past-tense, third-person"), "{prompt}");
    assert!(prompt.contains("What happened, in order"), "{prompt}");
    assert!(prompt.contains("Reproduce names, numbers"), "{prompt}");
    assert!(prompt.contains("no atmosphere"), "{prompt}");
    assert!(
        prompt.contains("NEW CHAPTER — A House With No Door"),
        "{prompt}"
    );
    assert!(prompt.contains("Do not restate the premise"), "{prompt}");
}

#[test]
fn an_authors_instruction_replaces_the_built_in_one() {
    let mut settings = AiSettings::default();
    settings.prompts.rewrite.text =
        "Make the passage colder and shorter, keep every fact.".to_string();
    let text = Action::Rewrite.instruction("", "The door gave way.", &settings.prompts);
    assert!(text.contains("colder and shorter"), "{text}");
    assert!(!text.contains("sharper and more concrete"), "{text}");
    // The passage is added by Folio, so an edited prompt cannot lose it.
    assert!(text.contains("PASSAGE:\nThe door gave way."), "{text}");

    // Reset means: back to the built-in text.
    settings.prompts.rewrite = Default::default();
    let text = Action::Rewrite.instruction("", "The door gave way.", &settings.prompts);
    assert!(text.contains("sharper and more concrete"), "{text}");
    assert!(text.contains("PASSAGE:\nThe door gave way."), "{text}");
}

#[test]
fn elaborating_uses_its_own_text_and_keeps_the_passage() {
    let mut settings = AiSettings::default();
    let text = Action::Elaborate.instruction("", "The door gave way.", &settings.prompts);
    assert!(text.contains("Elaborate the passage below"), "{text}");
    assert!(text.contains("PASSAGE:\nThe door gave way."), "{text}");

    settings.prompts.elaborate.text = "Slow the moment down.".to_string();
    let own = Action::Elaborate.instruction("", "The door gave way.", &settings.prompts);
    assert!(own.starts_with("Slow the moment down."), "{own}");
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
    assert_eq!(update.target, Some(brinehouse.id));
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
    assert_eq!(sloppy.target, Some(brinehouse.id));
    assert_eq!(sloppy.text, "Salt crusts the windows.");

    // No labels at all: the whole reply becomes the text of a new entry.
    let raw = parse_lore_reply("The bell is older than the town.", story);
    assert_eq!(raw.target, None);
    assert!(!raw.well_formed);
    assert_eq!(raw.text, "The bell is older than the town.");
}
