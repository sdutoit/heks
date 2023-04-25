pub mod terminal;

use crossterm::event::{poll, read, KeyCode, KeyEvent, KeyModifiers};
use log::debug;
use memmap2::{Mmap, MmapOptions};
use std::{
    cell::RefCell,
    cmp::min,
    fs::File,
    io::{self, ErrorKind},
    ops::Range,
    path::PathBuf,
    rc::Rc,
    sync::{atomic::AtomicBool, Arc, Mutex},
    time::Duration,
};
use tokio::time::{sleep_until, Instant};
use tui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Span, Spans},
    widgets::{Block, Paragraph, Widget},
    Frame, Terminal,
};

pub trait DataSource {
    fn name(&self) -> &str;
    fn fetch(&mut self, offset: u64, size: u32) -> &[u8];
}

struct DebugSource {
    buffer: &'static [u8],
}

#[allow(dead_code)]
impl DebugSource {
    fn new() -> Self {
        DebugSource {
            buffer: b"\x09\x00\x06\x00hello\
                      \x00\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0a\x0b\x0c\x0d\x0e\x0f\
                      \x10\x11\x12\x13\x14\x15\x16\x17\x18\x19\x1a\x1b\x1c\x1d\x1e\x1f\
                      \x7f\x80\x90\xa0\xb0\xc0\xd0\xe0\xf0\xf1\xf2\xf3\xf4\xf5\xf6\xf7\
                      \xf8\xf9\xfa\xfb\xfc\xfd\xfe\xff\
                      world01234567890",
        }
    }
}

impl DataSource for DebugSource {
    fn name(&self) -> &str {
        "debug"
    }
    fn fetch(&mut self, offset: u64, size: u32) -> &[u8] {
        &self.buffer[clamp(offset, size, self.buffer.len())]
    }
}

pub struct FileSource {
    name: String,
    mmap: Mmap,
}

impl FileSource {
    pub fn new(filename: &PathBuf) -> Result<Self, io::Error> {
        let name = filename
            .to_str()
            .ok_or(io::Error::new(
                ErrorKind::Other,
                format!("Unable to parse filename {:?}", filename),
            ))?
            .to_string();
        let file = File::open(filename)?;
        let mmap = unsafe { MmapOptions::new().map(&file)? };

        Ok(FileSource { name, mmap })
    }
}

fn clamp(offset: u64, size: u32, len: usize) -> Range<usize> {
    let begin: usize = min(offset as usize, len);
    let end: usize = min(offset as usize + size as usize, len);

    begin..end
}

impl DataSource for FileSource {
    fn name(&self) -> &str {
        self.name.as_str()
    }

    fn fetch(&mut self, offset: u64, size: u32) -> &[u8] {
        let range = clamp(offset, size, self.mmap.len());

        if !range.is_empty() {
            self.mmap.get(range).unwrap()
        } else {
            &[]
        }
    }
}

#[derive(Clone)]
struct HexDisplay {
    style: Style,
    source: Option<Rc<RefCell<Box<dyn DataSource>>>>,
    pub cursor_start: u64,
    pub cursor_end: u64,
}

impl HexDisplay {
    fn default() -> Self {
        HexDisplay {
            style: Style::default(),
            source: None,
            cursor_start: 0,
            cursor_end: 0,
        }
    }

    fn source(mut self, source: Rc<RefCell<Box<dyn DataSource>>>) -> Self {
        self.source = Some(source);
        self
    }

    fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }
}

const COLUMNS: u8 = 2 * 8;

