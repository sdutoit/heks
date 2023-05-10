use std::io;

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use once_cell::sync::OnceCell;
use pastel::ansi::AnsiColor;
use std::env;

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

#[derive(Clone, Copy)]
pub enum ColorDepth {
    Palette8, // 8-bit palette (i.e. 256 colors)
    Rgb888,   // 24-bit (8/8/8) RGB (aka "truecolor")
}

fn query_depth() -> ColorDepth {
    // Ideally we'd fall back to something like
    //
    //   https://gist.github.com/kurahaupo/6ce0eaefe5e730841f03cb82b061daa2#querying-the-terminal
    //
    // where we query the terminal after attempting to set an RGB color. But
    // either way we should respect COLORTERM first.
    match env::var("COLORTERM").unwrap_or(String::new()).as_str() {
        "truecolor" => ColorDepth::Rgb888,
        _ => ColorDepth::Palette8,
    }
}

pub fn get_depth() -> ColorDepth {
    static DEPTH: OnceCell<ColorDepth> = OnceCell::new();
    *DEPTH.get_or_init(query_depth)
}

pub fn color(r: u8, g: u8, b: u8) -> tui::style::Color {
    match get_depth() {
        ColorDepth::Palette8 => {
            let ansi = pastel::Color::from_rgb(r, g, b).to_ansi_8bit();
            tui::style::Color::Indexed(ansi)
        }
        ColorDepth::Rgb888 => tui::style::Color::Rgb(r, g, b),
    }
}

pub fn color_hsl(hue: f64, saturation: f64, lightness: f64) -> tui::style::Color {
    let rgba = pastel::Color::from_hsl(hue, saturation, lightness).to_rgba();
    color(rgba.r, rgba.g, rgba.b)
}
