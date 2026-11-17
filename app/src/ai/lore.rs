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
    pub well_formed: bool,
}

/// Reads the labeled reply. Local models drift, so the parser is forgiving:
/// bold markers and list dashes are ignored, a line without a label after TEXT
/// is appended to the text, and a reply with no labels at all still becomes a
/// proposal the author can fix in the window.
pub fn parse_lore_reply(reply: &str, story: &Story) -> LoreSuggestion {
    let clean = reply.replace("**", "").replace('`', "");
    let mut target = String::new();
    let mut name = String::new();
    let mut category = String::new();
    let mut aliases = String::new();
    let mut triggers = String::new();
    let mut text = String::new();
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
    if !wanted_new {
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
        well_formed: any_field,
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
