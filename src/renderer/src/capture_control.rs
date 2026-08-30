use std::error::Error;
use std::fmt;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, RecvError, TryRecvError, channel};
use std::thread;

use serde::Deserialize;
use serde_json::Value;

use crate::contract::{PresentationSnapshot, parse_snapshot};

const MAX_CONTROL_MESSAGE_BYTES: u64 = 64 * 1024;

#[derive(Debug)]
pub struct FixtureSelection {
    scenario: String,
    revision: u64,
    snapshot: PresentationSnapshot,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FixtureSelectionIdentity {
    scenario: String,
    revision: u64,
}

impl FixtureSelection {
    pub fn scenario(&self) -> &str {
        &self.scenario
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn into_snapshot(self) -> PresentationSnapshot {
        self.snapshot
    }

    pub fn identity(&self) -> FixtureSelectionIdentity {
        FixtureSelectionIdentity {
            scenario: self.scenario.clone(),
            revision: self.revision,
        }
    }
}

impl FixtureSelectionIdentity {
    pub fn new(scenario: impl Into<String>, revision: u64) -> Self {
        Self {
            scenario: scenario.into(),
            revision,
        }
    }

    pub fn scenario(&self) -> &str {
        &self.scenario
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }
}

#[derive(Debug, Default)]
pub struct PaintedFixtureSelection {
    pending: Option<FixtureSelectionIdentity>,
    ready_for_paint: Option<FixtureSelectionIdentity>,
}

impl PaintedFixtureSelection {
    pub fn new(initial: Option<FixtureSelectionIdentity>) -> Self {
        Self {
            pending: initial,
            ready_for_paint: None,
        }
    }

    pub fn select(&mut self, selection: FixtureSelectionIdentity) {
        self.pending = Some(selection);
        self.ready_for_paint = None;
    }

    pub fn pending_revision(&self) -> Option<u64> {
        self.pending
            .as_ref()
            .map(FixtureSelectionIdentity::revision)
    }

    pub fn presentation_completed(&mut self, revision: u64) {
        if self.pending_revision() == Some(revision) {
            self.ready_for_paint.clone_from(&self.pending);
        }
    }

    pub fn after_paint(&mut self) -> Option<FixtureSelectionIdentity> {
        let painted = self.ready_for_paint.take()?;
        if self.pending.as_ref() != Some(&painted) {
            return None;
        }
        self.pending = None;
        Some(painted)
    }
}

#[derive(Debug)]
pub enum CaptureControlEvent {
    Selection(Box<FixtureSelection>),
    Disconnected,
    Failed(String),
}

pub struct CaptureControl {
    events: Receiver<CaptureControlEvent>,
    wakeup: UnixStream,
    writer: Mutex<UnixStream>,
}

#[derive(Debug)]
pub enum CaptureControlError {
    Io(io::Error),
    EmptyMessage,
    IncompleteMessage,
    MessageTooLarge,
    InvalidMessage(String),
    RevisionMismatch { requested: u64, snapshot: u64 },
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
enum ControlMessage {
    Select {
        scenario: String,
        revision: u64,
        snapshot: Value,
    },
}

impl fmt::Display for CaptureControlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "could not use capture control channel: {error}"),
            Self::EmptyMessage => formatter.write_str("capture control channel disconnected"),
            Self::IncompleteMessage => {
                formatter.write_str("capture control channel closed during a command")
            }
            Self::MessageTooLarge => formatter.write_str("capture control command exceeds 64 KiB"),
            Self::InvalidMessage(error) => {
                write!(formatter, "invalid capture control command: {error}")
            }
            Self::RevisionMismatch {
                requested,
                snapshot,
            } => write!(
                formatter,
                "capture selection revision {requested} does not match snapshot revision {snapshot}"
            ),
        }
    }
}

