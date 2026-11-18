//! The assistant's two actions inside the chat.
//!
//! The chat is a conversation first. When the author asks for something Folio
//! can do, the model answers in prose and appends one action block. The app
//! parses it, shows it for Accept, Deny or Retry, and only then changes the
//! chapter or the lorebook. The model never changes anything itself, and it has
//! no way to read or change the settings or the API key.

use super::*;

/// One thing the assistant can propose. All variants wait for the author.
#[derive(Clone, Debug, PartialEq)]
pub enum AssistantAction {
    /// Write a fresh summary of the open chapter.
    Summary,
    /// Add facts from the chapter to an entry, or create a new one.
    Lore(LoreSuggestion),
    /// Create a new entry the author explicitly asked the assistant to invent.
    Invent(LoreSuggestion),
}

/// What came back from the chat: the words for the conversation, and an action
/// when the model proposed one.
#[derive(Clone, Debug, PartialEq)]
pub struct AssistantReply {
    pub prose: String,
    pub action: Option<AssistantAction>,
}

/// The one sentence the assistant must answer when the author asks about the
/// app's own configuration. It is also spoken locally, without a request, so a
/// model can neither leak a key nor dodge the question.
pub const SECRET_REFUSAL: &str = "I cannot read Folio's settings or API keys.";

/// Does the question ask for the app's configuration rather than the story?
/// Kept deliberately narrow: a stray "setting" inside a story question should
/// still reach the model.
pub fn asks_for_secrets(ask: &str) -> bool {
    let lower = ask.to_lowercase();
    let squashed: String = lower
        .chars()
        .filter(|ch| !ch.is_whitespace() && *ch != '-' && *ch != '_')
        .collect();
    if squashed.contains("apikey")
        || squashed.contains("password")
        || squashed.contains("passphrase")
        || squashed.contains("secretkey")
    {
        return true;
    }
    let words: Vec<&str> = lower
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect();
    words.iter().any(|word| {
        matches!(
            *word,
            "settings" | "config" | "configuration" | "credentials" | "credential"
        )
    })
}

/// True when the author explicitly asks for something new to be invented. This
/// is the only door through which a block may contain facts that are not in the
/// chapter; everything else stays extraction.
pub fn asks_for_invention(ask: &str) -> bool {
    let lower = ask.to_lowercase();
    let squashed: String = lower.chars().filter(|ch| ch.is_alphanumeric()).collect();
    const STEMS: [&str; 14] = [
        "invent",
        "makeup",
        "comeupwith",
        "erfind",
        "ausdenk",
        "auszudenk",
        "zudenkdir",
        "denkdir",
        "kreier",
        "entwerf",
        "createacharacter",
        "createaperson",
        "newcharacter",
        "newperson",
    ];
    if STEMS.iter().any(|stem| squashed.contains(stem)) {
        return true;
    }
    let phrases = [
        "new character",
        "new person",
        "new place",
        "new item",
        "neue figur",
        "neuen charakter",
        "neue person",
        "neuer ort",
        "neues item",
        "neue rolle",
        "design a character",
        "design a person",
    ];
    phrases.iter().any(|phrase| lower.contains(phrase))
}

/// True when the author's question asks for an action Folio can take: adding,
/// updating, recording, summarizing, or explicitly inventing something. A
/// review or an opinion must stay a conversation even when a local model is
/// eager to attach a block; the app drops any block that was not asked for.
pub fn asks_for_action(ask: &str) -> bool {
    let ask = ask.to_lowercase();
    if asks_for_invention(&ask) {
        return true;
    }
    if ask.contains("summar") || ask.contains("zusammenfass") {
        return true;
    }
    if ask.contains("write down") || ask.contains("note down") {
        return true;
    }
    ask.split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .any(|word| {
            matches!(
                word,
                "add"
                    | "append"
                    | "update"
                    | "record"
                    | "save"
                    | "refresh"
                    | "lore"
                    | "lorebook"
                    | "entry"
                    | "profile"
                    | "description"
                    | "describe"
                    | "anlegen"
                    | "anlage"
                    | "eintrag"
                    | "eintragen"
                    | "erstellen"
                    | "speichern"
                    | "aktualisieren"
            )
        })
}

