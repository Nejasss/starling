// Show Liteminer's selected-block count and current shape in the top-right HUD corner.
fn veinminer_hud(
    game: &mut crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    corners: &mut horizon_client::extras_hud::Corners,
) {
    game.veinminer_hud(batch, corners);
}
