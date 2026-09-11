#[allow(dead_code)]
#[path = "../src/content_evidence.rs"]
mod content_evidence;
// The production preparation and presentation code runs with one real upload
// context. Virtual presentation times make transition assertions deterministic.
#[allow(dead_code)]
#[path = "../src/lyric_motion.rs"]
mod lyric_motion;
#[allow(dead_code)]
#[path = "../src/native_view.rs"]
mod native_view;
#[allow(dead_code)]
#[path = "../src/prepared_presentation.rs"]
mod prepared_presentation;
#[allow(dead_code)]
#[path = "../src/qt_window.rs"]
mod qt_window;
#[allow(dead_code)]
#[path = "../src/scene.rs"]
mod scene;
#[allow(dead_code)]
#[path = "../src/status_glyph.rs"]
mod status_glyph;
#[allow(dead_code)]
#[path = "../src/text_preparation.rs"]
mod text_preparation;

use gtk::prelude::*;
use native_view::NativeView;
use prepared_presentation::{PreparedContent, PresentationPreparation};
use qt_window::{Scene, SpriteKind, Window};
use roonscape_renderer::{
    NowPlayingTitleFace, Presentation, PresentationStatusSymbol, Viewport, parse_snapshot,
    presentation_from_snapshot, register_packaged_fallback_fonts, select_capture_typography,
};
use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;

fn fixture(name: &str) -> Presentation {
    let source = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../shared/fixtures/{name}.json")),
    )
    .unwrap();
    presentation_from_snapshot(&parse_snapshot(&source).unwrap()).unwrap()
}
fn ms(value: u64) -> Duration {
    Duration::from_millis(value)
}
fn close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < 0.001,
        "expected {expected}, got {actual}"
    );
}
fn progress_alpha(scene: &Scene<'_>) -> f32 {
    scene
        .sprites
        .iter()
        .find(|sprite| sprite.geometry.kind == SpriteKind::Progress)
        .unwrap()
        .geometry
        .color
        .alpha
}
fn graphics_complete(scene: &Scene<'_>) {
    close(scene.graphics.iter().map(|g| g.geometry.weight).sum(), 1.0);
}

fn transitions(preparation: &mut PresentationPreparation<'_>, repository: &Path) {
    let viewport = Viewport::new(1280, 720);
    let playing = fixture("playing");
    let prepared = preparation
        .prepare(&playing, viewport, repository, true)
        .unwrap();
    let mut view = NativeView::new(prepared, 1);
    close(progress_alpha(&view.render(ms(0), true)), 1.0);
    view.begin_departure(ms(10), true);
    // Invisible destinations inherit the original departure deadline.
    view.begin_departure(ms(100), true);
    assert!(!view.departure_complete(ms(234), true));
    assert!(view.departure_complete(ms(235), true));
    let mut live = playing.clone();
    if let Presentation::NowPlaying(value) = &mut live {
        value.progress.as_mut().unwrap().fraction = 0.8;
    }
    view.update(&live, 2);
    let departing = view.render(ms(120), true);
    assert!(progress_alpha(&departing) > 0.4 && progress_alpha(&departing) < 0.6);
    close(
        departing
            .sprites
            .iter()
            .find(|sprite| sprite.geometry.kind == SpriteKind::Progress)
            .unwrap()
            .geometry
            .uv
            .x,
        0.8,
    );
    graphics_complete(&departing);

    let mut replacement = playing.clone();
    if let Presentation::NowPlaying(value) = &mut replacement {
        value.title = Some("A new song".into());
    }
    let prepared = preparation
        .prepare(&replacement, viewport, repository, true)
        .unwrap();
    view.replace(prepared, &replacement, 3, ms(235), true);
    let zero = view.render(ms(235), true);
    close(progress_alpha(&zero), 0.0);
    graphics_complete(&zero);
    let middle = view.render(ms(347), true);
    assert!(progress_alpha(&middle) > 0.49 && progress_alpha(&middle) < 0.51);
    graphics_complete(&middle);
    close(progress_alpha(&view.render(ms(460), true)), 1.0);

    let incoming = fixture("light-artwork");
    let prepared = preparation
        .prepare(&incoming, viewport, repository, true)
        .unwrap();
    view.install(prepared, ms(500), true);
    let middle = view.render(ms(612), true);
    assert_eq!(middle.graphics.len(), 2);
    close(
        middle.graphics[0].geometry.weight,
        1.0 - scene::motion_phase(112.0 / 225.0, 0.0, 1.0) as f32,
    );
    close(
        middle.graphics[1].geometry.weight,
        scene::motion_phase(112.0 / 225.0, 0.0, 1.0) as f32,
    );
    // Starting another artwork fade retains the current weighted appearance.
    let prepared = preparation
        .prepare(&playing, viewport, repository, true)
        .unwrap();
    view.install(prepared, ms(612), true);
    let interrupted = view.render(ms(612), true);
    graphics_complete(&interrupted);
    close(
        interrupted.graphics[0].geometry.weight,
        1.0 - scene::motion_phase(112.0 / 225.0, 0.0, 1.0) as f32,
    );
    close(
        interrupted.graphics[1].geometry.weight,
        scene::motion_phase(112.0 / 225.0, 0.0, 1.0) as f32,
    );
    assert_eq!(
        interrupted.graphics.len(),
        2,
        "unrevealed textures do not occupy the displayed scene"
    );
    let settled = view.render(ms(837), true);
    assert_eq!(settled.graphics.len(), 1);
    close(progress_alpha(&settled), 1.0);

    let full_field = fixture("disconnected");
    let prepared = preparation
        .prepare(&full_field, viewport, repository, true)
        .unwrap();
    view.replace(prepared, &full_field, 4, ms(1200), true);
    for time in [1200, 1300, 1425] {
        let scene = view.render(ms(time), true);
        graphics_complete(&scene);
        assert!(
            scene
                .sprites
                .iter()
                .all(|sprite| sprite.geometry.color.alpha == 0.0)
        );
    }
    let reveal = view.render(ms(1540), true);
    assert!(
        reveal
            .sprites
            .iter()
            .any(|sprite| sprite.geometry.color.alpha > 0.4)
    );
    let settled = view.render(ms(1650), true);
    assert_eq!(settled.graphics.len(), 1);
    assert!(!view.active(ms(1650)));
    println!(
        "native transitions preserve departure, coordinated reveals, live timing, and interrupted artwork"
    );
}

