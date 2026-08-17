use std::env;
use std::error::Error;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant, SystemTime};

use roonscape_renderer::{
    Availability, ConnectionState, Presentation, PresentationState, PresentationStatusSymbol,
    PresentationTime, SnapshotEvent, SnapshotSubscription,
};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("RoonScape IPC probe: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let socket_path = env::var_os("ROONSCAPE_SOCKET")
        .map(PathBuf::from)
        .ok_or("ROONSCAPE_SOCKET must name the private Unix socket")?;
    let clock = Instant::now();
    let subscription = SnapshotSubscription::start(socket_path, Duration::from_millis(50));
    let mut presentation = PresentationState::disconnected();

    loop {
        match subscription.recv_timeout(Duration::from_secs(10))? {
            SnapshotEvent::ConnectionChanged(connection) => {
                if connection == ConnectionState::Disconnected {
                    presentation.disconnect(clock.elapsed());
                }
                report_connection(connection, &presentation, clock.elapsed())?;
            }
            SnapshotEvent::Snapshot(snapshot) => {
                let revision = snapshot.revision;
                let availability = availability_name(snapshot.availability);
                presentation.update(
                    *snapshot,
                    PresentationTime::new(clock.elapsed(), SystemTime::now()),
                )?;
                println!(
                    "{{\"event\":\"snapshot\",\"availability\":\"{availability}\",\"revision\":{revision}}}"
                );
                io::stdout().flush()?;
            }
        }
    }
}

fn report_connection(
    connection: ConnectionState,
    presentation: &PresentationState,
    now: Duration,
) -> Result<(), Box<dyn Error>> {
    let connection = match connection {
        ConnectionState::Connected => "connected",
        ConnectionState::Disconnected => "disconnected",
    };
    let presentation = match presentation.presentation_at(now)? {
        Presentation::NowPlaying(_) => "nowPlaying",
        Presentation::FullField(full_field) => match full_field.status.symbol {
            PresentationStatusSymbol::Disconnected => "disconnected",
            _ => "unavailable",
        },
    };
    println!(
        "{{\"event\":\"connection\",\"connection\":\"{connection}\",\"presentation\":\"{presentation}\"}}"
    );
    io::stdout().flush()?;
    Ok(())
}

fn availability_name(availability: Availability) -> &'static str {
    match availability {
        Availability::PairingRequired => "pairingRequired",
        Availability::Disconnected => "disconnected",
        Availability::OutputUnavailable => "outputUnavailable",
        Availability::Available => "available",
    }
}
