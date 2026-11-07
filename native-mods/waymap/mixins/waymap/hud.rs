// The HUD: markers in the world, dots on the minimap, the list panel.
fn waymap_hud(
    game: &mut crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    corners: &mut horizon_client::extras_hud::Corners,
) {
    crate::game::waymap::hud(game, batch, corners);
}
