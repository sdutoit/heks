use std::io;

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use pastel::ansi::AnsiColor;

pub fn color(r: u8, g: u8, b: u8) -> tui::style::Color {
    // TODO: Just use Color::RGB if the terminal supports it.
    tui::style::Color::Indexed(pastel::Color::from_rgb(r, g, b).to_ansi_8bit())
}

pub fn color_hsl(hue: f64, saturation: f64, lightness: f64) -> tui::style::Color {
    // TODO: Just use Color::RGB if the terminal supports it.
    tui::style::Color::Indexed(pastel::Color::from_hsl(hue, saturation, lightness).to_ansi_8bit())
}

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
