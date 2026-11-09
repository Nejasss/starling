// The Liteminer key: hold by default, or toggle from Options > Mods.
fn veinminer_key(game: &mut crate::game::Game<'_>, key: &str, pressed: bool) {
    if game.native_on("veinminer") {
        game.veinminer_key(key, pressed);
    }
}
