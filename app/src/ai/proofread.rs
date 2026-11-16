//! The paragraph-by-paragraph spelling and grammar pass.

use super::*;

/// The proofreader gets its own short system prompt. The full fiction prompt
/// made the local model answer the paragraph instead of correcting it.
pub const PROOFREAD_SYSTEM: &str = "You are a proofreader. Keep every word of the \
author's text as it is and correct only spelling, grammar, punctuation and \
capitalization. Return only the corrected paragraph — no preamble, no \
commentary, no special tokens.";

/// One paragraph, corrected in place. The prompt is deliberately narrow: a
/// proofreader, not a rewriter.
pub fn proofread_prompt(paragraph: &str) -> String {
    format!(
        "You are a careful proofreader. Correct only spelling, grammar, punctuation and capitalization in the paragraph below. Keep the author's words, voice, style and every fact exactly as they are; do not rewrite, shorten, expand, explain or add anything. Do not use special tokens or channel markers.\n\nPARAGRAPH:\n{}\n\nWrite the corrected paragraph now, with no preamble and no commentary.",
        paragraph.trim()
    )
}

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
    Ok(reply.text)
}
