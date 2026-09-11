use std::time::Duration;

use roonscape_renderer::{
    FullFieldLayout, NowPlayingLayout, NowPlayingPresentation, Presentation, PresentationPalette,
    PresentationStatus, PresentationStatusEmphasis,
};

use crate::lyric_motion::LyricFrame;
use crate::prepared_presentation::{
    MetadataMovement, PreparedContent, PreparedIdentity, PreparedNowPlaying, PreparedPresentation,
    PreparedReel, PreparedStatus, TextAt,
};
use crate::qt_window::{Color, Rect, Scene, Sprite, SpriteGeometry, SpriteKind, Texture};

pub(crate) struct Foreground<'a> {
    pub presentation: &'a Presentation,
    pub palette: PresentationPalette,
    pub now: Duration,
    pub animated: bool,
    pub opacity: f32,
    pub metadata_opacity: f32,
    pub status: PresentationStatus,
    pub status_opacity: f32,
    pub timing_opacity: f32,
    pub lyrics: Option<&'a LyricFrame>,
}

pub(crate) fn composition_geometry(progress: f64) -> f64 {
    motion_phase(progress, 0.12, 0.72)
}

pub(crate) fn motion_phase(value: f64, start: f64, duration: f64) -> f64 {
    let value = ((value - start) / duration).clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
}

pub(crate) fn foreground<'window>(
    scene: &mut Scene<'window>,
    prepared: &PreparedPresentation<'window>,
    frame: &Foreground<'_>,
) {
    let clip = Rect::viewport(prepared.viewport);
    match (&prepared.content, frame.presentation) {
        (PreparedContent::NowPlaying(content), Presentation::NowPlaying(presentation)) => {
            let progress = frame
                .lyrics
                .map_or(f64::from(presentation.lyrics.is_some()), |lyrics| {
                    lyrics.composition_progress
                });
            let layout = NowPlayingLayout::for_composition_progress(
                presentation,
                prepared.viewport,
                composition_geometry(progress),
            );
            now_playing(scene, content, presentation, &layout, progress, frame, clip);
        }
        (PreparedContent::FullField(content), Presentation::FullField(presentation)) => {
            let layout = FullFieldLayout::for_viewport(prepared.viewport);
            for text in &content.text {
                text_at(scene, text, 0.0, 0.0, frame.palette, frame.opacity, clip);
            }
            let top = layout.presentation_status_slot.top_viewport_y_px;
            solid(
                scene,
                Rect::new(
                    layout.composition_left_viewport_x_px as f32,
                    top as f32,
                    layout.accent_width_px as f32,
                    (layout.accent_bottom_viewport_y_px(presentation.explanation.is_some()) - top)
                        as f32,
                ),
                Color::new(frame.palette.accent, frame.opacity),
                0.0,
                clip,
            );
            if let Some((_, prepared_status)) = content
                .statuses
                .iter()
                .find(|(symbol, _)| *symbol == frame.status.symbol)
            {
                status(
                    scene,
                    prepared_status,
                    layout.text_left_viewport_x_px as f32,
                    top as f32,
                    frame,
                    frame.opacity * frame.status_opacity,
                    clip,
                );
            }
            if let Some(identity) = &content.identity {
                let width = identity
                    .text
                    .iter()
                    .map(|text| text.x + text.text.width)
                    .fold(0.0f32, f32::max);
                let x = (prepared.viewport.width_px
                    - layout.outer_gutter_px
                    - layout.identity_right_inset_px) as f32
                    - width;
                let y = layout.identity_anchor.bottom_viewport_y_px as f32 - identity.height;
                draw_identity(scene, identity, x, y, frame.palette, frame.opacity, clip);
            }
        }
        _ => {}
    }
}

