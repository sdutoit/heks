pub mod cursor;
pub mod source;
pub mod terminal;

use crate::cursor::{Cursor, CursorStack};
use crate::terminal::color;
use crossterm::event::{poll, read, Event, KeyCode, KeyEvent, KeyModifiers};
use log::debug;
use nix::{sys::signal, unistd::getpid};
use source::DataSource;
use std::{
    io,
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
struct UnicodeDisplay {
    style: Style,
    data: Vec<u8>,
    data_start: u64,
    pub cursor: Cursor,
}

impl UnicodeDisplay {
    fn default() -> Self {
        UnicodeDisplay {
            style: Style::default(),
            data: vec![],
            data_start: 0,
            cursor: Cursor::new(0, 0),
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

pub struct App {
    source: Box<dyn DataSource>,
    hex_display: HexDisplay,
    unicode_display: UnicodeDisplay,
    cursor_stack: CursorStack,
    display_height: u16, // Number of rows in the content displays
    last_key: Option<KeyEvent>,
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
            cursor_stack: CursorStack::new(Cursor::new(0, 1)),
            display_height: 0,
            last_key: None,
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
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(stack[1]);

        let ui_columns = COLUMNS as u64;
        self.display_height = display_areas[0].height;
        let ui_rows = self.display_height as u64;

        // We'll clamp the cursor to within the slice we managed to fetch from
        // the source further down, but for now let's not make any assumptions
        // about it. For example, it may have been set to u64::MAX to skip to
        // the end.
        let mut cursor = self.cursor_stack.top();
        let pos = cursor.start().min(u64::MAX - ui_columns * ui_rows);
        let column_zero_pos: u64 = pos.saturating_sub(pos % ui_columns);

        let pos_row = column_zero_pos / ui_columns;

        let ui_pos_row = (ui_rows / 2).min(pos_row);
        let ui_first_pos = column_zero_pos - ui_pos_row * ui_columns;
        let ui_view_end = ui_first_pos + ui_rows * ui_columns;

        let slice = self.source.fetch(ui_first_pos, ui_view_end);
        let slice = slice.align_up(COLUMNS as u64);

        cursor.clamp(slice.location.clone());
        *self.cursor_stack.top_mut() = cursor;

        self.hex_display.cursor = cursor;
        self.hex_display
            .set_data(slice.data.to_vec(), slice.location.start);

        self.unicode_display.cursor = cursor;
        self.unicode_display
            .set_data(slice.data.to_vec(), slice.location.start);

        f.render_widget(self.hex_display.clone(), display_areas[0]);
        f.render_widget(self.unicode_display.clone(), display_areas[1]);

        let footer = Block::default()
            .style(style_frame)
            .title("🧹 𝓱𝓮𝓴𝓼 🧹")
            .title_alignment(Alignment::Center);
        f.render_widget(footer, stack[2]);
    }

    fn push_cursor_if_key_changed_else_set<F>(&mut self, key: &KeyEvent, f: F)
    where
        F: FnOnce(&mut Cursor) -> (),
    {
        let mut cursor = self.cursor_stack.top();

        f(&mut cursor);

        if self.last_key == Some(*key) {
            *self.cursor_stack.top_mut() = cursor;
        } else {
            self.cursor_stack.push(cursor);
        }
    }

    fn on_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Char('l')) | (KeyModifiers::NONE, KeyCode::Right) => {
                self.cursor_stack.top_mut().increment(1)
            }

            (KeyModifiers::NONE, KeyCode::Char('h')) | (KeyModifiers::NONE, KeyCode::Left) => {
                self.cursor_stack.top_mut().decrement(1);
            }
            (KeyModifiers::NONE, KeyCode::Char('j')) | (KeyModifiers::NONE, KeyCode::Down) => {
                self.cursor_stack.top_mut().increment(COLUMNS.into());
            }

            (KeyModifiers::NONE, KeyCode::Char('k')) | (KeyModifiers::NONE, KeyCode::Up) => {
                if self.cursor_stack.top().start() >= COLUMNS.into() {
                    self.cursor_stack.top_mut().decrement(COLUMNS.into());
                }
            }

            (KeyModifiers::SHIFT, KeyCode::Char('L')) => self.cursor_stack.top_mut().grow(),
            (KeyModifiers::SHIFT, KeyCode::Char('H')) => self.cursor_stack.top_mut().shrink(),

            (KeyModifiers::NONE, KeyCode::Tab)
            | (
                KeyModifiers::ALT,
                KeyCode::Char('f'), // Should be KeyCode::Right, but that's what I get from crossterm..
            ) => {
                self.cursor_stack.top_mut().skip_right();
            }

            (KeyModifiers::SHIFT, KeyCode::BackTab)
            | (
                KeyModifiers::ALT,
                KeyCode::Char('b'), // Should be KeyCode::Left, but that's what I get from crossterm..
            ) => {
                self.cursor_stack.top_mut().skip_left();
            }

            (KeyModifiers::NONE, KeyCode::PageDown) => {
                let page_size = COLUMNS as u64 * (self.display_height as u64 / 2);
                self.push_cursor_if_key_changed_else_set(&key, |cursor| {
                    cursor.increment(page_size)
                });
            }

            (KeyModifiers::NONE, KeyCode::PageUp) => {
                let page_size = COLUMNS as u64 * (self.display_height as u64 / 2);
                self.push_cursor_if_key_changed_else_set(&key, |cursor| {
                    cursor.decrement(page_size)
                });
            }

            (KeyModifiers::NONE, KeyCode::Home) => {
                let mut cursor = self.cursor_stack.top().clone();
                cursor.decrement(u64::MAX);
                self.cursor_stack.push(cursor);
            }

            (KeyModifiers::NONE, KeyCode::End) => {
                let mut cursor = self.cursor_stack.top().clone();
                cursor.increment(u64::MAX);
                self.cursor_stack.push(cursor);
            }

            (KeyModifiers::NONE, KeyCode::Char('z')) => self.cursor_stack.undo(),
            (KeyModifiers::SHIFT, KeyCode::Char('Z')) => self.cursor_stack.redo(),

            (_, _) => {
                debug!("key event: {:?}", key);
            }
        };

        self.last_key = Some(key);
    }
}

