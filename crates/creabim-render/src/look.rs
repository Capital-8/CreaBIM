//! The fast modes: one pass of rays from the camera (what a GPU gets from
//! rasterising), then sun shadows and ambient occlusion, then a look:
//! today's 3D view, the real-time PBR mode, or a drawing style. Every
//! look reads the same scene and the same materials.

use crate::scene::Scene;
use crate::sky::Sky;
use crate::util::*;
use glam::{Vec2, Vec3};
use std::f32::consts::PI;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Look {
    /// The 3D view as it is today (its shader, reproduced).
    Today,
    /// Real-time PBR: sun and sky for the place and time, soft shadows,
    /// ambient occlusion, filmic tone; `traced` adds ray-traced
    /// reflections in glass and metal (the hybrid tier).
    Realtime { traced: bool },
    /// Every surface white: the massing model.
    Clay,
    /// Crisp lines over flat pale washes and shadows.
    LineWash,
    /// Washes that bleed, pool at their edges and soak into paper.
    Watercolour,
    /// Pencil: outlines and hatching by tone.
    Sketch,
    /// The OSAL toon preset, as in Blender's Workbench: the materials' own
    /// colours in flat bands by a fixed studio light, a near-black outline
    /// on every edge, cavity darkening in corners, faint soft shadows, no
    /// highlights, a light grey studio background. Render its G-buffer
    /// with [`Look::TOON_LIGHT`] as the sun and a short occlusion radius.
    Toon,
}

impl Look {
    /// The toon preset's light: from the upper front right, as OSAL's
    /// Workbench setting (0.35, -0.45, 0.82).
    pub const TOON_LIGHT: Vec3 = Vec3::new(0.35, -0.45, 0.82);

    pub fn name(self) -> &'static str {
        match self {
            Look::Today => "today",
            Look::Realtime { traced: false } => "realtime",
            Look::Realtime { traced: true } => "realtime-rt-reflections",
            Look::Clay => "clay",
            Look::LineWash => "line-and-wash",
            Look::Watercolour => "watercolour",
            Look::Sketch => "sketch",
            Look::Toon => "toon",
        }
    }
}

/// What the camera sees at one sample.
#[derive(Clone, Copy, Default)]
pub struct G {
    pub hit: bool,
    /// The ray's start and direction.
    pub o: Vec3,
    pub dir: Vec3,
    /// Distance to the first opaque surface.
    pub t: f32,
    pub p: Vec3,
    /// Shading normal, turned to face the camera.
    pub n: Vec3,
    pub tri: u32,
    pub obj: u32,
    /// Panes of glass passed through first, and the first one's normal.
    pub glass: u8,
    pub glass_n: Vec3,
    pub glass_t: f32,
    /// Sun reaching the point, 0..1, softened by the sun's disc.
    pub sun: f32,
    /// Sky reaching the point (ambient occlusion), 0..1.
    pub ao: f32,
}

pub struct Settings {
    /// Samples per pixel along each side (2 = 4 samples).
    pub ss: usize,
    pub shadow_rays: u32,
    pub ao_rays: u32,
    pub ao_radius: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            ss: 2,
            shadow_rays: 4,
            ao_rays: 8,
            ao_radius: 2.5,
        }
    }
}

pub struct Timings {
    pub primary_ms: f64,
    pub lighting_ms: f64,
    pub look_ms: f64,
    pub rays: u64,
}

/// Rays from the camera, then shadows and occlusion: the G-buffer.
pub fn gbuffer(
    scene: &Scene,
    cam: &Camera,
    sky: &Sky,
    w: usize,
    h: usize,
    s: &Settings,
    t: &mut Timings,
) -> Vec<G> {
    let (gw, gh) = (w * s.ss, h * s.ss);
    let aspect = w as f32 / h as f32;
    let t0 = std::time::Instant::now();
    let mut g = par_rows(gw, gh, G::default(), |y, row| {
        for (x, out) in row.iter_mut().enumerate() {
            let (o, d) = cam.ray(
                (x as f32 + 0.5) / gw as f32,
                (y as f32 + 0.5) / gh as f32,
                aspect,
                None,
            );
            let (o, ahead) = scene.ray_start(o, d, cam.eye);
            let mut g = primary(scene, o, d);
            // how far from the eye, for the haze and the edges: not from
            // where the ray was traced from
            if g.hit {
                g.t = (g.t + ahead).max(0.0);
            }
            *out = g;
        }
    });
    t.primary_ms = t0.elapsed().as_secs_f64() * 1e3;
    let t1 = std::time::Instant::now();
    let lit = par_rows(gw, gh, (0f32, 0f32), |y, row| {
        for (x, out) in row.iter_mut().enumerate() {
            let s0 = &g[y * gw + x];
            if !s0.hit {
                *out = (1.0, 1.0);
                continue;
            }
            let mut rng = Rng::new(x as u32, y as u32, 7);
            *out = lighting(scene, sky, s0, s, &mut rng);
        }
    });
    // the few rays per sample leave grain: smooth it over neighbours on the
    // same surface, as a real-time shadow and occlusion pass would
    let smooth = par_rows(gw, gh, (0f32, 0f32), |y, row| {
        for (x, out) in row.iter_mut().enumerate() {
            let a = &g[y * gw + x];
            if !a.hit {
                *out = lit[y * gw + x];
                continue;
            }
            let (mut sum, mut wsum) = ((0.0, 0.0), 0.0);
            let r = 4i32;
            for dy in -r..=r {
                for dx in -r..=r {
                    let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                    if nx < 0 || ny < 0 || nx >= gw as i32 || ny >= gh as i32 {
                        continue;
                    }
                    let j = ny as usize * gw + nx as usize;
                    let b = &g[j];
                    if !b.hit
                        || a.n.dot(b.n) < 0.9
                        || (b.p - a.p).dot(a.n).abs() > 0.01 * a.t.max(1.0)
                    {
                        continue;
                    }
                    let wt = (-((dx * dx + dy * dy) as f32) / 12.0).exp();
                    sum.0 += lit[j].0 * wt;
                    sum.1 += lit[j].1 * wt;
                    wsum += wt;
                }
            }
            *out = (sum.0 / wsum, sum.1 / wsum);
        }
    });
    for (gs, (sun, ao)) in g.iter_mut().zip(smooth) {
        gs.sun = sun;
        gs.ao = ao;
    }
    t.lighting_ms = t1.elapsed().as_secs_f64() * 1e3;
    t.rays = (gw * gh) as u64 * (1 + s.shadow_rays as u64 + s.ao_rays as u64);
    g
}

