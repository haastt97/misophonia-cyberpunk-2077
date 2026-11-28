# Misophonia Audio Review

Audio Review is a native Rust desktop program for previewing installed Cyberpunk 2077 sounds and planning changes to Misophonia. It runs independently of the game.

## Build and run

Use a stable Rust toolchain with Cargo, a C linker and, on Linux, pkg-config, ALSA development headers (`alsa-lib-devel` on Fedora), and a Wayland or X11 desktop with OpenGL. Linux export uses the desktop's XDG file chooser portal.

Install [WolvenKit CLI](https://github.com/WolvenKit/WolvenKit/releases) for archive indexing and [vgmstream-cli](https://github.com/vgmstream/vgmstream/releases) for Wwise and packed Opus previews. Put the tools on your path or use `--wolvenkit` and `--decoder` to pass their paths. WolvenKit may require `DOTNET_ROOT` for a standalone .NET install. If vgmstream is unavailable, FFmpeg can decode Ogg and Opus previews.

From the Misophonia source checkout, build and open the window.

```sh
cargo run --release --locked --manifest-path tools/audio_review/Cargo.toml -- \
  --game "/path/to/Cyberpunk 2077"
```

The built executable is `tools/audio_review/target/release/misophonia-audio-review`. Use `--catalog /path/to/Misophonia` to select a specific source checkout and `--event-index /path/to/events.json` to use local event metadata. The first run without a local copy downloads and caches the public event-to-media index from [Cyberpunk SoundDB](https://sounddb.zhincore.eu/). Restart after updating the game or catalog.

The library includes packed effects, complete embedded soundbank clips, streamed audio, and installed voice languages from the base game and Phantom Liberty. In-game events can mix multiple clips or change how they play.

## Review sounds

- Search by filename, media ID, or linked event. Use **All sounds** for the full library and **Outside the mod** for installed files with no linked Misophonia setting.
- The category dropdown starts on **All**. In the library, choosing a category narrows sounds with mod coverage and keeps files outside the mod visible. In **Candidates** and **Stand-ins**, it filters your review queue.
- Use **Play** in a row to preview a clip. The shared player also supports pause, seeking, and preview volume.
- Select **X** to tag a trigger candidate and tint its row red. Then choose the relevant mod category.
- Sounds with mod coverage have red names. Click their grey **X** to queue or clear a removal candidate in the review. The export includes `tags.remove_coverage` for queued removals.
- Select **O** to tag a potential stand-in and tint its row green. Click the grey **O** on a stand-in to untag it. Choose a stand-in category if you want candidates in that category to use it. **Mute (default)** is available for every trigger candidate.
- Use **Export review** to save tagged sounds, categories, replacement choices, and linked events as JSON.

Coverage comes from the selected catalog. The configurable build supplies mute toggles and a **Screens** volume slider. The corresponding standalone archive uses a fixed mute. **Event only** identifies an existing control whose event has no linked installed clip in the SoundDB index.

## Saved data

On Linux, the default tag database is `~/.local/share/misophonia/audio-review/review.sqlite3`. The Rust version uses the same database and keeps all saved review tags and choices. The index cache defaults to `~/.cache/misophonia/audio-review/`. The Rust version reuses the existing archive index and creates its own preview cache on demand. Use `--data` and `--cache` to select other directories.

## Tests

```sh
cargo test --release --locked --manifest-path tools/audio_review/Cargo.toml
```
