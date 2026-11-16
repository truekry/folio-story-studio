//! The sentences that explain what the AI sees and why a key matters.

use super::*;

/// One line saying what the AI partner can actually see in this chapter, so the
/// window setting never stays abstract.
pub(crate) fn context_caption(recap_chapters: usize, recent: usize) -> String {
    let mut parts: Vec<String> = Vec::new();
    if recap_chapters > 0 {
        parts.push(format!(
            "the story so far ({})",
            plural(recap_chapters, "chapter", "chapters")
        ));
    }
    if recent > 0 {
        parts.push(format!(
            "the last {}",
            plural(recent, "summary", "summaries")
        ));
    }
    if parts.is_empty() {
        return "No earlier chapter is summarized yet — this chapter and the lorebook travel alone."
            .to_string();
    }
    format!(
        "Sent with each request: {}, plus this chapter and the lorebook.",
        parts.join(" and ")
    )
}

/// How full this chapter's own conversation is and where it gets condensed,
/// so the memory hint above the messages has a number behind it.
pub(crate) fn conversation_line(used: usize, limit: usize) -> String {
    let fill = limit * crate::ai::CONTEXT_FILL_PERCENT / 100;
    format!(
        "This chapter's conversation: about {used} of {limit} tokens, condensed once it passes {fill} ({}%).",
        crate::ai::CONTEXT_FILL_PERCENT
    )
}

/// Says, in one line, why Folio is or is not using the demo writer. The old rule
/// quietly treated every address but localhost as a cloud service, which left
/// servers on the home network stranded.
pub(crate) fn endpoint_note(settings: &AiSettings) -> String {
    let base = settings.base_url.trim();
    if base.is_empty() {
        return "No address yet - the demo writer answers until you add one. Without a scheme, http:// is used for a server on your network and https:// for the internet."
            .to_string();
    }
    if settings.is_demo() {
        return "Looks like a cloud service, so it needs a key. A server on your own network works without one."
            .to_string();
    }
    if crate::model::is_private_address(base) {
        if base.starts_with("https://") {
            return "A server on your own network — plain HTTP is used, no key needed.".to_string();
        }
        "A server on your own network — no key needed.".to_string()
    } else {
        "This address is used with the key above.".to_string()
    }
}

/// What the endpoint said about its context window, or an honest "it did not
/// say". The number decides how much of a chapter travels at most.
pub(crate) fn context_line(limit: usize) -> String {
    if limit == 0 {
        return "Context window: not reported by the endpoint — long chapters are cut by character count."
            .to_string();
    }
    format!(
        "Context window: {limit} tokens, reported by the endpoint — the reply is trimmed so prompt and answer fit."
    )
}