fn now_playing<'window>(
    scene: &mut Scene<'window>,
    content: &PreparedNowPlaying<'window>,
    presentation: &NowPlayingPresentation,
    layout: &NowPlayingLayout,
    progress: f64,
    frame: &Foreground<'_>,
    clip: Rect,
) {
    let rail = layout.information.left_viewport_x_px as f32;
    if let Some((_, prepared_status)) = content
        .statuses
        .iter()
        .find(|(symbol, _)| *symbol == frame.status.symbol)
    {
        status(
            scene,
            prepared_status,
            rail,
            layout
                .artwork_field_anchors
                .presentation_status_top_viewport_y_px as f32,
            frame,
            frame.opacity * frame.status_opacity,
            clip,
        );
    }
    let metadata_bottom = draw_metadata(scene, content, layout, progress, frame, clip);
    if let (Some(reel), Some(lyrics)) = (&content.reel, frame.lyrics) {
        let amount = composition_geometry(progress) as f32;
        let travel =
            (1.0 - amount) * (reel.bottom - reel.top + 2.0 * layout.typography.lyric_cue_px as f32);
        let settled_metadata_bottom = content
            .metadata
            .masthead
            .iter()
            .map(|text| {
                layout.metadata_region_top_viewport_y_px as f32
                    + text.y
                    + text.text.bounds.y
                    + text.text.bounds.height
            })
            .fold(0.0, f32::max);
        let clearance = (reel.top - settled_metadata_bottom).max(0.0);
        let top = (reel.top + travel).max(metadata_bottom + clearance);
        draw_reel(
            scene,
            reel,
            lyrics,
            Rect::new(rail, top, reel.width, (reel.bottom - top).max(0.0)),
            travel,
            frame.palette,
            frame.opacity,
        );
    }
    let amount = composition_geometry(progress) as f32;
    let identity = if amount >= 0.5 {
        &content.lyric_identity
    } else {
        &content.ordinary_identity
    };
    let identity_y = layout.footer_anchor.bottom_viewport_y_px as f32 - identity.height;
    let mix = |a: f32, b: f32| a + (b - a) * amount;
    for (index, text) in identity.text.iter().enumerate() {
        let mut text = text.clone();
        if let (Some(ordinary), Some(lyric)) = (
            content.ordinary_identity.text.get(index),
            content.lyric_identity.text.get(index),
        ) {
            text.x = mix(ordinary.x, lyric.x);
            text.y = mix(ordinary.y, lyric.y);
        }
        text_at(
            scene,
            &text,
            rail,
            identity_y,
            frame.palette,
            frame.opacity,
            clip,
        );
    }
    if let (Some(a), Some(b)) = (
        content.ordinary_identity.separator,
        content.lyric_identity.separator,
    ) {
        solid(
            scene,
            Rect::new(
                rail + mix(a.x, b.x),
                identity_y + mix(a.y, b.y),
                mix(a.width, b.width),
                mix(a.height, b.height),
            ),
            Color::new(frame.palette.muted_text, frame.opacity),
            a.width / 2.0,
            clip,
        );
    }
    let timing_height = layout.timing_height_px() as f32;
    let timing_y = identity_y - layout.footer_gap_px as f32 - timing_height;
    let opacity = frame.opacity * frame.timing_opacity;
    if let Some(progress) = &presentation.progress {
        let height =
            (layout.progress_fill_height_px + layout.time_spacing_px) as f32 + content.time_height;
        let top = timing_y + (timing_height - height) / 2.0;
        scene.sprites.push(Sprite {
            geometry: SpriteGeometry {
                bounds: Rect::new(
                    rail,
                    top,
                    layout.information.utility_width_px as f32,
                    layout.progress_fill_height_px as f32,
                ),
                uv: Rect::new(
                    progress.fraction.clamp(0.0, 1.0) as f32,
                    layout.progress_track_height_px as f32,
                    0.0,
                    0.0,
                ),
                clip,
                color: Color::new(frame.palette.progress_fill, opacity),
                secondary: Color::new(frame.palette.progress_track, 1.0),
                kind: SpriteKind::Progress,
                ..SpriteGeometry::default()
            },
            ..Sprite::default()
        });
        let y = top + (layout.progress_fill_height_px + layout.time_spacing_px) as f32;
        digits(
            scene,
            content,
            &progress.elapsed,
            [rail, y],
            frame.palette,
            opacity,
            clip,
        );
        let width = progress
            .remaining
            .chars()
            .filter_map(|ch| content.digits.get(&ch))
            .map(|glyph| glyph.width)
            .sum::<f32>();
        digits(
            scene,
            content,
            &progress.remaining,
            [rail + layout.information.utility_width_px as f32 - width, y],
            frame.palette,
            opacity,
            clip,
        );
    } else if let (Some(activity), Some((heading, detail))) =
        (&presentation.activity, &content.activity)
    {
        let height =
            (heading.height + detail.height).max(layout.activity_waveform_height_px as f32);
        let top = timing_y + (timing_height - height) / 2.0;
        let wave_top = top + (height - layout.activity_waveform_height_px as f32) / 2.0;
        let scales = activity.waveform.bar_scales_at(frame.now, frame.animated);
        let width = layout.activity_waveform_width_px as f32;
        let cell = width / 13.0;
        for (index, (reference, scale)) in activity
            .waveform
            .reference_heights_percent
            .iter()
            .zip(scales)
            .enumerate()
        {
            let bar_height = layout.activity_waveform_height_px as f32 * f32::from(*reference)
                / 100.0
                * scale as f32;
            solid(
                scene,
                Rect::new(
                    rail + index as f32 * cell * 2.0,
                    wave_top + (layout.activity_waveform_height_px as f32 - bar_height) / 2.0,
                    cell,
                    bar_height,
                ),
                Color::new(frame.palette.accent, opacity),
                cell.min(bar_height) / 2.0,
                clip,
            );
        }
        let x = rail + (layout.activity_waveform_width_px + layout.activity_copy_gap_px) as f32;
        let y = top + (height - heading.height - detail.height) / 2.0;
        scene.sprites.push(heading.sprite(
            x,
            y,
            Color::new(frame.palette.primary_text, opacity),
            clip,
        ));
        scene.sprites.push(detail.sprite(
            x,
            y + heading.height,
            Color::new(frame.palette.muted_text, opacity),
            clip,
        ));
    }
}

