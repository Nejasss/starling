// Bridging Mod's default toggle key (comma), configurable in Options > Mods.
fn bridging_assist_key(
    game: &mut crate::game::Game<'_>,
    key: &str,
    pressed: bool,
) {
    use crate::game::bridging_assist as bridging;

    if !pressed || !game.native_on(bridging::ID) || !game.native_playing() {
        return;
    }
    let bound = game.native_mod_key(bridging::ID, bridging::KEY, bridging::DEFAULT_KEY);
    if key != bound {
        return;
    }

    let mut settings = bridging::Settings::read(|name| game.native_setting(bridging::ID, name));
    settings.enabled = !settings.enabled;
    for (name, value) in settings.stored() {
        game.native_set_setting(bridging::ID, name, &value);
    }
    game.native_chat(if settings.enabled { bridging::ON } else { bridging::OFF });
}
