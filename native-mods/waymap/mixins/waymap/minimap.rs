// The minimap image, with the waypoints drawn over it. Horizon's own body is
// skipped (its rotation closure rules out a site hook or an overwrite); the
// library module draws the same map and adds a dot per waypoint.
fn waymap_minimap(
    batch: &mut crate::hud::HudBatch,
    atlas: &crate::hud::HudAtlas,
    corners: &mut crate::extras_hud::Corners,
    look: &crate::extras_hud::MinimapLook,
) -> std::ops::ControlFlow<()> {
    crate::extras_hud::waymap_minimap::draw(batch, atlas, corners, look);
    std::ops::ControlFlow::Break(())
}
