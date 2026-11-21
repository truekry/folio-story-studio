//! How much of the context window goes to the prompt, the history and the reply.

use super::*;

/// Four characters per token is a rough but serviceable estimate for English
/// and German prose. Used only to keep a request inside the server's window.
pub(crate) fn estimated_tokens(text: &str) -> usize {
    text.chars().count() / 4 + 16
}

/// The share of the context window a request may fill before the conversation
/// is condensed. The remainder is headroom for the reply.
pub const CONTEXT_FILL_PERCENT: usize = 85;

/// Tokens the chapter's own conversation may take while prompt and system text
/// stay under 85% of the server's window. Without a known window nothing is
/// ever condensed.
pub fn history_budget(settings: &AiSettings, prompt: &str) -> usize {
    if settings.context_limit == 0 {
        return usize::MAX;
    }
    let fill = settings.context_limit * CONTEXT_FILL_PERCENT / 100;
    fill.saturating_sub(estimated_tokens(SYSTEM_PROMPT) + estimated_tokens(prompt))
}

pub(crate) fn history_tokens(history: &[ChatMessage]) -> usize {
    history
        .iter()
        .map(|message| message.text.chars().count() / 4 + 8)
        .sum()
}

/// One line for the prompt editor: how big a request is right now, and how much
/// of the endpoint's window it takes. Zero means no window was reported.
pub(crate) fn size_line(label: &str, tokens: usize, window: usize) -> String {
    if window == 0 {
        format!("{label} ≈ {tokens} tokens · context window not reported")
    } else {
        let percent = (tokens * 100 + window / 2) / window;
        format!("{label} ≈ {tokens} tokens · {percent} % of the {window} window")
    }
}

/// True when the request would eat into the headroom the reply needs. The same
/// line the conversation budget uses, so the colour and the condensing agree.
pub(crate) fn size_is_tight(tokens: usize, window: usize) -> bool {
    window > 0 && tokens * 100 / window >= CONTEXT_FILL_PERCENT
}

/// How many reply tokens a word target needs. Two tokens per word is generous
/// for English prose; the extra hundred is room to finish the last sentence
/// without letting the reply run to several times the target.
pub(crate) fn target_reply_tokens(words: u32) -> u32 {
    words.saturating_mul(2).saturating_add(100)
}

/// How many reply tokens to ask for. A word target turns into a token wish;
/// zero means no word limit at all — the whole room the endpoint's window
/// leaves. Without a reported window the global reply-token setting is the only
/// safe cap.
pub(crate) fn reply_token_wish(settings: &AiSettings, target_words: u32) -> u32 {
    if target_words == 0 {
        if settings.context_limit > 0 {
            u32::MAX
        } else {
            settings.max_tokens
        }
    } else {
        target_reply_tokens(target_words)
            .min(settings.max_tokens)
            .max(128)
    }
}

/// What a continued reply is asked, when the last one ran into the limit.
pub(crate) const CONTINUE_AFTER_LIMIT: &str = "Continue exactly where you stopped. Do not repeat anything you already wrote, do not add a heading or a comment, and finish the sentence you were in. Then stop.";

/// How many tokens the fixed part of a request (system text, memory, earlier
/// turns and the prompt itself) takes, by the same estimate the reply budget
/// uses. Kept separate so a continuation can subtract what it has already
/// added to the conversation.
pub(crate) fn prompt_tokens(prompt: &str, history: &[ChatMessage], memory: &str) -> usize {
    estimated_tokens(SYSTEM_PROMPT)
        + estimated_tokens(memory)
        + history_tokens(history)
        + estimated_tokens(prompt)
}

/// What the reported context window leaves for the reply, or `None` when the
/// endpoint did not report one.
pub(crate) fn reply_room(
    settings: &AiSettings,
    prompt: &str,
    history: &[ChatMessage],
    memory: &str,
) -> Option<usize> {
    (settings.context_limit > 0).then(|| {
        settings
            .context_limit
            .saturating_sub(prompt_tokens(prompt, history, memory))
    })
}

/// How many reply tokens fit next to this prompt and conversation. A server
/// that reported its context window gets a reply shortened so everything stays
/// inside it; a request that cannot fit at all is stopped with a useful
/// message instead of a server-side truncation.
pub(crate) fn reply_budget(
    settings: &AiSettings,
    prompt: &str,
    history: &[ChatMessage],
    memory: &str,
    wanted: u32,
) -> Result<u32, String> {
    if settings.context_limit == 0 {
        return Ok(wanted);
    }
    let used = prompt_tokens(prompt, history, memory);
    let room = settings.context_limit.saturating_sub(used);
    if room < 64 {
        return Err(format!(
            "This request needs about {used} tokens, but the endpoint's context window holds {} tokens. Shorten the chapter or lower the summary window.",
            settings.context_limit
        ));
    }
    Ok(wanted.min(room as u32).max(64))
}

pub(crate) fn messages_for(
    prompt: &str,
    history: &[ChatMessage],
    memory: &str,
    system: Option<&str>,
) -> Vec<Value> {
    let base = system.unwrap_or(SYSTEM_PROMPT).trim();
    let system = if memory.trim().is_empty() {
        base.to_string()
    } else {
        format!(
            "{base}\n\nEARLIER IN THIS CONVERSATION (condensed):\n{}",
            memory.trim()
        )
    };
    let mut messages = vec![json!({"role": "system", "content": system})];
    for message in history {
        let role = match message.role.as_str() {
            "user" => "user",
            "assistant" => "assistant",
            _ => continue,
        };
        if message.text.trim().is_empty() {
            continue;
        }
        messages.push(json!({"role": role, "content": message.text.trim()}));
    }
    messages.push(json!({"role": "user", "content": prompt}));
    messages
}

/// Did the endpoint say it stopped because the reply was too long? Servers name
/// this differently: OpenAI says "length", some say "max_tokens", a few only
/// set a flag.
pub(crate) fn was_truncated(value: &Value) -> bool {
    let choice = &value["choices"][0];
    let reason = choice["finish_reason"]
        .as_str()
        .or_else(|| choice["stop_reason"].as_str())
        .or_else(|| value["finish_reason"].as_str());
    if let Some(reason) = reason {
        let reason = reason.trim().to_ascii_lowercase();
        return matches!(reason.as_str(), "length" | "max_tokens" | "limit");
    }
    value["truncated"].as_bool().unwrap_or(false)
}
