// Waymap v0.2.0 - Key press injection for waypoint list and editor navigation.

fn waymap_screen_key_pressed_pre(
    game: &crate::game::Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    key: &str,
    repeat: bool,
    modifiers: u32,
    canceled: &mut bool,
) -> std::ops::ControlFlow<()> {
    if !game.native_on("waymap") || screen.id != "mod" {
        return std::ops::ControlFlow::Continue(());
    }

    crate::game::waymap::screen_key_pressed_pre(game, screen, key, repeat, modifiers, canceled);
    std::ops::ControlFlow::Continue(())
}