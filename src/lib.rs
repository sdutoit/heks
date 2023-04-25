pub mod source;
pub mod terminal;

use crossterm::event::{poll, read, KeyCode, KeyEvent, KeyModifiers};
use log::debug;
use source::DataSource;
use std::{
    cmp::min,
    io,
    ops::Range,
    sync::{atomic::AtomicBool, Arc, Mutex},
    time::Duration,
};
use tokio::time::{sleep_until, Instant};
use tui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::Style,
    text::{Span, Spans},
    widgets::{Block, Paragraph, Widget},
    Frame, Terminal,
};

use crate::terminal::color;

#[derive(Clone)]
struct HexDisplay {
    style: Style,
    data: Vec<u8>,
    data_start: u64,
    pub cursor: Cursor,
}

impl HexDisplay {
    fn default() -> Self {
        HexDisplay {
            style: Style::default(),
            data: vec![],
            data_start: 0,
            cursor: Cursor { start: 0, end: 0 },
        }
    }

    fn set_data(&mut self, data: Vec<u8>, data_start: u64) {
        self.data = data;
        self.data_start = data_start;
    }

    fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
}

const COLUMNS: u8 = 2 * 8;

fn render_hex(bytes: &[u8], bytes_start: u64, cursor: Cursor) -> Vec<Spans> {
    // let mut result: Option<Text> = None;
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
                if byte == cursor.start {
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
struct UnicodeDisplay {
    style: Style,
    data: Vec<u8>,
    data_start: u64,
}

impl UnicodeDisplay {
    fn default() -> Self {
        UnicodeDisplay {
            style: Style::default(),
            data: vec![],
            data_start: 0,
        }
    }

    fn set_data(&mut self, data: Vec<u8>, data_start: u64) {
        self.data = data;
        self.data_start = data_start;
    }

    fn style(mut self, style: Style) -> Self {
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

fn render_unicode_byte(byte: u8) -> String {
    let high = byte / 16;
    let low = byte % 16;

    format!(
        "{}{}",
        unicode_superscript_hex(high),
        unicode_superscript_hex(low)
    )
}

fn render_unicode(bytes: &[u8]) -> String {
    let mut column = 0;
    let mut result = String::new();
    bytes
        .iter()
        .map(|&c| match c {
            0 => "⓪ ".to_string(),
            1 => "① ".to_string(),
            2 => "② ".to_string(),
            3 => "③ ".to_string(),
            4 => "④ ".to_string(),
            5 => "⑤ ".to_string(),
            6 => "⑥ ".to_string(),
            7 => "⑦ ".to_string(),
            8 => "⑧ ".to_string(),
            9 => "⑨ ".to_string(),
            0xa => "Ⓐ ".to_string(),
            0xb => "Ⓑ ".to_string(),
            0xc => "Ⓒ ".to_string(),
            0xd => "Ⓓ ".to_string(),
            0xe => "Ⓔ ".to_string(),
            0xf => "Ⓕ ".to_string(),
            0x10 => "0̚ ".to_string(),
            0x11 => "1̚ ".to_string(),
            0x12 => "2̚ ".to_string(),
            0x13 => "3̚ ".to_string(),
            0x14 => "4̚ ".to_string(),
            0x15 => "5̚ ".to_string(),
            0x16 => "6̚ ".to_string(),
            0x17 => "7̚ ".to_string(),
            0x18 => "8̚ ".to_string(),
            0x19 => "9̚ ".to_string(),
            0x1a => "a̚ ".to_string(),
            0x1b => "b̚ ".to_string(),
            0x1c => "c̚ ".to_string(),
            0x1d => "d̚ ".to_string(),
            0x1e => "e̚ ".to_string(),
            0x1f => "f̚ ".to_string(),
            0x7f..=0xff => render_unicode_byte(c),
            _ => format!("{} ", c as char),
        })
        .for_each(|s| {
            result.push_str(s.as_str());
            column += 1;
            if column == COLUMNS {
                result.push('\n');
                column = 0;
            }
        });
    result
}

impl Widget for UnicodeDisplay {
    fn render(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        let text = render_unicode(&self.data);

        Paragraph::new(text).style(self.style).render(area, buf);
    }
}

#[derive(Debug, Clone, Copy)]
struct Cursor {
    start: u64,
    end: u64, // one past the last character
}

impl Cursor {
    fn contains(&self, location: u64) -> bool {
        self.start <= location && location < self.end
    }

    fn increment(&mut self, delta: u64) {
        let width = self.end - self.start;
        self.end = self.end.saturating_add(delta);
        self.start = self.end - width;
    }

    fn decrement(&mut self, delta: u64) {
        let width = self.end - self.start;
        self.start = self.start.saturating_sub(delta);
        self.end = self.start + width;
    }

    fn grow(&mut self) {
        self.end = self.end.saturating_add(1);
    }

    fn shrink(&mut self) {
        if self.end > self.start + 1 {
            self.end -= 1;
        }
    }

    fn skip_right(&mut self) {
        assert!(self.start <= self.end);

        let width = self.end - self.start;

        self.end = self.end.saturating_add(width);
        self.start = self.end - width;
    }

    fn skip_left(&mut self) {
        assert!(self.start <= self.end);

        let width = self.end - self.start;

        self.start = self.start.saturating_sub(width);
        self.end = self.start + width;
    }

    fn clamp(&mut self, range: Range<u64>) {
        let width = min(self.end - self.start, range.end - range.start);
        if self.end > range.end {
            self.end = range.end;
            self.start = self.end - width;
        } else if self.start < range.start {
            self.start = range.start;
            self.end = self.start - width;
        }
    }
}

pub struct App {
    source: Box<dyn DataSource>,
    hex_display: HexDisplay,
    unicode_display: UnicodeDisplay,
    cursor: Cursor,
}

impl App {
    pub fn new<B: Backend>(
        terminal: &mut Terminal<B>,
        source: Box<dyn DataSource>,
    ) -> Result<Self, io::Error> {
        terminal.hide_cursor()?;

        let style_hex = Style::default()
            .bg(color(32, 32, 32))
            .fg(color(192, 192, 192));

        let hex_display = HexDisplay::default().style(style_hex);

        let style_unicode = Style::default()
            .bg(color(64, 64, 64))
            .fg(color(192, 192, 192));

        let unicode_display = UnicodeDisplay::default().style(style_unicode);

        Ok(App {
            source,
            hex_display,
            unicode_display,
            cursor: Cursor { start: 0, end: 1 },
        })
    }

    fn draw<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<(), io::Error> {
        terminal.draw(|f| self.paint(f))?;

        Ok(())
    }

    fn paint<B: Backend>(&mut self, f: &mut Frame<B>) {
        let style_frame = Style::default()
            .bg(color(0, 0, 192))
            .fg(color(224, 224, 224));

        let stack = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Length(1),
                    Constraint::Min(1),
                    Constraint::Length(1),
                ]
                .as_ref(),
            )
            .split(f.size());

        let header = Block::default()
            .style(style_frame)
            .title(self.source.name())
            .title_alignment(Alignment::Center);
        f.render_widget(header, stack[0]);

        let display_areas = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(stack[1]);

        // TODO clamp the cursor to the data source? or do we do that after we
        // fetch? I'd prefer no a priori knowledge about the data source's size.

        let ui_columns = COLUMNS as u64;
        let ui_rows = display_areas[0].height as u64;

        let pos = self.cursor.start;
        let column_zero_pos: u64 = pos.saturating_sub(pos % ui_columns);

        let pos_row = column_zero_pos / ui_columns;

        let ui_pos_row = (ui_rows / 2).min(pos_row);
        let ui_first_pos = column_zero_pos - ui_pos_row * ui_columns;
        let ui_view_end = ui_first_pos + ui_rows * ui_columns;

        let slice = self.source.fetch(ui_first_pos, ui_view_end);

        self.cursor.clamp(slice.location.clone());

        self.hex_display.cursor = self.cursor;
        self.hex_display
            .set_data(slice.data.to_vec(), slice.location.start);
        self.unicode_display
            .set_data(slice.data.to_vec(), slice.location.start);

        f.render_widget(self.hex_display.clone(), display_areas[0]);
        f.render_widget(self.unicode_display.clone(), display_areas[1]);

        let footer = Block::default()
            .style(style_frame)
            .title("heks 🧹")
            .title_alignment(Alignment::Center);
        f.render_widget(footer, stack[2]);
    }

    fn on_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Char('l')) | (KeyModifiers::NONE, KeyCode::Right) => {
                self.cursor.increment(1);
            }

            (KeyModifiers::NONE, KeyCode::Char('h')) | (KeyModifiers::NONE, KeyCode::Left) => {
                self.cursor.decrement(1);
            }
            (KeyModifiers::NONE, KeyCode::Char('j')) | (KeyModifiers::NONE, KeyCode::Down) => {
                self.cursor.increment(COLUMNS.into());
            }

            (KeyModifiers::NONE, KeyCode::Char('k')) | (KeyModifiers::NONE, KeyCode::Up) => {
                if self.cursor.start >= COLUMNS.into() {
                    self.cursor.decrement(COLUMNS.into());
                }
            }

            (KeyModifiers::SHIFT, KeyCode::Char('L')) => {
                self.cursor.grow();
            }

            (KeyModifiers::SHIFT, KeyCode::Char('H')) => {
                self.cursor.shrink();
            }

            (KeyModifiers::NONE, KeyCode::Tab)
            | (
                KeyModifiers::ALT,
                KeyCode::Char('f'), // Should be KeyCode::Right, but that's what I get from crossterm..
            ) => {
                self.cursor.skip_right();
            }

            (KeyModifiers::SHIFT, KeyCode::BackTab)
            | (
                KeyModifiers::ALT,
                KeyCode::Char('b'), // Should be KeyCode::Left, but that's what I get from crossterm..
            ) => {
                self.cursor.skip_left();
            }

            (_, _) => {
                debug!("key event: {:?}", key);
            }
        };
    }
}

