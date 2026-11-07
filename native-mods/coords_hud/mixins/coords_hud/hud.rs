// The box in the top-left corner, drawn as the WebAssembly mods' panels were;
// the debug screen takes that corner.
fn coords_hud(
    game: &mut crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    corners: &mut horizon_client::extras_hud::Corners,
) {
    use crate::game::official_coords_hud as m;
    use horizon_client::extras::Corner;
    // Turned off in Options > Mods: nothing until the next build leaves it out.
    if !game.native_on(m::ID) || !game.native_corner_shown(Corner::TopLeft) {
        return;
    }
    if !m::shown(|| game.native_setting(m::ID, "shown")) {
        return;
    }
    let Some(p) = game.native_player() else {
        return;
    };
    let lines = m::lines(p.x, p.y, p.z, p.yaw);
    let w = lines.iter().map(|l| game.native_text_width(l)).fold(0.0, f32::max) + 4.0;
    let h = 10.0 * lines.len() as f32 + 2.0;
    let (x, y) = corners.place(Corner::TopLeft, w, h);
    game.native_fill(batch, [x, y, w, h], m::BACKGROUND);
    for (i, line) in lines.iter().enumerate() {
        game.native_text(batch, line, [x + 2.0, y + 2.0 + 10.0 * i as f32], m::INK, true);
    }
}
