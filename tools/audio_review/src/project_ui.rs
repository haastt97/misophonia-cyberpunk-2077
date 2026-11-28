use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use anyhow::{Context, Result, ensure};
use eframe::egui;

use crate::review::Review;

#[derive(Default)]
pub struct ProjectUi {
    schedule: AutosaveSchedule,
    settings: Option<Settings>,
}

pub enum ProjectAction {
    New,
    Open,
    OpenAutosave,
    Save,
    SaveAs,
    Settings,
}

pub struct ProjectOutcome {
    pub switched: bool,
    pub message: String,
}

struct Settings {
    minutes: String,
    error: Option<String>,
}

impl ProjectUi {
    pub fn menu(ui: &mut egui::Ui) -> Option<ProjectAction> {
        ui.menu_button("Project", |ui| {
            for (label, action) in [
                ("New project", ProjectAction::New),
                ("Open project…", ProjectAction::Open),
                ("Open autosave…", ProjectAction::OpenAutosave),
                ("Save project", ProjectAction::Save),
                ("Save project as…", ProjectAction::SaveAs),
                ("Settings…", ProjectAction::Settings),
            ] {
                if ui.button(label).clicked() {
                    ui.close();
                    return Some(action);
                }
            }
            None
        })
        .inner
        .flatten()
    }

    pub fn label(ui: &mut egui::Ui, review: &Review) {
        let path = review.project_path();
        let name = path
            .and_then(Path::file_name)
            .map_or_else(|| "Untitled project".into(), |file| file.to_string_lossy());
        let response = ui.add(egui::Label::new(egui::RichText::new(name).weak()).truncate());
        if let Some(path) = path {
            response.on_hover_text(path.display().to_string());
        }
    }

    pub fn perform(
        &mut self,
        action: ProjectAction,
        frame: &eframe::Frame,
        review: &mut Review,
    ) -> Result<Option<ProjectOutcome>> {
        let outcome = match action {
            ProjectAction::New => {
                review
                    .new_project()
                    .context("Could not start a new project")?;
                ProjectOutcome {
                    switched: true,
                    message: "New project ready.".to_owned(),
                }
            }
            ProjectAction::Open | ProjectAction::OpenAutosave => {
                let from_autosaves = matches!(action, ProjectAction::OpenAutosave);
                let directory = if from_autosaves {
                    let path = review.autosave_directory();
                    fs::create_dir_all(&path).context("Could not open the autosave folder")?;
                    Some(path)
                } else {
                    project_directory(review)
                };
                let title = if from_autosaves {
                    "Open autosave"
                } else {
                    "Open project"
                };
                let Some(path) = project_dialog(frame, title, directory).pick_file() else {
                    return Ok(None);
                };
                review
                    .open_project(&path)
                    .context("Could not open the project")?;
                ProjectOutcome {
                    switched: true,
                    message: format!("Opened {}.", project_file_name(&path)),
                }
            }
            ProjectAction::Save | ProjectAction::SaveAs => {
                let save_as =
                    matches!(action, ProjectAction::SaveAs) || review.project_path().is_none();
                let destination = if save_as {
                    let Some(path) =
                        project_dialog(frame, "Save project as", project_directory(review))
                            .set_file_name("Untitled.misophonia.json")
                            .save_file()
                    else {
                        return Ok(None);
                    };
                    Some(path)
                } else {
                    None
                };
                let path = review
                    .save_project(destination.as_deref())
                    .context("Could not save the project")?;
                ProjectOutcome {
                    switched: false,
                    message: format!("Saved {}.", path.display()),
                }
            }
            ProjectAction::Settings => {
                self.settings = Some(Settings {
                    minutes: review.autosave_minutes().to_string(),
                    error: None,
                });
                return Ok(None);
            }
        };
        self.schedule = AutosaveSchedule::default();
        Ok(Some(outcome))
    }

    pub fn autosave(
        &mut self,
        ctx: &egui::Context,
        review: &mut Review,
    ) -> Result<Option<PathBuf>> {
        let (due, remaining) = self
            .schedule
            .tick(Instant::now(), review.autosave_minutes());
        ctx.request_repaint_after(remaining);
        if due {
            review.autosave().context("Could not autosave the project")
        } else {
            Ok(None)
        }
    }

