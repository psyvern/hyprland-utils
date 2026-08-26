mod jetbrains;

use crate::jetbrains::JetBrainsProduct;
use std::{
    fmt::Display,
    io::Write,
    path::{Path, PathBuf},
    process::Stdio,
    str::FromStr,
};

use chrono::Local;
use clap::{Parser, Subcommand, ValueEnum};
use hyprland::{
    Result as HResult,
    data::{Client, Clients, CursorPosition, FullscreenMode, Monitor, Workspace},
    dispatch::{Dispatch, DispatchType},
    shared::{HyprData, HyprDataActive, HyprDataActiveOptional},
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
    /// Creates a new explorer window in the same directory
    NewExplorer,
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
        Command::NewExplorer => new_explorer(),
    }
}

fn dispatch<S: AsRef<str>>(arg: S) -> HResult<()> {
    Dispatch::call(DispatchType::Custom(arg.as_ref(), ""))
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
        dispatch("hl.dsp.window.float()")?;
    } else if center {
        dispatch("hl.dsp.window.float()")?;
        dispatch(format!(
            "hl.dsp.window.resize({{ x = {}, y = {} }})",
            width / 2.0,
            height / 2.0
        ))?;
        dispatch(format!(
            "hl.dsp.window.move({{ x = {}, y = {} }})",
            width / 4.0,
            height / 4.0
        ))?;
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

        dispatch("hl.dsp.window.float()")?;
        dispatch(format!(
            "hl.dsp.window.resize({{ x = {}, y = {} }})",
            width / 2.0,
            height / 2.0
        ))?;
        dispatch(format!(
            "hl.dsp.window.move({{ x = {}, y = {} }})",
            x - width / 4.0,
            y - height / 4.0
        ))?;
    }

    Ok(())
}

fn toggle_fullscreen() -> HResult<()> {
    let active_window = match Client::get_active()? {
        Some(active_window) => active_window,
        None => return Ok(()),
    };

    dispatch(format!(
        "hl.dsp.window.fullscreen_state({{ internal = {}, client = -1 }})",
        if active_window.fullscreen == FullscreenMode::None {
            3
        } else {
            0
        }
    ))?;

    Ok(())
}

struct Geometry {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

#[derive(Debug)]
enum ParseGeometryError {
    WrongArgumentsCount,
    ParseArgument(usize),
}

impl FromStr for Geometry {
    type Err = ParseGeometryError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let Some([x, y, w, h]) = s.split_whitespace().collect_array() else {
            return Err(ParseGeometryError::WrongArgumentsCount);
        };

        let x = x
            .parse()
            .map_err(|_| ParseGeometryError::ParseArgument(0))?;
        let y = y
            .parse()
            .map_err(|_| ParseGeometryError::ParseArgument(1))?;
        let w = w
            .parse()
            .map_err(|_| ParseGeometryError::ParseArgument(2))?;
        let h = h
            .parse()
            .map_err(|_| ParseGeometryError::ParseArgument(3))?;

        Ok(Self {
            x,
            y,
            width: w,
            height: h,
        })
    }
}

impl Display for Geometry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{},{} {}x{}", self.x, self.y, self.width, self.height)
    }
}

fn grab_region() -> HResult<Option<Geometry>> {
    let child = std::process::Command::new("slurp")
        .arg("-c")
        .arg("A2C9FEFF")
        .arg("-b")
        .arg("00000080")
        .arg("-F")
        .arg("monospace")
        .arg("-f")
        .arg("%x %y %w %h")
        .arg("-d")
        .output()
        .unwrap();

    let output = child.stdout;

    if output.is_empty() {
        Ok(None)
    } else {
        let output = String::from_utf8(output).unwrap();
        let region = output.parse::<Geometry>().unwrap();

        if region.width > 2 && region.height > 2 {
            // Remove one pixel from every side
            let region = Geometry {
                x: region.x + 1,
                y: region.y + 1,
                width: region.width - 2,
                height: region.height - 2,
            };
            Ok(Some(region))
        } else {
            Ok(None)
        }
    }
}

fn grab_display() -> HResult<Option<Geometry>> {
    let monitor = Monitor::get_active()?;
    let data = Geometry {
        x: monitor.x,
        y: monitor.y,
        width: (f32::from(monitor.width) / monitor.scale).round() as u32,
        height: (f32::from(monitor.height) / monitor.scale).round() as u32,
    };

    Ok(Some(data))
}

fn grab_window() -> HResult<Option<Geometry>> {
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
        .arg("%x %y %w %h")
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
        let output = String::from_utf8(output).unwrap();
        Ok(Some(output.parse().unwrap()))
    }
}

fn save_geometry(path: &Path, geometry: Geometry) {
    std::process::Command::new("grim")
        .arg("-g")
        .arg(geometry.to_string())
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
            std::process::Command::new("hyprctl")
                .arg("-r")
                .arg("eval")
                .arg(
                    r#"hl.config({
                    general = {
                        col = {
                            active_border = "rgb(FFFFFF)",
                            inactive_border = "rgb(FFFFFF)",
                        }
                    },
                    decoration = {
                        rounding = 0,
                        dim_inactive = false,
                        inactive_opacity = 1,
                    }
                })
            "#,
                )
                .spawn()?
                .wait()?;
            dispatch("hl.dsp.submap(\"empty\")")?;

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
        dispatch("hl.dsp.submap(\"reset\")")?;
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

fn find_window_location(client: Client) -> Option<PathBuf> {
    if client.initial_class == "com.mitchellh.ghostty" {
        let mut title = client.title.rsplit(' ');

        let mut string = String::from(title.next().unwrap_or(""));
        while !(string.starts_with('/') || string.starts_with('~')) {
            if let Some(part) = title.next() {
                string = format!("{part} {string}");
            } else {
                return None;
            }
        }

        expanduser::expanduser(string).ok()
    } else if client.initial_class == "blender" {
        let path = client
            .title
            .split_once('[')?
            .1
            .rsplit_once("] - Blender")?
            .0;

        let path = expanduser::expanduser(path).ok()?.parent()?.to_owned();
        Some(path)
    } else if let Some(class) = client.initial_class.strip_prefix("jetbrains-")
        && client.class == client.initial_class
        && client.initial_title.is_empty()
    {
        JetBrainsProduct::parse(class).and_then(|x| x.find_location(&client.title))
    } else {
        None
    }
}

fn new_terminal() -> HResult<()> {
    let client = Client::get_active()?;

    let Some(client) = client else {
        return Ok(());
    };

    if let Some(path) = find_window_location(client) {
        let error = exec::Command::new("ghostty")
            .arg("+new-window")
            .arg(format!("--working-directory={}", path.to_string_lossy()))
            .exec();

        println!("{error:?}");
    }

    Ok(())
}

fn new_explorer() -> HResult<()> {
    let client = Client::get_active()?;

    let Some(client) = client else {
        return Ok(());
    };

    if let Some(path) = find_window_location(client) {
        let error = exec::Command::new("nautilus")
            .arg("--new-window")
            .arg(path)
            .exec();

        println!("{error:?}");
    }

    Ok(())
}
