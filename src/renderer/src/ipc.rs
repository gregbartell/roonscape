use std::error::Error;
use std::fmt;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError, TryRecvError, sync_channel};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::contract::{MAX_SNAPSHOT_BYTES, PresentationSnapshot, SnapshotError, parse_snapshot};

pub struct SnapshotReader {
    reader: BufReader<UnixStream>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionState {
    Connected,
    Disconnected,
}

#[derive(Debug, PartialEq)]
pub enum SnapshotEvent {
    ConnectionChanged(ConnectionState),
    Snapshot(Box<PresentationSnapshot>),
    RevisionRejected { incoming: u64, accepted: u64 },
}

pub struct SnapshotSubscription {
    events: Receiver<SnapshotEvent>,
    wakeup: UnixStream,
    writer: Arc<Mutex<Option<UnixStream>>>,
}

#[derive(Debug)]
pub enum SnapshotSocketError {
    Io(io::Error),
    EmptyMessage,
    IncompleteMessage,
    MessageTooLarge,
    InvalidSnapshot(SnapshotError),
}

impl fmt::Display for SnapshotSocketError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "could not read the local socket: {error}"),
            Self::EmptyMessage => formatter.write_str("the local socket closed without a snapshot"),
            Self::IncompleteMessage => {
                formatter.write_str("the local socket closed during a snapshot")
            }
            Self::MessageTooLarge => formatter.write_str("the snapshot exceeds 64 KiB"),
            Self::InvalidSnapshot(error) => error.fmt(formatter),
        }
    }
}

impl Error for SnapshotSocketError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::InvalidSnapshot(error) => Some(error),
            Self::EmptyMessage | Self::IncompleteMessage | Self::MessageTooLarge => None,
        }
    }
}

pub fn read_snapshot_from_socket(
    socket_path: &Path,
) -> Result<PresentationSnapshot, SnapshotSocketError> {
    SnapshotReader::connect(socket_path)?.read_snapshot()
}

impl SnapshotReader {
    pub fn connect(socket_path: &Path) -> Result<Self, SnapshotSocketError> {
        let connection = UnixStream::connect(socket_path).map_err(SnapshotSocketError::Io)?;
        Ok(Self {
            reader: BufReader::new(connection),
        })
    }

    pub fn read_snapshot(&mut self) -> Result<PresentationSnapshot, SnapshotSocketError> {
        read_snapshot(&mut self.reader)
    }

    fn try_clone_connection(&self) -> io::Result<UnixStream> {
        self.reader.get_ref().try_clone()
    }
}

impl SnapshotSubscription {
    pub fn start(socket_path: PathBuf, retry_delay: Duration) -> Self {
        let (sender, events) = sync_channel(1);
        let (wakeup, mut notifier) =
            UnixStream::pair().expect("the renderer should create a local wakeup socket pair");
        wakeup
            .set_nonblocking(true)
            .expect("the renderer should configure nonblocking wakeup reads");
        notifier
            .set_nonblocking(true)
            .expect("the renderer should configure nonblocking wakeup writes");
        let writer = Arc::new(Mutex::new(None));
        let worker_writer = Arc::clone(&writer);
        thread::spawn(move || {
            if !notify(
                &sender,
                &mut notifier,
                SnapshotEvent::ConnectionChanged(ConnectionState::Disconnected),
            ) {
                return;
            }

            loop {
                let Ok(mut reader) = SnapshotReader::connect(&socket_path) else {
                    thread::sleep(retry_delay);
                    continue;
                };
                let Ok(connection_writer) = reader.try_clone_connection() else {
                    thread::sleep(retry_delay);
                    continue;
                };
                let Ok(mut active_writer) = worker_writer.lock() else {
                    return;
                };
                *active_writer = Some(connection_writer);
                drop(active_writer);
                if !notify(
                    &sender,
                    &mut notifier,
                    SnapshotEvent::ConnectionChanged(ConnectionState::Connected),
                ) {
                    return;
                }
                let mut accepted_revision = None;
                let mut last_reported_rejection = None;

                loop {
                    match reader.read_snapshot() {
                        Ok(snapshot) => {
                            if let Some(accepted) = accepted_revision
                                && snapshot.revision <= accepted
                            {
                                let rejection = (snapshot.revision, accepted);
                                if last_reported_rejection != Some(rejection) {
                                    if !notify(
                                        &sender,
                                        &mut notifier,
                                        SnapshotEvent::RevisionRejected {
                                            incoming: snapshot.revision,
                                            accepted,
                                        },
                                    ) {
                                        return;
                                    }
                                    last_reported_rejection = Some(rejection);
                                }
                                continue;
                            }
                            accepted_revision = Some(snapshot.revision);
                            last_reported_rejection = None;
                            if !notify(
                                &sender,
                                &mut notifier,
                                SnapshotEvent::Snapshot(Box::new(snapshot)),
                            ) {
                                return;
                            }
                        }
                        Err(error) => {
                            eprintln!("RoonScape Renderer: {error}");
                            break;
                        }
                    }
                }

                let Ok(mut active_writer) = worker_writer.lock() else {
                    return;
                };
                *active_writer = None;
                drop(active_writer);

                if !notify(
                    &sender,
                    &mut notifier,
                    SnapshotEvent::ConnectionChanged(ConnectionState::Disconnected),
                ) {
                    return;
                }
                thread::sleep(retry_delay);
            }
        });

        Self {
            events,
            wakeup,
            writer,
        }
    }

    pub fn try_recv(&self) -> Result<SnapshotEvent, TryRecvError> {
        self.events.try_recv()
    }

    pub fn recv_timeout(&self, timeout: Duration) -> Result<SnapshotEvent, RecvTimeoutError> {
        self.events.recv_timeout(timeout)
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

    pub fn report_lyrics_visible(&self, revision: u64) -> io::Result<bool> {
        let mut writer = self
            .writer
            .lock()
            .map_err(|_| io::Error::other("snapshot connection writer lock was poisoned"))?;
        let Some(connection) = writer.as_mut() else {
            return Ok(false);
        };
        let report = format!("{{\"type\":\"lyricsVisible\",\"revision\":{revision}}}\n");
        if let Err(error) = connection.write_all(report.as_bytes()) {
            *writer = None;
            return Err(error);
        }
        Ok(true)
    }
}

fn notify(
    sender: &std::sync::mpsc::SyncSender<SnapshotEvent>,
    notifier: &mut UnixStream,
    event: SnapshotEvent,
) -> bool {
    if sender.send(event).is_err() {
        return false;
    }
    match notifier.write_all(&[1]) {
        Ok(()) => true,
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => true,
        Err(_) => false,
    }
}

fn read_snapshot(
    reader: &mut BufReader<UnixStream>,
) -> Result<PresentationSnapshot, SnapshotSocketError> {
    let mut limited_reader = reader.take(MAX_SNAPSHOT_BYTES + 1);
    let mut message = String::new();
    let bytes_read = limited_reader
        .read_line(&mut message)
        .map_err(SnapshotSocketError::Io)?;

    if bytes_read == 0 {
        return Err(SnapshotSocketError::EmptyMessage);
    }
    if bytes_read as u64 > MAX_SNAPSHOT_BYTES {
        return Err(SnapshotSocketError::MessageTooLarge);
    }
    if !message.ends_with('\n') {
        return Err(SnapshotSocketError::IncompleteMessage);
    }

    parse_snapshot(message.trim_end_matches(['\r', '\n']))
        .map_err(SnapshotSocketError::InvalidSnapshot)
}
