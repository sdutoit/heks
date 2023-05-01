use tui::{
    style::Style,
    text::{Span, Spans},
    widgets::{Paragraph, Widget},
};

use crate::{cursor::Cursor, terminal::color};

#[derive(Clone)]
pub struct HexDisplay {
    style: Style,
    data: Vec<u8>,
    data_start: u64,
    pub cursor: Cursor,
}

impl HexDisplay {
    pub fn default() -> Self {
        HexDisplay {
            style: Style::default(),
            data: vec![],
            data_start: 0,
            cursor: Cursor { start: 0, end: 0 },
        }
    }

    pub fn set_data(&mut self, data: Vec<u8>, data_start: u64) {
        self.data = data;
        self.data_start = data_start;
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
}

pub const COLUMNS: u8 = 2 * 8;

fn render_hex(bytes: &[u8], bytes_start: u64, cursor: Cursor) -> Vec<Spans> {
    let mut lines: Vec<Spans> = vec![];
    let mut spans = vec![];

    const BLOCKSIZE: u8 = 2;
    let mut column = 0;
    let mut byte = bytes_start;

    let cursor_style = Style::default().bg(color(0, 96, 0)).fg(color(96, 255, 96));
    bytes.iter().for_each(|value| {
        let style = if cursor.contains(byte) {
            cursor_style
        } else {
            Style::default()
        };
        if column > 0 && column % BLOCKSIZE == 0 {
            spans.push(Span::styled(
                " ",
                if byte == cursor.start() {
                    Style::default()
                } else {
                    style
                },
            ));
        };

        let text = format!("{:02x}", value);
        spans.push(Span::styled(text, style));

        column += 1;
        if column == COLUMNS {
            lines.push(Spans::from(spans.clone()));
            spans.clear();
            column = 0;
        }

        byte += 1;
    });

    if !spans.is_empty() {
        lines.push(Spans::from(spans.clone()));
    }

    lines
}

impl Widget for HexDisplay {
    fn render(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        Paragraph::new(render_hex(&self.data, self.data_start, self.cursor))
            .style(self.style)
            .render(area, buf);
    }
}

#[derive(Clone)]
pub struct UnicodeDisplay {
    style: Style,
    data: Vec<u8>,
    data_start: u64,
    pub cursor: Cursor,
}

impl UnicodeDisplay {
    pub fn default() -> Self {
        UnicodeDisplay {
            style: Style::default(),
            data: vec![],
            data_start: 0,
            cursor: Cursor::new(0, 0),
        }
    }

    pub fn set_data(&mut self, data: Vec<u8>, data_start: u64) {
        self.data = data;
        self.data_start = data_start;
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
}

fn unicode_superscript_hex(byte: u8) -> char {
    match byte {
        0x0 => '⁰',
        0x1 => '¹',
        0x2 => '²',
        0x3 => '³',
        0x4 => '⁴',
        0x5 => '⁵',
        0x6 => '⁶',
        0x7 => '⁷',
        0x8 => '⁸',
        0x9 => '⁹',
        0xa => 'ᵃ',
        0xb => 'ᵇ',
        0xc => 'ᶜ',
        0xd => 'ᵈ',
        0xe => 'ᵉ',
        0xf => 'ᶠ',
        _ => {
            panic!("value passed in is too large");
        }
    }
}

fn render_unicode_byte_as_hex(byte: u8) -> String {
    let high = byte / 16;
    let low = byte % 16;

    format!(
        "{}{}",
        unicode_superscript_hex(high),
        unicode_superscript_hex(low)
    )
}

fn render_unicode_byte(byte: u8) -> String {
    match byte {
        // non-printable lower range
        0 => "  ".to_string(),
        0x01..=0x1f => render_unicode_byte_as_hex(byte),

        // printable ASCII
        0x20..=0x7e => format!("{} ", byte as char),

        // non-printable upper range, including not just everything with the
        // high bit set (non-ASCII), but also 127/0x7f (DEL).
        0x7f..=0xff => render_unicode_byte_as_hex(byte),
    }
}

fn render_unicode(bytes: &[u8], bytes_start: u64, cursor: Cursor) -> Vec<Spans> {
    let mut column = 0;
    let mut lines: Vec<Spans> = vec![];
    let mut spans = vec![];

    let mut byte = bytes_start;

    let cursor_style = Style::default().bg(color(0, 96, 0)).fg(color(96, 255, 96));
    bytes.iter().map(|b| render_unicode_byte(*b)).for_each(|s| {
        let style = if cursor.contains(byte) {
            cursor_style
        } else {
            Style::default()
        };
        spans.push(Span::styled(s, style));
        column += 1;
        if column == COLUMNS {
            lines.push(Spans::from(spans.clone()));
            spans.clear();
            column = 0;
        }

        byte += 1;
    });

    if !spans.is_empty() {
        lines.push(Spans::from(spans.clone()));
    }

    lines
}

impl Widget for UnicodeDisplay {
    fn render(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        let text = render_unicode(&self.data, self.data_start, self.cursor);

        Paragraph::new(text).style(self.style).render(area, buf);
    }
}
