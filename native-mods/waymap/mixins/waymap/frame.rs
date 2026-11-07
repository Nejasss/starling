// Once a frame (`native_frame`): load the waypoints, forget the last frame's
// minimap geometry.
fn waymap_frame(game: &mut crate::game::Game<'_>) {
    crate::game::waymap::frame(game);
}
