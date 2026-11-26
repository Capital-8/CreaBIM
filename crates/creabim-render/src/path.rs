//! A progressive path tracer: each pass adds one sample to every pixel,
//! following light as it bounces (sun aimed at directly, sky found by
//! bouncing), through glass, off metal and rough surfaces. One pass looks
//! like static; a few hundred look like a photograph.
//!
//! Light is kept in two parts, so the denoiser can clean the noisy one:
//! `direct` is what the first surface (behind any glass) gets straight
//! from the sun, plus sky and reflections seen through glass; `indirect`
//! is everything that bounced on the way.

use crate::scene::{Scene, GROUPS, SKY};
use crate::sky::Sky;
use crate::util::*;
use glam::Vec3;
use std::f32::consts::PI;

pub struct Accum {
    pub w: usize,
    pub h: usize,
    pub spp: u32,
    pub direct: Vec<Vec3>,
    pub indirect: Vec<Vec3>,
    /// The first surface's colour, normal and distance: guides for the
    /// denoiser, averaged over the samples.
    pub albedo: Vec<Vec3>,
    pub normal: Vec<Vec3>,
    pub depth: Vec<f32>,
    /// The same light by group (0 the sun and sky, then the lamps'
    /// groups), for mixing afterwards: `direct` and `indirect` are their
    /// sums.
    pub group_direct: Vec<Vec<Vec3>>,
    pub group_indirect: Vec<Vec<Vec3>>,
}

/// Light by group.
type Gl = [Vec3; GROUPS];
const NONE: Gl = [Vec3::ZERO; GROUPS];

fn plus(a: Gl, b: Gl) -> Gl {
    std::array::from_fn(|i| a[i] + b[i])
}

#[derive(Clone, Copy)]
struct Px {
    direct: Gl,
    indirect: Gl,
    albedo: Vec3,
    normal: Vec3,
    depth: f32,
}

impl Default for Px {
    fn default() -> Px {
        Px {
            direct: NONE,
            indirect: NONE,
            albedo: Vec3::ZERO,
            normal: Vec3::ZERO,
            depth: 0.0,
        }
    }
}

pub const MAX_BOUNCES: u32 = 8;

impl Accum {
    pub fn new(w: usize, h: usize) -> Accum {
        let z = vec![Vec3::ZERO; w * h];
        Accum {
            w,
            h,
            spp: 0,
            direct: z.clone(),
            indirect: z.clone(),
            albedo: z.clone(),
            normal: z.clone(),
            depth: vec![0.0; w * h],
            group_direct: vec![z.clone(); GROUPS],
            group_indirect: vec![z; GROUPS],
        }
    }

    /// The light mixed again, each group times its weight (a colour, so
    /// a group can be dimmed or tinted): what Blender's light groups and
    /// V-Ray's LightMix do, without rendering again.
    pub fn mix(&self, weights: [Vec3; GROUPS]) -> Accum {
        let mut a = Accum {
            w: self.w,
            h: self.h,
            spp: self.spp,
            direct: vec![Vec3::ZERO; self.w * self.h],
            indirect: vec![Vec3::ZERO; self.w * self.h],
            albedo: self.albedo.clone(),
            normal: self.normal.clone(),
            depth: self.depth.clone(),
            group_direct: vec![],
            group_indirect: vec![],
        };
        for g in 0..GROUPS {
            for i in 0..self.w * self.h {
                a.direct[i] += self.group_direct[g][i] * weights[g];
                a.indirect[i] += self.group_indirect[g][i] * weights[g];
            }
        }
        a
    }

