// Waymap: client-side waypoints, in the world and on the minimap.
//
// A waypoint is a named point (`name`, `x`, `y`, `z`) in one dimension, with a
// colour. Waypoints are stored per context (server or singleplayer world) in
// the mod's own folder, so they survive restarts, world changes, and updates.
//
// The markers are drawn from `locator_camera`, the camera Horizon used for the
// locator bar (position, yaw, pitch and field of view), so they line up with
// the world without reading the renderer's matrices. The minimap dots are
// drawn in `extras_hud::waymap_minimap` from the view the minimap was
// composed from; this module hands it the waypoints once a frame.
//
// Commands: `/wp add [name]`, `/wp del <name>`, `/wp list`, `/wp rename <old>
// <new>`, `/wp color <name> <colour>`, `/wp clear`, `/wp help`, `/wp hide`,
// `/wp show`.
// Keys: N creates a waypoint here, B (or M) opens the waypoint list; both are
// rebindable in Options > Mods > Waymap.
//
// The screen architecture (Mode/Action/Widget, the `Out` drawing context,
// widget hit-testing and the key/char split) is adapted from Nirokx's
// "horizon-waypoints" (MIT License, Copyright (c) 2026 Nirokx),
// https://github.com/Nirokx/horizon-waypoints. See THIRD_PARTY_NOTICES.md.

use super::Game;
use horizon_client::extras::Corner;
use horizon_client::extras_hud::Corners;
use horizon_client::hud::{rgba, HudAtlas, HudBatch};
use std::cell::RefCell;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

/// The mod's id: its folder, its settings files and its keys.
pub(super) const MOD_ID: &str = "waymap";
const NAME: &str = "Waymap";
const VERSION: &str = "0.2.1";
const DESCRIPTION: &str = "Waypoints in the world and on the minimap. N creates one, B or M lists them.";
/// Maximum waypoints per context; more would only clutter the map.
const MAX_WAYPOINTS: usize = 512;
/// Player eye height while standing. Only the v1 parser uses it: v1 files
/// stored eye level, v2 and everything the mod writes now stores feet.
const EYE: f64 = 1.62;
/// The header of the current on-disk format.
const HEADER: &str = "# waymap 2";

const OVERWORLD: &str = "minecraft:overworld";
const NETHER: &str = "minecraft:the_nether";
const END: &str = "minecraft:the_end";

/// The rebindable keys, as Horizon keeps them.
const KEY_CREATE: &str = "add";
const KEY_LIST: &str = "panel";
const DEFAULT_CREATE: &str = "key.keyboard.n";
const DEFAULT_LIST: &str = "key.keyboard.b";
/// M opens the list too, as the reference mod does.
const ALT_LIST: &str = "key.keyboard.m";

/// `/wp color` names, cycled by a waypoint's index.
const COLORS: [(&str, u32); 12] = [
    ("white", 0xFF_FFFF),
    ("red", 0xFF_5555),
    ("orange", 0xFF_A033),
    ("yellow", 0xFF_EE55),
    ("green", 0x66_EE66),
    ("cyan", 0x55_EEEE),
    ("blue", 0x66_99_FF),
    ("purple", 0xBB_88_FF),
    ("pink", 0xFF_88_CC),
    ("gray", 0xBB_BB_BB),
    ("brown", 0xC8_8A_66),
    ("lime", 0xAA_FF_55),
];

/// The editor's palette.
const PALETTE: [u32; 16] = [
    0xFF5555, 0xFFAA00, 0xFFFF55, 0x55FF55, 0x00AA00, 0x55FFFF, 0x00AAAA, 0x5555FF, 0x0000AA,
    0xAA00AA, 0xFF55FF, 0xFFFFFF, 0xAAAAAA, 0x555555, 0xAA5500, 0x000000,
];

const NAME_MAX: usize = 32;
const COORD_MAX: usize = 12;
const LINE: f32 = 10.0;
const ROW: f32 = 24.0;

const INK: u32 = 0xFF_FFFF;
const MUTED: u32 = 0xAA_AAAA;
const ERROR: u32 = 0xFF_5555;
const SHADE: u32 = 0xA000_0000;
const PANEL: u32 = 0xE010_1018;
const PANEL_EDGE: u32 = 0xFF50_5064;
const BUTTON: u32 = 0xFF34_3442;
const BUTTON_HOVER: u32 = 0xFF4E_4E66;
const BUTTON_ON: u32 = 0xFF3C_6E46;
const FIELD: u32 = 0xFF00_0000;
const FIELD_EDGE: u32 = 0xFF70_7070;
const FIELD_FOCUS: u32 = 0xFF_FFFF;
const ROW_BG: u32 = 0x3000_0000;
const ROW_SELECTED: u32 = 0x50_50_70_A0;
/// Death waypoints are red, so they stand out in the world.
const DEATH_COLOR: u32 = 0xFF_5555;

// ---- Data ------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct Waypoint {
    pub name: String,
    /// Feet position: the same point the player stands on.
    pub x: f64,
    pub y: f64,
    pub z: f64,
    /// `minecraft:overworld`, `minecraft:the_nether`, `minecraft:the_end`.
    pub dim: String,
    /// `0xRRGGBB`.
    pub color: u32,
    /// Hidden waypoints stay in the list but leave the world and minimap.
    pub hidden: bool,
    /// Created where the player died; the Deaths tab lists exactly these.
    pub death: bool,
}

impl Waypoint {
    fn pos(&self) -> [f64; 3] {
        [self.x, self.y, self.z]
    }

    fn display_coords(&self) -> String {
        format!("{} {} {}", trim(self.x), trim(self.y), trim(self.z))
    }
}

/// Waypoints live on whole coordinates: every point read, created, edited or
/// written is rounded to the nearest whole number.
fn whole(v: f64) -> f64 {
    v.round()
}

/// A number as the file and the list show it: whole numbers without `.0`.
fn trim(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

/// Context key identifying the current world/server.
/// Multiplayer: `server:<host:port>`; singleplayer: `world:<folder>`.
#[derive(Clone, Debug, PartialEq, Eq)]
enum ContextKey {
    Multiplayer(String),
    Singleplayer(String),
}

impl ContextKey {
    fn file_name(&self) -> String {
        match self {
            ContextKey::Multiplayer(addr) => {
                let safe = addr.replace([':', '/', '\\'], "_");
                format!("server_{safe}.txt")
            }
            ContextKey::Singleplayer(folder) => {
                let safe = folder.replace(['/', '\\'], "_");
                format!("world_{safe}.txt")
            }
        }
    }

    fn display_name(&self) -> String {
        match self {
            ContextKey::Multiplayer(addr) => format!("Server: {addr}"),
            ContextKey::Singleplayer(folder) => format!("World: {folder}"),
        }
    }
}

/// The list's dimension filter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filter {
    All,
    Overworld,
    Nether,
    End,
    /// The waypoints created where the player died.
    Deaths,
}

impl Filter {
    const ALL: [Filter; 5] = [
        Filter::All,
        Filter::Overworld,
        Filter::Nether,
        Filter::End,
        Filter::Deaths,
    ];

    fn label(self) -> &'static str {
        match self {
            Filter::All => "All",
            Filter::Overworld => "Overworld",
            Filter::Nether => "Nether",
            Filter::End => "End",
            Filter::Deaths => "Deaths",
        }
    }

    fn matches(self, waypoint: &Waypoint) -> bool {
        match self {
            Filter::All => true,
            Filter::Overworld => waypoint.dim == OVERWORLD,
            Filter::Nether => waypoint.dim == NETHER,
            Filter::End => waypoint.dim == END,
            Filter::Deaths => waypoint.death,
        }
    }

    /// Parses `all`, `overworld`, `minecraft:overworld`, `nether`, `end`, `deaths`…
    #[cfg(test)]
    fn parse(text: &str) -> Option<Filter> {
        let text = text.trim().trim_start_matches("minecraft:");
        Some(match text {
            "" | "all" => Filter::All,
            "overworld" => Filter::Overworld,
            "the_nether" | "nether" => Filter::Nether,
            "the_end" | "end" => Filter::End,
            "deaths" | "death" => Filter::Deaths,
            _ => return None,
        })
    }
}

/// The editor's text fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Name,
    X,
    Y,
    Z,
}

impl Field {
    const ORDER: [Field; 4] = [Field::Name, Field::X, Field::Y, Field::Z];

    fn step(self, back: bool) -> Field {
        let at = Field::ORDER.iter().position(|&f| f == self).unwrap_or(0);
        let n = Field::ORDER.len();
        Field::ORDER[if back { (at + n - 1) % n } else { (at + 1) % n }]
    }
}

/// The create / edit form. `index` is the waypoint being edited, None for a
/// new one; `from_list` says Cancel returns to the list instead of closing.
#[derive(Clone, Debug, PartialEq)]
pub struct Editor {
    pub index: Option<usize>,
    pub name: String,
    pub coords: [String; 3],
    pub dimension: String,
    pub color: u32,
    pub hidden: bool,
    pub focus: Field,
    pub from_list: bool,
    pub error: Option<String>,
    /// The name is the suggested one, untouched: typing replaces it.
    pub suggested: bool,
}

impl Editor {
    fn field(&mut self, field: Field) -> &mut String {
        match field {
            Field::Name => &mut self.name,
            Field::X => &mut self.coords[0],
            Field::Y => &mut self.coords[1],
            Field::Z => &mut self.coords[2],
        }
    }
}

/// What the screen is showing.
#[derive(Clone, Debug, PartialEq)]
pub enum Mode {
    Closed,
    List,
    Edit(Editor),
}

/// What a click on the screen does.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    NewWaypoint,
    Filter(Filter),
    Select(usize),
    Toggle(usize),
    Edit(usize),
    Delete(usize),
    Focus(Field),
    Color(u32),
    CycleDimension,
    ToggleHidden,
    Here,
    Save,
    Cancel,
    Close,
}

/// Where the player is, as the hooks read it (feet position).
#[derive(Clone, Debug, PartialEq)]
pub struct Here {
    pub pos: [f64; 3],
    pub dimension: String,
}

/// What a hook draws: a filled rect or a line of text.
#[derive(Clone, Debug, PartialEq)]
pub enum Draw {
    Fill([f32; 4], u32),
    Text(String, [f32; 2], u32, bool),
}

/// A clickable rectangle with an action.
#[derive(Clone, Debug, PartialEq)]
pub struct Widget {
    pub rect: [f32; 4],
    pub action: Action,
}

/// Drawing context: accumulates draws and widgets, with layout helpers.
pub struct Out<'a> {
    draws: Vec<Draw>,
    widgets: Vec<Widget>,
    mouse: [f32; 2],
    measure: &'a dyn Fn(&str) -> f32,
}

fn inside(rect: [f32; 4], at: [f32; 2]) -> bool {
    at[0] >= rect[0] && at[0] < rect[0] + rect[2] && at[1] >= rect[1] && at[1] < rect[1] + rect[3]
}

impl<'a> Out<'a> {
    fn new(mouse: [f32; 2], measure: &'a dyn Fn(&str) -> f32) -> Out<'a> {
        Out {
            draws: Vec::new(),
            widgets: Vec::new(),
            mouse,
            measure,
        }
    }

    fn fill(&mut self, rect: [f32; 4], argb: u32) {
        self.draws.push(Draw::Fill(rect, argb));
    }

    fn text(&mut self, text: &str, at: [f32; 2], rgb: u32) {
        self.draws
            .push(Draw::Text(text.to_owned(), at, rgb, true));
    }

    fn width(&self, text: &str) -> f32 {
        (self.measure)(text)
    }

    /// A 1px frame around `rect`, filled with `inner`.
    fn boxed(&mut self, rect: [f32; 4], edge: u32, inner: u32) {
        let [x, y, w, h] = rect;
        self.fill(rect, edge);
        self.fill([x + 1.0, y + 1.0, w - 2.0, h - 2.0], inner);
    }

    /// `text` cut to `max` pixels with an ellipsis.
    fn fit(&self, text: &str, max: f32) -> String {
        if self.width(text) <= max {
            return text.to_owned();
        }
        let mut cut = text.to_owned();
        while !cut.is_empty() && self.width(&format!("{cut}...")) > max {
            cut.pop();
        }
        format!("{cut}...")
    }

    /// A button with a label, action and on/off state.
    fn button(&mut self, rect: [f32; 4], label: &str, action: Action, on: bool) {
        let hover = inside(rect, self.mouse);
        let fill = if on {
            BUTTON_ON
        } else if hover {
            BUTTON_HOVER
        } else {
            BUTTON
        };
        self.boxed(rect, if hover { FIELD_FOCUS } else { PANEL_EDGE }, fill);
        let w = self.width(label);
        let at = [
            rect[0] + (rect[2] - w) / 2.0,
            rect[1] + (rect[3] - 8.0) / 2.0,
        ];
        self.text(label, at, INK);
        self.widgets.push(Widget { rect, action });
    }

    /// A button sized to its label; returns its width.
    fn button_auto(&mut self, x: f32, y: f32, label: &str, action: Action, on: bool) -> f32 {
        let w = self.width(label) + 8.0;
        self.button([x, y, w, 12.0], label, action, on);
        w
    }
}

/// The widget under a point; the last drawn (topmost) one wins.
fn hit(widgets: &[Widget], at: [f32; 2]) -> Option<Action> {
    widgets
        .iter()
        .rev()
        .find(|w| inside(w.rect, at))
        .map(|w| w.action.clone())
}

/// The letter a key name types (`key.keyboard.n` types `n`), if it is one.
pub fn key_char(key: &str) -> Option<char> {
    let last = key.strip_prefix("key.keyboard.")?;
    let mut chars = last.chars();
    let c = chars.next()?;
    chars.next().is_none().then_some(c)
}

// ---- Storage ---------------------------------------------------------------

/// The v2 codec: header, then one tab-separated waypoint per line.
/// `dimension	x	y	z	RRGGBB	hidden	name` with name escapes.
struct WaypointCodec;

impl WaypointCodec {
    fn encode(waypoints: &[Waypoint]) -> String {
        let mut out = String::from(HEADER);
        out.push('\n');
        for w in waypoints {
            let hidden = if w.hidden { "1" } else { "0" };
            let escaped = escape_name(&w.name);
            // Coordinates are written whole; the death flag is a trailing
            // optional column, so files from before it still load.
            let _ = writeln!(
                out,
                "{}\t{}\t{}\t{}\t{:06x}\t{}\t{}\t{}",
                w.dim, whole(w.x), whole(w.y), whole(w.z), w.color, hidden, escaped,
                if w.death { "1" } else { "0" }
            );
        }
        out
    }

