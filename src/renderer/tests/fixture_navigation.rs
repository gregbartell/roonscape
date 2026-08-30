use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixListener;
use std::thread;

use roonscape_renderer::{FixtureNavigation, NavigationIntent};

#[test]
fn emits_semantic_intents_over_the_private_fixture_mode_control_boundary() {
    let runtime_directory = tempfile::Builder::new()
        .prefix("roonscape.")
        .tempdir()
        .expect("private runtime directory should be creatable");
    let control_socket_path = runtime_directory.path().join("fixture-navigation.sock");
    let listener =
        UnixListener::bind(&control_socket_path).expect("Fixture Mode control socket should bind");
    let receiver = thread::spawn(move || {
        let (connection, _) = listener.accept().expect("renderer should connect");
        BufReader::new(connection)
            .lines()
            .take(2)
            .collect::<Result<Vec<_>, _>>()
            .expect("navigation intents should be readable")
    });

    let mut navigation = FixtureNavigation::connect(&control_socket_path)
        .expect("renderer should connect to the private control boundary");
    navigation
        .send(NavigationIntent::Previous)
        .expect("Previous should be sent");
    navigation
        .send(NavigationIntent::Next)
        .expect("Next should be sent");
    drop(navigation);

    assert_eq!(
        receiver.join().expect("control receiver should finish"),
        ["Previous", "Next"]
    );
}