    /// Add one sample to every pixel; returns the rays traced.
    pub fn pass(&mut self, scene: &Scene, cam: &Camera, sky: &Sky) -> u64 {
        let (w, h) = (self.w, self.h);
        let aspect = w as f32 / h as f32;
        let pass = self.spp;
        let rays = std::sync::atomic::AtomicU64::new(0);
        let px = par_rows(w, h, Px::default(), |y, row| {
            let mut n = 0u64;
            for (x, out) in row.iter_mut().enumerate() {
                let mut rng = Rng::new(
                    x as u32 + 7919 * pass,
                    y as u32,
                    pass.wrapping_mul(2654435761),
                );
                let (fx, fy) = (
                    (x as f32 + rng.f()) / w as f32,
                    (y as f32 + rng.f()) / h as f32,
                );
                let (o, d) = cam.ray(fx, fy, aspect, Some((rng.f(), rng.f())));
                let (o, ahead) = scene.ray_start(o, d, cam.eye);
                let mut px = sample(scene, sky, o, d, &mut rng, &mut n);
                // the denoiser compares depths from the eye
                if px.depth < 1e4 {
                    px.depth = (px.depth + ahead).max(0.0);
                }
                *out = px;
            }
            rays.fetch_add(n, std::sync::atomic::Ordering::Relaxed);
        });
        for (i, p) in px.into_iter().enumerate() {
            for g in 0..GROUPS {
                self.direct[i] += p.direct[g];
                self.indirect[i] += p.indirect[g];
                self.group_direct[g][i] += p.direct[g];
                self.group_indirect[g][i] += p.indirect[g];
            }
            self.albedo[i] += p.albedo;
            self.normal[i] += p.normal;
            self.depth[i] += p.depth;
        }
        self.spp += 1;
        rays.into_inner()
    }

    /// The image so far, linear radiance.
    pub fn mean(&self) -> Vec<Vec3> {
        let k = 1.0 / self.spp.max(1) as f32;
        (0..self.w * self.h)
            .map(|i| (self.direct[i] + self.indirect[i]) * k)
            .collect()
    }
}

/// One camera path's light.
fn sample(scene: &Scene, sky: &Sky, o: Vec3, d: Vec3, rng: &mut Rng, rays: &mut u64) -> Px {
    let mut px = Px::default();
    let mut o = o;
    let mut thr = Vec3::ONE;
    let mut travelled = 0.0;
    // follow the ray through glass to the first surface that scatters
    for _ in 0..8 {
        *rays += 1;
        let Some(h) = scene.bvh.intersect(o, d, 1e6) else {
            let (sun, glow) = escape(sky, d, true);
            px.direct[0] += thr * sun;
            px.direct[SKY] += thr * glow;
            px.albedo = thr;
            px.normal = -d;
            px.depth = 1e4;
            return px;
        };
        travelled += h.t;
        let p = o + d * h.t;
        let (n, geo) = scene.normals(h.tri, h.u, h.v);
        let (n, geo) = if geo.dot(d) > 0.0 {
            (-n, -geo)
        } else {
            (n, geo)
        };
        let m = *scene.material(h.tri);
        if m.emit != Vec3::ZERO {
            // a lamp seen: its group is the nearest lamp's
            px.direct[lamp_group(scene, p)] += thr * m.emit;
            px.albedo = thr * m.emit.normalize_or_zero();
            px.normal = n;
            px.depth = travelled;
            return px;
        }
        if m.glass {
            // split: the reflection traced in full, the rest goes on through
            let f = schlick(n.dot(-d).abs(), 0.04);
            let r = reflect(d, n);
            let (rd, ri) = path(scene, sky, p + geo * 1e-3, r, thr * f, false, rng, rays);
            px.direct = plus(px.direct, plus(rd, ri));
            thr *= (1.0 - f) * pane(&m);
            o = p + d * 1e-3;
            continue;
        }
        px.albedo = thr
            * if m.metal > 0.5 {
                m.base.max(Vec3::splat(0.04))
            } else {
                m.base
            };
        px.normal = n;
        px.depth = travelled;
        let (dl, il) = scatter_from(scene, sky, p, n, geo, -d, &m, thr, rng, rays);
        px.direct = plus(px.direct, dl);
        px.indirect = plus(px.indirect, il);
        return px;
    }
    px
}

