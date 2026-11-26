//! A building checked against the lot it stands on: inside the setbacks,
//! under the height limit, within the coverage and floor area allowed,
//! and the site's figures (lot, buildable area, footprint, floor area).
//!
//! What counts as the building: walls, slabs, columns, beams, roofs,
//! stairs and railings whose middle is on the lot. Its footprint is
//! everything of it seen from above, roof overhangs included, measured
//! on a fine grid; its floor area is its slabs' (less their holes); its
//! height is its highest point above the project's zero.

use crate::scene::SceneItem;
use creabim_core::{lot, Element, ElementId, ElementKind, Model, SiteForm};
use glam::{DVec2, DVec3};
use std::collections::HashMap;

/// The figures and checks of one lot.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LotCheck {
    pub area: f64,
    /// What the setbacks leave; `None` when they leave nothing clean.
    pub buildable: Option<Vec<DVec2>>,
    pub buildable_area: f64,
    /// The building's elements on the lot.
    pub building: Vec<ElementId>,
    pub footprint: f64,
    pub floor_area: f64,
    /// Its highest point above the project's zero; `None` with nothing built.
    pub height: Option<f64>,
    /// Its elements reaching past the setbacks.
    pub outside: Vec<ElementId>,
    /// Its elements above an angle plane.
    pub over_planes: Vec<ElementId>,
    /// Its elements on an easement.
    pub on_easements: Vec<ElementId>,
}

impl LotCheck {
    pub fn coverage(&self) -> f64 {
        if self.area > 0.0 {
            self.footprint / self.area
        } else {
            0.0
        }
    }

    pub fn floor_ratio(&self) -> f64 {
        if self.area > 0.0 {
            self.floor_area / self.area
        } else {
            0.0
        }
    }
}

/// One rule, as checked: its name, what the building does, what the rule
/// allows, and whether it keeps to it.
#[derive(Clone, Debug, PartialEq)]
pub struct Verdict {
    pub rule: &'static str,
    pub is: String,
    pub allowed: String,
    pub ok: bool,
}

fn is_building(e: &Element) -> bool {
    matches!(
        e.kind,
        ElementKind::Wall(_)
            | ElementKind::Slab(_)
            | ElementKind::Column(_)
            | ElementKind::Beam(_)
            | ElementKind::Roof(_)
            | ElementKind::Stair(_)
            | ElementKind::Railing(_)
    )
}

/// How far inside a lot's buildable area counts as on its line.
const TOLERANCE: f64 = 0.005;
/// The footprint grid's cell, metres, and the most cells it may have.
const CELL: f64 = 0.05;
const MAX_CELLS: f64 = 4.0e6;

