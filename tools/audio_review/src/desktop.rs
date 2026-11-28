mod selection;

use std::time::Duration;

use anyhow::Result;
use eframe::egui::{self, Color32, RichText};
use egui_extras::{Column, TableBuilder};
use selection::Selection;

use crate::{
    Args, Library,
    catalog::{Category, Sound},
    job::Job,
    load,
    player::{Action, Player},
    review::{Review, Tags, View, kind_label},
};

const TRIGGER: Color32 = Color32::from_rgb(62, 40, 45);
const STAND_IN: Color32 = Color32::from_rgb(35, 57, 46);
const RED: Color32 = Color32::from_rgb(243, 145, 153);
const GREEN: Color32 = Color32::from_rgb(136, 207, 160);
const TAG_GRAY: Color32 = Color32::from_gray(153);

pub struct Desktop {
    loading: Option<Job<Library>>,
    library: Option<Library>,
    player: Player,
    filters: Filters,
    visible: Vec<usize>,
    selection: Selection,
    warning: String,
    status: Option<(String, bool)>,
}

#[derive(Default, PartialEq, Eq, Clone)]
struct Filters {
    query: String,
    view: View,
    category: Option<String>,
    kind: Option<String>,
}

enum RowAction {
    Select(usize),
    Play(usize),
    Candidate(usize),
    StandIn(usize),
}

enum BulkAction {
    Mark(bool),
    StandIn(bool),
    CandidateCategory(Option<String>),
    StandInCategory(Option<String>),
    Replacement(Option<String>),
}

impl Desktop {
    pub fn new(ctx: &egui::Context, args: Args) -> Self {
        let mut visuals = egui::Visuals::dark();
        visuals.selection.bg_fill = Color32::from_rgb(46, 60, 75);
        visuals.override_text_color = Some(Color32::from_gray(218));
        visuals.weak_text_color = Some(Color32::from_gray(151));
        ctx.set_visuals(visuals);
        ctx.style_mut(|style| {
            style.spacing.item_spacing.y = 5.;
            style
                .text_styles
                .insert(egui::TextStyle::Body, egui::FontId::proportional(14.));
            style
                .text_styles
                .insert(egui::TextStyle::Button, egui::FontId::proportional(14.));
            style
                .text_styles
                .insert(egui::TextStyle::Heading, egui::FontId::proportional(20.));
        });
        Self {
            loading: Some(Job::start(ctx, |cancelled| load(args, cancelled))),
            library: None,
            player: Player::default(),
            filters: Filters::default(),
            visible: Vec::new(),
            selection: Selection::default(),
            warning: String::new(),
            status: None,
        }
    }

    fn poll(&mut self) {
        if let Some(result) = self.loading.as_ref().and_then(Job::poll) {
            self.loading = None;
            match result {
                Ok(library) => {
                    eprintln!(
                        "Audio Review is ready with {} audio files.",
                        count(library.review.audio_count)
                    );
                    self.warning = library.game.warnings.join(" ");
                    self.library = Some(library);
                    self.refresh();
                    self.select_first_if_empty();
                }
                Err(error) => self.report(Err(error.context("Could not open the audio library"))),
            }
        }
        if let Err(error) = self.player.poll() {
            self.report(Err(error.context("Could not play the preview")));
        }
    }

    fn report(&mut self, result: Result<String>) {
        self.status = Some(match result {
            Ok(message) => (message, false),
            Err(error) => {
                eprintln!("{error:#}");
                (format!("{error:#}"), true)
            }
        });
    }

    fn refresh(&mut self) {
        if let Some(library) = &self.library {
            let filters = &self.filters;
            self.visible = library.review.filter(
                &filters.query,
                filters.view,
                filters.category.as_deref(),
                filters.kind.as_deref(),
            );
            self.selection.retain_visible(&self.visible);
        }
    }

    fn select_first_if_empty(&mut self) {
        if self.selection.primary().is_none()
            && let Some(first) = self.visible.first()
        {
            self.selection.single(*first);
        }
    }

