// Accurate Block Placement for Horizon: hold right click with a block and
// every new spot you look at gets a block at once, however fast you move,
// instead of vanilla's one block every four ticks (which skips spots when you
// move fast and stacks extra blocks when you move slowly).
//
// A port of Accurate Block Placement: Reborn (MIT; Clayborn, Flourick,
// schwar and KadTheHunter), rewritten in Rust for Horizon. Its rules:
// - The first click places as vanilla does.
// - While the key is held, a block goes on each *fresh* target: a block you
//   weren't already looking at and didn't just place. Staring at one spot
//   places nothing more.
// - Right after the first click it waits out vanilla's four ticks (so a
//   single click stays a single block), unless you turn the view, and then
//   places what you looked at meanwhile.
// - Building backwards, toward yourself, the block you just placed counts as
//   fresh again once you have stepped a full block back from it.
//
// The game samples the crosshair every frame (`frame`) and places once a tick
// (`due`, `still_fresh`, `placed`), so a fast sweep loses no spot and the
// server sees placements tick by tick. This file is both the mod's Cargo
// library (its tests) and the module the bundle adds to the game
// (`crate::game::accurate_block_placement`). It knows nothing of the game's
// types: the hooks pass plain values in and out.
use std::cell::RefCell;

/// The mod's id: its settings folder and its key's line.
pub const ID: &str = "accurate_block_placement";
pub const VERSION: &str = "1.0.0";
pub const DESCRIPTION: &str =
    "Hold right click with a block and every new spot you look at gets one, however fast you move.";
/// The toggle key's name, and its default: unbound, as in the original.
pub const KEY: &str = "toggle";
pub const DEFAULT_KEY: &str = "key.keyboard.unknown";
/// Said in chat when the toggle key turns it on or off.
pub const ON: &str = "Accurate Block Placement: ON";
pub const OFF: &str = "Accurate Block Placement: OFF";

/// Vanilla's delay between repeated uses, in ticks (`Minecraft.rightClickDelay`).
const COOLDOWN: u8 = 4;
/// Turning the view this far (degrees) since the first click ends the wait.
const TURN: f32 = 10.0;
/// At most this many blocks go out in one tick, however many frames saw.
pub const MAX_PER_TICK: usize = 4;

/// The settings, as kept in `horizon-mods/.horizon-mod-data/accurate_block_placement/`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    pub enabled: bool,
    /// Whether it works on servers too (always in your own worlds). Some
    /// servers see fast, accurate placing as a cheat.
    pub servers: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: true,
            servers: true,
        }
    }
}

impl Settings {
    /// From the stored values (`get` reads one key's file); the defaults for
    /// the ones missing.
    pub fn read(get: impl Fn(&str) -> Option<String>) -> Self {
        let flag = |key: &str, default: bool| {
            get(key).map_or(default, |v| match v.trim() {
                "true" => true,
                "false" => false,
                _ => default,
            })
        };
        Self {
            enabled: flag("enabled", true),
            servers: flag("servers", true),
        }
    }
    pub fn labels(&self) -> Vec<String> {
        let on = |b: bool| if b { "ON" } else { "OFF" };
        vec![
            format!("Accurate placement: {}", on(self.enabled)),
            format!("Allowed on servers: {}", on(self.servers)),
        ]
    }
    pub fn click(&mut self, i: usize) {
        match i {
            0 => self.enabled = !self.enabled,
            1 => self.servers = !self.servers,
            _ => {}
        }
    }
    pub fn stored(&self) -> [(&'static str, String); 2] {
        let flag = |b: bool| if b { "true" } else { "false" }.to_owned();
        [("enabled", flag(self.enabled)), ("servers", flag(self.servers))]
    }
}

/// What the crosshair hits: the block, the face (a unit vector, `[0, 1, 0]`
/// the top), the point, and whether the block is replaceable (grass, snow
/// layers: a block placed on it takes its place instead of going beside it).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    pub pos: [i32; 3],
    pub face: [i32; 3],
    pub point: [f64; 3],
    pub replaceable: bool,
}