/// Sky (and the sun's disc, when nothing could have aimed at it) for a ray
/// that leaves the scene.
/// Returned apart: the sun's disc, then the sky.
fn escape(sky: &Sky, d: Vec3, see_sun: bool) -> (Vec3, Vec3) {
    let disc = if see_sun && d.dot(sky.sun) > sky.sun_radius.cos() {
        sky.sun_disc()
    } else {
        Vec3::ZERO
    };
    (disc, sky.radiance(d))
}

fn pane(m: &crate::scene::Material) -> Vec3 {
    Vec3::splat(0.92).lerp(m.base, 0.15)
}

/// Light leaving a surface point toward `v`: the sun aimed at directly
/// (returned first), then everything found by bouncing on.
#[allow(clippy::too_many_arguments)]
fn scatter_from(
    scene: &Scene,
    sky: &Sky,
    p: Vec3,
    n: Vec3,
    geo: Vec3,
    v: Vec3,
    m: &crate::scene::Material,
    thr: Vec3,
    rng: &mut Rng,
    rays: &mut u64,
) -> (Gl, Gl) {
    let mut direct = NONE;
    direct[0] = thr * sun_light(scene, sky, p, n, geo, v, m, rng, rays);
    // lamps are aimed at one at a time, so their light is noisy: it goes
    // with the bounced light, which the denoiser cleans
    let mut indirect = NONE;
    if let Some((g, c)) = lamp_light(scene, p, n, geo, v, m, rng, rays) {
        indirect[g] += thr * c;
    }
    let Some((l, w)) = bounce(n, geo, v, m, rng) else {
        return (direct, indirect);
    };
    let o = p + if l.dot(geo) > 0.0 { geo } else { -geo } * 1e-3;
    let (a, b) = path(scene, sky, o, l, thr * w, true, rng, rays);
    (direct, plus(indirect, plus(a, b)))
}

/// The group of the lamp nearest a point.
fn lamp_group(scene: &Scene, p: Vec3) -> usize {
    scene
        .lamps
        .iter()
        .min_by(|a, b| {
            (a.pos - p)
                .length_squared()
                .total_cmp(&(b.pos - p).length_squared())
        })
        .map_or(0, |l| l.group)
}

/// One lamp, aimed at: chosen by how much light it could bring here (its
/// lumens over the distance squared), one shadow ray to a point on its
/// disc. Returns its group and the light, divided by the chance of
/// choosing it.
#[allow(clippy::too_many_arguments)]
fn lamp_light(
    scene: &Scene,
    p: Vec3,
    n: Vec3,
    geo: Vec3,
    v: Vec3,
    m: &crate::scene::Material,
    rng: &mut Rng,
    rays: &mut u64,
) -> Option<(usize, Vec3)> {
    if scene.lamps.is_empty() {
        return None;
    }
    let weight = |l: &crate::scene::Lamp| l.power() / (l.pos - p).length_squared().max(0.25);
    let total: f32 = scene.lamps.iter().map(weight).sum();
    if total <= 0.0 {
        return None;
    }
    let mut pick = rng.f() * total;
    let mut chosen = &scene.lamps[0];
    for l in &scene.lamps {
        pick -= weight(l);
        chosen = l;
        if pick <= 0.0 {
            break;
        }
    }
    let prob = weight(chosen) / total;
    let (t, b) = basis(chosen.axis);
    let r = chosen.radius * rng.f().sqrt();
    let ph = 2.0 * PI * rng.f();
    let q = chosen.pos + (t * ph.cos() + b * ph.sin()) * r;
    let to = q - p;
    let dist2 = to.length_squared().max(1e-4);
    let l = to / dist2.sqrt();
    let f = brdf_cos(n, v, l, m);
    if f == Vec3::ZERO {
        return None;
    }
    let i = chosen.intensity(-l);
    if i == Vec3::ZERO {
        return None;
    }
    *rays += 1;
    let o = if m.foliage {
        p + l * 0.08
    } else {
        p + if l.dot(geo) > 0.0 { geo } else { -geo } * 1e-3
    };
    let vis = scene
        .bvh
        .transmit(o, l, dist2.sqrt() - 0.02, |t| scene.is_glass(t));
    Some((chosen.group, clamp_lum(f * i * vis / dist2 / prob, 40.0)))
}

