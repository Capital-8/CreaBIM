//! Meshes and plan linework for extrusions, sweeps and voids, and for the
//! site: terrain, hard landscape areas and plants (see
//! `creabim_core::parts` and `creabim_core::site`).

use crate::plan::{Pen, PlanItem};
use crate::scene::{slab_mesh_with_holes, SceneItem, PLAIN_SURFACE};
use crate::triangulate::triangulate;
use crate::TriMesh;
use creabim_core::parts::{Form, Part};
use creabim_core::site::{
    GroundLineKind, GroundState, Pattern, PlantForm, Site, SiteForm, SiteView,
};
use creabim_core::{members, Element, ElementId, ElementKind, Model};
use glam::{DVec2, DVec3};
use std::f64::consts::{PI, TAU};

/// Default looks of elements without a material.
const PART_COLOR: [f32; 3] = [0.80, 0.76, 0.70];
const VOID_COLOR: [f32; 3] = [0.85, 0.45, 0.25];
const GROUND_COLOR: [f32; 3] = [0.50, 0.62, 0.38];
const AREA_COLOR: [f32; 3] = [0.74, 0.72, 0.68];
/// Painted lines on a car park, and its accessible bays' blue.
const MARKING_COLOR: [f32; 3] = [0.96, 0.96, 0.93];
const ACCESSIBLE_COLOR: [f32; 3] = [0.20, 0.42, 0.78];
/// A car park's raised walkway.
const WALKWAY_COLOR: [f32; 3] = [0.84, 0.83, 0.80];
const ROAD_COLOR: [f32; 3] = [0.42, 0.42, 0.44];
const ENVELOPE_COLOR: [f32; 3] = [0.45, 0.62, 0.85];
const LEAF_COLOR: [f32; 3] = [0.36, 0.58, 0.34];
/// A crown's clumps in shade and in the light, either side of its green.
const LEAF_DARK: [f32; 3] = [0.25, 0.44, 0.27];
const LEAF_LIGHT: [f32; 3] = [0.53, 0.72, 0.45];
/// A conifer's upper skirts.
const NEEDLE_LIGHT: [f32; 3] = [0.28, 0.48, 0.31];
const NEEDLE_COLOR: [f32; 3] = [0.22, 0.40, 0.26];
const TRUNK_COLOR: [f32; 3] = [0.45, 0.33, 0.22];

// ---- parts ----

/// A part's solid, as built.
pub fn part_mesh(model: &Model, p: &Part) -> Option<TriMesh> {
    match &p.form {
        Form::Extrusion {
            outline,
            holes,
            base,
            height,
            plane: Some(pl),
        } => {
            // built flat on the plane, then laid on it: the plane's axes are
            // right-handed, so the faces keep facing out
            let mut m = slab_mesh_with_holes(outline, holes, base + height, *height);
            let lift = DVec3::Z * members::elevation(model, p.level);
            let n = pl.normal();
            for v in &mut m.positions {
                let q = creabim_core::parts::on_plane(
                    pl,
                    DVec2::new(v[0] as f64, v[1] as f64),
                    v[2] as f64,
                );
                *v = (q + lift).as_vec3().to_array();
            }
            for v in &mut m.normals {
                let q = pl.x * v[0] as f64 + pl.y * v[1] as f64 + n * v[2] as f64;
                *v = q.as_vec3().to_array();
            }
            Some(m)
        }
        Form::Extrusion {
            outline,
            holes,
            base,
            height,
            plane: None,
        } => {
            let z = members::elevation(model, p.level) + base;
            Some(slab_mesh_with_holes(outline, holes, z + height, *height))
        }
        Form::Sweep { profile, .. } => {
            let (path, closed) = p.path(model)?;
            let m = match p.face_normal() {
                Some(n) => sweep_mesh_on(profile, &path, closed, n),
                None => sweep_mesh(profile, &path, closed),
            };
            (!m.is_empty()).then_some(m)
        }
    }
}

/// [`sweep_mesh`] with `up` (a face's outward normal) for the profile's
/// `y` instead of the vertical: the path is laid flat in a frame where
/// `up` is vertical, swept there and stood back up.
pub fn sweep_mesh_on(profile: &[DVec2], path: &[DVec3], closed: bool, up: DVec3) -> TriMesh {
    let up = up.normalize_or(DVec3::Z);
    let ex = if up.z.abs() > 0.999 {
        DVec3::X * up.z.signum()
    } else {
        DVec3::Z.cross(up).normalize()
    };
    let ey = up.cross(ex);
    let flat: Vec<DVec3> = path
        .iter()
        .map(|p| DVec3::new(p.dot(ex), p.dot(ey), p.dot(up)))
        .collect();
    let mut m = sweep_mesh(profile, &flat, closed);
    let back = |v: &mut [f32; 3]| {
        let q = ex * v[0] as f64 + ey * v[1] as f64 + up * v[2] as f64;
        *v = q.as_vec3().to_array();
    };
    m.positions.iter_mut().for_each(back);
    m.normals.iter_mut().for_each(back);
    m
}

/// A closed solid: `profile` (counter-clockwise, `x` to the path's left,
/// `y` up) carried along `path`, mitred at its corners and capped at its
/// ends unless it is `closed`.
pub fn sweep_mesh(profile: &[DVec2], path: &[DVec3], closed: bool) -> TriMesh {
    let mut m = TriMesh::default();
    // a profile drawn clockwise is turned round: its faces would face in,
    // and the solid show its inside, hollow (MU40)
    let turned: Vec<DVec2>;
    let profile = if signed_area(profile) < 0.0 {
        turned = profile.iter().rev().copied().collect();
        &turned[..]
    } else {
        profile
    };
    // drop repeated points, which have no direction
    let mut pts: Vec<DVec3> = vec![];
    for p in path {
        if pts.last().is_none_or(|q: &DVec3| q.distance(*p) > 1e-6) {
            pts.push(*p);
        }
    }
    if closed && pts.len() > 2 && pts[0].distance(pts[pts.len() - 1]) < 1e-6 {
        pts.pop();
    }
    let n = pts.len();
    if n < 2 || profile.len() < 3 {
        return m;
    }
    let segs = if closed { n } else { n - 1 };
    // each segment's direction in plan, and its left
    let dir: Vec<DVec2> = (0..segs)
        .map(|i| (pts[(i + 1) % n] - pts[i]).truncate().normalize_or_zero())
        .collect();
    if dir.iter().all(|d| *d == DVec2::ZERO) {
        return m;
    }
    let left = |d: DVec2| DVec2::new(-d.y, d.x);
    // the across-axis at each point, stretched to keep the profile's width
    // square to both segments at a mitre
    let across: Vec<DVec2> = (0..n)
        .map(|i| {
            let before = if i > 0 {
                Some(dir[i - 1])
            } else if closed {
                Some(dir[segs - 1])
            } else {
                None
            };
            let after = (i < segs).then(|| dir[i]);
            match (before, after) {
                (Some(a), Some(b)) => {
                    let (la, lb) = (left(a), left(b));
                    let mitre = (la + lb).normalize_or(lb);
                    let k = mitre.dot(lb).max(0.25);
                    mitre / k
                }
                (Some(a), None) => left(a),
                (None, Some(b)) => left(b),
                (None, None) => DVec2::X,
            }
        })
        .collect();
    let ring = |i: usize| -> Vec<DVec3> {
        profile
            .iter()
            .map(|q| pts[i] + (across[i] * q.x).extend(q.y))
            .collect()
    };
    let rings: Vec<Vec<DVec3>> = (0..n).map(ring).collect();
    let k = profile.len();
    for s in 0..segs {
        let (a, b) = (&rings[s], &rings[(s + 1) % n]);
        let f = (pts[(s + 1) % n] - pts[s]).normalize_or_zero();
        for j in 0..k {
            let j1 = (j + 1) % k;
            let e = profile[j1] - profile[j];
            // outward: the profile edge's right, in the path's frame
            let l = left(dir[s]).extend(0.0);
            let normal = l * e.y - DVec3::Z * e.x;
            let normal = if normal.length_squared() < 1e-18 {
                f
            } else {
                normal
            };
            m.add_quad([a[j], a[j1], b[j1], b[j]], normal);
        }
    }
    if !closed {
        let tris = triangulate(profile);
        let f0 = (pts[1] - pts[0]).normalize_or_zero();
        let f1 = (pts[n - 1] - pts[n - 2]).normalize_or_zero();
        m.add_polygon(&rings[0], &tris, -f0);
        m.add_polygon(&rings[n - 1], &tris, f1);
    }
    m
}

/// The shoelace area of `p`, positive counter-clockwise.
fn signed_area(p: &[DVec2]) -> f64 {
    let n = p.len();
    (0..n).map(|i| p[i].perp_dot(p[(i + 1) % n])).sum::<f64>() / 2.0
}

// ---- site ----

/// The triangles of a terrain's top surface as drawn, in model
/// coordinates (see [`crate::ground`]).
pub fn terrain_top(model: &Model, s: &Site) -> Vec<[DVec3; 3]> {
    let z = members::elevation(model, s.level);
    crate::earth::shaped(model, s).map_or(vec![], |g| g.ground.triangles(z))
}

/// The ground a shaped terrain is drawn and built from: its proposed
/// ground. None for a flat terrain nothing is worked into, which is a
/// plain slab of earth.
fn shaped_ground(model: &Model, s: &Site) -> Option<std::sync::Arc<crate::ground::Ground>> {
    let g = crate::earth::shaped(model, s)?;
    (creabim_core::site::surveyed(model, s) || g.changed()).then(|| g.ground.clone())
}

/// A terrain read as its plan shows it: its shading's bands with their
/// areas, and how many low points water gathers in (when its flow is
/// shown). None when it is not read or is a flat slab.
pub fn ground_reading(
    model: &Model,
    s: &Site,
    sv: &SiteView,
) -> Option<(crate::analysis::Shading, usize)> {
    let SiteForm::Terrain {
        boundary,
        contour,
        data,
        ..
    } = &s.form
    else {
        return None;
    };
    let look = sv.look_on(*contour, data);
    if look.shade.is_none() && !look.flow {
        return None;
    }
    let g = shaped_ground(model, s)?;
    let north = model.project_info().map_or(0.0, |(_, p)| p.true_north);
    let base = members::elevation(model, s.level);
    let sh = crate::analysis::shade(&g, look.shade, north, base, look.interval);
    let lows = if look.flow {
        let spacing = crate::analysis::flow_spacing(boundary);
        crate::analysis::flow(&g, boundary, spacing).1.len()
    } else {
        0
    };
    Some((sh, lows))
}

/// A terrain's solid: its top surface, sides down to a flat underside
/// `thickness` below its lowest point. A flat one leaves room for the
/// hard landscape lying in it.
fn terrain_mesh(model: &Model, s: &Site) -> TriMesh {
    let SiteForm::Terrain {
        boundary,
        top,
        thickness,
        ..
    } = &s.form
    else {
        return TriMesh::default();
    };
    let z = members::elevation(model, s.level);
    let Some(g) = shaped_ground(model, s) else {
        let holes = creabim_core::site::ground_holes(model, s);
        return slab_mesh_with_holes(boundary, &holes, z + top, *thickness);
    };
    ground_solid(&g, z, *thickness)
}

/// A terrain's solid as it was surveyed, before its works: none when the
/// works change nothing or it is a flat slab.
pub fn terrain_mesh_before(model: &Model, s: &Site) -> Option<TriMesh> {
    let SiteForm::Terrain { thickness, .. } = &s.form else {
        return None;
    };
    let sh = crate::earth::shaped(model, s)?;
    sh.changed()
        .then(|| ground_solid(&sh.existing, members::elevation(model, s.level), *thickness))
}

/// The surveyed ground's surface where a terrain's works moved it, for
/// showing it see-through over the ground as built. None where nothing
/// moved.
pub fn ground_moved_mesh(model: &Model, s: &Site) -> Option<TriMesh> {
    let sh = crate::earth::shaped(model, s)?;
    if !sh.changed() {
        return None;
    }
    let up = DVec3::Z * members::elevation(model, s.level);
    let g = &sh.existing;
    let mut m = TriMesh::default();
    for t in &g.fine.tris {
        let p = t.map(|i| g.fine.verts[i as usize]);
        let moved = p.iter().any(|q| {
            sh.ground
                .height(q.truncate())
                .is_some_and(|z| (z - q.z).abs() > 0.02)
        });
        if !moved {
            continue;
        }
        let p = p.map(|q| q + up);
        let n = (p[1] - p[0]).cross(p[2] - p[0]).normalize_or(DVec3::Z);
        m.add_polygon(&p, &[[0, 1, 2]], n);
    }
    (!m.is_empty()).then_some(m)
}

/// A ground's solid: its surface `z` up, down to `thickness` under its
/// lowest point.
fn ground_solid(g: &crate::ground::Ground, z: f64, thickness: f64) -> TriMesh {
    let up = DVec3::Z * z;
    let mut m = TriMesh::default();
    // the top: rounded where the ground is smooth, each triangle its own
    // face along a break line and on a faceted ground
    let normals = g.normals();
    for t in &g.fine.tris {
        let p = t.map(|i| g.fine.verts[i as usize] + up);
        let flat = (p[1] - p[0]).cross(p[2] - p[0]).normalize_or(DVec3::Z);
        let base = m.positions.len() as u32;
        for (i, q) in t.iter().zip(p) {
            m.positions.push(q.as_vec3().to_array());
            m.normals
                .push(normals[*i as usize].unwrap_or(flat).as_vec3().to_array());
        }
        m.indices.extend([base, base + 1, base + 2]);
    }
    let low = z + g.range().0 - thickness;
    // the underside: the surface's own triangles laid flat, so its edge
    // meets the sides corner for corner
    for t in &g.fine.tris {
        let p = t.map(|i| g.fine.verts[i as usize].truncate().extend(low));
        m.add_polygon(&[p[0], p[2], p[1]], &[[0, 1, 2]], -DVec3::Z);
    }
    // the sides, under the surface's own edge so they meet it exactly
    for (a, b) in g.rim() {
        let (a, b) = (a + up, b + up);
        let e = (b - a).truncate().normalize_or_zero();
        let out = DVec3::new(e.y, -e.x, 0.0);
        m.add_polygon(
            &[a.truncate().extend(low), b.truncate().extend(low), b, a],
            &[[0, 1, 2], [0, 2, 3]],
            out,
        );
    }
    m
}