    fn decode(content: &str) -> Result<Vec<Waypoint>, String> {
        let mut lines = content.lines();
        let header = lines.next().unwrap_or("");
        if header.trim() != HEADER {
            return Err(format!("missing or invalid header: expected '{HEADER}'"));
        }
        let mut waypoints = Vec::new();
        for (i, line) in lines.enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let wp = parse_v2_line(line).map_err(|e| format!("line {}: {e}", i + 2))?;
            waypoints.push(wp);
        }
        Ok(waypoints)
    }
}

fn parse_v2_line(line: &str) -> Result<Waypoint, String> {
    let mut parts = line.split('\t');
    let mut field = |name: &str| parts.next().ok_or_else(|| format!("missing {name}"));
    let dim = field("dimension")?.to_owned();
    let x = whole(parse_f64(field("X")?, "X")?);
    let y = whole(parse_f64(field("Y")?, "Y")?);
    let z = whole(parse_f64(field("Z")?, "Z")?);
    let color = u32::from_str_radix(field("colour")?, 16)
        .map_err(|_| "invalid colour hex".to_owned())?;
    let hidden = match field("hidden flag")? {
        "0" => false,
        "1" => true,
        _ => return Err("hidden flag must be 0 or 1".to_owned()),
    };
    let name = unescape_name(field("name")?)?;
    // The death flag is optional so files written before it still load.
    let death = match parts.next() {
        Some("1") => true,
        Some("0") | None => false,
        Some(_) => return Err("death flag must be 0 or 1".to_owned()),
    };
    if parts.next().is_some() {
        return Err("too many columns".to_owned());
    }
    Ok(Waypoint {
        name,
        x,
        y,
        z,
        dim,
        color,
        hidden,
        death,
    })
}

/// The v1 codec: six columns, no header, `y` at eye level.
struct LegacyWaypointCodec;

impl LegacyWaypointCodec {
    fn decode(content: &str) -> Result<Vec<Waypoint>, String> {
        let mut waypoints = Vec::new();
        let mut errors = Vec::new();
        for (i, line) in content.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            match parse_v1_line(line) {
                Ok(wp) => waypoints.push(wp),
                Err(e) => errors.push(format!("line {}: {e}", i + 1)),
            }
        }
        if waypoints.is_empty() && !errors.is_empty() {
            return Err(errors.join("; "));
        }
        Ok(waypoints)
    }
}

fn parse_f64(text: &str, field: &str) -> Result<f64, String> {
    text.trim()
        .parse()
        .map_err(|_| format!("invalid {field}"))
}

fn parse_v1_line(line: &str) -> Result<Waypoint, String> {
    let mut parts = line.split('\t');
    let dim = parts.next().ok_or("missing dimension")?.to_owned();
    let x = whole(parse_f64(parts.next().ok_or("missing X")?, "X")?);
    let y = parse_f64(parts.next().ok_or("missing Y")?, "Y")?;
    let z = whole(parse_f64(parts.next().ok_or("missing Z")?, "Z")?);
    let color = u32::from_str_radix(parts.next().ok_or("missing colour")?, 16)
        .map_err(|_| "invalid colour hex".to_owned())?;
    let name = unescape_name(parts.next().ok_or("missing name")?)?;
    if parts.next().is_some() {
        return Err("too many columns".to_owned());
    }
    // v1 kept the player's eyes, v2 and the world keep their feet.
    Ok(Waypoint {
        name,
        x,
        y: whole(y - EYE),
        z,
        dim,
        color,
        hidden: false,
        death: false,
    })
}

fn escape_name(name: &str) -> String {
    name.replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\n', "\\n")
}

fn unescape_name(escaped: &str) -> Result<String, String> {
    let mut out = String::new();
    let mut chars = escaped.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('\\') => out.push('\\'),
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some(other) => return Err(format!("invalid escape: \\{other}")),
            None => return Err("invalid escape at end of name".to_owned()),
        }
    }
    Ok(out)
}

/// Reading and writing one waypoint file.
struct WaypointFileIO;

impl WaypointFileIO {
    fn load_file(path: &Path) -> Result<Vec<Waypoint>, String> {
        if !path.exists() {
            return Ok(Vec::new());
        }
        let content =
            fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        if content.lines().next().is_some_and(|l| l.trim() == HEADER) {
            WaypointCodec::decode(&content)
        } else {
            LegacyWaypointCodec::decode(&content)
        }
    }

    /// Writes `waypoints` via a temporary file and an atomic rename. Always
    /// writes, so clearing a list leaves an empty (header-only) file.
    fn save_file(path: &Path, waypoints: &[Waypoint]) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
        }
        let temp = path.with_extension("tmp");
        fs::write(&temp, WaypointCodec::encode(waypoints))
            .map_err(|e| format!("cannot write {}: {e}", temp.display()))?;
        fs::rename(&temp, path).map_err(|e| format!("cannot replace {}: {e}", path.display()))
    }

    /// Moves a legacy file aside; `restore` puts it back if the new file
    /// could not be written.
    fn backup_legacy(path: &Path) -> Result<PathBuf, String> {
        let backup = path.with_file_name(format!(
            "{}.v1.bak",
            path.file_name().unwrap_or_default().to_string_lossy()
        ));
        fs::rename(path, &backup)
            .map_err(|e| format!("cannot back up {}: {e}", path.display()))?;
        Ok(backup)
    }
}

/// `true`/`on` and `false`/`off`, as a settings value reads.
fn setting_bool(value: Option<&str>, default: bool) -> bool {
    match value.map(str::trim) {
        Some("true") | Some("on") => true,
        Some("false") | Some("off") => false,
        _ => default,
    }
}

fn on_off(on: bool) -> &'static str {
    if on {
        "on"
    } else {
        "off"
    }
}

// ---- State -----------------------------------------------------------------

struct State {
    /// The world or server the waypoints belong to.
    context: Option<ContextKey>,
    /// The mod's data folder.
    dir: Option<PathBuf>,
    waypoints: Vec<Waypoint>,
    /// The current context was read successfully; writes are allowed.
    loaded: bool,
    /// The current context could not be read; never overwrite its file.
    load_failed: bool,
    /// The legacy `waypoints.txt` was looked at.
    migrated: bool,
    /// The death screen was open last frame, so one death is one waypoint.
    was_dead: bool,
    /// Settings (Options > Mods), read once.
    settings_loaded: bool,
    markers: bool,
    labels: bool,
    panel: bool,
    /// Screen state.
    mode: Mode,
    filter: Filter,
    scroll: usize,
    /// The selected waypoint (index into `waypoints`).
    selection: Option<usize>,
    /// Rows the last list render could show.
    visible_rows: usize,
    /// The row whose Delete was clicked once and asks to be clicked again.
    confirm_delete: Option<usize>,
    /// The character of the key that opened the screen, which must not be
    /// typed into it.
    skip_char: Option<char>,
    close_requested: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            context: None,
            dir: None,
            waypoints: Vec::new(),
            loaded: false,
            load_failed: false,
            migrated: false,
            was_dead: false,
            settings_loaded: false,
            markers: true,
            labels: true,
            panel: true,
            mode: Mode::Closed,
            filter: Filter::All,
            scroll: 0,
            selection: None,
            visible_rows: 8,
            confirm_delete: None,
            skip_char: None,
            close_requested: false,
        }
    }
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

/// Runs `f` on the mod's state. Hooks call it without calling back into the
/// game inside, so it is never borrowed twice.
fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with_borrow_mut(f)
}

impl State {
    /// Switches to `key`'s waypoints in `dir` when the context changed.
    fn switch_to(&mut self, key: Option<ContextKey>, dir: PathBuf) {
        if self.context == key && (self.loaded || self.load_failed) {
            self.dir = Some(dir);
            return;
        }
        if self.loaded {
            let _ = self.persist();
        }
        self.context = key;
        self.dir = Some(dir);
        self.waypoints.clear();
        self.loaded = false;
        self.load_failed = false;
        self.mode = Mode::Closed;
        self.filter = Filter::All;
        self.scroll = 0;
        self.selection = None;
        self.confirm_delete = None;
        self.skip_char = None;
        self.was_dead = false;
        match &self.context {
            Some(ctx) => {
                let path = self.dir.as_ref().unwrap().join(ctx.file_name());
                match WaypointFileIO::load_file(&path) {
                    Ok(waypoints) => {
                        self.waypoints = waypoints;
                        self.loaded = true;
                    }
                    Err(_) => {
                        self.load_failed = true;
                    }
                }
            }
            None => self.loaded = false,
        }
    }

    /// Writes the current context. Refuses after a read error, so a file that
    /// could not be parsed is never replaced by an empty one.
    fn persist(&self) -> Result<(), String> {
        let (Some(dir), Some(ctx)) = (&self.dir, &self.context) else {
            return Ok(());
        };
        if self.load_failed {
            return Err("the waypoint file could not be read; not overwriting it".to_owned());
        }
        WaypointFileIO::save_file(&dir.join(ctx.file_name()), &self.waypoints)
    }

    /// Migrates the v1 `waypoints.txt` once: read, back up, write v2.
    fn migrate_legacy(&mut self, dir: &Path) -> Result<(), String> {
        if self.migrated {
            return Ok(());
        }
        self.migrated = true;
        let legacy = dir.join("waypoints.txt");
        if !legacy.exists() {
            return Ok(());
        }
        let waypoints = WaypointFileIO::load_file(&legacy)?;
        let backup = WaypointFileIO::backup_legacy(&legacy)?;
        let ctx = ContextKey::Singleplayer("legacy".to_owned());
        if let Err(e) = WaypointFileIO::save_file(&dir.join(ctx.file_name()), &waypoints) {
            let _ = fs::rename(&backup, &legacy);
            return Err(e);
        }
        Ok(())
    }

    fn filtered(&self) -> Vec<usize> {
        self.waypoints
            .iter()
            .enumerate()
            .filter(|(_, w)| self.filter.matches(w))
            .map(|(i, _)| i)
            .collect()
    }

    fn clamp_scroll(&mut self, rows: usize) {
        let fits = self.visible_rows.max(1);
        self.scroll = self.scroll.min(rows.saturating_sub(fits));
    }

    fn select_first(&mut self) {
        let rows = self.filtered();
        self.selection = rows.first().copied();
        self.scroll = 0;
        self.confirm_delete = None;
    }

    fn clamp_selection(&mut self) {
        let rows = self.filtered();
        if rows.is_empty() {
            self.selection = None;
            self.scroll = 0;
            self.confirm_delete = None;
            return;
        }
        if !self.selection.is_some_and(|s| rows.contains(&s)) {
            self.selection = Some(rows[0]);
        }
        let pos = rows
            .iter()
            .position(|&i| Some(i) == self.selection)
            .unwrap_or(0);
        self.reveal(pos, rows.len());
    }

    /// Moves the selection by `delta` filtered rows and keeps it visible.
    fn move_selection(&mut self, delta: isize) {
        let rows = self.filtered();
        if rows.is_empty() {
            self.selection = None;
            return;
        }
        let pos = self
            .selection
            .and_then(|s| rows.iter().position(|&i| i == s))
            .unwrap_or(0);
        let pos = (pos as isize + delta).clamp(0, rows.len() as isize - 1) as usize;
        self.selection = Some(rows[pos]);
        self.reveal(pos, rows.len());
    }

    fn reveal(&mut self, pos: usize, rows: usize) {
        let fits = self.visible_rows.max(1);
        if pos < self.scroll {
            self.scroll = pos;
        } else if pos >= self.scroll + fits {
            self.scroll = pos + 1 - fits;
        }
        self.clamp_scroll(rows);
    }

    fn death_name(&self) -> String {
        let mut n = 1;
        while self
            .waypoints
            .iter()
            .any(|w| w.name.eq_ignore_ascii_case(&format!("Death {n}")))
        {
            n += 1;
        }
        format!("Death {n}")
    }

    /// Records where the player died, once per death: the death screen opening
    /// is the edge. `here` is the player's feet position while the screen is up.
    fn observe_death(&mut self, dead: bool, here: Option<&Here>) -> Option<String> {
        if !dead {
            self.was_dead = false;
            return None;
        }
        if std::mem::replace(&mut self.was_dead, true) {
            return None;
        }
        let here = here?;
        if self.waypoints.len() >= MAX_WAYPOINTS {
            return Some(format!(
                "Waymap: not adding a death waypoint (at most {MAX_WAYPOINTS})."
            ));
        }
        let name = self.death_name();
        self.waypoints.push(Waypoint {
            name: name.clone(),
            x: whole(here.pos[0]),
            y: whole(here.pos[1]),
            z: whole(here.pos[2]),
            dim: here.dimension.clone(),
            color: DEATH_COLOR,
            hidden: false,
            death: true,
        });
        match self.persist() {
            Ok(()) => Some(format!("Waymap: death waypoint '{name}' set.")),
            Err(e) => {
                self.waypoints.pop();
                Some(format!("Waymap: death waypoint not saved: {e}"))
            }
        }
    }

