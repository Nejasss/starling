// Options > Mods: activation mode, current shape, and local preview switches.
fn veinminer_settings(
    game: &mut crate::game::Game<'_>,
    mod_id: &str,
    buttons: &mut Vec<String>,
) {
    if mod_id == "veinminer" {
        game.veinminer_settings(buttons);
    }
}