/// The parts of a plant: (mesh, default colour, is it the crown).
fn plant_meshes(model: &Model, s: &Site) -> Vec<(TriMesh, [f32; 3], bool)> {
    let SiteForm::Plant {
        position,
        base,
        form,
        canopy,
        height,
        ..
    } = &s.form
    else {
        return vec![];
    };
    let _ = base;
    let z0 = crate::earth::plant_z(model, s);
    if let Some(shape) = s.tree_shape() {
        // grown once and kept; here only moved to where it stands
        let g = crate::tree::grow(shape, *canopy, *height);
        let at = position.extend(z0).as_vec3();
        let moved = |m: &TriMesh| {
            let mut m = m.clone();
            for p in &mut m.positions {
                *p = (glam::Vec3::from(*p) + at).to_array();
            }
            m
        };
        // in leaf, or not, in its month; south of the equator its year
        // runs six months on
        let southern = model
            .project_info()
            .and_then(|(_, p)| p.latitude)
            .is_some_and(|l| l < 0.0);
        // the project's month, or the tree's own
        let project = model.project_info().map_or(7, |(_, p)| p.season());
        let mut shape = shape.clone();
        shape.month = shape.month_in(project);
        let shape = &shape;
        let leaf = shape.leaf_state(southern).leaf;
        // its twigs show once its crown thins
        let wood = if leaf < 0.75 && !g.twigs.is_empty() {
            let mut w = g.wood.clone();
            w.append(&g.twigs);
            w
        } else {
            g.wood.clone()
        };
        return vec![
            (moved(&wood), TRUNK_COLOR, false),
            (
                moved(&g.foliage_at(leaf)),
                shape.foliage_colour(southern),
                true,
            ),
        ];
    }
    let (r, h) = (canopy / 2.0, *height);
    let at = |dz: f64| position.extend(z0 + dz);
    // a turn of its own from where it stands, so a row is not stamped
    let turn = plant_turn(*position);
    let round =
        |a: f64, out: f64, dz: f64| at(dz) + (DVec2::from_angle(turn + a) * out).extend(0.0);
    let mut parts: Vec<(TriMesh, [f32; 3], bool)> = vec![];
    let mut tones = [TriMesh::default(), TriMesh::default(), TriMesh::default()];
    match form {
        // a crown of clumps, dark below and light on top, over a
        // tapered trunk with two limbs running up into it
        PlantForm::Tree => {
            let stem = (h * 0.4).min(h - 0.3).max(h * 0.2);
            let tr = (r * 0.08).clamp(0.06, 0.35);
            let (cz, ch) = (stem + (h - stem) / 2.0, (h - stem) / 2.0);
            let mut wood = limb(at(0.0), at(stem + ch * 0.5), tr, tr * 0.6);
            for (a, out) in [(0.4, 0.45), (3.3, 0.4)] {
                wood.append(&limb(
                    at(stem * 0.72),
                    round(a, r * out, cz - ch * 0.35),
                    tr * 0.55,
                    tr * 0.3,
                ));
            }
            parts.push((wood, TRUNK_COLOR, false));
            for k in 0..5 {
                let a = TAU * k as f64 / 5.0;
                tones[0].append(&clump(
                    round(a, r * 0.52, cz - ch * 0.28),
                    DVec3::new(r * 0.48, r * 0.48, ch * 0.58),
                ));
            }
            tones[1].append(&clump(
                at(cz + ch * 0.02),
                DVec3::new(r * 0.72, r * 0.72, ch * 0.8),
            ));
            for k in 0..4 {
                let a = 0.6 + TAU * k as f64 / 4.0;
                tones[1].append(&clump(
                    round(a, r * 0.5, cz + ch * 0.22),
                    DVec3::new(r * 0.44, r * 0.44, ch * 0.5),
                ));
            }
            for a in [1.1, 3.2, 5.3] {
                tones[2].append(&clump(
                    round(a, r * 0.26, cz + ch * 0.55),
                    DVec3::new(r * 0.42, r * 0.42, ch * 0.44),
                ));
            }
        }
        // four skirts, each wider than the one above, on a trunk that
        // runs up into the lowest
        PlantForm::Conifer => {
            let stem = h * 0.14;
            let body = h - stem;
            parts.push((
                limb(
                    at(0.0),
                    at(stem + body * 0.3),
                    (r * 0.09).clamp(0.05, 0.3),
                    (r * 0.05).clamp(0.03, 0.2),
                ),
                TRUNK_COLOR,
                false,
            ));
            for (k, (base, tall, wide)) in [
                (0.0, 0.42, 1.0),
                (0.22, 0.40, 0.8),
                (0.44, 0.38, 0.6),
                (0.64, 0.36, 0.4),
            ]
            .into_iter()
            .enumerate()
            {
                tones[if k < 2 { 0 } else { 1 }].append(&cone(
                    at(stem + body * base),
                    r * wide,
                    body * tall,
                ));
            }
        }
        // feathered fronds arching up, out and down from the head of a
        // straight trunk with a flared foot
        PlantForm::Palm => {
            let head = (h - r * 0.85).max(h * 0.5);
            let tr = (r * 0.06).clamp(0.09, 0.22);
            let mut wood = limb(at(0.0), at(head * 0.12), tr * 1.7, tr);
            wood.append(&limb(at(head * 0.12), at(head), tr, tr * 0.8));
            parts.push((wood, TRUNK_COLOR, false));
            // (how many, its curve's control and tip as (out, up) of its
            // reach, which tone)
            #[allow(clippy::type_complexity)]
            let rings: [(usize, (f64, f64), (f64, f64), usize); 4] = [
                (3, (0.42, 0.0), (0.5, -0.66), 0),
                (4, (0.65, 0.27), (0.9, -0.42), 0),
                (4, (0.5, 0.62), (1.0, 0.24), 1),
                (3, (0.14, 0.68), (0.45, 0.85), 2),
            ];
            for (ring, (count, ctrl, tip, tone)) in rings.into_iter().enumerate() {
                for i in 0..count {
                    let a = turn + TAU * i as f64 / count as f64 + ring as f64 * 0.7;
                    let d = DVec2::from_angle(a);
                    let p = |q: (f64, f64)| at(head) + (d * r * q.0).extend(r * q.1);
                    tones[tone].append(&frond(at(head), p(ctrl), p(tip), r * 0.2));
                }
            }
        }
        // a low cluster of clumps that sits on the ground all round
        PlantForm::Shrub => {
            for a in [0.0, 2.1, 4.2] {
                tones[0].append(&clump(
                    round(a, r * 0.38, h * 0.3),
                    DVec3::new(r * 0.6, r * 0.6, h * 0.34),
                ));
            }
            tones[1].append(&clump(at(h * 0.5), DVec3::new(r * 0.66, r * 0.66, h * 0.5)));
            for a in [1.0, 3.6] {
                tones[2].append(&clump(
                    round(a, r * 0.28, h * 0.72),
                    DVec3::new(r * 0.36, r * 0.36, h * 0.28),
                ));
            }
        }
    }
    let greens = if *form == PlantForm::Conifer {
        [NEEDLE_COLOR, NEEDLE_LIGHT, NEEDLE_LIGHT]
    } else {
        [LEAF_DARK, LEAF_COLOR, LEAF_LIGHT]
    };
    for (m, c) in tones.into_iter().zip(greens) {
        if !m.is_empty() {
            parts.push((m, c, true));
        }
    }
    parts
}

/// A turn of a plant's own, from where it stands.
fn plant_turn(p: DVec2) -> f64 {
    let v = (p.x * 12.9898 + p.y * 78.233).sin() * 43758.5453;
    (v - v.floor()) * TAU
}

/// A clump of foliage: a coarse faceted ellipsoid.
fn clump(c: DVec3, r: DVec3) -> TriMesh {
    ellipsoid_of(c, r, 6, 12)
}

/// A tapered round limb from `a` (radius `ra`) to `b` (radius `rb`).
fn limb(a: DVec3, b: DVec3, ra: f64, rb: f64) -> TriMesh {
    let axis = (b - a).normalize_or_zero();
    let u = if axis.z.abs() < 0.9 {
        axis.cross(DVec3::Z)
    } else {
        axis.cross(DVec3::X)
    }
    .normalize_or_zero();
    let v = axis.cross(u);
    let n = 8;
    let ring = |c: DVec3, r: f64, i: usize| {
        let t = TAU * i as f64 / n as f64;
        c + (u * t.cos() + v * t.sin()) * r
    };
    let mut m = TriMesh::default();
    for i in 0..n {
        let q = [
            ring(a, ra, i),
            ring(a, ra, i + 1),
            ring(b, rb, i + 1),
            ring(b, rb, i),
        ];
        let out = (q[0] + q[1] + q[2] + q[3]) / 4.0 - (a + b) / 2.0;
        let out = out - axis * out.dot(axis);
        // wound so its face looks outward
        if (q[1] - q[0]).cross(q[2] - q[0]).dot(out) >= 0.0 {
            m.add_quad(q, out);
        } else {
            m.add_quad([q[1], q[0], q[3], q[2]], out);
        }
    }
    for (c, r, dir) in [(a, ra, -axis), (b, rb, axis)] {
        let cap: Vec<DVec3> = (0..n).map(|i| ring(c, r, i)).collect();
        let mut cap = cap;
        if (cap[1] - cap[0]).cross(cap[2] - cap[0]).dot(dir) < 0.0 {
            cap.reverse();
        }
        let tris: Vec<[usize; 3]> = (1..n - 1).map(|i| [0, i, i + 1]).collect();
        m.add_polygon(&cap, &tris, dir);
    }
    m
}

fn circle(c: DVec2, r: f64, n: usize) -> Vec<DVec2> {
    (0..n)
        .map(|i| c + DVec2::from_angle(TAU * i as f64 / n as f64) * r)
        .collect()
}

/// [`ellipsoid`] of `rings` bands and `segs` facets round.
fn ellipsoid_of(c: DVec3, r: DVec3, rings: usize, segs: usize) -> TriMesh {
    let p = |i: usize, j: usize| {
        let lat = -PI / 2.0 + PI * i as f64 / rings as f64;
        let lon = TAU * j as f64 / segs as f64;
        c + DVec3::new(lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()) * r
    };
    let mut m = TriMesh::default();
    for i in 0..rings {
        for j in 0..segs {
            let q = [p(i, j), p(i, j + 1), p(i + 1, j + 1), p(i + 1, j)];
            let mid = (q[0] + q[1] + q[2] + q[3]) / 4.0;
            let normal = ((mid - c) / (r * r)).normalize_or_zero();
            if i == 0 {
                m.add_polygon(&[q[0], q[2], q[3]], &[[0, 1, 2]], normal);
            } else if i == rings - 1 {
                m.add_polygon(&[q[0], q[1], q[2]], &[[0, 1, 2]], normal);
            } else {
                m.add_quad(q, normal);
            }
        }
    }
    m
}

/// An upright cone, its base centred on `c`.
fn cone(c: DVec3, r: f64, h: f64) -> TriMesh {
    let n = 14;
    let ring: Vec<DVec3> = circle(c.truncate(), r, n)
        .into_iter()
        .map(|p| p.extend(c.z))
        .collect();
    let apex = c + DVec3::Z * h;
    let mut m = TriMesh::default();
    for i in 0..n {
        let (a, b) = (ring[i], ring[(i + 1) % n]);
        let out = ((a + b) / 2.0 - c).truncate().extend(r / h.max(1e-6) * r);
        m.add_polygon(&[a, b, apex], &[[0, 1, 2]], out);
    }
    let tris: Vec<[usize; 3]> = (1..n - 1).map(|i| [0, i, i + 1]).collect();
    m.add_polygon(&ring, &tris, -DVec3::Z);
    m
}

/// A feathered palm frond along the curve from `p0` by `p1` (its control)
/// to `p2`, `width` across at its widest: its leaflets hang from its rib
/// either side, so it reads as a feather from the side and from above.
fn frond(p0: DVec3, p1: DVec3, p2: DVec3, width: f64) -> TriMesh {
    let steps = 10;
    // each cut across it: its rib, and the foot of the leaflets each side
    let mut cuts: Vec<[DVec3; 3]> = vec![];
    for i in 0..=steps {
        let t = i as f64 / steps as f64;
        let p = p0 * (1.0 - t) * (1.0 - t) + p1 * 2.0 * (1.0 - t) * t + p2 * t * t;
        let tan = ((p1 - p0) * (1.0 - t) + (p2 - p1) * t).normalize_or_zero();
        let flat = tan.truncate().normalize_or_zero();
        let flat = if flat == DVec2::ZERO { DVec2::X } else { flat };
        let side = DVec3::new(-flat.y, flat.x, 0.0);
        // widest a third of the way out, toothed along its edge
        let mut w =
            width * ((PI * (t * 1.15).min(1.0)).sin().max(0.0)).powf(0.7) * (1.0 - 0.35 * t);
        if i % 2 == 1 {
            w *= 0.6;
        }
        let w = w.max(0.004);
        let hang = DVec3::Z * w * 0.85;
        cuts.push([p, p + side * w * 0.6 - hang, p - side * w * 0.6 - hang]);
    }
    let mut m = TriMesh::default();
    for i in 0..steps {
        let (a, b) = (cuts[i], cuts[i + 1]);
        let core = (a[0] + a[1] + a[2] + b[0] + b[1] + b[2]) / 6.0;
        for (j, k) in [(0, 1), (2, 0), (1, 2)] {
            let q = [a[j], a[k], b[k], b[j]];
            let out = (q[0] + q[1] + q[2] + q[3]) / 4.0 - core;
            if (q[1] - q[0]).cross(q[2] - q[0]).dot(out) >= 0.0 {
                m.add_quad(q, out);
            } else {
                m.add_quad([q[1], q[0], q[3], q[2]], out);
            }
        }
    }
    m
}

/// A car park's painted markings at height `z`: the bays' lines (their
/// sides and back, the aisle side open), and the accessible bays' zones
/// hatched and their outlines.
fn car_park_marks(lay: &creabim_core::carpark::Layout, z: f64) -> (TriMesh, TriMesh) {
    use creabim_core::carpark::BayKind;
    let (mut white, mut blue) = (TriMesh::default(), TriMesh::default());
    let ribbon = |m: &mut TriMesh, a: DVec2, b: DVec2, half: f64| {
        let len = a.distance(b);
        if len < 1e-6 {
            return;
        }
        let side = (b - a).perp() / len * half;
        let quad = [
            (a - side).extend(z),
            (b - side).extend(z),
            (b + side).extend(z),
            (a + side).extend(z),
        ];
        m.add_polygon(&quad, &[[0, 1, 2], [0, 2, 3]], DVec3::Z);
    };
    for b in &lay.bays {
        let m = if b.kind == BayKind::Accessible {
            &mut blue
        } else {
            &mut white
        };
        for k in 1..4 {
            ribbon(m, b.corners[k], b.corners[(k + 1) % 4], 0.05);
        }
    }
    for zc in &lay.zones {
        for (a, b) in zone_hatch(zc) {
            ribbon(&mut blue, a, b, 0.05);
        }
        for k in 0..4 {
            ribbon(&mut blue, zc[k], zc[(k + 1) % 4], 0.04);
        }
    }
    (white, blue)
}

/// The slanted lines hatching an accessible bay's access zone.
fn zone_hatch(c: &[DVec2; 4]) -> Vec<(DVec2, DVec2)> {
    // c[0]-c[1] is its open end; c[1]-c[2] runs its length
    let (across, along) = (c[0] - c[1], c[2] - c[1]);
    let (w, l) = (across.length(), along.length());
    if w < 1e-6 || l < 1e-6 {
        return vec![];
    }
    let dir = along / l;
    let mut out = vec![];
    let mut d = 0.3;
    while d + w <= l + 1e-6 {
        out.push((c[1] + dir * d, c[0] + dir * (d + w)));
        d += 0.6;
    }
    out
}

