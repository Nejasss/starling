// Bridging Assist for Starling.
//
// This is a Rust reimplementation of the observable reacharound-placement
// behavior of Bridging Mod (MIT). It deliberately starts with a small,
// testable subset: voxel traversal, support-face selection at the adapter,
// and persisted basic settings. No Fabric classes or Java code are shipped.

use std::cell::Cell;

pub const ID: &str = "bridging_assist";
pub const VERSION: &str = "0.1.0";
pub const DESCRIPTION: &str =
    "Find a supported placement cell just beyond the normal crosshair target.";

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
    /// A synthetic hit is only installed for the duration of one normal
    /// remote_use_item_on call. RETURN injection consumes this marker and
    /// restores tick_hit, including when the original method exits early.
    static OVERRIDE_ACTIVE: Cell<bool> = const { Cell::new(false) };
}

pub fn arm_override() {
    OVERRIDE_ACTIVE.with(|active| active.set(true));
}

pub fn take_override() -> bool {
    OVERRIDE_ACTIVE.with(|active| active.replace(false))
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
}
