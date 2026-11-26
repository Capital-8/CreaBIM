//! Stretch (wave 3, decision 3, A): Shift+S or the quick button beside
//! Scale, then a box dragged in a plan. Every corner and end inside the
//! box lights up, across walls, beams, slabs, roofs, outlines, lines and
//! grid lines at once; then they move together, by the pointer or a typed
//! distance, as Move does. What lies wholly inside moves; what the box
//! crosses stretches, only its corners or ends inside moving; the rest
//! stays. Walls joined to a moving end follow it, and rooms follow their
//! walls. Edit mode on elements (MU44) will reuse the same rule.

use crate::editmode;
use crate::handles::Ring;
use creabim_core::transform::transformed;
use creabim_core::{Element, ElementId, ElementKind, Model, Xform};
use glam::{DVec2, DVec3};

/// What of an element a stretch moves.
#[derive(Clone, Debug, PartialEq)]
pub enum Part {
    /// All of it: it lies wholly inside the box.
    Whole,
    /// A wall's, beam's or grid line's start, end or both.
    Ends { start: bool, end: bool },
    /// These corners of its outlines.
    Corners(Vec<(Ring, usize)>),
}

/// The two ends of what runs as a line.
fn line_ends(e: &Element) -> Option<(DVec2, DVec2)> {
    match &e.kind {
        ElementKind::Wall(w) => Some((w.start, w.end)),
        ElementKind::Beam(b) => Some((b.start, b.end)),
        ElementKind::Grid(g) => Some((g.start, g.end)),
        ElementKind::RoomSeparator(s) => Some((s.a, s.b)),
        _ => None,
    }
}

fn inside(lo: DVec2, hi: DVec2, p: DVec2) -> bool {
    p.x >= lo.x && p.x <= hi.x && p.y >= lo.y && p.y <= hi.y
}

/// What a box from `lo` to `hi` (a plan's model coordinates) takes of
/// each of `candidates` (what the plan draws, with the bounds it draws
/// it in), and how many corners and ends that is.
pub fn parts_in(
    model: &Model,
    candidates: &[(ElementId, (DVec2, DVec2))],
    lo: DVec2,
    hi: DVec2,
) -> (Vec<(ElementId, Part)>, usize) {
    let (lo, hi) = (lo.min(hi), lo.max(hi));
    let mut out: Vec<(ElementId, Part)> = vec![];
    let mut corners = 0;
    for (id, (blo, bhi)) in candidates {
        if out.iter().any(|(o, _)| o == id) {
            continue;
        }
        let Some(e) = model.get(*id) else {
            continue;
        };
        if let Some((a, b)) = line_ends(e) {
            let (sa, sb) = (inside(lo, hi, a), inside(lo, hi, b));
            corners += usize::from(sa) + usize::from(sb);
            match (sa, sb) {
                (true, true) => out.push((*id, Part::Whole)),
                (false, false) => {}
                (start, end) => out.push((*id, Part::Ends { start, end })),
            }
            continue;
        }
        let rings = editmode::rings(model, e);
        if !rings.is_empty() {
            let mut picked = vec![];
            let mut all = 0;
            for r in &rings {
                for (i, c) in r.corners.iter().enumerate() {
                    all += 1;
                    if inside(lo, hi, c.truncate()) {
                        picked.push((r.ring, i));
                    }
                }
            }
            corners += picked.len();
            if picked.len() == all {
                out.push((*id, Part::Whole));
            } else if !picked.is_empty() {
                out.push((*id, Part::Corners(picked)));
            }
            continue;
        }
        // a column, a door, furniture, a tag: moved when wholly inside
        if inside(lo, hi, *blo) && inside(lo, hi, *bhi) {
            corners += 1;
            out.push((*id, Part::Whole));
        }
    }
    (out, corners)
}

/// Each of `originals` with its part moved `by`; refused with the reason
/// when an outline would break.
pub fn moved(
    model: &Model,
    originals: &[Element],
    parts: &[(ElementId, Part)],
    by: DVec3,
) -> Result<Vec<Element>, String> {
    let mut out = Vec::with_capacity(originals.len());
    for o in originals {
        let part = parts
            .iter()
            .find(|(id, _)| *id == o.id)
            .map_or(&Part::Whole, |(_, p)| p);
        let now = match part {
            Part::Whole => transformed(model, o, &Xform::Move(by))
                .ok_or_else(|| format!("{} can't take that change", o.name))?,
            Part::Ends { start, end } => {
                let mut e = o.clone();
                let d = by.truncate();
                let ends = match &mut e.kind {
                    ElementKind::Wall(w) => Some((&mut w.start, &mut w.end)),
                    ElementKind::Beam(b) => Some((&mut b.start, &mut b.end)),
                    ElementKind::Grid(g) => Some((&mut g.start, &mut g.end)),
                    ElementKind::RoomSeparator(s) => Some((&mut s.a, &mut s.b)),
                    _ => None,
                };
                if let Some((a, b)) = ends {
                    if *start {
                        *a += d;
                    }
                    if *end {
                        *b += d;
                    }
                    if a.distance(*b) < 0.01 {
                        return Err(format!("That would leave {} with no length.", o.name));
                    }
                }
                e
            }
            Part::Corners(cs) => {
                let mut e = o.clone();
                editmode::move_corners(model, &mut e, cs, by)
                    .map_err(|why| format!("{}: {why}", o.name))?;
                e
            }
        };
        out.push(now);
    }
    Ok(out)
}

