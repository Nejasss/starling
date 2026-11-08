fn bridging_assist_crosshair(
    game: &mut crate::game::Game<'_>,
    layer: &str,
    batch: &mut horizon_client::hud::HudBatch,
) {
    use crate::game::bridging_assist as bridging;

    if layer != "minecraft:crosshair" || !game.native_on(bridging::ID) {
        return;
    }
    let settings = bridging::Settings::read(|key| game.native_setting(bridging::ID, key));
    if !settings.enabled || !settings.show_crosshair {
        return;
    }
    let Some((_, face)) = bridging::visual_target() else { return; };

    let center_x = batch.width as f32 * 0.5;
    let center_y = batch.height as f32 * 0.5;
    let (marker, y) = if face == [0, 1, 0] {
        ("v", center_y + 5.0)
    } else if face == [0, -1, 0] {
        ("^", center_y - 13.0)
    } else {
        // The gap in "< >" leaves the vanilla crosshair visible in the middle.
        ("< >", center_y - 4.0)
    };
    let x = center_x - game.native_text_width(marker) * 0.5;
    game.native_text(batch, marker, [x, y], 0xFFFF_FFFF, true);
}
