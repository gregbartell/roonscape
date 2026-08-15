#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RendererKey {
    Escape,
    Other,
}

pub fn should_close_renderer(key: RendererKey) -> bool {
    key == RendererKey::Escape
}
