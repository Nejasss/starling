// A click on Accurate Block Placement's settings page: change the setting and keep it.
fn abp_settings_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    use crate::game::accurate_block_placement as abp;
    if mod_id != abp::ID {
        return;
    }
    let mut settings = {
        let game = &*game;
        abp::settings(|name| game.native_setting(abp::ID, name))
    };
    settings.click(button);
    abp::set_settings(settings);
    abp::reset();
    for (name, value) in settings.stored() {
        game.native_set_setting(abp::ID, name, &value);
    }
}
