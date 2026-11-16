//! Encoding and the PDF objects: strings, streams and page references.

use super::*;

/// Map a character to Windows-1252, the encoding the built-in fonts use.
pub(crate) fn winansi(ch: char) -> Option<u8> {
    let special = match ch {
        '€' => Some(0x80),
        '‚' => Some(0x82),
        'ƒ' => Some(0x83),
        '„' => Some(0x84),
        '…' => Some(0x85),
        '†' => Some(0x86),
        '‡' => Some(0x87),
        'ˆ' => Some(0x88),
        '‰' => Some(0x89),
        'Š' => Some(0x8A),
        '‹' => Some(0x8B),
        'Œ' => Some(0x8C),
        'Ž' => Some(0x8E),
        '‘' => Some(0x91),
        '’' => Some(0x92),
        '“' => Some(0x93),
        '”' => Some(0x94),
        '•' => Some(0x95),
        '–' => Some(0x96),
        '—' => Some(0x97),
        '˜' => Some(0x98),
        '™' => Some(0x99),
        'š' => Some(0x9A),
        '›' => Some(0x9B),
        'œ' => Some(0x9C),
        'ž' => Some(0x9E),
        'Ÿ' => Some(0x9F),
        '\u{00A0}' => Some(0x20),
        _ => None,
    };
    if special.is_some() {
        return special;
    }
    let code = ch as u32;
    if (0x20..=0x7E).contains(&code) || (0xA0..=0xFF).contains(&code) {
        Some(code as u8)
    } else {
        None
    }
}

pub(crate) fn escape_into(out: &mut Vec<u8>, text: &str) {
    for ch in text.chars() {
        match winansi(ch) {
            Some(b'\\') => out.extend_from_slice(b"\\\\"),
            Some(b'(') => out.extend_from_slice(b"\\("),
            Some(b')') => out.extend_from_slice(b"\\)"),
            Some(byte) => out.push(byte),
            None => out.push(b'?'),
        }
    }
}

pub(crate) fn push_object(out: &mut Vec<u8>, offsets: &mut Vec<usize>, number: usize, body: &[u8]) {
    assert_eq!(offsets.len() + 1, number, "objects are numbered in order");
    offsets.push(out.len());
    out.extend_from_slice(format!("{number} 0 obj\n").as_bytes());
    out.extend_from_slice(body);
    out.extend_from_slice(b"\nendobj\n");
}

/// The destination a bookmark or a contents row jumps to: the top of a page.
pub(crate) fn dest(page_object: usize) -> String {
    format!("[{page_object} 0 R /XYZ 0 {DEST_TOP:.2} 0]")
}
