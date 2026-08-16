use std::io::{self, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;

use crate::keyboard::NavigationIntent;

pub struct FixtureNavigation {
    control: UnixStream,
}

impl FixtureNavigation {
    pub fn connect(control_socket_path: &Path) -> io::Result<Self> {
        Ok(Self {
            control: UnixStream::connect(control_socket_path)?,
        })
    }

    pub fn send(&mut self, intent: NavigationIntent) -> io::Result<()> {
        let message = match intent {
            NavigationIntent::Previous => b"Previous\n" as &[u8],
            NavigationIntent::Next => b"Next\n",
        };
        self.control.write_all(message)
    }
}
