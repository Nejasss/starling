// Replace the vanilla target outline only when VeinMiner is drawing that selected block.
fn veinminer_block_highlight(
    game: &crate::game::Game<'_>,
    pos: [i32; 3],
    _state: horizon_world::registry::BlockStateId,
    _partial: f32,
    canceled: &mut bool,
) {
    game.veinminer_block_highlight(pos, canceled);
}
