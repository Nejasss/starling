# Bridging Assist for Starling — 0.4.1

A native `.hnmod` reacharound-placement implementation inspired by Bridging Mod 2.7.0 for Minecraft 26.3.

## Changes in 0.4.1

- Removes the improvised crosshair marker and custom placement-outline renderer. Starling's ordinary crosshair and block highlight remain untouched.
- Removes candidate-path and block-support scanning from the per-frame visual hook; the mod no longer performs its own world traversal every rendered frame.
- Adds a one-shot native-tick cooldown adjustment for successful assisted placements. It works with Starling's actual tick order: `use_delay` is decremented before `native_tick`, then the held-use loop runs. The default interval is one tick; higher values trade speed for a slower placement cadence.
- Keeps normal, non-assisted item use on Starling's existing cooldown path.
- Keeps the 0.4.0 reacharound candidate selection, placement hit, slab/trapdoor assist, axis filters, adjacency settings and toggle key.

## Known limitations

This remains a best-effort native port, not feature-identical parity. Still not ported: polished sprite crosshair, action-bar-style toggle notification, non-bridging outline and F3 debug trace modes, extension API and Sable/Dank Storage integrations, and automatic FreeLook detection (Auto currently uses the active camera). Numeric sliders remain coarse menu presets. Real in-game regression testing is still required for sprint-forward/reverse/sideways bridging, first/third-person camera, touch/controller, moving platforms, server-authority, slabs and trapdoors. The one-tick interval is a client request cadence; servers may reject or throttle overly rapid placements.

## Build

The folder is a source bundle for Starling's `packaging/android/native_build.sh`; it is not a standalone APK or an ordinary `.hmod`. Include `bridging_assist` in the workflow's `native_mods` input to compile it alongside the other native mods.

## Attribution and license

Bridging Mod is MIT-licensed. This implementation is a Rust reimplementation of the relevant behavior, not a repackaging of its Java classes. The upstream license and copyright notice are retained here:

Copyright (c) 2026 Will Scully

See `LICENSE`.