    fn default_name(&self) -> String {
        let mut n = self.waypoints.len() + 1;
        while self
            .waypoints
            .iter()
            .any(|w| w.name == format!("Waypoint {n}"))
        {
            n += 1;
        }
        format!("Waypoint {n}")
    }

    /// The create form, filled in with where the player stands.
    fn open_create(&mut self, here: &Here, from_list: bool, skip: Option<char>) {
        self.mode = Mode::Edit(Editor {
            index: None,
            name: self.default_name(),
            coords: here.pos.map(|v| trim(whole(v))),
            dimension: here.dimension.clone(),
            color: PALETTE[self.waypoints.len() % PALETTE.len()],
            hidden: false,
            focus: Field::Name,
            from_list,
            error: None,
            suggested: true,
        });
        self.skip_char = skip;
        self.confirm_delete = None;
        self.selection = None;
    }

    fn open_list(&mut self, skip: Option<char>) {
        self.mode = Mode::List;
        self.confirm_delete = None;
        self.skip_char = skip;
        self.select_first();
    }

    fn open_edit(&mut self, index: usize) {
        let Some(w) = self.waypoints.get(index) else {
            return;
        };
        self.mode = Mode::Edit(Editor {
            index: Some(index),
            name: w.name.clone(),
            coords: w.pos().map(|v| trim(whole(v))),
            dimension: w.dim.clone(),
            color: w.color,
            hidden: w.hidden,
            focus: Field::Name,
            from_list: true,
            error: None,
            suggested: false,
        });
        self.confirm_delete = None;
    }

    fn close(&mut self) {
        self.mode = Mode::Closed;
        self.confirm_delete = None;
        self.skip_char = None;
        self.close_requested = true;
    }

    /// Whether the game should close the screen now (once).
    fn take_close(&mut self) -> bool {
        std::mem::take(&mut self.close_requested)
    }

    /// The game's screen is ours or not: forget an open screen the game
    /// closed (Escape, another screen).
    fn sync_open(&mut self, ours: bool) {
        if !ours && self.mode != Mode::Closed {
            self.mode = Mode::Closed;
            self.confirm_delete = None;
            self.skip_char = None;
        }
    }

    /// Builds a waypoint from the editor, or says what is wrong with it.
    fn waypoint_from(&self, editor: &Editor) -> Result<Waypoint, String> {
        let mut coords = [0.0_f64; 3];
        for (i, field) in editor.coords.iter().enumerate() {
            let value = field
                .trim()
                .parse::<f64>()
                .map_err(|_| "Coordinates must be numbers".to_owned())?;
            if !value.is_finite() {
                return Err("Coordinates must be finite".to_owned());
            }
            coords[i] = whole(value);
        }
        let name = editor.name.trim().to_owned();
        let name = if name.is_empty() {
            self.default_name()
        } else {
            name
        };
        if editor.index
            .map_or(true, |i| i >= self.waypoints.len())
            && self.waypoints.iter().any(|w| w.name.eq_ignore_ascii_case(&name))
        {
            return Err(format!("'{name}' already exists"));
        }
        if let Some(index) = editor.index {
            if self
                .waypoints
                .iter()
                .enumerate()
                .any(|(i, w)| i != index && w.name.eq_ignore_ascii_case(&name))
            {
                return Err(format!("'{name}' already exists"));
            }
        }
        if self.waypoints.len() >= MAX_WAYPOINTS && editor.index.is_none() {
            return Err(format!("At most {MAX_WAYPOINTS} waypoints"));
        }
        let death = editor
            .index
            .and_then(|i| self.waypoints.get(i))
            .is_some_and(|w| w.death);
        Ok(Waypoint {
            name,
            x: coords[0],
            y: coords[1],
            z: coords[2],
            dim: editor.dimension.clone(),
            color: editor.color,
            hidden: editor.hidden,
            death,
        })
    }

    /// Saves the form, or keeps it open with an error.
    fn save(&mut self) {
        let Mode::Edit(mut editor) = std::mem::replace(&mut self.mode, Mode::Closed) else {
            return;
        };
        match self.waypoint_from(&editor) {
            Ok(waypoint) => {
                let index = editor.index.filter(|&i| i < self.waypoints.len());
                let previous = index.map(|i| self.waypoints[i].clone());
                match index {
                    Some(i) => self.waypoints[i] = waypoint,
                    None => self.waypoints.push(waypoint),
                }
                match self.persist() {
                    Ok(()) => {
                        // Saving always returns to the list, whether the form
                        // was opened from the list or with the create key.
                        self.mode = Mode::List;
                        self.confirm_delete = None;
                        self.clamp_selection();
                    }
                    Err(message) => {
                        // Put the list back the way it was and keep the form
                        // open, so a failed write is never shown as saved.
                        match (index, previous) {
                            (Some(i), Some(previous)) => self.waypoints[i] = previous,
                            (None, _) => {
                                self.waypoints.pop();
                            }
                            _ => {}
                        }
                        editor.error = Some(format!("Could not save: {message}"));
                        self.mode = Mode::Edit(editor);
                    }
                }
            }
            Err(message) => {
                editor.error = Some(message);
                self.mode = Mode::Edit(editor);
            }
        }
    }

    fn delete(&mut self, index: usize) {
        if self.confirm_delete != Some(index) {
            self.confirm_delete = Some(index);
            return;
        }
        self.confirm_delete = None;
        if index < self.waypoints.len() {
            self.waypoints.remove(index);
            let _ = self.persist();
            self.clamp_selection();
        }
    }

    fn leave_editor(&mut self, from_list: bool) {
        self.mode = if from_list { Mode::List } else { Mode::Closed };
        if from_list {
            self.clamp_selection();
        } else {
            self.close();
        }
    }

    /// Does what a click on `action` does.
    fn act(&mut self, action: Action, here: Option<&Here>) {
        if !matches!(action, Action::Delete(_)) {
            self.confirm_delete = None;
        }
        match action {
            Action::NewWaypoint => {
                if let Some(here) = here {
                    self.open_create(here, true, None);
                }
            }
            Action::Filter(filter) => {
                self.filter = filter;
                self.select_first();
            }
            Action::Select(index) => {
                if index < self.waypoints.len() {
                    self.selection = Some(index);
                    let rows = self.filtered();
                    if let Some(pos) = rows.iter().position(|&i| i == index) {
                        self.reveal(pos, rows.len());
                    }
                }
            }
            Action::Toggle(index) => {
                if let Some(w) = self.waypoints.get_mut(index) {
                    w.hidden = !w.hidden;
                    let _ = self.persist();
                }
            }
            Action::Edit(index) => self.open_edit(index),
            Action::Delete(index) => self.delete(index),
            Action::Close => self.close(),
            Action::Save => self.save(),
            Action::Cancel => {
                if let Mode::Edit(e) = &self.mode {
                    let from_list = e.from_list;
                    self.leave_editor(from_list);
                }
            }
            Action::Focus(field) => {
                if let Mode::Edit(e) = &mut self.mode {
                    e.focus = field;
                }
            }
            Action::Color(color) => {
                if let Mode::Edit(e) = &mut self.mode {
                    e.color = color;
                }
            }
            Action::CycleDimension => {
                if let Mode::Edit(e) = &mut self.mode {
                    e.dimension = next_dimension(&e.dimension).to_owned();
                }
            }
            Action::ToggleHidden => {
                if let Mode::Edit(e) = &mut self.mode {
                    e.hidden = !e.hidden;
                }
            }
            Action::Here => {
                if let (Mode::Edit(e), Some(here)) = (&mut self.mode, here) {
                    e.coords = here.pos.map(|v| trim(whole(v)));
                    e.dimension = here.dimension.clone();
                    e.error = None;
                }
            }
        }
    }

    /// A key pressed over the mod's screen (all of them are the mod's).
    /// `list_key` and `create_key` are the player's rebindable keys.
    fn key(&mut self, key: &str, shift: bool, here: Option<&Here>, list_key: &str, create_key: &str) {
        match self.mode.clone() {
            Mode::Closed => {}
            Mode::List => {
                // Any key but Delete drops a pending delete confirmation, so
                // the second press always has to be on the same row.
                if key != "key.keyboard.delete" {
                    self.confirm_delete = None;
                }
                match key {
                    "key.keyboard.escape" => self.close(),
                    "key.keyboard.up" => self.move_selection(-1),
                    "key.keyboard.down" => self.move_selection(1),
                    "key.keyboard.pageup" => self.move_selection(-(self.visible_rows.max(1) as isize)),
                    "key.keyboard.pagedown" => self.move_selection(self.visible_rows.max(1) as isize),
                    "key.keyboard.home" => {
                        let rows = self.filtered();
                        if let Some(&first) = rows.first() {
                            self.selection = Some(first);
                            self.scroll = 0;
                        }
                    }
                    "key.keyboard.end" => {
                        let rows = self.filtered();
                        if let Some(&last) = rows.last() {
                            self.selection = Some(last);
                            let pos = rows.len() - 1;
                            self.reveal(pos, rows.len());
                        }
                    }
                    "key.keyboard.enter" | "key.keyboard.keypad.enter" => {
                        let rows = self.filtered();
                        let index = self
                            .selection
                            .filter(|s| rows.contains(s))
                            .or_else(|| rows.first().copied());
                        if let Some(index) = index {
                            self.open_edit(index);
                        }
                    }
                    "key.keyboard.delete" => {
                        let rows = self.filtered();
                        let index = self
                            .selection
                            .filter(|s| rows.contains(s))
                            .or_else(|| rows.first().copied());
                        if let Some(index) = index {
                            self.delete(index);
                        }
                    }
                    k if k == list_key || k == ALT_LIST => self.close(),
                    k if k == create_key => {
                        if let Some(here) = here {
                            self.open_create(here, true, key_char(k));
                        }
                    }
                    _ => {}
                }
            }
            Mode::Edit(editor) => match key {
                "key.keyboard.escape" => self.leave_editor(editor.from_list),
                "key.keyboard.enter" | "key.keyboard.keypad.enter" => self.save(),
                "key.keyboard.tab" => {
                    if let Mode::Edit(e) = &mut self.mode {
                        e.focus = e.focus.step(shift);
                    }
                }
                "key.keyboard.up" => {
                    if let Mode::Edit(e) = &mut self.mode {
                        e.focus = e.focus.step(true);
                    }
                }
                "key.keyboard.down" => {
                    if let Mode::Edit(e) = &mut self.mode {
                        e.focus = e.focus.step(false);
                    }
                }
                "key.keyboard.backspace" => {
                    if let Mode::Edit(e) = &mut self.mode {
                        if e.focus == Field::Name && e.suggested {
                            e.name.clear();
                        } else {
                            e.field(e.focus).pop();
                        }
                        e.suggested = false;
                        e.error = None;
                    }
                }
                "key.keyboard.delete" => {
                    if let Mode::Edit(e) = &mut self.mode {
                        e.field(e.focus).clear();
                        e.error = None;
                    }
                }
                _ => {}
            },
        }
    }

    /// A character typed over the mod's screen.
    fn char_typed(&mut self, c: char) {
        if let Some(skip) = self.skip_char.take() {
            if skip.eq_ignore_ascii_case(&c) {
                return;
            }
        }
        let Mode::Edit(editor) = &mut self.mode else {
            return;
        };
        let focus = editor.focus;
        if focus == Field::Name && std::mem::take(&mut editor.suggested) {
            editor.name.clear();
        }
        let field = editor.field(focus);
        let ok = match focus {
            Field::Name => field.chars().count() < NAME_MAX && !c.is_control(),
            Field::X | Field::Y | Field::Z => {
                field.len() < COORD_MAX
                    && (c.is_ascii_digit()
                        || (c == '-' && field.is_empty())
                        || (c == '.' && !field.contains('.')))
            }
        };
        if ok {
            field.push(c);
            editor.error = None;
        }
    }

    /// The wheel over the list: one row per step.
    fn scroll_by(&mut self, steps: f64) {
        if matches!(self.mode, Mode::List) {
            let rows = self.filtered().len();
            if steps > 0.0 {
                self.scroll = self.scroll.saturating_sub(1);
            } else if steps < 0.0 {
                self.scroll = self.scroll.saturating_add(1);
            }
            self.clamp_scroll(rows);
        }
    }

    /// This dimension's visible waypoints, for the HUD and the minimap.
    fn visible_in(&self, dimension: &str) -> Vec<Waypoint> {
        self.waypoints
            .iter()
            .filter(|w| w.dim == dimension && !w.hidden)
            .cloned()
            .collect()
    }

    /// The minimap markers: `([x, z], colour, death)`, hidden waypoints left
    /// out. Death waypoints are drawn as a red X instead of a dot.
    fn minimap_dots(&self, dimension: &str) -> Vec<([f64; 2], u32, bool)> {
        self.waypoints
            .iter()
            .filter(|w| w.dim == dimension && !w.hidden)
            .map(|w| ([w.x, w.z], w.color, w.death))
            .collect()
    }

    /// The open screen's drawing and clickable areas for a `size` GUI.
    fn screen(
        &mut self,
        size: [f32; 2],
        mouse: [f32; 2],
        here: Option<&Here>,
        measure: &dyn Fn(&str) -> f32,
    ) -> (Vec<Draw>, Vec<Widget>) {
        let mut out = Out::new(mouse, measure);
        out.fill([0.0, 0.0, size[0], size[1]], SHADE);
        match self.mode.clone() {
            Mode::Closed => {}
            Mode::List => self.draw_list(&mut out, size, here),
            Mode::Edit(editor) => draw_editor(&mut out, size, &editor),
        }
        (out.draws, out.widgets)
    }

