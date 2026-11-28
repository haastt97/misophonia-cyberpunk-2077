use std::{
    collections::{BTreeMap, BTreeSet},
    env, fmt,
    fs::{self, File},
    io::{self, BufReader, BufWriter, Write},
    path::Path,
    process::Command,
    sync::atomic::AtomicBool,
};

use anyhow::{Context, Result, ensure};
use serde_json::Value;
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

use crate::{
    build_plan::BuildPlan,
    game::tools::{check_cancelled, run_tool},
};

const VANILLA_ARCHIVE: &str = "archive/pc/content/audio_1_general.archive";
const METADATA: &str = "base/sound/event/eventsmetadata.json";
const EVENTS: &str = "/Data/RootChunk/root/Data/events";
type Actions = BTreeMap<String, Option<String>>;

pub fn build(
    game_root: &Path,
    wolvenkit: &Path,
    output: &Path,
    version: &str,
    plan: &BuildPlan,
    cancelled: &AtomicBool,
) -> Result<Vec<String>> {
    let options = options(plan)?;
    let mut skipped = Vec::new();
    let mut eligible = Vec::new();
    for (option, actions) in options {
        match resolve_donors(plan, actions) {
            Ok(actions) => eligible.push((option, actions)),
            Err(reason) => skipped.push(format!("{option}: {reason}")),
        }
    }
    if eligible.is_empty() {
        return Ok(skipped);
    }

    check_cancelled(cancelled)?;
    let work = tempfile::tempdir_in(output)?;
    let builder = ArchiveBuilder::new(game_root, wolvenkit, work.path(), cancelled)?;
    let raw = output.join("Standalone");
    let packages = output.join("Nexus-Optional");

    let version = slug(version, "Custom");
    for (option, actions) in eligible {
        let name = format!("Misophonia-{version}-{option}");
        let archive = raw.join(format!("{name}.archive"));
        match builder.write_archive(&archive, &actions) {
            Ok(()) => {
                fs::create_dir_all(&packages)?;
                write_zip(&packages.join(format!("{name}.zip")), &archive)?;
            }
            Err(error) => {
                if let Some(reason) = error.downcast_ref::<Unsupported>() {
                    skipped.push(format!("{option}: {reason}"));
                } else {
                    return Err(error.context(format!("Build standalone option {option}")));
                }
            }
        }
    }
    Ok(skipped)
}

fn resolve_donors(plan: &BuildPlan, actions: Actions) -> Result<Actions> {
    actions
        .into_iter()
        .map(|(event, donor)| {
            let Some(donor) = donor else {
                return Ok((event, None));
            };
            let stand_in = plan
                .stand_ins
                .get(&donor)
                .with_context(|| format!("No stand-in data for {donor}, selected by {event}"))?;
            let donor_event = stand_in.event.clone().with_context(|| {
                format!(
                    "Stand-in {} for {event} has no exact vanilla event for the standalone archive",
                    stand_in.name
                )
            })?;
            Ok((event, Some(donor_event)))
        })
        .collect()
}

fn options(plan: &BuildPlan) -> Result<Vec<(String, Actions)>> {
    let mut all = Actions::new();
    let mut screens = Actions::new();
    let mut sections = BTreeMap::<&str, Actions>::new();
    for category in &plan.categories {
        if category.events.is_empty() {
            continue;
        }

        let variant = if category.screens {
            &mut screens
        } else {
            sections.entry(&category.section).or_default()
        };
        for (event, replacement) in &category.events {
            if let Some(previous) = all.get(event) {
                ensure!(
                    previous == replacement,
                    "{event} has conflicting sound changes across categories. Assign one action per event"
                );
            }
            all.insert(event.clone(), replacement.clone());
            variant.insert(event.clone(), replacement.clone());
        }
    }
    if all.is_empty() {
        return Ok(Vec::new());
    }

    let mut options = vec![("All-Covered-Sounds".to_owned(), all)];
    if !screens.is_empty() {
        options.push(("Screens".to_owned(), screens));
    }
    let mut used = BTreeSet::from(["all-covered-sounds".to_owned(), "screens".to_owned()]);
    for (section, actions) in sections {
        let base = slug(section, "Section");
        let mut name = base.clone();
        let mut suffix = 1;
        while !used.insert(name.to_ascii_lowercase()) {
            suffix += 1;
            name = format!("{base}-{suffix}");
        }
        options.push((name, actions));
    }
    Ok(options)
}

fn slug(text: &str, fallback: &str) -> String {
    let mut slug = String::new();
    let mut separated = false;
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            if separated && !slug.is_empty() {
                slug.push('-');
            }
            slug.push(ch);
            separated = false;
        } else {
            separated = true;
        }
        if slug.len() >= 63 {
            break;
        }
    }
    if slug.is_empty() {
        fallback.to_owned()
    } else {
        slug
    }
}

