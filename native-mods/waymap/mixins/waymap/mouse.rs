// Waymap v0.2.0 - Mouse button injection for waypoint list interactions.

fn waymap_screen_mouse_button_pressed_pre(
    game: &crate::game::Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    button: i32,
    modifiers: u32,
    canceled: &mut bool,
) -> std::ops::ControlFlow<()> {
    if !game.native_on("waymap") || screen.id != "mod" {
        return std::ops::ControlFlow::Continue(());
    }

    crate::game::waymap::screen_mouse_button_pressed_pre(game, screen, button, modifiers, canceled);
    std::ops::ControlFlow::Continue(())
}