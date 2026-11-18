//! Lore suggestions: reading a proposal out of a reply and the offline demo proposal.

use super::*;

/// One parsed answer from the lorebook pass, resolved against the entries that
/// exist right now. `target` is Some when the facts belong to an entry.
#[derive(Clone, Debug, PartialEq)]
pub struct LoreSuggestion {
    pub target: Option<String>,
    pub target_name: String,
    pub name: String,
    pub category: String,
    pub aliases: String,
    pub triggers: String,
    pub text: String,
    /// Requested details the passage does not give (e.g. "clothes"). The
    /// assistant must name them instead of inventing them.
    pub missing: String,
    /// True when the author explicitly asked the assistant to invent this
    /// entry. Such facts are a proposal, not a record of the chapter.
    pub invented: bool,
    pub well_formed: bool,
}

/// Reads the labeled reply. Local models drift, so the parser is forgiving:
/// bold markers and list dashes are ignored, a line without a label after TEXT
/// is appended to the text, and a reply with no labels at all still becomes a
/// proposal the author can fix in the window.
pub fn parse_lore_reply(reply: &str, story: &Story) -> LoreSuggestion {
    parse_lore_reply_inner(reply, story, true)
}

/// The same shape for an invented entry. No target is resolved: invention
/// always proposes a new entry, even when the name echoes an existing one.
pub fn parse_invented_reply(reply: &str, story: &Story) -> LoreSuggestion {
    let mut suggestion = parse_lore_reply_inner(reply, story, false);
    suggestion.target = None;
    suggestion.invented = true;
    suggestion
}

fn parse_lore_reply_inner(reply: &str, story: &Story, resolve: bool) -> LoreSuggestion {
    let clean = reply.replace("**", "").replace('`', "");
    let mut target = String::new();
    let mut name = String::new();
    let mut category = String::new();
    let mut aliases = String::new();
    let mut triggers = String::new();
    let mut text = String::new();
    let mut missing = String::new();
    let mut in_text = false;
    let mut any_field = false;

    for raw in clean.lines() {
        let line = raw
            .trim()
            .trim_start_matches(['-', '*', '•', '#', ' '])
            .trim();
        let (key, value) = match line.split_once(':') {
            Some((key, value)) => (key.trim().to_ascii_lowercase(), unquote(value)),
            None => (String::new(), String::new()),
        };
        match key.as_str() {
            "target" => {
                target = value;
                in_text = false;
                any_field = true;
            }
            "name" => {
                name = value;
                in_text = false;
                any_field = true;
            }
            "category" => {
                category = value;
                in_text = false;
                any_field = true;
            }
            "aliases" | "alias" => {
                aliases = value;
                in_text = false;
                any_field = true;
            }
            "triggers" | "trigger" => {
                triggers = value;
                in_text = false;
                any_field = true;
            }
            "missing" | "not described" | "not_described" => {
                missing = value;
                in_text = false;
                any_field = true;
            }
            "text" | "description" => {
                text.push_str(&value);
                in_text = true;
                any_field = true;
            }
            _ if in_text => {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(line);
            }
            _ => {}
        }
    }

    if !any_field {
        text = clean.trim().to_string();
    }
    let text = text.trim().to_string();
    let mut name = name.trim().to_string();
    let mut target_name = target.trim().to_string();
    let wanted_new = target_name.is_empty()
        || target_name.eq_ignore_ascii_case("new")
        || target_name.eq_ignore_ascii_case("none");

    let mut target_id = None;
    if resolve && !wanted_new {
        target_id = find_lore(story, &target_name, &name);
    }
    if target_id.is_some() {
        if let Some(entry) = story
            .lore
            .iter()
            .find(|entry| Some(&entry.key) == target_id.as_ref())
        {
            // An update only adds the new facts; the entry's own name and
            // category stay the truth.
            target_name = entry.name.clone();
            name = entry.name.clone();
            if category.trim().is_empty() {
                category = entry.category.clone();
            }
        }
    } else if name.is_empty() && !wanted_new {
        // The model named an entry that no longer exists: offer it as a new
        // entry rather than losing the facts.
        name = target_name.trim_end_matches('.').to_string();
    }
    if target_name.is_empty() {
        target_name = name.clone();
    }

    LoreSuggestion {
        target: target_id,
        target_name,
        name,
        category: normalize_category(&category),
        aliases: aliases.trim().to_string(),
        triggers: triggers.trim().to_string(),
        text,
        missing: missing.trim().to_string(),
        invented: false,
        well_formed: any_field,
    }
}