    fn header(&mut self, ui: &mut egui::Ui, frame: &eframe::Frame) {
        let Some(library) = &self.library else {
            ui.heading("Misophonia Audio Review");
            return;
        };
        let review = &library.review;
        let mut export = false;
        ui.horizontal(|ui| {
            ui.heading("Misophonia Audio Review");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                export = ui.button("Export review…").clicked();
                ui.weak(format!(
                    "{} audio files  ·  Misophonia {}",
                    count(review.audio_count),
                    review.catalog.version
                ));
            });
        });
        ui.add_space(6.);
        let previous = self.filters.clone();
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.filters.query)
                    .desired_width(260.)
                    .hint_text("Search sounds, events or IDs"),
            )
            .on_hover_text("Search filenames, media IDs, and linked game events");
            egui::ComboBox::from_id_salt("coverage-filter")
                .selected_text(self.filters.view.label())
                .width(120.)
                .show_ui(ui, |ui| {
                    for (view, label) in View::OPTIONS {
                        ui.selectable_value(&mut self.filters.view, view, label);
                    }
                })
                .response
                .on_hover_text("Filter by mod coverage or review tag");
            categories(
                ui,
                "filter-category",
                "All",
                &mut self.filters.category,
                &review.catalog.categories,
                150.,
            )
            .on_hover_text("Category filter. All sounds and Outside the mod keep files with no linked mod coverage visible.");
            egui::ComboBox::from_id_salt("kind-filter")
                .selected_text(
                    self.filters
                        .kind
                        .as_deref()
                        .map_or("All audio types", kind_label),
                )
                .width(115.)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.filters.kind, None, "All audio types");
                    for kind in &review.kinds {
                        ui.selectable_value(
                            &mut self.filters.kind,
                            Some(kind.clone()),
                            kind_label(kind),
                        );
                    }
                });
        });
        if previous != self.filters {
            self.refresh();
            self.select_first_if_empty();
        }
        if export {
            self.export(frame);
        }
    }

    fn export(&mut self, frame: &eframe::Frame) {
        let path = rfd::FileDialog::new()
            .set_parent(frame)
            .set_title("Export review")
            .set_file_name("misophonia-sound-review.json")
            .add_filter("JSON", &["json"])
            .save_file();
        if let Some(path) = path
            && let Some(library) = &self.library
        {
            self.report(
                library
                    .review
                    .write_export(&path)
                    .map(|()| format!("Review exported to {}", path.display()))
                    .map_err(|error| error.context("Could not export the review")),
            );
        }
    }

    fn table(&mut self, ui: &mut egui::Ui) -> Option<RowAction> {
        let review = &self.library.as_ref()?.review;
        let mut action = None;
        let multiselect = ui.input(|input| {
            input.modifiers.shift || input.modifiers.command || input.modifiers.ctrl
        });
        if self.visible.is_empty() {
            ui.centered_and_justified(|ui| ui.weak("No matching sounds"));
            return None;
        }
        TableBuilder::new(ui)
            .striped(true)
            .sense(egui::Sense::click())
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::exact(53.))
            .column(Column::remainder().at_least(150.).clip(true))
            .column(
                Column::initial(182.)
                    .range(100.0..=320.0)
                    .resizable(true)
                    .clip(true),
            )
            .columns(Column::exact(35.), 2)
            .header(27., |mut row| {
                for label in ["Audio", "Sound", "Mod coverage", "X", "O"] {
                    row.col(|ui| {
                        ui.strong(label);
                    });
                }
            })
            .body(|body| {
                body.rows(29., self.visible.len(), |mut row| {
                    let index = self.visible[row.index()];
                    let sound = &review.sounds[index];
                    let tags = review.tags(&sound.asset.id);
                    let covered = !sound.coverage.is_empty();
                    let removal_action = covered || tags.remove_coverage;
                    let active = self.player.action(index);
                    let (name_tint, coverage_tint) = tints(tags);
                    row.set_selected(self.selection.contains(index));
                    row.col(|ui| {
                        ui.scope_builder(
                            egui::UiBuilder::new().id(("preview", &sound.asset.id)),
                            |ui| {
                                if ui
                                    .add_enabled(sound.playable, egui::Button::new(active.label()))
                                    .on_hover_text(if sound.playable {
                                        active.label()
                                    } else {
                                        "No installed audio file linked"
                                    })
                                    .clicked()
                                {
                                    action = Some(RowAction::Play(index));
                                }
                            },
                        );
                    });
                    row.col(|ui| {
                        tint_cell(ui, name_tint);
                        let mut name = RichText::new(&sound.asset.name);
                        if covered {
                            name = name.color(RED);
                        } else if tags.stand_in {
                            name = name.color(GREEN);
                        }
                        let response = ui
                            .add(
                                egui::Label::new(name)
                                    .truncate()
                                    .sense(egui::Sense::click()),
                            )
                            .on_hover_text(
                                sound.asset.path.as_deref().unwrap_or(&sound.asset.detail),
                            );
                        if !multiselect && response.double_clicked() && sound.playable {
                            action = Some(RowAction::Play(index));
                        } else if response.clicked() {
                            action = Some(RowAction::Select(index));
                        }
                    });
                    row.col(|ui| {
                        tint_cell(ui, coverage_tint);
                        let mut labels = Vec::new();
                        if tags.remove_coverage {
                            labels.push("Removal candidate".to_owned());
                        }
                        for coverage in &sound.coverage {
                            let name =
                                category_name(&review.catalog.categories, &coverage.category);
                            let mode = if coverage.mode == "volume" {
                                "slider"
                            } else {
                                "mute"
                            };
                            let label = format!("{name} · {mode}");
                            if !labels.contains(&label) {
                                labels.push(label);
                            }
                        }
                        let coverage = if labels.is_empty() {
                            "—".to_owned()
                        } else {
                            labels.join(" / ")
                        };
                        let response = ui
                            .add(
                                egui::Label::new(&coverage)
                                    .truncate()
                                    .sense(egui::Sense::click()),
                            )
                            .on_hover_text(coverage);
                        if response.clicked() {
                            action = Some(RowAction::Select(index));
                        }
                    });
                    row.col(|ui| {
                        ui.scope_builder(
                            egui::UiBuilder::new().id(("candidate", &sound.asset.id)),
                            |ui| {
                                if tag_button(
                                    ui,
                                    "X",
                                    if removal_action { TAG_GRAY } else { RED },
                                    TRIGGER,
                                    if removal_action {
                                        tags.remove_coverage
                                    } else {
                                        tags.candidate
                                    },
                                    true,
                                    if tags.remove_coverage {
                                        "Clear coverage removal candidate"
                                    } else if covered {
                                        "Mark for removal from mod coverage"
                                    } else {
                                        "Tag trigger candidate"
                                    },
                                ) {
                                    action = Some(RowAction::Candidate(index));
                                }
                            },
                        );
                    });
                    row.col(|ui| {
                        ui.scope_builder(
                            egui::UiBuilder::new().id(("stand_in", &sound.asset.id)),
                            |ui| {
                                if tag_button(
                                    ui,
                                    "O",
                                    if tags.stand_in { TAG_GRAY } else { GREEN },
                                    STAND_IN,
                                    tags.stand_in,
                                    sound.playable,
                                    if tags.stand_in {
                                        "Remove stand-in tag"
                                    } else {
                                        "Tag stand-in"
                                    },
                                ) {
                                    action = Some(RowAction::StandIn(index));
                                }
                            },
                        );
                    });
                    let response = row.response();
                    if action.is_none()
                        && !multiselect
                        && response.double_clicked()
                        && sound.playable
                    {
                        action = Some(RowAction::Play(index));
                    } else if action.is_none() && response.clicked() {
                        action = Some(RowAction::Select(index));
                    }
                });
            });
        action
    }

    fn apply(&mut self, ctx: &egui::Context, action: RowAction) {
        let Some(library) = &self.library else { return };
        match action {
            RowAction::Select(index) => {
                let modifiers = ctx.input(|input| input.modifiers);
                self.selection.click(
                    &self.visible,
                    index,
                    modifiers.shift,
                    modifiers.command || modifiers.ctrl,
                );
            }
            RowAction::Play(index) => {
                if self.selection.len() <= 1 {
                    self.selection.single(index);
                }
                let sound = &library.review.sounds[index];
                self.player
                    .toggle(ctx, &library.game, index, &sound.asset.id);
                ctx.request_repaint();
            }
            RowAction::Candidate(index) | RowAction::StandIn(index) => {
                if !self.selection.contains(index) {
                    self.selection.single(index);
                }
                let sound = &library.review.sounds[index];
                let tags = library.review.tags(&sound.asset.id);
                let action = if matches!(action, RowAction::Candidate(_)) {
                    BulkAction::Mark(!candidate_marked(sound, tags))
                } else {
                    BulkAction::StandIn(!tags.stand_in)
                };
                self.apply_bulk(action);
            }
        }
    }

    fn apply_bulk(&mut self, action: BulkAction) {
        let Some(library) = &self.library else { return };
        let edits = self
            .selection
            .iter()
            .filter_map(|index| {
                let sound = &library.review.sounds[index];
                let existing = library.review.tags(&sound.asset.id);
                let mut tags = existing.clone();
                edit_tags(&action, sound, &mut tags);
                (tags != *existing).then(|| (sound.asset.id.clone(), tags))
            })
            .collect();
        self.save(edits);
    }

    fn save(&mut self, edits: Vec<(String, Tags)>) {
        if edits.is_empty() {
            return;
        }
        let Some(library) = &mut self.library else {
            return;
        };
        let edited = edits.len();
        let outcome = library.review.save_many(edits);
        self.refresh();
        self.report(
            outcome
                .map(|reset| {
                    let summary = if edited == 1 {
                        "Review tag saved.".to_owned()
                    } else {
                        format!("Saved tags for {} sounds.", count(edited))
                    };
                    if reset == 0 {
                        summary
                    } else {
                        format!(
                            "{summary} Reset {} replacement choices to mute.",
                            count(reset)
                        )
                    }
                })
                .map_err(|error| error.context("Could not save the tags")),
        );
    }

    fn transport(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let Some(library) = &self.library else { return };
        let usable = self
            .selection
            .primary()
            .filter(|index| library.review.sounds[*index].playable);
        let index = self.player.index.or(usable);
        let action = index.map_or(Action::Play, |index| self.player.action(index));
        let name = match (action, self.player.index) {
            (Action::Cancel, _) => "Decoding…",
            (_, Some(index)) => &library.review.sounds[index].asset.name,
            _ => "Ready to preview",
        };
        let (at, length) = self.player.timing();
        let mut seek = at.as_secs_f64();
        let mut seek_to = None;
        let mut toggle = false;
        ui.horizontal(|ui| {
            toggle = ui
                .add_enabled(index.is_some(), egui::Button::new(action.label()))
                .clicked();
            ui.allocate_ui_with_layout(
                egui::vec2(174., 18.),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.add(egui::Label::new(name).truncate())
                        .on_hover_text(name);
                },
            );
            ui.spacing_mut().slider_width = (ui.available_width() - 243.).max(70.);
            if track_slider(ui, &mut seek, length.as_secs_f64(), "Preview position").1 {
                seek_to = Some(Duration::from_secs_f64(seek));
            }
            ui.weak(format!("{} / {}", time(at), time(length)));
            ui.separator();
            ui.spacing_mut().slider_width = 66.;
            ui.label(format!("Volume {}%", self.player.volume));
            let mut volume = f64::from(self.player.volume);
            if track_slider(ui, &mut volume, 100., "Preview volume").1 {
                self.player.volume = volume.round() as u8;
                self.player.update_volume();
            }
        });
        if toggle && let Some(index) = index {
            let sound = &library.review.sounds[index];
            self.player
                .toggle(ctx, &library.game, index, &sound.asset.id);
            ctx.request_repaint();
        }
        if let Some(at) = seek_to {
            if let Err(error) = self.player.seek(at) {
                self.report(Err(error));
            }
            ctx.request_repaint();
        }
        if index.is_some_and(|index| self.player.action(index) == Action::Pause) {
            ctx.request_repaint_after(Duration::from_millis(40));
        }
    }
}

