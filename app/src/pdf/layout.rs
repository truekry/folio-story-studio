//! Filling pages: the title page, the contents, the chapter text and the entry point.

use super::*;

/// One finished page: what is drawn on it, and where it can be clicked.
#[derive(Default)]
pub(crate) struct Page {
    ops: Vec<u8>,
    links: Vec<Link>,
}

/// A clickable row on the contents page that jumps to a chapter.
pub(crate) struct Link {
    page: usize,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
}

pub(crate) struct Doc {
    title: String,
    pages: Vec<Page>,
    ops: Vec<u8>,
    links: Vec<Link>,
    y: f32,
}

impl Doc {
    fn new(title: &str) -> Self {
        Self {
            title: title.trim().to_string(),
            pages: Vec::new(),
            ops: Vec::new(),
            links: Vec::new(),
            y: PAGE_HEIGHT - MARGIN_TOP,
        }
    }

    fn title_page(&mut self, story: &Story) {
        self.new_page();
        let size = 27.0;
        let leading = size * 1.24;
        let lines = wrap(&story.title, size, text_width(), Face::Bold);
        let count = lines.len().max(1) as f32;
        let mut y = PAGE_HEIGHT * 0.56 + (count - 1.0) * leading * 0.5;
        for line in &lines {
            self.text_centered(y, size, Face::Bold, line);
            y -= leading;
        }
        if lines.is_empty() {
            y -= leading;
        }
        let premise = story.premise.trim();
        if !premise.is_empty() {
            y -= 22.0;
            self.horizontal_rule(y, 84.0);
            y -= 30.0;
            let size = 12.5;
            let leading = size * 1.55;
            for line in wrap(premise, size, text_width() - 90.0, Face::Italic) {
                self.text_centered(y, size, Face::Italic, &line);
                y -= leading;
            }
        }
    }

    fn chapter_heading(&mut self, number: usize, title: &str) {
        self.text_at(
            MARGIN_LEFT,
            self.y,
            9.0,
            Face::Bold,
            &format!("CHAPTER {number}"),
        );
        self.y -= 30.0;
        for line in wrap(title, HEADING_SIZE, text_width(), Face::Bold) {
            self.text_at(MARGIN_LEFT, self.y, HEADING_SIZE, Face::Bold, &line);
            self.y -= HEADING_LEADING;
        }
        self.y -= 4.0;
        self.horizontal_rule(self.y, text_width());
        self.y -= 26.0;
    }

    fn chapter_body(&mut self, body: &str) {
        for block in paragraphs(body) {
            let lines = wrap(&block, BODY_SIZE, text_width(), Face::Roman);
            if lines.is_empty() {
                continue;
            }
            // Keep the first two lines of a paragraph on the same page.
            self.ensure(BODY_LEADING * 2.0);
            for line in lines {
                self.ensure(BODY_LEADING);
                self.text_at(MARGIN_LEFT, self.y, BODY_SIZE, Face::Roman, &line);
                self.y -= BODY_LEADING;
            }
            self.y -= PARAGRAPH_GAP;
        }
    }

    fn ensure(&mut self, height: f32) {
        if self.y - height < MARGIN_BOTTOM {
            self.new_page();
        }
    }

    fn new_page(&mut self) {
        if !self.ops.is_empty() {
            self.close_page();
        }
        self.y = PAGE_HEIGHT - MARGIN_TOP;
    }

    fn close_page(&mut self) {
        let number = self.pages.len() + 1;
        self.push_footer(number);
        self.pages.push(Page {
            ops: std::mem::take(&mut self.ops),
            links: std::mem::take(&mut self.links),
        });
    }

    fn push_footer(&mut self, number: usize) {
        let label = format!("{} · {}", self.title, number);
        let size = 8.5;
        let x = ((PAGE_WIDTH - measure(&label, size, Face::Roman)) / 2.0).max(MARGIN_LEFT);
        self.ops.extend_from_slice(
            format!(
                "BT /{} {size} Tf 1 0 0 1 {x:.2} {FOOTER_Y} Tm 0.55 g (",
                Face::Roman.resource()
            )
            .as_bytes(),
        );
        escape_into(&mut self.ops, &label);
        self.ops.extend_from_slice(b") Tj ET\n");
    }

    /// Finish the page being drawn and leave an empty one in its place, to be
    /// filled once the chapters know their pages. Returns the page's index.
    fn reserve_page(&mut self) -> usize {
        if !self.ops.is_empty() {
            self.close_page();
        }
        let index = self.pages.len();
        self.pages.push(Page::default());
        self.y = PAGE_HEIGHT - MARGIN_TOP;
        index
    }

    fn text_at(&mut self, x: f32, y: f32, size: f32, face: Face, text: &str) {
        self.ops.extend_from_slice(
            format!(
                "BT /{} {size:.2} Tf 1 0 0 1 {x:.2} {y:.2} Tm 0 g (",
                face.resource()
            )
            .as_bytes(),
        );
        escape_into(&mut self.ops, text);
        self.ops.extend_from_slice(b") Tj ET\n");
    }