fn lyrics(preparation: &mut PresentationPreparation<'_>, repository: &Path) {
    let viewport = Viewport::new(1280, 720);
    let mut presentation = fixture("lyrics-reel-capacity");
    if let Presentation::NowPlaying(value) = &mut presentation {
        let lyrics = value.lyrics.as_mut().unwrap();
        lyrics.timeline = vec!["Before the pause".into()];
        lyrics.timeline.extend((0..100).map(|_| String::new()));
        lyrics.timeline.push("The next visible cue".into());
        lyrics.current_index = 1;
    }
    let prepared = preparation
        .prepare(&presentation, viewport, repository, true)
        .unwrap();
    let PreparedContent::NowPlaying(content) = &prepared.content else {
        panic!("Now Playing");
    };
    let reel = content.reel.as_ref().unwrap();
    assert!(reel.cues.contains_key(&0));
    assert!(
        reel.cues.contains_key(&101),
        "blank runs must not consume the prepared context budget"
    );
    let mut view = NativeView::new(prepared, 1);
    assert!(view.lyrics_ready(&presentation));
    let mut destination = presentation.clone();
    if let Presentation::NowPlaying(value) = &mut destination {
        value.lyrics.as_mut().unwrap().timeline_signature += 1;
    }
    assert!(
        !view.lyrics_ready(&destination),
        "old timeline indices cannot authorize a new timeline"
    );
    let before = view.render(ms(0), true);
    let mut without_lyrics = presentation.clone();
    if let Presentation::NowPlaying(value) = &mut without_lyrics {
        value.lyrics = None;
    }
    let prepared = preparation
        .prepare(&without_lyrics, viewport, repository, true)
        .unwrap();
    view.install(prepared, ms(10), true);
    view.update(&without_lyrics, 2);
    let departure = view.render(ms(10), true);
    assert_eq!(
        departure.sprites.len(),
        before.sprites.len(),
        "the departing reel retains its prepared drawings"
    );
    println!("native lyric preparation crosses blank runs and preserves departing context");
}

fn word_motion_meets_native_endpoints(
    preparation: &mut PresentationPreparation<'_>,
    repository: &Path,
) {
    use qt_window::{Color, Rect};
    for title in [
        "Last Light on Phobos",
        "ليلة القمر — Office affinities 東京の夜",
    ] {
        let viewport = Viewport::new(3840, 2160);
        let mut presentation = fixture("lyrics-reel-capacity");
        let Presentation::NowPlaying(value) = &mut presentation else {
            panic!("Now Playing")
        };
        value.title = Some(title.into());
        let prepared = preparation
            .prepare(&presentation, viewport, repository, true)
            .unwrap();
        let PreparedContent::NowPlaying(content) = &prepared.content else {
            panic!("Now Playing")
        };
        let movement = &content.metadata.movement[0];
        let layout = roonscape_renderer::NowPlayingLayout::for_composition_progress(
            match &presentation {
                Presentation::NowPlaying(value) => value,
                _ => unreachable!(),
            },
            viewport,
            0.0,
        );
        assert!(
            movement.ordinary.size * movement.normalized_extents[0]
                <= layout.information.musical_metadata_width_px as f32 + 3.0,
            "ordinary word bounds exceed native rail: size={} extent={} width={}",
            movement.ordinary.size,
            movement.normalized_extents[0],
            layout.information.musical_metadata_width_px
        );
        for word in movement.ordinary.words.iter() {
            let path = movement
                .paths
                .iter()
                .find(|path| {
                    path.source
                        == [
                            word.x / movement.ordinary.size,
                            movement.ordinary.y + word.baseline,
                        ]
                })
                .unwrap();
            if !path.visible[1] {
                continue;
            }
            let native = word
                .text
                .sprite(0.0, 0.0, Color::default(), Rect::viewport(viewport))
                .geometry
                .bounds;
            let height = path.bounds.height * movement.ordinary.size;
            assert!(
                (height - native.height).abs() < 4.0,
                "{title}: endpoint height jumps {} -> {}",
                height,
                native.height
            );
        }
    }
}

fn composition_keeps_shared_metadata_visible(
    preparation: &mut PresentationPreparation<'_>,
    repository: &Path,
) {
    let viewport = Viewport::new(1280, 720);
    let presentation = fixture("lyrics-reel-capacity");
    let prepared = preparation
        .prepare(&presentation, viewport, repository, true)
        .unwrap();
    let Presentation::NowPlaying(value) = &presentation else {
        panic!("Now Playing")
    };
    let mut motion = lyric_motion::LyricMotion::new(1, None);
    motion.update(1, value.lyrics.as_deref(), ms(0), true);
    let mut frame = motion.frame_at(ms(290));
    frame.cues.clear();
    let mut scene = Scene::default();
    scene::foreground(
        &mut scene,
        &prepared,
        &scene::Foreground {
            presentation: &presentation,
            palette: prepared.palette,
            now: ms(290),
            animated: true,
            opacity: 1.0,
            metadata_opacity: 1.0,
            status: value.status,
            status_opacity: 1.0,
            timing_opacity: 1.0,
            lyrics: Some(&frame),
        },
    );
    // Title remains legible throughout travel, even before the reel arrives.
    let title = scene
        .sprites
        .iter()
        .filter(|sprite| {
            sprite.geometry.color.red == f32::from(prepared.palette.primary_text.red) / 255.0
                && sprite.texture.is_some()
        })
        .collect::<Vec<_>>();
    assert!(
        title
            .iter()
            .any(|sprite| sprite.geometry.color.alpha == 1.0),
        "shared Title words must not fade during composition movement"
    );
}

