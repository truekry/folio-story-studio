//! The offline demo partner used when no endpoint is set.

use super::*;

pub fn local_summary(chapter: &Chapter) -> String {
    let body = chapter.body.trim();
    if body.is_empty() {
        return String::new();
    }
    if body.split_whitespace().count() < 22 {
        return format!("A short opening beat: {}", trim_chars(body, 260));
    }
    let mut sentences: Vec<String> = Vec::new();
    let mut current = String::new();
    for ch in body.chars() {
        current.push(ch);
        if matches!(ch, '.' | '!' | '?') {
            let candidate = current.trim();
            if candidate.split_whitespace().count() >= 4 {
                sentences.push(candidate.to_string());
                current.clear();
            }
        }
    }
    if current.trim().split_whitespace().count() >= 5 {
        sentences.push(current.trim().to_string());
    }
    if sentences.is_empty() {
        return trim_chars(body, 360);
    }
    let mut picked: Vec<String> = Vec::new();
    if let Some(first) = sentences.first() {
        picked.push(first.clone());
    }
    if sentences.len() > 2 {
        picked.push(sentences[1].clone());
    }
    if sentences.len() > 2 {
        if let Some(last) = sentences.last() {
            if !picked.iter().any(|s| s == last) {
                picked.push(last.clone());
            }
        }
    }
    let text = picked.join(" ");
    trim_chars(&text, 640)
}

pub fn demo_reply(
    action: Action,
    story: &Story,
    chapter: &Chapter,
    ask: &str,
    passage: &str,
) -> String {
    let name = first_name(story);
    let place = place_phrase(story);
    let item = first_lore(story, "Item")
        .map(|e| e.name.clone())
        .unwrap_or_else(|| "the key".to_string());
    let hook = first_lore(story, "Lore")
        .map(|e| e.name.clone())
        .unwrap_or_else(|| "the tide".to_string());
    let seed = chapter.words() + chapter.title.len();
    match action {
        Action::Continue => format!(
            "The wind changed before {name} reached the door. It came off the water carrying the smell of cold iron and something older, like a room opened after years.\n\n{place} had gone quiet in the way a room goes quiet when someone stops pretending to be asleep. A line of pale light lay across the floor. It was not moonlight. It was the colour of a page held up to a lamp, and it pointed, very patiently, toward the stairs.\n\n{name} thought about the sensible thing to do. The sensible thing was to go back, lock the door, and wait for morning. Instead, {name} picked up {item} and followed the light.\n\nAt the top of the stairs there was a window that had not been there that afternoon. Beyond it, out where the water should have been, someone was walking toward the house with a lantern of their own."
        ),
        Action::Rewrite => format!(
            "{name} stood at the edge of the light and did not move.\n\nThe air smelled of wet rope and cold stone. Somewhere behind the house the water was coming back, not all at once but in long, deliberate breaths, and with every breath the shadow of {place} grew shorter against the wall. {name} had spent years learning how to measure a coast. None of it helped with this.\n\nOn the step, {item} had begun to warm."
        ),
        Action::Elaborate => {
            let base = if passage.trim().is_empty() {
                format!("{name} stood at the edge of the light and did not move.")
            } else {
                passage.trim().to_string()
            };
            format!(
                "{base}\n\nThe air in {place} had the taste of cold iron, and every small sound arrived twice: once in the room, once somewhere behind it. {item} was warm in {name}'s hand, warmer than it had been a moment before. Nothing moved. That was the trouble."
            )
        }
        Action::Brainstorm => {
            let ask = ask.trim();
            if !ask.is_empty() {
                // The author asked something specific: answer it, the way the
                // desk always did before the panel became a brainstorm board.
                let anchor = format!("\"{}\"", trim_chars(ask, 90));
                let turn = if seed % 2 == 0 {
                    format!("{name} does the sensible thing first, and it fails in a way that reveals something true about {place}.")
                } else {
                    format!("{name} is interrupted by something ordinary — a cup, a key, a voice from the lane — and the ordinary thing is what makes the scene strange.")
                };
                return format!(
                    "I'd keep the next beat small. {anchor} is doing a lot of work already, and it will land harder if you let the reader notice one concrete detail before anything supernatural happens.\n\nA way in: {turn}\n\nYou could write it as three short paragraphs — the room, the detail, the decision — and stop before the explanation."
                );
            }
            let ideas = [
                format!("{name} follows the light and finds the missing door standing open in the middle of the room, with the sea visible through it. It creates trouble because the house is choosing a way out that {name} cannot follow."),
                format!("Wren arrives with a second lantern and refuses to come inside. It creates trouble because the one person who knows the rules will only explain them from the threshold."),
                format!("The bell rings a third time and every clock in the house starts moving backwards. It creates trouble because {name} realises the tide is not returning — it is being called."),
                format!("{name} finds a page of the map in their own handwriting, dated tomorrow. It creates trouble because being right has never once made {name} safe."),
                format!("Someone knocks from inside the sealed wall. It creates trouble because {name} has already decided the house is empty, and now has to decide whether to answer."),
            ];
            let mut out = String::from("Five directions worth trying:\n\n");
            for (i, idea) in ideas.iter().enumerate() {
                out.push_str(&format!("{}. {}\n\n", i + 1, idea));
            }
            out.push_str(&format!("The thread I'd pull first is {hook}. It is already in the room, and it gives the scene a clock."));
            out
        }
    }
}

#[allow(dead_code)]
pub fn passage_or_last_paragraph(body: &str) -> String {
    if let Some(pos) = body.rfind("\n\n") {
        body[pos + 2..].trim().to_string()
    } else {
        body.trim().to_string()
    }
}
