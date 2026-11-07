// Once a tick, before the tick's clicks (`Minecraft.handleKeybinds`): the
// fresh targets the frames saw get their blocks, through the game's own
// `startUseItem` (prediction, sound, the server's packet), one target at a
// time. Vanilla's repeat (a use every four ticks while held) stays off while
// a hold is under way on a block that doesn't react to the click; a door, a
// chest or a button still gets vanilla's click.
fn abp_tick(game: &mut crate::game::Game<'_>) {
    use crate::game::accurate_block_placement as abp;
    let Some(player) = game
        .player
        .as_ref()
        .map(|p| [p.position.x, p.position.y, p.position.z])
    else {
        return;
    };
    let held = game.held().cloned();
    if abp::holding()
        && let Some(hit) = game.pick()
        && !game.remote_block_reacts(&hit, held.as_ref()).0.consumes_action()
    {
        game.use_delay = game.use_delay.max(1);
    }
    for t in abp::due((game.yaw, game.pitch)) {
        if !abp::still_fresh(&t, player) {
            continue;
        }
        let hit = crate::game::Hit {
            pos: t.pos,
            face: t.face,
            point: t.point,
        };
        if game.remote_block_reacts(&hit, held.as_ref()).0.consumes_action() {
            continue;
        }
        let at = t.placement();
        let before = game.world.block(at[0], at[1], at[2]);
        // startUseItem on that target, as the tick's hit result.
        let kept = game.tick_hit.replace(Some(hit));
        game.remote_use();
        game.tick_hit = kept;
        if game.world.block(at[0], at[1], at[2]) != before {
            abp::placed(&t, at, player);
        }
    }
}
