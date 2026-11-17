//! The short, story-scoped identifiers shown to the author.
//!
//! A story gets one four-character key (for example `A43K`). Its chapters are
//! `A43K-001`, `A43K-002`, ... and its lore entries `A43K-L001`, ... . These
//! keys are the identifiers; the old numeric ids are gone. They are internal
//! bookkeeping and are not shown in the interface.

use super::*;
use std::collections::HashSet;

/// Four characters give about one million combinations. That is far more than
/// a personal library needs, and the collision check below makes the final key
/// unique even when a hash repeats.
pub const STORY_KEY_LEN: usize = 4;

/// Crockford-style Base32 without the easily confused I, L, O and U.
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

pub fn is_story_key(key: &str) -> bool {
    key.len() == STORY_KEY_LEN && key.bytes().all(|byte| ALPHABET.contains(&byte))
}

/// One candidate from the title/creation seed. The salt is increased when the
/// candidate is already in use.
fn story_key(title: &str, seed: u64, salt: u64) -> String {
    let input = format!("{}\u{1f}{seed}\u{1f}{salt}", title.trim());
    let mut value = content_hash(&input) & 0xF_FFFF; // 20 bits
    let mut key = String::with_capacity(STORY_KEY_LEN);
    for _ in 0..STORY_KEY_LEN {
        key.push(ALPHABET[(value & 0x1F) as usize] as char);
        value >>= 5;
    }
    key
}

/// A key that is not in `used` yet. The salt makes same-title, same-time books
/// distinct without adding another dependency.
pub fn unique_story_key(title: &str, seed: u64, used: &HashSet<String>) -> String {
    for salt in 0..100_000u64 {
        let key = story_key(title, seed, salt);
        if !used.contains(&key) {
            return key;
        }
    }
    // This library is larger than the four-character space can comfortably
    // serve. Keep looking with a different seed rather than returning a
    // duplicate.
    for extra in 1..u64::MAX {
        let key = story_key(title, seed.wrapping_add(extra), extra);
        if !used.contains(&key) {
            return key;
        }
    }
    unreachable!("the four-character key space cannot be exhausted")
}

pub fn chapter_key(story_key: &str, number: u32) -> String {
    format!("{story_key}-{number:03}")
}

pub fn lore_key(story_key: &str, number: u32) -> String {
    format!("{story_key}-L{number:03}")
}

/// Give every story, chapter and lore entry a key. Existing valid keys are
/// kept; missing or duplicated ones are filled in order. Old files therefore
/// survive the change without losing their numeric ids or their content.
pub fn ensure_library_keys(library: &mut Library) {
    let mut used_story_keys = HashSet::new();
    for (index, story) in library.stories.iter_mut().enumerate() {
        let old_key = story.key.clone();
        if !is_story_key(&story.key) || used_story_keys.contains(&story.key) {
            // Old stories have no creation time; their position in the file is
            // a stable seed, so a half-finished migration produces the same
            // key again.
            let seed = if story.created_at == 0 {
                index as u64
            } else {
                story.created_at
            };
            story.key = unique_story_key(&story.title, seed, &used_story_keys);
        }
        used_story_keys.insert(story.key.clone());
        if !old_key.is_empty() && old_key != story.key {
            remap_item_keys(story, &old_key);
        }
        ensure_chapter_keys(story);
        ensure_lore_keys(story);
    }
}

fn remap_item_keys(story: &mut Story, old_key: &str) {
    for chapter in &mut story.chapters {
        if let Some(rest) = chapter.key.strip_prefix(&format!("{old_key}-")) {
            chapter.key = format!("{}-{rest}", story.key);
        }
    }
    for entry in &mut story.lore {
        if let Some(rest) = entry.key.strip_prefix(&format!("{old_key}-L")) {
            entry.key = format!("{}-L{rest}", story.key);
        }
    }
}

fn ensure_chapter_keys(story: &mut Story) {
    let prefix = format!("{}-", story.key);
    let mut used = HashSet::new();
    let mut keep = Vec::with_capacity(story.chapters.len());
    let mut next = story.next_chapter.max(1);

    for chapter in &story.chapters {
        let number = chapter
            .key
            .strip_prefix(&prefix)
            .and_then(|rest| rest.parse::<u32>().ok())
            .filter(|number| *number >= 1);
        if let Some(number) = number {
            if used.insert(chapter.key.clone()) {
                next = next.max(number.saturating_add(1));
                keep.push(true);
                continue;
            }
        }
        keep.push(false);
    }

    for (index, chapter) in story.chapters.iter_mut().enumerate() {
        if keep[index] {
            continue;
        }
        while used.contains(&chapter_key(&story.key, next)) {
            next = next.saturating_add(1);
        }
        chapter.key = chapter_key(&story.key, next);
        used.insert(chapter.key.clone());
        next = next.saturating_add(1);
    }
    story.next_chapter = next.max(1);
}

fn ensure_lore_keys(story: &mut Story) {
    let prefix = format!("{}-L", story.key);
    let mut used = HashSet::new();
    let mut keep = Vec::with_capacity(story.lore.len());
    let mut next = story.next_lore.max(1);

    for entry in &story.lore {
        let number = entry
            .key
            .strip_prefix(&prefix)
            .and_then(|rest| rest.parse::<u32>().ok())
            .filter(|number| *number >= 1);
        if let Some(number) = number {
            if used.insert(entry.key.clone()) {
                next = next.max(number.saturating_add(1));
                keep.push(true);
                continue;
            }
        }
        keep.push(false);
    }

    for (index, entry) in story.lore.iter_mut().enumerate() {
        if keep[index] {
            continue;
        }
        while used.contains(&lore_key(&story.key, next)) {
            next = next.saturating_add(1);
        }
        entry.key = lore_key(&story.key, next);
        used.insert(entry.key.clone());
        next = next.saturating_add(1);
    }
    story.next_lore = next.max(1);
}

/// Milliseconds since the Unix epoch, used as the creation seed for a new
/// story key. The collision check handles two books created in the same
/// millisecond.
pub fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}
