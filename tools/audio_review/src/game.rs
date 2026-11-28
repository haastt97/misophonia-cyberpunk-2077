mod formats;
mod preview;
pub(crate) mod tools;

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, hash_map::Entry},
    fs::{self, File},
    io::{BufReader, BufWriter, Cursor, Write},
    path::{Path, PathBuf},
    sync::{OnceLock, atomic::AtomicBool},
    time::UNIX_EPOCH,
};

use anyhow::{Context, Result, anyhow, bail, ensure};
use byteorder::{ByteOrder, LittleEndian as Le};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;

use formats::{
    MAX_BLOCK, Member, Segment, archive_entries, audio_suffix, bank_members, opus_members, read_at,
};
use tools::{Kraken, check_cancelled, executable, run_tool, wolvenkit_list};

const CATALOG_SCHEMA: u32 = 1;
const AUDIO_SUFFIXES: &[&str] = &[
    ".wem",
    ".ogg",
    ".opus",
    ".wav",
    ".mp3",
    ".flac",
    ".bnk",
    ".opusinfo",
    ".opuspak",
];
type Fingerprint = (u32, PathBuf, BTreeMap<String, (u64, u64)>);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Asset {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archive: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_id: Option<u64>,
}

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize)]
struct Source {
    archive: String,
    offset: u64,
    stored: u32,
    size: u32,
    suffix: String,
    start: u64,
    length: u32,
}

impl Source {
    fn file(archive: &str, segment: Segment, suffix: &str) -> Self {
        Self {
            archive: archive.into(),
            offset: segment.offset,
            stored: segment.stored,
            size: segment.size,
            suffix: suffix.into(),
            start: 0,
            length: 0,
        }
    }

    fn segment(&self) -> Segment {
        Segment {
            offset: self.offset,
            stored: self.stored,
            size: self.size,
        }
    }

    fn member(archive: &str, segment: Segment, suffix: &str, member: Member) -> Self {
        Self {
            start: member.start,
            length: member.length,
            ..Self::file(archive, segment, suffix)
        }
    }
}

#[derive(Default, Deserialize, Serialize)]
struct Index {
    fingerprint: Option<Fingerprint>,
    rows: Vec<Asset>,
    sources: HashMap<String, Source>,
    warnings: Vec<String>,
}

impl Index {
    fn add(&mut self, mut asset: Asset, source: Source) -> Result<()> {
        let archive_id = format!("{:x}", Sha256::digest(source.archive.as_bytes()));
        asset.id.push('-');
        asset.id.push_str(&archive_id[..12]);
        if let Some(existing) = self.sources.get(&asset.id) {
            if existing == &source {
                return Ok(());
            }
            asset.id.push_str(&format!("-{:x}", source.start));
        }
        match self.sources.entry(asset.id.clone()) {
            Entry::Occupied(_) => bail!(
                "Conflicting audio ID for {} in {}",
                asset.name,
                source.archive
            ),
            Entry::Vacant(entry) => {
                entry.insert(source);
            }
        }
        self.rows.push(asset);
        Ok(())
    }
}

pub struct Game {
    archives: BTreeMap<String, PathBuf>,
    cache: PathBuf,
    preview_dir: PathBuf,
    wolvenkit: Option<PathBuf>,
    decoder: Option<PathBuf>,
    ffmpeg: Option<PathBuf>,
    kraken: OnceLock<Result<Kraken, String>>,
    sources: HashMap<String, Source>,
    pub warnings: Vec<String>,
}

impl Game {
    pub fn wolvenkit(&self) -> Option<&Path> {
        self.wolvenkit.as_deref()
    }

