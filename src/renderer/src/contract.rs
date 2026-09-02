use std::error::Error;
use std::fmt;
use std::sync::LazyLock;

use jsonschema::Validator;
use serde::Deserialize;
use serde_json::Value;

const SNAPSHOT_SCHEMA: &str = include_str!("../../shared/schema/presentation-snapshot.schema.json");
pub(crate) const MAX_SNAPSHOT_BYTES: u64 = 64 * 1024;
const MAX_LYRIC_TOTAL_CODE_POINTS: usize = 16_384;
static SNAPSHOT_VALIDATOR: LazyLock<Validator> = LazyLock::new(|| {
    let schema: Value = serde_json::from_str(SNAPSHOT_SCHEMA)
        .expect("embedded presentation snapshot schema should be valid JSON");
    jsonschema::options()
        .should_validate_formats(true)
        .build(&schema)
        .expect("embedded presentation snapshot schema should compile")
});

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PresentationSnapshot {
    pub schema_version: u32,
    pub revision: u64,
    pub availability: Availability,
    pub playback: Option<Playback>,
    pub tracked_output: Option<TrackedOutput>,
    pub tracked_zone: Option<TrackedZone>,
    pub now_playing: Option<NowPlaying>,
    pub progress: Option<Progress>,
    pub artwork: Option<ArtworkReference>,
    pub lyrics: Option<SynchronizedLyrics>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Availability {
    PairingRequired,
    Disconnected,
    OutputUnavailable,
    Available,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Playback {
    Playing,
    Paused,
    Loading,
    Stopped,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TrackedOutput {
    pub name: String,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TrackedZone {
    pub name: String,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct NowPlaying {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Progress {
    pub position_seconds: f64,
    pub duration_seconds: f64,
    pub sampled_at: String,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ArtworkReference {
    pub revision: u64,
    pub path: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SynchronizedLyrics {
    pub cues: Vec<LyricCue>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LyricCue {
    pub at_seconds: f64,
    pub text: String,
}

#[derive(Debug)]
pub enum SnapshotError {
    Json(serde_json::Error),
    MessageTooLarge,
    Schema(String),
}

impl fmt::Display for SnapshotError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(formatter, "snapshot is not valid JSON: {error}"),
            Self::MessageTooLarge => formatter.write_str("snapshot exceeds 64 KiB"),
            Self::Schema(error) => write!(formatter, "snapshot violates the schema: {error}"),
        }
    }
}

impl Error for SnapshotError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Json(error) => Some(error),
            Self::MessageTooLarge | Self::Schema(_) => None,
        }
    }
}

pub fn parse_snapshot(contents: &str) -> Result<PresentationSnapshot, SnapshotError> {
    let message = contents.trim_end_matches(['\r', '\n']);
    if message.len() as u64 + 1 > MAX_SNAPSHOT_BYTES {
        return Err(SnapshotError::MessageTooLarge);
    }
    let candidate: Value = serde_json::from_str(message).map_err(SnapshotError::Json)?;

    SNAPSHOT_VALIDATOR
        .validate(&candidate)
        .map_err(|error| SnapshotError::Schema(error.to_string()))?;

    let snapshot: PresentationSnapshot =
        serde_json::from_value(candidate).map_err(SnapshotError::Json)?;
    validate_lyrics(&snapshot)?;
    Ok(snapshot)
}

fn validate_lyrics(snapshot: &PresentationSnapshot) -> Result<(), SnapshotError> {
    let Some(lyrics) = &snapshot.lyrics else {
        return Ok(());
    };
    if lyrics
        .cues
        .windows(2)
        .any(|pair| pair[0].at_seconds >= pair[1].at_seconds)
    {
        return Err(SnapshotError::Schema(
            "synchronized lyric cue timestamps must be strictly increasing".to_owned(),
        ));
    }
    if lyrics
        .cues
        .iter()
        .map(|cue| cue.text.chars().count())
        .sum::<usize>()
        > MAX_LYRIC_TOTAL_CODE_POINTS
    {
        return Err(SnapshotError::Schema(
            "synchronized lyric total lyric text exceeds 16,384 Unicode code points".to_owned(),
        ));
    }
    Ok(())
}