fn primary(scene: &Scene, mut o: Vec3, d: Vec3) -> G {
    let mut g = G {
        o,
        dir: d,
        ..Default::default()
    };
    let mut travelled = 0.0;
    for _ in 0..8 {
        let Some(h) = scene.bvh.intersect(o, d, 1e6) else {
            g.t = 1e6;
            return g;
        };
        let (n, _) = scene.normals(h.tri, h.u, h.v);
        let n = if n.dot(d) > 0.0 { -n } else { n };
        travelled += h.t;
        if scene.is_glass(h.tri) {
            if g.glass == 0 {
                g.glass_n = n;
                g.glass_t = travelled;
            }
            g.glass = g.glass.saturating_add(1);
            o += d * (h.t + 1e-3);
            continue;
        }
        g.hit = true;
        g.t = travelled;
        g.p = o + d * h.t;
        g.n = n;
        g.tri = h.tri;
        g.obj = scene.obj[h.tri as usize];
        return g;
    }
    g
}

fn lighting(scene: &Scene, sky: &Sky, g: &G, s: &Settings, rng: &mut Rng) -> (f32, f32) {
    let (_, geo) = scene.normals(g.tri, 0.33, 0.33);
    let geo = if geo.dot(g.dir) > 0.0 { -geo } else { geo };
    let o = g.p + geo * 2e-3;
    let foliage = scene.material(g.tri).foliage;
    let glass = |t: u32| scene.is_glass(t);
    let mut sun = 0.0;
    for _ in 0..s.shadow_rays {
        let l = sky.sample_sun(rng.f(), rng.f());
        if l.z <= 0.0 || (!foliage && geo.dot(l) <= 0.0) {
            continue;
        }
        // leaves: start a little toward the sun, past the crown's own facets
        let start = if foliage { g.p + l * 0.08 } else { o };
        sun += scene.bvh.transmit(start, l, 1e6, glass);
    }
    sun /= s.shadow_rays.max(1) as f32;
    let mut ao = 0.0;
    let up = if foliage {
        (g.n + Vec3::Z).normalize_or(Vec3::Z)
    } else {
        g.n
    };
    for _ in 0..s.ao_rays {
        let d = cosine_dir(up, rng.f(), rng.f());
        let d = if d.dot(geo) < 0.0 {
            (d - geo * 2.0 * d.dot(geo)).normalize()
        } else {
            d
        };
        let start = if foliage { g.p + d * 0.08 } else { o };
        ao += scene
            .bvh
            .transmit(start, d, s.ao_radius, glass)
            .min(1.0)
            .ceil();
    }
    ao /= s.ao_rays.max(1) as f32;
    (sun, ao)
}

// --- shading ---

fn ggx(n: Vec3, v: Vec3, l: Vec3, rough: f32, f0: Vec3) -> Vec3 {
    let h = (v + l).normalize_or_zero();
    let nh = n.dot(h).max(0.0);
    let nv = n.dot(v).max(1e-4);
    let nl = n.dot(l).max(1e-4);
    let a = (rough * rough).max(0.002);
    let a2 = a * a;
    let dd = nh * nh * (a2 - 1.0) + 1.0;
    let d = a2 / (PI * dd * dd);
    let k = a * 0.5;
    let g = (nv / (nv * (1.0 - k) + k)) * (nl / (nl * (1.0 - k) + k));
    let f = f0 + (Vec3::ONE - f0) * (1.0 - v.dot(h).max(0.0)).powi(5);
    f * (d * g / (4.0 * nv * nl))
}

fn fresnel(cos: f32, f0: Vec3, rough: f32) -> Vec3 {
    let r = Vec3::splat(1.0 - rough).max(f0);
    f0 + (r - f0) * (1.0 - cos.clamp(0.0, 1.0)).powi(5)
}

