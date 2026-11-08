fn bridging_assist_frame_visuals(
    game: &crate::game::Game<'_>,
    lines: &mut Vec<horizon_client::gizmos::Line>,
) {
    use crate::game::bridging_assist as bridging;

    if !game.native_on(bridging::ID) || game.screen != crate::game::Screen::Playing {
        return;
    }
    // Reuse the parsed snapshot: native_setting reads a file from disk.
    let settings = bridging::cached_settings(game);
    if !settings.enabled || !settings.show_outline {
        return;
    }
    let Some((pos, _)) = bridging::visual_target() else { return; };

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
