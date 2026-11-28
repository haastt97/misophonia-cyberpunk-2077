#!/usr/bin/env python3
"""Run the loopback-only Misophonia audio review tool."""

import argparse
from collections import Counter
from contextlib import contextmanager
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import mimetypes
import os
from pathlib import Path
import re
import sqlite3
import subprocess
from urllib.parse import parse_qs, unquote, urlsplit

from catalog import event_media, read_mod, sound_rows

HERE = Path(__file__).resolve().parent
KINDS = {
    "sfx": "Sound effects", "sound": "Streamed sounds", "voice": "Voice-over",
    "music": "Music", "event": "Event only",
}
FILTERS = {"all", "candidate", "stand-in", "covered", "untagged"}
EMPTY_TAGS = {
    "candidate": False, "category": None, "stand_in": False, "stand_in_category": None, "replacement": None,
}
TAG_KEYS = tuple(EMPTY_TAGS)


def normalized(text):
    return re.sub(r"[_/\\-]+", " ", text.casefold())


class ReviewStore:
    def __init__(self, path):
        path.parent.mkdir(parents=True, exist_ok=True)
        self.path = path
        with self.connection() as db:
            db.execute("PRAGMA journal_mode=WAL")
            db.execute("""
                CREATE TABLE IF NOT EXISTS tags (
                    id TEXT PRIMARY KEY, candidate INTEGER NOT NULL, category TEXT,
                    stand_in INTEGER NOT NULL, stand_in_category TEXT, replacement TEXT
                )
            """)

    @contextmanager
    def connection(self):
        db = sqlite3.connect(self.path)
        db.row_factory = sqlite3.Row
        try:
            with db:
                yield db
        finally:
            db.close()

    def entries(self):
        with self.connection() as db:
            return {
                row["id"]: {
                    key: bool(row[key]) if key in ("candidate", "stand_in") else row[key]
                    for key in TAG_KEYS
                }
                for row in db.execute("SELECT * FROM tags")
            }

    def save(self, key, tags):
        with self.connection() as db:
            db.execute("BEGIN IMMEDIATE")
            if replacement := tags["replacement"]:
                donor = db.execute(
                    "SELECT stand_in, stand_in_category FROM tags WHERE id = ?", (replacement,),
                ).fetchone()
                if donor is None or not donor["stand_in"] or donor["stand_in_category"] != tags["category"]:
                    raise ValueError("Choose a stand-in from the same category.")
            if tags["candidate"] or tags["stand_in"]:
                db.execute("""
                    INSERT INTO tags VALUES (?, ?, ?, ?, ?, ?)
                    ON CONFLICT(id) DO UPDATE SET candidate = excluded.candidate, category = excluded.category,
                    stand_in = excluded.stand_in, stand_in_category = excluded.stand_in_category,
                    replacement = excluded.replacement
                """, (key, *(tags[field] for field in TAG_KEYS)))
            else:
                db.execute("DELETE FROM tags WHERE id = ?", (key,))

            donor_category = tags["stand_in_category"] if tags["stand_in"] else None
            result = db.execute(
                "UPDATE tags SET replacement = NULL WHERE replacement = ? AND category IS NOT ?",
                (key, donor_category),
            )
            return result.rowcount


