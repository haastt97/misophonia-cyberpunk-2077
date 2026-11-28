use std::{
    collections::BTreeMap,
    fs::File,
    io::BufWriter,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::AtomicBool,
};

use anyhow::{Context, Result, ensure};
use rodio::{Decoder, Source};

use crate::{
    build_plan::BuildPlan,
    configurable,
    game::{
        Game,
        tools::{check_cancelled, run_tool},
    },
    standalone,
};

#[derive(Clone)]
pub struct PackageSource {
    pub game: PathBuf,
    pub catalog: PathBuf,
    pub wolvenkit: Option<PathBuf>,
}

pub struct BuiltPackage {
    pub path: PathBuf,
    pub notices: Vec<String>,
}

pub fn version_name(input: &str) -> Result<&str> {
    let name = input.trim();
    ensure!(
        (1..=64).contains(&name.len())
            && name.as_bytes()[0].is_ascii_alphanumeric()
            && name
                .as_bytes()
                .last()
                .is_some_and(u8::is_ascii_alphanumeric)
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b" _-.".contains(&b)),
        "Enter a version name up to 64 characters using letters, numbers, spaces, dots, underscores, or hyphens. Start and end with a letter or number"
    );
    Ok(name)
}

pub fn build(
    source: &PackageSource,
    game: &Game,
    plan: &BuildPlan,
    version: &str,
    output: &Path,
    cancelled: &AtomicBool,
) -> Result<BuiltPackage> {
    let version = version_name(version)?;
    let output = output
        .canonicalize()
        .context("Open the selected save folder")?;
    ensure!(output.is_dir(), "Choose a save folder");
    ensure!(
        !output.starts_with(&source.game),
        "Choose a save folder outside the installed game"
    );
    let slug = version.replace(' ', "-");
    let path = output.join(format!("Misophonia-{slug}"));
    ensure!(
        !path.exists(),
        "{} already exists. Choose a different version name or folder",
        path.display()
    );

    let stage = tempfile::Builder::new()
        .prefix(".misophonia-build-")
        .tempdir_in(&output)?;
    check_cancelled(cancelled)?;
    let mut notices = Vec::new();
    {
        let audio = tempfile::Builder::new()
            .prefix(".misophonia-audio-")
            .tempdir_in(stage.path())?;
        let mut donor_audio = BTreeMap::new();
        for (num, (id, donor)) in plan.stand_ins.iter().enumerate() {
            check_cancelled(cancelled)?;
            let wav = audio.path().join(format!("donor-{num}.wav"));
            write_donor(game, id, &wav, cancelled)
                .with_context(|| format!("Prepare stand-in {}", donor.name))?;
            donor_audio.insert(id.clone(), wav);
        }
        notices.extend(configurable::build(
            &source.catalog,
            stage.path(),
            &slug,
            plan,
            &donor_audio,
        )?);
    }
    if !plan
        .categories
        .iter()
        .all(|category| category.events.is_empty())
    {
        if let Some(wolvenkit) = &source.wolvenkit {
            let skipped = standalone::build(
                &source.game,
                wolvenkit,
                stage.path(),
                &slug,
                plan,
                cancelled,
            )?;
            notices.extend(
                skipped
                    .into_iter()
                    .map(|message| format!("Standalone skipped: {message}")),
            );
        } else {
            notices.push(
                "Standalone packages require WolvenKit CLI. Set --wolvenkit and build again."
                    .to_owned(),
            );
        }
    }
    if !plan.already_uncovered.is_empty() {
        notices.push(format!(
            "{} removal candidates already have no linked coverage in the selected catalog. See build-report.json.",
            plan.already_uncovered.len()
        ));
    }
    if !plan.shared_events.is_empty() {
        notices.push(format!(
            "{} edited events also play other clips. See the event list in build-report.json.",
            plan.shared_events.len()
        ));
    }
    let report = serde_json::json!({
        "version": version,
        "notices": notices,
        "plan": plan,
    });
    let file = File::create(stage.path().join("build-report.json"))?;
    let mut writer = BufWriter::new(file);
    serde_json::to_writer_pretty(&mut writer, &report)?;
    use std::io::Write;
    writer.write_all(b"\n")?;
    writer.flush()?;
    check_cancelled(cancelled)?;
    publish(stage.path(), &path)?;
    Ok(BuiltPackage { path, notices })
}

#[cfg(target_os = "linux")]
fn publish(stage: &Path, output: &Path) -> Result<()> {
    use rustix::fs::{CWD, RenameFlags, renameat_with};
    renameat_with(CWD, stage, CWD, output, RenameFlags::NOREPLACE)
        .with_context(|| format!("Save the custom version to {}", output.display()))
}

#[cfg(not(target_os = "linux"))]
fn publish(stage: &Path, output: &Path) -> Result<()> {
    ensure!(!output.exists(), "{} already exists", output.display());
    std::fs::rename(stage, output)
        .with_context(|| format!("Save the custom version to {}", output.display()))
}

fn write_donor(game: &Game, id: &str, target: &Path, cancelled: &AtomicBool) -> Result<()> {
    let preview = game.preview(id, cancelled)?;
    let decoder = Decoder::try_from(File::open(&preview)?)?;
    let channels = decoder.channels();
    if channels > 2 {
        let ffmpeg = which::which("ffmpeg")
            .context("Install FFmpeg to downmix multichannel stand-ins to stereo")?;
        let mut command = Command::new(ffmpeg);
        command
            .args(["-v", "error", "-nostdin", "-i"])
            .arg(&preview)
            .args(["-ac", "2", "-c:a", "pcm_s16le"])
            .arg(target);
        let output = run_tool(
            &mut command,
            target.parent().unwrap_or(Path::new(".")),
            cancelled,
        )?;
        ensure!(
            output.status.success(),
            "FFmpeg could not downmix this stand-in: {}",
            output.diagnostic()
        );
        return Ok(());
    }
    let spec = hound::WavSpec {
        channels,
        sample_rate: decoder.sample_rate(),
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut wav = hound::WavWriter::create(target, spec)?;
    for (index, sample) in decoder.enumerate() {
        if index % 65536 == 0 {
            check_cancelled(cancelled)?;
        }
        wav.write_sample((sample.clamp(-1., 1.) * f32::from(i16::MAX)).round() as i16)?;
    }
    wav.finalize()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_names_are_safe_single_path_components() {
        assert_eq!(version_name("  1.0 Personal  ").unwrap(), "1.0 Personal");
        for invalid in ["", ".", "../test", "test.", "C:\\test", "x/y", " X\tY "] {
            assert!(version_name(invalid).is_err(), "{invalid}");
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn publishing_does_not_replace_even_an_empty_directory() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let old = temp.path().join("old");
        let stage = temp.path().join("stage");
        std::fs::create_dir(&old)?;
        std::fs::create_dir(&stage)?;
        assert!(publish(&stage, &old).is_err());
        assert!(stage.is_dir());
        Ok(())
    }
}