    fn draw_list(&mut self, out: &mut Out<'_>, size: [f32; 2], here: Option<&Here>) {
        let pw = (size[0] - 16.0).min(340.0);
        let ph = (size[1] - 16.0).max(120.0);
        let px = ((size[0] - pw) / 2.0).round();
        let py = 8.0;
        out.boxed([px, py, pw, ph], PANEL_EDGE, PANEL);
        let right = px + pw - 6.0;

        let title = format!("Waypoints ({})", self.waypoints.len());
        out.text(&title, [px + 6.0, py + 6.0], INK);
        let close_w = out.width("Close") + 8.0;
        out.button(
            [right - close_w, py + 4.0, close_w, 12.0],
            "Close",
            Action::Close,
            false,
        );
        let new_w = out.width("+ New") + 8.0;
        out.button(
            [right - close_w - 4.0 - new_w, py + 4.0, new_w, 12.0],
            "+ New",
            Action::NewWaypoint,
            false,
        );

        let mut x = px + 6.0;
        for filter in Filter::ALL {
            let on = self.filter == filter;
            x += out.button_auto(x, py + 20.0, filter.label(), Action::Filter(filter), on) + 3.0;
        }

        let rows = self.filtered();
        let top = py + 38.0;
        let bottom = py + ph - 16.0;
        let fits = (((bottom - top) / ROW).floor() as usize).max(1);
        self.visible_rows = fits;
        self.scroll = self.scroll.min(rows.len().saturating_sub(fits));
        if let Some(selected) = self.selection {
            if let Some(pos) = rows.iter().position(|&i| i == selected) {
                if pos < self.scroll {
                    self.scroll = pos;
                } else if pos >= self.scroll + fits {
                    self.scroll = pos + 1 - fits;
                }
            }
        }

        if rows.is_empty() {
            let msg = if self.waypoints.is_empty() {
                "No waypoints yet. Press N in game or click + New."
            } else if self.filter == Filter::Deaths {
                "No deaths recorded yet."
            } else {
                "No waypoints in this dimension."
            };
            out.text(msg, [px + 8.0, top + 6.0], MUTED);
        }

        for (slot, &i) in rows.iter().skip(self.scroll).take(fits).enumerate() {
            let w = self.waypoints[i].clone();
            let ry = top + slot as f32 * ROW;
            let selected = self.selection == Some(i);
            out.fill(
                [px + 4.0, ry, pw - 8.0, ROW - 2.0],
                if selected { ROW_SELECTED } else { ROW_BG },
            );
            // The row itself is clickable (selection); buttons drawn later win
            // the hit test.
            out.widgets.push(Widget {
                rect: [px + 4.0, ry, pw - 8.0, ROW - 2.0],
                action: Action::Select(i),
            });

            let del = if self.confirm_delete == Some(i) {
                "Sure?"
            } else {
                "Delete"
            };
            let del_w = out.width(del) + 8.0;
            let edit_w = out.width("Edit") + 8.0;
            let vis = if w.hidden { "Show" } else { "Hide" };
            let vis_w = out.width("Show").max(out.width("Hide")) + 8.0;
            let by = ry + (ROW - 2.0 - 12.0) / 2.0;
            let mut bx = right - del_w;
            out.button([bx, by, del_w, 12.0], del, Action::Delete(i), false);
            bx -= edit_w + 3.0;
            out.button([bx, by, edit_w, 12.0], "Edit", Action::Edit(i), false);
            bx -= vis_w + 3.0;
            out.button([bx, by, vis_w, 12.0], vis, Action::Toggle(i), w.hidden);

            // Dimension and distance, right-aligned before the buttons.
            let dim = dimension_label(&w.dim);
            let dist = match here {
                Some(h) if h.dimension == w.dim => distance_text(distance(h.pos, w.pos())),
                _ => "other dimension".into(),
            };
            let info_right = bx - 6.0;
            let dim_w = out.width(&dim);
            let dist_w = out.width(&dist);
            out.text(&dim, [info_right - dim_w, ry + 3.0], dimension_color(&w.dim));
            out.text(&dist, [info_right - dist_w, ry + 3.0 + LINE], MUTED);

            // Colour swatch, name and coordinates.
            out.fill([px + 8.0, ry + 6.0, 10.0, 10.0], 0xFF00_0000);
            out.fill([px + 9.0, ry + 7.0, 8.0, 8.0], 0xFF00_0000 | w.color);
            let text_x = px + 22.0;
            let room = info_right - dim_w.max(dist_w) - 6.0 - text_x;
            let name = out.fit(&w.name, room);
            out.text(
                &name,
                [text_x, ry + 3.0],
                if w.hidden { MUTED } else { INK },
            );
            let coords = out.fit(&w.display_coords(), room);
            out.text(&coords, [text_x, ry + 3.0 + LINE], MUTED);
        }

        let more = rows.len().saturating_sub(self.scroll + fits);
        let hint = if self.scroll > 0 || more > 0 {
            format!("Scroll for more ({} above, {} below)", self.scroll, more)
        } else {
            "N new waypoint  -  B or Esc close".to_owned()
        };
        out.text(&hint, [px + 6.0, py + ph - 12.0], MUTED);
    }
}

fn draw_editor(out: &mut Out<'_>, size: [f32; 2], e: &Editor) {
    let pw = (size[0] - 16.0).min(270.0);
    let ph = 196.0_f32.min(size[1] - 8.0);
    let px = ((size[0] - pw) / 2.0).round();
    let py = ((size[1] - ph) / 2.0).round().max(4.0);
    out.boxed([px, py, pw, ph], PANEL_EDGE, PANEL);
    let left = px + 6.0;
    let inner = pw - 12.0;

    let title = if e.index.is_some() {
        "Edit waypoint"
    } else {
        "New waypoint"
    };
    out.text(title, [left, py + 6.0], INK);

    out.text("Name", [left, py + 20.0], MUTED);
    text_field(
        out,
        [left, py + 30.0, inner, 14.0],
        &e.name,
        e.focus == Field::Name,
        Field::Name,
    );

    let third = (inner - 8.0) / 3.0;
    for (i, (field, label)) in [(Field::X, "X"), (Field::Y, "Y"), (Field::Z, "Z")]
        .into_iter()
        .enumerate()
    {
        let x = left + i as f32 * (third + 4.0);
        out.text(label, [x, py + 48.0], MUTED);
        text_field(
            out,
            [x, py + 58.0, third, 14.0],
            &e.coords[i],
            e.focus == field,
            field,
        );
    }

    let here_w = out.width("Use my position") + 8.0;
    out.button(
        [left, py + 76.0, here_w, 12.0],
        "Use my position",
        Action::Here,
        false,
    );

    let dim = format!("Dimension: {}", dimension_label(&e.dimension));
    let dim_w = out.width(&dim) + 8.0;
    out.button(
        [left + inner - dim_w, py + 76.0, dim_w, 12.0],
        &dim,
        Action::CycleDimension,
        false,
    );

    out.text("Colour", [left, py + 94.0], MUTED);
    let swatch = ((inner + 2.0) / PALETTE.len() as f32 - 2.0)
        .min(12.0)
        .floor();
    for (i, &color) in PALETTE.iter().enumerate() {
        let x = left + i as f32 * (swatch + 2.0);
        let rect = [x, py + 104.0, swatch, swatch];
        let edge = if color == e.color {
            FIELD_FOCUS
        } else {
            0xFF00_0000
        };
        out.boxed(rect, edge, 0xFF00_0000 | color);
        if color == e.color {
            out.fill(
                [x + 2.0, py + 106.0, swatch - 4.0, swatch - 4.0],
                0xFF00_0000 | color,
            );
        }
        out.widgets.push(Widget {
            rect,
            action: Action::Color(color),
        });
    }

    let vis = if e.hidden {
        "Visible in world: No"
    } else {
        "Visible in world: Yes"
    };
    out.button_auto(left, py + 122.0, vis, Action::ToggleHidden, !e.hidden);

    if let Some(error) = &e.error {
        out.text(error, [left, py + 138.0], ERROR);
    } else {
        out.text(
            "Enter save  -  Tab next field  -  Esc cancel",
            [left, py + 138.0],
            MUTED,
        );
    }

    let bw = (inner - 4.0) / 2.0;
    out.button([left, py + ph - 20.0, bw, 14.0], "Save", Action::Save, false);
    out.button(
        [left + bw + 4.0, py + ph - 20.0, bw, 14.0],
        "Cancel",
        Action::Cancel,
        false,
    );
}

fn text_field(out: &mut Out<'_>, rect: [f32; 4], text: &str, focused: bool, field: Field) {
    out.boxed(
        rect,
        if focused { FIELD_FOCUS } else { FIELD_EDGE },
        FIELD,
    );
    let shown = if focused {
        format!("{text}_")
    } else {
        text.to_owned()
    };
    let max = rect[2] - 6.0;
    let mut tail = shown.as_str();
    while out.width(tail) > max && !tail.is_empty() {
        let mut chars = tail.chars();
        chars.next();
        tail = chars.as_str();
    }
    out.text(tail, [rect[0] + 3.0, rect[1] + 3.0], INK);
    out.widgets.push(Widget {
        rect,
        action: Action::Focus(field),
    });
}

// ---- Projection and HUD ----------------------------------------------------

/// The camera of the last frame, as the locator bar uses it.
#[derive(Clone, Copy)]
struct Cam {
    pos: [f64; 3],
    forward: [f64; 3],
    right: [f64; 3],
    up: [f64; 3],
    focal: f64,
    half: [f64; 2],
}

fn camera(game: &Game<'_>, width: f32, height: f32) -> Option<Cam> {
    let (pos, yaw, pitch, fov) = game.locator_camera?;
    let (sy, cy) = (f64::from(yaw).to_radians()).sin_cos();
    let (sp, cp) = (f64::from(pitch).to_radians()).sin_cos();
    Some(Cam {
        pos,
        forward: [-sy * cp, -sp, cy * cp],
        right: [-cy, 0.0, -sy],
        up: [-sy * sp, cp, cy * sp],
        focal: 1.0 / (f64::from(fov) / 2.0).to_radians().tan(),
        half: [f64::from(width) / 2.0, f64::from(height) / 2.0],
    })
}

impl Cam {
    fn offset(&self, p: [f64; 3]) -> [f64; 3] {
        [
            p[0] - self.pos[0],
            p[1] - self.pos[1],
            p[2] - self.pos[2],
        ]
    }

    fn distance(&self, p: [f64; 3]) -> f64 {
        let o = self.offset(p);
        (o[0] * o[0] + o[1] * o[1] + o[2] * o[2]).sqrt()
    }

    fn project(&self, p: [f64; 3]) -> Option<[f32; 2]> {
        let o = self.offset(p);
        let z = o[0] * self.forward[0] + o[1] * self.forward[1] + o[2] * self.forward[2];
        if z <= 0.05 {
            return None;
        }
        let x = o[0] * self.right[0] + o[1] * self.right[1] + o[2] * self.right[2];
        let y = o[0] * self.up[0] + o[1] * self.up[1] + o[2] * self.up[2];
        // `focal` is 1 / tan(vertical FOV / 2), so the horizontal NDC also
        // divides by the screen's aspect; without it a wide window stretches
        // every marker sideways and they slide off the world when turning.
        let aspect = self.half[0] / self.half[1];
        let nx = self.focal * x / (z * aspect);
        let ny = self.focal * y / z;
        Some([
            ((nx + 1.0) * self.half[0]) as f32,
            ((1.0 - ny) * self.half[1]) as f32,
        ])
    }

}

fn metres(d: f64) -> String {
    if d >= 1000.0 {
        format!("{:.1} km", d / 1000.0)
    } else {
        format!("{} m", d.round() as i64)
    }
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    let (dx, dy, dz) = (a[0] - b[0], a[1] - b[1], a[2] - b[2]);
    (dx * dx + dy * dy + dz * dz).sqrt()
}

fn distance_text(d: f64) -> String {
    metres(d)
}

fn dimension_label(dim: &str) -> String {
    match dim.trim_start_matches("minecraft:") {
        "the_nether" => "Nether".to_owned(),
        "the_end" => "End".to_owned(),
        "overworld" => "Overworld".to_owned(),
        other => other.to_owned(),
    }
}

fn dimension_color(dim: &str) -> u32 {
    match dim.trim_start_matches("minecraft:") {
        "the_nether" => 0xFF_5555,
        "the_end" => 0xBB_88FF,
        _ => 0x55_FF55,
    }
}

fn next_dimension(dim: &str) -> &'static str {
    match dim.trim_start_matches("minecraft:") {
        "overworld" => NETHER,
        "the_nether" => END,
        _ => OVERWORLD,
    }
}

fn lighter(color: u32) -> u32 {
    let [_, r, g, b] = color.to_be_bytes();
    let up = |v: u8| v.saturating_add((255 - v) / 3);
    (u32::from(up(r)) << 16) | (u32::from(up(g)) << 8) | u32::from(up(b))
}

fn dot(batch: &mut HudBatch, atlas: &HudAtlas, [x, y]: [f32; 2], color: u32, size: f32) {
    let [_, r, g, b] = color.to_be_bytes();
    batch.fill(
        atlas,
        x - size - 1.0,
        y - size - 1.0,
        size * 2.0 + 2.0,
        size * 2.0 + 2.0,
        rgba(0, 0, 0, 150),
    );
    batch.fill(
        atlas,
        x - size,
        y - size,
        size * 2.0,
        size * 2.0,
        rgba(r, g, b, 255),
    );
}

