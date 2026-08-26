use std::path::{Path, PathBuf};

use itertools::Itertools;

#[derive(Clone, Copy)]
pub enum JetBrainsProduct {
    IntelliJIdea,
    CLion,
    AppCode,
    PyCharm,
    RubyMine,
    DataGrip,
    AndroidStudio,
    WebStorm,
    PhpStorm,
    GoLand,
    Rider,
    RustRover,
}

impl JetBrainsProduct {
    fn code(self) -> &'static str {
        match self {
            Self::IntelliJIdea => "idea",
            Self::CLion => "clion",
            Self::AppCode => "appcode",
            Self::PyCharm => "pycharm",
            Self::RubyMine => "rubymine",
            Self::DataGrip => "datagrip",
            Self::AndroidStudio => "studio",
            Self::WebStorm => "webide",
            Self::PhpStorm => "phpstorm",
            Self::GoLand => "goland",
            Self::Rider => "rider",
            Self::RustRover => "rustrover",
        }
    }

    fn vendor(self) -> &'static str {
        match self {
            Self::AndroidStudio => "Google",
            _ => "JetBrains",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "idea" => Some(Self::IntelliJIdea),
            "studio" => Some(Self::AndroidStudio),
            _ => None,
        }
    }

    pub fn find_location(self, title: &str) -> Option<PathBuf> {
        if let Some((name, _)) = title.split_once(" – ") {
            let home = PathBuf::from(std::env::var("HOME").ok()?);

            let config = std::env::var(format!("{}_PROPERTIES", self.code().to_uppercase()))
                .ok()
                .and_then(|x| PathBuf::from(x).parent().map(Path::to_owned))
                .or_else(|| {
                    let path = home.join(".config").join(self.vendor());

                    std::fs::read_dir(path)
                        .ok()?
                        .flatten()
                        .flat_map(|x| Some((x.metadata().ok()?.modified().ok()?, x)))
                        .max_by_key(|(x, _)| *x)
                        .map(|x| x.1.path())
                })?;

            let text =
                std::fs::read_to_string(config.join("options").join("recentProjects.xml")).ok()?;
            let document = roxmltree::Document::parse(&text).ok()?;

            let projects = document
                .root_element()
                .first_element_child()?
                .first_element_child()?
                .first_element_child()?
                .children()
                .flat_map(|child| {
                    let path = child.attribute("key")?;
                    let project_info = child.first_element_child()?.first_element_child()?;

                    let frame_title = project_info.attribute("frameTitle")?;
                    let workspace_id = project_info.attribute("projectWorkspaceId")?;

                    Some((path, (frame_title, workspace_id)))
                })
                .collect_vec();

            for (path, (frame_title, _)) in &projects {
                let project_name = frame_title.split_once(" – ").map(|(x, _)| x).or_else(|| {
                    frame_title
                        .strip_suffix(']')
                        .and_then(|x| x.rsplit_once('['))
                        .and_then(|(_, x)| x.split_once('.'))
                        .map(|(x, _)| x)
                });

                if project_name == Some(name) {
                    return Some(PathBuf::from(
                        path.replace("$USER_HOME$", &home.to_string_lossy()),
                    ));
                }
            }

            for (path, (_, workspace_id)) in &projects {
                let workspace = read_name_from_workspace(
                    &config.join("workspace").join(format!("{workspace_id}.xml")),
                );

                if workspace == Some(name.to_owned()) {
                    return Some(PathBuf::from(
                        path.replace("$USER_HOME$", &home.to_string_lossy()),
                    ));
                }
            }
        }

        None
    }
}

fn read_name_from_workspace(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let document = roxmltree::Document::parse(&text).ok()?;

    let project_view = document
        .root_element()
        .children()
        .find(|x| x.attribute("name") == Some("ProjectView"))?;

    let project_pane = project_view
        .children()
        .find(|x| x.tag_name().name() == "panes")?
        .children()
        .find(|x| x.attribute("id") == Some("ProjectPane"))?;

    let presentation = project_pane
        .first_element_child()?
        .children()
        .find(|x| x.tag_name().name() == "presentation")?;

    presentation
        .children()
        .find(|x| x.tag_name().name() == "item")?
        .attribute("name")
        .map(str::to_owned)
}