class Review:
    def __init__(self, game, assets, catalog_root, media_events, store):
        self.game = game
        self.store = store
        self.version, self.categories, coverage = read_mod(catalog_root)
        self.category_ids = {category["id"] for category in self.categories}
        rows = sorted(
            sound_rows(assets, media_events, coverage),
            key=lambda row: (not row["events"], row["name"].casefold(), row["id"]),
        )
        self.rows = {row["id"]: row for row in rows}
        if len(rows) != len(self.rows):
            raise ValueError("The game audio index contains repeated sound IDs.")
        self.search = [
            (row, normalized(" ".join([
                row["name"], row["detail"], row["id"], row.get("path", ""), *row["events"],
            ])))
            for row in rows
        ]
        kinds = Counter(row["kind"] for row in rows)
        self.info = {
            "version": self.version,
            "categories": self.categories,
            "kinds": [{"id": key, "name": KINDS.get(key, key.title())} for key in sorted(kinds)],
            "stats": {"sounds": sum(row["playable"] for row in rows), "events": len({e for row in rows for e in row["events"]})},
            "warnings": getattr(game, "warnings", []),
        }

    def list_sounds(self, query, tag_filter, category, kind, offset, limit):
        if tag_filter not in FILTERS or category and category not in self.category_ids:
            raise ValueError("Unknown review filter.")
        words = normalized(query).split()
        tagged = self.store.entries()
        items = []
        total = 0
        for row, searchable in self.search:
            if kind and row["kind"] != kind or any(word not in searchable for word in words):
                continue
            tags = tagged.get(row["id"], EMPTY_TAGS)
            if tag_filter == "candidate" and not tags["candidate"]:
                continue
            if tag_filter == "stand-in" and not tags["stand_in"]:
                continue
            if tag_filter == "covered" and not row["coverage"]:
                continue
            if tag_filter == "untagged" and (tags["candidate"] or tags["stand_in"]):
                continue
            if category and category not in (tags["category"], tags["stand_in_category"]) and not any(
                coverage["category"] == category for coverage in row["coverage"]
            ):
                continue
            if offset <= total < offset + limit:
                items.append({**row, "tags": tags})
            total += 1
        return {"total": total, "items": items}

    def stand_ins(self, category, excluded):
        return [
            {"id": key, "name": self.rows[key]["name"]}
            for key, tags in self.store.entries().items()
            if category and key != excluded and key in self.rows and self.rows[key]["playable"]
            and tags["stand_in"] and tags["stand_in_category"] == category
        ]

    def save(self, key, tags):
        if key not in self.rows:
            raise KeyError(key)
        if not isinstance(tags, dict) or set(tags) != set(TAG_KEYS):
            raise ValueError("Provide all five tag fields.")
        if type(tags["candidate"]) is not bool or type(tags["stand_in"]) is not bool:
            raise ValueError("Candidate and stand-in tags must be true or false.")
        for field, enabled in (("category", "candidate"), ("stand_in_category", "stand_in")):
            value = tags[field]
            if value is not None and (not isinstance(value, str) or value not in self.category_ids or not tags[enabled]):
                raise ValueError(f"Choose a valid category for {field}.")
        if tags["stand_in"] and not self.rows[key]["playable"]:
            raise ValueError("Only installed audio files can be stand-ins.")
        replacement = tags["replacement"]
        if replacement is not None and (
            not isinstance(replacement, str) or replacement == key or replacement not in self.rows
            or not self.rows[replacement]["playable"] or not tags["candidate"] or tags["category"] is None
        ):
            raise ValueError("Choose a different playable sound tagged as a stand-in for this category.")
        return {"tags": tags, "reset_count": self.store.save(key, tags)}

    def export(self):
        return {
            "schema_version": 1,
            "mod_version": self.version,
            "categories": self.categories,
            "sounds": [
                {**self.rows.get(key, {"id": key, "unavailable": True}), "tags": tags}
                for key, tags in sorted(self.store.entries().items())
            ],
        }


