// Once a frame: what the crosshair hits while the use key is held with a
// block in hand. Fresh targets are kept for the next tick (`tick.rs`).
fn abp_frame(game: &mut crate::game::Game<'_>) {
    use crate::game::accurate_block_placement as abp;
    let settings = {
        let game = &*game;
        abp::settings(|name| game.native_setting(abp::ID, name))
    };
    let Some(player) = game
        .player
        .as_ref()
        .map(|p| [p.position.x, p.position.y, p.position.z])
    else {
        abp::reset();
        return;
    };
    // Your own world always allows it; a server only with the setting on.
    let allowed = settings.enabled
        && game.native_on(abp::ID)
        && (game.integrated.is_some() || settings.servers);
    // The item a use would place: the main hand's, or the off hand's when the
    // main hand is empty (vanilla tries them in that order).
    let held = match game.held() {
        Some(stack) => Some(stack.clone()),
        None => game.offhand.clone(),
    };
    let block_item = held
        .as_ref()
        .and_then(|s| game.items().get(&s.item))
        .is_some_and(|i| i.block.is_some());
    let mut down = allowed
        && block_item
        && game.right
        && game.screen == crate::game::Screen::Playing
        && game.sleeping.is_none()
        && game.using.is_none();
    let mut target = None;
    if down && !game.entity_targeted() {
        if let Some(hit) = game.pick() {
            let state = game.world.block(hit.pos[0], hit.pos[1], hit.pos[2]);
            // Scaffolding on scaffolding climbs up the tower: vanilla's way.
            let scaffolding = held
                .as_ref()
                .is_some_and(|s| &*s.item == "minecraft:scaffolding")
                && &*game.res.facts().get(state).name == "minecraft:scaffolding";
            if scaffolding {
                down = false;
            } else {
                target = Some(abp::Sample {
                    pos: hit.pos,
                    face: hit.face,
                    point: hit.point,
                    replaceable: game.res.facts().get(state).replaceable,
                });
            }
        }
    }
    abp::frame(down, target, player, (game.yaw, game.pitch));
}
