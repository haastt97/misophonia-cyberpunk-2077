use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

use crate::build_plan::{BuildPlan, CategoryPlan};

mod runtime;

const SCRIPTS: &str = "r6/scripts/Misophonia";
const AUDIO: &str = "red4ext/plugins/AudioXL/sounds/Misophonia/Generated";

pub fn build(
    catalog_root: &Path,
    output: &Path,
    version: &str,
    plan: &BuildPlan,
    donor_audio: &BTreeMap<String, PathBuf>,
) -> Result<Vec<String>> {
    ensure!(
        !version.is_empty()
            && version.trim() == version
            && !version
                .chars()
                .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c)),
        "Choose a version name that is safe in a filename"
    );
    let source = |file: &str| {
        let path = catalog_root.join(SCRIPTS).join(file);
        fs::read_to_string(&path).with_context(|| format!("Read script {}", path.display()))
    };
    let base_screens_file = catalog_root.join("data/screen-events.txt");
    let base_screens = match fs::read_to_string(&base_screens_file) {
        Ok(contents) => contents
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_owned)
            .collect::<BTreeSet<_>>(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => BTreeSet::new(),
        Err(error) => {
            return Err(error).with_context(|| format!("Read {}", base_screens_file.display()));
        }
    };

    let settings = Settings::new(plan, &base_screens)?;
    let paths = donor_paths(plan, donor_audio)?;
    let config = settings.render_config()?;
    let catalog = settings.render_catalog(&source("Catalog.reds")?, &paths)?;
    let live = runtime::render(&source("LiveSettings.reds")?)?;
    let menu = source("MenuRefresh.reds")?;

    let temporary = NamedTempFile::new_in(output).context("Create staged configurable ZIP")?;
    let mut archive = ZipWriter::new(temporary.reopen()?);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for (filename, script) in [
        ("Config.reds", config),
        ("Catalog.reds", catalog),
        ("LiveSettings.reds", live),
        ("MenuRefresh.reds", menu),
    ] {
        archive.start_file(format!("{SCRIPTS}/{filename}"), options)?;
        archive.write_all(script.as_bytes())?;
    }
    for (donor_id, path) in paths {
        archive.start_file(path, options)?;
        std::io::copy(&mut File::open(&donor_audio[&donor_id])?, &mut archive)?;
    }
    archive.finish()?.sync_all()?;
    let path = output.join(format!("Misophonia-{version}-Mod-Settings-PC.zip"));
    temporary
        .persist_noclobber(&path)
        .with_context(|| format!("Save {}", path.display()))?;
    Ok(settings.notices)
}

struct Settings<'a> {
    groups: Vec<Group<'a>>,
    notices: Vec<String>,
    screens_section: Option<&'a str>,
}

struct Group<'a> {
    name: &'a str,
    key: String,
    rows: Vec<Row<'a>>,
}

struct Row<'a> {
    key: String,
    slider: bool,
    category: &'a CategoryPlan,
}

