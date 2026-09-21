//! Audio output.
//!
//! `rodio`'s output stream is not `Send`, so it lives on a dedicated thread
//! that owns the device and takes instructions over a channel. Everything the
//! rest of the app holds is a cheap, cloneable [`Audio`] handle.
//!
//! Losing the audio device is never fatal here: the thread keeps draining
//! commands so the timer carries on working in silence.

use std::sync::mpsc::{self, RecvTimeoutError, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use lockin_core::{
    chime,
    noise::{NoiseKind, NoiseSource, DEFAULT_FADE_MS},
    ChimeVoice,
};
use rodio::buffer::SamplesBuffer;
use rodio::{OutputStream, Sink, Source};

const SAMPLE_RATE: u32 = 48_000;
/// Frames rendered per lock of the shared noise state. Large enough that the
/// mutex is barely touched, small enough that a volume change lands promptly.
const BLOCK_FRAMES: usize = 512;
const POLL: Duration = Duration::from_millis(200);

enum Command {
    /// Idempotent: send the whole desired noise state and let the thread
    /// work out whether that means fading up, down or just retuning.
    SetNoise {
        enabled: bool,
        kind: NoiseKind,
        volume: f32,
        tone: f32,
    },
    Chime {
        voice: ChimeVoice,
        frequency: f32,
        gain: f32,
    },
}

/// A bounded channel keeps `Audio` `Sync` (an async `Sender` is not) and means
/// a wedged audio thread can never grow an unbounded backlog.
#[derive(Clone)]
pub struct Audio {
    tx: SyncSender<Command>,
}

impl Audio {
    pub fn spawn() -> Self {
        let (tx, rx) = mpsc::sync_channel::<Command>(64);
        thread::Builder::new()
            .name("lockin-audio".into())
            .spawn(move || run(rx))
            .expect("audio thread should spawn");
        Audio { tx }
    }

    /// Never blocks: audio is a nicety, and the UI thread waiting on it is not.
    fn send(&self, command: Command) {
        let _ = self.tx.try_send(command);
    }

    pub fn set_noise(&self, enabled: bool, kind: NoiseKind, volume: f32, tone: f32) {
        self.send(Command::SetNoise {
            enabled,
            kind,
            volume,
            tone,
        });
    }

    pub fn chime(&self, voice: ChimeVoice, frequency: f32, gain: f32) {
        self.send(Command::Chime {
            voice,
            frequency,
            gain,
        });
    }
}

/// An endless `rodio` source reading from shared noise state.
struct NoiseStream {
    source: Arc<Mutex<NoiseSource>>,
    block: Vec<f32>,
    position: usize,
}

impl NoiseStream {
    fn new(source: Arc<Mutex<NoiseSource>>) -> Self {
        NoiseStream {
            source,
            block: vec![0.0; BLOCK_FRAMES * 2],
            position: BLOCK_FRAMES * 2,
        }
    }
}

impl Iterator for NoiseStream {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.position >= self.block.len() {
            // A poisoned lock still holds perfectly good noise state, and
            // dropping audio over it would be worse than carrying on.
            let mut source = self
                .source
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            source.fill(&mut self.block);
            self.position = 0;
        }
        let sample = self.block[self.position];
        self.position += 1;
        Some(sample)
    }
}

impl Source for NoiseStream {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        2
    }

    fn sample_rate(&self) -> u32 {
        SAMPLE_RATE
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

fn run(rx: mpsc::Receiver<Command>) {
    let device = OutputStream::try_default();
    let Ok((_stream, handle)) = device else {
        eprintln!("lock-in: no audio output device; running silently");
        drain(rx);
        return;
    };

    let (Ok(noise_sink), Ok(chime_sink)) = (Sink::try_new(&handle), Sink::try_new(&handle)) else {
        eprintln!("lock-in: could not open audio sinks; running silently");
        drain(rx);
        return;
    };

    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x5EED);
    let state = Arc::new(Mutex::new(NoiseSource::new(
        NoiseKind::Brown,
        SAMPLE_RATE,
        seed,
    )));

    noise_sink.append(NoiseStream::new(Arc::clone(&state)));
    noise_sink.pause();
    let mut noise_playing = false;

    loop {
        match rx.recv_timeout(POLL) {
            Ok(Command::SetNoise {
                enabled,
                kind,
                volume,
                tone,
            }) => {
                let mut source = state
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                if source.kind() != kind {
                    source.set_kind(kind);
                }
                source.set_tone(tone);
                let target = if enabled { volume } else { 0.0 };
                if (source.target_gain() - target).abs() > f32::EPSILON {
                    source.fade_to(target, DEFAULT_FADE_MS);
                }
            }
            Ok(Command::Chime {
                voice,
                frequency,
                gain,
            }) => {
                let samples = chime::render(voice, SAMPLE_RATE, frequency, gain);
                if !samples.is_empty() {
                    chime_sink.append(SamplesBuffer::new(2, SAMPLE_RATE, samples));
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }

        // Park the noise sink once a fade-out has landed, so an idle timer
        // is not feeding silence to the sound card forever.
        let silent = state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .is_silent();
        if silent && noise_playing {
            noise_sink.pause();
            noise_playing = false;
        } else if !silent && !noise_playing {
            noise_sink.play();
            noise_playing = true;
        }
    }
}

fn drain(rx: mpsc::Receiver<Command>) {
    while rx.recv().is_ok() {}
}