impl Error for CaptureControlError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl CaptureControl {
    pub fn connect(
        control_socket_path: &Path,
    ) -> Result<(Self, FixtureSelection), CaptureControlError> {
        let connection =
            UnixStream::connect(control_socket_path).map_err(CaptureControlError::Io)?;
        let writer = connection.try_clone().map_err(CaptureControlError::Io)?;
        let mut reader = BufReader::new(connection);
        let initial = read_selection(&mut reader)?;
        let (sender, events) = channel();
        let (wakeup, mut notifier) =
            UnixStream::pair().expect("the renderer should create a local wakeup socket pair");
        wakeup
            .set_nonblocking(true)
            .expect("the renderer should configure nonblocking wakeup reads");
        notifier
            .set_nonblocking(true)
            .expect("the renderer should configure nonblocking wakeup writes");
        thread::spawn(move || {
            loop {
                let event = match read_selection(&mut reader) {
                    Ok(selection) => CaptureControlEvent::Selection(Box::new(selection)),
                    Err(CaptureControlError::EmptyMessage) => CaptureControlEvent::Disconnected,
                    Err(error) => CaptureControlEvent::Failed(error.to_string()),
                };
                let finished = !matches!(event, CaptureControlEvent::Selection(_));
                if sender.send(event).is_err() {
                    return;
                }
                match notifier.write_all(&[1]) {
                    Ok(()) => {}
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
                    Err(_) => return,
                }
                if finished {
                    return;
                }
            }
        });

        Ok((
            Self {
                events,
                wakeup,
                writer: Mutex::new(writer),
            },
            initial,
        ))
    }

    pub fn acknowledge(&self, selection: &FixtureSelectionIdentity) -> io::Result<()> {
        let message = serde_json::json!({
            "type": "painted",
            "scenario": selection.scenario(),
            "revision": selection.revision(),
        });
        let mut writer = self
            .writer
            .lock()
            .map_err(|_| io::Error::other("capture control writer lock was poisoned"))?;
        writeln!(writer, "{message}")
    }

    pub fn recv(&self) -> Result<CaptureControlEvent, RecvError> {
        self.events.recv()
    }

    pub fn try_recv(&self) -> Result<CaptureControlEvent, TryRecvError> {
        self.events.try_recv()
    }

    pub fn wakeup_fd(&self) -> RawFd {
        self.wakeup.as_raw_fd()
    }

    pub fn clear_wakeup(&self) -> io::Result<bool> {
        let mut signaled = false;
        let mut buffer = [0_u8; 64];
        let mut wakeup = &self.wakeup;
        loop {
            match wakeup.read(&mut buffer) {
                Ok(0) => return Ok(signaled),
                Ok(_) => signaled = true,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(signaled),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => return Err(error),
            }
        }
    }
}

fn read_selection(
    reader: &mut BufReader<UnixStream>,
) -> Result<FixtureSelection, CaptureControlError> {
    let mut limited_reader = reader.take(MAX_CONTROL_MESSAGE_BYTES + 1);
    let mut message = String::new();
    let bytes_read = limited_reader
        .read_line(&mut message)
        .map_err(CaptureControlError::Io)?;
    if bytes_read == 0 {
        return Err(CaptureControlError::EmptyMessage);
    }
    if bytes_read as u64 > MAX_CONTROL_MESSAGE_BYTES {
        return Err(CaptureControlError::MessageTooLarge);
    }
    if !message.ends_with('\n') {
        return Err(CaptureControlError::IncompleteMessage);
    }

    let command: ControlMessage = serde_json::from_str(message.trim_end_matches(['\r', '\n']))
        .map_err(|error| CaptureControlError::InvalidMessage(error.to_string()))?;
    let ControlMessage::Select {
        scenario,
        revision,
        snapshot,
    } = command;
    if scenario.is_empty() {
        return Err(CaptureControlError::InvalidMessage(
            "scenario must not be empty".to_owned(),
        ));
    }
    let snapshot = parse_snapshot(&snapshot.to_string())
        .map_err(|error| CaptureControlError::InvalidMessage(error.to_string()))?;
    if snapshot.revision != revision {
        return Err(CaptureControlError::RevisionMismatch {
            requested: revision,
            snapshot: snapshot.revision,
        });
    }

    Ok(FixtureSelection {
        scenario,
        revision,
        snapshot,
    })
}
