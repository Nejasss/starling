// While VeinMiner is active, scroll through the registered Liteminer shapes.
fn veinminer_wheel(game: &mut crate::game::Game<'_>, steps: f32, claimed: &mut bool) {
    if game.native_on("veinminer") {
        game.veinminer_wheel(steps, claimed);
    }
}
