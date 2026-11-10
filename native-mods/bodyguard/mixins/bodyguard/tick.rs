// native_player_tick HEAD: the player's bodyguards follow them.
fn bodyguard_tick(game: &mut crate::game::Game<'_>) {
    crate::game::bodyguard::on_player_tick(game);
}
