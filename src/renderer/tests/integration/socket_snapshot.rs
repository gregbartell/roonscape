use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::support;

use roonscape_renderer::{
    Availability, ConnectionState, Playback, SnapshotEvent, SnapshotReader, SnapshotSubscription,
    read_snapshot_from_socket,
};

#[test]
fn reads_one_complete_snapshot_from_the_local_socket() {
    let runtime_directory = runtime_directory();
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

#[test]
fn reads_availability_transitions_from_one_local_socket_connection() {
    let runtime_directory = runtime_directory();
    let socket_path = runtime_directory.path().join("roonscape.sock");
    let listener = UnixListener::bind(&socket_path).expect("fixture socket should bind");
    let pairing_required = compact_fixture("pairing-required.json");
    let output_unavailable = compact_fixture("output-unavailable.json");

    let publisher = thread::spawn(move || {
        let (mut connection, _) = listener.accept().expect("renderer should connect");
        connection
            .write_all(format!("{pairing_required}\n{output_unavailable}\n").as_bytes())
            .expect("availability snapshots should be sent");
    });

    let mut reader = SnapshotReader::connect(&socket_path).expect("renderer should connect");
    let initial = reader
        .read_snapshot()
        .expect("renderer should read pairing state");
    let transition = reader
        .read_snapshot()
        .expect("renderer should read connected state");

    assert_eq!(initial.availability, Availability::PairingRequired);
    assert_eq!(transition.availability, Availability::OutputUnavailable);
    publisher.join().expect("fixture publisher should finish");
}

#[test]
fn reports_a_visible_lyric_revision_to_the_snapshot_publisher() {
    let runtime_directory = runtime_directory();
    let socket_path = runtime_directory.path().join("roonscape.sock");
    let listener = UnixListener::bind(&socket_path).expect("fixture socket should bind");
    let lyrics = compact_fixture("lyrics-one-line.json");
    let (report, reported) = mpsc::channel();
    let publisher = thread::spawn(move || {
        let (mut connection, _) = listener.accept().expect("renderer should connect");
        let reader_connection = connection
            .try_clone()
            .expect("publisher should clone the connection for reports");
        connection
            .write_all(format!("{lyrics}\n").as_bytes())
            .expect("lyric snapshot should be sent");
        let mut line = String::new();
        BufReader::new(reader_connection)
            .read_line(&mut line)
            .expect("visibility report should be readable");
        report.send(line).expect("test should receive the report");
    });
    let subscription = SnapshotSubscription::start(socket_path, Duration::from_millis(10));

    expect_connection(&subscription, ConnectionState::Disconnected);
    expect_connection(&subscription, ConnectionState::Connected);
    let revision = match subscription
        .recv_timeout(Duration::from_secs(1))
        .expect("renderer should receive lyric state")
    {
        SnapshotEvent::Snapshot(snapshot) => snapshot.revision,
        event => panic!("expected a snapshot, got {event:?}"),
    };
    assert!(
        subscription
            .report_lyrics_visible(revision)
            .expect("visibility report should be sent")
    );
    assert_eq!(
        reported
            .recv_timeout(Duration::from_secs(1))
            .expect("publisher should receive visibility"),
        format!("{{\"type\":\"lyricsVisible\",\"revision\":{revision}}}\n")
    );

    publisher.join().expect("fixture publisher should finish");
}

#[test]
fn reconnects_when_the_publisher_starts_later_and_replays_its_current_snapshot() {
    let runtime_directory = runtime_directory();
    let socket_path = runtime_directory.path().join("roonscape.sock");
    let subscription = SnapshotSubscription::start(socket_path.clone(), Duration::from_millis(10));

    assert_eq!(
        subscription
            .recv_timeout(Duration::from_secs(1))
            .expect("renderer should begin disconnected"),
        SnapshotEvent::ConnectionChanged(ConnectionState::Disconnected)
    );

    let listener = UnixListener::bind(&socket_path).expect("fixture socket should bind");
    let playing = compact_fixture("playing.json");
    let paused = snapshot_at_revision("paused.json", 7);
    let (disconnect, disconnect_requested) = mpsc::channel();
    let publisher = thread::spawn(move || {
        let (mut first_connection, _) = listener.accept().expect("renderer should connect");
        first_connection
            .write_all(format!("{playing}\n").as_bytes())
            .expect("current snapshot should be sent");
        disconnect_requested
            .recv()
            .expect("test should request a disconnect");
        drop(first_connection);

        let (mut reconnected, _) = listener.accept().expect("renderer should reconnect");
        reconnected
            .write_all(format!("{paused}\n").as_bytes())
            .expect("new current snapshot should be replayed");
    });

    assert_eq!(
        subscription
            .recv_timeout(Duration::from_secs(1))
            .expect("renderer should report the connection"),
        SnapshotEvent::ConnectionChanged(ConnectionState::Connected)
    );
    assert_snapshot_event(
        subscription
            .recv_timeout(Duration::from_secs(1))
            .expect("renderer should receive current state"),
        7,
        Playback::Playing,
    );

    disconnect.send(()).expect("publisher should be listening");
    assert_eq!(
        subscription
            .recv_timeout(Duration::from_secs(1))
            .expect("renderer should report lost connection"),
        SnapshotEvent::ConnectionChanged(ConnectionState::Disconnected)
    );
    assert_eq!(
        subscription
            .recv_timeout(Duration::from_secs(1))
            .expect("renderer should reconnect"),
        SnapshotEvent::ConnectionChanged(ConnectionState::Connected)
    );
    assert_snapshot_event(
        subscription
            .recv_timeout(Duration::from_secs(1))
            .expect("renderer should receive replayed current state"),
        7,
        Playback::Paused,
    );

    publisher.join().expect("fixture publisher should finish");
}

#[test]
fn accepts_a_lower_baseline_after_the_publisher_restarts() {
    let runtime_directory = runtime_directory();
    let socket_path = runtime_directory.path().join("roonscape.sock");
    let listener = UnixListener::bind(&socket_path).expect("fixture socket should bind");
    let playing = snapshot_at_revision("playing.json", 40);
    let restarted = snapshot_at_revision("paused.json", 1);
    let (disconnect, disconnect_requested) = mpsc::channel();
    let publisher = thread::spawn(move || {
        let (mut first_connection, _) = listener.accept().expect("renderer should connect");
        first_connection
            .write_all(format!("{playing}\n").as_bytes())
            .expect("current snapshot should be sent");
        disconnect_requested
            .recv()
            .expect("test should request a publisher restart");
        drop(first_connection);

        let (mut restarted_connection, _) = listener.accept().expect("renderer should reconnect");
        restarted_connection
            .write_all(format!("{restarted}\n").as_bytes())
            .expect("restarted publisher snapshot should be sent");
    });
    let subscription = SnapshotSubscription::start(socket_path, Duration::from_millis(10));

    expect_connection(&subscription, ConnectionState::Disconnected);
    expect_connection(&subscription, ConnectionState::Connected);
    assert_snapshot_event(
        subscription
            .recv_timeout(Duration::from_secs(1))
            .expect("renderer should accept the first epoch"),
        40,
        Playback::Playing,
    );
    disconnect.send(()).expect("publisher should be listening");
    expect_connection(&subscription, ConnectionState::Disconnected);
    expect_connection(&subscription, ConnectionState::Connected);
    assert_snapshot_event(
        subscription
            .recv_timeout(Duration::from_secs(1))
            .expect("renderer should accept the restarted epoch baseline"),
        1,
        Playback::Paused,
    );

    publisher.join().expect("fixture publisher should finish");
}

#[test]
fn ignores_and_reports_a_duplicate_revision_on_one_connection() {
    let runtime_directory = runtime_directory();
    let socket_path = runtime_directory.path().join("roonscape.sock");
    let listener = UnixListener::bind(&socket_path).expect("fixture socket should bind");
    let playing = snapshot_at_revision("playing.json", 7);
    let paused = snapshot_at_revision("paused.json", 7);

    let publisher = thread::spawn(move || {
        let (mut connection, _) = listener.accept().expect("renderer should connect");
        connection
            .write_all(format!("{playing}\n{paused}\n").as_bytes())
            .expect("snapshots should be sent");
    });
    let subscription = SnapshotSubscription::start(socket_path, Duration::from_millis(10));

    expect_connection(&subscription, ConnectionState::Disconnected);
    expect_connection(&subscription, ConnectionState::Connected);
    assert_snapshot_event(
        subscription
            .recv_timeout(Duration::from_secs(1))
            .expect("renderer should accept the epoch baseline"),
        7,
        Playback::Playing,
    );
    assert_eq!(
        subscription
            .recv_timeout(Duration::from_secs(1))
            .expect("renderer should report the duplicate revision"),
        SnapshotEvent::RevisionRejected {
            incoming: 7,
            accepted: 7,
        }
    );

    publisher.join().expect("fixture publisher should finish");
}

#[test]
fn deduplicates_stale_revision_reports_until_an_increasing_snapshot_is_accepted() {
    let runtime_directory = runtime_directory();
    let socket_path = runtime_directory.path().join("roonscape.sock");
    let listener = UnixListener::bind(&socket_path).expect("fixture socket should bind");
    let baseline = snapshot_at_revision("playing.json", 10);
    let stale = snapshot_at_revision("paused.json", 9);
    let increasing = snapshot_at_revision("paused.json", 11);
    let stale_after_increase = snapshot_at_revision("playing.json", 10);

    let publisher = thread::spawn(move || {
        let (mut connection, _) = listener.accept().expect("renderer should connect");
        connection
            .write_all(
                format!("{baseline}\n{stale}\n{stale}\n{increasing}\n{stale_after_increase}\n")
                    .as_bytes(),
            )
            .expect("snapshots should be sent");
    });
    let subscription = SnapshotSubscription::start(socket_path, Duration::from_millis(10));

    expect_connection(&subscription, ConnectionState::Disconnected);
    expect_connection(&subscription, ConnectionState::Connected);
    assert_snapshot_event(
        subscription
            .recv_timeout(Duration::from_secs(1))
            .expect("renderer should accept the epoch baseline"),
        10,
        Playback::Playing,
    );
    assert_eq!(
        subscription
            .recv_timeout(Duration::from_secs(1))
            .expect("renderer should report the decreasing revision"),
        SnapshotEvent::RevisionRejected {
            incoming: 9,
            accepted: 10,
        }
    );
    assert_snapshot_event(
        subscription
            .recv_timeout(Duration::from_secs(1))
            .expect("an identical stale revision should not emit another report"),
        11,
        Playback::Paused,
    );
    assert_eq!(
        subscription
            .recv_timeout(Duration::from_secs(1))
            .expect("a valid revision should reset stale-report deduplication"),
        SnapshotEvent::RevisionRejected {
            incoming: 10,
            accepted: 11,
        }
    );

    publisher.join().expect("fixture publisher should finish");
}

#[test]
fn signals_the_main_loop_when_a_subscription_event_arrives() {
    let runtime_directory = runtime_directory();
    let subscription = SnapshotSubscription::start(
        runtime_directory.path().join("missing.sock"),
        Duration::from_secs(1),
    );

    assert_eq!(
        subscription
            .recv_timeout(Duration::from_secs(1))
            .expect("renderer should begin disconnected"),
        SnapshotEvent::ConnectionChanged(ConnectionState::Disconnected)
    );
    let mut signaled = false;
    for _ in 0..100 {
        signaled = subscription
            .clear_wakeup()
            .expect("the subscription wakeup should be readable");
        if signaled {
            break;
        }
        thread::sleep(Duration::from_millis(1));
    }
    assert!(signaled);
    assert!(subscription.wakeup_fd() >= 0);
    assert!(
        !subscription
            .clear_wakeup()
            .expect("clearing the wakeup should drain it")
    );
}

#[test]
fn rejects_malformed_and_oversized_frames_then_recovers_current_state() {
    let runtime_directory = runtime_directory();
    let socket_path = runtime_directory.path().join("roonscape.sock");
    let listener = UnixListener::bind(&socket_path).expect("fixture socket should bind");
    let playing = compact_fixture("playing.json");

    let publisher = thread::spawn(move || {
        let (mut malformed_connection, _) = listener.accept().expect("renderer should connect");
        malformed_connection
            .write_all(b"{not-json}\n")
            .expect("malformed frame should be sent");
        drop(malformed_connection);

        let (mut oversized_connection, _) = listener.accept().expect("renderer should reconnect");
        oversized_connection
            .write_all(format!("{}\n", "x".repeat(64 * 1024)).as_bytes())
            .expect("oversized frame should be sent");
        drop(oversized_connection);

        let (mut recovered_connection, _) = listener.accept().expect("renderer should reconnect");
        recovered_connection
            .write_all(format!("{playing}\n").as_bytes())
            .expect("current snapshot should be sent");
    });
    let subscription = SnapshotSubscription::start(socket_path, Duration::from_millis(10));

    expect_connection(&subscription, ConnectionState::Disconnected);
    expect_connection(&subscription, ConnectionState::Connected);
    expect_connection(&subscription, ConnectionState::Disconnected);
    expect_connection(&subscription, ConnectionState::Connected);
    expect_connection(&subscription, ConnectionState::Disconnected);
    expect_connection(&subscription, ConnectionState::Connected);
    assert_snapshot_event(
        subscription
            .recv_timeout(Duration::from_secs(1))
            .expect("renderer should receive valid current state after framing failures"),
        7,
        Playback::Playing,
    );

    publisher.join().expect("fixture publisher should finish");
}

fn runtime_directory() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("roonscape.")
        .tempdir()
        .expect("private runtime directory should be creatable")
}

fn compact_fixture(name: &str) -> String {
    let fixture = support::fixture(name);
    let fixture: serde_json::Value =
        serde_json::from_str(&fixture).expect("shared fixture should be JSON");
    serde_json::to_string(&fixture).expect("fixture should serialize on one line")
}

fn snapshot_at_revision(name: &str, revision: u64) -> String {
    let mut snapshot: serde_json::Value =
        serde_json::from_str(&support::fixture(name)).expect("shared fixture should be JSON");
    snapshot["revision"] = revision.into();
    serde_json::to_string(&snapshot).expect("fixture should serialize on one line")
}

fn assert_snapshot_event(event: SnapshotEvent, revision: u64, playback: Playback) {
    let SnapshotEvent::Snapshot(snapshot) = event else {
        panic!("expected a complete snapshot event, got {event:?}");
    };
    assert_eq!(snapshot.revision, revision);
    assert_eq!(snapshot.playback, Some(playback));
}

fn expect_connection(subscription: &SnapshotSubscription, expected: ConnectionState) {
    assert_eq!(
        subscription
            .recv_timeout(Duration::from_secs(1))
            .expect("renderer should report connection state"),
        SnapshotEvent::ConnectionChanged(expected)
    );
}
