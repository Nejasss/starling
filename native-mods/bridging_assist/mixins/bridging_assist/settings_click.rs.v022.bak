fn bridging_assist_settings_click(
    game: &mut crate::game::Game<'_>,
    mod_id: &str,
    button: usize,
) {
    use crate::game::bridging_assist as bridging;
    if mod_id != bridging::ID || button >= 5 {
        return;
    }
    let mut settings = bridging::Settings::read(|key| game.native_setting(bridging::ID, key));
    settings.click(button);
    for (key, value) in settings.stored() {
        game.native_set_setting(bridging::ID, key, &value);
    }
}
