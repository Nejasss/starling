fn bridging_assist_settings_click(
    game: &mut crate::game::Game<'_>,
    mod_id: &str,
    button: usize,
) {
    use crate::game::bridging_assist as bridging;
    if mod_id != bridging::ID || button >= 14 {
        return;
    }
    let mut settings = bridging::cached_settings(game);
    settings.click(button);
    bridging::cache_settings(settings);
    for (key, value) in settings.stored() {
        game.native_set_setting(bridging::ID, key, &value);
    }
}
