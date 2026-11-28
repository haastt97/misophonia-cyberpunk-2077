"""Index and play local Cyberpunk 2077 audio without extracting entire archives."""

from __future__ import annotations

import ctypes
from dataclasses import asdict, dataclass
import hashlib
import io
import json
import os
from pathlib import Path, PureWindowsPath
import re
import shutil
import struct
import subprocess
from tempfile import NamedTemporaryFile, TemporaryDirectory
from threading import BoundedSemaphore, Lock
from typing import BinaryIO


# RED4 archive layout and Opus pack tables are described by WolvenKit's
# ArchiveReader.cs and OpusInfo.cs. Wwise DIDX rows are media ID, offset, size.
ARCHIVE_HEADER = struct.Struct("<4sIQI")
INDEX_HEADER = struct.Struct("<IIQIII")
ENTRY = struct.Struct("<QqIIIII20s")
SEGMENT = struct.Struct("<QII")
BANK_CHUNK = struct.Struct("<4sI")
BANK_MEDIA = struct.Struct("<III")
OPUS_PATH = PureWindowsPath(r"base\sound\soundbanks\sfx_container.opusinfo")
PACK_NAME = re.compile(r"sfx_container_(\d+)\.opuspak", re.IGNORECASE)
FILE_TYPES = (".wem", ".ogg", ".opus", ".wav", ".mp3", ".flac", ".bnk", ".opusinfo", ".opuspak")
CATALOG_SCHEMA = 1
CONVERSION_TIMEOUT = 180
MAX_EXPANDED_SEGMENT = 512 * 1024 * 1024


@dataclass(frozen=True)
class Segment:
    offset: int
    stored: int
    size: int


@dataclass(frozen=True)
class Source:
    archive: str
    offset: int
    stored: int
    size: int
    suffix: str
    start: int = 0
    length: int = 0