    fn text_centered(&mut self, y: f32, size: f32, face: Face, text: &str) {
        let x = ((PAGE_WIDTH - measure(text, size, face)) / 2.0).max(MARGIN_LEFT);
        self.text_at(x, y, size, face, text);
    }

    fn horizontal_rule(&mut self, y: f32, width: f32) {
        let x = (PAGE_WIDTH - width) / 2.0;
        self.ops.extend_from_slice(
            format!(
                "0.72 g 0.7 w {x:.2} {y:.2} m {:.2} {y:.2} l S 0 g\n",
                x + width
            )
            .as_bytes(),
        );
    }

    fn begin_contents_page(&mut self, first: bool) {
        self.ops.clear();
        self.links.clear();
        let top = PAGE_HEIGHT - MARGIN_TOP;
        if first {
            self.text_at(MARGIN_LEFT, top - 8.0, 22.0, Face::Bold, "Contents");
            self.horizontal_rule(top - 26.0, text_width());
            self.y = top - 60.0;
        } else {
            self.y = top - 20.0;
        }
    }

    /// Write the contents onto the pages reserved for it. Every row points at
    /// the page its chapter really starts on, and is clickable.
    fn fill_contents(
        &mut self,
        slots: &[usize],
        layout: &[usize],
        story: &Story,
        starts: &[usize],
    ) {
        if slots.is_empty() {
            return;
        }
        if !self.ops.is_empty() {
            self.close_page();
        }
        let mut index = 0usize;
        for (page, per_page) in layout.iter().enumerate() {
            self.begin_contents_page(page == 0);
            for _ in 0..*per_page {
                let chapter = &story.chapters[index];
                let row = self.y;
                let number = format!("{:02}", index + 1);
                let page_text = format!("{}", starts[index] + 1);
                let page_x = PAGE_WIDTH - MARGIN_RIGHT - measure(&page_text, 11.0, Face::Roman);
                self.text_at(MARGIN_LEFT, row, 9.5, Face::Bold, &number);
                let title_x = MARGIN_LEFT + 30.0;
                let title = fit(chapter.title.trim(), 12.5, page_x - title_x - 24.0);
                self.text_at(title_x, row, 12.5, Face::Roman, &title);
                self.text_at(page_x, row, 11.0, Face::Roman, &page_text);
                self.links.push(Link {
                    page: starts[index],
                    x0: MARGIN_LEFT - 6.0,
                    y0: row - 5.0,
                    x1: PAGE_WIDTH - MARGIN_RIGHT + 6.0,
                    y1: row + 12.0,
                });
                self.y -= ENTRY_LEADING;
                index += 1;
            }
            let number = slots[page] + 1;
            self.push_footer(number);
            let filled = Page {
                ops: std::mem::take(&mut self.ops),
                links: std::mem::take(&mut self.links),
            };
            self.pages[slots[page]] = filled;
        }
    }

    fn finish(mut self, outline: &[(String, usize)]) -> Vec<u8> {
        if !self.ops.is_empty() {
            self.close_page();
        }
        assemble(&self.title, &self.pages, outline)
    }
}

/// How many chapter rows fit on each contents page. The same numbers drive the
/// drawing, so the reserved slots always match what gets written.
pub(crate) fn contents_layout(chapters: usize) -> Vec<usize> {
    let top = PAGE_HEIGHT - MARGIN_TOP;
    let mut layout = Vec::new();
    let mut left = chapters;
    let mut y = top - 60.0;
    while left > 0 {
        let mut count = 0;
        let mut cursor = y;
        while left > 0 && cursor - 16.0 >= MARGIN_BOTTOM {
            count += 1;
            left -= 1;
            cursor -= ENTRY_LEADING;
        }
        if count == 0 {
            break;
        }
        layout.push(count);
        y = top - 20.0;
    }
    layout
}

