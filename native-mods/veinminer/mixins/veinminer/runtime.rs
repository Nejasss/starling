// Native adapter for Horizon 028f60881678a9a0e3e198f693a82858fa475c76.
use super::{Game, Result, Vec3};
mod engine {
    // Original Rust implementation of the documented vein-mining behavior.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub type Position = [i32; 3];
pub type Actor = [i32; 4];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Shape {
    #[default]
    Shapeless,
    Tunnel,
    ThreeByThree,
    StaircaseUp,
    StaircaseDown,
}
impl Shape {
    pub fn label(self) -> &'static str {
        match self {
            Self::Shapeless => "Shapeless",
            Self::Tunnel => "Tunnel",
            Self::ThreeByThree => "3x3",
            Self::StaircaseUp => "Staircase Up",
            Self::StaircaseDown => "Staircase Down",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DropMode {
    #[default]
    Together,
    EachBlock,
}
impl DropMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Together => "Together",
            Self::EachBlock => "Each Block",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct Settings {
    pub enabled: bool,
    pub cooldown: i64,
    pub must_sneak: bool,
    pub max_chain: usize,
    pub need_correct_tool: bool,
    pub search_radius: i32,
    pub decrease_durability: bool,
    pub prevent_tool_breaking: bool,
    pub drop_mode: DropMode,
    pub hunger_per_block: f32,
    pub separate_group_mining: bool,
    pub permission_restricted: bool,
    pub shape: Shape,
    pub match_families: bool,
    pub match_crop_maturity: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: true,
            cooldown: 20,
            must_sneak: false,
            max_chain: 100,
            need_correct_tool: true,
            search_radius: 1,
            decrease_durability: true,
            prevent_tool_breaking: true,
            drop_mode: DropMode::Together,
            hunger_per_block: 0.0,
            separate_group_mining: false,
            permission_restricted: false,
            shape: Shape::Shapeless,
            match_families: true,
            match_crop_maturity: true,
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=4096).contains(&self.max_chain) {
            return Err("maxChain must be 1..4096 (including the original block)".into());
        }
        if !(1..=3).contains(&self.search_radius) {
            return Err("searchRadius must be 1..3".into());
        }
        if !(0..=72_000).contains(&self.cooldown) {
            return Err("cooldown must be 0..72000 ticks".into());
        }
        if !self.hunger_per_block.is_finite() || !(0.0..=40.0).contains(&self.hunger_per_block) {
            return Err("hungerPerBlock must be finite and 0..40".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct Overrides {
    pub cooldown: Option<i64>,
    pub must_sneak: Option<bool>,
    pub max_chain: Option<usize>,
    pub need_correct_tool: Option<bool>,
    pub search_radius: Option<i32>,
    pub decrease_durability: Option<bool>,
    pub prevent_tool_breaking: Option<bool>,
    pub hunger_per_block: Option<f32>,
    pub separate_group_mining: Option<bool>,
    pub permission_restricted: Option<bool>,
}
impl Overrides {
    fn apply(&self, settings: &mut Settings) {
        macro_rules! fields { ($($field:ident),*) => { $(if let Some(value) = self.$field { settings.$field = value; })* }; }
        fields!(
            cooldown,
            must_sneak,
            max_chain,
            need_correct_tool,
            search_radius,
            decrease_durability,
            prevent_tool_breaking,
            hunger_per_block,
            separate_group_mining,
            permission_restricted
        );
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Group {
    pub id: String,
    pub blocks: BTreeSet<String>,
    #[serde(default)]
    pub tools: BTreeSet<String>,
    #[serde(default)]
    pub overrides: Overrides,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub settings: Settings,
    pub blocks: BTreeSet<String>,
    pub groups: Vec<Group>,
    pub players: BTreeSet<String>,
}
fn id_valid(s: &str) -> bool {
    let Some((namespace, path)) = s.split_once(':') else {
        return false;
    };
    !namespace.is_empty()
        && !path.is_empty()
        && namespace
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_.-".contains(&b))
        && path
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"/_.-".contains(&b))
}
impl Config {
    pub fn from_json(source: &str) -> Result<Self, String> {
        let config: Self = serde_json::from_str(source).map_err(|e| e.to_string())?;
        config.validate()?;
        Ok(config)
    }
    pub fn validate(&self) -> Result<(), String> {
        self.settings.validate()?;
        if self.groups.len() > 256 || self.blocks.len() > 4096 {
            return Err("too many groups or blocks".into());
        }
        let mut ids = BTreeSet::new();
        for group in &self.groups {
            if group.id.is_empty() || !ids.insert(&group.id) {
                return Err(format!("empty/duplicate group id: {}", group.id));
            }
            if group.blocks.is_empty() || group.blocks.len() > 4096 || group.tools.len() > 4096 {
                return Err(format!("group {} has invalid block/tool count", group.id));
            }
            let mut settings = self.settings.clone();
            group.overrides.apply(&mut settings);
            settings
                .validate()
                .map_err(|e| format!("group {}: {e}", group.id))?;
        }
        for id in self.blocks.iter().chain(
            self.groups
                .iter()
                .flat_map(|g| g.blocks.iter().chain(&g.tools)),
        ) {
            if !id_valid(id) {
                return Err(format!(
                    "expected explicit namespaced block/item id, got {id:?}"
                ));
            }
        }
        Ok(())
    }
    pub fn action(&self, name: &str, tool: Option<&str>) -> Option<(Settings, BTreeSet<String>)> {
        let groups: Vec<_> = self
            .groups
            .iter()
            .filter(|g| g.blocks.contains(name))
            .collect();
        if groups.is_empty() && !self.blocks.contains(name) {
            return None;
        }
        let mut settings = self.settings.clone();
        if let Some(first) = groups.first() {
            first.overrides.apply(&mut settings);
        }
        let tools: BTreeSet<_> = groups.iter().flat_map(|g| &g.tools).collect();
        if !tools.is_empty()
            && tool.is_none_or(|id| !tools.iter().any(|allowed| allowed.as_str() == id))
        {
            return None;
        }
        let targets = if settings.separate_group_mining || groups.is_empty() {
            BTreeSet::from([name.to_owned()])
        } else {
            groups
                .iter()
                .flat_map(|g| g.blocks.iter().cloned())
                .collect()
        };
        Some((settings, targets))
    }
}
impl Default for Config {
    fn default() -> Self {
        let mut groups = Vec::new();
        for ore in [
            "coal", "copper", "iron", "gold", "redstone", "lapis", "diamond", "emerald",
        ] {
            groups.push(Group {
                id: format!("{ore}-ore"),
                blocks: {
                    let mut blocks = BTreeSet::from([
                        format!("minecraft:{ore}_ore"),
                        format!("minecraft:deepslate_{ore}_ore"),
                    ]);
                    if matches!(ore, "iron" | "gold" | "copper") {
                        blocks.insert(format!("minecraft:raw_{ore}_block"));
                    }
                    blocks
                },
                tools: BTreeSet::new(),
                overrides: Overrides::default(),
            });
        }
        for wood in [
            "oak", "spruce", "birch", "jungle", "acacia", "dark_oak", "mangrove", "cherry",
            "pale_oak",
        ] {
            groups.push(Group {
                id: format!("{wood}-logs"),
                blocks: [
                    format!("minecraft:{wood}_log"),
                    format!("minecraft:{wood}_wood"),
                ]
                .into(),
                tools: BTreeSet::new(),
                overrides: Overrides::default(),
            });
        }
        Self {
            settings: Settings::default(),
            blocks: [
                "minecraft:nether_quartz_ore".into(),
                "minecraft:nether_gold_ore".into(),
                "minecraft:ancient_debris".into(),
            ]
            .into(),
            groups,
            players: BTreeSet::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub name: String,
    pub state: u32,
    pub breakable: bool,
    pub plant: bool,
    pub growth: Option<(u32, u32)>,
}
#[derive(Clone, Debug)]
pub struct Player {
    pub actor: Actor,
    pub name: String,
    pub tick: i64,
    pub survival: bool,
    pub sneaking: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tool {
    pub item: String,
    pub remaining: Option<u32>,
}

pub trait Host {
    type Error;
    fn player(&mut self) -> Player;
    // Cardinal horizontal direction the player is facing (used for vertical-hit staircases).
    fn horizontal_forward(&self) -> Position;
    // None means unloaded/outside build bounds. This must never load a chunk.
    fn block(&self, position: Position) -> Option<Block>;
    fn tool(&self) -> Option<Tool>;
    fn can_harvest(&self, block: &Block) -> bool;
    fn has_food(&self) -> bool;
    // Must use the game's ordinary player-mining path, never /setblock.
    fn mine(&mut self, position: Position, preserve_durability: bool) -> Result<bool, Self::Error>;
    fn exhaust(&mut self, amount: f32);
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hit {
    pub position: Position,
    pub block: Block,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub planned: usize,
    pub extra_broken: usize,
    pub changed: usize,
    pub stopped: bool,
}

fn block_family(name: &str) -> Option<String> {
    let path = name.strip_prefix("minecraft:")?;
    for metal in ["iron", "gold", "copper"] {
        if path == format!("{metal}_ore")
            || path == format!("deepslate_{metal}_ore")
            || path == format!("raw_{metal}_block")
        {
            return Some(format!("metal:{metal}"));
        }
    }
    if let Some(ore) = path.strip_prefix("deepslate_") {
        if ore.ends_with("_ore") {
            return Some(format!("ore:{ore}"));
        }
    }
    path.ends_with("_ore").then(|| format!("ore:{path}"))
}

fn same_crop_maturity(origin: &Block, candidate: &Block, settings: &Settings) -> bool {
    if !settings.match_crop_maturity || origin.name != candidate.name {
        return true;
    }
    match (origin.growth, candidate.growth) {
        (Some((age_a, max_a)), Some((age_b, max_b))) => {
            (age_a >= max_a) == (age_b >= max_b)
        }
        _ => true,
    }
}

fn same_shapeless_family(
    origin: &Block,
    candidate: &Block,
    targets: &BTreeSet<String>,
    settings: &Settings,
) -> bool {
    if origin.name == candidate.name || targets.contains(&candidate.name) {
        return same_crop_maturity(origin, candidate, settings);
    }
    if !settings.match_families {
        return false;
    }
    if origin.plant && candidate.plant {
        return true;
    }
    matches!((block_family(&origin.name), block_family(&candidate.name)), (Some(a), Some(b)) if a == b)
}

fn face_axis(face: Position) -> Position {
    let axis = (0..3).max_by_key(|i| face[*i].abs()).unwrap_or(2);
    if face[axis] == 0 {
        [0, 0, 1]
    } else {
        let mut direction = [0, 0, 0];
        direction[axis] = face[axis].signum();
        direction
    }
}

// Hit.face is the outward normal of the face the ray entered. Liteminer's
// walkers advance in the opposite direction: away from the player and through
// the block, not back toward the player.
fn inward_direction(face: Position) -> Position {
    let normal = face_axis(face);
    [-normal[0], -normal[1], -normal[2]]
}

pub fn horizontal_direction_from_yaw(yaw: f32) -> Position {
    // Direction::fromYRot: nearest horizontal cardinal, including exact 45° ties.
    let quadrant = (f64::from(yaw) / 90.0 + 0.5).floor() as i32 & 3;
    match quadrant {
        0 => [0, 0, 1],
        1 => [-1, 0, 0],
        2 => [0, 0, -1],
        _ => [1, 0, 0],
    }
}

fn offset(position: Position, delta: Position) -> Option<Position> {
    Some([
        position[0].checked_add(delta[0])?,
        position[1].checked_add(delta[1])?,
        position[2].checked_add(delta[2])?,
    ])
}

fn candidate_block<H: Host>(host: &H, position: Position, settings: &Settings) -> Option<Block> {
    let block = host.block(position)?;
    if !block.breakable || settings.need_correct_tool && !host.can_harvest(&block) {
        return None;
    }
    Some(block)
}

fn push_candidate<H: Host>(
    host: &H,
    position: Position,
    settings: &Settings,
    output: &mut Vec<Hit>,
    seen: &mut BTreeSet<Position>,
) -> bool {
    let Some(block) = candidate_block(host, position, settings) else {
        return false;
    };
    if seen.insert(position) {
        output.push(Hit { position, block });
    }
    true
}

fn geometric_preview<H: Host>(
    host: &H,
    origin: Position,
    settings: &Settings,
    face: Position,
    horizontal_forward: Position,
) -> Vec<Hit> {
    let limit = settings.max_chain;
    let mut output = Vec::with_capacity(limit.min(256));
    let mut seen = BTreeSet::new();
    if !push_candidate(host, origin, settings, &mut output, &mut seen) || limit <= 1 {
        return output;
    }

    let inward = inward_direction(face);
    match settings.shape {
        Shape::Shapeless => {}
        Shape::ThreeByThree => {
            // Match Liteminer's 3x3 plane: horizontal when the ray hit a top
            // or bottom face; otherwise vertical, perpendicular to the ray.
            let offsets: Vec<Position> = if inward[1] != 0 {
                vec![
                    [0, 0, -1], [1, 0, -1], [-1, 0, -1],
                    [1, 0, 0], [-1, 0, 0],
                    [0, 0, 1], [1, 0, 1], [-1, 0, 1],
                ]
            } else {
                // The horizontal vector is the ray direction. Its
                // counter-clockwise perpendicular defines the left side.
                let left = [inward[2], 0, -inward[0]];
                let right = [-left[0], 0, -left[2]];
                vec![
                    [left[0], 1, left[2]], [0, 1, 0], [right[0], 1, right[2]],
                    left, right,
                    [left[0], -1, left[2]], [0, -1, 0], [right[0], -1, right[2]],
                ]
            };
            for delta in offsets {
                if output.len() >= limit {
                    break;
                }
                if let Some(position) = offset(origin, delta) {
                    push_candidate(host, position, settings, &mut output, &mut seen);
                }
            }
        }
        Shape::Tunnel => {
            // Liteminer's Small Tunnel is one block wide and advances exactly
            // one block along the ray. It is not a two-block-high column.
            let mut cursor = origin;
            while output.len() < limit {
                let Some(next) = offset(cursor, inward) else { break; };
                if !push_candidate(host, next, settings, &mut output, &mut seen) {
                    break;
                }
                cursor = next;
            }
        }
        Shape::StaircaseUp | Shape::StaircaseDown => {
            // When the player targets a top/bottom face, Liteminer uses the
            // player's horizontal facing direction for staircase progression.
            let forward = if inward[1] != 0 { horizontal_forward } else { inward };
            let rise = if settings.shape == Shape::StaircaseUp { 1 } else { -1 };
            let mut cursor = origin;
            while output.len() < limit {
                let candidates: Vec<Position> = if rise > 0 {
                    vec![cursor, offset(cursor, [0, -1, 0]).unwrap_or(cursor), offset(cursor, [0, 1, 0]).unwrap_or(cursor)]
                } else {
                    vec![cursor, offset(cursor, [0, -1, 0]).unwrap_or(cursor), offset(cursor, [0, -2, 0]).unwrap_or(cursor)]
                };
                let mut any_valid = false;
                for position in candidates {
                    any_valid |= push_candidate(host, position, settings, &mut output, &mut seen);
                    if output.len() >= limit {
                        break;
                    }
                }
                // Liteminer stops when an entire step has no eligible blocks;
                // it does not skip gaps and resume farther away.
                if !any_valid || output.len() >= limit {
                    break;
                }
                let Some(mut next) = offset(cursor, forward) else { break; };
                next[1] = match next[1].checked_add(rise) {
                    Some(value) => value,
                    None => break,
                };
                cursor = next;
            }
        }
    }
    output
}

pub fn preview<H: Host>(
    host: &H,
    origin: Position,
    targets: &BTreeSet<String>,
    settings: &Settings,
) -> Vec<Hit> {
    preview_with_face(host, origin, targets, settings, [0, 1, 0])
}

pub fn preview_with_face<H: Host>(
    host: &H,
    origin: Position,
    targets: &BTreeSet<String>,
    settings: &Settings,
    face: Position,
) -> Vec<Hit> {
    preview_with_orientation(host, origin, targets, settings, Some(face))
}

fn preview_with_orientation<H: Host>(
    host: &H,
    origin: Position,
    targets: &BTreeSet<String>,
    settings: &Settings,
    face: Option<Position>,
) -> Vec<Hit> {
    if settings.validate().is_err() {
        return Vec::new();
    }
    if settings.shape != Shape::Shapeless {
        let Some(face) = face else { return Vec::new(); };
        return geometric_preview(host, origin, settings, face, host.horizontal_forward());
    }

    let Some(origin_block) = host.block(origin) else { return Vec::new(); };
    if !origin_block.breakable || !targets.contains(&origin_block.name) {
        return Vec::new();
    }
    let mut hits = Vec::new();
    let mut seen = BTreeSet::from([origin]);
    let mut queue = VecDeque::from([origin]);
    // This bound caps reads of nonmatching neighbours as well as accepted blocks.
    let max_examined = settings.max_chain.saturating_mul(343).min(1_000_000);
    while let Some(position) = queue.pop_front() {
        if hits.len() >= settings.max_chain {
            break;
        }
        let Some(block) = host.block(position) else { continue; };
        if !block.breakable
            || !same_shapeless_family(&origin_block, &block, targets, settings)
            || settings.need_correct_tool && !host.can_harvest(&block)
        {
            continue;
        }
        hits.push(Hit { position, block });
        let r = settings.search_radius;
        for x in -r..=r {
            for y in -r..=r {
                for z in -r..=r {
                    let (Some(x), Some(y), Some(z)) = (
                        position[0].checked_add(x),
                        position[1].checked_add(y),
                        position[2].checked_add(z),
                    ) else { continue; };
                    let next = [x, y, z];
                    if seen.len() < max_examined && seen.insert(next) {
                        queue.push_back(next);
                    }
                }
            }
        }
    }
    hits
}

#[derive(Default)]
pub struct Runtime {
    cooldowns: BTreeMap<Actor, i64>,
    disabled: BTreeSet<Actor>,
}
impl Runtime {
    pub fn enabled(&self, actor: Actor) -> bool {
        !self.disabled.contains(&actor)
    }
    pub fn set_enabled(&mut self, actor: Actor, enabled: bool) {
        if enabled {
            self.disabled.remove(&actor);
        } else {
            self.disabled.insert(actor);
        }
    }
    pub fn mine<H: Host>(
        &mut self,
        host: &mut H,
        config: &Config,
        origin: Position,
    ) -> Result<Report, H::Error> {
        self.mine_with_face(host, config, origin, Some([0, 1, 0]))
    }

    pub fn mine_with_face<H: Host>(
        &mut self,
        host: &mut H,
        config: &Config,
        origin: Position,
        face: Option<Position>,
    ) -> Result<Report, H::Error> {
        let player = host.player();
        let tool = host.tool();
        let seed = host.block(origin);
        let action = seed.as_ref().filter(|b| b.breakable).and_then(|b| {
            if config.settings.shape == Shape::Shapeless {
                config.action(&b.name, tool.as_ref().map(|t| t.item.as_str()))
            } else if config.settings.enabled {
                // Geometric Liteminer shapes are selected by the clicked face,
                // not by matching the origin against the shapeless whitelist.
                Some((config.settings.clone(), BTreeSet::new()))
            } else {
                None
            }
        });
        let action = action.filter(|(settings, _)| {
            settings.enabled
                && player.survival
                && self.enabled(player.actor)
                && (!settings.must_sneak || player.sneaking)
                && (!settings.permission_restricted || config.players.contains(&player.name))
                // Keep the legacy cooldown for shapeless veins only. Liteminer geometric
                // shapes are evaluated once for each player block-break action; applying
                // this delay makes the next 3x3/tunnel/staircase click break only its origin.
                && (settings.shape != Shape::Shapeless
                    || self
                        .cooldowns
                        .get(&player.actor)
                        .is_none_or(|end| player.tick >= *end))
        });
        let hits = if let Some((settings, targets)) = &action {
            preview_with_orientation(host, origin, targets, settings, face)
        } else {
            Vec::new()
        };
        let mut report = Report {
            planned: hits.len(),
            ..Report::default()
        };
        // The original click always follows Horizon's original behavior, even if the mod is off.
        if !host.mine(origin, false)? || hits.len() <= 1 {
            return Ok(report);
        }
        let Some((settings, _)) = action else {
            return Ok(report);
        };
        if settings.shape == Shape::Shapeless {
            self.cooldowns.retain(|_, end| *end > player.tick);
            self.cooldowns
                .insert(player.actor, player.tick.saturating_add(settings.cooldown));
        }
        for hit in hits.into_iter().skip(1) {
            if host.tool() != tool
                && host.tool().as_ref().map(|t| &t.item) != tool.as_ref().map(|t| &t.item)
            {
                report.stopped = true;
                break;
            }
            if (settings.prevent_tool_breaking
                && settings.decrease_durability
                && host
                    .tool()
                    .as_ref()
                    .and_then(|t| t.remaining)
                    .is_some_and(|remaining| remaining <= 2))
                || (settings.hunger_per_block > 0.0 && !host.has_food())
            {
                report.stopped = true;
                break;
            }
            if host.block(hit.position).as_ref() != Some(&hit.block) {
                report.changed += 1;
                continue;
            }
            if settings.need_correct_tool && !host.can_harvest(&hit.block) {
                report.stopped = true;
                break;
            }
            if !host.mine(hit.position, !settings.decrease_durability)? {
                report.stopped = true;
                break;
            }
            host.exhaust(settings.hunger_per_block);
            report.extra_broken += 1;
        }
        Ok(report)
    }
}

}
use engine::{Block, Config, DropMode, Host, Player, Position, Runtime, Shape, Tool};

#[derive(Default)]
pub(super) struct State {
    runtime: Runtime,
    config: Option<Config>,
}

thread_local! {
    static STATES: std::cell::RefCell<std::collections::BTreeMap<std::path::PathBuf, State>> = const { std::cell::RefCell::new(std::collections::BTreeMap::new()) };
}
fn with_state<T>(game: &mut Game<'_>, run: impl FnOnce(&mut Game<'_>, &mut State) -> T) -> T {
    struct Lease {
        key: std::path::PathBuf,
        state: State,
    }
    impl Drop for Lease {
        fn drop(&mut self) {
            STATES.with(|states| {
                states
                    .borrow_mut()
                    .insert(self.key.clone(), std::mem::take(&mut self.state));
            });
        }
    }
    let key = game.world_root.clone();
    let state = STATES.with(|states| states.borrow_mut().remove(&key).unwrap_or_default());
    let mut lease = Lease { key, state };
    run(game, &mut lease.state)
}

fn config_path() -> Option<std::path::PathBuf> {
    std::env::var_os("HORIZON_VEINMINER_CONFIG").map(Into::into)
}
fn load_config() -> std::result::Result<Config, String> {
    match config_path() {
        Some(path) => Config::from_json(
            &std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?,
        ),
        None => Ok(Config::default()),
    }
}

struct Native<'g, 'r> {
    game: &'g mut Game<'r>,
    origin: Position,
    drop_mode: DropMode,
}
impl Host for Native<'_, '_> {
    type Error = Box<dyn std::error::Error>;
    fn player(&mut self) -> Player {
        Player {
            actor: self.game.acting_player_uuid(),
            name: self
                .game
                .lan_acting_name
                .as_deref()
                .unwrap_or(super::chat::PLAYER_NAME)
                .to_owned(),
            tick: self.game.world.game_time,
            survival: self.game.remote.is_none()
                && self.game.player.is_some()
                && self.game.may_build()
                && !self.game.creative,
            sneaking: self
                .game
                .keys
                .contains(&winit::keyboard::KeyCode::ShiftLeft),
        }
    }
    fn block(&self, p: Position) -> Option<Block> {
        if p[1] < self.game.world.shape.min_y
            || p[1] >= self.game.world.shape.max_y()
            || !self.game.world.loaded(p[0], p[2])
        {
            return None;
        }
        let state = self.game.world.block(p[0], p[1], p[2]);
        let facts = self.game.res.facts().get(state);
        let name = facts.name.to_string();
        let plant = facts.classes.iter().any(|class| {
            matches!(class.as_ref(), "FlowerBlock" | "TallGrassBlock" | "DoublePlantBlock")
        });
        let growth = ["age", "stage"].into_iter().find_map(|property| {
            let value = self.game.native_block_property(p, property)?;
            let values = self.game.pack.assets.registry.property_values(&name, property);
            let max = values.len().checked_sub(1)?;
            let current = values.iter().position(|candidate| *candidate == value.as_str())?;
            Some((current as u32, max as u32))
        });
        Some(Block {
            name,
            state: state.0,
            breakable: facts.hardness >= 0.0
                && !self.game.world.is_air(state)
                && facts.fluid == horizon_client::blocks::Fluid::None,
            plant,
            growth,
        })
    }
    fn horizontal_forward(&self) -> Position {
        engine::horizontal_direction_from_yaw(self.game.interaction_rotation().0)
    }
    fn tool(&self) -> Option<Tool> {
        let stack = self.game.held()?;
        let max = self
            .game
            .res
            .items()
            .get(&stack.item)
            .map_or(0, |i| i.max_damage);
        Some(Tool {
            item: stack.item.to_string(),
            remaining: (max > 0).then_some(max.saturating_sub(stack.damage)),
        })
    }
    fn can_harvest(&self, block: &Block) -> bool {
        let facts = self
            .game
            .res
            .facts()
            .get(horizon_world::registry::BlockStateId(block.state));
        !facts.requires_tool
            || self
                .game
                .res
                .items()
                .correct_for_drops(self.game.held(), &block.name)
    }
    fn has_food(&self) -> bool {
        self.game
            .player
            .as_ref()
            .is_some_and(|p| p.health.food.food > 0 || p.health.food.saturation > 0.0)
    }
    fn mine(&mut self, p: Position, preserve: bool) -> Result<bool> {
        let before = self.block(p);
        let together = self.drop_mode == DropMode::Together && p != self.origin;
        let dropped_before = self.game.dropped.len();
        let orbs_before: Vec<(u32, (u32, u32, u32))> = if together {
            self.game
                .orbs
                .iter()
                .map(|orb| (orb.id, (orb.count, orb.value, orb.age)))
                .collect()
        } else {
            Vec::new()
        };
        let held = preserve.then(|| self.game.inventory[self.game.selected].clone());
        // Temporarily remove pre-existing damage, then restore damage (and a tool
        // destroyed by this one operation) without changing other mining effects.
        if preserve && let Some(stack) = self.game.inventory[self.game.selected].as_mut() {
            stack.damage = 0;
        }
        let result = self.game.break_block_as_player(p);
        if let Some(original) = held {
            match (self.game.inventory[self.game.selected].as_mut(), original) {
                (Some(now), Some(was)) if now.item == was.item => now.damage = was.damage,
                (None, Some(was)) => self.game.inventory[self.game.selected] = Some(was),
                _ => {}
            }
        }
        result?;
        if together {
            let target = self.origin.map(|coordinate| f64::from(coordinate) + 0.5);
            for item in self.game.dropped.iter_mut().skip(dropped_before) {
                item.position = Vec3::new(target[0], target[1], target[2]);
                item.previous = Vec3::new(target[0], target[1], target[2]);
                item.needs_sync = true;
            }
            for orb in &mut self.game.orbs {
                let previous = orbs_before
                    .iter()
                    .find(|(id, _)| *id == orb.id)
                    .map(|(_, state)| *state);
                let current = (orb.count, orb.value, orb.age);
                if previous != Some(current) {
                    orb.position = Vec3::new(target[0], target[1], target[2]);
                    orb.previous = Vec3::new(target[0], target[1], target[2]);
                }
            }
        }
        Ok(before.is_some() && self.block(p) != before)
    }
    fn exhaust(&mut self, amount: f32) {
        if amount > 0.0
            && let Some(p) = self.game.player.as_mut()
        {
            p.cause_food_exhaustion(amount);
        }
    }
}
impl Game<'_> {
    pub(super) fn veinminer_break_block(&mut self, pos: Position) -> Result<()> {
        with_state(self, |game, state| {
            if state.config.is_none() {
                state.config = Some(load_config().unwrap_or_else(|error| {
                    eprintln!("Veinminer disabled: {error}");
                    let mut config = Config::default();
                    config.settings.enabled = false;
                    config
                }));
            }
            // Prefer the hit that actually matches the block being mined.
            // tick_hit/forced_hit can briefly refer to a stale target; retry
            // the current aim ray before giving up on geometric expansion.
            let hit = game.pick().filter(|hit| hit.pos == pos).or_else(|| {
                let (origin, direction) = game.aim()?;
                game.clip_ray(origin, direction, game.block_reach).filter(|hit| hit.pos == pos)
            });
            let face = hit.map(|hit| hit.face);
            let config = state.config.as_ref().unwrap();
            let drop_mode = config.settings.drop_mode;
            state
                .runtime
                .mine_with_face(
                    &mut Native { game, origin: pos, drop_mode },
                    config,
                    pos,
                    face,
                )
                .map(|_| ())
        })
    }
    pub(super) fn veinminer_command(&mut self, command: &str) -> Option<bool> {
        with_state(self, |game, state| {
            let mut words = command.split_whitespace();
            if words.next()? != "veinminer" {
                return None;
            }
            let args: Vec<_> = words.collect();
            let actor = game.acting_player_uuid();
            let message = match args.as_slice() {
                [] | ["status"] => format!(
                    "Veinminer: {} | Shape: {} | Drops: {} | Tool safety: {}",
                    if state.runtime.enabled(actor) { "on" } else { "off" },
                    state.config.as_ref().map_or("Shapeless", |c| c.settings.shape.label()),
                    state.config.as_ref().map_or("Together", |c| c.settings.drop_mode.label()),
                    if state.config.as_ref().is_none_or(|c| c.settings.prevent_tool_breaking) { "on" } else { "off" }
                ),
                ["shape", name] => {
                    let shape = match *name {
                        "shapeless" => Shape::Shapeless,
                        "tunnel" => Shape::Tunnel,
                        "3x3" | "three_by_three" => Shape::ThreeByThree,
                        "up" | "staircase_up" => Shape::StaircaseUp,
                        "down" | "staircase_down" => Shape::StaircaseDown,
                        _ => {
                            game.command_failure(horizon_client::text::Text::literal(
                                "Usage: /veinminer shape [shapeless|tunnel|3x3|up|down]",
                            ));
                            return Some(false);
                        }
                    };
                    if state.config.is_none() {
                        match load_config() {
                            Ok(config) => state.config = Some(config),
                            Err(error) => {
                                game.command_failure(horizon_client::text::Text::literal(error));
                                return Some(false);
                            }
                        }
                    }
                    state.config.as_mut().unwrap().settings.shape = shape;
                    format!("VeinMiner shape: {}", shape.label())
                },
                ["drops", name] => {
                    let drop_mode = match *name {
                        "together" => DropMode::Together,
                        "each" | "each_block" => DropMode::EachBlock,
                        _ => {
                            game.command_failure(horizon_client::text::Text::literal(
                                "Usage: /veinminer drops [together|each]",
                            ));
                            return Some(false);
                        }
                    };
                    if state.config.is_none() {
                        match load_config() {
                            Ok(config) => state.config = Some(config),
                            Err(error) => {
                                game.command_failure(horizon_client::text::Text::literal(error));
                                return Some(false);
                            }
                        }
                    }
                    state.config.as_mut().unwrap().settings.drop_mode = drop_mode;
                    format!("VeinMiner drops: {}", drop_mode.label())
                }
                ["tool-safety", mode] => {
                    let enabled = match *mode {
                        "on" => true,
                        "off" => false,
                        _ => {
                            game.command_failure(horizon_client::text::Text::literal(
                                "Usage: /veinminer tool-safety [on|off]",
                            ));
                            return Some(false);
                        }
                    };
                    if state.config.is_none() {
                        match load_config() {
                            Ok(config) => state.config = Some(config),
                            Err(error) => {
                                game.command_failure(horizon_client::text::Text::literal(error));
                                return Some(false);
                            }
                        }
                    }
                    state.config.as_mut().unwrap().settings.prevent_tool_breaking = enabled;
                    format!("VeinMiner tool safety: {}", if enabled { "on" } else { "off" })
                }
                ["toggle"] | ["on"] | ["off"] => {
                    let on = match args[0] {
                        "on" => true,
                        "off" => false,
                        _ => !state.runtime.enabled(actor),
                    };
                    state.runtime.set_enabled(actor, on);
                    format!("Veinminer: {}", if on { "on" } else { "off" })
                }
                ["reload"] => {
                    if game.ui.registries.permission < 2 {
                        game.command_failure(horizon_client::text::Text::literal(
                            "Veinminer reload requires permission level 2",
                        ));
                        return Some(false);
                    }
                    match load_config() {
                        Ok(config) => {
                            state.config = Some(config);
                            "Veinminer configuration reloaded".into()
                        }
                        Err(error) => {
                            game.command_failure(horizon_client::text::Text::literal(error));
                            return Some(false);
                        }
                    }
                }
                _ => {
                    game.command_failure(horizon_client::text::Text::literal(
                        "Usage: /veinminer [status|toggle|on|off|reload|shape <shapeless|tunnel|3x3|up|down>|drops <together|each>|tool-safety <on|off>]",
                    ));
                    return Some(false);
                }
            };
            game.system_message(horizon_client::text::Text::literal(message));
            Some(true)
        })
    }
}

#[cfg(test)]
mod engine_tests {
    use super::engine as horizon_veinminer;
    use horizon_veinminer::*;
use std::collections::{BTreeMap, BTreeSet};

struct World {
    blocks: BTreeMap<Position, Block>,
    player: Player,
    tool: Option<Tool>,
    forward: Position,
    food: bool,
    harvest: bool,
    calls: Vec<(Position, bool)>,
    drops: usize,
    xp: usize,
    exhaustion: f32,
    failed: BTreeSet<Position>,
    error: Option<Position>,
    change_after_origin: Option<Position>,
}
impl World {
    fn new(points: &[Position]) -> Self {
        Self {
            blocks: points
                .iter()
                .map(|p| (*p, block("minecraft:iron_ore", 1)))
                .collect(),
            player: Player {
                actor: [0, 0, 0, 1],
                name: "Alex".into(),
                tick: 0,
                survival: true,
                sneaking: false,
            },
            tool: Some(Tool {
                item: "minecraft:iron_pickaxe".into(),
                remaining: Some(1000),
            }),
            forward: [0, 0, 1],
            food: true,
            harvest: true,
            calls: Vec::new(),
            drops: 0,
            xp: 0,
            exhaustion: 0.0,
            failed: BTreeSet::new(),
            error: None,
            change_after_origin: None,
        }
    }
}
fn block(name: &str, state: u32) -> Block {
    Block {
        name: name.into(),
        state,
        breakable: true,
        plant: false,
        growth: None,
    }
}
impl Host for World {
    type Error = &'static str;
    fn player(&mut self) -> Player {
        self.player.clone()
    }
    fn block(&self, p: Position) -> Option<Block> {
        self.blocks.get(&p).cloned()
    }
    fn tool(&self) -> Option<Tool> {
        self.tool.clone()
    }
    fn horizontal_forward(&self) -> Position {
        self.forward
    }
    fn can_harvest(&self, _: &Block) -> bool {
        self.harvest
    }
    fn has_food(&self) -> bool {
        self.food
    }
    fn mine(&mut self, p: Position, preserve: bool) -> Result<bool, Self::Error> {
        self.calls.push((p, preserve));
        if self.error == Some(p) {
            return Err("native mining failed");
        }
        if self.failed.contains(&p) {
            return Ok(false);
        }
        let removed = self.blocks.remove(&p).is_some();
        if removed {
            self.drops += 1;
            self.xp += 2;
            if !preserve
                && let Some(t) = self.tool.as_mut()
                && let Some(n) = t.remaining.as_mut()
            {
                *n = n.saturating_sub(1);
                if *n == 0 {
                    self.tool = None;
                }
            }
        }
        if self.calls.len() == 1
            && let Some(p) = self.change_after_origin
        {
            self.blocks.insert(p, block("minecraft:diamond_ore", 99));
        }
        Ok(removed)
    }
    fn exhaust(&mut self, n: f32) {
        self.exhaustion += n;
    }
}
fn mine(world: &mut World, config: &Config) -> Report {
    Runtime::default().mine(world, config, [0, 0, 0]).unwrap()
}

#[test]
fn connected_diagonals_group_variants_and_original_once_keep_drops_and_xp() {
    let mut w = World::new(&[[0, 0, 0], [1, 1, 1], [2, 1, 1], [20, 0, 0]]);
    w.blocks
        .insert([2, 1, 1], block("minecraft:deepslate_iron_ore", 2));
    let report = mine(&mut w, &Config::default());
    assert_eq!((report.planned, report.extra_broken), (3, 2));
    assert_eq!((w.calls.len(), w.drops, w.xp), (3, 3, 6));
    assert_eq!(w.calls.iter().filter(|(p, _)| *p == [0, 0, 0]).count(), 1);
    assert!(w.blocks.contains_key(&[20, 0, 0]));
    assert_eq!(w.tool.unwrap().remaining, Some(997));
}
#[test]
fn plan_is_pure_deterministic_bounded_and_max_chain_includes_origin() {
    let points: Vec<_> = (-8..=8)
        .flat_map(|x| (-8..=8).map(move |z| [x, 0, z]))
        .collect();
    let mut w = World::new(&points);
    let mut c = Config::default();
    c.settings.max_chain = 7;
    let targets = BTreeSet::from(["minecraft:iron_ore".into()]);
    let a = preview(&w, [0, 0, 0], &targets, &c.settings);
    let b = preview(&w, [0, 0, 0], &targets, &c.settings);
    assert_eq!(a, b);
    assert_eq!(a.len(), 7);
    assert_eq!(w.calls.len(), 0);
    assert_eq!(mine(&mut w, &c).extra_broken, 6);
    assert_eq!(w.calls.len(), 7);
}
#[test]
fn loaded_matching_blocks_only_no_generated_chunks_or_unbreakable_bridge() {
    let mut w = World::new(&[[0, 0, 0], [2, 0, 0]]);
    w.blocks.insert([1, 0, 0], block("minecraft:stone", 2));
    assert_eq!(mine(&mut w, &Config::default()).extra_broken, 0);
    let mut w = World::new(&[[0, 0, 0], [1, 0, 0], [2, 0, 0]]);
    w.blocks.get_mut(&[1, 0, 0]).unwrap().breakable = false;
    assert_eq!(mine(&mut w, &Config::default()).extra_broken, 0);
    let mut w = World::new(&[[0, 0, 0], [2, 0, 0]]);
    let mut c = Config::default();
    c.settings.search_radius = 2;
    assert_eq!(mine(&mut w, &c).extra_broken, 1);
}
#[test]
fn last_durability_point_is_kept_and_broken_initial_tool_stops_chain() {
    let mut w = World::new(&[[0, 0, 0], [1, 0, 0], [2, 0, 0], [3, 0, 0]]);
    w.tool.as_mut().unwrap().remaining = Some(3);
    let r = mine(&mut w, &Config::default());
    assert_eq!(r.extra_broken, 1);
    assert!(r.stopped);
    assert_eq!(w.tool.unwrap().remaining, Some(1));
    let mut w = World::new(&[[0, 0, 0], [1, 0, 0]]);
    w.tool.as_mut().unwrap().remaining = Some(1);
    assert_eq!(mine(&mut w, &Config::default()).extra_broken, 0);
    assert!(w.tool.is_none());
}
#[test]
fn preservation_and_additional_exhaustion_only_apply_to_extra_blocks() {
    let mut w = World::new(&[[0, 0, 0], [1, 0, 0], [2, 0, 0]]);
    w.tool.as_mut().unwrap().remaining = Some(2);
    let mut c = Config::default();
    c.settings.decrease_durability = false;
    c.settings.hunger_per_block = 0.5;
    assert_eq!(mine(&mut w, &c).extra_broken, 2);
    assert_eq!(
        w.calls,
        vec![([0, 0, 0], false), ([1, 0, 0], true), ([2, 0, 0], true)]
    );
    assert_eq!(w.tool.unwrap().remaining, Some(1));
    assert_eq!(w.exhaustion, 1.0);
}
#[test]
fn stale_blocks_are_not_mined_even_when_the_new_block_is_whitelisted() {
    let mut w = World::new(&[[0, 0, 0], [1, 0, 0], [2, 0, 0]]);
    w.change_after_origin = Some([1, 0, 0]);
    let r = mine(&mut w, &Config::default());
    assert_eq!((r.changed, r.extra_broken), (1, 1));
    assert_eq!(w.blocks[&[1, 0, 0]].name, "minecraft:diamond_ore");
}
#[test]
fn original_failure_never_triggers_extras_and_extra_failure_aborts() {
    let mut w = World::new(&[[0, 0, 0], [1, 0, 0], [2, 0, 0]]);
    w.failed.insert([0, 0, 0]);
    assert_eq!(mine(&mut w, &Config::default()).extra_broken, 0);
    assert_eq!(w.calls.len(), 1);
    let mut w = World::new(&[[0, 0, 0], [1, 0, 0], [2, 0, 0]]);
    w.failed.insert([1, 0, 0]);
    let r = mine(&mut w, &Config::default());
    assert!(r.stopped);
    assert_eq!(w.calls.len(), 2);
}
#[test]
fn native_errors_propagate_without_attempting_later_blocks() {
    let mut w = World::new(&[[0, 0, 0], [1, 0, 0], [2, 0, 0]]);
    w.error = Some([1, 0, 0]);
    assert_eq!(
        Runtime::default().mine(&mut w, &Config::default(), [0, 0, 0]),
        Err("native mining failed")
    );
    assert_eq!(w.calls.len(), 2);
}
#[test]
fn creative_adventure_sneak_tool_food_permissions_and_global_disable_gate_only_extras() {
    for gate in 0..7 {
        let mut w = World::new(&[[0, 0, 0], [1, 0, 0]]);
        let mut c = Config::default();
        match gate {
            0 => w.player.survival = false,
            1 => c.settings.must_sneak = true,
            2 => w.harvest = false,
            3 => {
                c.settings.hunger_per_block = 0.2;
                w.food = false;
            }
            4 => c.settings.permission_restricted = true,
            5 => c.settings.enabled = false,
            _ => {
                c.groups.clear();
                c.blocks.clear();
            }
        }
        assert_eq!(mine(&mut w, &c).extra_broken, 0, "gate {gate}");
        assert_eq!(w.calls.len(), 1);
    }
    let mut w = World::new(&[[0, 0, 0], [1, 0, 0]]);
    w.player.sneaking = true;
    let mut c = Config::default();
    c.settings.must_sneak = true;
    c.settings.permission_restricted = true;
    c.players.insert("Alex".into());
    assert_eq!(mine(&mut w, &c).extra_broken, 1);
}
#[test]
fn cooldown_and_toggles_are_per_player_and_original_mining_is_never_cancelled() {
    let mut rt = Runtime::default();
    let c = Config::default();
    let mut w = World::new(&[[0, 0, 0], [1, 0, 0]]);
    assert_eq!(rt.mine(&mut w, &c, [0, 0, 0]).unwrap().extra_broken, 1);
    for tick in [0, 19] {
        let mut w = World::new(&[[0, 0, 0], [1, 0, 0]]);
        w.player.tick = tick;
        assert_eq!(rt.mine(&mut w, &c, [0, 0, 0]).unwrap().extra_broken, 0);
        assert_eq!(w.calls.len(), 1);
    }
    let mut w = World::new(&[[0, 0, 0], [1, 0, 0]]);
    w.player.actor = [0, 0, 0, 2];
    assert_eq!(rt.mine(&mut w, &c, [0, 0, 0]).unwrap().extra_broken, 1);
    let mut w = World::new(&[[0, 0, 0], [1, 0, 0]]);
    w.player.tick = 20;
    assert_eq!(rt.mine(&mut w, &c, [0, 0, 0]).unwrap().extra_broken, 1);
    rt.set_enabled([0, 0, 0, 1], false);
    let mut w = World::new(&[[0, 0, 0], [1, 0, 0]]);
    w.player.tick = 100;
    assert_eq!(rt.mine(&mut w, &c, [0, 0, 0]).unwrap().extra_broken, 0);
    rt.set_enabled(w.player.actor, true);
    w.blocks.insert([0, 0, 0], block("minecraft:iron_ore", 1));
    assert_eq!(rt.mine(&mut w, &c, [0, 0, 0]).unwrap().extra_broken, 1);
}
#[test]
fn geometric_three_by_three_can_run_on_consecutive_breaks_without_cooldown() {
    let mut runtime = Runtime::default();
    let mut config = Config::default();
    config.settings.shape = Shape::ThreeByThree;
    config.settings.max_chain = 9;

    // Two separate 3x3 wall panels. Break both at the same player tick to
    // reproduce quick consecutive clicks while the legacy cooldown is active.
    let first_panel: Vec<_> = (-1..=1)
        .flat_map(|y| (-1..=1).map(move |z| [0, y, z]))
        .collect();
    let mut world = World::new(&first_panel);
    assert_eq!(world.player.tick, 0);

    let first = runtime
        .mine_with_face(&mut world, &config, [0, 0, 0], [1, 0, 0])
        .unwrap();
    assert_eq!((first.planned, first.extra_broken), (9, 8));

    for y in -1..=1 {
        for z in -1..=1 {
            world.blocks.insert([10, y, z], block("minecraft:iron_ore", 1));
        }
    }
    let second = runtime
        .mine_with_face(&mut world, &config, [10, 0, 0], [1, 0, 0])
        .unwrap();
    assert_eq!((second.planned, second.extra_broken), (9, 8));
    assert_eq!(world.calls.len(), 18);
}

#[test]
fn liteminer_drop_mode_defaults_to_together_and_round_trips_each_block() {
    let defaults = Config::default();
    assert_eq!(defaults.settings.drop_mode, DropMode::Together);
    assert!(defaults.settings.prevent_tool_breaking);

    let mut config = Config::default();
    config.settings.drop_mode = DropMode::EachBlock;
    let encoded = serde_json::to_string(&config).unwrap();
    let decoded = Config::from_json(&encoded).unwrap();
    assert_eq!(decoded.settings.drop_mode, DropMode::EachBlock);
}

#[test]
fn prevent_tool_breaking_matches_liteminer_two_durability_buffer() {
    // The origin consumes one point first. With three points at entry, two
    // remain; Liteminer stops before damaging the tool on an extra block.
    let mut world = World::new(&[[0, 0, 0], [1, 0, 0], [2, 0, 0], [3, 0, 0]]);
    world.tool.as_mut().unwrap().remaining = Some(3);
    let report = mine(&mut world, &Config::default());
    assert_eq!(report.extra_broken, 0);
    assert!(report.stopped);
    assert_eq!(world.tool.unwrap().remaining, Some(2));

    // With two points at entry, the origin leaves one; safety still blocks extras.
    let mut world = World::new(&[[0, 0, 0], [1, 0, 0]]);
    world.tool.as_mut().unwrap().remaining = Some(2);
    let report = mine(&mut world, &Config::default());
    assert_eq!(report.extra_broken, 0);
    assert!(report.stopped);
    assert_eq!(world.tool.unwrap().remaining, Some(1));

    // Turning safety off allows the secondary block to consume the final point.
    let mut world = World::new(&[[0, 0, 0], [1, 0, 0], [2, 0, 0]]);
    world.tool.as_mut().unwrap().remaining = Some(2);
    let mut config = Config::default();
    config.settings.prevent_tool_breaking = false;
    let report = mine(&mut world, &config);
    assert_eq!(report.extra_broken, 1);
    assert!(world.tool.is_none());
}

#[test]
fn group_tools_overrides_and_separate_group_mining_have_defined_precedence() {
    let mut c = Config::default();
    let g = c.groups.iter_mut().find(|g| g.id == "iron-ore").unwrap();
    g.tools.insert("minecraft:diamond_pickaxe".into());
    g.overrides.max_chain = Some(2);
    let mut w = World::new(&[[0, 0, 0], [1, 0, 0], [2, 0, 0]]);
    assert_eq!(mine(&mut w, &c).extra_broken, 0);
    w.blocks.insert([0, 0, 0], block("minecraft:iron_ore", 1));
    w.tool.as_mut().unwrap().item = "minecraft:diamond_pickaxe".into();
    assert_eq!(mine(&mut w, &c).extra_broken, 1);
    let mut w = World::new(&[[0, 0, 0], [1, 0, 0]]);
    w.blocks
        .insert([1, 0, 0], block("minecraft:deepslate_iron_ore", 2));
    let mut c = Config::default();
    c.settings.separate_group_mining = true;
    assert_eq!(mine(&mut w, &c).extra_broken, 0);
}
#[test]
fn malformed_unknown_and_unbounded_configuration_fails_closed() {
    assert!(Config::default().validate().is_ok());
    for source in [
        r#"{"settings":{"maxChain":0}}"#,
        r#"{"settings":{"searchRadius":100}}"#,
        r#"{"settings":{"hungerPerBlock":-1}}"#,
        r#"{"settings":{"delay":5}}"#,
        r#"{"groups":[{"id":"x","blocks":["no-namespace"]}]}"#,
        r#"{"groups":[{"id":"x","blocks":["minecraft:stone"]},{"id":"x","blocks":["minecraft:dirt"]}]}"#,
    ] {
        assert!(Config::from_json(source).is_err(), "{source}");
    }
    let c = Config::default();
    let json = serde_json::to_string(&c).unwrap();
    assert!(Config::from_json(&json).is_ok());
}
#[test]
fn coordinates_at_integer_limits_do_not_wrap_to_other_chunks() {
    let p = [i32::MAX, 0, i32::MIN];
    let w = World::new(&[p]);
    assert_eq!(
        preview(
            &w,
            p,
            &["minecraft:iron_ore".into()].into(),
            &Settings::default()
        )
        .len(),
        1
    );
}

#[test]
fn three_by_three_shape_uses_clicked_face_plane_and_includes_mixed_blocks() {
    let points: Vec<_> = (-1..=1)
        .flat_map(|x| (-1..=1).map(move |z| [x, 0, z]))
        .collect();
    let mut w = World::new(&points);
    w.blocks.insert([1, 0, 1], block("minecraft:stone", 9));
    let mut c = Config::default();
    c.settings.shape = Shape::ThreeByThree;
    c.settings.max_chain = 9;
    let hits = preview_with_face(&w, [0, 0, 0], &BTreeSet::new(), &c.settings, [0, 1, 0]);
    assert_eq!(hits.len(), 9);
    assert!(hits.iter().any(|hit| hit.block.name == "minecraft:stone"));
}

#[test]
fn tunnel_is_one_block_wide_and_follows_the_ray_not_the_player_backwards() {
    let points = [
        [0, 0, 0], [1, 0, 0], [2, 0, 0], [3, 0, 0],
        [1, 1, 0], [2, 1, 0],
    ];
    let w = World::new(&points);
    let mut c = Config::default();
    c.settings.shape = Shape::Tunnel;
    c.settings.max_chain = 8;
    let hits = preview_with_face(&w, [0, 0, 0], &BTreeSet::new(), &c.settings, [-1, 0, 0]);
    let positions: BTreeSet<_> = hits.into_iter().map(|hit| hit.position).collect();
    assert_eq!(positions, BTreeSet::from([[0, 0, 0], [1, 0, 0], [2, 0, 0], [3, 0, 0]]));
}

#[test]
fn tunnel_follows_vertical_face_instead_of_turning_sideways() {
    let w = World::new(&[[0, 0, 0], [0, -1, 0], [0, -2, 0], [0, 0, 1], [1, 0, 0]]);
    let mut c = Config::default();
    c.settings.shape = Shape::Tunnel;
    c.settings.max_chain = 5;
    let hits = preview_with_face(&w, [0, 0, 0], &BTreeSet::new(), &c.settings, [0, 1, 0]);
    let positions: BTreeSet<_> = hits.into_iter().map(|hit| hit.position).collect();
    assert_eq!(positions, BTreeSet::from([[0, 0, 0], [0, -1, 0], [0, -2, 0]]));
}

#[test]
fn staircase_up_and_down_follow_forward_steps_with_liteminer_vertical_columns() {
    let up_points = [
        [0, 0, 0], [0, -1, 0], [0, 1, 0],
        [1, 0, 0], [1, 1, 0], [1, 2, 0],
        [2, 1, 0], [2, 2, 0], [2, 3, 0],
    ];
    let mut w = World::new(&up_points);
    let mut c = Config::default();
    c.settings.shape = Shape::StaircaseUp;
    c.settings.max_chain = 9;
    let hits = preview_with_face(&w, [0, 0, 0], &BTreeSet::new(), &c.settings, [-1, 0, 0]);
    let positions: BTreeSet<_> = hits.into_iter().map(|hit| hit.position).collect();
    assert_eq!(positions, BTreeSet::from(up_points));

    let down_points = [
        [0, 0, 0], [0, -1, 0], [0, -2, 0],
        [1, -1, 0], [1, -2, 0], [1, -3, 0],
        [2, -2, 0], [2, -3, 0], [2, -4, 0],
    ];
    w = World::new(&down_points);
    c.settings.shape = Shape::StaircaseDown;
    let hits = preview_with_face(&w, [0, 0, 0], &BTreeSet::new(), &c.settings, [-1, 0, 0]);
    let positions: BTreeSet<_> = hits.into_iter().map(|hit| hit.position).collect();
    assert_eq!(positions, BTreeSet::from(down_points));
}

#[test]
fn horizontal_direction_from_yaw_matches_cardinal_player_facing() {
    assert_eq!(horizontal_direction_from_yaw(0.0), [0, 0, 1]);
    assert_eq!(horizontal_direction_from_yaw(90.0), [-1, 0, 0]);
    assert_eq!(horizontal_direction_from_yaw(180.0), [0, 0, -1]);
    assert_eq!(horizontal_direction_from_yaw(270.0), [1, 0, 0]);
    assert_eq!(horizontal_direction_from_yaw(-90.0), [1, 0, 0]);
}

#[test]
fn vertical_staircase_hit_uses_player_horizontal_facing() {
    let points = [
        [0, 0, 0], [0, -1, 0], [0, 1, 0],
        [-1, 0, 0], [-1, 1, 0], [-1, 2, 0],
    ];
    let mut w = World::new(&points);
    w.forward = [-1, 0, 0];
    let mut c = Config::default();
    c.settings.shape = Shape::StaircaseUp;
    c.settings.max_chain = 6;
    let hits = preview_with_face(&w, [0, 0, 0], &BTreeSet::new(), &c.settings, [0, 1, 0]);
    let positions: BTreeSet<_> = hits.into_iter().map(|hit| hit.position).collect();
    assert_eq!(positions, BTreeSet::from(points));
}

#[test]
fn shapeless_matches_ore_variants_and_plant_families_but_keeps_crop_maturity() {
    let mut w = World::new(&[[0, 0, 0], [1, 0, 0]]);
    w.blocks.insert([1, 0, 0], block("minecraft:raw_iron_block", 2));
    let targets = BTreeSet::from(["minecraft:iron_ore".into()]);
    assert_eq!(preview(&w, [0, 0, 0], &targets, &Settings::default()).len(), 2);

    w = World::new(&[[0, 0, 0], [1, 0, 0]]);
    w.blocks.insert([0, 0, 0], block("minecraft:diamond_ore", 3));
    w.blocks.insert([1, 0, 0], block("minecraft:deepslate_diamond_ore", 4));
    assert_eq!(preview(&w, [0, 0, 0], &["minecraft:diamond_ore".into()].into(), &Settings::default()).len(), 2);

    w = World::new(&[[0, 0, 0], [1, 0, 0]]);
    let mut flower = block("minecraft:dandelion", 5);
    flower.plant = true;
    let mut grass = block("minecraft:short_grass", 6);
    grass.plant = true;
    w.blocks.insert([0, 0, 0], flower);
    w.blocks.insert([1, 0, 0], grass);
    assert_eq!(preview(&w, [0, 0, 0], &["minecraft:dandelion".into()].into(), &Settings::default()).len(), 2);

    w = World::new(&[[0, 0, 0], [1, 0, 0]]);
    let mut young = block("minecraft:wheat", 1);
    young.growth = Some((1, 7));
    w.blocks.insert([0, 0, 0], young);
    let mut mature = block("minecraft:wheat", 2);
    mature.growth = Some((7, 7));
    w.blocks.insert([1, 0, 0], mature);
    let mut settings = Settings::default();
    assert_eq!(preview(&w, [0, 0, 0], &["minecraft:wheat".into()].into(), &settings).len(), 1);
    settings.match_crop_maturity = false;
    assert_eq!(preview(&w, [0, 0, 0], &["minecraft:wheat".into()].into(), &settings).len(), 2);
}

}