struct ArchiveBuilder<'a> {
    wolvenkit: &'a Path,
    work: &'a Path,
    cancelled: &'a AtomicBool,
    metadata: Value,
}

impl<'a> ArchiveBuilder<'a> {
    fn new(
        game_root: &Path,
        wolvenkit: &'a Path,
        work: &'a Path,
        cancelled: &'a AtomicBool,
    ) -> Result<Self> {
        let source = game_root.join(VANILLA_ARCHIVE);
        ensure!(
            source.is_file(),
            "Vanilla audio archive is missing at {}",
            source.display()
        );
        let vanilla = work.join("vanilla");
        fs::create_dir(&vanilla)?;
        let mut command = wolvenkit_command(wolvenkit, "unbundle");
        command
            .arg(source)
            .arg("-o")
            .arg(&vanilla)
            .args(["-w", "*eventsmetadata.json"]);
        run_wolvenkit(&mut command, work, cancelled)?;
        let metadata_path = vanilla.join(METADATA);
        ensure!(
            metadata_path.is_file(),
            "WolvenKit did not extract {METADATA} from {VANILLA_ARCHIVE}"
        );
        let mut command = wolvenkit_command(wolvenkit, "cr2w");
        command.arg(&metadata_path).arg("-s");
        run_wolvenkit(&mut command, work, cancelled)?;
        let metadata = read_json(&metadata_path.with_extension("json.json"))?;
        Ok(Self {
            wolvenkit,
            work,
            cancelled,
            metadata,
        })
    }

    fn write_archive(&self, destination: &Path, actions: &Actions) -> Result<()> {
        check_cancelled(self.cancelled)?;
        ensure!(
            !actions.is_empty(),
            "No audio changes to write to a standalone archive"
        );
        ensure!(
            !destination.exists(),
            "Output already exists at {}",
            destination.display()
        );
        let name = destination
            .file_stem()
            .context("Archive needs a filename")?;
        let mut expected = self.metadata.clone();
        apply_actions(&mut expected, actions)?;

        let job = tempfile::tempdir_in(self.work)?;
        let input = job.path().join(name);
        let expected_path = input.join(METADATA);
        let target_dir = expected_path
            .parent()
            .context("Missing archive resource directory")?;
        fs::create_dir_all(target_dir)?;
        let source = job.path().join("eventsmetadata.json.json");
        let mut writer = BufWriter::new(File::create(&source)?);
        serde_json::to_writer(&mut writer, &expected)?;
        writer.flush()?;
        drop(writer);

        let mut command = wolvenkit_command(self.wolvenkit, "cr2w");
        command.arg(&source).arg("-d").arg("-o").arg(target_dir);
        run_wolvenkit(&mut command, job.path(), self.cancelled)?;
        ensure!(
            expected_path.is_file(),
            "WolvenKit did not convert the edited event metadata"
        );
        let mut command = wolvenkit_command(self.wolvenkit, "pack");
        command.arg(&input).arg("-o").arg(job.path());
        run_wolvenkit(&mut command, job.path(), self.cancelled)?;
        let archive = job.path().join(name).with_extension("archive");
        ensure!(
            archive.is_file(),
            "WolvenKit did not pack {}",
            archive.display()
        );

        let extracted = job.path().join("extracted");
        fs::create_dir(&extracted)?;
        let mut command = wolvenkit_command(self.wolvenkit, "unbundle");
        command.arg(&archive).arg("-o").arg(&extracted);
        run_wolvenkit(&mut command, job.path(), self.cancelled)?;
        let rebuilt = extracted.join(METADATA);
        for directory in [
            &extracted,
            &extracted.join("base"),
            &extracted.join("base/sound"),
            &extracted.join("base/sound/event"),
        ] {
            ensure!(
                fs::read_dir(directory)?
                    .collect::<io::Result<Vec<_>>>()?
                    .len()
                    == 1,
                "Generated standalone archive should contain only {METADATA}"
            );
        }
        ensure!(
            rebuilt.is_file(),
            "Generated standalone archive is missing {METADATA}"
        );
        let mut command = wolvenkit_command(self.wolvenkit, "cr2w");
        command.arg(&rebuilt).arg("-s");
        run_wolvenkit(&mut command, job.path(), self.cancelled)?;
        let round_trip = read_json(&rebuilt.with_extension("json.json"))?;
        ensure!(
            round_trip.get("Data") == expected.get("Data"),
            "Standalone archive round-trip did not preserve the edited metadata"
        );

        check_cancelled(self.cancelled)?;
        fs::create_dir_all(
            destination
                .parent()
                .context("Missing archive output directory")?,
        )?;
        fs::copy(&archive, destination)
            .with_context(|| format!("Write {}", destination.display()))?;
        Ok(())
    }
}

#[derive(Debug)]
struct Unsupported(String);

