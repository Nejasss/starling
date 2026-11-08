// Bridging Assist for Starling.
//
// This is a Rust reimplementation of the observable reacharound-placement
// behavior of Bridging Mod (MIT). It deliberately starts with a small,
// testable subset: voxel traversal, support-face selection at the adapter,
// and persisted basic settings. No Fabric classes or Java code are shipped.

use std::cell::Cell;

pub const ID: &str = "bridging_assist";
pub const VERSION: &str = "0.4.3";
pub const DESCRIPTION: &str =
    "Reacharound placement using separate camera and player perspectives.";
pub const KEY: &str = "toggle";
pub const DEFAULT_KEY: &str = "key.keyboard.comma";
pub const ON: &str = "Bridging Assist: ON";
pub const OFF: &str = "Bridging Assist: OFF";


#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdjacencyMode { Disabled, Faces, Edges, Corners }
impl AdjacencyMode {
    fn parse(value: Option<String>, fallback: Self) -> Self {
        match value.as_deref().map(str::trim) {
            Some("disabled") => Self::Disabled, Some("faces") => Self::Faces,
            Some("edges") => Self::Edges, Some("corners") => Self::Corners, _ => fallback,
        }
    }
    fn as_str(self) -> &'static str { match self { Self::Disabled => "disabled", Self::Faces => "faces", Self::Edges => "edges", Self::Corners => "corners" } }
    fn next(self) -> Self { match self { Self::Disabled => Self::Faces, Self::Faces => Self::Edges, Self::Edges => Self::Corners, Self::Corners => Self::Disabled } }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AxisMode { Horizontal, Vertical, Both }
impl AxisMode {
    fn parse(value: Option<String>, fallback: Self) -> Self {
        match value.as_deref().map(str::trim) { Some("horizontal") => Self::Horizontal, Some("vertical") => Self::Vertical, Some("both") => Self::Both, _ => fallback }
    }
    fn as_str(self) -> &'static str { match self { Self::Horizontal => "horizontal", Self::Vertical => "vertical", Self::Both => "both" } }
    fn next(self) -> Self { match self { Self::Both => Self::Horizontal, Self::Horizontal => Self::Vertical, Self::Vertical => Self::Both } }
    pub fn allows(self, face: [i32; 3]) -> bool { match self { Self::Both => true, Self::Vertical => face[1] != 0, Self::Horizontal => face[1] == 0 } }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AxisOverride { Fallback, Horizontal, Vertical, Both }
impl AxisOverride {
    fn parse(value: Option<String>, fallback: Self) -> Self {
        match value.as_deref().map(str::trim) { Some("fallback") => Self::Fallback, Some("horizontal") => Self::Horizontal, Some("vertical") => Self::Vertical, Some("both") => Self::Both, _ => fallback }
    }
    fn as_str(self) -> &'static str { match self { Self::Fallback => "fallback", Self::Horizontal => "horizontal", Self::Vertical => "vertical", Self::Both => "both" } }
    fn next(self) -> Self { match self { Self::Fallback => Self::Both, Self::Both => Self::Horizontal, Self::Horizontal => Self::Vertical, Self::Vertical => Self::Fallback } }
    fn resolve(self, base: AxisMode) -> AxisMode { match self { Self::Fallback => base, Self::Horizontal => AxisMode::Horizontal, Self::Vertical => AxisMode::Vertical, Self::Both => AxisMode::Both } }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PerspectiveMode { Auto, Camera, Eyeline }
impl PerspectiveMode {
    fn parse(value: Option<String>, fallback: Self) -> Self {
        match value.as_deref().map(str::trim) { Some("auto") => Self::Auto, Some("camera") => Self::Camera, Some("eyeline") => Self::Eyeline, _ => fallback }
    }
    fn as_str(self) -> &'static str { match self { Self::Auto => "auto", Self::Camera => "camera", Self::Eyeline => "eyeline" } }
    fn next(self) -> Self { match self { Self::Auto => Self::Camera, Self::Camera => Self::Eyeline, Self::Eyeline => Self::Auto } }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub enabled: bool,
    pub crouch_only: bool,
    pub replace_non_solid: bool,
    pub skip_torches: bool,
    pub min_distance_percent: u8,
    pub adjacency: AdjacencyMode,
    pub snap_strength_percent: u8,
    pub axis_mode: AxisMode,
    pub crouch_axis_override: AxisOverride,
    pub perspective: PerspectiveMode,
    pub slab_assist: bool,
    pub post_delay_ticks: u8,
    pub show_crosshair: bool,
    pub show_outline: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self { enabled: true, crouch_only: false, replace_non_solid: true, skip_torches: true,
            min_distance_percent: 20, adjacency: AdjacencyMode::Disabled, snap_strength_percent: 100,
            axis_mode: AxisMode::Horizontal, crouch_axis_override: AxisOverride::Fallback,
            perspective: PerspectiveMode::Auto, slab_assist: true, post_delay_ticks: 3,
            show_crosshair: true, show_outline: true }
    }
}
impl Settings {
    pub fn read(get: impl Fn(&str) -> Option<String>) -> Self {
        let d = Self::default();
        let boolean = |key: &str, fallback: bool| get(key).map_or(fallback, |v| match v.trim() { "true" => true, "false" => false, _ => fallback });
        let number = |key: &str, fallback: u8, max: u8| get(key).and_then(|v| v.trim().parse::<u8>().ok()).filter(|v| *v <= max).unwrap_or(fallback);
        Self {
            enabled: boolean("enabled", d.enabled), crouch_only: boolean("crouch_only", d.crouch_only),
            replace_non_solid: boolean("replace_non_solid", d.replace_non_solid), skip_torches: boolean("skip_torches", d.skip_torches),
            min_distance_percent: number("min_distance_percent", d.min_distance_percent, 100),
            adjacency: AdjacencyMode::parse(get("path_adjacency"), d.adjacency),
            snap_strength_percent: number("snap_strength_percent", d.snap_strength_percent, 100),
            // Fresh keys avoid silently restoring the old BOTH/1-tick profile.
            axis_mode: AxisMode::parse(get("placement_axes"), d.axis_mode),
            crouch_axis_override: AxisOverride::parse(get("crouch_axis_override"), d.crouch_axis_override),
            perspective: PerspectiveMode::parse(get("perspective"), d.perspective),
            slab_assist: boolean("slab_assist", d.slab_assist),
            post_delay_ticks: number("placement_interval_ticks", d.post_delay_ticks, 4),
            show_crosshair: boolean("show_crosshair", d.show_crosshair),
            show_outline: boolean("show_outline", d.show_outline),
        }
    }
    pub fn effective_axis(self, crouching: bool) -> AxisMode { if crouching { self.crouch_axis_override.resolve(self.axis_mode) } else { self.axis_mode } }
    pub fn labels(self) -> Vec<String> {
        let on = |v: bool| if v { "ON" } else { "OFF" };
        vec![
            format!("Bridging Assist: {}", on(self.enabled)), format!("Crouch only: {}", on(self.crouch_only)),
            format!("Replace non-solid targets: {}", on(self.replace_non_solid)), format!("Skip torches: {}", on(self.skip_torches)),
            format!("Minimum bridge distance: {}%", self.min_distance_percent),
            format!("Path adjacency: {}", self.adjacency.as_str().to_uppercase()),
            format!("Path snap strength: {}%", self.snap_strength_percent),
            format!("Placement axes: {}", self.axis_mode.as_str().to_uppercase()),
            format!("Crouch axis override: {}", self.crouch_axis_override.as_str().to_uppercase()),
            format!("Camera perspective: {}", self.perspective.as_str().to_uppercase()),
            format!("Slab assist: {}", on(self.slab_assist)),
            format!("Placement interval: {} tick(s)", self.post_delay_ticks),
            format!("Crosshair indicator: {}", on(self.show_crosshair)),
            format!("Placement outline: {}", on(self.show_outline)),
        ]
    }
    pub fn click(&mut self, button: usize) {
        match button {
            0 => self.enabled = !self.enabled, 1 => self.crouch_only = !self.crouch_only,
            2 => self.replace_non_solid = !self.replace_non_solid, 3 => self.skip_torches = !self.skip_torches,
            4 => self.min_distance_percent = match self.min_distance_percent { 0 => 5, 5 => 10, 10 => 20, 20 => 30, 30 => 40, 40 => 50, 50 => 60, 60 => 80, 80 => 100, _ => 0 },
            5 => self.adjacency = self.adjacency.next(),
            6 => self.snap_strength_percent = match self.snap_strength_percent { 0 => 25, 25 => 50, 50 => 75, 75 => 100, _ => 0 },
            7 => self.axis_mode = self.axis_mode.next(), 8 => self.crouch_axis_override = self.crouch_axis_override.next(),
            9 => self.perspective = self.perspective.next(), 10 => self.slab_assist = !self.slab_assist,
            11 => self.post_delay_ticks = match self.post_delay_ticks { 1 => 2, 2 => 3, 3 => 4, _ => 1 },
            12 => self.show_crosshair = !self.show_crosshair,
            13 => self.show_outline = !self.show_outline,
            _ => {}
        }
    }
    pub fn stored(self) -> [(&'static str, String); 14] {
        let flag = |v: bool| if v { "true" } else { "false" }.to_owned();
        [
            ("enabled", flag(self.enabled)), ("crouch_only", flag(self.crouch_only)),
            ("replace_non_solid", flag(self.replace_non_solid)), ("skip_torches", flag(self.skip_torches)),
            ("min_distance_percent", self.min_distance_percent.to_string()), ("path_adjacency", self.adjacency.as_str().to_owned()),
            ("snap_strength_percent", self.snap_strength_percent.to_string()), ("placement_axes", self.axis_mode.as_str().to_owned()),
            ("crouch_axis_override", self.crouch_axis_override.as_str().to_owned()), ("perspective", self.perspective.as_str().to_owned()),
            ("slab_assist", flag(self.slab_assist)), ("placement_interval_ticks", self.post_delay_ticks.to_string()),
            ("show_crosshair", flag(self.show_crosshair)), ("show_outline", flag(self.show_outline)),
        ]
    }
}
thread_local! {
    /// Remembers whether the original per-tick hit was a cached MISS.
    /// RETURN injection restores that exact state after the synthetic hit.
    static OVERRIDE_ACTIVE: Cell<Option<bool>> = const { Cell::new(None) };
}

pub fn arm_override(restore_cached_miss: bool) {
    OVERRIDE_ACTIVE.with(|active| active.set(Some(restore_cached_miss)));
}

pub fn take_override() -> Option<bool> {
    OVERRIDE_ACTIVE.with(|active| active.replace(None))
}

thread_local! {
    static VISUAL_TARGET: Cell<Option<([i32; 3], [i32; 3])>> = const { Cell::new(None) };
}
pub fn set_visual_target(target: Option<([i32; 3], [i32; 3])>) {
    VISUAL_TARGET.with(|value| value.set(target));
}
pub fn visual_target() -> Option<([i32; 3], [i32; 3])> {
    VISUAL_TARGET.with(|value| value.get())
}

thread_local! {
    static PENDING_ASSISTED_DELAY: Cell<Option<(usize, u8)>> = const { Cell::new(None) };
}

pub fn arm_assisted_delay(game_key: usize, interval_ticks: u8) {
    PENDING_ASSISTED_DELAY.with(|pending| pending.set(Some((game_key, interval_ticks))));
}

pub fn take_assisted_delay(game_key: usize) -> Option<u8> {
    PENDING_ASSISTED_DELAY.with(|pending| match pending.get() {
        Some((key, ticks)) if key == game_key => {
            pending.set(None);
            Some(ticks)
        }
        _ => None,
    })
}

/// Starling decrements use_delay before native_tick, while its held-use loop
/// writes a hard-coded four ticks after the assisted interaction returns.
/// Convert the requested interval to the remaining delay for the next tick.
pub fn remaining_use_delay(current: u32, interval_ticks: u8) -> u32 {
    current.min(u32::from(interval_ticks.saturating_sub(1)))
}


thread_local! {
    static LAST_GROUNDED_Y_FRAC: Cell<f64> = const { Cell::new(0.0) };
}
thread_local! {
    static LAST_GROUNDED_BLOCK_Y: Cell<Option<i32>> = const { Cell::new(None) };
}
fn block_below_feet(y: f64) -> Option<i32> {
    let block_y = (y - 1.0e-4).floor();
    (block_y.is_finite() && block_y >= i32::MIN as f64 && block_y <= i32::MAX as f64)
        .then_some(block_y as i32)
}
pub fn remember_grounded_y_fraction(y: f64, on_ground: bool) {
    if on_ground && y.is_finite() {
        LAST_GROUNDED_Y_FRAC.with(|value| value.set(y.rem_euclid(1.0)));
        if let Some(block_y) = block_below_feet(y) {
            LAST_GROUNDED_BLOCK_Y.with(|value| value.set(Some(block_y)));
        }
    }
}
pub fn last_grounded_y_fraction() -> f64 { LAST_GROUNDED_Y_FRAC.with(|value| value.get()) }
pub fn grounded_block_y(fallback_player_y: f64) -> Option<i32> {
    LAST_GROUNDED_BLOCK_Y.with(|value| value.get()).or_else(|| block_below_feet(fallback_player_y))
}
/// Normalize a view vector onto the horizontal XZ plane. A near-vertical view
/// has no stable horizontal direction and must use the caller's yaw fallback.
pub fn horizontal_direction(direction: [f64; 3]) -> Option<[f64; 3]> {
    let length_sq = direction[0] * direction[0] + direction[2] * direction[2];
    if !length_sq.is_finite() || length_sq <= 1.0e-12 {
        return None;
    }
    let inverse_length = length_sq.sqrt().recip();
    Some([direction[0] * inverse_length, 0.0, direction[2] * inverse_length])
}


/// Builds Bridging Mod's two-perspective voxel path.
///
/// The path starts at the camera perspective, but its length is measured to
/// the endpoint of the player's own reach ray. This distinction matters in
/// detached/third-person camera modes; using the camera direction for both
/// rays shortens or lengthens the candidate path incorrectly.
pub fn candidate_path(
    camera_origin: [f64; 3],
    camera_direction: [f64; 3],
    player_eye: [f64; 3],
    player_direction: [f64; 3],
    reach: f64,
    min_distance_percent: u8,
) -> Option<Vec<[i32; 3]>> {
    candidate_path_with_options(
        camera_origin,
        camera_direction,
        player_eye,
        player_direction,
        reach,
        min_distance_percent,
        AdjacencyMode::Corners,
        100,
    )
}

pub fn candidate_path_with_options(
    camera_origin: [f64; 3],
    camera_direction: [f64; 3],
    player_eye: [f64; 3],
    player_direction: [f64; 3],
    reach: f64,
    min_distance_percent: u8,
    adjacency: AdjacencyMode,
    snap_strength_percent: u8,
) -> Option<Vec<[i32; 3]>> {
    if !reach.is_finite()
        || reach <= 0.0
        || min_distance_percent > 100
        || snap_strength_percent > 100
        || camera_origin
            .into_iter()
            .chain(camera_direction)
            .chain(player_eye)
            .chain(player_direction)
            .any(|value| !value.is_finite())
    {
        return None;
    }

    let player_end = [
        player_eye[0] + player_direction[0] * reach,
        player_eye[1] + player_direction[1] * reach,
        player_eye[2] + player_direction[2] * reach,
    ];
    let distance = ((player_end[0] - camera_origin[0]).powi(2)
        + (player_end[1] - camera_origin[1]).powi(2)
        + (player_end[2] - camera_origin[2]).powi(2))
    .sqrt();
    if !distance.is_finite() || distance <= f64::EPSILON {
        return None;
    }

    let fraction = f64::from(min_distance_percent) / 100.0;
    let start = [
        camera_origin[0] + camera_direction[0] * distance * fraction,
        camera_origin[1] + camera_direction[1] * distance * fraction,
        camera_origin[2] + camera_direction[2] * distance * fraction,
    ];
    let end = [
        camera_origin[0] + camera_direction[0] * distance,
        camera_origin[1] + camera_direction[1] * distance,
        camera_origin[2] + camera_direction[2] * distance,
    ];

    fn cell(point: [f64; 3]) -> Option<[i32; 3]> {
        let mut result = [0; 3];
        for axis in 0..3 {
            let value = point[axis].floor();
            if !value.is_finite() || value < i32::MIN as f64 || value > i32::MAX as f64 {
                return None;
            }
            result[axis] = value as i32;
        }
        Some(result)
    }

    Some(voxel_line_with_adjacency(
        cell(start)?,
        cell(end)?,
        adjacency,
        snap_strength_percent,
    ))
}

/// The native equivalent of the upstream result's opposite-side hit face.
/// The support-side vector points from the candidate toward its support;
/// the hit face sent to the ordinary placement pipeline points the other way.
pub fn placement_hit_face(support_face: [i32; 3]) -> [i32; 3] {
    [-support_face[0], -support_face[1], -support_face[2]]
}

/// Integer 3-D Bresenham traversal, including both endpoints, in camera-to-
/// player order. The caller supplies floored block coordinates.
pub fn voxel_line(start: [i32; 3], end: [i32; 3]) -> Vec<[i32; 3]> {
    let mut point = start;
    let dx = (end[0] - start[0]).abs();
    let dy = (end[1] - start[1]).abs();
    let dz = (end[2] - start[2]).abs();
    let sx = if end[0] >= start[0] { 1 } else { -1 };
    let sy = if end[1] >= start[1] { 1 } else { -1 };
    let sz = if end[2] >= start[2] { 1 } else { -1 };
    let mut out = Vec::with_capacity((dx.max(dy).max(dz) as usize).saturating_add(1));

    if dx >= dy && dx >= dz {
        let (mut p1, mut p2) = (2 * dy - dx, 2 * dz - dx);
        for _ in 0..=dx {
            out.push(point);
            if point == end {
                break;
            }
            point[0] += sx;
            if p1 >= 0 {
                point[1] += sy;
                p1 -= 2 * dx;
            }
            if p2 >= 0 {
                point[2] += sz;
                p2 -= 2 * dx;
            }
            p1 += 2 * dy;
            p2 += 2 * dz;
        }
    } else if dy >= dx && dy >= dz {
        let (mut p1, mut p2) = (2 * dx - dy, 2 * dz - dy);
        for _ in 0..=dy {
            out.push(point);
            if point == end {
                break;
            }
            point[1] += sy;
            if p1 >= 0 {
                point[0] += sx;
                p1 -= 2 * dy;
            }
            if p2 >= 0 {
                point[2] += sz;
                p2 -= 2 * dy;
            }
            p1 += 2 * dx;
            p2 += 2 * dz;
        }
    } else {
        let (mut p1, mut p2) = (2 * dy - dz, 2 * dx - dz);
        for _ in 0..=dz {
            out.push(point);
            if point == end {
                break;
            }
            point[2] += sz;
            if p1 >= 0 {
                point[1] += sy;
                p1 -= 2 * dz;
            }
            if p2 >= 0 {
                point[0] += sx;
                p2 -= 2 * dz;
            }
            p1 += 2 * dy;
            p2 += 2 * dx;
        }
    }
    out
}

/// Bresenham's primary cells plus the voxels the reach segment clips at
/// shared edges/corners. Bridging Mod 2.7.0 expands these cells before testing
/// placement support; omitting them makes a fast forward run skip valid cells
/// and can produce a staircase-like bridge line.
pub fn voxel_line_with_adjacency(
    start: [i32; 3],
    end: [i32; 3],
    adjacency: AdjacencyMode,
    snap_strength_percent: u8,
) -> Vec<[i32; 3]> {
    let primary = voxel_line(start, end);
    if adjacency == AdjacencyMode::Disabled || primary.len() < 2 {
        return primary;
    }

    let strength = f64::from(snap_strength_percent) / 100.0;
    let extent = 0.5 * strength;
    let a = start.map(f64::from);
    let b = end.map(f64::from);
    let mut output = Vec::with_capacity(primary.len().saturating_mul(3));
    let mut seen = std::collections::HashSet::new();
    output.push(start);
    seen.insert(start);

    for pair in primary.windows(2) {
        let previous = pair[0];
        let next = pair[1];
        let delta = [next[0] - previous[0], next[1] - previous[1], next[2] - previous[2]];
        let manhattan = delta.iter().map(|v| v.abs()).sum::<i32>();
        let mut candidates = Vec::with_capacity(6);

        if manhattan == 2 && adjacency != AdjacencyMode::Disabled {
            for axis in 0..3 {
                if delta[axis] != 0 {
                    let mut offset = [0; 3];
                    offset[axis] = delta[axis];
                    candidates.push([
                        previous[0] + offset[0],
                        previous[1] + offset[1],
                        previous[2] + offset[2],
                    ]);
                }
            }
        } else if manhattan == 3 && matches!(adjacency, AdjacencyMode::Edges | AdjacencyMode::Corners) {
            let offsets = [
                [delta[0], 0, 0],
                [0, delta[1], 0],
                [0, 0, delta[2]],
                [delta[0], delta[1], 0],
                [delta[0], 0, delta[2]],
                [0, delta[1], delta[2]],
            ];
            let count = if adjacency == AdjacencyMode::Edges { 3 } else { 6 };
            for offset in offsets.into_iter().take(count) {
                candidates.push([
                    previous[0] + offset[0],
                    previous[1] + offset[1],
                    previous[2] + offset[2],
                ]);
            }
        }

        for candidate in candidates {
            if seen.contains(&candidate) || !segment_intersects_snap_box(a, b, candidate, extent) {
                continue;
            }
            seen.insert(candidate);
            output.push(candidate);
        }
        if seen.insert(next) {
            output.push(next);
        }
    }
    output
}

/// Separating-axis test for a segment against a block-sized snap box. `extent`
/// is 0.5 at 100% (the full block), matching Bridging Mod's Path.CUBE_EXTENT.
fn segment_intersects_snap_box(
    line_start: [f64; 3],
    line_end: [f64; 3],
    block: [i32; 3],
    extent: f64,
) -> bool {
    let origin = block.map(f64::from);
    let start: [f64; 3] = std::array::from_fn(|i| line_start[i] - origin[i]);
    let end: [f64; 3] = std::array::from_fn(|i| line_end[i] - origin[i]);
    let mid: [f64; 3] = std::array::from_fn(|i| (start[i] + end[i]) * 0.5);
    let half_line: [f64; 3] = std::array::from_fn(|i| start[i] - mid[i]);
    let line_ext = half_line.map(f64::abs);

    for axis in 0..3 {
        if mid[axis].abs() > extent + line_ext[axis] {
            return false;
        }
    }
    let cross = [
        mid[1] * half_line[2] - mid[2] * half_line[1],
        mid[0] * half_line[2] - mid[2] * half_line[0],
        mid[0] * half_line[1] - mid[1] * half_line[0],
    ];
    let limits = [
        extent * (line_ext[1] + line_ext[2]),
        extent * (line_ext[0] + line_ext[2]),
        extent * (line_ext[0] + line_ext[1]),
    ];
    (0..3).all(|axis| cross[axis].abs() <= limits[axis] + 1.0e-9)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stored<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        |key| pairs.iter().find(|pair| pair.0 == key).map(|pair| pair.1.to_owned())
    }

