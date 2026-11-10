// native_living_incoming_damage HEAD: a bodyguard's blow is worth its axe.
fn bodyguard_damage(causing: i32, amount: &mut f32) {
    crate::game::bodyguard::on_damage(causing, amount);
}
