//! Opt-in preparation and native draw observations, never physical delivery.
//! Producers only enqueue bounded records; the file is written off render threads.
use std::fs::OpenOptions;
use std::io::{self, BufWriter, Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc::{SyncSender, sync_channel},
};
use std::thread::{self, JoinHandle};

use serde_json::{Value, json};

static RECORDER: OnceLock<Recorder> = OnceLock::new();
static STOP: AtomicBool = AtomicBool::new(false);
const MAX_RECORDS: u64 = 65_536;
struct Recorder {
    send: SyncSender<Option<Value>>,
    worker: Mutex<Option<JoinHandle<io::Result<()>>>>,
    lost: Arc<AtomicU64>,
    attempts: AtomicU64,
    finished: AtomicBool,
}

pub(crate) fn enabled() -> bool {
    RECORDER
        .get()
        .is_some_and(|recorder| !recorder.finished.load(Ordering::Relaxed))
}
pub(crate) fn stop_requested() -> bool {
    STOP.load(Ordering::Acquire)
}
pub(crate) fn initialize() -> io::Result<()> {
    let Some(path) = std::env::var_os("ROONSCAPE_CONTENT_EVIDENCE") else {
        return Ok(());
    };
    let output = OpenOptions::new().write(true).create_new(true).open(path)?;
    let control = std::env::var_os("ROONSCAPE_CONTENT_EVIDENCE_CONTROL")
        .map(UnixStream::connect)
        .transpose()?;
    let (send, receive) = sync_channel::<Option<Value>>(1024);
    let lost = Arc::new(AtomicU64::new(0));
    let worker_lost = lost.clone();
    let worker = thread::spawn(move || {
        let mut output = BufWriter::new(output);
        write(
            &mut output,
            &json!({"event":"start","version":1,"clock":"CLOCK_MONOTONIC","pid":std::process::id(),"physicalDelivery":false}),
        )?;
        output.flush()?;
        let mut count = 0;
        while let Ok(Some(entry)) = receive.recv() {
            write(&mut output, &entry)?;
            output.flush()?;
            count += 1;
        }
        let lost = worker_lost.load(Ordering::Relaxed);
        write(
            &mut output,
            &json!({"event":"end","records":count,"lost":lost,"complete":lost==0}),
        )?;
        output.flush()?;
        if lost > 0 {
            return Err(io::Error::other(
                "Content evidence overflowed; observations are invalid",
            ));
        }
        Ok(())
    });
    RECORDER
        .set(Recorder {
            send,
            worker: Mutex::new(Some(worker)),
            lost,
            attempts: AtomicU64::new(0),
            finished: AtomicBool::new(false),
        })
        .map_err(|_| io::Error::other("Content evidence already initialized"))?;
    if let Some(mut control) = control {
        thread::spawn(move || {
            let mut byte = [0];
            if (control.read_exact(&mut byte).is_err() || byte[0] != b'F')
                && let Some(recorder) = RECORDER.get()
            {
                recorder.lost.fetch_add(1, Ordering::Relaxed);
            }
            STOP.store(true, Ordering::Release);
        });
    }
    Ok(())
}
pub(crate) fn record(entry: impl FnOnce() -> Value) {
    let Some(recorder) = RECORDER.get() else {
        return;
    };
    if recorder.finished.load(Ordering::Relaxed) {
        return;
    }
    if recorder.attempts.fetch_add(1, Ordering::Relaxed) >= MAX_RECORDS {
        recorder.lost.fetch_add(1, Ordering::Relaxed);
        return;
    }
    let mut entry = entry();
    entry["observedMicros"] = json!(crate::qt_window::clock_micros());
    if recorder.send.try_send(Some(entry)).is_err() {
        recorder.lost.fetch_add(1, Ordering::Relaxed);
    }
}
// Call after the native runtime, its preparation worker, and window have closed.
// Normal completion drains the queue. The command bounds process shutdown if the
// filesystem stalls; absence of a complete footer then invalidates that evidence.
pub(crate) fn finish() -> io::Result<()> {
    let Some(recorder) = RECORDER.get() else {
        return Ok(());
    };
    recorder.finished.store(true, Ordering::Relaxed);
    let _ = recorder.send.send(None);
    recorder
        .worker
        .lock()
        .unwrap()
        .take()
        .map_or(Ok(()), |worker| {
            worker
                .join()
                .map_err(|_| io::Error::other("Content evidence writer panicked"))?
        })
}
fn write(output: &mut impl Write, entry: &Value) -> io::Result<()> {
    let bytes = serde_json::to_vec(entry).map_err(io::Error::other)?;
    if bytes.len() > 65_536 {
        return Err(io::Error::other(
            "Content evidence record exceeds its size bound",
        ));
    }
    output.write_all(&bytes)?;
    output.write_all(b"\n")
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Drawing {
    target_artwork: Option<u64>,
    artwork: Vec<(u64, f32)>,
    progress: Option<f32>,
    ready: bool,
}
impl Drawing {
    pub(crate) fn capture(
        scene: &crate::qt_window::Scene<'_>,
        ready: bool,
        target: Option<u64>,
    ) -> Option<Self> {
        enabled().then(|| {
            let artwork: Vec<_> = scene
                .graphics
                .iter()
                .filter_map(|graphic| {
                    graphic
                        .artwork
                        .as_ref()
                        .filter(|_| graphic.geometry.weight > 0.0)
                        .map(|texture| (texture.identity(), graphic.geometry.weight))
                })
                .collect();
            let ready = ready
                && target
                    .is_none_or(|target| artwork.iter().any(|(resource, _)| *resource == target));
            let progress = scene
                .sprites
                .iter()
                .find(|sprite| sprite.geometry.kind == crate::qt_window::SpriteKind::Progress)
                .map(|sprite| sprite.geometry.uv.x);
            Self {
                target_artwork: target,
                artwork,
                progress,
                ready,
            }
        })
    }
}
