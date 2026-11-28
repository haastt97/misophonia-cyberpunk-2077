use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{Review, Store, Tags};
use crate::review_categories::{self, CategoryEdits};

const FORMAT: &str = "misophonia-audio-review-project";
const VERSION: u32 = 1;
const MAX_FILE_BYTES: usize = 128 * 1024 * 1024;

#[derive(Clone)]
pub(super) struct ProjectState {
    pub epoch: i64,
    path: Option<PathBuf>,
    recovery_path: Option<PathBuf>,
    catalog_version: Option<String>,
    digest: Option<String>,
    dirty: bool,
    minutes: u32,
}

impl ProjectState {
    pub fn initialize(db: &Connection) -> Result<()> {
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS review_project (
                singleton INTEGER PRIMARY KEY CHECK(singleton = 1),
                epoch INTEGER NOT NULL, path TEXT, recovery_path TEXT,
                catalog_version TEXT, digest TEXT, dirty INTEGER NOT NULL, minutes INTEGER NOT NULL CHECK(minutes > 0)
             );
             INSERT OR IGNORE INTO review_project
             VALUES (1, 1, NULL, NULL, NULL, NULL, (EXISTS (SELECT 1 FROM tags) OR EXISTS (SELECT 1 FROM category_edits)), 1);",
        )?;
        Ok(())
    }

    pub fn read(db: &Connection) -> Result<Self> {
        Ok(db.query_row(
            "SELECT epoch, path, recovery_path, catalog_version, digest, dirty, minutes FROM review_project WHERE singleton = 1",
            [],
            |row| {
                Ok(Self {
                    epoch: row.get(0)?,
                    path: row.get::<_, Option<String>>(1)?.map(PathBuf::from),
                    recovery_path: row.get::<_, Option<String>>(2)?.map(PathBuf::from),
                    catalog_version: row.get(3)?,
                    digest: row.get(4)?,
                    dirty: row.get(5)?,
                    minutes: row.get(6)?,
                })
            },
        )?)
    }

    pub fn current(db: &Connection, expected_epoch: i64) -> Result<Self> {
        let state = Self::read(db)?;
        ensure!(
            state.epoch == expected_epoch,
            "Another window switched projects. Reopen Audio Review to continue with the active project"
        );
        Ok(state)
    }

    pub fn mark_dirty(db: &Connection) -> Result<()> {
        db.execute(
            "UPDATE review_project SET dirty = 1 WHERE singleton = 1",
            [],
        )?;
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
struct ProjectFile {
    format: String,
    schema_version: u32,
    catalog_version: String,
    category_edits: CategoryEdits,
    tags: BTreeMap<String, Tags>,
}

impl ProjectFile {
    fn empty(version: &str) -> Self {
        Self {
            format: FORMAT.to_owned(),
            schema_version: VERSION,
            catalog_version: version.to_owned(),
            category_edits: BTreeMap::new(),
            tags: BTreeMap::new(),
        }
    }

    fn from_db(db: &Connection, state: &ProjectState, version: &str) -> Result<Self> {
        Ok(Self {
            category_edits: Store::categories(db)?,
            tags: Store::entries(db)?,
            ..Self::empty(state.catalog_version.as_deref().unwrap_or(version))
        })
    }

    fn read(path: &Path) -> Result<(Self, String)> {
        let bytes = read_bytes(path)?;
        let project: Self = serde_json::from_slice(&bytes)
            .with_context(|| format!("Read Audio Review project {}", path.display()))?;
        ensure!(
            project.format == FORMAT && project.schema_version == VERSION,
            "This file is not a supported Audio Review project"
        );
        Ok((project, digest(&bytes)))
    }

    fn validate(&self, review: &Review) -> Result<()> {
        for (id, edit) in &self.category_edits {
            ensure!(
                !id.is_empty()
                    && id
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_'),
                "Invalid category ID {id:?} in this project"
            );
            review_categories::label(&edit.section)?;
            review_categories::label(&edit.name)?;
        }
        let categories =
            review_categories::effective(&review.catalog.categories, &self.category_edits);
        review_categories::validate(&categories)?;
        for (id, tag) in &self.tags {
            ensure!(!id.is_empty(), "A sound has no ID in this project");
            ensure!(
                (tag.candidate || tag.category.is_none())
                    && (tag.stand_in || tag.stand_in_category.is_none()),
                "Invalid category assignments for {id}"
            );
            if let Some(replacement) = &tag.replacement {
                ensure!(
                    replacement != id
                        && tag.candidate
                        && tag.category.is_some()
                        && self.tags.get(replacement).is_some_and(
                            |item| item.stand_in && item.stand_in_category == tag.category
                        ),
                    "Invalid replacement choice for {id}"
                );
            }
        }
        Ok(())
    }

    fn write(&self, path: &Path, previous: Option<&str>) -> Result<String> {
        // Keep a lock on the old file while checking the current path, including
        // when independent Audio Review workspaces save the same project.
        let _lock = if let Some(expected) = previous {
            let existing = File::open(path).with_context(|| {
                format!(
                    "The project file at {} could not be checked. Use Save As to keep your edits",
                    path.display()
                )
            })?;
            existing
                .try_lock()
                .context("The saved project is in use. Try saving again")?;
            ensure!(
                digest(&read_bytes(path)?) == expected,
                "The project file changed outside this workspace. Use Save As to keep your changes"
            );
            Some(existing)
        } else {
            None
        };
        let folder = path.parent().context("Choose a project folder")?;
        let mut file = tempfile::NamedTempFile::new_in(folder)
            .with_context(|| format!("Create a project file in {}", folder.display()))?;
        let written = self.write_content(&mut file)?;
        if previous.is_some() {
            file.persist(path)
                .with_context(|| format!("Save project to {}", path.display()))?;
        } else {
            file.persist_noclobber(path).with_context(|| {
                format!(
                    "Save a new project to {}. Choose a new name if this file already exists",
                    path.display()
                )
            })?;
        }
        Ok(written)
    }

    fn write_content(&self, file: &mut tempfile::NamedTempFile) -> Result<String> {
        let mut bytes = serde_json::to_vec_pretty(self)?;
        bytes.push(b'\n');
        ensure!(
            bytes.len() <= MAX_FILE_BYTES,
            "The project exceeds the 128 MiB project file limit"
        );
        file.write_all(&bytes)?;
        file.as_file().sync_all()?;
        Ok(digest(&bytes))
    }

    fn recovery_file(&self, directory: &Path) -> Result<(PathBuf, String)> {
        fs::create_dir_all(directory).with_context(|| format!("Create {}", directory.display()))?;
        let mut file = tempfile::Builder::new()
            .prefix(".autosave-")
            .tempfile_in(directory)?;
        let salt = file
            .path()
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_prefix(".autosave-"))
            .context("Name the recovery file")?;
        let path = directory.join(format!("Untitled-{salt}.misophonia.json"));
        let digest = self.write_content(&mut file)?;
        file.persist_noclobber(&path)?;
        Ok((path, digest))
    }
}