impl fmt::Display for Unsupported {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        output.write_str(&self.0)
    }
}

impl std::error::Error for Unsupported {}

fn apply_actions(metadata: &mut Value, actions: &Actions) -> Result<()> {
    let records = metadata
        .pointer_mut(EVENTS)
        .and_then(Value::as_array_mut)
        .context("WolvenKit metadata does not contain an audio event list")?;
    let mut required: BTreeMap<&str, BTreeSet<u32>> = actions
        .iter()
        .flat_map(|(event, donor)| [Some(event.as_str()), donor.as_deref()])
        .flatten()
        .map(|event| (event, BTreeSet::new()))
        .collect();
    for record in &*records {
        let event = event_name(record)?;
        if let Some(ids) = required.get_mut(event) {
            let id = record
                .get("wwiseId")
                .and_then(Value::as_u64)
                .and_then(|id| u32::try_from(id).ok())
                .filter(|id| *id > 0)
                .ok_or_else(|| {
                    Unsupported(format!(
                        "{event} has no original Wwise event ID in the vanilla archive"
                    ))
                })?;
            ids.insert(id);
        }
    }
    let missing: Vec<_> = required
        .iter()
        .filter(|(_, ids)| ids.is_empty())
        .map(|(event, _)| *event)
        .collect();
    if !missing.is_empty() {
        let mut sample = missing
            .iter()
            .take(5)
            .copied()
            .collect::<Vec<_>>()
            .join(", ");
        if missing.len() > 5 {
            sample.push_str(&format!(", and {} more", missing.len() - 5));
        }
        return Err(
            Unsupported(format!("The vanilla archive has no metadata for {sample}")).into(),
        );
    }

    let replacements: BTreeMap<&str, u32> = actions
        .iter()
        .map(|(event, donor)| -> Result<_> {
            let Some(donor) = donor else {
                return Ok((event.as_str(), 0));
            };
            if event == donor {
                return Err(
                    Unsupported(format!("{event} cannot be its own stand-in event")).into(),
                );
            }
            let ids = &required[donor.as_str()];
            if ids.len() != 1 {
                return Err(Unsupported(format!(
                    "Stand-in event {donor} has multiple original Wwise IDs"
                ))
                .into());
            }
            Ok((
                event.as_str(),
                *ids.first().context("Missing stand-in Wwise ID")?,
            ))
        })
        .collect::<Result<_>>()?;

    for record in records {
        if let Some(&wwise_id) = replacements.get(event_name(record)?) {
            record["wwiseId"] = Value::from(wwise_id);
        }
    }
    Ok(())
}

fn event_name(record: &Value) -> Result<&str> {
    record
        .pointer("/redId/$value")
        .and_then(Value::as_str)
        .context("WolvenKit metadata contains an unnamed audio event")
}

fn wolvenkit_command(executable: &Path, action: &str) -> Command {
    let mut command = Command::new(executable);
    command.arg(action);
    if env::var_os("DOTNET_ROOT").is_none()
        && let Some(root) = executable.parent().and_then(Path::parent)
        && root.join("dotnet/shared").is_dir()
    {
        command.env("DOTNET_ROOT", root.join("dotnet"));
    }
    command
}

fn run_wolvenkit(command: &mut Command, work: &Path, cancelled: &AtomicBool) -> Result<()> {
    let action = command
        .get_args()
        .next()
        .context("Missing WolvenKit command")?
        .to_string_lossy()
        .into_owned();
    let result = run_tool(command, work, cancelled)?;
    ensure!(
        result.status.success()
            && [&result.stdout, &result.stderr]
                .iter()
                .all(|log| !log.contains("[ERROR]") && !log.contains("Exception")),
        "WolvenKit {action} failed: {}",
        result.diagnostic()
    );
    Ok(())
}

fn read_json(path: &Path) -> Result<Value> {
    let file = File::open(path).with_context(|| format!("Open {}", path.display()))?;
    serde_json::from_reader(BufReader::new(file))
        .with_context(|| format!("Read {}", path.display()))
}

fn write_zip(destination: &Path, archive: &Path) -> Result<()> {
    let filename = archive
        .file_name()
        .context("Archive has no filename")?
        .to_string_lossy();
    let entry = format!("archive/pc/mod/{filename}");
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let mut writer = ZipWriter::new(File::create_new(destination)?);
    writer.start_file(&entry, options)?;
    let bytes = io::copy(&mut File::open(archive)?, &mut writer)?;
    writer.finish()?;

    let mut zip = ZipArchive::new(File::open(destination)?)?;
    ensure!(
        zip.len() == 1,
        "Standalone ZIP must contain a single archive"
    );
    let size = io::copy(&mut zip.by_name(&entry)?, &mut io::sink())?;
    ensure!(bytes == size, "Standalone ZIP integrity check failed");
    Ok(())
}

#[cfg(test)]
mod tests;