/// Haze over distance, towards the sky's horizon.
fn haze(c: Vec3, sky: &Sky, d: Vec3, t: f32) -> Vec3 {
    let f = 1.0 - (-t / 900.0).exp();
    c.lerp(sky.radiance(Vec3::new(d.x, d.y, 0.02).normalize()), f)
}

/// Real-time PBR shading of a G-buffer sample, linear radiance.
fn realtime(scene: &Scene, sky: &Sky, g: &G, traced: bool, rng: &mut Rng) -> Vec3 {
    if !g.hit && g.glass == 0 {
        return sky.radiance(g.dir);
    }
    let mut c = if g.hit {
        let c = surface(
            scene, sky, g.tri, g.p, g.n, -g.dir, g.sun, g.ao, traced, rng,
        );
        haze(c, sky, g.dir, g.t)
    } else {
        sky.radiance(g.dir)
    };
    if g.glass > 0 {
        let tint = scene
            .materials
            .iter()
            .find(|m| m.glass)
            .map(|m| m.base)
            .unwrap_or(Vec3::ONE);
        let pane = Vec3::splat(0.88).lerp(tint, 0.25);
        for _ in 0..g.glass.min(4) {
            c *= pane;
        }
        let v = -g.dir;
        let f = fresnel(g.glass_n.dot(v).abs(), Vec3::splat(0.04), 0.0);
        let r = g.dir - g.glass_n * 2.0 * g.dir.dot(g.glass_n);
        let refl = if traced {
            let o = g.o + g.dir * g.glass_t + g.glass_n * 2e-3;
            trace_simple(scene, sky, o, r, rng)
        } else {
            sky.radiance(r)
        };
        // panes read as reflective: the in-between of a single surface and
        // a double-glazed unit
        c = c * (Vec3::ONE - f * 1.6) + refl * f * 1.6;
    }
    c
}

/// A lamp's light is passed over where it would add less than this much
/// radiance (a step of the screen's at lamplight's exposure is about
/// 3e-5), or less than this share of the light already there.
const LAMP_FLOOR: f32 = 1.5e-5;
const LAMP_SHARE: f32 = 0.004;

#[allow(clippy::too_many_arguments)]
fn surface(
    scene: &Scene,
    sky: &Sky,
    tri: u32,
    p: Vec3,
    n: Vec3,
    v: Vec3,
    sun: f32,
    ao: f32,
    traced: bool,
    rng: &mut Rng,
) -> Vec3 {
    let m = scene.material(tri);
    if m.emit != Vec3::ZERO {
        // a lamp's glowing face
        return m.emit;
    }
    let l = sky.sun;
    let f0 = Vec3::splat(0.04).lerp(m.base, m.metal);
    let diffuse = m.base * (1.0 - m.metal);
    let ndl = if m.foliage {
        (n.dot(l) * 0.6 + 0.4).max(0.0)
    } else {
        n.dot(l).max(0.0)
    };
    let direct =
        (diffuse / PI * ndl + ggx(n, v, l, m.rough, f0) * n.dot(l).max(0.0)) * sky.sun_light * sun;
    let an = if m.foliage {
        (n + Vec3::Z).normalize_or(Vec3::Z)
    } else {
        n
    };
    let ambient = diffuse / PI * sky.ambient(an) * (0.25 + 0.75 * ao) * ao.max(0.35);
    // the lamps: each one's light straight onto the surface, behind
    // whatever stands between (always: unshadowed, a lamp would light the
    // rooms next door and the floor below). A lamp that could add next
    // to nothing here is passed over before its shadow is looked for:
    // by day that is nearly every lamp, at night the far ones. Looking
    // for every lamp's shadow from every point made the look five times
    // slower.
    let lit = crate::util::luminance(direct + ambient);
    let mut lamps = Vec3::ZERO;
    let least = LAMP_FLOOR + LAMP_SHARE * lit;
    for (lamp, peak) in scene.lamps.iter().zip(&scene.lamp_peak) {
        let to = lamp.pos - p;
        let d2 = to.length_squared().max(0.01);
        // too far to matter even at its strongest, square on
        if *peak < least * d2 {
            continue;
        }
        let dist = d2.sqrt();
        let ld = to / dist;
        let c = n.dot(ld);
        if c <= 0.0 {
            continue;
        }
        let i = lamp.intensity(-ld);
        if i == Vec3::ZERO {
            continue;
        }
        let add = (diffuse / PI * c + ggx(n, v, ld, m.rough, f0) * c) * i / d2;
        if crate::util::luminance(add) < least {
            continue;
        }
        let vis = scene
            .bvh
            .transmit(p + n * 2e-3, ld, (dist - 0.02).max(0.0), |t| {
                scene.is_glass(t)
            });
        lamps += add * vis;
    }
    let f = fresnel(n.dot(v), f0, m.rough);
    let r = (-v) - n * 2.0 * (-v).dot(n);
    let env = if traced && m.rough < 0.4 {
        trace_simple(scene, sky, p + n * 2e-3, r, rng)
    } else {
        sky.radiance(r).lerp(sky.ambient(n) / PI, m.rough)
    };
    let spec = f * env * (1.0 - 0.7 * m.rough) * ao;
    direct + ambient + spec + lamps
}

