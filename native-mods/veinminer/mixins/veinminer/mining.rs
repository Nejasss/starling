fn mining(game: &mut crate::game::Game<'_>, pos: [i32; 3]) -> core::ops::ControlFlow<Result<()>> {
    if !game.native_on("veinminer") {
        core::ops::ControlFlow::Continue(())
    } else {
        core::ops::ControlFlow::Break(
            game.with_player_actor(|game| game.veinminer_break_block(pos)),
        )
    }
}
