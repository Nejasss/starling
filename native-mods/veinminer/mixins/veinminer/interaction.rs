// Runs only after the clicked block has declined its own use.
fn veinminer_block_interaction(
    game: &mut crate::game::Game<'_>,
    stack: &horizon_client::items::ItemStack,
    pos: [i32; 3],
    face: [i32; 3],
    claimed: &mut bool,
) {
    game.veinminer_use_item_on(stack, pos, face, claimed);
}