/// The lot `lot` (a property) with `scene`'s building checked against it.
pub fn check(model: &Model, lot: &Element, scene: &[SceneItem]) -> Option<LotCheck> {
    let ElementKind::Site(s) = &lot.kind else {
        return None;
    };
    let SiteForm::Property {
        boundary,
        edges,
        rules,
    } = &s.form
    else {
        return None;
    };
    let mut out = LotCheck {
        area: lot::area(boundary),
        buildable: lot::buildable(boundary, edges, rules),
        ..Default::default()
    };
    out.buildable_area = out.buildable.as_deref().map_or(0.0, lot::area);
    // each element's triangles, gathered
    let mut meshes: HashMap<ElementId, Vec<[DVec2; 3]>> = HashMap::new();
    let mut top = f64::NEG_INFINITY;
    let mut tops: HashMap<ElementId, f64> = HashMap::new();
    let mut points: HashMap<ElementId, Vec<DVec3>> = HashMap::new();
    for item in scene.iter().filter(|i| !i.ghost) {
        let Some(e) = model.get(item.id).filter(|e| is_building(e)) else {
            continue;
        };
        let p = &item.mesh.positions;
        let tris = meshes.entry(e.id).or_default();
        let idx = &item.mesh.indices;
        let at = |k: u32| {
            let v = p[k as usize];
            DVec2::new(v[0] as f64, v[1] as f64)
        };
        for t in 0..idx.len() / 3 {
            tris.push([at(idx[3 * t]), at(idx[3 * t + 1]), at(idx[3 * t + 2])]);
        }
        let z = p
            .iter()
            .map(|v| v[2] as f64)
            .fold(f64::NEG_INFINITY, f64::max);
        let t = tops.entry(e.id).or_insert(f64::NEG_INFINITY);
        *t = t.max(z);
        points.entry(e.id).or_default().extend(
            p.iter()
                .map(|v| DVec3::new(v[0] as f64, v[1] as f64, v[2] as f64)),
        );
    }
    let strips: Vec<Vec<DVec2>> = rules
        .easements
        .iter()
        .filter_map(|e| lot::easement_outline(boundary, e))
        .collect();
    // the elements whose middle is on the lot
    let mut ids: Vec<ElementId> = meshes.keys().copied().collect();
    ids.sort();
    let mut lo = DVec2::splat(f64::INFINITY);
    let mut hi = DVec2::splat(f64::NEG_INFINITY);
    for id in ids {
        let tris = &meshes[&id];
        let pts = tris.iter().flatten();
        let (a, b) = pts.fold(
            (DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY)),
            |(a, b), p| (a.min(*p), b.max(*p)),
        );
        if !a.is_finite() || !creabim_core::polygon::contains(boundary, (a + b) / 2.0) {
            continue;
        }
        out.building.push(id);
        lo = lo.min(a);
        hi = hi.max(b);
        top = top.max(tops[&id]);
        let past = match &out.buildable {
            Some(area) => tris
                .iter()
                .flatten()
                .any(|p| !inside_or_on(area, *p, TOLERANCE)),
            // with nothing left to build on, all of it is past the setbacks
            None => true,
        };
        if past {
            out.outside.push(id);
        }
        // above an angle plane, a centimetre to spare
        if rules.angle > 0.0
            && points[&id].iter().any(|q| {
                lot::envelope_top(boundary, edges, rules, q.truncate())
                    .is_some_and(|t| q.z > t + 0.01)
            })
        {
            out.over_planes.push(id);
        }
        // on an easement: a corner of it inside the strip, or the strip's
        // corner under it
        let on = strips.iter().any(|st| {
            tris.iter().flatten().any(|q| strictly_inside(st, *q))
                || st.iter().any(|c| tris.iter().any(|t| in_triangle(t, *c)))
        });
        if on {
            out.on_easements.push(id);
        }
        if let Some(ElementKind::Slab(sl)) = model.get(id).map(|e| &e.kind) {
            out.floor_area +=
                lot::area(&sl.boundary) - sl.holes.iter().map(|h| lot::area(h)).sum::<f64>();
        }
    }
    if top.is_finite() {
        out.height = Some(top);
        out.footprint = footprint(out.building.iter().flat_map(|id| meshes[id].iter()), lo, hi);
    }
    Some(out)
}

/// Is `p` inside `poly` and more than the tolerance from its edges?
fn strictly_inside(poly: &[DVec2], p: DVec2) -> bool {
    creabim_core::polygon::contains(poly, p) && !inside_or_on_edge(poly, p, TOLERANCE)
}

fn inside_or_on_edge(poly: &[DVec2], p: DVec2, tol: f64) -> bool {
    let n = poly.len();
    (0..n).any(|i| {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        let d = b - a;
        let t = ((p - a).dot(d) / d.length_squared().max(1e-18)).clamp(0.0, 1.0);
        (a + d * t).distance(p) <= tol
    })
}

/// Is `p` well inside triangle `t` (seen from above)?
fn in_triangle(t: &[DVec2; 3], p: DVec2) -> bool {
    let a2 = (t[1] - t[0]).perp_dot(t[2] - t[0]);
    if a2.abs() < 1e-9 {
        return false;
    }
    let s = |a: DVec2, b: DVec2| (b - a).perp_dot(p - a) * a2.signum() / (b - a).length();
    s(t[0], t[1]) > TOLERANCE && s(t[1], t[2]) > TOLERANCE && s(t[2], t[0]) > TOLERANCE
}

