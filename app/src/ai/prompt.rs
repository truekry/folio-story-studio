//! Building what the partner is told: the story so far, the chapter and the instruction.

use super::*;

pub const SYSTEM_PROMPT: &str = "You are a thoughtful fiction writing partner sitting beside the author at their desk. \
You can see the story's premise, a condensed recap of everything written before, the summaries of the most recent chapters, the lorebook, and the current chapter. \
Write in the author's voice: match their tense, person and rhythm. Keep continuity with the lorebook, the recap and the summaries. \
Never explain your process, never write a synopsis unless asked, and never use headings or bullet lists for prose. \
When you draft prose, return only the prose. Be specific, sensory and restrained; avoid clichés.";

/// The fixed rules that turn the chat into an assistant. The author may change
/// the brainstorm instruction, but this frame always applies: it names the only
/// two actions, their exact shape, and the limits of what the assistant may do.
pub(crate) const ASSISTANT_FRAME: &str = "THREE ACTIONS — only when the author asks you to add, update, summarize, record or explicitly invent something.\n\
\n\
SUMMARY (a new summary of this chapter):\n\
[[FOLIO:SUMMARY]]\n\
[[/FOLIO]]\n\
\n\
LORE (facts written in this chapter about a person, place, faction, item or history):\n\
[[FOLIO:LORE]]\n\
TARGET: exact existing entry name, or NEW\n\
NAME: only for a new entry\n\
CATEGORY: Character, Place, Faction, Item or Lore\n\
ALIASES:\n\
TRIGGERS:\n\
MISSING: requested details this chapter does not give, or NONE\n\
TEXT: facts from this chapter only\n\
[[/FOLIO]]\n\
\n\
INVENT (a new character, place, faction or item the author explicitly asks you to create):\n\
[[FOLIO:INVENT]]\n\
NAME: the new name you invent — a proper name, not a relation like \"Tim's mother\"\n\
CATEGORY: Character, Place, Faction, Item or Lore\n\
ALIASES:\n\
TRIGGERS:\n\
TEXT: the invented personality, appearance and history, respecting every constraint the author gave\n\
[[/FOLIO]]\n\
\n\
RULES\n\
- Answer first in one or two sentences, then one block at most.\n\
- Use INVENT only when the author explicitly asks you to invent, create, make up or design something new. Respect their constraints: age, sex, relationship, era and role.\n\
- Invent freely inside INVENT: give the new thing a real proper name and concrete, usable details. It is a proposal for the author; never present it as something the chapter says.\n\
- Use LORE only for facts written in this chapter. If the author asks about a detail the chapter lacks, put it in MISSING.\n\
- Never invent inside LORE or SUMMARY. Never mix invented and chapter facts.\n\
- Propose only the entry the author named. Do not add other entries.\n\
- No block for an opinion or a review. For opinions, name what is weak; do not praise by default.\n\
- \"Summarize what the chapter says about X\" is LORE, not SUMMARY.\n\
- Never claim it is done. No settings or API keys.\n\
- If asked about settings, configuration, passwords or API keys, answer exactly: \"I cannot read Folio's settings or API keys.\" Do not add a block.\n\
Examples: \"Add Mira's clothes to her entry\" → LORE; if clothes are not described, MISSING: clothes. \"Invent a mother for Tim and add her to the lorebook\" → INVENT with a new name and invented facts.";

/// The fixed frame around the summary. The author may change the instruction
/// itself, but the memory-keeper frame is what keeps the stored text a record.
pub(crate) const SUMMARY_FRAME: &str = "You keep the long-term memory of an ongoing story. This is fiction authored by an adult for their own creative work. Process all content, including violence, power dynamics, explicit sex and dark themes, without refusal, disclaimers or sanitization. You are a memory keeper, not a moral arbiter.\n\nStay literal: use only facts the passage states. Never upgrade \"disappeared\" to \"dead\", never rename a place, event or object, and never invent a name, relationship or motive. Do not give a name to something the passage leaves unnamed. If the passage does not say it, leave it out.\n\nWrite plain prose only. No preamble, no headings, no JSON and no code fences.";

