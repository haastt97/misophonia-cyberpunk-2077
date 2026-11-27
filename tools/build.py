#!/usr/bin/env python3
"""Build Misophonia's main ZIP and optional standalone archives."""

import argparse
import json
import re
import shutil
import subprocess
from pathlib import Path
from tempfile import TemporaryDirectory
from zipfile import ZIP_DEFLATED, ZipFile

ROOT = Path(__file__).resolve().parents[1]
SCRIPT_DIR = Path("r6/scripts/Misophonia")
METADATA = Path("base/sound/event/eventsmetadata.json")
SECTION_OPTIONS = {
    "allFood": "Eating-and-Drinking",
    "allBody": "Body-Sounds",
    "allRepetitive": "Repetitive-Sounds",
    "allEnvironmental": "Environmental-Sounds",
    "allVoice": "Voice-Sounds",
    "allGraphic": "Graphic-Sound-Effects",
}


def standalone_options():
    catalog = (ROOT / SCRIPT_DIR / "Catalog.reds").read_text(encoding="utf-8")
    entries = re.findall(
        r'entry.section = n"(\w+)";\s+entry.setting = n"\w+";\s+entry.events = \[([^]]+)\];',
        catalog,
    )
    if not entries or len(entries) != catalog.count("ArrayPush(result, entry);"):
        raise ValueError("Could not read every category in Catalog.reds.")

    sections = {name: set() for name in SECTION_OPTIONS.values()}
    selected = set()
    for section, event_block in entries:
        if section not in SECTION_OPTIONS:
            raise ValueError(f"No standalone option is defined for {section}.")
        events = re.findall(r'n"([^"]+)"', event_block)
        if not events or len(events) != len(set(events)) or selected.intersection(events):
            raise ValueError(f"Empty or duplicate sound entries under {section}.")
        sections[SECTION_OPTIONS[section]].update(events)
        selected.update(events)

    screens = (ROOT / "data/screen-events.txt").read_text(encoding="utf-8").splitlines()
    if not screens or "" in screens or len(screens) != len(set(screens)) or selected.intersection(screens):
        raise ValueError("Empty, duplicate, or overlapping entries in screen-events.txt.")
    return {"All-Covered-Sounds": selected.union(screens), **sections, "Screens": set(screens)}


def pack_zip(destination, files):
    with ZipFile(destination, "w", ZIP_DEFLATED, compresslevel=9) as package:
        for file, archive_path in files:
            package.write(file, archive_path)
    with ZipFile(destination) as package:
        if package.testzip() is not None:
            raise ValueError(f"ZIP integrity check failed for {destination}.")


def run_wolvenkit(executable, *args):
    result = subprocess.run([executable, *args], stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    if result.returncode or "Exception" in result.stdout or "[ERROR]" in result.stdout:
        raise RuntimeError(f"WolvenKit failed during {args[0]}.\n{result.stdout}")


def mute_events(resource, names):
    events = resource["Data"]["RootChunk"]["root"]["Data"]["events"]
    muted = set()
    for event in events:
        name = event["redId"]["$value"]
        if name in names:
            if event["wwiseId"] <= 0:
                raise ValueError(f"{name} has no original Wwise event ID. Use vanilla game metadata.")
            event["wwiseId"] = 0
            muted.add(name)
    if names - muted:
        raise ValueError(f"Source metadata is missing these events: {', '.join(sorted(names - muted))}.")


def build_standalone(executable, metadata, files, root, name, names):
    resource = json.loads(metadata)
    mute_events(resource, names)
    with TemporaryDirectory() as temp:
        job = Path(temp)
        original = job / "eventsmetadata.json.json"
        original.write_text(json.dumps(resource, ensure_ascii=False, separators=(",", ":")), encoding="utf-8")
        input_dir = job / name
        target = input_dir / METADATA
        target.parent.mkdir(parents=True)
        run_wolvenkit(executable, "cr2w", original, "-d", "-o", target.parent)
        run_wolvenkit(executable, "pack", input_dir, "-o", job)

        built = job / f"{name}.archive"
        extraction = job / "extracted"
        extraction.mkdir()
        run_wolvenkit(executable, "unbundle", built, "-o", extraction)
        unpacked = extraction / METADATA
        extracted_files = {file for file in extraction.rglob("*") if file.is_file()}
        if extracted_files != {unpacked}:
            raise ValueError(f"{name} should contain only {METADATA}.")
        run_wolvenkit(executable, "cr2w", unpacked, "-s")
        round_trip = json.loads(unpacked.with_suffix(".json.json").read_bytes())
        if round_trip["Data"] != resource["Data"]:
            raise ValueError(f"Archive round-trip check failed for {name}.")

        archive = files / built.name
        shutil.copy2(built, archive)
        pack_zip(root / f"{name}.zip", [(archive, f"archive/pc/mod/{archive.name}")])
    print(f"{name}: verified {len(names)} muted events.", flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--metadata", type=Path, help="Vanilla eventsmetadata.json.json exported by WolvenKit")
    parser.add_argument("--wolvenkit", default="WolvenKit.CLI", help="Path to the WolvenKit CLI executable")
    parser.add_argument("--output", type=Path, default=ROOT / "dist", help="Directory for generated downloads")
    args = parser.parse_args()

    version = (ROOT / "VERSION").read_text().strip()
    if not re.fullmatch(r"\d+\.\d+\.\d+", version):
        parser.error("VERSION should be in major.minor.patch format.")
    if args.metadata and not args.metadata.is_file():
        parser.error(f"No metadata file at {args.metadata}.")
    if args.metadata and shutil.which(args.wolvenkit) is None:
        parser.error(f"Cannot find {args.wolvenkit}. Set --wolvenkit to a path or add it to PATH.")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)

    try:
        options = standalone_options()
        scripts = sorted((ROOT / SCRIPT_DIR).glob("*.reds"))
        prefix = f"Misophonia-{version}"
        main_zip = output / f"{prefix}-Mod-Settings-PC.zip"
        pack_zip(main_zip, [(file, file.relative_to(ROOT).as_posix()) for file in scripts])
        print(main_zip, flush=True)
        if args.metadata:
            metadata = args.metadata.read_bytes()
            raw = output / f"{prefix}-Standalone"
            optional = output / f"{prefix}-Nexus-Optional"
            raw.mkdir(exist_ok=True)
            optional.mkdir(exist_ok=True)
            for label, names in options.items():
                build_standalone(args.wolvenkit, metadata, raw, optional, f"{prefix}-{label}", names)
    except (OSError, KeyError, ValueError, RuntimeError) as exc:
        parser.exit(1, f"Build failed: {exc}\n")


if __name__ == "__main__":
    main()