    pub fn show_settings(&mut self, ctx: &egui::Context, review: &mut Review) -> Option<String> {
        let settings = self.settings.as_mut()?;
        let mut open = true;
        let mut applied = None;
        egui::Window::new("Project settings")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .default_width(300.)
            .show(ctx, |ui| {
                let edit = ui
                    .horizontal(|ui| {
                        ui.label("Autosave interval in minutes");
                        ui.add(egui::TextEdit::singleline(&mut settings.minutes).desired_width(68.))
                    })
                    .inner;
                if edit.changed() {
                    settings.error = None;
                }
                let save = ui.button("Save settings").clicked()
                    || (edit.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)));
                if save {
                    match parse_minutes(&settings.minutes)
                        .and_then(|minutes| review.set_autosave_minutes(minutes).map(|()| minutes))
                    {
                        Ok(minutes) => applied = Some(minutes),
                        Err(error) => settings.error = Some(format!("{error:#}")),
                    }
                }
                if let Some(error) = &settings.error {
                    ui.colored_label(egui::Color32::LIGHT_RED, error);
                }
            });
        if !open || applied.is_some() {
            self.settings = None;
        }
        applied.map(|minutes| {
            self.schedule = AutosaveSchedule::default();
            ctx.request_repaint();
            let unit = if minutes == 1 { "minute" } else { "minutes" };
            format!("Autosave set to every {minutes} {unit}.")
        })
    }
}

fn project_dialog(
    frame: &eframe::Frame,
    title: &str,
    directory: Option<PathBuf>,
) -> rfd::FileDialog {
    let dialog = rfd::FileDialog::new()
        .set_parent(frame)
        .set_title(title)
        .add_filter("Misophonia project", &["json"]);
    if let Some(directory) = directory {
        dialog.set_directory(directory)
    } else {
        dialog
    }
}

fn project_directory(review: &Review) -> Option<PathBuf> {
    review
        .project_path()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .or_else(std::env::home_dir)
}

fn project_file_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .into_owned()
}

fn parse_minutes(input: &str) -> Result<u32> {
    let minutes = input
        .trim()
        .parse::<u32>()
        .context("Enter a positive whole number of minutes")?;
    ensure!(minutes > 0, "Enter a positive whole number of minutes");
    Ok(minutes)
}

#[derive(Default)]
struct AutosaveSchedule {
    start: Option<(Instant, u32)>,
}

impl AutosaveSchedule {
    fn tick(&mut self, now: Instant, minutes: u32) -> (bool, Duration) {
        let interval = Duration::from_secs(u64::from(minutes) * 60);
        let remaining = match self.start {
            Some((start, previous)) if minutes == previous => {
                interval.saturating_sub(now.saturating_duration_since(start))
            }
            _ => {
                self.start = Some((now, minutes));
                interval
            }
        };
        if remaining.is_zero() {
            self.start = Some((now, minutes));
            (true, interval)
        } else {
            (false, remaining)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autosave_fires_on_each_deadline_and_reschedules_when_the_setting_changes() {
        let mut schedule = AutosaveSchedule::default();
        let t = Instant::now();
        let seconds = Duration::from_secs;
        assert_eq!(schedule.tick(t, 1), (false, seconds(60)));
        assert_eq!(schedule.tick(t + seconds(59), 1), (false, seconds(1)));
        assert_eq!(schedule.tick(t + seconds(60), 1), (true, seconds(60)));
        assert_eq!(schedule.tick(t + seconds(61), 3), (false, seconds(180)));
        assert_eq!(schedule.tick(t + seconds(120), 3), (false, seconds(121)));
        assert_eq!(schedule.tick(t + seconds(241), 3), (true, seconds(180)));
    }

    #[test]
    fn interval_requires_a_positive_whole_number() {
        assert_eq!(parse_minutes(" 3 ").unwrap(), 3);
        for invalid in ["", "0", "-1", "1.5", "minutes", "4294967296"] {
            assert!(parse_minutes(invalid).is_err(), "{invalid}");
        }
    }
}