    #[test]
    fn visual_target_is_shared_and_can_be_cleared() {
        set_visual_target(Some(([4, 5, 6], [0, 1, 0])));
        assert_eq!(visual_target(), Some(([4, 5, 6], [0, 1, 0])));
        set_visual_target(None);
        assert_eq!(visual_target(), None);
    }

    #[test]
    fn override_marker_restores_cached_miss_once() {
        arm_override(true);
        assert_eq!(take_override(), Some(true));

        arm_override(false);
        assert_eq!(take_override(), Some(false));

        assert_eq!(take_override(), None);
    }

    #[test]
    fn defaults_and_persisted_settings_roundtrip() {
        let mut settings = Settings::read(stored(&[]));
        assert_eq!(settings, Settings::default());
        settings.click(1);
        settings.click(2);
        settings.click(4);
        for button in 5..=13 { settings.click(button); }
        let pairs: Vec<(&str, String)> = settings.stored().into();
        let borrowed: Vec<(&str, &str)> = pairs
            .iter()
            .map(|(key, value)| (*key, value.as_str()))
            .collect();
        assert_eq!(Settings::read(stored(&borrowed)), settings);
    }

    #[test]
    fn horizontal_direction_and_grounded_level_are_stable() {
        assert_eq!(horizontal_direction([3.0, 8.0, 4.0]), Some([0.6, 0.0, 0.8]));
        assert_eq!(horizontal_direction([0.0, -1.0, 0.0]), None);
        remember_grounded_y_fraction(65.0, true);
        assert_eq!(grounded_block_y(70.0), Some(64));
        remember_grounded_y_fraction(70.0, false);
        assert_eq!(grounded_block_y(70.0), Some(64));
    }

