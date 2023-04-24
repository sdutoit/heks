use std::io;

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

pub struct TerminalSetup {}

impl TerminalSetup {
    pub fn new() -> Result<TerminalSetup, io::Error> {
        TerminalSetup::show().expect("unable to enter alternate screen/enable mouse capture");

        Ok(TerminalSetup {})
    }

    pub fn show() -> Result<(), io::Error> {
        execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
        enable_raw_mode()?;
        Ok(())
    }

    pub fn hide() -> Result<(), io::Error> {
        disable_raw_mode().ok();
        execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;
        Ok(())
    }
}

impl Drop for TerminalSetup {
    fn drop(&mut self) {
        TerminalSetup::hide().expect("unable to leave alternate screen/disable mouse capture");
    }
}