pub struct EventLoop<B: Backend> {
    pub terminal: Arc<Mutex<Terminal<B>>>,
    pub app: App,
    pub done: Arc<AtomicBool>,
}

impl<B: Backend> EventLoop<B> {
    pub fn new(terminal: Terminal<B>, app: App) -> Self {
        EventLoop::<B> {
            terminal: Arc::new(Mutex::new(terminal)),
            app,
            done: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn run(&mut self) -> io::Result<()> {
        let mut next_tick = Instant::now();

        while !self.done.load(std::sync::atomic::Ordering::Acquire) {
            sleep_until(next_tick).await;

            self.tick()?;

            next_tick += Duration::from_micros(16667);
            let now = Instant::now();
            if next_tick < now {
                next_tick = now;
            }
        }

        Ok(())
    }

    pub fn tick(&mut self) -> io::Result<()> {
        while poll(Duration::from_secs(0))? {
            let event = read()?;
            match event {
                crossterm::event::Event::FocusGained => {}
                crossterm::event::Event::FocusLost => {}
                crossterm::event::Event::Key(key) => match (key.modifiers, key.code) {
                    (KeyModifiers::NONE, KeyCode::Esc)
                    | (KeyModifiers::NONE, KeyCode::Char('q')) => {
                        self.done.store(true, std::sync::atomic::Ordering::Release);
                    }

                    (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
                        nix::sys::signal::kill(nix::unistd::getpid(), nix::sys::signal::SIGINT)
                            .ok();
                    }

                    (KeyModifiers::CONTROL, KeyCode::Char('z')) => {
                        nix::sys::signal::kill(nix::unistd::getpid(), nix::sys::signal::SIGTSTP)
                            .ok();
                    }
                    (_, _) => self.app.on_key(key),
                },
                crossterm::event::Event::Mouse(_) => {}
                crossterm::event::Event::Paste(_) => {}
                crossterm::event::Event::Resize(_, _) => {}
            }
        }

        let mut terminal = self.terminal.lock().unwrap();
        self.app.draw(&mut terminal)?;

        Ok(())
    }
}
