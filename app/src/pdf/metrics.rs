//! Page geometry, the three Helvetica faces and how wide a line is.

pub(crate) const PAGE_WIDTH: f32 = 595.28;

pub(crate) const PAGE_HEIGHT: f32 = 841.89;

pub(crate) const MARGIN_LEFT: f32 = 84.0;

pub(crate) const MARGIN_RIGHT: f32 = 84.0;

pub(crate) const MARGIN_TOP: f32 = 92.0;

pub(crate) const MARGIN_BOTTOM: f32 = 88.0;

pub(crate) const FOOTER_Y: f32 = 46.0;

pub(crate) const BODY_SIZE: f32 = 11.0;

pub(crate) const BODY_LEADING: f32 = 16.4;

pub(crate) const PARAGRAPH_GAP: f32 = 7.0;

pub(crate) const HEADING_SIZE: f32 = 17.0;

pub(crate) const HEADING_LEADING: f32 = 22.0;

/// One row of the contents page.
pub(crate) const ENTRY_LEADING: f32 = 25.0;

/// The top of a chapter as a bookmark opens it.
pub(crate) const DEST_TOP: f32 = PAGE_HEIGHT - MARGIN_TOP + 24.0;

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Face {
    Roman,
    Bold,
    Italic,
}

impl Face {
    pub(crate) fn resource(self) -> &'static str {
        match self {
            Face::Roman => "F1",
            Face::Bold => "F2",
            Face::Italic => "F3",
        }
    }

    /// Helvetica-Bold is a little wider than the regular face; a small factor
    /// keeps headings from reaching past the right margin.
    fn width_scale(self) -> f32 {
        if self == Face::Bold {
            1.05
        } else {
            1.0
        }
    }
}

pub(crate) fn measure(text: &str, size: f32, face: Face) -> f32 {
    let units: f32 = text.chars().map(char_width).sum();
    units / 1000.0 * size * face.width_scale()
}

/// Widths of Helvetica's printable ASCII range, in 1/1000 em.
pub(crate) const ASCII_WIDTHS: [f32; 95] = [
    278.0, 278.0, 355.0, 556.0, 556.0, 889.0, 667.0, 191.0, 333.0, 333.0, 389.0, 584.0, 278.0,
    333.0, 278.0, 278.0, 556.0, 556.0, 556.0, 556.0, 556.0, 556.0, 556.0, 556.0, 556.0, 556.0,
    278.0, 278.0, 584.0, 584.0, 584.0, 556.0, 1015.0, 667.0, 667.0, 722.0, 722.0, 667.0, 611.0,
    778.0, 722.0, 278.0, 500.0, 667.0, 556.0, 833.0, 722.0, 778.0, 667.0, 778.0, 722.0, 667.0,
    611.0, 722.0, 667.0, 944.0, 667.0, 667.0, 611.0, 278.0, 278.0, 278.0, 469.0, 556.0, 333.0,
    556.0, 556.0, 500.0, 556.0, 556.0, 278.0, 556.0, 556.0, 222.0, 222.0, 500.0, 222.0, 833.0,
    556.0, 556.0, 556.0, 556.0, 333.0, 500.0, 278.0, 556.0, 500.0, 722.0, 500.0, 500.0, 500.0,
    334.0, 260.0, 334.0, 584.0,
];

pub(crate) fn char_width(ch: char) -> f32 {
    let code = ch as u32;
    if (32..=126).contains(&code) {
        return ASCII_WIDTHS[(code - 32) as usize];
    }
    match ch {
        'Ä' | 'Ö' | 'Ü' => 667.0,
        'ä' | 'ö' | 'ü' => 556.0,
        'ß' => 611.0,
        '–' => 556.0,
        '—' => 1000.0,
        '…' => 1000.0,
        '·' => 278.0,
        '„' | '“' | '”' => 333.0,
        '‘' | '’' => 222.0,
        '€' => 500.0,
        _ => 556.0,
    }
}

pub(crate) fn text_width() -> f32 {
    PAGE_WIDTH - MARGIN_LEFT - MARGIN_RIGHT
}