    #[test]
    fn old_corner_adjacency_setting_does_not_override_safe_default() {
        let settings = Settings::read(stored(&[("adjacency", "corners")]));
        assert_eq!(settings.adjacency, AdjacencyMode::Disabled);
    }

    #[test]
    fn invalid_values_fall_back_to_safe_defaults() {
        let settings = Settings::read(stored(&[
            ("enabled", "maybe"),
            ("crouch_only", "false"),
            ("min_distance_percent", "255"),
            ("path_adjacency", "invalid"),
            ("snap_strength_percent", "999"),
            ("placement_axes", "invalid"),
            ("crouch_axis_override", "invalid"),
            ("perspective", "invalid"),
            ("placement_interval_ticks", "255"),
            ]));
        assert!(settings.enabled);
        assert!(!settings.crouch_only);
        assert_eq!(settings.min_distance_percent, 20);
    }

    #[test]
    fn voxel_line_handles_x_axis_and_reverse() {
        assert_eq!(voxel_line([0, 0, 0], [3, 0, 0]), vec![[0, 0, 0], [1, 0, 0], [2, 0, 0], [3, 0, 0]]);
        assert_eq!(voxel_line([2, 1, 0], [0, 1, 0]), vec![[2, 1, 0], [1, 1, 0], [0, 1, 0]]);
    }

