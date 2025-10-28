use std::{fmt::Display, io::Write, path::Path, process::Stdio};

use chrono::Local;
use clap::{Parser, Subcommand, ValueEnum};
use hyprland::{
    data::{Client, Clients, CursorPosition, FullscreenMode, Monitor, Workspace},
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
    ToggleFloat {
        #[arg(short = 'c')]
        center: bool,
    },
    /// Toggles the fullscreen state of a window, keeping its client state
    ToggleFullscreen,
    /// Takes a screenshot
    Screenshot { mode: ScreenshotMode },
    /// Creates a new terminal window in the same directory
    NewTerminal,
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
        Command::ToggleFloat { center } => toggle_float(center),
        Command::ToggleFullscreen => toggle_fullscreen(),
        Command::Screenshot { mode } => screenshot(mode),
        Command::NewTerminal => new_terminal(),
    }
}

fn toggle_float(center: bool) -> HResult<()> {
    let border = 4.0;
    let gaps = (20.0, 10.0, 20.0, 20.0);

    let active_window = match Client::get_active()? {
        Some(active_window) => active_window,
        None => return Ok(()),
    };

    let monitor = Monitor::get_active()?;
    let scale = monitor.scale;
    let width = monitor.width as f32 / scale;
    let height = monitor.height as f32 / scale;

    if active_window.floating {
        Dispatch::call(DispatchType::ToggleFloating(None))?;
    } else if center {
        hyprland::dispatch!(ToggleFloating, None)?;
        hyprland::dispatch!(
            ResizeActive,
            Position::Exact((width / 2.0) as i16, (height / 2.0) as i16,)
        )?;
        hyprland::dispatch!(
            MoveActive,
            Position::Exact((width / 4.0) as i16, (height / 4.0) as i16)
        )?;
    } else {
        let reserved = (
            monitor.reserved.0 as f32,
            monitor.reserved.1 as f32,
            monitor.reserved.2 as f32,
            monitor.reserved.3 as f32,
        );

        let position = CursorPosition::get()?;
        let x = (position.x as f32)
            .min(width - width / 4.0 - gaps.2 - reserved.2 - border)
            .max(width / 4.0 + gaps.0 + reserved.0 + border);
        let y = (position.y as f32)
            .min(height - height / 4.0 - gaps.3 - reserved.3 - border)
            .max(height / 4.0 + gaps.1 + reserved.1 + border);

        hyprland::dispatch!(ToggleFloating, None)?;
        hyprland::dispatch!(
            ResizeActive,
            Position::Exact((width / 2.0) as i16, (height / 2.0) as i16)
        )?;
        hyprland::dispatch!(
            MoveActive,
            Position::Exact((x - width / 4.0) as i16, (y - height / 4.0) as i16)
        )?;
    }

    Ok(())
}

fn toggle_fullscreen() -> HResult<()> {
    let active_window = match Client::get_active()? {
        Some(active_window) => active_window,
        None => return Ok(()),
    };

    hyprland::dispatch!(
        Custom,
        "fullscreenstate",
        &format!(
            "{} -1",
            if active_window.fullscreen == FullscreenMode::None {
                3
            } else {
                0
            }
        )
    )?;

    Ok(())
}

fn grab_region() -> HResult<Option<String>> {
    let child = std::process::Command::new("slurp")
        .arg("-c")
        .arg("A2C9FEFF")
        .arg("-b")
        .arg("00000080")
        .arg("-F")
        .arg("monospace")
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
        .arg("-b")
        .arg("00000080")
        .arg("-B")
        .arg("00000080")
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

fn new_terminal() -> HResult<()> {
    let client = Client::get_active()?;

    let Some(client) = client else {
        return Ok(());
    };

    if client.initial_class == "com.mitchellh.ghostty" {
        let mut title = client.title.rsplit(' ');

        let mut string = String::from(title.next().unwrap_or(""));
        while !(string.starts_with('/') || string.starts_with('~')) {
            if let Some(part) = title.next() {
                string = format!("{part} {string}");
            } else {
                return Ok(());
            }
        }

        let Ok(path) = expanduser::expanduser(string) else {
            return Ok(());
        };

        let error = exec::Command::new("ghostty")
            .arg("--gtk-single-instance=true")
            .arg(format!("--working-directory={}", path.to_string_lossy()))
            .exec();

        println!("{error:?}");
    }

    Ok(())
}
