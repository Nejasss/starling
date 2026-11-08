// Starling decrements use_delay before native_tick. Adjust a successful
// assisted use, then refresh one target for placement and both visuals.
// The path/support scan runs once per tick, not once per rendered frame.
fn bridging_assist_use_tick(game: &mut crate::game::Game<'_>) {
    let game_key = game as *mut _ as usize;
    if let Some(interval_ticks) = crate::game::bridging_assist::take_assisted_delay(game_key) {
        game.use_delay = crate::game::bridging_assist::remaining_use_delay(
            game.use_delay,
            interval_ticks,
        );
    }

    use crate::game::bridging_assist as bridging;
    bridging::set_visual_target(None);

    if !game.native_on(bridging::ID)
        || game.screen != crate::game::Screen::Playing
        || game.sleeping.is_some()
        || game.using.is_some()
        || game.forced_hit.is_some()
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
    let allowed_axes = settings.effective_axis(crouching);

    // Main hand first; if it is not a usable block stack, preview the offhand.
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
    let held = if main_is_placeable { main_held } else { game.offhand.clone() };
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

    // Entity targeting is deliberately left to the actual click hook. Doing a
    // full mob ray scan here every client tick made preview expensive in busy worlds.
    if game.pick().is_some() {
        return;
    }

    let Some((aim_origin, aim_direction)) = game.aim() else { return; };
    let Some(player_eye) = game.eye_position() else { return; };
    let Some(player) = game.player.as_ref() else { return; };
    bridging::remember_grounded_y_fraction(player.position.y, player.ground.on_ground);

    let player_direction = crate::game::view_vector(game.yaw, game.pitch);
    // Camera direction supplies the ray; player eye direction supplies reach.
    let (camera_origin, camera_direction) = game.native_camera().map_or(
        (aim_origin, aim_direction),
        |camera| (camera.position, crate::game::view_vector(camera.yaw, camera.pitch)),
    );
    let (origin, direction) = match settings.perspective {
        bridging::PerspectiveMode::Eyeline => (player_eye, player_direction),
        bridging::PerspectiveMode::Auto | bridging::PerspectiveMode::Camera => {
            (camera_origin, camera_direction)
        }
    };

    // Horizontal means a genuinely level bridge: flatten both traversal rays
    // and lock every candidate to the last grounded block plane. Only the
    // camera yaw determines the forward XZ direction, never camera pitch.
    let horizontal_lock = allowed_axes == bridging::AxisMode::Horizontal;
    let (direction, player_path_direction, target_y) = if horizontal_lock {
        let yaw_fallback = crate::game::view_vector(game.yaw, 0.0);
        let Some(direction) = bridging::horizontal_direction(direction)
            .or_else(|| bridging::horizontal_direction(yaw_fallback))
        else {
            return;
        };
        let Some(player_path_direction) = bridging::horizontal_direction(player_direction)
            .or_else(|| bridging::horizontal_direction(yaw_fallback))
        else {
            return;
        };
        let Some(target_y) = bridging::grounded_block_y(player.position.y) else {
            return;
        };
        (direction, player_path_direction, Some(target_y))
    } else {
        (direction, player_direction, None)
    };
    let adjacency = if horizontal_lock {
        bridging::AdjacencyMode::Disabled
    } else {
        settings.adjacency
    };
    let Some(candidates) = bridging::candidate_path_with_options(
        origin,
        direction,
        player_eye,
        player_path_direction,
        game.block_reach,
        settings.min_distance_percent,
        adjacency,
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
    // Support-side directions, ordered like Bridging Mod's valid-side list.
    let faces = [
        [0, 1, 0],
        [0, -1, 0],
        [0, 0, 1],
        [0, 0, -1],
        [1, 0, 0],
        [-1, 0, 0],
    ];
    let facts = game.res.facts();

    for mut pos in candidates {
        if let Some(target_y) = target_y {
            pos[1] = target_y;
        }
        let cell_min = [f64::from(pos[0]), f64::from(pos[1]), f64::from(pos[2])];
        let cell_max = [cell_min[0] + 1.0, cell_min[1] + 1.0, cell_min[2] + 1.0];
        if (0..3).all(|axis| cell_min[axis] < player_max[axis] && cell_max[axis] > player_min[axis]) {
            continue;
        }

        let state = game.world.block(pos[0], pos[1], pos[2]);
        let replaceable = facts.get(state).replaceable;
        if !facts.is_air(state) && !(settings.replace_non_solid && replaceable) {
            continue;
        }

        let selected_face = faces.into_iter().find(|face| {
            if !allowed_axes.allows(*face) {
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
        if let Some(face) = selected_face {
            bridging::set_visual_target(Some((pos, face)));
            return;
        }
    }
}
