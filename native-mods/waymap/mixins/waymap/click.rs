// A click on the mod's settings page.
fn waymap_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    crate::game::waymap::click(game, mod_id, button);
}