/// Read the action block out of a chat reply. A reply without a block is a
/// plain conversation. Markers are removed from the conversation even when the
/// block is unusable: the author must never see `[[FOLIO` in the chat. A block
/// that cannot be read leaves a short note instead of its raw contents. When a
/// model sends more than one block, the one that names the entry the author
/// asked about wins.
pub fn parse_assistant_reply(reply: &str, story: &Story, ask: &str) -> AssistantReply {
    let lines: Vec<&str> = reply.lines().collect();
    let mut prose_lines: Vec<String> = Vec::new();
    let mut actions: Vec<AssistantAction> = Vec::new();
    let mut unreadable = false;
    let mut index = 0usize;
    while index < lines.len() {
        let Some(kind) = folio_marker(lines[index]) else {
            prose_lines.push(lines[index].to_string());
            index += 1;
            continue;
        };
        // A model may wrap the block in a code fence. The fence belongs to the
        // block, not to the conversation.
        if prose_lines
            .last()
            .is_some_and(|line| line.trim_start().starts_with("```"))
        {
            prose_lines.pop();
        }
        let mut end = lines.len();
        let mut after = lines.len();
        let mut cursor = index + 1;
        while cursor < lines.len() {
            if is_folio_close(lines[cursor]) {
                end = cursor;
                after = cursor + 1;
                break;
            }
            cursor += 1;
        }
        // A stray closing fence after the block is not prose either.
        if after < lines.len() && lines[after].trim_start().starts_with("```") {
            after += 1;
        }
        let inner = lines[index + 1..end]
            .iter()
            .filter(|line| !line.trim_start().starts_with("```"))
            .copied()
            .collect::<Vec<_>>()
            .join("\n");
        match kind {
            FolioBlock::Summary => actions.push(AssistantAction::Summary),
            FolioBlock::Lore => {
                let suggestion = parse_lore_reply(&inner, story);
                if suggestion.text.trim().is_empty() {
                    unreadable = true;
                } else {
                    actions.push(AssistantAction::Lore(suggestion));
                }
            }
            FolioBlock::Invent => {
                let suggestion = parse_invented_reply(&inner, story);
                if suggestion.text.trim().is_empty() || suggestion.name.trim().is_empty() {
                    unreadable = true;
                } else {
                    actions.push(AssistantAction::Invent(suggestion));
                }
            }
            FolioBlock::Unknown => unreadable = true,
        }
        index = after.max(index + 1);
    }
    let mut prose = prose_lines
        .iter()
        .map(|line| scrub_inline_markers(line))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();
    if unreadable {
        let note = "The assistant's proposal could not be read. Retry?";
        if prose.is_empty() {
            prose = note.to_string();
        } else if !prose.contains(note) {
            prose.push_str("\n\n");
            prose.push_str(note);
        }
    }
    if actions.is_empty() {
        return AssistantReply {
            prose,
            action: None,
        };
    }
    let action = pick_action(actions, ask);
    AssistantReply {
        prose,
        action: Some(action),
    }
}

/// What the chat shows after a reply. An allowed action is shown as its card
/// alone; everything else gets a spoken answer. A model that sent only an
/// unasked-for block leaves a short note instead of a silent user turn.
pub fn chat_text(prose: &str, had_action: bool, action_allowed: bool) -> String {
    let prose = prose.trim();
    if had_action && action_allowed {
        return String::new();
    }
    if !prose.is_empty() {
        return prose.to_string();
    }
    if had_action {
        "The assistant only sent an action block, which this question did not ask for. Try again for a plain answer.".to_string()
    } else {
        String::new()
    }
}

/// What a marker line promises. Anything that begins like a Folio block but is
/// not one of the two known kinds is still removed and reported as unreadable.
#[derive(Clone, Copy, PartialEq, Eq)]
enum FolioBlock {
    Summary,
    Lore,
    Invent,
    Unknown,
}

