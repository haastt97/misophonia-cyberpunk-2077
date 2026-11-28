use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, TryRecvError},
    },
    thread::{self, JoinHandle},
};

use anyhow::{Result, anyhow};
use eframe::egui;

pub struct Job<T> {
    cancelled: Arc<AtomicBool>,
    result: Receiver<Result<T>>,
    worker: Option<JoinHandle<()>>,
}

impl<T: Send + 'static> Job<T> {
    pub fn start(
        ctx: &egui::Context,
        work: impl FnOnce(&AtomicBool) -> Result<T> + Send + 'static,
    ) -> Self {
        let (send, result) = mpsc::sync_channel(1);
        let cancelled = Arc::new(AtomicBool::new(false));
        let token = Arc::clone(&cancelled);
        let ctx = ctx.clone();
        let worker = thread::spawn(move || {
            let _ = send.send(work(&token));
            ctx.request_repaint();
        });
        Self {
            cancelled,
            result,
            worker: Some(worker),
        }
    }

    pub fn poll(&self) -> Option<Result<T>> {
        match self.result.try_recv() {
            Ok(result) => Some(result),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                Some(Err(anyhow!("An audio worker exited unexpectedly")))
            }
        }
    }
}

impl<T> Job<T> {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn is_finished(&self) -> bool {
        self.worker
            .as_ref()
            .is_none_or(|worker| worker.is_finished())
    }
}

impl<T> Drop for Job<T> {
    fn drop(&mut self) {
        self.cancel();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
