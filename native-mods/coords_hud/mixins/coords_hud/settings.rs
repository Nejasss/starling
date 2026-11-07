// The mod's settings page: one button, the box shown or hidden.
fn official_coords_hud_page(game: &mut crate::game::Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    use crate::game::official_coords_hud as m;
    if mod_id != m::ID {
        return;
    }
    buttons.push(m::shown_label(m::shown(|| game.native_setting(m::ID, "shown"))));
}
