// native_place_block HEAD: a carved pumpkin on a chest on an iron block builds a bodyguard.
fn bodyguard_place(game: &mut crate::game::Game<'_>, stack: &horizon_client::mod_api::ItemStack, pos: [i32; 3]) {
    crate::game::bodyguard::on_place(game, stack, pos);
}