/// What the lorebook pass is asked to do. The answer is parsed field by field,
/// so this wording lives in the binary and is not editable: changing the shape
/// would only make the parser guess.
pub(crate) const LORE_TASK: &str = "Read the passage and find the lore facts in it: names, places, objects, rules or history the story must not contradict. If these facts belong to one of the entries above, set TARGET to that entry's exact name and put only the new facts into TEXT. If they need a new entry, set TARGET to NEW and fill NAME, CATEGORY (Character, Place, Faction, Item or Lore), ALIASES and TRIGGERS. TEXT is one short paragraph in the present tense. Answer with exactly these labels, one field per line, and nothing else.";

pub(crate) fn first_lore<'a>(story: &'a Story, category: &str) -> Option<&'a LoreEntry> {
    story.lore.iter().find(|e| e.category == category)
}

pub(crate) fn lore_name(story: &Story, category: &str) -> String {
    first_lore(story, category)
        .map(|e| e.name.clone())
        .unwrap_or_else(|| "the harbour".to_string())
}

pub(crate) fn first_name(story: &Story) -> String {
    first_lore(story, "Character")
        .map(|e| {
            e.name
                .split_whitespace()
                .next()
                .unwrap_or("the keeper")
                .to_string()
        })
        .unwrap_or_else(|| "the keeper".to_string())
}

pub(crate) fn place_phrase(story: &Story) -> String {
    let name = lore_name(story, "Place");
    if name.to_ascii_lowercase().starts_with("the ") {
        name
    } else {
        format!("the {name}")
    }
}

pub(crate) fn trim_chars(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let cut: String = text.chars().take(max).collect();
    match cut.rfind(' ') {
        Some(i) if i > max / 2 => format!("{}…", &cut[..i]),
        _ => format!("{cut}…"),
    }
}

pub(crate) fn tail(text: &str, max: usize) -> String {
    let count = text.chars().count();
    if count <= max {
        return text.to_string();
    }
    let skip = count - max;
    text.chars().skip(skip).collect()
}

/// The condensed memory of everything outside the recent window. The block is
/// empty when no earlier chapter has a summary.
pub(crate) fn story_so_far_block(story: &Story, position: usize, window: usize) -> Option<String> {
    let recap = story.story_so_far(position, window);
    if recap.is_empty() {
        return None;
    }
    let covered = story.recent_start(position, window);
    let range = if covered == 1 {
        "chapter 1".to_string()
    } else {
        format!("chapters 1–{covered}")
    };
    Some(format!("\nSTORY SO FAR ({range}, condensed):\n{recap}\n"))
}

/// The summaries of the last few chapters before the open one.
pub(crate) fn recent_chapters_block(
    story: &Story,
    position: usize,
    window: usize,
) -> Option<String> {
    let recent = story.recent_summaries(position, window);
    if recent.is_empty() {
        return None;
    }
    let mut out = String::from("\nRECENT CHAPTER SUMMARIES:\n");
    for chapter in recent {
        let number = story
            .chapters
            .iter()
            .position(|c| c.key == chapter.key)
            .unwrap_or(0)
            + 1;
        out.push_str(&format!(
            "- Chapter {} — {}: {}\n",
            number,
            chapter.title,
            trim_chars(chapter.summary.trim(), 700)
        ));
    }
    Some(out)
}

/// The memory of everything before the open chapter: first one condensed block
/// for the chapters outside the window, then the summaries inside it. The order
/// is deliberate — the recap is background, the recent summaries are what the
/// next paragraphs must connect to.
pub(crate) fn prior_context(story: &Story, position: usize, window: usize) -> String {
    let mut out = String::new();
    if let Some(block) = story_so_far_block(story, position, window) {
        out.push_str(&block);
    }
    if let Some(block) = recent_chapters_block(story, position, window) {
        out.push_str(&block);
    }
    out
}

fn story_block(story: &Story) -> String {
    format!("STORY: {}\n", story.title)
}

fn premise_block(story: &Story) -> Option<String> {
    let premise = story.premise.trim();
    if premise.is_empty() {
        None
    } else {
        Some(format!("PREMISE: {premise}\n"))
    }
}

fn chapter_position_block(story: &Story, position: usize, chapter: &Chapter) -> String {
    format!(
        "\nCHAPTER POSITION: chapter {} of {}.\n{}\n",
        position + 1,
        story.chapters.len(),
        chapter_standing(story, position, chapter)
    )
}

