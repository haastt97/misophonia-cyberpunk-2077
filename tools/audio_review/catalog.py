"""Read the Misophonia catalog and relate named audio events to local media."""

from collections import defaultdict
import hashlib
import json
from pathlib import Path
import re
from tempfile import NamedTemporaryFile
import urllib.request

EVENTS_URL = "https://sounddb.zhincore.eu/v1/data/events.json"
SCRIPTS = Path("r6/scripts/Misophonia")
SETTING = re.compile(
    r'((?:^[ \t]*@runtimeProperty\([^\n]+\)\n)+)'
    r'[ \t]*public let (\w+): (?:Bool|Int32) = [^;]+;',
    re.MULTILINE,
)
CATEGORY = re.compile(
    r'entry.section = n"(\w+)";\s+entry.setting = n"(\w+)";\s+entry.events = \[([^]]+)\];'
)


def read_mod(root):
    version = (root / "VERSION").read_text(encoding="utf-8").strip()
    config = (root / SCRIPTS / "Config.reds").read_text(encoding="utf-8")
    catalog = (root / SCRIPTS / "Catalog.reds").read_text(encoding="utf-8")
    settings = {}
    for annotations, key in SETTING.findall(config):
        settings[key] = dict(re.findall(r'@runtimeProperty\("ModSettings\.([\w.]+)", "([^"]+)"\)', annotations))

    categories = []
    coverage = defaultdict(list)
    entries = CATEGORY.findall(catalog)
    if not entries or len(entries) != catalog.count("ArrayPush(result, entry);"):
        raise ValueError("Could not read every sound category in Catalog.reds.")

    for parent, key, event_list in entries:
        setting = settings[key]
        if settings[parent]["category"] != setting["category"]:
            raise ValueError(f"Setting {key} is assigned to the wrong category.")
        categories.append({
            "id": key, "section": setting["category"], "name": setting["displayName"], "mode": "mute",
        })
        for event in re.findall(r'n"([^"]+)"', event_list):
            coverage[event].extend(((key, "mute", "Configurable"), (key, "mute", "Standalone")))

    screens = settings["screenPercent"]
    categories.insert(0, {
        "id": "screenPercent", "section": screens["category"], "name": screens["displayName"], "mode": "volume",
    })
    for event in (root / "data/screen-events.txt").read_text(encoding="utf-8").splitlines():
        if event.strip():
            coverage[event].extend((("screenPercent", "volume", "Configurable"), ("screenPercent", "mute", "Standalone")))
    return version, categories, coverage


def event_media(index_path, cache_dir):
    if index_path is None:
        index_path = cache_dir / "events.json"
        if not index_path.exists():
            print("Downloading Cyberpunk SoundDB event-to-media metadata for the first run.", flush=True)
            request = urllib.request.Request(EVENTS_URL, headers={
                "User-Agent": "Misophonia-Audio-Review/1.0",
                "Accept": "application/json",
            })
            with urllib.request.urlopen(request, timeout=30) as response:
                payload = response.read(32 * 1024 * 1024 + 1)
            if len(payload) > 32 * 1024 * 1024:
                raise ValueError("SoundDB event index exceeds 32 MiB.")
            index = decode_index(payload)
            with NamedTemporaryFile(dir=cache_dir, delete=False) as temporary:
                temporary.write(payload)
            Path(temporary.name).replace(index_path)
            return index
    return decode_index(index_path.read_bytes())


def decode_index(payload):
    data = json.loads(payload)
    if not isinstance(data, dict) or not data:
        raise ValueError("Expected a SoundDB events.json object with named events.")
    index = defaultdict(set)
    for event, value in data.items():
        if isinstance(value, dict):
            for media in (value.get("sounds") or {}).get("PLAY", []):
                index[int(media)].add(event)
    if not index:
        raise ValueError("SoundDB index has no playable event-to-media links.")
    return index


def coverage_for(events, coverage):
    groups = defaultdict(list)
    for event in events:
        for key in coverage.get(event, ()):
            groups[key].append(event)
    return [
        {"category": key, "mode": mode, "edition": edition, "events": names}
        for (key, mode, edition), names in sorted(groups.items())
    ]


def sound_rows(assets, media_events, coverage):
    linked = set()
    for base in assets:
        events = sorted(media_events.get(base.get("media_id"), ()))
        linked.update(events)
        row = {**base, "events": events, "coverage": coverage_for(events, coverage), "playable": True}
        if events and row.get("media_id") is not None:
            # Keep the physical filename searchable while showing a descriptive event name.
            preferred = next((event for event in events if event in coverage), events[0])
            row["name"] = f"{preferred} · {base['name']}"
        if row["kind"] == "sound" and any(event.startswith("mus_") for event in events):
            row["kind"] = "music"
        yield row

    for event in sorted(coverage.keys() - linked):
        yield {
            "id": "event-" + hashlib.sha256(event.encode()).hexdigest()[:24],
            "kind": "event",
            "name": event,
            "detail": "No installed media linked by the sound index",
            "events": [event],
            "coverage": coverage_for([event], coverage),
            "playable": False,
        }
