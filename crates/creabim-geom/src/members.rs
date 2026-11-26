//! Meshes and plan linework for columns, beams, roofs, stairs and railings
//! (see `creabim_core::members`).

use crate::plan::{Pen, PlanItem};
use crate::scene::slab_mesh;
use crate::triangulate::triangulate;
use crate::TriMesh;
use creabim_core::members::{self, Flight, RAIL_SIZE};
use creabim_core::{Element, ElementId, ElementKind, Model, Roof, Stair};
use glam::{DVec2, DVec3};

/// A beam: its section (a library one, or width × depth) swept along its
/// top line, sloped by its rise, lying on the face it was drawn on, run
/// past or pulled back by its extensions and its ends cut where it meets
/// the next beam of its chain (MU40).
pub fn beam_mesh(model: &Model, b: &creabim_core::Beam) -> TriMesh {
    let prof: Vec<DVec2> = match &b.section {
        Some(s) if s.points.len() >= 3 => {
            let hi = s.points.iter().map(|p| p.y).fold(f64::MIN, f64::max);
            s.points.iter().map(|p| DVec2::new(p.x, p.y - hi)).collect()
        }
        _ => {
            let (w, d) = (b.width / 2.0, b.depth);
            vec![
                DVec2::new(-w, -d),
                DVec2::new(w, -d),
                DVec2::new(w, 0.0),
                DVec2::new(-w, 0.0),
            ]
        }
    };
    let (a, z) = b.axis(model);
    let mut m = match b.up {
        Some(n) => crate::parts::sweep_mesh_on(&prof, &[a, z], false, n),
        None => crate::parts::sweep_mesh(&prof, &[a, z], false),
    };
    // each end cut by its plane, through the corner it meets the next at:
    // the end's vertices slid along the beam onto the plane
    let dir = (z - a).normalize_or_zero();
    if dir == DVec3::ZERO {
        return m;
    }
    let top = b.top(model);
    let corners = [
        b.start.extend(top + b.rise[0]),
        b.end.extend(top + b.rise[1]),
    ];
    let len = a.distance(z);
    for (k, cut) in b.cuts.iter().enumerate() {
        let Some(n) = cut.map(|n| n.normalize_or_zero()) else {
            continue;
        };
        let along = n.dot(dir);
        if along.abs() < 0.05 {
            continue;
        }
        // a two-point sweep has only its two end rings: a vertex belongs
        // to the end it is nearer (MU40: on a sloped beam the section's
        // top and bottom sit off the end's plane square to the axis, and
        // were left uncut, ragged)
        let at_end = |p: DVec3| {
            let s = (p - a).dot(dir);
            if k == 0 {
                s < len / 2.0
            } else {
                s >= len / 2.0
            }
        };
        let out = if k == 0 { -dir } else { dir };
        for (p, nm) in m.positions.iter_mut().zip(m.normals.iter_mut()) {
            let q = DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64);
            if !at_end(q) {
                continue;
            }
            let t = (corners[k] - q).dot(n) / along;
            *p = (q + dir * t).as_vec3().to_array();
            // the end face takes the plane's normal, facing out
            let nv = DVec3::new(nm[0] as f64, nm[1] as f64, nm[2] as f64);
            if nv.dot(out) > 0.9 {
                let face = if n.dot(out) >= 0.0 { n } else { -n };
                *nm = face.as_vec3().to_array();
            }
        }
    }
    m
}

/// The mesh and default colour of a member; `None` for other elements.
pub fn member_mesh(model: &Model, e: &Element) -> Option<(TriMesh, [f32; 3])> {
    Some(match &e.kind {
        ElementKind::Column(c) => {
            let (z0, z1) = c.span(model);
            (slab_mesh(&c.outline(), z1, z1 - z0), [0.78, 0.76, 0.72])
        }
        // a library section swept along the beam, hung so its top is the
        // beam's top (MU37)
        ElementKind::Beam(b) => (beam_mesh(model, b), [0.72, 0.64, 0.52]),
        ElementKind::Roof(r) => {
            let holes = creabim_core::slab::holes_of(model, e);
            (
                cut_holes(
                    roof_mesh_with(r, r.base(model), &roof_holes(model, e.id)),
                    &holes,
                ),
                [0.66, 0.34, 0.26],
            )
        }
        ElementKind::Stair(s) => (
            stair_mesh(s, s.base(model), s.rise(model)),
            [0.82, 0.80, 0.76],
        ),
        ElementKind::Railing(r) => {
            let (mut m, glass) = railing_parts(model, r);
            m.append(&glass);
            (m, [0.32, 0.34, 0.38])
        }
        ElementKind::Footing(f) => {
            let top = f.top(model);
            let mut m = TriMesh::default();
            for piece in creabim_core::footing::outlines(model, e, None) {
                m.append(&slab_mesh(&piece, top, f.thickness));
            }
            (m, [0.66, 0.66, 0.64])
        }
        ElementKind::Part(_) | ElementKind::Site(_) => crate::parts::mesh_of(model, e)?,
        _ => return None,
    })
}

/// A mesh with each hole (a polygon in plan) cut upright through it.
pub fn cut_holes(mesh: TriMesh, holes: &[Vec<DVec2>]) -> TriMesh {
    let Some((lo, hi)) = mesh.bounds() else {
        return mesh;
    };
    let (z0, z1) = (lo.z as f64 - 1.0, hi.z as f64 + 1.0);
    holes.iter().filter(|h| h.len() >= 3).fold(mesh, |m, h| {
        let tool = slab_mesh(h, z1, z1 - z0);
        crate::scene::boolean_mesh(&m, &tool, creabim_core::BoolOp::Difference)
    })
}

/// A roof as a closed solid: each flat piece of its underside, raised by the
/// roof's upright thickness for the top, with upright edges round the eaves.
pub fn roof_mesh(r: &Roof, base: f64) -> TriMesh {
    roof_layer_mesh(r, base, 0.0, r.vertical_thickness(), None)
}

/// One layer of a layered roof: the part of the roof from `from` to `to`
/// above its underside, measured upright.
pub fn roof_layer_mesh(r: &Roof, base: f64, from: f64, to: f64, layer: Option<usize>) -> TriMesh {
    roof_layer_mesh_with(r, base, from, to, layer, &[])
}

/// A hole a roof window needs through its roof: its outline where its
/// lining starts, each corner carried into the roof along its own
/// direction (a splayed lining's sill plumb, its head level) and out of it
/// square to the slope.
#[derive(Clone, Copy, Debug)]
pub struct RoofHole {
    pub corners: [DVec3; 4],
    pub out: DVec3,
    /// Into the roof from each corner.
    pub inward: [DVec3; 4],
}

/// The holes for the roof windows a roof hosts.
pub fn roof_holes(model: &Model, roof: ElementId) -> Vec<RoofHole> {
    creabim_core::roofwindow::windows_in(model, roof)
        .into_iter()
        .filter_map(|(_, o)| {
            let s = creabim_core::roofwindow::seat(model, o)?;
            // where its lining starts, and how far its head and sill reach
            // by the underside
            let (y0, head, sill) =
                crate::opening::roof_lining_reach(model, o).unwrap_or((0.0, 0.0, 0.0));
            let c = |x: f64, z: f64| s.at(DVec3::new(x, y0, z));
            let d = s.thickness / 2.0 - y0;
            let dir = |dz: f64| (s.at(DVec3::new(0.0, d, dz)) - s.at(DVec3::ZERO)).normalize();
            Some(RoofHole {
                corners: [
                    c(0.0, 0.0),
                    c(o.width, 0.0),
                    c(o.width, o.height),
                    c(0.0, o.height),
                ],
                out: s.out,
                inward: [dir(sill), dir(sill), dir(head), dir(head)],
            })
        })
        .collect()
}

/// A roof as [`roof_mesh`] builds it, with roof windows' holes through it.
pub fn roof_mesh_with(r: &Roof, base: f64, holes: &[RoofHole]) -> TriMesh {
    roof_layer_mesh_with(r, base, 0.0, r.vertical_thickness(), None, holes)
}

/// [`roof_layer_mesh`] with roof windows' holes through it, each square to
/// the slope: left out of the faces it falls in, and lined round with the
/// hole's sides.
pub fn roof_layer_mesh_with(
    r: &Roof,
    base: f64,
    from: f64,
    to: f64,
    layer: Option<usize>,
    holes: &[RoofHole],
) -> TriMesh {
    let mut m = TriMesh::default();
    let tv = to - from;
    // a layer whose edges were moved by hand has eaves of its own
    let (eaves, faces) = match layer {
        Some(i) => (r.layer_eaves(i), r.layer_faces(i)),
        None => (r.eave_outline(), r.faces()),
    };
    let on_eaves = |a: DVec2, b: DVec2| {
        let mid = (a + b) / 2.0;
        let n = eaves.len();
        (0..n).any(|i| {
            let (p, q) = (eaves[i], eaves[(i + 1) % n]);
            let d = q - p;
            let t = ((mid - p).dot(d) / d.length_squared().max(1e-12)).clamp(0.0, 1.0);
            mid.distance(p + d * t) < 1e-6
                && (b - a)
                    .normalize_or_zero()
                    .perp_dot(d.normalize_or_zero())
                    .abs()
                    < 1e-6
        })
    };
    for f in faces {
        let pts = &f.outline;
        // where a hole meets the plane at `off` over the base, in plan:
        // its corners carried along `out` onto it
        let onto = |h: &RoofHole, off: f64| -> Vec<DVec2> {
            let k = base + off + f.c;
            // along `d` from `c` to the plane, and how far
            let hit = |c: DVec3, d: DVec3| {
                let den = d.z - f.g.dot(d.truncate());
                let t = if den.abs() < 1e-9 {
                    0.0
                } else {
                    (k + f.g.dot(c.truncate()) - c.z) / den
                };
                (t, c + d * t)
            };
            h.corners
                .iter()
                .zip(h.inward)
                .map(|(c, d)| {
                    // deeper than the corner: along its own way in; nearer
                    // the roofing: square to the slope
                    let (t, p) = hit(*c, d);
                    if t >= -1e-9 { p } else { hit(*c, h.out).1 }.truncate()
                })
                .collect()
        };
        // the holes in this face: those whose middle it is over, that lie
        // wholly in it and clear of the ones before them (one that would
        // run off its slope or into another window is left uncut)
        let mut mine: Vec<&RoofHole> = vec![];
        let mut taken: Vec<(Vec<DVec2>, Vec<DVec2>)> = vec![];
        for h in holes {
            let mid = h.corners.iter().copied().sum::<DVec3>() / 4.0;
            if pts.len() < 3 || !creabim_core::polygon::contains(pts, mid.truncate()) {
                continue;
            }
            let (a, b) = (onto(h, from), onto(h, to));
            let inside = a
                .iter()
                .chain(&b)
                .all(|p| creabim_core::polygon::contains(pts, *p));
            let clear = taken.iter().all(|(ta, tb)| {
                !creabim_core::polygon::convex_overlap(&a, ta)
                    && !creabim_core::polygon::convex_overlap(&b, tb)
            });
            if inside && clear {
                taken.push((a, b));
                mine.push(h);
            }
        }
        let up = DVec3::new(-f.g.x, -f.g.y, 1.0);
        let at = |p: DVec2, off: f64| p.extend(base + off + f.z(p));
        for (off, n) in [(to, up), (from, -up)] {
            let rings: Vec<Vec<DVec2>> = mine.iter().map(|h| onto(h, off)).collect();
            let ring = if rings.is_empty() {
                pts.clone()
            } else {
                creabim_core::polygon::bridged(pts, &rings)
            };
            let tris = triangulate(&ring);
            let p3: Vec<DVec3> = ring.iter().map(|p| at(*p, off)).collect();
            m.add_polygon(&p3, &tris, n);
        }
        // each hole's sides, facing into it
        for h in &mine {
            let (a, b) = (onto(h, from), onto(h, to));
            let mid = a.iter().copied().sum::<DVec2>() / 4.0;
            for i in 0..4 {
                let j = (i + 1) % 4;
                let e = (a[j] - a[i]).normalize_or_zero();
                let mut inward = e.perp();
                if inward.dot(mid - a[i]) < 0.0 {
                    inward = -inward;
                }
                m.add_quad(
                    [at(a[i], from), at(a[j], from), at(b[j], to), at(b[i], to)],
                    inward.extend(0.0),
                );
            }
        }
        let lo: Vec<DVec3> = pts
            .iter()
            .map(|p| p.extend(base + from + f.z(*p)))
            .collect();
        let hi: Vec<DVec3> = lo.iter().map(|p| *p + DVec3::Z * tv).collect();
        let ccw = crate::triangulate::signed_area2(pts) > 0.0;
        let n = pts.len();
        for i in 0..n {
            let j = (i + 1) % n;
            if !on_eaves(pts[i], pts[j]) {
                continue;
            }
            let e = (pts[j] - pts[i]).normalize_or_zero();
            let out = if ccw { -e.perp() } else { e.perp() };
            m.add_quad([lo[i], lo[j], hi[j], hi[i]], out.extend(0.0));
        }
    }
    m
}

