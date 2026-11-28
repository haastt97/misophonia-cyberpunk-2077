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
- Use **Search categories…** next to the filter or above an assignment. Results show both names, such as **Body sounds / Breathing**. Click a result to select the pair, or press **Enter** to choose the first match. **Categories…** also has a search box for the category tree.
- Use **Play** in a row to preview a clip. The shared player also supports pause, seeking, and preview volume.
- Select **X** to tag a trigger candidate and tint its row red. Choose a category and then a subcategory in the details panel.
- Sounds with mod coverage have red names. Click their grey **X** to queue or clear a removal candidate in the review. The export includes `tags.remove_coverage` for queued removals.
- Select **O** to tag a potential stand-in and tint its row green. Click the grey **O** on a stand-in to untag it. Choose a stand-in category if you want candidates in that category to use it. **Mute (default)** is available for every trigger candidate.
- Shift-click to select a range. Ctrl-click (Cmd-click on macOS) to add or remove individual rows. The selection panel can tag or clear sounds, assign categories, and set a shared replacement. Play always previews a single sound.
- Use **Categories…** to add a subcategory under an existing or new category, rename categories, move subcategories, or remove them. Removing a category clears its review assignments. Reassign or clear any affected trigger tags before building.
- Use **Export review** to save tagged sounds, categories, replacement choices, and linked events as JSON.

Coverage comes from the selected catalog. The configurable build supplies mute toggles and a **Screens** volume slider. The corresponding standalone archive uses a fixed mute. **Event only** identifies an existing control whose event has no linked installed clip in the SoundDB index.

## Build a custom mod version

Choose **Build version…**, enter a name, select a save folder, and click **Build packages**. Audio Review creates a new folder containing the configurable PC ZIP, eligible dependency-free standalone `.archive` files, corresponding single-archive Nexus ZIPs, and `build-report.json`. The configurable ZIP uses the standard Misophonia dependencies and bundles audio for the stand-ins you selected. Its Mod Settings menu reflects the edited category names and controls.

The build applies decisions to named game events. Several clips can share one event, and sounds with no named event link need to be cleared from the build queue. A standalone archive can use a stand-in when the selected clip has an exact native event mapping. The report identifies options skipped for unavailable mappings and edited events shared by multiple clips. If individual **Screens** events change, the custom version uses an on/off switch for those events because the stock slider adjusts the whole Screens mixer.

## Saved data

Use **Project → Save project** to name your work and **Open project…** to switch between saved `.misophonia.json` files. Each project includes your trigger and stand-in tags, replacements, coverage removals, and category edits. **New project** starts with the base catalog and an empty review. Pending changes are saved before switching.

By default, Audio Review autosaves changed projects every minute. Use **Project → Settings…** to enter a different number of minutes. Named projects autosave to their own files. Untitled projects get recovery files under **Project → Open autosave…**. If you reopen an older project against a different mod catalog, the app displays both catalog versions.

The active workspace also persists as you edit and is reopened at startup. On Linux it defaults to `~/.local/share/misophonia/audio-review/review.sqlite3`, with untitled recovery files in the adjacent `autosaves/` folder. The index cache defaults to `~/.cache/misophonia/audio-review/`. Use `--data` and `--cache` to select other directories.

## Tests

```sh
cargo test --release --locked --manifest-path tools/audio_review/Cargo.toml
```