/// The points a stretch box lights: the corners and ends it takes, in the
/// model, for drawing as it is dragged.
pub fn lit_points(model: &Model, parts: &[(ElementId, Part)]) -> Vec<DVec2> {
    let mut out = vec![];
    for (id, part) in parts {
        let Some(e) = model.get(*id) else {
            continue;
        };
        match part {
            Part::Ends { start, end } => {
                if let Some((a, b)) = line_ends(e) {
                    if *start {
                        out.push(a);
                    }
                    if *end {
                        out.push(b);
                    }
                }
            }
            Part::Corners(cs) => {
                for (ring, i) in cs {
                    if let Some(c) =
                        editmode::ring_of(model, e, *ring).and_then(|r| r.corners.get(*i).copied())
                    {
                        out.push(c.truncate());
                    }
                }
            }
            Part::Whole => {
                if let Some((a, b)) = line_ends(e) {
                    out.extend([a, b]);
                } else {
                    for r in editmode::rings(model, e) {
                        out.extend(r.corners.iter().map(|c| c.truncate()));
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use creabim_core::{Document, Transaction};

    #[test]
    fn a_box_over_one_end_stretches_the_wall_and_moves_what_is_wholly_inside() {
        let mut doc = Document::from_model(creabim_core::sample::sample_house());
        let model = doc.model();
        let mut w = model
            .iter()
            .find_map(|e| match &e.kind {
                ElementKind::Wall(w) => Some(w.clone()),
                _ => None,
            })
            .unwrap();
        let v = |x, y| DVec2::new(x, y);
        // two walls far from the house: a runs 100 to 104, b sits at 103..104
        (w.start, w.end) = (v(100.0, 0.0), v(104.0, 0.0));
        let a = Element::new("Wall", ElementKind::Wall(w.clone()));
        (w.start, w.end) = (v(103.0, 2.0), v(104.0, 2.0));
        let b = Element::new("Wall", ElementKind::Wall(w));
        let (ia, ib) = (a.id, b.id);
        doc.commit(Transaction::new("Walls").insert(a).insert(b))
            .unwrap();
        let model = doc.model();
        let bounds = |id| {
            let (p, q) = line_ends(model.get(id).unwrap()).unwrap();
            (id, (p.min(q), p.max(q)))
        };
        // the box takes a's end at 104 and all of b
        let (parts, n) = parts_in(
            model,
            &[bounds(ia), bounds(ib)],
            v(102.5, -1.0),
            v(105.0, 3.0),
        );
        assert_eq!(n, 3, "a's one end and b's two");
        assert!(parts.contains(&(
            ia,
            Part::Ends {
                start: false,
                end: true
            }
        )));
        assert!(parts.contains(&(ib, Part::Whole)));
        let originals: Vec<Element> = [ia, ib].map(|id| model.get(id).unwrap().clone()).into();
        let now = moved(model, &originals, &parts, DVec3::new(1.0, 0.0, 0.0)).unwrap();
        assert_eq!(
            line_ends(&now[0]).unwrap(),
            (v(100.0, 0.0), v(105.0, 0.0)),
            "a stretched"
        );
        assert_eq!(
            line_ends(&now[1]).unwrap(),
            (v(104.0, 2.0), v(105.0, 2.0)),
            "b moved"
        );
        // drawn back past its start: refused
        let short = moved(model, &originals[..1], &parts, DVec3::new(-4.0, 0.0, 0.0));
        assert!(short.is_err());
        // a box over nothing takes nothing
        assert!(parts_in(model, &[bounds(ia)], v(0.0, 50.0), v(1.0, 51.0))
            .0
            .is_empty());
    }

    #[test]
    fn a_box_over_two_corners_of_a_slab_moves_only_those() {
        let model = creabim_core::sample::sample_house();
        let slab = model
            .iter()
            .find(|e| matches!(e.kind, ElementKind::Slab(_)))
            .unwrap();
        let ring = editmode::rings(&model, slab).remove(0);
        let cs: Vec<DVec2> = ring.corners.iter().map(|c| c.truncate()).collect();
        // a box round the corners with the largest x
        let xmax = cs.iter().map(|c| c.x).fold(f64::MIN, f64::max);
        let lo = DVec2::new(xmax - 0.01, -1e3);
        let hi = DVec2::new(xmax + 1.0, 1e3);
        let (parts, n) = parts_in(&model, &[(slab.id, (lo, hi))], lo, hi);
        let right = cs.iter().filter(|c| c.x >= xmax - 0.01).count();
        assert_eq!(n, right);
        assert!(right < cs.len());
        let Part::Corners(picked) = &parts[0].1 else {
            panic!("{parts:?}")
        };
        assert_eq!(picked.len(), right);
        let now = moved(
            &model,
            std::slice::from_ref(slab),
            &parts,
            DVec3::new(0.5, 0.0, 0.0),
        )
        .unwrap();
        let after = editmode::rings(&model, &now[0]).remove(0);
        for (before, after) in ring.corners.iter().zip(&after.corners) {
            let dx = after.x - before.x;
            if before.x >= xmax - 0.01 {
                assert!((dx - 0.5).abs() < 1e-6);
            } else {
                assert!(dx.abs() < 1e-6);
            }
        }
    }
}
