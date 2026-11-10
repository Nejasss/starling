// Bodyguard 0.1, injected as `crate::game::bodyguard`.
//
// A carved pumpkin placed on a chest on an iron block builds a bodyguard: an iron
// golem (its own AI finds and fights hostile mobs) that follows its builder. It
// takes an axe by right click, and nothing else. The chest and the iron block are
// used up: EMPTY THE CHEST FIRST, what it holds is lost with it.
//
// 0.1 keeps its bodyguards in memory: after the world is closed and opened again a
// bodyguard is an ordinary iron golem. Persistence, the vest and the 25 slot bag
// come in later versions.
use super::Game;
use horizon_client::mod_api::{Host, ItemStack, Tag, Text};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

pub const ID: &str = "bodyguard";
pub const VERSION: &str = "0.1.2";
pub const DESCRIPTION: &str = "Rakit bodyguard: iron block, chest kosong, carved pumpkin. Klik kanan pakai kapak. Selalu aktif.";
const BASE_KIND: &str = "minecraft:iron_golem";
/// Health points of a bodyguard.
const MAX_HEALTH: f32 = 20.0;
/// The blow of a bodyguard with no axe.
const FIST_DAMAGE: f32 = 3.0;
/// Farther than this (blocks) from its owner, a bodyguard walks over.
const FOLLOW_FROM: f64 = 5.0;
/// Farther than this, it is teleported next to its owner.
const TELEPORT_FROM: f64 = 20.0;
/// Its follow step is taken every this many player ticks.
const FOLLOW_EVERY: u64 = 4;

/// The axes a bodyguard takes, and the blow of each.
const AXES: [(&str, f32); 7] = [
    ("minecraft:wooden_axe", 4.0),
    ("minecraft:golden_axe", 4.0),
    ("minecraft:copper_axe", 5.0),
    ("minecraft:stone_axe", 5.0),
    ("minecraft:iron_axe", 6.0),
    ("minecraft:diamond_axe", 7.0),
    ("minecraft:netherite_axe", 8.0),
];

/// The entity data of a new bodyguard: kept in the world, a player made it (it
/// doesn't fight players), 20 health points and a weak fist.
const SNBT: &str = "{PersistenceRequired:1b,PlayerCreated:1b,Health:20.0f,attributes:[{id:\"minecraft:max_health\",base:20.0d},{id:\"minecraft:attack_damage\",base:3.0d}]}";

struct Guard {
    id: i32,
    owner: u128,
    /// Index into `AXES`.
    axe: Option<usize>,
}

static GUARDS: Mutex<Vec<Guard>> = Mutex::new(Vec::new());
static TICKS: AtomicU64 = AtomicU64::new(0);
static LOADED: AtomicBool = AtomicBool::new(false);

fn guards() -> std::sync::MutexGuard<'static, Vec<Guard>> {
    GUARDS.lock().unwrap_or_else(|e| e.into_inner())
}

fn axe_index(item: &str) -> Option<usize> {
    AXES.iter().position(|(name, _)| *name == item)
}

fn say(host: &mut dyn Host, uuid: u128, line: &str) {
    let _ = host.action_bar(uuid, Text::literal(line.to_string()));
}

/// `native_place_block`'s HEAD: the pumpkin was just placed at `pos`.
pub fn on_place(game: &mut Game<'_>, stack: &ItemStack, pos: [i32; 3]) {
    if &*stack.item != "minecraft:carved_pumpkin" {
        return;
    }
    let air = game.world.air;
    let chest = [pos[0], pos[1] - 1, pos[2]];
    let base = [pos[0], pos[1] - 2, pos[2]];
    // DEBUG 0.1.2: say in chat what the hook sees.
    let (b1, b2, who) = {
        let h: &mut dyn Host = &mut *game;
        (h.block(chest), h.block(base), h.acting_player().is_some())
    };
    game.native_chat(&format!(
        "[bodyguard] labu dipasang: bawah1={b1:?} bawah2={b2:?} pemain={who}"
    ));
    let host: &mut dyn Host = game;
    if b1 != "minecraft:chest" || b2 != "minecraft:iron_block" {
        return;
    }
    let Some(builder) = host.acting_player() else {
        return;
    };
    // Read the entity data before anything is taken down.
    let nbt = match horizon_world::nbt::parse_snbt(SNBT) {
        Ok((Tag::Compound(c), _)) => c,
        _ => return,
    };
    let dim = builder.dimension.clone();
    for p in [pos, chest, base] {
        let _ = host.set_block(&dim, p, air, 3);
    }
    let at = [
        f64::from(base[0]) + 0.5,
        f64::from(base[1]) + 0.05,
        f64::from(base[2]) + 0.5,
    ];
    let Some(id) = host.summon(&dim, BASE_KIND, at, nbt) else {
        // Nothing was built: the builder gets what went in back.
        let _ = host.give_items("minecraft:carved_pumpkin", 1);
        let _ = host.give_items("minecraft:chest", 1);
        let _ = host.give_items("minecraft:iron_block", 1);
        say(host, builder.uuid, "Bodyguard gagal dirakit, bahannya dikembalikan.");
        return;
    };
    let _ = host.set_mob_health(&dim, id, MAX_HEALTH);
    guards().push(Guard {
        id: id as i32,
        owner: builder.uuid,
        axe: None,
    });
    say(
        host,
        builder.uuid,
        "Bodyguard dirakit! Klik kanan pakai kapak buat ngasih senjata.",
    );
}