impl eframe::App for Desktop {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.poll();
        egui::TopBottomPanel::top("header").show(ctx, |ui| self.header(ui, frame));
        egui::TopBottomPanel::bottom("transport").show(ctx, |ui| {
            self.transport(ui, ctx);
            if self.library.is_some() {
                ui.horizontal(|ui| {
                    ui.weak(format!("{} matching sounds", count(self.visible.len())));
                    if self.selection.len() > 1 {
                        ui.separator();
                        ui.weak(format!("{} selected", count(self.selection.len())));
                    }
                    if let Some((message, error)) = &self.status {
                        ui.separator();
                        ui.label(RichText::new(message).color(if *error {
                            RED
                        } else {
                            ui.visuals().weak_text_color()
                        }));
                    }
                });
                if !self.warning.is_empty() {
                    ui.colored_label(RED, &self.warning);
                }
            }
        });
        if let Some(library) = &self.library {
            if self.selection.len() > 1 {
                let action = egui::SidePanel::right("inspect")
                    .default_width(294.)
                    .width_range(270.0..=440.0)
                    .show(ctx, |ui| {
                        bulk_inspector(ui, &library.review, &self.selection)
                    })
                    .inner;
                if let Some(action) = action {
                    self.apply_bulk(action);
                }
            } else if let Some(index) = self.selection.primary() {
                let changed = egui::SidePanel::right("inspect")
                    .default_width(294.)
                    .width_range(270.0..=440.0)
                    .show(ctx, |ui| inspector(ui, &library.review, index))
                    .inner;
                if let Some(tags) = changed {
                    let id = library.review.sounds[index].asset.id.clone();
                    self.save(vec![(id, tags)]);
                }
            }
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.library.is_some() {
                if let Some(action) = self.table(ui) {
                    self.apply(ctx, action);
                }
            } else if self.loading.is_some() {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Preparing the game audio library…");
                });
            } else if let Some((message, _)) = &self.status {
                ui.colored_label(RED, message);
            }
        });
    }
}

