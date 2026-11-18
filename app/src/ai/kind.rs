//! The actions the author can ask for, and the prompts they use.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Continue,
    Rewrite,
    Elaborate,
    Brainstorm,
}

impl Action {
    pub fn is_prose(self) -> bool {
        matches!(self, Self::Continue | Self::Rewrite | Self::Elaborate)
    }

    /// Which editable prompt this action uses.
    pub fn kind(self) -> PromptKind {
        match self {
            Self::Continue => PromptKind::Continue,
            Self::Rewrite => PromptKind::Rewrite,
            Self::Elaborate => PromptKind::Elaborate,
            Self::Brainstorm => PromptKind::Brainstorm,
        }
    }

    pub fn instruction(self, ask: &str, prompts: &PromptSettings) -> String {
        let kind = self.kind();
        let mut text = kind.instruction(prompts);
        // The brainstorm prompt is the partner's role. The typed question is
        // appended to it, so the answer stays a conversation about the story
        // instead of turning into the next passage.
        if matches!(self, Self::Brainstorm) {
            let ask = ask.trim();
            if !ask.is_empty() {
                text.push_str(&format!(
                    "\n\nAUTHOR'S QUESTION:\n{ask}\n\nAnswer as a partner: suggest and explain, do not write the scene."
                ));
            }
        }
        text
    }
}

/// The AI functions whose instruction the author can read and rewrite in the
/// settings. The brainstorm text is the partner's role; a typed question is
/// appended to it instead of replacing it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromptKind {
    Summary,
    Continue,
    Rewrite,
    Elaborate,
    Brainstorm,
}

impl PromptKind {
    pub const ALL: [PromptKind; 5] = [
        PromptKind::Summary,
        PromptKind::Continue,
        PromptKind::Rewrite,
        PromptKind::Elaborate,
        PromptKind::Brainstorm,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Summary => "Chapter summary (each block)",
            Self::Continue => "Continue the scene",
            Self::Rewrite => "Rewrite a passage",
            Self::Elaborate => "Elaborate a passage",
            Self::Brainstorm => "Assistant chat",
        }
    }

    /// What this function is asked to do when the author has not written an
    /// instruction of their own.
    pub fn default_instruction(self) -> &'static str {
        match self {
            Self::Summary => "Summarize the passage below in one factual, past-tense, third-person paragraph. Keep every name, number, date, fact and open question exactly as the passage gives it. Do not add anything, do not resolve anything it leaves open, and do not interpret or comment on it. Write only the summary, with no preamble and no headings.",
            Self::Continue => "Continue this chapter with the next few paragraphs of prose, starting after its last line. Stay in the same place, time and scene; do not jump to a new location. Match the voice and tense. Begin with the prose immediately: no questions, no options, no explanation, no headings. End on a turn or question.",
            Self::Rewrite => "Replace the text under PASSAGE with a sharper, more concrete version. The BEFORE and AFTER paragraphs are read-only context: do not return them. Change every sentence; do not repeat the original wording. Keep the same events, facts and point of view. Return only the new passage: no explanation and no commentary.",
            Self::Elaborate => "Expand the text under PASSAGE into a fuller scene of about twice the length. The BEFORE and AFTER paragraphs are read-only context: do not repeat them. Keep every event and every fact, including the opening. Add sensory detail and small actions; do not summarize, shorten or drop sentences.",
            Self::Brainstorm => "Answer as the author's assistant for this chapter. Answer the question first, briefly. When asked to add, update, summarize, record or explicitly invent something, use the action rules below. Be honest: name what is weak instead of praising. Do not continue, rewrite or draft the scene.",
        }
    }

    /// Roughly how many words the answer should aim for. Zero means the
    /// function has no target of its own.
    pub fn default_words(self) -> u32 {
        match self {
            Self::Summary => 90,
            Self::Continue => 240,
            Self::Rewrite => 0,
            Self::Elaborate => 0,
            Self::Brainstorm => 0,
        }
    }

    pub fn words_range(self) -> std::ops::RangeInclusive<u32> {
        match self {
            Self::Brainstorm => 0..=600,
            _ => 0..=1200,
        }
    }

    pub fn slot(self, prompts: &PromptSettings) -> &PromptSetting {
        match self {
            Self::Summary => &prompts.summary,
            Self::Continue => &prompts.continuing,
            Self::Rewrite => &prompts.rewrite,
            Self::Elaborate => &prompts.elaborate,
            Self::Brainstorm => &prompts.brainstorm,
        }
    }

    pub fn slot_mut(self, prompts: &mut PromptSettings) -> &mut PromptSetting {
        match self {
            Self::Summary => &mut prompts.summary,
            Self::Continue => &mut prompts.continuing,
            Self::Rewrite => &mut prompts.rewrite,
            Self::Elaborate => &mut prompts.elaborate,
            Self::Brainstorm => &mut prompts.brainstorm,
        }
    }

    /// Has the author written something of their own for this function?
    pub fn is_edited(self, prompts: &PromptSettings) -> bool {
        !self.slot(prompts).text.trim().is_empty()
    }

    /// The length the reply should aim for, zero when there is none.
    pub fn words(self, prompts: &PromptSettings) -> u32 {
        self.slot(prompts)
            .words
            .unwrap_or_else(|| self.default_words())
    }

    /// The instruction as sent, before the length line is added: the author's
    /// own text or the built-in default. The stored text is returned verbatim —
    /// whitespace included — so the edit field does not eat a space the author
    /// has just typed.
    pub fn base_text(self, prompts: &PromptSettings) -> String {
        let own = &self.slot(prompts).text;
        if own.trim().is_empty() {
            self.default_instruction().to_string()
        } else {
            own.clone()
        }
    }

    /// The text actually sent: the author's own instruction, or the default,
    /// plus the length. Short and binding: local models treat a polite "aim
    /// for" as a suggestion, but obey a hard limit.
    pub fn instruction(self, prompts: &PromptSettings) -> String {
        let base = self.base_text(prompts);
        let base = base.trim();
        let words = self.words(prompts);
        if words == 0 {
            return base.to_string();
        }
        format!("{base}\n\n{}", length_line(words))
    }
}

/// The one sentence that carries a word target. Shared by the prompt editor and
/// the summary merge so both speak with the same voice.
pub(crate) fn length_line(words: u32) -> String {
    format!("Answer in at most {words} words. Do not exceed {words}.")
}
