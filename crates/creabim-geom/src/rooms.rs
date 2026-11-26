//! Rooms, room separators and ceilings in drawings: the rooms' floors (to
//! pick them by, and washed in colour when the view says so), separation
//! lines, ceiling plans, and rooms shaded in sections.

use crate::plan::{Pen, PlanItem};
use creabim_core::objects::{self, Mount};
use creabim_core::room::{self, Room};
use creabim_core::{ElementId, ElementKind, Model, OpeningKind, View};
use glam::DVec2;

/// Pale washes for rooms, in turn.
const PALETTE: [[f32; 3]; 8] = [
    [0.99, 0.85, 0.62],
    [0.74, 0.87, 0.97],
    [0.78, 0.92, 0.74],
    [0.96, 0.78, 0.80],
    [0.86, 0.80, 0.95],
    [0.97, 0.93, 0.65],
    [0.72, 0.90, 0.88],
    [0.93, 0.84, 0.74],
];

/// A room's wash: one colour for each department (or, without one, each
/// name), the same every time.
pub fn colour_of(name: &str, r: &Room) -> [f32; 3] {
    let key = if r.department.is_empty() {
        name
    } else {
        &r.department
    };
    let h = key
        .bytes()
        .fold(7u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32));
    PALETTE[h as usize % PALETTE.len()]
}

