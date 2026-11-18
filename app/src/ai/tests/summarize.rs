//! Block splitting and the block/merge prompts.

use super::*;

fn words(text: &str) -> usize {
    text.split_whitespace().count()
}

fn all_words<'a>(body: &'a str, blocks: &[std::ops::Range<usize>]) -> Vec<&'a str> {
    blocks
        .iter()
        .flat_map(|range| body[range.clone()].split_whitespace())
        .collect()
}

#[test]
fn a_short_chapter_is_one_block() {
    let body = "Mira opened the door.\n\nOutside, the sea was quiet.";
    let blocks = summary_blocks(body, SUMMARY_BLOCK_WORDS);
    assert_eq!(blocks.len(), 1);
    assert_eq!(&body[blocks[0].clone()], body);
}

#[test]
fn blank_text_has_no_blocks() {
    assert!(summary_blocks("", SUMMARY_BLOCK_WORDS).is_empty());
    assert!(summary_blocks(" \n\n \t ", SUMMARY_BLOCK_WORDS).is_empty());
}

#[test]
fn blocks_fill_to_the_limit_and_keep_every_word() {
    // One-word sentences, so every boundary is also a sentence end.
    let paragraph = "word. ".repeat(500);
    let body = format!("{paragraph}\n\n{paragraph}\n\n{paragraph}");
    let blocks = summary_blocks(&body, 1200);
    assert_eq!(blocks.len(), 2, "1500 words need two blocks");
    for range in &blocks {
        let text = body[range.clone()].trim_end();
        assert!(words(text) <= 1200);
        assert!(text.ends_with('.'), "block ended mid-sentence: {text:?}");
    }
    assert_eq!(
        all_words(&body, &blocks),
        body.split_whitespace().collect::<Vec<_>>(),
        "no word may be lost or repeated"
    );
}

#[test]
fn a_giant_paragraph_is_split_at_sentences_and_stays_bounded() {
    let body = (0..3000)
        .map(|index| format!("Sentence {index}."))
        .collect::<Vec<_>>()
        .join(" ");
    let blocks = summary_blocks(&body, 1200);
    assert!(blocks.len() >= 3, "blocks were {blocks:?}");
    for range in &blocks {
        assert!(
            words(&body[range.clone()]) <= SUMMARY_BLOCK_MAX_WORDS,
            "block {} is too large",
            words(&body[range.clone()])
        );
    }
    assert_eq!(
        all_words(&body, &blocks),
        body.split_whitespace().collect::<Vec<_>>()
    );
}

#[test]
fn the_block_size_is_a_setting_and_blocks_end_at_sentences() {
    let mut settings = AiSettings::default();
    assert_eq!(summary_block_size(&settings), SUMMARY_BLOCK_WORDS);

    settings.summary_block_size = 300;
    assert_eq!(summary_block_size(&settings), 300);
    let body = (0..1000)
        .map(|index| format!("Sentence {index}."))
        .collect::<Vec<_>>()
        .join(" ");
    let blocks = summary_blocks(&body, summary_block_size(&settings));
    assert!(blocks.len() >= 3, "blocks were {blocks:?}");
    for range in &blocks {
        let text = body[range.clone()].trim_end();
        assert!(
            text.ends_with('.'),
            "a block ended in the middle of a sentence: {text:?}"
        );
        assert!(
            words(text) <= 300,
            "block is over the limit: {}",
            words(text)
        );
    }
    assert_eq!(
        all_words(&body, &blocks),
        body.split_whitespace().collect::<Vec<_>>()
    );

    // A hand-edited file cannot ask for a useless block size.
    settings.summary_block_size = 1;
    assert_eq!(summary_block_size(&settings), SUMMARY_BLOCK_MIN_WORDS);
    settings.summary_block_size = 99_999;
    assert_eq!(summary_block_size(&settings), SUMMARY_BLOCK_MAX_WORDS);
}

#[test]
fn a_sentence_longer_than_the_limit_is_not_cut() {
    // One 401-word sentence, then two short ones. The limit is 300, but the
    // long sentence must stay whole.
    let body = format!("{}end. Short one. Next sentence.", "word ".repeat(400));
    let blocks = summary_blocks(&body, 300);
    assert_eq!(blocks.len(), 2, "blocks were {blocks:?}");
    let first = body[blocks[0].clone()].trim_end();
    assert!(
        first.ends_with("end."),
        "cut inside the sentence: {first:?}"
    );
    assert_eq!(words(first), 401);
    assert_eq!(
        body[blocks[1].clone()].trim_end(),
        "Short one. Next sentence."
    );
}

#[test]
fn umlauts_and_emoji_never_split_a_character() {
    let body = "Über den Hafen. 🌊 ".repeat(1400);
    let blocks = summary_blocks(&body, 1200);
    assert!(!blocks.is_empty());
    for range in &blocks {
        assert!(
            body.get(range.clone()).is_some(),
            "range {range:?} is not a byte boundary"
        );
        assert!(words(&body[range.clone()]) <= SUMMARY_BLOCK_MAX_WORDS);
    }
    assert_eq!(
        all_words(&body, &blocks),
        body.split_whitespace().collect::<Vec<_>>()
    );
}

#[test]
fn the_word_target_is_the_target_of_each_block() {
    let mut settings = AiSettings::default();
    assert_eq!(summary_block_words(&settings), 90);

    settings.prompts.summary.words = Some(140);
    assert_eq!(summary_block_words(&settings), 140);

    settings.prompts.summary.words = Some(0);
    assert_eq!(
        summary_block_words(&settings),
        0,
        "no target means no target"
    );
}

