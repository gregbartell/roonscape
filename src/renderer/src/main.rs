mod animation_evidence;
mod displayed_state;
mod lyric_motion;
mod native_runtime;
mod native_view;
mod prepared_presentation;
// Native GPU helpers run in the dedicated integration harnesses.
#[cfg_attr(test, allow(dead_code))]
mod qt_window;
mod scene;
mod status_glyph;
#[cfg_attr(test, allow(dead_code))]
mod text_preparation;

use std::env;
use std::error::Error;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use roonscape_renderer::{
    InactivityConfiguration, NowPlayingTitleFace, PresentationBehavior, Viewport,
    display_configuration_file_path, load_inactivity_configuration,
};

#[derive(Clone, Copy)]
struct CaptureConfiguration {
    viewport: Option<Viewport>,
    typography: Option<NowPlayingTitleFace>,
    reduced_animation: bool,
}
#[derive(Clone, Copy)]
struct RendererConfiguration {
    capture: CaptureConfiguration,
    behavior: PresentationBehavior,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("RoonScape Renderer: {error}");
            ExitCode::FAILURE
        }
    }
}
fn run() -> Result<(), Box<dyn Error>> {
    animation_evidence::initialize()?;
    let result = native_runtime::run();
    let evidence = animation_evidence::finish();
    result?;
    evidence?;
    Ok(())
}

fn resource_root() -> Result<PathBuf, io::Error> {
    env::current_exe()?
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "renderer executable should be inside target/release",
            )
        })
}

fn capture_configuration_from_environment() -> Result<CaptureConfiguration, Box<dyn Error>> {
    let viewport = env::var("ROONSCAPE_CAPTURE_VIEWPORT")
        .ok()
        .map(|value| parse_capture_viewport(&value))
        .transpose()?;
    let typography = env::var("ROONSCAPE_CAPTURE_TYPOGRAPHY")
        .ok()
        .map(|value| match value.as_str() {
            "preferred" => Ok(NowPlayingTitleFace::Preferred),
            "fallback" => Ok(NowPlayingTitleFace::Fallback),
            _ => Err("ROONSCAPE_CAPTURE_TYPOGRAPHY must be preferred or fallback"),
        })
        .transpose()?;
    if typography.is_some() && viewport.is_none() {
        return Err("ROONSCAPE_CAPTURE_TYPOGRAPHY requires ROONSCAPE_CAPTURE_VIEWPORT".into());
    }
    let reduced_animation = env::var("ROONSCAPE_CAPTURE_REDUCED_ANIMATION").as_deref() == Ok("1");
    if reduced_animation && viewport.is_none() {
        return Err(
            "ROONSCAPE_CAPTURE_REDUCED_ANIMATION requires ROONSCAPE_CAPTURE_VIEWPORT".into(),
        );
    }

    Ok(CaptureConfiguration {
        viewport,
        typography,
        reduced_animation,
    })
}

fn renderer_configuration_from_environment() -> Result<RendererConfiguration, Box<dyn Error>> {
    Ok(RendererConfiguration {
        capture: capture_configuration_from_environment()?,
        behavior: if env::var("ROONSCAPE_STATIC_FIXTURE").as_deref() == Ok("1") {
            PresentationBehavior::StaticFixture
        } else {
            PresentationBehavior::Dynamic
        },
    })
}

fn parse_capture_viewport(value: &str) -> Result<Viewport, Box<dyn Error>> {
    let (width, height) = value
        .split_once('x')
        .ok_or("ROONSCAPE_CAPTURE_VIEWPORT must use WIDTHxHEIGHT")?;
    let width = width
        .parse::<u32>()
        .map_err(|_| "ROONSCAPE_CAPTURE_VIEWPORT width must be a positive integer")?;
    let height = height
        .parse::<u32>()
        .map_err(|_| "ROONSCAPE_CAPTURE_VIEWPORT height must be a positive integer")?;
    if width == 0 || height == 0 || width > i32::MAX as u32 || height > i32::MAX as u32 {
        return Err("ROONSCAPE_CAPTURE_VIEWPORT dimensions must fit positive window sizes".into());
    }

    Ok(Viewport::new(width, height))
}

fn configuration_file_from_arguments() -> Result<PathBuf, Box<dyn Error>> {
    let mut arguments = env::args_os().skip(1);
    match (arguments.next(), arguments.next(), arguments.next()) {
        (None, None, None) => Ok(display_configuration_file_path()?),
        (Some(option), Some(configuration_file), None)
            if option == "--config" && !configuration_file.is_empty() =>
        {
            Ok(PathBuf::from(configuration_file))
        }
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "RoonScape Renderer accepts only a launcher-provided --config PATH",
        )
        .into()),
    }
}

fn host_inactivity_configuration(configuration_file: &Path) -> InactivityConfiguration {
    let configuration = load_inactivity_configuration(configuration_file);
    match configuration {
        Ok(configuration) => configuration,
        Err(error) => {
            eprintln!("RoonScape Renderer: {error}; using default OLED inactivity calibration");
            InactivityConfiguration::default()
        }
    }
}
