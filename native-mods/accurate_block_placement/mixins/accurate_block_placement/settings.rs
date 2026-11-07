// Accurate Block Placement's settings page.
fn abp_page(game: &mut crate::game::Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    use crate::game::accurate_block_placement as abp;
    if mod_id != abp::ID {
        return;
    }
    let game = &*game;
    let settings = abp::settings(|name| game.native_setting(abp::ID, name));
    buttons.extend(settings.labels());
}
