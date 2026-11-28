"""Exercise persistence, category constraints, and catalog coverage."""

from pathlib import Path
from tempfile import TemporaryDirectory
import unittest

from server import EMPTY_TAGS, Review, ReviewStore

ROOT = Path(__file__).resolve().parents[2]


class ReviewTests(unittest.TestCase):
    def setUp(self):
        scratch = TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        self.database = Path(scratch.name) / "review.sqlite3"
        self.assets = [
            {"id": "donor", "name": "Tone", "kind": "sfx", "detail": "OpusPack"},
            {"id": "trigger", "name": "Chewing", "kind": "sfx", "detail": "OpusPack", "media_id": 42},
            {"id": "screen", "name": "TV Clip", "kind": "sound", "detail": "WEM", "media_id": 10},
        ]
        screen_event = (ROOT / "data/screen-events.txt").read_text().splitlines()[0]
        self.events = {42: {"cmn_generic_male_eat_chrunchy"}, 10: {screen_event}}
        self.review = self.open_review()

    def open_review(self):
        return Review(None, self.assets, ROOT, self.events, ReviewStore(self.database))

    def write(self, sound, **tags):
        return self.review.save(sound, {**EMPTY_TAGS, **tags})

    def test_stand_in_replacement_persists_across_restart(self):
        self.write("donor", stand_in=True, stand_in_category="food")
        self.assertEqual(self.review.stand_ins("food", "trigger"), [{"id": "donor", "name": "Tone"}])
        self.assertEqual(self.review.stand_ins("food", "donor"), [])
        self.write("trigger", candidate=True, category="food", replacement="donor")

        reopened = self.open_review()
        self.assertEqual(reopened.list_sounds("", "candidate", "food", "", 0, 40)["items"][0]["tags"]["replacement"], "donor")
        saved = next(row for row in reopened.export()["sounds"] if row["id"] == "trigger")
        self.assertEqual(saved["events"], ["cmn_generic_male_eat_chrunchy"])
        self.assertEqual(saved["media_id"], 42)

    def test_category_validation_and_reset_on_removing_stand_in(self):
        self.write("donor", stand_in=True, stand_in_category="breathing")
        with self.assertRaisesRegex(ValueError, "same category"):
            self.write("trigger", candidate=True, category="food", replacement="donor")
        with self.assertRaises(ValueError):
            self.write("trigger", candidate=True, category="food", replacement="trigger")
        self.write("donor", stand_in=True, stand_in_category="food")
        self.write("trigger", candidate=True, category="food", replacement="donor")
        result = self.write("donor")
        self.assertEqual(result["reset_count"], 1)
        self.assertIsNone(self.open_review().store.entries()["trigger"]["replacement"])

    def test_mod_coverage_does_not_depend_on_tags(self):
        categories = {c["id"]: c for c in self.review.categories}
        self.assertEqual(categories["food"]["name"], "Eating")
        self.assertEqual(categories["screenPercent"]["name"], "Screens")
        screen = self.review.rows["screen"]
        self.assertEqual(
            {(item["edition"], item["mode"]) for item in screen["coverage"]},
            {("Configurable", "volume"), ("Standalone", "mute")},
        )
        results = self.review.list_sounds("male eat chrunchy", "covered", "food", "sfx", 0, 40)
        self.assertEqual([row["id"] for row in results["items"]], ["trigger"])
        self.assertEqual(results["items"][0]["tags"], EMPTY_TAGS)

    def test_unlinked_covered_events_are_visible_and_unplayable(self):
        results = self.review.list_sounds("", "covered", "breathing", "event", 0, 1)
        self.assertGreater(results["total"], 1)
        row = results["items"][0]
        self.assertFalse(row["playable"])
        self.assertEqual(row["kind"], "event")
        with self.assertRaisesRegex(ValueError, "installed audio"):
            self.write(row["id"], stand_in=True, stand_in_category="breathing")


if __name__ == "__main__":
    unittest.main()