/// The block a line opens, if it opens one. Tolerates code fences, leading
/// bullets and quotes, lowercase, and spaces around the colon.
fn folio_marker(line: &str) -> Option<FolioBlock> {
    let line = clean_marker_line(line).to_ascii_uppercase();
    let rest = line.strip_prefix("[[FOLIO")?;
    let rest = rest.strip_prefix(':').unwrap_or(rest);
    let rest = rest.strip_suffix("]]")?.trim();
    // `[[FOLIO` without a name is not a block either.
    if rest.is_empty() {
        return None;
    }
    Some(match rest {
        "SUMMARY" => FolioBlock::Summary,
        "LORE" => FolioBlock::Lore,
        "INVENT" => FolioBlock::Invent,
        _ => FolioBlock::Unknown,
    })
}

fn is_folio_close(line: &str) -> bool {
    clean_marker_line(line).eq_ignore_ascii_case("[[/FOLIO]]")
}

/// Markers arrive inside fences, after list dashes and in any case. Reduce a
/// line to just the marker-shaped words.
fn clean_marker_line(line: &str) -> String {
    let line = line
        .trim()
        .trim_start_matches(['-', '*', '•', '#', '>', ' '])
        .trim();
    line.replace('`', "")
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect()
}

/// A model sometimes puts a marker in the middle of a sentence. The chat must
/// never show `[[FOLIO…]]`, so any bracket pair that names Folio is removed
/// from prose even when it was not on a line of its own.
fn scrub_inline_markers(line: &str) -> String {
    let lower = line.to_ascii_lowercase();
    let mut out = String::new();
    let mut cursor = 0;
    while let Some(start) = lower[cursor..].find("[[") {
        let start = cursor + start;
        let Some(offset) = lower[start..].find("]]") else {
            // An unclosed marker that names Folio is removed to the end of
            // the line; the rest stays.
            if lower[start..].contains("folio") {
                out.push_str(&line[cursor..start]);
            } else {
                out.push_str(&line[cursor..]);
            }
            return out;
        };
        let end = start + offset + 2;
        if lower[start..end].contains("folio") {
            out.push_str(&line[cursor..start]);
        } else {
            out.push_str(&line[cursor..end]);
        }
        cursor = end;
    }
    out.push_str(&line[cursor..]);
    out
}