pub struct EventLoop<B: Backend> {
    pub terminal: Arc<Mutex<Terminal<B>>>,
    pub app: App,
    pub done: Arc<AtomicBool>,
    pub dirty: Arc<AtomicBool>,
}

impl<B: Backend> EventLoop<B> {
    pub fn new(terminal: Terminal<B>, app: App) -> Self {
        EventLoop::<B> {
            terminal: Arc::new(Mutex::new(terminal)),
            app,
            done: Arc::new(AtomicBool::new(false)),
            dirty: Arc::new(AtomicBool::new(true)),
        }
    }

    pub fn mark_dirty(&mut self) {
        self.dirty.store(true, std::sync::atomic::Ordering::Release);
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
        if self.handle_events()? {
            self.dirty.store(true, std::sync::atomic::Ordering::Release);
        }

        if self.dirty.swap(false, std::sync::atomic::Ordering::Acquire) {
            let mut terminal = self.terminal.lock().unwrap();
            self.app.draw(&mut terminal)?;
        }

        Ok(())
    }

    fn handle_events(&mut self) -> io::Result<bool> {
        let mut seen_event = false;
        while poll(Duration::from_secs(0))? {
            seen_event = true;

            let event = read()?;
            match event {
                Event::FocusGained => {}
                Event::FocusLost => {}
                Event::Key(key) => match (key.modifiers, key.code) {
                    (KeyModifiers::NONE, KeyCode::Esc)
                    | (KeyModifiers::NONE, KeyCode::Char('q')) => {
                        self.done.store(true, std::sync::atomic::Ordering::Release);
                    }

                    (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
                        signal::kill(getpid(), signal::SIGINT).ok();
                    }

                    (KeyModifiers::CONTROL, KeyCode::Char('z')) => {
                        signal::kill(getpid(), signal::SIGTSTP).ok();
                    }

                    (_, _) => self.app.on_key(key),
                },
                Event::Mouse(_) => {}
                Event::Paste(_) => {}
                Event::Resize(_, _) => {}
            }
        }

        Ok(seen_event)
    }
}