fn composition_preserves_clipped_title_at_motion_boundary(
    preparation: &mut PresentationPreparation<'_>,
    repository: &Path,
) {
    let viewport = Viewport::new(1280, 720);
    let mut presentation = fixture("lyrics-reel-capacity");
    let Presentation::NowPlaying(value) = &mut presentation else {
        panic!("Now Playing")
    };
    value.title = Some(format!(
        "{} alpha beta gamma delta epsilon",
        "W".repeat(200)
    ));
    value.artist = None;
    value.album = None;
    let prepared = preparation
        .prepare(&presentation, viewport, repository, true)
        .unwrap();
    let PreparedContent::NowPlaying(content) = &prepared.content else {
        panic!("Now Playing")
    };
    let Presentation::NowPlaying(value) = &presentation else {
        unreachable!()
    };
    let layout =
        roonscape_renderer::NowPlayingLayout::for_composition_progress(value, viewport, 0.0);
    let first_word = &content.metadata.movement[0].ordinary.words[0];
    let first_texture = first_word
        .text
        .sprite(
            0.0,
            0.0,
            qt_window::Color::default(),
            qt_window::Rect::viewport(viewport),
        )
        .texture
        .unwrap()
        .identity();
    assert!(
        first_word.text.bounds.width > layout.information.musical_metadata_width_px as f32,
        "the native ordinary endpoint clips the overlong first token"
    );
    let mut motion = lyric_motion::LyricMotion::new(1, None);
    motion.update(1, value.lyrics.as_deref(), ms(0), true);
    let mut frame = motion.frame_at(ms(0));
    frame.composition_progress = 0.12001;
    frame.cues.clear();
    let mut scene = Scene::default();
    scene::foreground(
        &mut scene,
        &prepared,
        &scene::Foreground {
            presentation: &presentation,
            palette: prepared.palette,
            now: ms(0),
            animated: true,
            opacity: 1.0,
            metadata_opacity: 1.0,
            status: value.status,
            status_opacity: 1.0,
            timing_opacity: 1.0,
            lyrics: Some(&frame),
        },
    );
    let moving = &scene
        .sprites
        .iter()
        .find(|sprite| {
            sprite.texture.as_ref().map(|texture| texture.identity()) == Some(first_texture)
        })
        .expect("the first Title word remains visible")
        .geometry;
    assert!(
        (moving.bounds.height - first_word.text.bounds.height).abs() < 1.0,
        "movement must retain native clipped font size: {} -> {}",
        first_word.text.bounds.height,
        moving.bounds.height
    );
    assert!(
        (moving.clip.x + moving.clip.width
            - (layout.information.left_viewport_x_px + layout.information.musical_metadata_width_px)
                as f32)
            .abs()
            < 1.0,
        "movement must retain native horizontal clipping"
    );
}

fn composition_reel_translation_and_clipping(
    preparation: &mut PresentationPreparation<'_>,
    repository: &Path,
) {
    use qt_window::{Color, Rect};
    for viewport in [
        Viewport::new(1280, 720),
        Viewport::new(1600, 1200),
        Viewport::new(3840, 2160),
    ] {
        let presentation = fixture("lyrics-reel-capacity");
        let prepared = preparation
            .prepare(&presentation, viewport, repository, true)
            .unwrap();
        let PreparedContent::NowPlaying(content) = &prepared.content else {
            panic!("Now Playing")
        };
        let reel = content.reel.as_ref().unwrap();
        let textures: Vec<_> = reel
            .cues
            .values()
            .map(|text| {
                text.sprite(0.0, 0.0, Color::default(), Rect::viewport(viewport))
                    .texture
                    .unwrap()
                    .identity()
            })
            .collect();
        let Presentation::NowPlaying(value) = &presentation else {
            panic!("Now Playing")
        };
        let mut motion = lyric_motion::LyricMotion::new(1, None);
        motion.update(1, value.lyrics.as_deref(), ms(0), true);
        let render = |frame: &lyric_motion::LyricFrame| {
            let mut result = Scene::default();
            scene::foreground(
                &mut result,
                &prepared,
                &scene::Foreground {
                    presentation: &presentation,
                    palette: prepared.palette,
                    now: ms(400),
                    animated: true,
                    opacity: 1.0,
                    metadata_opacity: 1.0,
                    status: value.status,
                    status_opacity: 1.0,
                    timing_opacity: 1.0,
                    lyrics: Some(frame),
                },
            );
            result
        };
        let frame = motion.frame_at(ms(400));
        let moving = render(&frame);
        let mut settled_frame = frame.clone();
        settled_frame.composition_progress = 1.0;
        let settled = render(&settled_frame);
        let mut translations = Vec::new();
        for identity in &textures {
            let sprite = |scene: &Scene<'_>| {
                scene
                    .sprites
                    .iter()
                    .find(|sprite| {
                        sprite
                            .texture
                            .as_ref()
                            .is_some_and(|texture| texture.identity() == *identity)
                    })
                    .map(|sprite| sprite.geometry)
            };
            if let (Some(moving), Some(settled)) = (sprite(&moving), sprite(&settled)) {
                close(moving.color.alpha, settled.color.alpha);
                close(moving.bounds.width, settled.bounds.width);
                translations.push(moving.bounds.y - settled.bounds.y);
                assert!(moving.clip.y >= reel.top);
                assert!(moving.clip.y + moving.clip.height <= reel.bottom + 0.01);
                close(moving.fade_bottom, settled.fade_bottom);
            }
        }
        assert!(translations.len() >= 2);
        assert!(translations[0] > 0.0);
        let first_translation = translations[0];
        for translation in translations {
            close(translation, first_translation);
        }
        let before = render(&motion.frame_at(ms(290)));
        motion.update(1, None, ms(290), true);
        let reversed = render(&motion.frame_at(ms(290)));
        assert_eq!(
            serde_json::to_value(
                before
                    .sprites
                    .iter()
                    .map(|s| s.geometry)
                    .collect::<Vec<_>>()
            )
            .unwrap(),
            serde_json::to_value(
                reversed
                    .sprites
                    .iter()
                    .map(|s| s.geometry)
                    .collect::<Vec<_>>()
            )
            .unwrap(),
            "reversal must begin at the displayed scene"
        );
        let ordinary = render(&motion.frame_at(ms(870)));
        assert!(
            ordinary.sprites.iter().all(|sprite| sprite
                .texture
                .as_ref()
                .is_none_or(|texture| !textures.contains(&texture.identity()))),
            "empty lyric clip must emit no cue sprites"
        );
    }
}