/// `native_interact_entity`'s HEAD: a right click on entity `entity`.
pub fn on_interact(
    game: &mut Game<'_>,
    entity: i32,
    stack: Option<&ItemStack>,
    claimed: &mut bool,
) {
    if *claimed {
        return;
    }
    let found = guards()
        .iter()
        .find(|g| g.id == entity)
        .map(|g| (g.owner, g.axe));
    let Some((owner, old_axe)) = found else {
        return;
    };
    let host: &mut dyn Host = game;
    let Some(me) = host.acting_player() else {
        return;
    };
    if me.uuid != owner {
        return;
    }
    // An empty hand does nothing, as on any golem.
    let Some(stack) = stack else {
        return;
    };
    let Some(index) = axe_index(&stack.item) else {
        *claimed = true;
        say(host, me.uuid, "Bodyguard cuma mau kapak.");
        return;
    };
    let name = AXES[index].0;
    if host.take_items(name, 1) < 1 {
        *claimed = true;
        say(host, me.uuid, "Kapaknya harus utuh (belum rusak atau di-enchant).");
        return;
    }
    if let Some(old) = old_axe {
        // The axe it held goes back to the player.
        if host.give_items(AXES[old].0, 1) < 1 {
            let _ = host.give_items(name, 1);
            *claimed = true;
            say(host, me.uuid, "Inventory penuh, kapak lama nggak bisa dikembalikan.");
            return;
        }
    }
    if let Some(g) = guards().iter_mut().find(|g| g.id == entity) {
        g.axe = Some(index);
    }
    *claimed = true;
    say(
        host,
        me.uuid,
        &format!("Bodyguard sekarang pegang kapak (damage {}).", AXES[index].1),
    );
}

/// `native_living_incoming_damage`'s HEAD: whoever `causing` is, a bodyguard's
/// blow is its axe's.
pub fn on_damage(causing: i32, amount: &mut f32) {
    let list = guards();
    if let Some(g) = list.iter().find(|g| g.id == causing) {
        *amount = match g.axe {
            Some(i) => AXES[i].1,
            None => FIST_DAMAGE,
        };
    }
}

/// `native_player_tick`'s HEAD: the acting player's bodyguards come closer.
pub fn on_player_tick(game: &mut Game<'_>) {
    // DEBUG 0.1.2: proves the tick hook runs.
    if !LOADED.swap(true, Ordering::Relaxed) {
        game.native_chat("[bodyguard] 0.1.2 aktif (hook tick jalan)");
    }
    let host: &mut dyn Host = game;
    let Some(me) = host.acting_player() else {
        return;
    };
    if TICKS.fetch_add(1, Ordering::Relaxed) % FOLLOW_EVERY != 0 {
        return;
    }
    let mine: Vec<i32> = guards()
        .iter()
        .filter(|g| g.owner == me.uuid)
        .map(|g| g.id)
        .collect();
    let dim = me.dimension.as_str();
    for id in mine {
        // Only the entities of the player's own level are reached: a bodyguard
        // left in another dimension stays there.
        let Some(entity) = host.entity(id as u32) else {
            continue;
        };
        let Some(at) = host.entity_position(entity) else {
            continue;
        };
        let dx = me.position[0] - at[0];
        let dy = me.position[1] - at[1];
        let dz = me.position[2] - at[2];
        let far = (dx * dx + dy * dy + dz * dz).sqrt();
        if far > TELEPORT_FROM {
            let to = [me.position[0] + 1.5, me.position[1], me.position[2]];
            let _ = host.teleport_mob(dim, id as u32, to);
        } else if far > FOLLOW_FROM {
            let flat = (dx * dx + dz * dz).sqrt().max(0.001);
            let _ = host.push_mob(dim, id as u32, [dx / flat * 0.25, 0.0, dz / flat * 0.25]);
        }
    }
}