/// Follow a ray on from a bounce, adding light where it is found.
#[allow(clippy::too_many_arguments)]
fn path(
    scene: &Scene,
    sky: &Sky,
    mut o: Vec3,
    mut d: Vec3,
    mut thr: Vec3,
    after_scatter: bool,
    rng: &mut Rng,
    rays: &mut u64,
) -> (Gl, Gl) {
    let mut acc = NONE;
    // after a rough bounce the sun was already aimed at: don't add it twice
    let mut see_sun = !after_scatter;
    for depth in 0..MAX_BOUNCES {
        *rays += 1;
        let Some(h) = scene.bvh.intersect(o, d, 1e6) else {
            let (sun, glow) = escape(sky, d, see_sun);
            acc[0] += clamp_lum(thr * sun, 12.0);
            acc[SKY] += clamp_lum(thr * glow, 12.0);
            break;
        };
        let p = o + d * h.t;
        let (n, geo) = scene.normals(h.tri, h.u, h.v);
        let (n, geo) = if geo.dot(d) > 0.0 {
            (-n, -geo)
        } else {
            (n, geo)
        };
        let m = *scene.material(h.tri);
        if m.glass {
            let f = schlick(n.dot(-d).abs(), 0.04);
            if rng.f() < f {
                d = reflect(d, n);
                o = p + geo * 1e-3;
            } else {
                thr *= pane(&m);
                o = p + d * 1e-3;
            }
            continue;
        }
        if m.emit != Vec3::ZERO {
            // lamps were aimed at already
            break;
        }
        acc[0] += clamp_lum(
            thr * sun_light(scene, sky, p, n, geo, -d, &m, rng, rays),
            12.0,
        );
        if let Some((g, c)) = lamp_light(scene, p, n, geo, -d, &m, rng, rays) {
            acc[g] += clamp_lum(thr * c, 12.0);
        }
        let Some((l, w)) = bounce(n, geo, -d, &m, rng) else {
            break;
        };
        thr *= w;
        see_sun = false;
        if depth >= 2 {
            let q = thr.max_element().clamp(0.05, 0.95);
            if rng.f() > q {
                break;
            }
            thr /= q;
        }
        o = p + if l.dot(geo) > 0.0 { geo } else { -geo } * 1e-3;
        d = l;
    }
    (NONE, acc)
}

fn clamp_lum(c: Vec3, max: f32) -> Vec3 {
    let l = luminance(c);
    if l > max {
        c * (max / l)
    } else {
        c
    }
}

fn reflect(d: Vec3, n: Vec3) -> Vec3 {
    d - n * 2.0 * d.dot(n)
}

fn schlick(cos: f32, f0: f32) -> f32 {
    f0 + (1.0 - f0) * (1.0 - cos.clamp(0.0, 1.0)).powi(5)
}

fn fresnel3(cos: f32, f0: Vec3) -> Vec3 {
    f0 + (Vec3::ONE - f0) * (1.0 - cos.clamp(0.0, 1.0)).powi(5)
}

/// Diffuse plus GGX specular, times the cosine.
fn brdf_cos(n: Vec3, v: Vec3, l: Vec3, m: &crate::scene::Material) -> Vec3 {
    let nl = n.dot(l);
    let nv = n.dot(v).max(1e-4);
    if m.foliage && nl < 0.0 {
        // leaves let some light through
        return m.base * (0.35 / PI) * -nl;
    }
    if nl <= 0.0 {
        return Vec3::ZERO;
    }
    let h = (v + l).normalize_or_zero();
    let f0 = Vec3::splat(0.04).lerp(m.base, m.metal);
    let a = m.rough * m.rough;
    let a2 = (a * a).max(1e-5);
    let nh = n.dot(h).max(0.0);
    let dd = nh * nh * (a2 - 1.0) + 1.0;
    let dist = a2 / (PI * dd * dd);
    let g = smith(nv, nl, a);
    let f = fresnel3(v.dot(h).max(0.0), f0);
    let spec = f * (dist * g / (4.0 * nv * nl));
    let diffuse = m.base * (1.0 - m.metal) * (Vec3::ONE - f) / PI;
    (diffuse + spec.min(Vec3::splat(50.0))) * nl
}

