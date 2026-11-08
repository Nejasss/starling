// Register VeinMiner in Options > Mods. Its existing command/mining
// injections remain unchanged; this only restores its visible menu entry.
fn veinminer_list(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    mods.push(crate::game::native_hooks::NativeMod {
        id: "veinminer".into(),
        title: "VeinMiner 0.3.2".into(),
        description: "Mine connected blocks of the same vein.".into(),
        keys: vec![],
    });
}
