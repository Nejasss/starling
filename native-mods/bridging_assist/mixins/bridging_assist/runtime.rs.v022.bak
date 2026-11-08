// Bridging Assist for Starling.
//
// This is a Rust reimplementation of the observable reacharound-placement
// behavior of Bridging Mod (MIT). It deliberately starts with a small,
// testable subset: voxel traversal, support-face selection at the adapter,
// and persisted basic settings. No Fabric classes or Java code are shipped.

use std::cell::Cell;

pub const ID: &str = "bridging_assist";
pub const VERSION: &str = "0.2.2";
pub const DESCRIPTION: &str =
    "Reacharound placement using separate camera and player perspectives.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub enabled: bool,
    pub crouch_only: bool,
    pub replace_non_solid: bool,
    pub skip_torches: bool,
    /// Start the candidate path this percentage of the camera-to-reach ray.
    pub min_distance_percent: u8,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: true,
            crouch_only: false,
            replace_non_solid: true,
            skip_torches: true,
            min_distance_percent: 20,
        }
    }
}

impl Settings {
    pub fn read(get: impl Fn(&str) -> Option<String>) -> Self {
        let default = Self::default();
        let boolean = |key: &str, fallback: bool| {
            get(key).map_or(fallback, |value| match value.trim() {
                "true" => true,
                "false" => false,
                _ => fallback,
            })
        };
        let distance = get("min_distance_percent")
            .and_then(|value| value.trim().parse::<u8>().ok())
            .filter(|value| (5..=80).contains(value))
            .unwrap_or(default.min_distance_percent);
        Self {
            enabled: boolean("enabled", default.enabled),
            crouch_only: boolean("crouch_only", default.crouch_only),
            replace_non_solid: boolean("replace_non_solid", default.replace_non_solid),
            skip_torches: boolean("skip_torches", default.skip_torches),
            min_distance_percent: distance,
        }
    }

    pub fn labels(self) -> Vec<String> {
        let on = |value: bool| if value { "ON" } else { "OFF" };
        vec![
            format!("Bridging Assist: {}", on(self.enabled)),
            format!("Crouch only: {}", on(self.crouch_only)),
            format!("Replace non-solid targets: {}", on(self.replace_non_solid)),
            format!("Skip torches: {}", on(self.skip_torches)),
            format!("Minimum bridge distance: {}%", self.min_distance_percent),
        ]
    }

    pub fn click(&mut self, button: usize) {
        match button {
            0 => self.enabled = !self.enabled,
            1 => self.crouch_only = !self.crouch_only,
            2 => self.replace_non_solid = !self.replace_non_solid,
            3 => self.skip_torches = !self.skip_torches,
            4 => {
                self.min_distance_percent = match self.min_distance_percent {
                    5 => 10,
                    10 => 20,
                    20 => 30,
                    30 => 40,
                    40 => 50,
                    50 => 60,
                    60 => 80,
                    _ => 5,
                };
            }
            _ => {}
        }
    }

    pub fn stored(self) -> [(&'static str, String); 5] {
        let flag = |value: bool| if value { "true" } else { "false" }.to_owned();
        [
            ("enabled", flag(self.enabled)),
            ("crouch_only", flag(self.crouch_only)),
            ("replace_non_solid", flag(self.replace_non_solid)),
            ("skip_torches", flag(self.skip_torches)),
            (
                "min_distance_percent",
                self.min_distance_percent.to_string(),
            ),
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
    if !reach.is_finite()
        || reach <= 0.0
        || !(5..=80).contains(&min_distance_percent)
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

    Some(voxel_line(cell(start)?, cell(end)?))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn stored<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        |key| pairs.iter().find(|pair| pair.0 == key).map(|pair| pair.1.to_owned())
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
        let pairs: Vec<(&str, String)> = settings.stored().into();
        let borrowed: Vec<(&str, &str)> = pairs
            .iter()
            .map(|(key, value)| (*key, value.as_str()))
            .collect();
        assert_eq!(Settings::read(stored(&borrowed)), settings);
    }

    #[test]
    fn invalid_values_fall_back_to_safe_defaults() {
        let settings = Settings::read(stored(&[
            ("enabled", "maybe"),
            ("crouch_only", "false"),
            ("min_distance_percent", "255"),
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
}
