# Misophonia Audio Review

Audio Review is a native desktop program for auditioning game audio and planning changes to Misophonia. It reads the installed Cyberpunk 2077 archives and the selected Misophonia source checkout.

## Run

Requirements are Python 3.10 or later, Qt for Python, the local game installation, [WolvenKit CLI](https://github.com/WolvenKit/WolvenKit/releases), and [vgmstream-cli](https://github.com/vgmstream/vgmstream/releases). Make the executables available on your path or pass their full paths using `--wolvenkit` and `--decoder`. A standalone WolvenKit install may also need `DOTNET_ROOT`.

From the Misophonia source checkout, install the Qt dependency in a virtual environment and launch the window.

```sh
uv venv .venv-audio-review
uv pip install --python .venv-audio-review/bin/python -r tools/audio_review/requirements.txt
.venv-audio-review/bin/python tools/audio_review/desktop.py --game "/path/to/Cyberpunk 2077"
```

The first run indexes the archives and caches the public event-to-media metadata from [Cyberpunk SoundDB](https://sounddb.zhincore.eu/). Supply `--event-index /path/to/events.json` to use a local copy. Use `--catalog /path/to/Misophonia` to show coverage from a different source checkout. Restart the tool after changing the game files or catalog.

The library includes packed sound effects, complete embedded soundbank clips, streamed audio, and installed voice languages from the base game and Phantom Liberty. The previews play individual recorded files. Game events can select several files, mix them, or change their playback in the game.

## Review sounds

- Search by filename, media ID, or linked event name. Use the filters to narrow by source, review status, and mod category.
- Use **Play** to decode a preview on demand. Pause it from the row or the shared player.
- Click the red **X** to mark a future mute or replacement candidate. Then choose a mod category.
- Click the green **O** to mark a potential stand-in and assign its stand-in category. Candidates can select a different sound tagged **O** in their own category. **Mute** is the default.
- **Export review** saves your tagged files, candidate categories, replacements, and linked events as JSON. It contains planning data for future mod updates.

Coverage is derived from the selected source. A configurable mute is available through its switch and starts off on a fresh install. **Screens** is a volume slider in the configurable build and a fixed mute in applicable standalone archives. Coverage follows known event-to-media links. An **Event only** row records an existing control where the index has no linked installed file.

## Local files

On Linux, review tags are saved in `~/.local/share/misophonia/audio-review/review.sqlite3` by default. Copy this database for a backup while the tool is stopped. The desktop and earlier browser versions use the same tag database. The preview and index cache defaults to `~/.cache/misophonia/audio-review/`. Use `--data` and `--cache` to choose other locations. Preview audio is read from your installed game and stored locally.
