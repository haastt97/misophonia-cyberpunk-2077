# Misophonia for Cyberpunk 2077

Misophonia is a PC mod for muting selected sounds in Cyberpunk 2077. The configurable build has 37 sound categories, section-wide switches, a global **All** switch, and a **Screens** slider for TV and advert audio. The standalone archives apply a fixed set of mutes with the game's built-in archive loader.

This repository starts with version **0.3.8**, built for Cyberpunk 2077 patch **2.5**. See the [changelog](CHANGELOG.md) for changes.

## Installation

Choose the configurable build or one standalone archive. Close the game before replacing an older Quiet Eating or Misophonia installation.

### Configurable build

1. Install the required mods below.
2. Install the main ZIP through Vortex and deploy it, or extract it into the Cyberpunk 2077 game directory. The scripts belong in `r6/scripts/Misophonia/`.
3. Launch the game and open **Mod Settings → Misophonia**. On a fresh install, the mutes start off and **Screens** starts at 100%.

#### Requirements

- [RED4ext](https://www.nexusmods.com/cyberpunk2077/mods/2380)
- [redscript](https://www.nexusmods.com/cyberpunk2077/mods/1511)
- [Codeware](https://www.nexusmods.com/cyberpunk2077/mods/7780)
- [AudioXL](https://www.nexusmods.com/cyberpunk2077/mods/33442) — tested with 0.11.0.
- [Mod Settings](https://www.nexusmods.com/cyberpunk2077/mods/4885) — tested with 0.2.35.
- ArchiveXL is a prerequisite of Mod Settings. [ArchiveXL](https://www.nexusmods.com/cyberpunk2077/mods/4198) · [Mod Settings requirements](https://www.nexusmods.com/cyberpunk2077/mods/4885?tab=requirements)

Redscript Configuration Framework provides optional access through **Configure Mods → Misophonia**. [Redscript Configuration Framework](https://www.nexusmods.com/cyberpunk2077/mods/30726)

### Standalone archives

Place one `.archive` in `Cyberpunk 2077/archive/pc/mod/` and start the game. The game loads these files without additional mods. Each archive replaces `base/sound/event/eventsmetadata.json`, so install one option at a time and check for conflicts with other mods serving that same resource. [Cyberpunk archive load order](https://wiki.redmodding.org/cyberpunk-2077-modding/for-mod-users/users-modding-cyberpunk-2077/load-order)

| Standalone option | Muted events in 0.3.8 |
| --- | ---: |
| All Covered Sounds | 630 |
| Eating and Drinking | 84 |
| Body Sounds | 126 |
| Repetitive Sounds | 92 |
| Environmental Sounds | 32 |
| Voice Sounds | 9 |
| Graphic Sound Effects | 44 |
| Screens | 243 identified TV and advert events |

## Audio Review

Use the desktop program to listen to sounds, track candidates for future updates, and pick stand-ins by category. [Audio Review setup](tools/audio_review/README.md)

## Building the downloads

The [runtime scripts](r6/scripts/Misophonia/) contain the Mod Settings UI and the catalog of sound events. The [standalone Screens list](data/screen-events.txt) contains the named TV and advert events used by the fixed archives. Files generated from game assets are built from a local copy of the game.

To create the configurable ZIP with Python 3, run:

```sh
python3 tools/build.py
```

To also build the standalone downloads, use WolvenKit to extract `base/sound/event/eventsmetadata.json` from `archive/pc/content/audio_1_general.archive` in your patch 2.5 game installation, and convert the extracted file to JSON. Then run the build with the converted metadata and the WolvenKit CLI available on your path. [Modding Events Metadata](https://wiki.redmodding.org/cyberpunk-2077-modding/modding-guides/sound/audio-modding-basics)

```sh
python3 tools/build.py --metadata /path/to/eventsmetadata.json.json
```

Use `--wolvenkit /path/to/WolvenKit.CLI` to supply a CLI executable explicitly. Outputs go to `dist/`, with raw archives and single-archive Nexus ZIPs in separate folders. The builder uses the category catalog from the runtime source so changes apply to both editions.
