fn bridging_assist_list(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    use crate::game::bridging_assist as bridging;
    mods.push(crate::game::native_hooks::NativeMod {
        id: bridging::ID.into(),
        title: format!("Bridging Assist {}", bridging::VERSION),
        description: bridging::DESCRIPTION.into(),
        keys: vec![[bridging::KEY.into(), "Toggle Bridging Assist".into(), bridging::DEFAULT_KEY.into()]],
    });
}