/// Is `p` inside `poly`, or within `tol` of its edges?
fn inside_or_on(poly: &[DVec2], p: DVec2, tol: f64) -> bool {
    if creabim_core::polygon::contains(poly, p) {
        return true;
    }
    let n = poly.len();
    (0..n).any(|i| {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        let d = b - a;
        let t = ((p - a).dot(d) / d.length_squared().max(1e-18)).clamp(0.0, 1.0);
        (a + d * t).distance(p) <= tol
    })
}

/// The area the triangles cover seen from above, counted on a grid.
fn footprint<'a>(tris: impl Iterator<Item = &'a [DVec2; 3]>, lo: DVec2, hi: DVec2) -> f64 {
    let size = hi - lo;
    let cell = CELL.max((size.x * size.y / MAX_CELLS).sqrt());
    let (w, h) = (
        (size.x / cell).ceil() as usize + 1,
        (size.y / cell).ceil() as usize + 1,
    );
    let mut on = vec![false; w * h];
    for t in tris {
        let area2 = (t[1] - t[0]).perp_dot(t[2] - t[0]);
        if area2.abs() < 1e-12 {
            continue; // upright faces cover nothing from above
        }
        let (a, b) = (t[0].min(t[1]).min(t[2]), t[0].max(t[1]).max(t[2]));
        let (i0, i1) = (
            ((a.x - lo.x) / cell).floor().max(0.0) as usize,
            (((b.x - lo.x) / cell).ceil() as usize).min(w - 1),
        );
        let (j0, j1) = (
            ((a.y - lo.y) / cell).floor().max(0.0) as usize,
            (((b.y - lo.y) / cell).ceil() as usize).min(h - 1),
        );
        for j in j0..=j1 {
            for i in i0..=i1 {
                let c = lo + DVec2::new(i as f64 + 0.5, j as f64 + 0.5) * cell;
                let s = |p: DVec2, q: DVec2| (q - p).perp_dot(c - p) * area2.signum();
                if s(t[0], t[1]) >= 0.0 && s(t[1], t[2]) >= 0.0 && s(t[2], t[0]) >= 0.0 {
                    on[j * w + i] = true;
                }
            }
        }
    }
    on.iter().filter(|v| **v).count() as f64 * cell * cell
}

/// The lot's rules, each as checked: the setbacks, then the limits set.
/// `len` and `area` write a length and an area in the project's units.
pub fn verdicts(
    check: &LotCheck,
    rules: &creabim_core::PlanningRules,
    len: impl Fn(f64) -> String,
) -> Vec<Verdict> {
    let mut out = vec![];
    if !check.building.is_empty() {
        // a lot with no setbacks: the building is checked against its
        // boundary
        out.push(Verdict {
            rule: if rules.has_setbacks {
                "Setbacks"
            } else {
                "Boundary"
            },
            is: match (check.outside.len(), check.buildable.is_some()) {
                (_, false) => "they leave nothing to build on".into(),
                (0, _) => "all inside".into(),
                (1, _) => "1 element past them".into(),
                (n, _) => format!("{n} elements past them"),
            },
            // nothing to compare: inside is the rule
            allowed: String::new(),
            ok: check.outside.is_empty(),
        });
    }
    if rules.angle > 0.0 && !check.building.is_empty() {
        out.push(Verdict {
            rule: "Angle planes",
            is: match check.over_planes.len() {
                0 => "all under".into(),
                1 => "1 element above them".into(),
                n => format!("{n} elements above them"),
            },
            allowed: String::new(),
            ok: check.over_planes.is_empty(),
        });
    }
    if !rules.easements.is_empty() && !check.building.is_empty() {
        out.push(Verdict {
            rule: "Easements",
            is: match check.on_easements.len() {
                0 => "kept clear".into(),
                1 => "1 element on them".into(),
                n => format!("{n} elements on them"),
            },
            allowed: String::new(),
            ok: check.on_easements.is_empty(),
        });
    }
    if let (true, Some(h)) = (rules.height > 0.0, check.height) {
        out.push(Verdict {
            rule: "Height",
            is: len(h),
            allowed: len(rules.height),
            ok: h <= rules.height + 1e-6,
        });
    }
    let pc = |v: f64| format!("{:.1}%", v * 100.0);
    if rules.coverage > 0.0 {
        out.push(Verdict {
            rule: "Coverage",
            is: pc(check.coverage()),
            allowed: pc(rules.coverage),
            ok: check.coverage() <= rules.coverage + 1e-9,
        });
    }
    if rules.floor_ratio > 0.0 {
        out.push(Verdict {
            rule: "Floor area ratio",
            is: format!("{:.2}", check.floor_ratio()),
            allowed: format!("{:.2}", rules.floor_ratio),
            ok: check.floor_ratio() <= rules.floor_ratio + 1e-9,
        });
    }
    out
}

