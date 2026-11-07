// Waymap's minimap view, kept for the waypoint dots drawn over its image.
// `minimap_shown` is private to this module, so the view is read here; the
// drawing happens in the client library (`extras_hud::waymap_minimap`).
fn waymap_minimap_view(game: &mut crate::game::Game<'_>) {
    let map = game.minimap_shown();
    horizon_client::extras_hud::waymap_minimap::set_view(map);
}