    #[test]
    fn voxel_line_handles_all_axes_and_diagonal() {
        assert_eq!(voxel_line([0, 0, 0], [0, 3, 0]).last(), Some(&[0, 3, 0]));
        assert_eq!(voxel_line([0, 0, 0], [0, 0, 3]).last(), Some(&[0, 0, 3]));
        let line = voxel_line([0, 0, 0], [3, 3, 3]);
        assert_eq!(line, vec![[0, 0, 0], [1, 1, 1], [2, 2, 2], [3, 3, 3]]);
    }

    #[test]
    fn candidate_path_uses_player_ray_length_and_camera_ray_direction() {
        let path = candidate_path(
            [0.5, 5.5, 0.5],
            [1.0, 0.0, 0.0],
            [0.5, 5.5, 0.5],
            [0.0, 0.0, 1.0],
            5.0,
            20,
        )
        .unwrap();
        assert_eq!(
            path,
            vec![[1, 5, 0], [2, 5, 0], [3, 5, 0], [4, 5, 0], [5, 5, 0]]
        );
    }

    #[test]
    fn candidate_path_rejects_invalid_geometry() {
        assert!(candidate_path(
            [0.0; 3], [1.0, 0.0, 0.0], [0.0; 3], [1.0, 0.0, 0.0],
            0.0, 20
        ).is_none());
        assert!(candidate_path(
            [f64::NAN, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0; 3], [1.0, 0.0, 0.0],
            5.0, 20
        ).is_none());
    }