fn candidate_marked(sound: &Sound, tags: &Tags) -> bool {
    if !sound.coverage.is_empty() || tags.remove_coverage {
        tags.remove_coverage
    } else {
        tags.candidate
    }
}

fn edit_tags(action: &BulkAction, sound: &Sound, tags: &mut Tags) {
    match action {
        BulkAction::Mark(marked) if !sound.coverage.is_empty() || tags.remove_coverage => {
            tags.remove_coverage = *marked;
        }
        BulkAction::Mark(marked) if tags.candidate != *marked => {
            tags.candidate = *marked;
            tags.category = if *marked {
                tags.stand_in_category.clone()
            } else {
                None
            };
            tags.replacement = None;
        }
        BulkAction::StandIn(marked) if sound.playable && tags.stand_in != *marked => {
            tags.stand_in = *marked;
            tags.stand_in_category = if *marked { tags.category.clone() } else { None };
        }
        BulkAction::CandidateCategory(category) if tags.candidate && tags.category != *category => {
            tags.category.clone_from(category);
            tags.replacement = None;
        }
        BulkAction::StandInCategory(category) if tags.stand_in => {
            tags.stand_in_category.clone_from(category);
        }
        BulkAction::Replacement(donor) if tags.candidate => tags.replacement.clone_from(donor),
        _ => {}
    }
}

