use std::{fs::File, path::PathBuf, sync::Arc, time::Duration};

use anyhow::{Context, Result};
use eframe::egui;
use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink, Source};

use crate::{game::Game, job::Job};

struct PlayingFile {
    path: PathBuf,
    sink: Sink,
    start: Duration,
    duration: Duration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Play,
    Pause,
    Cancel,
}

impl Action {
    pub fn label(self) -> &'static str {
        match self {
            Self::Play => "Play",
            Self::Pause => "Pause",
            Self::Cancel => "Cancel",
        }
    }
}

pub struct Player {
    pending: Option<Job<PathBuf>>,
    retired: Vec<Job<PathBuf>>,
    active: Option<PlayingFile>,
    output: Option<OutputStream>,
    pub index: Option<usize>,
    pub volume: u8,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            pending: None,
            retired: Vec::new(),
            active: None,
            output: None,
            index: None,
            volume: 35,
        }
    }
}

impl Player {
    pub fn action(&self, index: usize) -> Action {
        if self.index != Some(index) {
            return Action::Play;
        }
        if self.pending.is_some() {
            return Action::Cancel;
        }
        if self
            .active
            .as_ref()
            .is_some_and(|file| !file.sink.empty() && !file.sink.is_paused())
        {
            return Action::Pause;
        }
        Action::Play
    }

    pub fn toggle(&mut self, ctx: &egui::Context, game: &Arc<Game>, index: usize, id: &str) {
        match self.action(index) {
            Action::Cancel => self.stop(),
            Action::Pause => {
                if let Some(file) = &self.active {
                    file.sink.pause()
                }
            }
            Action::Play => {
                if let Some(file) = self
                    .active
                    .as_ref()
                    .filter(|file| self.index == Some(index) && !file.sink.empty())
                {
                    file.sink.play();
                    return;
                }
                self.stop();
                let game = Arc::clone(game);
                let id = id.to_owned();
                self.index = Some(index);
                self.pending = Some(Job::start(ctx, move |cancelled| {
                    game.preview(&id, cancelled)
                }));
            }
        }
    }

    fn stop(&mut self) {
        if let Some(job) = self.pending.take() {
            job.cancel();
            self.retired.push(job);
        }
        self.active = None;
        self.index = None;
    }

    pub fn poll(&mut self) -> Result<()> {
        self.retired.retain(|job| !job.is_finished());
        if let Some(result) = self.pending.as_ref().and_then(Job::poll) {
            self.pending = None;
            let playing = result.and_then(|path| self.open(path, Duration::ZERO, false));
            self.active = Some(playing.inspect_err(|_| self.index = None)?);
        }
        Ok(())
    }

    fn open(&mut self, path: PathBuf, start: Duration, paused: bool) -> Result<PlayingFile> {
        let mut audio = Decoder::try_from(File::open(&path)?)
            .with_context(|| format!("Decode preview {}", path.display()))?;
        let duration = audio.total_duration().unwrap_or_default();
        if !start.is_zero() {
            audio
                .try_seek(start)
                .map_err(|error| anyhow::anyhow!("Seek preview: {error}"))?;
        }
        if self.output.is_none() {
            let mut output = OutputStreamBuilder::open_default_stream()
                .context("Open the default audio output")?;
            output.log_on_drop(false);
            self.output = Some(output);
        }
        let sink = Sink::connect_new(self.output.as_ref().unwrap().mixer());
        sink.set_volume(f32::from(self.volume) / 100.);
        if paused {
            sink.pause()
        }
        sink.append(audio);
        Ok(PlayingFile {
            path,
            sink,
            start,
            duration,
        })
    }

    pub fn timing(&self) -> (Duration, Duration) {
        self.active
            .as_ref()
            .map_or((Duration::ZERO, Duration::ZERO), |file| {
                let position = if file.sink.empty() {
                    file.duration
                } else {
                    file.start + file.sink.get_pos()
                };
                (position.min(file.duration), file.duration)
            })
    }

    pub fn seek(&mut self, position: Duration) -> Result<()> {
        if let Some(file) = &self.active {
            let paused = file.sink.is_paused() || file.sink.empty();
            let max = file.duration.saturating_sub(Duration::from_millis(1));
            self.active = Some(self.open(file.path.clone(), position.min(max), paused)?);
        }
        Ok(())
    }

    pub fn update_volume(&self) {
        if let Some(file) = &self.active {
            file.sink.set_volume(f32::from(self.volume) / 100.)
        }
    }
}