/// The solids of a site element, each with its default colour and
/// whether the element's material covers it (a plant's trunk keeps its own).
pub fn site_meshes(model: &Model, s: &Site) -> Vec<(TriMesh, [f32; 3], bool)> {
    match &s.form {
        SiteForm::Terrain { .. } => vec![(terrain_mesh(model, s), GROUND_COLOR, true)],
        SiteForm::Area {
            boundary,
            holes,
            top,
            thickness,
            ..
        } => {
            let z = members::elevation(model, s.level) + top;
            let mut out = vec![(
                slab_mesh_with_holes(boundary, holes, z, *thickness),
                AREA_COLOR,
                true,
            )];
            // a car park's markings, painted on it
            if let Some(lay) = s.car_park_layout(model) {
                let (white, blue) = car_park_marks(&lay, z + 0.015);
                if !white.indices.is_empty() {
                    out.push((white, MARKING_COLOR, false));
                }
                if !blue.indices.is_empty() {
                    out.push((blue, ACCESSIBLE_COLOR, false));
                }
                // its edge strips: a walkway or planting, a kerb's height
                // proud; a clearance is level with it
                for (_, ring, what) in &lay.strips {
                    use creabim_core::carpark::StripUse;
                    let ccw = |mut r: Vec<DVec2>| {
                        if creabim_core::polygon::signed_area2(&r) < 0.0 {
                            r.reverse();
                        }
                        r
                    };
                    match what {
                        StripUse::Walkway => out.push((
                            slab_mesh_with_holes(&ccw(ring.clone()), &[], z + 0.125, 0.125),
                            WALKWAY_COLOR,
                            false,
                        )),
                        // a planted bed in the ground's own green, behind a
                        // kerb along the car park's side (owner: a flat
                        // dark green band read as a fault)
                        StripUse::Planting => {
                            let (a, b, bi, ai) = (ring[0], ring[1], ring[2], ring[3]);
                            let deep = ai.distance(a).max(1e-6);
                            let t = (0.125 / deep).min(0.5);
                            let (ak, bk) = (ai.lerp(a, t), bi.lerp(b, t));
                            out.push((
                                slab_mesh_with_holes(&ccw(vec![a, b, bk, ak]), &[], z + 0.1, 0.1),
                                GROUND_COLOR,
                                false,
                            ));
                            out.push((
                                slab_mesh_with_holes(
                                    &ccw(vec![ak, bk, bi, ai]),
                                    &[],
                                    z + 0.125,
                                    0.125,
                                ),
                                WALKWAY_COLOR,
                                false,
                            ));
                        }
                        StripUse::Clear => {}
                    }
                }
                // its islands: kerbed and planted, a kerb's height proud
                for c in &lay.islands {
                    let mut ring = c.to_vec();
                    if creabim_core::polygon::signed_area2(&ring) < 0.0 {
                        ring.reverse();
                    }
                    out.push((
                        slab_mesh_with_holes(&ring, &[], z + 0.125, 0.125),
                        GROUND_COLOR,
                        false,
                    ));
                }
            }
            out
        }
        // an earthwork is the ground's shape, drawn by its terrain; only
        // a pad's base course is a thing of its own
        SiteForm::Pad {
            boundary,
            top,
            base,
            depth,
            ..
        } => {
            if *depth > 0.0 {
                return vec![];
            }
            let z = members::elevation(model, s.level) + top;
            if *base < 0.005 {
                // no base course: a thin skin all the same, so the pad
                // reads as a made platform and not as more grass
                return vec![(
                    crate::scene::slab_mesh(boundary, z + PAD_SKIN, PAD_SKIN),
                    PAD_COLOR,
                    true,
                )];
            }
            vec![(crate::scene::slab_mesh(boundary, z, *base), PAD_COLOR, true)]
        }
        SiteForm::Plant { .. } => plant_meshes(model, s),
        // a lot is lines on the ground; its envelope is drawn apart
        SiteForm::Property { .. } => vec![],
        SiteForm::Road(r) => {
            let base = members::elevation(model, s.level);
            let mut out = vec![(crate::road::mesh(r, base), ROAD_COLOR, true)];
            if let Some(g) = crate::services::gully_meshes(model, s, r) {
                out.push((g, crate::services::GULLY_COLOR, false));
            }
            out
        }
        SiteForm::Drain(d) => crate::services::drain_meshes(model, s, d),
        SiteForm::Tank(t) => crate::services::tank_meshes(model, s, t),
        SiteForm::Walk(w) => crate::walk::walk_meshes(model, s, w),
        SiteForm::Investigation(p) => crate::investigation::probe_meshes(model, s, p),
        // ground data: the terrain it lies on shows what it does
        SiteForm::Spot { .. } | SiteForm::Line(_) => vec![],
    }
}

/// A bank's ticks in plan: from its top down the slope, long and short in
/// turn, one every [`TICK_STEP`] along the top, so they keep their spacing
/// round a corner (fanning about a hip) whatever the bank was sampled at.
fn bank_ticks(out: &mut Vec<PlanItem>, id: ElementId, bank: &crate::earth::Bank) {
    let n = bank.len();
    // each sample's top and foot: a dug bank's top is where it meets the
    // ground, a made-up one's the pad's edge
    let tops: Vec<(DVec2, DVec2, bool)> = bank
        .iter()
        .map(|(a, d)| {
            let (top, foot) = if d.z > a.z { (d, a) } else { (a, d) };
            (
                top.truncate(),
                foot.truncate(),
                a.truncate().distance(d.truncate()) > 0.05,
            )
        })
        .collect();
    let mut k = 0usize;
    let mut carry = 0.0;
    for i in 0..n {
        let (t0, f0, w0) = tops[i];
        let (t1, f1, w1) = tops[(i + 1) % n];
        if !(w0 && w1) {
            carry = 0.0;
            continue;
        }
        let seg = t0.distance(t1);
        let mut s = carry;
        while s <= seg {
            let u = if seg > 1e-9 { s / seg } else { 0.0 };
            let (t, f) = (t0.lerp(t1, u), f0.lerp(f1, u));
            let reach = if k % 2 == 0 { 0.85 } else { 0.42 };
            out.push(PlanItem::Line {
                id,
                pen: Pen::Contour,
                a: t,
                b: t.lerp(f, reach),
            });
            k += 1;
            s += TICK_STEP;
        }
        carry = s - seg;
    }
}

/// How far apart a bank's ticks are along its top, in metres.
const TICK_STEP: f64 = 0.45;

/// Does the contour piece `a`–`b` lie on an upright cut? There the
/// contours of every height stack along the cut's line: in 3D a ribbon
/// would stand up the face as a fin, in plan they pile up into a band
/// (owner, site polish 6). Steeper than 3 in 1 across it is a cut.
fn on_a_cut(g: &crate::ground::Ground, a: DVec2, b: DVec2) -> bool {
    steeper_than(g, a, b, 3.0)
}

/// Is the ground across `a`–`b` steeper than `rise` in 1?
fn steeper_than(g: &crate::ground::Ground, a: DVec2, b: DVec2, rise: f64) -> bool {
    let m = (a + b) / 2.0;
    let across = (b - a).perp().normalize_or_zero();
    if across == DVec2::ZERO {
        return false;
    }
    // close in, so a low upright face (a cut running out at its ends)
    // is caught as well as a tall one; and wider, for a bank
    [0.02, 0.1].iter().any(
        |d| match (g.height(m + across * *d), g.height(m - across * *d)) {
            (Some(p), Some(q)) => (p - q).abs() > 2.0 * d * rise,
            _ => false,
        },
    )
}

/// A terrain's contours as thin ribbons lying on it, for the 3D view: the
/// heavier lines wider. None for flat ground or with contours off.
fn ground_lines(model: &Model, s: &Site) -> Option<TriMesh> {
    let SiteForm::Terrain { contour, data, .. } = &s.form else {
        return None;
    };
    let g = shaped_ground(model, s)?;
    let base = members::elevation(model, s.level);
    // 3D has no view of its own to ask: as the first plan that set its
    // contours, or as the terrain says
    let look = creabim_core::site::look_in_3d(model, *contour, data);
    let every = look.major_every() as i64;
    // none on the works at all, floors and banks: there the ribbons of
    // a bank stack into steps and a dug floor's mirror the ground above
    // it (owner, cut test); the works' own edges say where they are
    let rings: Vec<Vec<DVec2>> = crate::earth::shaped(model, s)
        .filter(|sh| std::sync::Arc::ptr_eq(&sh.ground, &g))
        .map_or(vec![], |sh| {
            sh.banks
                .iter()
                .map(|b| b.iter().map(|(_, d)| d.truncate()).collect())
                .collect()
        });
    let on_works = |p: DVec2| rings.iter().any(|r| creabim_core::polygon::contains(r, p));
    let mut m = TriMesh::default();
    for c in g.contours(look.interval, base) {
        let half = if every > 0 && c.index.rem_euclid(every) == 0 {
            0.075
        } else {
            0.035
        };
        // lying on the slope, a little proud of it so it never fights
        // with the surface
        let on = |p: DVec2| p.extend(base + g.height(p).unwrap_or(c.z).max(c.z) + 0.015);
        let n = c.points.len();
        let last = if c.closed { n } else { n.saturating_sub(1) };
        for i in 0..last {
            let (a, b) = (c.points[i], c.points[(i + 1) % n]);
            let side = (b - a).perp().normalize_or_zero() * half;
            // none on a bank steeper than 1 in 1 either: in 3D the ribbons
            // of a steep bank stack into steps (owner, site polish 6); its
            // own edges say where it is, and plans still draw them
            if side == DVec2::ZERO || steeper_than(&g, a, b, 1.0) || on_works((a + b) / 2.0) {
                continue;
            }
            let q = [on(a - side), on(b - side), on(b + side), on(a + side)];
            let up = (q[1] - q[0]).cross(q[3] - q[0]).normalize_or(DVec3::Z);
            m.add_polygon(&q, &[[0, 1, 2], [0, 2, 3]], up);
        }
    }
    (!m.is_empty()).then_some(m)
}

/// The colour a lot's lines are drawn in, in plan and 3D.
const LOT_COLOR: [f32; 3] = [0.85, 0.45, 0.13];

/// A closed line lying on the ground as a ribbon `half` wide each side,
/// a little proud of it; `dash` (on, off) breaks it into dashes. Where no
/// ground lies under it, it lies at `level`.
fn lot_lines(
    model: &Model,
    ring: &[DVec2],
    half: f64,
    dash: Option<(f64, f64)>,
    level: f64,
) -> TriMesh {
    let mut m = TriMesh::default();
    let n = ring.len();
    if n < 2 {
        return m;
    }
    let ground = crate::earth::ground_sampler(model);
    let on = |p: DVec2| p.extend(ground(p).unwrap_or(level) + 0.03);
    let mut gone = 0.0;
    for i in 0..n {
        let (a, b) = (ring[i], ring[(i + 1) % n]);
        let len = a.distance(b);
        if len < 1e-6 {
            continue;
        }
        let side = (b - a).perp() / len * half;
        let k = (len / 0.5).ceil().max(1.0) as usize;
        for j in 0..k {
            let (p, q) = (
                a.lerp(b, j as f64 / k as f64),
                a.lerp(b, (j + 1) as f64 / k as f64),
            );
            let mid = gone + len * (j as f64 + 0.5) / k as f64;
            if let Some((d_on, d_off)) = dash {
                if mid.rem_euclid(d_on + d_off) > d_on {
                    continue;
                }
            }
            let quad = [on(p - side), on(q - side), on(q + side), on(p + side)];
            m.add_polygon(&quad, &[[0, 1, 2], [0, 2, 3]], DVec3::Z);
        }
        gone += len;
    }
    m
}

/// The whole mesh and default colour of a part or a site element.
pub fn mesh_of(model: &Model, e: &Element) -> Option<(TriMesh, [f32; 3])> {
    match &e.kind {
        ElementKind::Part(p) => Some((
            part_mesh(model, p)?,
            if p.void { VOID_COLOR } else { PART_COLOR },
        )),
        ElementKind::Site(s) => {
            let parts = site_meshes(model, s);
            let color = parts.iter().find(|p| p.2).map_or(GROUND_COLOR, |p| p.1);
            let mut m = TriMesh::default();
            for (mesh, ..) in parts {
                m.append(&mesh);
            }
            Some((m, color))
        }
        _ => None,
    }
}

/// The scene items of a part or a site element: a void as a see-through
/// ghost, a plant's trunk and crown each in its own look.
pub fn scene_items(model: &Model, e: &Element) -> Vec<SceneItem> {
    let look = |default: [f32; 3], own: bool| {
        creabim_core::material_of(model, e)
            .filter(|_| own)
            .map_or((default, PLAIN_SURFACE), |(_, m)| (m.color, m.surface()))
    };
    let item = |mesh: TriMesh, (color, surface): ([f32; 3], [f32; 3]), ghost: bool| SceneItem {
        id: e.id,
        mesh,
        color,
        surface,
        ghost,
        layer: None,
        finish: None,
    };
    match &e.kind {
        ElementKind::Part(p) => match part_mesh(model, p) {
            Some(m) if p.void => vec![item(m, (VOID_COLOR, PLAIN_SURFACE), true)],
            Some(m) => vec![item(m, look(PART_COLOR, true), false)],
            None => vec![],
        },
        // a lot's envelope: a see-through outline in 3D, never drawn
        ElementKind::Site(creabim_core::Site {
            form:
                SiteForm::Property {
                    boundary,
                    edges,
                    rules,
                },
            ..
        }) => {
            let level = match &e.kind {
                ElementKind::Site(st) => members::elevation(model, st.level),
                _ => 0.0,
            };
            let mut out: Vec<SceneItem> = crate::planning::envelope_mesh(boundary, edges, rules)
                .map(|m| {
                    let mut it = item(m, (ENVELOPE_COLOR, PLAIN_SURFACE), true);
                    it.finish = Some(crate::scene::LOT_ENVELOPE);
                    vec![it]
                })
                .unwrap_or_default();
            // its boundary and setback lying on the ground, in the plan's
            // colour: never printed or exported (owner, cut test)
            let mut lines = lot_lines(model, boundary, 0.07, None, level);
            if rules.has_setbacks {
                if let Some(inner) = creabim_core::lot::buildable(boundary, edges, rules) {
                    lines.append(&lot_lines(model, &inner, 0.045, Some((0.9, 0.6)), level));
                }
            }
            if !lines.is_empty() {
                let mut it = item(lines, (LOT_COLOR, [PLAIN_SURFACE[0], 0.0, 0.95]), true);
                it.finish = Some(crate::scene::LOT_LINES);
                out.push(it);
            }
            out
        }
        // a plant's crown draws as its outline in elevations and sections
        ElementKind::Site(s) => site_meshes(model, s)
            .into_iter()
            .filter(|(m, ..)| !m.is_empty())
            .map(|(m, color, own)| SceneItem {
                finish: if is_pad_skin(s) {
                    Some(crate::scene::PAD_SKIN)
                } else {
                    s.is_plant().then_some(if own {
                        crate::tree::FOLIAGE
                    } else if s.tree_shape().is_some_and(|t| !t.solid) {
                        crate::tree::HOLLOW_WOOD
                    } else {
                        crate::tree::WOOD
                    })
                },
                // a generated crown's own leaf colour, which turns with
                // its month, over its material's
                ..item(m, look(color, own && s.tree_shape().is_none()), false)
            })
            .chain(ground_lines(model, s).map(|m| {
                // the contours, drawn on the ground in a darker shade of it
                let (c, surface) = look(GROUND_COLOR, true);
                SceneItem {
                    finish: Some(crate::scene::GROUND_LINES),
                    ..item(m, (c.map(|v| v * 0.42), surface), false)
                }
            }))
            .collect(),
        _ => vec![],
    }
}

// ---- plans ----