/// One reflection: what a mirror ray sees, lit simply.
fn trace_simple(scene: &Scene, sky: &Sky, o: Vec3, d: Vec3, rng: &mut Rng) -> Vec3 {
    let mut o = o;
    for _ in 0..4 {
        let Some(h) = scene.bvh.intersect(o, d, 1e6) else {
            return sky.radiance(d);
        };
        if scene.is_glass(h.tri) {
            o += d * (h.t + 1e-3);
            continue;
        }
        let (n, geo) = scene.normals(h.tri, h.u, h.v);
        let (n, geo) = if geo.dot(d) > 0.0 {
            (-n, -geo)
        } else {
            (n, geo)
        };
        let p = o + d * h.t;
        let l = sky.sample_sun(rng.f(), rng.f());
        let sun = if geo.dot(l) > 0.0 {
            scene
                .bvh
                .transmit(p + geo * 2e-3, l, 1e6, |t| scene.is_glass(t))
        } else {
            0.0
        };
        let m = scene.material(h.tri);
        let diffuse = m.base * (1.0 - m.metal);
        let c = diffuse / PI * (sky.sun_light * n.dot(l).max(0.0) * sun + sky.ambient(n) * 0.8);
        return haze(c, sky, d, h.t);
    }
    sky.radiance(d)
}

/// Today's 3D view shader, as in creabim-app's render3d.rs (display
/// values).
fn today(scene: &Scene, g: &G) -> Vec3 {
    let bg = Vec3::new(0.93, 0.94, 0.96);
    let mut c = if g.hit {
        let m = scene.material(g.tri);
        shade_today(m, g.n, -g.dir)
    } else {
        bg
    };
    if g.glass > 0 {
        let glass = scene.materials.iter().find(|m| m.glass).copied();
        if let Some(m) = glass {
            let gc = shade_today(&m, g.glass_n, -g.dir);
            for _ in 0..g.glass.min(2) {
                c = c.lerp(gc, 0.3);
            }
        }
    }
    c
}

fn shade_today(m: &crate::scene::Material, n: Vec3, v: Vec3) -> Vec3 {
    let l = Vec3::new(0.4, -0.6, 0.85).normalize();
    let base = m.base;
    let diffuse = n.dot(l).max(0.0);
    let sky = 0.5 + 0.5 * n.z;
    let lit = base * (1.0 - m.metal) * (0.30 + 0.25 * sky + 0.55 * diffuse);
    let f0 = Vec3::splat(0.04).lerp(base, m.metal);
    let fres = (1.0 - n.dot(v).max(0.0)).powi(5);
    let f = f0 + (Vec3::ONE - f0) * fres * (1.0 - m.rough);
    let h = (l + v).normalize();
    let a = m.rough * m.rough;
    let shine = (2.0 / (a * a) - 2.0).clamp(1.0, 2048.0);
    let hl = f * n.dot(h).max(0.0).powf(shine) * (shine + 2.0) / 8.0 * diffuse;
    let r = (-v) - n * 2.0 * (-v).dot(n);
    let env = Vec3::new(0.30, 0.30, 0.32).lerp(Vec3::new(0.95, 0.97, 1.0), 0.5 + 0.5 * r.z);
    let refl = f * env * (1.0 - 0.6 * m.rough);
    let c = lit + hl.min(Vec3::splat(4.0)) * (1.0 - 0.5 * m.rough) + refl;
    c.max(Vec3::ZERO).powf(1.0 / 2.2)
}

// --- outlines ---

/// Edge strength at each sample: element boundaries, creases and
/// silhouettes. Leaves outline only their crown against other things.
fn edges(scene: &Scene, g: &[G], gw: usize, gh: usize, crease: f32) -> Vec<f32> {
    par_rows(gw, gh, 0f32, |y, row| {
        for (x, out) in row.iter_mut().enumerate() {
            let a = &g[y * gw + x];
            let mut e = 0f32;
            for (dx, dy) in [(1i32, 0i32), (0, 1), (-1, 0), (0, -1)] {
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                if nx < 0 || ny < 0 || nx >= gw as i32 || ny >= gh as i32 {
                    continue;
                }
                let b = &g[ny as usize * gw + nx as usize];
                if a.hit != b.hit {
                    e = 1.0;
                    break;
                }
                if !a.hit {
                    continue;
                }
                let fa = scene.material(a.tri).foliage;
                let fb = scene.material(b.tri).foliage;
                let gap = (b.p - a.p).dot(a.n).abs() / a.t.max(0.1);
                if fa && fb {
                    if gap > 0.03 && b.t > a.t {
                        e = e.max(0.6);
                    }
                    continue;
                }
                if a.obj != b.obj && b.t >= a.t - 1e-3 {
                    e = 1.0;
                } else if a.n.dot(b.n) < crease {
                    e = e.max(0.9);
                } else if gap > 0.012 && b.t > a.t {
                    e = 1.0;
                }
            }
            *out = e;
        }
    })
}

// --- the styles ---