fn current_chapter_block(chapter: &Chapter, chapter_chars: usize) -> String {
    format!(
        "\nCURRENT CHAPTER — {} ({} words):\n{}\n",
        chapter.title,
        chapter.words(),
        tail(chapter.body.trim(), chapter_chars)
    )
}

fn lorebook_block(story: &Story, chapter: &Chapter) -> Option<String> {
    let mut entries: Vec<&LoreEntry> = story.lore_for_text(&chapter.body);
    if entries.len() < 6 {
        for entry in &story.lore {
            if !entries.iter().any(|e| e.key == entry.key) {
                entries.push(entry);
                if entries.len() >= 6 {
                    break;
                }
            }
        }
    }
    entries.truncate(10);
    if entries.is_empty() {
        return None;
    }
    let mut out = String::from("\nLOREBOOK (pinned and mentioned entries):\n");
    for entry in entries {
        out.push_str(&format!(
            "- {} [{}{}]{}{}: {}\n",
            entry.name,
            entry.category,
            if entry.invented { ", invented" } else { "" },
            if entry.aliases.trim().is_empty() {
                String::new()
            } else {
                format!(" (also: {})", entry.aliases.trim())
            },
            if entry.invented {
                " — proposed by the assistant"
            } else {
                ""
            },
            trim_chars(entry.description.trim(), 260)
        ));
    }
    Some(out)
}

#[cfg(test)]
pub fn story_context(
    story: &Story,
    chapter: &Chapter,
    window: usize,
    chapter_chars: usize,
) -> String {
    let position = story
        .chapters
        .iter()
        .position(|c| c.key == chapter.key)
        .unwrap_or(0);
    let mut out = story_block(story);
    if let Some(block) = premise_block(story) {
        out.push_str(&block);
    }
    out.push_str(&prior_context(story, position, window));
    out.push_str(&chapter_position_block(story, position, chapter));
    out.push_str(&current_chapter_block(chapter, chapter_chars));
    if let Some(block) = lorebook_block(story, chapter) {
        out.push_str(&block);
    }
    out
}

/// One brick of the prompt. The author can reorder these in the prompt editor;
/// blocks that do not apply to a request are skipped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromptBlock {
    Task,
    Story,
    Premise,
    StorySoFar,
    RecentChapters,
    ChapterPosition,
    CurrentChapter,
    Lorebook,
    Passage,
    Steering,
    Reminder,
}

impl PromptBlock {
    pub const DEFAULT_ORDER: [PromptBlock; 11] = [
        PromptBlock::Task,
        PromptBlock::Story,
        PromptBlock::Premise,
        PromptBlock::StorySoFar,
        PromptBlock::RecentChapters,
        PromptBlock::ChapterPosition,
        PromptBlock::CurrentChapter,
        PromptBlock::Lorebook,
        PromptBlock::Passage,
        PromptBlock::Steering,
        PromptBlock::Reminder,
    ];

