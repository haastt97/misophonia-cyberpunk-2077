"""Exercise the native review workflow through Qt widgets."""

import json
import os
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest
from unittest.mock import patch
import wave

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

try:
    from PySide6.QtCore import Qt
    from PySide6.QtGui import QPalette
    from PySide6.QtTest import QTest
    from PySide6.QtWidgets import QApplication, QFileDialog, QStyleOptionViewItem
    from desktop import ReviewWindow, STAND_IN_TINT, STYLE, TRIGGER_TINT, set_value
except ModuleNotFoundError as error:
    if not error.name.startswith("PySide6"):
        raise
    raise unittest.SkipTest("Install the desktop requirements to run the Qt workflow tests.") from error

from review import Review, ReviewStore

ROOT = Path(__file__).resolve().parents[2]


class SampleGame:
    def __init__(self, sound):
        self.sound = sound

    def preview(self, _sound_id, _cancelled):
        return self.sound


class DesktopTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.app = QApplication.instance() or QApplication([])
        cls.app.setStyle("Fusion")
        cls.app.setStyleSheet(STYLE)

    def setUp(self):
        scratch = TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        self.scratch = Path(scratch.name)
        self.database = self.scratch / "review.sqlite3"
        self.clip = self.scratch / "sample.wav"
        with wave.open(str(self.clip), "wb") as output:
            output.setparams((1, 2, 8000, 16000, "NONE", ""))
            output.writeframes(b"\x00" * 32000)
        self.window = ReviewWindow()
        self.window.show()
        self.addCleanup(self.window.close)
        self.window.library_ready(0, self.open_review(), None)
        set_value(self.window.kind, "sfx")

    def open_review(self):
        assets = [
            {"id": "donor", "name": "Gentle sample", "kind": "sfx", "detail": "Test audio"},
            {"id": "candidate", "name": "Trigger sample", "kind": "sfx", "detail": "Test audio", "media_id": 42},
        ]
        return Review(SampleGame(self.clip), assets, ROOT,
                      {42: {"cmn_generic_male_eat_chrunchy"}}, ReviewStore(self.database))

    def click_tag(self, sound_id, column):
        index = next(i for i, row in enumerate(self.window.items) if row["id"] == sound_id)
        self.window.table.scrollToItem(self.window.table.item(index, 1))
        QTest.mouseClick(self.window.table.cellWidget(index, column), Qt.MouseButton.LeftButton)

    def test_category_gates_and_persistent_replacement(self):
        win = self.window
        self.assertFalse(win.candidate_category.isEnabled())
        self.assertFalse(win.replacement.isEnabled())
        self.click_tag("donor", 4)
        self.assertTrue(win.stand_in_category.isEnabled())
        self.assertFalse(win.candidate_category.isEnabled())
        set_value(win.stand_in_category, "food")
        self.click_tag("candidate", 3)
        self.assertTrue(win.candidate_category.isEnabled())
        self.assertTrue(win.replacement.isEnabled())
        self.assertEqual(win.replacement.currentText(), "Mute (default)")
        set_value(win.candidate_category, "food")
        set_value(win.replacement, "donor")
        self.assertEqual(self.open_review().store.entries()["candidate"]["replacement"], "donor")
        self.click_tag("donor", 4)
        self.assertIsNone(self.open_review().store.entries()["candidate"]["replacement"])

    def test_export_writes_the_same_review_format(self):
        self.click_tag("candidate", 3)
        destination = self.scratch / "review.json"
        with patch.object(QFileDialog, "getSaveFileName", return_value=(str(destination), "JSON")):
            QTest.mouseClick(self.window.export_button, Qt.MouseButton.LeftButton)
        self.assertEqual(json.loads(destination.read_bytes())["sounds"][0]["id"], "candidate")

    def test_outside_filter_and_tag_colors(self):
        win = self.window
        set_value(win.view, "outside")
        self.assertEqual(win.view.currentText(), "Outside the mod")
        self.assertEqual([row["id"] for row in win.items], ["donor"])
        self.click_tag("donor", 4)
        self.assertEqual(win.table.item(0, 1).background().color(), STAND_IN_TINT)
        self.assertEqual(win.table.item(0, 2).background().color(), STAND_IN_TINT)
        self.click_tag("donor", 3)
        self.assertEqual(win.table.item(0, 1).background().color(), TRIGGER_TINT)
        self.assertEqual(win.table.item(0, 2).background().color(), STAND_IN_TINT)
        option = QStyleOptionViewItem()
        win.table.itemDelegate().initStyleOption(option, win.table.model().index(0, 1))
        self.assertEqual(option.palette.color(QPalette.ColorRole.Highlight), TRIGGER_TINT.lighter(128))
        set_value(win.view, "all")
        self.assertEqual({row["id"] for row in win.items}, {"candidate", "donor"})

    def test_preview_uses_a_local_file_and_toggles_pause(self):
        win = self.window
        win.preview.output.setMuted(True)
        QTest.mouseClick(win.table.cellWidget(0, 0), Qt.MouseButton.LeftButton)
        for _ in range(120):
            if win.preview.playing and win.preview.media.duration() > 0:
                break
            QTest.qWait(25)
        self.assertEqual(win.preview.media.source().toLocalFile(), str(self.clip))
        self.assertGreater(win.preview.media.duration(), 0)
        self.assertTrue(win.preview.playing)
        QTest.mouseClick(win.play_selected, Qt.MouseButton.LeftButton)
        self.assertFalse(win.preview.playing)
        QTest.keyClick(win.seek, Qt.Key.Key_Right)
        self.assertGreaterEqual(win.preview.media.position(), 1000)


if __name__ == "__main__":
    unittest.main()
