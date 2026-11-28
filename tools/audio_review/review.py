"""Search local audio and store Misophonia review decisions."""

from collections import Counter
from contextlib import contextmanager
import re
import sqlite3

from catalog import read_mod, sound_rows

KINDS = {
    "sfx": "Sound effects", "sound": "Streamed sounds", "voice": "Voice-over",
    "music": "Music", "event": "Event only",
}
FILTERS = {
    "all": "All sounds",
    "covered": "In the mod",
    "outside": "Outside the mod",
    "candidate": "Candidates",
    "stand-in": "Stand-ins",
    "untagged": "Untagged",
}
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
            if tag_filter == "outside" and (row["coverage"] or not row["playable"]):
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
