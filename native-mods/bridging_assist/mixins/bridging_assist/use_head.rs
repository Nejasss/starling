// On a MISS only, use the target cached for this tick by native_tick.
// The exact same target drives placement, the center indicator and the outline.
fn bridging_assist_use_head(
    game: &mut crate::game::Game<'_>,
    hand: horizon_client::first_person::Hand,
) {
    use crate::game::bridging_assist as bridging;

    if !game.native_on(bridging::ID)
        || game.screen != crate::game::Screen::Playing
        || game.sleeping.is_some()
        || game.using.is_some()
        || game.forced_hit.is_some()
        || game.entity_targeted()
        || game.pick().is_some()
    {
        return;
    }

    let settings = bridging::Settings::read(|key| game.native_setting(bridging::ID, key));
    let crouching = game.player.as_ref().is_some_and(|player| player.local_crouching)
        || game.keys.contains(&winit::keyboard::KeyCode::ShiftLeft)
        || game.keys.contains(&winit::keyboard::KeyCode::ShiftRight);
    if !settings.enabled || (settings.crouch_only && !crouching) {
        return;
    }

    let held = game.with_hand(hand, |current| current.held().cloned());
    let Some(held) = held else { return; };
    let Some(info) = game.items().get(&held.item) else { return; };
    if info.block.is_none() {
        return;
    }
    if settings.skip_torches
        && matches!(
            &*held.item,
            "minecraft:torch" | "minecraft:soul_torch" | "minecraft:redstone_torch"
                | "minecraft:wall_torch" | "minecraft:soul_wall_torch" | "minecraft:redstone_wall_torch"
        )
    {
        return;
    }

    let Some((pos, face)) = bridging::visual_target() else { return; };
    if !settings.effective_axis(crouching).allows(face) {
        return;
    }

    // Revalidate the target because the world/player may have changed since
    // the preceding tick. This prevents stale previews from placing blocks.
    let Some(player) = game.player.as_ref() else { return; };
    let half_width = f64::from(player.dimensions.width) * 0.5;
    let player_min = [
        player.position.x - half_width,
        player.position.y,
        player.position.z - half_width,
    ];
    let player_max = [
        player.position.x + half_width,
        player.position.y + f64::from(player.dimensions.height),
        player.position.z + half_width,
    ];
    let cell_min = [f64::from(pos[0]), f64::from(pos[1]), f64::from(pos[2])];
    let cell_max = [cell_min[0] + 1.0, cell_min[1] + 1.0, cell_min[2] + 1.0];
    if (0..3).all(|axis| cell_min[axis] < player_max[axis] && cell_max[axis] > player_min[axis]) {
        return;
    }
    if !game.may_build() && !game.with_hand(hand, |current| current.can_place_on_in_adventure(pos)) {
        return;
    }

    let facts = game.res.facts();
    let state = game.world.block(pos[0], pos[1], pos[2]);
    let target_facts = facts.get(state);
    if !facts.is_air(state) && !(settings.replace_non_solid && target_facts.replaceable) {
        return;
    }
    let support = [pos[0] + face[0], pos[1] + face[1], pos[2] + face[2]];
    let support_state = game.world.block(support[0], support[1], support[2]);
    let support_facts = facts.get(support_state);
    if facts.is_air(support_state)
        || support_facts.replaceable
        || !(support_facts.fluid == horizon_client::blocks::Fluid::None || support_facts.waterlogged)
    {
        return;
    }

    // Bridging Mod passes the opposite of the support-side direction into
    // getDefaultPlaceAssistTarget; keep that original face convention.
    let placement_face = bridging::placement_hit_face(face);
    let mut hit = crate::game::Hit {
        pos,
        face: placement_face,
        point: [cell_min[0] + 0.5, cell_min[1] + 0.5, cell_min[2] + 0.5],
    };

    let held_id = &*held.item;
    let held_slab = held_id.ends_with("_slab");
    let held_slab_or_trapdoor = held_slab || held_id.ends_with("_trapdoor");
    if settings.slab_assist && held_slab_or_trapdoor {
        if placement_face[1] == 0 {
            let yf = bridging::last_grounded_y_fraction();
            let lower = yf > (3.0 / 16.0 - 0.01) && yf < (0.5 + 0.01);
            hit.face = if lower { [0, 1, 0] } else { [0, -1, 0] };
            hit.point[1] = cell_min[1] + if lower { 0.1 } else { 0.9 };
        } else if held_slab && facts.get(support_state).name.ends_with("_slab") {
            let slab_type = game.native_block_property(support, "type");
            let compatible = match slab_type.as_deref() {
                Some("top") => placement_face == [0, -1, 0],
                Some("bottom") => placement_face == [0, 1, 0],
                _ => false,
            };
            if compatible {
                hit.pos = support;
            }
        }
    }

    // Preserve the previously cached MISS after the synthetic hit returns.
    let restore_cached_miss = game.tick_hit.is_some();
    game.tick_hit = Some(Some(hit));
    bridging::arm_override(restore_cached_miss);
}
