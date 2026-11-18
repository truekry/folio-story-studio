//! Lorebook entries and how they match a passage.

use super::*;

#[derive(Clone, Serialize, Deserialize)]
pub struct LoreEntry {
    /// The entry's identifier, e.g. `A43K-L001`.
    #[serde(default)]
    pub key: String,
    pub name: String,
    #[serde(default = "default_category")]
    pub category: String,
    #[serde(default)]
    pub aliases: String,
    /// Comma-separated words or phrases the author chooses. A mention of any of
    /// them in a chapter brings the entry into the AI context, even when the
    /// entry's name or aliases are never written out.
    #[serde(default)]
    pub triggers: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub pinned: bool,
    /// True when the entry was invented by the assistant at the author's
    /// request rather than taken from a chapter. The lorebook marks it so the
    /// author can tell canon from invention; it still travels as story memory.
    #[serde(default)]
    pub invented: bool,
}

pub(crate) fn default_category() -> String {
    "Lore".to_string()
}

impl LoreEntry {
    /// Split a comma-separated field into usable terms. Empty and single-letter
    /// terms are dropped, because one letter matches almost every page.
    pub fn terms(list: &str) -> Vec<&str> {
        list.split(',')
            .map(str::trim)
            .filter(|term| term.chars().count() > 1)
            .collect()
    }

    pub fn alias_list(&self) -> Vec<&str> {
        Self::terms(&self.aliases)
    }

    pub fn trigger_list(&self) -> Vec<&str> {
        Self::terms(&self.triggers)
    }

    /// The first thing in `lower_text` that names this entry: its name, one of
    /// its aliases or one of its trigger words.
    pub fn matched_term(&self, lower_text: &str) -> Option<String> {
        let name = self.name.trim();
        if !name.is_empty() && lower_text.contains(&name.to_lowercase()) {
            return Some(name.to_string());
        }
        self.alias_list()
            .into_iter()
            .chain(self.trigger_list())
            .find(|term| lower_text.contains(&term.to_lowercase()))
            .map(str::to_string)
    }

    pub fn matches(&self, lower_text: &str) -> bool {
        self.matched_term(lower_text).is_some()
    }
}
