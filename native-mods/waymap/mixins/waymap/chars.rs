// Waymap v0.2.0 - Character input injection for waypoint list and editor text entry.

fn waymap_screen_char_typed_pre(
    game: &crate::game::Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    c: char,
    modifiers: u32,
    canceled: &mut bool,
) -> std::ops::ControlFlow<()> {
    if !game.native_on("waymap") || screen.id != "mod" {
        return std::ops::ControlFlow::Continue(());
    }

    crate::game::waymap::screen_char_typed_pre(game, screen, c, modifiers, canceled);
    std::ops::ControlFlow::Continue(())
}