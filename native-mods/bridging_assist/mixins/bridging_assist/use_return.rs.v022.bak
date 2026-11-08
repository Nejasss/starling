// Restore the regular raycast state after Starling has consumed the
// synthetic hit. Preserve the original result and all early-return behavior.
fn bridging_assist_use_return(
    game: &mut crate::game::Game<'_>,
    value: bool,
) -> bool {
    if let Some(restore_cached_miss) =
        crate::game::bridging_assist::take_override()
    {
        game.tick_hit = if restore_cached_miss {
            Some(None)
        } else {
            None
        };
    }
    value
}
