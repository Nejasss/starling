fn bridging_assist_settings(
    game: &mut crate::game::Game<'_>,
    mod_id: &str,
    buttons: &mut Vec<String>,
) {
    use crate::game::bridging_assist as bridging;
    if mod_id != bridging::ID {
        return;
    }
    let settings = bridging::reload_settings(game);
    buttons.extend(settings.labels());
}
