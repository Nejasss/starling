# Bridging Assist for Starling — 0.1.0

A first native `.hnmod` implementation of the core reacharound-placement behavior inspired by Bridging Mod 2.7.0 for Minecraft 26.3.

## Included in 0.1.0

- Runs only when the ordinary block raycast misses and an entity is not the target.
- Checks for a block item in the hand currently being used.
- Traverses candidate cells from the camera toward the player's reach ray using a 3-D voxel line.
- Rejects cells inside the player's own bounding box.
- Checks for a non-air, non-replaceable support block behind the candidate.
- Reuses Starling's ordinary `remote_use_item_on` path for packet creation, prediction, hand animation and server validation.
- Exposes settings for enablement, crouch-only, replacing non-solid targets, torch skipping and minimum bridge distance.
- Uses a RETURN hook to restore the temporary hit after the original interaction path returns.

## Not yet included

This is intentionally a first functional slice, not a complete reproduction of the Fabric mod. It does not yet include the crosshair direction indicator, placement outline, slab assist, special item handlers, perspective modes, snap strength, adjacency modes, or the full set of distance/axis tuning options. The target search is aimed at the first-person/touch-aim path and needs in-game testing for camera modes and block-face edge cases.

## Build

The folder is a source bundle for Starling's `packaging/android/native_build.sh`; it is not a standalone APK or an ordinary `.hmod`. Include `bridging_assist` in the workflow's `native_mods` input to compile it alongside the other trusted native mods.

The source targets Starling 26.3 and the Horizon Mixins API used by this fork. Build-time Rust checking and interactive testing are required before relying on it in a world/server.

## Attribution and license

Bridging Mod is MIT-licensed. This implementation is a Rust reimplementation of the relevant behavior, not a repackaging of its Java classes. The upstream license and copyright notice are retained here:

Copyright (c) 2026 Will Scully

See `LICENSE`.