/// How a part or a site element on its level is drawn in its plan, cut
/// `cut` metres above the level.
pub fn plan_items(model: &Model, e: &Element, cut: f64) -> Vec<PlanItem> {
    let z = |l: ElementId| members::elevation(model, l) + cut;
    match &e.kind {
        // a void: what it cuts shows the cut; the void itself is its
        // footprint, dashed, so it can be seen and picked (MU23)
        ElementKind::Part(p) if p.void => {
            let Some(mesh) = part_mesh(model, p) else {
                return vec![];
            };
            let item = SceneItem {
                id: e.id,
                mesh,
                color: VOID_COLOR,
                surface: PLAIN_SURFACE,
                ghost: true,
                layer: None,
                finish: None,
            };
            let mut out = vec![];
            let crease = if matches!(p.form, creabim_core::parts::Form::Sweep { .. }) {
                0.906
            } else {
                0.995
            };
            overhead_outline_creased(&item, &mut out, crease);
            out
        }
        ElementKind::Part(p) => {
            let Some(mesh) = part_mesh(model, p) else {
                return vec![];
            };
            let item = SceneItem {
                id: e.id,
                mesh,
                color: PART_COLOR,
                surface: PLAIN_SURFACE,
                ghost: false,
                layer: None,
                finish: None,
            };
            let mut out = vec![];
            let cut_z = z(p.level);
            // a sweep's seams along a curve are smooth, not edges (MU23)
            let crease = if matches!(p.form, creabim_core::parts::Form::Sweep { .. }) {
                0.906
            } else {
                0.995
            };
            match item.mesh.bounds() {
                // wholly above the cut, like a coping on a low wall: its
                // outline dashed, as overhead
                Some((lo, _)) if lo.z as f64 > cut_z + 1e-6 => {
                    overhead_outline_creased(&item, &mut out, crease)
                }
                // a sweep along a level path: its cut, and below the cut
                // only its outer edges (the profile's width either side of
                // the path), filled; no seams inside (MU37)
                _ if matches!(p.form, creabim_core::parts::Form::Sweep { .. })
                    && p.face_normal().is_none() =>
                {
                    crate::plan::sliced_plan_creased(model, &item, cut_z, &mut out, -2.0);
                    if let (
                        creabim_core::parts::Form::Sweep { profile, .. },
                        Some((path, closed)),
                    ) = (&p.form, p.path(model))
                    {
                        let lo = profile.iter().map(|q| q.x).fold(f64::MAX, f64::min);
                        let hi = profile.iter().map(|q| q.x).fold(f64::MIN, f64::max);
                        let pts: Vec<DVec2> = path.iter().map(|q| q.truncate()).collect();
                        let side = |off: f64| -> Vec<DVec2> {
                            let n = pts.len();
                            (0..n)
                                .map(|i| {
                                    let d0 = if i > 0 {
                                        (pts[i] - pts[i - 1]).normalize_or_zero()
                                    } else if closed && n > 2 {
                                        (pts[0] - pts[n - 1]).normalize_or_zero()
                                    } else {
                                        DVec2::ZERO
                                    };
                                    let d1 = if i + 1 < n {
                                        (pts[i + 1] - pts[i]).normalize_or_zero()
                                    } else if closed && n > 2 {
                                        (pts[0] - pts[i]).normalize_or_zero()
                                    } else {
                                        DVec2::ZERO
                                    };
                                    let m = (d0.perp() + d1.perp()).normalize_or_zero();
                                    let c = m
                                        .dot(if d1 != DVec2::ZERO {
                                            d1.perp()
                                        } else {
                                            d0.perp()
                                        })
                                        .max(0.3);
                                    pts[i] + m * (off / c)
                                })
                                .collect()
                        };
                        let (l, r) = (side(hi), side(lo));
                        let mut lines = |v: &[DVec2]| {
                            let k = if closed {
                                v.len()
                            } else {
                                v.len().saturating_sub(1)
                            };
                            for i in 0..k {
                                out.push(PlanItem::Line {
                                    id: e.id,
                                    pen: Pen::Projection,
                                    a: v[i],
                                    b: v[(i + 1) % v.len()],
                                });
                            }
                        };
                        lines(&l);
                        lines(&r);
                        if !closed && !l.is_empty() {
                            out.push(PlanItem::Line {
                                id: e.id,
                                pen: Pen::Projection,
                                a: l[0],
                                b: r[0],
                            });
                            out.push(PlanItem::Line {
                                id: e.id,
                                pen: Pen::Projection,
                                a: l[l.len() - 1],
                                b: r[r.len() - 1],
                            });
                        }
                    }
                }
                _ => crate::plan::sliced_plan_creased(model, &item, cut_z, &mut out, crease),
            }
            out
        }
        ElementKind::Site(s) => site_plan(model, e.id, s),
        _ => vec![],
    }
}

/// A part's edges seen from below, flattened and dashed: how something
/// above the plan cut is drawn.
fn overhead_outline_creased(item: &SceneItem, out: &mut Vec<PlanItem>, crease: f32) {
    let key = |p: DVec2| ((p.x * 1e3).round() as i64, (p.y * 1e3).round() as i64);
    let mut seen = std::collections::HashSet::new();
    for (a, b) in crate::section::feature_edges_over(item, crease) {
        let (a, b) = (a.truncate(), b.truncate());
        if a.distance(b) < 1e-4 {
            continue;
        }
        let (ka, kb) = (key(a), key(b));
        if !seen.insert(if ka <= kb { (ka, kb) } else { (kb, ka) }) {
            continue;
        }
        out.push(PlanItem::Line {
            id: item.id,
            pen: Pen::Hidden,
            a,
            b,
        });
    }
}

fn loop_lines(out: &mut Vec<PlanItem>, id: ElementId, pen: Pen, pts: &[DVec2]) {
    let n = pts.len();
    for i in 0..n {
        out.push(PlanItem::Line {
            id,
            pen,
            a: pts[i],
            b: pts[(i + 1) % n],
        });
    }
}

/// The ground read: shaded by slope, aspect or height, and with which
/// way water runs off it and where it gathers.
#[allow(clippy::too_many_arguments)]
fn ground_analysis(
    model: &Model,
    out: &mut Vec<PlanItem>,
    id: ElementId,
    g: &crate::ground::Ground,
    boundary: &[DVec2],
    look: &creabim_core::GroundLook,
    base: f64,
) {
    let north = model.project_info().map_or(0.0, |(_, p)| p.true_north);
    let sh = crate::analysis::shade(g, look.shade, north, base, look.interval);
    for (t, k) in &sh.tris {
        out.push(PlanItem::Tint {
            id,
            points: t.to_vec(),
            rgb: sh.bands[*k].rgb,
        });
    }
    if !look.flow {
        return;
    }
    let spacing = crate::analysis::flow_spacing(boundary);
    let (arrows, lows) = crate::analysis::flow(g, boundary, spacing);
    let len = spacing * 0.7;
    for (p, d) in arrows {
        let (a, b) = (p - d * len / 2.0, p + d * len / 2.0);
        out.push(PlanItem::Line {
            id,
            pen: Pen::Symbol,
            a,
            b,
        });
        let side = d.perp() * len * 0.18;
        for s in [side, -side] {
            out.push(PlanItem::Line {
                id,
                pen: Pen::Symbol,
                a: b,
                b: b - d * len * 0.3 + s,
            });
        }
    }
    for p in lows {
        out.push(PlanItem::Arc {
            id,
            pen: Pen::Symbol,
            center: p,
            radius: spacing * 0.3,
            start: 0.0,
            end: std::f64::consts::TAU,
        });
    }
}

/// A site element in plan: ground with its contours and spot heights, an
/// area with its pattern, a plant's symbol.
fn site_plan(model: &Model, id: ElementId, s: &Site) -> Vec<PlanItem> {
    site_plan_with(model, id, s, &SiteView::default())
}

/// The colour of ground dug `d` deep (negative) or made up (positive):
/// orange for cut, teal for fill, deeper for more.
pub(crate) fn cut_fill_rgb(d: f64) -> [f32; 3] {
    let (base, paper) = if d < 0.0 {
        ([0.85_f32, 0.42, 0.20], [0.98_f32, 0.90, 0.84])
    } else {
        ([0.18_f32, 0.60, 0.58], [0.85_f32, 0.94, 0.93])
    };
    let t = (d.abs() / 1.5).clamp(0.15, 1.0) as f32;
    [0, 1, 2].map(|k| paper[k] + (base[k] - paper[k]) * t)
}

/// A site element in plan as view `sv` shows the site: the ground
/// before or after its works, and each kind of site information on or
/// off.
pub fn site_plan_with(model: &Model, id: ElementId, s: &Site, sv: &SiteView) -> Vec<PlanItem> {
    let mut out = vec![];
    let before = sv.ground == GroundState::Before;
    // how this view draws the ground: its own look, or the terrain's
    let look = match &s.form {
        SiteForm::Terrain { contour, data, .. } => sv.look_on(*contour, data),
        _ => creabim_core::GroundLook::default(),
    };
    match &s.form {
        SiteForm::Terrain {
            boundary,
            points,
            contour: _,
            data,
            ..
        } => {
            loop_lines(&mut out, id, Pen::Projection, boundary);
            let base = members::elevation(model, s.level);
            let sh = crate::earth::shaped(model, s);
            let shown = if before {
                sh.as_ref()
                    .filter(|_| creabim_core::site::surveyed(model, s))
                    .map(|sh| sh.existing.clone())
            } else {
                shaped_ground(model, s)
            };
            // what the works did, coloured by depth
            if let Some(sh) = sh
                .as_ref()
                .filter(|sh| sv.cut_fill && !before && sh.changed())
            {
                for t in &sh.ground.fine.tris {
                    let q = t.map(|i| sh.ground.fine.verts[i as usize]);
                    let c = (q[0] + q[1] + q[2]).truncate() / 3.0;
                    let z = (q[0].z + q[1].z + q[2].z) / 3.0;
                    let Some(e) = sh.existing.height(c) else {
                        continue;
                    };
                    let d = z - e;
                    if d.abs() < 0.02 {
                        continue;
                    }
                    out.push(PlanItem::Tint {
                        id,
                        points: q.iter().map(|p| p.truncate()).collect(),
                        rgb: cut_fill_rgb(d),
                    });
                }
            }
            // the ground as it was, dashed, where the works changed it
            if let Some(sh) = sh
                .as_ref()
                .filter(|sh| sv.ground == GroundState::Both && sv.contours && sh.changed())
            {
                let moved = |p: DVec2| match (sh.ground.height(p), sh.existing.height(p)) {
                    (Some(a), Some(b)) => (a - b).abs() > 0.02,
                    _ => false,
                };
                for c in sh.existing.contours(look.interval, base) {
                    let n = c.points.len();
                    let last = if c.closed { n } else { n.saturating_sub(1) };
                    for i in 0..last {
                        let (a, b) = (c.points[i], c.points[(i + 1) % n]);
                        if moved((a + b) / 2.0) {
                            out.push(PlanItem::Line {
                                id,
                                pen: Pen::Existing,
                                a,
                                b,
                            });
                        }
                    }
                }
            }
            if let Some(g) = shown {
                ground_analysis(model, &mut out, id, &g, boundary, &look, base);
                let every = look.major_every() as i64;
                // under a road the ground is its formation: no contours
                let roads: Vec<Vec<DVec2>> = if before {
                    vec![]
                } else {
                    crate::earth::road_pads(model, s)
                        .into_iter()
                        .map(|p| p.outline)
                        .collect()
                };
                let under_road = |a: DVec2, b: DVec2| {
                    let m = (a + b) / 2.0;
                    roads.iter().any(|r| creabim_core::polygon::contains(r, m))
                };
                // nor over a pit's or a trench's floor: it follows the
                // ground above it, so its contours read as the ground's
                // own heights where the ground is lower (owner, cut test)
                let dug: Vec<Vec<DVec2>> = if before {
                    vec![]
                } else {
                    crate::earth::works(model, s)
                        .into_iter()
                        .filter(|p| p.depth > 0.0)
                        .map(|p| p.outline)
                        .collect()
                };
                let on_dug = |a: DVec2, b: DVec2| {
                    let m = (a + b) / 2.0;
                    dug.iter().any(|r| creabim_core::polygon::contains(r, m))
                };
                // nor on a bank drawn hatched: the ticks say its slope, and
                // its contours with them read as a ladder (owner,
                // Mockup_Site_Earthworks); unhatched, they are its drawing
                let hatched: Vec<(Vec<DVec2>, Vec<DVec2>)> = match sh.as_ref() {
                    Some(sh) if sv.banks && !before => sh
                        .banks
                        .iter()
                        .map(|b| {
                            (
                                b.iter().map(|(a, _)| a.truncate()).collect(),
                                b.iter().map(|(_, d)| d.truncate()).collect(),
                            )
                        })
                        .collect(),
                    _ => vec![],
                };
                let on_bank = |a: DVec2, b: DVec2| {
                    let m = (a + b) / 2.0;
                    hatched.iter().any(|(edge, ring)| {
                        creabim_core::polygon::contains(ring, m)
                            && !creabim_core::polygon::contains(edge, m)
                    })
                };
                let contours = if sv.contours {
                    g.contours(look.interval, base)
                } else {
                    vec![]
                };
                for c in contours {
                    let major = c.index.rem_euclid(every) == 0;
                    let pen = if major {
                        Pen::ContourHeavy
                    } else {
                        Pen::Contour
                    };
                    let n = c.points.len();
                    let last = if c.closed { n } else { n.saturating_sub(1) };
                    for i in 0..last {
                        let (a, b) = (c.points[i], c.points[(i + 1) % n]);
                        if under_road(a, b) || on_dug(a, b) || on_bank(a, b) || on_a_cut(&g, a, b) {
                            continue;
                        }
                        out.push(PlanItem::Line { id, pen, a, b });
                    }
                    if major && look.heights && sv.levels {
                        if let Some(at) = contour_label_at(&c.points)
                            .filter(|p| !under_road(*p, *p) && !on_dug(*p, *p) && !on_bank(*p, *p))
                        {
                            out.push(ground_text(id, at, contour_level(model, base + c.z)));
                        }
                    }
                }
                // break lines, as the surveyor drew them
                for l in data
                    .lines
                    .iter()
                    .filter(|l| l.kind == GroundLineKind::Break)
                {
                    let n = l.points.len();
                    let last = if l.closed { n } else { n - 1 };
                    for i in 0..last {
                        out.push(PlanItem::Line {
                            id,
                            pen: Pen::Hidden,
                            a: l.points[i].truncate(),
                            b: l.points[(i + 1) % n].truncate(),
                        });
                    }
                }
            }
            // the works: each pad's edge, the line where its bank meets
            // the ground, and the bank hatched from its top, long and short
            if let Some(sh) = sh.as_ref().filter(|_| !before) {
                // the limit of works: round where the banks meet the ground
                if sv.limit {
                    for bank in &sh.banks {
                        let n = bank.len();
                        for i in 0..n {
                            let (a, b) = (bank[i].1.truncate(), bank[(i + 1) % n].1.truncate());
                            if a.distance(b) > 1e-6 {
                                out.push(PlanItem::Line {
                                    id,
                                    pen: Pen::Hidden,
                                    a,
                                    b,
                                });
                            }
                        }
                    }
                }
                // each bank: its edge heavy, the line where it meets the
                // ground fine, and (hatched) ticks from its top, long and
                // short, evenly along it (owner, Mockup_Site_Earthworks)
                for bank in &sh.banks {
                    let n = bank.len();
                    let wide = |i: usize| {
                        let (a, d) = bank[i % n];
                        a.truncate().distance(d.truncate()) > 0.05
                    };
                    if !(0..n).any(wide) {
                        continue;
                    }
                    for i in 0..n {
                        let ((a0, d0), (a1, d1)) = (bank[i], bank[(i + 1) % n]);
                        if a0.truncate().distance(a1.truncate()) > 1e-6 {
                            out.push(PlanItem::Line {
                                id,
                                pen: Pen::ContourHeavy,
                                a: a0.truncate(),
                                b: a1.truncate(),
                            });
                        }
                        if wide(i) && wide(i + 1) && d0.truncate().distance(d1.truncate()) > 1e-6 {
                            out.push(PlanItem::Line {
                                id,
                                pen: Pen::Contour,
                                a: d0.truncate(),
                                b: d1.truncate(),
                            });
                        }
                    }
                    if !sv.banks {
                        continue;
                    }
                    bank_ticks(&mut out, id, bank);
                }
            }
            // spot heights: a small cross with its level, where the view
            // shows levels. Left out where the works moved the ground (the
            // level there is gone), under buildings, and where they would
            // crowd each other.
            let mut placed: Vec<DVec2> = vec![];
            let gone = |c: DVec2| {
                !before
                    && sh.as_ref().is_some_and(|sh| {
                        match (sh.existing.height(c), sh.ground.height(c)) {
                            (Some(a), Some(b)) => (a - b).abs() > 0.02,
                            _ => false,
                        }
                    })
            };
            for p in points {
                let c = p.truncate();
                if !(look.spots && sv.levels) || gone(c) || under_building(model, c) {
                    continue;
                }
                if placed.iter().all(|q| q.distance(c) > 2.5) {
                    placed.push(c);
                    for d in [DVec2::new(0.15, 0.15), DVec2::new(0.15, -0.15)] {
                        out.push(PlanItem::Line {
                            id,
                            pen: Pen::Symbol,
                            a: c - d,
                            b: c + d,
                        });
                    }
                    out.push(level_tag(
                        model,
                        id,
                        c,
                        creabim_core::site_level(model, base + p.z),
                        sv,
                    ));
                }
            }
        }
        SiteForm::Area {
            boundary,
            holes,
            pattern,
            ..
        } => {
            loop_lines(&mut out, id, Pen::Projection, boundary);
            for h in holes {
                loop_lines(&mut out, id, Pen::Projection, h);
            }
            let mut loops = vec![boundary.clone()];
            loops.extend(holes.iter().cloned());
            for (a, b) in pattern_lines(pattern, &loops) {
                out.push(PlanItem::Line {
                    id,
                    pen: Pen::Pattern,
                    a,
                    b,
                });
            }
            // a car park: its bays, their sides and back (the aisle side
            // open), numbered; accessible zones hatched
            if let Some(lay) = s.car_park_layout(model) {
                let numbers = s.as_car_park().is_some_and(|c| c.numbers);
                for b in &lay.bays {
                    for k in 1..4 {
                        out.push(PlanItem::Line {
                            id,
                            pen: Pen::Symbol,
                            a: b.corners[k],
                            b: b.corners[(k + 1) % 4],
                        });
                    }
                    let mark = b.kind.mark();
                    let text = match (numbers, mark) {
                        // an accessible bay's number already says it
                        (true, "A") => b.label.clone(),
                        (true, "") => b.label.clone(),
                        (true, m) => format!("{m} {}", b.label),
                        (false, m) => m.to_string(),
                    };
                    if !text.is_empty() {
                        let mut t = ground_text(id, b.middle(), text);
                        if let PlanItem::Text { size, bold, .. } = &mut t {
                            *size = 1.8;
                            *bold = !mark.is_empty();
                        }
                        out.push(t);
                    }
                }
                // edge strips: their inner line (a kerb, or a fine line for
                // a clearance), a walkway's joints, planting's hatch
                for (_, ring, what) in &lay.strips {
                    use creabim_core::carpark::StripUse;
                    let (a, b, bi, ai) = (ring[0], ring[1], ring[2], ring[3]);
                    let pen = if *what == StripUse::Clear {
                        Pen::Fine
                    } else {
                        Pen::Projection
                    };
                    out.push(PlanItem::Line {
                        id,
                        pen,
                        a: ai,
                        b: bi,
                    });
                    let len = a.distance(b);
                    let (step, lean) = match what {
                        StripUse::Walkway => (0.6, 0.0),
                        StripUse::Planting => (1.0, 0.5),
                        StripUse::Clear => continue,
                    };
                    if len < 1e-6 {
                        continue;
                    }
                    // square across it (a walkway's joints) or leaning
                    // (planting), kept between its mitred ends
                    let d = (b - a) / len;
                    let deep = (ai - a).dot(d.perp());
                    let (lo, hi) = ((ai - a).dot(d), (bi - a).dot(d));
                    let k = (len / step).floor() as usize;
                    for j in 1..=k {
                        let at = j as f64 * step;
                        let to = at + lean;
                        if at >= len || to <= lo || to >= hi {
                            continue;
                        }
                        out.push(PlanItem::Line {
                            id,
                            pen: Pen::Fine,
                            a: a + d * at,
                            b: a + d * to + d.perp() * deep,
                        });
                    }
                }
                // islands as kerbed outlines; turning heads crossed, kept clear
                for c in &lay.islands {
                    loop_lines(&mut out, id, Pen::Projection, c);
                }
                for c in &lay.heads {
                    loop_lines(&mut out, id, Pen::Fine, c);
                    for (a, b) in [(c[0], c[2]), (c[1], c[3])] {
                        out.push(PlanItem::Line {
                            id,
                            pen: Pen::Fine,
                            a,
                            b,
                        });
                    }
                }
                // a one-way aisle's arrow on each of its stretches
                for l in lay.lines.iter().filter(|l| l.one_way) {
                    for s in l.path.windows(2) {
                        let len = s[0].distance(s[1]);
                        if len < 4.0 {
                            continue;
                        }
                        let d = (s[1] - s[0]) / len;
                        let mid = (s[0] + s[1]) / 2.0;
                        let (tail, head) = (mid - d * 1.2, mid + d * 1.2);
                        out.push(PlanItem::Line {
                            id,
                            pen: Pen::Symbol,
                            a: tail,
                            b: head,
                        });
                        for w in [d.perp(), -d.perp()] {
                            out.push(PlanItem::Line {
                                id,
                                pen: Pen::Symbol,
                                a: head,
                                b: head - d * 0.6 + w * 0.3,
                            });
                        }
                    }
                }
                for z in &lay.zones {
                    for k in 0..4 {
                        out.push(PlanItem::Line {
                            id,
                            pen: Pen::Fine,
                            a: z[k],
                            b: z[(k + 1) % 4],
                        });
                    }
                    for (a, b) in zone_hatch(z) {
                        out.push(PlanItem::Line {
                            id,
                            pen: Pen::Fine,
                            a,
                            b,
                        });
                    }
                }
            }
        }
        // an earthwork: its outline, which is what a click takes it by
        // (as a symbol, so it wins over the ground it is worked into);
        // its banks and its floor are the terrain's to draw
        SiteForm::Pad {
            boundary,
            depth,
            retained,
            ..
        } => {
            // a pad's platform, tinted apart from the ground round it
            if *depth <= 0.0 && !before {
                out.push(PlanItem::Tint {
                    id,
                    points: boundary.clone(),
                    rgb: PAD_COLOR,
                });
            }
            for k in 0..boundary.len() {
                out.push(PlanItem::Line {
                    id,
                    // an edge held by a retaining wall: a heavy line
                    pen: if retained.contains(&k) {
                        Pen::Cut
                    } else {
                        Pen::Symbol
                    },
                    a: boundary[k],
                    b: boundary[(k + 1) % boundary.len()],
                });
            }
        }
        SiteForm::Plant {
            position,
            form,
            canopy,
            height,
            ..
        } => match s.tree_shape() {
            Some(shape) => {
                let g = crate::tree::grow(shape, *canopy, *height);
                crate::tree::plan_symbol(&mut out, id, *position, &g, shape)
            }
            None => plant_symbol(&mut out, id, *position, *form, canopy / 2.0),
        },
        SiteForm::Property {
            boundary,
            edges,
            rules,
        } => {
            if sv.legal {
                lot_plan(
                    model,
                    &mut out,
                    id,
                    boundary,
                    edges,
                    rules,
                    (sv.setbacks_of(rules), sv.bearings_of(rules), sv.along),
                )
            }
        }
        SiteForm::Road(r) => {
            if sv.roads && !before {
                road_plan(model, &mut out, id, s, r, sv);
                crate::services::gullies_plan(model, &mut out, id, s, r, sv.levels);
            }
        }
        SiteForm::Drain(d) => {
            if sv.services && !before {
                crate::services::drain_plan(model, &mut out, id, s, d, sv.levels)
            }
        }
        SiteForm::Tank(t) => {
            if sv.services && !before {
                crate::services::tank_plan(model, &mut out, id, s, t, sv.levels)
            }
        }
        SiteForm::Walk(w) => {
            if sv.roads && !before {
                crate::walk::walk_plan(&mut out, id, w, sv.levels)
            }
        }
        // where the report looked is drawn on the ground as it was
        SiteForm::Investigation(p) => {
            if sv.investigation {
                crate::investigation::probe_plan(model, &mut out, id, s, p, sv.levels)
            }
        }
        // a spot level: a small cross with its level, in views that show
        // levels. It is what a click takes it by
        SiteForm::Spot { position, height } => {
            if sv.levels {
                for d in [DVec2::new(0.15, 0.15), DVec2::new(0.15, -0.15)] {
                    out.push(PlanItem::Line {
                        id,
                        pen: Pen::Symbol,
                        a: *position - d,
                        b: *position + d,
                    });
                }
                let base = members::elevation(model, s.level);
                out.push(level_tag(
                    model,
                    id,
                    *position,
                    creabim_core::site_level(model, base + height),
                    sv,
                ));
            }
        }
        // a line drawn on the ground: a contour as it was traced, in
        // views that show contours; a break line dashed, as a survey's
        SiteForm::Line(l) => {
            if sv.contours {
                let pen = match l.kind {
                    GroundLineKind::Contour => Pen::Symbol,
                    GroundLineKind::Break => Pen::Hidden,
                };
                let n = l.points.len();
                let last = if l.closed { n } else { n.saturating_sub(1) };
                for i in 0..last {
                    out.push(PlanItem::Line {
                        id,
                        pen,
                        a: l.points[i].truncate(),
                        b: l.points[(i + 1) % n].truncate(),
                    });
                }
            }
        }
    }
    out
}