fn common<'a>(mut values: impl Iterator<Item = Option<&'a str>>) -> Option<Option<&'a str>> {
    let first = values.next()?;
    values.all(|value| value == first).then_some(first)
}

fn bulk_inspector(ui: &mut egui::Ui, review: &Review, selection: &Selection) -> Option<BulkAction> {
    let selected = selection.iter().map(|index| {
        let sound = &review.sounds[index];
        (sound, review.tags(&sound.asset.id))
    });
    let mut marked = 0;
    let mut stand_in_count = 0;
    let mut can_tag_stand_in = false;
    for (sound, tags) in selected.clone() {
        marked += usize::from(candidate_marked(sound, tags));
        stand_in_count += usize::from(tags.stand_in);
        can_tag_stand_in |= sound.playable && !tags.stand_in;
    }
    let candidates = selected
        .clone()
        .filter_map(|(_, tags)| tags.candidate.then_some(tags));
    let stand_ins = selected.filter_map(|(_, tags)| tags.stand_in.then_some(tags));
    let mut action = None;
    ui.push_id("bulk-review", |ui| {
        ui.heading(format!("{} sounds selected", count(selection.len())));
        ui.horizontal(|ui| {
            if ui
                .add_enabled(marked < selection.len(), egui::Button::new("Mark X"))
                .on_hover_text("Tag trigger candidates or mark covered sounds for removal")
                .clicked()
            {
                action = Some(BulkAction::Mark(true));
            }
            if ui
                .add_enabled(marked > 0, egui::Button::new("Clear X"))
                .on_hover_text("Clear the X mark from selected sounds")
                .clicked()
            {
                action = Some(BulkAction::Mark(false));
            }
        });
        ui.horizontal(|ui| {
            if ui
                .add_enabled(can_tag_stand_in, egui::Button::new("Tag stand-ins"))
                .clicked()
            {
                action = Some(BulkAction::StandIn(true));
            }
            if ui
                .add_enabled(stand_in_count > 0, egui::Button::new("Clear stand-ins"))
                .clicked()
            {
                action = Some(BulkAction::StandIn(false));
            }
        });
        ui.separator();

        let candidate_category = common(candidates.clone().map(|tags| tags.category.as_deref()));
        let common_replacement = common(candidates.clone().map(|tags| tags.replacement.as_deref()));
        let category_list = &review.catalog.categories;
        let category_width = ui.available_width() - 12.;
        ui.label("Candidate category");
        ui.add_enabled_ui(candidates.clone().next().is_some(), |ui| {
            // An empty ID lets the shared picker distinguish mixed values from clearing all.
            let mut choice =
                candidate_category.map_or_else(|| Some(String::new()), |id| id.map(str::to_owned));
            let placeholder = if candidate_category.is_none() {
                "Mixed or unassigned"
            } else {
                "Choose category"
            };
            if categories(
                ui,
                "candidate",
                placeholder,
                &mut choice,
                category_list,
                category_width,
            )
            .changed()
            {
                action = Some(BulkAction::CandidateCategory(choice));
            }
        });

        ui.label("Replacement");
        ui.add_enabled_ui(candidates.clone().next().is_some(), |ui| {
            let donors = review.stand_ins(candidate_category.flatten(), "");
            let description = match common_replacement {
                Some(None) => "Mute (default)",
                Some(Some(id)) => donors
                    .iter()
                    .find(|donor| donor.id == id)
                    .map_or("Saved stand-in (unavailable)", |donor| donor.name.as_str()),
                None => "Mixed replacements",
            };
            egui::ComboBox::from_id_salt("replacement")
                .selected_text(description)
                .truncate()
                .width(category_width)
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(common_replacement == Some(None), "Mute (default)")
                        .clicked()
                    {
                        action = Some(BulkAction::Replacement(None));
                    }
                    if candidate_category.flatten().is_none() {
                        ui.weak("Assign a shared category to choose a stand-in.");
                    }
                    for donor in donors {
                        let self_replacement = review.index(&donor.id).is_some_and(|index| {
                            selection.contains(index) && review.tags(&donor.id).candidate
                        });
                        if !self_replacement
                            && ui
                                .selectable_label(
                                    common_replacement == Some(Some(donor.id.as_str())),
                                    &donor.name,
                                )
                                .clicked()
                        {
                            action = Some(BulkAction::Replacement(Some(donor.id.clone())));
                        }
                    }
                });
        });

        let stand_in_category = common(stand_ins.map(|tags| tags.stand_in_category.as_deref()));
        ui.label("Stand-in category");
        ui.add_enabled_ui(stand_in_count > 0, |ui| {
            let mut choice =
                stand_in_category.map_or_else(|| Some(String::new()), |id| id.map(str::to_owned));
            let placeholder = if stand_in_category.is_none() {
                "Mixed or unassigned"
            } else {
                "Choose category"
            };
            if categories(
                ui,
                "stand-in",
                placeholder,
                &mut choice,
                category_list,
                category_width,
            )
            .changed()
            {
                action = Some(BulkAction::StandInCategory(choice));
            }
        });
    });
    action
}

