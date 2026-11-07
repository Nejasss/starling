// Waymap v0.2.0 - Mouse scroll injection for waypoint list navigation.

fn waymap_screen_mouse_scrolled_pre(
    game: &crate::game::Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    scroll: [f64; 2],
    modifiers: u32,
    canceled: &mut bool,
) -> std::ops::ControlFlow<()> {
    if !game.native_on("waymap") || screen.id != "mod" {
        return std::ops::ControlFlow::Continue(());
    }

    crate::game::waymap::screen_mouse_scrolled_pre(game, screen, scroll, modifiers, canceled);
    std::ops::ControlFlow::Continue(())
}