/// A road in plan: its carriageway tinted between its kerbs, the
/// footpaths' backs, its centreline, its ends, its bays; along it, an
/// arrow down each stretch with its grade, and the level at each point.
fn road_plan(
    model: &Model,
    out: &mut Vec<PlanItem>,
    id: ElementId,
    s: &Site,
    r: &creabim_core::road::Road,
    sv: &SiteView,
) {
    if r.path.len() < 2 {
        return;
    }
    let plan = r.plan();
    let half = r.width / 2.0;
    let line = |out: &mut Vec<PlanItem>, pen: Pen, pts: &[DVec2]| {
        for w in pts.windows(2) {
            out.push(PlanItem::Line {
                id,
                pen,
                a: w[0],
                b: w[1],
            });
        }
    };
    let (l, rt) = (r.side_line(half), r.side_line(-half));
    let mut carriageway = rt.clone();
    carriageway.extend(l.iter().rev());
    // a tight bend gathers an edge onto one point several times; the
    // fill took them as a broken outline and left gaps (owner, cut test)
    let once = |mut ring: Vec<DVec2>| {
        ring.dedup_by(|b, a| a.distance(*b) < 1e-6);
        while ring.len() > 3 && ring[0].distance(ring[ring.len() - 1]) < 1e-6 {
            ring.pop();
        }
        ring
    };
    let carriageway = once(carriageway);
    out.push(PlanItem::Tint {
        id,
        points: carriageway,
        rgb: [0.72, 0.72, 0.74],
    });
    let n = plan.len();
    let mut backs = (l.clone(), rt.clone());
    if r.footpath > 0.0 {
        let (bl, br) = (
            r.side_line(half + r.footpath),
            r.side_line(-half - r.footpath),
        );
        for (edge, back) in [(&l, &bl), (&rt, &br)] {
            let mut path = edge.clone();
            path.extend(back.iter().rev());
            out.push(PlanItem::Tint {
                id,
                points: once(path),
                rgb: [0.86, 0.85, 0.82],
            });
        }
        line(out, Pen::Projection, &bl);
        line(out, Pen::Projection, &br);
        backs = (bl, br);
    }
    line(out, Pen::Projection, &l);
    line(out, Pen::Projection, &rt);
    // a kerb's back, a kerb's width behind its face
    if r.kerb > 0.0 {
        line(out, Pen::Fine, &r.side_line(half + 0.125));
        line(out, Pen::Fine, &r.side_line(-half - 0.125));
    }
    line(out, Pen::Grid, &plan);
    for i in [0, n - 1] {
        out.push(PlanItem::Line {
            id,
            pen: Pen::Projection,
            a: backs.0[i],
            b: backs.1[i],
        });
    }
    for b in r.bays() {
        for k in 1..4 {
            out.push(PlanItem::Line {
                id,
                pen: Pen::Symbol,
                a: b[k],
                b: b[(k + 1) % 4],
            });
        }
    }
    // its grades and levels
    if !sv.levels {
        return;
    }
    let base = members::elevation(model, s.level);
    for (i, g) in r.grades().iter().enumerate() {
        let (a, b) = (plan[i], plan[i + 1]);
        let len = a.distance(b);
        if len < 1.0 || g.abs() < 1e-4 {
            continue;
        }
        // the arrow points down the slope
        let down = if *g > 0.0 {
            (a - b) / len
        } else {
            (b - a) / len
        };
        let mid = (a + b) / 2.0;
        let reach = (len * 0.3).min(4.0);
        let side = down.perp() * (half * 0.45);
        let (t, h) = (
            mid + side - down * reach / 2.0,
            mid + side + down * reach / 2.0,
        );
        out.push(PlanItem::Line {
            id,
            pen: Pen::Symbol,
            a: t,
            b: h,
        });
        for w in [down.perp(), -down.perp()] {
            out.push(PlanItem::Line {
                id,
                pen: Pen::Symbol,
                a: h,
                b: h - down * 0.5 + w * 0.25,
            });
        }
        let text = if g.abs() >= 0.01 {
            format!("1:{:.0}  {:.1}%", 1.0 / g.abs(), g.abs() * 100.0)
        } else {
            format!("{:.1}%", g.abs() * 100.0)
        };
        out.push(ground_text(id, mid - side, text));
    }
    // each level on its path: a tag over a small cross where it is read,
    // as a spot level is (MU142)
    for p in &r.path {
        let c = p.truncate();
        for d in [DVec2::new(0.15, 0.15), DVec2::new(0.15, -0.15)] {
            out.push(PlanItem::Line {
                id,
                pen: Pen::Symbol,
                a: c - d,
                b: c + d,
            });
        }
        out.push(level_tag(
            model,
            id,
            c,
            creabim_core::site_level(model, base + p.z),
            sv,
        ));
    }
}

/// A level read on the ground in a plan, as a tag (MU142, owner: one tag
/// language): its words alone, in the tag text style and the annotation
/// colour, just over the point `at` it reads ([`fit_ground_text`] puts
/// them there for the drawing's scale), moved and its leader bent as the
/// view `sv` keeps them; a leader back once its words are away.
fn level_tag(model: &Model, id: ElementId, at: DVec2, text: String, sv: &SiteView) -> PlanItem {
    let mut look = crate::annotate::look_of(model, None, None);
    look.shape = creabim_core::TagShape::TextOnly;
    let moved = sv.spot_tag(id, at);
    PlanItem::Tag {
        id,
        anchor: at,
        at: at + moved.map_or(DVec2::ZERO, |m| m.by),
        elbow: moved.and_then(|m| m.elbow).map(|e| at + e),
        text,
        round: false,
        look,
    }
}

/// How far over its point a plan level tag's words sit: half their box
/// and a little air, so no leader shows while they are home.
pub fn level_tag_lift(look: &crate::annotation::TextLook, mm: f64) -> f64 {
    (2.2 * look.size / crate::annotate::TEXT + 0.6) * mm
}

/// A lot in plan: its boundary in long dashes, each edge's bearing and
/// length along it outside, and the setback lines dashed round a lightly
/// tinted buildable area, each setback written between.
fn lot_plan(
    model: &Model,
    out: &mut Vec<PlanItem>,
    id: ElementId,
    boundary: &[DVec2],
    edges: &[creabim_core::LotEdge],
    rules: &creabim_core::PlanningRules,
    (setbacks, bearings, along): (bool, bool, bool),
) {
    use creabim_core::lot;
    let n = boundary.len();
    loop_lines(out, id, Pen::Boundary, boundary);
    let units = model.units();
    let length = |d: f64| {
        if units.is_imperial() {
            units.length(d)
        } else {
            format!("{d:.2} m")
        }
    };
    let north = model.project_info().map_or(0.0, |(_, p)| p.true_north);
    let buildable = setbacks
        .then(|| lot::buildable(boundary, edges, rules))
        .flatten();
    // easements: their strips hatched and named
    for e in &rules.easements {
        let Some(strip) = lot::easement_outline(boundary, e) else {
            continue;
        };
        loop_lines(out, id, Pen::Hidden, &strip);
        let hatch = creabim_core::site::Pattern::Lines {
            spacing: 0.5,
            angle: 45.0,
        };
        for (a, b) in pattern_lines(&hatch, std::slice::from_ref(&strip)) {
            out.push(PlanItem::Line {
                id,
                pen: Pen::Fine,
                a,
                b,
            });
        }
        let mid = strip.iter().copied().sum::<DVec2>() / strip.len() as f64;
        out.push(lot_text(id, mid, format!("{} {}", e.name, length(e.width))));
    }
    if let Some(b) = &buildable {
        out.push(PlanItem::Tint {
            id,
            points: b.clone(),
            // a soft green, a notch above the site's own (Site polish 5)
            rgb: [0.82, 0.86, 0.80],
        });
        loop_lines(out, id, Pen::Setback, b);
    }
    for i in 0..n {
        let (a, b) = (boundary[i], boundary[(i + 1) % n]);
        let d = b - a;
        if d.length() < 1e-6 {
            continue;
        }
        let mid = (a + b) / 2.0;
        let inward = d.normalize().perp();
        // along the edge: its direction, turned to read from below or the
        // right, never upside down
        let turn = if along { readable(d) } else { 0.0 };
        if bearings {
            let text = format!(
                "{}  {}",
                lot::bearing_text(lot::azimuth(d, north)),
                length(d.length())
            );
            let mut t = lot_text(id, mid - inward * 0.6, text);
            if let PlanItem::Text { align, angle, .. } = &mut t {
                *angle = turn;
                // level beside an upright edge, the text runs away from
                // it, outside
                if !along && inward.x.abs() > 0.7 {
                    *align = if inward.x > 0.0 {
                        crate::paper::Anchor::End
                    } else {
                        crate::paper::Anchor::Start
                    };
                }
            }
            out.push(t);
        }
        let setback = rules.setback_at(edges, i, n);
        if buildable.is_some() && setback > 0.0 {
            // the setback names its kind of edge; a note goes on the line
            // below it, or where it was dragged, a leader back (MU98_06)
            let name = lot::edge_kind(edges, i, n).label();
            let at = mid + inward * (setback / 2.0);
            let mut t = lot_text(id, at, format!("{name} {}", length(setback)));
            if let PlanItem::Text { angle, .. } = &mut t {
                *angle = turn;
            }
            out.push(t);
            if let Some(note) = rules.note_at(i) {
                let off = rules.note_off(i).unwrap_or(DVec2::ZERO);
                let mut t = lot_text(id, at + off, note.to_string());
                if let PlanItem::Text { angle, size, .. } = &mut t {
                    *angle = turn;
                    *size = LOT_NOTE_MM;
                }
                out.push(t);
                if off.length() > 1e-6 {
                    // a leader from the setback line it belongs to, clear
                    // of the setback's own label
                    out.push(PlanItem::Line {
                        id,
                        pen: Pen::Fine,
                        a: mid + inward * setback,
                        b: at + off,
                    });
                }
            }
        }
    }
}