fn ink() -> Vec3 {
    Vec3::new(0.13, 0.13, 0.15)
}

pub fn render(
    scene: &Scene,
    cam: &Camera,
    sky: &Sky,
    w: usize,
    h: usize,
    look: Look,
    s: &Settings,
) -> (Image, Timings) {
    let mut t = Timings {
        primary_ms: 0.0,
        lighting_ms: 0.0,
        look_ms: 0.0,
        rays: 0,
    };
    let g = gbuffer(scene, cam, sky, w, h, s, &mut t);
    let t2 = std::time::Instant::now();
    let img = shade(scene, sky, &g, w, h, look, s, 1.0);
    t.look_ms = t2.elapsed().as_secs_f64() * 1e3;
    (img, t)
}

/// One look over a G-buffer already made; `exposure` brightens the
/// real-time mode (an interior wants more than the street).
#[allow(clippy::too_many_arguments)]
pub fn shade(
    scene: &Scene,
    sky: &Sky,
    g: &[G],
    w: usize,
    h: usize,
    look: Look,
    s: &Settings,
    exposure: f32,
) -> Image {
    let (gw, gh) = (w * s.ss, h * s.ss);
    let edge = match look {
        Look::Today | Look::Realtime { .. } => vec![],
        _ => edges(scene, g, gw, gh, 0.8),
    };
    let ss = s.ss as f32;
    let px = par_rows(gw, gh, Vec3::ZERO, |y, row| {
        for (x, out) in row.iter_mut().enumerate() {
            let i = y * gw + x;
            let gs = &g[i];
            let mut rng = Rng::new(x as u32, y as u32, 99);
            // final-image pixel coordinates, for paper and strokes
            let q = Vec2::new(x as f32 / ss, y as f32 / ss);
            *out = match look {
                Look::Today => today(scene, gs),
                Look::Realtime { traced } => {
                    let c = realtime(scene, sky, gs, traced, &mut rng);
                    to_srgb(aces(c * 0.85 * exposure))
                }
                Look::Clay => clay(scene, sky, gs),
                Look::LineWash => line_wash(scene, sky, gs, edge[i], q, h as f32),
                Look::Watercolour => watercolour_wash(scene, sky, gs, q),
                Look::Sketch => sketch(scene, sky, gs, edge[i], q),
                Look::Toon => toon(scene, sky, gs, edge[i]),
            };
        }
    });
    let mut img = Image { w: gw, h: gh, px }.downsample(s.ss);
    match look {
        Look::Watercolour => {
            let e = Image {
                w: gw,
                h: gh,
                px: edge.iter().map(|&e| Vec3::splat(e)).collect(),
            }
            .downsample(s.ss);
            img = watercolour_paper(&img, &e);
        }
        Look::Realtime { .. } => img = vignette(&img, 0.18),
        _ => {}
    }
    img
}

pub fn vignette(img: &Image, k: f32) -> Image {
    let mut out = img.clone();
    for y in 0..img.h {
        for x in 0..img.w {
            let u = (x as f32 / img.w as f32 - 0.5) * 2.0;
            let v = (y as f32 / img.h as f32 - 0.5) * 2.0;
            let r = (u * u * 0.6 + v * v * 0.4).min(1.0);
            out.px[y * img.w + x] *= 1.0 - k * r * r;
        }
    }
    out
}

fn clay(scene: &Scene, sky: &Sky, g: &G) -> Vec3 {
    let bg = Vec3::new(0.95, 0.95, 0.94);
    if !g.hit {
        // a soft studio sweep in place of the sky
        let k = (g.dir.z * 2.0).clamp(0.0, 1.0);
        let c = bg.lerp(Vec3::new(0.86, 0.88, 0.91), k);
        return if g.glass > 0 { c * 0.92 } else { c };
    }
    let m = scene.material(g.tri);
    let albedo = if m.foliage {
        0.62
    } else if m.metal > 0.5 {
        0.5
    } else {
        0.78
    };
    let ndl = if m.foliage {
        (g.n.dot(sky.sun) * 0.6 + 0.4).max(0.0)
    } else {
        g.n.dot(sky.sun).max(0.0)
    };
    // studio light: whiter sun, even sky
    let nz = if m.foliage { 0.5 + 0.5 * g.n.z } else { g.n.z };
    let light = 2.1 * ndl * g.sun + 1.25 * (0.55 + 0.45 * nz) * g.ao.powf(1.3);
    let mut c = Vec3::splat(albedo / PI * light);
    c = to_srgb(aces(c * 1.3));
    if g.glass > 0 {
        // glass as a pale grey-blue film
        c = c.lerp(Vec3::new(0.70, 0.76, 0.80), 0.45);
    }
    c
}

fn desaturate(c: Vec3, k: f32) -> Vec3 {
    Vec3::splat(luminance(c)).lerp(c, 1.0 - k)
}

