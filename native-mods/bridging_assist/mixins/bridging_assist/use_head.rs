// On a MISS only, find a supported cell along the camera-to-player bridging
// ray and temporarily pass that hit through Starling's ordinary use pipeline.
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
        || game.aim().is_none()
    {
        return;
    }

    let settings = bridging::Settings::read(|key| game.native_setting(bridging::ID, key));
    if !settings.enabled
        || (settings.crouch_only
            && !game.keys.contains(&winit::keyboard::KeyCode::ShiftLeft)
            && !game.keys.contains(&winit::keyboard::KeyCode::ShiftRight))
    {
        return;
    }

    let held = game.with_hand(hand, |current| current.held().cloned());
    let Some(held) = held else {
        return;
    };
    let Some(info) = game.items().get(&held.item) else {
        return;
    };
    if info.block.is_none() {
        return;
    }
    if settings.skip_torches
        && matches!(
            &*held.item,
            "minecraft:torch" | "minecraft:soul_torch" | "minecraft:redstone_torch"
        )
    {
        return;
    }

    let Some((origin, direction)) = game.aim() else {
        return;
    };
    let Some(player_eye) = game.eye_position() else {
        return;
    };
    let reach = game.block_reach;
    if !reach.is_finite() || reach <= 0.0 {
        return;
    }

    // Bridging Mod measures the reach endpoint from the player's own
    // perspective, then traverses from the camera perspective for that
    // distance. These directions differ in detached/third-person views.
    let player_direction = crate::game::view_vector(game.yaw, game.pitch);
    let Some(candidates) = bridging::candidate_path(
        origin,
        direction,
        player_eye,
        player_direction,
        reach,
        settings.min_distance_percent,
    ) else {
        return;
    };

    let Some(player) = game.player.as_ref() else {
        return;
    };
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

    // Prefer a support face most closely aligned with the player's view.
    // Minecraft Direction.values() order, reversed to the face returned by
    // Bridging Mod's getValidAssistSides(): UP, DOWN, SOUTH, NORTH, EAST, WEST.
    let faces = [
        [0, 1, 0],
        [0, -1, 0],
        [0, 0, 1],
        [0, 0, -1],
        [1, 0, 0],
        [-1, 0, 0],
    ];
    let dot = |face: [i32; 3]| {
        direction[0] * f64::from(face[0])
            + direction[1] * f64::from(face[1])
            + direction[2] * f64::from(face[2])
    };

    for pos in candidates {
        // Do not propose a block inside the player's own bounding box.
        let cell_min = [f64::from(pos[0]), f64::from(pos[1]), f64::from(pos[2])];
        let cell_max = [cell_min[0] + 1.0, cell_min[1] + 1.0, cell_min[2] + 1.0];
        let intersects_player = (0..3).all(|axis| {
            cell_min[axis] < player_max[axis] && cell_max[axis] > player_min[axis]
        });
        if intersects_player {
            continue;
        }

        // Keep Adventure placement restrictions: the server remains
        // authoritative, and the client only proposes targets its item may use.
        if !game.may_build() && !game.with_hand(hand, |current| current.can_place_on_in_adventure(pos)) {
            continue;
        }

        let state = game.world.block(pos[0], pos[1], pos[2]);
        let facts = game.res.facts();
        let replaceable = facts.get(state).replaceable;
        if !facts.is_air(state) && !(settings.replace_non_solid && replaceable) {
            continue;
        }

        let mut selected_face = None;
        for face in faces {
            if dot(face) > -0.1 {
                continue;
            }
            // The face points back toward the support block, just as
            // getValidAssistSides() returns Direction.getOpposite().
            let support = [pos[0] + face[0], pos[1] + face[1], pos[2] + face[2]];
            let support_state = game.world.block(support[0], support[1], support[2]);
            let support_facts = facts.get(support_state);
            let has_support = !facts.is_air(support_state)
                && !support_facts.replaceable
                && (support_facts.fluid == horizon_client::blocks::Fluid::None
                    || support_facts.waterlogged);
            if has_support {
                selected_face = Some(face);
                break;
            }
        }
        let Some(face) = selected_face else {
            continue;
        };

        // Match Bridging Mod's Bridge.getDefaultPlaceAssistTarget:
        // target the empty candidate cell itself, use the opposite of the
        // support-side direction as the clicked face, and click its center.
        // Starling's ordinary BlockItem placement pipeline then resolves the
        // replaceable candidate as the actual placement position.
        let placement_face = bridging::placement_hit_face(face);
        let hit = crate::game::Hit {
            pos,
            face: placement_face,
            point: [
                cell_min[0] + 0.5,
                cell_min[1] + 0.5,
                cell_min[2] + 0.5,
            ],
        };
        // InteractionInput captures a per-tick hit before dispatching use.
        // Preserve its cached MISS (Some(None)) across this synthetic click.
        let restore_cached_miss = game.tick_hit.is_some();
        game.tick_hit = Some(Some(hit));
        bridging::arm_override(restore_cached_miss);
        return;
    }
}