impl Sample {
    /// Where a block placed on this target goes.
    pub fn placement(&self) -> [i32; 3] {
        if self.replaceable {
            self.pos
        } else {
            [
                self.pos[0] + self.face[0],
                self.pos[1] + self.face[1],
                self.pos[2] + self.face[2],
            ]
        }
    }
    /// The axis the face points along (0 x, 1 y, 2 z).
    fn axis(&self) -> usize {
        self.face.iter().position(|&f| f != 0).unwrap_or(1)
    }
}

#[derive(Default)]
struct State {
    /// Read on first use, then kept.
    settings: Option<Settings>,
    /// Whether the use key was held at the last frame.
    held: bool,
    /// The block looked at last frame while the key was held.
    last_seen: Option<[i32; 3]>,
    /// Where the last block went.
    last_placed: Option<[i32; 3]>,
    /// The player's position when it went (stepped one block along the face
    /// at each later placement, so rounding never drifts).
    last_player: Option<[f64; 3]>,
    /// Ticks left of the wait after the first click.
    wait: u8,
    /// The view (yaw, pitch) at the first click.
    press_view: (f32, f32),
    /// Fresh targets seen since the last tick, in order.
    queue: Vec<Sample>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// The settings, read once through `get`.
pub fn settings(get: impl Fn(&str) -> Option<String>) -> Settings {
    STATE.with_borrow_mut(|s| *s.settings.get_or_insert_with(|| Settings::read(get)))
}

/// Replaces the settings (the settings page or the key changed them).
pub fn set_settings(settings: Settings) {
    STATE.with_borrow_mut(|s| s.settings = Some(settings));
}

/// Whether a hold is under way (the use key held with a block, on a target
/// vanilla would place on): vanilla's own repeat stays off meanwhile.
pub fn holding() -> bool {
    STATE.with_borrow(|s| s.held)
}

/// Forgets everything about the current hold.
pub fn reset() {
    STATE.with_borrow_mut(|s| {
        let settings = s.settings;
        *s = State::default();
        s.settings = settings;
    });
}

/// Whether `t` is a fresh target for state `s`, the player standing at `player`.
fn fresh(s: &State, t: &Sample, player: [f64; 3]) -> bool {
    let unseen = s.last_seen != Some(t.pos) && s.last_placed != Some(t.pos);
    unseen || backwards(s, t, player)
}

/// Building toward yourself: the target is the block you just placed, and
/// you have stepped a full block back from where you placed it.
fn backwards(s: &State, t: &Sample, player: [f64; 3]) -> bool {
    let (Some(placed), Some(then)) = (s.last_placed, s.last_player) else {
        return false;
    };
    if placed != t.pos {
        return false;
    }
    let a = t.axis();
    // The face's plane.
    let plane = f64::from(t.pos[a] + i32::from(t.face[a] > 0));
    (player[a] - then[a]).abs() >= 0.99 && (then[a] - plane).abs() < (player[a] - plane).abs()
}

/// One frame: `held` is whether the use key is held with a block to place
/// (and the mod on); `target` what the crosshair hits; `player` the player's
/// position and `view` its yaw and pitch.
pub fn frame(held: bool, target: Option<Sample>, player: [f64; 3], view: (f32, f32)) {
    STATE.with_borrow_mut(|s| {
        if !held {
            s.held = false;
            s.queue.clear();
            return;
        }
        if !s.held {
            // A fresh press: vanilla places this one on the next tick.
            let settings = s.settings;
            *s = State::default();
            s.settings = settings;
            s.held = true;
            s.wait = COOLDOWN;
            s.press_view = view;
            if let Some(t) = target {
                s.last_seen = Some(t.pos);
                s.last_placed = Some(t.placement());
                s.last_player = Some(player);
            }
            return;
        }
        let Some(t) = target else {
            return;
        };
        if fresh(s, &t, player) && !s.queue.iter().any(|q| q.pos == t.pos && q.face == t.face) {
            s.queue.push(t);
        }
        s.last_seen = Some(t.pos);
    });
}

/// One tick: the fresh targets to place now, oldest first. Empty while the
/// first click's wait runs and the view hasn't turned; those wait for later.
pub fn due(view: (f32, f32)) -> Vec<Sample> {
    STATE.with_borrow_mut(|s| {
        if !s.held {
            s.queue.clear();
            return Vec::new();
        }
        if s.wait > 0 {
            let (dy, dp) = (
                angle(view.0 - s.press_view.0).abs(),
                (view.1 - s.press_view.1).abs(),
            );
            if dy.hypot(dp) < TURN {
                s.wait -= 1;
                return Vec::new();
            }
            s.wait = 0;
        }
        let n = s.queue.len().min(MAX_PER_TICK);
        s.queue.drain(..n).collect()
    })
}

/// Whether `t`, due this tick, is still fresh after the blocks placed before
/// it this tick.
pub fn still_fresh(t: &Sample, player: [f64; 3]) -> bool {
    STATE.with_borrow(|s| s.last_placed != Some(t.pos) || backwards(s, t, player))
}

/// A block went on `t`, at `at`, the player standing at `player`.
pub fn placed(t: &Sample, at: [i32; 3], player: [f64; 3]) {
    STATE.with_borrow_mut(|s| {
        s.last_player = Some(match s.last_player {
            None => player,
            Some(then) => {
                // One block further along the face, from where the last one
                // went; the other axes where the player is now.
                let a = t.axis();
                let mut p = player;
                p[a] = then[a] + f64::from(t.face[a]);
                p
            }
        });
        s.last_placed = Some(at);
    });
}

/// `d` wrapped into -180..180 degrees.
fn angle(d: f32) -> f32 {
    (d + 180.0).rem_euclid(360.0) - 180.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stored<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        |key| pairs.iter().find(|p| p.0 == key).map(|p| p.1.to_owned())
    }

