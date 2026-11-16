//! Splitting prose into paragraphs and fitting it to the measure.

use super::*;

/// A title as wide as the row allows; anything longer ends in an ellipsis
/// rather than running into the page number.
pub(crate) fn fit(text: &str, size: f32, max_width: f32) -> String {
    if measure(text, size, Face::Roman) <= max_width {
        return text.to_string();
    }
    let mut out = String::new();
    for ch in text.chars() {
        let mut test = out.clone();
        test.push(ch);
        if measure(&format!("{test}…"), size, Face::Roman) > max_width {
            break;
        }
        out.push(ch);
    }
    out.push('…');
    out
}

/// Split a chapter into paragraphs: blank lines separate them, single line
/// breaks inside a paragraph are treated as wrapping, not as new paragraphs.
pub(crate) fn paragraphs(text: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut current = String::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            if !current.is_empty() {
                blocks.push(std::mem::take(&mut current));
            }
        } else {
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(line);
        }
    }
    if !current.is_empty() {
        blocks.push(current);
    }
    blocks
}

pub(crate) fn wrap(text: &str, size: f32, max_width: f32, face: Face) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if line.is_empty() {
            if measure(word, size, face) > max_width {
                // A single word wider than the line is broken by character.
                let mut chunk = String::new();
                for ch in word.chars() {
                    let mut test = chunk.clone();
                    test.push(ch);
                    if measure(&test, size, face) > max_width && !chunk.is_empty() {
                        lines.push(std::mem::take(&mut chunk));
                    }
                    chunk.push(ch);
                }
                line = chunk;
            } else {
                line.push_str(word);
            }
        } else {
            let width =
                measure(&line, size, face) + measure(" ", size, face) + measure(word, size, face);
            if width <= max_width {
                line.push(' ');
                line.push_str(word);
            } else {
                lines.push(std::mem::take(&mut line));
                line.push_str(word);
            }
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}
