// Keys pressed: a waypoint here, the list panel.
fn waymap_key(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    crate::game::waymap::key(game, key, pressed);
}
