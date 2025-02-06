use clap::Parser;
use hyprland::{
    data::{Client, CursorPosition, Monitor},
    dispatch::{Dispatch, DispatchType, Position},
    shared::{HyprData, HyprDataActive, HyprDataActiveOptional, HyprError},
};

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
enum Command {
    /// Toggles the floating state of a window, resizing it if necessary
    ToggleFloat,
}

fn main() -> Result<(), HyprError> {
    let command = Command::parse();
    match command {
        Command::ToggleFloat => toggle_float(),
    }
}

fn toggle_float() -> Result<(), HyprError> {
    let position = CursorPosition::get()?;
    let x: i16 = position.x.try_into().unwrap_or_default();
    let y: i16 = position.y.try_into().unwrap_or_default();

    let active_window = match Client::get_active()? {
        Some(active_window) => active_window,
        None => return Ok(()),
    };
    let floating = active_window.floating;

    let monitor = Monitor::get_active()?;
    let width: i16 = monitor.width.try_into().unwrap_or_default();
    let height: i16 = monitor.height.try_into().unwrap_or_default();

    if floating {
        hyprland::dispatch!(ToggleFloating, None)?;
    } else {
        hyprland::dispatch!(ToggleFloating, None)?;
        hyprland::dispatch!(ResizeActive, Position::Exact(width / 2, height / 2))?;
        hyprland::dispatch!(MoveActive, Position::Exact(x - width / 4, y - height / 4))?;
    }

    Ok(())
}
