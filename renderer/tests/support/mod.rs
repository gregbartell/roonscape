use std::fs;
use std::path::Path;

pub fn fixture(name: &str) -> String {
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../fixtures")
        .join(name);
    fs::read_to_string(fixture_path).expect("shared fixture should be readable")
}