    pub fn build(
        game: &Path,
        cache: &Path,
        wolvenkit: Option<&Path>,
        decoder: Option<&Path>,
        cancelled: &AtomicBool,
    ) -> Result<(Self, Vec<Asset>)> {
        let game = game
            .canonicalize()
            .with_context(|| format!("Cannot open game directory {}", game.display()))?;
        fs::create_dir_all(cache)?;
        let archives = audio_archives(&game.join("archive/pc"))?;
        let fingerprints = archives
            .iter()
            .map(|(id, path)| {
                let info = path.metadata()?;
                let time = info
                    .modified()?
                    .duration_since(UNIX_EPOCH)?
                    .as_nanos()
                    .try_into()?;
                Ok((id.clone(), (info.len(), time)))
            })
            .collect::<Result<BTreeMap<_, _>>>()?;
        let fingerprint = (CATALOG_SCHEMA, game, fingerprints);
        let generation = format!("{:x}", Sha256::digest(serde_json::to_vec(&fingerprint)?));
        let preview_dir = cache.join("previews-wav").join(&generation[..16]);
        fs::create_dir_all(&preview_dir)?;
        let wolvenkit = executable(wolvenkit, "WolvenKit.CLI")?;
        let decoder = executable(decoder, "vgmstream-cli")?;
        let ffmpeg = executable(None, "ffmpeg")?;
        let mut game = Self {
            archives,
            cache: cache.into(),
            preview_dir,
            wolvenkit,
            decoder,
            ffmpeg,
            kraken: OnceLock::new(),
            sources: HashMap::new(),
            warnings: Vec::new(),
        };

        let index_file = cache.join("assets.json");
        check_cancelled(cancelled)?;
        let cached = || -> Option<Index> {
            let reader = BufReader::new(File::open(&index_file).ok()?);
            let cached: Index = serde_json::from_reader(reader).ok()?;
            (cached.fingerprint.as_ref() == Some(&fingerprint) && !cached.rows.is_empty())
                .then_some(cached)
        }();
        let index = if let Some(cached) = cached {
            cached
        } else {
            let wolvenkit = game
                .wolvenkit
                .as_deref()
                .context("Set --wolvenkit to WolvenKit.CLI to index sound file names.")?;
            let mut index = game.scan(wolvenkit, cancelled)?;
            check_cancelled(cancelled)?;
            index.fingerprint = Some(fingerprint);
            let mut temp = NamedTempFile::new_in(cache)?;
            {
                let mut output = BufWriter::new(temp.as_file_mut());
                serde_json::to_writer(&mut output, &index)?;
                output.flush()?;
            }
            check_cancelled(cancelled)?;
            temp.persist(&index_file)?;
            index
        };
        check_cancelled(cancelled)?;
        game.sources = index.sources;
        game.warnings = index.warnings;
        Ok((game, index.rows))
    }

    fn scan(&self, wolvenkit: &Path, cancelled: &AtomicBool) -> Result<Index> {
        let mut index = Index::default();
        let mut infos = Vec::new();
        let mut packs = BTreeMap::new();
        let mut banks = Vec::new();

        for (archive, path) in &self.archives {
            check_cancelled(cancelled)?;
            let entries = archive_entries(path)
                .with_context(|| format!("Could not index {}", path.display()))?;
            let names = self.resource_paths(wolvenkit, path, cancelled)?;
            let mut stream = File::open(path)?;
            for (hash, segment) in entries {
                check_cancelled(cancelled)?;
                let name = names.get(&hash).map(String::as_str);
                let suffix = match name {
                    Some(name) => file_suffix(name).unwrap_or_default(),
                    None => self.guess_suffix(&mut stream, segment)?.into(),
                };
                match suffix.as_str() {
                    "" => {}
                    ".opusinfo" => {
                        infos.push((archive.as_str(), self.read_segment(&mut stream, segment)?))
                    }
                    ".opuspak" => {
                        if let Some(pack) = name.and_then(opus_pack) {
                            packs.insert((archive.as_str(), pack), segment);
                        }
                    }
                    ".bnk" => {
                        banks.push((archive.as_str(), hash, segment, name.map(str::to_owned)))
                    }
                    _ => index.add(
                        file_asset(archive, hash, name),
                        Source::file(archive, segment, &suffix),
                    )?,
                }
            }
        }

        for (archive, hash, segment, path) in banks {
            check_cancelled(cancelled)?;
            let mut stream = File::open(&self.archives[archive])?;
            let members = if segment.stored == segment.size {
                bank_members(&mut stream, segment.offset, segment.size)?
            } else {
                let data = self.read_segment(&mut stream, segment)?;
                bank_members(&mut Cursor::new(data), 0, segment.size)?
            };
            let name = path
                .as_deref()
                .map(file_name)
                .map_or_else(|| format!("{hash:016x}.bnk"), str::to_owned);
            let stem = name.strip_suffix(".bnk").unwrap_or(&name);
            for member in members {
                check_cancelled(cancelled)?;
                let media = member.media_id;
                index.add(
                    Asset {
                        id: format!("b-{hash:016x}-{media}"),
                        name: format!("{stem} · {media}"),
                        kind: "sfx".into(),
                        detail: format!("Embedded in {name}"),
                        archive: Some(archive.into()),
                        path: path.as_deref().map(normalized_path),
                        media_id: Some(u64::from(media)),
                    },
                    Source::member(archive, segment, ".wem", member),
                )?;
            }
        }

        let mut by_region = BTreeMap::new();
        for (&(archive, pack), &segment) in &packs {
            by_region
                .entry((archive_region(archive), pack))
                .or_insert((archive, segment));
        }
        let mut missing = BTreeSet::new();
        for (archive, info) in &infos {
            for (pack, member) in opus_members(info)? {
                check_cancelled(cancelled)?;
                let found = packs
                    .get(&(*archive, pack))
                    .map(|segment| (*archive, *segment))
                    .or_else(|| by_region.get(&(archive_region(archive), pack)).copied())
                    .or_else(|| by_region.get(&("content", pack)).copied());
                let Some((origin, segment)) = found.filter(|(_, segment)| {
                    member.start + u64::from(member.length) <= u64::from(segment.size)
                }) else {
                    missing.insert((*archive, pack));
                    continue;
                };

                let media = member.media_id;
                index.add(
                    Asset {
                        id: format!("o-{media}"),
                        name: media.to_string(),
                        kind: "sfx".into(),
                        detail: format!("Opus pack {pack} · {media}"),
                        archive: Some(origin.into()),
                        path: Some(format!(
                            "base/sound/soundbanks/sfx_container_{pack}.opuspak"
                        )),
                        media_id: Some(u64::from(media)),
                    },
                    Source::member(origin, segment, ".ogg", member),
                )?;
            }
        }
        if !missing.is_empty() {
            index.warnings.push(format!(
                "{} Opus pack references are absent or out of bounds in this installation.",
                missing.len()
            ));
        }
        if infos.is_empty() {
            index
                .warnings
                .push("No packed sound effect index was found.".into());
        }
        ensure!(
            !index.rows.is_empty(),
            "No playable sound files were found in the installed audio archives."
        );
        Ok(index)
    }

