//! Story keys and the chapter/lore keys derived from them.

use super::*;
use std::collections::HashSet;

fn chapter_number(key: &str, story_key: &str) -> Option<u32> {
    key.strip_prefix(&format!("{story_key}-"))?.parse().ok()
}

fn lore_number(key: &str, story_key: &str) -> Option<u32> {
    key.strip_prefix(&format!("{story_key}-L"))?.parse().ok()
}

#[test]
fn item_keys_use_the_story_key_and_three_digit_numbers() {
    assert_eq!(chapter_key("A43K", 1), "A43K-001");
    assert_eq!(chapter_key("A43K", 12), "A43K-012");
    assert_eq!(chapter_key("A43K", 1000), "A43K-1000");
    assert_eq!(lore_key("A43K", 1), "A43K-L001");
}

#[test]
fn sample_stories_get_one_short_key_and_matching_item_keys() {
    let library = Library::sample();
    let mut story_keys = HashSet::new();
    for story in &library.stories {
        assert!(
            is_story_key(&story.key),
            "invalid story key: {:?}",
            story.key
        );
        assert!(story_keys.insert(story.key.clone()), "duplicate story key");

        let mut chapters = HashSet::new();
        for chapter in &story.chapters {
            let number = chapter_number(&chapter.key, &story.key)
                .unwrap_or_else(|| panic!("bad chapter key: {:?}", chapter.key));
            assert!(number >= 1);
            assert!(chapters.insert(chapter.key.clone()));
        }

        let mut lore = HashSet::new();
        for entry in &story.lore {
            let number = lore_number(&entry.key, &story.key)
                .unwrap_or_else(|| panic!("bad lore key: {:?}", entry.key));
            assert!(number >= 1);
            assert!(lore.insert(entry.key.clone()));
        }
    }
}

#[test]
fn a_new_key_avoids_an_existing_one_even_for_the_same_title_and_time() {
    let mut used = HashSet::new();
    let first = unique_story_key("The Lantern Coast", 1_700_000_000_000, &used);
    used.insert(first.clone());
    let second = unique_story_key("The Lantern Coast", 1_700_000_000_000, &used);
    assert!(is_story_key(&first));
    assert!(is_story_key(&second));
    assert_ne!(first, second);
}

#[test]
fn old_stories_and_items_get_keys_without_losing_their_numbers() {
    let mut library = Library::sample();
    for story in &mut library.stories {
        story.key.clear();
        story.created_at = 0;
        story.next_chapter = 1;
        story.next_lore = 1;
        for chapter in &mut story.chapters {
            chapter.key.clear();
        }
        for entry in &mut story.lore {
            entry.key.clear();
        }
    }
    ensure_library_keys(&mut library);

    for story in &library.stories {
        assert!(is_story_key(&story.key));
        let chapter_numbers: Vec<u32> = story
            .chapters
            .iter()
            .map(|chapter| chapter_number(&chapter.key, &story.key).expect("chapter key"))
            .collect();
        assert_eq!(
            chapter_numbers,
            (1..=story.chapters.len() as u32).collect::<Vec<_>>()
        );
        let lore_numbers: Vec<u32> = story
            .lore
            .iter()
            .map(|entry| lore_number(&entry.key, &story.key).expect("lore key"))
            .collect();
        assert_eq!(
            lore_numbers,
            (1..=story.lore.len() as u32).collect::<Vec<_>>()
        );
        assert_eq!(
            story.next_chapter,
            story.chapters.len() as u32 + 1,
            "the next chapter number follows the highest used one"
        );
        assert_eq!(
            story.next_lore,
            story.lore.len() as u32 + 1,
            "the next lore number follows the highest used one"
        );
    }
}

#[test]
fn a_title_change_does_not_change_an_existing_story_key() {
    let mut library = Library::sample();
    let before = library.stories[0].key.clone();
    library.stories[0].title = "A Completely Different Title".to_string();
    ensure_library_keys(&mut library);
    assert_eq!(library.stories[0].key, before);
}

#[test]
fn a_key_with_the_wrong_shape_is_replaced_and_items_follow_it() {
    let mut library = Library::sample();
    let old = library.stories[0].key.clone();
    library.stories[0].key = "bad".to_string();
    ensure_library_keys(&mut library);
    let new = &library.stories[0].key;
    assert!(is_story_key(new));
    let _ = old;
    for chapter in &library.stories[0].chapters {
        assert!(
            chapter_number(&chapter.key, new).is_some(),
            "{:?}",
            chapter.key
        );
    }
    for entry in &library.stories[0].lore {
        assert!(lore_number(&entry.key, new).is_some(), "{:?}", entry.key);
    }
}