/// Is `s` a pad drawn with only its thin skin (no base course)?
fn is_pad_skin(s: &Site) -> bool {
    matches!(s.form, SiteForm::Pad { depth, base, .. } if depth <= 0.0 && base < 0.005)
}

/// A pad's platform: the light grey of a formation, apart from grass.
pub const PAD_COLOR: [f32; 3] = [0.80, 0.79, 0.75];
/// How thick the skin a pad without a base course is drawn with, metres.
const PAD_SKIN: f64 = 0.012;

/// The size of a terrain's own text (contour heights, spot levels) on
/// paper, millimetres.
pub const GROUND_TEXT_MM: f64 = 2.8;

/// The size of a lot's own text (bearings, setbacks) on paper, mm.
pub const LOT_TEXT_MM: f64 = 3.5;
/// A lot edge's note, on paper: a step smaller than its setback, and set
/// the line below it at every scale ([`fit_ground_text`]).
pub const LOT_NOTE_MM: f64 = 3.0;

/// A bearing or a setback written on a lot, sized on paper as
/// [`ground_text`] is.
/// The angle, in radians counter-clockwise, at which text along `d`
/// reads from below or from the right: never upside down.
pub fn readable(d: DVec2) -> f64 {
    let mut a = d.y.atan2(d.x);
    let half = std::f64::consts::FRAC_PI_2;
    if a > half + 1e-9 {
        a -= std::f64::consts::PI;
    } else if a <= -half + 1e-9 {
        a += std::f64::consts::PI;
    }
    a
}

fn lot_text(id: ElementId, at: DVec2, text: String) -> PlanItem {
    match ground_text(id, at, text) {
        PlanItem::Text {
            id,
            at,
            text,
            align,
            mm,
            rgb,
            ..
        } => PlanItem::Text {
            angle: 0.0,
            id,
            at,
            text,
            size: LOT_TEXT_MM,
            align,
            mm,
            rgb,
            bold: false,
        },
        it => it,
    }
}

/// A height written on a terrain. Drawn for a 1:100 drawing until
/// [`fit_ground_text`] sizes it for its view.
pub(crate) fn ground_text(id: ElementId, at: DVec2, text: String) -> PlanItem {
    PlanItem::Text {
        angle: 0.0,
        id,
        at,
        text,
        size: GROUND_TEXT_MM,
        align: crate::paper::Anchor::Middle,
        mm: 0.1,
        rgb: None,
        bold: false,
    }
}

/// Where a contour carries its height: the middle of its straightest
/// stretch, well inside the line. None for a line too short to carry one.
fn contour_label_at(pts: &[DVec2]) -> Option<DVec2> {
    let total: f64 = pts.windows(2).map(|w| w[0].distance(w[1])).sum();
    if total < 4.0 {
        return None;
    }
    // candidates along the middle half, the one where the line turns least
    let at = |d: f64| {
        let mut run = 0.0;
        for w in pts.windows(2) {
            let len = w[0].distance(w[1]);
            if run + len >= d && len > 0.0 {
                return w[0] + (w[1] - w[0]) * ((d - run) / len);
            }
            run += len;
        }
        pts[pts.len() - 1]
    };
    (0..=8)
        .map(|i| total * (0.25 + 0.0625 * i as f64))
        .map(|d| {
            let (a, m, b) = (at(d - 0.8), at(d), at(d + 0.8));
            // how far the middle sits off the chord, and off centre
            let bend = (b - a).perp_dot(m - a).abs() / (b - a).length().max(1e-9);
            (bend + (d / total - 0.5).abs() * 0.05, m)
        })
        .min_by(|x, y| x.0.total_cmp(&y.0))
        .map(|x| x.1)
}

/// The least a surface pattern's lines stand apart on paper, in mm. Closer
/// than this they print as a grey or black patch and not as a pattern
/// (2 mm was tried and still read as dense, owner MU125).
pub const PATTERN_MIN_MM: f64 = 4.0;

/// The least a surface pattern's lines stand apart on screen, in pixels.
pub const PATTERN_MIN_PX: f64 = 8.0;

/// The distance between a pattern's closest lines, in metres.
pub fn pattern_step(pattern: &Pattern) -> f64 {
    match *pattern {
        Pattern::None => f64::INFINITY,
        Pattern::Units { size, .. } => size.min_element(),
        Pattern::Lines { spacing, .. } | Pattern::Stipple { spacing } => spacing,
    }
}

/// `pattern` opened up by a whole number of its own steps, the fewest
/// that put its lines at least `least` metres apart.
fn opened(pattern: &Pattern, least: f64) -> Pattern {
    let step = pattern_step(pattern);
    if !step.is_finite() || step <= 0.0 || step >= least {
        return *pattern;
    }
    let n = (least / step - 1e-9).ceil().max(1.0);
    match *pattern {
        Pattern::None => Pattern::None,
        Pattern::Units {
            size,
            stagger,
            angle,
        } => Pattern::Units {
            size: size * n,
            stagger,
            angle,
        },
        Pattern::Lines { spacing, angle } => Pattern::Lines {
            spacing: spacing * n,
            angle,
        },
        Pattern::Stipple { spacing } => Pattern::Stipple {
            spacing: spacing * n,
        },
    }
}

/// What a plan shows of `pattern` on screen at `px` pixels a metre, when
/// its drawing holds it as `drawn` (the pattern at the view's scale):
/// `None` where the drawing's own lines stand far enough apart to be
/// painted as they are; otherwise the lines to paint in their place,
/// opened up to [`PATTERN_MIN_PX`]. None are left when the surface is too
/// small on screen to hold two courses of them: its tint says it then.
pub fn pattern_on_screen(
    drawn: &Pattern,
    loops: &[Vec<DVec2>],
    px: f64,
) -> Option<Vec<(DVec2, DVec2)>> {
    let step = pattern_step(drawn);
    if !step.is_finite() || px <= 0.0 || step * px >= PATTERN_MIN_PX {
        return None;
    }
    let shown = opened(drawn, PATTERN_MIN_PX / px);
    let outline = loops.first()?;
    let (mut lo, mut hi) = (DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY));
    for p in outline {
        lo = lo.min(*p);
        hi = hi.max(*p);
    }
    if (hi - lo).min_element() < 2.0 * pattern_step(&shown) {
        return Some(vec![]);
    }
    Some(pattern_lines(&shown, loops))
}

/// `pattern` as a drawing at 1:`scale` shows it: itself where its lines
/// stand at least [`PATTERN_MIN_MM`] apart on paper, and otherwise drawn
/// with every second, third… joint, the fewest left out that open it up.
/// Block paving at 1:50 shows every block; at 1:200 it shows the bond
/// with a few courses to the mark, as a draughtsman would hatch it.
pub fn pattern_at_scale(pattern: &Pattern, scale: f64) -> Pattern {
    opened(pattern, PATTERN_MIN_MM * scale / 1000.0)
}

/// Open up the patterns of paved and planted areas for a drawing at
/// 1:`scale`: where an area's pattern would print too fine, its lines give
/// way to those of [`pattern_at_scale`]. The area itself is not changed.
pub fn fit_site_patterns(model: &Model, items: &mut Vec<PlanItem>, scale: f64) {
    let mut done: Vec<ElementId> = vec![];
    let mut i = 0;
    while i < items.len() {
        let id = items[i].id();
        i += 1;
        if done.contains(&id) {
            continue;
        }
        let Some(ElementKind::Site(Site {
            form:
                SiteForm::Area {
                    boundary,
                    holes,
                    pattern,
                    ..
                },
            ..
        })) = model.get(id).map(|e| &e.kind)
        else {
            continue;
        };
        done.push(id);
        let shown = pattern_at_scale(pattern, scale);
        if shown == *pattern {
            continue;
        }
        let mut loops = vec![boundary.clone()];
        loops.extend(holes.iter().cloned());
        // its own lines, as the plan drew them, go...
        let mut first = None;
        let mut k = 0;
        items.retain(|it| {
            let at = k;
            k += 1;
            let own = matches!(it, PlanItem::Line { id: of, pen: Pen::Pattern, .. } if *of == id);
            if own {
                first.get_or_insert(at);
            }
            !own
        });
        // ...and the opened ones come in their place
        let Some(at) = first else {
            continue;
        };
        let fresh: Vec<PlanItem> = pattern_lines(&shown, &loops)
            .into_iter()
            .map(|(a, b)| PlanItem::Line {
                id,
                pen: Pen::Pattern,
                a,
                b,
            })
            .collect();
        let at = at.min(items.len());
        items.splice(at..at, fresh);
        i = 0;
    }
}

/// Size the heights written on terrains for a drawing at 1:`scale`, and
/// break the contours where a height sits on them.
pub fn fit_ground_text(model: &Model, items: &mut Vec<PlanItem>, scale: f64) {
    let mm = scale / 1000.0;
    let terrain = |id: ElementId| {
        matches!(
            model.get(id).map(|e| &e.kind),
            Some(ElementKind::Site(Site {
                form: SiteForm::Terrain { .. }
                    | SiteForm::Property { .. }
                    | SiteForm::Road(_)
                    | SiteForm::Drain(_)
                    | SiteForm::Tank(_)
                    | SiteForm::Investigation(_),
                ..
            }))
        )
    };
    let mut gaps: Vec<(ElementId, DVec2, f64)> = vec![];
    for it in items.iter_mut() {
        if let PlanItem::Text {
            id,
            at,
            text,
            size,
            mm: m,
            angle,
            ..
        } = it
        {
            if terrain(*id) && *size == LOT_NOTE_MM {
                // a lot's note: the line below its setback, at this scale
                *m = mm;
                let down = DVec2::new(angle.sin(), -angle.cos());
                *at += down * (LOT_TEXT_MM * 1.25 * mm);
                continue;
            }
            if terrain(*id) && (*size == GROUND_TEXT_MM || *size == LOT_TEXT_MM) {
                *m = mm;
                // a label's half width, with a little air
                let half = (text.chars().count() as f64 * 0.3 + 0.35) * *size * mm;
                gaps.push((*id, *at, half));
            }
        }
        // a level tag on the ground: its words lifted just over the point
        // it reads, for this scale (MU142), the contours broken under them
        if let PlanItem::Tag {
            id, at, text, look, ..
        } = it
        {
            if matches!(model.get(*id).map(|e| &e.kind), Some(ElementKind::Site(_))) {
                *at += DVec2::new(0.0, level_tag_lift(look, mm));
                let half = (look.width(text) / 2.0 + 0.35 * look.size) * mm;
                gaps.push((*id, *at, half));
            }
        }
    }
    if gaps.is_empty() {
        return;
    }
    let mut out = Vec::with_capacity(items.len());
    for it in items.drain(..) {
        let PlanItem::Line {
            id,
            pen: pen @ (Pen::Contour | Pen::ContourHeavy),
            a,
            b,
        } = it
        else {
            out.push(it);
            continue;
        };
        // the stretches of the line clear of every label on its terrain
        let mut keep: Vec<(f64, f64)> = vec![(0.0, 1.0)];
        let d = b - a;
        let len2 = d.length_squared();
        for (_, at, half) in gaps.iter().filter(|g| g.0 == id && len2 > 0.0) {
            // where the line is within `half` of the label
            let t0 = (*at - a).dot(d) / len2;
            let off2 = (a + d * t0).distance_squared(*at);
            if off2 >= half * half {
                continue;
            }
            let reach = ((half * half - off2) / len2).sqrt();
            let (lo, hi) = (t0 - reach, t0 + reach);
            keep = keep
                .into_iter()
                .flat_map(|(s, e)| [(s, e.min(lo)), (s.max(hi), e)])
                .filter(|(s, e)| e - s > 1e-9)
                .collect();
        }
        for (s, e) in keep {
            out.push(PlanItem::Line {
                id,
                pen,
                a: a + d * s,
                b: a + d * e,
            });
        }
    }
    *items = out;
}

/// The parts of the line `a`–`b` inside the region bounded by `loops`
/// (an outline and its holes, even–odd).
pub fn clip_line(loops: &[Vec<DVec2>], a: DVec2, b: DVec2) -> Vec<(DVec2, DVec2)> {
    let d = b - a;
    let mut ts = vec![];
    for lp in loops {
        let n = lp.len();
        for i in 0..n {
            let (p, q) = (lp[i], lp[(i + 1) % n]);
            let e = q - p;
            let den = d.perp_dot(e);
            if den.abs() < 1e-12 {
                continue;
            }
            let t = (p - a).perp_dot(e) / den;
            let u = (p - a).perp_dot(d) / den;
            if (0.0..1.0).contains(&u) && (0.0..=1.0).contains(&t) {
                ts.push(t);
            }
        }
    }
    ts.push(0.0);
    ts.push(1.0);
    ts.sort_by(f64::total_cmp);
    ts.dedup_by(|x, y| (*x - *y).abs() < 1e-9);
    let inside = |p: DVec2| {
        loops
            .iter()
            .filter(|l| creabim_core::polygon::contains(l, p))
            .count()
            % 2
            == 1
    };
    ts.windows(2)
        .filter(|w| w[1] - w[0] > 1e-9 && inside(a + d * ((w[0] + w[1]) / 2.0)))
        .map(|w| (a + d * w[0], a + d * w[1]))
        .collect()
}

