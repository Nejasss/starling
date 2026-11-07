// A click on the settings page: turn the labels in the world or the death
// waypoints on or off.
fn waypoints_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    use crate::game::waypoints_mod as m;
    if mod_id != m::ID || button > 1 {
        return;
    }
    let name = if button == 0 { "labels" } else { "deaths" };
    let stored = game.native_setting(m::ID, name);
    let on = m::with(|s| {
        let slot = if button == 0 { &mut s.labels } else { &mut s.deaths };
        let on = !slot.unwrap_or_else(|| m::read_on(stored));
        *slot = Some(on);
        on
    });
    game.native_set_setting(m::ID, name, if on { "true" } else { "false" });
}
