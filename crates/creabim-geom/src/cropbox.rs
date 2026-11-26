//! Cropping a scene with crop boxes (see [`creabim_core::cropbox`]). Each
//! element a box crops keeps only what is inside it, cut plane by plane,
//! and every cut is closed with a cap so the element still reads as solid.
//! Caps are items of their own, with the element's id and a darker shade
//! of its look, so they pick and highlight with it and their outlines draw
//! as the cut edges.

use crate::scene::SceneItem;
use crate::TriMesh;
use creabim_core::{cropbox, CropBox, ElementId, ElementKind, Model};
use glam::{DVec2, DVec3, Vec3};
use std::collections::HashMap;

/// The switched-on crop boxes of a 3D view (`None`: the 3D window's own).
pub fn active(model: &Model, view: Option<ElementId>) -> Vec<CropBox> {
    cropbox::crop_boxes(model, view)
        .into_iter()
        .filter(|(_, c)| c.enabled)
        .map(|(_, c)| c.clone())
        .collect()
}

/// How much darker a cut face is than the element's surface.
const CAP_SHADE: f32 = 0.78;

/// Crop `scene` with `boxes`. An element several boxes crop keeps what is
/// inside all of them; elements no box crops are left alone. Returns the
/// outlines of the cuts, for drawing them as edges.
pub fn crop_scene(
    model: &Model,
    scene: &mut Vec<SceneItem>,
    boxes: &[CropBox],
) -> Vec<(DVec3, DVec3)> {
    if boxes.is_empty() {
        return vec![];
    }
    // doors and windows go with the wall they are in
    let hosts: HashMap<ElementId, ElementId> = model
        .iter()
        .filter_map(|e| match &e.kind {
            ElementKind::Opening(o) => Some((e.id, o.host)),
            _ => None,
        })
        .collect();
    let mut out = Vec::with_capacity(scene.len());
    let mut edges = Vec::new();
    // the caps made, in `out`, and whether each is the ground's
    let mut caps_at: Vec<(usize, bool)> = vec![];
    let ground = |id: ElementId| {
        matches!(
            model.get(id).map(|e| &e.kind),
            Some(ElementKind::Site(s)) if matches!(s.form, creabim_core::SiteForm::Terrain { .. })
        )
    };
    for item in scene.drain(..) {
        let host = hosts.get(&item.id).copied();
        let mine: Vec<&CropBox> = boxes.iter().filter(|b| b.crops(item.id, host)).collect();
        if mine.is_empty() {
            out.push(item);
            continue;
        }
        let Some((body, caps)) = crop_mesh(&item.mesh, &mine, !item.ghost) else {
            continue; // all of it is outside
        };
        let cap = (!caps.is_empty()).then(|| SceneItem {
            mesh: caps,
            color: item.color.map(|c| c * CAP_SHADE),
            ..item.clone()
        });
        if !body.is_empty() {
            out.push(SceneItem { mesh: body, ..item });
        }
        if let Some(cap) = cap {
            caps_at.push((out.len(), ground(cap.id) && !cap.ghost));
            out.push(cap);
        }
    }
    // the ground lies against what is built in it (footings, foundation
    // walls: their trenches are filled back) and so fills where they
    // stand. Its cut face leaves out theirs, or it painted over them: a
    // foundation wall below the ground read as a bare outline (MU139)
    let built: Vec<&TriMesh> = caps_at
        .iter()
        .filter(|(k, g)| !*g && !out[*k].ghost)
        .map(|(k, _)| &out[*k].mesh)
        .collect();
    let mut cut: Vec<(usize, TriMesh)> = vec![];
    for (k, g) in &caps_at {
        if *g {
            cut.push((*k, leave_out(&out[*k].mesh, &built)));
        }
    }
    for (k, mesh) in cut {
        out[k].mesh = mesh;
    }
    for (k, _) in &caps_at {
        edges.extend(crate::section::feature_edges(&out[*k]));
    }
    out.retain(|i| !i.mesh.indices.is_empty());
    *scene = out;
    edges
}