/// The envelope the building may fill: the buildable area from the
/// project's zero up to the height limit, cut by the angle planes. `None`
/// with no buildable area, or nothing limiting the height.
pub fn envelope_mesh(
    boundary: &[DVec2],
    edges: &[creabim_core::LotEdge],
    rules: &creabim_core::PlanningRules,
) -> Option<crate::mesh::TriMesh> {
    let base = lot::buildable(boundary, edges, rules)?;
    let planes = lot::angle_planes(boundary, edges, rules);
    let top = if rules.height > 0.0 {
        rules.height
    } else if !planes.is_empty() {
        // as high as a plane could reach over the lot
        let (lo, hi) = base.iter().fold(
            (DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY)),
            |(a, b), p| (a.min(*p), b.max(*p)),
        );
        rules.angle_from + (hi - lo).length() * rules.angle.to_radians().tan() + 1.0
    } else {
        return None;
    };
    let n = base.len();
    let mut faces: Vec<Vec<DVec3>> = vec![
        base.iter().rev().map(|p| p.extend(0.0)).collect(),
        base.iter().map(|p| p.extend(top)).collect(),
    ];
    for i in 0..n {
        let (a, b) = (base[i], base[(i + 1) % n]);
        faces.push(vec![
            a.extend(0.0),
            b.extend(0.0),
            b.extend(top),
            a.extend(top),
        ]);
    }
    for (normal, off) in planes {
        faces = clip(faces, normal, off);
    }
    let mut m = crate::mesh::TriMesh::default();
    for f in &faces {
        add_face(&mut m, f);
    }
    (!m.is_empty()).then_some(m)
}

/// A solid's faces cut by a plane, keeping `normal · x <= off`, the cut
/// closed by a new face (the hull of where the faces were cut).
fn clip(faces: Vec<Vec<DVec3>>, normal: DVec3, off: f64) -> Vec<Vec<DVec3>> {
    const EPS: f64 = 1e-9;
    let mut out = vec![];
    let mut cut = vec![];
    for f in faces {
        let mut g = vec![];
        let k = f.len();
        for i in 0..k {
            let (a, b) = (f[i], f[(i + 1) % k]);
            let (da, db) = (normal.dot(a) - off, normal.dot(b) - off);
            if da <= EPS {
                g.push(a);
            }
            if da.abs() <= EPS {
                cut.push(a);
            }
            if (da < -EPS && db > EPS) || (da > EPS && db < -EPS) {
                let p = a + (b - a) * (da / (da - db));
                g.push(p);
                cut.push(p);
            }
        }
        if g.len() >= 3 {
            out.push(g);
        }
    }
    // the cut face: the hull of the cut points on the plane, facing out
    let u = normal.any_orthonormal_vector();
    let v = normal.normalize().cross(u);
    let mut pts: Vec<(DVec2, DVec3)> = cut
        .iter()
        .map(|p| (DVec2::new(p.dot(u), p.dot(v)), *p))
        .collect();
    pts.sort_by(|a, b| a.0.x.total_cmp(&b.0.x).then(a.0.y.total_cmp(&b.0.y)));
    pts.dedup_by(|a, b| a.0.distance(b.0) < 1e-7);
    if pts.len() >= 3 {
        let turn = |o: DVec2, a: DVec2, b: DVec2| (a - o).perp_dot(b - o);
        let mut hull: Vec<(DVec2, DVec3)> = vec![];
        for pass in [false, true] {
            let start = hull.len();
            let it: Box<dyn Iterator<Item = &(DVec2, DVec3)>> = if pass {
                Box::new(pts.iter().rev())
            } else {
                Box::new(pts.iter())
            };
            for p in it {
                while hull.len() >= start + 2
                    && turn(hull[hull.len() - 2].0, hull[hull.len() - 1].0, p.0) <= 1e-12
                {
                    hull.pop();
                }
                hull.push(*p);
            }
            hull.pop();
        }
        if hull.len() >= 3 {
            out.push(hull.into_iter().map(|h| h.1).collect());
        }
    }
    out
}