fn smith(nv: f32, nl: f32, a: f32) -> f32 {
    let k = a * 0.5;
    (nv / (nv * (1.0 - k) + k)) * (nl / (nl * (1.0 - k) + k))
}

/// The sun, aimed at: one shadow ray toward a point on its disc.
#[allow(clippy::too_many_arguments)]
fn sun_light(
    scene: &Scene,
    sky: &Sky,
    p: Vec3,
    n: Vec3,
    geo: Vec3,
    v: Vec3,
    m: &crate::scene::Material,
    rng: &mut Rng,
    rays: &mut u64,
) -> Vec3 {
    let l = sky.sample_sun(rng.f(), rng.f());
    if l.z <= 0.0 {
        return Vec3::ZERO;
    }
    let f = brdf_cos(n, v, l, m);
    if f == Vec3::ZERO {
        return Vec3::ZERO;
    }
    *rays += 1;
    let o = if m.foliage {
        p + l * 0.08
    } else {
        p + if l.dot(geo) > 0.0 { geo } else { -geo } * 1e-3
    };
    let vis = scene.bvh.transmit(o, l, 1e6, |t| scene.is_glass(t));
    f * sky.sun_light * vis
}

/// Pick the next direction by the surface's lobes; returns it with the
/// path's weight (brdf * cos / pdf).
fn bounce(
    n: Vec3,
    geo: Vec3,
    v: Vec3,
    m: &crate::scene::Material,
    rng: &mut Rng,
) -> Option<(Vec3, Vec3)> {
    let f0 = Vec3::splat(0.04).lerp(m.base, m.metal);
    let nv = n.dot(v).max(1e-4);
    let fv = fresnel3(nv, f0);
    let spec_w = luminance(fv) * (1.0 - 0.5 * m.rough * (1.0 - m.metal));
    let diff_w = luminance(m.base * (1.0 - m.metal) * (Vec3::ONE - fv));
    let p_spec = if diff_w + spec_w <= 0.0 {
        0.5
    } else {
        (spec_w / (spec_w + diff_w)).clamp(0.05, 1.0)
    };
    if m.foliage && rng.f() < 0.25 {
        // through the leaf
        let l = cosine_dir(-n, rng.f(), rng.f());
        return Some((l, m.base * 0.35 / 0.25));
    }
    if rng.f() < p_spec {
        let a = (m.rough * m.rough).max(1e-3);
        let (r1, r2) = (rng.f(), rng.f());
        let ph = 2.0 * PI * r1;
        let cos_t = ((1.0 - r2) / (1.0 + (a * a - 1.0) * r2)).sqrt();
        let sin_t = (1.0 - cos_t * cos_t).max(0.0).sqrt();
        let (t, b) = basis(n);
        let h = (t * (sin_t * ph.cos()) + b * (sin_t * ph.sin()) + n * cos_t).normalize();
        let l = reflect(-v, h);
        let nl = n.dot(l);
        if nl <= 0.0 || l.dot(geo) <= 0.0 {
            return None;
        }
        let vh = v.dot(h).max(1e-4);
        let nh = n.dot(h).max(1e-4);
        let f = fresnel3(vh, f0);
        let w = f * (smith(nv, nl, a) * vh / (nv * nh)) / p_spec;
        Some((l, w))
    } else {
        let l = cosine_dir(n, rng.f(), rng.f());
        if l.dot(geo) <= 0.0 {
            return None;
        }
        let w = m.base * (1.0 - m.metal) * (Vec3::ONE - fv) / (1.0 - p_spec).max(1e-3);
        Some((l, w))
    }
}
