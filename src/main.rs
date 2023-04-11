use crossterm::{
    event::{poll, read, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use std::{io, time::Duration};
use tokio::time::{sleep_until, Instant};

async fn event_loop() -> io::Result<()> {
    let mut next_tick = Instant::now();
    let mut done = false;

    while !done {
        sleep_until(next_tick).await;

        while poll(Duration::from_secs(0))? {
            let event = read()?;
            match event {
                crossterm::event::Event::FocusGained => {}
                crossterm::event::Event::FocusLost => {}
                crossterm::event::Event::Key(key) => {
                    if key.code == KeyCode::Esc {
                        done = true;
                    }
                }
                crossterm::event::Event::Mouse(_) => {}
                crossterm::event::Event::Paste(_) => {}
                crossterm::event::Event::Resize(_, _) => {}
            }
        }

        let now = Instant::now();
        next_tick += Duration::from_micros(16667);
        if next_tick < now {
            next_tick = now;
        }
    }

    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> io::Result<()> {
    enable_raw_mode()?;
    event_loop().await?;
    disable_raw_mode()?;

    Ok(())
}