/// A side profile (along, up) extruded `width` across, centred on `origin`.
fn extrude_profile(m: &mut TriMesh, profile: &[DVec2], origin: DVec3, dir: DVec2, width: f64) {
    let side = dir.perp();
    let at = |p: DVec2, v: f64| origin + (dir * p.x + side * v).extend(p.y);
    let tris = triangulate(profile);
    let left: Vec<DVec3> = profile.iter().map(|p| at(*p, width / 2.0)).collect();
    let right: Vec<DVec3> = profile.iter().map(|p| at(*p, -width / 2.0)).collect();
    m.add_polygon(&left, &tris, side.extend(0.0));
    m.add_polygon(&right, &tris, -side.extend(0.0));
    let ccw = crate::triangulate::signed_area2(profile) > 0.0;
    let n = profile.len();
    for i in 0..n {
        let j = (i + 1) % n;
        let e = profile[j] - profile[i];
        if e.length() < 1e-9 {
            continue;
        }
        let out = if ccw {
            DVec2::new(e.y, -e.x)
        } else {
            DVec2::new(-e.y, e.x)
        };
        let normal = (dir * out.x).extend(out.y);
        m.add_quad([right[i], right[j], left[j], left[i]], normal);
    }
}

/// The side profile of a flight of `risers` (along x from its first
/// riser, z up from its foot), its underside `waist` square to the slope.
pub fn flight_profile(risers: u32, riser: f64, tread: f64, waist: f64) -> Vec<DVec2> {
    flight_profile_joined(risers, riser, tread, waist, false, false)
}

/// The waist measured straight down: how far the underside of a flight
/// lies below the line through its inner corners. A landing is this
/// thick, so the soffits of the flights meet it in a line.
pub fn vertical_waist(riser: f64, tread: f64, waist: f64) -> f64 {
    let slope = riser / tread.max(1e-6);
    waist * (1.0 + slope * slope).sqrt()
}

/// [`flight_profile`] for a flight that leaves a landing (`from_landing`)
/// or climbs to one (`to_landing`): its underside runs on as one plane
/// to meet the landing's underside, with no notch under the joint. A
/// flight leaving a landing starts at the landing's underside; one
/// climbing to a landing takes a wedge one going in under it, where its
/// underside reaches the landing's (a landing being [`vertical_waist`]
/// thick). The pieces touch and never overlap, so a section cuts each
/// as a clean outline.
pub fn flight_profile_joined(
    risers: u32,
    riser: f64,
    tread: f64,
    waist: f64,
    from_landing: bool,
    to_landing: bool,
) -> Vec<DVec2> {
    let n = risers.max(1);
    if n == 1 {
        return vec![
            DVec2::ZERO,
            DVec2::new(tread, 0.0),
            DVec2::new(tread, riser),
            DVec2::new(0.0, riser),
        ];
    }
    let run = (n - 1) as f64 * tread;
    let slope = riser / tread;
    // the underside runs parallel to the line through the inner corners
    let wv = vertical_waist(riser, tread, waist);
    let u0 = wv / slope;
    let mut p = vec![DVec2::new(0.0, riser)];
    if from_landing {
        // down to the landing's underside, then up the slope from there
        p.push(DVec2::new(0.0, -wv));
    } else {
        p.push(DVec2::ZERO);
    }
    if to_landing {
        // on under the landing until the underside meets the landing's
        let end = run + tread;
        if !from_landing && u0 < end - 1e-9 {
            p.push(DVec2::new(u0, 0.0));
        }
        // a wedge under the landing, up to its underside (not into it)
        p.push(DVec2::new(end, n as f64 * riser - wv));
        p.push(DVec2::new(run, n as f64 * riser - wv));
    } else if from_landing {
        p.push(DVec2::new(run, (n - 1) as f64 * riser - wv));
    } else if u0 < run - 1e-9 {
        p.push(DVec2::new(u0, 0.0));
        p.push(DVec2::new(run, (n - 1) as f64 * riser - wv));
    } else {
        p.push(DVec2::new(run, 0.0));
    }
    // back down the steps: the last riser is the edge of the floor (or
    // landing) it climbs to, so the flight stops at its top tread
    let mut steps = Vec::new();
    for i in 1..n {
        let u = i as f64 * tread;
        steps.push(DVec2::new(u, i as f64 * riser));
        if i + 1 < n {
            steps.push(DVec2::new(u, (i + 1) as f64 * riser));
        }
    }
    p.extend(steps.into_iter().rev());
    p.dedup_by(|a, b| a.distance(*b) < 1e-9);
    p
}

/// Solid steps for every flight, and the landing between them.
pub fn stair_mesh(s: &Stair, base: f64, rise: f64) -> TriMesh {
    let (mut m, finishes) = stair_parts(s, base, rise);
    for (f, _) in finishes {
        m.append(&f);
    }
    m
}

/// A stair's structure and its finishes apart: the treads' finish (on
/// landings and winders too) and the risers', each with its material.
/// The finished treads are a riser apart; the structure sits the tread
/// finish lower.
pub fn stair_parts(
    s: &Stair,
    base: f64,
    rise: f64,
) -> (TriMesh, Vec<(TriMesh, Option<ElementId>)>) {
    let (mut structure, mut finishes) = half_parts(s, base, rise);
    // a split stair: the drawn arm and its mirror image, with no faces
    // between them up the shared flight
    if let Some(m) = s.mirror(rise) {
        structure = off_the_line(&structure, &m);
        for (f, _) in &mut finishes {
            *f = off_the_line(f, &m);
        }
        structure.append(&mirrored(&structure, &m));
        for (f, _) in &mut finishes {
            let image = mirrored(f, &m);
            f.append(&image);
        }
    }
    (structure, finishes)
}

/// `mesh` mirrored across a split stair's mirror line, its faces turned
/// to keep facing out.
pub fn mirrored(mesh: &TriMesh, m: &creabim_core::members::Mirror) -> TriMesh {
    let p3 = |p: &[f32; 3]| DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64);
    let f3 = |v: DVec3| [v.x as f32, v.y as f32, v.z as f32];
    let d = m.dir;
    let flip = |n: DVec3| {
        let v = n.truncate();
        (2.0 * v.dot(d) * d - v).extend(n.z)
    };
    let mut out = TriMesh {
        positions: mesh.positions.iter().map(|p| f3(m.pt3(p3(p)))).collect(),
        normals: mesh.normals.iter().map(|n| f3(flip(p3(n)))).collect(),
        indices: mesh.indices.clone(),
    };
    for t in out.indices.chunks_mut(3) {
        t.swap(1, 2);
    }
    out
}

/// `mesh` without the faces lying in the mirror line's upright plane:
/// the side of a split stair's shared flight, inside it once mirrored.
fn off_the_line(mesh: &TriMesh, m: &creabim_core::members::Mirror) -> TriMesh {
    let on = |i: u32| {
        let p = mesh.positions[i as usize];
        let q = DVec2::new(p[0] as f64, p[1] as f64) - m.origin;
        q.perp_dot(m.dir).abs() < 1e-4
    };
    let mut out = mesh.clone();
    out.indices = mesh
        .indices
        .chunks(3)
        .filter(|t| !(on(t[0]) && on(t[1]) && on(t[2])))
        .flatten()
        .copied()
        .collect();
    out
}

/// One arm of a stair (all of an unsplit one): structure and finishes.
fn half_parts(s: &Stair, base: f64, rise: f64) -> (TriMesh, Vec<(TriMesh, Option<ElementId>)>) {
    if s.is_ramp() {
        return (ramp_structure(s, base, rise), vec![]);
    }
    let tf = s.finish.tread();
    let structure = stair_structure(s, base - tf, rise);
    let mut out = vec![];
    if tf > 1e-9 {
        out.push((stair_tread_finish(s, base, rise), s.finish.tread_material));
    }
    if s.finish.riser(s.build) > 1e-9 {
        out.push((stair_riser_finish(s, base, rise), s.finish.riser_material));
    }
    out.retain(|(m, _)| !m.positions.is_empty());
    (structure, out)
}

/// A ramp: each flight one smooth slope, the stair's waist thick (square
/// to the slope), its underside stopping at the floor or the landing it
/// starts from; each landing a level slab as thick, measured upright.
fn ramp_structure(s: &Stair, base: f64, rise: f64) -> TriMesh {
    let lay = s.layout(rise);
    let (r, g, half) = (lay.riser, s.tread.max(1e-6), s.width / 2.0);
    let wv = s.waist * (1.0 + (r / g).powi(2)).sqrt();
    let mut m = TriMesh::default();
    for f in &lay.flights {
        let run = f.run(g);
        let climb = f.risers as f64 * r;
        if run < 1e-6 {
            continue;
        }
        let floor = base + if f.z0 > 1e-9 { f.z0 - wv } else { 0.0 };
        let k = ((run / 0.25).ceil() as usize).max(1);
        // each cross-section: left and right, top and bottom
        let rows: Vec<[DVec3; 4]> = (0..=k)
            .map(|i| {
                let u = run * i as f64 / k as f64;
                let (p, d) = f.at(u);
                let n = d.perp() * half;
                let z = base + f.z0 + climb * u / run;
                let zb = (z - wv).max(floor);
                [
                    (p + n).extend(z),
                    (p - n).extend(z),
                    (p - n).extend(zb),
                    (p + n).extend(zb),
                ]
            })
            .collect();
        let mid = |q: &[DVec3; 4]| (q[0] + q[1] + q[2] + q[3]) / 4.0;
        // a face of four corners, facing away from `inside`
        let mut face = |q: [DVec3; 4], inside: DVec3| {
            let n = (q[1] - q[0]).cross(q[3] - q[0]);
            let c = (q[0] + q[1] + q[2] + q[3]) / 4.0;
            let n = if n.dot(c - inside) < 0.0 { -n } else { n };
            if n.length() > 1e-12 {
                m.add_quad(q, n);
            }
        };
        for w in rows.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            let inside = (mid(a) + mid(b)) / 2.0;
            for e in 0..4 {
                let f = (e + 1) % 4;
                face([a[e], a[f], b[f], b[e]], inside);
            }
        }
        let (first, last) = (rows[0], rows[k]);
        face(first, mid(&rows[1.min(k)]));
        face(last, mid(&rows[k - 1]));
    }
    for (outline, z) in &lay.landings {
        m.append(&slab_mesh(outline, base + z, wv));
    }
    m
}

/// The finish on each tread, running out past the riser below by the
/// nosing and the riser finish, and on each landing and winder.
fn stair_tread_finish(s: &Stair, base: f64, rise: f64) -> TriMesh {
    let lay = s.layout(rise);
    let (r, g, half) = (lay.riser, s.tread, s.width / 2.0);
    let tf = s.finish.tread();
    let over = s.finish.riser(s.build) + s.finish.nosing.max(0.0);
    let mut m = TriMesh::default();
    for f in &lay.flights {
        for t in 0..f.risers.saturating_sub(1) {
            let z = f.z0 + (t + 1) as f64 * r;
            let band = f.band(t as f64 * g - over, (t + 1) as f64 * g, half);
            m.append(&slab_mesh(&band, base + z, tf));
        }
    }
    for (outline, z) in &lay.landings {
        m.append(&slab_mesh(outline, base + z, tf));
    }
    for w in &lay.winders {
        m.append(&slab_mesh(&w.outline, base + w.z, tf));
    }
    m
}

/// The finish on each riser's face, from the finished tread below to the
/// underside of the tread finish above.
fn stair_riser_finish(s: &Stair, base: f64, rise: f64) -> TriMesh {
    let lay = s.layout(rise);
    let (r, g, half) = (lay.riser, s.tread, s.width / 2.0);
    let (tf, rf) = (s.finish.tread(), s.finish.riser(s.build));
    let mut m = TriMesh::default();
    for f in &lay.flights {
        for t in 0..f.risers {
            let z0 = f.z0 + t as f64 * r;
            let h = r - tf;
            if h <= 1e-9 {
                continue;
            }
            let band = f.band(t as f64 * g - rf, t as f64 * g, half);
            m.append(&slab_mesh(&band, base + z0 + h, h));
        }
    }
    m
}