fn line_wash(scene: &Scene, sky: &Sky, g: &G, edge: f32, q: Vec2, h: f32) -> Vec3 {
    let paper = Vec3::new(0.985, 0.98, 0.97);
    let mut c = if !g.hit {
        // a pale wash of sky at the top, fading out
        let k = smoothstep(0.55, 0.0, q.y / h);
        paper.lerp(Vec3::new(0.80, 0.87, 0.93), k * 0.7)
    } else {
        let m = scene.material(g.tri);
        let base = to_srgb(m.base);
        let wash = desaturate(base, 0.35).lerp(paper, 0.45);
        let lit = g.n.dot(sky.sun).max(0.0) * g.sun;
        // flat shadows: two tones, lit and shade
        let shade = smoothstep(0.05, 0.25, lit);
        let shadow = wash * Vec3::new(0.72, 0.74, 0.82);
        let mut c = shadow.lerp(wash, shade);
        c *= 0.85 + 0.15 * g.ao;
        c
    };
    if g.glass > 0 {
        c = c.lerp(Vec3::new(0.68, 0.80, 0.88), 0.45);
    }
    c.lerp(ink(), edge.min(1.0))
}

fn watercolour_wash(scene: &Scene, sky: &Sky, g: &G, q: Vec2) -> Vec3 {
    let paper = Vec3::new(0.97, 0.955, 0.92);
    if !g.hit {
        // sky: a loose blue wash, stronger high up
        let k = (g.dir.z * 3.0 + 0.1).clamp(0.0, 1.0);
        let n = fbm(q / 90.0, 4, 3);
        let blue = Vec3::new(0.62, 0.76, 0.90);
        let c = paper.lerp(blue, (k * 0.8 * (0.6 + 0.6 * n)).clamp(0.0, 0.85));
        return if g.glass > 0 {
            c.lerp(Vec3::new(0.55, 0.68, 0.78), 0.4)
        } else {
            c
        };
    }
    let m = scene.material(g.tri);
    let base = to_srgb(m.base);
    // pigment: the colour pushed a little richer, then thinned with water
    let pig = desaturate(base, -0.25).clamp(Vec3::ZERO, Vec3::ONE);
    let lit = if m.foliage {
        (g.n.dot(sky.sun) * 0.6 + 0.4).max(0.0) * g.sun
    } else {
        g.n.dot(sky.sun).max(0.0) * g.sun
    };
    let shade = smoothstep(0.03, 0.35, lit);
    let lightwash = pig.lerp(paper, 0.35);
    let shadow = pig.lerp(paper, 0.15) * Vec3::new(0.74, 0.74, 0.86);
    let mut c = shadow.lerp(lightwash, shade);
    c = c.lerp(c * 0.8, 1.0 - g.ao);
    if g.glass > 0 {
        c = c.lerp(Vec3::new(0.45, 0.58, 0.70), 0.5);
    }
    c
}

/// The paper pass: washes wobble and bleed, darken where they pool at
/// their edges, granulate into the paper's grain, and fade out toward the
/// sheet's edge; faint pencil lines under it all.
fn watercolour_paper(img: &Image, edge: &Image) -> Image {
    let paper = Vec3::new(0.97, 0.955, 0.92);
    let (w, h) = (img.w, img.h);
    // bleed: sample through a noise-warped grid
    let warped = par_rows(w, h, Vec3::ZERO, |y, row| {
        for (x, out) in row.iter_mut().enumerate() {
            let q = Vec2::new(x as f32, y as f32);
            let off = Vec2::new(fbm(q / 22.0, 3, 11) - 0.5, fbm(q / 22.0, 3, 17) - 0.5) * 9.0;
            *out = img.sample(q + off + Vec2::splat(0.5));
        }
    });
    let warped = Image { w, h, px: warped };
    // a blurred copy, for pooling at edges
    let blur = box_blur(&box_blur(&warped, 4), 4);
    let px = par_rows(w, h, Vec3::ZERO, |y, row| {
        for (x, out) in row.iter_mut().enumerate() {
            let q = Vec2::new(x as f32, y as f32);
            let i = y * w + x;
            let c = warped.px[i];
            let b = blur.px[i];
            // pigment carried to the wash's rim
            let diff = (c - b).length();
            let mut c = c - Vec3::splat(diff * 0.3);
            c = c.lerp(c * c, (diff * 2.0).min(0.3));
            // granulation: pigment settles in the grain, more where darker
            let grain = fbm(q / 2.2, 2, 5) - 0.5;
            let density = (1.0 - luminance(c)).clamp(0.0, 1.0);
            c -= Vec3::splat(grain * 0.10 * density);
            // the fade to bare paper toward the sheet's edges, raggedly
            let u = (x as f32 / w as f32 - 0.5) * 2.0;
            let v = (y as f32 / h as f32 - 0.5) * 2.0;
            let r = (u.abs().powf(3.0) + v.abs().powf(3.0)).powf(1.0 / 3.0);
            let rag = fbm(q / 40.0, 4, 23) * 0.35;
            let keep = smoothstep(1.02, 0.78, r + rag - 0.12);
            c = paper.lerp(c, keep);
            // pencil under the wash
            let e = edge.px[i].x * (0.45 + 0.4 * fbm(q / 6.0, 2, 41));
            c = c.lerp(Vec3::new(0.35, 0.33, 0.32), e * 0.55);
            // paper texture
            let tex = fbm(q / 1.3, 2, 77) - 0.5;
            *out = c * (1.0 + tex * 0.05);
        }
    });
    Image { w, h, px }
}

