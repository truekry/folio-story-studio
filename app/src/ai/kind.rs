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

    pub fn instruction(self, ask: &str, passage: &str, prompts: &PromptSettings) -> String {
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
        // The passage is not part of the editable text: the author may rewrite
        // the instruction without losing the words it must work on.
        if matches!(self, Self::Rewrite | Self::Elaborate) {
            let shown = if passage.trim().is_empty() {
                "(the last paragraph of the chapter)"
            } else {
                passage.trim()
            };
            text.push_str(&format!("\n\nPASSAGE:\n{shown}"));
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
            Self::Brainstorm => "Brainstorm with the author",
        }
    }

    /// What this function is asked to do when the author has not written an
    /// instruction of their own.
    pub fn default_instruction(self) -> &'static str {
        match self {
            Self::Summary => "Summarize the passage below in one factual, past-tense, third-person paragraph. Keep every name, number, date, fact and open question exactly as the passage gives it. Do not add anything, do not resolve anything it leaves open, and do not interpret or comment on it. Write only the summary, with no preamble and no headings.",
            Self::Continue => "Continue the current chapter from where it stops. Match the existing voice, keep every established fact, and end on a small turn or question rather than a conclusion. Do not summarize what came before and do not use a heading. Return only the prose.",
            Self::Rewrite => "Rewrite the passage below so it is sharper and more concrete, keeping the same events, facts and point of view. Return only the rewritten passage, with no preamble and no commentary.",
            Self::Elaborate => "Elaborate the passage below into a fuller scene. Keep every event, fact and point of view exactly as it is; add concrete sensory detail, small actions and the beat of the moment. Do not invent a new plot turn and do not summarize. Aim for roughly twice the length of the passage, and return only the prose.",
            Self::Brainstorm => "Answer as the author's writing partner, in your own voice and addressed to the author. Suggest what could happen next and why it would work, point out what is missing, or weigh the options — talk it through with them. Do not continue, rewrite or draft the scene, and do not return prose from the chapter. Keep it conversational and short.",
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
    /// plus one sentence about the length. The length line is what keeps a
    /// talkative local model on the point instead of letting it wander.
    pub fn instruction(self, prompts: &PromptSettings) -> String {
        let base = self.base_text(prompts);
        let base = base.trim();
        let words = self.words(prompts);
        if words == 0 {
            return base.to_string();
        }
        format!(
            "{base}\n\nLength: about {words} words. Keep to what matters, and always finish your last sentence."
        )
    }
}
