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
    let marker = if face == [0, 1, 0] {
        "v"
    } else if face == [0, -1, 0] {
        "^"
    } else {
        "<>"
    };
    let x = batch.width as f32 * 0.5 - 4.0;
    let y = batch.height as f32 * 0.5 + 9.0;
    game.native_text(batch, marker, [x, y], 0xFFFF_FFFF, true);
}