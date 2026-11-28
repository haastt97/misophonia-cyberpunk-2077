use std::collections::{BTreeMap, BTreeSet};

use eframe::egui;

use crate::{catalog::Category, review::Review};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Choice<'a> {
    Clear,
    Section(&'a str),
    Subcategory { id: &'a str, section: &'a str },
}

impl<'a> Choice<'a> {
    fn subcategory(category: &'a Category) -> Self {
        Self::Subcategory {
            id: &category.id,
            section: &category.section,
        }
    }
}

struct CategorySearch {
    words: Vec<String>,
}

impl CategorySearch {
    fn new(value: &str) -> Self {
        Self {
            words: value
                .split(|ch: char| ch.is_whitespace() || ch == '/')
                .filter(|word| !word.is_empty())
                .map(str::to_lowercase)
                .collect(),
        }
    }

    fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    fn matches(&self, section: &str, name: &str) -> bool {
        if self.is_empty() {
            return true;
        }
        let section = section.to_lowercase();
        let name = name.to_lowercase();
        self.words
            .iter()
            .all(|word| section.contains(word) || name.contains(word))
    }
}

fn subcategory_choices<'a>(
    categories: &'a [Category],
    section: Option<&str>,
    search: &CategorySearch,
) -> Vec<(Choice<'a>, String)> {
    let scope = section.filter(|_| search.is_empty());
    categories
        .iter()
        .filter(|category| {
            scope.is_none_or(|name| category.section == name)
                && search.matches(&category.section, &category.name)
        })
        .map(|category| {
            let label = if scope.is_some() {
                category.name.clone()
            } else {
                format!("{} / {}", category.section, category.name)
            };
            (Choice::subcategory(category), label)
        })
        .collect()
}

fn searchable_combo<'a>(
    ui: &mut egui::Ui,
    salt: impl std::hash::Hash + Copy,
    label: &str,
    width: f32,
    selected: Choice<'a>,
    options: impl FnOnce(&CategorySearch) -> Vec<(Choice<'a>, String)>,
) -> (egui::Response, Option<Choice<'a>>) {
    let combo_id = ui.make_persistent_id(egui::Id::new(salt));
    let was_open = egui::ComboBox::is_open(ui.ctx(), combo_id);
    let enabled = ui.is_enabled();
    let search_id = combo_id.with("query");
    let mut query: String = if was_open {
        ui.data(|data| data.get_temp(search_id)).unwrap_or_default()
    } else {
        String::new()
    };

    let response = egui::ComboBox::from_id_salt(salt)
        .selected_text(label)
        .truncate()
        .width(width)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show_ui(ui, |ui| {
            if !enabled {
                return None;
            }
            let search_box = ui.add(
                egui::TextEdit::singleline(&mut query)
                    .desired_width(width)
                    .hint_text("Search categories…"),
            );
            if !was_open {
                search_box.request_focus();
            }
            let search = CategorySearch::new(&query);
            let items = options(&search);
            if items.is_empty() {
                ui.weak("No matches");
            }
            let enter = !search.is_empty()
                && (search_box.has_focus() || search_box.lost_focus())
                && ui.input(|input| input.key_pressed(egui::Key::Enter));
            for (index, (choice, label)) in items.into_iter().enumerate() {
                if ui.selectable_label(choice == selected, label).clicked() || (enter && index == 0)
                {
                    ui.close();
                    return Some(choice);
                }
            }
            None
        });
    if response.inner.is_some() {
        ui.data_mut(|data| data.insert_temp(search_id, query));
    }
    (response.response, response.inner.flatten())
}

pub fn filter_picker(
    ui: &mut egui::Ui,
    value: &mut Option<String>,
    categories: &[Category],
) -> egui::Response {
    let chosen = categories
        .iter()
        .find(|category| Some(&category.id) == value.as_ref());
    let title = chosen.map_or_else(
        || "All".to_owned(),
        |category| format!("{} / {}", category.section, category.name),
    );
    let selected = chosen.map_or(Choice::Clear, Choice::subcategory);
    let (response, choice) =
        searchable_combo(ui, "filter-category", &title, 150., selected, |search| {
            let mut options = subcategory_choices(categories, None, search);
            if search.is_empty() {
                options.insert(0, (Choice::Clear, "All".to_owned()));
            }
            options
        });
    match choice {
        Some(Choice::Subcategory { id, .. }) => *value = Some(id.to_owned()),
        Some(Choice::Clear) => *value = None,
        _ => (),
    }
    response
}

