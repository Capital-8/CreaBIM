//! The model as the renderer sees it: triangles with one material each,
//! taken from the same scene items the 3D view draws, so every mode reads
//! the materials the project already has (colour, roughness, metal,
//! opacity) and nothing has to be set up twice.

use crate::bvh::Bvh;
use creabim_geom::scene::SceneItem;
use glam::Vec3;
use std::collections::HashMap;

/// What a surface is made of, in linear colour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Material {
    pub base: Vec3,
    pub rough: f32,
    pub metal: f32,
    /// See-through: glass, shown by its opacity below one.
    pub glass: bool,
    /// Leaves: lit through from behind a little.
    pub foliage: bool,
    /// Light given off, as radiance: a lamp's diffuser seen by the camera.
    pub emit: Vec3,
}

impl Material {
    pub fn plain(srgb: [f32; 3], rough: f32) -> Self {
        Material {
            base: srgb_to_linear(srgb),
            rough,
            metal: 0.0,
            glass: false,
            foliage: false,
            emit: Vec3::ZERO,
        }
    }
}

pub fn srgb_to_linear(c: [f32; 3]) -> Vec3 {
    let f = |x: f32| {
        if x <= 0.04045 {
            x / 12.92
        } else {
            ((x + 0.055) / 1.055).powf(2.4)
        }
    };
    Vec3::new(f(c[0]), f(c[1]), f(c[2]))
}

pub struct Scene {
    /// Corners of each triangle.
    pub pos: Vec<[Vec3; 3]>,
    /// Normals at the corners, for smooth shading.
    pub nrm: Vec<[Vec3; 3]>,
    pub mat: Vec<u32>,
    /// Which element each triangle belongs to (an index), for outlines.
    pub obj: Vec<u32>,
    pub materials: Vec<Material>,
    pub bvh: Bvh,
    pub lo: Vec3,
    pub hi: Vec3,
    /// Lamps: light fittings that give light, each aimed at directly.
    pub lamps: Vec<Lamp>,
    /// Each lamp's strongest intensity, as luminance at one metre: the
    /// most it can give anywhere, for passing it over from afar.
    pub lamp_peak: Vec<f32>,
}

/// How many groups light is kept in, for mixing after rendering: group 0
/// is the sun, 1 to 3 the lamps' groups, and the last the sky.
pub const GROUPS: usize = 5;
pub const SKY: usize = GROUPS - 1;

/// Light units: the renderer's sun gives 3.2 on a surface facing it, for
/// about 100 000 lux; this turns lux into the renderer's units.
pub const PER_LUX: f32 = 3.2 / 100_000.0;

/// A lamp: a small glowing disc with a photometric profile, as an IES file
/// gives it (candela by angle from its axis).
#[derive(Clone, Debug)]
pub struct Lamp {
    /// The centre of its light-giving face.
    pub pos: Vec3,
    /// Where its beam points (down for a downlight).
    pub axis: Vec3,
    /// The glowing face's radius, metres: bigger gives softer shadows.
    pub radius: f32,
    /// Candela per 1000 lumens, every 5 degrees from the axis (0 to 180),
    /// the way photometric files list them.
    pub candela: Vec<f32>,
    pub lumens: f32,
    /// Its light's colour, linear, luminance one.
    pub colour: Vec3,
    /// Which light group it belongs to, 1 to GROUPS - 1.
    pub group: usize,
}

impl Lamp {
    /// Intensity toward a direction (from the lamp), in renderer units at
    /// one metre.
    pub fn intensity(&self, d: Vec3) -> Vec3 {
        let a = self.axis.dot(d).clamp(-1.0, 1.0).acos().to_degrees() / 5.0;
        let i = (a.floor() as usize).min(self.candela.len() - 1);
        let j = (i + 1).min(self.candela.len() - 1);
        let t = a - i as f32;
        let cd = self.candela[i] * (1.0 - t) + self.candela[j] * t;
        self.colour * (cd * self.lumens / 1000.0 * PER_LUX)
    }

    /// Roughly how much it gives, for choosing which lamp to aim at.
    pub fn power(&self) -> f32 {
        self.lumens
    }
}

