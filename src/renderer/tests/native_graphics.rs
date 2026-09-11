#[allow(dead_code)]
#[path = "../src/content_evidence.rs"]
mod content_evidence;
// Qt must run on the process's main thread. This harness exercises the same
// native window and scene interface as the Renderer under the native session.
#[allow(dead_code)]
#[path = "../src/qt_window.rs"]
mod qt_window;

use qt_window::{
    Color, Graphic, GraphicGeometry, Rect, Scene, Sprite, SpriteGeometry, SpriteKind, Window,
    WindowEvents,
};
use roonscape_renderer::{
    NowPlayingGradient, NowPlayingGradientLookup, PresentationPalette, Viewport,
};

enum ExpectedPixels {
    Exact(Vec<u8>),
    SaveReference,
    SavedReference,
}

impl From<Vec<u8>> for ExpectedPixels {
    fn from(pixels: Vec<u8>) -> Self {
        Self::Exact(pixels)
    }
}

struct GradientCase<'a> {
    scene: Scene<'a>,
    expected: ExpectedPixels,
    viewport: Viewport,
}

struct GradientCheck<'a> {
    cases: std::collections::VecDeque<GradientCase<'a>>,
    submitted: bool,
    passed: usize,
    reference: Option<Vec<u8>>,
}

impl WindowEvents for GradientCheck<'_> {
    fn render(&mut self, frame: &qt_window::RenderFrame) {
        if self.submitted && !self.passed.is_multiple_of(2) {
            frame.submit(&self.cases.front().unwrap().scene);
        }
    }
    fn update(&mut self, window: &Window, _: i64, viewport: Viewport, _: u32) {
        let Some(case) = self.cases.front() else {
            return;
        };
        if viewport != case.viewport {
            window.resize(case.viewport);
            window.wake();
            return;
        }
        if !self.submitted {
            window.request_capture();
            if self.passed.is_multiple_of(2) {
                window.submit(&case.scene);
            }
            self.submitted = true;
        }
    }
    fn input(&mut self, _: &Window, event: i32) {
        if event == 99 {
            panic!("Native graphics check exceeded its display deadline");
        }
    }
    fn painted(&mut self, window: &Window, _: qt_window::PaintedFrame) {
        if !self.submitted {
            return;
        }
        let case = self.cases.front().unwrap();
        let Ok(actual) = window.capture(case.viewport) else {
            return;
        };
        let expected = match &case.expected {
            ExpectedPixels::Exact(pixels) => Some(pixels),
            ExpectedPixels::SavedReference => {
                Some(self.reference.as_ref().expect("missing reference frame"))
            }
            ExpectedPixels::SaveReference => None,
        };
        if let Some(expected) = expected {
            assert_eq!(actual.len(), expected.len());
            if let Some(index) = actual.iter().zip(expected).position(|(a, b)| a != b) {
                panic!(
                    "GPU compositing mismatch at ({}, {}), channel {}: actual {}, expected {}",
                    index / 4 % case.viewport.width_px as usize,
                    index / 4 / case.viewport.width_px as usize,
                    index % 4,
                    actual[index],
                    expected[index]
                );
            }
            println!(
                "native graphics match every reference pixel at {}×{}",
                case.viewport.width_px, case.viewport.height_px
            );
        } else {
            self.reference = Some(actual);
            println!(
                "native graphics reference captured at {}×{}",
                case.viewport.width_px, case.viewport.height_px
            );
        }
        self.passed += 1;
        self.cases.pop_front();
        self.submitted = false;
        if self.cases.is_empty() {
            window.quit();
        } else {
            window.wake();
        }
    }
}

