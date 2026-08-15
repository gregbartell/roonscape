use std::fs;
use std::io::Write;
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::thread;

mod support;

use roonscape_renderer::{Playback, read_snapshot_from_socket};

#[test]
fn reads_one_complete_snapshot_from_the_local_socket() {
    let scratch_root = Path::new("/tmp/codex/roonscape");
    fs::create_dir_all(scratch_root).expect("scratch root should be creatable");
    let runtime_directory = tempfile::Builder::new()
        .prefix("task.")
        .tempdir_in(scratch_root)
        .expect("private runtime directory should be creatable");
    let socket_path = runtime_directory.path().join("roonscape.sock");
    let listener = UnixListener::bind(&socket_path).expect("fixture socket should bind");
    let fixture = support::fixture("playing.json");
    let fixture: serde_json::Value =
        serde_json::from_str(&fixture).expect("shared Playing fixture should be JSON");
    let fixture = serde_json::to_string(&fixture).expect("fixture should serialize on one line");

    let publisher = thread::spawn(move || {
        let (mut connection, _) = listener.accept().expect("renderer should connect");
        connection
            .write_all(format!("{fixture}\n").as_bytes())
            .expect("snapshot should be sent");
    });

    let snapshot = read_snapshot_from_socket(&socket_path)
        .expect("renderer should read the published snapshot");

    assert_eq!(snapshot.revision, 7);
    assert_eq!(snapshot.playback, Some(Playback::Playing));
    publisher.join().expect("fixture publisher should finish");
}