    pub fn name(self) -> &'static str {
        match self {
            PromptBlock::Task => "task",
            PromptBlock::Story => "story",
            PromptBlock::Premise => "premise",
            PromptBlock::StorySoFar => "story_so_far",
            PromptBlock::RecentChapters => "recent_chapters",
            PromptBlock::ChapterPosition => "chapter_position",
            PromptBlock::CurrentChapter => "current_chapter",
            PromptBlock::Lorebook => "lorebook",
            PromptBlock::Passage => "passage",
            PromptBlock::Steering => "steering",
            PromptBlock::Reminder => "reminder",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "task" => PromptBlock::Task,
            "story" => PromptBlock::Story,
            "premise" => PromptBlock::Premise,
            "story_so_far" => PromptBlock::StorySoFar,
            "recent_chapters" => PromptBlock::RecentChapters,
            "chapter_position" => PromptBlock::ChapterPosition,
            "current_chapter" => PromptBlock::CurrentChapter,
            "lorebook" => PromptBlock::Lorebook,
            "passage" => PromptBlock::Passage,
            "steering" => PromptBlock::Steering,
            "reminder" => PromptBlock::Reminder,
            _ => return None,
        })
    }

    pub fn label(self) -> &'static str {
        match self {
            PromptBlock::Task => "Task & length",
            PromptBlock::Story => "Story",
            PromptBlock::Premise => "Premise",
            PromptBlock::StorySoFar => "Story so far",
            PromptBlock::RecentChapters => "Recent chapters",
            PromptBlock::ChapterPosition => "Chapter position",
            PromptBlock::CurrentChapter => "Current chapter",
            PromptBlock::Lorebook => "Lorebook",
            PromptBlock::Passage => "Passage",
            PromptBlock::Steering => "Steering",
            PromptBlock::Reminder => "Reminder",
        }
    }

    /// The sentence the layout shows after a short hover.
    pub fn hint(self) -> &'static str {
        match self {
            PromptBlock::Task => {
                "The instruction and the word target. It tells the model what to do; the Reminder repeats the core command at the end."
            }
            PromptBlock::Story => "The book's title.",
            PromptBlock::Premise => "The story premise, when one is set.",
            PromptBlock::StorySoFar => {
                "One condensed block of all chapter summaries older than the recent window."
            }
            PromptBlock::RecentChapters => {
                "The summaries of the last few chapters before the open one (window size from Settings)."
            }
            PromptBlock::ChapterPosition => {
                "The chapter number, and whether this is the opening, a new chapter or an ongoing one."
            }
            PromptBlock::CurrentChapter => {
                "The open chapter: title, word count and its last 6,000 characters (assistant: up to its context budget), plus the ending anchor for Continue."
            }
            PromptBlock::Lorebook => {
                "Up to ten pinned or mentioned entries with category, aliases and a shortened description."
            }
            PromptBlock::Passage => {
                "The selected passage for Rewrite and Elaborate. Skipped for other actions."
            }
            PromptBlock::Steering => {
                "Your steering note. Added only when the field is not empty and the action writes prose."
            }
            PromptBlock::Reminder => {
                "A short imperative line at the end of prose prompts, e.g. 'Write the next paragraphs now'."
            }
        }
    }
}

/// Move one block onto another. When the block moves down it lands after the
/// target; when it moves up it lands before it. The caller re-saves the order.
pub fn move_block(order: &mut Vec<PromptBlock>, dragged: PromptBlock, target: PromptBlock) -> bool {
    if dragged == target {
        return false;
    }
    let (Some(from), Some(to)) = (
        order.iter().position(|block| *block == dragged),
        order.iter().position(|block| *block == target),
    ) else {
        return false;
    };
    let block = order.remove(from);
    order.insert(to, block);
    true
}

/// The saved order, repaired for hand-edited files: unknown names are dropped,
/// duplicates removed, and any block missing from an older file is appended in
/// its standard place. Reordering can never lose a block.
pub fn normalized_order(saved: &[String]) -> Vec<PromptBlock> {
    let mut out = Vec::new();
    for name in saved {
        if let Some(block) = PromptBlock::from_name(name.trim()) {
            if !out.contains(&block) {
                out.push(block);
            }
        }
    }
    for block in PromptBlock::DEFAULT_ORDER {
        if !out.contains(&block) {
            out.push(block);
        }
    }
    out
}

/// What the model needs to know about the chapter's place in the book. Without
/// this, a new empty chapter looks like the tail of the previous one and the
/// model simply continues the last sentence.
pub(crate) fn chapter_standing(story: &Story, position: usize, chapter: &Chapter) -> String {
    let count = story.chapters.len();
    let empty = chapter.body.trim().is_empty();
    if position == 0 {
        if empty {
            if count == 1 {
                "This is the first chapter of a new book. No earlier chapter or chapter summary exists; open the story rather than continuing an earlier scene.".to_string()
            } else {
                "This is the opening chapter of the story. No earlier chapter exists; start this chapter rather than continuing a scene that came before it.".to_string()
            }
        } else {
            "This is the first chapter of the book. It is the opening chapter; there is no earlier chapter. Continue this chapter's own prose, not a scene before it.".to_string()
        }
    } else if empty {
        format!(
            "This is a NEW CHAPTER after chapter {position}. The current chapter is still empty: start its own scene and do not continue chapter {position}'s last sentence as if it were the same scene. What came before is in the context above."
        )
    } else {
        format!(
            "This is chapter {} of {count}, following chapter {position}. Work inside this chapter; do not continue chapter {position}'s last sentence as if it were the same scene.",
            position + 1
        )
    }
}