fn cue_preparation_reuses_metadata_words(
    preparation: &mut PresentationPreparation<'_>,
    repository: &Path,
) {
    use qt_window::{Color, Rect};
    let viewport = Viewport::new(1280, 720);
    let mut presentation = fixture("lyrics-reel-capacity");
    let Presentation::NowPlaying(value) = &mut presentation else {
        panic!("Now Playing")
    };
    value.title = Some("go ".repeat(70).trim().to_owned());
    value.artist = Some("la ".repeat(120).trim().to_owned());
    let first = preparation
        .prepare(&presentation, viewport, repository, true)
        .unwrap();
    let PreparedContent::NowPlaying(content) = &first.content else {
        panic!("Now Playing")
    };
    let identity = content.metadata.movement[0].ordinary.words[0]
        .text
        .sprite(0.0, 0.0, Color::default(), Rect::viewport(viewport))
        .texture
        .unwrap()
        .identity();
    let Presentation::NowPlaying(value) = &mut presentation else {
        panic!("Now Playing")
    };
    value.lyrics.as_mut().unwrap().current_index += 1;
    let next = preparation
        .prepare(&presentation, viewport, repository, true)
        .unwrap();
    let PreparedContent::NowPlaying(content) = &next.content else {
        panic!("Now Playing")
    };
    let next_identity = content.metadata.movement[0].ordinary.words[0]
        .text
        .sprite(0.0, 0.0, Color::default(), Rect::viewport(viewport))
        .texture
        .unwrap()
        .identity();
    assert_eq!(
        identity, next_identity,
        "cue preparation must reuse metadata word textures even for dense credits"
    );
    let texture_identity = |prepared: &prepared_presentation::PreparedPresentation<'_>| {
        let PreparedContent::NowPlaying(content) = &prepared.content else {
            panic!("Now Playing")
        };
        content.metadata.movement[0].ordinary.words[0]
            .text
            .sprite(0.0, 0.0, Color::default(), Rect::viewport(viewport))
            .texture
            .unwrap()
            .identity()
    };
    for index in 0..10 {
        let mut changed = presentation.clone();
        let Presentation::NowPlaying(value) = &mut changed else {
            panic!("Now Playing")
        };
        value.title = Some(format!("Changed title {index}"));
        let changed = preparation
            .prepare(&changed, viewport, repository, true)
            .unwrap();
        assert_ne!(
            identity,
            texture_identity(&changed),
            "changed text invalidates its words"
        );
    }
    let refreshed = preparation
        .prepare(&presentation, viewport, repository, true)
        .unwrap();
    assert_ne!(
        identity,
        texture_identity(&refreshed),
        "old endpoints are evicted from the bounded cache"
    );
    let resized = preparation
        .prepare(&presentation, Viewport::new(1600, 1200), repository, true)
        .unwrap();
    assert_ne!(
        texture_identity(&refreshed),
        texture_identity(&resized),
        "viewport fitting invalidates native word geometry"
    );
}

fn wrapping_words_do_not_collide_during_composition(
    preparation: &mut PresentationPreparation<'_>,
    repository: &Path,
) {
    use qt_window::{Color, Rect};
    let viewport = Viewport::new(1280, 720);
    let presentation = fixture("lyrics-reel-capacity");
    let prepared = preparation
        .prepare(&presentation, viewport, repository, true)
        .unwrap();
    let PreparedContent::NowPlaying(content) = &prepared.content else {
        panic!("Now Playing")
    };
    let words = &content.metadata.movement[0].ordinary.words;
    assert_eq!(
        words.iter().map(|word| word.occurrence).collect::<Vec<_>>(),
        vec![Some(0), Some(1), Some(2), Some(3)]
    );
    let textures: Vec<_> = words
        .iter()
        .map(|word| {
            word.text
                .sprite(0.0, 0.0, Color::default(), Rect::viewport(viewport))
                .texture
                .unwrap()
                .identity()
        })
        .collect();
    let Presentation::NowPlaying(value) = &presentation else {
        panic!("Now Playing")
    };
    let mut motion = lyric_motion::LyricMotion::new(1, None);
    motion.update(1, value.lyrics.as_deref(), ms(0), true);
    for time in [290, 319, 348, 377] {
        let mut frame = motion.frame_at(ms(time));
        frame.cues.clear();
        let mut scene = Scene::default();
        scene::foreground(
            &mut scene,
            &prepared,
            &scene::Foreground {
                presentation: &presentation,
                palette: prepared.palette,
                now: ms(time),
                animated: true,
                opacity: 1.0,
                metadata_opacity: 1.0,
                status: value.status,
                status_opacity: 1.0,
                timing_opacity: 1.0,
                lyrics: Some(&frame),
            },
        );
        let bounds: Vec<_> = textures
            .iter()
            .map(|identity| {
                scene
                    .sprites
                    .iter()
                    .find(|sprite| {
                        sprite
                            .texture
                            .as_ref()
                            .is_some_and(|texture| texture.identity() == *identity)
                    })
                    .unwrap()
                    .geometry
                    .bounds
            })
            .collect();
        for (index, a) in bounds.iter().enumerate() {
            for (other, b) in bounds.iter().enumerate().skip(index + 1) {
                if words[index].baseline == words[other].baseline {
                    continue;
                }
                let overlap_x = (a.x + a.width).min(b.x + b.width) - a.x.max(b.x);
                let overlap_y = (a.y + a.height).min(b.y + b.height) - a.y.max(b.y);
                assert!(
                    overlap_x <= 2.0 || overlap_y <= 2.0,
                    "line-changing Title words collide at {time} ms: {index}/{other}"
                );
            }
        }
    }
}

