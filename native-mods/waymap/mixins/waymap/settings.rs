// The mod's settings page.
fn waymap_page(game: &mut crate::game::Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    crate::game::waymap::settings(game, mod_id, buttons);
}