pub fn split_picker(
    ui: &mut egui::Ui,
    id: &str,
    value: &mut Option<String>,
    categories: &[Category],
    width: f32,
) -> bool {
    let memory = ui.make_persistent_id((id, "pending-category"));
    let find_category = |value: &Option<String>| {
        categories
            .iter()
            .find(|category| Some(&category.id) == value.as_ref())
    };
    let find_section =
        |value: &Option<String>| find_category(value).map(|category| category.section.to_owned());
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
    let selected = section.as_deref().map_or(Choice::Clear, Choice::Section);
    let (_, choice) = searchable_combo(
        ui,
        (id, "category"),
        section.as_deref().unwrap_or("Choose category"),
        width,
        selected,
        |search| {
            let mut options: Vec<_> = sections
                .iter()
                .filter(|section| search.matches(section, ""))
                .map(|&section| (Choice::Section(section), section.to_owned()))
                .collect();
            if search.is_empty() {
                options.insert(0, (Choice::Clear, "Choose category".to_owned()));
            } else {
                options.extend(subcategory_choices(categories, None, search));
            }
            options
        },
    );
    match choice {
        Some(Choice::Clear) => {
            *value = None;
            section = None;
        }
        Some(Choice::Section(name)) => section = Some(name.to_owned()),
        Some(Choice::Subcategory {
            id,
            section: parent,
        }) => {
            *value = Some(id.to_owned());
            section = Some(parent.to_owned());
        }
        None => (),
    }

    let chosen =
        find_category(value).filter(|category| Some(&category.section) == section.as_ref());
    let title = chosen.map_or(
        if value.as_deref() == Some("") {
            "Mixed subcategories"
        } else {
            "Choose subcategory"
        },
        |category| category.name.as_str(),
    );
    let selected = chosen.map_or(Choice::Clear, Choice::subcategory);
    let (_, choice) = searchable_combo(ui, (id, "subcategory"), title, width, selected, |search| {
        let mut options = subcategory_choices(categories, section.as_deref(), search);
        if search.is_empty() {
            options.insert(0, (Choice::Clear, "Choose subcategory".to_owned()));
        }
        options
    });
    match choice {
        Some(Choice::Clear) => *value = None,
        Some(Choice::Subcategory {
            id,
            section: parent,
        }) => {
            *value = Some(id.to_owned());
            section = Some(parent.to_owned());
        }
        _ => (),
    }
    ui.data_mut(|data| data.insert_temp(memory, (value.clone(), find_section(value), section)));
    *value != old
}

#[derive(Default)]
pub struct CategoryEditor {
    pub open: bool,
    form: Option<Form>,
    query: String,
    reveal_section: Option<String>,
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
            query,
            reveal_section,
            confirm_remove,
            error,
        } = self;
        let mut changed = false;
        egui::Window::new("Manage categories")
            .open(open)
            .default_width(695.)
            .show(ctx, |ui| {
                let reveal = reveal_section.take();
                let mut sections: BTreeMap<String, Vec<Category>> = BTreeMap::new();
                for category in &review.categories {
                    sections.entry(category.section.clone()).or_default().push(category.clone());
                }
                let mut next_form = None;
                ui.columns(2, |cols| {
                    let left = &mut cols[0];
                    left.add(egui::TextEdit::singleline(query).hint_text("Search categories…"));
                    let search = CategorySearch::new(query);
                    if left.button("Add subcategory").clicked() {
                        let section = match form.as_ref() {
                            Some(Form::Category { original, .. }) => original.clone(),
                            Some(Form::Subcategory { section, .. }) => section.clone(),
                            None => String::new(),
                        };
                        next_form = Some(Form::Subcategory { id: None, section, name: String::new() });
                    }
                    egui::ScrollArea::vertical().id_salt("manage-category-list").max_height(375.).show(left, |ui| {
                        let mut found = false;
                        for (section, mut children) in sections {
                            children.retain(|category| search.matches(&section, &category.name));
                            if children.is_empty() {
                                continue;
                            }
                            found = true;
                            let searching = !search.is_empty();
                            ui.push_id((&section, searching), |ui| {
                                egui::CollapsingHeader::new(&section)
                                    .open((searching || reveal.as_deref() == Some(section.as_str())).then_some(true))
                                    .show(ui, |ui| {
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
                        if !found && !search.is_empty() {
                            ui.weak("No matches");
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
                            right.text_edit_singleline(section).on_hover_text(if id.is_some() {
                                "Change the parent category to move this subcategory and its assigned sounds."
                            } else {
                                "Enter an existing category or create a new one."
                            });
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
                                query.clear();
                                changed = true;
                                if removed {
                                    *form = None;
                                    *confirm_remove = false;
                                } else {
                                    *reveal_section = Some(match current {
                                        Form::Category { original, .. } => original.clone(),
                                        Form::Subcategory { section, .. } => section.trim().to_owned(),
                                    });
                                }
                                ctx.request_repaint();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subcategory_search_matches_words_across_groups_and_labels() {
        let categories = [
            Category {
                id: "paper".into(),
                section: "Repetitive".into(),
                name: "Paper and Plastic".into(),
                mode: "mute",
            },
            Category {
                id: "pages".into(),
                section: "Environment".into(),
                name: "Paper pages".into(),
                mode: "mute",
            },
            Category {
                id: "water".into(),
                section: "Environment".into(),
                name: "Water".into(),
                mode: "mute",
            },
        ];
        let labels = |scope, query| {
            subcategory_choices(&categories, scope, &CategorySearch::new(query))
                .into_iter()
                .map(|(_, label)| label)
                .collect::<Vec<_>>()
        };
        assert_eq!(labels(Some("Environment"), ""), ["Paper pages", "Water"]);
        assert_eq!(
            labels(Some("Environment"), " PAPER "),
            [
                "Repetitive / Paper and Plastic",
                "Environment / Paper pages"
            ]
        );
        assert_eq!(
            labels(None, "paper repet"),
            ["Repetitive / Paper and Plastic"]
        );
        assert_eq!(
            labels(None, "Repetitive / Paper"),
            ["Repetitive / Paper and Plastic"]
        );
        assert!(labels(None, "paper water").is_empty());
    }
}
