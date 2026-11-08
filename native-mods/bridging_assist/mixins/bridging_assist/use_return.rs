
// Restore the ordinary raycast state after Starling consumes the synthetic
// hit. Apply Bridging Mod's configurable delay after a successful assisted use.
fn bridging_assist_use_return(game: &mut crate::game::Game<'_>, value: bool) -> bool {
    if let Some(restore_cached_miss) = crate::game::bridging_assist::take_override() {
        game.tick_hit = if restore_cached_miss { Some(None) } else { None };
        if value {
            let settings = crate::game::bridging_assist::Settings::read(|key| game.native_setting(crate::game::bridging_assist::ID, key));
            crate::game::bridging_assist::arm_assisted_delay(
                game as *mut _ as usize,
                settings.post_delay_ticks,
            );
        }
    }
    value
}
