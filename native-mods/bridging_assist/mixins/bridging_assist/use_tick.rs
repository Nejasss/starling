// Starling's held-use loop hard-codes four ticks after remote_use_item_on.
// The successful assisted-use RETURN hook arms a one-shot adjustment. This
// HEAD hook runs after Starling decrements use_delay and before held-use runs.
fn bridging_assist_use_tick(game: &mut crate::game::Game<'_>) {
    let game_key = game as *mut _ as usize;
    if let Some(interval_ticks) = crate::game::bridging_assist::take_assisted_delay(game_key) {
        game.use_delay = crate::game::bridging_assist::remaining_use_delay(
            game.use_delay,
            interval_ticks,
        );
    }
}
