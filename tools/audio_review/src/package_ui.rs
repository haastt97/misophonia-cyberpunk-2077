use std::{path::PathBuf, sync::Arc};

use eframe::egui;

use crate::{
    Library,
    build_plan::BuildPlan,
    job::Job,
    package_build::{self, BuiltPackage},
};

#[derive(Default)]
pub struct PackageDialog {
    open: bool,
    name: String,
    destination: Option<PathBuf>,
    running: Option<Job<BuiltPackage>>,
    result: Option<Result<BuiltPackage, String>>,
}

impl PackageDialog {
    pub fn open(&mut self, base: &str) {
        if self.name.is_empty() {
            self.name = format!("{base}-custom");
        }
        self.open = true;
    }

    pub fn show(&mut self, ctx: &egui::Context, frame: &eframe::Frame, library: &Library) {
        if let Some(result) = self.running.as_ref().and_then(Job::poll) {
            self.running = None;
            self.result = Some(result.map_err(|error| format!("{error:#}")));
            self.open = true;
        }
        if !self.open {
            return;
        }
        let Self {
            open,
            name,
            destination,
            running,
            result,
        } = self;
        egui::Window::new("Build custom mod")
            .open(open)
            .default_width(530.)
            .show(ctx, |ui| {
                ui.add_enabled_ui(running.is_none(), |ui| {
                    ui.label("Version name");
                    ui.text_edit_singleline(name);
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("Choose save folder…").clicked() {
                            let mut chooser = rfd::FileDialog::new()
                                .set_parent(frame).set_title("Save custom Misophonia version");
                            if let Some(current) = destination.clone().or_else(std::env::home_dir) {
                                chooser = chooser.set_directory(current);
                            }
                            if let Some(folder) = chooser.pick_folder() {
                                *destination = Some(folder);
                                *result = None;
                            }
                        }
                        if let Some(destination) = destination.as_ref() {
                            ui.label(destination.display().to_string());
                        }
                    });
                    ui.label("The packages apply edits to named game events. An event can play more than one clip.");
                    if ui.add_enabled(destination.is_some(), egui::Button::new("Build packages")).clicked() {
                        let preparation = package_build::version_name(name).and_then(|version| {
                            let plan = BuildPlan::from_review(&library.review)?;
                            Ok((plan, version.to_owned()))
                        });
                        match preparation {
                            Ok((plan, version)) => {
                                *result = None;
                                let source = library.package_source.clone();
                                let game = Arc::clone(&library.game);
                                let folder = destination.clone().expect("build enabled with a destination");
                                *running = Some(Job::start(ctx, move |cancelled| {
                                    package_build::build(&source, &game, &plan, &version, &folder, cancelled)
                                }));
                            }
                            Err(error) => *result = Some(Err(format!("{error:#}"))),
                        }
                    }
                });
                if let Some(job) = running {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Building custom packages…");
                        if ui.button("Cancel build").clicked() {
                            job.cancel();
                        }
                    });
                }
                if let Some(result) = result {
                    ui.separator();
                    match result {
                        Ok(package) => {
                            ui.strong("Packages saved");
                            ui.add(egui::Label::new(package.path.display().to_string()).selectable(true));
                            egui::ScrollArea::vertical().max_height(200.).show(ui, |ui| {
                                for notice in &package.notices {
                                    ui.label(notice);
                                }
                            });
                        }
                        Err(error) => { ui.colored_label(egui::Color32::LIGHT_RED, error.as_str()); }
                    }
                }
            });
    }
}
