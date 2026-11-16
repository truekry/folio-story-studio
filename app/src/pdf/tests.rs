//! Unit tests for the PDF writer.

use super::*;
use crate::model::Library;

fn sample() -> Story {
    Library::sample().stories[0].clone()
}

fn page_count(bytes: &[u8]) -> usize {
    let text = String::from_utf8_lossy(bytes);
    text.matches("/Type /Page ").count()
}

/// The drawing commands of one page's content stream.
fn object_stream<'a>(text: &'a str, object: usize) -> &'a str {
    let marker = format!("\n{object} 0 obj\n");
    let start = text.find(&marker).expect("object exists") + marker.len();
    let open = text[start..].find("stream\n").expect("stream") + start + 7;
    let close = text[open..].find("endstream").expect("end of stream") + open;
    &text[open..close]
}

#[test]
fn bookmarks_jump_to_the_page_a_chapter_starts_on() {
    let story = sample();
    let text = String::from_utf8_lossy(&story_pdf(&story)).into_owned();
    assert!(text.contains("/Type /Outlines"));
    assert!(text.contains("/PageMode /UseOutlines"));

    let mut cursor = text.find("/Type /Outlines").expect("outline root");
    let root_end = text[cursor..]
        .find(">>")
        .map(|offset| offset + cursor)
        .expect("root end");
    assert!(
        text[cursor..root_end].contains(&format!("/Count {} ", story.chapters.len())),
        "the outline should count every chapter"
    );
    for (index, chapter) in story.chapters.iter().enumerate() {
        let at = text[cursor..]
            .find("/Title (")
            .map(|offset| offset + cursor)
            .expect("a bookmark title");
        let end = text[at..]
            .find(") /Parent")
            .map(|offset| offset + at)
            .expect("title end");
        let label = &text[at + 8..end];
        assert!(
            label.starts_with(&format!("{} ", index + 1)),
            "bookmark {index} has the wrong number: {label:?}"
        );
        assert!(
            label.contains(chapter.title.as_str()),
            "no title in {label:?}"
        );
        let after = &text[end..];
        let dest_at = after.find("/Dest [").expect("a destination");
        let page_object: usize = after[dest_at + 7..]
            .split_whitespace()
            .next()
            .expect("page object")
            .parse()
            .expect("a number");
        let stream = object_stream(&text, page_object + 1);
        assert!(
            stream.contains(&format!("CHAPTER {}", index + 1)),
            "bookmark {} lands on the wrong page",
            index + 1
        );
        cursor = end;
    }
}

#[test]
fn the_contents_page_lists_every_chapter_and_is_clickable() {
    let story = sample();
    let text = String::from_utf8_lossy(&story_pdf(&story)).into_owned();
    assert!(text.contains("(Contents)"));
    for (index, chapter) in story.chapters.iter().enumerate() {
        assert!(
            text.contains(&format!("({:02})", index + 1)),
            "no number for chapter {}",
            index + 1
        );
        assert!(
            text.contains(chapter.title.as_str()),
            "no title for chapter {}",
            index + 1
        );
    }
    assert_eq!(text.matches("/Subtype /Link").count(), story.chapters.len());
    // The contents page comes right after the title page.
    let contents = object_stream(&text, 7 + 2 + 1);
    assert!(contents.contains("(Contents)"));
}

#[test]
fn a_single_chapter_story_skips_the_contents_page() {
    let mut story = sample();
    story.chapters.truncate(1);
    let text = String::from_utf8_lossy(&story_pdf(&story)).into_owned();
    assert!(!text.contains("(Contents)"));
    assert!(text.contains("/Type /Outlines"));
}

#[test]
fn many_chapters_run_the_contents_over_more_than_one_page() {
    let mut story = sample();
    while story.chapters.len() < 60 {
        let mut chapter = story.chapters[0].clone();
        chapter.title = format!("Chapter {}", story.chapters.len() + 1);
        story.chapters.push(chapter);
    }
    let bytes = story_pdf(&story);
    let text = String::from_utf8_lossy(&bytes).into_owned();
    assert_eq!(text.matches("/Subtype /Link").count(), story.chapters.len());
    assert_eq!(text.matches("(Contents)").count(), 1);
    assert!(page_count(&bytes) > story.chapters.len());
}

#[test]
fn a_pdf_has_a_catalogue_the_right_pages_and_an_end() {
    let story = sample();
    let bytes = story_pdf(&story);
    assert!(bytes.starts_with(b"%PDF-1.4\n"));
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("/Type /Catalog"));
    assert!(text.contains("/BaseFont /Helvetica"));
    assert!(text.ends_with("%%EOF\n"));

    // A title page, then every chapter on a fresh page (some run over).
    let pages = page_count(&bytes);
    assert!(
        pages >= story.chapters.len() + 1,
        "only {pages} pages for {} chapters",
        story.chapters.len()
    );
    assert!(text.contains(&format!("/Count {pages} >>")));
    for number in 1..=story.chapters.len() {
        assert!(
            text.contains(&format!("CHAPTER {number}")),
            "chapter {number} is missing its heading"
        );
    }

    // The cross-reference table has to point at the table itself.
    let start = text.find("startxref").expect("startxref");
    let number: usize = text[start + 9..]
        .trim_start()
        .split_whitespace()
        .next()
        .expect("offset")
        .parse()
        .expect("number");
    assert_eq!(&bytes[number..number + 4], b"xref");
}

#[test]
fn chapter_titles_and_text_reach_the_pages() {
    let story = sample();
    let text = String::from_utf8_lossy(&story_pdf(&story)).into_owned();
    assert!(text.contains("The Lantern Coast"));
    assert!(text.contains("CHAPTER 1"));
    assert!(text.contains("The Tide Map"));
    assert!(text.contains("Her mother's house had no door."));
}

#[test]
fn umlauts_are_written_in_windows_1252() {
    let mut story = sample();
    story.chapters[0].title = "Grüße".to_string();
    story.chapters[0].body = "Über den Wellen, äußerlich ruhig.".to_string();
    let bytes = story_pdf(&story);
    let text = bytes.as_slice();
    assert!(
        text.windows(5).any(|w| w == b"Gr\xFC\xDFe"),
        "the title should travel as 0xFC and 0xDF"
    );
    assert!(text.windows(4).any(|w| w == b"\xDCber"));
}

#[test]
fn a_long_chapter_runs_over_several_pages() {
    let mut story = sample();
    story.chapters = vec![story.chapters[0].clone()];
    story.chapters[0].body = "word ".repeat(4000);
    let bytes = story_pdf(&story);
    assert!(
        page_count(&bytes) > 4,
        "4000 words should not fit on four pages"
    );
}

#[test]
fn lines_never_exceed_the_measure() {
    let text =
        "A long sentence with many ordinary words, meant to wrap over several lines. ".repeat(6);
    for line in wrap(&text, BODY_SIZE, text_width(), Face::Roman) {
        assert!(
            measure(&line, BODY_SIZE, Face::Roman) <= text_width() + 0.01,
            "line was {line:?}"
        );
    }
}

#[test]
fn a_word_wider_than_the_line_is_broken() {
    let word = "x".repeat(300);
    let lines = wrap(&word, BODY_SIZE, text_width(), Face::Roman);
    assert!(lines.len() > 1);
}

#[test]
fn paragraphs_keep_blank_line_breaks_only() {
    let blocks = paragraphs("First line\nstill first.\n\nSecond block.");
    assert_eq!(blocks, vec!["First line still first.", "Second block."]);
}
