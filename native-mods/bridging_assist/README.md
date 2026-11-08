# Bridging Assist for Starling — 0.2.2

A first native `.hnmod` implementation of the core reacharound-placement behavior inspired by Bridging Mod 2.7.0 for Minecraft 26.3.

## Included in 0.2.2

- Runs only when the ordinary block raycast misses and an entity is not the target.
- Checks for a block item in the hand currently being used.
- Reconstructs the two-perspective path: the player's look ray determines the reach endpoint, while the camera look ray determines candidate traversal.
- Uses finite-checked geometry and tests path construction and placement-face orientation.
- Rejects cells inside the player's own bounding box.
- Checks for a non-air, non-replaceable support block behind the candidate.
- Reuses Starling's ordinary `remote_use_item_on` path for packet creation, prediction, hand animation and server validation.
- Converts the support-side direction to the opposite hit face used by Bridging Mod's default placement target.
- Converts the support-side direction to the opposite hit face used by Bridging Mod's default placement target.
- Exposes settings for enablement, crouch-only, replacing non-solid targets, torch skipping and minimum bridge distance.
- Uses a RETURN hook to restore the temporary hit after the original interaction path returns.

## Not yet included

This is still a staged native port, not a complete reproduction of the Fabric mod. The reacharound candidate path and hit orientation are closer to the upstream algorithm, but the crosshair direction indicator, placement outline, slab assist, special item/environment handlers, perspective modes, snap strength, adjacency modes, and full distance/axis tuning are not yet implemented. First-person, touch, detached-camera, replaceable-block, and slab edge cases still require in-game testing.

## Build

The folder is a source bundle for Starling's `packaging/android/native_build.sh`; it is not a standalone APK or an ordinary `.hmod`. Include `bridging_assist` in the workflow's `native_mods` input to compile it alongside the other trusted native mods.

The source targets Starling 26.3 and the Horizon Mixins API used by this fork. Build-time Rust checking and interactive testing are required before relying on it in a world/server.

## Attribution and license

Bridging Mod is MIT-licensed. This implementation is a Rust reimplementation of the relevant behavior, not a repackaging of its Java classes. The upstream license and copyright notice are retained here:

Copyright (c) 2026 Will Scully

See `LICENSE`.