fn draw_metadata<'window>(
    scene: &mut Scene<'window>,
    content: &PreparedNowPlaying<'window>,
    layout: &NowPlayingLayout,
    progress: f64,
    frame: &Foreground<'_>,
    clip: Rect,
) -> f32 {
    let metadata = &content.metadata;
    let rail = layout.information.left_viewport_x_px as f32;
    let amount = composition_geometry(progress) as f32;
    let opacity = frame.opacity * frame.metadata_opacity;
    let start = scene.sprites.len();
    if amount == 0.0 {
        let metadata_clip = Rect::new(
            rail,
            layout.metadata_region_top_viewport_y_px as f32,
            layout.information.musical_metadata_width_px as f32,
            (layout.metadata_region_bottom_viewport_y_px - layout.metadata_region_top_viewport_y_px)
                as f32,
        );
        for text in &metadata.ordinary {
            text_at(
                scene,
                text,
                rail,
                0.0,
                frame.palette,
                opacity,
                metadata_clip,
            );
        }
    } else if amount == 1.0 {
        for text in &metadata.masthead {
            text_at(
                scene,
                text,
                rail,
                layout.metadata_region_top_viewport_y_px as f32,
                frame.palette,
                opacity,
                clip,
            );
        }
    } else {
        // Ordinary metadata can clip oversized native tokens. Preserve that
        // boundary when movement starts, then release it toward the masthead's
        // viewport clip without exposing hidden glyphs in a single frame.
        let mix = |a: f32, b: f32| a + (b - a) * amount;
        let moving_clip = Rect::new(
            mix(rail, clip.x),
            mix(layout.metadata_region_top_viewport_y_px as f32, clip.y),
            mix(
                layout.information.musical_metadata_width_px as f32,
                clip.width,
            ),
            mix(
                (layout.metadata_region_bottom_viewport_y_px
                    - layout.metadata_region_top_viewport_y_px) as f32,
                clip.height,
            ),
        );
        for movement in &metadata.movement {
            moving_words(
                scene,
                movement,
                layout,
                amount,
                frame.palette,
                opacity,
                moving_clip,
            );
        }
        if let Some(album) = metadata
            .album_index
            .and_then(|index| metadata.ordinary.get(index))
        {
            text_at(
                scene,
                album,
                rail,
                metadata.album_displacement * amount,
                frame.palette,
                opacity * (1.0 - motion_phase(progress, 0.12, 0.35)) as f32,
                moving_clip,
            );
        }
    }
    scene.sprites[start..]
        .iter()
        .filter(|sprite| sprite.geometry.color.alpha > 0.0)
        .map(|sprite| sprite.geometry.bounds.y + sprite.geometry.bounds.height)
        .fold(0.0, f32::max)
}

