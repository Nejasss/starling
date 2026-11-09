use super::Game;
use horizon_client::{dynamic_light, gizmos};

fn rim_target_valid(
    loaded: bool,
    state: horizon_world::registry::BlockStateId,
    air: horizon_world::registry::BlockStateId,
) -> bool {
    loaded && state != air
}

impl Game<'_> {
    /// Frame-local requests never persist after a mod is removed or a world changes.
    pub(super) fn native_frame_visuals(
        &self,
        camera: glam::DVec3,
        alpha: f64,
        lines: &mut Vec<gizmos::Line>,
        on_top: &mut Vec<gizmos::Line>,
    ) -> Vec<dynamic_light::Colored> {
        // Preserve the parameter name used by existing native mixin bindings.
        let _ = lines;
        let mut particle_lights = self
            .renderer
            .as_ref()
            .and_then(|r| r.particle_lights())
            .map_or_else(Vec::new, |entries| {
                self.particles
                    .lights(entries, self.clock.camera_fraction() as f32)
            });
        // Held and dropped items whose block the pack's `[[lights]]` colour shine in that colour,
        // as far as their dynamic light reaches.
        if let Some(renderer) = self.renderer.as_ref().filter(|r| r.has_light_colors()) {
            for (source, item) in self.dynamic_light_sources(alpha) {
                let Some(color) = item.and_then(|i| renderer.light_color(&i)) else {
                    continue;
                };
                let top = color[0].max(color[1]).max(color[2]);
                if source.level == 0 || top <= 0.0 {
                    continue;
                }
                let [r, g, b] = color.map(|c| (c / top * 255.0).round() as u32);
                particle_lights.push((
                    source.at.to_array(),
                    r << 16 | g << 8 | b,
                    f32::from(source.level),
                    1.0,
                ));
            }
        }
        let Some(player) = self.native_player() else {
            return dynamic_light::select_colored(particle_lights.into_iter(), camera);
        };
        let mut rims = Vec::new();
        let mut lights = Vec::new();
        self.native_visuals(&mut rims, &mut lights);
        let close = |at: [f64; 3]| {
            at.iter().all(|v| v.is_finite())
                && (at[0] - player.x).powi(2)
                    + (at[1] - player.y).powi(2)
                    + (at[2] - player.z).powi(2)
                    <= 1024.0
        };
        // A tunnel can legitimately extend farther than 32 blocks from the player.
        // Keep rim range separate from the shorter dynamic-light range.
        let close_rim = |at: [f64; 3]| {
            at.iter().all(|v| v.is_finite())
                && (at[0] - player.x).powi(2)
                    + (at[1] - player.y).powi(2)
                    + (at[2] - player.z).powi(2)
                    <= 16384.0
        };
        const MAX_RIMS: usize = 64;
        const MAX_EDGES: usize = MAX_RIMS * 24;
        let mut edges = 0;
        for rim in rims.into_iter().take(MAX_RIMS) {
            let pos = rim.pos;
            let state = self.world.block(pos[0], pos[1], pos[2]);
            if edges >= MAX_EDGES
                || !rim.width.is_finite()
                || !(0.5..=4.0).contains(&rim.width)
                || !close_rim(pos.map(|v| f64::from(v) + 0.5))
                || !rim_target_valid(
                    self.world
                        .chunks
                        .contains_key(&(pos[0].div_euclid(16), pos[2].div_euclid(16))),
                    state,
                    self.world.air,
                )
            {
                continue;
            }
            let facts = self.res.facts().get(state);
            let offset = if facts.outline_offset {
                self.pack
                    .context
                    .tables
                    .offset(state, pos[0], pos[2])
                    .unwrap_or([0.0; 3])
            } else {
                [0.0; 3]
            };
            let origin = pos.map(f64::from);
            let box_shape = self.context_outline(pos, state);
            let cube = horizon_client::blocks::cube_edges();
            let segments = if box_shape.is_some() {
                cube.as_slice()
            } else {
                facts.outline_edges.as_ref()
            };
            for edge in segments.iter().take(MAX_EDGES - edges) {
                let point = |end: usize| {
                    std::array::from_fn(|i| {
                        let v = edge[i + 3 * end];
                        if let Some(b) = box_shape {
                            origin[i] + b[i] + v * (b[i + 3] - b[i])
                        } else {
                            origin[i] + offset[i] + v
                        }
                    })
                };
                // X-ray selection: draw in the on-top pass, with partial opacity.
                on_top.push(gizmos::Line {
                    start: point(0),
                    end: point(1),
                    color: (rim.argb & 0x00ff_ffff) | ((rim.argb >> 1) & 0x7f00_0000),
                    width: rim.width,
                });
                edges += 1;
            }
        }
        let sources = lights.into_iter().take(8).filter(|l| {
            close(l.at)
                && l.rgb <= 0xffffff
                && l.radius.is_finite()
                && (0.25..=16.0).contains(&l.radius)
                && l.intensity.is_finite()
                && (0.0..=1.0).contains(&l.intensity)
                && self.world.chunks.contains_key(&(
                    (l.at[0].floor() as i32).div_euclid(16),
                    (l.at[2].floor() as i32).div_euclid(16),
                ))
        });
        dynamic_light::select_colored(
            sources
                .map(|l| (l.at, l.rgb, l.radius, l.intensity))
                .chain(particle_lights),
            camera,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::rim_target_valid;
    use horizon_world::registry::BlockStateId;

    #[test]
    fn ordinary_loaded_blocks_can_have_rims() {
        let air = BlockStateId(0);
        let stone = BlockStateId(1); // No emission requirement.
        assert!(rim_target_valid(true, stone, air));
        assert!(!rim_target_valid(false, stone, air));
        assert!(!rim_target_valid(true, air, air));
    }
}
