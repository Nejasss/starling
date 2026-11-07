// Waymap's minimap overlay: Horizon's minimap image, plus a dot per waypoint.
//
// `extras_hud::draw_minimap` and this module are both in the client library,
// while the waypoints live in the game module (the binary). The game pushes
// this frame's dots and the minimap's view here each frame; the cancellable
// HEAD over `draw_minimap` draws them. The view is what the minimap was
// composed from, so a dot sits on the column it belongs to.

use super::{Corners, HudAtlas, HudBatch, MinimapLook, rgba};
use std::cell::RefCell;

/// The minimap image is always this many pixels square (`minimap::SIZE`).
const SIZE: f32 = 128.0;

thread_local! {
    /// The view the minimap was composed from this frame.
    static VIEW: RefCell<Option<crate::minimap::View>> = const { RefCell::new(None) };
    /// This frame's waypoints in the shown dimension: `([x, z], 0xRRGGBB, death)`.
    static DOTS: RefCell<Vec<([f64; 2], u32, bool)>> = const { RefCell::new(Vec::new()) };
}

/// Keeps what `Game::minimap_shown` returned for the dots.
pub fn set_view(map: Option<(crate::extras::Corner, f32, crate::minimap::View)>) {
    VIEW.with_borrow_mut(|view| *view = map.map(|(_, _, view)| view));
}

/// Keeps this frame's waypoints, for the map the player sees.
pub fn set_dots(dots: Vec<([f64; 2], u32, bool)>) {
    DOTS.with_borrow_mut(|slot| *slot = dots);
}

/// `extras_hud::draw_minimap`, with the dots over the image.
pub fn draw(
    batch: &mut HudBatch,
    atlas: &HudAtlas,
    corners: &mut Corners,
    look: &MinimapLook,
) {
    let MinimapLook {
        corner,
        size,
        image,
        arrow,
        north,
    } = *look;
    let (x, y) = corners.place(corner, size, size);
    batch.rect_uv(x, y, size, size, image, u32::MAX);
    let (cx, cy) = (x + size / 2.0, y + size / 2.0);
    draw_dots(batch, atlas, [x, y, size]);
    if let Some(r) = atlas.sprites.get("map/decorations/player").copied() {
        // MapRenderer's decorations draw the sprite flipped: its arrow
        // points down the texture, so up on screen is half a turn.
        let (s, c) = (arrow + 180.0).to_radians().sin_cos();
        let p = |dx: f32, dy: f32| [cx + dx * c - dy * s, cy + dx * s + dy * c];
        let h = 4.0;
        batch.quad(
            [p(-h, h), p(h, h), p(h, -h), p(-h, -h)],
            [[r.u0, r.v0], [r.u1, r.v0], [r.u1, r.v1], [r.u0, r.v1]],
            u32::MAX,
        );
    }
    if let Some((nx, ny)) = north {
        let reach = size / 2.0 - 6.0;
        let tw = HudBatch::text_width(atlas, "N");
        let (tx, ty) = (cx + nx * reach - tw / 2.0, cy + ny * reach - 4.0);
        batch.text(
            atlas,
            "N",
            tx.round(),
            ty.round(),
            rgba(255, 255, 255, 255),
            true,
        );
    }
}

fn draw_dots(batch: &mut HudBatch, atlas: &HudAtlas, at: [f32; 3]) {
    let Some(view) = VIEW.with_borrow(|view| *view) else {
        return;
    };
    if view.blocks_per_pixel <= 0.0 {
        return;
    }
    let size = at[2];
    let scale = size / SIZE;
    let (cx, cy) = (at[0] + size / 2.0, at[1] + size / 2.0);
    let bpp = f64::from(view.blocks_per_pixel);
    // `View::axes`: the map's right and forward as the image's x and up.
    let (r, f) = if view.rotate {
        let (s, c) = (f64::from(view.yaw).to_radians()).sin_cos();
        ([-c, -s], [-s, c])
    } else {
        ([1.0, 0.0], [0.0, -1.0])
    };
    let det = -r[0] * f[1] + f[0] * r[1];
    if det.abs() < 1e-9 {
        return;
    }
    DOTS.with_borrow(|dots| {
        for &([wx, wz], color, death) in dots {
            let d0 = (wx - view.x) / bpp;
            let d1 = (wz - view.z) / bpp;
            let u = (-d0 * f[1] + f[0] * d1) / det;
            let v = (r[0] * d1 - r[1] * d0) / det;
            let x = (cx + u as f32 * scale).clamp(at[0] + 2.0, at[0] + size - 2.0);
            let y = (cy + v as f32 * scale).clamp(at[1] + 2.0, at[1] + size - 2.0);
            if death {
                // A death waypoint is a red X, matching the world marker.
                const RED: u32 = 0xFF_5555;
                let [_, red, green, blue] = RED.to_be_bytes();
                for (half, colour) in [
                    (4.0, rgba(0, 0, 0, 150)),
                    (3.0, rgba(red, green, blue, 255)),
                ] {
                    let t = half * 0.45;
                    batch.fill_quad(
                        atlas,
                        [
                            [x - half + t, y - half - t],
                            [x + half + t, y + half - t],
                            [x + half - t, y + half + t],
                            [x - half - t, y - half + t],
                        ],
                        colour,
                    );
                    batch.fill_quad(
                        atlas,
                        [
                            [x + half + t, y - half + t],
                            [x - half + t, y + half + t],
                            [x - half - t, y + half - t],
                            [x + half - t, y - half - t],
                        ],
                        colour,
                    );
                }
            } else {
                let [_, red, green, blue] = color.to_be_bytes();
                batch.fill(atlas, x - 2.5, y - 2.5, 5.0, 5.0, rgba(0, 0, 0, 150));
                batch.fill(
                    atlas,
                    x - 1.5,
                    y - 1.5,
                    3.0,
                    3.0,
                    rgba(red, green, blue, 255),
                );
            }
        }
    });
}