fn partially_retained_words_follow_native_ellipsis(
    preparation: &mut PresentationPreparation<'_>,
    repository: &Path,
) {
    use qt_window::{Color, Rect};
    let viewport = Viewport::new(1280, 720);
    let mut presentation = fixture("lyrics-reel-capacity");
    let Presentation::NowPlaying(value) = &mut presentation else {
        panic!("Now Playing")
    };
    value.title = Some("affinity ".repeat(18).trim().to_owned());
    let prepared = preparation
        .prepare(&presentation, viewport, repository, true)
        .unwrap();
    let PreparedContent::NowPlaying(content) = &prepared.content else {
        panic!("Now Playing")
    };
    let movement = &content.metadata.movement[0];
    let (source, destination) = movement
        .ordinary
        .words
        .iter()
        .find_map(|source| {
            let destination = movement
                .compact
                .words
                .iter()
                .find(|word| word.occurrence == source.occurrence)?;
            (source.visible_bytes > destination.visible_bytes && destination.occurrence.is_some())
                .then_some((source, destination))
        })
        .expect("fixture must exercise a native partially retained word");
    let source_texture = source
        .text
        .sprite(0.0, 0.0, Color::default(), Rect::viewport(viewport))
        .texture
        .unwrap()
        .identity();
    let Presentation::NowPlaying(value) = &presentation else {
        panic!("Now Playing")
    };
    let mut motion = lyric_motion::LyricMotion::new(1, None);
    motion.update(1, value.lyrics.as_deref(), ms(0), true);
    let mut frame = motion.frame_at(ms(487));
    frame.cues.clear();
    let mut scene = Scene::default();
    scene::foreground(
        &mut scene,
        &prepared,
        &scene::Foreground {
            presentation: &presentation,
            palette: prepared.palette,
            now: ms(487),
            animated: true,
            opacity: 1.0,
            metadata_opacity: 1.0,
            status: value.status,
            status_opacity: 1.0,
            timing_opacity: 1.0,
            lyrics: Some(&frame),
        },
    );
    let sprite = scene
        .sprites
        .iter()
        .find(|sprite| {
            sprite
                .texture
                .as_ref()
                .is_some_and(|texture| texture.identity() == source_texture)
        })
        .expect("retained word travels continuously");
    let layout =
        roonscape_renderer::NowPlayingLayout::for_composition_progress(value, viewport, 1.0);
    let endpoint_right = layout.information.left_viewport_x_px as f32
        + destination.text.bounds.x
        + destination.text.bounds.width;
    let visible_right = (sprite.geometry.bounds.x + sprite.geometry.bounds.width)
        .min(sprite.geometry.clip.x + sprite.geometry.clip.width);
    assert!(
        visible_right <= endpoint_right + 1.0,
        "partial word must approach native ellipsis, got {visible_right} beyond {endpoint_right}"
    );
}

fn compatible_content(preparation: &mut PresentationPreparation<'_>, repository: &Path) {
    let viewport = Viewport::new(1280, 720);
    let mut playing = fixture("playing");
    if let Presentation::NowPlaying(value) = &mut playing {
        value.album = None;
    }
    let prepared = preparation
        .prepare(&playing, viewport, repository, true)
        .unwrap();
    let mut view = NativeView::new(prepared, 1);
    view.render(ms(0), true);
    let incoming = fixture("playing");
    let prepared = preparation
        .prepare(&incoming, viewport, repository, true)
        .unwrap();
    let PreparedContent::NowPlaying(content) = &prepared.content else {
        panic!("Now Playing");
    };
    let album = content
        .metadata
        .ordinary
        .last()
        .unwrap()
        .text
        .sprite(
            0.0,
            0.0,
            qt_window::Color::default(),
            qt_window::Rect::viewport(viewport),
        )
        .texture
        .unwrap()
        .identity();
    view.install(prepared, ms(10), true);
    view.update(&incoming, 2);
    view.render(ms(10), true);
    let early = view.render(ms(120), true);
    close(progress_alpha(&early), 1.0);
    assert!(!early.sprites.iter().any(|sprite| {
        sprite
            .texture
            .as_ref()
            .is_some_and(|texture| texture.identity() == album)
    }));
    let swapped = view.render(ms(235), true);
    close(progress_alpha(&swapped), 1.0);
    assert!(swapped.sprites.iter().any(|sprite| {
        sprite
            .texture
            .as_ref()
            .is_some_and(|texture| texture.identity() == album)
            && sprite.geometry.color.alpha == 0.0
    }));
    let settled = view.render(ms(460), true);
    assert!(settled.sprites.iter().any(|sprite| {
        sprite
            .texture
            .as_ref()
            .is_some_and(|texture| texture.identity() == album)
            && sprite.geometry.color.alpha == 1.0
    }));

    let mut activity = incoming.clone();
    if let (Presentation::NowPlaying(value), Presentation::NowPlaying(reference)) =
        (&mut activity, fixture("indeterminate-progress"))
    {
        value.progress = None;
        value.activity = reference.activity;
    }
    let prepared = preparation
        .prepare(&activity, viewport, repository, true)
        .unwrap();
    view.install(prepared, ms(500), true);
    view.update(&activity, 3);
    view.render(ms(500), true);
    assert!(progress_alpha(&view.render(ms(610), true)) < 0.6);
    let swapped = view.render(ms(725), true);
    assert!(
        swapped
            .sprites
            .iter()
            .all(|sprite| sprite.geometry.kind != SpriteKind::Progress)
    );
    let settled = view.render(ms(950), true);
    assert!(
        settled
            .sprites
            .iter()
            .filter(|sprite| sprite.geometry.kind == SpriteKind::RoundedRect
                && sprite.geometry.color.alpha > 0.9)
            .count()
            >= 7
    );
    // Returning authoritative numeric timing is immediate, including a seek.
    view.update(&incoming, 4);
    close(progress_alpha(&view.render(ms(960), true)), 1.0);
    println!("native metadata and timing replacements preserve independent live groups");
}

