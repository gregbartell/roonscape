use std::fs;
use std::path::Path;
use std::time::Duration;

use roonscape_renderer::{
    InactivityConfiguration, inactivity_configuration_from_display_configuration,
    load_inactivity_configuration,
};

#[test]
fn default_inactivity_configuration_uses_inactivity_defaults() {
    let configuration = inactivity_configuration_from_display_configuration(&fixture(
        "display-configuration-default-inactivity.json",
    ))
    .expect("default inactivity configuration should be valid");

    assert_eq!(configuration, InactivityConfiguration::default());
}

#[test]
fn reads_host_inactivity_calibration_from_display_configuration() {
    let configuration = inactivity_configuration_from_display_configuration(&fixture(
        "display-configuration-inactivity.json",
    ))
    .expect("inactivity calibration should be valid");

    assert_eq!(
        configuration,
        InactivityConfiguration::new(Duration::from_secs(240), 0.3, Duration::from_secs(45))
            .expect("test calibration should be valid")
    );
}

#[test]
fn rejects_invalid_host_inactivity_calibration() {
    for contents in [
        r#"{"trackedOutputId":"output-speaker-system","trackedOutputName":"Speaker System","inactivity":{"gracePeriodSeconds":0,"dimmedOpacity":0.3,"repositionCadenceSeconds":45}}"#,
        r#"{"trackedOutputId":"output-speaker-system","trackedOutputName":"Speaker System","inactivity":{"gracePeriodSeconds":240,"dimmedOpacity":0,"repositionCadenceSeconds":45}}"#,
        r#"{"trackedOutputId":"output-speaker-system","trackedOutputName":"Speaker System","inactivity":{"gracePeriodSeconds":240,"dimmedOpacity":1,"repositionCadenceSeconds":45}}"#,
        r#"{"trackedOutputId":"output-speaker-system","trackedOutputName":"Speaker System","inactivity":{"gracePeriodSeconds":240,"dimmedOpacity":1.1,"repositionCadenceSeconds":45}}"#,
        r#"{"trackedOutputId":"output-speaker-system","trackedOutputName":"Speaker System","inactivity":{"gracePeriodSeconds":240,"dimmedOpacity":0.3,"repositionCadenceSeconds":0}}"#,
    ] {
        assert!(inactivity_configuration_from_display_configuration(contents).is_err());
    }
}

#[test]
fn rejects_display_configuration_outside_the_shared_contract() {
    let contents = r#"{"inactivity":{"gracePeriodSeconds":240,"dimmedOpacity":0.3,"repositionCadenceSeconds":45}}"#;
    assert!(inactivity_configuration_from_display_configuration(contents).is_err());
}

#[test]
fn a_missing_display_configuration_file_uses_inactivity_defaults() {
    let task_directory = tempfile::tempdir().expect("temporary directory should be available");
    let configuration = load_inactivity_configuration(&task_directory.path().join("display.json"))
        .expect("a fresh host should use inactivity defaults");

    assert_eq!(configuration, InactivityConfiguration::default());
}

#[test]
fn loads_inactivity_calibration_from_the_host_file() {
    let task_directory = tempfile::tempdir().expect("temporary directory should be available");
    let configuration_file = task_directory.path().join("display.json");
    fs::write(
        &configuration_file,
        r#"{"trackedOutputId":"output-speaker-system","trackedOutputName":"Speaker System","inactivity":{"gracePeriodSeconds":240,"dimmedOpacity":0.3,"repositionCadenceSeconds":45}}"#,
    )
    .expect("test Display Configuration should be writable");

    let configuration = load_inactivity_configuration(&configuration_file)
        .expect("host inactivity calibration should load");

    assert_eq!(configuration.grace_period(), Duration::from_secs(240));
    assert_eq!(configuration.dimmed_opacity(), 0.3);
    assert_eq!(configuration.reposition_cadence(), Duration::from_secs(45));
}

fn fixture(name: &str) -> String {
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../shared/fixtures")
        .join(name);
    fs::read_to_string(fixture_path)
        .expect("shared Display Configuration fixture should be readable")
}
