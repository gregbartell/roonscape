use roonscape_renderer::{RendererKey, should_close_renderer};

#[test]
fn escape_closes_the_renderer() {
    assert!(should_close_renderer(RendererKey::Escape));
}

#[test]
fn an_unrelated_key_does_not_close_the_renderer() {
    assert!(!should_close_renderer(RendererKey::Other));
}