    fn resource_paths(
        &self,
        wolvenkit: &Path,
        path: &Path,
        cancelled: &AtomicBool,
    ) -> Result<HashMap<u64, String>> {
        let output = run_tool(&mut wolvenkit_list(wolvenkit, path), &self.cache, cancelled)?;
        ensure!(
            output.status.success()
                && !output.stdout.contains("[ERROR]")
                && !output.stdout.contains("Exception"),
            "WolvenKit could not list {}: {}",
            path.display(),
            output.diagnostic()
        );
        let mut paths = HashMap::new();
        for line in output.stdout.lines() {
            let name = line.trim().replace('/', "\\");
            if name.contains('\\')
                && !name.starts_with("\\\\")
                && name.as_bytes().get(1) != Some(&b':')
                && file_suffix(&name)
                    .is_some_and(|suffix| AUDIO_SUFFIXES.contains(&suffix.as_str()))
            {
                paths.insert(resource_hash(&name), name.clone());
                paths
                    .entry(resource_hash(&name.to_lowercase()))
                    .or_insert(name);
            }
        }
        Ok(paths)
    }

    fn guess_suffix(&self, stream: &mut File, segment: Segment) -> Result<&'static str> {
        if segment.stored == segment.size {
            let mut header = [0; 12];
            let size = (segment.size as usize).min(header.len());
            read_at(stream, segment.offset, &mut header[..size])?;
            Ok(audio_suffix(&header[..size]))
        } else {
            let data = self.read_segment(stream, segment)?;
            Ok(audio_suffix(&data[..data.len().min(12)]))
        }
    }

    fn read_segment(&self, stream: &mut File, segment: Segment) -> Result<Vec<u8>> {
        ensure!(
            segment.size <= MAX_BLOCK && segment.stored <= MAX_BLOCK,
            "Audio archive segment is too large to read ({} bytes)",
            segment.size
        );
        let mut data = vec![0; segment.stored as usize];
        read_at(stream, segment.offset, &mut data)?;
        if segment.size == segment.stored {
            return Ok(data);
        }
        ensure!(
            data.len() > 8 && data.starts_with(b"KARK") && Le::read_u32(&data[4..]) == segment.size,
            "Unrecognized compressed audio archive segment"
        );

        let kraken = self.kraken.get_or_init(|| {
            Kraken::load(self.wolvenkit.as_deref()).map_err(|error| format!("{error:#}"))
        });
        let kraken = kraken.as_ref().map_err(|error| anyhow!("{error}"))?;
        kraken.decompress(&data[8..], segment.size)
    }
}

