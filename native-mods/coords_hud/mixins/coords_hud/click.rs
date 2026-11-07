// A click on the mod's settings page: show or hide the box, and keep it.
fn official_coords_hud_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    use crate::game::official_coords_hud as m;
    if mod_id != m::ID || button != 0 {
        return;
    }
    let shown = !m::shown(|| game.native_setting(m::ID, "shown"));
    m::set_shown(shown);
    game.native_set_setting(m::ID, "shown", if shown { "true" } else { "false" });
}
