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
fn blocks_follow_paragraph_boundaries_and_keep_every_word() {
    let paragraph = "word ".repeat(500);
    let body = format!("{paragraph}\n\n{paragraph}\n\n{paragraph}");
    let blocks = summary_blocks(&body, 1200);
    assert_eq!(
        blocks.len(),
        2,
        "1000 words fit, the third paragraph starts a new block"
    );
    for range in &blocks {
        assert!(words(&body[range.clone()]) <= 1200);
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
    let prompt = summary_merge_prompt(&combined);
    assert!(
        prompt.contains(combined.trim()),
        "the merge must see every part"
    );
}

#[test]
fn the_merge_prompt_asks_for_one_continuous_text() {
    let combined = "Mira opens the door. The boy gives her a key.";
    let prompt = summary_merge_prompt(combined);
    assert!(prompt.contains("one continuous past-tense, third-person summary"));
    assert!(prompt.contains("Remove repetition and seams"));
    assert!(prompt.contains("COMBINED BLOCK SUMMARIES:\nMira opens the door."));
    assert!(
        !prompt.contains("Length:"),
        "the word target belongs to each block, not to the merge: {prompt}"
    );
    assert!(prompt.contains("do not add themes"), "{prompt}");
}