fn box_blur(img: &Image, r: isize) -> Image {
    let (w, h) = (img.w, img.h);
    let horiz = par_rows(w, h, Vec3::ZERO, |y, row| {
        for (x, out) in row.iter_mut().enumerate() {
            let mut s = Vec3::ZERO;
            for k in -r..=r {
                s += img.at(x as isize + k, y as isize);
            }
            *out = s / (2 * r + 1) as f32;
        }
    });
    let hi = Image { w, h, px: horiz };
    let px = par_rows(w, h, Vec3::ZERO, |y, row| {
        for (x, out) in row.iter_mut().enumerate() {
            let mut s = Vec3::ZERO;
            for k in -r..=r {
                s += hi.at(x as isize, y as isize + k);
            }
            *out = s / (2 * r + 1) as f32;
        }
    });
    Image { w, h, px }
}

/// Pencil hatching: tone picks how many layers of strokes cross a point.
fn sketch(scene: &Scene, sky: &Sky, g: &G, edge: f32, q: Vec2) -> Vec3 {
    let paper = Vec3::new(0.975, 0.965, 0.94);
    let graphite = Vec3::new(0.24, 0.24, 0.27);
    let tone = if !g.hit {
        if g.glass > 0 {
            0.35
        } else {
            0.0
        }
    } else {
        let m = scene.material(g.tri);
        let ndl = if m.foliage {
            (g.n.dot(sky.sun) * 0.6 + 0.4).max(0.0)
        } else {
            g.n.dot(sky.sun).max(0.0)
        };
        let light = 0.75 * ndl * g.sun + 0.35 * (0.5 + 0.5 * g.n.z) * g.ao;
        let albedo = luminance(to_srgb(m.base)).clamp(0.25, 0.95);
        let l = (light * (0.55 + 0.45 * albedo)).clamp(0.0, 1.0);
        let t = 1.0 - l;
        if g.glass > 0 {
            t.max(0.45)
        } else {
            t
        }
    };
    let stroke = |angle: f32, spacing: f32, seed: u32| -> f32 {
        let (s, c) = angle.to_radians().sin_cos();
        let along = q.x * c + q.y * s;
        let across = -q.x * s + q.y * c;
        let wob = (noise(Vec2::new(along / 40.0, across / 9.0), seed) - 0.5) * 1.6;
        let f = ((across + wob) / spacing).fract();
        let line = smoothstep(0.30, 0.0, (f - 0.5).abs() * spacing / 1.1 - 0.15);
        // strokes start and stop
        let gap = noise(
            Vec2::new(along / 55.0, (across / spacing).floor() * 1.7),
            seed + 3,
        );
        line * smoothstep(0.18, 0.32, gap)
    };
    let mut ink_amt = 0f32;
    if tone > 0.22 {
        ink_amt = ink_amt.max(stroke(52.0, 6.0, 1) * smoothstep(0.22, 0.4, tone) * 0.55);
    }
    if tone > 0.45 {
        ink_amt = ink_amt.max(stroke(-38.0, 5.0, 2) * smoothstep(0.45, 0.6, tone) * 0.7);
    }
    if tone > 0.68 {
        ink_amt = ink_amt.max(stroke(8.0, 4.0, 3) * smoothstep(0.68, 0.85, tone) * 0.8);
    }
    let pressure = 0.7 + 0.3 * noise(q / 30.0, 9);
    let e = edge * (0.75 + 0.25 * noise(q / 4.0, 13));
    let a = (ink_amt * pressure).max(e * 0.95).min(1.0);
    let tex = fbm(q / 1.4, 2, 61) - 0.5;
    let mut c = paper.lerp(graphite, a);
    // a hint of the materials' colour, as a light coloured pencil
    if g.hit {
        let m = scene.material(g.tri);
        let tint = desaturate(to_srgb(m.base), 0.2);
        c = c.lerp(c * tint.lerp(Vec3::ONE, 0.45), 0.6);
    }
    c * (1.0 + tex * 0.04)
}

/// The toon preset (see [`Look::Toon`]).
fn toon(scene: &Scene, sky: &Sky, g: &G, edge: f32) -> Vec3 {
    let bg = Vec3::new(0.90, 0.90, 0.92);
    let outline = Vec3::new(0.10, 0.11, 0.13);
    let mut c = if !g.hit {
        bg
    } else {
        let m = scene.material(g.tri);
        let base = to_srgb(m.base);
        // a toon matcap: the normal as seen from the eye, lit from the
        // upper left, in three flat bands
        let f = g.dir;
        let r = f.cross(Vec3::Z).normalize_or(Vec3::X);
        let u = r.cross(f);
        let nv = Vec3::new(g.n.dot(r), g.n.dot(u), -g.n.dot(f));
        let l = Vec3::new(-0.35, 0.55, 0.76).normalize();
        let t = nv.dot(l);
        let band = if t > 0.55 {
            1.0
        } else if t > 0.15 {
            0.86
        } else {
            0.72
        };
        let mut c = base * band;
        // cavity: corners and crevices darken
        c *= 1.0 - 0.38 * (1.0 - g.ao).powf(1.5);
        // faint diffuse shadows, intensity 0.3
        c *= 1.0 - 0.3 * (1.0 - g.sun);
        let _ = sky;
        c
    };
    if g.glass > 0 {
        let tint = Vec3::new(0.70, 0.80, 0.86);
        c = c.lerp(tint, 0.45);
    }
    c.lerp(outline, edge.min(1.0))
}

