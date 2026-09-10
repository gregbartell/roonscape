//! Opt-in evidence records the immutable scene that actually drew. Frame swaps
//! are not physical presentation timestamps; external display observations are
//! associated through the exported native frame identity.
use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc::{SyncSender, sync_channel},
};
use std::thread::{self, JoinHandle};

use crate::qt_window::{GraphicGeometry, PaintedFrame, Scene, SpriteGeometry};
use roonscape_renderer::Viewport;
use serde::Serialize;
use serde_json::json;

static EVIDENCE: Mutex<Option<AnimationEvidence>> = Mutex::new(None);
static ENABLED: AtomicBool = AtomicBool::new(false);

#[derive(Serialize)]
struct GraphicValues {
    geometry: GraphicGeometry,
    artwork: Option<u64>,
    gradient: Option<u64>,
}
#[derive(Serialize)]
struct SpriteValues {
    geometry: SpriteGeometry,
    texture: Option<u64>,
    foreground: Option<u64>,
}
#[derive(Serialize)]
pub(crate) struct SceneValues {
    graphics: Vec<GraphicValues>,
    sprites: Vec<SpriteValues>,
}
impl SceneValues {
    fn capture(scene: &Scene<'_>) -> Self {
        Self {
            graphics: scene
                .graphics
                .iter()
                .map(|graphic| GraphicValues {
                    geometry: graphic.geometry,
                    artwork: graphic.artwork.as_ref().map(|texture| texture.identity()),
                    gradient: graphic.gradient.as_ref().map(|texture| texture.identity()),
                })
                .collect(),
            sprites: scene
                .sprites
                .iter()
                .filter(|sprite| sprite.geometry.color.alpha > 0.0)
                .map(|sprite| SpriteValues {
                    geometry: sprite.geometry,
                    texture: sprite.texture.as_ref().map(|texture| texture.identity()),
                    foreground: sprite.foreground.as_ref().map(|texture| texture.identity()),
                })
                .collect(),
        }
    }
}

enum Entry {
    Event(serde_json::Value),
    Painted {
        frame: PaintedFrame,
        viewport: Viewport,
        refresh: u32,
        active: bool,
        values: Arc<SceneValues>,
        observed_micros: i64,
    },
}
struct AnimationEvidence {
    send: SyncSender<Entry>,
    worker: JoinHandle<io::Result<()>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PaintedState<'a> {
    event: &'static str,
    source: &'static str,
    widget: &'static str,
    frame: u64,
    frame_time_micros: i64,
    effective_opacity: f64,
    mapped: bool,
    observed_micros: i64,
    values: &'a SceneValues,
}
impl AnimationEvidence {
    fn new(output: File) -> Self {
        // Bound diagnostic memory without letting a slow disk stall animation.
        // A full queue invalidates the run instead of dropping evidence.
        let (send, receive) = sync_channel(64);
        let worker = thread::spawn(move || {
            let mut output = BufWriter::new(output);
            let mut display_recorded = false;
            for entry in receive {
                match entry {
                    Entry::Event(value) => write(&mut output, &value)?,
                    Entry::Painted {
                        frame,
                        viewport,
                        refresh,
                        active,
                        values,
                        observed_micros,
                    } => {
                        if !display_recorded {
                            write(
                                &mut output,
                                &json!({"event":"display","width":viewport.width_px,"height":viewport.height_px,
                                "renderer":"Qt Quick OpenGL","monitorRefreshMillihertz":refresh,"frameTimeSource":"render-start"}),
                            )?;
                            display_recorded = true;
                        }
                        write(
                            &mut output,
                            &json!({"event":"state","source":"scheduled-update","widget":"presentation",
                                "frame":frame.frame,"frameTimeMicros":frame.time_micros,"effectiveOpacity":1.0,"mapped":true,
                                "observedMicros":observed_micros,"values":{"animationActive":active}}),
                        )?;
                        write(
                            &mut output,
                            &PaintedState {
                                event: "state",
                                source: "composition-painted",
                                widget: "presentation",
                                frame: frame.frame,
                                frame_time_micros: frame.time_micros,
                                effective_opacity: 1.0,
                                mapped: true,
                                observed_micros,
                                values: &values,
                            },
                        )?;
                        write(
                            &mut output,
                            &json!({"event":"after-paint","frame":frame.frame,"scene":frame.scene,
                            "frameTimeMicros":frame.time_micros,"observedMicros":observed_micros}),
                        )?;
                        output.flush()?;
                    }
                }
            }
            output.flush()
        });
        Self { send, worker }
    }