class ReviewHandler(BaseHTTPRequestHandler):
    def permitted(self):
        allowed = f"127.0.0.1:{self.server.server_port}"
        opening_tool = self.command == "GET" and self.path in ("/", "/index.html") and (
            self.headers.get("Sec-Fetch-Mode") == "navigate"
        )
        if (
            self.headers.get("Host") != allowed
            or self.headers.get("Origin") not in (None, f"http://{allowed}")
            or (not opening_tool and self.headers.get("Sec-Fetch-Site") not in (None, "none", "same-origin"))
        ):
            self.send_json({"error": "Open the tool at its printed loopback address."}, HTTPStatus.FORBIDDEN)
            return False
        return True

    def send_json(self, value, status=HTTPStatus.OK, filename=None):
        data = json.dumps(value, ensure_ascii=False).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Cross-Origin-Resource-Policy", "same-origin")
        self.send_header("Cache-Control", "no-store")
        if filename:
            self.send_header("Content-Disposition", f'attachment; filename="{filename}"')
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def send_file(self, path):
        size = path.stat().st_size
        start, end = 0, size - 1
        status = HTTPStatus.OK
        request_range = self.headers.get("Range")
        if request_range:
            match = re.fullmatch(r"bytes=(\d*)-(\d*)", request_range)
            if match:
                first, last = match.groups()
                if first:
                    start, end = int(first), min(end, int(last)) if last else end
                elif last:
                    start = max(0, size - int(last))
            if not match or not any(match.groups()) or not 0 <= start <= end:
                self.send_response(HTTPStatus.REQUESTED_RANGE_NOT_SATISFIABLE)
                self.send_header("Content-Range", f"bytes */{size}")
                self.end_headers()
                return
            status = HTTPStatus.PARTIAL_CONTENT

        content_type = mimetypes.guess_type(path.name)[0] or "application/octet-stream"
        self.send_response(status)
        self.send_header("Content-Type", content_type)
        self.send_header("Cross-Origin-Resource-Policy", "same-origin")
        self.send_header("Content-Length", str(end - start + 1))
        self.send_header("Accept-Ranges", "bytes")
        self.send_header("Cache-Control", "no-cache")
        if request_range:
            self.send_header("Content-Range", f"bytes {start}-{end}/{size}")
        self.end_headers()
        with path.open("rb") as file:
            file.seek(start)
            remaining = end - start + 1
            while remaining and (chunk := file.read(min(remaining, 128 * 1024))):
                self.wfile.write(chunk)
                remaining -= len(chunk)

    def do_GET(self):
        if not self.permitted():
            return
        path, query = urlsplit(self.path).path, parse_qs(urlsplit(self.path).query)
        review = self.server.review

        def param(name, default=""):
            return query.get(name, [default])[0]

        try:
            if path == "/api/catalog":
                self.send_json(review.info)
            elif path == "/api/sounds":
                self.send_json(review.list_sounds(
                    param("q"), param("filter", "all"), param("category"), param("kind"),
                    max(0, int(param("offset", "0"))), min(100, max(1, int(param("limit", "40")))),
                ))
            elif path == "/api/stand-ins":
                self.send_json({"items": review.stand_ins(param("category"), param("exclude"))})
            elif path == "/api/export":
                self.send_json(review.export(), filename="misophonia-sound-review.json")
            elif path.startswith("/api/audio/"):
                key = unquote(path.rsplit("/", 1)[-1])
                row = review.rows[key]
                if not row["playable"]:
                    self.send_json({"error": "No installed audio file is linked to this event."}, HTTPStatus.NOT_FOUND)
                else:
                    self.send_file(review.game.preview(key))
            elif path in ("/", "/index.html", "/app.js", "/styles.css"):
                self.send_file(HERE / "static" / ("index.html" if path == "/" else path[1:]))
            else:
                self.send_json({"error": "This resource does not exist."}, HTTPStatus.NOT_FOUND)
        except KeyError:
            self.send_json({"error": "Sound not found."}, HTTPStatus.NOT_FOUND)
        except ValueError as exc:
            self.send_json({"error": str(exc)}, HTTPStatus.BAD_REQUEST)
        except (BrokenPipeError, ConnectionResetError):
            pass
        except (OSError, RuntimeError) as exc:
            print(f"Audio Review request failed for {path}. {exc}", flush=True)
            self.send_json({"error": str(exc)}, HTTPStatus.UNPROCESSABLE_ENTITY)

    def do_PUT(self):
        if not self.permitted():
            return
        match = re.fullmatch(r"/api/sounds/([^/]+)/tags", urlsplit(self.path).path)
        if not match:
            self.send_json({"error": "This resource does not exist."}, HTTPStatus.NOT_FOUND)
            return
        try:
            size = int(self.headers.get("Content-Length", "0"))
            if self.headers.get_content_type() != "application/json" or not 1 <= size <= 8192:
                raise ValueError("Send a JSON tag record up to 8 KiB.")
            tags = json.loads(self.rfile.read(size))
            self.send_json(self.server.review.save(unquote(match[1]), tags))
        except KeyError:
            self.send_json({"error": "Sound not found."}, HTTPStatus.NOT_FOUND)
        except (ValueError, UnicodeError) as exc:
            self.send_json({"error": str(exc)}, HTTPStatus.BAD_REQUEST)

    def log_message(self, *_args):
        pass


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--game", type=Path, required=True, help="Cyberpunk 2077 installation directory")
    parser.add_argument("--catalog", type=Path, default=HERE.parent.parent, help="Misophonia source checkout to read")
    parser.add_argument("--event-index", type=Path, help="Optional SoundDB events.json to use instead of fetching it")
    data = Path(os.environ.get("XDG_DATA_HOME", Path.home() / ".local/share")) / "misophonia/audio-review"
    cache = Path(os.environ.get("XDG_CACHE_HOME", Path.home() / ".cache")) / "misophonia/audio-review"
    parser.add_argument("--data", type=Path, default=data, help="Folder for persistent review tags")
    parser.add_argument("--cache", type=Path, default=cache, help="Folder for game index and audio previews")
    parser.add_argument("--wolvenkit", type=Path, help="Path to WolvenKit.CLI")
    parser.add_argument("--decoder", type=Path, help="Path to vgmstream-cli")
    parser.add_argument("--port", type=int, default=8789, help="Loopback port")
    args = parser.parse_args()

    from game_audio import GameAudio

    try:
        args.cache.mkdir(parents=True, exist_ok=True)
        media_events = event_media(args.event_index, args.cache)
        game = GameAudio(args.game.expanduser(), args.cache, args.wolvenkit, args.decoder)
        print("Reading the local Cyberpunk audio archive indexes.", flush=True)
        review = Review(
            game, game.build(), args.catalog.expanduser().resolve(), media_events,
            ReviewStore(args.data / "review.sqlite3"),
        )
        with ThreadingHTTPServer(("127.0.0.1", args.port), ReviewHandler) as server:
            server.review = review
            print(f"Audio Review · {review.info['stats']['sounds']:,} sounds · http://127.0.0.1:{server.server_port}/", flush=True)
            try:
                server.serve_forever()
            except KeyboardInterrupt:
                pass
    except (OSError, ValueError, KeyError, RuntimeError, subprocess.TimeoutExpired) as exc:
        parser.exit(1, f"Audio Review could not start. {exc}\n")


if __name__ == "__main__":
    main()