/// A death waypoint's marker: a red X rather than a coloured dot, so it is
/// unmistakable in the world.
fn cross(batch: &mut HudBatch, atlas: &HudAtlas, [x, y]: [f32; 2], color: u32, size: f32) {
    let [_, r, g, b] = color.to_be_bytes();
    // A dark, slightly wider X behind keeps the marker legible on any terrain.
    for (half, colour) in [
        (size + 1.0, rgba(0, 0, 0, 150)),
        (size, rgba(r, g, b, 255)),
    ] {
        let t = half * 0.45;
        // One diagonal stroke, top-left to bottom-right.
        let c = [
            [x - half + t, y - half - t],
            [x + half + t, y + half - t],
            [x + half - t, y + half + t],
            [x - half - t, y - half + t],
        ];
        batch.fill_quad(atlas, c, colour);
        // The other diagonal, top-right to bottom-left.
        let c = [
            [x + half + t, y - half + t],
            [x - half + t, y + half + t],
            [x - half - t, y + half - t],
            [x + half - t, y - half - t],
        ];
        batch.fill_quad(atlas, c, colour);
    }
}

fn draw_world_markers(game: &Game<'_>, batch: &mut HudBatch, dim: &str, labels: bool) {
    let Some(cam) = camera(game, batch.width, batch.height) else {
        return;
    };
    let mut waypoints = with(|s| s.visible_in(dim));
    waypoints.sort_by(|a, b| cam.distance(b.pos()).total_cmp(&cam.distance(a.pos())));
    let (w, h) = (batch.width, batch.height);
    let atlas = &game.gui.atlas;
    for wp in &waypoints {
        // Outside the view a waypoint is not drawn at all: no dot pinned to
        // the screen edge for something the player cannot see.
        let Some(at) = marker_at(&cam, wp.pos()) else {
            continue;
        };
        if wp.death {
            cross(batch, atlas, at, DEATH_COLOR, 4.0);
        } else {
            dot(batch, atlas, at, wp.color, 3.0);
        }
        if !labels {
            continue;
        }
        let text = format!("{} · {}", wp.name, metres(cam.distance(wp.pos())));
        let width = game.native_text_width(&text);
        let x = (at[0] - width / 2.0).clamp(2.0, (w - width - 2.0).max(2.0));
        let y = (at[1] - 13.0).clamp(2.0, h - 12.0);
        game.native_text(batch, &text, [x, y], lighter(wp.color), true);
    }
}

/// Where a waypoint's marker goes: its projection, or None when it is behind
/// the camera or outside the view. Off-screen waypoints are hidden rather
/// than pinned to the screen edge, so only what the player can see is drawn.
fn marker_at(cam: &Cam, p: [f64; 3]) -> Option<[f32; 2]> {
    let at = cam.project(p)?;
    let size = [cam.half[0] * 2.0, cam.half[1] * 2.0];
    let visible = at[0] >= 0.0
        && at[0] <= size[0] as f32
        && at[1] >= 0.0
        && at[1] <= size[1] as f32;
    visible.then_some(at)
}

fn draw_panel(game: &Game<'_>, batch: &mut HudBatch, corners: &mut Corners, dim: &str) {
    // F3's debug screen draws a chart in the bottom left while it is open.
    if !game.native_corner_shown(Corner::BottomLeft) {
        return;
    }
    let Some(cam) = camera(game, batch.width, batch.height) else {
        return;
    };
    let mut lines: Vec<(String, f64)> = with(|s| s.visible_in(dim))
        .iter()
        .map(|wp| {
            (
                format!("{}  {}", wp.name, metres(cam.distance(wp.pos()))),
                cam.distance(wp.pos()),
            )
        })
        .collect();
    if lines.is_empty() {
        return;
    }
    lines.sort_by(|a, b| a.1.total_cmp(&b.1));
    lines.truncate(6);
    let width = lines
        .iter()
        .map(|(text, _)| game.native_text_width(text))
        .fold(0.0, f32::max)
        + 8.0;
    let height = 10.0 * lines.len() as f32 + 5.0;
    let (x, y) = corners.place(Corner::BottomLeft, width, height);
    game.native_fill(batch, [x, y, width, height], 0x7800_0000);
    for (i, (text, _)) in lines.iter().enumerate() {
        game.native_text(
            batch,
            text,
            [x + 4.0, y + 3.0 + 10.0 * i as f32],
            0xFF_FFFF,
            true,
        );
    }
}

// ---- Context, settings and lifecycle ---------------------------------------

fn current_context(game: &Game<'_>) -> Option<ContextKey> {
    if let Some(remote) = game.remote.as_ref() {
        Some(ContextKey::Multiplayer(remote.session.address.clone()))
    } else {
        game.integrated
            .as_ref()
            .map(|i| ContextKey::Singleplayer(i.world_name.clone()))
    }
}

fn update_context(game: &Game<'_>) {
    let key = current_context(game);
    let dir = game.native_data_dir(MOD_ID);
    with(|s| s.switch_to(key, dir));
}

/// Reads the mod's settings once, so toggles survive restarts.
fn load_settings(game: &Game<'_>) {
    let markers = game.native_setting(MOD_ID, "markers");
    let labels = game.native_setting(MOD_ID, "labels");
    let panel = game.native_setting(MOD_ID, "panel");
    with(|s| {
        if s.settings_loaded {
            return;
        }
        s.markers = setting_bool(markers.as_deref(), true);
        s.labels = setting_bool(labels.as_deref(), true);
        s.panel = setting_bool(panel.as_deref(), true);
        s.settings_loaded = true;
    });
}

/// Once a frame (`native_frame`): migrate a v1 file, follow the world, feed
/// the minimap, and close/open the screen as the state asks.
pub(super) fn frame(game: &mut Game<'_>) {
    if !game.native_window() {
        return;
    }
    let dir = game.native_data_dir(MOD_ID);
    let _ = with(|s| s.migrate_legacy(&dir));
    load_settings(game);
    update_context(game);

    // The death screen opening is the moment the player died; the client is
    // still standing where it happened until a respawn.
    let dead = game.screen == crate::game::Screen::Dead;
    let here = game.native_player().map(|p| Here {
        pos: [p.x, p.y, p.z],
        dimension: p.dimension.to_owned(),
    });
    if let Some(message) = with(|s| s.observe_death(dead, here.as_ref())) {
        game.native_chat(&message);
    }

    let dots = if game.screen == crate::game::Screen::Chat {
        Vec::new()
    } else if game.native_on(MOD_ID) {
        match game.native_player() {
            Some(p) => {
                let dim = p.dimension.to_owned();
                with(|s| s.minimap_dots(&dim))
            }
            None => Vec::new(),
        }
    } else {
        Vec::new()
    };
    horizon_client::extras_hud::waymap_minimap::set_dots(dots);

    let close = with(|s| s.take_close());
    if close {
        game.native_mod_screen_close(MOD_ID);
    }
    let ours = game.native_mod_screen() == Some(MOD_ID);
    with(|s| s.sync_open(ours));
}

/// The mod's row in Options > Mods (from `native_mods`).
pub(super) fn listed(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    mods.push(crate::game::native_hooks::NativeMod {
        id: MOD_ID.into(),
        title: format!("{NAME} {VERSION}"),
        description: DESCRIPTION.into(),
        keys: vec![
            [
                KEY_CREATE.into(),
                "Create waypoint".into(),
                DEFAULT_CREATE.into(),
            ],
            [KEY_LIST.into(), "Waypoint list".into(), DEFAULT_LIST.into()],
        ],
    });
}

/// The buttons of the mod's settings page (from `native_settings`).
pub(super) fn settings(game: &mut Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    if mod_id != MOD_ID {
        return;
    }
    load_settings(game);
    let (markers, labels, panel) = with(|s| (s.markers, s.labels, s.panel));
    buttons.push(format!("Markers: {}", on_off(markers)));
    buttons.push(format!("Labels: {}", on_off(labels)));
    buttons.push(format!("List panel: {}", on_off(panel)));
}

/// A click on the settings page (from `native_settings_click`).
pub(super) fn click(game: &mut Game<'_>, mod_id: &str, button: usize) {
    if mod_id != MOD_ID {
        return;
    }
    load_settings(game);
    let changed = with(|s| match button {
        0 => {
            s.markers = !s.markers;
            Some(("markers", s.markers))
        }
        1 => {
            s.labels = !s.labels;
            Some(("labels", s.labels))
        }
        2 => {
            s.panel = !s.panel;
            Some(("panel", s.panel))
        }
        _ => None,
    });
    let Some((key, on)) = changed else {
        return;
    };
    game.native_set_setting(MOD_ID, key, on_off(on));
    game.native_chat(&format!("{NAME}: {key} {}", on_off(on)));
}

// ---- Commands --------------------------------------------------------------

/// `/wp ...` (from `native_command`).
pub(super) fn command(game: &mut Game<'_>, command: &str, ran: &mut bool) {
    if *ran || !game.native_window() || !game.native_on(MOD_ID) {
        return;
    }
    let Some(rest) = command
        .strip_prefix("wp")
        .filter(|rest| rest.is_empty() || rest.starts_with(' '))
    else {
        return;
    };
    update_context(game);
    *ran = true;
    let rest = rest.trim();
    let (sub, args) = rest.split_once(' ').unwrap_or((rest, ""));
    let args = args.trim();
    let message = match sub {
        "" | "help" => help(),
        "add" => add_here(game, (!args.is_empty()).then(|| clean_name(args))),
        "del" | "delete" | "remove" => remove(args),
        "list" => list(),
        "rename" => rename(args),
        "color" | "colour" => recolor(args),
        "clear" => clear(),
        "set" | "edit" => edit(args),
        "hide" => set_hidden(args, true),
        "show" => set_hidden(args, false),
        other => format!("Unknown: /wp {other}. /wp help shows commands."),
    };
    for line in message.lines() {
        game.native_chat(line);
    }
}

fn help() -> String {
    format!(
        "{NAME} {VERSION}:\n\
         /wp add [name]          - Add waypoint at current position\n\
         /wp del <name>          - Delete waypoint\n\
         /wp list                - List all waypoints\n\
         /wp rename <old> <new>  - Rename waypoint (quoted names work)\n\
         /wp color <name> <color> - Change colour (name or #rrggbb)\n\
         /wp clear               - Clear all waypoints in this context\n\
         /wp edit <name>         - Show waypoint info\n\
         /wp hide <name>         - Hide from world and minimap\n\
         /wp show <name>         - Show a hidden waypoint\n\
         /wp help                - Show this help\n\
         Keys: N create, B or M list (Options > Mods > Waymap)"
    )
}

fn add_here(game: &Game<'_>, name: Option<String>) -> String {
    let Some(p) = game.native_player() else {
        return "No player in a world.".to_owned();
    };
    let here = Here {
        pos: [p.x, p.y, p.z],
        dimension: p.dimension.to_owned(),
    };
    let mut message = String::new();
    with(|s| {
        if s.waypoints.len() >= MAX_WAYPOINTS {
            message = format!("Cannot have more than {MAX_WAYPOINTS} waypoints.");
            return;
        }
        let name = name.unwrap_or_else(|| auto_name(&s.waypoints));
        if s.waypoints
            .iter()
            .any(|w| w.name.eq_ignore_ascii_case(&name))
        {
            message = format!("'{name}' already exists.");
            return;
        }
        let color = default_color(s.waypoints.len());
        s.waypoints.push(Waypoint {
            name: name.clone(),
            x: whole(here.pos[0]),
            y: whole(here.pos[1]),
            z: whole(here.pos[2]),
            dim: here.dimension.clone(),
            color,
            hidden: false,
            death: false,
        });
        match s.persist() {
            Ok(()) => {
                message = format!(
                    "Waypoint '{name}' set: {} {} {} ({}).",
                    trim(whole(here.pos[0])),
                    trim(whole(here.pos[1])),
                    trim(whole(here.pos[2])),
                    color_name(color)
                );
            }
            Err(e) => message = format!("Waypoint added, not saved: {e}"),
        }
    });
    message
}

fn remove(args: &str) -> String {
    let name = unquote(args.trim());
    if name.is_empty() {
        return "Which waypoint? /wp del <name>".to_owned();
    }
    let mut message = String::new();
    with(|s| {
        let Some(index) = s
            .waypoints
            .iter()
            .position(|w| w.name.eq_ignore_ascii_case(&name))
        else {
            message = format!("No waypoint '{name}'. Use /wp list to see all.");
            return;
        };
        let removed = s.waypoints.remove(index);
        s.clamp_selection();
        match s.persist() {
            Ok(()) => message = format!("Waypoint '{}' deleted.", removed.name),
            Err(e) => message = format!("Waypoint removed, not saved: {e}"),
        }
    });
    message
}

fn list() -> String {
    with(|s| {
        if s.waypoints.is_empty() {
            return format!("{NAME}: no waypoints. Use /wp add [name] to add one.");
        }
        let mut text = format!("{NAME}: {} waypoints", s.waypoints.len());
        if let Some(ctx) = s.context.as_ref() {
            text.push_str(&format!(" ({})", ctx.display_name()));
        }
        for (i, w) in s.waypoints.iter().enumerate().take(20) {
            let _ = write!(
                text,
                "\n{}. {} · {} · {}{} · {}",
                i + 1,
                w.name,
                w.display_coords(),
                short_dim(&w.dim),
                if w.hidden { " (hidden)" } else { "" },
                color_name(w.color)
            );
        }
        if s.waypoints.len() > 20 {
            let _ = write!(text, "\n... and {} more", s.waypoints.len() - 20);
        }
        text
    })
}

/// Splits `old new`, honouring a quoted old name.
fn split_pair(args: &str) -> Option<(String, String)> {
    let args = args.trim();
    if args.is_empty() {
        return None;
    }
    if let Some(rest) = args.strip_prefix('"') {
        let end = rest.find('"')?;
        let old = rest[..end].trim().to_owned();
        let new = rest[end + 1..].trim();
        let new = unquote(new);
        return (!old.is_empty() && !new.is_empty()).then_some((old, new));
    }
    let (old, new) = args.split_once(' ')?;
    let (old, new) = (old.trim(), unquote(new.trim()));
    (!old.is_empty() && !new.is_empty()).then_some((old.to_owned(), new))
}

