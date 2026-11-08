fn bridging_assist_frame_visuals(
    game: &crate::game::Game<'_>,
    lines: &mut Vec<horizon_client::gizmos::Line>,
) {
    use crate::game::bridging_assist as bridging;

    bridging::set_visual_target(None);
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
    let axes = settings.effective_axis(crouching);

    let main_held = game.held().cloned();
    let main_is_placeable = main_held
        .as_ref()
        .and_then(|stack| game.items().get(&stack.item))
        .is_some_and(|info| info.block.is_some())
        && !main_held.as_ref().is_some_and(|stack| {
            settings.skip_torches
                && matches!(
                    &*stack.item,
                    "minecraft:torch" | "minecraft:soul_torch" | "minecraft:redstone_torch"
                        | "minecraft:wall_torch" | "minecraft:soul_wall_torch" | "minecraft:redstone_wall_torch"
                )
        });
    let held = if main_is_placeable { main_held.clone() } else { game.offhand.clone() };
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

    let Some((aim_origin, aim_direction)) = game.aim() else { return; };
    let Some(eye) = game.eye_position() else { return; };
    let Some(player) = game.player.as_ref() else { return; };
    bridging::remember_grounded_y_fraction(player.position.y, player.ground.on_ground);
    let player_direction = crate::game::view_vector(game.yaw, game.pitch);
    let (camera_origin, camera_direction) = game.native_camera().map_or(
        (aim_origin, aim_direction),
        |camera| (camera.position, crate::game::view_vector(camera.yaw, camera.pitch)),
    );
    let (origin, direction) = match settings.perspective {
        bridging::PerspectiveMode::Eyeline => (eye, player_direction),
        bridging::PerspectiveMode::Auto | bridging::PerspectiveMode::Camera => {
            (camera_origin, camera_direction)
        }
    };
    let Some(candidates) = bridging::candidate_path_with_options(
        origin,
        direction,
        eye,
        player_direction,
        game.block_reach,
        settings.min_distance_percent,
        settings.adjacency,
        settings.snap_strength_percent,
    ) else {
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
    let faces = [
        [0, 1, 0], [0, -1, 0], [0, 0, 1],
        [0, 0, -1], [1, 0, 0], [-1, 0, 0],
    ];
    for pos in candidates {
        let lo = pos.map(f64::from);
        let hi = [lo[0] + 1.0, lo[1] + 1.0, lo[2] + 1.0];
        if (0..3).all(|axis| lo[axis] < player_max[axis] && hi[axis] > player_min[axis]) {
            continue;
        }
        // Preview is visual-only; adventure placement permission is checked
        // by the real use hook immediately before dispatch.
        let facts = game.res.facts();
        let state = game.world.block(pos[0], pos[1], pos[2]);
        let target_facts = facts.get(state);
        if !facts.is_air(state) && !(settings.replace_non_solid && target_facts.replaceable) {
            continue;
        }
        let selected = faces.into_iter().find(|face| {
            if !axes.allows(*face) {
                return false;
            }
            let dot = direction[0] * f64::from(face[0])
                + direction[1] * f64::from(face[1])
                + direction[2] * f64::from(face[2]);
            if dot > -0.1 {
                return false;
            }
            let support = [pos[0] + face[0], pos[1] + face[1], pos[2] + face[2]];
            let support_state = game.world.block(support[0], support[1], support[2]);
            let support_facts = facts.get(support_state);
            !facts.is_air(support_state)
                && !support_facts.replaceable
                && (support_facts.fluid == horizon_client::blocks::Fluid::None
                    || support_facts.waterlogged)
        });
        let Some(face) = selected else { continue; };
        bridging::set_visual_target(Some((pos, face)));
        if settings.show_outline {
            let [x, y, z] = pos.map(f64::from);
            let corners = [
                [x, y, z], [x + 1.0, y, z], [x + 1.0, y + 1.0, z], [x, y + 1.0, z],
                [x, y, z + 1.0], [x + 1.0, y, z + 1.0],
                [x + 1.0, y + 1.0, z + 1.0], [x, y + 1.0, z + 1.0],
            ];
            for (a, b) in [
                (0, 1), (1, 2), (2, 3), (3, 0), (4, 5), (5, 6),
                (6, 7), (7, 4), (0, 4), (1, 5), (2, 6), (3, 7),
            ] {
                lines.push(horizon_client::gizmos::Line {
                    start: corners[a],
                    end: corners[b],
                    color: 0x9900_99FF,
                    width: 2.0,
                });
            }
        }
        break;
    }
}