    /// The top of the block at `pos`.
    fn top(pos: [i32; 3]) -> Sample {
        Sample {
            pos,
            face: [0, 1, 0],
            point: [f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 1.0, f64::from(pos[2]) + 0.5],
            replaceable: false,
        }
    }

    const STILL: (f32, f32) = (0.0, 60.0);
    const AT: [f64; 3] = [0.5, 64.0, 0.5];

    /// Ticks until the first click's wait is over.
    fn wait_out() {
        for _ in 0..COOLDOWN {
            assert!(due(STILL).is_empty());
        }
    }

    #[test]
    fn settings_read_click_and_store() {
        assert_eq!(Settings::read(stored(&[])), Settings::default());
        let mut s = Settings::read(stored(&[("enabled", "false"), ("servers", "nope")]));
        assert_eq!(
            s,
            Settings {
                enabled: false,
                servers: true
            }
        );
        s.click(0);
        s.click(1);
        assert_eq!(
            s.labels(),
            ["Accurate placement: ON", "Allowed on servers: OFF"]
        );
        let round: Vec<(&str, String)> = s.stored().into();
        let back: Vec<(&str, &str)> = round.iter().map(|(k, v)| (*k, v.as_str())).collect();
        assert_eq!(Settings::read(stored(&back)), s);
    }

    #[test]
    fn placement_goes_beside_or_replaces() {
        assert_eq!(top([1, 2, 3]).placement(), [1, 3, 3]);
        let grass = Sample {
            replaceable: true,
            ..top([1, 2, 3])
        };
        assert_eq!(grass.placement(), [1, 2, 3]);
    }

    #[test]
    fn one_block_per_fresh_target_none_while_staring() {
        reset();
        // The press: vanilla places on [0,63,0]'s top.
        frame(true, Some(top([0, 63, 0])), AT, STILL);
        // Staring at the same spot: nothing, during and after the wait.
        for _ in 0..10 {
            frame(true, Some(top([0, 63, 0])), AT, STILL);
            assert!(due(STILL).is_empty());
        }
        // Looking at the block vanilla just placed isn't fresh either.
        frame(true, Some(top([0, 64, 0])), AT, STILL);
        assert!(due(STILL).is_empty());
        // A new spot is placed at once, and only once.
        frame(true, Some(top([1, 63, 0])), AT, STILL);
        frame(true, Some(top([1, 63, 0])), AT, STILL);
        let now = due(STILL);
        assert_eq!(now, vec![top([1, 63, 0])]);
        assert!(still_fresh(&now[0], AT));
        placed(&now[0], now[0].placement(), AT);
        frame(true, Some(top([1, 63, 0])), AT, STILL);
        assert!(due(STILL).is_empty());
        // Letting go forgets the hold.
        frame(false, Some(top([2, 63, 0])), AT, STILL);
        assert!(due(STILL).is_empty());
    }