pub(crate) fn assemble(title: &str, pages: &[Page], outline: &[(String, usize)]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    // The binary comment marks the file as binary for transfer tools.
    out.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");
    let mut offsets: Vec<usize> = Vec::new();

    // Object numbers are fixed before anything is written, because a page has
    // to name the link annotations that belong to it.
    let page_object = |index: usize| 7 + index * 2;
    let outline_root = 7 + pages.len() * 2;
    let outline_first = outline_root + 1;
    let link_start = outline_first + outline.len();
    let mut link_numbers = Vec::with_capacity(pages.len());
    let mut next_link = link_start;
    for page in pages {
        link_numbers.push(next_link);
        next_link += page.links.len();
    }

    let mut catalog = String::from("<< /Type /Catalog /Pages 2 0 R");
    if !outline.is_empty() {
        catalog.push_str(&format!(
            " /Outlines {outline_root} 0 R /PageMode /UseOutlines"
        ));
    }
    catalog.push_str(" >>");
    push_object(&mut out, &mut offsets, 1, catalog.as_bytes());

    let kids: Vec<String> = (0..pages.len())
        .map(|index| format!("{} 0 R", page_object(index)))
        .collect();
    push_object(
        &mut out,
        &mut offsets,
        2,
        format!(
            "<< /Type /Pages /Kids [{}] /Count {} >>",
            kids.join(" "),
            pages.len()
        )
        .as_bytes(),
    );
    push_object(
        &mut out,
        &mut offsets,
        3,
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>",
    );
    push_object(
        &mut out,
        &mut offsets,
        4,
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>",
    );
    push_object(
        &mut out,
        &mut offsets,
        5,
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Oblique /Encoding /WinAnsiEncoding >>",
    );

    let mut info = b"<< /Producer (Folio) /Creator (Folio) /Title (".to_vec();
    escape_into(&mut info, title);
    info.extend_from_slice(b") >>");
    push_object(&mut out, &mut offsets, 6, &info);

    for (index, page) in pages.iter().enumerate() {
        let object = page_object(index);
        let stream = object + 1;
        let mut body = format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {PAGE_WIDTH} {PAGE_HEIGHT}] \
             /Resources << /Font << /F1 3 0 R /F2 4 0 R /F3 5 0 R >> >> \
             /Contents {stream} 0 R"
        );
        if !page.links.is_empty() {
            let refs: Vec<String> = (0..page.links.len())
                .map(|offset| format!("{} 0 R", link_numbers[index] + offset))
                .collect();
            body.push_str(&format!(" /Annots [{}]", refs.join(" ")));
        }
        body.push_str(" >>");
        push_object(&mut out, &mut offsets, object, body.as_bytes());

        let content = &page.ops;
        let mut stream_body = format!("<< /Length {} >>\nstream\n", content.len()).into_bytes();
        stream_body.extend_from_slice(content);
        if !content.ends_with(b"\n") {
            stream_body.push(b'\n');
        }
        stream_body.extend_from_slice(b"endstream");
        push_object(&mut out, &mut offsets, stream, &stream_body);
    }

    if !outline.is_empty() {
        push_object(
            &mut out,
            &mut offsets,
            outline_root,
            format!(
                "<< /Type /Outlines /First {outline_first} 0 R /Last {} 0 R /Count {} >>",
                outline_first + outline.len() - 1,
                outline.len()
            )
            .as_bytes(),
        );
        for (index, (label, page)) in outline.iter().enumerate() {
            let number = outline_first + index;
            let mut body = b"<< /Title (".to_vec();
            escape_into(&mut body, label);
            body.extend_from_slice(format!(") /Parent {outline_root} 0 R").as_bytes());
            if index > 0 {
                body.extend_from_slice(format!(" /Prev {} 0 R", number - 1).as_bytes());
            }
            if index + 1 < outline.len() {
                body.extend_from_slice(format!(" /Next {} 0 R", number + 1).as_bytes());
            }
            body.extend_from_slice(format!(" /Dest {}", dest(page_object(*page))).as_bytes());
            body.extend_from_slice(b" >>");
            push_object(&mut out, &mut offsets, number, &body);
        }
    }

    for (index, page) in pages.iter().enumerate() {
        for (offset, link) in page.links.iter().enumerate() {
            let number = link_numbers[index] + offset;
            let body = format!(
                "<< /Type /Annot /Subtype /Link /Rect [{:.2} {:.2} {:.2} {:.2}] \
                 /Border [0 0 0] /Dest {} >>",
                link.x0,
                link.y0,
                link.x1,
                link.y1,
                dest(page_object(link.page))
            );
            push_object(&mut out, &mut offsets, number, body.as_bytes());
        }
    }

    let xref_at = out.len();
    let count = offsets.len() + 1;
    out.extend_from_slice(format!("xref\n0 {count}\n").as_bytes());
    out.extend_from_slice(b"0000000000 65535 f \n");
    for offset in &offsets {
        out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {count} /Root 1 0 R /Info 6 0 R >>\nstartxref\n{xref_at}\n%%EOF\n"
        )
        .as_bytes(),
    );
    out
}

/// The whole story as one PDF: a title page, a contents page, then every
/// chapter. Each chapter also gets a bookmark, so a reader can jump straight
/// from the index to the page it starts on.
pub fn story_pdf(story: &Story) -> Vec<u8> {
    let mut doc = Doc::new(&story.title);
    doc.title_page(story);
    if story.chapters.is_empty() {
        return doc.finish(&[]);
    }

    // A contents page has to name the page each chapter begins on, so its
    // pages are reserved before the chapters are laid out and filled after.
    let layout = if story.chapters.len() > 1 {
        contents_layout(story.chapters.len())
    } else {
        Vec::new()
    };
    let slots: Vec<usize> = layout.iter().map(|_| doc.reserve_page()).collect();

    let mut starts = Vec::new();
    for (index, chapter) in story.chapters.iter().enumerate() {
        doc.new_page();
        starts.push(doc.pages.len());
        doc.chapter_heading(index + 1, &chapter.title);
        doc.chapter_body(&chapter.body);
    }
    doc.fill_contents(&slots, &layout, story, &starts);

    let outline: Vec<(String, usize)> = story
        .chapters
        .iter()
        .enumerate()
        .map(|(index, chapter)| {
            (
                format!("{} · {}", index + 1, chapter.title.trim()),
                starts[index],
            )
        })
        .collect();
    doc.finish(&outline)
}