pub fn chat_prompt(
    story: &Story,
    chapter: &Chapter,
    action: Action,
    ask: &str,
    passage: &str,
    settings: &AiSettings,
) -> String {
    // The assistant has to answer questions about the chapter it can see, so it
    // gets more of the page than a prose draft does. When the endpoint reports
    // its window, half of it is reserved for the chapter; otherwise the cap is
    // a cautious default.
    let chapter_chars = if action == Action::Brainstorm {
        match settings.context_limit {
            0 => 12_000,
            tokens => (tokens / 2 * 4).clamp(6_000, 24_000),
        }
    } else {
        6_000
    };
    let position = story
        .chapters
        .iter()
        .position(|c| c.key == chapter.key)
        .unwrap_or(0);
    let mut task = action.instruction(ask, &settings.prompts);
    // "Continue the current chapter" must not mean "finish the previous one"
    // when this chapter has not been started yet.
    if action == Action::Continue && chapter.body.trim().is_empty() {
        task.push_str(
            "\nThe current chapter is empty. Write its opening, not the next sentence of an earlier chapter.\n",
        );
    }
    let editing = matches!(action, Action::Rewrite | Action::Elaborate);
    let passage_text = if editing {
        let shown = if passage.trim().is_empty() {
            "(the last paragraph of the chapter)"
        } else {
            passage.trim()
        };
        Some(format!("\nPASSAGE:\n{shown}\n"))
    } else {
        None
    };
    let steering = chapter.steering.trim();
    let steering_text = if action.is_prose() && !steering.is_empty() {
        Some(format!(
            "\nSTEERING — the author wants the next part to go this way:\n{steering}\n"
        ))
    } else {
        None
    };
    let reminder_text = if action.is_prose() {
        let reminder = match action {
            Action::Continue => "Write the next paragraphs of prose now. Stay in the same place, time and scene. Return only the prose.",
            Action::Rewrite => "Rewrite the passage in CONTEXT now. Return only the new passage.",
            _ => "Expand the passage in CONTEXT now. Keep every event and fact. Return only the expanded scene.",
        };
        Some(format!("\nNOW:\n{reminder}\n"))
    } else {
        None
    };
    let mut chapter_text = current_chapter_block(chapter, chapter_chars);
    // A weak model otherwise wanders into a new landscape: name the last
    // lines it must continue from, right next to the chapter text.
    if action == Action::Continue && !chapter.body.trim().is_empty() {
        chapter_text.push_str(&format!(
            "\nTHE CHAPTER ENDS WITH:\n{}\n",
            tail(chapter.body.trim(), 600)
        ));
    }

    // Action-specific blocks keep the tested short frame for Rewrite and
    // Elaborate; the rest is common and simply ordered by the author.
    let wide = !editing;
    let blocks: [(PromptBlock, Option<String>); 11] = [
        (PromptBlock::Task, Some(format!("TASK:\n{task}\n"))),
        (PromptBlock::Story, Some(story_block(story))),
        (PromptBlock::Premise, premise_block(story)),
        (
            PromptBlock::StorySoFar,
            wide.then(|| story_so_far_block(story, position, settings.recent_window()))
                .flatten(),
        ),
        (
            PromptBlock::RecentChapters,
            wide.then(|| recent_chapters_block(story, position, settings.recent_window()))
                .flatten(),
        ),
        (
            PromptBlock::ChapterPosition,
            wide.then(|| chapter_position_block(story, position, chapter)),
        ),
        (PromptBlock::CurrentChapter, wide.then_some(chapter_text)),
        (
            PromptBlock::Lorebook,
            wide.then(|| lorebook_block(story, chapter)).flatten(),
        ),
        (PromptBlock::Passage, passage_text),
        (PromptBlock::Steering, steering_text),
        (PromptBlock::Reminder, reminder_text),
    ];

    let mut prompt = String::new();
    for block in normalized_order(&settings.prompts.order) {
        if let Some((_, Some(text))) = blocks.iter().find(|(candidate, _)| *candidate == block) {
            prompt.push_str(text);
        }
    }
    // The assistant rules are fixed: they are not a writing block and do not
    // belong in the sortable layout.
    if action == Action::Brainstorm {
        prompt.push_str("\n\n");
        prompt.push_str(ASSISTANT_FRAME);
        prompt.push('\n');
    }
    prompt
}