/// Write a parsed lore proposal into the story. Returns the entry name and
/// whether a new entry was created. Updates only add facts; the entry's own
/// name and category stay the truth.
pub fn apply_lore_suggestion(
    story: &mut Story,
    suggestion: &LoreSuggestion,
) -> Option<(String, bool)> {
    let facts = suggestion.text.trim();
    if facts.is_empty() {
        return None;
    }
    if let Some(target) = suggestion.target.as_deref() {
        if let Some(entry) = story.lore.iter_mut().find(|entry| entry.key == target) {
            if !entry.description.trim().is_empty() {
                entry.description.push_str("\n\n");
            }
            entry.description.push_str(facts);
            merge_list(&mut entry.aliases, &suggestion.aliases);
            merge_list(&mut entry.triggers, &suggestion.triggers);
            return Some((entry.name.clone(), false));
        }
    }
    let name = suggestion.name.trim();
    if name.is_empty() {
        return None;
    }
    let category = match suggestion.category.trim() {
        "" => "Lore".to_string(),
        category => category.to_string(),
    };
    let number = story.next_lore.max(1);
    story.lore.push(LoreEntry {
        key: crate::model::lore_key(&story.key, number),
        name: name.to_string(),
        category,
        aliases: suggestion.aliases.trim().to_string(),
        triggers: suggestion.triggers.trim().to_string(),
        description: facts.to_string(),
        pinned: false,
        invented: suggestion.invented,
    });
    story.next_lore = number.saturating_add(1);
    Some((name.to_string(), true))
}

/// Add comma-separated items that are not already in the list, ignoring case
/// and stray spaces.
fn merge_list(existing: &mut String, extra: &str) {
    for item in extra.split(',') {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        if existing
            .split(',')
            .any(|known| known.trim().eq_ignore_ascii_case(item))
        {
            continue;
        }
        if !existing.trim().is_empty() {
            existing.push_str(", ");
        }
        existing.push_str(item);
    }
}

/// Offline version of the lorebook pass: uses the first known name in the
/// selection, or proposes a new entry the author can rename.
pub fn demo_lore_reply(story: &Story, passage: &str) -> String {
    let passage = passage.trim();
    let lower = passage.to_lowercase();
    let hit = story.lore.iter().find(|entry| {
        lower.contains(&entry.name.to_lowercase())
            || entry.aliases.split(',').any(|alias| {
                let alias = alias.trim().to_lowercase();
                !alias.is_empty() && lower.contains(&alias)
            })
    });
    let sentence = passage
        .split(['.', '!', '?'])
        .next()
        .unwrap_or(passage)
        .trim();
    let fact = if sentence.is_empty() {
        "Something in this passage belongs in the lorebook.".to_string()
    } else {
        format!("{sentence}.")
    };
    match hit {
        Some(entry) => format!(
            "TARGET: {}\nNAME: {}\nCATEGORY: {}\nTEXT: {}",
            entry.name, entry.name, entry.category, fact
        ),
        None => format!(
            "TARGET: NEW\nNAME: A detail from the page\nCATEGORY: Lore\nALIASES: \nTRIGGERS: \nTEXT: {}",
            fact
        ),
    }
}

pub(crate) fn find_lore(story: &Story, target: &str, name: &str) -> Option<String> {
    for candidate in [target, name] {
        let candidate = candidate.trim().trim_end_matches('.');
        if candidate.is_empty()
            || candidate.eq_ignore_ascii_case("new")
            || candidate.eq_ignore_ascii_case("none")
        {
            continue;
        }
        if let Some(entry) = story.lore.iter().find(|entry| {
            entry.name.eq_ignore_ascii_case(candidate)
                || entry
                    .aliases
                    .split(',')
                    .any(|alias| alias.trim().eq_ignore_ascii_case(candidate))
        }) {
            return Some(entry.key.clone());
        }
    }
    None
}

pub(crate) fn normalize_category(raw: &str) -> String {
    let raw = raw.trim().trim_end_matches('.');
    for known in ["Character", "Place", "Faction", "Item", "Lore"] {
        if raw.eq_ignore_ascii_case(known) {
            return known.to_string();
        }
    }
    let lower = raw.to_ascii_lowercase();
    if lower.contains("char") {
        "Character".to_string()
    } else if lower.contains("place") || lower.contains("location") {
        "Place".to_string()
    } else if lower.contains("faction") || lower.contains("group") {
        "Faction".to_string()
    } else if lower.contains("item") || lower.contains("object") {
        "Item".to_string()
    } else {
        "Lore".to_string()
    }
}

