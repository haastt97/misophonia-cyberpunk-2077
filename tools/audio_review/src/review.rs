use std::{
    collections::{BTreeMap, HashMap},
    fs,
    io::{BufWriter, Write},
    path::Path,
    time::Duration,
};

use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, OptionalExtension, Row, TransactionBehavior, params};
use serde::Serialize;

use crate::{
    catalog::{Catalog, MediaEvents, Sound},
    game::Asset,
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
            Self::Candidates => tags.candidate,
            Self::StandIns => tags.stand_in,
            Self::Untagged => !tags.candidate && !tags.stand_in,
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

#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
pub struct Tags {
    pub candidate: bool,
    pub category: Option<String>,
    pub stand_in: bool,
    pub stand_in_category: Option<String>,
    pub replacement: Option<String>,
}

static EMPTY_TAGS: Tags = Tags {
    candidate: false,
    category: None,
    stand_in: false,
    stand_in_category: None,
    replacement: None,
};

fn read_tags(row: &Row<'_>, first: usize) -> rusqlite::Result<Tags> {
    Ok(Tags {
        candidate: row.get(first)?,
        category: row.get(first + 1)?,
        stand_in: row.get(first + 2)?,
        stand_in_category: row.get(first + 3)?,
        replacement: row.get(first + 4)?,
    })
}

struct Store(Connection);

impl Store {
    fn open(path: &Path) -> Result<Self> {
        let db = Connection::open(path).with_context(|| format!("Open {}", path.display()))?;
        db.busy_timeout(Duration::from_secs(5))?;
        db.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS tags (
                id TEXT PRIMARY KEY, candidate INTEGER NOT NULL, category TEXT,
                stand_in INTEGER NOT NULL, stand_in_category TEXT, replacement TEXT
             );",
        )?;
        Ok(Self(db))
    }

    fn entries(&self) -> Result<BTreeMap<String, Tags>> {
        let mut stmt = self.0.prepare(
            "SELECT id, candidate, category, stand_in, stand_in_category, replacement FROM tags",
        )?;
        let entries = stmt.query_map([], |row| Ok((row.get(0)?, read_tags(row, 1)?)))?;
        Ok(entries.collect::<rusqlite::Result<_>>()?)
    }

    fn save(&mut self, id: &str, expected: &Tags, tags: &Tags) -> Result<usize> {
        let db = self
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = db.query_row(
            "SELECT candidate, category, stand_in, stand_in_category, replacement FROM tags WHERE id = ?",
            [id], |row| read_tags(row, 0),
        ).optional()?.unwrap_or_default();
        ensure!(
            &current == expected,
            "This sound changed in another window. Review the refreshed tags and try again"
        );
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
        if tags.candidate || tags.stand_in {
            db.execute(
                "INSERT INTO tags (id, candidate, category, stand_in, stand_in_category, replacement)
                 VALUES (?, ?, ?, ?, ?, ?)
                 ON CONFLICT(id) DO UPDATE SET candidate = excluded.candidate,
                    category = excluded.category, stand_in = excluded.stand_in,
                    stand_in_category = excluded.stand_in_category, replacement = excluded.replacement",
                params![id, tags.candidate, tags.category, tags.stand_in, tags.stand_in_category, tags.replacement],
            )?;
        } else {
            db.execute("DELETE FROM tags WHERE id = ?", [id])?;
        }
        let donor_category = if tags.stand_in {
            tags.stand_in_category.as_deref()
        } else {
            None
        };
        let reset = db.execute(
            "UPDATE tags SET replacement = NULL WHERE replacement = ? AND (? IS NULL OR category IS NOT ?)",
            params![id, donor_category, donor_category],
        )?;
        db.commit()?;
        Ok(reset)
    }
}

fn normalized(value: &str) -> String {
    value.to_lowercase().replace(['_', '/', '\\', '-'], " ")
}

pub struct Review {
    pub catalog: Catalog,
    pub sounds: Vec<Sound>,
    pub kinds: Vec<String>,
    pub audio_count: usize,
    by_id: HashMap<String, usize>,
    search: Vec<String>,
    tags: BTreeMap<String, Tags>,
    store: Store,
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
        let tags = store.entries()?;
        Ok(Self {
            catalog,
            sounds,
            kinds,
            audio_count,
            by_id,
            search,
            tags,
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
        self.sounds
            .iter()
            .zip(&self.search)
            .enumerate()
            .filter_map(|(index, (sound, searchable))| {
                let tags = self.tags(&sound.asset.id);
                let matches = kind.is_none_or(|kind| kind == sound.asset.kind)
                    && view.matches(sound, tags)
                    && category.is_none_or(|category| {
                        tags.category.as_deref() == Some(category)
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

    pub fn save(&mut self, id: &str, tags: Tags) -> Result<usize> {
        let index = self
            .index(id)
            .context("The selected sound is unavailable")?;
        for (value, enabled) in [
            (&tags.category, tags.candidate),
            (&tags.stand_in_category, tags.stand_in),
        ] {
            if let Some(key) = value {
                ensure!(
                    enabled
                        && self
                            .catalog
                            .categories
                            .iter()
                            .any(|category| &category.id == key),
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
        let result = self.store.save(id, expected, &tags);
        self.tags = self.store.entries()?;
        result
    }

    fn export(&self) -> Result<serde_json::Value> {
        let sounds: Result<Vec<_>> = self
            .store
            .entries()?
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
            "categories": self.catalog.categories,
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
