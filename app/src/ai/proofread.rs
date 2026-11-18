//! The paragraph-by-paragraph spelling and grammar pass.
//!
//! A local model that is asked to hand back a whole corrected paragraph often
//! answers with advice, a summary or a rewritten scene instead. That reply
//! would replace the author's words wholesale. Folio therefore asks for the
//! smallest exact corrections (`OLD`/`NEW` pairs) and applies only edits whose
//! old words really appear in the paragraph.

use super::*;

/// The proofreader gets its own short system prompt. The full fiction prompt
/// made the local model answer the paragraph instead of correcting it.
pub const PROOFREAD_SYSTEM: &str =
    "You are a copy editor. You never rewrite and you never comment. \
You answer only with exact OLD/NEW pairs or the single word NONE.";

/// One exact correction: the words as written, and the words as they should be.
#[derive(Clone, Debug, PartialEq)]
pub struct Correction {
    pub old: String,
    pub new: String,
}

/// Ask for corrections, not for a rewritten paragraph. Keeping the paragraph
/// as data and the imperative last mirrors the sandwich that works for prose.
pub fn proofread_prompt(paragraph: &str) -> String {
    format!(
        "Correct only spelling, grammar, punctuation and capitalization in the paragraph below. Do not rewrite, shorten, expand, summarize or comment. For each error, copy the exact wrong words as OLD and the corrected words as NEW. Leave out anything that is already correct. If there are no errors, answer NONE.\n\nPARAGRAPH:\n{}\n\nAnswer in exactly this shape, one pair per error:\nOLD: <exact words from the paragraph>\nNEW: <corrected words>\n\nList the corrections now. If there are none, answer NONE.",
        paragraph.trim()
    )
}

/// Read the OLD/NEW pairs out of a reply. Tolerates bullets, bold markers and
/// several pairs in one block; ignores anything that is not a pair.
pub fn parse_corrections(reply: &str) -> Vec<Correction> {
    let mut out = Vec::new();
    let mut pending: Option<String> = None;
    for raw in reply.lines() {
        let line = raw
            .trim()
            .trim_start_matches(['-', '*', '•', '#', '>', '`', ' '])
            .trim();
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim().trim_matches('*').to_ascii_lowercase();
        let value = clean_value(value);
        match key.as_str() {
            "old" => {
                // A model may write `OLD: begun  NEW: began` on one line.
                if let Some((old, new)) = split_inline_pair(&value) {
                    out.push(Correction { old, new });
                    pending = None;
                } else {
                    pending = Some(value);
                }
            }
            "new" => {
                if let Some(old) = pending.take() {
                    if !old.is_empty() && !value.is_empty() {
                        out.push(Correction { old, new: value });
                    }
                }
            }
            _ => {}
        }
    }
    out
}

fn clean_value(value: &str) -> String {
    value
        .trim()
        .trim_matches('*')
        .trim()
        .trim_matches('"')
        .trim_matches('`')
        .trim()
        .to_string()
}

fn split_inline_pair(value: &str) -> Option<(String, String)> {
    let lower = value.to_ascii_lowercase();
    let at = lower.find("new:")?;
    let old = clean_value(&value[..at]);
    let new = clean_value(&value[at + 4..]);
    (!old.is_empty() && !new.is_empty()).then_some((old, new))
}

/// Apply the corrections the reply actually contains. A pair is used only when
/// its OLD text appears verbatim in the paragraph and the NEW text is a small,
/// lowercase fix of the same words. Everything else is ignored, so a model that
/// drifts into advice, a summary or a name change leaves the paragraph alone.
pub fn apply_proofread_reply(paragraph: &str, reply: &str) -> String {
    let mut text = paragraph.to_string();
    let mut corrections = parse_corrections(reply);
    // Apply the longest exact phrase first: a model often sends both the whole
    // corrected sentence and its parts. The parts are then already gone.
    corrections.sort_by_key(|correction| std::cmp::Reverse(correction.old.chars().count()));
    for correction in corrections {
        if correction.old == correction.new
            || correction.old.is_empty()
            || correction.new.is_empty()
        {
            continue;
        }
        // A proofreading pair may change a word or two, never a paragraph and
        // never a name. This stops "survey office" → "software office" and
        // "Cray" → "Cary" as surely as it stops a full rewrite.
        if !minimal_edit(&correction.old, &correction.new) {
            continue;
        }
        if !text.contains(&correction.old) {
            continue;
        }
        text = text.replacen(&correction.old, &correction.new, 1);
    }
    text
}

/// True when `new` is the same words as `old` with at most two small,
/// lowercase fixes. Proper nouns are never "corrected": a model that turns
/// Cray into Cary is not proofreading. Punctuation-only changes are not
/// applied either; the author's commas are not the model's business.
fn minimal_edit(old: &str, new: &str) -> bool {
    fn words(text: &str) -> Vec<String> {
        text.split(|ch: char| !ch.is_alphanumeric())
            .filter(|word| !word.is_empty())
            .map(|word| word.to_string())
            .collect()
    }
    let old_words = words(old);
    let new_words = words(new);
    if old_words.len() != new_words.len() || old_words.is_empty() {
        return false;
    }
    let mut changed = 0usize;
    for (before, after) in old_words.iter().zip(new_words.iter()) {
        if before == after {
            continue;
        }
        // Proper nouns and sentence-initial capitals stay untouched.
        if before.chars().any(|ch| ch.is_uppercase()) || after.chars().any(|ch| ch.is_uppercase()) {
            return false;
        }
        let before = before.to_lowercase();
        let after = after.to_lowercase();
        if before == after {
            continue;
        }
        if before.chars().count().abs_diff(after.chars().count()) > 1 {
            return false;
        }
        if edit_distance(&before, &after) > 2 {
            return false;
        }
        changed += 1;
        if changed > 2 {
            return false;
        }
    }
    changed > 0
}

