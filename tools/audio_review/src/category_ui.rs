use std::collections::{BTreeMap, BTreeSet};

use eframe::egui;

use crate::{catalog::Category, review::Review};

pub fn split_picker(
    ui: &mut egui::Ui,
    id: &str,
    value: &mut Option<String>,
    categories: &[Category],
    width: f32,
) -> bool {
    let memory = ui.make_persistent_id((id, "pending-category"));
    let find_section = |value: &Option<String>| {
        categories
            .iter()
            .find(|category| Some(&category.id) == value.as_ref())
            .map(|category| category.section.to_owned())
    };
    let assigned_section = find_section(value);
    type Pending = (Option<String>, Option<String>, Option<String>);
    let saved: Option<Pending> = ui.data(|data| data.get_temp(memory));
    let sections: BTreeSet<_> = categories
        .iter()
        .map(|category| category.section.as_str())
        .collect();
    let mut section = match saved {
        Some((previous, previous_assignment, section))
            if previous == *value
                && previous_assignment == assigned_section
                && section
                    .as_deref()
                    .is_none_or(|section| sections.contains(section)) =>
        {
            section
        }
        _ => assigned_section,
    };
    let old = value.clone();

    egui::ComboBox::from_id_salt((id, "category"))
        .selected_text(section.as_deref().unwrap_or("Choose category"))
        .truncate()
        .width(width)
        .show_ui(ui, |ui| {
            if ui
                .selectable_value(&mut section, None, "Choose category")
                .clicked()
            {
                *value = None;
            }
            for section_name in sections {
                ui.selectable_value(&mut section, Some(section_name.to_owned()), section_name);
            }
        });
    let label = categories
        .iter()
        .find(|category| {
            Some(&category.id) == value.as_ref() && Some(&category.section) == section.as_ref()
        })
        .map_or_else(
            || {
                if value.as_deref() == Some("") {
                    "Mixed subcategories"
                } else {
                    "Choose subcategory"
                }
            },
            |category| category.name.as_str(),
        );
    ui.add_enabled_ui(section.is_some(), |ui| {
        egui::ComboBox::from_id_salt((id, "subcategory"))
            .selected_text(label)
            .truncate()
            .width(width)
            .show_ui(ui, |ui| {
                ui.selectable_value(value, None, "Choose subcategory");
                for category in categories
                    .iter()
                    .filter(|category| Some(&category.section) == section.as_ref())
                {
                    ui.selectable_value(value, Some(category.id.clone()), &category.name);
                }
            });
    });
    ui.data_mut(|data| data.insert_temp(memory, (value.clone(), find_section(value), section)));
    *value != old
}

#[derive(Default)]
pub struct CategoryEditor {
    pub open: bool,
    form: Option<Form>,
    confirm_remove: bool,
    error: Option<String>,
}

#[derive(Clone)]
enum Form {
    Category {
        original: String,
        name: String,
    },
    Subcategory {
        id: Option<String>,
        section: String,
        name: String,
    },
}

impl CategoryEditor {
    pub fn show(&mut self, ctx: &egui::Context, review: &mut Review) -> bool {
        if !self.open {
            return false;
        }
        let Self {
            open,
            form,
            confirm_remove,
            error,
        } = self;
        let mut changed = false;
        egui::Window::new("Manage categories")
            .open(open)
            .default_width(695.)
            .show(ctx, |ui| {
                let mut sections: BTreeMap<String, Vec<Category>> = BTreeMap::new();
                for category in &review.categories {
                    sections.entry(category.section.clone()).or_default().push(category.clone());
                }
                let mut next_form = None;
                ui.columns(2, |cols| {
                    let left = &mut cols[0];
                    if left.button("Add subcategory").clicked() {
                        let section = match form.as_ref() {
                            Some(Form::Category { original, .. }) => original.clone(),
                            Some(Form::Subcategory { section, .. }) => section.clone(),
                            None => String::new(),
                        };
                        next_form = Some(Form::Subcategory { id: None, section, name: String::new() });
                    }
                    egui::ScrollArea::vertical().id_salt("manage-category-list").max_height(375.).show(left, |ui| {
                        for (section, children) in sections {
                            ui.push_id(&section, |ui| {
                                ui.collapsing(&section, |ui| {
                                    if ui.button("Edit category").clicked() {
                                        next_form = Some(Form::Category { original: section.to_owned(), name: section.to_owned() });
                                    }
                                    for category in children {
                                        let selected = matches!(form, Some(Form::Subcategory { id: Some(id), .. }) if *id == category.id);
                                        if ui.selectable_label(selected, &category.name).clicked() {
                                            next_form = Some(Form::Subcategory {
                                                id: Some(category.id.clone()),
                                                section: category.section.clone(),
                                                name: category.name.clone(),
                                            });
                                        }
                                    }
                                });
                            });
                        }
                    });

                    if let Some(next) = next_form {
                        *form = Some(next);
                        *confirm_remove = false;
                        *error = None;
                    }
                    let right = &mut cols[1];
                    let Some(current) = form else {
                        right.weak("Choose a category or subcategory to edit.");
                        return;
                    };
                    let existing = match current {
                        Form::Category { name, .. } => {
                            right.label("Category name");
                            right.text_edit_singleline(name);
                            true
                        }
                        Form::Subcategory { id, section, name } => {
                            right.label("Category name");
                            right.text_edit_singleline(section).on_hover_text("Enter an existing category or create a new one.");
                            right.label("Subcategory name");
                            right.text_edit_singleline(name);
                            id.is_some()
                        }
                    };
                    let mut result = None;
                    right.horizontal(|ui| {
                        if ui.button("Save").clicked() {
                            result = Some(match current {
                                Form::Category { original, name } => {
                                    review.rename_section(original, name).map(|()| *original = name.trim().to_owned())
                                }
                                Form::Subcategory { id, section, name } => {
                                    if let Some(id) = id {
                                        review.edit_category(id, section, name)
                                    } else {
                                        review.add_category(section, name).map(|new| *id = Some(new))
                                    }
                                }
                            });
                        }
                        if existing && !*confirm_remove && ui.button("Remove…").clicked() {
                            *confirm_remove = true;
                        }
                    });
                    let mut removed = false;
                    if *confirm_remove {
                        right.label("Remove this from the custom mod and clear its review assignments?");
                        right.horizontal(|ui| {
                            if ui.button("Remove").clicked() {
                                result = Some(match current {
                                    Form::Category { original, .. } => review.remove_section(original),
                                    Form::Subcategory { id: Some(id), .. } => review.remove_category(id),
                                    Form::Subcategory { id: None, .. } => unreachable!("only saved categories can be removed"),
                                });
                                removed = true;
                            }
                            if ui.button("Cancel").clicked() {
                                *confirm_remove = false;
                            }
                        });
                    }
                    if let Some(result) = result {
                        match result {
                            Ok(()) => {
                                *error = None;
                                changed = true;
                                if removed {
                                    *form = None;
                                    *confirm_remove = false;
                                }
                            }
                            Err(problem) => *error = Some(format!("{problem:#}")),
                        }
                    }
                    if let Some(message) = error.as_ref() {
                        right.colored_label(egui::Color32::LIGHT_RED, message);
                    }
                });
            });
        changed
    }
}
