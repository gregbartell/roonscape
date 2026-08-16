use roonscape_renderer::{NavigationIntent, RendererAction, RendererKey, RendererKeyboard};

#[test]
fn fixture_mode_arrows_map_to_semantic_navigation_intents() {
    let mut keyboard = RendererKeyboard::new(true);

    assert_eq!(
        keyboard.press(RendererKey::Left),
        RendererAction::Navigate(NavigationIntent::Previous)
    );
    assert_eq!(
        keyboard.press(RendererKey::Right),
        RendererAction::Navigate(NavigationIntent::Next)
    );
}

#[test]
fn held_arrows_advance_once_until_released() {
    let mut keyboard = RendererKeyboard::new(true);

    assert_eq!(
        keyboard.press(RendererKey::Right),
        RendererAction::Navigate(NavigationIntent::Next)
    );
    assert_eq!(keyboard.press(RendererKey::Right), RendererAction::None);
    keyboard.release(RendererKey::Right);
    assert_eq!(
        keyboard.press(RendererKey::Right),
        RendererAction::Navigate(NavigationIntent::Next)
    );
}

#[test]
fn distinct_rapid_arrow_presses_remain_responsive() {
    let mut keyboard = RendererKeyboard::new(true);

    for _ in 0..3 {
        assert_eq!(
            keyboard.press(RendererKey::Left),
            RendererAction::Navigate(NavigationIntent::Previous)
        );
        keyboard.release(RendererKey::Left);
    }
}

#[test]
fn arrows_are_inert_without_fixture_mode_navigation() {
    let mut keyboard = RendererKeyboard::new(false);

    assert_eq!(keyboard.press(RendererKey::Left), RendererAction::None);
    assert_eq!(keyboard.press(RendererKey::Right), RendererAction::None);
}

#[test]
fn fixture_mode_arrows_require_window_focus() {
    let mut keyboard = RendererKeyboard::new(true);
    keyboard.set_focused(false);

    assert_eq!(keyboard.press(RendererKey::Left), RendererAction::None);
    assert_eq!(keyboard.press(RendererKey::Right), RendererAction::None);

    keyboard.set_focused(true);
    assert_eq!(
        keyboard.press(RendererKey::Right),
        RendererAction::Navigate(NavigationIntent::Next)
    );
}

#[test]
fn losing_focus_clears_held_arrow_suppression() {
    let mut keyboard = RendererKeyboard::new(true);
    assert_eq!(
        keyboard.press(RendererKey::Left),
        RendererAction::Navigate(NavigationIntent::Previous)
    );

    keyboard.set_focused(false);
    keyboard.set_focused(true);

    assert_eq!(
        keyboard.press(RendererKey::Left),
        RendererAction::Navigate(NavigationIntent::Previous)
    );
}

#[test]
fn escape_action_remains_available_in_every_mode() {
    for navigation_enabled in [false, true] {
        let mut keyboard = RendererKeyboard::new(navigation_enabled);
        assert_eq!(keyboard.press(RendererKey::Escape), RendererAction::Close);
    }
}