def resource_hash(path: str) -> int:
    value = 0xCBF29CE484222325
    for byte in path.encode("utf-8"):
        value = ((value ^ byte) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return value


def archive_entries(path: Path) -> dict[int, Segment]:
    size = path.stat().st_size
    with path.open("rb") as stream:
        header = stream.read(ARCHIVE_HEADER.size)
        if len(header) != ARCHIVE_HEADER.size:
            raise ValueError(f"Truncated archive header in {path.name}")
        magic, version, index_position, index_size = ARCHIVE_HEADER.unpack(header)
        if magic != b"RDAR" or version != 12:
            raise ValueError(f"Unsupported archive version in {path.name}")
        if index_position + index_size > size or index_size < INDEX_HEADER.size:
            raise ValueError(f"Invalid index range in {path.name}")
        stream.seek(index_position)
        index = stream.read(index_size)
    _, _, _, entries, segments, dependencies = INDEX_HEADER.unpack_from(index)
    segments_offset = INDEX_HEADER.size + entries * ENTRY.size
    required = segments_offset + segments * SEGMENT.size + dependencies * 8
    if required > len(index):
        raise ValueError(f"Truncated archive index in {path.name}")
    result = {}
    for entry_index in range(entries):
        offset = INDEX_HEADER.size + entry_index * ENTRY.size
        name_hash, _, _, start, end, *_ = ENTRY.unpack_from(index, offset)
        if start >= end or end > segments:
            raise ValueError(f"Invalid segment range for {name_hash} in {path.name}")
        segment = Segment(*SEGMENT.unpack_from(index, segments_offset + start * SEGMENT.size))
        if segment.offset + segment.stored > size:
            raise ValueError(f"Invalid segment data for {name_hash} in {path.name}")
        # Raw media files occupy one segment. Other archive entries, such as
        # voiceset metadata, can have buffers and are not playable files.
        if end == start + 1:
            result[name_hash] = segment
    return result


def opus_members(info: bytes) -> list[tuple[int, int, int, int]]:
    if len(info) < 20 or info[:4] != b"SND ":
        raise ValueError("Unrecognized OpusInfo header")
    count, = struct.unpack_from("<I", info, 12)
    if 20 + 20 * count > len(info):
        raise ValueError("Truncated OpusInfo index")
    hashes = struct.unpack_from(f"<{count}I", info, 20)
    packs = struct.unpack_from(f"<{count}H", info, 20 + count * 4)
    offsets = struct.unpack_from(f"<{count}I", info, 20 + count * 6)
    headers = struct.unpack_from(f"<{count}H", info, 20 + count * 10)
    lengths = struct.unpack_from(f"<{count}I", info, 20 + count * 12)
    return [
        (media, pack, offset + header, length - header)
        for media, pack, offset, header, length in zip(hashes, packs, offsets, headers, lengths)
        if length > header
    ]


def bank_members(stream: BinaryIO, base: int, size: int) -> list[tuple[int, int, int]]:
    chunks = {}
    cursor = 0
    while cursor + BANK_CHUNK.size <= size:
        stream.seek(base + cursor)
        header = stream.read(BANK_CHUNK.size)
        if len(header) != BANK_CHUNK.size:
            break
        tag, length = BANK_CHUNK.unpack(header)
        cursor += BANK_CHUNK.size
        if cursor + length > size:
            raise ValueError("Soundbank contains a truncated section")
        chunks[tag] = (cursor, length)
        cursor += length
    if b"DIDX" not in chunks or b"DATA" not in chunks:
        return []
    index, length = chunks[b"DIDX"]
    data, data_length = chunks[b"DATA"]
    if length % BANK_MEDIA.size:
        raise ValueError("Soundbank media index is malformed")
    stream.seek(base + index)
    members = list(BANK_MEDIA.iter_unpack(stream.read(length)))
    result = []
    for media, offset, length in members:
        if length < 12 or offset + length > data_length:
            continue
        # Some banks include only a prefetch prefix for a separately stored WEM.
        stream.seek(base + data + offset)
        header = stream.read(12)
        if header[:4] == b"RIFF" and header[8:12] == b"WAVE":
            media_size = int.from_bytes(header[4:8], "little") + 8
            if media_size <= length:
                result.append((media, data + offset, media_size))
    return result


def audio_suffix(header: bytes) -> str:
    if header.startswith(b"RIFF") and header[8:12] == b"WAVE":
        return ".wem"
    if header.startswith(b"BKHD"):
        return ".bnk"
    if header.startswith(b"OggS"):
        return ".ogg"
    if header.startswith(b"SND "):
        return ".opusinfo"
    if header.startswith(b"fLaC"):
        return ".flac"
    return ""


def executable(argument: Path | None, name: str) -> Path | None:
    if argument is not None:
        path = Path(argument).expanduser().resolve()
        if not path.is_file() or not os.access(path, os.X_OK):
            raise ValueError(f"{name} is missing or cannot be executed at {path}")
        return path
    found = shutil.which(name)
    return Path(found).resolve() if found else None


class GameAudio:
    """Read-only access to vanilla archives and a local, disposable preview cache."""

    def __init__(
        self,
        game_dir: Path,
        cache_dir: Path,
        wolvenkit: Path | None = None,
        decoder: Path | None = None,
    ):
        self.game_dir = Path(game_dir).expanduser().resolve()
        self.cache_dir = Path(cache_dir).expanduser().resolve()
        self.cache_dir.mkdir(parents=True, exist_ok=True)
        self.wolvenkit = executable(wolvenkit, "WolvenKit.CLI")
        self.decoder = executable(decoder, "vgmstream-cli")
        self.ffmpeg = executable(None, "ffmpeg")
        self.warnings: list[str] = []
        self._decompress = None
        self._sources: dict[str, Source] = {}
        self._archives: dict[str, Path] = {}
        self._preview_locks = tuple(Lock() for _ in range(32))
        self._conversion_slots = BoundedSemaphore(2)

    def build(self) -> list[dict]:
        archive_dir = self.game_dir / "archive" / "pc"
        paths = sorted(
            path
            for region in ("content", "ep1")
            for path in (archive_dir / region).glob("*.archive")
            if path.name.startswith("audio_")
            or (path.name.startswith("lang_") and path.name.endswith("_voice.archive"))
        )
        if not paths:
            raise ValueError(f"No Cyberpunk 2077 audio archives found in {archive_dir}")
        self._archives = {path.relative_to(archive_dir).as_posix(): path for path in paths}
        fingerprints = {
            name: [stat.st_size, stat.st_mtime_ns]
            for name, path in self._archives.items()
            for stat in [path.stat()]
        }
        fingerprint = [CATALOG_SCHEMA, str(self.game_dir), fingerprints]
        self._preview_dir = self.cache_dir / "previews" / hashlib.sha256(
            json.dumps(fingerprint, sort_keys=True).encode("utf-8")
        ).hexdigest()[:16]
        self._preview_dir.mkdir(parents=True, exist_ok=True)
        index_file = self.cache_dir / "assets.json"
        try:
            cached = json.loads(index_file.read_bytes())
            if cached["fingerprint"] == fingerprint:
                self.warnings = cached["warnings"]
                self._sources = {
                    row_id: Source(**source) for row_id, source in cached["sources"].items()
                }
                return cached["rows"]
        except (OSError, ValueError, KeyError, TypeError):
            pass
        if self.wolvenkit is None:
            raise ValueError("Set --wolvenkit to WolvenKit.CLI to index sound file names.")

        rows = self._scan()
        data = {
            "fingerprint": fingerprint,
            "warnings": self.warnings,
            "sources": {row_id: asdict(source) for row_id, source in self._sources.items()},
            "rows": rows,
        }
        with NamedTemporaryFile(mode="w", dir=self.cache_dir, suffix=".json", delete=False) as out:
            temp = Path(out.name)
            try:
                json.dump(data, out, ensure_ascii=False, separators=(",", ":"))
                out.close()
                temp.replace(index_file)
            finally:
                temp.unlink(missing_ok=True)
        return rows

    def _paths(self, path: Path) -> dict[int, PureWindowsPath]:
        env = os.environ.copy()
        local_dotnet = self.wolvenkit.parent.parent / "dotnet"
        if "DOTNET_ROOT" not in env and (local_dotnet / "shared").is_dir():
            env["DOTNET_ROOT"] = str(local_dotnet)
        result = subprocess.run(
            [
                self.wolvenkit, "archive", path, "--list", "--regex",
                r"(?i)\.(wem|ogg|opus|wav|mp3|flac|bnk|opusinfo|opuspak)$",
            ],
            env=env,
            capture_output=True,
            text=True,
            errors="replace",
            timeout=CONVERSION_TIMEOUT,
        )
        if result.returncode or "[ERROR]" in result.stdout or "Exception" in result.stdout:
            message = (result.stderr or result.stdout).strip().splitlines()
            raise RuntimeError(f"WolvenKit could not list {path.name}: {' '.join(message[-4:])}")
        paths = {}
        for line in result.stdout.splitlines():
            name = line.strip().replace("/", "\\")
            file = PureWindowsPath(name)
            if file.suffix.lower() in FILE_TYPES and "\\" in name and not file.drive:
                paths[resource_hash(name)] = file
                paths.setdefault(resource_hash(name.lower()), file)
        return paths

    def _scan(self) -> list[dict]:
        rows = []
        self.warnings = []
        self._sources = {}
        infos = []
        packs = {}
        banks = []
        for archive, path in self._archives.items():
            entries = archive_entries(path)
            paths = self._paths(path)
            with path.open("rb") as stream:
                for asset_hash, segment in sorted(entries.items(), key=lambda entry: entry[1].offset):
                    file = paths.get(asset_hash)
                    suffix = file.suffix.lower() if file else ""
                    if not suffix:
                        suffix = self._guess_suffix(stream, segment)
                    if suffix == ".opusinfo":
                        infos.append((archive, self._read_segment(stream, segment)))
                    elif suffix == ".opuspak":
                        match = PACK_NAME.fullmatch(file.name) if file else None
                        if match:
                            packs[archive, int(match.group(1))] = segment
                    elif suffix == ".bnk":
                        banks.append((archive, asset_hash, segment, file))
                    elif suffix:
                        self._add_file(rows, archive, asset_hash, segment, file, suffix)

        for archive, asset_hash, segment, file in banks:
            with self._archives[archive].open("rb") as stream:
                if segment.stored == segment.size:
                    members = bank_members(stream, segment.offset, segment.size)
                else:
                    members = bank_members(io.BytesIO(self._read_segment(stream, segment)), 0, segment.size)
            for media, start, length in members:
                name = file.name if file else f"{asset_hash:016x}.bnk"
                self._add(
                    rows, f"b-{asset_hash:016x}-{media}",
                    f"{name.removesuffix('.bnk')} · {media}", "sfx", f"Embedded in {name}",
                    Source(archive, segment.offset, segment.stored, segment.size, ".wem", start, length),
                    file, media,
                )

        by_region = {}
        for archive, pack in packs:
            by_region.setdefault((archive.split("/", 1)[0], pack), archive)
        missing_packs = set()
        for archive, info in infos:
            region = archive.split("/", 1)[0]
            for media, pack, start, length in opus_members(info):
                origin = archive if (archive, pack) in packs else by_region.get((region, pack))
                origin = origin or by_region.get(("content", pack))
                segment = packs.get((origin, pack))
                if segment is None or start + length > segment.size:
                    missing_packs.add((archive, pack))
                    continue
                self._add(
                    rows, f"o-{media}", str(media), "sfx", f"Opus pack {pack} · {media}",
                    Source(origin, segment.offset, segment.stored, segment.size, ".ogg", start, length),
                    OPUS_PATH.with_name(f"sfx_container_{pack}.opuspak"), media,
                )
        if missing_packs:
            self.warnings.append(
                f"{len(missing_packs)} Opus pack references are absent or out of bounds in this installation."
            )
        if not infos:
            self.warnings.append("No packed sound effect index was found.")
        if not rows:
            raise ValueError("No playable sound files were found in the installed audio archives.")
        return rows

    def _add_file(
        self, rows: list[dict], archive: str, asset_hash: int,
        segment: Segment, file: PureWindowsPath | None, suffix: str,
    ) -> None:
        name = file.stem if file else f"{asset_hash:016x}"
        media = int(name) if name.isascii() and name.isdecimal() else None
        if archive.endswith("_voice.archive") or (file and "\\vo\\" in str(file).lower()):
            kind = "voice"
        elif file and any(part.lower() in {"music", "radio"} for part in file.parts):
            kind = "music"
        else:
            kind = "sound"
        self._add(
            rows, f"f-{asset_hash:016x}", name, kind, archive,
            Source(archive, segment.offset, segment.stored, segment.size, suffix), file, media,
        )

    def _add(
        self, rows: list[dict], key: str, name: str, kind: str, detail: str, source: Source,
        file: PureWindowsPath | None, media: int | None,
    ) -> None:
        archive_id = hashlib.sha256(source.archive.encode("utf-8")).hexdigest()[:12]
        row_id = f"{key}-{archive_id}"
        if row_id in self._sources:
            if self._sources[row_id] == source:
                return
            row_id = f"{row_id}-{source.start:x}"
        if row_id in self._sources:
            raise ValueError(f"Conflicting audio ID for {name} in {source.archive}")
        row = {"id": row_id, "name": name, "kind": kind, "detail": detail, "archive": source.archive}
        if file:
            row["path"] = file.as_posix()
        if media is not None:
            row["media_id"] = media
        rows.append(row)
        self._sources[row_id] = source

    def _guess_suffix(self, stream: BinaryIO, segment: Segment) -> str:
        if segment.stored == segment.size:
            stream.seek(segment.offset)
            return audio_suffix(stream.read(min(segment.size, 12)))
        return audio_suffix(self._read_segment(stream, segment)[:12])

    def _read_segment(self, stream: BinaryIO, segment: Segment) -> bytes:
        if segment.size > MAX_EXPANDED_SEGMENT:
            raise ValueError(f"Audio archive segment is too large to read ({segment.size} bytes)")
        stream.seek(segment.offset)
        data = stream.read(segment.stored)
        if len(data) != segment.stored:
            raise ValueError("Audio archive changed or a segment is truncated")
        if segment.stored == segment.size:
            return data
        if data[:4] != b"KARK" or int.from_bytes(data[4:8], "little") != segment.size:
            raise ValueError("Unrecognized compressed audio archive segment")
        if self._decompress is None:
            name = {"nt": "kraken.dll"}.get(os.name, "libkraken.so")
            if shutil.which("uname") and os.uname().sysname == "Darwin":
                name = "libkraken.dylib"
            if self.wolvenkit is None:
                raise ValueError("Set --wolvenkit to preview compressed archive entries.")
            try:
                library = ctypes.CDLL(str(self.wolvenkit.parent / name))
                decompress = library.Kraken_Decompress
            except (OSError, AttributeError) as exc:
                raise ValueError(f"Cannot load WolvenKit's {name}: {exc}") from exc
            decompress.argtypes = [ctypes.c_char_p, ctypes.c_size_t, ctypes.c_void_p, ctypes.c_size_t]
            decompress.restype = ctypes.c_int
            self._decompress = decompress
        output = ctypes.create_string_buffer(segment.size + 64)
        if self._decompress(data[8:], len(data) - 8, output, segment.size) != segment.size:
            raise ValueError("WolvenKit could not decompress an audio archive segment")
        return output.raw[:segment.size]

    def preview(self, row_id: str) -> Path:
        source = self._sources.get(row_id)
        if source is None:
            raise KeyError(f"No indexed audio file named {row_id}")
        native_formats = {".ogg", ".opus", ".mp3", ".flac", ".wav"}
        if source.suffix in native_formats:
            suffix = source.suffix
        else:
            suffix = ".ogg" if self.ffmpeg else ".wav"
        target = self._preview_dir / f"{row_id}{suffix}"
        lock = self._preview_locks[hash(row_id) % len(self._preview_locks)]
        with lock:
            if target.is_file():
                return target
            with self._conversion_slots, TemporaryDirectory(dir=self.cache_dir, prefix="preview-") as work:
                work = Path(work)
                encoded = work / ("sound" + source.suffix)
                with self._archives[source.archive].open("rb") as stream:
                    if source.stored == source.size and source.length:
                        stream.seek(source.offset + source.start)
                        data = stream.read(source.length)
                    else:
                        data = self._read_segment(stream, Segment(source.offset, source.stored, source.size))
                        if source.length:
                            data = data[source.start:source.start + source.length]
                if not data or (source.length and len(data) != source.length):
                    raise ValueError("Indexed audio data is missing or truncated")
                if source.suffix in {".ogg", ".opus"} and not data.startswith(b"OggS"):
                    raise ValueError("The indexed Opus stream does not contain an Ogg header")
                encoded.write_bytes(data)
                if source.suffix in native_formats:
                    encoded.replace(target)
                else:
                    self._decode_wem(work, encoded, target)
        return target

    def _decode_wem(self, work: Path, encoded: Path, target: Path) -> None:
        if self.decoder is None:
            raise ValueError("Set --decoder to vgmstream-cli to preview Wwise WEM audio.")
        wav = work / "decoded.wav"
        try:
            self._run([self.decoder, "-i", "-D", "2", "-o", wav, encoded], wav, "vgmstream")
            if self.ffmpeg:
                ogg = work / "preview.ogg"
                self._run(
                    [self.ffmpeg, "-v", "error", "-nostdin", "-i", wav,
                     "-c:a", "libopus", "-b:a", "112k", ogg],
                    ogg, "FFmpeg",
                )
                ogg.replace(target)
            else:
                wav.replace(target)
        except subprocess.TimeoutExpired as exc:
            raise RuntimeError("Audio conversion timed out") from exc

    @staticmethod
    def _run(command: list, output: Path, tool: str) -> None:
        result = subprocess.run(
            command, capture_output=True, text=True, errors="replace", timeout=CONVERSION_TIMEOUT,
        )
        if result.returncode or not output.is_file() or output.stat().st_size <= 44:
            message = (result.stderr or result.stdout).strip().splitlines()
            raise RuntimeError(f"{tool} could not decode this sound: {' '.join(message[-4:])}")