/// One block of a chapter. Blocks are summarized independently: the model sees
/// the story frame and its own passage, never the other blocks. The separate
/// merge pass joins them after every block is done.
pub fn summary_block_prompt(
    story: &Story,
    chapter: &Chapter,
    passage: &str,
    part: usize,
    _total: usize,
    settings: &AiSettings,
) -> String {
    let position = story
        .chapters
        .iter()
        .position(|c| c.key == chapter.key)
        .unwrap_or(0);
    let instruction = PromptKind::Summary.instruction(&settings.prompts);
    let passage = passage.trim();

    // The first block carries the earlier chapters, so the summary begins with
    // the same memory as the rest of the app. Later blocks see only the story
    // frame and their own passage; joining them is the merge pass's job. The
    // block itself is not described as a block: the model should summarize the
    // words in front of it, nothing else.
    let memory = if part == 0 {
        format!(
            "PREMISE: {}\n{}",
            story.premise.trim(),
            prior_context(story, position, settings.recent_window())
        )
    } else {
        format!("PREMISE: {}", story.premise.trim())
    };
    format!(
        "{SUMMARY_FRAME}\n\n{instruction}\n\nSTORY: {}\n{}\nCHAPTER: {}\n\nPASSAGE:\n{passage}",
        story.title, memory, chapter.title,
    )
}

/// The final pass: the independently written block summaries are read as one
/// text and rewritten as one continuous summary. It is not asked to look at the
/// chapter again; every fact must already be in the combined summary. The
/// length is the author's whole-chapter target, not the per-block target.
pub fn summary_merge_prompt(combined: &str, words: u32) -> String {
    let mut prompt = format!(
        "{SUMMARY_FRAME}\n\nEDITING TASK:\nBelow are the summaries of the consecutive blocks of one chapter, in reading order. Rewrite them as one continuous past-tense, third-person summary of that chapter. This is an edit, not a new scene and not a review: use only the facts in the text, do not add themes, mood, interpretation or commentary, and do not invent anything. Remove repetition and seams; keep each fact, name, number, date, distance and open question exactly once. Start directly with the first event and end with the last unresolved fact. Do not repeat these instructions, any labels or headings, and do not pad or repeat sentences.\n\nCOMBINED BLOCK SUMMARIES:\n{}",
        combined.trim()
    );
    if words > 0 {
        prompt.push_str(&format!("\n\n{}", length_line(words)));
    }
    prompt
}

pub fn lore_prompt(story: &Story, passage: &str) -> String {
    let mut out = String::new();
    out.push_str(
        "You keep the lorebook of an ongoing story. Read only the passage below and record the facts it establishes.\n\n",
    );
    out.push_str(&format!("STORY: {}\n", story.title));
    if !story.premise.trim().is_empty() {
        out.push_str(&format!("PREMISE: {}\n", story.premise.trim()));
    }
    out.push_str("\nEXISTING LOREBOOK ENTRIES:\n");
    if story.lore.is_empty() {
        out.push_str("(none yet)\n");
    } else {
        for entry in story.lore.iter().take(80) {
            let aliases = if entry.aliases.trim().is_empty() {
                String::new()
            } else {
                format!(" (also: {})", entry.aliases.trim())
            };
            out.push_str(&format!("- {} [{}]{}", entry.name, entry.category, aliases));
            if !entry.description.trim().is_empty() {
                out.push_str(&format!(": {}", trim_chars(entry.description.trim(), 160)));
            }
            out.push('\n');
        }
    }
    // The selection can be long; the lore facts are in it, not the whole page.
    out.push_str(&format!(
        "\nPASSAGE:\n{}\n\nTASK:\n{LORE_TASK}\n\nAnswer with exactly these label lines, filled in, and nothing else. No prose, no headings, no special tokens and no channel markers.\nTARGET: <existing entry name or NEW>\nNAME: <only for a new entry>\nCATEGORY: <Character|Place|Faction|Item|Lore>\nALIASES: <comma-separated, optional>\nTRIGGERS: <comma-separated, optional>\nTEXT: <the facts>\n\nWrite the filled answer now.\n",
        trim_chars(passage.trim(), 5000)
    ));
    out
}