/// The cut faces of `cap` less where any cut face of `holes` lies in the
/// same plane: crop caps lie in a crop box's axis-aligned faces.
fn leave_out(cap: &TriMesh, holes: &[&TriMesh]) -> TriMesh {
    // each triangle with area, flattened, with its own facing: a triangle
    // with none is left out (it covers nothing), and nothing else moves
    // along with it (MU143: dropped from one list but not the other, every
    // triangle after it took the next one's facing, and the seams between
    // triangles facing apart drew as lines across the cut face)
    type Tri2 = (usize, f64, [DVec2; 3], [f32; 3]);
    let flat = |m: &TriMesh| -> Vec<Tri2> {
        let at = |i: u32| Vec3::from(m.positions[i as usize]).as_dvec3();
        m.indices
            .chunks(3)
            .filter_map(|t| {
                let p = [at(t[0]), at(t[1]), at(t[2])];
                let n = (p[1] - p[0]).cross(p[2] - p[0]);
                if n.length() < 1e-12 {
                    return None;
                }
                let axis = n.abs().max_position();
                let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
                Some((
                    axis,
                    p[0][axis],
                    p.map(|q| DVec2::new(q[u], q[v])),
                    m.normals[t[0] as usize],
                ))
            })
            .collect()
    };
    let holes: Vec<Tri2> = holes.iter().flat_map(|h| flat(h)).collect();
    let bb = |t: &[DVec2]| {
        t.iter()
            .fold((DVec2::MAX, DVec2::MIN), |(l, h), q| (l.min(*q), h.max(*q)))
    };
    let mut out = TriMesh::default();
    for (axis, value, tri, n) in flat(cap) {
        let (lo, hi) = bb(&tri);
        let mut pieces: Vec<Vec<DVec2>> = vec![tri.to_vec()];
        for (ha, hv, h, _) in &holes {
            if *ha != axis || (hv - value).abs() > 1e-4 {
                continue;
            }
            let (hl, hh) = bb(h);
            if hl.x >= hi.x || hl.y >= hi.y || hh.x <= lo.x || hh.y <= lo.y {
                continue;
            }
            pieces = pieces
                .iter()
                .flat_map(|p| crate::plan::subtract_convex(p, h))
                .collect();
            if pieces.is_empty() {
                break;
            }
        }
        let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
        let to3 = |q: DVec2| {
            let mut p = DVec3::ZERO;
            p[axis] = value;
            p[u] = q.x;
            p[v] = q.y;
            p.as_vec3().to_array()
        };
        // each piece is convex, in the triangle's turn: a fan closes it
        for piece in pieces {
            let base = out.positions.len() as u32;
            for q in &piece {
                out.positions.push(to3(*q));
                out.normals.push(n);
            }
            for k in 1..piece.len().saturating_sub(1) as u32 {
                out.indices.extend([base, base + k, base + k + 1]);
            }
        }
    }
    mend(&out)
}

/// `m` with no corner of one triangle lying along the side of another:
/// each such side is split at the corners on it. A triangle cut round a
/// footing or a drain took new corners its neighbours didn't share, and
/// the long sides left between them drew as lines across the cut face
/// (MU143: "too many strange zigzags on that site when I use crop box").
pub(crate) fn mend(m: &TriMesh) -> TriMesh {
    // a split can lay a new side over a corner further on: again until
    // nothing more is split
    let mut out = mend_once(m);
    for _ in 0..3 {
        if out.indices.len() == m.indices.len() {
            break;
        }
        let again = mend_once(&out);
        if again.indices.len() == out.indices.len() {
            break;
        }
        out = again;
    }
    out
}