    #[test]
    fn a_fast_sweep_loses_no_spot() {
        reset();
        frame(true, Some(top([0, 63, 0])), AT, STILL);
        wait_out();
        // Five spots between two ticks.
        for x in 1..=5 {
            frame(true, Some(top([x, 63, 0])), AT, STILL);
        }
        let first = due(STILL);
        assert_eq!(first.len(), MAX_PER_TICK);
        assert_eq!(first[0].pos, [1, 63, 0]);
        // The rest goes on the next tick.
        assert_eq!(due(STILL).iter().map(|t| t.pos).collect::<Vec<_>>(), [[5, 63, 0]]);
    }

    #[test]
    fn the_wait_keeps_a_click_single_unless_the_view_turns() {
        reset();
        frame(true, Some(top([0, 63, 0])), AT, STILL);
        frame(true, Some(top([1, 63, 0])), AT, STILL);
        // Waiting: nothing yet, but it isn't forgotten.
        assert!(due(STILL).is_empty());
        assert!(due(STILL).is_empty());
        assert!(due(STILL).is_empty());
        assert!(due(STILL).is_empty());
        assert_eq!(due(STILL).len(), 1, "placed once the wait is over");
        // Turning the view ends the wait at once.
        reset();
        frame(true, Some(top([0, 63, 0])), AT, STILL);
        frame(true, Some(top([1, 63, 0])), AT, (15.0, 60.0));
        assert_eq!(due((15.0, 60.0)).len(), 1);
    }

    #[test]
    fn building_backwards_needs_a_full_step_back() {
        reset();
        // Facing the east side (+x face) of the block at x 0, standing east of it.
        let side = |x: i32| Sample {
            pos: [x, 63, 0],
            face: [1, 0, 0],
            point: [f64::from(x) + 1.0, 63.5, 0.5],
            replaceable: false,
        };
        frame(true, Some(side(0)), [2.5, 64.0, 0.5], STILL);
        wait_out();
        // The press placed at x 1; now the crosshair is on it.
        frame(true, Some(side(1)), [2.6, 64.0, 0.5], STILL);
        assert!(due(STILL).is_empty(), "not before a step back");
        // A full block further east: fresh again.
        frame(true, Some(side(1)), [3.55, 64.0, 0.5], STILL);
        let now = due(STILL);
        assert_eq!(now, vec![side(1)]);
        placed(&now[0], now[0].placement(), [3.55, 64.0, 0.5]);
        // And the next one a block further still.
        frame(true, Some(side(2)), [3.9, 64.0, 0.5], STILL);
        assert!(due(STILL).is_empty());
        frame(true, Some(side(2)), [4.6, 64.0, 0.5], STILL);
        assert_eq!(due(STILL), vec![side(2)]);
    }

    #[test]
    fn a_target_used_this_tick_is_not_used_again() {
        reset();
        frame(true, Some(top([0, 63, 0])), AT, STILL);
        wait_out();
        frame(true, Some(top([1, 63, 0])), AT, STILL);
        let side = Sample {
            face: [0, 0, 1],
            ..top([1, 63, 0])
        };
        frame(true, Some(side), AT, STILL);
        // Two faces of one block were seen; the first placement makes the
        // second stale only if it went there.
        let now = due(STILL);
        assert_eq!(now.len(), 1, "one entry per block looked at: {now:?}");
        placed(&now[0], [1, 63, 0], AT);
        assert!(!still_fresh(&top([1, 63, 0]), AT));
    }

    #[test]
    fn angles_wrap() {
        assert_eq!(angle(350.0), -10.0);
        assert_eq!(angle(-350.0), 10.0);
    }
}
