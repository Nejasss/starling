# Bridging Assist for Starling — 0.4.0

A first native `.hnmod` implementation of the core reacharound-placement behavior inspired by Bridging Mod 2.7.0 for Minecraft 26.3.

## Included in 0.4.0

- Runs only when the ordinary block raycast misses and an entity is not the target.
- Checks for a block item in the hand currently being used.
- Reconstructs the two-perspective path: the player's look ray determines the reach endpoint, while the camera look ray determines candidate traversal.
- Uses finite-checked geometry and tests path construction and placement-face orientation.
- Rejects cells inside the player's own bounding box.
- Checks for a non-air, non-replaceable support block behind the candidate.
- Reuses Starling's ordinary `remote_use_item_on` path for packet creation, prediction, hand animation and server validation.
- Converts the support-side direction to the opposite hit face used by Bridging Mod's default placement target.
- Expands the Bresenham path with edge/corner candidates using a segment-versus-voxel snap test.
- Adds configurable adjacency, snap strength, horizontal/vertical axis filters and crouch axis override.
- Adds Camera / Eyeline perspective selection; Auto currently falls back to Camera because Starling has no FreeLook compatibility registry for this mod.
- Adds vanilla slab and trapdoor hit-point assist, plus compatible vertical-slab merging.
- Applies the configurable post-placement delay (default: 4 ticks).
- Adds a placement outline, a simple directional marker below the crosshair, and a configurable comma-default toggle key with chat feedback.
- Keeps Starling's ordinary use-item placement path for prediction, packets, animation and server validation.
- Exposes settings for enablement, crouch-only, replacing non-solid targets, torch skipping and minimum bridge distance.
- Uses a RETURN hook to restore the temporary hit after the original interaction path returns.

## Not yet included

This is a best-effort native port, not byte-for-byte or feature-identical parity. Still not ported: polished sprite crosshair and action-bar-style toggle notification, non-bridging outline and F3 debug trace modes, the extension API and its Sable/Dank Storage integrations, and automatic FreeLook detection (Auto currently uses the active camera). Numeric sliders are exposed as coarse menu presets rather than the upstream continuous controls. First-person, touch/controller, detached-camera, moving-platform, server-authority, slab/trapdoor and sprint-forward behavior still require real in-game regression testing before calling the port complete.

## Build

The folder is a source bundle for Starling's `packaging/android/native_build.sh`; it is not a standalone APK or an ordinary `.hmod`. Include `bridging_assist` in the workflow's `native_mods` input to compile it alongside the other trusted native mods.

The source targets Starling 26.3 and the Horizon Mixins API used by this fork. Build-time Rust checking and interactive testing are required before relying on it in a world/server.

## Attribution and license

Bridging Mod is MIT-licensed. This implementation is a Rust reimplementation of the relevant behavior, not a repackaging of its Java classes. The upstream license and copyright notice are retained here:

Copyright (c) 2026 Will Scully

See `LICENSE`.