/// A photometric profile from a beam shape: `half` the half-angle of full
/// intensity, falling to nothing by `edge`, plus `spill` (0 to 1) of the
/// light sent evenly everywhere (a shade's glow). Scaled so the lamp
/// gives 1000 lumens.
pub fn profile(half: f32, edge: f32, spill: f32) -> Vec<f32> {
    let raw: Vec<f32> = (0..=36)
        .map(|k| {
            let a = k as f32 * 5.0;
            let beam = if a <= half {
                1.0
            } else if a >= edge {
                0.0
            } else {
                let t = (a - half) / (edge - half);
                1.0 - t * t * (3.0 - 2.0 * t)
            };
            beam * (1.0 - spill) + spill * 0.25
        })
        .collect();
    // lumens = integral of I over the sphere
    let mut flux = 0.0;
    for k in 0..36 {
        let (a0, a1) = (
            (k as f32 * 5.0).to_radians(),
            ((k + 1) as f32 * 5.0).to_radians(),
        );
        let i = (raw[k] + raw[k + 1]) * 0.5;
        flux += i * 2.0 * std::f32::consts::PI * (a0.cos() - a1.cos());
    }
    raw.iter().map(|c| c * 1000.0 / flux).collect()
}

/// Linear colour of light at a colour temperature, luminance one.
pub fn kelvin(k: f32) -> Vec3 {
    // Tanner Helland's fit, then to linear
    let t = k / 100.0;
    let r = if t <= 66.0 {
        255.0
    } else {
        329.7 * (t - 60.0).powf(-0.1332)
    };
    let g = if t <= 66.0 {
        99.47 * t.ln() - 161.12
    } else {
        288.12 * (t - 60.0).powf(-0.0755)
    };
    let b = if t >= 66.0 {
        255.0
    } else if t <= 19.0 {
        0.0
    } else {
        138.52 * (t - 10.0).ln() - 305.04
    };
    let c = srgb_to_linear([r, g, b].map(|v| (v / 255.0).clamp(0.0, 1.0)));
    c / crate::util::luminance(c).max(1e-4)
}

/// What to multiply every light by to balance the picture for light of
/// `k` kelvin (6500, daylight, changes nothing): the camera's white
/// balance. Its luminance is one, so it changes colour and not brightness.
///
/// It goes most of the way, not all of it ([`WHITE_PULL`]): balanced in
/// full, daylight under a lamplight balance was one solid blue with no
/// green left in the grass, and lamplight lost all its warmth. A camera's
/// own balance is as gentle.
pub fn white_gain(k: f32) -> Vec3 {
    let g = (kelvin(6500.0) / kelvin(k.clamp(1500.0, 15_000.0))).powf(WHITE_PULL);
    g / crate::util::luminance(g).max(1e-4)
}

/// How far a white balance goes towards making its own light white: 1 is
/// all the way.
pub const WHITE_PULL: f32 = 0.55;

impl Scene {
    /// Everything that shows in 3D, less cutters and drawing-only helpers.
    pub fn from_items(items: &[SceneItem]) -> Scene {
        Scene::from_items_on(items, None)
    }

    /// The same, standing on a wide flat ground at `ground` (height and
    /// colour) when the model has no terrain of its own to stand on.
    pub fn from_items_on(items: &[SceneItem], ground: Option<(f32, [f32; 3])>) -> Scene {
        let mut pos = vec![];
        let mut nrm = vec![];
        let mut mat = vec![];
        let mut obj = vec![];
        let mut materials: Vec<Material> = vec![];
        let mut objects: HashMap<_, u32> = HashMap::new();
        for item in items {
            if item.ghost {
                continue;
            }
            match item.finish {
                Some("Headroom")
                | Some(creabim_geom::scene::GROUND_LINES)
                | Some(creabim_geom::tree::DRAWN_FOLIAGE) => continue,
                _ => {}
            }
            let glass = item.surface[2] < 0.95 || item.finish == Some("Glass");
            let m = Material {
                base: srgb_to_linear(item.color),
                rough: item.surface[0].clamp(0.04, 1.0),
                metal: item.surface[1].clamp(0.0, 1.0),
                glass,
                foliage: item.finish == Some(creabim_geom::tree::FOLIAGE),
                emit: Vec3::ZERO,
            };
            let mi = match materials.iter().position(|x| *x == m) {
                Some(i) => i,
                None => {
                    materials.push(m);
                    materials.len() - 1
                }
            } as u32;
            let n = objects.len() as u32;
            let oi = *objects.entry(item.id).or_insert(n);
            let ms = &item.mesh;
            for c in ms.indices.chunks_exact(3) {
                let p = [0, 1, 2].map(|k| Vec3::from(ms.positions[c[k] as usize]));
                let area = (p[1] - p[0]).cross(p[2] - p[0]);
                if area.length_squared() < 1e-14 {
                    continue;
                }
                let face = area.normalize();
                let n = [0, 1, 2].map(|k| {
                    let v = ms
                        .normals
                        .get(c[k] as usize)
                        .map(|n| Vec3::from(*n))
                        .unwrap_or(face);
                    if v.length_squared() < 1e-6 {
                        face
                    } else {
                        v.normalize()
                    }
                });
                pos.push(p);
                nrm.push(n);
                mat.push(mi);
                obj.push(oi);
            }
        }
        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for t in &pos {
            for p in t {
                lo = lo.min(*p);
                hi = hi.max(*p);
            }
        }
        if let (Some((z, colour)), true) = (ground, lo.x <= hi.x) {
            // reaching well past the model, so the horizon is ground
            let c = (lo + hi) * 0.5;
            let r = (hi - lo).length().max(50.0) * 8.0;
            let q = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
                .map(|(x, y)| Vec3::new(c.x + x * r, c.y + y * r, z));
            materials.push(Material::plain(colour, 0.95));
            let mi = materials.len() as u32 - 1;
            let oi = objects.len() as u32;
            for t in [[q[0], q[1], q[2]], [q[0], q[2], q[3]]] {
                pos.push(t);
                nrm.push([Vec3::Z; 3]);
                mat.push(mi);
                obj.push(oi);
            }
        }
        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for t in &pos {
            for p in t {
                lo = lo.min(*p);
                hi = hi.max(*p);
            }
        }
        let bvh = Bvh::build(&pos);
        Scene {
            pos,
            nrm,
            mat,
            obj,
            materials,
            bvh,
            lo,
            hi,
            lamps: vec![],
            lamp_peak: vec![],
        }
    }