    fn enqueue(&self, entry: Entry) {
        if let Err(error) = self.send.try_send(entry) {
            panic!("Animation evidence cannot keep up; this run is incomplete: {error}");
        }
    }

    fn finish(self) -> io::Result<()> {
        drop(self.send);
        self.worker
            .join()
            .map_err(|_| io::Error::other("Animation evidence writer panicked"))?
    }
}
fn write(output: &mut impl Write, entry: &impl Serialize) -> io::Result<()> {
    serde_json::to_writer(&mut *output, entry).map_err(io::Error::other)?;
    output.write_all(b"\n")
}

pub(crate) fn initialize() -> io::Result<()> {
    let Some(path) = std::env::var_os("ROONSCAPE_ANIMATION_EVIDENCE") else {
        return Ok(());
    };
    let output = OpenOptions::new().write(true).create_new(true).open(path)?;
    *EVIDENCE.lock().unwrap() = Some(AnimationEvidence::new(output));
    ENABLED.store(true, Ordering::Relaxed);
    Ok(())
}

pub(crate) fn scene_values(scene: &Scene<'_>) -> Option<Arc<SceneValues>> {
    ENABLED
        .load(Ordering::Relaxed)
        .then(|| Arc::new(SceneValues::capture(scene)))
}

pub(crate) fn record(entry: impl FnOnce() -> serde_json::Value) {
    if ENABLED.load(Ordering::Relaxed)
        && let Some(evidence) = EVIDENCE.lock().unwrap().as_ref()
    {
        evidence.enqueue(Entry::Event(entry()));
    }
}

pub(crate) fn painted(
    frame: PaintedFrame,
    viewport: Viewport,
    refresh: u32,
    active: bool,
    values: &Arc<SceneValues>,
) {
    if ENABLED.load(Ordering::Relaxed)
        && let Some(evidence) = EVIDENCE.lock().unwrap().as_ref()
    {
        evidence.enqueue(Entry::Painted {
            frame,
            viewport,
            refresh,
            active,
            values: values.clone(),
            observed_micros: crate::qt_window::clock_micros(),
        });
    }
}

pub(crate) fn finish() -> io::Result<()> {
    ENABLED.store(false, Ordering::Relaxed);
    let evidence = EVIDENCE.lock().unwrap().take();
    evidence.map_or(Ok(()), AnimationEvidence::finish)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qt_window::Graphic;

    #[test]
    fn background_writer_preserves_drawn_values_and_observation_identity() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("evidence.jsonl");
        let evidence = AnimationEvidence::new(File::create(&path).unwrap());
        let mut scene = Scene {
            graphics: vec![Graphic {
                geometry: GraphicGeometry {
                    weight: 0.25,
                    ..GraphicGeometry::default()
                },
                ..Graphic::default()
            }],
            sprites: vec![],
        };
        evidence.enqueue(Entry::Event(
            json!({"event":"snapshot-received","revision":7}),
        ));
        evidence.enqueue(Entry::Painted {
            frame: PaintedFrame {
                frame: 43,
                scene: 27,
                time_micros: 1_000_000,
            },
            viewport: Viewport::new(1280, 720),
            refresh: 60_000,
            active: true,
            values: Arc::new(SceneValues::capture(&scene)),
            observed_micros: 1_005_000,
        });
        scene.graphics[0].geometry.weight = 1.0;
        evidence.finish().unwrap();
        let entries: Vec<serde_json::Value> = std::fs::read_to_string(path)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(entries.len(), 5);
        assert_eq!(entries[0]["revision"], 7);
        assert_eq!(entries[1]["event"], "display");
        assert_eq!(entries[1]["frameTimeSource"], "render-start");
        assert_eq!(entries[2]["source"], "scheduled-update");
        assert_eq!(entries[3]["source"], "composition-painted");
        assert_eq!(
            entries[3]["values"]["graphics"][0]["geometry"]["weight"],
            0.25
        );
        for entry in &entries[2..] {
            assert_eq!(entry["frame"], 43);
            assert_eq!(entry["frameTimeMicros"], 1_000_000);
            assert_eq!(entry["observedMicros"], 1_005_000);
        }
        assert_eq!(entries[4]["scene"], 27);
    }
}
