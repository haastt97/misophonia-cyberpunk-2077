"""Asynchronous local decoding and a native Qt audio transport."""

from threading import Event

from PySide6.QtCore import QObject, QRunnable, QThreadPool, Qt, QUrl, Signal, Slot
from PySide6.QtMultimedia import QAudioOutput, QMediaPlayer


class JobSignals(QObject):
    finished = Signal(int, object, object)


class Job(QRunnable):
    def __init__(self, token, run):
        super().__init__()
        self.token = token
        self.work = run
        self.signals = JobSignals()
        self.cancelled = Event()

    @Slot()
    def run(self):
        try:
            result = self.work(self.cancelled)
        except Exception as error:
            if not self.cancelled.is_set():
                self.signals.finished.emit(self.token, None, error)
        else:
            if not self.cancelled.is_set():
                self.signals.finished.emit(self.token, result, None)


class Preview(QObject):
    changed = Signal()
    failed = Signal(str)

    def __init__(self, parent=None):
        super().__init__(parent)
        self.pool = QThreadPool(self)
        self.pool.setMaxThreadCount(2)
        self.media = QMediaPlayer(self)
        self.output = QAudioOutput(self)
        self.output.setVolume(0.35)
        self.media.setAudioOutput(self.output)
        self.media.playbackStateChanged.connect(self.changed)
        self.media.errorOccurred.connect(self.on_error)
        self.sound_id = None
        self.name = ""
        self.loading = False
        self.token = 0
        self.decoder = None
        self.pending_cancel = None

    @property
    def playing(self):
        return self.media.playbackState() == QMediaPlayer.PlaybackState.PlayingState

    def toggle(self, row):
        if not row["playable"]:
            return
        if self.sound_id == row["id"]:
            if self.loading:
                self.stop()
                return
            if self.playing:
                self.media.pause()
                return
            if not self.media.source().isEmpty() and self.media.error() == QMediaPlayer.Error.NoError:
                if self.media.mediaStatus() == QMediaPlayer.MediaStatus.EndOfMedia:
                    self.media.setPosition(0)
                self.media.play()
                return

        self.stop()
        self.sound_id, self.name = row["id"], row["name"]
        self.loading = True
        job = Job(self.token, lambda cancelled: self.decoder(row["id"], cancelled))
        self.pending_cancel = job.cancelled
        job.signals.finished.connect(self.play_decoded, Qt.ConnectionType.QueuedConnection)
        self.pool.start(job)
        self.changed.emit()

    def stop(self):
        self.token += 1
        if self.pending_cancel is not None:
            self.pending_cancel.set()
            self.pending_cancel = None
        self.pool.clear()
        self.sound_id = None
        self.name = ""
        self.loading = False
        self.media.stop()
        self.media.setSource(QUrl())
        self.changed.emit()

    @Slot(int, object, object)
    def play_decoded(self, token, path, error):
        if token != self.token:
            return
        self.loading = False
        self.pending_cancel = None
        if error is not None:
            self.failed.emit(str(error))
        else:
            self.media.setSource(QUrl.fromLocalFile(str(path)))
            self.media.play()
        self.changed.emit()

    def on_error(self, _error, message):
        if self.sound_id is not None:
            self.failed.emit(message or "This audio preview could not be played.")
        self.changed.emit()
