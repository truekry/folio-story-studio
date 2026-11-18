//! A manual tuning aid: print a real request for one prompt kind so it can be
//! sent to an endpoint. Run it explicitly, it never runs on its own:
//!
//! ```text
//! $env:FOLIO_TUNING_KIND="continue"   # assistant|summary|merge|continue|rewrite|elaborate|proofread|lore
//! $env:FOLIO_TUNING_ASK="..."         # only for assistant / prose requests
//! cargo test print_the_assistant_prompt_for_tuning -- --ignored --nocapture
//! ```

use super::*;

fn fixture() -> Option<(Story, Chapter)> {
    let body = match std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/assistant_chapter.txt"
    )) {
        Ok(body) => body,
        Err(error) => {
            eprintln!("skipped: local fixture tests/assistant_chapter.txt is missing ({error})");
            return None;
        }
    };
    let library = Library::sample();
    let mut story = library.stories[0].clone();
    story.chapters[1].title = "The Salt Room".to_string();
    story.chapters[1].body = body;
    let chapter = story.chapters[1].clone();
    Some((story, chapter))
}

fn paragraph(body: &str, starts_with: &str) -> String {
    body.split("\n\n")
        .find(|part| part.trim_start().starts_with(starts_with))
        .unwrap_or_default()
        .trim()
        .to_string()
}

#[test]
#[ignore = "prints a request; run with --nocapture to tune it against an endpoint"]
fn print_the_assistant_prompt_for_tuning() {
    let Some((story, chapter)) = fixture() else {
        return;
    };
    let settings = AiSettings {
        context_limit: 32_000,
        ..Default::default()
    };
    let kind = std::env::var("FOLIO_TUNING_KIND").unwrap_or_else(|_| "assistant".to_string());
    let ask = std::env::var("FOLIO_TUNING_ASK").unwrap_or_else(|_| {
        "Summarize what this chapter says about Mira: her appearance, her clothes, anything else that matters. Add it to her lore entry.".to_string()
    });
    let weak = paragraph(&chapter.body, "The sea was very beautiful");
    let neutral = paragraph(&chapter.body, "Mira put her bag on the floor");
    let flawed = "The sea was very beautiful that evening, and everone in Brinehouse felt sad. Mira stand at the window and watch the water climb the harbour steps, she felt nothing at all.";

    let (system, prompt, temperature, max_tokens) = match kind.as_str() {
        "summary" => {
            let blocks = summary_blocks(&chapter.body, 300);
            let passage = chapter
                .body
                .get(blocks[0].clone())
                .unwrap_or_default()
                .to_string();
            (
                SYSTEM_PROMPT,
                summary_block_prompt(&story, &chapter, &passage, 0, blocks.len(), &settings),
                0.3,
                500,
            )
        }
        "merge" => {
            let combined = "Mira returns to the house and meets Wren.\n\n\
                            Wren gives her a key that is too large for the doors.\n\n\
                            A map of an impossible island appears in her mother's room.";
            (SYSTEM_PROMPT, summary_merge_prompt(combined, 150), 0.2, 500)
        }
        "continue" => (
            SYSTEM_PROMPT,
            chat_prompt(&story, &chapter, Action::Continue, &ask, "", &settings),
            0.8,
            700,
        ),
        "rewrite" => (
            SYSTEM_PROMPT,
            chat_prompt(&story, &chapter, Action::Rewrite, "", &weak, &settings),
            0.5,
            500,
        ),
        "elaborate" => (
            SYSTEM_PROMPT,
            chat_prompt(&story, &chapter, Action::Elaborate, "", &neutral, &settings),
            0.5,
            700,
        ),
        "proofread" => (PROOFREAD_SYSTEM, proofread_prompt(flawed), 0.8, 300),
        "lore" => (SYSTEM_PROMPT, lore_prompt(&story, &weak), 0.3, 600),
        _ => (
            SYSTEM_PROMPT,
            chat_prompt(&story, &chapter, Action::Brainstorm, &ask, "", &settings),
            0.6,
            1200,
        ),
    };
    let model = std::env::var("FOLIO_LAN_MODEL").unwrap_or_else(|_| "local-model".to_string());
    let request = serde_json::json!({
        "model": model,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": prompt},
        ],
        "temperature": temperature,
        "max_tokens": max_tokens,
        "stream": false,
    });
    println!("===REQUEST===");
    println!("{}", serde_json::to_string(&request).expect("request body"));
    println!("===END===");
}
