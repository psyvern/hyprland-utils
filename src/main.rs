use std::{fmt::Display, io::Write, path::Path, process::Stdio};

use chrono::Local;
use clap::{Parser, Subcommand, ValueEnum};
use hyprland::{
    data::{Client, Clients, CursorPosition, Monitor, Workspace},
    dispatch::{Dispatch, DispatchType, Position},
    keyword::Keyword,
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

fn grab_region() -> HResult<Option<String>> {
    let child = std::process::Command::new("slurp")
        .arg("-c")
        .arg("FFFFFFFF")
        .arg("-F")
        .arg("Fira Code")
        .arg("-f")
        .arg("%x,%y %wx%h")
        .arg("-d")
        .output()
        .unwrap();

    let output = child.stdout;

    if output.is_empty() {
        Ok(None)
    } else {
        Ok(Some(String::from_utf8(output).unwrap()))
    }
}

fn grab_display() -> HResult<Option<String>> {
    let monitor = Monitor::get_active()?;
    let data = format!(
        "{},{} {}x{}",
        monitor.x,
        monitor.y,
        (f32::from(monitor.width) / monitor.scale).round(),
        (f32::from(monitor.height) / monitor.scale).round()
    );

    Ok(Some(data))
}

fn grab_window() -> HResult<Option<String>> {
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
        Ok(None)
    } else {
        Ok(Some(String::from_utf8(output).unwrap()))
    }
}

fn save_geometry(path: &Path, geometry: String) {
    std::process::Command::new("grim")
        .arg("-g")
        .arg(geometry)
        .arg(path)
        .spawn()
        .unwrap()
        .wait()
        .unwrap();

    std::process::Command::new("sh")
        .arg("-c")
        .arg(format!(
            "wl-copy --type image/png < {}",
            path.to_string_lossy()
        ))
        .spawn()
        .unwrap()
        .wait()
        .unwrap();
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
            Keyword::set("general:col.inactive_border", 0xFFFFFFFFu32)?;
            Keyword::set("general:col.active_border", 0xFFFFFFFFu32)?;
            Keyword::set("decoration:rounding", 0)?;
            Keyword::set("decoration:dim_inactive", 0)?;
            hyprland::dispatch!(Custom, "submap", "empty")?;

            grab_window()?
        }
        ScreenshotMode::Region => grab_region()?,
        ScreenshotMode::Display => grab_display()?,
    };
    let has_result = result.is_some();

    if let Some(result) = result {
        save_geometry(&path, result);
    }

    if mode == ScreenshotMode::Window {
        hyprland::ctl::reload::call()?;
        hyprland::dispatch!(Custom, "submap", "reset")?;
    }

    if has_result {
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
