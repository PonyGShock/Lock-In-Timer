//! The one place the settings file is written from.
//!
//! Saving straight from wherever a setting changed had two problems. Dragging
//! a volume slider fires a change on every step, so one gesture rewrote the
//! file about a hundred times. And because saves ran on whichever thread asked
//! for them, two could overlap on the shared temporary file and publish a
//! corrupt result. A single writer thread fixes both: every save goes through
//! it in order, and a burst of saves collapses into one write of the last.

use std::path::PathBuf;
use std::sync::mpsc::{self, RecvTimeoutError, Sender, SyncSender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::settings::Settings;

/// How long the writer waits for the burst to finish before writing.
const QUIET: Duration = Duration::from_millis(400);
/// The longest a change can sit unwritten, however busy the burst. Bounds what
/// a crash could lose during an unbroken stream of changes.
const MAX_DELAY: Duration = Duration::from_secs(2);

enum Message {
    Save(Box<Settings>),
    Flush(SyncSender<()>),
}

pub struct Persister {
    tx: Option<Sender<Message>>,
    worker: Option<JoinHandle<()>>,
}

impl Persister {
    pub fn spawn(path: PathBuf) -> Self {
        let (tx, rx) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("lockin-settings".into())
            .spawn(move || run(path, rx))
            .expect("settings writer thread should spawn");
        Persister {
            tx: Some(tx),
            worker: Some(worker),
        }
    }

    /// Queues `settings` to be written. Returns immediately.
    pub fn save(&self, settings: &Settings) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Message::Save(Box::new(settings.clone())));
        }
    }

    /// Writes anything still pending and waits until it is on disk. Call this
    /// before the process exits.
    pub fn flush(&self) {
        let Some(tx) = &self.tx else {
            return;
        };
        let (done_tx, done_rx) = mpsc::sync_channel(1);
        if tx.send(Message::Flush(done_tx)).is_ok() {
            let _ = done_rx.recv();
        }
    }
}

impl Drop for Persister {
    fn drop(&mut self) {
        // Closing the channel tells the writer to finish what it holds.
        self.tx.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn run(path: PathBuf, rx: mpsc::Receiver<Message>) {
    let mut pending: Option<(Box<Settings>, Instant)> = None;

    loop {
        let message = match &pending {
            None => rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
            Some((_, since)) => {
                let waited = since.elapsed();
                let budget = MAX_DELAY.saturating_sub(waited).min(QUIET);
                rx.recv_timeout(budget)
            }
        };

        match message {
            Ok(Message::Save(settings)) => {
                // Keep when the burst began, not when it was last extended, so
                // MAX_DELAY is measured from the first unwritten change.
                let since = pending.map(|(_, since)| since).unwrap_or_else(Instant::now);
                pending = Some((settings, since));
            }
            Ok(Message::Flush(done)) => {
                write(&path, pending.take());
                let _ = done.send(());
            }
            Err(RecvTimeoutError::Timeout) => write(&path, pending.take()),
            Err(RecvTimeoutError::Disconnected) => {
                write(&path, pending.take());
                return;
            }
        }
    }
}

fn write(path: &std::path::Path, pending: Option<(Box<Settings>, Instant)>) {
    if let Some((settings, _)) = pending {
        if let Err(error) = settings.save(path) {
            eprintln!("lock-in: could not save settings: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("lockin-persist-{name}-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir.join("settings.json")
    }

    fn with_volume(volume: f32) -> Settings {
        Settings {
            noise_volume: volume,
            completed_date: "2026-01-01".to_string(),
            ..Settings::default()
        }
    }

    #[test]
    fn a_burst_of_saves_lands_as_the_last_one() {
        let path = scratch("burst");
        let persister = Persister::spawn(path.clone());
        for step in 0..=100 {
            persister.save(&with_volume(step as f32 / 100.0));
        }
        persister.flush();
        assert_eq!(Settings::load(&path), with_volume(1.0));
        fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    #[test]
    fn a_burst_is_not_written_until_it_goes_quiet() {
        let path = scratch("quiet");
        let persister = Persister::spawn(path.clone());
        persister.save(&with_volume(0.3));
        // Well inside the quiet window: nothing should have been written.
        thread::sleep(Duration::from_millis(50));
        assert!(!path.exists(), "the write should wait for the burst to end");

        // Then it lands on its own, without a flush. Poll rather than sleep a
        // fixed amount, so a slow machine cannot turn this into a flake.
        let deadline = Instant::now() + Duration::from_secs(5);
        while !path.exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(Settings::load(&path), with_volume(0.3));
        drop(persister);
        fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    #[test]
    fn dropping_the_writer_does_not_lose_a_pending_save() {
        let path = scratch("drop");
        {
            let persister = Persister::spawn(path.clone());
            persister.save(&with_volume(0.42));
        }
        assert_eq!(Settings::load(&path), with_volume(0.42));
        fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    #[test]
    fn saves_from_many_threads_never_corrupt_the_file() {
        // The bug this module exists for: concurrent writers sharing a temp
        // file could publish one payload spliced onto the tail of another.
        let path = scratch("threads");
        let persister = std::sync::Arc::new(Persister::spawn(path.clone()));
        let handles: Vec<_> = (0..8)
            .map(|worker| {
                let persister = persister.clone();
                thread::spawn(move || {
                    for step in 0..50 {
                        let mut settings = with_volume((worker * 50 + step) as f32 / 400.0);
                        // Vary the length of the file so a splice would show.
                        settings.custom_preset.label = "x".repeat(step);
                        persister.save(&settings);
                    }
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }
        persister.flush();

        let text = fs::read_to_string(&path).unwrap();
        assert!(
            serde_json::from_str::<Settings>(&text).is_ok(),
            "settings file was corrupted:\n{text}"
        );
        fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    #[test]
    fn flushing_with_nothing_pending_returns_promptly() {
        let path = scratch("idle-flush");
        let persister = Persister::spawn(path.clone());
        let started = Instant::now();
        persister.flush();
        assert!(started.elapsed() < Duration::from_millis(200));
        assert!(!path.exists());
        drop(persister);
        fs::remove_dir_all(path.parent().unwrap()).ok();
    }
}
