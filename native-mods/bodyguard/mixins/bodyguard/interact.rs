// native_interact_entity HEAD: right click on a bodyguard.
fn bodyguard_interact(game: &mut crate::game::Game<'_>, entity: i32, stack: Option<&horizon_client::mod_api::ItemStack>, claimed: &mut bool) {
    crate::game::bodyguard::on_interact(game, entity, stack, claimed);
}
