// The mod's row in Options > Mods.
fn waymap_listed(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    crate::game::waymap::listed(mods);
}