/// A small Levenshtein distance. Proofreading fixes are tiny; a whole-word
/// substitution of a different length is already rejected before this runs.
fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    let mut current = vec![0usize; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        current[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            current[j + 1] = (previous[j] + cost)
                .min(previous[j + 1] + 1)
                .min(current[j] + 1);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[b.len()]
}

/// One paragraph, corrected in place. The reply is a list of exact edits; the
/// app never pastes a whole generated paragraph over the author's words.
pub fn proofread_paragraph(
    settings: &AiSettings,
    paragraph: &str,
    cancel: &AtomicBool,
) -> Result<String, String> {
    let prompt = proofread_prompt(paragraph);
    let words = paragraph.split_whitespace().count().max(40) as u32 + 40;
    let reply = stream_with_target(
        settings,
        &prompt,
        &[],
        "",
        Some(PROOFREAD_SYSTEM),
        words,
        false,
        cancel,
        &mut |_: &str| {},
    )?;
    Ok(apply_proofread_reply(paragraph, &reply.text))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PARAGRAPH: &str =
        "Mira sat at the table and begun, slowly, to drew the map. She did not made it beautiful.";

    #[test]
    fn exact_pairs_are_read_and_applied() {
        let reply = "OLD: begun\nNEW: began\n\nOLD: to drew\nNEW: to draw\n\nOLD: did not made\nNEW: did not make";
        let fixed = apply_proofread_reply(PARAGRAPH, reply);
        assert_eq!(
            fixed,
            "Mira sat at the table and began, slowly, to draw the map. She did not make it beautiful."
        );
    }

    #[test]
    fn bullets_bold_and_one_line_pairs_are_read_too() {
        let reply = "- **OLD:** begun **NEW:** began\n* OLD: to drew NEW: to draw";
        let corrections = parse_corrections(reply);
        assert_eq!(corrections.len(), 2, "{corrections:?}");
        assert_eq!(corrections[0].old, "begun");
        assert_eq!(corrections[1].new, "to draw");
    }

    #[test]
    fn an_essay_or_a_summary_changes_nothing() {
        let reply =
            "Here is a summary of the key points.\n\n### Structure\nUse shorter paragraphs.";
        assert_eq!(apply_proofread_reply(PARAGRAPH, reply), PARAGRAPH);
        assert_eq!(apply_proofread_reply(PARAGRAPH, "NONE"), PARAGRAPH);
    }

    #[test]
    fn a_rewrite_hidden_in_a_pair_is_rejected() {
        let old = "Mira sat at the table and begun, slowly, to drew the map. She did not made it beautiful.";
        let new = "A cartographer works at dawn, tracing coastlines with a steady hand while the harbour wakes.";
        let reply = format!("OLD: {old}\nNEW: {new}");
        assert_eq!(apply_proofread_reply(PARAGRAPH, &reply), PARAGRAPH);
    }

    #[test]
    fn a_pair_that_is_not_in_the_paragraph_is_ignored() {
        let reply = "OLD: the sea was green\nNEW: the sea was grey";
        assert_eq!(apply_proofread_reply(PARAGRAPH, reply), PARAGRAPH);
        // A no-op pair changes nothing either.
        let reply = "OLD: begun\nNEW: begun";
        assert_eq!(apply_proofread_reply(PARAGRAPH, reply), PARAGRAPH);
    }

    #[test]
    fn names_and_invented_words_are_never_corrected() {
        let text = "She thought about the survey office in Cray and the marigolds.";
        for (old, new) in [
            ("survey office", "software office"),
            ("Cray", "Cary"),
            ("marigolds", "amaryllis"),
            ("cut out", "cutout"),
        ] {
            let reply = format!("OLD: {old}\nNEW: {new}");
            assert_eq!(apply_proofread_reply(text, &reply), text, "{old} -> {new}");
        }
    }

    #[test]
    fn a_legitimate_plural_and_a_typo_still_pass() {
        let text = "She stood at the top of the stair and begun to climb.";
        let reply = "OLD: stair\nNEW: stairs\nOLD: begun\nNEW: began";
        let fixed = apply_proofread_reply(text, reply);
        assert!(fixed.contains("the stairs"));
        assert!(fixed.contains("began to climb"));
    }

    #[test]
    fn punctuation_only_changes_are_left_to_the_author() {
        let text = "Mira walked to the harbour and the sea was quiet.";
        let reply = "OLD: harbour and the sea\nNEW: harbour, and the sea";
        assert_eq!(apply_proofread_reply(text, reply), text);
    }

    #[test]
    fn a_long_pair_and_its_shorter_parts_do_not_apply_twice() {
        let reply =
            "OLD: begun\nNEW: began\nOLD: begun, slowly, to drew\nNEW: began, slowly, to draw";
        let fixed = apply_proofread_reply(PARAGRAPH, reply);
        assert_eq!(fixed.matches("began").count(), 1);
        assert!(fixed.contains("began, slowly, to draw"));
    }
}
