#!/usr/bin/env python3
"""Misophonia Audio Review desktop program."""

import argparse
from functools import partial
import json
import os
from pathlib import Path
import sqlite3
import sys

from PySide6.QtCore import QIODevice, QSaveFile, QSignalBlocker, QThreadPool, QTimer, Qt, Slot
from PySide6.QtGui import QIcon
from PySide6.QtWidgets import (
    QAbstractItemView, QApplication, QComboBox, QFileDialog, QFormLayout, QFrame,
    QHBoxLayout, QHeaderView, QLabel, QLineEdit, QListWidget, QMainWindow,
    QProgressBar, QPushButton, QSizePolicy, QSlider, QSplitter, QTableWidget, QTableWidgetItem,
    QVBoxLayout, QWidget,
)

from catalog import event_media
from game_audio import GameAudio
from player import Job, Preview
from review import Review, ReviewStore

HERE = Path(__file__).resolve().parent
PAGE_SIZE = 40
FILTERS = {
    "All sounds": "all", "Candidates": "candidate", "Stand-ins": "stand-in",
    "In the mod": "covered", "Untagged": "untagged",
}
STYLE = (HERE / "desktop.qss").read_text(encoding="utf-8")


def argument_parser():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--game", type=Path, required=True, help="Cyberpunk 2077 installation directory")
    parser.add_argument("--catalog", type=Path, default=HERE.parent.parent, help="Misophonia source checkout")
    parser.add_argument("--event-index", type=Path, help="Optional local SoundDB events.json")
    data = Path(os.environ.get("XDG_DATA_HOME", Path.home() / ".local/share")) / "misophonia/audio-review"
    cache = Path(os.environ.get("XDG_CACHE_HOME", Path.home() / ".cache")) / "misophonia/audio-review"
    parser.add_argument("--data", type=Path, default=data, help="Persistent review folder")
    parser.add_argument("--cache", type=Path, default=cache, help="Disposable index and preview folder")
    parser.add_argument("--wolvenkit", type=Path, help="Path to WolvenKit.CLI")
    parser.add_argument("--decoder", type=Path, help="Path to vgmstream-cli")
    return parser


def load_library(args, cancelled):
    cache = args.cache.expanduser()
    cache.mkdir(parents=True, exist_ok=True)
    index = args.event_index.expanduser() if args.event_index else None
    events = event_media(index, cache)
    game = GameAudio(args.game.expanduser(), cache, args.wolvenkit, args.decoder)
    return Review(game, game.build(cancelled), args.catalog.expanduser(), events,
                  ReviewStore(args.data.expanduser() / "review.sqlite3"))


def light_label(text):
    label = QLabel(text)
    label.setProperty("quiet", True)
    label.setTextFormat(Qt.TextFormat.PlainText)
    return label


def dropdown(label):
    combo = QComboBox()
    combo.setAccessibleName(label)
    combo.setSizeAdjustPolicy(QComboBox.SizeAdjustPolicy.AdjustToMinimumContentsLengthWithIcon)
    combo.setMinimumContentsLength(10)
    return combo


def set_value(combo, value):
    index = combo.findData(value)
    combo.setCurrentIndex(max(index, 0))