fn unquote(text: &str) -> String {
    let text = text.trim();
    text.strip_prefix('"')
        .and_then(|t| t.strip_suffix('"'))
        .unwrap_or(text)
        .to_owned()
}

fn rename(args: &str) -> String {
    let Some((old, new)) = split_pair(args) else {
        return "Usage: /wp rename <old> <new>".to_owned();
    };
    let new = clean_name(&new);
    let mut message = String::new();
    with(|s| {
        if s.waypoints
            .iter()
            .any(|w| w.name.eq_ignore_ascii_case(&new))
        {
            message = format!("'{new}' already exists.");
            return;
        }
        let Some(index) = s
            .waypoints
            .iter()
            .position(|w| w.name.eq_ignore_ascii_case(&old))
        else {
            message = format!("No waypoint '{old}'.");
            return;
        };
        s.waypoints[index].name = new.clone();
        match s.persist() {
            Ok(()) => message = format!("'{old}' renamed to '{new}'."),
            Err(e) => message = format!("Renamed, not saved: {e}"),
        }
    });
    message
}

fn recolor(args: &str) -> String {
    let args = args.trim();
    if args.is_empty() {
        return colors_help();
    }
    let (name, color_text) = if let Some(rest) = args.strip_prefix('"') {
        match rest.find('"') {
            Some(end) => {
                let name = rest[..end].trim().to_owned();
                let color = rest[end + 1..].trim().to_owned();
                (name, color)
            }
            None => return colors_help(),
        }
    } else {
        match args.rsplit_once(' ') {
            Some((name, color)) => (name.trim().to_owned(), color.trim().to_owned()),
            None => return colors_help(),
        }
    };
    if name.is_empty() || color_text.is_empty() {
        return colors_help();
    }
    let Some(color) = parse_color(&color_text) else {
        return format!("Unknown colour '{color_text}'. {}", colors_help());
    };
    let mut message = String::new();
    with(|s| {
        let Some(w) = s
            .waypoints
            .iter_mut()
            .find(|w| w.name.eq_ignore_ascii_case(&name))
        else {
            message = format!("No waypoint '{name}'.");
            return;
        };
        w.color = color;
        let name = w.name.clone();
        match s.persist() {
            Ok(()) => message = format!("'{name}' is now {}.", color_name(color)),
            Err(e) => message = format!("Recoloured, not saved: {e}"),
        }
    });
    message
}

fn colors_help() -> String {
    let names: Vec<&str> = COLORS.iter().map(|(name, _)| *name).collect();
    format!("Colours: {} or #rrggbb", names.join(", "))
}

fn clear() -> String {
    let mut message = String::new();
    with(|s| {
        let n = s.waypoints.len();
        s.waypoints.clear();
        s.selection = None;
        s.scroll = 0;
        s.confirm_delete = None;
        match s.persist() {
            Ok(()) if n == 0 => message = "No waypoints to clear.".to_owned(),
            Ok(()) => message = format!("{n} waypoints cleared."),
            Err(e) => message = format!("Cleared, not saved: {e}"),
        }
    });
    message
}

fn edit(args: &str) -> String {
    let name = unquote(args);
    with(|s| {
        let Some(w) = s
            .waypoints
            .iter()
            .find(|w| w.name.eq_ignore_ascii_case(&name))
        else {
            return format!("No waypoint '{name}'. Use /wp list to see all.");
        };
        format!(
            "Waypoint '{name}': {} · {} · {} · {}",
            w.display_coords(),
            short_dim(&w.dim),
            color_name(w.color),
            if w.hidden { "hidden" } else { "visible" }
        )
    })
}

fn set_hidden(args: &str, hidden: bool) -> String {
    let name = unquote(args);
    if name.is_empty() {
        return format!("Usage: /wp {}", if hidden { "hide" } else { "show" });
    }
    let mut message = String::new();
    with(|s| {
        let Some(w) = s
            .waypoints
            .iter_mut()
            .find(|w| w.name.eq_ignore_ascii_case(&name))
        else {
            message = format!("No waypoint '{name}'.");
            return;
        };
        w.hidden = hidden;
        let label = w.name.clone();
        match s.persist() {
            Ok(()) if hidden => message = format!("Waypoint '{label}' hidden."),
            Ok(()) => message = format!("Waypoint '{label}' shown."),
            Err(e) => message = format!("Changed, not saved: {e}"),
        }
    });
    message
}

fn clean_name(name: &str) -> String {
    let cleaned: String = name.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        "waypoint".to_owned()
    } else {
        cleaned.chars().take(48).collect()
    }
}

fn auto_name(waypoints: &[Waypoint]) -> String {
    let mut next = 1;
    for w in waypoints {
        if let Some(n) = w.name.strip_prefix("WP ").and_then(|n| n.parse::<u32>().ok()) {
            next = next.max(n + 1);
        }
    }
    format!("WP {next}")
}

fn parse_color(text: &str) -> Option<u32> {
    let lower = text.to_ascii_lowercase();
    if let Some((_, color)) = COLORS.iter().find(|(name, _)| *name == lower) {
        return Some(*color);
    }
    let hex = lower.strip_prefix('#').unwrap_or(&lower);
    (hex.len() == 6).then(|| u32::from_str_radix(hex, 16).ok()).flatten()
}

fn color_name(color: u32) -> String {
    COLORS
        .iter()
        .find(|(_, c)| *c == color)
        .map_or_else(|| format!("#{color:06x}"), |(name, _)| (*name).to_owned())
}

fn default_color(index: usize) -> u32 {
    COLORS[index % COLORS.len()].1
}

fn short_dim(dim: &str) -> &str {
    dim.strip_prefix("minecraft:").unwrap_or(dim)
}

// ---- Screen hooks ----------------------------------------------------------

/// `native_screen_render_post`: the list or the create/edit form.
pub(super) fn screen_render_post(
    game: &Game<'_>,
    batch: &mut HudBatch,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
) {
    if !game.native_on(MOD_ID) || screen.id != "mod" || game.native_mod_screen() != Some(MOD_ID) {
        return;
    }
    update_context(game);
    let here = game.native_player().map(|p| Here {
        pos: [p.x, p.y, p.z],
        dimension: p.dimension.to_owned(),
    });
    let size = [batch.width, batch.height];
    let measure = |text: &str| game.native_text_width(text);
    let draws = with(|s| {
        s.sync_open(true);
        s.screen(size, screen.mouse, here.as_ref(), &measure).0
    });
    for draw in draws {
        match draw {
            Draw::Fill(rect, argb) => game.native_fill(batch, rect, argb),
            Draw::Text(text, at, rgb, shadow) => game.native_text(batch, &text, at, rgb, shadow),
        }
    }
}

/// `native_screen_mouse_button_pressed_pre`: a click does the action of the
/// widget under the pointer.
pub(super) fn screen_mouse_button_pressed_pre(
    game: &Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    button: i32,
    _modifiers: u32,
    canceled: &mut bool,
) {
    if !game.native_on(MOD_ID) || screen.id != "mod" || game.native_mod_screen() != Some(MOD_ID) {
        return;
    }
    *canceled = true;
    if button != 0 {
        return;
    }
    update_context(game);
    let here = game.native_player().map(|p| Here {
        pos: [p.x, p.y, p.z],
        dimension: p.dimension.to_owned(),
    });
    // A mod's screen has no container panel: its rect is the whole window,
    // the size its drawing was laid out for.
    let size = [screen.rect[2], screen.rect[3]];
    let measure = |text: &str| game.native_text_width(text);
    with(|s| {
        let (_, widgets) = s.screen(size, screen.mouse, here.as_ref(), &measure);
        if let Some(action) = hit(&widgets, screen.mouse) {
            s.act(action, here.as_ref());
        }
    });
}

/// `native_screen_mouse_scrolled_pre`: the wheel scrolls the list.
pub(super) fn screen_mouse_scrolled_pre(
    game: &Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    scroll: [f64; 2],
    _modifiers: u32,
    canceled: &mut bool,
) {
    if !game.native_on(MOD_ID) || screen.id != "mod" || game.native_mod_screen() != Some(MOD_ID) {
        return;
    }
    *canceled = true;
    update_context(game);
    with(|s| s.scroll_by(scroll[1]));
}

/// `native_screen_key_pressed_pre`: navigation, editing and closing.
pub(super) fn screen_key_pressed_pre(
    game: &Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    key: &str,
    repeat: bool,
    modifiers: u32,
    canceled: &mut bool,
) {
    if !game.native_on(MOD_ID) || screen.id != "mod" || game.native_mod_screen() != Some(MOD_ID) {
        return;
    }
    *canceled = true;
    update_context(game);
    if repeat && key == "key.keyboard.escape" {
        return;
    }
    let here = game.native_player().map(|p| Here {
        pos: [p.x, p.y, p.z],
        dimension: p.dimension.to_owned(),
    });
    let list_key = game.native_mod_key(MOD_ID, KEY_LIST, DEFAULT_LIST);
    let create_key = game.native_mod_key(MOD_ID, KEY_CREATE, DEFAULT_CREATE);
    let shift = modifiers & 1 != 0;
    with(|s| s.key(key, shift, here.as_ref(), &list_key, &create_key));
}

/// `native_screen_char_typed_pre`: typing goes into the focused field.
pub(super) fn screen_char_typed_pre(
    game: &Game<'_>,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
    c: char,
    _modifiers: u32,
    canceled: &mut bool,
) {
    if !game.native_on(MOD_ID) || screen.id != "mod" || game.native_mod_screen() != Some(MOD_ID) {
        return;
    }
    *canceled = true;
    with(|s| s.char_typed(c));
}

/// The create/list keys while playing: opens the mod's own screen.
pub(super) fn key(game: &mut Game<'_>, key: &str, pressed: bool) {
    if !pressed || !game.native_window() || !game.native_on(MOD_ID) || !game.native_playing() {
        return;
    }
    update_context(game);
    let create = game.native_mod_key(MOD_ID, KEY_CREATE, DEFAULT_CREATE);
    let list = game.native_mod_key(MOD_ID, KEY_LIST, DEFAULT_LIST);
    if key != create && key != list && key != ALT_LIST {
        return;
    }
    let Some(p) = game.native_player() else {
        return;
    };
    let here = Here {
        pos: [p.x, p.y, p.z],
        dimension: p.dimension.to_owned(),
    };
    with(|s| {
        if key == create {
            s.open_create(&here, false, key_char(key));
        } else {
            s.open_list(key_char(key));
        }
    });
    if !game.native_mod_screen_open(MOD_ID) {
        with(|s| s.close());
        with(|s| {
            s.close_requested = false;
        });
    }
}

/// The HUD (from `native_hud`): markers in the world and the list panel. The
/// minimap dots are drawn by the library overlay.
pub(super) fn hud(game: &mut Game<'_>, batch: &mut HudBatch, corners: &mut Corners) {
    if !game.native_on(MOD_ID) {
        return;
    }
    update_context(game);
    let Some(player) = game.native_player() else {
        return;
    };
    if !game.native_playing() {
        return;
    }
    let dim = player.dimension.to_owned();
    let (markers, labels, panel) = with(|s| (s.markers, s.labels, s.panel));
    if markers {
        draw_world_markers(game, batch, &dim, labels);
    }
    if panel {
        draw_panel(game, batch, corners, &dim);
    }
}

#[cfg(test)]
mod runtime_tests {
    use super::*;
    use std::path::Path;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "waymap_rt_{}_{}",
            std::process::id(),
            name
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn state(dir: &Path) -> State {
        let mut s = State::default();
        s.context = Some(ContextKey::Singleplayer("test".to_owned()));
        s.dir = Some(dir.to_path_buf());
        s.loaded = true;
        s
    }

    fn wp(name: &str, x: f64, y: f64, z: f64, dim: &str) -> Waypoint {
        Waypoint {
            name: name.to_owned(),
            x,
            y,
            z,
            dim: dim.to_owned(),
            color: 0xFF_5555,
            hidden: false,
            death: false,
        }
    }

    fn here() -> Here {
        Here {
            pos: [12.5, 64.0, -3.25],
            dimension: OVERWORLD.to_owned(),
        }
    }

    fn measure(text: &str) -> f32 {
        text.chars().count() as f32 * 6.0
    }

    fn reload(dir: &Path) -> Vec<Waypoint> {
        WaypointFileIO::load_file(&dir.join("world_test.txt")).unwrap()
    }

    #[test]
    fn v2_roundtrip_rounds_coordinates() {
        let waypoints = vec![
            wp("Base", 123.456_789_012_345, 64.789_123_4, -45.123_456_7, OVERWORLD),
            wp("Fortress", 100.0, 64.0, 200.0, NETHER),
        ];
        let encoded = WaypointCodec::encode(&waypoints);
        assert!(encoded.starts_with(HEADER));
        let decoded = WaypointCodec::decode(&encoded).unwrap();
        assert_eq!(decoded[0].x, 123.0);
        assert_eq!(decoded[0].y, 65.0);
        assert_eq!(decoded[0].z, -45.0);
        assert_eq!(decoded[1], waypoints[1]);
    }

    #[test]
    fn coordinates_are_rounded_to_whole_numbers() {
        // Decimals in an existing file are rounded when read.
        let dir = temp_dir("rounding");
        let path = dir.join("world_test.txt");
        fs::write(
            &path,
            format!("{HEADER}\nminecraft:overworld\t1.4\t64.5\t-2.6\tff5555\t0\tOdd\n"),
        )
        .unwrap();
        let loaded = WaypointFileIO::load_file(&path).unwrap();
        assert_eq!(loaded[0].x, 1.0);
        assert_eq!(loaded[0].y, 65.0);
        assert_eq!(loaded[0].z, -3.0);

        // What is written back is whole, too.
        WaypointFileIO::save_file(&path, &loaded).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("\t1\t65\t-3\t"), "file kept decimals: {text}");

