use std::path::Path;
use std::sync::{Arc, Barrier};

use roonscape_renderer::register_packaged_fallback_fonts;

#[test]
fn registers_packaged_fonts_safely_from_concurrent_callers() {
    const CALLER_COUNT: usize = 8;
    const REGISTRATION_ROUNDS: usize = 20;

    let start = Arc::new(Barrier::new(CALLER_COUNT));
    let callers = (0..CALLER_COUNT)
        .map(|_| {
            let start = Arc::clone(&start);
            std::thread::spawn(move || {
                let renderer_root = Path::new(env!("CARGO_MANIFEST_DIR"));
                start.wait();

                for _ in 0..REGISTRATION_ROUNDS {
                    register_packaged_fallback_fonts(renderer_root)
                        .expect("packaged fallback fonts should register concurrently");
                }
            })
        })
        .collect::<Vec<_>>();

    for caller in callers {
        caller
            .join()
            .expect("concurrent font registration should not crash");
    }
}

#[test]
fn rejects_a_different_renderer_root_after_registration() {
    let renderer_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    register_packaged_fallback_fonts(renderer_root)
        .expect("packaged fallback fonts should register from the renderer root");
    let different_root = tempfile::tempdir().expect("temporary renderer root should be available");

    assert!(register_packaged_fallback_fonts(different_root.path()).is_err());
}