fn inspector(ui: &mut egui::Ui, review: &Review, index: usize) -> Option<Tags> {
    let sound = &review.sounds[index];
    let original = review.tags(&sound.asset.id);
    let mut tags = original.clone();
    let categories_list = &review.catalog.categories;
    ui.push_id(&sound.asset.id, |ui| {
        ui.heading(&sound.asset.name);
        ui.add(
            egui::Label::new(sound.asset.path.as_deref().unwrap_or(&sound.asset.detail))
                .selectable(true),
        );
        ui.add_space(8.);
        for entry in &sound.coverage {
            let category = categories_list
                .iter()
                .find(|category| category.id == entry.category);
            if let Some(category) = category {
                let control = if entry.mode == "volume" {
                    "volume slider"
                } else {
                    "mute"
                };
                ui.weak(format!(
                    "{} / {} · {} {control}",
                    category.section, category.name, entry.edition
                ));
            }
        }
        if sound.coverage.is_empty() {
            ui.weak("No linked mod coverage");
        }
        if tags.remove_coverage {
            ui.colored_label(RED, "Candidate for removal from mod coverage");
        }
        if tags.stand_in {
            ui.colored_label(GREEN, "Stand-in");
        }
        if tags.candidate
            && (!sound.coverage.is_empty() || tags.remove_coverage)
            && ui.button("Clear trigger candidate").clicked()
        {
            tags.candidate = false;
            tags.category = None;
            tags.replacement = None;
        }
        ui.separator();

        ui.label("Candidate category");
        ui.add_enabled_ui(tags.candidate, |ui| {
            if categories(
                ui,
                "candidate",
                "Choose category",
                &mut tags.category,
                categories_list,
                ui.available_width() - 12.,
            )
            .changed()
            {
                tags.replacement = None;
            }
        });
        ui.label("Replacement");
        ui.add_enabled_ui(tags.candidate, |ui| {
            let donors = review.stand_ins(tags.category.as_deref(), &sound.asset.id);
            let selection = tags.replacement.as_deref().map_or("Mute (default)", |id| {
                donors
                    .iter()
                    .find(|asset| asset.id == id)
                    .map_or("Saved stand-in (unavailable)", |asset| &asset.name)
            });
            egui::ComboBox::from_id_salt("replacement")
                .selected_text(selection)
                .truncate()
                .width(ui.available_width() - 12.)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut tags.replacement, None, "Mute (default)");
                    for donor in donors {
                        ui.selectable_value(
                            &mut tags.replacement,
                            Some(donor.id.clone()),
                            &donor.name,
                        );
                    }
                });
        });
        ui.label("Stand-in category");
        ui.add_enabled_ui(tags.stand_in, |ui| {
            categories(
                ui,
                "stand-in",
                "Choose category",
                &mut tags.stand_in_category,
                categories_list,
                ui.available_width() - 12.,
            );
        });
        ui.separator();
        ui.strong("Linked events");
        egui::ScrollArea::vertical().show(ui, |ui| {
            if sound.events.is_empty() {
                ui.weak("None");
            }
            for name in &sound.events {
                ui.add(egui::Label::new(name).selectable(true));
            }
        });
    });
    (tags != *original).then_some(tags)
}