impl<'a> Settings<'a> {
    fn new(plan: &'a BuildPlan, base_screens: &BTreeSet<String>) -> Result<Self> {
        let mut groups = vec![Group {
            name: "General",
            key: generated_key("misophoniaSection", "General"),
            rows: Vec::new(),
        }];
        let mut notices = Vec::new();
        let mut screens_section = None;
        let mut has_screens = false;
        let mut fields = BTreeSet::from(["muteAll".to_owned()]);
        let mut events = BTreeSet::new();

        for category in &plan.categories {
            ensure!(
                !category.name.trim().is_empty() && !category.section.trim().is_empty(),
                "Every setting needs a category and sub-category name"
            );
            ensure!(
                !category
                    .name
                    .chars()
                    .chain(category.section.chars())
                    .any(char::is_control),
                "Setting names cannot contain control characters"
            );
            ensure!(
                !category.name.eq_ignore_ascii_case("All"),
                "The sub-category name All is reserved for the section switch"
            );
            for event in category.events.keys() {
                ensure!(valid_event(event), "Invalid sound event name {event:?}");
                ensure!(
                    events.insert(event),
                    "Sound event {event} is assigned to more than one setting"
                );
            }

            let slider = if category.screens {
                ensure!(
                    !has_screens,
                    "Only one Screens setting can use the native mixer"
                );
                has_screens = true;
                let exact = !base_screens.is_empty()
                    && category.events.len() == base_screens.len()
                    && category
                        .events
                        .iter()
                        .all(|(name, donor)| donor.is_none() && base_screens.contains(name));
                if exact {
                    screens_section = Some(category.section.as_str());
                } else {
                    notices.push(format!(
                        "{} uses an on/off switch in this version. The native Screens slider controls the whole TV and adverts mixer, so individual changes use event controls.",
                        category.name
                    ));
                }
                exact
            } else {
                false
            };
            let key = if slider {
                "screenPercent".to_owned()
            } else if category.screens {
                generated_key("misophoniaScreens", &category.id)
            } else if valid_identifier(&category.id) {
                ensure!(
                    category.id != "screenPercent",
                    "screenPercent is reserved for Screens"
                );
                category.id.clone()
            } else {
                generated_key("misophoniaSetting", &category.id)
            };
            ensure!(
                fields.insert(key.clone()),
                "Duplicate setting ID {}",
                category.id
            );

            let index = groups
                .iter()
                .position(|group| group.name == category.section);
            let group = match index {
                Some(index) => &mut groups[index],
                None => {
                    groups.push(Group {
                        name: &category.section,
                        key: generated_key("misophoniaSection", &category.section),
                        rows: Vec::new(),
                    });
                    groups.last_mut().expect("Group just added")
                }
            };
            ensure!(
                !group
                    .rows
                    .iter()
                    .any(|row| row.category.name.eq_ignore_ascii_case(&category.name)),
                "Repeated sub-category {} in {}",
                category.name,
                group.name
            );
            group.rows.push(Row {
                key,
                slider,
                category,
            });
        }
        for group in &groups {
            ensure!(
                fields.insert(group.key.clone()),
                "Conflicting section ID for {}",
                group.name
            );
        }
        Ok(Self {
            groups,
            notices,
            screens_section,
        })
    }

    fn render_config(&self) -> Result<String> {
        let mut text = String::from("module Misophonia\n\npublic class Config {\n");
        let mut order = 1;
        for (section_index, group) in self.groups.iter().enumerate() {
            let section_order = section_index + 1;
            if section_index == 0 {
                let description = if self.screens_section.is_some() {
                    "Switch all sound controls and set the Screens volume to 0% or 100%."
                } else {
                    "Switch all sound controls on or off."
                };
                setting(
                    &mut text,
                    group.name,
                    section_order,
                    &mut order,
                    "All",
                    "muteAll",
                    description,
                    false,
                )?;
            }
            if group.has_switch() {
                let label = if section_index == 0 {
                    "All in General"
                } else {
                    "All"
                };
                setting(
                    &mut text,
                    group.name,
                    section_order,
                    &mut order,
                    label,
                    &group.key,
                    "Switch every sound control in this category.",
                    false,
                )?;
            }
            for row in &group.rows {
                let description = if row.slider {
                    "TV and advert volume. 100% uses stock gain. 0% silences both."
                } else if row.category.events.is_empty() {
                    "No sound events are assigned to this control."
                } else {
                    "Apply the selected mutes and replacements for these sounds."
                };
                setting(
                    &mut text,
                    group.name,
                    section_order,
                    &mut order,
                    &row.category.name,
                    &row.key,
                    description,
                    row.slider,
                )?;
            }
        }
        text.push_str("}\n");
        Ok(text)
    }

