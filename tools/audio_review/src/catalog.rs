use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs::{self, File},
    io::{Read, Write},
    path::Path,
    time::Duration,
};

use anyhow::{Context, Result, ensure};
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::game::Asset;

const EVENTS_URL: &str = "https://sounddb.zhincore.eu/v1/data/events.json";
const MAX_EVENT_BYTES: u64 = 32 * 1024 * 1024;

#[derive(Clone, Debug, Serialize)]
pub struct Category {
    pub id: String,
    pub section: String,
    pub name: String,
    pub mode: &'static str,
}

#[derive(Debug, Serialize)]
pub struct Coverage {
    pub category: String,
    pub mode: &'static str,
    pub edition: &'static str,
    pub events: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct Sound {
    #[serde(flatten)]
    pub asset: Asset,
    pub events: Vec<String>,
    pub coverage: Vec<Coverage>,
    pub playable: bool,
}

pub struct Catalog {
    pub version: String,
    pub categories: Vec<Category>,
    assignments: HashMap<String, Vec<usize>>,
}

impl Catalog {
    pub fn read(root: &Path) -> Result<Self> {
        let read = |name| {
            let path = root.join(name);
            fs::read_to_string(&path).with_context(|| format!("Read {}", path.display()))
        };
        let version = read("VERSION")?.trim().to_owned();
        let config = read("r6/scripts/Misophonia/Config.reds")?;
        let catalog = read("r6/scripts/Misophonia/Catalog.reds")?;

        let setting = Regex::new(
            r"(?m)((?:^[ \t]*@runtimeProperty\([^\n]+\)\r?\n)+)[ \t]*public let (\w+): (?:Bool|Int32) = [^;]+;",
        )?;
        let property =
            Regex::new(r#"@runtimeProperty\("ModSettings\.(category|displayName)", "([^"]+)"\)"#)?;
        let entries = Regex::new(
            r#"entry.section = n"(\w+)";\s+entry.setting = n"(\w+)";\s+entry.events = \[([^]]+)\];"#,
        )?;
        let event = Regex::new(r#"n"([^"]+)""#)?;
        let settings: HashMap<_, HashMap<_, _>> = setting
            .captures_iter(&config)
            .map(|capture| {
                let [properties, key] = capture.extract().1;
                let fields = property
                    .captures_iter(properties)
                    .map(|field| {
                        let [name, value] = field.extract().1;
                        (name, value)
                    })
                    .collect();
                (key, fields)
            })
            .collect();
        let field = |key: &str, name: &str| {
            settings
                .get(key)
                .and_then(|fields| fields.get(name))
                .copied()
                .with_context(|| format!("Missing {name} for {key} in Config.reds"))
        };
        let category = |key: &str, mode| -> Result<Category> {
            Ok(Category {
                id: key.to_owned(),
                section: field(key, "category")?.to_owned(),
                name: field(key, "displayName")?.to_owned(),
                mode,
            })
        };

        let mut categories = vec![category("screenPercent", "volume")?];
        let mut assignments: HashMap<String, Vec<usize>> = HashMap::new();
        for name in read("data/screen-events.txt")?
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
        {
            assignments.entry(name.to_owned()).or_default().push(0);
        }
        for entry in entries.captures_iter(&catalog) {
            let [parent, key, events] = entry.extract().1;
            let section = category(key, "mute")?;
            ensure!(
                section.section == field(parent, "category")?,
                "Setting {key} is assigned to the wrong category"
            );
            ensure!(
                !categories.iter().any(|existing| existing.id == key),
                "Repeated category {key}"
            );
            for capture in event.captures_iter(events) {
                assignments
                    .entry(capture[1].to_owned())
                    .or_default()
                    .push(categories.len());
            }
            categories.push(section);
        }
        ensure!(
            categories.len() > 1
                && categories.len() - 1 == catalog.matches("ArrayPush(result, entry);").count(),
            "Could not read every sound category in Catalog.reds"
        );
        Ok(Self {
            version,
            categories,
            assignments,
        })
    }

    pub fn event_assignments(&self) -> impl Iterator<Item = (&str, &str)> {
        self.assignments.iter().flat_map(move |(event, indices)| {
            indices
                .iter()
                .map(move |&index| (self.categories[index].id.as_str(), event.as_str()))
        })
    }

    fn coverage(&self, events: &[String]) -> Vec<Coverage> {
        let mut grouped: BTreeMap<_, Vec<String>> = BTreeMap::new();
        for event in events {
            for index in self.assignments.get(event).into_iter().flatten() {
                let category = &self.categories[*index];
                grouped
                    .entry((&category.id, category.mode, "Configurable"))
                    .or_default()
                    .push(event.clone());
                grouped
                    .entry((&category.id, "mute", "Standalone"))
                    .or_default()
                    .push(event.clone());
            }
        }
        grouped
            .into_iter()
            .map(|((category, mode, edition), events)| Coverage {
                category: category.clone(),
                mode,
                edition,
                events,
            })
            .collect()
    }

    pub fn sounds(&self, assets: Vec<Asset>, media: &MediaEvents) -> Vec<Sound> {
        let mut linked = BTreeSet::new();
        let mut sounds: Vec<_> = assets
            .into_iter()
            .map(|mut asset| {
                let events: Vec<_> = asset
                    .media_id
                    .and_then(|id| media.get(&id))
                    .into_iter()
                    .flatten()
                    .cloned()
                    .collect();
                linked.extend(events.iter().cloned());
                if let Some(preferred) = events
                    .iter()
                    .find(|event| self.assignments.contains_key(*event))
                    .or_else(|| events.first())
                {
                    asset.name = format!("{preferred} · {}", asset.name);
                }
                if asset.kind == "sound" && events.iter().any(|event| event.starts_with("mus_")) {
                    asset.kind = "music".to_owned();
                }
                let coverage = self.coverage(&events);
                Sound {
                    asset,
                    events,
                    coverage,
                    playable: true,
                }
            })
            .collect();
        for event in self
            .assignments
            .keys()
            .filter(|event| !linked.contains(*event))
        {
            let hash = format!("{:x}", Sha256::digest(event.as_bytes()));
            let asset = Asset {
                id: format!("event-{}", &hash[..24]),
                kind: "event".to_owned(),
                name: event.clone(),
                detail: "No installed media linked by the sound index".to_owned(),
                archive: None,
                path: None,
                media_id: None,
            };
            let events = vec![event.clone()];
            let coverage = self.coverage(&events);
            sounds.push(Sound {
                asset,
                events,
                coverage,
                playable: false,
            });
        }
        sounds
    }
}

pub type MediaEvents = HashMap<u64, BTreeSet<String>>;

#[derive(Deserialize)]
#[serde(untagged)]
enum MediaId {
    Number(u64),
    Text(String),
}

#[derive(Deserialize, Default)]
struct EventEntry {
    sounds: Option<EventSounds>,
}

#[derive(Deserialize)]
struct EventSounds {
    #[serde(rename = "PLAY", default)]
    play: Vec<MediaId>,
}

fn decode_index(reader: impl Read) -> Result<MediaEvents> {
    let events: HashMap<String, Option<EventEntry>> =
        serde_json::from_reader(reader).context("Read SoundDB events.json")?;
    let mut media = MediaEvents::new();
    for (event, entry) in events {
        if let Some(sounds) = entry.and_then(|entry| entry.sounds) {
            for id in sounds.play {
                let id = match id {
                    MediaId::Number(id) => id,
                    MediaId::Text(id) => id.parse().context("Invalid SoundDB media ID")?,
                };
                media.entry(id).or_default().insert(event.clone());
            }
        }
    }
    ensure!(
        !media.is_empty(),
        "SoundDB index has no playable event-to-media links"
    );
    Ok(media)
}

pub fn event_media(path: Option<&Path>, cache: &Path) -> Result<MediaEvents> {
    let cached = cache.join("events.json");
    if let Some(path) = path.or(cached.is_file().then_some(cached.as_path())) {
        let file = File::open(path).with_context(|| format!("Open {}", path.display()))?;
        return decode_index(std::io::BufReader::new(file));
    }

    eprintln!("Downloading Cyberpunk SoundDB event-to-media metadata.");
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(30)))
        .build()
        .into();
    let mut response = agent
        .get(EVENTS_URL)
        .header("User-Agent", "Misophonia-Audio-Review/1.0")
        .header("Accept", "application/json")
        .call()
        .context("Download SoundDB metadata or supply --event-index")?;
    let mut data = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take(MAX_EVENT_BYTES + 1)
        .read_to_end(&mut data)?;
    ensure!(
        data.len() as u64 <= MAX_EVENT_BYTES,
        "SoundDB event index exceeds 32 MiB"
    );
    let media = decode_index(data.as_slice())?;
    let mut temporary = tempfile::NamedTempFile::new_in(cache)?;
    temporary.write_all(&data)?;
    temporary.persist(cached)?;
    Ok(media)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sounddb_media_links_accept_text_and_numbers() -> Result<()> {
        let source = br#"{"eat":{"sounds":{"PLAY":[42,"42","10"]}},"unlinked":{"sounds":null}}"#;
        let media = decode_index(source.as_slice())?;
        assert_eq!(media.len(), 2);
        assert_eq!(media[&42].iter().collect::<Vec<_>>(), vec!["eat"]);
        Ok(())
    }
}