fn mend_once(m: &TriMesh) -> TriMesh {
    const ON: f64 = 2e-5;
    let at = |i: u32| Vec3::from(m.positions[i as usize]).as_dvec3();
    // the corners, once each, sorted along x to find those near a side
    let mut pts: Vec<DVec3> = vec![];
    let mut seen = std::collections::HashSet::new();
    for i in &m.indices {
        let p = at(*i);
        let k = (
            (p.x * 1e5).round() as i64,
            (p.y * 1e5).round() as i64,
            (p.z * 1e5).round() as i64,
        );
        if seen.insert(k) {
            pts.push(p);
        }
    }
    pts.sort_by(|a, b| a.x.total_cmp(&b.x));
    let on_side = |a: DVec3, b: DVec3| -> Vec<(f64, DVec3)> {
        let ab = b - a;
        let len2 = ab.length_squared();
        if len2 < ON * ON {
            return vec![];
        }
        let (x0, x1) = (a.x.min(b.x) - ON, a.x.max(b.x) + ON);
        let from = pts.partition_point(|p| p.x < x0);
        let mut out: Vec<(f64, DVec3)> = pts[from..]
            .iter()
            .take_while(|p| p.x <= x1)
            .filter_map(|p| {
                let t = (*p - a).dot(ab) / len2;
                let off = (a + ab * t).distance(*p);
                let len = len2.sqrt();
                (off < ON && t * len > ON && (1.0 - t) * len > ON).then_some((t, *p))
            })
            .collect();
        out.sort_by(|x, y| x.0.total_cmp(&y.0));
        out
    };
    // a side with corners on it is split from the corner across, in the
    // triangle's turn, and each piece looked at again for its other sides.
    // No new corner is made: a fan from a middle of its own turned over on
    // a thin triangle and drew its seams
    fn split(
        c: [DVec3; 3],
        n: [f32; 3],
        on_side: &dyn Fn(DVec3, DVec3) -> Vec<(f64, DVec3)>,
        depth: u32,
        out: &mut TriMesh,
    ) {
        // a triangle with no area covers nothing: its own corner lies on
        // its side, and splitting it would go on for ever
        let long = c[0]
            .distance(c[1])
            .max(c[1].distance(c[2]))
            .max(c[2].distance(c[0]));
        if (c[1] - c[0]).cross(c[2] - c[0]).length() <= 1e-9 * long.max(1.0) {
            return;
        }
        if depth < 12 {
            for k in 0..3 {
                let (a, b, o) = (c[k], c[(k + 1) % 3], c[(k + 2) % 3]);
                let mid = on_side(a, b);
                if mid.is_empty() {
                    continue;
                }
                let chain: Vec<DVec3> = std::iter::once(a)
                    .chain(mid.into_iter().map(|(_, p)| p))
                    .chain(std::iter::once(b))
                    .collect();
                for w in chain.windows(2) {
                    split([w[0], w[1], o], n, on_side, depth + 1, out);
                }
                return;
            }
        }
        let base = out.positions.len() as u32;
        for p in c {
            out.positions.push(p.as_vec3().to_array());
            out.normals.push(n);
        }
        out.indices.extend([base, base + 1, base + 2]);
    }
    let mut out = TriMesh::default();
    for t in m.indices.chunks(3) {
        split(
            [at(t[0]), at(t[1]), at(t[2])],
            m.normals[t[0] as usize],
            &on_side,
            0,
            &mut out,
        );
    }
    out
}

/// What of `mesh` is inside every box: the cut surface, and the caps that
/// close it (when `capped`). `None` when nothing is left; the same mesh
/// and no caps when it is inside them all.
pub fn crop_mesh(mesh: &TriMesh, boxes: &[&CropBox], capped: bool) -> Option<(TriMesh, TriMesh)> {
    let (lo, hi) = mesh.bounds()?;
    let (lo, hi) = (lo.as_dvec3(), hi.as_dvec3());
    let mut body = mesh.clone();
    let mut caps = TriMesh::default();
    for b in boxes {
        if (0..3).any(|a| hi[a] < b.min[a] || lo[a] > b.max[a]) {
            return None;
        }
        for face in 0..6 {
            let axis = face / 2;
            // keep `side * (value - p) >= 0`: below the top, above the bottom
            let (value, side) = if face % 2 == 0 {
                (b.min[axis], -1.0)
            } else {
                (b.max[axis], 1.0)
            };
            if side * (value - hi[axis]) >= 0.0 && side * (value - lo[axis]) >= 0.0 {
                continue; // all of it is on the kept side
            }
            let cap = if capped {
                let mut closed = body.clone();
                closed.append(&caps);
                cap_at(&closed, axis, value, side)
            } else {
                TriMesh::default()
            };
            body = clip(&body, axis, value, side);
            caps = clip(&caps, axis, value, side);
            caps.append(&cap);
        }
    }
    if body.is_empty() && caps.is_empty() {
        return None;
    }
    Some((body, caps))
}

