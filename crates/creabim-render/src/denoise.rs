//! An edge-aware denoiser for the path tracer (an "à-trous" wavelet
//! filter, as in SVGF): the noisy bounced light is divided by the
//! surface colour, blurred in widening steps that stop at changes in
//! normal, distance and colour, and multiplied back. Detail in textures
//! and edges survives; grain in flat light goes.

use crate::path::Accum;
use crate::util::{luminance, Image};
use glam::Vec3;

pub fn denoise(acc: &Accum, steps: u32) -> Vec<Vec3> {
    let (w, h) = (acc.w, acc.h);
    let k = 1.0 / acc.spp.max(1) as f32;
    let albedo: Vec<Vec3> = acc.albedo.iter().map(|a| *a * k).collect();
    let normal: Vec<Vec3> = acc
        .normal
        .iter()
        .map(|n| (*n * k).normalize_or_zero())
        .collect();
    let depth: Vec<f32> = acc.depth.iter().map(|d| d * k).collect();
    let demod = |a: Vec3| a.max(Vec3::splat(0.03));
    let mut light: Vec<Vec3> = (0..w * h)
        .map(|i| acc.indirect[i] * k / demod(albedo[i]))
        .collect();
    // luminance spread falls as samples grow: noise is ~1/sqrt(spp)
    let mut sigma_l = 4.0 / (acc.spp as f32).sqrt();
    let kernel = [1.0 / 16.0, 1.0 / 4.0, 3.0 / 8.0, 1.0 / 4.0, 1.0 / 16.0];
    for step in 0..steps {
        let gap = 1isize << step;
        let src = light.clone();
        let out = crate::util::par_rows(w, h, Vec3::ZERO, |y, row| {
            for (x, o) in row.iter_mut().enumerate() {
                let i = y * w + x;
                let (c0, n0, d0, a0) = (src[i], normal[i], depth[i], albedo[i]);
                let l0 = luminance(c0);
                let mut sum = Vec3::ZERO;
                let mut wsum = 0.0;
                for (ky, kv) in kernel.iter().enumerate() {
                    let yy = y as isize + (ky as isize - 2) * gap;
                    if yy < 0 || yy >= h as isize {
                        continue;
                    }
                    for (kx, ku) in kernel.iter().enumerate() {
                        let xx = x as isize + (kx as isize - 2) * gap;
                        if xx < 0 || xx >= w as isize {
                            continue;
                        }
                        let j = yy as usize * w + xx as usize;
                        let wn = n0.dot(normal[j]).max(0.0).powi(32);
                        let wd = (-(d0 - depth[j]).abs() / (0.02 * d0.max(0.5) * gap as f32)).exp();
                        let wa = (-(a0 - albedo[j]).length() * 6.0).exp();
                        let wl = (-(l0 - luminance(src[j])).abs() / (sigma_l * (1.0 + l0))).exp();
                        let wt = ku * kv * wn * wd * wa * wl;
                        sum += src[j] * wt;
                        wsum += wt;
                    }
                }
                *o = if wsum > 0.0 { sum / wsum } else { c0 };
            }
        });
        light = out;
        sigma_l *= 0.7;
    }
    (0..w * h)
        .map(|i| acc.direct[i] * k + light[i] * demod(albedo[i]))
        .collect()
}

/// Linear radiance to a display image: exposure, filmic curve, sRGB.
pub fn develop(px: &[Vec3], w: usize, h: usize, exposure: f32) -> Image {
    Image {
        w,
        h,
        px: px
            .iter()
            .map(|c| crate::util::to_srgb(crate::util::aces(*c * exposure)))
            .collect(),
    }
}
