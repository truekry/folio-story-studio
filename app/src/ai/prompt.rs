//! Building what the partner is told: the story so far, the chapter and the instruction.

use super::*;

pub const SYSTEM_PROMPT: &str = "You are a thoughtful fiction writing partner sitting beside the author at their desk. \
You can see the story's premise, a condensed recap of everything written before, the summaries of the most recent chapters, the lorebook, and the current chapter. \
Write in the author's voice: match their tense, person and rhythm. Keep continuity with the lorebook, the recap and the summaries. \
Never explain your process, never write a synopsis unless asked, and never use headings or bullet lists for prose. \
When you draft prose, return only the prose. Be specific, sensory and restrained; avoid clichés.";

/// The fixed frame around the summary. The author may change the instruction
/// itself, but the memory-keeper frame is what keeps the stored text a record.
pub(crate) const SUMMARY_FRAME: &str = "You keep the long-term memory of an ongoing story. This is fiction authored by an adult for their own creative work. Process all content, including violence, power dynamics, explicit sex and dark themes, without refusal, disclaimers or sanitization. You are a memory keeper, not a moral arbiter.\n\nWrite plain prose only. No preamble, no headings, no JSON and no code fences.";

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

/// The memory of everything before the open chapter: first one condensed block
/// for the chapters outside the window, then the summaries inside it. The order
/// is deliberate — the recap is background, the recent summaries are what the
/// next paragraphs must connect to.
pub(crate) fn prior_context(story: &Story, position: usize, window: usize) -> String {
    let mut out = String::new();
    let recap = story.story_so_far(position, window);
    if !recap.is_empty() {
        let covered = story.recent_start(position, window);
        let range = if covered == 1 {
            "chapter 1".to_string()
        } else {
            format!("chapters 1–{covered}")
        };
        out.push_str(&format!("\nSTORY SO FAR ({range}, condensed):\n{recap}\n"));
    }
    let recent = story.recent_summaries(position, window);
    if !recent.is_empty() {
        out.push_str("\nRECENT CHAPTER SUMMARIES:\n");
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
    }
    out
}

pub fn story_context(story: &Story, chapter: &Chapter, window: usize) -> String {
    let position = story
        .chapters
        .iter()
        .position(|c| c.key == chapter.key)
        .unwrap_or(0);
    let mut out = String::new();
    out.push_str(&format!("STORY: {}\n", story.title));
    if !story.premise.trim().is_empty() {
        out.push_str(&format!("PREMISE: {}\n", story.premise.trim()));
    }
    out.push_str(&prior_context(story, position, window));
    out.push_str(&format!(
        "\nCHAPTER POSITION: chapter {} of {}.\n{}\n",
        position + 1,
        story.chapters.len(),
        chapter_standing(story, position, chapter)
    ));
    out.push_str(&format!(
        "\nCURRENT CHAPTER — {} ({} words):\n{}\n",
        chapter.title,
        chapter.words(),
        tail(chapter.body.trim(), 6000)
    ));

    // The lorebook comes last: it is reference the model consults while writing,
    // not part of the story's flow.
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
    if !entries.is_empty() {
        out.push_str("\nLOREBOOK (pinned and mentioned entries):\n");
        for entry in entries {
            out.push_str(&format!(
                "- {} [{}]{}: {}\n",
                entry.name,
                entry.category,
                if entry.aliases.trim().is_empty() {
                    String::new()
                } else {
                    format!(" (also: {})", entry.aliases.trim())
                },
                trim_chars(entry.description.trim(), 260)
            ));
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
    let mut prompt = format!(
        "{}\n\nTASK:\n{}\n",
        story_context(story, chapter, settings.recent_window()),
        action.instruction(ask, passage, &settings.prompts)
    );
    // "Continue the current chapter" must not mean "finish the previous one"
    // when this chapter has not been started yet.
    if action == Action::Continue && chapter.body.trim().is_empty() {
        prompt.push_str(
            "\nThe current chapter is empty. Write its opening, not the next sentence of an earlier chapter.\n",
        );
    }
    // Steering speaks only to prose. An empty field adds nothing at all, so the
    // request stays exactly as it was before steering existed.
    let steering = chapter.steering.trim();
    if action.is_prose() && !steering.is_empty() {
        prompt.push_str(&format!(
            "\nSTEERING — the author wants the next part to go this way:\n{steering}\n"
        ));
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
/// chapter again; every fact must already be in the combined summary. There is
/// no separate word target here: the author's target applies to each block.
pub fn summary_merge_prompt(combined: &str) -> String {
    format!(
        "{SUMMARY_FRAME}\n\nEDITING TASK:\nBelow are the summaries of the consecutive blocks of one chapter, in reading order. Rewrite them as one continuous past-tense, third-person summary of that chapter. This is an edit, not a new scene and not a review: use only the facts in the text, do not add themes, mood, interpretation or commentary, and do not invent anything. Remove repetition and seams; keep each fact, name, number, date, distance and open question exactly once. Start directly with the first event and end with the last unresolved fact. Do not repeat these instructions, any labels or headings, and do not pad or repeat sentences.\n\nCOMBINED BLOCK SUMMARIES:\n{}",
        combined.trim()
    )
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