/// The part of `mesh` with `side * (value - p[axis]) >= 0`.
fn clip(mesh: &TriMesh, axis: usize, value: f64, side: f64) -> TriMesh {
    let mut out = TriMesh::default();
    let at = |i: u32| Vec3::from(mesh.positions[i as usize]).as_dvec3();
    for t in mesh.indices.chunks(3) {
        let p = [at(t[0]), at(t[1]), at(t[2])];
        let d = p.map(|q| side * (value - q[axis]));
        let n = mesh.normals[t[0] as usize];
        if d.iter().all(|v| *v >= 0.0) {
            let base = out.positions.len() as u32;
            for q in p {
                out.positions.push(q.as_vec3().to_array());
                out.normals.push(n);
            }
            out.indices.extend([base, base + 1, base + 2]);
            continue;
        }
        if d.iter().all(|v| *v <= 0.0) {
            continue;
        }
        // Sutherland–Hodgman against the one plane
        let mut poly = Vec::with_capacity(4);
        for i in 0..3 {
            let j = (i + 1) % 3;
            if d[i] >= 0.0 {
                poly.push(p[i]);
            }
            if (d[i] >= 0.0) != (d[j] >= 0.0) {
                let s = d[i] / (d[i] - d[j]);
                let mut q = p[i] + (p[j] - p[i]) * s;
                q[axis] = value;
                poly.push(q);
            }
        }
        let base = out.positions.len() as u32;
        for q in &poly {
            out.positions.push(q.as_vec3().to_array());
            out.normals.push(n);
        }
        for k in 1..poly.len().saturating_sub(1) as u32 {
            out.indices.extend([base, base + k, base + k + 1]);
        }
    }
    out
}

