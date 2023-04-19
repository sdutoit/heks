use crossterm::{
    event::{poll, read, DisableMouseCapture, EnableMouseCapture, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::{stream::FuturesUnordered, StreamExt};
use std::{
    io,
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};
use tokio::signal::unix::{signal, SignalKind};
use tokio::time::{sleep_until, Instant};
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout},
    widgets::{Block, Paragraph, Widget},
    Terminal,
};

struct UnicodeDisplay<'b, GetData: Fn(u32, u32) -> &'b [u8]> {
    get_data: Option<GetData>,
}

impl<'b, GetData: Fn(u32, u32) -> &'b [u8]> UnicodeDisplay<'b, GetData> {
    fn default() -> Self {
        UnicodeDisplay { get_data: None }
    }

    fn get_data(mut self, get_data: GetData) -> Self {
        self.get_data = Some(get_data);
        self
    }
}

fn render_unicode(bytes: &mut [u8]) -> String {
    let mut result = String::new();
    bytes
        .iter()
        .map(|&c| match c {
            0 => 'Ø',
            1 => '①',
            2 => '②',
            3 => '③',
            4 => '④',
            5 => '⑤',
            6 => '⑥',
            7 => '⑦',
            8 => '⑧',
            9 => '⑨',
            0xa => '🅐',
            0xb => '🅑',
            0xc => '🅒',
            0xd => '🅓',
            0xe => '🅔',
            0xf => '🅕',
            _ => c as char,
        })
        .for_each(|c| {
            result.push(c);
            result.push(' ');
        });
    result
}

impl<'b, GetData: Fn(u32, u32) -> &'b [u8]> Widget for UnicodeDisplay<'b, GetData> {
    fn render(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        let mut data = (self.get_data.unwrap())(0, 1024).to_vec();
        let text = render_unicode(&mut data[..]);

        Paragraph::new(text).render(area, buf);
    }
}

struct App<B: Backend> {
    terminal: Terminal<B>,
}

impl<B: Backend> App<B> {
    fn new(mut terminal: Terminal<B>) -> Result<Self, io::Error> {
        terminal.hide_cursor()?;
        Ok(App { terminal })
    }

    fn draw(&mut self) -> Result<(), io::Error> {
        self.terminal.draw(|f| {
            let stack = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Max(1), Constraint::Min(0)].as_ref())
                .split(f.size());

            // TODO: Set this to something like " - filename.bin"
            let title_info = "";
            let title = Block::default()
                .title(format!("Heks{}", title_info))
                .title_alignment(Alignment::Center);

            f.render_widget(title, stack[0]);

            let buffer = b"\x09\x00\x06\x00hello\x0a\x0b\x0c\x0d\x0e\x0f";

            // let buffer = b"\x09\x00\x06\x00hello";
            // let buffer = b"\x00hello";
            // TODO use _offset and _size
            let window = UnicodeDisplay::default().get_data(|_offset, _size| buffer);

            f.render_widget(window, stack[1]);
        })?;

        Ok(())
    }
}

struct EventLoop<B: Backend> {
    pub app: App<B>,
    pub done: Arc<AtomicBool>,
}

impl<B: Backend> EventLoop<B> {
    pub fn new(app: App<B>) -> Self {
        EventLoop::<B> {
            app,
            done: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn run(&mut self) -> io::Result<()> {
        let mut next_tick = Instant::now();

        while !self.done.load(std::sync::atomic::Ordering::Acquire) {
            sleep_until(next_tick).await;

            self.tick()?;

            let now = Instant::now();
            next_tick += Duration::from_micros(16667);
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
                crossterm::event::Event::Key(key) => {
                    if key.code == KeyCode::Esc || key.code == KeyCode::Char('q') {
                        self.done.store(true, std::sync::atomic::Ordering::Release);
                    }
                }
                crossterm::event::Event::Mouse(_) => {}
                crossterm::event::Event::Paste(_) => {}
                crossterm::event::Event::Resize(_, _) => {}
            }
        }

        self.app.draw()?;

        Ok(())
    }
}

fn install_exit_handler<F: Fn() + Send + 'static>(handler: F) {
    tokio::spawn(async move {
        let mut handlers: Vec<_> = [
            SignalKind::hangup(),
            SignalKind::interrupt(),
            SignalKind::terminate(),
        ]
        .iter()
        .map(|&kind| signal(kind).unwrap())
        .collect();

        let mut signals: FuturesUnordered<_> =
            handlers.iter_mut().map(|handler| handler.recv()).collect();

        signals.next().await;

        handler();
    });
}

struct TerminalSetup {}

impl TerminalSetup {
    fn new() -> Result<TerminalSetup, io::Error> {
        execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
        enable_raw_mode()?;

        Ok(TerminalSetup {})
    }

    fn cleanup(&mut self) {
        disable_raw_mode().unwrap_or(());
        execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)
            .expect("unable to leave alternate screen/disable mouse capture");
    }
}

impl Drop for TerminalSetup {
    fn drop(&mut self) {
        self.cleanup();
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> io::Result<()> {
    let backend = CrosstermBackend::new(io::stdout());
    let terminal = Terminal::new(backend)?;
    let app = App::new(terminal)?;
    let mut event_loop = EventLoop::new(app);

    let _terminal_setup = TerminalSetup::new()?;

    let done_clone = Arc::clone(&event_loop.done);
    install_exit_handler(move || {
        // We might want to hold onto the signal so we can reflect that in our
        // exit code, but this is fine for now.
        done_clone.store(true, std::sync::atomic::Ordering::Release);
    });

    event_loop.run().await?;

    Ok(())
}
