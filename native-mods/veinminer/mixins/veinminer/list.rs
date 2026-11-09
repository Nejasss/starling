// Register VeinMiner in Options > Mods. Its existing command/mining
// injections remain unchanged; this only restores its visible menu entry.
fn veinminer_list(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    mods.push(crate::game::native_hooks::NativeMod {
        id: "veinminer".into(),
        title: "VeinMiner 0.4.0".into(),
        description: "Shapeless veins, tunnel, 3x3 plane and stair mining shapes.".into(),
        keys: vec![],
    });
}
