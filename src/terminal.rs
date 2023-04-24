use std::io;

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

pub struct TerminalSetup {}

impl TerminalSetup {
    pub fn new() -> Result<TerminalSetup, io::Error> {
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
