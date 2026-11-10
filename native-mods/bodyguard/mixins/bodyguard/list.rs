// native_mods HEAD: the bodyguard's row in Options > Mods.
fn bodyguard_listed(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    use crate::game::bodyguard as bg;
    mods.push(crate::game::native_hooks::NativeMod {
        id: bg::ID.into(),
        title: format!("Bodyguard {}", bg::VERSION),
        description: bg::DESCRIPTION.into(),
        keys: Vec::new(),
    });
}