#[test]
fn the_first_block_carries_the_story_memory_and_later_blocks_do_not() {
    let library = Library::sample();
    let story = &library.stories[0];
    let chapter = &story.chapters[2];
    let settings = AiSettings {
        summary_window: 0,
        ..Default::default()
    };

    let first = summary_block_prompt(story, chapter, "First passage.", 0, 3, &settings);
    assert!(first.contains("STORY SO FAR"), "{first}");
    assert!(first.contains("PASSAGE:\nFirst passage."), "{first}");
    assert!(first.contains("Summarize the passage below"), "{first}");

    let later = summary_block_prompt(story, chapter, "Second passage.", 1, 3, &settings);
    assert!(later.contains("PASSAGE:\nSecond passage."), "{later}");
    assert!(
        !later.contains("First passage."),
        "a block must not see another block's text: {later}"
    );
    assert!(
        !later.contains("STORY SO FAR"),
        "the prior chapters must not be repeated for every block"
    );
    assert!(
        !later.contains("SUMMARY SO FAR"),
        "no block may see the earlier block summaries: {later}"
    );
}

#[test]
fn a_block_prompt_does_not_cut_the_passage_at_a_character_limit() {
    let library = Library::sample();
    let story = &library.stories[0];
    let chapter = &story.chapters[0];
    let settings = AiSettings::default();
    let passage = "word ".repeat(5000);
    let prompt = summary_block_prompt(story, chapter, &passage, 0, 2, &settings);
    assert!(
        prompt.contains(passage.trim()),
        "the whole block must reach the model"
    );
}

#[test]
fn the_merge_prompt_keeps_the_whole_combined_summary() {
    let combined = "A fact that must survive. ".repeat(1000);
    let prompt = summary_merge_prompt(&combined, 0);
    assert!(
        prompt.contains(combined.trim()),
        "the merge must see every part"
    );
}

#[test]
fn the_merge_prompt_asks_for_one_continuous_text() {
    let combined = "Mira opens the door. The boy gives her a key.";
    let prompt = summary_merge_prompt(combined, 180);
    assert!(prompt.contains("one continuous past-tense, third-person summary"));
    assert!(prompt.contains("Remove repetition and seams"));
    assert!(prompt.contains("COMBINED BLOCK SUMMARIES:\nMira opens the door."));
    assert!(
        prompt.contains("at most 180 words"),
        "the whole-chapter target belongs to the merge: {prompt}"
    );
    assert!(prompt.contains("do not add themes"), "{prompt}");

    // Zero means the author asked for no length at all.
    let open = summary_merge_prompt(combined, 0);
    assert!(!open.contains("Answer in at most"), "{open}");
}

#[test]
fn a_runaway_summary_is_trimmed_at_a_sentence_end() {
    // Five sentences of ten words each: 50 words against a 20-word target
    // (limit 24) keeps the first two sentences.
    let sentence = "One two three four five six seven eight nine ten. ";
    let text = sentence.repeat(5);
    let (trimmed, changed) = trim_to_target(&text, 20);
    assert!(changed);
    assert_eq!(trimmed, sentence.repeat(2).trim_end());
    assert_eq!(trimmed.split_whitespace().count(), 20);
    assert!(trimmed.ends_with('.'));
}

#[test]
fn a_summary_within_the_tolerance_is_left_alone() {
    let text = "One two three. Four five six.";
    // 6 words against a target of 6: well inside 20%.
    assert_eq!(trim_to_target(text, 6), (text.to_string(), false));
    // The tolerance is +20%: 12 words against 10 (limit 12) still fit.
    let twelve = "one two three four five six seven eight nine ten eleven twelve";
    assert_eq!(trim_to_target(twelve, 10), (twelve.to_string(), false));
    // A target of zero means no limit.
    let long = "word. ".repeat(500);
    assert_eq!(trim_to_target(&long, 0).1, false);
}

#[test]
fn trimming_never_cuts_a_sentence_even_when_it_is_too_long() {
    let first = "This single sentence is deliberately much longer than the target and still must survive whole";
    let text = format!("{first}. A second sentence follows here.");
    let (trimmed, changed) = trim_to_target(&text, 5);
    assert!(changed);
    assert_eq!(trimmed, format!("{first}."));
    assert!(trimmed.ends_with('.'), "a full sentence survives");
}

#[test]
fn trimming_is_unicode_safe() {
    let text = "Mira trug einen Mantel — schwer und nass. Dann ging sie hinaus. Der Regen blieb.";
    let (trimmed, changed) = trim_to_target(text, 6);
    assert!(changed);
    assert!(text.starts_with(&trimmed));
    assert!(trimmed.ends_with('.'));
}

#[test]
fn the_merge_has_its_own_default_target() {
    let mut settings = AiSettings::default();
    assert_eq!(summary_merge_words(&settings), DEFAULT_SUMMARY_MERGE_WORDS);
    assert_eq!(DEFAULT_SUMMARY_MERGE_WORDS, 150);

    settings.prompts.merge_words = Some(220);
    assert_eq!(summary_merge_words(&settings), 220);

    settings.prompts.merge_words = Some(0);
    assert_eq!(summary_merge_words(&settings), 0, "zero is no target");

    // The two targets are independent.
    settings.prompts.summary.words = Some(70);
    assert_eq!(summary_block_words(&settings), 70);
    assert_eq!(summary_merge_words(&settings), 0);
}