        // The editor rounds typed decimals when saving.
        let mut s = state(&dir);
        s.open_create(&here(), false, None);
        if let Mode::Edit(e) = &mut s.mode {
            e.coords = ["-0.6".to_owned(), "10.5".to_owned(), "2.49".to_owned()];
        }
        s.act(Action::Save, None);
        let saved = reload(&dir);
        assert_eq!((saved[0].x, saved[0].y, saved[0].z), (-1.0, 11.0, 2.0));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn v2_escapes_names() {
        let waypoints = vec![
            wp("Base\twith\ttabs", 1.0, 2.0, 3.0, OVERWORLD),
            wp("Path\\with\\backslashes", 1.0, 2.0, 3.0, OVERWORLD),
            wp("Line\nbreak", 1.0, 2.0, 3.0, OVERWORLD),
        ];
        let decoded = WaypointCodec::decode(&WaypointCodec::encode(&waypoints)).unwrap();
        assert_eq!(decoded, waypoints);
    }

    #[test]
    fn v1_converts_eye_level_to_feet() {
        let content = "minecraft:overworld\t123.456\t65.620\t-45.123\tff5555\tBase\n";
        let waypoints = LegacyWaypointCodec::decode(content).unwrap();
        assert_eq!(waypoints.len(), 1);
        assert_eq!(waypoints[0].name, "Base");
        assert!((waypoints[0].y - 64.0).abs() < 0.001);
        assert!(!waypoints[0].hidden);
    }

    #[test]
    fn v1_skips_malformed_lines() {
        let content = "minecraft:overworld\t100.0\t65.62\t100.0\tff5555\tBase\n\
                       invalid line\n\
                       minecraft:the_nether\t200.0\t64.0\t200.0\t55ff55\tFortress\n";
        let waypoints = LegacyWaypointCodec::decode(content).unwrap();
        assert_eq!(waypoints.len(), 2);
        assert_eq!(waypoints[0].name, "Base");
        assert_eq!(waypoints[1].name, "Fortress");
    }