    fn render_catalog(
        &self,
        source: &str,
        donor_paths: &BTreeMap<String, String>,
    ) -> Result<String> {
        let screens_gain = source
            .lines()
            .find(|line| line.starts_with("  public static func ScreenGain("))
            .and_then(|line| source.split_once(line).map(|(_, rest)| (line, rest)))
            .context("The selected Catalog.reds has no Screens gain curve")?;

        let mut text = String::from(
            "module Misophonia\n\npublic struct SoundCategory {\n  public let section: CName;\n  public let setting: CName;\n  public let events: array<CName>;\n  public let replacements: array<String>;\n}\n\npublic abstract class Catalog {\n  public static func Create() -> array<SoundCategory> {\n    let result: array<SoundCategory>;\n",
        );
        if self.groups.iter().any(Group::has_switch) {
            text.push_str("    let entry: SoundCategory;\n    let noEvents: array<CName>;\n    let noReplacements: array<String>;\n");
        }
        for group in &self.groups {
            for row in group.rows.iter().filter(|row| !row.slider) {
                writeln!(text, "    entry.section = n\"{}\";", group.key)?;
                writeln!(text, "    entry.setting = n\"{}\";", row.key)?;
                if row.category.events.is_empty() {
                    text.push_str(
                        "    entry.events = noEvents;\n    entry.replacements = noReplacements;\n",
                    );
                } else {
                    let names = row
                        .category
                        .events
                        .keys()
                        .map(|event| format!("n{}", literal(event)))
                        .collect::<Vec<_>>()
                        .join(", ");
                    let replacements = row
                        .category
                        .events
                        .values()
                        .map(|id| id.as_ref().map(|id| donor_paths[id].as_str()).unwrap_or(""))
                        .map(literal)
                        .collect::<Vec<_>>()
                        .join(", ");
                    writeln!(text, "    entry.events = [{names}];")?;
                    writeln!(text, "    entry.replacements = [{replacements}];")?;
                }
                text.push_str("    ArrayPush(result, entry);\n");
            }
        }
        text.push_str("    return result;\n  }\n\n");
        writeln!(
            text,
            "  public static func HasScreens() -> Bool {{ return {}; }}",
            self.screens_section.is_some()
        )?;
        writeln!(
            text,
            "  public static func ScreensSection() -> CName {{ return n{}; }}",
            literal(self.screens_section.unwrap_or("General"))
        )?;
        writeln!(
            text,
            "  public static func SectionCount() -> Int32 {{ return {}; }}",
            self.groups
                .iter()
                .filter(|group| group.has_switch())
                .count()
        )?;
        text.push('\n');
        text.push_str(screens_gain.0);
        text.push_str(screens_gain.1);
        Ok(text)
    }
}

impl Group<'_> {
    fn has_switch(&self) -> bool {
        self.rows.iter().any(|row| !row.slider)
    }
}

#[allow(clippy::too_many_arguments)]
fn setting(
    text: &mut String,
    section: &str,
    section_order: usize,
    order: &mut usize,
    label: &str,
    key: &str,
    description: &str,
    slider: bool,
) -> Result<()> {
    writeln!(
        text,
        "  @runtimeProperty(\"ModSettings.mod\", \"Misophonia\")"
    )?;
    writeln!(
        text,
        "  @runtimeProperty(\"ModSettings.category\", {})",
        literal(section)
    )?;
    writeln!(
        text,
        "  @runtimeProperty(\"ModSettings.category.order\", \"{section_order}\")"
    )?;
    writeln!(
        text,
        "  @runtimeProperty(\"ModSettings.order\", \"{order}\")"
    )?;
    writeln!(
        text,
        "  @runtimeProperty(\"ModSettings.displayName\", {})",
        literal(label)
    )?;
    writeln!(
        text,
        "  @runtimeProperty(\"ModSettings.description\", {})",
        literal(description)
    )?;
    if slider {
        text.push_str("  @runtimeProperty(\"ModSettings.min\", \"0\")\n  @runtimeProperty(\"ModSettings.max\", \"100\")\n  @runtimeProperty(\"ModSettings.step\", \"1\")\n");
        writeln!(text, "  public let {key}: Int32 = 100;\n")?;
    } else {
        writeln!(text, "  public let {key}: Bool = false;\n")?;
    }
    *order += 1;
    Ok(())
}

fn valid_identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn valid_event(value: &str) -> bool {
    !value.is_empty() && !value.chars().any(char::is_control)
}

fn generated_key(prefix: &str, value: &str) -> String {
    let digest = format!("{:x}", Sha256::digest(value.as_bytes()));
    format!("{prefix}_{}", &digest[..16])
}

fn literal(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('\"', "\\\""))
}

fn donor_paths(
    plan: &BuildPlan,
    audio: &BTreeMap<String, PathBuf>,
) -> Result<BTreeMap<String, String>> {
    let mut paths = BTreeMap::new();
    for id in plan
        .categories
        .iter()
        .flat_map(|category| category.events.values().flatten())
    {
        if paths.contains_key(id) {
            continue;
        }
        let path = audio
            .get(id)
            .with_context(|| format!("No decoded WAV is available for stand-in {id}"))?;
        let mut file =
            File::open(path).with_context(|| format!("Open stand-in {}", path.display()))?;
        let mut header = [0; 12];
        file.read_exact(&mut header)
            .with_context(|| format!("Read stand-in {}", path.display()))?;
        ensure!(
            &header[..4] == b"RIFF" && &header[8..] == b"WAVE" && file.metadata()?.len() > 44,
            "Stand-in {} is not a nonempty WAV file",
            path.display()
        );
        paths.insert(
            id.clone(),
            format!("{AUDIO}/{}.wav", generated_key("standin", id)),
        );
    }
    Ok(paths)
}

#[cfg(test)]
mod tests;
