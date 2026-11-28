mod projects;

use projects::ProjectState;

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, OptionalExtension, Row, TransactionBehavior, params};
use serde::{Deserialize, Serialize};

use crate::{
    catalog::{Catalog, Category, MediaEvents, Sound},
    game::Asset,
    review_categories::{self, CategoryEdit, CategoryEdits},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum View {
    #[default]
    All,
    Covered,
    Outside,
    Candidates,
    StandIns,
    Untagged,
}

impl View {
    pub const OPTIONS: [(Self, &'static str); 6] = [
        (Self::All, "All sounds"),
        (Self::Covered, "In the mod"),
        (Self::Outside, "Outside the mod"),
        (Self::Candidates, "Candidates"),
        (Self::StandIns, "Stand-ins"),
        (Self::Untagged, "Untagged"),
    ];

    pub fn label(self) -> &'static str {
        Self::OPTIONS
            .iter()
            .find(|(item, _)| *item == self)
            .unwrap()
            .1
    }

    fn matches(self, sound: &Sound, tags: &Tags) -> bool {
        match self {
            Self::All => true,
            Self::Covered => !sound.coverage.is_empty(),
            Self::Outside => sound.playable && sound.coverage.is_empty(),
            Self::Candidates => tags.candidate || tags.remove_coverage,
            Self::StandIns => tags.stand_in,
            Self::Untagged => !tags.candidate && !tags.stand_in && !tags.remove_coverage,
        }
    }
}

pub fn kind_label(kind: &str) -> &str {
    match kind {
        "sfx" => "Sound effects",
        "sound" => "Streamed sounds",
        "voice" => "Voice-over",
        "music" => "Music",
        "event" => "Event only",
        other => other,
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Tags {
    pub candidate: bool,
    pub category: Option<String>,
    pub stand_in: bool,
    pub stand_in_category: Option<String>,
    pub replacement: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub remove_coverage: bool,
}

static EMPTY_TAGS: Tags = Tags {
    candidate: false,
    category: None,
    stand_in: false,
    stand_in_category: None,
    replacement: None,
    remove_coverage: false,
};

fn read_tags(row: &Row<'_>, first: usize) -> rusqlite::Result<Tags> {
    Ok(Tags {
        candidate: row.get(first)?,
        category: row.get(first + 1)?,
        stand_in: row.get(first + 2)?,
        stand_in_category: row.get(first + 3)?,
        replacement: row.get(first + 4)?,
        remove_coverage: row.get(first + 5)?,
    })
}

struct Store(Connection);

struct Workspace {
    categories: CategoryEdits,
    tags: BTreeMap<String, Tags>,
    project: ProjectState,
}

impl Store {
    fn open(path: &Path) -> Result<Self> {
        let mut db = Connection::open(path).with_context(|| format!("Open {}", path.display()))?;
        db.busy_timeout(Duration::from_secs(5))?;
        db.execute_batch("PRAGMA journal_mode=WAL;")?;
        let schema = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        schema.execute_batch(
            "CREATE TABLE IF NOT EXISTS tags (
                id TEXT PRIMARY KEY, candidate INTEGER NOT NULL, category TEXT,
                stand_in INTEGER NOT NULL, stand_in_category TEXT, replacement TEXT,
                remove_coverage INTEGER NOT NULL DEFAULT 0
             );
             CREATE TABLE IF NOT EXISTS category_edits (
                id TEXT PRIMARY KEY, section TEXT NOT NULL, name TEXT NOT NULL, removed INTEGER NOT NULL
             );",
        )?;
        let has_removal_flag = schema.query_row(
            "SELECT EXISTS (SELECT 1 FROM pragma_table_info('tags') WHERE name = 'remove_coverage')",
            [],
            |row| row.get::<_, bool>(0),
        )?;
        if !has_removal_flag {
            schema.execute_batch(
                "ALTER TABLE tags ADD COLUMN remove_coverage INTEGER NOT NULL DEFAULT 0;",
            )?;
        }
        ProjectState::initialize(&schema)?;
        schema.commit()?;
        Ok(Self(db))
    }

    fn entries(db: &Connection) -> Result<BTreeMap<String, Tags>> {
        let mut stmt = db.prepare(
            "SELECT id, candidate, category, stand_in, stand_in_category, replacement, remove_coverage FROM tags",
        )?;
        let entries = stmt.query_map([], |row| Ok((row.get(0)?, read_tags(row, 1)?)))?;
        Ok(entries.collect::<rusqlite::Result<_>>()?)
    }

    fn categories(db: &Connection) -> Result<CategoryEdits> {
        let mut stmt = db.prepare("SELECT id, section, name, removed FROM category_edits")?;
        let entries = stmt.query_map([], |row| {
            Ok((
                row.get(0)?,
                CategoryEdit {
                    section: row.get(1)?,
                    name: row.get(2)?,
                    removed: row.get(3)?,
                },
            ))
        })?;
        Ok(entries.collect::<rusqlite::Result<_>>()?)
    }

    fn snapshot(&self) -> Result<Workspace> {
        let snapshot = self.0.unchecked_transaction()?;
        let workspace = Workspace {
            categories: Self::categories(&snapshot)?,
            tags: Self::entries(&snapshot)?,
            project: ProjectState::read(&snapshot)?,
        };
        snapshot.commit()?;
        Ok(workspace)
    }

    fn write_category(db: &Connection, id: &str, edit: &CategoryEdit) -> Result<()> {
        db.execute(
            "INSERT INTO category_edits (id, section, name, removed) VALUES (?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET section = excluded.section, name = excluded.name, removed = excluded.removed",
            params![id, edit.section, edit.name, edit.removed],
        )?;
        Ok(())
    }

    fn write_tags(db: &Connection, id: &str, tags: &Tags) -> Result<()> {
        if tags.candidate || tags.stand_in || tags.remove_coverage {
            db.execute(
                "INSERT INTO tags (id, candidate, category, stand_in, stand_in_category, replacement, remove_coverage)
                 VALUES (?, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT(id) DO UPDATE SET candidate = excluded.candidate,
                    category = excluded.category, stand_in = excluded.stand_in,
                    stand_in_category = excluded.stand_in_category, replacement = excluded.replacement,
                    remove_coverage = excluded.remove_coverage",
                params![id, tags.candidate, tags.category, tags.stand_in, tags.stand_in_category, tags.replacement, tags.remove_coverage],
            )?;
        } else {
            db.execute("DELETE FROM tags WHERE id = ?", [id])?;
        }
        Ok(())
    }

    fn edit_categories(
        &mut self,
        expected_epoch: i64,
        expected: &CategoryEdits,
        edits: &CategoryEdits,
    ) -> Result<()> {
        let db = self
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        ProjectState::current(&db, expected_epoch)?;
        ensure!(
            &Self::categories(&db)? == expected,
            "Categories changed in another window. Review the refreshed categories and try again"
        );
        for (id, edit) in edits {
            Self::write_category(&db, id, edit)?;
            if edit.removed {
                db.execute(
                    "UPDATE tags SET replacement = NULL
                     WHERE replacement IN (SELECT id FROM tags WHERE stand_in_category = ?)",
                    [id],
                )?;
                db.execute(
                    "UPDATE tags SET category = NULL, replacement = NULL WHERE category = ?",
                    [id],
                )?;
                db.execute(
                    "UPDATE tags SET stand_in_category = NULL WHERE stand_in_category = ?",
                    [id],
                )?;
            }
        }
        ProjectState::mark_dirty(&db)?;
        db.commit()?;
        Ok(())
    }

    fn save_many(
        &mut self,
        expected_epoch: i64,
        categories: &CategoryEdits,
        edits: &[(&str, &Tags, &Tags)],
    ) -> Result<usize> {
        let db = self
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        ProjectState::current(&db, expected_epoch)?;
        ensure!(
            &Self::categories(&db)? == categories,
            "Categories changed in another window. Review the refreshed categories and try again"
        );
        for (id, expected, tags) in edits {
            let current = db.query_row(
                "SELECT candidate, category, stand_in, stand_in_category, replacement, remove_coverage FROM tags WHERE id = ?",
                [id], |row| read_tags(row, 0),
            ).optional()?.unwrap_or_default();
            ensure!(
                &current == *expected,
                "A selected sound changed in another window. Review the refreshed tags and try again"
            );
            Self::write_tags(&db, id, tags)?;
        }
        for (_, expected, tags) in edits {
            if tags.replacement == expected.replacement && tags.category == expected.category {
                continue;
            }
            if let Some(replacement) = &tags.replacement {
                let donor: Option<(bool, Option<String>)> = db
                    .query_row(
                        "SELECT stand_in, stand_in_category FROM tags WHERE id = ?",
                        [replacement],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .optional()?;
                ensure!(
                    donor.is_some_and(|(enabled, category)| enabled && category == tags.category),
                    "Choose a stand-in from the same category"
                );
            }
        }
        let mut reset = 0;
        for (id, _, tags) in edits {
            let donor_category = if tags.stand_in {
                tags.stand_in_category.as_deref()
            } else {
                None
            };
            reset += db.execute(
                "UPDATE tags SET replacement = NULL WHERE replacement = ? AND (? IS NULL OR category IS NOT ?)",
                params![id, donor_category, donor_category],
            )?;
        }
        if !edits.is_empty() {
            ProjectState::mark_dirty(&db)?;
        }
        db.commit()?;
        Ok(reset)
    }
}

fn normalized(value: &str) -> String {
    value.to_lowercase().replace(['_', '/', '\\', '-'], " ")
}

pub struct Review {
    pub catalog: Catalog,
    pub categories: Vec<Category>,
    category_edits: CategoryEdits,
    pub(crate) media_events: MediaEvents,
    pub sounds: Vec<Sound>,
    pub kinds: Vec<String>,
    pub audio_count: usize,
    by_id: HashMap<String, usize>,
    search: Vec<String>,
    tags: BTreeMap<String, Tags>,
    store: Store,
    project: ProjectState,
    data_dir: PathBuf,
}

impl Review {
    pub fn open(
        catalog: Catalog,
        assets: Vec<Asset>,
        media: &MediaEvents,
        data: &Path,
    ) -> Result<Self> {
        let sounds = catalog.sounds(assets, media);
        let audio_count = sounds.iter().filter(|sound| sound.playable).count();
        let mut ordered: Vec<_> = sounds
            .into_iter()
            .map(|sound| (sound.asset.name.to_lowercase(), sound))
            .collect();
        ordered.sort_unstable_by(|(left_name, left), (right_name, right)| {
            (left.events.is_empty(), left_name, &left.asset.id).cmp(&(
                right.events.is_empty(),
                right_name,
                &right.asset.id,
            ))
        });
        let sounds: Vec<_> = ordered.into_iter().map(|(_, sound)| sound).collect();
        let by_id: HashMap<_, _> = sounds
            .iter()
            .enumerate()
            .map(|(index, sound)| (sound.asset.id.clone(), index))
            .collect();
        ensure!(
            by_id.len() == sounds.len(),
            "The game audio index contains repeated sound IDs"
        );
        let search = sounds
            .iter()
            .map(|sound| {
                let asset = &sound.asset;
                normalized(&format!(
                    "{} {} {} {} {}",
                    asset.name,
                    asset.detail,
                    asset.id,
                    asset.path.as_deref().unwrap_or_default(),
                    sound.events.join(" ")
                ))
            })
            .collect();
        let mut kinds: Vec<_> = sounds
            .iter()
            .map(|sound| sound.asset.kind.as_str())
            .collect();
        kinds.sort_unstable();
        kinds.dedup();
        let kinds = kinds.into_iter().map(str::to_owned).collect();

        fs::create_dir_all(data).with_context(|| format!("Create {}", data.display()))?;
        let store = Store::open(&data.join("review.sqlite3"))?;
        let snapshot = store.snapshot()?;
        let categories = review_categories::effective(&catalog.categories, &snapshot.categories);
        review_categories::validate(&categories)?;
        Ok(Self {
            catalog,
            categories,
            category_edits: snapshot.categories,
            media_events: media.clone(),
            sounds,
            kinds,
            audio_count,
            by_id,
            search,
            tags: snapshot.tags,
            project: snapshot.project,
            data_dir: data.canonicalize()?,
            store,
        })
    }

    pub fn index(&self, id: &str) -> Option<usize> {
        self.by_id.get(id).copied()
    }

    pub fn tags(&self, id: &str) -> &Tags {
        self.tags.get(id).unwrap_or(&EMPTY_TAGS)
    }

    pub fn filter(
        &self,
        query: &str,
        view: View,
        category: Option<&str>,
        kind: Option<&str>,
    ) -> Vec<usize> {
        let query = normalized(query);
        let words: Vec<_> = query.split_whitespace().collect();
        let keep_outside = matches!(view, View::All | View::Outside | View::Untagged);
        self.sounds
            .iter()
            .zip(&self.search)
            .enumerate()
            .filter_map(|(index, (sound, searchable))| {
                let tags = self.tags(&sound.asset.id);
                let matches = kind.is_none_or(|kind| kind == sound.asset.kind)
                    && view.matches(sound, tags)
                    && category.is_none_or(|category| {
                        (keep_outside && sound.playable && sound.coverage.is_empty())
                            || tags.category.as_deref() == Some(category)
                            || tags.stand_in_category.as_deref() == Some(category)
                            || sound
                                .coverage
                                .iter()
                                .any(|entry| entry.category == category)
                    })
                    && words.iter().all(|word| searchable.contains(word));
                matches.then_some(index)
            })
            .collect()
    }

    pub fn stand_ins(&self, category: Option<&str>, excluded: &str) -> Vec<&Asset> {
        let Some(category) = category else {
            return Vec::new();
        };
        self.tags
            .iter()
            .filter_map(|(id, tags)| {
                if id == excluded
                    || !tags.stand_in
                    || tags.stand_in_category.as_deref() != Some(category)
                {
                    return None;
                }
                let sound = &self.sounds[self.index(id)?];
                sound.playable.then_some(&sound.asset)
            })
            .collect()
    }

    #[cfg(test)]
    pub fn save(&mut self, id: &str, tags: Tags) -> Result<usize> {
        self.save_many(vec![(id.to_owned(), tags)])
    }

    pub fn save_many(&mut self, edits: Vec<(String, Tags)>) -> Result<usize> {
        let mut ids = BTreeSet::new();
        let mut rows = Vec::with_capacity(edits.len());
        for (id, tags) in &edits {
            ensure!(ids.insert(id), "The same sound was selected more than once");
            self.validate_tags(id, tags)?;
            rows.push((id.as_str(), self.tags.get(id).unwrap_or(&EMPTY_TAGS), tags));
        }
        let result = self
            .store
            .save_many(self.project.epoch, &self.category_edits, &rows);
        self.refresh_saved_review()?;
        result
    }

    fn validate_tags(&self, id: &str, tags: &Tags) -> Result<()> {
        let index = self
            .index(id)
            .context("The selected sound is unavailable")?;
        for (value, enabled) in [
            (&tags.category, tags.candidate),
            (&tags.stand_in_category, tags.stand_in),
        ] {
            if let Some(key) = value {
                ensure!(
                    enabled && self.categories.iter().any(|category| &category.id == key),
                    "Choose a valid category for the active tag"
                );
            }
        }
        ensure!(
            !tags.stand_in || self.sounds[index].playable,
            "Only installed audio files can be stand-ins"
        );
        if let Some(replacement) = &tags.replacement {
            ensure!(
                replacement != id
                    && tags.candidate
                    && tags.category.is_some()
                    && self
                        .index(replacement)
                        .is_some_and(|index| self.sounds[index].playable),
                "Choose a different playable stand-in from the same category"
            );
        }
        let expected = self.tags.get(id).unwrap_or(&EMPTY_TAGS);
        ensure!(
            !tags.remove_coverage
                || expected.remove_coverage
                || !self.sounds[index].coverage.is_empty(),
            "Only sounds with mod coverage can be marked for removal"
        );
        Ok(())
    }

    pub fn add_category(&mut self, section: &str, name: &str) -> Result<String> {
        let id = (1..)
            .map(|next| format!("custom_{next}"))
            .find(|id| {
                !self.category_edits.contains_key(id)
                    && !self
                        .catalog
                        .categories
                        .iter()
                        .any(|category| &category.id == id)
            })
            .context("Could not allocate a category identifier")?;
        self.apply_category_edits(BTreeMap::from([(
            id.clone(),
            CategoryEdit::assign(section, name)?,
        )]))?;
        Ok(id)
    }

    pub fn edit_category(&mut self, id: &str, section: &str, name: &str) -> Result<()> {
        self.category(id)
            .context("This subcategory no longer exists")?;
        self.apply_category_edits(BTreeMap::from([(
            id.to_owned(),
            CategoryEdit::assign(section, name)?,
        )]))
    }

    pub fn rename_section(&mut self, section: &str, name: &str) -> Result<()> {
        let name = review_categories::label(name)?;
        let edits = self
            .categories
            .iter()
            .filter(|category| category.section == section)
            .map(|category| {
                (
                    category.id.clone(),
                    CategoryEdit {
                        section: name.clone(),
                        name: category.name.clone(),
                        removed: false,
                    },
                )
            })
            .collect();
        self.apply_category_edits(edits)
    }

    pub fn remove_section(&mut self, name: &str) -> Result<()> {
        let edits = self
            .categories
            .iter()
            .filter(|category| category.section == name)
            .map(|category| (category.id.clone(), CategoryEdit::remove(category)))
            .collect();
        self.apply_category_edits(edits)
    }

    pub fn remove_category(&mut self, id: &str) -> Result<()> {
        let category = self
            .category(id)
            .context("This subcategory no longer exists")?;
        self.apply_category_edits(BTreeMap::from([(
            id.to_owned(),
            CategoryEdit::remove(category),
        )]))
    }

    pub fn category(&self, id: &str) -> Option<&Category> {
        self.categories.iter().find(|category| category.id == id)
    }

    fn apply_category_edits(&mut self, changes: CategoryEdits) -> Result<()> {
        ensure!(!changes.is_empty(), "This category no longer exists");
        let mut proposed = self.category_edits.clone();
        proposed.extend(changes.clone());
        let categories = review_categories::effective(&self.catalog.categories, &proposed);
        review_categories::validate(&categories)?;
        let result = self
            .store
            .edit_categories(self.project.epoch, &self.category_edits, &changes);
        self.refresh_saved_review()?;
        result
    }

    fn refresh_saved_review(&mut self) -> Result<()> {
        let snapshot = self.store.snapshot()?;
        ensure!(
            snapshot.project.epoch == self.project.epoch,
            "Another window switched projects. Reopen Audio Review to continue with the active project"
        );
        if self.category_edits != snapshot.categories {
            self.categories =
                review_categories::effective(&self.catalog.categories, &snapshot.categories);
            self.category_edits = snapshot.categories;
        }
        self.tags = snapshot.tags;
        self.project = snapshot.project;
        Ok(())
    }

    pub(crate) fn saved_tags(&self) -> Result<BTreeMap<String, Tags>> {
        let workspace = self.store.snapshot()?;
        ensure!(
            workspace.project.epoch == self.project.epoch,
            "Another window switched projects. Reopen Audio Review to continue with the active project"
        );
        ensure!(
            workspace.categories == self.category_edits,
            "Categories changed in another window. Reopen Audio Review to load them"
        );
        Ok(workspace.tags)
    }

    fn export(&self) -> Result<serde_json::Value> {
        let sounds: Result<Vec<_>> = self
            .saved_tags()?
            .into_iter()
            .map(|(id, tags)| {
                let mut sound = match self.index(&id) {
                    Some(index) => serde_json::to_value(&self.sounds[index])?,
                    None => serde_json::json!({"id": id, "unavailable": true}),
                };
                sound["tags"] = serde_json::to_value(tags)?;
                Ok(sound)
            })
            .collect();
        Ok(serde_json::json!({
            "schema_version": 1,
            "mod_version": self.catalog.version,
            "categories": self.categories,
            "sounds": sounds?,
        }))
    }

    pub fn write_export(&self, path: &Path) -> Result<()> {
        let directory = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let mut temporary = tempfile::NamedTempFile::new_in(directory)?;
        let mut writer = BufWriter::new(temporary.as_file_mut());
        serde_json::to_writer_pretty(&mut writer, &self.export()?)?;
        writer.write_all(b"\n")?;
        writer.flush()?;
        drop(writer);
        temporary.as_file().sync_all()?;
        temporary.persist(path)?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "review_tests.rs"]
mod tests;