fn moving_words<'window>(
    scene: &mut Scene<'window>,
    movement: &MetadataMovement<'window>,
    layout: &NowPlayingLayout,
    amount: f32,
    palette: PresentationPalette,
    opacity: f32,
    clip: Rect,
) {
    let rail = layout.information.left_viewport_x_px as f32;
    let width = layout.information.musical_metadata_width_px as f32;
    let mix = |a: f32, b: f32| a + (b - a) * amount;
    let horizontal = 1.0 - (1.0 - amount).powi(3);
    let visibility = motion_phase(horizontal as f64, 0.35, 0.45) as f32;
    let [source_extent, destination_extent] = movement.normalized_extents;
    let normalized_extent = source_extent + (destination_extent - source_extent) * horizontal;
    let size = (movement.ordinary.size
        + (movement.compact.size - movement.ordinary.size) * horizontal)
        .min((width + 2.0) / normalized_extent.max(1.0));
    let rgb = movement.role.color(palette);
    for word in &movement.paths {
        let alpha = opacity
            * match word.visible {
                [true, true] => 1.0,
                [true, false] => 1.0 - visibility,
                [false, true] => visibility,
                [false, false] => 0.0,
            };
        if alpha <= 0.0 {
            continue;
        }
        // Horizontal line changes and shrinking lead vertical convergence so
        // neighboring native lines do not collide on their way to the masthead.
        let mut x =
            rail + (word.source[0] + (word.destination[0] - word.source[0]) * horizontal) * size;
        let mut y = mix(word.source[1], word.destination[1]);
        let disappearing = match word.visible {
            [true, false] => horizontal,
            [false, true] => 1.0 - horizontal,
            _ => 0.0,
        };
        let spread =
            disappearing * (1.0 - motion_phase(disappearing as f64, 0.8, 0.2) as f32) * size;
        x += word.omitted_spread[0] * spread;
        y += word.omitted_spread[1] * spread;
        let mut sprite = word.text.sprite(0.0, 0.0, Color::new(rgb, alpha), clip);
        sprite.geometry.bounds = Rect::new(
            x + word.bounds.x * size,
            y + word.bounds.y * size,
            word.bounds.width * size,
            word.bounds.height * size,
        );
        if let Some([a, b]) = word.clipping {
            let clipping = motion_phase(horizontal as f64, 0.35, 0.55) as f32;
            let interpolate = |a: f32, b: f32| a + (b - a) * clipping;
            let left = x + interpolate(a.x, b.x) * size - 1.0;
            let right = x + interpolate(a.x + a.width, b.x + b.width) * size + 1.0;
            sprite.geometry.clip.x = clip.x.max(left);
            sprite.geometry.clip.width =
                ((clip.x + clip.width).min(right) - sprite.geometry.clip.x).max(0.0);
        }
        scene.sprites.push(sprite);
    }
}

fn status<'window>(
    scene: &mut Scene<'window>,
    prepared: &PreparedStatus<'window>,
    x: f32,
    y: f32,
    frame: &Foreground<'_>,
    opacity: f32,
    clip: Rect,
) {
    let status = frame.status;
    let rgb = match status.emphasis {
        PresentationStatusEmphasis::FullAccent => frame.palette.accent,
        PresentationStatusEmphasis::MutedAccent => frame.palette.status_muted_accent,
    };
    let size = prepared.layout.symbol_size_px as f32;
    let bounds = Rect::new(x, y, size, size);
    if let Some(circle) = &prepared.circle {
        mask(scene, circle, bounds, Color::new(rgb, opacity), 0.0, clip);
    }
    let glyph_opacity = if prepared.circle.is_some() {
        opacity * 0.92 / (1.0 - opacity * 0.08)
    } else {
        opacity
    };
    mask(
        scene,
        &prepared.glyph,
        bounds,
        Color::new(rgb, glyph_opacity),
        status.motion.rotation_at(frame.now, frame.animated) as f32,
        clip,
    );
    scene.sprites.push(prepared.label.sprite(
        x + size + prepared.layout.symbol_gap_px as f32,
        y + (size - prepared.label.height) / 2.0,
        Color::new(rgb, opacity),
        clip,
    ));
}

fn draw_identity<'window>(
    scene: &mut Scene<'window>,
    identity: &PreparedIdentity<'window>,
    x: f32,
    y: f32,
    palette: PresentationPalette,
    opacity: f32,
    clip: Rect,
) {
    for text in &identity.text {
        text_at(scene, text, x, y, palette, opacity, clip);
    }
    if let Some(rect) = identity.separator {
        solid(
            scene,
            Rect::new(x + rect.x, y + rect.y, rect.width, rect.height),
            Color::new(palette.muted_text, opacity),
            rect.width / 2.0,
            clip,
        );
    }
}

fn digits<'window>(
    scene: &mut Scene<'window>,
    content: &PreparedNowPlaying<'window>,
    text: &str,
    position: [f32; 2],
    palette: PresentationPalette,
    opacity: f32,
    clip: Rect,
) {
    let [mut x, y] = position;
    for glyph in text.chars().filter_map(|ch| content.digits.get(&ch)) {
        scene
            .sprites
            .push(glyph.sprite(x, y, Color::new(palette.secondary_text, opacity), clip));
        x += glyph.width;
    }
}