/// The lines of a surface pattern over the region bounded by `loops`.
pub fn pattern_lines(pattern: &Pattern, loops: &[Vec<DVec2>]) -> Vec<(DVec2, DVec2)> {
    let Some(outline) = loops.first() else {
        return vec![];
    };
    // the pattern's own axes, and the outline's extent in them
    let (angle, step) = match *pattern {
        Pattern::None => return vec![],
        Pattern::Units { angle, size, .. } => (angle, size.min_element()),
        Pattern::Lines { angle, spacing } => (angle, spacing),
        Pattern::Stipple { spacing } => (0.0, spacing),
    };
    let u = DVec2::from_angle(angle.to_radians());
    let v = DVec2::new(-u.y, u.x);
    let (mut lo, mut hi) = (DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY));
    for p in outline {
        let q = DVec2::new(p.dot(u), p.dot(v));
        lo = lo.min(q);
        hi = hi.max(q);
    }
    let area = (hi - lo).element_product();
    // too fine a pattern for the area is left out rather than drawn black
    if step.is_nan() || step <= 0.0 || area / (step * step) > 200_000.0 {
        return vec![];
    }
    let world = |x: f64, y: f64| u * x + v * y;
    let mut out = vec![];
    let push = |a: DVec2, b: DVec2, out: &mut Vec<(DVec2, DVec2)>| {
        out.extend(clip_line(loops, a, b));
    };
    match *pattern {
        Pattern::None => {}
        Pattern::Lines { spacing, .. } => {
            let mut y = (lo.y / spacing).ceil() * spacing;
            while y < hi.y {
                push(world(lo.x, y), world(hi.x, y), &mut out);
                y += spacing;
            }
        }
        Pattern::Units { size, stagger, .. } => {
            // courses along u, joints across each course
            let mut row = (lo.y / size.y).floor() as i64;
            loop {
                let y0 = row as f64 * size.y;
                if y0 > hi.y {
                    break;
                }
                if y0 > lo.y {
                    push(world(lo.x, y0), world(hi.x, y0), &mut out);
                }
                let shift = if stagger && row.rem_euclid(2) == 1 {
                    size.x / 2.0
                } else {
                    0.0
                };
                let mut x = ((lo.x - shift) / size.x).floor() * size.x + shift;
                while x < hi.x {
                    if x > lo.x {
                        push(world(x, y0), world(x, y0 + size.y), &mut out);
                    }
                    x += size.x;
                }
                row += 1;
            }
        }
        Pattern::Stipple { spacing } => {
            // a jittered grid of dots, the same every time
            let mut j = 0u64;
            let mut y = (lo.y / spacing).floor() * spacing;
            while y < hi.y {
                let mut x = (lo.x / spacing).floor() * spacing;
                while x < hi.x {
                    j = j
                        .wrapping_mul(6364136223846793005)
                        .wrapping_add(1442695040888963407);
                    let rx = ((j >> 33) % 1000) as f64 / 1000.0;
                    let ry = ((j >> 13) % 1000) as f64 / 1000.0;
                    let p = world(x + rx * spacing, y + ry * spacing);
                    let inside = loops
                        .iter()
                        .filter(|l| creabim_core::polygon::contains(l, p))
                        .count()
                        % 2
                        == 1;
                    if inside {
                        let d = DVec2::splat(spacing * 0.06);
                        out.push((p - d, p + d));
                    }
                    x += spacing;
                }
                y += spacing;
            }
        }
    }
    out
}

/// A plant's plan symbol, `r` its canopy's radius: a scalloped crown for
/// a tree, a toothed one for a conifer, feathered fronds for a palm, an
/// uneven cloud for a shrub (Site plants proposal, round 2).
pub fn plant_symbol(out: &mut Vec<PlanItem>, id: ElementId, c: DVec2, form: PlantForm, r: f64) {
    let line = |out: &mut Vec<PlanItem>, pen, a, b| out.push(PlanItem::Line { id, pen, a, b });
    let turn = plant_turn(c);
    let dot = |out: &mut Vec<PlanItem>, radius: f64| {
        out.push(PlanItem::Arc {
            id,
            pen: Pen::Projection,
            center: c,
            radius,
            start: 0.0,
            end: TAU,
        })
    };
    // a ring of outward arcs through points round `c`, `uneven` of 1
    let scallops =
        |out: &mut Vec<PlanItem>, pen: Pen, radius: f64, lobes: usize, uneven: f64, rot: f64| {
            let at = |k: usize| {
                let a = rot + TAU * k as f64 / lobes as f64;
                let wob = ((k % lobes) as f64 * 2.399 + turn).sin() * uneven;
                c + DVec2::from_angle(a) * radius * (1.0 + wob)
            };
            for k in 0..lobes {
                let (p, q) = (at(k), at(k + 1));
                let chord = p.distance(q);
                let rho = chord * 0.6;
                let mid = (p + q) / 2.0;
                let away = (mid - c).normalize_or_zero();
                let o = mid - away * (rho * rho - chord * chord / 4.0).max(0.0).sqrt();
                let start = (p - o).to_angle();
                let mut end = (q - o).to_angle();
                while end <= start {
                    end += TAU;
                }
                out.push(PlanItem::Arc {
                    id,
                    pen,
                    center: o,
                    radius: rho,
                    start,
                    end,
                });
            }
        };
    match form {
        PlantForm::Tree => {
            let lobes = ((TAU * r / 1.1).round() as usize).clamp(8, 18);
            scallops(out, Pen::Projection, r * 0.93, lobes, 0.03, turn);
            // a finer ring inside, and a few forked branches
            if r > 0.9 {
                scallops(out, Pen::Fine, r * 0.56, 7, 0.05, turn + 0.4);
                for a in [0.3, 1.5, 2.7, 3.9, 5.2] {
                    let d = DVec2::from_angle(turn + a);
                    let tip = c + d * r * 0.5;
                    let fork = c + d * r * 0.32;
                    line(out, Pen::Fine, c, tip);
                    for s in [0.7_f64, -0.7] {
                        line(
                            out,
                            Pen::Fine,
                            fork,
                            fork + DVec2::from_angle(turn + a + s) * r * 0.15,
                        );
                    }
                }
            }
            dot(out, (r * 0.08).clamp(0.06, 0.35));
        }
        PlantForm::Shrub => {
            let lobes = ((TAU * r / 0.55).round() as usize).clamp(6, 12);
            scallops(out, Pen::Projection, r * 0.92, lobes, 0.1, turn);
            // a few short arcs inside: its clumps
            if r > 0.5 {
                for (a, far) in [(0.6, 0.3), (2.9, 0.28), (4.6, 0.22)] {
                    let o = c + DVec2::from_angle(turn + a) * r * far;
                    out.push(PlanItem::Arc {
                        id,
                        pen: Pen::Fine,
                        center: o,
                        radius: r * 0.26,
                        start: turn + a - 1.1,
                        end: turn + a + 1.3,
                    });
                }
            }
        }
        PlantForm::Conifer => {
            let toothed =
                |out: &mut Vec<PlanItem>, pen: Pen, tip: f64, root: f64, teeth: usize, rot: f64| {
                    let p = |k: usize| {
                        let rr = if k % 2 == 0 { tip } else { root };
                        c + DVec2::from_angle(rot + PI * k as f64 / teeth as f64) * rr
                    };
                    for k in 0..teeth * 2 {
                        out.push(PlanItem::Line {
                            id,
                            pen,
                            a: p(k),
                            b: p(k + 1),
                        });
                    }
                };
            let teeth = ((TAU * r / 0.55).round() as usize).clamp(9, 18);
            toothed(out, Pen::Projection, r, r * 0.83, teeth, turn);
            if r > 0.9 {
                toothed(out, Pen::Fine, r * 0.54, r * 0.42, 7, turn + 0.2);
                for k in 0..7 {
                    line(
                        out,
                        Pen::Fine,
                        c,
                        c + DVec2::from_angle(turn + 0.2 + TAU * k as f64 / 7.0) * r * 0.36,
                    );
                }
            }
            dot(out, (r * 0.07).clamp(0.05, 0.3));
        }
        PlantForm::Palm => {
            // feathered fronds fanning from the trunk, long and short
            let n = 9;
            for k in 0..n {
                let a = turn + TAU * k as f64 / n as f64;
                let reach = if k % 2 == 0 { r } else { r * 0.84 };
                let tip = c + DVec2::from_angle(a) * reach;
                let ctrl = c + DVec2::from_angle(a + 0.16) * reach * 0.55;
                let steps = 10;
                let mut left = vec![];
                let mut right = vec![];
                let mut rib = vec![];
                for i in 0..=steps {
                    let t = i as f64 / steps as f64;
                    let p = c * (1.0 - t) * (1.0 - t) + ctrl * 2.0 * (1.0 - t) * t + tip * t * t;
                    let tan = ((ctrl - c) * (1.0 - t) + (tip - ctrl) * t).normalize_or_zero();
                    let side = tan.perp();
                    let mut w = r
                        * 0.15
                        * ((PI * (t * 1.15).min(1.0)).sin().max(0.0)).powf(0.7)
                        * (1.0 - 0.35 * t);
                    if i % 2 == 1 {
                        w *= 0.55;
                    }
                    left.push(p + side * w);
                    right.push(p - side * w);
                    rib.push(p);
                }
                for w in left.windows(2).chain(right.windows(2)) {
                    line(out, Pen::Projection, w[0], w[1]);
                }
                for w in rib.windows(2) {
                    line(out, Pen::Fine, w[0], w[1]);
                }
            }
            dot(out, (r * 0.07).clamp(0.1, 0.25));
        }
    }
}

#[cfg(test)]
mod tests {

    /// MU142 (Mockup_Tags_And_Splay_REV00, A on all: ground levels in the
    /// site plan are tags the same way): each level read on the ground is
    /// a tag, in the annotation colour, its words alone just over the
    /// point with no leader while home; moved by hand in the view, drawn
    /// there with a leader back to its point. None is plain text.
    #[test]
    fn site_plan_levels_are_tags() {
        use crate::section::view_drawing;
        let mut m = creabim_core::ridgeline::ridgeline_house();
        let view = m.iter().find(|e| e.name == "Site plan").unwrap().id;
        let (scale, sv) = match &m.get(view).unwrap().kind {
            ElementKind::View(v) => (v.scale, v.site.clone()),
            _ => unreachable!(),
        };
        let mm = scale / 1000.0;
        let site = |m: &Model, id: ElementId| {
            matches!(m.get(id).map(|e| &e.kind), Some(ElementKind::Site(_)))
        };
        let tags = |m: &Model| -> Vec<(
            ElementId,
            DVec2,
            DVec2,
            String,
            crate::annotation::TextLook,
            Option<DVec2>,
        )> {
            view_drawing(m, view)
                .unwrap()
                .items
                .into_iter()
                .filter_map(|i| match i {
                    PlanItem::Tag {
                        id,
                        anchor,
                        at,
                        text,
                        look,
                        elbow,
                        ..
                    } if site(m, id) => Some((id, anchor, at, text, look, elbow)),
                    _ => None,
                })
                .collect()
        };
        let all = tags(&m);
        assert!(!all.is_empty(), "the site plan reads levels");
        for (_, anchor, at, text, look, elbow) in &all {
            assert!(look.rgb.is_none(), "{text}: the annotation colour");
            assert_eq!(look.shape, creabim_core::TagShape::TextOnly);
            // its words over its point; no leader while home
            assert!(at.y > anchor.y, "{text}");
            let g = crate::annotate::tag_geometry(*anchor, *at, *elbow, text, false, look, mm);
            let home = (*at - *anchor - DVec2::new(0.0, level_tag_lift(look, mm))).length() < 1e-9;
            assert!(!home || g.leader.is_empty(), "{text}: a leader at home");
        }
        // levels are no longer plain text
        let d = view_drawing(&m, view).unwrap();
        assert!(!d
            .items
            .iter()
            .any(|i| matches!(i, PlanItem::Text { text, id, .. }
            if site(&m, *id) && all.iter().any(|t| &t.3 == text))));
        // moved by hand in this view: drawn there, a leader back
        let (id, anchor, ..) = all[0].clone();
        let v = m.get(view).unwrap().clone();
        let mut moved = v.clone();
        if let ElementKind::View(x) = &mut moved.kind {
            x.site.set_spot_move(id, anchor, Some(DVec2::new(3.0, 2.0)));
        }
        creabim_core::Transaction::new("move")
            .replace(v, moved)
            .apply(&mut m)
            .unwrap();
        let now = tags(&m);
        let (_, a2, at2, text, look, elbow) = now
            .iter()
            .find(|t| t.0 == id && t.1.distance(anchor) < 1e-9)
            .unwrap();
        assert!(
            at2.distance(*a2 + DVec2::new(3.0, 2.0)) < 1.0,
            "moved: {at2} from {a2}"
        );
        let g = crate::annotate::tag_geometry(*a2, *at2, *elbow, text, false, look, mm);
        assert_eq!(
            g.leader.last().copied(),
            Some(*a2),
            "a leader back to its point"
        );
        let _ = sv;
    }

    #[test]
    fn a_view_shows_the_ground_before_or_after_its_works() {
        use creabim_core::{GroundState, Level, Pad, SiteView};
        let lv = Element::new(
            "Ground",
            ElementKind::Level(Level {
                elevation: 0.0,
                height: 3.0,
            }),
        );
        let mut t = Site::terrain(
            lv.id,
            creabim_core::polygon::rectangle(DVec2::ZERO, DVec2::new(40.0, 40.0)),
            0.0,
            1.0,
        );
        if let SiteForm::Terrain {
            points,
            data,
            contour,
            ..
        } = &mut t.form
        {
            for i in 0..=8 {
                for j in 0..=8 {
                    let (x, y) = (i as f64 * 5.0, j as f64 * 5.0);
                    points.push(DVec3::new(x, y, y * 0.1));
                }
            }
            data.survey = true;
            data.faceted = true;
            *contour = 0.25;
            data.pads.push(Pad::new(
                creabim_core::polygon::rectangle(DVec2::new(15.0, 15.0), DVec2::new(25.0, 25.0)),
                2.0,
            ));
        }
        let te = Element::new("Terrain", ElementKind::Site(t.clone()));
        let id = te.id;
        let m = Model::from_elements([lv, te]).unwrap();
        let count = |sv: &SiteView, f: &dyn Fn(&PlanItem) -> bool| {
            site_plan_with(&m, id, &t, sv)
                .iter()
                .filter(|i| f(i))
                .count()
        };
        let existing = |i: &PlanItem| {
            matches!(
                i,
                PlanItem::Line {
                    pen: Pen::Existing,
                    ..
                }
            )
        };
        let tint = |i: &PlanItem| matches!(i, PlanItem::Tint { .. });
        let heavy = |i: &PlanItem| {
            matches!(
                i,
                PlanItem::Line {
                    pen: Pen::ContourHeavy,
                    ..
                }
            )
        };
        let after = SiteView::default();
        assert_eq!(count(&after, &existing), 0);
        assert_eq!(count(&after, &tint), 0);
        let both = SiteView {
            ground: GroundState::Both,
            cut_fill: true,
            ..after.clone()
        };
        // the old contours dashed where the pad changed them, and the
        // ground coloured where it was dug and made up
        assert!(count(&both, &existing) > 0);
        assert!(count(&both, &tint) > 0);
        // before: the survey alone, no banks and no colours
        let before = SiteView {
            ground: GroundState::Before,
            cut_fill: true,
            ..after.clone()
        };
        assert_eq!(count(&before, &tint), 0);
        assert!(count(&before, &heavy) < count(&after, &heavy));
        // no contours: only the works' own edges stay heavy (a work's edge
        // is always drawn, Mockup_Site_Earthworks)
        let bare = SiteView {
            contours: false,
            banks: false,
            ..after.clone()
        };
        assert!(count(&bare, &heavy) > 0);
        assert!(count(&bare, &heavy) < count(&after, &heavy));
        let tick = |i: &PlanItem| {
            matches!(
                i,
                PlanItem::Line {
                    pen: Pen::Contour,
                    ..
                }
            )
        };
        // and unhatched, a bank has no ticks
        let unhatched = SiteView {
            contours: false,
            ..after.clone()
        };
        assert!(count(&bare, &tick) < count(&unhatched, &tick));
    }

    #[test]
    fn a_sweep_drawn_up_a_wall_face_stands_out_of_it() {
        // a path up and across a wall face looking -y, at y = 0
        let path = [
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(0.0, 0.0, 2.0),
            DVec3::new(1.5, 0.0, 2.0),
        ];
        let profile = vec![
            DVec2::new(-0.05, 0.0),
            DVec2::new(0.05, 0.0),
            DVec2::new(0.05, 0.1),
            DVec2::new(-0.05, 0.1),
        ];
        let m = sweep_mesh_on(&profile, &path, false, -DVec3::Y);
        assert!(m.triangle_count() > 0);
        for v in &m.positions {
            // out of the face (toward -y) by up to the profile's height
            assert!(v[1] <= 1e-6 && v[1] >= -0.1 - 1e-6, "{v:?}");
        }
        // closed and outward, as long as the path: 2 m + 1.5 m of a 0.1 x 0.1 bar
        let vol: f64 = m
            .indices
            .chunks(3)
            .map(|t| {
                let p = |k: usize| glam::Vec3::from(m.positions[t[k] as usize]).as_dvec3();
                p(0).dot(p(1).cross(p(2))) / 6.0
            })
            .sum();
        assert!((vol - 0.035).abs() < 2e-3, "{vol}");
    }