fn fitted_text(text: &text_preparation::TextPreparation<'_>) {
    use text_preparation::Face;
    for (face, weight, condensed) in [
        (Face::Title, 700, false),
        (Face::Artist, 600, false),
        (Face::Album, 400, false),
        (Face::IdentityLabel, 600, true),
        (Face::Time, 560, true),
    ] {
        let layout = text.layout("Typography", face, 24, 0);
        let font = layout.font_description().unwrap();
        let actual: i32 = gtk::glib::translate::IntoGlib::into_glib(font.weight());
        assert_eq!(actual, weight);
        assert_eq!(font.variations().as_deref(), condensed.then_some("wdth=96"));
    }
    let first_layout = text.layout("Reusable lyric", Face::Artist, 42, 0);
    let first = text.rasterize(&first_layout, 1.0).unwrap();
    let second_layout = text.layout("Reusable lyric", Face::Artist, 42, 0);
    let second = text.rasterize(&second_layout, 1.0).unwrap();
    let texture_id = |drawing: &text_preparation::PreparedText<'_>| {
        drawing
            .sprite(
                0.0,
                0.0,
                qt_window::Color::default(),
                qt_window::Rect::default(),
            )
            .texture
            .unwrap()
            .identity()
    };
    assert_eq!(
        texture_id(&first),
        texture_id(&second),
        "equal layouts reuse their immutable drawing"
    );
    let changed = text
        .rasterize(&text.layout("Reusable lyric", Face::Artist, 43, 0), 1.0)
        .unwrap();
    assert_ne!(texture_id(&first), texture_id(&changed));
    let value = "We cross the state-of-the-art horizon";
    let prepared = text.cue(value, 390, 42, 240.0).unwrap();
    assert!(!prepared.ellipsized);
    assert!(
        prepared
            .lines
            .iter()
            .any(|line| line.contains("state-of-the-art"))
    );
    let long = (0..35)
        .map(|index| format!("Complete multilingual cue {index}: café 世界"))
        .collect::<Vec<_>>()
        .join("\n");
    let prepared = text.cue(&long, 480, 42, 240.0).unwrap();
    assert!(prepared.height <= 240.01);
    assert!(!prepared.ellipsized);
    assert_eq!(
        prepared
            .lines
            .join(" ")
            .split_whitespace()
            .collect::<Vec<_>>(),
        long.split_whitespace().collect::<Vec<_>>()
    );
    println!("native Pango fitting preserves complete long cues and intact hyphenated words");
}

fn palette_surfaces(preparation: &mut PresentationPreparation<'_>, repository: &Path) {
    use prepared_presentation::TextRole;
    use qt_window::{Color, Rect};
    use roonscape_renderer::{NowPlayingLayout, PresentationPalette};
    let palette = PresentationPalette::fallback();
    for viewport in [Viewport::new(1280, 720), Viewport::new(3840, 2160)] {
        let playing = fixture("playing");
        let mut prepared = preparation
            .prepare(&playing, viewport, repository, true)
            .unwrap();
        prepared.palette = palette;
        let PreparedContent::NowPlaying(content) = &prepared.content else {
            panic!("Now Playing")
        };
        let metadata = content
            .metadata
            .ordinary
            .iter()
            .map(|text| {
                let texture = text
                    .text
                    .sprite(0.0, 0.0, Color::default(), Rect::viewport(viewport))
                    .texture
                    .unwrap()
                    .identity();
                (
                    texture,
                    match text.role {
                        TextRole::Primary => palette.primary_text,
                        TextRole::Secondary => palette.secondary_text,
                        TextRole::Muted => palette.muted_text,
                    },
                )
            })
            .collect::<Vec<_>>();
        let mut view = NativeView::new(prepared.clone(), 1);
        let scene = view.render(ms(0), false);
        for (identity, expected) in metadata {
            let sprite = scene
                .sprites
                .iter()
                .find(|sprite| {
                    sprite
                        .texture
                        .as_ref()
                        .is_some_and(|texture| texture.identity() == identity)
                })
                .unwrap();
            assert_color(sprite.geometry.color, expected, 1.0);
        }
        let progress = scene
            .sprites
            .iter()
            .find(|sprite| sprite.geometry.kind == SpriteKind::Progress)
            .unwrap();
        assert_color(progress.geometry.color, palette.progress_fill, 1.0);
        assert_color(progress.geometry.secondary, palette.progress_track, 1.0);
        let layout = NowPlayingLayout::for_viewport(viewport);
        let graphic = &scene.graphics[0].geometry;
        assert_color(graphic.plate, palette.accent, 1.0);
        assert_color(graphic.border, palette.primary_text, 0.16);
        close(graphic.border_width, layout.artwork_border_width_px as f32);
        close(graphic.shadow_radius, layout.artwork_shadow_blur_px as f32);
        close(graphic.shadow_y, layout.artwork_shadow_offset_px as f32);
        close(graphic.shadow_alpha, 0.38);
        let mut overlay = Scene::default();
        scene::diagnostics(
            &mut overlay,
            &content.metadata.ordinary[0].text,
            palette,
            viewport,
        );
        assert_color(
            overlay.sprites[0].geometry.color,
            palette.diagnostics_border,
            1.0,
        );
        assert_color(
            overlay.sprites[1].geometry.color,
            palette.diagnostics_field,
            1.0,
        );
        assert_color(
            overlay.sprites[2].geometry.color,
            palette.diagnostics_text,
            1.0,
        );
    }
    println!("native scene preserves semantic palette roles and artwork decoration geometry");
}