    /// Add lamps, each with a glowing disc the camera sees.
    /// Where a camera's ray is best traced from: just short of the box
    /// round the scene. With how far that point is ahead of the eye, which
    /// added to a distance from it gives the distance from the eye.
    ///
    /// A parallel 3D view's rays start at its near plane, 2 km behind the
    /// eye (so nothing near it is clipped). Traced from there, every
    /// surface was 2 km away: the real-time look's haze took it for far
    /// off (Quick came out washed in the sky's colour) and the distances
    /// lost the precision the shadow and occlusion rays need (streaks on
    /// the ground in every look). Nothing lies before the box, so starting
    /// there changes nothing that is seen (MU144).
    pub fn ray_start(&self, o: Vec3, d: Vec3, eye: Vec3) -> (Vec3, f32) {
        let skip = self.bvh.entry(o, d).map_or(0.0, |t| (t - 0.01).max(0.0));
        (o + d * skip, skip + (o - eye).dot(d))
    }

    pub fn add_lamps(&mut self, lamps: Vec<Lamp>) {
        for l in &lamps {
            let (t, b) = crate::util::basis(l.axis);
            let area = std::f32::consts::PI * l.radius * l.radius;
            // the disc's radiance: its intensity along the axis over its area
            let m = Material {
                base: Vec3::splat(0.8),
                rough: 1.0,
                metal: 0.0,
                glass: false,
                foliage: false,
                emit: l.intensity(l.axis) / area,
            };
            self.materials.push(m);
            let mi = self.materials.len() as u32 - 1;
            let oi = self.obj.iter().max().map_or(0, |o| o + 1);
            // just behind the point light is aimed at, so it never shadows it
            let c = l.pos - l.axis * 0.005;
            let n = 12;
            for k in 0..n {
                let a0 = k as f32 / n as f32 * std::f32::consts::TAU;
                let a1 = (k + 1) as f32 / n as f32 * std::f32::consts::TAU;
                let p0 = c + (t * a0.cos() + b * a0.sin()) * l.radius;
                let p1 = c + (t * a1.cos() + b * a1.sin()) * l.radius;
                self.pos.push([c, p1, p0]);
                self.nrm.push([l.axis; 3]);
                self.mat.push(mi);
                self.obj.push(oi);
            }
        }
        self.lamp_peak.extend(lamps.iter().map(|l| {
            let cd = l.candela.iter().copied().fold(0.0f32, f32::max);
            crate::util::luminance(l.colour) * cd * l.lumens / 1000.0 * PER_LUX
        }));
        self.lamps.extend(lamps);
        self.bvh = Bvh::build(&self.pos);
    }

    pub fn triangles(&self) -> usize {
        self.pos.len()
    }

    /// The shading normal at a hit, and the triangle's own.
    pub fn normals(&self, tri: u32, u: f32, v: f32) -> (Vec3, Vec3) {
        let p = &self.pos[tri as usize];
        let geo = (p[1] - p[0]).cross(p[2] - p[0]).normalize();
        let n = &self.nrm[tri as usize];
        let s = (n[0] * (1.0 - u - v) + n[1] * u + n[2] * v).normalize_or(geo);
        // keep smoothing from tipping a normal round the face it is on
        let s = if s.dot(geo) < 0.2 { geo } else { s };
        (s, geo)
    }

    pub fn material(&self, tri: u32) -> &Material {
        &self.materials[self.mat[tri as usize] as usize]
    }

    pub fn is_glass(&self, tri: u32) -> bool {
        self.material(tri).glass
    }
}
