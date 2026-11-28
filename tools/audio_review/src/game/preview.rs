use std::{
    fs::{self, File},
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::AtomicBool,
};

use anyhow::{Context, Result, bail, ensure};
use tempfile::Builder;

use super::{
    Game,
    formats::Segment,
    tools::{check_cancelled, run_tool},
};

impl Game {
    pub fn preview(&self, id: &str, cancelled: &AtomicBool) -> Result<PathBuf> {
        check_cancelled(cancelled)?;
        let source = self
            .sources
            .get(id)
            .with_context(|| format!("No indexed audio file named {id}"))?;
        ensure!(
            id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'),
            "Invalid audio file ID"
        );
        let target_suffix = match source.suffix.as_str() {
            ".ogg" | ".opus" | ".wem" => ".wav",
            ".mp3" | ".flac" | ".wav" => source.suffix.as_str(),
            suffix => bail!("Unsupported source audio format {suffix}"),
        };
        let target = self.preview_dir.join(format!("{id}{target_suffix}"));
        if target.is_file() {
            return Ok(target);
        }

        let work = Builder::new()
            .prefix("preview-")
            .tempdir_in(&self.preview_dir)?;
        let encoded = work.path().join(format!("sound{}", source.suffix));
        let archive = self
            .archives
            .get(&source.archive)
            .context("Indexed archive is not installed")?;
        let mut stream = File::open(archive)?;
        let mut segment = source.segment();
        let mut range = 0..segment.size as usize;
        if source.length != 0 {
            let end = source
                .start
                .checked_add(u64::from(source.length))
                .context("Invalid audio range")?;
            ensure!(
                end <= u64::from(segment.size),
                "Indexed audio range is outside its archive segment"
            );
            if segment.stored == segment.size {
                // Only read the member's bytes from a raw bank or pack.
                segment = Segment {
                    offset: segment
                        .offset
                        .checked_add(source.start)
                        .context("Invalid audio offset")?,
                    stored: source.length,
                    size: source.length,
                };
                range = 0..source.length as usize;
            } else {
                range = source.start as usize..end as usize;
            }
        }
        let data = self.read_segment(&mut stream, segment)?;
        let payload = &data[range];
        ensure!(!payload.is_empty(), "Indexed audio data is missing");
        if matches!(source.suffix.as_str(), ".ogg" | ".opus") {
            ensure!(
                payload.starts_with(b"OggS"),
                "The indexed Opus stream does not contain an Ogg header"
            );
        }
        check_cancelled(cancelled)?;
        fs::write(&encoded, payload)?;

        let ready = if matches!(source.suffix.as_str(), ".ogg" | ".opus" | ".wem") {
            let decoded = work.path().join("decoded.wav");
            self.decode_preview(&encoded, &decoded, work.path(), &source.suffix, cancelled)?;
            decoded
        } else {
            encoded
        };
        check_cancelled(cancelled)?;
        fs::rename(ready, &target)?;
        Ok(target)
    }

    fn decode_preview(
        &self,
        source: &Path,
        target: &Path,
        work: &Path,
        suffix: &str,
        cancelled: &AtomicBool,
    ) -> Result<()> {
        if let Some(decoder) = &self.decoder {
            let mut command = Command::new(decoder);
            command
                .args(["-i", "-D", "2", "-o"])
                .arg(target)
                .arg(source);
            let result = decode(&mut command, work, target, "vgmstream", cancelled);
            if result.is_ok() || suffix == ".wem" || self.ffmpeg.is_none() {
                return result;
            }
        }
        check_cancelled(cancelled)?;
        ensure!(
            suffix != ".wem",
            "Set --decoder to vgmstream-cli to preview Wwise WEM audio."
        );
        let ffmpeg = self.ffmpeg.as_deref().context(
            "Set --decoder to vgmstream-cli or make ffmpeg available on PATH to preview Ogg and Opus audio.",
        )?;
        let mut command = Command::new(ffmpeg);
        command
            .args(["-v", "error", "-nostdin", "-y", "-i"])
            .arg(source);
        command.args(["-c:a", "pcm_s16le"]).arg(target);
        decode(&mut command, work, target, "FFmpeg", cancelled)
    }
}

fn decode(
    command: &mut Command,
    work: &Path,
    target: &Path,
    tool: &str,
    cancelled: &AtomicBool,
) -> Result<()> {
    let output = run_tool(command, work, cancelled)?;
    ensure!(
        output.status.success() && target.metadata().is_ok_and(|info| info.len() > 44),
        "{tool} could not decode this sound: {}",
        output.diagnostic()
    );
    Ok(())
}
