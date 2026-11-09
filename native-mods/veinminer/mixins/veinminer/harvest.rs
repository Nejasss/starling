fn apply_liteminer_harvest_time_modifier(
    value: f32,
    game: &crate::game::Game<'_>,
) -> f32 {
    value * game.veinminer_harvest_speed_modifier()
}