fn read_bytes(path: &Path) -> Result<Vec<u8>> {
    let metadata = fs::metadata(path).with_context(|| format!("Open {}", path.display()))?;
    ensure!(metadata.is_file(), "Choose a project file");
    let file = File::open(path).with_context(|| format!("Open {}", path.display()))?;
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= MAX_FILE_BYTES,
        "Project file exceeds 128 MiB"
    );
    Ok(bytes)
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn path_text(path: &Path) -> Result<&str> {
    path.to_str()
        .context("Choose a project path with a UTF-8 filename")
}

fn save_target(path: &Path) -> Result<PathBuf> {
    let name = path.file_name().context("Enter a project filename")?;
    let mut name = name
        .to_str()
        .context("Enter a UTF-8 project filename")?
        .to_owned();
    if !path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
    {
        name.push_str(".misophonia.json");
    }
    let directory = path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let directory = directory
        .canonicalize()
        .with_context(|| format!("Open {}", directory.display()))?;
    Ok(directory.join(name))
}

fn checkpoint(
    db: &Connection,
    state: &ProjectState,
    directory: &Path,
    version: &str,
) -> Result<Option<PathBuf>> {
    if !state.dirty {
        return Ok(None);
    }
    let project = ProjectFile::from_db(db, state, version)?;
    let (target, new_digest) =
        if let Some(path) = state.path.as_ref().or(state.recovery_path.as_ref()) {
            let expected = state
                .digest
                .as_deref()
                .context("The project has no saved fingerprint. Use Save As")?;
            (path.clone(), project.write(path, Some(expected))?)
        } else {
            project.recovery_file(directory)?
        };
    let recovery = state
        .path
        .is_none()
        .then(|| path_text(&target))
        .transpose()?;
    db.execute(
        "UPDATE review_project
         SET dirty = 0, digest = ?, recovery_path = ?, catalog_version = ? WHERE singleton = 1",
        params![new_digest, recovery, project.catalog_version],
    )?;
    Ok(Some(target))
}

impl Review {
    pub fn project_path(&self) -> Option<&Path> {
        self.project.path.as_deref()
    }