#[cfg(test)]
mod parallel_view_tests {
    use super::*;
    use crate::scene::Scene;
    use crate::sky::Sky;
    use creabim_geom::mesh::TriMesh;
    use creabim_geom::scene::SceneItem;
    use glam::{DVec3, Mat4};

    /// Ground like the owner's: a slope 40 m across, and the vertical face
    /// a crop box cut through it, facing the camera.
    fn ground() -> Scene {
        let mut m = TriMesh::default();
        let (lo, hi) = (-20.0, 20.0);
        let z = |x: f64| 0.08 * x;
        m.add_quad(
            [
                DVec3::new(lo, lo, z(lo)),
                DVec3::new(hi, lo, z(hi)),
                DVec3::new(hi, hi, z(hi)),
                DVec3::new(lo, hi, z(lo)),
            ],
            DVec3::new(-0.08, 0.0, 1.0).normalize(),
        );
        m.add_quad(
            [
                DVec3::new(lo, lo, -4.0),
                DVec3::new(hi, lo, -4.0),
                DVec3::new(hi, lo, z(hi)),
                DVec3::new(lo, lo, z(lo)),
            ],
            DVec3::new(0.0, -1.0, 0.0),
        );
        let item = SceneItem {
            id: creabim_core::ElementId::new(),
            mesh: m,
            color: [0.36, 0.49, 0.25],
            surface: [0.9, 0.0, 1.0],
            ghost: false,
            layer: None,
            finish: None,
        };
        Scene::from_items_on(&[item], None)
    }

    /// The 3D window's camera, as it hands it over: perspective (near
    /// plane 5 cm ahead of the eye) or parallel (2 km behind it).
    fn camera(parallel: bool) -> Camera {
        let (eye, target) = (Vec3::new(16.0, -24.0, 16.0), Vec3::new(0.0, -6.0, 0.0));
        let view = Mat4::look_at_rh(eye, target, Vec3::Z);
        let aspect = 1.5;
        let proj = if parallel {
            let h = 12.0;
            Mat4::orthographic_rh(-h * aspect, h * aspect, -h, h, -2000.0, 4000.0)
        } else {
            Mat4::perspective_rh(45f32.to_radians(), aspect, 0.05, 2000.0)
        };
        Camera::from_view_proj(proj * view, eye, target)
    }

    /// MU144: a parallel 3D view rendered Quick came out washed in the
    /// sky's colour, and every look streaked the ground: its rays started
    /// at the near plane 2 km behind the eye, so the haze took everything
    /// for far away and the distances lost the precision the shadow rays
    /// need. Seen through either camera, the ground is the same.
    #[test]
    fn a_parallel_view_renders_as_a_perspective_one() {
        let scene = ground();
        // a low sun from the front right: it lights both faces
        let sky = Sky::new(Vec3::new(0.5, -0.6, 0.35).normalize());
        let s = Settings {
            ss: 1,
            ..Settings::default()
        };
        let (w, h) = (96, 64);
        let mut seen = vec![];
        for parallel in [false, true] {
            let cam = camera(parallel);
            let mut t = Timings {
                primary_ms: 0.0,
                lighting_ms: 0.0,
                look_ms: 0.0,
                rays: 0,
            };
            let g = gbuffer(&scene, &cam, &sky, w, h, &s, &mut t);
            let hits: Vec<&G> = g.iter().filter(|g| g.hit).collect();
            assert!(
                hits.len() > w * h / 3,
                "the ground fills the view: {} of {} (parallel {parallel})",
                hits.len(),
                w * h
            );
            // nothing stands between the ground and the sun
            let shaded = hits.iter().filter(|g| g.sun < 0.999).count();
            assert_eq!(
                shaded,
                0,
                "{shaded} of {} samples in shade (parallel {parallel})",
                hits.len()
            );
            // nor does anything close over it: the ground is convex
            let closed = hits.iter().filter(|g| g.ao < 0.97).count();
            assert_eq!(
                closed,
                0,
                "{closed} of {} samples occluded (parallel {parallel})",
                hits.len()
            );
            let img = shade(
                &scene,
                &sky,
                &g,
                w,
                h,
                Look::Realtime { traced: false },
                &s,
                1.0,
            );
            let lit: Vec<Vec3> = g
                .iter()
                .zip(&img.px)
                .filter(|(g, _)| g.hit)
                .map(|(_, c)| *c)
                .collect();
            seen.push(lit.iter().fold(Vec3::ZERO, |a, c| a + *c) / lit.len() as f32);
        }
        let (persp, para) = (seen[0], seen[1]);
        assert!(
            (persp - para).length() < 0.05 * persp.length(),
            "the ground's colour: {persp:?} in perspective, {para:?} parallel"
        );
    }
}