pub(crate) fn unquote(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim()
        .to_string()
}

#[cfg(test)]
mod apply_tests {
    use super::*;
    use crate::model::Library;

    fn suggestion(target: Option<String>, name: &str, text: &str, aliases: &str) -> LoreSuggestion {
        LoreSuggestion {
            target,
            target_name: name.to_string(),
            name: name.to_string(),
            category: "Character".to_string(),
            aliases: aliases.to_string(),
            triggers: "brass compass".to_string(),
            text: text.to_string(),
            missing: String::new(),
            invented: false,
            well_formed: true,
        }
    }

    #[test]
    fn an_update_only_adds_the_new_facts() {
        let mut library = Library::sample();
        let story = &mut library.stories[0];
        let mira = story
            .lore
            .iter()
            .find(|entry| entry.name == "Mira Vale")
            .expect("sample Mira")
            .clone();
        let before = mira.description.clone();
        let count = story.lore.len();
        let result = apply_lore_suggestion(
            story,
            &suggestion(
                Some(mira.key.clone()),
                "Mira Vale",
                "She keeps a brass compass on a leather cord.",
                "the cartographer",
            ),
        );
        assert_eq!(result, Some(("Mira Vale".to_string(), false)));
        let after = story
            .lore
            .iter()
            .find(|entry| entry.key == mira.key)
            .expect("still there");
        assert!(after.description.starts_with(&before));
        assert!(after.description.contains("brass compass"));
        assert_eq!(
            after.aliases.matches("the cartographer").count(),
            1,
            "an alias the entry already had must not be repeated"
        );
        assert!(after.triggers.contains("brass compass"));
        assert_eq!(story.lore.len(), count);
    }

    #[test]
    fn a_new_proposal_creates_an_entry_with_the_next_key() {
        let mut library = Library::sample();
        let story = &mut library.stories[0];
        let count = story.lore.len();
        let next = story.next_lore.max(1);
        let result = apply_lore_suggestion(
            story,
            &suggestion(None, "The Salt Widow", "A woman who counts the tides.", ""),
        );
        assert_eq!(result, Some(("The Salt Widow".to_string(), true)));
        assert_eq!(story.lore.len(), count + 1);
        assert_eq!(story.next_lore, next + 1);
        let created = story.lore.last().expect("new entry");
        assert_eq!(created.name, "The Salt Widow");
        assert_eq!(created.category, "Character");
        assert!(!created.key.is_empty());
    }

    #[test]
    fn an_invented_entry_is_stored_with_its_origin() {
        let mut library = Library::sample();
        let story = &mut library.stories[0];
        let count = story.lore.len();
        let mut proposal = suggestion(
            None,
            "Margit Hale",
            "Tim's mother, a night-shift nurse with a dry humour.",
            "",
        );
        proposal.invented = true;
        let result = apply_lore_suggestion(story, &proposal);
        assert_eq!(result, Some(("Margit Hale".to_string(), true)));
        assert_eq!(story.lore.len(), count + 1);
        let created = story.lore.last().expect("new entry");
        assert!(created.invented, "the entry must be marked as invented");
    }

    #[test]
    fn an_extracted_entry_stays_canon() {
        let mut library = Library::sample();
        let story = &mut library.stories[0];
        apply_lore_suggestion(
            story,
            &suggestion(None, "The Salt Widow", "A woman who counts the tides.", ""),
        );
        assert!(!story.lore.last().expect("new entry").invented);
    }

    #[test]
    fn an_invented_name_never_resolves_to_an_existing_entry() {
        let library = Library::sample();
        let story = &library.stories[0];
        let suggestion = parse_invented_reply(
            "TARGET: Mira Vale\nNAME: Mira Vale\nCATEGORY: Character\nTEXT: A new invented sister.",
            story,
        );
        assert!(suggestion.invented);
        assert_eq!(
            suggestion.target, None,
            "invention always creates a new entry"
        );
        assert_eq!(suggestion.name, "Mira Vale");
    }

    #[test]
    fn an_empty_proposal_changes_nothing() {
        let mut library = Library::sample();
        let story = &mut library.stories[0];
        assert_eq!(
            apply_lore_suggestion(story, &suggestion(None, "", "  ", "")),
            None
        );
        assert_eq!(
            apply_lore_suggestion(story, &suggestion(None, "Name only", "", "")),
            None
        );
    }
}
