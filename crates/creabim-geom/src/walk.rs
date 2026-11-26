//! Paths on the ground: their solids, laid on the ground as it is after
//! the works, and their plan, tinted and patterned by their surface.

use crate::mesh::TriMesh;
use crate::plan::{Pen, PlanItem};
use creabim_core::walk::{Surface, Walk};
use creabim_core::{members, ElementId, Model, Site};
use glam::{DVec2, DVec3};

/// How far a path's top stands over the ground, so it shows on it.
const LIFT: f64 = 0.03;
/// How far its edging stands over its surface.
const EDGING_UP: f64 = 0.02;
/// The edging's colour.
pub const EDGING_COLOR: [f32; 3] = [0.30, 0.28, 0.26];
/// How often along a path the ground under it is read, metres.
const STEP: f64 = 0.5;

/// The ground's height at `p` after the works, or the site level's.
fn ground(model: &Model, s: &Site, p: DVec2) -> f64 {
    crate::services::grounds_at(model, p)
        .map(|g| g.1)
        .unwrap_or_else(|| members::elevation(model, s.level))
}

/// A band between two lines (each from the path's first point to its
/// last) laid on the ground, `up` over it at its top and `depth` deep:
/// a closed solid.
fn band(model: &Model, s: &Site, a: &[DVec2], b: &[DVec2], up: f64, depth: f64) -> TriMesh {
    let mut m = TriMesh::default();
    // the cross lines along it, each pair of points with its ground
    let mut cross: Vec<(DVec3, DVec3)> = vec![];
    for i in 0..a.len().saturating_sub(1) {
        let len = a[i].lerp(b[i], 0.5).distance(a[i + 1].lerp(b[i + 1], 0.5));
        let n = ((len / STEP).ceil() as usize).max(1);
        for k in 0..=n {
            if i > 0 && k == 0 {
                continue;
            }
            let t = k as f64 / n as f64;
            let (p, q) = (a[i].lerp(a[i + 1], t), b[i].lerp(b[i + 1], t));
            cross.push((
                p.extend(ground(model, s, p) + up),
                q.extend(ground(model, s, q) + up),
            ));
        }
    }
    let down = DVec3::Z * depth;
    for w in cross.windows(2) {
        let ((a0, b0), (a1, b1)) = (w[0], w[1]);
        let top_n = (a1 - a0).cross(b0 - a0);
        let top_n = if top_n.z < 0.0 { -top_n } else { top_n };
        m.add_quad([a0, a1, b1, b0], top_n);
        m.add_quad([a0 - down, b0 - down, b1 - down, a1 - down], -top_n);
        let side = |p: DVec3, q: DVec3, away: DVec3| {
            let along = q - p;
            let n = DVec3::new(along.y, -along.x, 0.0);
            if n.dot(away) < 0.0 {
                -n
            } else {
                n
            }
        };
        m.add_quad([a0, a0 - down, a1 - down, a1], side(a0, a1, a0 - b0));
        m.add_quad([b0, b1, b1 - down, b0 - down], side(b0, b1, b0 - a0));
    }
    if let (Some(f), Some(l)) = (cross.first(), cross.last()) {
        for ((p, q), toward) in [
            (*f, cross[1.min(cross.len() - 1)].0),
            (*l, cross[cross.len().saturating_sub(2)].0),
        ] {
            let away = (p - toward) * DVec3::new(1.0, 1.0, 0.0);
            m.add_quad([p, q, q - down, p - down], away);
        }
    }
    m
}

/// A path's solids: its surface and make-up, and its edging each side.
pub fn walk_meshes(model: &Model, s: &Site, w: &Walk) -> Vec<(TriMesh, [f32; 3], bool)> {
    // as it is laid: its bends rounded
    // as it is laid: its bends rounded, its edges paired with its line
    let mid = w.line();
    if mid.len() < 2 {
        return vec![];
    }
    let half = w.width / 2.0;
    let (l, r) = w.edges(half);
    let mut out = vec![(
        band(model, s, &l, &r, LIFT, w.thickness + LIFT),
        w.surface.rgb(),
        true,
    )];
    if w.edging > 0.0 {
        let (lo, ro) = w.edges(half + w.edging);
        let mut e = band(
            model,
            s,
            &lo,
            &l,
            LIFT + EDGING_UP,
            w.thickness + LIFT + EDGING_UP,
        );
        e.append(&band(
            model,
            s,
            &r,
            &ro,
            LIFT + EDGING_UP,
            w.thickness + LIFT + EDGING_UP,
        ));
        out.push((e, EDGING_COLOR, false));
    }
    out
}

fn line(out: &mut Vec<PlanItem>, id: ElementId, pen: Pen, a: DVec2, b: DVec2) {
    out.push(PlanItem::Line { id, pen, a, b });
}

fn polyline(out: &mut Vec<PlanItem>, id: ElementId, pen: Pen, pts: &[DVec2]) {
    for w in pts.windows(2) {
        line(out, id, pen, w[0], w[1]);
    }
}

