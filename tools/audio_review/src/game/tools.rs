use std::{
    env,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail, ensure};
use libloading::Library;

pub(super) fn check_cancelled(cancelled: &AtomicBool) -> Result<()> {
    ensure!(
        !cancelled.load(Ordering::Relaxed),
        "Audio operation canceled."
    );
    Ok(())
}

pub(super) fn executable(argument: Option<&Path>, name: &str) -> Result<Option<PathBuf>> {
    match argument {
        Some(path) => which::which(path)
            .and_then(|path| {
                std::fs::canonicalize(path).map_err(|_| which::Error::CannotCanonicalize)
            })
            .map(Some)
            .with_context(|| {
                format!(
                    "{name} is missing or cannot be executed at {}",
                    path.display()
                )
            }),
        None => Ok(which::which(name).ok()),
    }
}

pub(super) struct ToolOutput {
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
}

impl ToolOutput {
    pub fn diagnostic(&self) -> String {
        let message = if self.stderr.trim().is_empty() {
            &self.stdout
        } else {
            &self.stderr
        };
        let lines: Vec<_> = message.lines().rev().take(4).collect();
        let diagnostic = lines.into_iter().rev().collect::<Vec<_>>().join(" ");
        if diagnostic.is_empty() {
            self.status.to_string()
        } else {
            diagnostic
        }
    }
}

pub(super) fn run_tool(
    command: &mut Command,
    work: &Path,
    cancelled: &AtomicBool,
) -> Result<ToolOutput> {
    check_cancelled(cancelled)?;
    let mut stdout = tempfile::tempfile_in(work)?;
    let mut stderr = tempfile::tempfile_in(work)?;
    let tool = Path::new(command.get_program())
        .file_name()
        .unwrap_or(command.get_program())
        .to_string_lossy()
        .into_owned();
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout.try_clone()?))
        .stderr(Stdio::from(stderr.try_clone()?))
        .spawn()
        .with_context(|| format!("Could not start {tool}"))?;
    let deadline = Instant::now() + Duration::from_secs(180);
    let result = (|| {
        loop {
            check_cancelled(cancelled)?;
            if let Some(status) = child.try_wait()? {
                return Ok(status);
            }
            ensure!(Instant::now() < deadline, "{tool} timed out.");
            thread::sleep(Duration::from_millis(30));
        }
    })();
    if result.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    let status = result?;
    Ok(ToolOutput {
        status,
        stdout: read_log(&mut stdout, 64 * 1024 * 1024)?,
        stderr: read_log(&mut stderr, 4 * 1024 * 1024)?,
    })
}

fn read_log(file: &mut File, max: u64) -> Result<String> {
    let length = file.metadata()?.len();
    ensure!(length <= max, "An audio tool produced too much log output");
    let mut log = Vec::with_capacity(length as usize);
    file.seek(SeekFrom::Start(0))?;
    file.take(max).read_to_end(&mut log)?;
    Ok(String::from_utf8_lossy(&log).into_owned())
}

pub(super) fn wolvenkit_list(exe: &Path, archive: &Path) -> Command {
    let mut command = Command::new(exe);
    command.args(["archive"]).arg(archive).args([
        "--list",
        "--regex",
        r"(?i)\.(wem|ogg|opus|wav|mp3|flac|bnk|opusinfo|opuspak)$",
    ]);
    if env::var_os("DOTNET_ROOT").is_none()
        && let Some(root) = exe.parent().and_then(Path::parent)
        && root.join("dotnet/shared").is_dir()
    {
        command.env("DOTNET_ROOT", root.join("dotnet"));
    }
    command
}

type Decompress = unsafe extern "C" fn(*const u8, usize, *mut u8, usize) -> i32;

pub(super) struct Kraken {
    // Keep the native code loaded as long as the copied function pointer is used.
    _library: Library,
    decode: Decompress,
}

impl Kraken {
    pub fn load(wolvenkit: Option<&Path>) -> Result<Self> {
        let Some(directory) = wolvenkit.and_then(Path::parent) else {
            bail!("Set --wolvenkit to preview compressed archive entries.");
        };
        let name = match env::consts::OS {
            "windows" => "kraken.dll",
            "macos" => "libkraken.dylib",
            _ => "libkraken.so",
        };
        let path = directory.join(name);
        // SAFETY: The user-selected WolvenKit distribution supplies this native library and ABI.
        let library = unsafe { Library::new(&path) }
            .with_context(|| format!("Cannot load WolvenKit's {}", path.display()))?;
        let decode = unsafe { *library.get(b"Kraken_Decompress\0")? };
        Ok(Self {
            _library: library,
            decode,
        })
    }

    pub fn decompress(&self, input: &[u8], size: u32) -> Result<Vec<u8>> {
        // The Kraken decoder uses padded output for its SIMD copy operations.
        let mut output = vec![0; size as usize + 64];
        // SAFETY: Both buffers outlive the call. The declared output length excludes its padding.
        let written = unsafe {
            (self.decode)(
                input.as_ptr(),
                input.len(),
                output.as_mut_ptr(),
                size as usize,
            )
        };
        ensure!(
            i64::from(written) == i64::from(size),
            "WolvenKit could not decompress an audio archive segment"
        );
        output.truncate(size as usize);
        Ok(output)
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn cancelled_converter_exits_and_is_reaped() -> Result<()> {
        let work = tempfile::tempdir()?;
        let pidfile = work.path().join("pid");
        let cancelled = AtomicBool::new(false);
        thread::scope(|scope| {
            scope.spawn(|| {
                let deadline = Instant::now() + Duration::from_secs(3);
                while !pidfile.exists() && Instant::now() < deadline {
                    thread::sleep(Duration::from_millis(10));
                }
                cancelled.store(true, Ordering::Relaxed);
            });
            let mut command = Command::new("sh");
            command
                .args(["-c", "echo $$ > \"$1\"; exec sleep 20", "converter"])
                .arg(&pidfile);
            assert!(
                run_tool(&mut command, work.path(), &cancelled)?
                    .status
                    .success()
            );
            Ok::<(), anyhow::Error>(())
        })
        .expect_err("Process should be canceled");
        let pid = std::fs::read_to_string(&pidfile)?;
        let process_exists = Command::new("kill")
            .args(["-0", pid.trim()])
            .stderr(Stdio::null())
            .status()?;
        assert!(!process_exists.success());
        Ok(())
    }
}
