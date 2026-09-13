// edit/apply.rs — apply the edit stack to a decoded image.
//
// Pure, deterministic ops. Same input + same stack ⇒ same output bytes.
// R·01 handles JPEG/PNG/WebP/TIFF/BMP via the `image` crate. RAW dev
// lands in R·02 via LibRaw; the caller will decide whether to hand off
// the RAW branch or fall back to a matched JPEG sidecar.

use anyhow::Result;
use image::{DynamicImage, GenericImageView, ImageBuffer, Rgba};

use super::EditOp;

pub fn apply_stack(img: DynamicImage, stack: &[EditOp]) -> Result<DynamicImage> {
    let mut out = img;
    for op in stack {
        out = match op {
            EditOp::Crop { x, y, w, h } => crop_relative(out, *x, *y, *w, *h),
            EditOp::Rotate { deg } => rotate(out, *deg),
            EditOp::Expose { ev } => expose(out, *ev),
            EditOp::Contrast { amount } => contrast(out, *amount),
            EditOp::Sat { amount } => saturate(out, *amount),
            EditOp::WhiteBalance { kelvin, tint } => white_balance(out, *kelvin, *tint),
        };
    }
    Ok(out)
}

fn crop_relative(img: DynamicImage, rx: f32, ry: f32, rw: f32, rh: f32) -> DynamicImage {
    let (w, h) = img.dimensions();
    let x = ((rx.clamp(0.0, 1.0)) * w as f32) as u32;
    let y = ((ry.clamp(0.0, 1.0)) * h as f32) as u32;
    let cw = ((rw.clamp(0.0, 1.0)) * w as f32) as u32;
    let ch = ((rh.clamp(0.0, 1.0)) * h as f32) as u32;
    let cw = cw.min(w.saturating_sub(x)).max(1);
    let ch = ch.min(h.saturating_sub(y)).max(1);
    img.crop_imm(x, y, cw, ch)
}

fn rotate(img: DynamicImage, deg: i32) -> DynamicImage {
    let d = ((deg % 360) + 360) % 360;
    match d {
        0 => img,
        90 => img.rotate90(),
        180 => img.rotate180(),
        270 => img.rotate270(),
        _ => img, // arbitrary angles wait for R·02
    }
}

fn expose(img: DynamicImage, ev: f32) -> DynamicImage {
    let gain = 2f32.powf(ev.clamp(-5.0, 5.0));
    map_rgba8(img, |r, g, b, a| {
        (clamp8(r as f32 * gain), clamp8(g as f32 * gain), clamp8(b as f32 * gain), a)
    })
}

fn contrast(img: DynamicImage, amount: f32) -> DynamicImage {
    // amount ∈ [-1, +1]; slope = 1 + amount * 2 (so +1 → 3x slope, -1 → -1x)
    let slope = (1.0 + amount.clamp(-1.0, 1.0) * 2.0).max(0.0);
    map_rgba8(img, |r, g, b, a| {
        let f = |v: u8| clamp8((v as f32 - 128.0) * slope + 128.0);
        (f(r), f(g), f(b), a)
    })
}

fn saturate(img: DynamicImage, amount: f32) -> DynamicImage {
    let s = (1.0 + amount.clamp(-1.0, 1.0)).max(0.0);
    map_rgba8(img, |r, g, b, a| {
        let (rf, gf, bf) = (r as f32, g as f32, b as f32);
        let lum = 0.2126 * rf + 0.7152 * gf + 0.0722 * bf;
        (
            clamp8(lum + (rf - lum) * s),
            clamp8(lum + (gf - lum) * s),
            clamp8(lum + (bf - lum) * s),
            a,
        )
    })
}

fn white_balance(img: DynamicImage, kelvin: f32, tint: f32) -> DynamicImage {
    // Approximate: kelvin > 0 warms (more R, less B); tint > 0 greens.
    let k = kelvin.clamp(-1.0, 1.0);
    let t = tint.clamp(-1.0, 1.0);
    let r_gain = 1.0 + k * 0.35;
    let b_gain = 1.0 - k * 0.35;
    let g_gain = 1.0 + t * 0.15;
    map_rgba8(img, |r, g, b, a| {
        (
            clamp8(r as f32 * r_gain),
            clamp8(g as f32 * g_gain),
            clamp8(b as f32 * b_gain),
            a,
        )
    })
}

fn clamp8(v: f32) -> u8 {
    v.round().clamp(0.0, 255.0) as u8
}

fn map_rgba8<F>(img: DynamicImage, f: F) -> DynamicImage
where
    F: Fn(u8, u8, u8, u8) -> (u8, u8, u8, u8) + Sync + Send,
{
    let mut rgba = img.to_rgba8();
    for px in rgba.pixels_mut() {
        let [r, g, b, a] = px.0;
        let (nr, ng, nb, na) = f(r, g, b, a);
        px.0 = [nr, ng, nb, na];
    }
    DynamicImage::ImageRgba8(rgba)
}

/// Convenience: build an 8-bit RGBA image from raw pixels for tests.
#[allow(dead_code)]
pub(crate) fn from_pixels(w: u32, h: u32, px: Vec<u8>) -> DynamicImage {
    DynamicImage::ImageRgba8(ImageBuffer::<Rgba<u8>, _>::from_raw(w, h, px).unwrap())
}
