mod build_plan;
mod catalog;
mod category_ui;
mod configurable;
mod desktop;
mod game;
mod job;
mod package_build;
mod package_ui;
mod player;
mod project_ui;
mod review;
mod review_categories;
mod standalone;

use std::{
    env, fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use anyhow::{Context, Result, ensure};
use clap::Parser;
use eframe::egui;

use crate::{catalog::Catalog, game::Game, review::Review};

#[derive(Parser)]
#[command(about = "Browse the local Cyberpunk 2077 sound library and plan Misophonia updates")]
struct Args {
    /// Cyberpunk 2077 installation directory
    #[arg(long)]
    game: PathBuf,
    /// Misophonia source checkout (defaults to the checkout or working directory)
    #[arg(long)]
    catalog: Option<PathBuf>,
    /// Local SoundDB events.json
    #[arg(long)]
    event_index: Option<PathBuf>,
    /// Persistent review folder
    #[arg(long)]
    data: Option<PathBuf>,
    /// Disposable index and preview folder
    #[arg(long)]
    cache: Option<PathBuf>,
    /// Path to WolvenKit.CLI
    #[arg(long)]
    wolvenkit: Option<PathBuf>,
    /// Path to vgmstream-cli
    #[arg(long)]
    decoder: Option<PathBuf>,
}

struct Library {
    game: Arc<Game>,
    review: Review,
    package_source: package_build::PackageSource,
}

fn expand(path: &Path) -> PathBuf {
    if let Some(home) = env::home_dir()
        && let Ok(relative) = path.strip_prefix("~")
    {
        return home.join(relative);
    }
    path.to_owned()
}

fn default_dir(variable: &str, fallback: &str) -> Result<PathBuf> {
    let base = if let Some(value) = env::var_os(variable).filter(|value| !value.is_empty()) {
        PathBuf::from(value)
    } else {
        env::home_dir()
            .context("Home directory unavailable. Supply --data and --cache")?
            .join(fallback)
    };
    Ok(base.join("misophonia/audio-review"))
}

fn load(args: Args, cancelled: &AtomicBool) -> Result<Library> {
    let data = args
        .data
        .as_deref()
        .map(expand)
        .map_or_else(|| default_dir("XDG_DATA_HOME", ".local/share"), Ok)?;
    let cache = args
        .cache
        .as_deref()
        .map(expand)
        .map_or_else(|| default_dir("XDG_CACHE_HOME", ".cache"), Ok)?;
    let checkout = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let current = env::current_dir()?;
    let catalog_root = args.catalog.as_deref().map(expand).unwrap_or_else(|| {
        if current.join("r6/scripts/Misophonia/Config.reds").is_file() || !checkout.is_dir() {
            current
        } else {
            checkout
        }
    });
    let catalog = Catalog::read(&catalog_root)?;
    fs::create_dir_all(&cache).with_context(|| format!("Create {}", cache.display()))?;
    let events = catalog::event_media(args.event_index.as_deref().map(expand).as_deref(), &cache)?;
    ensure!(
        !cancelled.load(Ordering::Relaxed),
        "Audio library indexing canceled"
    );
    let game_root = expand(&args.game)
        .canonicalize()
        .context("Open the Cyberpunk installation")?;
    let (game, assets) = Game::build(
        &game_root,
        &cache,
        args.wolvenkit.as_deref().map(expand).as_deref(),
        args.decoder.as_deref().map(expand).as_deref(),
        cancelled,
    )?;
    let review = Review::open(catalog, assets, &events, &data)?;
    let package_source = package_build::PackageSource {
        game: game_root,
        catalog: catalog_root
            .canonicalize()
            .context("Open the Misophonia source folder")?,
        wolvenkit: game.wolvenkit().map(Path::to_path_buf),
    };
    Ok(Library {
        game: Arc::new(game),
        review,
        package_source,
    })
}

fn main() -> eframe::Result {
    let args = Args::parse();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_app_id("misophonia-audio-review")
            .with_inner_size([1180., 760.])
            .with_min_inner_size([880., 520.]),
        ..Default::default()
    };
    eframe::run_native(
        "Misophonia Audio Review",
        options,
        Box::new(|cc| Ok(Box::new(desktop::Desktop::new(&cc.egui_ctx, args)))),
    )
}