    #[test]
    fn an_extrusion_on_a_sloped_face_stands_square_to_it() {
        let level = Element::new(
            "L",
            ElementKind::Level(creabim_core::Level {
                elevation: 2.0,
                height: 3.0,
            }),
        );
        let lid = level.id;
        let m = Model::from_elements(vec![level]).unwrap();
        // a 30° slope rising along y, through (0, 0, 1) above the level
        let a = 30f64.to_radians();
        let n = DVec3::new(0.0, -a.sin(), a.cos());
        let pl = creabim_core::Workplane::facing(DVec3::new(0.0, 0.0, 1.0), n);
        let mut p = Part::extrusion(
            lid,
            creabim_core::polygon::rectangle(DVec2::ZERO, DVec2::ONE),
            -0.4,
            0.5,
        );
        if let Form::Extrusion { plane, .. } = &mut p.form {
            *plane = Some(pl);
        }
        p.validate().unwrap();
        let mesh = part_mesh(&m, &p).unwrap();
        // every corner is 0.4 under the face or 0.1 over it, along its normal
        for v in &mesh.positions {
            let q = glam::Vec3::from(*v).as_dvec3() - DVec3::new(0.0, 0.0, 3.0);
            let h = q.dot(pl.normal());
            assert!((h + 0.4).abs() < 1e-5 || (h - 0.1).abs() < 1e-5, "{h}");
        }
        // and the solid is closed and outward: its volume is the box's
        let vol: f64 = mesh
            .indices
            .chunks(3)
            .map(|t| {
                let p = |k: usize| glam::Vec3::from(mesh.positions[t[k] as usize]).as_dvec3();
                p(0).dot(p(1).cross(p(2))) / 6.0
            })
            .sum();
        assert!((vol - 0.5).abs() < 1e-4, "{vol}");
        let (lo, hi) = p.bounds(&m).unwrap();
        assert!(lo.z < 3.0 && hi.z > 3.5, "{lo} {hi}");
    }

    use super::*;
    use creabim_core::parts::{self, Path};
    use creabim_core::{Level, Wall};

    fn level() -> Element {
        Element::new(
            "L0",
            ElementKind::Level(Level {
                elevation: 0.0,
                height: 3.0,
            }),
        )
    }

    #[test]
    fn a_swept_coping_is_closed_and_as_big_as_its_profile() {
        let profile = parts::rectangle(0.3, 0.05);
        let path = [DVec3::ZERO, DVec3::new(4.0, 0.0, 0.0)];
        let m = sweep_mesh(&profile, &path, false);
        assert!((m.volume() - 0.3 * 0.05 * 4.0).abs() < 1e-4);
        // round an L, mitred: the centreline length times the section
        let path = [
            DVec3::ZERO,
            DVec3::new(4.0, 0.0, 0.0),
            DVec3::new(4.0, 3.0, 0.0),
        ];
        let m = sweep_mesh(&profile, &path, false);
        assert!(
            (m.volume() - 0.3 * 0.05 * 7.0).abs() < 1e-4,
            "{}",
            m.volume()
        );
        // a closed ring: no caps, still closed
        let ring = [
            DVec3::ZERO,
            DVec3::new(4.0, 0.0, 0.0),
            DVec3::new(4.0, 3.0, 0.0),
            DVec3::new(0.0, 3.0, 0.0),
        ];
        let m = sweep_mesh(&profile, &ring, true);
        assert!(
            (m.volume() - 0.3 * 0.05 * 14.0).abs() < 1e-4,
            "{}",
            m.volume()
        );
    }

    #[test]
    fn steps_as_one_element() {
        let lv = level();
        let p = Part::sweep(
            lv.id,
            parts::steps(4, 0.17, 0.28),
            Path::Points {
                points: vec![DVec3::ZERO, DVec3::new(1.2, 0.0, 0.0)],
                closed: false,
                normal: None,
            },
        );
        let m = Model::from_elements([lv]).unwrap();
        let mesh = part_mesh(&m, &p).unwrap();
        let want = 0.17 * 0.28 * (4 + 3 + 2 + 1) as f64 * 1.2;
        assert!((mesh.volume() - want).abs() < 1e-4, "{}", mesh.volume());
        let (_, hi) = mesh.bounds().unwrap();
        assert!((hi.z as f64 - 0.68).abs() < 1e-5);
    }

    #[test]
    fn a_coping_sits_on_its_wall_in_plan_and_3d() {
        let lv = level();
        let w = Element::new(
            "W",
            ElementKind::Wall(Wall::new(
                lv.id,
                DVec2::ZERO,
                DVec2::new(3.0, 0.0),
                0.2,
                1.0,
            )),
        );
        let c = Element::new(
            "Coping",
            ElementKind::Part(Part::sweep(
                lv.id,
                parts::coping(0.2, 0.03, 0.05),
                Path::Wall {
                    wall: w.id,
                    edge: parts::WallEdge::Top,
                    offset: 0.0,
                    extend: [0.0, 0.0],
                },
            )),
        );
        let m = Model::from_elements([lv.clone(), w, c.clone()]).unwrap();
        let (mesh, _) = mesh_of(&m, &c).unwrap();
        let (lo, hi) = mesh.bounds().unwrap();
        assert!((lo.z - 1.0).abs() < 1e-5 && (hi.z - 1.05).abs() < 1e-5);
        assert!((hi.y - 0.13).abs() < 1e-5);
        // a plan cut through the coping's height shows it cut
        let items = plan_items(&m, &c, 1.02);
        assert!(items
            .iter()
            .any(|i| matches!(i, PlanItem::Fill { id, .. } if *id == c.id)));
        // a cut below it shows it overhead, dashed
        let items = plan_items(&m, &c, 0.5);
        assert!(!items.is_empty());
        assert!(items.iter().all(|i| matches!(
            i,
            PlanItem::Line {
                pen: Pen::Hidden,
                ..
            }
        )));
    }

    #[test]
    fn a_hill_shows_its_contours_in_3d_and_whole_in_plan() {
        let lv = level();
        let level_id = lv.id;
        let mut t = Site::terrain(
            lv.id,
            creabim_core::polygon::rectangle(DVec2::ZERO, DVec2::new(10.0, 10.0)),
            0.0,
            0.5,
        );
        if let SiteForm::Terrain {
            points, contour, ..
        } = &mut t.form
        {
            // a hill that climbs through an ordinary plan's cut
            points.push(DVec3::new(5.0, 5.0, 3.0));
            *contour = 0.5;
        }
        let e = Element::new("Ground", ElementKind::Site(t));
        let m = Model::from_elements([lv, e.clone()]).unwrap();
        // the contours lie on the ground in 3D, apart from its solid, and
        // drawings and exports leave them out
        let items = scene_items(&m, &e);
        assert_eq!(items.len(), 2);
        assert!(!items[0].is_lining() && items[0].mesh.is_closed());
        assert!(items[1].is_lining() && !items[1].mesh.is_empty());
        let top = items[1]
            .mesh
            .positions
            .iter()
            .fold(0.0f32, |a, p| a.max(p[2]));
        assert!(top > 2.0 && top < 3.0, "{top}");
        // the plan's cut does not slice the hill: its whole outline is tinted
        let plan = crate::plan::plan_view(&m, level_id, 1.2);
        let shade: Vec<_> = plan
            .items
            .iter()
            .filter_map(|i| match i {
                PlanItem::Shade { points, .. } if i.id() == e.id => Some(points.len()),
                _ => None,
            })
            .collect();
        assert_eq!(shade, vec![4]);
    }

    #[test]
    fn terrain_follows_its_spot_heights() {
        let lv = level();
        let mut t = Site::terrain(
            lv.id,
            creabim_core::polygon::rectangle(DVec2::ZERO, DVec2::new(10.0, 10.0)),
            0.0,
            0.5,
        );
        if let SiteForm::Terrain {
            points, contour, ..
        } = &mut t.form
        {
            points.push(DVec3::new(5.0, 5.0, 2.0));
            *contour = 0.5;
        }
        let e = Element::new("Ground", ElementKind::Site(t.clone()));
        let m = Model::from_elements([lv, e.clone()]).unwrap();
        let tops = terrain_top(&m, &t);
        // the spot height became a corner of the surface
        assert!(tops.iter().flatten().any(|p| (p.z - 2.0).abs() < 1e-9));
        let (mesh, _) = mesh_of(&m, &e).unwrap();
        assert!(mesh.volume() > 10.0 * 10.0 * 0.5);
        // three contours (0.5, 1.0, 1.5) between the corners and the top
        let g = crate::ground::of(&t).unwrap();
        let lines = g.contours(0.5, 0.0);
        assert_eq!(lines.iter().map(|c| c.index).max(), Some(3));
        assert!(mesh.is_closed());
        let items = plan_items(&m, &e, 1.2);
        assert!(items.len() > 4);
        // asked for, the heavier contours carry their height, and the
        // line breaks round it at the drawing's scale
        if let SiteForm::Terrain { data, .. } = &mut t.form {
            data.labels = true;
            data.major = 2;
        }
        let e = Element::new("Ground", ElementKind::Site(t));
        let m = Model::from_elements([m.iter().next().unwrap().clone(), e.clone()]).unwrap();
        let mut items = plan_items(&m, &e, 1.2);
        let texts = |items: &[PlanItem]| {
            items
                .iter()
                .filter_map(|i| match i {
                    PlanItem::Text { text, mm, at, .. } => Some((text.clone(), *mm, *at)),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let before = texts(&items);
        assert_eq!(before.len(), 1, "{before:?}");
        fit_ground_text(&m, &mut items, 200.0);
        let after = texts(&items);
        assert_eq!(after[0].1, 0.2);
        let clear = items.iter().all(|i| match i {
            PlanItem::Line {
                pen: Pen::ContourHeavy,
                a,
                b,
                ..
            } => ((*a + *b) / 2.0).distance(after[0].2) > 0.2,
            _ => true,
        });
        assert!(clear);
    }

    #[test]
    fn paving_patterns_stay_inside_the_area() {
        let sq = creabim_core::polygon::rectangle(DVec2::ZERO, DVec2::new(2.0, 1.0));
        let hole = creabim_core::polygon::rectangle(DVec2::new(0.5, 0.25), DVec2::new(1.0, 0.75));
        let loops = vec![sq.clone(), hole];
        let lines = pattern_lines(
            &Pattern::Units {
                size: DVec2::new(0.4, 0.2),
                stagger: true,
                angle: 0.0,
            },
            &loops,
        );
        assert!(!lines.is_empty());
        for (a, b) in &lines {
            let m = (*a + *b) / 2.0;
            assert!(creabim_core::polygon::contains(&sq, m));
            assert!(!(m.x > 0.5 && m.x < 1.0 && m.y > 0.25 && m.y < 0.75));
        }
        assert!(!pattern_lines(&Pattern::Stipple { spacing: 0.1 }, &loops).is_empty());
        assert!(pattern_lines(&Pattern::None, &loops).is_empty());
    }

    #[test]
    fn plants_draw_a_symbol_and_a_form() {
        let lv = level();
        for form in PlantForm::ALL {
            let p = Element::new(
                form.label(),
                ElementKind::Site(Site::plant(lv.id, DVec2::ZERO, form, 4.0, 6.0)),
            );
            let m = Model::from_elements([lv.clone(), p.clone()]).unwrap();
            assert!(!plan_items(&m, &p, 1.2).is_empty());
            let items = scene_items(&m, &p);
            assert!(!items.is_empty());
            let (_, hi) = mesh_of(&m, &p).unwrap().0.bounds().unwrap();
            assert!(hi.z > 1.0 && hi.z < 6.5, "{form:?} {hi}");
        }
    }
}

#[cfg(test)]
mod plan_look_tests {
    use super::*;
    use creabim_core::parts::Path;

    fn level_model() -> (Model, ElementId) {
        let level = Element::new(
            "L",
            ElementKind::Level(creabim_core::Level {
                elevation: 0.0,
                height: 3.0,
            }),
        );
        let id = level.id;
        (Model::from_elements(vec![level]).unwrap(), id)
    }

    /// A step 900 wide and 510 high, swept round a tight curve.
    fn curved_step(level: ElementId, void: bool) -> Element {
        let points: Vec<DVec3> = (0..=24)
            .map(|i| {
                let a = PI * i as f64 / 24.0;
                DVec3::new(2.0 * a.cos(), 2.0 * a.sin(), 0.0)
            })
            .collect();
        let profile = vec![
            DVec2::new(-0.45, 0.0),
            DVec2::new(0.45, 0.0),
            DVec2::new(0.45, 0.51),
            DVec2::new(-0.45, 0.51),
        ];
        Element::new(
            "Steps",
            ElementKind::Part(Part {
                level,
                form: Form::Sweep {
                    profile,
                    path: Path::Points {
                        points,
                        closed: false,
                        normal: None,
                    },
                },
                void,
                material: None,
                ifc_class: creabim_core::parts::PROXY.into(),
                profile: None,
            }),
        )
    }

    #[test]
    fn a_curved_sweep_draws_no_ribs_and_a_void_draws_dashed() {
        // MU23: the seams between a curved sweep's segments drew as short
        // ribs; a void drew nothing at all in plan
        let (m, level) = level_model();
        let step = curved_step(level, false);
        let items = plan_items(&m, &step, 1.2);
        let ribs = items
            .iter()
            // across the step, away from its two ends (whose end faces are
            // real edges, at y = 0)
            .filter(|i| {
                matches!(i, PlanItem::Line { a, b, .. }
                if (a.length() - b.length()).abs() > 0.5 && (a.y + b.y) / 2.0 > 0.1)
            })
            .count();
        assert_eq!(
            ribs, 0,
            "lines across the step, from its inner edge to its outer one"
        );
        assert!(!items.is_empty(), "its edges are drawn");
        let void = curved_step(level, true);
        let items = plan_items(&m, &void, 1.2);
        assert!(!items.is_empty(), "a void shows in plan");
        assert!(items.iter().all(|i| matches!(
            i,
            PlanItem::Line {
                pen: Pen::Hidden,
                ..
            }
        )));
    }

    #[test]
    fn a_profile_drawn_clockwise_still_faces_out() {
        // a 0.2 x 0.3 box, clockwise, along 2 m
        let cw = vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(0.0, 0.3),
            DVec2::new(0.2, 0.3),
            DVec2::new(0.2, 0.0),
        ];
        let m = sweep_mesh(&cw, &[DVec3::ZERO, DVec3::new(2.0, 0.0, 0.0)], false);
        let v = |i: u32| glam::Vec3::from(m.positions[i as usize]);
        let mut volume = 0.0;
        for t in m.indices.chunks(3) {
            let (a, b, c) = (v(t[0]), v(t[1]), v(t[2]));
            volume += a.dot(b.cross(c)) / 6.0;
            // each face's normal agrees with the way round it is wound
            let n = glam::Vec3::from(m.normals[t[0] as usize]);
            assert!(n.dot((b - a).cross(c - a)) > 0.0, "a face turned in");
        }
        assert!((volume - 0.12).abs() < 1e-4, "the solid is whole: {volume}");
    }
}

/// A contour's height as it is written along it: a site level with no
/// trailing zeros, `100` or `99.5`.
fn contour_level(model: &Model, z: f64) -> String {
    let t = creabim_core::site_level(model, z);
    if t.contains('.') && !t.ends_with('\'') {
        t.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        t
    }
}

/// Whether `p` is inside a building's slab, in plan.
fn under_building(model: &Model, p: DVec2) -> bool {
    model.iter().any(|e| match &e.kind {
        ElementKind::Slab(sl) => crate::plan::point_in(p, &sl.boundary),
        _ => false,
    })
}