    #[test]
    fn v1_comment_is_not_a_v2_header() {
        let dir = temp_dir("comment");
        let path = dir.join("world_test.txt");
        fs::write(&path, "# a comment\nminecraft:overworld\t100.0\t65.62\t100.0\tff5555\tBase\n")
            .unwrap();
        let waypoints = WaypointFileIO::load_file(&path).unwrap();
        assert_eq!(waypoints.len(), 1);
        assert!((waypoints[0].y - 64.0).abs() < 0.001);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn v2_header_only_is_empty() {
        let dir = temp_dir("empty");
        let path = dir.join("world_test.txt");
        fs::write(&path, format!("{HEADER}\n")).unwrap();
        assert!(WaypointFileIO::load_file(&path).unwrap().is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn file_io_writes_v2_header_atomically() {
        let dir = temp_dir("atomic");
        let path = dir.join("world_test.txt");
        WaypointFileIO::save_file(&path, &[wp("Test", 123.456, 64.789, -45.123, OVERWORLD)])
            .unwrap();
        assert!(fs::read_to_string(&path).unwrap().starts_with(HEADER));
        assert!(!path.with_extension("tmp").exists());
        assert_eq!(WaypointFileIO::load_file(&path).unwrap().len(), 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn migration_backs_up_v1_and_writes_v2() {
        let dir = temp_dir("migration");
        let legacy = dir.join("waypoints.txt");
        fs::write(
            &legacy,
            "minecraft:overworld\t100.0\t65.62\t100.0\tff5555\tBase\n",
        )
        .unwrap();

        let mut s = State::default();
        s.migrate_legacy(&dir).unwrap();

        assert!(!legacy.exists());
        assert!(dir.join("waypoints.txt.v1.bak").exists());
        let world = dir.join("world_legacy.txt");
        assert!(world.exists());
        let text = fs::read_to_string(&world).unwrap();
        assert!(text.starts_with(HEADER));
        let loaded = WaypointFileIO::load_file(&world).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "Base");
        assert!((loaded[0].y - 64.0).abs() < 0.001);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn unreadable_file_is_never_overwritten() {
        let dir = temp_dir("unreadable");
        let path = dir.join("world_test.txt");
        let bad = format!("{HEADER}\ninvalid\tdata\n");
        fs::write(&path, &bad).unwrap();

        let mut s = State::default();
        s.switch_to(Some(ContextKey::Singleplayer("test".to_owned())), dir.clone());
        assert!(s.load_failed);
        assert!(s.persist().is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), bad);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn clear_writes_an_empty_file() {
        let dir = temp_dir("clear");
        let mut s = state(&dir);
        s.waypoints = vec![wp("One", 1.0, 2.0, 3.0, OVERWORLD)];
        s.persist().unwrap();
        s.waypoints.clear();
        s.persist().unwrap();
        assert!(reload(&dir).is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn context_switch_saves_and_loads() {
        let dir = temp_dir("context");
        let mut s = State::default();
        let one = ContextKey::Singleplayer("one".to_owned());
        let two = ContextKey::Singleplayer("two".to_owned());

        s.switch_to(Some(one.clone()), dir.clone());
        s.waypoints = vec![wp("WP1", 100.0, 64.0, 100.0, OVERWORLD)];
        s.persist().unwrap();

        s.switch_to(Some(two), dir.clone());
        assert!(s.waypoints.is_empty());

        s.switch_to(Some(one), dir.clone());
        assert_eq!(s.waypoints.len(), 1);
        assert_eq!(s.waypoints[0].name, "WP1");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn create_saves_feet_position_and_returns_to_list() {
        let dir = temp_dir("create");
        let mut s = state(&dir);
        s.open_create(&here(), false, Some('n'));

        // The opening 'n' must not reach the suggested name.
        s.char_typed('n');
        assert_eq!(s.mode, Mode::Edit(Editor {
            index: None,
            name: "Waypoint 1".to_owned(),
            coords: ["13".to_owned(), "64".to_owned(), "-3".to_owned()],
            dimension: OVERWORLD.to_owned(),
            color: PALETTE[0],
            hidden: false,
            focus: Field::Name,
            from_list: false,
            error: None,
            suggested: true,
        }));

        for c in "Base".chars() {
            s.char_typed(c);
        }
        s.act(Action::Save, Some(&here()));

        assert_eq!(s.mode, Mode::List);
        let loaded = reload(&dir);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "Base");
        assert_eq!(loaded[0].x, 13.0);
        assert_eq!(loaded[0].y, 64.0);
        assert_eq!(loaded[0].z, -3.0);
        assert_eq!(loaded[0].dim, OVERWORLD);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn edit_save_replaces_the_same_entry() {
        let dir = temp_dir("edit");
        let mut s = state(&dir);
        s.waypoints = vec![wp("Old", 1.0, 2.0, 3.0, OVERWORLD)];
        s.persist().unwrap();
        s.open_edit(0);
        if let Mode::Edit(e) = &mut s.mode {
            e.name = "New".to_owned();
            e.coords = ["4.5".to_owned(), "5".to_owned(), "6.75".to_owned()];
        }
        s.act(Action::Save, None);

        let loaded = reload(&dir);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "New");
        assert_eq!(loaded[0].x, 5.0);
        assert_eq!(loaded[0].y, 5.0);
        assert_eq!(loaded[0].z, 7.0);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_validation_keeps_the_form_open() {
        let dir = temp_dir("invalid");
        let mut s = state(&dir);
        s.open_create(&here(), false, None);
        if let Mode::Edit(e) = &mut s.mode {
            e.coords[0] = "abc".to_owned();
        }
        s.act(Action::Save, None);

        match &s.mode {
            Mode::Edit(e) => assert!(e.error.is_some()),
            other => panic!("expected editor, got {other:?}"),
        }
        assert!(!dir.join("world_test.txt").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn cancel_discards_changes() {
        let dir = temp_dir("cancel");
        let mut s = state(&dir);
        s.open_create(&here(), false, None);
        for c in "Discarded".chars() {
            s.char_typed(c);
        }
        s.act(Action::Cancel, None);

        assert_eq!(s.mode, Mode::Closed);
        assert!(s.waypoints.is_empty());
        assert!(!dir.join("world_test.txt").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn duplicate_names_are_rejected() {
        let dir = temp_dir("duplicate");
        let mut s = state(&dir);
        s.waypoints = vec![
            wp("Base", 1.0, 2.0, 3.0, OVERWORLD),
            wp("Other", 4.0, 5.0, 6.0, OVERWORLD),
        ];
        s.open_edit(1);
        if let Mode::Edit(e) = &mut s.mode {
            e.name = "base".to_owned();
        }
        s.act(Action::Save, None);
        assert_eq!(s.waypoints[1].name, "Other");
        assert!(matches!(s.mode, Mode::Edit(_)));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn delete_needs_a_second_click() {
        let dir = temp_dir("delete");
        let mut s = state(&dir);
        s.waypoints = vec![
            wp("One", 1.0, 2.0, 3.0, OVERWORLD),
            wp("Two", 4.0, 5.0, 6.0, OVERWORLD),
        ];
        s.persist().unwrap();

        s.act(Action::Delete(0), None);
        assert_eq!(s.waypoints.len(), 2);
        assert_eq!(s.confirm_delete, Some(0));

        s.act(Action::Delete(0), None);
        assert_eq!(s.waypoints.len(), 1);
        assert_eq!(reload(&dir).len(), 1);
        assert_eq!(reload(&dir)[0].name, "Two");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_non_delete_key_drops_the_delete_confirmation() {
        let dir = temp_dir("delete-reset");
        let mut s = state(&dir);
        s.waypoints = vec![
            wp("One", 1.0, 2.0, 3.0, OVERWORLD),
            wp("Two", 4.0, 5.0, 6.0, OVERWORLD),
        ];
        s.persist().unwrap();
        s.open_list(None);

        s.key("key.keyboard.delete", false, None, DEFAULT_LIST, DEFAULT_CREATE);
        assert_eq!(s.confirm_delete, Some(0));
        s.key("key.keyboard.down", false, None, DEFAULT_LIST, DEFAULT_CREATE);
        assert_eq!(s.confirm_delete, None);

        // Delete now needs two presses again, and acts on the new selection.
        s.key("key.keyboard.delete", false, None, DEFAULT_LIST, DEFAULT_CREATE);
        assert_eq!(s.confirm_delete, Some(1));
        s.key("key.keyboard.delete", false, None, DEFAULT_LIST, DEFAULT_CREATE);
        assert_eq!(s.waypoints.len(), 1);
        assert_eq!(reload(&dir)[0].name, "One");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn filter_limits_the_rows_and_selection() {
        let mut s = State::default();
        s.waypoints = vec![
            wp("Over", 1.0, 2.0, 3.0, OVERWORLD),
            wp("Nether", 1.0, 2.0, 3.0, NETHER),
            wp("End", 1.0, 2.0, 3.0, END),
        ];
        s.open_list(None);
        assert_eq!(s.filtered(), vec![0, 1, 2]);
        assert_eq!(s.selection, Some(0));

        s.act(Action::Filter(Filter::Nether), None);
        assert_eq!(s.filtered(), vec![1]);
        assert_eq!(s.selection, Some(1));
        assert_eq!(s.scroll, 0);
    }

    #[test]
    fn selection_moves_and_delete_targets_it() {
        let dir = temp_dir("selection");
        let mut s = state(&dir);
        s.waypoints = vec![
            wp("A", 1.0, 2.0, 3.0, OVERWORLD),
            wp("B", 1.0, 2.0, 3.0, OVERWORLD),
            wp("C", 1.0, 2.0, 3.0, OVERWORLD),
        ];
        s.persist().unwrap();
        s.open_list(None);
        assert_eq!(s.selection, Some(0));

        s.key("key.keyboard.down", false, None, DEFAULT_LIST, DEFAULT_CREATE);
        assert_eq!(s.selection, Some(1));
        s.key("key.keyboard.down", false, None, DEFAULT_LIST, DEFAULT_CREATE);
        assert_eq!(s.selection, Some(2));

        // Delete on the third entry asks once, then removes that entry.
        s.key("key.keyboard.delete", false, None, DEFAULT_LIST, DEFAULT_CREATE);
        assert_eq!(s.confirm_delete, Some(2));
        s.key("key.keyboard.delete", false, None, DEFAULT_LIST, DEFAULT_CREATE);
        assert_eq!(s.waypoints.len(), 2);
        assert_eq!(reload(&dir)[0].name, "A");
        assert_eq!(reload(&dir)[1].name, "B");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn enter_opens_the_selected_entry() {
        let mut s = State::default();
        s.waypoints = vec![
            wp("A", 1.0, 2.0, 3.0, OVERWORLD),
            wp("B", 1.0, 2.0, 3.0, OVERWORLD),
        ];
        s.open_list(None);
        s.key("key.keyboard.down", false, None, DEFAULT_LIST, DEFAULT_CREATE);
        s.key("key.keyboard.enter", false, None, DEFAULT_LIST, DEFAULT_CREATE);
        match s.mode {
            Mode::Edit(e) => assert_eq!(e.index, Some(1)),
            other => panic!("expected editor, got {other:?}"),
        }
    }

    #[test]
    fn hidden_leaves_the_world_and_minimap_but_stays_listed() {
        let mut s = State::default();
        let mut w = wp("Hidden", 1.0, 2.0, 3.0, OVERWORLD);
        w.hidden = true;
        s.waypoints = vec![w];

        assert!(s.visible_in(OVERWORLD).is_empty());
        assert!(s.minimap_dots(OVERWORLD).is_empty());
        assert_eq!(s.waypoints.len(), 1);
        assert_eq!(s.filtered(), vec![0]);
    }

    #[test]
    fn toggle_persists_hidden_state() {
        let dir = temp_dir("hidden-persist");
        let mut s = state(&dir);
        s.waypoints = vec![wp("Base", 1.0, 2.0, 3.0, OVERWORLD)];
        s.persist().unwrap();
        s.act(Action::Toggle(0), None);
        assert!(reload(&dir)[0].hidden);

        let mut again = State::default();
        again.switch_to(Some(ContextKey::Singleplayer("test".to_owned())), dir.clone());
        assert!(again.waypoints[0].hidden);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn hit_testing_finds_buttons_and_rows() {
        let mut s = State::default();
        s.waypoints = vec![wp("Base", 1.0, 2.0, 3.0, OVERWORLD)];
        s.open_list(None);
        let (draws, widgets) = s.screen([400.0, 300.0], [0.0, 0.0], Some(&here()), &measure);
        assert!(!draws.is_empty());

        let close = widgets
            .iter()
            .find(|w| w.action == Action::Close)
            .expect("close button");
        let at = [close.rect[0] + close.rect[2] / 2.0, close.rect[1] + close.rect[3] / 2.0];
        assert_eq!(hit(&widgets, at), Some(Action::Close));

        let row = widgets
            .iter()
            .find(|w| w.action == Action::Select(0))
            .expect("row");
        let at = [row.rect[0] + row.rect[2] / 2.0, row.rect[1] + row.rect[3] / 2.0];
        assert_eq!(hit(&widgets, at), Some(Action::Select(0)));

        // Nothing under a point outside every widget.
        assert_eq!(hit(&widgets, [-10.0, -10.0]), None);
    }

    #[test]
    fn editor_widgets_are_clickable() {
        let mut s = State::default();
        s.open_create(&here(), false, None);
        let (_, widgets) = s.screen([400.0, 300.0], [0.0, 0.0], Some(&here()), &measure);
        for action in [
            Action::Focus(Field::Y),
            Action::Here,
            Action::CycleDimension,
            Action::ToggleHidden,
            Action::Save,
            Action::Cancel,
            Action::Color(PALETTE[3]),
        ] {
            assert!(
                widgets.iter().any(|w| w.action == action),
                "missing widget {action:?}"
            );
        }
    }

    #[test]
    fn scroll_stays_within_bounds() {
        let mut s = State::default();
        s.waypoints = (0..30)
            .map(|i| wp(&format!("WP{i}"), 1.0, 2.0, 3.0, OVERWORLD))
            .collect();
        s.open_list(None);
        s.visible_rows = 5;

        // Positive wheel values scroll up (toward the first row).
        for _ in 0..10 {
            s.scroll_by(1.0);
        }
        assert_eq!(s.scroll, 0);

        for _ in 0..100 {
            s.scroll_by(-1.0);
        }
        assert_eq!(s.scroll, 25);
    }

    #[test]
    fn here_action_uses_the_feet_position() {
        let mut s = State::default();
        s.open_create(&Here {
            pos: [0.0, 100.0, 0.0],
            dimension: NETHER.to_owned(),
        }, false, None);
        s.act(
            Action::Here,
            Some(&Here {
                pos: [12.5, 64.0, -3.25],
                dimension: OVERWORLD.to_owned(),
            }),
        );
        match s.mode {
            Mode::Edit(e) => {
                assert_eq!(e.coords, ["13".to_owned(), "64".to_owned(), "-3".to_owned()]);
                assert_eq!(e.dimension, OVERWORLD);
            }
            other => panic!("expected editor, got {other:?}"),
        }
    }

    #[test]
    fn close_is_requested_once() {
        let mut s = State::default();
        s.open_list(None);
        s.key("key.keyboard.escape", false, None, DEFAULT_LIST, DEFAULT_CREATE);
        assert_eq!(s.mode, Mode::Closed);
        assert!(s.take_close());
        assert!(!s.take_close());
    }

    #[test]
    fn screen_closed_by_the_game_resets_the_mode() {
        let mut s = State::default();
        s.open_list(None);
        s.sync_open(false);
        assert_eq!(s.mode, Mode::Closed);
    }

    #[test]
    fn char_input_is_limited_per_field() {
        let mut s = State::default();
        s.open_create(&here(), false, None);
        for c in "Name".chars() {
            s.char_typed(c);
        }
        match &s.mode {
            Mode::Edit(e) => assert_eq!(e.name, "Name"),
            _ => panic!("editor closed"),
        }
        if let Mode::Edit(e) = &mut s.mode {
            e.focus = Field::X;
            e.coords[0].clear();
        }
        for c in "ab-1.2.3".chars() {
            s.char_typed(c);
        }
        match &s.mode {
            Mode::Edit(e) => assert_eq!(e.coords[0], "-1.23"),
            _ => panic!("editor closed"),
        }
    }

    #[test]
    fn field_navigation_wraps() {
        assert_eq!(Field::Name.step(false), Field::X);
        assert_eq!(Field::X.step(false), Field::Y);
        assert_eq!(Field::Y.step(false), Field::Z);
        assert_eq!(Field::Z.step(false), Field::Name);
        assert_eq!(Field::Name.step(true), Field::Z);
    }

    fn cam(yaw: f32, pitch: f32, fov: f32, width: f32, height: f32) -> Cam {
        let (sy, cy) = (f64::from(yaw).to_radians()).sin_cos();
        let (sp, cp) = (f64::from(pitch).to_radians()).sin_cos();
        Cam {
            pos: [0.0; 3],
            forward: [-sy * cp, -sp, cy * cp],
            right: [-cy, 0.0, -sy],
            up: [-sy * sp, cp, cy * sp],
            focal: 1.0 / (f64::from(fov) / 2.0).to_radians().tan(),
            half: [f64::from(width) / 2.0, f64::from(height) / 2.0],
        }
    }

    #[test]
    fn projection_puts_a_point_ahead_in_the_middle() {
        let c = cam(0.0, 0.0, 70.0, 800.0, 400.0);
        assert_eq!(c.project([0.0, 0.0, 10.0]), Some([400.0, 200.0]));
        assert_eq!(c.project([0.0, 0.0, -10.0]), None);
        assert_eq!(c.project([0.0, 0.0, 0.01]), None);
    }

    #[test]
    fn projection_scales_horizontally_by_the_aspect() {
        // Facing south (+Z); west (-X) is on the player's right.
        let c = cam(0.0, 0.0, 70.0, 800.0, 400.0);
        let at = c.project([-10.0, 0.0, 10.0]).unwrap();
        let focal = 1.0 / (35.0_f64).to_radians().tan();
        let nx = focal * 10.0 / (10.0 * 2.0);
        let expected = ((nx + 1.0) * 400.0) as f32;
        assert!((at[0] - expected).abs() < 0.01);
        assert!((at[1] - 200.0).abs() < 0.01);

        // A square window must not use the wide window's scale.
        let square = cam(0.0, 0.0, 70.0, 400.0, 400.0);
        let at = square.project([-10.0, 0.0, 10.0]).unwrap();
        let square_nx = focal * 10.0 / 10.0;
        let expected = ((square_nx + 1.0) * 200.0) as f32;
        assert!((at[0] - expected).abs() < 0.01);
        assert!(at[0] > 200.0);
    }

    #[test]
    fn projection_follows_yaw_and_pitch() {
        // Facing west (-X): south (+Z) is on the left.
        let west = cam(90.0, 0.0, 70.0, 800.0, 400.0);
        let at = west.project([-10.0, 0.0, 10.0]).unwrap();
        assert!(at[0] < 400.0);
        assert!((at[1] - 200.0).abs() < 0.01);

        // Looking down: a point at eye level is above the centre.
        let down = cam(0.0, 30.0, 70.0, 800.0, 400.0);
        let at = down.project([0.0, 0.0, 10.0]).unwrap();
        assert!(at[1] < 200.0);
    }

    #[test]
    fn only_waypoints_inside_the_view_are_drawn() {
        let c = cam(0.0, 0.0, 70.0, 800.0, 400.0);
        // Ahead and inside the view.
        assert!(marker_at(&c, [0.0, 0.0, 10.0]).is_some());
        // In front, but far to the side: outside the view, so hidden.
        assert_eq!(marker_at(&c, [-100.0, 0.0, 10.0]), None);
        // Behind: hidden.
        assert_eq!(marker_at(&c, [0.0, 0.0, -10.0]), None);
        // A point at the very edge of a wide view is still drawn.
        let at = marker_at(&c, [-10.0, 0.0, 10.0]).unwrap();
        assert!(at[0] > 0.0 && at[0] < 800.0);
    }

    #[test]
    fn waypoints_behind_the_camera_are_hidden() {
        let c = cam(0.0, 0.0, 70.0, 800.0, 400.0);
        assert_eq!(marker_at(&c, [0.0, 0.0, -10.0]), None);
        assert_eq!(marker_at(&c, [5.0, 0.0, -10.0]), None);
        assert!(marker_at(&c, [0.0, 0.0, 10.0]).is_some());

        // Turning around puts the point behind: no marker at all.
        let turned = cam(180.0, 0.0, 70.0, 800.0, 400.0);
        assert_eq!(marker_at(&turned, [0.0, 0.0, 10.0]), None);
    }

    #[test]
    fn deaths_tab_lists_only_death_waypoints() {
        let mut s = State::default();
        let mut death = wp("Death 1", 1.0, 2.0, 3.0, OVERWORLD);
        death.death = true;
        s.waypoints = vec![
            wp("Base", 1.0, 2.0, 3.0, OVERWORLD),
            death,
            wp("Fortress", 1.0, 2.0, 3.0, NETHER),
        ];
        s.open_list(None);
        s.act(Action::Filter(Filter::Deaths), None);
        assert_eq!(s.filtered(), vec![1]);
        assert_eq!(s.selection, Some(1));

        s.act(Action::Filter(Filter::All), None);
        assert_eq!(s.filtered(), vec![0, 1, 2]);
    }

    #[test]
    fn death_screen_creates_one_waypoint_per_death() {
        let dir = temp_dir("death");
        let mut s = state(&dir);

        let message = s.observe_death(true, Some(&here())).unwrap();
        assert!(message.contains("Death 1"), "message: {message}");
        assert_eq!(s.waypoints.len(), 1);
        let death = &s.waypoints[0];
        assert!(death.death);
        assert!(!death.hidden);
        assert_eq!(death.color, DEATH_COLOR);
        assert_eq!((death.x, death.y, death.z), (13.0, 64.0, -3.0));
        assert_eq!(death.dim, OVERWORLD);
        assert!(reload(&dir)[0].death);
        // The minimap is told to draw it as an X, not a coloured dot.
        let dots = s.minimap_dots(OVERWORLD);
        assert_eq!(dots.len(), 1);
        assert!(dots[0].2);

        // Still on the death screen: not a second waypoint.
        assert_eq!(s.observe_death(true, Some(&here())), None);
        assert_eq!(s.waypoints.len(), 1);

        // Respawn and die again: a new name and a new entry.
        assert_eq!(s.observe_death(false, Some(&here())), None);
        let message = s.observe_death(true, Some(&here())).unwrap();
        assert!(message.contains("Death 2"), "message: {message}");
        assert_eq!(s.waypoints.len(), 2);
        assert_eq!(reload(&dir).len(), 2);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn death_flag_survives_a_roundtrip_and_edits() {
        let dir = temp_dir("death-edit");
        let mut s = state(&dir);
        s.observe_death(true, Some(&here()));
        s.observe_death(false, None);
        assert!(reload(&dir)[0].death);

        // Renaming a death waypoint keeps it in the Deaths tab.
        s.open_edit(0);
        if let Mode::Edit(e) = &mut s.mode {
            e.name = "Crash site".to_owned();
        }
        s.act(Action::Save, None);
        assert_eq!(s.waypoints[0].name, "Crash site");
        assert!(s.waypoints[0].death);
        assert!(reload(&dir)[0].death);

        // A file from before the column has normal waypoints.
        let old = format!("{HEADER}\nminecraft:overworld\t1\t64\t2\tff5555\t0\tOld\n");
        assert!(!WaypointCodec::decode(&old).unwrap()[0].death);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn filter_parsing() {
        assert_eq!(Filter::parse("all"), Some(Filter::All));
        assert_eq!(Filter::parse("minecraft:overworld"), Some(Filter::Overworld));
        assert_eq!(Filter::parse("nether"), Some(Filter::Nether));
        assert_eq!(Filter::parse("the_end"), Some(Filter::End));
        assert_eq!(Filter::parse("deaths"), Some(Filter::Deaths));
        assert_eq!(Filter::parse("death"), Some(Filter::Deaths));
        assert_eq!(Filter::parse("invalid"), None);
    }

    #[test]
    fn failed_save_keeps_the_form_and_the_file() {
        let dir = temp_dir("failed-save");
        let path = dir.join("world_test.txt");
        let bad = format!("{HEADER}\ninvalid\tdata\n");
        fs::write(&path, &bad).unwrap();

        let mut s = State::default();
        s.switch_to(Some(ContextKey::Singleplayer("test".to_owned())), dir.clone());
        assert!(s.load_failed);
        s.open_create(&here(), false, None);
        for c in "Base".chars() {
            s.char_typed(c);
        }
        s.act(Action::Save, None);

        match &s.mode {
            Mode::Edit(e) => {
                assert!(e.error.as_deref().is_some_and(|m| m.contains("Could not save")));
            }
            other => panic!("expected editor, got {other:?}"),
        }
        assert!(s.waypoints.is_empty());
        assert_eq!(fs::read_to_string(&path).unwrap(), bad);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rename_split_handles_quotes() {
        assert_eq!(
            split_pair("\"WP 1\" test"),
            Some(("WP 1".to_owned(), "test".to_owned()))
        );
        assert_eq!(
            split_pair("old new name"),
            Some(("old".to_owned(), "new name".to_owned()))
        );
        assert_eq!(split_pair("onlyone"), None);
        assert_eq!(unquote("\"WP 1\""), "WP 1");
    }
}
