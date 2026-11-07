// /where puts the position in chat; /coords hides or shows the box.
fn coords_hud_command(game: &mut crate::game::Game<'_>, command: &str, ran: &mut bool) {
    use crate::game::official_coords_hud as m;
    let name = command.split(' ').next().unwrap_or("");
    if !game.native_on(m::ID) || !matches!(name, "where" | "coords") {
        return;
    }
    *ran = true;
    if name == "coords" {
        let shown = !m::shown(|| game.native_setting(m::ID, "shown"));
        m::set_shown(shown);
        game.native_set_setting(m::ID, "shown", if shown { "true" } else { "false" });
        game.native_chat(if shown { "Coordinates panel shown" } else { "Coordinates panel hidden" });
        return;
    }
    match game.native_player() {
        Some(p) => game.native_chat(&m::where_line(p.x, p.y, p.z, p.dimension)),
        None => game.native_chat("Not in a world"),
    }
}
