use std::fmt::Display;

use chrono::Local;
use clap::{Parser, Subcommand, ValueEnum};
use hyprland::{
    data::{Client, CursorPosition, Monitor},
    dispatch::{Dispatch, DispatchType, Position},
    shared::{HyprData, HyprDataActive, HyprDataActiveOptional},
    Result as HResult,
};

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
enum Command {
    /// Toggles the floating state of a window, resizing it if necessary
    ToggleFloat,
    /// Takes a screenshot
    Screenshot { mode: ScreenshotMode },
}

#[derive(Debug, Clone, Subcommand, ValueEnum)]
enum ScreenshotMode {
    Point,
    Region,
    Window,
    Display,
}

impl Display for ScreenshotMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ScreenshotMode::Point => "point",
                ScreenshotMode::Region => "region",
                ScreenshotMode::Window => "window",
                ScreenshotMode::Display => "display",
            }
        )
    }
}

fn main() -> HResult<()> {
    let command = Command::parse();
    match command {
        Command::ToggleFloat => toggle_float(),
        Command::Screenshot { mode } => screenshot(mode),
    }
}

fn toggle_float() -> HResult<()> {
    let position = CursorPosition::get()?;
    let x: i16 = position.x.try_into().unwrap_or_default();
    let y: i16 = position.y.try_into().unwrap_or_default();

    let active_window = match Client::get_active()? {
        Some(active_window) => active_window,
        None => return Ok(()),
    };

    let monitor = Monitor::get_active()?;
    let width: i16 = monitor.width.try_into().unwrap_or_default();
    let height: i16 = monitor.height.try_into().unwrap_or_default();

    if active_window.floating {
        Dispatch::call(DispatchType::ToggleFloating(None))?;
    } else {
        hyprland::dispatch!(ToggleFloating, None)?;
        hyprland::dispatch!(ResizeActive, Position::Exact(width / 2, height / 2))?;
        hyprland::dispatch!(MoveActive, Position::Exact(x - width / 4, y - height / 4))?;
    }

    Ok(())
}

fn screenshot(mode: ScreenshotMode) -> HResult<()> {
    let file = Local::now().format("%Y-%m-%d_%H-%M-%S.png").to_string();
    let directory = homedir::my_home()
        .unwrap()
        .unwrap()
        .join("Pictures")
        .join("screenshots");
    let path = directory.clone().join(&file);
    dbg!(&path);

    std::fs::create_dir_all(&directory).unwrap();

    let output = std::process::Command::new("hyprshot")
        .arg("-m")
        .arg(mode.to_string())
        .arg("-o")
        .arg(&directory)
        .arg("-f")
        .arg(&file)
        .arg("-s")
        .output()
        .unwrap();

    if output.stderr.is_empty() {
        use notify_rust::Notification;
        Notification::new()
            .image_path(&path.to_string_lossy())
            .summary("Screenshot saved")
            .body(&file)
            .action("show", "Show in Files")
            .action("open", "View")
            // .action("edit", "Edit")
            .show()
            .unwrap()
            .wait_for_action(|action| match action {
                "show" => open::that_detached(directory).unwrap(),
                "open" => open::that_detached(path).unwrap(),
                "edit" => open::that_detached(path).unwrap(),
                _ => (),
            });
    }

    Ok(())
}