/// A flat face of any shape into triangles.
fn add_face(m: &mut crate::mesh::TriMesh, f: &[DVec3]) {
    let k = f.len();
    let mut nrm = DVec3::ZERO;
    for i in 0..k {
        let (a, b) = (f[i], f[(i + 1) % k]);
        nrm += DVec3::new(
            (a.y - b.y) * (a.z + b.z),
            (a.z - b.z) * (a.x + b.x),
            (a.x - b.x) * (a.y + b.y),
        );
    }
    if nrm.length() < 1e-12 {
        return;
    }
    let u = nrm.any_orthonormal_vector();
    let v = nrm.normalize().cross(u);
    let flat: Vec<DVec2> = f.iter().map(|p| DVec2::new(p.dot(u), p.dot(v))).collect();
    let tris = creabim_core::polygon::triangulate(&flat);
    m.add_polygon(f, &tris, nrm);
}

/// Every property in the model, checked: for the site statistics.
pub fn all(model: &Model, scene: &[SceneItem]) -> Vec<(ElementId, LotCheck)> {
    model
        .iter()
        .filter_map(|e| Some((e.id, check(model, e, scene)?)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use creabim_core::{Site, Transaction};

    fn lot_of(model: &Model) -> &Element {
        model
            .iter()
            .find(|e| matches!(&e.kind, ElementKind::Site(s) if s.is_property()))
            .unwrap()
    }

    #[test]
    fn the_sample_house_is_checked_on_its_lot() {
        let mut doc = creabim_core::Document::from_model(creabim_core::sample::sample_house());
        let level = doc
            .model()
            .iter()
            .find(|e| matches!(e.kind, ElementKind::Level(_)))
            .unwrap()
            .id;
        // the house's walls, to put a lot round them
        let scene = crate::scene::build_scene(doc.model());
        let (lo, hi) = scene
            .iter()
            .filter(|i| doc.model().get(i.id).is_some_and(is_building))
            .flat_map(|i| i.mesh.positions.iter())
            .fold(
                (DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY)),
                |(a, b), p| {
                    let p = DVec2::new(p[0] as f64, p[1] as f64);
                    (a.min(p), b.max(p))
                },
            );
        let pad = DVec2::new(8.0, 10.0);
        let (a, b) = (lo - pad, hi + pad);
        let lot = Site::property(
            level,
            vec![a, DVec2::new(b.x, a.y), b, DVec2::new(a.x, b.y)],
        );
        doc.commit(Transaction::new("Lot").insert(Element::new("Lot", ElementKind::Site(lot))))
            .unwrap();
        let m = doc.model();
        let c = check(m, lot_of(m), &scene).unwrap();
        let size = b - a;
        assert!((c.area - size.x * size.y).abs() < 1e-6);
        assert!(!c.building.is_empty());
        // 8 m to the sides and 10 m front and rear clear 3 m and 6 m setbacks
        assert!(c.outside.is_empty(), "{:?}", c.outside);
        // the footprint is the house's outline, near enough
        let house = (hi - lo).x * (hi - lo).y;
        assert!(c.footprint > 0.5 * house && c.footprint <= house * 1.01);
        assert!(c.floor_area > 0.0 && c.height.unwrap() > 2.0);
        // a lot only 1 m clear of it on every side breaks the setbacks
        let tight = Site::property(
            level,
            vec![
                lo - DVec2::ONE,
                DVec2::new(hi.x + 1.0, lo.y - 1.0),
                hi + DVec2::ONE,
                DVec2::new(lo.x - 1.0, hi.y + 1.0),
            ],
        );
        let e = Element::new("Tight", ElementKind::Site(tight));
        let c = check(m, &e, &scene).unwrap();
        assert!(!c.outside.is_empty());
        let rules = creabim_core::PlanningRules {
            height: 1.0,
            coverage: 0.9,
            ..Default::default()
        };
        let v = verdicts(&c, &rules, |d| format!("{d:.2}"));
        assert_eq!(
            v.iter().map(|v| v.rule).collect::<Vec<_>>(),
            ["Setbacks", "Height", "Coverage"]
        );
        assert!(!v[0].ok && !v[1].ok && v[2].ok);
        // the roomy lot with low, shallow angle planes and an easement
        // over the whole front half: both broken
        let ElementKind::Site(roomy) = &lot_of(m).kind else {
            panic!()
        };
        let mut roomy = roomy.clone();
        if let SiteForm::Property { rules, .. } = &mut roomy.form {
            rules.angle = 10.0;
            rules.angle_from = 0.5;
            rules.easements.push(creabim_core::Easement {
                name: "Sewer".into(),
                edge: 0,
                width: size.y / 2.0,
            });
        }
        let e = Element::new("Roomy", ElementKind::Site(roomy.clone()));
        let c = check(m, &e, &scene).unwrap();
        assert!(c.outside.is_empty());
        assert!(!c.over_planes.is_empty() && !c.on_easements.is_empty());
        let SiteForm::Property { rules, .. } = &roomy.form else {
            panic!()
        };
        let v = verdicts(&c, rules, |d| format!("{d:.2}"));
        assert!(v.iter().any(|v| v.rule == "Angle planes" && !v.ok));
        assert!(v.iter().any(|v| v.rule == "Easements" && !v.ok));
    }

    #[test]
    fn the_envelope_is_cut_by_the_angle_planes() {
        let lot = vec![
            DVec2::ZERO,
            DVec2::new(20.0, 0.0),
            DVec2::new(20.0, 30.0),
            DVec2::new(0.0, 30.0),
        ];
        let rules = creabim_core::PlanningRules {
            height: 9.0,
            angle: 45.0,
            angle_from: 3.0,
            ..Default::default()
        };
        let m = envelope_mesh(&lot, &[], &rules).unwrap();
        let (lo, hi) = m.bounds().unwrap();
        // the setbacks' box, no higher than the limit
        assert!((lo.x - 3.0).abs() < 1e-4 && (hi.x - 17.0).abs() < 1e-4);
        assert!((hi.z - 9.0).abs() < 1e-4 && lo.z.abs() < 1e-4);
        // at the side setback the side plane allows 6 m: no point there above it
        for p in &m.positions {
            let q = DVec2::new(p[0] as f64, p[1] as f64);
            let t = lot::envelope_top(&lot, &[], &rules, q).unwrap();
            assert!(p[2] as f64 <= t + 1e-3, "{p:?} over {t}");
        }
        // it is closed: every edge shared by two triangles
        let vol = m.volume();
        assert!(vol > 0.0 && vol < 14.0 * 18.0 * 9.0);
        assert!(envelope_mesh(&lot, &[], &creabim_core::PlanningRules::default()).is_none());
    }

    #[test]
    fn a_footprint_counts_overlaps_once() {
        let sq = |o: DVec2| {
            [
                [o, o + DVec2::new(2.0, 0.0), o + DVec2::new(2.0, 2.0)],
                [o, o + DVec2::new(2.0, 2.0), o + DVec2::new(0.0, 2.0)],
            ]
        };
        let tris: Vec<[DVec2; 3]> = sq(DVec2::ZERO)
            .into_iter()
            .chain(sq(DVec2::new(1.0, 0.0)))
            .collect();
        let a = footprint(tris.iter(), DVec2::ZERO, DVec2::new(3.0, 2.0));
        assert!((a - 6.0).abs() < 0.05, "{a}");
    }
}