    #[test]
    fn placement_hit_face_points_from_candidate_toward_support() {
        assert_eq!(placement_hit_face([0, 1, 0]), [0, -1, 0]);
        assert_eq!(placement_hit_face([-1, 0, 0]), [1, 0, 0]);
    }

    #[test]
    fn edge_adjacency_includes_voxels_clipped_between_diagonal_steps() {
        let path = voxel_line_with_adjacency(
            [0, 0, 0],
            [1, 1, 0],
            AdjacencyMode::Edges,
            100,
        );
        assert!(path.contains(&[1, 0, 0]));
        assert!(path.contains(&[0, 1, 0]));
    }

    #[test]
    fn corners_mode_includes_more_candidates_than_edges_mode() {
        let edges = voxel_line_with_adjacency(
            [0, 0, 0],
            [1, 1, 1],
            AdjacencyMode::Edges,
            100,
        );
        let corners = voxel_line_with_adjacency(
            [0, 0, 0],
            [1, 1, 1],
            AdjacencyMode::Corners,
            100,
        );
        assert!(corners.len() > edges.len());
    }

    #[test]
    fn disabled_adjacency_keeps_the_primary_voxel_line() {
        let primary = voxel_line([0, 0, 0], [1, 1, 1]);
        let disabled = voxel_line_with_adjacency([0, 0, 0], [1, 1, 1], AdjacencyMode::Disabled, 100);
        assert_eq!(disabled, primary);
    }