fn categories(
    ui: &mut egui::Ui,
    id: &str,
    empty: &str,
    value: &mut Option<String>,
    categories: &[Category],
    width: f32,
) -> egui::Response {
    let label = value
        .as_deref()
        .and_then(|id| categories.iter().find(|item| item.id == id))
        .map_or_else(
            || empty.to_owned(),
            |item| format!("{} / {}", item.section, item.name),
        );
    let mut changed = false;
    let mut response = egui::ComboBox::from_id_salt(id)
        .selected_text(label)
        .truncate()
        .width(width)
        .show_ui(ui, |ui| {
            changed |= ui.selectable_value(value, None, empty).changed();
            for category in categories {
                changed |= ui
                    .selectable_value(
                        value,
                        Some(category.id.clone()),
                        format!("{} / {}", category.section, category.name),
                    )
                    .changed();
            }
        })
        .response;
    if changed {
        response.mark_changed();
    }
    response
}

fn category_name<'a>(categories: &'a [Category], key: &'a str) -> &'a str {
    categories
        .iter()
        .find(|category| category.id == key)
        .map_or(key, |category| &category.name)
}

fn tints(tags: &Tags) -> (Option<Color32>, Option<Color32>) {
    let name = if tags.candidate || tags.remove_coverage {
        Some(TRIGGER)
    } else if tags.stand_in {
        Some(STAND_IN)
    } else {
        None
    };
    let coverage = if tags.stand_in { Some(STAND_IN) } else { name };
    (name, coverage)
}

