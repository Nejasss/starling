// The toggle key (unbound by default): Accurate Block Placement on or off.
fn abp_key(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    use crate::game::accurate_block_placement as abp;
    if !pressed || !game.native_on(abp::ID) || !game.native_playing() {
        return;
    }
    let bound = game.native_mod_key(abp::ID, abp::KEY, abp::DEFAULT_KEY);
    if bound == abp::DEFAULT_KEY || key != bound {
        return;
    }
    let mut settings = {
        let game = &*game;
        abp::settings(|name| game.native_setting(abp::ID, name))
    };
    settings.enabled = !settings.enabled;
    abp::set_settings(settings);
    abp::reset();
    for (name, value) in settings.stored() {
        game.native_set_setting(abp::ID, name, &value);
    }
    game.native_chat(if settings.enabled { abp::ON } else { abp::OFF });
}
