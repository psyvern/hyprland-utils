use std::{fmt::Display, io::Write, path::Path, process::Stdio};

use chrono::Local;
use clap::{Parser, Subcommand, ValueEnum};
use hyprland::{
    data::{Client, Clients, CursorPosition, Monitor, Workspace},
    dispatch::{Dispatch, DispatchType, Position},
    shared::{HyprData, HyprDataActive, HyprDataActiveOptional},
    Result as HResult,
};
use itertools::Itertools;

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
enum Command {
    /// Toggles the floating state of a window, resizing it if necessary
    ToggleFloat,
    /// Takes a screenshot
    Screenshot { mode: ScreenshotMode },
}

#[derive(PartialEq, Eq, Debug, Clone, Subcommand, ValueEnum)]
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
                ScreenshotMode::Display => "active",
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

fn grab_window(path: &Path) -> HResult<bool> {
    let workspace = Workspace::get_active()?;
    let clients = Clients::get()?
        .into_iter()
        .filter(|x| x.workspace.id == workspace.id)
        .map(|x| format!("{},{} {}x{}", x.at.0, x.at.1, x.size.0, x.size.1))
        .join("\n");
    let mut child = std::process::Command::new("slurp")
        .arg("-w")
        .arg("0")
        .arg("-f")
        .arg("%x,%y %wx%h")
        .arg("-r")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let mut stdin = child.stdin.take().expect("Failed to open stdin");
    std::thread::spawn(move || {
        stdin
            .write_all(clients.as_bytes())
            .expect("Failed to write to stdin");
    });
    let output = child
        .wait_with_output()
        .expect("Failed to read stdout")
        .stdout;

    if output.is_empty() {
        Ok(false)
    } else {
        save_geometry(path, output).map(|_| true)
    }
}

fn save_geometry(path: &Path, geometry: Vec<u8>) -> HResult<()> {
    std::process::Command::new("grim")
        .arg("-g")
        .arg(String::from_utf8(geometry).unwrap())
        .arg(path)
        .spawn()
        .unwrap()
        .wait()
        .unwrap();
    // wl-copy --type image/png < "$output"

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

    std::fs::create_dir_all(&directory).unwrap();

    let result = match mode {
        ScreenshotMode::Window => {
            hyprland::keyword::Keyword::set("general:col.inactive_border", 0xFFFFFFFFu32)?;
            hyprland::keyword::Keyword::set("general:col.active_border", 0xFFFFFFFFu32)?;
            hyprland::keyword::Keyword::set("decoration:rounding", 0)?;
            hyprland::keyword::Keyword::set("decoration:dim_inactive", 0)?;
            hyprland::dispatch!(Custom, "submap", "empty")?;
            let result = grab_window(&path)?;
            hyprland::ctl::reload::call()?;
            hyprland::dispatch!(Custom, "submap", "reset")?;

            result
        }
        _ => {
            let mut output = std::process::Command::new("hyprshot");
            let output = output.arg("-m");
            let output = match mode {
                ScreenshotMode::Point => output.arg("point"),
                ScreenshotMode::Region => output.arg("region"),
                ScreenshotMode::Window => unreachable!("Window already handled"),
                ScreenshotMode::Display => output.arg("active").arg("-m").arg("output"),
            };
            let output = output
                .arg("-o")
                .arg(&directory)
                .arg("-f")
                .arg(&file)
                .arg("-s")
                .output()
                .unwrap();

            output.stderr.is_empty()
        }
    };

    if result {
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
