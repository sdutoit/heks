use clap::Parser;
use futures::{stream::FuturesUnordered, StreamExt};
use heks::{source::FileSource, terminal::TerminalSetup, App, EventLoop};
use home::home_dir;
use log::{error, info};
use nix::unistd::getcwd;
use std::{env, fs::OpenOptions, io, path::PathBuf, process::ExitCode, sync::Arc};
use tokio::signal::unix::{signal, SignalKind};
use tui::{backend::CrosstermBackend, Terminal};

#[derive(Parser, Debug)]
struct Args {
    filename: PathBuf,
}

fn install_exit_handler<F: FnMut() + Send + 'static>(mut handler: F) {
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

fn install_suspend_handler<F: FnMut() + Send + 'static>(mut handler: F) {
    tokio::spawn(async move {
        loop {
            signal(SignalKind::from_raw(nix::sys::signal::SIGTSTP as i32))
                .unwrap()
                .recv()
                .await;
            handler();
        }
    });
}
fn init_log_file(logger: &mut env_logger::Builder, path: PathBuf) {
    let log_file = OpenOptions::new()
        .write(true)
        .append(true)
        .create(true)
        .open(path)
        .unwrap();
    let target = Box::new(log_file);
    logger.target(env_logger::Target::Pipe(target));
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let mut logger = env_logger::builder();
    logger.filter_level(log::LevelFilter::Info);
    logger.parse_default_env();
    logger.parse_env("HEKS_LOG");

    // Create a log file at ~/.heks.log (as long as we can figure out the user's
    // home directory).
    if let Some(path) = home_dir() {
        init_log_file(&mut logger, path.join(".heks.log"))
    }

    // After these calls, logs go to the log file, and panics go to the log.
    logger.init();
    log_panics::init();

    info!("############################## 🧹 𝓱𝓮𝓴𝓼 🧹 ##############################");
    info!("args: {}", shell_words::join(env::args()));
    info!(" cwd: {}", getcwd().unwrap().display());
    info!("");

    let args = Args::parse();

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend).unwrap();

    let source = FileSource::new(&args.filename)
        .map(Box::new)
        .unwrap_or_else(|error| {
            eprintln!("Unable to open {:?}: {}", &args.filename, error);
            error!("Unable to open {:?}", &args.filename);
            panic!("{:?}", error);
        });

    let _terminal_setup = TerminalSetup::new().unwrap();
    let app = App::new(&mut terminal, source).unwrap();
    let mut event_loop = EventLoop::new(terminal, app);

    let done_clone = Arc::clone(&event_loop.done);
    install_exit_handler(move || {
        // We might want to hold onto the signal so we can reflect that in our
        // exit code, but this is fine for now.
        done_clone.store(true, std::sync::atomic::Ordering::Release);
    });

    let terminal_clone = Arc::clone(&event_loop.terminal);
    let dirty_clone = Arc::clone(&event_loop.dirty);
    install_suspend_handler(move || {
        TerminalSetup::hide().ok();
        {
            let mut terminal = terminal_clone.lock().unwrap();
            terminal.show_cursor().ok();
        }
        nix::sys::signal::kill(nix::unistd::getpid(), nix::sys::signal::SIGSTOP).ok();
        TerminalSetup::show().ok();

        // Ensure that the terminal gets redrawn next frame.
        let mut terminal = terminal_clone.lock().unwrap();
        terminal.hide_cursor().ok();
        terminal.clear().ok();

        dirty_clone.store(true, std::sync::atomic::Ordering::Release);
    });

    event_loop.run().await.unwrap();

    ExitCode::SUCCESS
}