    pub fn project_catalog_notice(&self) -> Option<String> {
        let version = self.project.catalog_version.as_ref()?;
        (version != &self.catalog.version).then(|| {
            format!(
                "Project catalog {version}. Builds use the loaded Misophonia {} catalog.",
                self.catalog.version
            )
        })
    }

    pub fn autosave_minutes(&self) -> u32 {
        self.project.minutes
    }

    pub fn autosave_directory(&self) -> PathBuf {
        self.data_dir.join("autosaves")
    }

    pub fn set_autosave_minutes(&mut self, minutes: u32) -> Result<()> {
        ensure!(minutes > 0, "Enter a positive number of minutes");
        let db = self
            .store
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        ProjectState::current(&db, self.project.epoch)?;
        db.execute(
            "UPDATE review_project SET minutes = ? WHERE singleton = 1",
            [minutes],
        )?;
        db.commit()?;
        self.project.minutes = minutes;
        Ok(())
    }

    pub fn autosave(&mut self) -> Result<Option<PathBuf>> {
        let directory = self.autosave_directory();
        let db = self
            .store
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let state = ProjectState::current(&db, self.project.epoch)?;
        let saved = checkpoint(&db, &state, &directory, &self.catalog.version)?;
        let updated = ProjectState::read(&db)?;
        db.commit()?;
        self.project = updated;
        Ok(saved)
    }

    pub fn save_project(&mut self, path: Option<&Path>) -> Result<PathBuf> {
        let db = self
            .store
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let state = ProjectState::current(&db, self.project.epoch)?;
        let path = path
            .map(save_target)
            .transpose()?
            .or_else(|| state.path.clone())
            .context("Choose where to save this project")?;
        let same = state.path.as_ref() == Some(&path);
        let previous = if same {
            Some(
                state
                    .digest
                    .as_deref()
                    .context("This project has no saved fingerprint. Use Save As")?,
            )
        } else {
            None
        };
        let text = path_text(&path)?;
        let project = ProjectFile::from_db(&db, &state, &self.catalog.version)?;
        let saved_digest = project.write(&path, previous)?;
        db.execute(
            "UPDATE review_project
             SET epoch = epoch + ?, path = ?, recovery_path = NULL,
                 catalog_version = ?, digest = ?, dirty = 0 WHERE singleton = 1",
            params![!same, text, project.catalog_version, saved_digest],
        )?;
        let updated = ProjectState::read(&db)?;
        db.commit()?;
        self.project = updated;
        Ok(path)
    }

    pub fn new_project(&mut self) -> Result<()> {
        self.switch_project(ProjectFile::empty(&self.catalog.version), None, None)
    }

    pub fn open_project(&mut self, path: &Path) -> Result<()> {
        let path = path
            .canonicalize()
            .with_context(|| format!("Open {}", path.display()))?;
        let (project, saved_digest) = ProjectFile::read(&path)?;
        project.validate(self)?;
        let recovery = self
            .autosave_directory()
            .canonicalize()
            .is_ok_and(|directory| path.starts_with(directory));
        if recovery {
            self.switch_project(project, None, None)
        } else {
            self.switch_project(project, Some(path), Some(saved_digest))
        }
    }

    fn switch_project(
        &mut self,
        project: ProjectFile,
        path: Option<PathBuf>,
        saved_digest: Option<String>,
    ) -> Result<()> {
        let directory = self.autosave_directory();
        let db = self
            .store
            .0
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let state = ProjectState::current(&db, self.project.epoch)?;
        if state.dirty && state.path.is_some() && state.path == path {
            checkpoint(&db, &state, &directory, &self.catalog.version)?;
            db.commit()?;
            self.refresh_saved_review()?;
            return Ok(());
        }
        checkpoint(&db, &state, &directory, &self.catalog.version)?;
        db.execute_batch("DELETE FROM tags; DELETE FROM category_edits;")?;
        for (id, tag) in &project.tags {
            Store::write_tags(&db, id, tag)?;
        }
        for (id, edit) in &project.category_edits {
            Store::write_category(&db, id, edit)?;
        }
        db.execute(
            "UPDATE review_project
             SET epoch = epoch + 1, path = ?, recovery_path = NULL,
                 catalog_version = ?, digest = ?, dirty = 0 WHERE singleton = 1",
            params![
                path.as_deref().map(path_text).transpose()?,
                project.catalog_version,
                saved_digest
            ],
        )?;
        db.commit()?;
        self.project.epoch = state.epoch + 1;
        self.refresh_saved_review()?;
        Ok(())
    }
}
