#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RendererKey {
    Escape,
    Left,
    Right,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NavigationIntent {
    Previous,
    Next,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RendererAction {
    Close,
    Navigate(NavigationIntent),
    None,
}

pub struct RendererKeyboard {
    navigation_enabled: bool,
    focused: bool,
    left_pressed: bool,
    right_pressed: bool,
}

impl RendererKeyboard {
    pub fn new(navigation_enabled: bool) -> Self {
        Self {
            navigation_enabled,
            focused: true,
            left_pressed: false,
            right_pressed: false,
        }
    }

    pub fn press(&mut self, key: RendererKey) -> RendererAction {
        match key {
            RendererKey::Escape => RendererAction::Close,
            RendererKey::Left if self.navigation_enabled && self.focused && !self.left_pressed => {
                self.left_pressed = true;
                RendererAction::Navigate(NavigationIntent::Previous)
            }
            RendererKey::Right
                if self.navigation_enabled && self.focused && !self.right_pressed =>
            {
                self.right_pressed = true;
                RendererAction::Navigate(NavigationIntent::Next)
            }
            RendererKey::Left | RendererKey::Right | RendererKey::Other => RendererAction::None,
        }
    }

    pub fn release(&mut self, key: RendererKey) {
        match key {
            RendererKey::Left => self.left_pressed = false,
            RendererKey::Right => self.right_pressed = false,
            RendererKey::Escape | RendererKey::Other => {}
        }
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
        if !focused {
            self.left_pressed = false;
            self.right_pressed = false;
        }
    }
}