fn assert_color(actual: qt_window::Color, expected: roonscape_renderer::Rgb, alpha: f32) {
    close(actual.red, f32::from(expected.red) / 255.0);
    close(actual.green, f32::from(expected.green) / 255.0);
    close(actual.blue, f32::from(expected.blue) / 255.0);
    close(actual.alpha, alpha);
}

fn metadata_wrapping(preparation: &mut PresentationPreparation<'_>, repository: &Path) {
    for (viewport, counts) in [
        (Viewport::new(1280, 720), [4, 2, 1]),
        (Viewport::new(1600, 900), [5, 3, 2]),
    ] {
        let prepared = preparation
            .prepare(&fixture("extreme-metadata"), viewport, repository, true)
            .unwrap();
        let PreparedContent::NowPlaying(content) = prepared.content else {
            panic!("Now Playing")
        };
        assert_eq!(
            content
                .metadata
                .ordinary
                .iter()
                .map(|line| line.text.lines.len())
                .collect::<Vec<_>>(),
            counts,
            "metadata fitting preserves title, artist and album allocations at {viewport:?}"
        );
    }
}

fn artwork_palettes(preparation: &mut PresentationPreparation<'_>, repository: &Path) {
    for name in ["playing", "light-artwork"] {
        let presentation = fixture(name);
        let Presentation::NowPlaying(value) = &presentation else {
            panic!("Now Playing")
        };
        let expected = roonscape_renderer::PresentationPalette::from_artwork(
            &repository.join(value.artwork_path.as_ref().unwrap()),
        )
        .unwrap();
        let actual = preparation
            .prepare(&presentation, Viewport::new(1280, 720), repository, true)
            .unwrap()
            .palette;
        for (actual, expected) in [
            (actual.background, expected.background),
            (actual.artwork_field, expected.artwork_field),
            (actual.metadata_field, expected.metadata_field),
            (actual.primary_text, expected.primary_text),
            (actual.secondary_text, expected.secondary_text),
            (actual.accent, expected.accent),
        ] {
            // Native JPEG decoders can differ by a rounding step. The selected
            // color families and their presentation roles must remain intact.
            assert!(
                actual.red.abs_diff(expected.red) <= 2
                    && actual.green.abs_diff(expected.green) <= 2
                    && actual.blue.abs_diff(expected.blue) <= 2,
                "{name}: decoded artwork changed palette role from {expected:?} to {actual:?}"
            );
        }
    }
}

fn disabling_animation_settles_active_graphics(
    preparation: &mut PresentationPreparation<'_>,
    repository: &Path,
) {
    let viewport = Viewport::new(1280, 720);
    let playing = fixture("playing");
    let light = fixture("light-artwork");
    let first = preparation
        .prepare(&playing, viewport, repository, true)
        .unwrap();
    let next = preparation
        .prepare(&light, viewport, repository, true)
        .unwrap();
    let destination = next.artwork.as_ref().unwrap().identity();
    let mut view = NativeView::new(first, 1);
    view.install(next.clone(), ms(0), true);
    view.update(&light, 2);
    assert!(view.render(ms(100), true).graphics.len() > 1);
    let settled = view.render(ms(101), false);
    assert_eq!(settled.graphics.len(), 1);
    assert_eq!(
        settled.graphics[0].artwork.as_ref().unwrap().identity(),
        destination
    );
    close(settled.graphics[0].geometry.weight, 1.0);
    view.replace(next, &light, 3, ms(200), true);
    close(progress_alpha(&view.render(ms(200), true)), 0.0);
    close(progress_alpha(&view.render(ms(201), false)), 1.0);
    view.begin_departure(ms(300), true);
    close(progress_alpha(&view.render(ms(301), false)), 0.0);
}

fn arriving_artwork_is_prepared_once_and_keyed_by_revision(
    preparation: &mut PresentationPreparation<'_>,
    repository: &Path,
) {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("arriving.jpg");
    std::fs::copy(
        repository.join("src/shared/fixtures/artwork/playing.jpg"),
        &path,
    )
    .unwrap();
    let artwork = roonscape_renderer::ArtworkReference {
        path: path.to_string_lossy().into_owned(),
        revision: 1,
    };
    preparation.prepare_arriving_artwork(artwork.clone(), repository);
    std::fs::remove_file(&path).unwrap();
    let mut incoming = fixture("playing");
    if let Presentation::NowPlaying(value) = &mut incoming {
        value.artwork_path = Some(artwork.path);
        value.artwork_revision = Some(1);
    }
    let prepared = preparation
        .prepare(&incoming, Viewport::new(1280, 720), repository, true)
        .unwrap();
    assert!(
        prepared.artwork.is_some(),
        "main-loop delivery must reuse prepared pixels"
    );
    if let Presentation::NowPlaying(value) = &mut incoming {
        value.artwork_revision = Some(2);
    }
    assert!(
        preparation
            .prepare(&incoming, Viewport::new(1280, 720), repository, true)
            .is_err(),
        "a new artwork revision must not reuse the previous image"
    );
}

