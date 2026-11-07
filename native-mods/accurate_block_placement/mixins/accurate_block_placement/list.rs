// Accurate Block Placement's row in Options > Mods, with its key.
fn abp_listed(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    use crate::game::accurate_block_placement as abp;
    mods.push(crate::game::native_hooks::NativeMod {
        id: abp::ID.into(),
        title: format!("Accurate Block Placement {}", abp::VERSION),
        description: abp::DESCRIPTION.into(),
        keys: vec![[
            abp::KEY.into(),
            "Toggle accurate placement".into(),
            abp::DEFAULT_KEY.into(),
        ]],
    });
}
