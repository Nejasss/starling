fn bridging_assist_settings(
    game: &mut crate::game::Game<'_>,
    mod_id: &str,
    buttons: &mut Vec<String>,
) {
    use crate::game::bridging_assist as bridging;
    if mod_id != bridging::ID {
        return;
    }
    let settings = bridging::Settings::read(|key| game.native_setting(bridging::ID, key));
    buttons.extend(settings.labels());
}
