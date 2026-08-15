use std::error::Error;
use std::fmt;
use std::io::{self, BufRead, BufReader, Read};
use std::os::unix::net::UnixStream;
use std::path::Path;

use crate::contract::{PresentationSnapshot, SnapshotError, parse_snapshot};

const MAX_SNAPSHOT_BYTES: u64 = 64 * 1024;

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
    let connection = UnixStream::connect(socket_path).map_err(SnapshotSocketError::Io)?;
    let mut reader = BufReader::new(connection).take(MAX_SNAPSHOT_BYTES + 1);
    let mut message = String::new();
    let bytes_read = reader
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