/// A stair's structure, the tops of its treads at `base` plus each
/// riser.
fn stair_structure(s: &Stair, base: f64, rise: f64) -> TriMesh {
    use members::StairBuild as B;
    let lay = s.layout(rise);
    let mut m = TriMesh::default();
    let (r, g, half) = (lay.riser, s.tread, s.width / 2.0);
    // flush soffits: each landing is the flights' waist thick, measured
    // straight down, and the flights' undersides run on to meet it
    let wv = vertical_waist(r, g, s.waist);
    // how much deeper a depth square to the pitch is, measured upright
    let sec = (1.0 + (r / g).powi(2)).sqrt();
    let build = match s.build {
        // a post needs a centre to stand at
        B::Post if s.centre().is_none() => B::Spine,
        b => b,
    };
    let last = lay.flights.len().saturating_sub(1);
    for (i, f) in lay.flights.iter().enumerate() {
        let treads = f.risers.saturating_sub(1);
        let run = f.run(g);
        // the floor under it: the stair's base, or a landing's underside
        let floor = if f.z0 > 1e-9 { f.z0 - wv } else { 0.0 };
        // the line through the steps' inner corners, `u` along the flight
        let corners = |u: f64| f.z0 + u * r / g;
        // how far along that line climbs to height `z`
        let up_to = |z: f64| (z - f.z0) * g / r;
        let top = |t: u32| f.z0 + (t + 1) as f64 * r;
        let tread = |t: u32| f.tread_outline(t, g, half);
        match build {
            B::Concrete if !f.curved() => {
                let from = i > 0 && f.z0 > 1e-9;
                let to = i < last;
                let prof = flight_profile_joined(f.risers, r, g, s.waist, from, to);
                extrude_profile(&mut m, &prof, f.start.extend(base + f.z0), f.dir, s.width);
            }
            B::Concrete => {
                // round a curve: a helical underside, each tread a block
                // down to it
                for t in 0..treads {
                    let ring: Vec<(DVec2, f64, f64)> = tread(t)
                        .into_iter()
                        .map(|p| {
                            let u = along(f, p);
                            (p, base + top(t), base + (corners(u) - wv).max(floor))
                        })
                        .collect();
                    solid(&mut m, &ring);
                }
            }
            B::Stepped => {
                for t in 0..treads {
                    let z = top(t);
                    m.append(&slab_mesh(
                        &tread(t),
                        base + z,
                        (r + wv).min(z - floor).max(0.02),
                    ));
                }
            }
            B::FoldedPlate => {
                let tk = s.waist.max(0.005);
                for t in 0..treads {
                    m.append(&slab_mesh(&tread(t), base + top(t), tk));
                }
                // each riser a plate up from the tread below's underside
                for t in 0..f.risers {
                    let (a, b) = f.riser(t, g, half);
                    let d = f.at(t as f64 * g).1;
                    let z1 = f.z0 + (t + 1) as f64 * r;
                    let z0 = (z1 - r - tk).max(floor);
                    let ring = [a, a + d * tk, b + d * tk, b].map(|p| (p, base + z1, base + z0));
                    solid(&mut m, &ring);
                }
            }
            B::Stringers | B::CutStringers | B::Spine | B::Cantilever | B::Post => {
                let board = if build == B::Cantilever {
                    2.0 * members::TREAD_BOARD
                } else {
                    members::TREAD_BOARD
                };
                for t in 0..treads {
                    m.append(&slab_mesh(&tread(t), base + top(t), board));
                }
                let (sw, sd) = members::STRINGER;
                match build {
                    B::Stringers => {
                        // a board along each side, just inside its edge,
                        // its top a little over the nosings
                        // where the underside reaches the floor
                        let kink = [up_to(floor - r - 0.05 + sd * sec)];
                        for side in [-1.0, 1.0] {
                            let path = samples(f, run, &kink).into_iter().map(|u| {
                                let (p, d) = f.at(u);
                                let t = corners(u) + r + 0.05;
                                (
                                    p + d.perp() * side * (half - sw / 2.0),
                                    base + t,
                                    base + (t - sd * sec).max(floor),
                                )
                            });
                            ribbon(&mut m, &path.collect::<Vec<_>>(), sw);
                        }
                    }
                    B::CutStringers => {
                        // under each tread at each side, cut to the steps
                        for t in 0..treads {
                            for left in [false, true] {
                                let pick = |p: (DVec2, DVec2)| if left { p.1 } else { p.0 };
                                let (o0, o1) =
                                    (pick(f.riser(t, g, half)), pick(f.riser(t + 1, g, half)));
                                let (i0, i1) = (
                                    pick(f.riser(t, g, half - sw)),
                                    pick(f.riser(t + 1, g, half - sw)),
                                );
                                let zt = base + top(t) - board;
                                let ring: Vec<(DVec2, f64, f64)> = [o0, o1, i1, i0]
                                    .into_iter()
                                    .map(|p| {
                                        (
                                            p,
                                            zt,
                                            base + (corners(along(f, p)) - 0.15 * sec).max(floor),
                                        )
                                    })
                                    .collect();
                                solid(&mut m, &ring);
                            }
                        }
                    }
                    B::Spine => {
                        let (w, d) = members::SPINE;
                        let kinks = [up_to(floor + 0.05 + board), up_to(floor + board + d * sec)];
                        let path: Vec<(DVec2, f64, f64)> = samples(f, run, &kinks)
                            .into_iter()
                            .map(|u| {
                                // under the back of each tread, and never below
                                // the floor where it starts
                                let t = (corners(u) - board).max(floor + 0.05);
                                (f.at(u).0, base + t, base + (t - d * sec).max(floor))
                            })
                            .collect();
                        ribbon(&mut m, &path, w);
                        // a bracket up from it under the middle of each tread
                        for t in 0..treads {
                            let u = (t as f64 + 0.5) * g;
                            let (p, dir) = f.at(u);
                            let k = 0.08;
                            let sq = [
                                p - dir * k - dir.perp() * k,
                                p + dir * k - dir.perp() * k,
                                p + dir * k + dir.perp() * k,
                                p - dir * k + dir.perp() * k,
                            ];
                            let zt = top(t) - board;
                            let ring = sq.map(|q| {
                                (
                                    q,
                                    base + zt,
                                    base + (corners(along(f, q)) - board).max(floor + 0.05) - 0.02,
                                )
                            });
                            solid(&mut m, &ring);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    // the post up the middle of a spiral, to handrail height
    if build == B::Post {
        if let Some(c) = s.centre() {
            let ring: Vec<DVec2> = (0..16)
                .map(|k| {
                    c + DVec2::from_angle(k as f64 * std::f64::consts::TAU / 16.0)
                        * members::POST_RADIUS
                })
                .collect();
            let h = rise + 0.9;
            m.append(&slab_mesh(&ring, base + h, h));
        }
    }
    let landing = match build {
        B::Concrete | B::Stepped => wv,
        B::FoldedPlate => s.waist.max(0.005),
        _ => 0.1,
    };
    for (outline, z) in &lay.landings {
        m.append(&slab_mesh(outline, base + z, landing.min(*z).max(0.02)));
    }
    // winders: each a tread as thick as a landing, its underside a riser
    // higher than the one before; on an open stair, boards
    for w in &lay.winders {
        let t = match build {
            B::Concrete | B::Stepped => wv,
            B::FoldedPlate => r + s.waist.max(0.005),
            _ => members::TREAD_BOARD,
        };
        m.append(&slab_mesh(&w.outline, base + w.z, t.min(w.z).max(0.02)));
    }
    m
}

/// How far along flight `f`'s walk line plan point `p` lies.
fn along(f: &Flight, p: DVec2) -> f64 {
    if f.path.len() < 2 {
        return (p - f.start).dot(f.dir);
    }
    let mut best = (f64::MAX, 0.0);
    let mut run = 0.0;
    for w in f.path.windows(2) {
        let d = w[1] - w[0];
        let l = d.length();
        let t = if l > 0.0 {
            ((p - w[0]).dot(d) / (l * l)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let dist = (w[0] + d * t).distance(p);
        if dist < best.0 {
            best = (dist, run + t * l);
        }
        run += l;
    }
    best.1
}

/// Where along a flight of plan length `run` to set a sloping member's
/// points: its two ends when straight, every 100 mm round a curve.
fn samples(f: &Flight, run: f64, kinks: &[f64]) -> Vec<f64> {
    let n = if f.curved() {
        (run / 0.1).ceil().max(1.0) as usize
    } else {
        1
    };
    let mut us: Vec<f64> = (0..=n).map(|k| run * k as f64 / n as f64).collect();
    // and where the member's top or underside turns, so that a straight
    // member keeps its depth up to where it meets the floor
    us.extend(
        kinks
            .iter()
            .copied()
            .filter(|u| *u > 1e-6 && *u < run - 1e-6),
    );
    us.sort_by(f64::total_cmp);
    us.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
    us
}

/// A closed solid over the plan outline `ring` (either way round), each
/// corner with its own top and bottom height.
fn solid(m: &mut TriMesh, ring: &[(DVec2, f64, f64)]) {
    let mut ring = ring.to_vec();
    let plan: Vec<DVec2> = ring.iter().map(|c| c.0).collect();
    if crate::triangulate::signed_area2(&plan) < 0.0 {
        ring.reverse();
    }
    let plan: Vec<DVec2> = ring.iter().map(|c| c.0).collect();
    let tris = triangulate(&plan);
    let top: Vec<DVec3> = ring.iter().map(|c| c.0.extend(c.1)).collect();
    let bottom: Vec<DVec3> = ring.iter().map(|c| c.0.extend(c.2)).collect();
    let face = |m: &mut TriMesh, a: DVec3, b: DVec3, c: DVec3| {
        let n = (b - a).cross(c - a);
        if n.length_squared() > 1e-18 {
            m.add_polygon(&[a, b, c], &[[0, 1, 2]], n);
        }
    };
    for t in &tris {
        face(m, top[t[0]], top[t[1]], top[t[2]]);
        face(m, bottom[t[0]], bottom[t[2]], bottom[t[1]]);
    }
    let n = ring.len();
    for i in 0..n {
        let j = (i + 1) % n;
        face(m, bottom[i], bottom[j], top[j]);
        face(m, bottom[i], top[j], top[i]);
    }
}

/// A board `width` wide along `path` (plan point, top, bottom at each).
fn ribbon(m: &mut TriMesh, path: &[(DVec2, f64, f64)], width: f64) {
    for w in path.windows(2) {
        let (a, b) = (w[0], w[1]);
        let n = (b.0 - a.0).normalize_or_zero().perp() * width / 2.0;
        solid(
            m,
            &[
                (a.0 - n, a.1, a.2),
                (b.0 - n, b.1, b.2),
                (b.0 + n, b.1, b.2),
                (a.0 + n, a.1, a.2),
            ],
        );
    }
}

/// A square bar of side `size` from `a` to `b`.
fn bar(m: &mut TriMesh, a: DVec3, b: DVec3, size: f64) {
    let along = (b - a).normalize_or_zero();
    if along == DVec3::ZERO {
        return;
    }
    let s = DVec3::Z.cross(along).try_normalize().unwrap_or(DVec3::X);
    let u = along.cross(s).normalize();
    let h = size / 2.0;
    let c = |p: DVec3, i: f64, j: f64| p + s * (i * h) + u * (j * h);
    let corners = [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)];
    for k in 0..4 {
        let (i0, j0) = corners[k];
        let (i1, j1) = corners[(k + 1) % 4];
        let normal = s * (i0 + i1) + u * (j0 + j1);
        m.add_quad(
            [c(a, i0, j0), c(a, i1, j1), c(b, i1, j1), c(b, i0, j0)],
            normal,
        );
    }
    let ring = |p: DVec3| corners.map(|(i, j)| c(p, i, j));
    m.add_quad(ring(a), -along);
    m.add_quad(ring(b), along);
}

/// Posts, a top rail and a bottom rail along `path` (model coordinates).
pub fn railing_mesh(path: &[DVec3], height: f64, spacing: f64) -> TriMesh {
    let mut m = TriMesh::default();
    let top = height - RAIL_SIZE / 2.0;
    for w in path.windows(2) {
        bar(
            &mut m,
            w[0] + DVec3::Z * top,
            w[1] + DVec3::Z * top,
            RAIL_SIZE,
        );
        bar(
            &mut m,
            w[0] + DVec3::Z * 0.1,
            w[1] + DVec3::Z * 0.1,
            members::POST_SIZE,
        );
    }
    for p in members::posts_along(path, spacing) {
        bar(
            &mut m,
            p,
            p + DVec3::Z * (height - RAIL_SIZE),
            members::POST_SIZE,
        );
    }
    m
}

/// A railing's metal (rails, posts, balusters, newels) and its glass, apart.
pub fn railing_parts(model: &Model, r: &creabim_core::Railing) -> (TriMesh, TriMesh) {
    let (mut metal, mut glass) = railing_half(model, r);
    // a split stair's railing runs up both arms
    if let Some(m) = railing_mirror(model, r) {
        metal.append(&mirrored(&metal, &m));
        glass.append(&mirrored(&glass, &m));
    }
    (metal, glass)
}

/// The mirror line of the split stair railing `r` follows, if it does.
pub fn railing_mirror(
    model: &Model,
    r: &creabim_core::Railing,
) -> Option<creabim_core::members::Mirror> {
    let side = r.stair?;
    match model.get(side.stair).map(|e| &e.kind) {
        Some(ElementKind::Stair(s)) => s.mirror(s.rise(model)),
        _ => None,
    }
}

fn railing_half(model: &Model, r: &creabim_core::Railing) -> (TriMesh, TriMesh) {
    let path = r.path_in(model);
    let rail = r
        .rail_profile
        .as_deref()
        .and_then(|n| creabim_core::profiles::find(model, n));
    let left = r.stair.is_some_and(|sd| sd.left);
    if r.on_wall && r.stair.is_some() {
        let mut m = wall_rail_mesh(&path, r.height, left, rail.is_none());
        if let Some(p) = rail {
            let top: Vec<DVec3> = path
                .iter()
                .map(|q| *q + DVec3::Z * (r.height - RAIL_SIZE))
                .collect();
            m.append(&crate::parts::sweep_mesh(&p, &top, false));
        }
        return (m, TriMesh::default());
    }
    // the side the stair is on, for a handrail on the guard's posts
    let inward = r.stair.map(|sd| if sd.left { -1.0 } else { 1.0 });
    guard_mesh(&path, r, rail.as_deref(), inward)
}

/// A guard along `path`: its top rail (the profile `rail`, else a square
/// bar), a bottom rail, posts, its infill, and where asked newels and a
/// handrail lower than the guard on the `inward` side (+1 to the left of
/// the way along).
pub fn guard_mesh(
    path: &[DVec3],
    r: &creabim_core::Railing,
    rail: Option<&[DVec2]>,
    inward: Option<f64>,
) -> (TriMesh, TriMesh) {
    use creabim_core::members::{RailInfill, BALUSTER, GLASS, NEWEL};
    let mut m = TriMesh::default();
    let mut glass = TriMesh::default();
    if path.len() < 2 {
        return (m, glass);
    }
    let top = r.top_height();
    let low = 0.1;
    // where the rail jumps up at a newel, and the runs between
    let jumps = if r.newels { rail_jumps(path) } else { vec![] };
    let mut runs: Vec<Vec<DVec3>> = vec![];
    let mut from = 0;
    let mut start: Option<DVec3> = None;
    for &(i, j, at) in &jumps {
        let mut run: Vec<DVec3> = start.into_iter().collect();
        run.extend_from_slice(&path[from..=i]);
        // on into the newel, level
        run.push(at.extend(path[i].z));
        runs.push(run);
        start = Some(at.extend(path[j].z));
        from = j;
    }
    let mut run: Vec<DVec3> = start.into_iter().collect();
    run.extend_from_slice(&path[from..]);
    runs.push(run);
    for run in &mut runs {
        run.dedup_by(|a, b| a.distance(*b) < 1e-9);
    }
    runs.retain(|run| run.len() >= 2);
    let up =
        |run: &[DVec3], h: f64| -> Vec<DVec3> { run.iter().map(|q| *q + DVec3::Z * h).collect() };
    for run in &runs {
        // the top rail
        match rail {
            Some(p) => m.append(&crate::parts::sweep_mesh(
                p,
                &up(run, top - RAIL_SIZE),
                false,
            )),
            None => {
                for w in run.windows(2) {
                    let h = top - RAIL_SIZE / 2.0;
                    bar(&mut m, w[0] + DVec3::Z * h, w[1] + DVec3::Z * h, RAIL_SIZE);
                }
            }
        }
        for w in run.windows(2) {
            let (a, b) = (w[0], w[1]);
            let d = (b - a).truncate().normalize_or_zero();
            // the bottom rail
            bar(
                &mut m,
                a + DVec3::Z * low,
                b + DVec3::Z * low,
                members::POST_SIZE,
            );
            // a handrail lower than the guard, on the stair's side of its posts
            if r.guard_height.is_some() && top > r.height + 0.02 {
                let off = d.perp().extend(0.0)
                    * inward.unwrap_or(0.0)
                    * (members::POST_SIZE / 2.0 + RAIL_SIZE / 2.0 + 0.01);
                let h = r.height - RAIL_SIZE / 2.0;
                bar(
                    &mut m,
                    a + off + DVec3::Z * h,
                    b + off + DVec3::Z * h,
                    RAIL_SIZE,
                );
            }
            let len = a.truncate().distance(b.truncate());
            let (z0, z1) = (low + members::POST_SIZE / 2.0, top - RAIL_SIZE);
            match r.infill {
                RailInfill::Rail => {}
                RailInfill::Balusters => {
                    for p in baluster_spots(a, b) {
                        bar(&mut m, p + DVec3::Z * z0, p + DVec3::Z * z1, BALUSTER);
                    }
                }
                RailInfill::Glass => {
                    let n = d.perp() * (GLASS / 2.0);
                    let (ta, tb) = (a.z + z1 - 0.03, b.z + z1 - 0.03);
                    let (ba, bb) = (a.z + z0 + 0.03, b.z + z0 + 0.03);
                    let (pa, pb) = (a.truncate(), b.truncate());
                    if len > 1e-6 {
                        solid(
                            &mut glass,
                            &[
                                (pa - n, ta, ba),
                                (pb - n, tb, bb),
                                (pb + n, tb, bb),
                                (pa + n, ta, ba),
                            ],
                        );
                    }
                }
            }
        }
    }
    // newels: at the ends, at the corners in plan, and where a steep
    // piece stops the rail, up past the higher rail
    let mut newels: Vec<(DVec3, f64)> = vec![];
    if r.newels {
        let n = path.len();
        let corner = |i: usize| {
            if i == 0 || i + 1 == n {
                return true;
            }
            let a = (path[i] - path[i - 1]).truncate().normalize_or_zero();
            let b = (path[i + 1] - path[i]).truncate().normalize_or_zero();
            a != DVec2::ZERO && b != DVec2::ZERO && a.dot(b) < 20f64.to_radians().cos()
        };
        for (i, p) in path.iter().enumerate() {
            if corner(i) {
                newels.push((*p, p.z + top + 0.05));
            }
        }
        for &(i, j, at) in &jumps {
            let z = path[i].z.max(path[j].z);
            newels.push((at.extend(path[i].z.min(path[j].z)), z + top + 0.05));
        }
        // one at each place, the tallest
        newels.sort_by(|x, y| y.1.total_cmp(&x.1));
        let mut kept: Vec<(DVec3, f64)> = vec![];
        for (p, h) in newels {
            if !kept
                .iter()
                .any(|(q, _)| q.truncate().distance(p.truncate()) < NEWEL)
            {
                kept.push((p, h));
            }
        }
        newels = kept;
        for (p, h) in &newels {
            bar(&mut m, *p, p.truncate().extend(*h), NEWEL);
        }
    }
    for p in members::posts_along(path, r.post_spacing) {
        if newels
            .iter()
            .any(|(q, _)| q.truncate().distance(p.truncate()) < NEWEL)
        {
            continue;
        }
        bar(
            &mut m,
            p,
            p + DVec3::Z * (top - RAIL_SIZE),
            members::POST_SIZE,
        );
    }
    (m, glass)
}

/// Where a rail along `path` stops at a newel and starts again higher:
/// a short stretch steeper than [`STEEP_RAIL`](creabim_core::members::STEEP_RAIL)
/// that climbs at least a few centimetres, as at the inside of a
/// half-turn landing or round winders. Each is the points `i` and `j`
/// either side of it and where in plan the one newel stands, halfway
/// along it. A long steep stretch (round a spiral's column) and the
/// slivers where a curve's pieces meet are no jump: the rail runs on.
pub fn rail_jumps(path: &[DVec3]) -> Vec<(usize, usize, DVec2)> {
    use creabim_core::members::STEEP_RAIL;
    /// The longest a jump runs in plan, and the least it climbs.
    const JUMP_RUN: f64 = 0.4;
    const JUMP_RISE: f64 = 0.05;
    let run = |a: DVec3, b: DVec3| a.truncate().distance(b.truncate());
    let steep = |a: DVec3, b: DVec3| (b.z - a.z).abs() > STEEP_RAIL * run(a, b) + 1e-9;
    let mut out = vec![];
    let mut k = 0;
    while k + 1 < path.len() {
        if !steep(path[k], path[k + 1]) {
            k += 1;
            continue;
        }
        let i = k;
        while k + 1 < path.len()
            && (steep(path[k], path[k + 1]) || path[k].distance(path[k + 1]) < 1e-9)
        {
            k += 1;
        }
        let j = k;
        let along: f64 = path[i..=j].windows(2).map(|w| run(w[0], w[1])).sum();
        if along <= JUMP_RUN && (path[j].z - path[i].z).abs() >= JUMP_RISE {
            // halfway along it in plan
            let mut left = along / 2.0;
            let mut at = path[i].truncate();
            for w in path[i..=j].windows(2) {
                let d = run(w[0], w[1]);
                if d >= left {
                    at = w[0]
                        .truncate()
                        .lerp(w[1].truncate(), if d > 0.0 { left / d } else { 0.0 });
                    break;
                }
                left -= d;
            }
            out.push((i, j, at));
        }
    }
    out
}

/// Where balusters stand between `a` and `b` (posts at both): evenly,
/// with no clear gap a 100 mm sphere passes.
pub fn baluster_spots(a: DVec3, b: DVec3) -> Vec<DVec3> {
    use creabim_core::members::{BALUSTER, BALUSTER_GAP};
    let len = a.truncate().distance(b.truncate());
    let n = (len / (BALUSTER + BALUSTER_GAP)).ceil().max(1.0) as usize;
    (1..n).map(|k| a.lerp(b, k as f64 / n as f64)).collect()
}

/// A handrail fixed to a wall: the rail along `path` (its ends already
/// returned to the wall) at `height`, on brackets about every metre that
/// reach back to the wall on the `left` (else right) of the way up. No
/// posts, no bottom rail.
pub fn wall_rail_mesh(path: &[DVec3], height: f64, left: bool, square: bool) -> TriMesh {
    let mut m = TriMesh::default();
    let top = height - RAIL_SIZE / 2.0;
    // its square bar, unless a profile is swept in its place (MU103: a
    // Tube rail showed inside the square bar)
    if square {
        for w in path.windows(2) {
            bar(
                &mut m,
                w[0] + DVec3::Z * top,
                w[1] + DVec3::Z * top,
                RAIL_SIZE,
            );
        }
    }
    if path.len() < 4 {
        return m;
    }
    // the rail proper, without its two returns
    let run = &path[1..path.len() - 1];
    let reach = members::WALL_GAP + RAIL_SIZE / 2.0;
    for w in run.windows(2) {
        let d = (w[1] - w[0]).truncate();
        let len = d.length();
        if len < 0.05 {
            continue;
        }
        let n = d.perp() / len * if left { 1.0 } else { -1.0 };
        let k = (len / 1.0).ceil().max(1.0) as usize;
        for i in 1..k.max(2) {
            let p = w[0].lerp(w[1], i as f64 / k.max(2) as f64) + DVec3::Z * (top - RAIL_SIZE);
            let elbow = p - DVec3::Z * 0.05;
            bar(&mut m, p, elbow, 0.012);
            bar(&mut m, elbow, elbow + (n * reach).extend(0.0), 0.012);
        }
    }
    m
}

/// Closed outline lines.
/// The plan footprints of the roofs wholly above `above` (their faces'
/// outlines, eaves included), other than `not`: what hangs over a lower
/// roof seen from above.
fn roofs_over(model: &Model, not: creabim_core::ElementId, above: f64) -> Vec<Vec<DVec2>> {
    let mut out = Vec::new();
    for e in model.iter() {
        let ElementKind::Roof(r) = &e.kind else {
            continue;
        };
        if e.id == not {
            continue;
        }
        let base = r.base(model);
        let faces = r.faces();
        let low = faces
            .iter()
            .flat_map(|f| f.outline.iter().map(|p| f.z(*p)).collect::<Vec<_>>())
            .fold(f64::MAX, f64::min);
        if base + low > above {
            out.extend(faces.into_iter().map(|f| f.outline));
        }
    }
    out
}

/// `a`–`b` cut where it passes under any of `over`: each piece, and
/// whether it is under one.
fn split_under(a: DVec2, b: DVec2, over: &[Vec<DVec2>]) -> Vec<(DVec2, DVec2, bool)> {
    let d = b - a;
    let mut ts = vec![0.0, 1.0];
    for poly in over {
        let n = poly.len();
        for i in 0..n {
            let (p, q) = (poly[i], poly[(i + 1) % n]);
            let e = q - p;
            let den = d.perp_dot(e);
            if den.abs() < 1e-12 {
                continue;
            }
            let t = (p - a).perp_dot(e) / den;
            let u = (p - a).perp_dot(d) / den;
            if t > 1e-9 && t < 1.0 - 1e-9 && (-1e-9..=1.0 + 1e-9).contains(&u) {
                ts.push(t);
            }
        }
    }
    ts.sort_by(f64::total_cmp);
    ts.dedup_by(|x, y| (*x - *y).abs() < 1e-9);
    let mut out: Vec<(DVec2, DVec2, bool)> = Vec::new();
    for w in ts.windows(2) {
        let (p, q) = (a + d * w[0], a + d * w[1]);
        let m = (p + q) / 2.0;
        let under = over
            .iter()
            .any(|poly| creabim_core::polygon::contains(poly, m));
        match out.last_mut() {
            Some(last) if last.2 == under => last.1 = q,
            _ => out.push((p, q, under)),
        }
    }
    out
}

fn ring(items: &mut Vec<PlanItem>, e: &Element, pen: Pen, pts: &[DVec2]) {
    for i in 0..pts.len() {
        items.push(PlanItem::Line {
            id: e.id,
            pen,
            a: pts[i],
            b: pts[(i + 1) % pts.len()],
        });
    }
}

/// How a member on `level` is drawn in its plan, cut `cut` metres above
/// the level.
pub fn member_plan(model: &Model, e: &Element, cut: f64) -> Vec<PlanItem> {
    let mut items = Vec::new();
    let Some(level) = members::level_of(&e.kind) else {
        return items;
    };
    let z = members::elevation(model, level) + cut;
    match &e.kind {
        ElementKind::Part(_) | ElementKind::Site(_) => {
            return crate::parts::plan_items(model, e, cut);
        }
        ElementKind::Column(c) => {
            let (z0, z1) = c.span(model);
            let pts = c.outline();
            if z0 < z && z < z1 {
                items.push(PlanItem::Polygon {
                    id: e.id,
                    pen: Pen::Cut,
                    points: pts,
                });
            } else if z1 <= z {
                ring(&mut items, e, Pen::Projection, &pts);
            } else {
                ring(&mut items, e, Pen::Hidden, &pts);
            }
        }
        ElementKind::Beam(b) => {
            // a sloped beam spans from its lowest to its highest point
            let base = b.top(model);
            let top = base + b.rise[0].max(b.rise[1]);
            let bottom = base + b.rise[0].min(b.rise[1]) - b.depth;
            let pts = b.outline();
            if bottom < z && z < top {
                items.push(PlanItem::Polygon {
                    id: e.id,
                    pen: Pen::Cut,
                    points: pts,
                });
            } else {
                // above the cut, as usual: dashed
                let pen = if top <= z {
                    Pen::Projection
                } else {
                    Pen::Hidden
                };
                ring(&mut items, e, pen, &pts);
            }
        }
        ElementKind::Roof(r) => {
            let from = items.len();
            // seen from above: eaves, and the ridges and hips between faces
            let mut seen: Vec<(DVec2, DVec2)> = Vec::new();
            for f in r.faces() {
                let n = f.outline.len();
                for i in 0..n {
                    let (a, b) = (f.outline[i], f.outline[(i + 1) % n]);
                    let dup = seen.iter().any(|(p, q)| {
                        (p.distance(a) < 1e-6 && q.distance(b) < 1e-6)
                            || (p.distance(b) < 1e-6 && q.distance(a) < 1e-6)
                    });
                    if !dup && a.distance(b) > 1e-6 {
                        seen.push((a, b));
                        items.push(PlanItem::Line {
                            id: e.id,
                            pen: Pen::Projection,
                            a,
                            b,
                        });
                    }
                }
            }
            // the wall plate, where the roof meets the walls, dashed
            if r.overhang > 1e-6 {
                ring(&mut items, e, Pen::Hidden, &r.boundary);
            }
            // holes through it, crossed
            for h in creabim_core::slab::holes_of(model, e) {
                items.extend(crate::plan::hole_marks(e.id, &h, Pen::Projection));
            }
            // seen from above the roof plan shows which way each face falls
            let top = r
                .faces()
                .iter()
                .flat_map(|f| f.outline.iter().map(|p| f.z(*p)).collect::<Vec<_>>())
                .fold(0.0, f64::max);
            let base = r.base(model);
            let low = r
                .faces()
                .iter()
                .flat_map(|f| f.outline.iter().map(|p| f.z(*p)).collect::<Vec<_>>())
                .fold(f64::MAX, f64::min);
            // its roof windows: each frame's outline and its glass inside;
            // overhead in a plan under the roof, the frame and its
            // lining's hole at the ceiling, no glass (MU142)
            for (id, o) in creabim_core::roofwindow::windows_in(model, e.id) {
                if z < base + low {
                    items.extend(crate::opening::roof_window_overhead(model, id, o));
                } else {
                    items.extend(crate::opening::roof_window_plan(model, id, o));
                }
            }
            if z < base + low {
                // wholly above the cut, as in a floor plan under it: its
                // edges and roof windows overhead, dashed
                for it in &mut items[from..] {
                    if let PlanItem::Line { pen, .. } | PlanItem::Polygon { pen, .. } = it {
                        *pen = Pen::Hidden;
                    }
                }
            } else if z > base + top + r.vertical_thickness() {
                // a roof plan: what a higher roof's eaves hang over is
                // hidden under it
                let over = roofs_over(model, e.id, base + top + r.vertical_thickness());
                if !over.is_empty() {
                    let drawn: Vec<PlanItem> = items.drain(from..).collect();
                    for it in drawn {
                        match it {
                            PlanItem::Line { id, pen, a, b } => {
                                for (p, q, under) in split_under(a, b, &over) {
                                    items.push(PlanItem::Line {
                                        id,
                                        pen: if under { Pen::Hidden } else { pen },
                                        a: p,
                                        b: q,
                                    });
                                }
                            }
                            PlanItem::Polygon { id, pen, points } => {
                                let n = points.len();
                                for i in 0..n {
                                    let (a, b) = (points[i], points[(i + 1) % n]);
                                    for (p, q, under) in split_under(a, b, &over) {
                                        items.push(PlanItem::Line {
                                            id,
                                            pen: if under { Pen::Hidden } else { pen },
                                            a: p,
                                            b: q,
                                        });
                                    }
                                }
                            }
                            other => items.push(other),
                        }
                    }
                }
                slope_arrows(&mut items, e, r);
            }
        }
        ElementKind::Stair(s) => {
            let from = items.len();
            stair_plan(
                &mut items,
                e,
                s,
                s.rise(model),
                cut - s.base_offset,
                &model.units(),
            );
            if let Some(m) = s.mirror(s.rise(model)) {
                mirror_items(&mut items, from, &m, true);
            }
        }
        ElementKind::Footing(f) => {
            // below the floor, as usual: dashed; cut when the plan plane
            // passes through it
            let top = f.top(model);
            let z = members::elevation(model, f.plan_level(model)) + cut;
            for piece in creabim_core::footing::outlines(model, e, None) {
                if top - f.thickness < z && z < top {
                    items.push(PlanItem::Polygon {
                        id: e.id,
                        pen: Pen::Cut,
                        points: piece,
                    });
                } else {
                    let pen = if top <= z {
                        Pen::Hidden
                    } else {
                        Pen::Projection
                    };
                    ring(&mut items, e, pen, &piece);
                }
            }
        }
        ElementKind::Railing(r) => {
            let from = items.len();
            railing_plan(&mut items, model, e, r);
            if let Some(m) = railing_mirror(model, r) {
                mirror_items(&mut items, from, &m, false);
            }
        }
        _ => {}
    }
    items
}

/// A railing in plan: its path and its posts.
fn railing_plan(items: &mut Vec<PlanItem>, model: &Model, e: &Element, r: &creabim_core::Railing) {
    let path = r.path_in(model);
    for w in path.windows(2) {
        items.push(PlanItem::Line {
            id: e.id,
            pen: Pen::Projection,
            a: w[0].truncate(),
            b: w[1].truncate(),
        });
    }
    let h = members::POST_SIZE / 2.0;
    for p in r.posts(model) {
        let c = p.truncate();
        ring(
            items,
            e,
            Pen::Projection,
            &[
                c + DVec2::new(-h, -h),
                c + DVec2::new(h, -h),
                c + DVec2::new(h, h),
                c + DVec2::new(-h, h),
            ],
        );
    }
}

/// Mirror the plan items from `from` on across a split stair's mirror
/// line, adding the images. Lines along the line itself over the shared
/// flight are dropped (`shared`): the two halves are one flight there.
fn mirror_items(
    items: &mut Vec<PlanItem>,
    from: usize,
    m: &creabim_core::members::Mirror,
    shared: bool,
) {
    let on = |p: DVec2| (p - m.origin).perp_dot(m.dir).abs() < 1e-6;
    let mut images = vec![];
    for it in &items[from..] {
        let image = match it {
            PlanItem::Line { id, pen, a, b } => PlanItem::Line {
                id: *id,
                pen: *pen,
                a: m.pt(*a),
                b: m.pt(*b),
            },
            PlanItem::Polygon { id, pen, points } => PlanItem::Polygon {
                id: *id,
                pen: *pen,
                points: points.iter().rev().map(|p| m.pt(*p)).collect(),
            },
            PlanItem::Fill {
                id,
                points,
                hatch,
                rgb,
                material,
            } => PlanItem::Fill {
                id: *id,
                points: points.iter().rev().map(|p| m.pt(*p)).collect(),
                hatch: *hatch,
                rgb: *rgb,
                material: *material,
            },
            PlanItem::Arc {
                id,
                pen,
                center,
                radius,
                start,
                end,
            } => {
                let phi = m.dir.y.atan2(m.dir.x);
                PlanItem::Arc {
                    id: *id,
                    pen: *pen,
                    center: m.pt(*center),
                    radius: *radius,
                    start: 2.0 * phi - end,
                    end: 2.0 * phi - start,
                }
            }
            // its tag once; riser numbers on both arms
            // (the shared flight's once)
            PlanItem::Text { at, text, .. } if text.parse::<u32>().is_ok() && !m.on_shared(*at) => {
                let mut t = it.clone();
                if let PlanItem::Text { at: a, .. } = &mut t {
                    *a = m.pt(*at);
                }
                t
            }
            _ => continue,
        };
        images.push(image);
    }
    items.extend(images);
    if shared {
        let mut k = from;
        while k < items.len() {
            let drop = matches!(&items[k], PlanItem::Line { a, b, pen, .. }
                if on(*a) && on(*b) && *pen != Pen::Symbol);
            if drop {
                items.remove(k);
            } else {
                k += 1;
            }
        }
    }
}

/// The pitch of a slope as roof plans write it: degrees, or for a slope
/// too shallow to read in degrees a fall of one in so many.
pub fn pitch_text(degrees: f64) -> String {
    if degrees >= 5.0 {
        format!("{}°", (degrees * 10.0).round() / 10.0)
    } else {
        format!("1:{}", (1.0 / degrees.to_radians().tan()).round())
    }
}

/// An arrow down each sloping face of a roof, from near its top toward
/// its eave, with the pitch written beside it.
fn slope_arrows(items: &mut Vec<PlanItem>, e: &Element, r: &Roof) {
    for f in r.faces() {
        let n = f.outline.len();
        if n < 3 || f.g.length() < 1e-6 {
            continue;
        }
        // the face's centre, by area
        let (mut c, mut area) = (DVec2::ZERO, 0.0);
        for i in 0..n {
            let (a, b) = (f.outline[i], f.outline[(i + 1) % n]);
            let k = a.perp_dot(b);
            c += (a + b) * k;
            area += k;
        }
        if area.abs() < 1e-9 {
            continue;
        }
        let c = c / (3.0 * area);
        let down = -f.g.normalize();
        // as long as the face is deep down its slope, up to 1.6 m
        let depth = {
            let ts = f.outline.iter().map(|p| (*p - c).dot(down));
            let (lo, hi) = ts.fold((f64::MAX, f64::MIN), |(lo, hi), t| (lo.min(t), hi.max(t)));
            hi - lo
        };
        let len = (depth * 0.5).min(1.6);
        if len < 0.2 {
            continue;
        }
        let (tail, head) = (c - down * len / 2.0, c + down * len / 2.0);
        let line = |items: &mut Vec<PlanItem>, a, b| {
            items.push(PlanItem::Line {
                id: e.id,
                pen: Pen::Symbol,
                a,
                b,
            })
        };
        line(items, tail, head);
        let (back, wing) = (-down * 0.25, down.perp() * 0.1);
        line(items, head, head + back + wing);
        line(items, head, head + back - wing);
        // the pitch beside the shaft, on its right going down the slope
        let slope = f.g.length().atan().to_degrees();
        items.push(PlanItem::Label {
            id: e.id,
            at: c - down.perp() * 0.45,
            text: pitch_text(slope),
        });
    }
}

/// A stair in plan: its sides and risers up to the cut, a break line where
/// the plan cuts it, the rest dashed, and an arrow up the walking line.
fn stair_plan(
    items: &mut Vec<PlanItem>,
    e: &Element,
    s: &Stair,
    rise: f64,
    cut: f64,
    units: &creabim_core::Units,
) {
    let lay = s.layout(rise);
    let r = lay.riser;
    let line = |items: &mut Vec<PlanItem>, pen, a, b| {
        items.push(PlanItem::Line {
            id: e.id,
            pen,
            a,
            b,
        });
    };
    let mut broken = false;
    let half = s.width / 2.0;
    let poly = |items: &mut Vec<PlanItem>, pen: Pen, pts: &[DVec2]| {
        for w in pts.windows(2) {
            items.push(PlanItem::Line {
                id: e.id,
                pen,
                a: w[0],
                b: w[1],
            });
        }
    };
    for f in &lay.flights {
        // the first riser the cut plane meets
        let cut_at: Option<u32> = (0..f.risers).find(|i| f.z0 + (*i + 1) as f64 * r > cut + 1e-9);
        let pen_for = |i: u32| match cut_at {
            Some(k) if i > k || broken => Pen::Hidden,
            _ => Pen::Projection,
        };
        // a ramp has no risers to draw: its arrow shows the way up
        for i in (0..f.risers).filter(|_| !s.is_ramp()) {
            let (a, b) = f.riser(i, s.tread, half);
            line(items, pen_for(i), a, b);
        }
        // the sides, split at the break
        let run = f.run(s.tread);
        let split = match (cut_at, broken) {
            (_, true) => 0.0,
            (Some(k), false) => (k as f64 + 0.5) * s.tread,
            (None, false) => run,
        };
        let split = split.min(run);
        for left in [false, true] {
            if split > 1e-9 {
                poly(items, Pen::Projection, &f.side(left, 0.0, split, half));
            }
            if run - split > 1e-9 {
                poly(items, Pen::Hidden, &f.side(left, split, run, half));
            }
        }
        if let (Some(_), false) = (cut_at, broken) {
            break_line(items, e, f, s, split);
            broken = true;
        }
    }
    for (outline, z) in &lay.landings {
        let pen = if broken || *z > cut {
            Pen::Hidden
        } else {
            Pen::Projection
        };
        ring(items, e, pen, outline);
    }
    for w in &lay.winders {
        let pen = if broken || w.z > cut {
            Pen::Hidden
        } else {
            Pen::Projection
        };
        ring(items, e, pen, &w.outline);
    }
    // walking line with an arrow at its top end
    let walk: Vec<DVec2> = lay.walk.iter().map(|p| p.truncate()).collect();
    for w in walk.windows(2) {
        line(items, Pen::Symbol, w[0], w[1]);
    }
    if let [.., a, end] = walk[..] {
        let d = (end - a).normalize_or_zero();
        let back = -d * 0.25;
        let wing = d.perp() * 0.12;
        line(items, Pen::Symbol, end, end + back + wing);
        line(items, Pen::Symbol, end, end + back - wing);
    }
    if let Some(f) = lay.flights.first() {
        items.push(PlanItem::Label {
            id: e.id,
            at: s.up_label_at(&lay).unwrap_or(f.start - f.dir * 0.35),
            text: "UP".into(),
        });
        // the stair's tag: risers and goings, on the right of its first
        // flight, or below its start where its other flights are there
        let (mid, along) = f.at(f.run(s.tread).min(2.0) / 2.0);
        let off = along.perp() * (s.width / 2.0 + 0.3);
        let shapes = s.outlines(rise);
        let clear = |p: DVec2| !shapes.iter().any(|o| creabim_core::polygon::contains(o, p));
        // the label's width along the flight, its height across it
        let n = -along.perp();
        let at = Some(mid - off)
            .filter(|p| {
                [-0.7, 0.0, 0.7]
                    .iter()
                    .all(|u| clear(*p + along * *u) && clear(*p + along * *u + n * 0.25))
            })
            .unwrap_or(f.start - f.dir * 0.75);
        items.push(PlanItem::Label {
            id: e.id,
            at,
            text: stair_tag(s, &lay, units),
        });
    }
}

/// A ramp's slope as drawings give it: "1:12 · 8.3%", the run for one of
/// rise rounded down to a tenth.
pub fn ramp_tag(run: f64) -> String {
    let run = (run * 10.0).floor() / 10.0;
    let ratio = if (run - run.round()).abs() < 1e-6 {
        format!("{run:.0}")
    } else {
        format!("{run:.1}")
    };
    format!("Ramp 1:{ratio} · {:.1}%", 100.0 / run.max(1e-9))
}

/// The riser numbers of the stairs that start on `level`, for a plan at
/// 1:`scale`: each riser's number on the tread it lifts you to, counted
/// from the bottom, a winder's in its middle. None past 1:100, where they
/// would crowd the treads.
pub fn riser_numbers(model: &Model, level: ElementId, scale: f64) -> Vec<PlanItem> {
    let mut items = vec![];
    if scale > 100.0 {
        return items;
    }
    for e in model.iter() {
        let ElementKind::Stair(s) = &e.kind else {
            continue;
        };
        if s.level != level || s.is_ramp() {
            continue;
        }
        let lay = s.layout(s.rise(model));
        let r = lay.riser;
        if r <= 1e-9 {
            continue;
        }
        // where each number goes, a winder taking the place of the flight
        // tread it stands for
        let mut at = std::collections::BTreeMap::new();
        let side = (s.width / 2.0 * 0.5).min(0.3);
        for f in &lay.flights {
            for i in 0..f.risers {
                let (p, d) = f.at((i as f64 + 0.5) * s.tread);
                let n = ((f.z0 + (i + 1) as f64 * r) / r).round() as u32;
                at.insert(n, p + d.perp() * side);
            }
        }
        for w in &lay.winders {
            if w.outline.is_empty() {
                continue;
            }
            let c = w.outline.iter().copied().sum::<DVec2>() / w.outline.len() as f64;
            at.insert((w.z / r).round() as u32, c);
        }
        let from = items.len();
        for (n, p) in at {
            if n == 0 || n > lay.risers {
                continue;
            }
            items.push(PlanItem::Text {
                angle: 0.0,
                id: e.id,
                at: p,
                text: n.to_string(),
                size: 1.8,
                align: crate::paper::Anchor::Middle,
                mm: scale / 1000.0,
                rgb: None,
                bold: false,
            });
        }
        if let Some(m) = s.mirror_of(&lay) {
            mirror_items(&mut items, from, &m, false);
        }
    }
    items
}

/// A stair's tag: `17R 171 · 16G 280`, its risers and their height, its
/// goings and their depth, in millimetres.
/// A stair's tag in the project's units: "16R 184 · 15G 279", or in feet
/// and inches "16R 7 1/4" · 15G 11"" (it was always millimetres).
pub fn stair_tag(
    s: &Stair,
    lay: &creabim_core::members::StairLayout,
    units: &creabim_core::Units,
) -> String {
    if s.is_ramp() {
        return ramp_tag(s.tread / lay.riser.max(1e-9));
    }
    let goings = lay.treads();
    // a riser and a going are under a foot: in inches, without feet
    let short = |m: f64| match units.length {
        creabim_core::units::LengthUnit::FeetInches => creabim_core::Units {
            length: creabim_core::units::LengthUnit::Inches,
            ..*units
        }
        .dimension(m),
        _ => units.dimension(m),
    };
    format!(
        "{}R {} · {}G {}",
        lay.risers,
        short(lay.riser),
        goings,
        short(s.tread)
    )
}

/// A whole stair seen from above, as the floor over it sees it through
/// its well: `line(pen, a, b, z)` takes each tread edge, side and landing
/// edge in projection and the walking line with its arrow, `z` being how
/// high above the stair's base the line is.
pub fn stair_lines(s: &Stair, rise: f64, line: &mut dyn FnMut(Pen, DVec2, DVec2, f64)) {
    let Some(m) = s.mirror(rise) else {
        return half_lines(s, rise, line);
    };
    let on = |p: DVec2| (p - m.origin).perp_dot(m.dir).abs() < 1e-6;
    let mut all = vec![];
    half_lines(s, rise, &mut |pen, a, b, z| all.push((pen, a, b, z)));
    for (pen, a, b, z) in all {
        if pen != Pen::Symbol && on(a) && on(b) {
            continue;
        }
        line(pen, a, b, z);
        line(pen, m.pt(a), m.pt(b), z);
    }
}

fn half_lines(s: &Stair, rise: f64, line: &mut dyn FnMut(Pen, DVec2, DVec2, f64)) {
    let lay = s.layout(rise);
    let r = lay.riser;
    for f in &lay.flights {
        for i in (0..f.risers).filter(|_| !s.is_ramp()) {
            let (a, b) = f.riser(i, s.tread, s.width / 2.0);
            line(Pen::Projection, a, b, f.z0 + (i + 1) as f64 * r);
        }
        let run = f.run(s.tread);
        let top = f.z0 + f.risers as f64 * r;
        for left in [false, true] {
            for w in f.side(left, 0.0, run, s.width / 2.0).windows(2) {
                line(Pen::Projection, w[0], w[1], top);
            }
        }
    }
    for (outline, z) in lay
        .landings
        .iter()
        .map(|(o, z)| (o, *z))
        .chain(lay.winders.iter().map(|w| (&w.outline, w.z)))
    {
        let n = outline.len();
        for i in 0..n {
            line(Pen::Projection, outline[i], outline[(i + 1) % n], z);
        }
    }
    for w in lay.walk.windows(2) {
        line(Pen::Symbol, w[0].truncate(), w[1].truncate(), w[1].z);
    }
    if let [.., a, end] = lay.walk[..] {
        let d = (end - a).truncate().normalize_or_zero();
        let (end, z) = (end.truncate(), end.z);
        let back = -d * 0.25;
        let wing = d.perp() * 0.12;
        line(Pen::Symbol, end, end + back + wing, z);
        line(Pen::Symbol, end, end + back - wing, z);
    }
}

/// The zigzag drawn across a flight where the plan cuts it.
fn break_line(items: &mut Vec<PlanItem>, e: &Element, f: &Flight, s: &Stair, u: f64) {
    let (o, dir) = f.at(u);
    let side = dir.perp();
    let w = s.width / 2.0 + 0.1;
    let slant = 0.3 * s.width;
    let p = |v: f64, du: f64| o + dir * (du + slant * v / w) + side * v;
    let pts = [
        p(-w, 0.0),
        p(-0.15, 0.0),
        p(-0.05, 0.15),
        p(0.05, -0.15),
        p(0.15, 0.0),
        p(w, 0.0),
    ];
    for k in 0..pts.len() - 1 {
        items.push(PlanItem::Line {
            id: e.id,
            pen: Pen::Symbol,
            a: pts[k],
            b: pts[k + 1],
        });
    }
}

/// The clear space a stair's headroom asks for over each stretch where it
/// falls short: a box `need` high over the pitch line, `half` either side
/// of the walk line.
pub fn headroom_envelope(low: &[Vec<DVec3>], half: f64, need: f64) -> TriMesh {
    let mut m = TriMesh::default();
    let up = DVec3::Z * need;
    for run in low {
        for w in run.windows(2) {
            let (a, b) = (w[0], w[1]);
            let d = (b - a).truncate().normalize_or_zero();
            if d == DVec2::ZERO {
                continue;
            }
            let side = d.perp().extend(0.0) * half;
            let (al, ar, bl, br) = (a + side, a - side, b + side, b - side);
            let n = d.perp().extend(0.0);
            m.add_quad([ar, br, br + up, ar + up], -n);
            m.add_quad([al, al + up, bl + up, bl], n);
            m.add_quad([al + up, ar + up, br + up, bl + up], DVec3::Z);
            m.add_quad([al, bl, br, ar], -DVec3::Z);
        }
        // its ends
        if let (Some(a), Some(b)) = (run.first(), run.get(1)) {
            let d = (*b - *a).truncate().normalize_or_zero();
            let side = d.perp().extend(0.0) * half;
            m.add_quad(
                [*a - side, *a + side, *a + side + up, *a - side + up],
                -d.extend(0.0),
            );
        }
        if run.len() >= 2 {
            let (a, b) = (run[run.len() - 2], run[run.len() - 1]);
            let d = (b - a).truncate().normalize_or_zero();
            let side = d.perp().extend(0.0) * half;
            m.add_quad(
                [b + side, b - side, b - side + up, b + side + up],
                d.extend(0.0),
            );
        }
    }
    m
}

#[cfg(test)]
mod tests {

    #[test]
    fn mitred_beams_meet_on_the_bisector_and_butt_ones_overlap_by_half() {
        use creabim_core::members::{join_beams, BeamCorner};
        use creabim_core::{Beam, Element, Level};
        let lv = Element::new(
            "L0",
            ElementKind::Level(Level {
                elevation: 0.0,
                height: 3.0,
            }),
        );
        let m = Model::from_elements([lv.clone()]).unwrap();
        let mut a = Beam::new(lv.id, DVec2::ZERO, DVec2::new(4.0, 0.0));
        let mut b = Beam::new(lv.id, DVec2::new(4.0, 0.0), DVec2::new(4.0, 3.0));
        join_beams(&mut a, &mut b, BeamCorner::Mitred);
        // every vertex of a's far end lies on the 45 degree plane through
        // the corner, so the two close up with no gap and no overlap
        let n = DVec3::new(1.0, 1.0, 0.0).normalize();
        let corner = DVec3::new(4.0, 0.0, 0.0);
        let mesh = beam_mesh(&m, &a);
        let far: Vec<DVec3> = mesh
            .positions
            .iter()
            .map(|p| DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64))
            .filter(|p| p.x > 3.5)
            .collect();
        assert!(!far.is_empty());
        for p in far {
            assert!(
                (p - corner).truncate().dot(n.truncate()).abs() < 1e-6,
                "{p}"
            );
        }
        // far from the origin, where f32 rounds the mesh, every end vertex
        // is still cut (MU40: ragged mitres at about 17 m out)
        let off = DVec2::new(16.919, -6.502);
        let mut fa = Beam::new(lv.id, off, off + DVec2::new(4.0, 0.0));
        let mut fb = Beam::new(
            lv.id,
            off + DVec2::new(4.0, 0.0),
            off + DVec2::new(1.0, 3.0),
        );
        join_beams(&mut fa, &mut fb, BeamCorner::Mitred);
        let corner = (off + DVec2::new(4.0, 0.0)).extend(0.0);
        let da = DVec2::X;
        let db = (DVec2::new(-3.0, 3.0)).normalize();
        let nn = (da + db).normalize();
        for p in beam_mesh(&m, &fa).positions {
            let q = DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64);
            if q.x - off.x > 3.0 {
                assert!((q - corner).truncate().dot(nn).abs() < 1e-3, "{q}");
            }
        }
        // and the second's start, cut on the same plane
        for p in beam_mesh(&m, &fb).positions {
            let q = DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64);
            if (q - corner).truncate().length() < 0.5 {
                assert!((q - corner).truncate().dot(nn).abs() < 1e-3, "start {q}");
            }
        }
        // a library section hung off its insertion point (a channel)
        let chan = vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(0.1, 0.0),
            DVec2::new(0.1, 0.2),
            DVec2::new(0.09, 0.2),
            DVec2::new(0.09, 0.01),
            DVec2::new(0.0, 0.01),
        ];
        let sec = creabim_core::members::Section {
            name: "C".into(),
            points: chan,
        };
        let (mut ca, mut cb) = (fa.clone(), fb.clone());
        ca.section = Some(sec.clone());
        cb.section = Some(sec);
        join_beams(&mut ca, &mut cb, BeamCorner::Mitred);
        for (k, bm) in [(1, &ca), (0, &cb)] {
            for p in beam_mesh(&m, bm).positions {
                let q = DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64);
                if (q - corner).truncate().length() < 0.5 {
                    assert!(
                        (q - corner).truncate().dot(nn).abs() < 1e-3,
                        "channel end {k} {q}"
                    );
                }
            }
        }
        // every preset, mitred both ways round a sharp and a gentle turn
        for name in creabim_core::profiles::PRESETS {
            let pts = creabim_core::profiles::preset(name).unwrap();
            for to in [
                DVec2::new(1.0, 3.0),
                DVec2::new(7.0, 1.0),
                DVec2::new(6.0, -2.5),
            ] {
                let sec = creabim_core::members::Section {
                    name: name.to_string(),
                    points: pts.clone(),
                };
                let mut pa = Beam::new(lv.id, off, off + DVec2::new(4.0, 0.0));
                let mut pb = Beam::new(lv.id, off + DVec2::new(4.0, 0.0), off + to);
                pa.section = Some(sec.clone());
                pb.section = Some(sec);
                join_beams(&mut pa, &mut pb, BeamCorner::Mitred);
                let db = (to - DVec2::new(4.0, 0.0)).normalize();
                let nn = (DVec2::X + db).normalize();
                for (k, bm) in [(1, &pa), (0, &pb)] {
                    let mesh = beam_mesh(&m, bm);
                    let (ax, bx) = bm.axis(&m);
                    let dir = (bx - ax).normalize();
                    let len = ax.distance(bx);
                    for p in &mesh.positions {
                        let q = DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64);
                        let s = (q - ax).dot(dir);
                        // nothing of either beam crosses the mitre plane
                        let side = (q - corner).truncate().dot(nn);
                        if k == 1 {
                            assert!(side < 1e-3, "{name} a past the cut {q} side {side}");
                        } else {
                            assert!(side > -1e-3, "{name} b before the cut {q} side {side}");
                        }
                        let _ = (s, len);
                    }
                }
            }
        }
        // sloped beams (a roof's rafters), an I-section: every vertex
        // of each end lies on the shared cut
        let ipts = creabim_core::profiles::preset("I-section").unwrap();
        let sec = creabim_core::members::Section {
            name: "I".into(),
            points: ipts,
        };
        let mut sa = Beam::new(lv.id, DVec2::ZERO, DVec2::new(4.0, 0.0));
        let mut sb = Beam::new(lv.id, DVec2::new(4.0, 0.0), DVec2::new(4.0, 3.0));
        sa.rise = [0.0, 1.5];
        sb.rise = [1.5, 1.5];
        sa.section = Some(sec.clone());
        sb.section = Some(sec);
        join_beams(&mut sa, &mut sb, BeamCorner::Mitred);
        let n = sa.cuts[1].unwrap().normalize();
        let c = sa.axis(&m).1;
        for bm in [&sa, &sb] {
            let (ax, bx) = bm.axis(&m);
            for p in beam_mesh(&m, bm).positions {
                let q = DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64);
                let near = q.distance(c) < ax.distance(bx) / 2.0;
                if near {
                    assert!((q - c).dot(n).abs() < 1e-3, "sloped end {q}");
                }
            }
        }
        // butt: the first runs through to the second's far face
        let (mut a, mut b) = (a.clone(), b.clone());
        join_beams(&mut a, &mut b, BeamCorner::Butt);
        assert!((a.axis(&m).1.x - 4.1).abs() < 1e-9);
        assert!((b.axis(&m).0.y - 0.1).abs() < 1e-9);
        // a sloped beam: its top rises from start to end
        let mut r = Beam::new(lv.id, DVec2::ZERO, DVec2::new(4.0, 0.0));
        r.rise = [0.0, 1.0];
        let (s, e) = r.axis(&m);
        assert!((e.z - s.z - 1.0).abs() < 1e-9);
    }

    use super::*;
    use creabim_core::{
        Beam, Column, ElementId, Landing, Level, Profile, Railing, RoofShape, Transaction,
    };

    fn model() -> (Model, ElementId) {
        let mut m = Model::new();
        let l = Element::new(
            "Ground",
            ElementKind::Level(Level {
                elevation: 0.0,
                height: 3.0,
            }),
        );
        let id = l.id;
        Transaction::new("l").insert(l).apply(&mut m).unwrap();
        (m, id)
    }

    fn rect(w: f64, d: f64) -> Vec<DVec2> {
        vec![
            DVec2::ZERO,
            DVec2::new(w, 0.0),
            DVec2::new(w, d),
            DVec2::new(0.0, d),
        ]
    }

    #[test]
    fn column_and_beam_meshes_have_the_right_volume() {
        let (m, l) = model();
        let c = Element::new(
            "C",
            ElementKind::Column(Column::new(
                l,
                DVec2::new(2.0, 2.0),
                Profile::Rectangle {
                    width: 0.3,
                    depth: 0.4,
                },
            )),
        );
        let (mesh, _) = member_mesh(&m, &c).unwrap();
        assert!((mesh.volume() - 0.3 * 0.4 * 3.0).abs() < 1e-5);
        let round = Element::new(
            "C",
            ElementKind::Column(Column::new(
                l,
                DVec2::ZERO,
                Profile::Round { diameter: 0.4 },
            )),
        );
        let (mesh, _) = member_mesh(&m, &round).unwrap();
        let exact = std::f64::consts::PI * 0.04 * 3.0;
        assert!((mesh.volume() - exact).abs() / exact < 0.02);
        let b = Element::new(
            "B",
            ElementKind::Beam(Beam::new(l, DVec2::ZERO, DVec2::new(5.0, 0.0))),
        );
        let (mesh, _) = member_mesh(&m, &b).unwrap();
        assert!((mesh.volume() - 5.0 * 0.2 * 0.4).abs() < 1e-5);
        let (lo, hi) = mesh.bounds().unwrap();
        assert!((hi.z - 3.0).abs() < 1e-5 && (lo.z - 2.6).abs() < 1e-5);
    }

    #[test]
    fn roof_volume_is_plan_area_times_upright_thickness() {
        let (_, l) = model();
        for shape in RoofShape::ALL {
            let mut r = Roof::new(l, rect(10.0, 6.0), shape);
            r.pitch = 30.0;
            let m = roof_mesh(&r, 3.0);
            let want = r.plan_area() * r.vertical_thickness();
            assert!(
                (m.volume() - want).abs() / want < 1e-4,
                "{shape:?}: {} vs {want}",
                m.volume()
            );
        }
    }

    #[test]
    fn a_straight_stair_climbs_its_full_rise() {
        let (m, l) = model();
        let s = Stair::new(l, DVec2::ZERO, 0.0);
        let mesh = stair_mesh(&s, 0.0, s.rise(&m));
        let (lo, hi) = mesh.bounds().unwrap();
        assert!(lo.z.abs() < 1e-5);
        // up to the last tread, one riser below the floor above
        assert!((hi.z as f64 - 16.0 * 3.0 / 17.0).abs() < 1e-5);
        // 17 risers of 176 mm, 16 treads
        assert!((hi.x - 16.0 * 0.28).abs() < 1e-4);
        assert!(mesh.volume() > 0.0);
        let prof = flight_profile(17, 3.0 / 17.0, 0.28, 0.15);
        assert!(crate::triangulate::signed_area2(&prof) > 0.0);
    }

    #[test]
    fn a_ramp_is_one_smooth_slope_from_floor_to_floor() {
        let (_, l) = model();
        let mut s = Stair::new(l, DVec2::ZERO, 0.0);
        s.line = vec![DVec2::ZERO, DVec2::new(9.0, 0.0)];
        s.height = Some(0.75);
        s.make_ramp(1.0 / 12.0);
        let (mesh, finishes) = stair_parts(&s, 0.0, 0.75);
        assert!(finishes.is_empty());
        let (lo, hi) = mesh.bounds().unwrap();
        assert!(lo.z.abs() < 1e-5, "{lo}");
        assert!((hi.z as f64 - 0.75).abs() < 0.01, "{hi}");
        assert!(mesh.volume() > 0.0);
        // no steps: along the middle, the top rises at one slope
        let top = |x: f32| {
            mesh.positions
                .iter()
                .filter(|p| (p[0] - x).abs() < 0.13)
                .map(|p| p[2])
                .fold(f32::MIN, f32::max)
        };
        for x in [1.0f32, 3.0, 5.0, 7.0] {
            assert!((top(x) - x / 12.0).abs() < 0.03, "{x}: {}", top(x));
        }
        assert_eq!(ramp_tag(12.0), "Ramp 1:12 · 8.3%");
        assert_eq!(ramp_tag(15.5), "Ramp 1:15.5 · 6.5%");
    }

    #[test]
    fn flights_meet_their_landing_in_one_soffit() {
        let (r, t, w) = (3.0 / 17.0, 0.28, 0.15);
        let wv = vertical_waist(r, t, w);
        let below = |p: &[DVec2], u: f64| {
            // the lowest point of the profile at `u`
            p.iter()
                .filter(|q| (q.x - u).abs() < 1e-9)
                .map(|q| q.y)
                .fold(f64::INFINITY, f64::min)
        };
        // climbing to a landing: one going on under it, its underside
        // there at the landing's underside
        let up = flight_profile_joined(8, r, t, w, false, true);
        assert!(crate::triangulate::signed_area2(&up) > 0.0);
        assert!((below(&up, 8.0 * t) - (8.0 * r - wv)).abs() < 1e-9);
        // leaving a landing: from the landing's underside at its edge
        let down = flight_profile_joined(9, r, t, w, true, false);
        assert!(crate::triangulate::signed_area2(&down) > 0.0);
        assert!((below(&down, 0.0) + wv).abs() < 1e-9);
        // and the underside is one straight line from there
        let slope = r / t;
        assert!((below(&down, 8.0 * t) - (8.0 * t * slope - wv)).abs() < 1e-9);
    }

    #[test]
    fn an_l_stair_and_its_railings_mesh() {
        let (mut m, l) = model();
        let mut s = Stair::new(l, DVec2::ZERO, 0.0);
        s.landing = Some(Landing {
            after: 8,
            left: true,
        });
        let e = Element::new("S", ElementKind::Stair(s.clone()));
        let sid = e.id;
        Transaction::new("s").insert(e).apply(&mut m).unwrap();
        let mesh = stair_mesh(&s, 0.0, 3.0);
        assert!(mesh.volume() > 0.0);
        let r = Railing::along_stair(&m, sid, false).unwrap();
        let re = Element::new("R", ElementKind::Railing(r));
        let (rm, _) = member_mesh(&m, &re).unwrap();
        let (_, hi) = rm.bounds().unwrap();
        assert!(
            (hi.z - 4.0).abs() < 0.01,
            "a metre over the top nosing: {hi:?}"
        );
    }

    #[test]
    fn winders_mesh_and_draw_round_their_turn() {
        let (_, l) = model();
        let pts = [DVec2::ZERO, DVec2::new(3.3, 0.0), DVec2::new(3.3, -3.0)];
        let mut s = Stair::from_line(l, &pts, creabim_core::members::StairLine::Walk).unwrap();
        s.at_turns = creabim_core::members::TurnKind::Winders(0);
        let lay = s.layout(3.0);
        assert_eq!(lay.winders.len(), 3);
        let mesh = stair_mesh(&s, 0.0, 3.0);
        assert!(mesh.volume() > 0.0);
        // the top winder's tread is up where the layout puts it
        let top = lay.winders.last().unwrap().z;
        let (lo, hi) = mesh.bounds().unwrap();
        assert!(lo.z > -1e-9 && hi.z <= 3.0 + 1e-9);
        let near = mesh
            .positions
            .iter()
            .any(|p| (p[2] as f64 - top).abs() < 1e-4);
        assert!(near, "a face at {top}");
        // seen from above, each winder's edges
        let mut edges = 0;
        stair_lines(&s, 3.0, &mut |_, _, _, z| {
            if lay.winders.iter().any(|w| (w.z - z).abs() < 1e-9) {
                edges += 1;
            }
        });
        assert!(edges >= 9, "{edges}");
    }

    #[test]
    fn every_way_of_building_a_stair_meshes() {
        use creabim_core::members::StairBuild as B;
        let (_, l) = model();
        let pts = [DVec2::ZERO, DVec2::new(3.3, 0.0), DVec2::new(3.3, -3.0)];
        let mut s = Stair::from_line(l, &pts, creabim_core::members::StairLine::Walk).unwrap();
        let concrete = stair_mesh(&s, 0.0, 3.0).volume();
        for b in B::ALL {
            s.build = b;
            let mesh = stair_mesh(&s, 0.0, 3.0);
            let (lo, hi) = mesh.bounds().unwrap();
            assert!(mesh.volume() > 0.0, "{b:?}");
            assert!(lo.z > -1e-6 && hi.z < 3.1, "{b:?}: {lo:?} {hi:?}");
            if b.open() {
                // boards and a frame are far lighter than a concrete flight
                assert!(mesh.volume() < 0.6 * concrete, "{b:?}");
            }
        }
        // stringers stand a little over the nosings, at both sides
        s.build = B::Stringers;
        let (lo, hi) = stair_mesh(&s, 0.0, 3.0).bounds().unwrap();
        assert!(hi.z as f64 > s.layout(3.0).flights[0].z0 + 0.05);
        assert!(
            (lo.y as f64) < -0.45 && (hi.y as f64) > 0.45,
            "{lo:?} {hi:?}"
        );
    }

    #[test]
    fn a_spine_keeps_its_depth_down_to_the_floor() {
        use creabim_core::members::StairBuild as B;
        let (_, l) = model();
        let mut s = Stair::new(l, DVec2::ZERO, 0.0);
        s.build = B::Spine;
        let lay = s.layout(3.0);
        let sec = (1.0 + (lay.riser / s.tread).powi(2)).sqrt();
        let depth = creabim_core::members::SPINE.1 * sec;
        let mesh = stair_mesh(&s, 0.0, 3.0);
        // the spine's own corners: its sides are 100 mm off the walk
        // line, where no tread or bracket has a corner
        let mut at: std::collections::BTreeMap<i64, (f64, f64)> = Default::default();
        for p in &mesh.positions {
            if ((p[1] as f64).abs() - 0.1).abs() < 1e-4 {
                let e = at
                    .entry((p[0] as f64 * 1e3).round() as i64)
                    .or_insert((f64::MAX, f64::MIN));
                e.0 = e.0.min(p[2] as f64);
                e.1 = e.1.max(p[2] as f64);
            }
        }
        // full depth wherever its top is that high over the floor
        let full: Vec<_> = at
            .iter()
            .filter(|(_, (_, hi))| *hi > depth - 1e-3)
            .collect();
        assert!(full.len() >= 2, "{at:?}");
        for (x, (lo, hi)) in full {
            assert!((hi - lo - depth).abs() < 1e-3, "at {x} mm: {}", hi - lo);
        }
    }

    #[test]
    fn finishes_sit_on_the_structure_a_riser_apart() {
        use creabim_core::members::StairFinish;
        let (_, l) = model();
        let mut s = Stair::new(l, DVec2::ZERO, 0.0);
        let r = s.layout(3.0).riser;
        s.finish = StairFinish {
            tread: 0.03,
            riser: 0.02,
            nosing: 0.025,
            ..Default::default()
        };
        let (structure, finishes) = stair_parts(&s, 0.0, 3.0);
        assert_eq!(finishes.len(), 2);
        // the finished treads are a riser apart, the top one a riser
        // below the floor above; the structure sits the finish lower
        let (_, top) = finishes[0].0.bounds().unwrap();
        assert!((top.z as f64 - 16.0 * r).abs() < 1e-5, "{top:?}");
        let (lo, hi) = structure.bounds().unwrap();
        assert!((hi.z as f64 - (16.0 * r - 0.03)).abs() < 1e-5);
        assert!((lo.z as f64 + 0.03).abs() < 1e-5);
        // the first tread's finish runs out past its riser's finish by
        // the nosing
        let (flo, _) = finishes[0].0.bounds().unwrap();
        assert!((flo.x as f64 + 0.045).abs() < 1e-5, "{flo:?}");
        let (rlo, rhi) = finishes[1].0.bounds().unwrap();
        assert!((rlo.x as f64 + 0.02).abs() < 1e-5 && rlo.z.abs() < 1e-6);
        assert!((rhi.z as f64 - (17.0 * r - 0.03)).abs() < 1e-5);
        // an open stair has no riser finish
        s.build = creabim_core::members::StairBuild::Stringers;
        assert_eq!(stair_parts(&s, 0.0, 3.0).1.len(), 1);
    }

    #[test]
    fn balusters_leave_no_gap_a_sphere_passes() {
        use creabim_core::members::{BALUSTER, POST_SIZE};
        for len in [0.3, 1.0, 1.17, 2.5] {
            let (a, b) = (DVec3::ZERO, DVec3::new(len, 0.0, len * 0.6));
            let mut xs = vec![POST_SIZE / 2.0 - BALUSTER / 2.0];
            xs.extend(baluster_spots(a, b).iter().map(|p| p.x));
            xs.push(len - POST_SIZE / 2.0 + BALUSTER / 2.0);
            for w in xs.windows(2) {
                let clear = w[1] - w[0] - BALUSTER;
                assert!(clear < 0.1, "{len}: {clear}");
            }
        }
    }

    #[test]
    fn a_rail_jumps_once_round_winders_and_runs_on_round_a_spiral() {
        // level, then up 0.53 over three short steep pieces (winders'
        // inside corner), then level: one jump, its newel halfway
        let w = [
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(1.0, 0.0, 0.0),
            DVec3::new(1.05, 0.0, 0.18),
            DVec3::new(1.05, 0.05, 0.35),
            DVec3::new(1.05, 0.1, 0.53),
            DVec3::new(1.05, 1.1, 0.53),
        ];
        let j = rail_jumps(&w);
        assert_eq!(j.len(), 1, "{j:?}");
        assert_eq!((j[0].0, j[0].1), (1, 4));
        assert!(
            j[0].2.distance(DVec2::new(1.05, 0.025)) < 1e-9,
            "{:?}",
            j[0].2
        );
        // a sliver where a curve's pieces meet climbs too little to jump
        let c = [
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(1.0, 0.0, 0.5),
            DVec3::new(1.012, 0.0, 0.52),
            DVec3::new(2.0, 0.0, 1.0),
        ];
        assert!(rail_jumps(&c).is_empty());
        // round a spiral's column the rail is steep all the way: no jump,
        // one rail up it with a newel only at each end
        let helix: Vec<DVec3> = (0..=40)
            .map(|k| {
                let a = k as f64 * 0.12;
                DVec3::new(0.15 * a.cos(), 0.15 * a.sin(), k as f64 * 0.045)
            })
            .collect();
        assert!(rail_jumps(&helix).is_empty());
        let (_, l) = model();
        let mut r = creabim_core::Railing::new(l, vec![]);
        r.newels = true;
        r.infill = creabim_core::members::RailInfill::Rail;
        let (m, _) = guard_mesh(&helix, &r, None, None);
        // the top rail runs on halfway up
        let mid = helix[20];
        let h = mid.z + r.top_height() - RAIL_SIZE / 2.0;
        assert!(m.positions.iter().any(|p| {
            DVec2::new(p[0] as f64, p[1] as f64).distance(mid.truncate()) < 0.06
                && (p[2] as f64 - h).abs() < 0.06
        }));
    }

    #[test]
    fn a_steep_rail_stops_at_a_newel_and_starts_again_higher() {
        use creabim_core::members::{RailInfill, NEWEL};
        let (_, l) = model();
        let mut r = creabim_core::Railing::new(l, vec![]);
        r.height = 0.9;
        r.newels = true;
        // level, then up 0.6 round a corner in 10 mm, then level again
        let path = [
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(1.0, 0.0, 0.0),
            DVec3::new(1.01, 0.0, 0.6),
            DVec3::new(2.0, 0.0, 0.6),
        ];
        let (m, glass) = guard_mesh(&path, &r, None, None);
        assert!(glass.positions.is_empty());
        // no rail climbs the steep piece: nothing near halfway up it at
        // the rail's height
        let mid = 0.3 + 0.9 - RAIL_SIZE / 2.0;
        assert!(!m.positions.iter().any(|p| {
            let (x, z) = (p[0] as f64, p[2] as f64);
            x > 1.0 + NEWEL && x < 1.5 && (z - mid).abs() < 0.1
        }));
        // a newel at the corner up past the higher rail
        let (_, hi) = m.bounds().unwrap();
        assert!((hi.z as f64 - (0.6 + 0.9 + 0.05)).abs() < 1e-5, "{hi:?}");
        // a taller guard: its top rail at the guard's height, glass below
        r.guard_height = Some(1.07);
        r.newels = false;
        r.infill = RailInfill::Glass;
        let flat = [DVec3::ZERO, DVec3::new(2.0, 0.0, 0.0)];
        let (m, glass) = guard_mesh(&flat, &r, None, Some(1.0));
        let (_, hi) = m.bounds().unwrap();
        assert!((hi.z as f64 - 1.07).abs() < 1e-5);
        // and the handrail at 900 on the inward side of the posts
        assert!(m
            .positions
            .iter()
            .any(|p| (p[2] as f64 - 0.9).abs() < 1e-5 && p[1] > 0.0));
        let (glo, ghi) = glass.bounds().unwrap();
        assert!(glo.z > 0.1 && ghi.z < 1.07 && glo.x < 0.01 && ghi.x > 1.99);
    }

    #[test]
    fn a_spiral_on_a_post_stands_round_its_centre() {
        use creabim_core::members::StairBuild as B;
        let (_, l) = model();
        let c = DVec2::new(10.0, 0.0);
        let pts: Vec<DVec2> = (0..=2)
            .map(|k| {
                c + DVec2::from_angle(std::f64::consts::PI * k as f64).rotate(DVec2::new(0.7, 0.0))
            })
            .collect();
        let mut s = Stair::from_line(l, &pts, creabim_core::members::StairLine::Walk).unwrap();
        s.width = 0.9;
        s.tread = 0.2;
        s.line = pts.iter().map(|p| s.to_local(*p)).collect();
        s.set_bend(0, 180.0);
        s.set_bend(1, 180.0);
        s.build = B::Post;
        let centre = s.centre().expect("a centre");
        assert!((centre - c).length() < 1e-3, "{centre}");
        let mesh = stair_mesh(&s, 0.0, 3.0);
        let (_, hi) = mesh.bounds().unwrap();
        // the post goes on up to take the rail at the top
        assert!(hi.z as f64 > 3.0 - 1e-6);
        let tall = mesh.positions.iter().filter(|p| p[2] as f64 > 3.0).count();
        assert!(tall >= 16, "{tall}");
    }

    #[test]
    fn columns_and_beams_have_a_schedule() {
        let (mut m, l) = model();
        Transaction::new("add")
            .insert(Element::new(
                "C1",
                ElementKind::Column(Column::new(
                    l,
                    DVec2::ZERO,
                    Profile::Round { diameter: 0.3 },
                )),
            ))
            .insert(Element::new(
                "B1",
                ElementKind::Beam(Beam::new(l, DVec2::ZERO, DVec2::new(4.0, 0.0))),
            ))
            .apply(&mut m)
            .unwrap();
        let s = crate::schedule::schedule(&m, creabim_core::ScheduleCategory::Structure);
        assert_eq!(s.rows.len(), 2);
        assert_eq!(s.rows[0].cells[1], "Beam");
        assert_eq!(s.rows[1].cells[4], "Ø 300");
        assert_eq!(s.totals[5], "7000");
    }

    #[test]
    fn a_split_stair_meshes_both_arms_as_one_flight_up_the_middle() {
        let (m, l) = model();
        let mut s = Stair::from_line(
            l,
            &[
                DVec2::new(0.0, 0.0),
                DVec2::new(2.75, 0.0),
                DVec2::new(2.75, 1.5),
                DVec2::new(0.0, 1.5),
            ],
            creabim_core::members::StairLine::Walk,
        )
        .unwrap();
        s.split = true;
        let half = {
            let mut h = s.clone();
            h.split = false;
            stair_mesh(&h, 0.0, 3.0)
        };
        let both = stair_mesh(&s, 0.0, 3.0);
        assert!(both.triangle_count() < 2 * half.triangle_count());
        assert!((both.volume() - 2.0 * half.volume()).abs() < 1e-3);
        // symmetric about y = -0.5
        let (lo, hi) = both.bounds().unwrap();
        assert!(((lo.y + hi.y) as f64 / 2.0 + 0.5).abs() < 1e-4, "{lo} {hi}");
        // in plan, nothing down the middle of the shared flight
        let e = Element::new("S", ElementKind::Stair(s.clone()));
        let items = member_plan(&m, &e, 1.2);
        let mirror = s.mirror(3.0).unwrap();
        let on = |p: DVec2| (p - mirror.origin).perp_dot(mirror.dir).abs() < 1e-6;
        assert!(!items.iter().any(|i| matches!(i,
            PlanItem::Line { a, b, pen, .. } if *pen != Pen::Symbol && on(*a) && on(*b))));
        // but both arms' treads
        let below = items
            .iter()
            .filter(|i| matches!(i, PlanItem::Line { a, .. } if a.y < -1.0))
            .count();
        assert!(below > 5, "{below}");
    }

    #[test]
    fn plans_show_beams_dashed_and_stairs_broken() {
        let (m, l) = model();
        let b = Element::new(
            "B",
            ElementKind::Beam(Beam::new(l, DVec2::ZERO, DVec2::new(5.0, 0.0))),
        );
        let items = member_plan(&m, &b, 1.2);
        assert!(items.iter().all(|i| matches!(
            i,
            PlanItem::Line {
                pen: Pen::Hidden,
                ..
            }
        )));
        let s = Element::new("S", ElementKind::Stair(Stair::new(l, DVec2::ZERO, 0.0)));
        let items = member_plan(&m, &s, 1.2);
        let count = |pen: Pen| {
            items
                .iter()
                .filter(|i| matches!(i, PlanItem::Line { pen: p, .. } if *p == pen))
                .count()
        };
        assert!(count(Pen::Hidden) > 0 && count(Pen::Projection) > 0);
        assert!(items
            .iter()
            .any(|i| matches!(i, PlanItem::Label { text, .. } if text == "UP")));
        let c = Element::new(
            "C",
            ElementKind::Column(Column::new(
                l,
                DVec2::ZERO,
                Profile::Round { diameter: 0.3 },
            )),
        );
        assert!(matches!(
            member_plan(&m, &c, 1.2)[0],
            PlanItem::Polygon { pen: Pen::Cut, .. }
        ));
    }

    #[test]
    fn roof_plans_show_slope_arrows_with_their_pitch() {
        let m = creabim_core::starter::starter_house();
        let view = |name: &str| {
            let id = m.iter().find(|e| e.name == name).unwrap().id;
            crate::section::view_drawing(&m, id).unwrap().items
        };
        let pitches = |items: &[PlanItem]| -> Vec<String> {
            items
                .iter()
                .filter_map(|it| match it {
                    PlanItem::Label { text, .. } if text.ends_with('°') => Some(text.clone()),
                    _ => None,
                })
                .collect()
        };
        // the gable's two faces, each falling to its eave; the flat roofs none
        assert_eq!(pitches(&view("Roof plan")), vec!["35°", "35°"]);
        // the first floor is under the roof: no arrows there
        assert!(pitches(&view("First Floor plan")).is_empty());
        assert_eq!(pitch_text(35.0), "35°");
        assert_eq!(pitch_text((1.0f64 / 60.0).atan().to_degrees()), "1:60");
    }

    #[test]
    fn a_stair_tag_is_written_in_the_projects_units() {
        let model = creabim_core::starter::starter_house();
        let (e, s) = model
            .iter()
            .find_map(|e| match &e.kind {
                ElementKind::Stair(s) => Some((e, s.clone())),
                _ => None,
            })
            .expect("the starter house has a stair");
        let rise = s.rise(&model);
        let lay = s.layout(rise);
        let _ = e;
        let metric = stair_tag(&s, &lay, &creabim_core::Units::default());
        let imperial = stair_tag(&s, &lay, &creabim_core::Units::imperial());
        assert!(!metric.contains('"'), "{metric}");
        assert!(
            imperial.contains('"') && !imperial.contains('\''),
            "{imperial}"
        );
    }
}
