# Six native mods added to this Starling fork

This folder contains these `.hnmod` bundles unpacked without changing their upstream source,
except for Waypoints default controls to avoid colliding with Waymap:

| Mod | Version | Source |
|---|---:|---|
| Zoom | 2.0.0 | https://github.com/horizon-rust/horizon-mods/tree/main/native/zoom |
| VeinMiner | 0.3.2 | https://github.com/levzzz5154/horizon-veinminer |
| Waypoints | 1.1.0 | https://github.com/elyesghazel/horizon-waypoints |
| Accurate Block Placement | 1.0.0 | https://github.com/Lutziano888/horizon-accurate-block-placement |
| Waymap | 0.2.1 | https://github.com/Lauch2/horizon-waymap-mod |
| Coordinates HUD | 2.0.0 | https://github.com/horizon-rust/horizon-mods/tree/main/native/coords-hud |

## Combined controls

Waymap keeps its defaults: `N` creates a waypoint; `B` or `M` opens its list.
Waypoints is assigned `K` to create and `L` to open its list. This avoids both mods handling the same key.
Waypoints' settings page can still be used to rebind these keys.

The two waypoint mods keep separate storage formats and features. Waymap also has a `/wp` command suite;
the Waypoints bundle in this package has no chat-command hook.

`prebuilt.txt` keeps the existing `wrench` build and adds one combined set containing all six mods.
The APK workflow can also be dispatched directly with `native_mods` set to the six IDs.

All six are native mods: their code is compiled into the game's native library and executes with the game's
full access. Install only if you trust the source. Each bundle keeps its original `horizon-native.json`,
`horizon-mixin.toml`, and source files; per-mod MIT license notices are included beside each bundle.
