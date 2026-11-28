use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

use crate::catalog::Category;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct CategoryEdit {
    pub section: String,
    pub name: String,
    pub removed: bool,
}

impl CategoryEdit {
    pub fn assign(section: &str, name: &str) -> Result<Self> {
        Ok(Self {
            section: label(section)?,
            name: label(name)?,
            removed: false,
        })
    }

    pub fn remove(category: &Category) -> Self {
        Self {
            section: category.section.clone(),
            name: category.name.clone(),
            removed: true,
        }
    }
}

pub type CategoryEdits = BTreeMap<String, CategoryEdit>;

pub fn effective(base: &[Category], edits: &CategoryEdits) -> Vec<Category> {
    let mut result = Vec::with_capacity(base.len() + edits.len());
    for category in base {
        match edits.get(&category.id) {
            Some(edit) if edit.removed => (),
            Some(edit) => result.push(Category {
                section: edit.section.clone(),
                name: edit.name.clone(),
                ..category.clone()
            }),
            None => result.push(category.clone()),
        }
    }
    for (id, edit) in edits {
        if id.starts_with("custom_") && !edit.removed && !base.iter().any(|c| &c.id == id) {
            result.push(Category {
                id: id.clone(),
                section: edit.section.clone(),
                name: edit.name.clone(),
                mode: "mute",
            });
        }
    }
    result
}

pub fn validate(categories: &[Category]) -> Result<()> {
    let mut display_names = BTreeSet::new();
    let mut section_spellings = BTreeMap::new();
    for category in categories {
        let section = category.section.to_lowercase();
        let name = category.name.to_lowercase();
        let spelling = section_spellings
            .entry(section.clone())
            .or_insert(&category.section);
        ensure!(
            *spelling == &category.section,
            "Use the existing category spelling: {spelling}"
        );
        ensure!(
            section != "none",
            "None is reserved by Mod Settings. Choose another category name"
        );
        ensure!(
            name != "all" && !(section == "general" && name == "all in general"),
            "{} is reserved for a Mod Settings switch",
            category.name
        );
        ensure!(
            display_names.insert((section, name)),
            "A subcategory named {} already exists in {}",
            category.name,
            category.section
        );
    }
    Ok(())
}

pub fn label(text: &str) -> Result<String> {
    let text = text.trim();
    ensure!(!text.is_empty(), "Enter a category and subcategory name");
    ensure!(
        text.chars().count() <= 80,
        "Category names must be at most 80 characters"
    );
    ensure!(
        !text
            .chars()
            .any(|c| c.is_control() || matches!(c, '"' | '\\')),
        "Category names cannot contain double quotes, backslashes, or control characters"
    );
    Ok(text.to_owned())
}