fn render_hex(bytes: &[u8], bytes_start: u64, cursor_start: u64, cursor_end: u64) -> Vec<Spans> {
    // let mut result: Option<Text> = None;
    let mut lines: Vec<Spans> = vec![];
    let mut spans = vec![];

    const BLOCKSIZE: u8 = 2;
    let mut column = 0;
    let mut byte = bytes_start;

    const COLOR_CURSOR_BACKGROUND: Color = Color::Rgb(0, 96, 0);
    const COLOR_CURSOR_FOREGROUND: Color = Color::Rgb(96, 255, 96);
    let cursor_style = Style::default()
        .bg(COLOR_CURSOR_BACKGROUND)
        .fg(COLOR_CURSOR_FOREGROUND);
    bytes.iter().for_each(|value| {
        let style = if byte >= cursor_start && byte < cursor_end {
            cursor_style
        } else {
            Style::default()
        };
        if column > 0 && column % BLOCKSIZE == 0 {
            spans.push(Span::styled(
                " ",
                if byte == cursor_start {
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
        let data = self.source.unwrap();
        let mut data = data.borrow_mut();
        let data = data.fetch(0, 1024);
        // TODO get data_start from data.fetch()
        let data_start = 0;
        Paragraph::new(render_hex(
            data,
            data_start,
            self.cursor_start,
            self.cursor_end,
        ))
        .style(self.style)
        .render(area, buf);
    }
}

#[derive(Clone)]
struct UnicodeDisplay {
    style: Style,
    source: Option<Rc<RefCell<Box<dyn DataSource>>>>,
}

impl UnicodeDisplay {
    fn default() -> Self {
        UnicodeDisplay {
            style: Style::default(),
            source: None,
        }
    }

    fn source(mut self, source: Rc<RefCell<Box<dyn DataSource>>>) -> Self {
        self.source = Some(source);
        self
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
        let data = self.source.unwrap();
        let mut data = data.borrow_mut();
        let data = data.fetch(0, 1024);
        let text = render_unicode(data);

        Paragraph::new(text).style(self.style).render(area, buf);
    }
}

pub struct App {
    source_name: String,
    hex_display: HexDisplay,
    unicode_display: UnicodeDisplay,
    cursor_start: u64,
    cursor_end: u64, // one past the last character
}

impl App {
    pub fn new<B: Backend>(
        terminal: &mut Terminal<B>,
        source: Box<dyn DataSource>,
    ) -> Result<Self, io::Error> {
        terminal.hide_cursor()?;
        let source = Rc::new(RefCell::new(source));
        let source_name = source.borrow().name().to_string();

        const COLOR_HEX_BACKGROUND: Color = Color::Rgb(32, 32, 32);
        const COLOR_HEX_FOREGROUND: Color = Color::Rgb(192, 192, 192);
        let style_hex = Style::default()
            .bg(COLOR_HEX_BACKGROUND)
            .fg(COLOR_HEX_FOREGROUND);

        let hex_display = HexDisplay::default()
            .source(source.clone())
            .style(style_hex);

        const COLOR_UNICODE_BACKGROUND: Color = Color::Rgb(64, 64, 64);
        const COLOR_UNICODE_FOREGROUND: Color = Color::Rgb(192, 192, 192);
        let style_unicode = Style::default()
            .bg(COLOR_UNICODE_BACKGROUND)
            .fg(COLOR_UNICODE_FOREGROUND);

        let unicode_display = UnicodeDisplay::default()
            .source(source.clone())
            .style(style_unicode);

        Ok(App {
            source_name,
            hex_display,
            unicode_display,
            cursor_start: 0,
            cursor_end: 1,
        })
    }

    fn draw<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<(), io::Error> {
        self.send_state();
        terminal.draw(|f| self.paint(f))?;

        Ok(())
    }

    fn send_state(&mut self) {
        self.hex_display.cursor_start = self.cursor_start;
        self.hex_display.cursor_end = self.cursor_end;
    }

    fn paint<B: Backend>(&self, f: &mut Frame<B>) {
        const COLOR_FRAME_BACKGROUND: Color = Color::Rgb(0, 0, 64);
        const COLOR_FRAME_FOREGROUND: Color = Color::Rgb(128, 128, 192);
        let style_frame = Style::default()
            .bg(COLOR_FRAME_BACKGROUND)
            .fg(COLOR_FRAME_FOREGROUND);

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
            .title(self.source_name.clone())
            .title_alignment(Alignment::Center);
        f.render_widget(header, stack[0]);

        let file_display = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(stack[1]);

        f.render_widget(self.hex_display.clone(), file_display[0]);
        f.render_widget(self.unicode_display.clone(), file_display[1]);

        let footer = Block::default()
            .style(style_frame)
            .title("heks 🧹")
            .title_alignment(Alignment::Center);
        f.render_widget(footer, stack[2]);
    }

    fn on_key(&mut self, key: KeyEvent) {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Char('l')) | (KeyModifiers::NONE, KeyCode::Right) => {
                assert!(self.cursor_start <= self.cursor_end);

                if self.cursor_end < u64::MAX {
                    self.cursor_start += 1;
                    self.cursor_end += 1;
                }
            }

            (KeyModifiers::NONE, KeyCode::Char('h')) | (KeyModifiers::NONE, KeyCode::Left) => {
                assert!(self.cursor_end >= self.cursor_start);

                if self.cursor_start > 0 {
                    self.cursor_start -= 1;
                    self.cursor_end -= 1;
                }
            }

            (KeyModifiers::SHIFT, KeyCode::Char('L')) => {
                self.cursor_end = self.cursor_end.saturating_add(1);
            }

            (KeyModifiers::SHIFT, KeyCode::Char('H')) => {
                if self.cursor_end > self.cursor_start + 1 {
                    self.cursor_end -= 1;
                }
            }

            (KeyModifiers::NONE, KeyCode::Tab)
            | (
                KeyModifiers::ALT,
                KeyCode::Char('f'), // Should be KeyCode::Right, but that's what I get from crossterm..
            ) => {
                assert!(self.cursor_start <= self.cursor_end);

                let width = self.cursor_end - self.cursor_start;

                self.cursor_end = self.cursor_end.saturating_add(width);
                self.cursor_start = self.cursor_end - width;
            }

            (KeyModifiers::SHIFT, KeyCode::BackTab)
            | (
                KeyModifiers::ALT,
                KeyCode::Char('b'), // Should be KeyCode::Left, but that's what I get from crossterm..
            ) => {
                assert!(self.cursor_start <= self.cursor_end);

                let width = self.cursor_end - self.cursor_start;

                self.cursor_start = self.cursor_start.saturating_sub(width);
                self.cursor_end = self.cursor_start + width;
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