/// The faces closing a closed `mesh` where the plane `p[axis] = value`
/// cuts it, facing out of the kept side.
fn cap_at(mesh: &TriMesh, axis: usize, value: f64, side: f64) -> TriMesh {
    // turn the axes round so the plane is level, slice, and turn back
    let to_flat = |p: [f32; 3]| match axis {
        0 => [p[1], p[2], p[0]],
        1 => [p[2], p[0], p[1]],
        _ => p,
    };
    let from_flat = |q: DVec2| match axis {
        0 => DVec3::new(value, q.x, q.y),
        1 => DVec3::new(q.y, value, q.x),
        _ => DVec3::new(q.x, q.y, value),
    };
    let flat = TriMesh {
        positions: mesh.positions.iter().map(|p| to_flat(*p)).collect(),
        normals: mesh.normals.iter().map(|n| to_flat(*n)).collect(),
        indices: mesh.indices.clone(),
    };
    let mut normal = DVec3::ZERO;
    normal[axis] = side;
    let mut out = TriMesh::default();
    for outline in crate::slice::slice_closed(&flat, value) {
        let tris = crate::triangulate::triangulate(&outline);
        let pts: Vec<DVec3> = outline.iter().map(|q| from_flat(*q)).collect();
        out.add_polygon(&pts, &tris, normal);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use creabim_core::{CropScope, EditMesh};

    fn cuboid(size: DVec3, at: DVec3) -> TriMesh {
        crate::scene::edit_mesh_to_tri(&EditMesh::cuboid(size), at)
    }

    fn bounds(m: &TriMesh) -> (DVec3, DVec3) {
        let (lo, hi) = m.bounds().unwrap();
        (lo.as_dvec3(), hi.as_dvec3())
    }

    #[test]
    fn a_cut_box_stays_closed_with_caps_on_its_cut_faces() {
        let m = cuboid(DVec3::new(4.0, 4.0, 4.0), DVec3::ZERO);
        let (lo, hi) = bounds(&m);
        let mid = (lo + hi) / 2.0;
        // a crop box over one corner of the block
        let c = CropBox::new(None, mid - DVec3::splat(10.0), mid);
        let (body, caps) = crop_mesh(&m, &[&c], true).unwrap();
        let mut whole = body.clone();
        whole.append(&caps);
        // an eighth of the block is left, and it is closed
        assert!((whole.volume() - 8.0).abs() < 1e-3, "{}", whole.volume());
        let (l, h) = bounds(&whole);
        assert!((h - mid).abs().max_element() < 1e-5 && (l - lo).abs().max_element() < 1e-5);
        // three cut faces of 2 × 2
        let area: f64 = caps
            .indices
            .chunks(3)
            .map(|t| {
                let p = |i: u32| Vec3::from(caps.positions[i as usize]).as_dvec3();
                (p(t[1]) - p(t[0])).cross(p(t[2]) - p(t[0])).length() / 2.0
            })
            .sum();
        assert!((area - 12.0).abs() < 1e-3, "{area}");
    }

    /// Area of the triangles of `a` whose middles lie inside a triangle of
    /// `b` in the same axis plane.
    fn overlap_area(a: &TriMesh, b: &[&TriMesh]) -> f64 {
        let tris = |m: &TriMesh| -> Vec<[DVec3; 3]> {
            let p = |i: u32| Vec3::from(m.positions[i as usize]).as_dvec3();
            m.indices
                .chunks(3)
                .map(|t| [p(t[0]), p(t[1]), p(t[2])])
                .collect()
        };
        let holes: Vec<[DVec3; 3]> = b.iter().flat_map(|m| tris(m)).collect();
        let area2 = |p: &[DVec2]| {
            (0..p.len())
                .map(|i| p[i].perp_dot(p[(i + 1) % p.len()]))
                .sum::<f64>()
                .abs()
                / 2.0
        };
        let mut area = 0.0;
        for t in tris(a) {
            let n = (t[1] - t[0]).cross(t[2] - t[0]);
            let axis = n.abs().max_position();
            let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
            let t2 = t.map(|p| DVec2::new(p[u], p[v]));
            for h in &holes {
                if (h[0][axis] - t[0][axis]).abs() > 1e-4
                    || ((h[1] - h[0]).cross(h[2] - h[0])).abs().max_position() != axis
                {
                    continue;
                }
                let h2 = h.map(|p| DVec2::new(p[u], p[v]));
                // what of t lies in h: t less the parts outside h
                let outside: f64 = crate::plan::subtract_convex(&t2, &h2)
                    .iter()
                    .map(|p| area2(p))
                    .sum();
                area += (area2(&t2) - outside).max(0.0);
            }
        }
        area
    }

    /// Cut through a house's foundation, the ground's cut face leaves out
    /// the foundation walls' and footings' cut faces (MU139: the ground's
    /// green painted over a foundation wall, which read as a bare outline).
    #[test]
    fn the_grounds_cut_face_leaves_out_the_foundations() {
        let model = creabim_core::ridgeline::ridgeline_house();
        let scene = crate::scene::build_scene(&model);
        let is_ground = |id: ElementId| {
            matches!(model.get(id).map(|e| &e.kind),
                Some(ElementKind::Site(s)) if matches!(s.form, creabim_core::SiteForm::Terrain { .. }))
        };
        let built = |id: ElementId| {
            matches!(
                model.get(id).map(|e| &e.kind),
                Some(ElementKind::Wall(_) | ElementKind::Footing(_))
            )
        };
        // the middle of the house, across it
        let (mut lo, mut hi) = (DVec3::splat(f64::MAX), DVec3::splat(f64::MIN));
        for i in scene.iter().filter(|i| built(i.id)) {
            let (l, h) = bounds(&i.mesh);
            lo = lo.min(l);
            hi = hi.max(h);
        }
        let mid = (lo + hi) / 2.0;
        let c = CropBox::new(None, DVec3::new(mid.x, -1e4, -1e4), DVec3::splat(1e4));
        // before: each element capped on its own
        let caps_of = |f: &dyn Fn(ElementId) -> bool| -> Vec<TriMesh> {
            scene
                .iter()
                .filter(|i| f(i.id) && !i.ghost)
                .filter_map(|i| crop_mesh(&i.mesh, &[&c], true).map(|(_, caps)| caps))
                .collect()
        };
        let (g0, b0) = (caps_of(&is_ground), caps_of(&built));
        let b0r: Vec<&TriMesh> = b0.iter().collect();
        let before: f64 = g0.iter().map(|g| overlap_area(g, &b0r)).sum();
        assert!(
            before > 0.05,
            "the ground fills where the foundations stand: {before} m²"
        );
        // after: the scene cropped as the 3D window crops it
        let mut cropped = scene.clone();
        crop_scene(&model, &mut cropped, std::slice::from_ref(&c));
        let on_plane = |i: &&SceneItem| {
            i.mesh
                .positions
                .iter()
                .all(|p| (p[0] as f64 - mid.x).abs() < 1e-4)
        };
        let g1: Vec<&SceneItem> = cropped
            .iter()
            .filter(|i| is_ground(i.id))
            .filter(on_plane)
            .collect();
        let b1: Vec<&TriMesh> = cropped
            .iter()
            .filter(|i| built(i.id))
            .filter(on_plane)
            .map(|i| &i.mesh)
            .collect();
        assert!(!g1.is_empty() && !b1.is_empty());
        let after: f64 = g1.iter().map(|g| overlap_area(&g.mesh, &b1)).sum();
        assert!(
            after < 1e-6,
            "{after} m² of ground over the foundations, of {before}"
        );
    }

    /// Cut across the site, the ground's cut faces draw as their outlines
    /// only: no line runs across them (MU143, owner: "too many strange
    /// zigzags on that site when I use crop box? it's not presentable").
    /// Two faults drew them: in leaving the foundations out of the cut
    /// face, a triangle with no area was dropped from one list and not the
    /// other, so every one after it took the next one's facing; and a
    /// triangle cut round a drain or a footing took corners its neighbours
    /// didn't share.
    #[test]
    fn the_grounds_cut_faces_draw_only_their_outlines() {
        let model = creabim_core::ridgeline::ridgeline_house();
        let scene = crate::scene::build_scene(&model);
        let terrain = |id: ElementId| {
            matches!(model.get(id).map(|e| &e.kind),
            Some(ElementKind::Site(s)) if matches!(s.form, creabim_core::SiteForm::Terrain { .. }))
        };
        // round the house, 3 m out: the drains cross two of its faces
        let (mut lo, mut hi) = (DVec3::splat(f64::MAX), DVec3::splat(f64::MIN));
        for i in scene
            .iter()
            .filter(|i| matches!(model.get(i.id).map(|e| &e.kind), Some(ElementKind::Wall(_))))
        {
            let (l, h) = bounds(&i.mesh);
            lo = lo.min(l);
            hi = hi.max(h);
        }
        let c = CropBox::new(
            None,
            lo - DVec3::new(3.0, 3.0, 4.0),
            hi + DVec3::new(3.0, 3.0, 1.0),
        );
        let mut cropped = scene.clone();
        let edges = crop_scene(&model, &mut cropped, std::slice::from_ref(&c));
        let mut faces = 0;
        for a in 0..3 {
            for v in [c.min[a], c.max[a]] {
                let (u, w) = ((a + 1) % 3, (a + 2) % 3);
                let flat = |q: DVec3| DVec2::new(q[u], q[w]);
                // the ground's cut face in this face of the box, and every
                // line drawn on it: the cuts' outlines and the ground's edges
                let mut face: Vec<[DVec2; 3]> = vec![];
                let mut lines: Vec<(DVec3, DVec3)> = edges.clone();
                for i in cropped.iter().filter(|i| terrain(i.id) && !i.ghost) {
                    let p = |k: u32| Vec3::from(i.mesh.positions[k as usize]).as_dvec3();
                    for t in i.mesh.indices.chunks(3) {
                        let q = [p(t[0]), p(t[1]), p(t[2])];
                        let f = q.map(flat);
                        if q.iter().all(|q| (q[a] - v).abs() < 1e-3)
                            && (f[1] - f[0]).perp_dot(f[2] - f[0]).abs() > 1e-10
                        {
                            face.push(f);
                        }
                    }
                    lines.extend(crate::section::feature_edges(i));
                }
                if face.is_empty() {
                    continue;
                }
                faces += 1;
                let inside = |q: DVec2| {
                    face.iter().any(|t| {
                        let s = |a: DVec2, b: DVec2| (b - a).perp_dot(q - a);
                        let d = [s(t[0], t[1]), s(t[1], t[2]), s(t[2], t[0])];
                        d.iter().all(|x| *x > 0.0) || d.iter().all(|x| *x < 0.0)
                    })
                };
                // a line with the cut face 2 cm either side of its middle
                // runs across it; one closer to the outline lies on it
                let across: f64 = lines
                    .iter()
                    .filter(|(x, y)| (x[a] - v).abs() < 1e-3 && (y[a] - v).abs() < 1e-3)
                    .filter_map(|(x, y)| {
                        let (x, y) = (flat(*x), flat(*y));
                        let (m, n) = ((x + y) / 2.0, (y - x).perp().normalize_or_zero() * 0.02);
                        (inside(m + n) && inside(m - n)).then(|| x.distance(y))
                    })
                    .sum();
                assert!(
                    across < 1e-6,
                    "{across:.1} m of lines across the ground's cut face at {v:.2} (axis {a})"
                );
            }
        }
        assert!(faces >= 4, "the box cuts the ground on {faces} faces");
    }

    /// Cut where the owner cut the Ridgeline, through the north foundation
    /// wall's length (x = 7.385 m), nothing of the ground's cut face lies
    /// in a foundation's, and no outline of it runs through one (MU145:
    /// "why do we see this?", a dark line from the wall's inside insulation
    /// 16 cm into its concrete). A long ground triangle's tip, under 0.1 mm
    /// wide, ran into the wall: taken for a hole only touching it, it was
    /// kept, and the insulation's hole split it off from the rest.
    #[test]
    fn no_outline_of_the_ground_runs_through_a_foundation() {
        let model = creabim_core::ridgeline::ridgeline_house();
        let scene = crate::scene::build_scene(&model);
        let x = 7.385;
        let c = CropBox::new(None, DVec3::splat(-1e3), DVec3::new(x, 1e3, 1e3));
        let mut cropped = scene.clone();
        crop_scene(&model, &mut cropped, &[c]);
        let kind = |id: ElementId| model.get(id).map(|e| &e.kind);
        let on_plane = |i: &&SceneItem| {
            !i.mesh.indices.is_empty()
                && i.mesh
                    .positions
                    .iter()
                    .all(|p| (p[0] as f64 - x).abs() < 1e-4)
        };
        let tris = |m: &TriMesh| -> Vec<[DVec2; 3]> {
            let p = |k: u32| {
                let q = m.positions[k as usize];
                DVec2::new(q[1] as f64, q[2] as f64)
            };
            m.indices
                .chunks(3)
                .map(|t| [p(t[0]), p(t[1]), p(t[2])])
                .collect()
        };
        // the foundations' cut faces, flat in (y, z)
        let built: Vec<[DVec2; 3]> = cropped
            .iter()
            .filter(|i| {
                !i.ghost
                    && matches!(
                        kind(i.id),
                        Some(ElementKind::Wall(_) | ElementKind::Footing(_))
                    )
            })
            .filter(on_plane)
            .flat_map(|i| tris(&i.mesh))
            .collect();
        assert!(!built.is_empty());
        let in_built = |q: DVec2| {
            built.iter().any(|t| {
                let s = |a: DVec2, b: DVec2| (b - a).perp_dot(q - a);
                let d = [s(t[0], t[1]), s(t[1], t[2]), s(t[2], t[0])];
                d.iter().all(|v| *v >= -1e-12) || d.iter().all(|v| *v <= 1e-12)
            })
        };
        // a point with a foundation's cut face 1 mm round it on all sides
        let deep = |q: DVec2| {
            [DVec2::X, DVec2::NEG_X, DVec2::Y, DVec2::NEG_Y]
                .iter()
                .all(|d| in_built(q + *d * 1e-3))
        };
        let (mut inside, mut through) = (vec![], vec![]);
        let grounds = cropped.iter().filter(|i| {
            !i.ghost
                && matches!(kind(i.id),
                    Some(ElementKind::Site(s)) if matches!(s.form, creabim_core::SiteForm::Terrain { .. }))
        });
        for g in grounds.filter(on_plane) {
            for t in tris(&g.mesh) {
                if deep((t[0] + t[1] + t[2]) / 3.0) {
                    inside.push(t);
                }
            }
            for (a, b) in crate::section::feature_edges(g) {
                let (a, b) = (DVec2::new(a.y, a.z), DVec2::new(b.y, b.z));
                if a.distance(b) > 2e-3 && deep((a + b) / 2.0) {
                    through.push((a, b));
                }
            }
        }
        assert!(
            inside.is_empty(),
            "{} ground triangles in a foundation: {inside:?}",
            inside.len()
        );
        assert!(
            through.is_empty(),
            "{} lines of the ground through a foundation: {through:?}",
            through.len()
        );
    }

    #[test]
    fn what_is_inside_is_untouched_and_what_is_outside_goes() {
        let m = cuboid(DVec3::ONE, DVec3::ZERO);
        let (lo, hi) = bounds(&m);
        let around = CropBox::new(None, lo - DVec3::ONE, hi + DVec3::ONE);
        let (body, caps) = crop_mesh(&m, &[&around], true).unwrap();
        assert_eq!(body, m);
        assert!(caps.is_empty());
        let away = CropBox::new(None, hi + DVec3::ONE, hi + DVec3::splat(2.0));
        assert!(crop_mesh(&m, &[&away], true).is_none());
    }

    #[test]
    fn a_box_for_some_elements_leaves_the_others_whole() {
        let model = Model::new();
        let (a_id, b_id) = (ElementId::new(), ElementId::new());
        let item = |id, at: DVec3| SceneItem {
            id,
            mesh: cuboid(DVec3::splat(2.0), at),
            color: [0.5; 3],
            surface: crate::scene::PLAIN_SURFACE,
            ghost: false,
            layer: None,
            finish: None,
        };
        let mut scene = vec![item(a_id, DVec3::ZERO), item(b_id, DVec3::ZERO)];
        let (lo, hi) = bounds(&scene[0].mesh);
        let mut c = CropBox::new(
            None,
            lo - DVec3::ONE,
            (lo + hi) / 2.0 + DVec3::new(10.0, 10.0, 0.0),
        );
        c.scope = CropScope::Only(vec![a_id]);
        let edges = crop_scene(&model, &mut scene, &[c]);
        assert!(!edges.is_empty(), "the cut is outlined");
        // a: its lower half and a cap on top; b: whole
        let a_items: Vec<&SceneItem> = scene.iter().filter(|i| i.id == a_id).collect();
        assert_eq!(a_items.len(), 2);
        assert!(a_items[1].color[0] < 0.5, "the cap is darker");
        let vol: f64 = a_items.iter().map(|i| i.mesh.volume()).sum();
        assert!((vol - 4.0).abs() < 1e-3, "{vol}");
        let b: Vec<&SceneItem> = scene.iter().filter(|i| i.id == b_id).collect();
        assert_eq!(b.len(), 1);
        assert!((b[0].mesh.volume() - 8.0).abs() < 1e-6);
    }

    #[test]
    fn two_boxes_keep_what_is_inside_both() {
        let m = cuboid(DVec3::splat(4.0), DVec3::ZERO);
        let (lo, _) = bounds(&m);
        let a = CropBox::new(None, lo - DVec3::ONE, lo + DVec3::new(3.0, 10.0, 10.0));
        let b = CropBox::new(
            None,
            lo + DVec3::new(1.0, -1.0, -1.0),
            lo + DVec3::splat(10.0),
        );
        let (body, caps) = crop_mesh(&m, &[&a, &b], true).unwrap();
        let mut whole = body;
        whole.append(&caps);
        // x from 1 to 3 of the 4 m block
        assert!((whole.volume() - 32.0).abs() < 1e-3, "{}", whole.volume());
    }

    #[test]
    fn a_3d_view_on_a_sheet_draws_cropped_by_its_own_boxes() {
        use creabim_core::{Document, Element, Transaction, ViewKind};
        let mut doc = Document::from_model(creabim_core::sample::sample_house());
        let view = doc
            .model()
            .iter()
            .find(|e| matches!(&e.kind, ElementKind::View(v) if matches!(v.kind, ViewKind::Axonometric { .. })))
            .unwrap()
            .id;
        let height = |m: &Model| {
            let (lo, hi) = crate::section::view_drawing(m, view)
                .unwrap()
                .bounds()
                .unwrap();
            hi.y - lo.y
        };
        let whole = height(doc.model());
        // a box over the ground floor, for the 3D window: the view is not cropped
        let low = |owner| {
            Element::new(
                "Crop box 1",
                ElementKind::CropBox(CropBox::new(
                    owner,
                    DVec3::new(-50.0, -50.0, -5.0),
                    DVec3::new(50.0, 50.0, 1.0),
                )),
            )
        };
        doc.commit(Transaction::new("add").insert(low(None)))
            .unwrap();
        assert!((height(doc.model()) - whole).abs() < 1e-6);
        // the view's own box crops its drawing
        doc.commit(Transaction::new("add").insert(low(Some(view))))
            .unwrap();
        let cropped = height(doc.model());
        assert!(cropped < whole * 0.8, "{cropped} vs {whole}");
    }
}