fn main() {
    let window = Window::new(Viewport::new(1280, 720), false).unwrap();
    let uploader = window.uploader();
    let noise = uploader.noise(NowPlayingGradientLookup::noise()).unwrap();
    let dark = PresentationPalette::fallback();
    let mut light = dark;
    light.background = roonscape_renderer::Rgb {
        red: 221,
        green: 232,
        blue: 208,
    };
    light.artwork_field = roonscape_renderer::Rgb {
        red: 240,
        green: 188,
        blue: 142,
    };
    light.metadata_field = roonscape_renderer::Rgb {
        red: 189,
        green: 203,
        blue: 242,
    };
    let mut extremes = dark;
    extremes.background = roonscape_renderer::Rgb {
        red: 0,
        green: 255,
        blue: 255,
    };
    extremes.artwork_field = roonscape_renderer::Rgb {
        red: 0,
        green: 0,
        blue: 0,
    };
    extremes.metadata_field = roonscape_renderer::Rgb {
        red: 255,
        green: 255,
        blue: 0,
    };
    let mut cases = std::collections::VecDeque::new();
    for viewport in [
        Viewport::new(1280, 720),
        Viewport::new(1291, 733),
        Viewport::new(3840, 2160),
    ] {
        for palette in [dark, light, extremes] {
            let lookup = NowPlayingGradientLookup::new(palette, viewport);
            let gradient = uploader.lookup(&lookup.colors).unwrap();
            let scene = Scene {
                graphics: vec![Graphic {
                    gradient: Some(gradient),
                    noise: Some(noise.clone()),
                    artwork: None,
                    geometry: GraphicGeometry {
                        background: Color::new(palette.background, 1.0),
                        origin: lookup.origin,
                        step_x: lookup.step_x,
                        step_y: lookup.step_y,
                        weight: 1.0,
                        ..GraphicGeometry::default()
                    },
                }],
                sprites: vec![],
            };
            cases.push_back(GradientCase {
                scene,
                expected: NowPlayingGradient::new(palette, viewport)
                    .into_rgba8()
                    .into(),
                viewport,
            });
        }
    }
    let viewport = Viewport::new(1280, 720);
    let rgb = |red, green, blue| roonscape_renderer::Rgb { red, green, blue };
    cases.push_back(GradientCase {
        scene: Scene {
            graphics: vec![Graphic {
                geometry: GraphicGeometry {
                    background: Color::new(rgb(80, 120, 160), 1.0),
                    weight: 0.4,
                    ..GraphicGeometry::default()
                },
                ..Graphic::default()
            }],
            sprites: vec![Sprite {
                geometry: SpriteGeometry {
                    bounds: Rect::viewport(viewport),
                    clip: Rect::viewport(viewport),
                    color: Color::new(rgb(200, 150, 100), 0.5),
                    dimming: 0.6,
                    ..SpriteGeometry::default()
                },
                ..Sprite::default()
            }],
        },
        expected: [56, 54, 52, 255]
            .repeat(viewport.width_px as usize * viewport.height_px as usize)
            .into(),
        viewport,
    });
    // Opaque source rows can require padding even though the GPU samples RGB.
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("odd-width.png");
    let source = vec![
        255, 0, 0, 0, 255, 0, 0, 0, 255, 91, 92, 93, 255, 255, 0, 0, 255, 255, 255, 0, 255, 94, 95,
        96,
    ];
    let pixbuf = gdk_pixbuf::Pixbuf::from_mut_slice(
        source.clone(),
        gdk_pixbuf::Colorspace::Rgb,
        false,
        8,
        3,
        2,
        12,
    );
    pixbuf.savev(&path, "png", &[]).unwrap();
    let decoded = qt_window::DecodedImage::open(&path)
        .unwrap()
        .palette_image()
        .unwrap();
    assert_eq!(decoded.size, Viewport::new(3, 2));
    assert!(!decoded.has_alpha);
    assert_eq!(decoded.stride, 12);
    assert_eq!(&decoded.pixels()[9..12], &[0, 0, 0]);
    let mut expected =
        [0, 0, 0, 255].repeat(viewport.width_px as usize * viewport.height_px as usize);
    for y in 0..2 {
        for x in 0..3 {
            let index = (y * viewport.width_px as usize + x) * 4;
            expected[index..index + 3].copy_from_slice(&source[y * 12 + x * 3..y * 12 + x * 3 + 3]);
        }
    }
    cases.push_back(GradientCase {
        scene: Scene {
            graphics: vec![Graphic {
                geometry: GraphicGeometry {
                    background: Color::new(rgb(0, 0, 0), 1.0),
                    weight: 1.0,
                    ..GraphicGeometry::default()
                },
                ..Graphic::default()
            }],
            sprites: vec![Sprite {
                texture: Some(uploader.image(&decoded).unwrap()),
                geometry: SpriteGeometry {
                    bounds: Rect::new(0.0, 0.0, 3.0, 2.0),
                    uv: Rect::new(0.0, 0.0, 1.0, 1.0),
                    clip: Rect::viewport(viewport),
                    color: Color::new(rgb(255, 255, 255), 1.0),
                    kind: SpriteKind::Image,
                    ..SpriteGeometry::default()
                },
                ..Sprite::default()
            }],
        },
        expected: expected.into(),
        viewport,
    });
    for (weights, expected_pixel) in [
        (vec![0.25, 0.75], [160, 80, 144, 255]),
        (vec![0.25, 0.25, 0.5], [64, 144, 112, 255]),
    ] {
        let colors = [rgb(64, 128, 192), rgb(192, 64, 128), rgb(0, 192, 64)];
        cases.push_back(GradientCase {
            scene: Scene {
                graphics: weights
                    .into_iter()
                    .zip(colors)
                    .map(|(weight, color)| Graphic {
                        geometry: GraphicGeometry {
                            background: Color::new(color, 1.0),
                            weight,
                            ..GraphicGeometry::default()
                        },
                        ..Graphic::default()
                    })
                    .collect(),
                sprites: vec![],
            },
            expected: expected_pixel
                .repeat((viewport.width_px * viewport.height_px) as usize)
                .into(),
            viewport,
        });
    }
    // Palette blends retain every channel before framebuffer conversion.
    // Repeated source textures also exercise reuse when layer order changes.
    let palettes = [dark, light, extremes];
    let gradients: Vec<_> = palettes
        .iter()
        .map(|palette| {
            let lookup = NowPlayingGradientLookup::new(*palette, viewport);
            (uploader.lookup(&lookup.colors).unwrap(), lookup)
        })
        .collect();
    let pixels: Vec<_> = palettes
        .iter()
        .map(|palette| NowPlayingGradient::new(*palette, viewport).into_rgba8())
        .collect();
    for (indices, weights) in [
        (vec![0, 1], vec![0.17_f32, 0.83]),
        (vec![1, 2, 0], vec![0.13_f32, 0.29, 0.58]),
        (vec![2, 1], vec![0.31_f32, 0.69]),
    ] {
        let graphics = indices
            .iter()
            .zip(&weights)
            .map(|(&index, &weight)| {
                let (texture, lookup) = &gradients[index];
                Graphic {
                    gradient: Some(texture.clone()),
                    noise: Some(noise.clone()),
                    geometry: GraphicGeometry {
                        origin: lookup.origin,
                        step_x: lookup.step_x,
                        step_y: lookup.step_y,
                        weight,
                        ..GraphicGeometry::default()
                    },
                    ..Graphic::default()
                }
            })
            .collect();
        let expected: Vec<u8> = (0..(viewport.width_px * viewport.height_px * 4) as usize)
            .map(|channel| {
                if channel % 4 == 3 {
                    return 255;
                }
                let value: f32 = indices
                    .iter()
                    .zip(&weights)
                    .map(|(&index, &weight)| f32::from(pixels[index][channel]) / 255.0 * weight)
                    .sum();
                (value * 255.0).round_ties_even() as u8
            })
            .collect();
        cases.push_back(GradientCase {
            scene: Scene {
                graphics,
                sprites: vec![],
            },
            expected: expected.into(),
            viewport,
        });
    }
    // Reuse one immutable lookup through viewport and coordinate changes.
    // Neither the texture identity nor its palette alone identifies a raster.
    for (target, coordinates, translated) in [
        (Viewport::new(1291, 733), Viewport::new(1291, 733), false),
        (viewport, Viewport::new(1291, 733), false),
        (viewport, viewport, false),
        (viewport, viewport, true),
    ] {
        let lookup = NowPlayingGradientLookup::new(dark, coordinates);
        let reference = NowPlayingGradient::new(dark, coordinates).into_rgba8();
        let mut expected = Vec::with_capacity((target.width_px * target.height_px * 4) as usize);
        for y in 0..target.height_px {
            for x in 0..target.width_px {
                if translated && x == 0 {
                    expected.extend_from_slice(&[0, 0, 0, 255]);
                } else {
                    let x = x - u32::from(translated);
                    let offset = ((y * coordinates.width_px + x) * 4) as usize;
                    expected.extend_from_slice(&reference[offset..offset + 4]);
                }
            }
        }
        cases.push_back(GradientCase {
            scene: Scene {
                graphics: vec![Graphic {
                    gradient: Some(gradients[0].0.clone()),
                    noise: Some(noise.clone()),
                    geometry: GraphicGeometry {
                        canvas: Rect::new(
                            f32::from(translated),
                            0.0,
                            target.width_px as f32,
                            target.height_px as f32,
                        ),
                        origin: lookup.origin,
                        step_x: lookup.step_x,
                        step_y: lookup.step_y,
                        weight: 1.0,
                        ..GraphicGeometry::default()
                    },
                    ..Graphic::default()
                }],
                sprites: vec![],
            },
            expected: expected.into(),
            viewport: target,
        });
    }
    // Splitting a layer into exact binary weights must preserve its pixels.
    // Reordering different half-weight layers must also preserve their sum.
    let decorated = |index: usize, bounds: Rect, radius: f32, weight: f32| {
        let palette = palettes[index];
        let (texture, lookup) = &gradients[index];
        Graphic {
            gradient: Some(texture.clone()),
            noise: Some(noise.clone()),
            geometry: GraphicGeometry {
                canvas: Rect::viewport(viewport),
                artwork_bounds: bounds,
                plate_bounds: Rect::new(
                    bounds.x + 12.0,
                    bounds.y + 8.0,
                    bounds.width,
                    bounds.height,
                ),
                background: Color::new(palette.background, 1.0),
                border: Color::new(palette.primary_text, 0.16),
                plate: Color::new(palette.accent, 1.0),
                quiet: Color::new(palette.artwork_field, 1.0),
                muted: Color::new(palette.muted_text, 1.0),
                origin: lookup.origin,
                step_x: lookup.step_x,
                step_y: lookup.step_y,
                border_width: 2.0,
                shadow_radius: radius,
                shadow_y: 6.0,
                shadow_alpha: 0.38,
                weight,
            },
            ..Graphic::default()
        }
    };
    let first_bounds = Rect::new(160.0, 150.0, 320.0, 320.0);
    let second_bounds = Rect::new(600.0, 250.0, 240.0, 240.0);
    for (graphics, expected) in [
        (
            vec![decorated(0, first_bounds, 28.0, 1.0)],
            ExpectedPixels::SaveReference,
        ),
        (
            vec![
                decorated(0, first_bounds, 28.0, 0.5),
                decorated(0, first_bounds, 28.0, 0.5),
            ],
            ExpectedPixels::SavedReference,
        ),
        (
            vec![
                decorated(0, first_bounds, 28.0, 0.5),
                decorated(0, first_bounds, 28.0, 0.25),
                decorated(0, first_bounds, 28.0, 0.25),
            ],
            ExpectedPixels::SavedReference,
        ),
        (
            vec![
                decorated(0, first_bounds, 28.0, 0.5),
                decorated(1, second_bounds, 13.0, 0.5),
            ],
            ExpectedPixels::SaveReference,
        ),
        (
            vec![
                decorated(1, second_bounds, 13.0, 0.5),
                decorated(0, first_bounds, 28.0, 0.5),
            ],
            ExpectedPixels::SavedReference,
        ),
    ] {
        cases.push_back(GradientCase {
            scene: Scene {
                graphics,
                sprites: vec![],
            },
            expected,
            viewport,
        });
    }
    // Equivalent opaque RGB and RGBA uploads must composite identically,
    // including shared geometry, fractional edges, and multiple draw batches.
    let mut opaque_images = Vec::new();
    let mut rgba_images = Vec::new();
    for index in 0..3 {
        let mut pixels = source.clone();
        for row in pixels.chunks_exact_mut(12) {
            for pixel in row[..9].chunks_exact_mut(3) {
                pixel.rotate_left(index);
            }
        }
        let path = directory.path().join(format!("opaque-{index}.png"));
        gdk_pixbuf::Pixbuf::from_mut_slice(
            pixels.clone(),
            gdk_pixbuf::Colorspace::Rgb,
            false,
            8,
            3,
            2,
            12,
        )
        .savev(&path, "png", &[])
        .unwrap();
        let decoded = qt_window::DecodedImage::open(&path)
            .unwrap()
            .palette_image()
            .unwrap();
        opaque_images.push(uploader.image(&decoded).unwrap());
        let rgba: Vec<_> = pixels
            .chunks_exact(12)
            .flat_map(|row| row[..9].chunks_exact(3))
            .flat_map(|pixel| [pixel[0], pixel[1], pixel[2], 255])
            .collect();
        rgba_images.push(uploader.rgba(3, 2, &rgba).unwrap());
    }
    for (weights, different_geometry) in [
        (vec![1.0], false),
        (vec![0.25, 0.75], false),
        (vec![0.125, 0.25, 0.625], false),
        (vec![0.125, 0.25, 0.625], true),
        (vec![0.125, 0.125, 0.25, 0.5], false),
    ] {
        for (images, expected) in [
            (&rgba_images, ExpectedPixels::SaveReference),
            (&opaque_images, ExpectedPixels::SavedReference),
        ] {
            let graphics = weights
                .iter()
                .enumerate()
                .map(|(index, &weight)| {
                    let bounds = if different_geometry && index == 1 {
                        second_bounds
                    } else {
                        Rect::new(160.25, 150.5, 320.75, 320.125)
                    };
                    let mut graphic = decorated(index % 3, bounds, 28.0, weight);
                    graphic.artwork = Some(images[index % 3].clone());
                    graphic.geometry.shadow_alpha = 0.18 + index as f32 * 0.1;
                    graphic.geometry.canvas.x = 7.0;
                    graphic.geometry.canvas.y = 11.0;
                    graphic
                })
                .collect();
            cases.push_back(GradientCase {
                scene: Scene {
                    graphics,
                    sprites: vec![],
                },
                expected,
                viewport,
            });
        }
    }
    let mut check = GradientCheck {
        cases,
        submitted: false,
        passed: 0,
        reference: None,
    };
    window.timer(15000, 99);
    window.run(&mut check).unwrap();
    assert_eq!(
        check.passed, 35,
        "The native window did not display every test scene"
    );
}