fn tint_cell(ui: &egui::Ui, color: Option<Color32>) {
    if let Some(color) = color {
        ui.painter().rect_filled(ui.max_rect(), 2., color);
    }
}

fn tag_button(
    ui: &mut egui::Ui,
    text: &str,
    foreground: Color32,
    tint: Color32,
    selected: bool,
    enabled: bool,
    tooltip: &str,
) -> bool {
    let label = RichText::new(text).strong().color(foreground);
    let mut button = egui::Button::new(label).min_size(egui::vec2(29., 21.));
    if selected {
        button = button.fill(tint);
    }
    ui.add_enabled(enabled, button)
        .on_hover_text(tooltip)
        .clicked()
}

fn track_slider(
    ui: &mut egui::Ui,
    value: &mut f64,
    max: f64,
    label: &str,
) -> (egui::Response, bool) {
    let response = ui
        .add_enabled(
            max > 0.,
            egui::Slider::new(value, 0.0..=max.max(0.001)).show_value(false),
        )
        .on_hover_text(label)
        .interact(egui::Sense::click());
    let mut changed = response.changed();
    // egui's drag-only slider can miss a press and release in the same frame.
    if !changed
        && response.clicked()
        && let Some(pointer) = response.interact_pointer_pos()
    {
        let bounds = response.rect.x_range().shrink(5.);
        let fraction = egui::remap_clamp(pointer.x, bounds, 0.0..=1.0);
        *value = f64::from(fraction) * max;
        changed = true;
    }
    (response, changed)
}

fn time(value: Duration) -> String {
    let seconds = value.as_secs();
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

fn count(number: usize) -> String {
    let digits = number.to_string();
    let mut result = String::new();
    for (index, c) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            result.push(',');
        }
        result.push(c);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn track_slider_accepts_a_quick_click() {
        let context = egui::Context::default();
        let mut value = 0.0;
        let mut rect = egui::Rect::NOTHING;
        let mut changed = false;
        let mut frame = |events| {
            let _ = context.run(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let result = track_slider(ui, &mut value, 100., "Test slider");
                        rect = result.0.rect;
                        changed = result.1;
                    });
                },
            );
            rect.center()
        };
        frame(Vec::new());
        let pos = frame(Vec::new());
        frame(vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        assert!(changed);
        assert!((value - 50.).abs() < 1.);
    }

    #[test]
    fn dual_tag_has_distinct_cells() {
        assert_eq!(tints(&Tags::default()), (None, None));
        let mut tags = Tags {
            candidate: true,
            ..Default::default()
        };
        assert_eq!(tints(&tags), (Some(TRIGGER), Some(TRIGGER)));
        tags.stand_in = true;
        assert_eq!(tints(&tags), (Some(TRIGGER), Some(STAND_IN)));
        tags.candidate = false;
        assert_eq!(tints(&tags), (Some(STAND_IN), Some(STAND_IN)));
    }
}
