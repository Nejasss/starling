// Feed the current Liteminer selection to Horizon's bounded world-rim renderer.
fn veinminer_visuals(
    game: &crate::game::Game<'_>,
    rims: &mut Vec<horizon_client::mod_api::NativeBlockRim>,
    _lights: &mut Vec<horizon_client::mod_api::NativeColoredLight>,
) {
    game.veinminer_visuals(rims);
}