/// A cheap repeatable scatter: 0..1 from a point's grid cell.
fn hash(i: i64, j: i64, k: u64) -> f64 {
    let mut h = (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (j as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ k.wrapping_mul(0x1656_67B1_9E37_79F9);
    h ^= h >> 31;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 29;
    (h >> 11) as f64 / (1u64 << 53) as f64
}

/// A path in plan: tinted by its surface between its edging, its edges,
/// its edging's inner lines, and its surface's pattern: gravel stippled,
/// paving jointed, bark flecked; with `levels`, what it is written along
/// it.
pub fn walk_plan(out: &mut Vec<PlanItem>, id: ElementId, w: &Walk, levels: bool) {
    // as it is laid: its bends rounded, its edges paired with its line
    let mid = w.line();
    if mid.len() < 2 {
        return;
    }
    let half = w.width / 2.0;
    let (l, r) = w.edges(half);
    let mut ring = r.clone();
    ring.extend(l.iter().rev());
    out.push(PlanItem::Tint {
        id,
        points: ring.clone(),
        rgb: w.surface.rgb(),
    });
    let (lo, ro) = w.edges(half + w.edging);
    polyline(out, id, Pen::Projection, &lo);
    polyline(out, id, Pen::Projection, &ro);
    line(out, id, Pen::Projection, lo[0], ro[0]);
    let n = lo.len() - 1;
    line(out, id, Pen::Projection, lo[n], ro[n]);
    if w.edging > 0.0 {
        polyline(out, id, Pen::Fine, &l);
        polyline(out, id, Pen::Fine, &r);
    }
    for i in 0..mid.len() - 1 {
        let (a0, a1, b0, b1) = (l[i], l[i + 1], r[i], r[i + 1]);
        let len = mid[i].distance(mid[i + 1]);
        match w.surface {
            Surface::Paving => {
                // joints square across every 600, kept off the bends
                let dir = (mid[i + 1] - mid[i]).normalize_or(DVec2::X);
                let side = dir.perp() * half;
                let keep = |k: usize| {
                    if k == 0 || k == mid.len() - 1 {
                        0.0
                    } else {
                        half
                    }
                };
                let (from, to) = (keep(i), len - keep(i + 1));
                let n = (len / 0.6).round().max(1.0) as usize;
                for k in 1..n {
                    let t = len * k as f64 / n as f64;
                    if t <= from + 0.05 || t >= to - 0.05 {
                        continue;
                    }
                    let c = mid[i] + dir * t;
                    line(out, id, Pen::Fine, c + side, c - side);
                }
            }
            Surface::Gravel | Surface::Bark => {
                // dots (or flecks) scattered on a grid, each nudged
                let gap = if w.surface == Surface::Gravel {
                    0.18
                } else {
                    0.25
                };
                let along = ((len / gap) as i64).max(1);
                let across = ((w.width / gap) as i64).max(1);
                let dot = if w.surface == Surface::Gravel {
                    0.025
                } else {
                    0.07
                };
                for k in 0..along {
                    for j in 0..across {
                        let t = (k as f64 + 0.15 + 0.7 * hash(k, j, i as u64)) / along as f64;
                        let u = (j as f64 + 0.15 + 0.7 * hash(j, k, 7 + i as u64)) / across as f64;
                        let p = a0.lerp(a1, t).lerp(b0.lerp(b1, t), u);
                        let turn = DVec2::from_angle(hash(k, j, 99) * std::f64::consts::TAU);
                        line(out, id, Pen::Fine, p - turn * dot, p + turn * dot);
                    }
                }
            }
            Surface::Asphalt => {}
        }
    }
    if w.surface == Surface::Paving {
        // the courses along it, between the joints
        let courses = (w.width / 0.6).round().max(1.0) as usize;
        for c in 1..courses {
            let off = half - w.width * c as f64 / courses as f64;
            let (course, _) = w.edges(off);
            polyline(out, id, Pen::Fine, &course);
        }
    }
    if levels && w.length() > 3.0 {
        // its name along its longest stretch
        let (i, _) = mid
            .windows(2)
            .enumerate()
            .max_by(|a, b| a.1[0].distance(a.1[1]).total_cmp(&b.1[0].distance(b.1[1])))
            .unwrap();
        let at = mid[i].lerp(mid[i + 1], 0.5);
        let side = (mid[i + 1] - mid[i]).normalize_or(DVec2::X).perp();
        let side = if side.y < 0.0 { -side } else { side };
        out.push(crate::parts::ground_text(
            id,
            at + side * (half + w.edging + 0.45),
            format!("{} path {:.1} m", w.surface.label(), w.width),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use creabim_core::{Element, ElementKind, Level};

    #[test]
    fn a_path_lies_on_the_ground_and_is_drawn_with_its_edging() {
        let lv = Element::new(
            "Ground",
            ElementKind::Level(Level {
                elevation: 0.0,
                height: 3.0,
            }),
        );
        let s = Site::walk(
            lv.id,
            vec![DVec2::ZERO, DVec2::new(6.0, 0.0), DVec2::new(6.0, 4.0)],
            Surface::Paving,
        );
        let creabim_core::SiteForm::Walk(w) = s.form.clone() else {
            unreachable!()
        };
        let m = Model::from_elements([lv]).unwrap();
        let meshes = walk_meshes(&m, &s, &w);
        assert_eq!(meshes.len(), 2);
        let (lo, hi) = meshes[0].0.bounds().unwrap();
        assert!((hi.z - LIFT as f32).abs() < 1e-4, "{hi:?}");
        assert!((lo.z + w.thickness as f32).abs() < 1e-4, "{lo:?}");
        let mut items = vec![];
        walk_plan(&mut items, ElementId::new(), &w, true);
        assert!(items.iter().any(|i| matches!(i, PlanItem::Tint { .. })));
        let joints = items
            .iter()
            .filter(|i| matches!(i, PlanItem::Line { pen: Pen::Fine, .. }))
            .count();
        assert!(joints > 10, "{joints}");
    }
}