fn audio_archives(dir: &Path) -> Result<BTreeMap<String, PathBuf>> {
    let mut archives = BTreeMap::new();
    for region in ["content", "ep1"] {
        let directory = dir.join(region);
        let files = match directory.read_dir() {
            Ok(files) => files,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("Could not read {}", directory.display()));
            }
        };
        for entry in files {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.ends_with(".archive")
                && (name.starts_with("audio_")
                    || (name.starts_with("lang_") && name.ends_with("_voice.archive")))
                && entry.path().is_file()
            {
                archives.insert(format!("{region}/{name}"), entry.path());
            }
        }
    }
    ensure!(
        !archives.is_empty(),
        "No Cyberpunk 2077 audio archives found in {}",
        dir.display()
    );
    Ok(archives)
}

fn archive_region(archive: &str) -> &str {
    archive.split('/').next().unwrap_or_default()
}

fn file_name(path: &str) -> &str {
    path.rsplit('\\').next().unwrap_or(path)
}

fn file_suffix(path: &str) -> Option<String> {
    let (_, suffix) = file_name(path).rsplit_once('.')?;
    Some(format!(".{}", suffix.to_ascii_lowercase()))
}

fn opus_pack(path: &str) -> Option<u16> {
    file_name(path)
        .to_ascii_lowercase()
        .strip_prefix("sfx_container_")?
        .strip_suffix(".opuspak")?
        .parse()
        .ok()
}

fn normalized_path(path: &str) -> String {
    path.split('\\')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect::<Vec<_>>()
        .join("/")
}

fn resource_hash(path: &str) -> u64 {
    path.bytes().fold(0xcbf29ce484222325, |value, byte| {
        (value ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    })
}

fn file_asset(archive: &str, hash: u64, path: Option<&str>) -> Asset {
    let name = path
        .map(file_name)
        .and_then(|name| name.rsplit_once('.').map(|(stem, _)| stem))
        .map_or_else(|| format!("{hash:016x}"), str::to_owned);
    let media_id = if name.bytes().all(|b| b.is_ascii_digit()) {
        name.parse().ok()
    } else {
        None
    };
    let kind = if archive.ends_with("_voice.archive")
        || path.is_some_and(|path| path.to_ascii_lowercase().contains("\\vo\\"))
    {
        "voice"
    } else if path.is_some_and(|path| {
        path.split('\\')
            .any(|part| part.eq_ignore_ascii_case("music") || part.eq_ignore_ascii_case("radio"))
    }) {
        "music"
    } else {
        "sound"
    };
    Asset {
        id: format!("f-{hash:016x}"),
        name,
        kind: kind.into(),
        detail: archive.into(),
        archive: Some(archive.into()),
        path: path.map(normalized_path),
        media_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_ids_match_the_previous_app() -> Result<()> {
        let archive = "content/audio_1_general.archive";
        let path = r"base\localization\common\vo\ash_radio_growl_f_32d3f1017a8ae000.wem";
        let hash = resource_hash(path);
        let mut index = Index::default();
        let segment = Segment {
            offset: 20,
            stored: 64,
            size: 64,
        };
        index.add(
            file_asset(archive, hash, Some(path)),
            Source::file(archive, segment, ".wem"),
        )?;
        let sound = &index.rows[0];
        assert_eq!(sound.id, "f-cfce1bd3891a0a1e-5b443ee0633a");
        assert_eq!(sound.kind, "voice");
        assert_eq!(
            sound.path.as_deref(),
            Some("base/localization/common/vo/ash_radio_growl_f_32d3f1017a8ae000.wem")
        );
        Ok(())
    }

    #[test]
    fn index_reads_the_previous_apps_cache_layout() -> Result<()> {
        let json = r#"{
            "fingerprint":[1,"/game",{"content/audio_1_general.archive":[88,4]}],
            "warnings":[],
            "rows":[{"id":"f-ab-42","name":"growl","kind":"voice","detail":"content/audio_1_general.archive","archive":"content/audio_1_general.archive"}],
            "sources":{"f-ab-42":{"archive":"content/audio_1_general.archive","offset":20,"stored":64,"size":64,"suffix":".wem","start":0,"length":0}}
        }"#;
        let index: Index = serde_json::from_str(json)?;
        assert_eq!(index.fingerprint.as_ref().unwrap().1, Path::new("/game"));
        assert_eq!(index.rows[0].name, "growl");
        assert_eq!(index.sources["f-ab-42"].stored, 64);
        assert_eq!(
            serde_json::to_value(index)?,
            serde_json::from_str::<serde_json::Value>(json)?
        );
        Ok(())
    }
}
