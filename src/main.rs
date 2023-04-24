use clap::Parser;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use heks::terminal::TerminalSetup;
use heks::App;
use heks::EventLoop;
use heks::FileSource;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::signal::unix::signal;
use tokio::signal::unix::SignalKind;
use tui::backend::CrosstermBackend;
use tui::Terminal;

#[derive(Parser, Debug)]
struct Args {
    filename: PathBuf,
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

#[tokio::main(flavor = "current_thread")]
async fn main() -> io::Result<()> {
    let args = Args::parse();

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    // let source = Box::new(DebugSource::new());
    let source = Box::new(
        FileSource::new(&args.filename)
            .expect(format!("Unable to open '{}'", args.filename.to_str().unwrap()).as_str()),
    );
    let app = App::new(&mut terminal, source)?;
    let mut event_loop = EventLoop::new(terminal, app);

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