fn draw_reel<'window>(
    scene: &mut Scene<'window>,
    reel: &PreparedReel<'window>,
    frame: &LyricFrame,
    clip: Rect,
    travel: f32,
    palette: PresentationPalette,
    opacity: f32,
) {
    if opacity <= 0.0 {
        return;
    }
    let mut cues = Vec::new();
    for cue in &frame.cues {
        let prepared = if cue.text.trim().is_empty() {
            Some(&reel.blank)
        } else {
            usize::try_from(cue.index)
                .ok()
                .and_then(|index| reel.cues.get(&index))
        };
        if let Some(prepared) = prepared {
            cues.push((cue, prepared));
        }
    }
    let mut y = 0.0f64;
    let tops: Vec<_> = cues
        .iter()
        .map(|(cue, prepared)| {
            let top = y;
            y += (prepared.height + reel.gap) as f64 * cue.extent;
            top
        })
        .collect();
    let anchor = frame
        .anchors
        .iter()
        .map(|(index, weight)| {
            let top = cues
                .iter()
                .position(|(cue, _)| cue.index == *index)
                .map_or(0.0, |index| tops[index]);
            top * weight
        })
        .sum::<f64>();
    let x = clip.x;
    if clip.height <= 0.0 {
        return;
    }
    for ((cue, prepared), top) in cues.into_iter().zip(tops) {
        let y = reel.top + travel + reel.primary_y + (top - anchor) as f32;
        if cue.opacity <= 0.0
            || cue.text.trim().is_empty()
            || y >= reel.bottom
            || y + prepared.height < clip.y
        {
            continue;
        }
        let focal = cue.color_weights[1];
        let mix =
            |a: u8, b: u8| (f64::from(a) * (1.0 - focal) + f64::from(b) * focal).round() as u8;
        let rgb = roonscape_renderer::Rgb {
            red: mix(palette.secondary_text.red, palette.primary_text.red),
            green: mix(palette.secondary_text.green, palette.primary_text.green),
            blue: mix(palette.secondary_text.blue, palette.primary_text.blue),
        };
        let mut sprite = prepared.sprite(x, y, Color::new(rgb, opacity * cue.opacity as f32), clip);
        sprite.geometry.fade_top = reel.fade;
        sprite.geometry.fade_top_origin = reel.top + travel;
        sprite.geometry.fade_bottom = reel.fade;
        scene.sprites.push(sprite);
    }
}

fn text_at<'window>(
    scene: &mut Scene<'window>,
    text: &TextAt<'window>,
    x: f32,
    y: f32,
    palette: PresentationPalette,
    opacity: f32,
    clip: Rect,
) {
    let rgb = text.role.color(palette);
    scene.sprites.push(
        text.text
            .sprite(x + text.x, y + text.y, Color::new(rgb, opacity), clip),
    );
}

fn solid<'window>(scene: &mut Scene<'window>, bounds: Rect, color: Color, radius: f32, clip: Rect) {
    scene.sprites.push(Sprite {
        geometry: SpriteGeometry {
            bounds,
            clip,
            color,
            radius,
            ..SpriteGeometry::default()
        },
        ..Sprite::default()
    });
}

fn mask<'window>(
    scene: &mut Scene<'window>,
    texture: &Texture<'window>,
    bounds: Rect,
    color: Color,
    angle: f32,
    clip: Rect,
) {
    scene.sprites.push(Sprite {
        texture: Some(texture.clone()),
        geometry: SpriteGeometry {
            bounds,
            clip,
            color,
            angle,
            uv: Rect::new(0.0, 0.0, 1.0, 1.0),
            kind: SpriteKind::AlphaMask,
            ..SpriteGeometry::default()
        },
        ..Sprite::default()
    });
}

pub(crate) fn diagnostics<'window>(
    scene: &mut Scene<'window>,
    text: &crate::text_preparation::PreparedText<'window>,
    palette: PresentationPalette,
    viewport: roonscape_renderer::Viewport,
) {
    let clip = Rect::viewport(viewport);
    let x = viewport.width_px as f32 - 24.0 - text.width - 38.0;
    let y = 24.0;
    solid(
        scene,
        Rect::new(x, y, text.width + 38.0, text.height + 30.0),
        Color::new(palette.diagnostics_border, 1.0),
        4.0,
        clip,
    );
    solid(
        scene,
        Rect::new(x + 1.0, y + 1.0, text.width + 36.0, text.height + 28.0),
        Color::new(palette.diagnostics_field, 1.0),
        3.0,
        clip,
    );
    scene.sprites.push(text.sprite(
        x + 19.0,
        y + 15.0,
        Color::new(palette.diagnostics_text, 1.0),
        clip,
    ));
}