    #[test]
    fn axis_filters_and_crouch_override_resolve_correctly() {
        assert!(AxisMode::Horizontal.allows([1, 0, 0]));
        assert!(!AxisMode::Horizontal.allows([0, 1, 0]));
        assert!(AxisMode::Vertical.allows([0, -1, 0]));
        assert_eq!(AxisOverride::Fallback.resolve(AxisMode::Horizontal), AxisMode::Horizontal);
        assert_eq!(AxisOverride::Vertical.resolve(AxisMode::Horizontal), AxisMode::Vertical);
    }

    #[test]
    fn defaults_use_flat_forward_bridge_profile() {
        let settings = Settings::default();
        assert_eq!(settings.perspective, PerspectiveMode::Auto);
        assert_eq!(settings.post_delay_ticks, 3);
        assert_eq!(settings.adjacency, AdjacencyMode::Corners);
        assert_eq!(settings.snap_strength_percent, 100);
        assert_eq!(settings.axis_mode, AxisMode::Horizontal);
        assert_eq!(settings.crouch_axis_override, AxisOverride::Fallback);
        assert!(settings.slab_assist);
        assert!(settings.show_crosshair);
        assert!(settings.show_outline);
    }

    #[test]
    fn zero_minimum_distance_is_a_valid_upstream_boundary() {
        let path = candidate_path_with_options(
            [0.5, 5.5, 0.5], [1.0, 0.0, 0.0],
            [0.5, 5.5, 0.5], [1.0, 0.0, 0.0],
            5.0, 0, AdjacencyMode::Disabled, 100,
        ).unwrap();
        assert_eq!(path.first(), Some(&[0, 5, 0]));
    }


    #[test]
    fn assisted_interval_maps_to_remaining_starling_delay() {
        assert_eq!(remaining_use_delay(3, 1), 0);
        assert_eq!(remaining_use_delay(3, 2), 1);
        assert_eq!(remaining_use_delay(3, 3), 2);
        assert_eq!(remaining_use_delay(3, 4), 3);
        assert_eq!(remaining_use_delay(1, 4), 1);
    }

}
