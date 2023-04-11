use crossterm::{
    event::{poll, read, DisableMouseCapture, EnableMouseCapture, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::{stream::FuturesUnordered, StreamExt};
use std::{io, process::exit, time::Duration};
use tokio::signal::unix::{signal, SignalKind};
use tokio::time::{sleep_until, Instant};
use tui::{
    backend::{Backend, CrosstermBackend},
    widgets::{Block, Borders},
    Terminal,
};

struct EventLoop<B: Backend> {
    terminal: Terminal<B>,
    done: bool,
}

impl<B: Backend> EventLoop<B> {
    pub fn new(terminal: Terminal<B>) -> EventLoop<B> {
        EventLoop::<B> {
            terminal,
            done: false,
        }
    }

    pub async fn run(&mut self) -> io::Result<()> {
        let mut next_tick = Instant::now();

        while !self.done {
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
                        self.done = true;
                    }
                }
                crossterm::event::Event::Mouse(_) => {}
                crossterm::event::Event::Paste(_) => {}
                crossterm::event::Event::Resize(_, _) => {}
            }
        }

        self.draw()?;

        Ok(())
    }

    fn draw(&mut self) -> Result<(), io::Error> {
        self.terminal.draw(|f| {
            let size = f.size();
            let block = Block::default().title("Heks").borders(Borders::ALL);
            f.render_widget(block, size);
        })?;

        self.terminal.hide_cursor()?;

        Ok(())
    }
}

fn setup_signal_handler() {
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

        let _ = disable_raw_mode();
        exit(1);
    });
}

struct TerminalSetup {}

impl TerminalSetup {
    fn new() -> Result<TerminalSetup, io::Error> {
        execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
        enable_raw_mode()?;

        Ok(TerminalSetup {})
    }
}

impl Drop for TerminalSetup {
    fn drop(&mut self) {
        disable_raw_mode().unwrap_or(());
        execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)
            .expect("unable to leave alternate screen/disable mouse capture");
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> io::Result<()> {
    let mut _terminal_setup = TerminalSetup::new()?;

    setup_signal_handler();

    let backend = CrosstermBackend::new(io::stdout());
    let terminal = Terminal::new(backend)?;
    let mut event_loop = EventLoop::new(terminal);
    event_loop.run().await?;

    Ok(())
}