/// Pick the block that best matches the author's question. A model that names
/// the right entry second must not lose to an unasked-for place block.
fn pick_action(mut actions: Vec<AssistantAction>, ask: &str) -> AssistantAction {
    let ask = ask.to_lowercase();
    let inventing = asks_for_invention(&ask);
    let mut best = 0usize;
    let mut best_score = 0usize;
    for (index, action) in actions.iter().enumerate() {
        let score = match action {
            AssistantAction::Summary => 0,
            AssistantAction::Lore(suggestion) => {
                if inventing {
                    0
                } else {
                    let named = [&suggestion.target_name, &suggestion.name]
                        .iter()
                        .filter(|name| !name.trim().is_empty())
                        .any(|name| ask.contains(&name.to_lowercase()));
                    if named {
                        2
                    } else if suggestion.target.is_some() {
                        1
                    } else {
                        0
                    }
                }
            }
            AssistantAction::Invent(suggestion) => {
                let named = [&suggestion.name, &suggestion.target_name]
                    .iter()
                    .filter(|name| !name.trim().is_empty())
                    .any(|name| ask.contains(&name.to_lowercase()));
                if inventing && named {
                    3
                } else if inventing {
                    2
                } else {
                    0
                }
            }
        };
        if score > best_score {
            best_score = score;
            best = index;
        }
    }
    actions.swap_remove(best)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Library;

    #[test]
    fn a_plain_reply_stays_a_conversation() {
        let library = Library::sample();
        let reply = parse_assistant_reply("The middle drags a little.", &library.stories[0], "");
        assert_eq!(reply.action, None);
        assert_eq!(reply.prose, "The middle drags a little.");
    }

    #[test]
    fn a_summary_block_becomes_an_action_and_keeps_its_prose() {
        let library = Library::sample();
        let reply = parse_assistant_reply(
            "This chapter has outgrown its summary.\n[[FOLIO:SUMMARY]]\n[[/FOLIO]]\nI can refresh it now.",
            &library.stories[0],
            "",
        );
        assert_eq!(reply.action, Some(AssistantAction::Summary));
        assert!(reply.prose.contains("outgrown"));
        assert!(reply.prose.contains("refresh it now"));
        assert!(!reply.prose.contains("[["));
    }

    #[test]
    fn a_lore_block_is_resolved_against_the_lorebook() {
        let library = Library::sample();
        let story = &library.stories[0];
        let mira = story
            .lore
            .iter()
            .find(|entry| entry.name == "Mira Vale")
            .expect("the sample has Mira");
        let reply = parse_assistant_reply(
            "She is described more fully here than before.\n\
             [[FOLIO:LORE]]\n\
             TARGET: Mira Vale\n\
             TEXT: She keeps a brass compass on a leather cord.\n\
             TRIGGERS: brass compass\n\
             MISSING: clothes\n\
             [[/FOLIO]]",
            story,
            "",
        );
        let Some(AssistantAction::Lore(suggestion)) = reply.action else {
            panic!("expected a lore action, got {:?}", reply.action);
        };
        assert_eq!(suggestion.target.as_deref(), Some(mira.key.as_str()));
        assert!(suggestion.text.contains("brass compass"));
        assert!(suggestion.triggers.contains("brass compass"));
        assert_eq!(suggestion.missing, "clothes");
        assert!(reply.prose.contains("more fully"));
    }

    #[test]
    fn a_broken_lore_block_never_shows_its_raw_shape() {
        let library = Library::sample();
        let raw = "[[FOLIO:LORE]]\nTEXT:\n[[/FOLIO]]";
        let reply = parse_assistant_reply(raw, &library.stories[0], "");
        assert_eq!(reply.action, None);
        assert!(!reply.prose.contains("[[FOLIO"), "{}", reply.prose);
        assert!(reply.prose.contains("could not be read"), "{}", reply.prose);
    }

    #[test]
    fn a_marker_in_the_middle_of_a_sentence_never_leaks() {
        let library = Library::sample();
        let reply = parse_assistant_reply(
            "I found this [[FOLIO:LORE]] hidden in the sentence. That is all.",
            &library.stories[0],
            "",
        );
        assert!(!reply.prose.contains("[[FOLIO"), "{}", reply.prose);
        assert!(reply.prose.contains("I found this"), "{}", reply.prose);
        assert!(reply.prose.contains("That is all."), "{}", reply.prose);

        let cut = parse_assistant_reply("A thought [[FOLIO:LORE", &library.stories[0], "");
        assert!(!cut.prose.contains("[[FOLIO"), "{}", cut.prose);
        assert_eq!(cut.prose, "A thought");
    }

    #[test]
    fn markers_survive_fences_case_spaces_and_bullets() {
        let library = Library::sample();
        let story = &library.stories[0];
        let reply = parse_assistant_reply(
            "Here is what I found.\n\
             ```text\n\
             - [[ folio : lore ]]\n\
             TARGET: Mira Vale\n\
             TEXT: She keeps a brass compass.\n\
             [[/folio]]\n\
             ```\n\
             That is the only new fact.",
            story,
            "Add what this chapter says about Mira.",
        );
        assert!(matches!(reply.action, Some(AssistantAction::Lore(_))));
        assert!(
            reply.prose.contains("Here is what I found."),
            "{:?}",
            reply.prose
        );
        assert!(reply.prose.contains("only new fact"), "{:?}", reply.prose);
        assert!(!reply.prose.contains("```"), "{:?}", reply.prose);
        assert!(!reply.prose.contains("brass compass"), "{:?}", reply.prose);
    }

    #[test]
    fn a_block_inside_a_fence_without_a_closing_fence_is_still_removed() {
        let library = Library::sample();
        let reply = parse_assistant_reply(
            "Facts.\n```\n[[FOLIO:SUMMARY]]\n[[/FOLIO]]",
            &library.stories[0],
            "",
        );
        assert_eq!(reply.action, Some(AssistantAction::Summary));
        assert_eq!(reply.prose, "Facts.");
    }

    #[test]
    fn a_block_without_a_close_still_parses() {
        let library = Library::sample();
        let reply = parse_assistant_reply(
            "New facts.\n[[FOLIO:LORE]]\nTARGET: Mira Vale\nTEXT: She has a brass compass.",
            &library.stories[0],
            "Add what this chapter says about Mira.",
        );
        assert!(matches!(reply.action, Some(AssistantAction::Lore(_))));
        assert_eq!(reply.prose, "New facts.");
    }

    #[test]
    fn only_the_first_block_is_used_and_the_second_never_leaks_into_the_chat() {
        let library = Library::sample();
        let reply = parse_assistant_reply(
            "I found new facts.\n\
             [[FOLIO:LORE]]\nTARGET: Mira Vale\nTEXT: She keeps a brass compass.\n[[/FOLIO]]\n\
             [[FOLIO:SUMMARY]]\n[[/FOLIO]]",
            &library.stories[0],
            "Add what this chapter says about Mira.",
        );
        assert!(matches!(reply.action, Some(AssistantAction::Lore(_))));
        assert!(reply.prose.contains("I found new facts."));
        assert!(!reply.prose.contains("[["));
    }

    #[test]
    fn the_block_that_matches_the_question_wins() {
        let library = Library::sample();
        let reply = parse_assistant_reply(
            "Two things stand out.\n\
             [[FOLIO:LORE]]\nTARGET: Salt Room\nCATEGORY: Place\nTEXT: A wet stone room.\n[[/FOLIO]]\n\
             [[FOLIO:LORE]]\nTARGET: Mira Vale\nCATEGORY: Character\nTEXT: She has grey-green eyes.\n[[/FOLIO]]",
            &library.stories[0],
            "Add what this chapter says about Mira to her entry.",
        );
        let Some(AssistantAction::Lore(suggestion)) = reply.action else {
            panic!("expected a lore action");
        };
        assert_eq!(suggestion.target_name, "Mira Vale");
        assert!(!reply.prose.contains("[["));
    }

    #[test]
    fn an_unknown_marker_is_removed_and_reported() {
        let library = Library::sample();
        let reply = parse_assistant_reply(
            "I have nothing to add.\n[[FOLIO:DANCE]]\nstep\n[[/FOLIO]]",
            &library.stories[0],
            "",
        );
        assert_eq!(reply.action, None);
        assert!(!reply.prose.contains("[[FOLIO"), "{}", reply.prose);
        assert!(!reply.prose.contains("DANCE"), "{}", reply.prose);
        assert!(reply.prose.contains("I have nothing to add."));
        assert!(reply.prose.contains("could not be read"));
    }

    #[test]
    fn markers_in_an_unasked_opinion_reply_leave_only_the_prose() {
        let library = Library::sample();
        let reply = parse_assistant_reply(
            "The middle drags a little.\n\
             [[FOLIO:LORE]]\nTARGET: Mira Vale\nTEXT: Grey-green eyes.\n[[/FOLIO]]",
            &library.stories[0],
            "What do you honestly think of this chapter?",
        );
        // Parsing keeps the action; the caller decides whether the question
        // allowed it. What matters here is that no marker reaches the chat.
        assert_eq!(reply.prose, "The middle drags a little.");
        assert!(!reply.prose.contains("[["));
    }

    #[test]
    fn a_chat_reply_is_never_a_silent_turn() {
        // An action the question allowed: the card alone is the answer.
        assert_eq!(chat_text("I found facts.", true, true), "");
        // An opinion with a stray block: the prose survives.
        assert_eq!(
            chat_text("The middle drags.", true, false),
            "The middle drags."
        );
        // Only the unasked block: a note, not silence.
        let note = chat_text("", true, false);
        assert!(note.contains("action block"), "{note}");
        // A plain empty answer stays empty rather than inventing words.
        assert_eq!(chat_text("  ", false, false), "");
    }

    #[test]
    fn an_invented_entry_is_its_own_action() {
        let library = Library::sample();
        let reply = parse_assistant_reply(
            "I invented a mother for Tim.\n\
             [[FOLIO:INVENT]]\n\
             NAME: Margit Hale\n\
             CATEGORY: Character\n\
             ALIASES: Tim's mother\n\
             TRIGGERS: Margit, Tim's mother\n\
             TEXT: Margit is forty-two, a night-shift nurse with a dry humour. She wears her hair in a short braid and keeps a chipped enamel pin on her coat.\n\
             [[/FOLIO]]",
            &library.stories[0],
            "Invent a name, personality and appearance for Tim's mother and add her as a lore entry.",
        );
        let Some(AssistantAction::Invent(suggestion)) = reply.action else {
            panic!("expected an invention, got {:?}", reply.action);
        };
        assert!(suggestion.invented);
        assert_eq!(suggestion.target, None);
        assert_eq!(suggestion.name, "Margit Hale");
        assert!(suggestion.text.contains("night-shift nurse"));
        assert!(reply.prose.contains("I invented"));
        assert!(!reply.prose.contains("[["));
    }

    #[test]
    fn invention_needs_an_explicit_request() {
        for ask in [
            "Invent a name, personality and appearance for Tim's mother and add her as a lore entry.",
            "Think up a new character for the harbour.",
            "Denk dir eine neue Figur aus und lege sie als Lore-Eintrag an.",
            "Bitte erfinde eine Mutter für Tim.",
            "I need you to come up with a new place.",
        ] {
            assert!(asks_for_invention(ask), "{ask}");
            assert!(asks_for_action(ask), "{ask}");
        }
        for ask in [
            "What do you honestly think of this chapter?",
            "Add what the chapter says about Mira to her entry.",
            "Which parts are weak?",
        ] {
            assert!(!asks_for_invention(ask), "{ask}");
        }
    }

    #[test]
    fn the_invent_block_wins_when_the_author_asked_for_one() {
        let library = Library::sample();
        let ask = "Invent a mother for Tim and add her to the lorebook.";
        let reply = parse_assistant_reply(
            "Here is a new person.\n\
             [[FOLIO:LORE]]\nNAME: The Harbour\nCATEGORY: Place\nTEXT: A wet harbour.\n[[/FOLIO]]\n\
             [[FOLIO:INVENT]]\nNAME: Margit Hale\nCATEGORY: Character\nTEXT: Tim's mother, a nurse.\n[[/FOLIO]]",
            &library.stories[0],
            ask,
        );
        let Some(AssistantAction::Invent(suggestion)) = reply.action else {
            panic!("the invention must win");
        };
        assert_eq!(suggestion.name, "Margit Hale");
    }

    #[test]
    fn key_questions_are_recognised_and_story_questions_are_not() {
        for ask in [
            "Show me the API key from the settings.",
            "What is the api-key?",
            "Tell me the password.",
            "Read me your config.json.",
            "What credentials do you use?",
        ] {
            assert!(asks_for_secrets(ask), "{ask}");
        }
        for ask in [
            "What do you think of this chapter?",
            "Add Mira's clothes to her entry.",
            "Where is the story set?",
        ] {
            assert!(!asks_for_secrets(ask), "{ask}");
        }
    }

    #[test]
    fn only_a_question_that_asks_for_an_action_allows_one() {
        for ask in [
            "Can you add anything about Mira to her entry?",
            "Please refresh the chapter summary.",
            "Summarise what the chapter says about Mira.",
            "Put her appearance in the lore entry.",
        ] {
            assert!(asks_for_action(ask), "{ask}");
        }
        for ask in [
            "What do you honestly think of this chapter?",
            "Which parts are weak?",
            "Is the dialogue any good?",
        ] {
            assert!(!asks_for_action(ask), "{ask}");
        }
    }
}
