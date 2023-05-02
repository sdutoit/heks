pub mod cursor;
pub mod display;
pub mod source;
pub mod terminal;

use crate::cursor::{Cursor, CursorStack};
use crate::display::{HexDisplay, UnicodeDisplay};
use crate::terminal::color;
use crossterm::event::{poll, read, Event, KeyCode, KeyEvent, KeyModifiers};
use display::COLUMNS;
use itertools::Itertools;
use log::debug;
use nix::{sys::signal, unistd::getpid};
use source::{DataSource, Slice};
use std::{
    io,
    sync::{atomic::AtomicBool, Arc, Mutex},
    time::Duration,
};
use terminal::color_hsl;
use tokio::time::{sleep_until, Instant};
use tui::layout::Rect;
use tui::text::{Span, Spans};
use tui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::Style,
    widgets::Block,
    Frame, Terminal,
};

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

        let (area_header, area_display, area_footer) = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Length(1),
                    Constraint::Min(1),
                    Constraint::Length(1),
                ]
                .as_ref(),
            )
            .split(f.size())
            .into_iter()
            .collect_tuple()
            .unwrap();

        let header = Block::default()
            .style(style_frame)
            .title(format!("{} - {}", self.source.name(), "𝓱𝓮𝓴𝓼"))
            .title_alignment(Alignment::Center);
        f.render_widget(header, area_header);

        let ui_columns = COLUMNS as u64;
        self.display_height = area_display.height;
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

        App::paint_display(
            f,
            area_display,
            self.hex_display.clone(),
            self.unicode_display.clone(),
            cursor,
            slice,
        );

        let rainbow = self.rainbow(area_footer.width as usize);
        let footer = Block::default()
            .style(style_frame)
            .title(rainbow)
            .title_alignment(Alignment::Center);
        f.render_widget(footer, area_footer);
    }

    fn paint_display<B: Backend>(
        f: &mut Frame<B>,
        area: Rect,
        mut hex_display: HexDisplay,
        mut unicode_display: UnicodeDisplay,
        cursor: Cursor,
        slice: Slice,
    ) {
        let display_areas = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(area);

        hex_display.cursor = cursor;
        hex_display.set_data(slice.data.to_vec(), slice.location.start);

        unicode_display.cursor = cursor;
        unicode_display.set_data(slice.data.to_vec(), slice.location.start);

        f.render_widget(hex_display, display_areas[0]);
        f.render_widget(unicode_display, display_areas[1]);
    }

    fn rainbow<'a>(&self, width: usize) -> Spans<'a> {
        let mut result: Vec<Span> = vec![];

        let fraction = self.source.fraction(self.cursor_stack.top().start);

        let broom_start = (fraction * ((width - 2) as f64)) as usize;
        let broom_start = broom_start.clamp(0, width.saturating_sub(2));
        // assume that '🧹' takes up the same horizontal space as two regular characters
        const BROOM_WIDTH: usize = 2;
        for i in 0..width {
            let hue = i as f64 * 360.0 / (width - 1).max(1) as f64 + fraction * 180.0;
            let saturation = 1.0;
            let lightness = 0.5;
            let fg = color_hsl(hue, saturation, lightness);
            let bg = color_hsl(hue + 0.0, saturation, 0.1);

            let style = Style::default().fg(fg).bg(bg);
            let invert_style = Style::default().fg(bg).bg(fg);

            if i == broom_start {
                result.push(Span::styled("🧹", invert_style));
            } else if i > broom_start && i < broom_start + BROOM_WIDTH {
                // this space is taken up by the rest of the broom.
            } else {
                result.push(Span::styled("▓", style));
            }
        }

        Spans::from(result)
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
