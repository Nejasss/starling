// Persist the selected Liteminer client option.
fn veinminer_settings_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    if mod_id == "veinminer" {
        game.veinminer_settings_click(button);
    }
}