def time_label(milliseconds):
    minutes, seconds = divmod(milliseconds // 1000, 60)
    return f"{minutes}:{seconds:02d}"


class FittedLabel(QLabel):
    def __init__(self, text, **properties):
        super().__init__(**properties)
        self.setTextFormat(Qt.TextFormat.PlainText)
        self.setSizePolicy(QSizePolicy.Policy.Ignored, QSizePolicy.Policy.Fixed)
        self.setText(text)

    def setText(self, text):
        self.full_text = text
        self.setToolTip(text)
        self.fit()

    def resizeEvent(self, event):
        super().resizeEvent(event)
        self.fit()

    def fit(self):
        text = self.fontMetrics().elidedText(self.full_text, Qt.TextElideMode.ElideRight, self.width())
        super().setText(text)


class ReviewWindow(QMainWindow):
    def __init__(self):
        super().__init__()
        self.setWindowTitle("Misophonia Audio Review")
        self.resize(1230, 770)
        self.setMinimumSize(850, 540)
        self.review = None
        self.items = []
        self.categories = {}
        self.kinds = {}
        self.offset = 0
        self.current = None
        self.row_players = {}
        self.updating = False
        self.index_cancel = None
        self.preview = Preview(self)

        central = QWidget()
        layout = QVBoxLayout(central)
        layout.setContentsMargins(20, 14, 20, 12)
        layout.setSpacing(11)
        self.setCentralWidget(central)

        header = QHBoxLayout()
        heading = QLabel("Misophonia Audio Review", objectName="heading")
        header.addWidget(heading, 1)
        self.export_button = QPushButton("Export review…")
        self.export_button.setEnabled(False)
        self.export_button.clicked.connect(self.export)
        header.addWidget(self.export_button)
        layout.addLayout(header)
        self.summary = light_label("Indexing the installed game audio…")
        layout.addWidget(self.summary)
        self.progress = QProgressBar()
        self.progress.setRange(0, 0)
        self.progress.setTextVisible(False)
        self.progress.setFixedHeight(5)
        layout.addWidget(self.progress)

        self.query = QLineEdit()
        self.query.setPlaceholderText("Search sound, event, or media ID")
        self.query.setAccessibleName("Search sounds")
        self.query.setClearButtonEnabled(True)
        self.view = dropdown("Review status")
        for label, key in FILTERS.items():
            self.view.addItem(label, key)
        self.category = dropdown("Filter by category")
        self.category.addItem("All categories", "")
        self.kind = dropdown("Filter by sound type")
        self.kind.addItem("All sound types", "")

        self.filters = QWidget()
        filters = QHBoxLayout(self.filters)
        filters.setContentsMargins(0, 0, 0, 0)
        filters.addWidget(self.query, 1)
        for control in (self.view, self.category, self.kind):
            filters.addWidget(control)
            control.currentIndexChanged.connect(self.first_page)
        self.filters.setEnabled(False)
        layout.addWidget(self.filters)
        self.debounce = QTimer(self, interval=160, singleShot=True)
        self.debounce.timeout.connect(self.first_page)
        self.query.textChanged.connect(lambda: self.debounce.start())
        self.query.returnPressed.connect(self.first_page)

        splitter = QSplitter(Qt.Orientation.Horizontal)
        splitter.setChildrenCollapsible(False)
        self.table = QTableWidget(0, 5)
        self.table.setAccessibleName("Sound library")
        self.table.setHorizontalHeaderLabels(["Preview", "Sound", "Mod coverage", "X", "O"])
        self.table.setSelectionBehavior(QAbstractItemView.SelectionBehavior.SelectRows)
        self.table.setSelectionMode(QAbstractItemView.SelectionMode.SingleSelection)
        self.table.setEditTriggers(QAbstractItemView.EditTrigger.NoEditTriggers)
        self.table.setAlternatingRowColors(True)
        self.table.setShowGrid(False)
        self.table.verticalHeader().hide()
        self.table.verticalHeader().setDefaultSectionSize(36)
        self.table.itemSelectionChanged.connect(self.select_current)
        self.table.cellDoubleClicked.connect(lambda index, _: self.preview.toggle(self.items[index]))
        columns = self.table.horizontalHeader()
        columns.setSectionResizeMode(QHeaderView.ResizeMode.Interactive)
        columns.setSectionResizeMode(1, QHeaderView.ResizeMode.Stretch)
        for column, width in ((0, 66), (2, 194), (3, 49), (4, 49)):
            self.table.setColumnWidth(column, width)
        splitter.addWidget(self.table)
        splitter.addWidget(self.inspector())
        splitter.setStretchFactor(0, 1)
        splitter.setSizes([890, 308])
        layout.addWidget(splitter, 1)

        pager = QHBoxLayout()
        self.count = light_label("Preparing the library")
        pager.addWidget(self.count, 1)
        self.previous = QPushButton("Previous")
        self.next = QPushButton("Next")
        self.previous.setEnabled(False)
        self.next.setEnabled(False)
        self.previous.clicked.connect(lambda: self.load_page(self.offset - PAGE_SIZE))
        self.next.clicked.connect(lambda: self.load_page(self.offset + PAGE_SIZE))
        pager.addWidget(self.previous)
        pager.addWidget(self.next)
        layout.addLayout(pager)
        layout.addWidget(self.transport())

    def inspector(self):
        panel = QFrame(objectName="panel")
        panel.setMinimumWidth(287)
        layout = QVBoxLayout(panel)
        layout.setContentsMargins(13, 11, 13, 13)
        self.selected_title = FittedLabel("Select a sound", objectName="detailTitle")
        layout.addWidget(self.selected_title)
        self.selected_source = light_label("")
        self.selected_source.setWordWrap(True)
        self.selected_source.setTextInteractionFlags(Qt.TextInteractionFlag.TextSelectableByMouse)
        layout.addWidget(self.selected_source)
        self.selected_coverage = light_label("")
        self.selected_coverage.setWordWrap(True)
        layout.addWidget(self.selected_coverage)

        self.candidate_category = dropdown("Candidate category")
        self.replacement = dropdown("Replacement")
        self.stand_in_category = dropdown("Stand-in category")
        form = QFormLayout()
        form.addRow("Candidate category", self.candidate_category)
        form.addRow("Replacement", self.replacement)
        form.addRow("Stand-in category", self.stand_in_category)
        for control, field in ((self.candidate_category, "category"), (self.replacement, "replacement"),
                               (self.stand_in_category, "stand_in_category")):
            control.setEnabled(False)
            control.currentIndexChanged.connect(partial(self.set_field, field, control))
        layout.addLayout(form)
        layout.addWidget(light_label("Linked events"))
        self.events = QListWidget()
        self.events.setAccessibleName("Linked game sound events")
        layout.addWidget(self.events, 1)
        return panel

    def transport(self):
        panel = QFrame(objectName="panel")
        layout = QHBoxLayout(panel)
        layout.setContentsMargins(9, 5, 9, 5)
        self.now = FittedLabel("Ready to preview")
        self.now.setProperty("quiet", True)
        self.now.setMinimumWidth(140)
        self.now.setMaximumWidth(350)
        self.play_selected = QPushButton("Play")
        self.play_selected.setEnabled(False)
        self.play_selected.clicked.connect(self.transport_toggle)
        self.seek = QSlider(Qt.Orientation.Horizontal, maximum=0)
        self.seek.setAccessibleName("Preview position")
        self.seek.setSingleStep(1000)
        self.seek.valueChanged.connect(self.preview.media.setPosition)
        self.elapsed = light_label("0:00 / 0:00")
        self.volume = QSlider(Qt.Orientation.Horizontal, maximum=100, value=35)
        self.volume.setFixedWidth(94)
        self.volume.setAccessibleName("Preview volume")
        self.volume.valueChanged.connect(lambda value: self.preview.output.setVolume(value / 100))
        layout.addWidget(self.play_selected)
        layout.addWidget(self.now)
        layout.addWidget(self.seek, 1)
        layout.addWidget(self.elapsed)
        layout.addWidget(light_label("Volume"))
        layout.addWidget(self.volume)
        self.preview.changed.connect(self.update_playback)
        self.preview.failed.connect(partial(self.statusBar().showMessage))
        self.preview.media.positionChanged.connect(self.update_position)
        self.preview.media.durationChanged.connect(self.update_duration)
        return panel

    def begin(self, args):
        job = Job(0, partial(load_library, args))
        self.index_cancel = job.cancelled
        job.signals.finished.connect(self.library_ready, Qt.ConnectionType.QueuedConnection)
        QThreadPool.globalInstance().start(job)

    @Slot(int, object, object)
    def library_ready(self, _token, review, error):
        self.progress.hide()
        self.index_cancel = None
        if error is not None:
            self.summary.setText(f"Could not open the audio library. {error}")
            self.summary.setWordWrap(True)
            self.count.setText("")
            print(f"Audio Review could not start. {error}", file=sys.stderr, flush=True)
            return
        self.review = review
        self.preview.decoder = review.game.preview if review.game else None
        self.categories = {entry["id"]: entry for entry in review.categories}
        self.kinds = {entry["id"]: entry["name"] for entry in review.info["kinds"]}
        self.summary.setText(f"{review.info['stats']['sounds']:,} audio files  ·  Misophonia {review.version}")
        self.updating = True
        self.fill_categories(self.category, "All categories", "")
        for kind, label in self.kinds.items():
            self.kind.addItem(label, kind)
        self.updating = False
        self.filters.setEnabled(True)
        self.export_button.setEnabled(True)
        if review.info["warnings"]:
            self.statusBar().showMessage(" ".join(review.info["warnings"]))
        self.load_page()
        print(f"Audio Review is ready with {review.info['stats']['sounds']:,} audio files.", flush=True)

    def fill_categories(self, combo, empty, blank=None):
        combo.clear()
        combo.addItem(empty, blank)
        for key, entry in self.categories.items():
            combo.addItem(f"{entry['section']} / {entry['name']}", key)

    def first_page(self):
        self.debounce.stop()
        self.load_page()

    def load_page(self, offset=0, keep=None):
        if self.review is None or self.updating:
            return
        result = self.review.list_sounds(
            self.query.text(), self.view.currentData(), self.category.currentData(), self.kind.currentData(),
            max(0, offset), PAGE_SIZE,
        )
        total = result["total"]
        offset = max(0, min(offset, ((max(total, 1) - 1) // PAGE_SIZE) * PAGE_SIZE))
        if offset and not result["items"]:
            self.load_page(offset, keep)
            return
        if total and not result["items"]:
            self.load_page(0, keep)
            return
        self.offset = offset
        self.items = result["items"]
        self.previous.setEnabled(offset > 0)
        self.next.setEnabled(offset + PAGE_SIZE < total)
        self.count.setText(f"{offset + 1:,}–{offset + len(self.items):,} of {total:,}" if total else "No matching sounds")
        self.draw_table(keep)

    def draw_table(self, keep):
        self.updating = True
        self.table.clearContents()
        self.table.setRowCount(len(self.items))
        self.row_players.clear()
        selected = 0
        for index, row in enumerate(self.items):
            if row["id"] == keep:
                selected = index
            key = row["id"]
            play = self.row_button(index, 0, "▷", f"Play {row['name']}",
                                   partial(self.preview.toggle, row), enabled=row["playable"])
            self.row_players[key] = (play, row["name"], row["playable"])
            self.set_cell(index, 1, row["name"], row.get("path", row["detail"]))
            coverage = list(dict.fromkeys(
                f"{self.categories[item['category']]['name']} · {'slider' if item['mode'] == 'volume' else 'mute'}"
                for item in row["coverage"]
            ))
            self.set_cell(index, 2, " / ".join(coverage) if coverage else "—")
            self.row_button(index, 3, "X", f"Candidate tag for {row['name']}",
                            partial(self.toggle_tag, row, "candidate"), checked=row["tags"]["candidate"],
                            name="candidate")
            self.row_button(index, 4, "O", f"Stand-in tag for {row['name']}",
                            partial(self.toggle_tag, row, "stand_in"), checked=row["tags"]["stand_in"],
                            enabled=row["playable"], name="standIn")
        if self.items:
            self.table.selectRow(selected)
        self.updating = False
        self.select_current()

    def row_button(self, index, column, text, label, action, checked=None, enabled=True, name=""):
        button = QPushButton(text, objectName=name)
        button.setAccessibleName(label)
        button.setToolTip(label)
        button.setEnabled(enabled)
        if checked is not None:
            button.setCheckable(True)
            button.setChecked(checked)
        button.clicked.connect(action)
        self.table.setCellWidget(index, column, button)
        return button

    def set_cell(self, row, column, value, tooltip=None):
        cell = QTableWidgetItem(value)
        cell.setToolTip(tooltip or value)
        self.table.setItem(row, column, cell)

    def select_current(self):
        if self.updating:
            return
        selection = self.table.selectionModel().selectedRows()
        self.current = self.items[selection[0].row()] if selection else None
        self.draw_details()
        self.update_playback()

    def draw_details(self):
        row = self.current
        self.updating = True
        self.events.clear()
        self.selected_title.setText(row["name"] if row else "Select a sound")
        self.selected_source.setText(row.get("path", row["detail"]) if row else "")
        tags = row["tags"] if row else {}
        self.fill_categories(self.candidate_category, "Choose category")
        self.fill_categories(self.stand_in_category, "Choose category")
        self.replacement.clear()
        self.replacement.addItem("Mute (default)", None)
        groups = []
        if row:
            for item in row["coverage"]:
                category = self.categories[item["category"]]
                mode = "volume slider" if item["mode"] == "volume" else "mute"
                groups.append(f"{category['section']} / {category['name']} · {item['edition']} {mode}")
            self.events.addItems(row["events"])
            for donor in self.review.stand_ins(tags["category"], row["id"]):
                self.replacement.addItem(donor["name"], donor["id"])
            if tags["replacement"] and self.replacement.findData(tags["replacement"]) < 0:
                self.replacement.addItem("Saved stand-in (unavailable)", tags["replacement"])
        for field, control, enabled in (
            ("category", self.candidate_category, "candidate"), ("replacement", self.replacement, "candidate"),
            ("stand_in_category", self.stand_in_category, "stand_in"),
        ):
            set_value(control, tags.get(field))
            control.setEnabled(bool(tags.get(enabled)))
        self.selected_coverage.setText("\n".join(groups) if groups else ("No linked mod coverage" if row else ""))
        self.updating = False

    def toggle_tag(self, row, field):
        tags = row["tags"].copy()
        tags[field] = not tags[field]
        if field == "candidate":
            tags["category"] = tags["stand_in_category"] if tags[field] else None
            tags["replacement"] = None
        else:
            tags["stand_in_category"] = tags["category"] if tags[field] else None
        self.save(row, tags)

    def set_field(self, field, control, _index):
        if self.updating or self.current is None:
            return
        tags = {**self.current["tags"], field: control.currentData()}
        if field == "category":
            tags["replacement"] = None
        self.save(self.current, tags)

    def save(self, row, tags):
        try:
            result = self.review.save(row["id"], tags)
        except (ValueError, sqlite3.Error) as error:
            self.statusBar().showMessage(f"Could not save the tag. {error}")
        else:
            count = result["reset_count"]
            self.statusBar().showMessage(f"Reset {count} replacement choices to mute." if count else "Review tag saved.", 3500)
        self.load_page(self.offset, row["id"])

    def transport_toggle(self):
        if self.preview.sound_id:
            row = self.review.rows[self.preview.sound_id]
        else:
            row = self.current
        if row:
            self.preview.toggle(row)

    def update_playback(self):
        preview = self.preview
        for key, (button, name, playable) in self.row_players.items():
            active = key == preview.sound_id and (preview.playing or preview.loading)
            button.setText("■" if active and preview.loading else ("Ⅱ" if active else "▷"))
            action = "Cancel preview of" if active and preview.loading else ("Pause" if active else "Play")
            label = f"{action} {name}" if playable else f"No playable installed file linked to {name}"
            button.setAccessibleName(label)
            button.setToolTip(label)
        self.now.setText(("Decoding…" if preview.loading else preview.name) or "Ready to preview")
        self.play_selected.setText("Cancel" if preview.loading else ("Pause" if preview.playing else "Play"))
        self.play_selected.setEnabled(bool(preview.sound_id or (self.current and self.current["playable"])))

    def update_position(self, position):
        if not self.seek.isSliderDown():
            with QSignalBlocker(self.seek):
                self.seek.setValue(position)
        self.elapsed.setText(f"{time_label(position)} / {time_label(self.preview.media.duration())}")

    def update_duration(self, duration):
        with QSignalBlocker(self.seek):
            self.seek.setRange(0, duration)
        self.seek.setEnabled(duration > 0)
        self.update_position(self.preview.media.position())

    def export(self):
        path, _ = QFileDialog.getSaveFileName(self, "Export review", "misophonia-sound-review.json", "JSON (*.json)")
        if not path:
            return
        try:
            data = (json.dumps(self.review.export(), ensure_ascii=False, indent=2) + "\n").encode("utf-8")
            file = QSaveFile(path)
            if not file.open(QIODevice.OpenModeFlag.WriteOnly) or file.write(data) != len(data) or not file.commit():
                raise OSError(file.errorString())
        except (OSError, sqlite3.Error) as error:
            self.statusBar().showMessage(f"Export failed. {error}")
        else:
            self.statusBar().showMessage("Review exported.", 4500)

    def closeEvent(self, event):
        if self.index_cancel is not None:
            self.index_cancel.set()
        self.preview.stop()
        super().closeEvent(event)


def main():
    args = argument_parser().parse_args()
    app = QApplication(sys.argv[:1])
    app.setApplicationName("Misophonia Audio Review")
    app.setDesktopFileName("misophonia-audio-review")
    app.setWindowIcon(QIcon.fromTheme("audio-headphones"))
    app.setStyle("Fusion")
    app.setStyleSheet(STYLE)
    window = ReviewWindow()
    window.show()
    window.begin(args)
    return app.exec()


if __name__ == "__main__":
    sys.exit(main())
