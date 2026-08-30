use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::thread;

use roonscape_renderer::{
    CaptureControl, CaptureControlEvent, FixtureSelectionIdentity, PaintedFixtureSelection,
};

mod support;

#[test]
fn receives_exact_fixture_scenario_revisions_and_acknowledges_only_the_painted_selection() {
    let runtime_directory = tempfile::Builder::new()
        .prefix("roonscape.")
        .tempdir()
        .expect("private runtime directory should be creatable");
    let control_socket_path = runtime_directory.path().join("capture-control.sock");
    let listener =
        UnixListener::bind(&control_socket_path).expect("capture control socket should bind");
    let first_snapshot = support::fixture("playing.json");
    let second_snapshot = support::fixture("stopped.json");
    let controller = thread::spawn(move || {
        let (mut connection, _) = listener.accept().expect("renderer should connect");
        writeln!(
            connection,
            "{{\"type\":\"select\",\"scenario\":\"playing\",\"revision\":41,\"snapshot\":{}}}",
            snapshot_at_revision(&first_snapshot, 41)
        )
        .expect("initial selection should be writable");
        writeln!(
            connection,
            "{{\"type\":\"select\",\"scenario\":\"idle\",\"revision\":42,\"snapshot\":{}}}",
            snapshot_at_revision(&second_snapshot, 42)
        )
        .expect("repeated selection should be writable");

        BufReader::new(connection)
            .lines()
            .next()
            .expect("renderer should acknowledge")
            .expect("acknowledgement should be readable")
    });

    let (control, initial) = CaptureControl::connect(&control_socket_path)
        .expect("renderer should accept the initial Fixture Scenario selection");
    assert_eq!(initial.scenario(), "playing");
    assert_eq!(initial.revision(), 41);

    let CaptureControlEvent::Selection(repeated) = control
        .recv()
        .expect("renderer should receive a repeated selection")
    else {
        panic!("expected a repeated Fixture Scenario selection");
    };
    assert_eq!(repeated.scenario(), "idle");
    assert_eq!(repeated.revision(), 42);

    control
        .acknowledge(&repeated.identity())
        .expect("the painted revision should be acknowledged");
    let acknowledgement: serde_json::Value =
        serde_json::from_str(&controller.join().expect("controller should finish"))
            .expect("acknowledgement should be JSON");
    assert_eq!(
        acknowledgement,
        serde_json::json!({"type": "painted", "scenario": "idle", "revision": 42})
    );
}

#[test]
fn rejects_a_selection_whose_command_and_snapshot_revisions_differ() {
    let runtime_directory = tempfile::Builder::new()
        .prefix("roonscape.")
        .tempdir()
        .expect("private runtime directory should be creatable");
    let control_socket_path = runtime_directory.path().join("capture-control.sock");
    let listener =
        UnixListener::bind(&control_socket_path).expect("capture control socket should bind");
    let snapshot = support::fixture("playing.json");
    thread::spawn(move || {
        let (mut connection, _) = listener.accept().expect("renderer should connect");
        writeln!(
            connection,
            "{{\"type\":\"select\",\"scenario\":\"playing\",\"revision\":8,\"snapshot\":{}}}",
            snapshot_at_revision(&snapshot, 7)
        )
        .expect("selection should be writable");
    });

    let error = CaptureControl::connect(&control_socket_path)
        .err()
        .expect("a mismatched revision should fail the session");
    assert_eq!(
        error.to_string(),
        "capture selection revision 8 does not match snapshot revision 7"
    );
}

#[test]
fn waits_for_deferred_presentation_work_and_a_subsequent_paint_without_using_stale_readiness() {
    let mut painted = PaintedFixtureSelection::new(Some(FixtureSelectionIdentity::new(
        "output-unavailable",
        51,
    )));

    assert_eq!(painted.after_paint(), None, "fitting is still deferred");
    painted.presentation_completed(51);
    painted.select(FixtureSelectionIdentity::new("playing", 52));
    assert_eq!(
        painted.after_paint(),
        None,
        "a stale completed presentation cannot satisfy the new selection"
    );

    painted.presentation_completed(52);
    assert_eq!(
        painted.after_paint(),
        Some(FixtureSelectionIdentity::new("playing", 52)),
        "only a paint after the exact presentation completed is ready"
    );
    assert_eq!(
        painted.after_paint(),
        None,
        "one frame is acknowledged once"
    );
}

fn snapshot_at_revision(snapshot: &str, revision: u64) -> String {
    let mut snapshot: serde_json::Value =
        serde_json::from_str(snapshot).expect("fixture should be valid JSON");
    snapshot["revision"] = revision.into();
    snapshot.to_string()
}