fn repeated_replacements_evict_old_prepared_artwork(
    preparation: &mut PresentationPreparation<'_>,
    repository: &Path,
) {
    let temporary = tempfile::tempdir().unwrap();
    let mut first = None;
    for revision in 1..=12 {
        let path = temporary.path().join(format!("replacement-{revision}.jpg"));
        std::fs::copy(
            repository.join("src/shared/fixtures/artwork/playing.jpg"),
            &path,
        )
        .unwrap();
        let mut incoming = fixture("playing");
        if let Presentation::NowPlaying(value) = &mut incoming {
            value.artwork_path = Some(path.to_string_lossy().into_owned());
            value.artwork_revision = Some(revision);
        }
        let prepared = preparation
            .prepare(&incoming, Viewport::new(1280, 720), repository, true)
            .unwrap();
        std::fs::remove_file(path).unwrap();
        let reused = preparation
            .prepare(&incoming, Viewport::new(1280, 720), repository, true)
            .unwrap();
        assert_eq!(
            prepared.artwork.as_ref().unwrap().identity(),
            reused.artwork.as_ref().unwrap().identity()
        );
        first.get_or_insert(incoming);
    }
    assert!(
        preparation
            .prepare(&first.unwrap(), Viewport::new(1280, 720), repository, true)
            .is_err(),
        "repeated replacements must evict old artwork instead of retaining every image"
    );
}

fn decoded_artwork(repository: &Path) {
    let first =
        qt_window::DecodedImage::open(&repository.join("src/shared/fixtures/artwork/playing.jpg"))
            .unwrap();
    let original = first.pixels().to_vec();
    let second =
        qt_window::DecodedImage::open(&repository.join("src/shared/fixtures/artwork/light.jpg"))
            .unwrap();
    assert!(!first.has_alpha);
    assert_eq!(first.size, Viewport::new(1600, 1600));
    assert_eq!(
        first.pixels(),
        original,
        "a later decode cannot mutate retained pixels"
    );
    assert_ne!(first.pixels(), second.pixels());
    assert_eq!(first.stride, 1600 * 3);
    assert_eq!(first.channels, 3);
}

fn gradient_reuse_survives_content_changes(
    preparation: &mut PresentationPreparation<'_>,
    repository: &Path,
) {
    let viewport = Viewport::new(1280, 720);
    let playing = fixture("playing");
    let first = preparation
        .prepare(&playing, viewport, repository, true)
        .unwrap();
    let original = first.gradient.as_ref().unwrap().identity();
    let mut light = fixture("light-artwork");
    for index in 0..5 {
        if let Presentation::NowPlaying(value) = &mut light {
            value.title = Some(format!("Updated title {index}"));
        }
        let next = preparation
            .prepare(&light, viewport, repository, true)
            .unwrap();
        assert_ne!(next.gradient.as_ref().unwrap().identity(), original);
    }
    let returned = preparation
        .prepare(&playing, viewport, repository, true)
        .unwrap();
    assert_eq!(
        returned.gradient.as_ref().unwrap().identity(),
        original,
        "content changes must not evict an otherwise reusable palette gradient"
    );
    let resized = preparation
        .prepare(&playing, Viewport::new(1600, 900), repository, true)
        .unwrap();
    assert_ne!(
        resized.gradient_steps, first.gradient_steps,
        "gradient sampling must follow the current physical dimensions"
    );
}

fn main() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    register_packaged_fallback_fonts(&repository.join("src/renderer")).unwrap();
    let families = pangocairo::FontMap::new()
        .list_families()
        .into_iter()
        .map(|family| family.name().to_string())
        .collect::<HashSet<_>>();
    let typography = select_capture_typography(&families, NowPlayingTitleFace::Fallback).unwrap();
    let window = Window::new(Viewport::new(1280, 720), false).unwrap();
    let mut preparation = PresentationPreparation::new(window.uploader(), typography, 1.0).unwrap();
    decoded_artwork(repository);
    repeated_replacements_evict_old_prepared_artwork(&mut preparation, repository);
    arriving_artwork_is_prepared_once_and_keyed_by_revision(&mut preparation, repository);
    artwork_palettes(&mut preparation, repository);
    gradient_reuse_survives_content_changes(&mut preparation, repository);
    metadata_wrapping(&mut preparation, repository);
    transitions(&mut preparation, repository);
    disabling_animation_settles_active_graphics(&mut preparation, repository);
    lyrics(&mut preparation, repository);
    word_motion_meets_native_endpoints(&mut preparation, repository);
    composition_keeps_shared_metadata_visible(&mut preparation, repository);
    composition_preserves_clipped_title_at_motion_boundary(&mut preparation, repository);
    partially_retained_words_follow_native_ellipsis(&mut preparation, repository);
    wrapping_words_do_not_collide_during_composition(&mut preparation, repository);
    cue_preparation_reuses_metadata_words(&mut preparation, repository);
    composition_reel_translation_and_clipping(&mut preparation, repository);
    compatible_content(&mut preparation, repository);
    palette_surfaces(&mut preparation, repository);
    fitted_text(&text_preparation::TextPreparation::new(
        typography,
        window.uploader(),
        1.0,
    ));
    // Platform-independent status symbols remain available before an in-place
    // replacement starts, including the less common Full-field statuses.
    let prepared = preparation
        .prepare(
            &fixture("disconnected"),
            Viewport::new(1280, 720),
            repository,
            true,
        )
        .unwrap();
    let PreparedContent::FullField(content) = prepared.content else {
        panic!("Full-field");
    };
    assert!(
        content
            .statuses
            .iter()
            .any(|(symbol, _)| *symbol == PresentationStatusSymbol::Starting)
    );
}