/// What a plan of `level` draws of its rooms, room separators and (in a
/// ceiling plan) ceilings. Rooms go first, under everything.
pub fn plan_items(model: &Model, view: &View, level: ElementId, cut_height: f64) -> Vec<PlanItem> {
    let mut out = vec![];
    for e in model.iter() {
        match &e.kind {
            ElementKind::Room(r) if r.level == level => {
                if let Some(s) = room::shape_of(model, r) {
                    out.push(PlanItem::Room {
                        id: e.id,
                        points: s.outline,
                        rgb: view.room_colours.then(|| colour_of(&e.name, r)),
                    });
                }
            }
            ElementKind::RoomSeparator(s) if s.level == level => out.push(PlanItem::Line {
                id: e.id,
                pen: Pen::Symbol,
                a: s.a,
                b: s.b,
            }),
            ElementKind::Ceiling(c) if c.level == level && view.ceiling_plan => {
                if c.height < cut_height {
                    continue;
                }
                let Some(mut outline) = c.outline(model) else {
                    continue;
                };
                // from its lowest-left corner, so the drawing is the same
                // every time (a room's shape can start at any corner)
                if let Some(i) = (0..outline.len()).min_by(|a, b| {
                    let (p, q) = (outline[*a], outline[*b]);
                    (p.y, p.x)
                        .partial_cmp(&(q.y, q.x))
                        .unwrap_or(std::cmp::Ordering::Equal)
                }) {
                    outline.rotate_left(i);
                }
                out.push(PlanItem::Polygon {
                    id: e.id,
                    pen: Pen::Projection,
                    points: outline.clone(),
                });
                // its height is its tag's ("CH 2700", issue #70), drawn with
                // the view's annotation (MU142, see `ceiling_height_text`)
                if c.grid > 0.0 {
                    // a square grid, its lines from the outline's corner
                    for (a, b) in grid_lines(&outline, c.grid) {
                        out.push(PlanItem::Line {
                            id: e.id,
                            pen: Pen::Symbol,
                            a,
                            b,
                        });
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// What a ceiling's height tag reads: its prefix ("CH" unless set) and the
/// ceiling's height above its floor, as ceiling plans write it (issue #70).
pub fn ceiling_height_text(model: &Model, prefix: &str, c: &creabim_core::Ceiling) -> String {
    let h = model.units().length(c.height);
    let h = h.trim_end_matches(" mm");
    if prefix.is_empty() {
        h.to_string()
    } else {
        format!("{prefix} {h}")
    }
}

/// Is `id` a tag on a ceiling?
fn is_ceiling_tag(model: &Model, id: ElementId) -> bool {
    match model.get(id).map(|e| &e.kind) {
        Some(ElementKind::Tag(t)) => {
            matches!(
                model.get(t.host).map(|h| &h.kind),
                Some(ElementKind::Ceiling(_))
            )
        }
        _ => false,
    }
}

/// Move each ceiling's height tag off the marks drawn over the plan
/// (section marks, grid bubbles, tags, notes), to the nearest clear spot a
/// few text heights away (MU94: the study's sat under a section mark). A
/// tag put by hand stays put. Ceiling grid lines don't count: a tag is
/// read across them.
pub fn clear_ceiling_tags(model: &Model, items: &mut [PlanItem], mm: f64) {
    let marks: Vec<crate::declutter::TextBox> = items
        .iter()
        .filter(|it| {
            !matches!(
                model.get(it.id()).map(|e| &e.kind),
                Some(ElementKind::Ceiling(_))
            ) && !is_ceiling_tag(model, it.id())
        })
        .filter_map(|it| match it {
            // section and elevation marks, bubbles drawn with a symbol
            PlanItem::Symbol {
                symbol,
                at,
                unit,
                angle,
                origin,
                ..
            } => crate::symbol::shapes_at(symbol, *at, DVec2::splat(*unit), *angle, *origin)
                .bounds()
                .map(|(lo, hi)| crate::declutter::TextBox { lo, hi }),
            // a section head's letter and sheet: plain text in a circle,
            // so its box is grown to the circle round it
            PlanItem::Text {
                at, size, mm: tm, ..
            } => {
                let r = *size * 1.4 * tm;
                Some(crate::declutter::TextBox {
                    lo: *at - DVec2::splat(r),
                    hi: *at + DVec2::splat(r),
                })
            }
            _ => crate::declutter::text_box(it, mm).map(|b| b.0),
        })
        .collect();
    for it in items.iter_mut() {
        let PlanItem::Annotation { id, .. } = it else {
            continue;
        };
        // put by hand, or led out of its ceiling: where it was put
        let by_hand = match model.get(*id).map(|e| &e.kind) {
            Some(ElementKind::Tag(t)) => t.offset.length() > 1e-9,
            _ => true,
        };
        if by_hand || !is_ceiling_tag(model, *id) {
            continue;
        }
        let Some((b, _)) = crate::declutter::text_box(it, mm) else {
            continue;
        };
        let PlanItem::Annotation { item, .. } = it else {
            continue;
        };
        let size = item.look.size;
        let crate::annotation::Shape::Note {
            at, leader: None, ..
        } = &mut item.shape
        else {
            continue;
        };
        let hit = |d: DVec2| marks.iter().any(|m| m.overlaps(&b.moved(d)));
        if !hit(DVec2::ZERO) {
            continue;
        }
        let step = size * 1.6 * mm;
        let found = (1..=6).find_map(|k| {
            let d = step * k as f64;
            [
                DVec2::new(0.0, -d),
                DVec2::new(0.0, d),
                DVec2::new(d * 1.5, 0.0),
                DVec2::new(-d * 1.5, 0.0),
            ]
            .into_iter()
            .find(|o| !hit(*o))
        });
        if let Some(d) = found {
            *at += d;
        }
    }
}

/// A square grid of `size` over `outline`, clipped to it.
fn grid_lines(outline: &[DVec2], size: f64) -> Vec<(DVec2, DVec2)> {
    let mut out = vec![];
    let (lo, hi) = outline.iter().fold(
        (DVec2::splat(f64::MAX), DVec2::splat(f64::MIN)),
        |(lo, hi), p| (lo.min(*p), hi.max(*p)),
    );
    if (hi - lo).max_element() / size > 400.0 {
        return out;
    }
    let mut x = lo.x + size;
    while x < hi.x - 1e-6 {
        out.extend(clip_line(
            outline,
            DVec2::new(x, lo.y - 1.0),
            DVec2::new(x, hi.y + 1.0),
        ));
        x += size;
    }
    let mut y = lo.y + size;
    while y < hi.y - 1e-6 {
        out.extend(clip_line(
            outline,
            DVec2::new(lo.x - 1.0, y),
            DVec2::new(hi.x + 1.0, y),
        ));
        y += size;
    }
    out
}

/// The pieces of segment `a` → `b` inside `poly`.
fn clip_line(poly: &[DVec2], a: DVec2, b: DVec2) -> Vec<(DVec2, DVec2)> {
    let d = b - a;
    let n = poly.len();
    let mut ts: Vec<f64> = vec![];
    for i in 0..n {
        let (p, q) = (poly[i], poly[(i + 1) % n]);
        let e = q - p;
        let den = d.perp_dot(e);
        if den.abs() < 1e-12 {
            continue;
        }
        let t = (p - a).perp_dot(e) / den;
        let s = (p - a).perp_dot(d) / den;
        if (0.0..=1.0).contains(&t) && (0.0..1.0).contains(&s) {
            ts.push(t);
        }
    }
    ts.sort_by(f64::total_cmp);
    ts.chunks(2)
        .filter(|c| c.len() == 2 && c[1] - c[0] > 1e-9)
        .map(|c| (a + d * c[0], a + d * c[1]))
        .collect()
}

/// A plan turned into a ceiling plan: what stands on the floor (furniture
/// and fixtures that don't hang from the ceiling, floors, stairs and
/// railings) and the swings of doors are left out; walls, windows, what
/// hangs from the ceiling and the ceilings themselves stay.
pub fn as_ceiling_plan(model: &Model, items: &mut Vec<PlanItem>) {
    // roof windows looked up at: frame and glass as seen, and a splayed
    // lining's hole at the ceiling with its splay (MU142, S1)
    let mut roof_windows: Vec<ElementId> = vec![];
    for it in items.iter() {
        let id = it.id();
        if matches!(model.get(id).map(|e| &e.kind), Some(ElementKind::Opening(o)) if o.roof_at.is_some())
            && !roof_windows.contains(&id)
        {
            roof_windows.push(id);
        }
    }
    if !roof_windows.is_empty() {
        items.retain(|it| {
            !(roof_windows.contains(&it.id())
                && matches!(it, PlanItem::Polygon { .. } | PlanItem::Line { .. }))
        });
        for id in roof_windows {
            if let Some(ElementKind::Opening(o)) = model.get(id).map(|e| &e.kind) {
                items.extend(crate::opening::roof_window_ceiling(model, id, o));
            }
        }
    }
    // beams over the cut, and what hangs from the ceiling, are what a
    // ceiling plan looks up at: drawn as seen, not dashed as overhead
    // (issue #70; F-07)
    for it in items.iter_mut() {
        let seen = match model.get(it.id()) {
            Some(e) if matches!(e.kind, ElementKind::Beam(_)) => true,
            Some(e) if matches!(e.kind, ElementKind::Object(_)) => {
                objects::type_of(model, e).is_some_and(|t| t.mount == Mount::Ceiling)
            }
            _ => false,
        };
        if !seen {
            continue;
        }
        match it {
            PlanItem::Line { pen, .. }
            | PlanItem::Polygon { pen, .. }
            | PlanItem::Arc { pen, .. }
                if *pen == Pen::Hidden =>
            {
                *pen = Pen::Projection
            }
            _ => {}
        }
    }
    items.retain(|it| {
        let id = it.id();
        match model.get(id).map(|e| &e.kind) {
            Some(ElementKind::Object(_)) => {
                let e = model.get(id).unwrap();
                objects::type_of(model, e).is_some_and(|t| t.mount == Mount::Ceiling)
            }
            Some(
                ElementKind::Slab(_)
                | ElementKind::Stair(_)
                | ElementKind::Railing(_)
                | ElementKind::Footing(_),
            ) => false,
            Some(ElementKind::Opening(o)) if o.kind == OpeningKind::Door => {
                let Some(ElementKind::Wall(w)) = model.get(o.host).map(|h| &h.kind) else {
                    return true;
                };
                // keep the frame in the wall, not the leaf swung out of it
                let inside = |p: &DVec2| {
                    (*p - w.start).dot(w.direction().perp()).abs() <= w.thickness / 2.0 + 1e-6
                };
                match it {
                    PlanItem::Arc { .. } => false,
                    PlanItem::Polygon { points, .. } => points.iter().all(inside),
                    PlanItem::Line { a, b, .. } => inside(a) && inside(b),
                    _ => true,
                }
            }
            _ => true,
        }
    });
}

/// The rooms a section cuts through, washed in their colours from their
/// floor up to their top: one band for each stretch of the section line
/// inside a room. Drawing coordinates are (along the line, height).
pub fn section_items(model: &Model, start: DVec2, end: DVec2) -> Vec<PlanItem> {
    let mut out = vec![];
    let len = start.distance(end);
    if len < 1e-9 {
        return out;
    }
    for e in model.iter() {
        let ElementKind::Room(r) = &e.kind else {
            continue;
        };
        let Some(s) = room::shape_of(model, r) else {
            continue;
        };
        let floor = model.level(r.level).map_or(0.0, |l| l.elevation);
        let top = floor + room::height(model, r);
        for (a, b) in clip_line(&s.outline, start, end) {
            let (x0, x1) = (a.distance(start), b.distance(start));
            out.push(PlanItem::Tint {
                id: e.id,
                points: vec![
                    DVec2::new(x0, floor),
                    DVec2::new(x1, floor),
                    DVec2::new(x1, top),
                    DVec2::new(x0, top),
                ],
                rgb: colour_of(&e.name, r),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A roof window over a floor is overhead in its plan: dashed, as the
    /// roof round it is (owner, MU141: the stair's roof window drew solid
    /// in the first floor plan).
    #[test]
    fn roof_windows_are_overhead_in_the_floor_plan_under_them() {
        let m = creabim_core::ridgeline::ridgeline_house();
        let fp = m.iter().find(|e| e.name == "First Floor plan").unwrap().id;
        let d = crate::section::view_drawing(&m, fp).unwrap();
        let windows: Vec<(String, Pen)> = d
            .items
            .iter()
            .filter_map(|i| {
                let e = m.get(i.id())?;
                let ElementKind::Opening(o) = &e.kind else {
                    return None;
                };
                o.roof_at?;
                let pen = match i {
                    PlanItem::Line { pen, .. }
                    | PlanItem::Polygon { pen, .. }
                    | PlanItem::Arc { pen, .. } => *pen,
                    _ => return None,
                };
                Some((e.name.clone(), pen))
            })
            .collect();
        assert!(!windows.is_empty(), "no roof window drawn");
        assert!(
            windows.iter().all(|(_, p)| *p == Pen::Hidden),
            "{windows:?}"
        );
    }

    /// A floor plan dashes what hangs from the ceiling, lights included;
    /// the switches and sockets on the walls stay drawn in full; a ceiling
    /// plan draws the lights in full (Builder 2's F-07 on Ridgeline).
    #[test]
    fn ceiling_lights_are_overhead_in_a_floor_plan_and_seen_in_a_ceiling_plan() {
        let m = creabim_core::ridgeline::ridgeline_house();
        let view = |name: &str| {
            m.iter()
                .find(|e| e.name == name)
                .unwrap_or_else(|| panic!("{name}"))
                .id
        };
        let pens = |view_name: &str, hung: bool| -> Vec<Pen> {
            let d = crate::section::view_drawing(&m, view(view_name)).unwrap();
            d.items
                .iter()
                .filter(|i| {
                    let Some(e) = m.get(i.id()) else { return false };
                    let Some(t) = objects::type_of(&m, e) else {
                        return false;
                    };
                    t.category.is_services() && (t.mount == Mount::Ceiling) == hung
                })
                .filter_map(|i| match i {
                    PlanItem::Line { pen, .. }
                    | PlanItem::Polygon { pen, .. }
                    | PlanItem::Arc { pen, .. } => Some(*pen),
                    _ => None,
                })
                .collect()
        };
        // on both floors: the first floor's landing is vaulted, its lights
        // hang from no ceiling a plan's cut could be under
        for plan in ["Ground Floor plan", "First Floor plan"] {
            let lights = pens(plan, true);
            assert!(
                !lights.is_empty() && lights.iter().all(|p| *p == Pen::Hidden),
                "{plan}: {lights:?}"
            );
        }
        let on_walls = pens("Ground Floor plan", false);
        assert!(
            !on_walls.is_empty() && on_walls.iter().all(|p| *p != Pen::Hidden),
            "{on_walls:?}"
        );
        let seen = pens("Ground Floor ceiling plan", true);
        assert!(
            !seen.is_empty() && seen.iter().all(|p| *p != Pen::Hidden),
            "{seen:?}"
        );
    }

    #[test]
    fn grids_and_lines_clip_to_the_outline() {
        let sq = creabim_core::polygon::rectangle(DVec2::ZERO, DVec2::new(3.0, 2.0));
        let lines = grid_lines(&sq, 0.6);
        // four lines across x (0.6..2.4) and three across y (0.6..1.8)
        assert_eq!(lines.len(), 4 + 3);
        for (a, b) in &lines {
            assert!(a.x >= -1e-9 && b.x <= 3.0 + 1e-9 && a.y >= -1e-9 && b.y <= 2.0 + 1e-9);
        }
        let l = clip_line(&sq, DVec2::new(-1.0, 1.0), DVec2::new(5.0, 1.0));
        assert_eq!(l.len(), 1);
        assert!((l[0].0.x).abs() < 1e-9 && (l[0].1.x - 3.0).abs() < 1e-9);
    }
}
