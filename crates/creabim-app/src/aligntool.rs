//! The Align tool, as in Revit, with ArchiCAD's extend-to-a-reference:
//! click a reference line (a wall face, a slab edge, a guide, a grid line,
//! any edge of the drawing, or a temporary guide line), then the edge of
//! what to align. The element turns to lie parallel to the reference and
//! moves until its edge is flush with it. In Extend mode the end of a
//! wall, beam or grid line nearest the click stretches (or trims) to meet
//! the reference instead. In Stretch mode the face the picked edge stands
//! for moves to the reference and the rest of the element stays: a wall's
//! end or side, a slab's or roof's edge, a beam's end, a column's top or
//! bottom, a door's jamb (see `creabim_core::pushpull`). It works in plans,
//! sections and elevations here, and in 3D through `crate::align3d`. The
//! reference stays picked, so several things can be aligned to it; Esc
//! lets it go. Every alignment is one undo step.

use crate::modal::Space;
use crate::paneface;
use crate::planview::{PaneKind, PlanState, Xf};
use crate::selection::Selection;
use crate::snaps;
use creabim_core::transform::transformed;
use creabim_core::{pushpull, Document, ElementId, ElementKind, ModelError, Transaction, Xform};
use creabim_geom::PlanItem;
use eframe::egui::{self, Stroke};
use glam::{DVec2, DVec3};

/// What the second click does to what it picks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AlignMode {
    /// Turn it parallel to the reference and move it flush.
    #[default]
    Align,
    /// Stretch or trim its nearer end to the reference.
    Extend,
    /// Stretch the face its edge stands for to the reference; the rest of
    /// it stays.
    Stretch,
    /// Trim or extend two to a corner (wave 3): click the part of each to
    /// keep; both run to where they meet. No reference.
    Corner,
}

// Corner's key (J, not T, which is Tag) is the keymap's (keymap.rs).

impl AlignMode {
    pub const ALL: [AlignMode; 4] = [
        AlignMode::Align,
        AlignMode::Stretch,
        AlignMode::Extend,
        AlignMode::Corner,
    ];

    pub fn label(self) -> &'static str {
        match self {
            AlignMode::Align => "Align",
            AlignMode::Extend => "Extend to it",
            AlignMode::Stretch => "Stretch to it",
            AlignMode::Corner => "Corner (J)",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            AlignMode::Align => "Turn the edge you pick parallel to the reference and move it flush",
            AlignMode::Extend => {
                "Stretch or trim the end of a wall, beam or grid line you pick to meet the reference"
            }
            AlignMode::Stretch => {
                "Stretch the edge or face you pick to the reference, the rest staying put: a wall end or side, a slab or roof edge, a beam end, a column top"
            }
            AlignMode::Corner => {
                "Trim or extend two to a corner: click the part of each to keep (walls, beams, lines, grid lines, room separators). J from any tool"
            }
        }
    }
}

/// How far from parallel, in degrees, an edge or face may be from the
/// reference and still be stretched or moved flush to it: it moves
/// square to itself until the point picked on it meets the reference.
pub const PARALLEL_TOL_DEG: f64 = 5.0;

/// The tool's picks.
#[derive(Clone, Debug, Default)]
pub struct AlignState {
    /// The reference line: the view it was picked in, two points on it
    /// (that view's drawing), and the element it belongs to.
    pub reference: Option<(ElementId, DVec2, DVec2, Option<ElementId>)>,
    /// The reference picked in 3D, in the 3D window or a 3D view: it
    /// holds in every 3D pane.
    pub reference_3d: Option<crate::align3d::Reference>,
    pub mode: AlignMode,
    /// The 3D view's workplane: on a sloped or upright one, what is drawn
    /// on it turns on it to line up.
    pub workplane: Option<creabim_core::Workplane>,
    /// Corner mode's first pick: the view, the element and the point
    /// clicked on the part to keep.
    pub corner: Option<(ElementId, ElementId, DVec2)>,
    /// Shift held on this click: Corner goes on round the next corner.
    pub shift: bool,
}

impl AlignState {
    pub fn clear(&mut self) {
        self.reference = None;
        self.reference_3d = None;
        self.corner = None;
    }

    pub fn has_input(&self) -> bool {
        self.reference.is_some() || self.reference_3d.is_some() || self.corner.is_some()
    }
}

/// The tool's hint for the status bar.
pub fn hint(state: &AlignState) -> &'static str {
    if state.mode == AlignMode::Corner {
        return match state.corner {
            None => "Corner: click the part of the first wall, beam, line or grid line to keep.",
            Some(_) => "Now click the part of the second to keep: both run to where they meet. Shift+click goes on round the next corner. Esc starts again.",
        };
    }
    match (state.reference.is_some(), state.mode) {
        (false, _) => "Align: click the reference line: a wall face, slab edge, centre line (dashed purple), guide, grid line or a temporary guide.",
        (true, AlignMode::Align) => "Now click the edge to align to it: the element turns parallel and moves flush. Esc lets go of the reference.",
        (true, AlignMode::Extend) => "Now click near the end of a wall, beam or grid line to stretch it to the reference. Esc lets go of the reference.",
        (true, AlignMode::Stretch) => "Now click the edge to stretch to it: a wall end or face, slab or roof edge, beam end, column top or bottom. The rest stays put. Esc lets go of the reference.",
        (true, AlignMode::Corner) => unreachable!("Corner returns above"),
    }
}

/// The straight edge of the drawing under `p`: its element and ends.
pub fn edge_at(items: &[PlanItem], p: DVec2, tol: f64) -> Option<(ElementId, DVec2, DVec2)> {
    crate::annotate::segment_at(items, p, tol)
}

/// The edge the next click takes: with a reference picked in Stretch
/// mode, one that stretches wins where edges lie on top of each other.
pub fn hover_edge(
    model: &creabim_core::Model,
    state: &AlignState,
    items: &[PlanItem],
    centres: &[CentreLine],
    p: DVec2,
    tol: f64,
) -> Option<(ElementId, DVec2, DVec2, bool)> {
    if state.reference.is_some() && state.mode == AlignMode::Stretch {
        let rank = |i: ElementId| {
            let push = model.get(i).is_some_and(|e| pushpull::can_push(&e.kind));
            Some(u8::from(push))
        };
        return paneface::edge_at(items, p, tol, &rank).map(|(i, a, b)| (i, a, b, false));
    }
    pick_line(items, centres, p, tol)
}

/// A centre line as a pane draws it: its element and ends.
pub type CentreLine = (ElementId, DVec2, DVec2);

/// The line a click at `p` takes: the edge of the drawing under it, or a
/// centre line nearer it (then true).
pub fn pick_line(
    items: &[PlanItem],
    centres: &[CentreLine],
    p: DVec2,
    tol: f64,
) -> Option<(ElementId, DVec2, DVec2, bool)> {
    let edge = edge_at(items, p, tol);
    let centre = crate::centres::line_at(centres, p, tol);
    match (edge, centre) {
        (Some((_, a, b)), Some(c)) if c.3 < crate::centres::segment_distance(p, a, b) => {
            Some((c.0, c.1, c.2, true))
        }
        (Some((id, a, b)), _) => Some((id, a, b, false)),
        (None, Some(c)) => Some((c.0, c.1, c.2, true)),
        (None, None) => None,
    }
}

/// The turn, in degrees, that makes direction `t` parallel to `r` by the
/// shorter way round (lines have no direction).
pub fn turn_to(t: DVec2, r: DVec2) -> f64 {
    let mut a = (r.to_angle() - t.to_angle()).to_degrees();
    a = a.rem_euclid(180.0);
    if a > 90.0 {
        a -= 180.0;
    }
    a
}

/// The moves that align the edge `t0`–`t1` with the line through `r0`
/// and `r1` in a plan: a turn about the point of the edge nearest the
/// click `at` (none when already parallel), then a move square to the
/// reference.
pub fn align_moves(t0: DVec2, t1: DVec2, r0: DVec2, r1: DVec2, at: DVec2) -> Vec<Xform> {
    let t = (t1 - t0).normalize_or_zero();
    let r = (r1 - r0).normalize_or_zero();
    let pivot = t0 + t * (at - t0).dot(t).clamp(0.0, t0.distance(t1));
    let mut out = vec![];
    let deg = turn_to(t, r);
    if deg.abs() > 1e-9 {
        out.push(Xform::Rotate {
            center: pivot,
            degrees: deg,
        });
    }
    let n = r.perp();
    let d = (r0 - pivot).dot(n);
    if d.abs() > 1e-9 {
        out.push(Xform::Move((n * d).extend(0.0)));
    }
    out
}

/// Where the line through `a` and `b` meets the line through `r0` and `r1`.
fn meet(a: DVec2, b: DVec2, r0: DVec2, r1: DVec2) -> Option<DVec2> {
    creabim_geom::annotation::line_intersection(a, b, r0, r1)
}

/// A click with the Align tool. `snap` is the snapped point under the
/// pointer, `raw` the pointer itself, in the pane's drawing.
#[allow(clippy::too_many_arguments)]
pub fn click(
    doc: &mut Document,
    view: ElementId,
    kind: PaneKind,
    items: &[PlanItem],
    centres: &[CentreLine],
    raw: DVec2,
    snap: Option<snaps::Snap>,
    tol: f64,
    mm: f64,
    state: &mut PlanState,
    selection: &mut Selection,
) -> Result<(), ModelError> {
    let _ = (snap, mm);
    if state.align.mode == AlignMode::Corner {
        return corner_click(doc, view, items, centres, raw, tol, state, selection);
    }
    let line_picked = pick_line(items, centres, raw, tol);
    let on_centre = line_picked.is_some_and(|l| l.3);
    let picked = line_picked.map(|(id, a, b, _)| (id, a, b));
    let reference = state.align.reference.filter(|r| r.0 == view);
    let Some((_, r0, r1, ref_id)) = reference else {
        // the reference: an edge of the drawing or a centre line, else the
        // guide line under the pointer
        let line = picked.map(|(id, a, b)| (a, b, Some(id))).or_else(|| {
            match state.hover_guide.filter(|h| h.0 == view)?.1.shape {
                snaps::GuideShape::Line { p, d } => Some((p, p + d, None)),
                snaps::GuideShape::Circle { .. } => None,
            }
        });
        match line {
            Some((a, b, id)) => {
                state.align.reference = Some((view, a, b, id));
                let what = match (on_centre, id.and_then(|i| doc.model().get(i))) {
                    (true, Some(e)) => format!("Reference: the centre line of {}. ", e.name),
                    _ => String::new(),
                };
                state.status = Some(format!("{what}{}", hint(&state.align)));
            }
            None => {
                state.status =
                    Some("Click on a straight edge, centre line, guide or grid line.".into())
            }
        }
        return Ok(());
    };
    let Some((id, t0, t1)) = picked else {
        state.status = Some("Click on the edge of what to align.".into());
        return Ok(());
    };
    let same_line =
        (t1 - t0).perp_dot(r1 - r0).abs() < 1e-9 && (t0 - r0).perp_dot(r1 - r0).abs() < 1e-9;
    if Some(id) == ref_id && same_line {
        state.status =
            Some("That is the reference itself: click the edge of something else.".into());
        return Ok(());
    }
    // a drawn outline has no faces, but its edges stretch and its ends
    // extend, in the plan it is drawn in (MU47)
    if let (Some(e), AlignMode::Stretch | AlignMode::Extend) =
        (doc.model().get(id).cloned(), state.align.mode)
    {
        if let ElementKind::Sketch(sk) = &e.kind {
            let extend = state.align.mode == AlignMode::Extend;
            let flat = matches!(kind, PaneKind::Plan { .. }) && sk.plane.normal().z.abs() > 0.999;
            let done = if flat {
                sketch_edge_to(sk, (t0, t1), (r0, r1), raw, extend)
            } else {
                Err("An outline stretches and extends in the plan it is drawn in; Align moves it anywhere.".into())
            };
            state.status = Some(match done {
                Ok(now) => {
                    let label = if extend {
                        "Extend to reference"
                    } else {
                        "Stretch to reference"
                    };
                    let mut after = e.clone();
                    after.kind = ElementKind::Sketch(now);
                    doc.commit(Transaction::new(label).replace(e, after))?;
                    selection.only(id);
                    format!("{label}. Click more edges to bring them to the same reference; Esc lets it go.")
                }
                Err(why) => why,
            });
            return Ok(());
        }
    }
    let stretch = match (state.align.mode, kind) {
        (AlignMode::Stretch, _) => true,
        // a section has no ends at an angle to meet: Extend stretches there
        (AlignMode::Extend, PaneKind::Section { .. }) => true,
        _ => false,
    };
    let grid = matches!(
        doc.model().get(id).map(|e| &e.kind),
        Some(ElementKind::Grid(_))
    );
    // a grid line has no faces: in a plan its end extends instead
    if stretch && !(grid && matches!(kind, PaneKind::Plan { .. })) {
        // of edges on top of each other (a beam's end on a column's face),
        // the first that can stretch there goes
        let model = doc.model();
        let rank = |i: ElementId| {
            let push = model.get(i).is_some_and(|e| pushpull::can_push(&e.kind));
            Some(u8::from(push))
        };
        let mut tried = paneface::edges_at(items, raw, tol, &rank);
        if tried.is_empty() {
            if on_centre {
                state.status = Some(
                    "Stretch moves a face: click an edge, or use Align to bring a centre line onto the reference."
                        .into(),
                );
                return Ok(());
            }
            tried.push((id, t0, t1));
        }
        let mut first_err = None;
        let mut done = None;
        for (id, t0, t1) in tried {
            match stretch_to(model, kind, items, id, (t0, t1), (r0, r1), raw) {
                Ok(r) => {
                    done = Some((id, r));
                    break;
                }
                Err(e) => {
                    first_err.get_or_insert(e);
                }
            }
        }
        state.status = match done {
            Some((id, (tx, msg))) => Some(match doc.commit(tx) {
                Ok(()) => {
                    selection.only(id);
                    msg
                }
                Err(e) => format!("Couldn't do that: {e}"),
            }),
            None => first_err,
        };
        return Ok(());
    }
    let model = doc.model();
    let Some(e) = model.get(id).cloned() else {
        return Ok(());
    };
    // what an edge of a hosted door or window belongs to is the opening;
    // aligning moves its wall (by its centre line it slides along it)
    let e = match &e.kind {
        ElementKind::Opening(o) if !on_centre => model.get(o.host).cloned().unwrap_or(e),
        _ => e,
    };
    let (label, after) = match (state.align.mode, kind) {
        (AlignMode::Align, PaneKind::Plan { .. }) => {
            let mut now = e.clone();
            for t in align_moves(t0, t1, r0, r1, raw) {
                match transformed(model, &now, &t) {
                    Some(n) => now = n,
                    None => {
                        state.status = Some(format!("{} can't be aligned.", e.name));
                        return Ok(());
                    }
                }
            }
            ("Align", now)
        }
        (AlignMode::Align, PaneKind::Section { start, end }) => {
            // a section only moves things, across or up its plane
            let (t, r) = ((t1 - t0).normalize_or_zero(), (r1 - r0).normalize_or_zero());
            if turn_to(t, r).abs() > PARALLEL_TOL_DEG {
                state.status = Some(
                    "In a section Align only moves: pick an edge parallel to the reference (turn it in a plan)."
                        .into(),
                );
                return Ok(());
            }
            // the point of the edge nearest the click comes onto the line
            let q = t0 + t * (raw - t0).dot(t).clamp(0.0, t0.distance(t1));
            let n = r.perp();
            let d = n * (r0 - q).dot(n);
            let along = (end - start).normalize_or_zero().extend(0.0);
            let shift = along * d.x + DVec3::Z * d.y;
            match transformed(model, &e, &Xform::Move(shift)) {
                Some(n) => ("Align", n),
                None => {
                    state.status = Some(format!("{} can't be aligned.", e.name));
                    return Ok(());
                }
            }
        }
        (AlignMode::Extend | AlignMode::Stretch, PaneKind::Plan { .. }) => {
            let mut now = e.clone();
            let ends = match &mut now.kind {
                ElementKind::Wall(w) => Some((&mut w.start, &mut w.end)),
                ElementKind::Beam(b) => Some((&mut b.start, &mut b.end)),
                ElementKind::Grid(g) => Some((&mut g.start, &mut g.end)),
                _ => None,
            };
            let Some((a, b)) = ends else {
                state.status = Some(
                    "Extend works on walls, beams and grid lines; use Stretch for the edges of other things, Align to move them."
                        .into(),
                );
                return Ok(());
            };
            let Some(hit) = meet(*a, *b, r0, r1) else {
                state.status =
                    Some("It runs parallel to the reference, so it never meets it.".into());
                return Ok(());
            };
            // the end nearer the click goes
            let end = if raw.distance(*a) <= raw.distance(*b) {
                a
            } else {
                b
            };
            *end = hit;
            ("Extend to reference", now)
        }
        (AlignMode::Extend | AlignMode::Stretch, _) => {
            state.status = Some("Extend works in plans, sections and elevations.".into());
            return Ok(());
        }
        _ => return Ok(()),
    };
    if let (ElementKind::Wall(w), true) = (&after.kind, label == "Extend to reference") {
        if w.length() < 0.01 {
            state.status = Some("That would leave the wall with no length.".into());
            return Ok(());
        }
    }
    let target = after.id;
    let before = model.get(target).cloned().expect("aligned element exists");
    if before == after {
        state.status = Some("Already aligned.".into());
        return Ok(());
    }
    doc.commit(Transaction::new(label).replace(before, after))?;
    selection.only(target);
    state.status = Some(format!(
        "{label}. Click more edges to align them to the same reference; Esc lets it go."
    ));
    Ok(())
}

/// The straight line an element runs along in a plan, which Corner trims
/// or extends: a wall's or beam's line, a grid line, a room separator, or
/// a drawn line of one straight segment lying flat.
pub fn corner_line(e: &creabim_core::Element) -> Option<(DVec2, DVec2)> {
    match &e.kind {
        ElementKind::Wall(w) => Some((w.start, w.end)),
        ElementKind::Beam(b) => Some((b.start, b.end)),
        ElementKind::Grid(g) => Some((g.start, g.end)),
        ElementKind::RoomSeparator(s) => Some((s.a, s.b)),
        ElementKind::Sketch(sk)
            if !sk.closed
                && sk.circle.is_none()
                && sk.points.len() == 2
                && sk
                    .kinds
                    .iter()
                    .all(|k| *k == creabim_core::SegmentKind::Line)
                && sk.plane.normal().z.abs() > 0.999 =>
        {
            Some((
                sk.plane.to_model(sk.points[0]).truncate(),
                sk.plane.to_model(sk.points[1]).truncate(),
            ))
        }
        _ => None,
    }
}

/// `e` running from `a` to `b` instead (see [`corner_line`]).
fn with_corner_line(e: &creabim_core::Element, a: DVec2, b: DVec2) -> creabim_core::Element {
    let mut now = e.clone();
    match &mut now.kind {
        ElementKind::Wall(w) => (w.start, w.end) = (a, b),
        ElementKind::Beam(m) => (m.start, m.end) = (a, b),
        ElementKind::Grid(g) => (g.start, g.end) = (a, b),
        ElementKind::RoomSeparator(s) => (s.a, s.b) = (a, b),
        ElementKind::Sketch(sk) => {
            let z = sk.plane.origin.z;
            sk.points[0] = sk.plane.to_plane(a.extend(z));
            sk.points[1] = sk.plane.to_plane(b.extend(z));
        }
        _ => {}
    }
    now
}

/// Two lines run to a corner: each keeps the part on the side of their
/// meeting point where `keep` was clicked, its other end moving to the
/// meeting point (trimmed when past it, extended when short of it).
/// Returns the two lines and the corner, or None when they run parallel.
#[allow(clippy::type_complexity)]
pub fn corner_ends(
    (a0, a1): (DVec2, DVec2),
    keep_a: DVec2,
    (b0, b1): (DVec2, DVec2),
    keep_b: DVec2,
) -> Option<((DVec2, DVec2), (DVec2, DVec2), DVec2)> {
    let (da, db) = (a1 - a0, b1 - b0);
    if da.perp_dot(db).abs() < 1e-9 * da.length() * db.length() {
        return None;
    }
    let hit = meet(a0, a1, b0, b1)?;
    let run = |(p0, p1): (DVec2, DVec2), keep: DVec2| {
        let d = p1 - p0;
        let t = |p: DVec2| (p - p0).dot(d) / d.length_squared();
        if t(keep) <= t(hit) {
            (p0, hit)
        } else {
            (hit, p1)
        }
    };
    Some((run((a0, a1), keep_a), run((b0, b1), keep_b), hit))
}

/// The edit that runs `a` and `b` to their corner, each keeping the part
/// on the side of its `keep` point; else why it can't.
pub fn corner_tx(
    model: &creabim_core::Model,
    (a, keep_a): (ElementId, DVec2),
    (b, keep_b): (ElementId, DVec2),
) -> Result<(Transaction, DVec2), String> {
    let get = |id: ElementId| {
        let e = model.get(id).ok_or_else(|| "It is gone.".to_string())?;
        corner_line(e).map(|l| (e, l)).ok_or_else(|| {
            format!(
                "{} has no straight line to run to a corner: Corner works on walls, beams, lines, grid lines and room separators.",
                e.name
            )
        })
    };
    let (ea, la) = get(a)?;
    let (eb, lb) = get(b)?;
    let (na, nb, hit) = corner_ends(la, keep_a, lb, keep_b)
        .ok_or_else(|| "They run parallel, so they never meet.".to_string())?;
    for (e, (p, q)) in [(ea, na), (eb, nb)] {
        if p.distance(q) < 0.01 {
            return Err(format!("That would leave {} with no length.", e.name));
        }
    }
    let (aa, bb) = (
        with_corner_line(ea, na.0, na.1),
        with_corner_line(eb, nb.0, nb.1),
    );
    if &aa == ea && &bb == eb {
        return Err("They already meet there.".into());
    }
    let tx = Transaction::new("Trim to corner")
        .replace(ea.clone(), aa)
        .replace(eb.clone(), bb);
    Ok((tx, hit))
}

/// For two lines with no clicks to say which parts stay: each keeps its
/// longer side of their meeting point. The point to keep on each, or None
/// when they run parallel.
pub fn longer_parts(a: (DVec2, DVec2), b: (DVec2, DVec2)) -> Option<(DVec2, DVec2)> {
    let hit = meet(a.0, a.1, b.0, b.1)?;
    let far = |(p, q): (DVec2, DVec2)| {
        if p.distance(hit) >= q.distance(hit) {
            p
        } else {
            q
        }
    };
    Some((far(a), far(b)))
}

/// What Corner would make of the first pick and `second` (an element and
/// the point on the part to keep): both lines as they would run, for a
/// ghost before the click (MU69); None when it can't.
pub fn corner_preview(
    model: &creabim_core::Model,
    first: (ElementId, DVec2),
    second: (ElementId, DVec2),
) -> Option<[(DVec2, DVec2); 2]> {
    if first.0 == second.0 {
        return None;
    }
    let la = corner_line(model.get(first.0)?)?;
    let lb = corner_line(model.get(second.0)?)?;
    let (a, b, _) = corner_ends(la, first.1, lb, second.1)?;
    Some([a, b])
}

/// Paint a Corner ghost: the two lines as they would run, dashed.
pub fn paint_corner_ghost(
    painter: &egui::Painter,
    to_screen: &dyn Fn(DVec2) -> Option<egui::Pos2>,
    lines: [(DVec2, DVec2); 2],
) {
    let stroke = Stroke::new(2.0, crate::theme::interaction());
    for (a, b) in lines {
        if let (Some(a), Some(b)) = (to_screen(a), to_screen(b)) {
            painter.extend(egui::Shape::dashed_line(&[a, b], stroke, 8.0, 4.0));
            painter.circle_filled(a, 3.0, stroke.color);
            painter.circle_filled(b, 3.0, stroke.color);
        }
    }
}

/// The element a Corner click takes: the line under the pointer, an
/// opening's edge standing for its wall.
pub fn corner_pick(
    model: &creabim_core::Model,
    items: &[PlanItem],
    centres: &[CentreLine],
    raw: DVec2,
    tol: f64,
) -> Option<ElementId> {
    let id = pick_line(items, centres, raw, tol)?.0;
    match &model.get(id)?.kind {
        ElementKind::Opening(o) => Some(o.host),
        _ => Some(id),
    }
}

/// A click in Corner mode: the first picks the part of one to keep, the
/// second the part of the other, and both run to their corner.
#[allow(clippy::too_many_arguments)]
fn corner_click(
    doc: &mut Document,
    view: ElementId,
    items: &[PlanItem],
    centres: &[CentreLine],
    raw: DVec2,
    tol: f64,
    state: &mut PlanState,
    selection: &mut Selection,
) -> Result<(), ModelError> {
    let model = doc.model();
    let Some(id) = corner_pick(model, items, centres, raw, tol) else {
        state.status = Some("Click on a wall, beam, line or grid line.".into());
        return Ok(());
    };
    if let Some(e) = model.get(id).filter(|e| corner_line(e).is_none()) {
        state.status = Some(format!(
            "{} has no straight line to run to a corner: Corner works on walls, beams, lines, grid lines and room separators.",
            e.name
        ));
        return Ok(());
    }
    let first = state.align.corner.filter(|c| c.0 == view && c.1 != id);
    let Some((_, a, keep_a)) = first else {
        state.align.corner = Some((view, id, raw));
        selection.only(id);
        state.status = Some(hint(&state.align).into());
        return Ok(());
    };
    match corner_tx(model, (a, keep_a), (id, raw)) {
        Ok((tx, _)) => {
            doc.commit(tx)?;
            selection.extend(vec![a, id], false);
            // round the next corner: the second keeps the part that runs to
            // the corner just made (its middle, now)
            let middle = doc
                .model()
                .get(id)
                .and_then(corner_line)
                .map(|(p, q)| (p + q) / 2.0);
            if let (true, Some(middle)) = (state.align.shift, middle) {
                state.align.corner = Some((view, id, middle));
                state.status = Some(
                    "Trim to corner. Now click the part of the next to keep, for the next corner; Esc stops."
                        .into(),
                );
            } else {
                state.align.corner = None;
                state.status = Some(
                    "Trim to corner. Click the next pair, or hold Shift on the second click to go on round."
                        .into(),
                );
            }
        }
        Err(why) => state.status = Some(why),
    }
    Ok(())
}

/// A flat outline with the edge `t` (in the plan's own axes, as picked)
/// brought to the reference line `r`. Stretch moves the whole edge onto
/// it, each end sliding along the edge next to it so that one keeps its
/// direction; Extend slides the end nearer the click `at` along the edge
/// itself until it meets the reference. Else why it can't.
pub fn sketch_edge_to(
    sk: &creabim_core::Sketch,
    (t0, t1): (DVec2, DVec2),
    (r0, r1): (DVec2, DVec2),
    at: DVec2,
    extend: bool,
) -> Result<creabim_core::Sketch, String> {
    use creabim_core::SegmentKind;
    let n = sk.points.len();
    if sk.circle.is_some() || n < 2 {
        return Err("A circle has no edges to stretch; Scale changes its size.".into());
    }
    let z = sk.plane.origin.z;
    let plan = |p: DVec2| sk.plane.to_model(p).truncate();
    let back = |p: DVec2| sk.plane.to_plane(p.extend(z));
    // the segment both ends of the picked edge lie on
    let on = |p: DVec2, a: DVec2, b: DVec2| {
        let d = (b - a).normalize_or_zero();
        let along = (p - a).dot(d);
        (p - a).perp_dot(d).abs() < 1e-4 && along > -1e-4 && along < a.distance(b) + 1e-4
    };
    let found = (0..sk.segment_count()).find(|i| {
        let (a, b) = sk.ends(*i);
        let (a, b) = (plan(a), plan(b));
        a.distance(b) > 1e-9 && on(t0, a, b) && on(t1, a, b)
    });
    let Some(i) = found else {
        return Err("Click on a straight edge of the outline.".into());
    };
    if !matches!(sk.kind(i), SegmentKind::Line) {
        return Err("An arc can't be stretched to a line: click a straight edge.".into());
    }
    let j = (i + 1) % n;
    let (a, b) = (plan(sk.points[i]), plan(sk.points[j]));
    let (t, r) = ((b - a).normalize_or_zero(), (r1 - r0).normalize_or_zero());
    let mut now = sk.clone();
    if extend {
        let Some(hit) = meet(a, b, r0, r1) else {
            return Err("It runs parallel to the reference, so it never meets it.".into());
        };
        // which end goes (MU49, "think of all the possible conditions"):
        // where the reference crosses the edge itself, the end nearer
        // the click comes back to it (a trim); where it meets the edge's
        // line past an end, that end goes out to it, wherever the click
        // was: the other would have to pass through the whole edge
        let len = a.distance(b);
        let along = (hit - a).dot(t);
        let end = if along <= 1e-9 {
            i
        } else if along >= len - 1e-9 {
            j
        } else if at.distance(a) <= at.distance(b) {
            i
        } else {
            j
        };
        let other = if end == i { b } else { a };
        if hit.distance(other) < 1e-6 {
            return Err("That would leave the edge with no length.".into());
        }
        now.points[end] = back(hit);
    } else {
        // the edge before corner `i`, and the one after corner `j`
        let before = (sk.closed || i > 0).then(|| (i + n - 1) % n);
        let after = (sk.closed || j + 1 < n).then_some(j);
        let beside = |corner: DVec2, seg: Option<usize>, far: usize| {
            seg.filter(|s| matches!(sk.kind(*s), SegmentKind::Line))
                .map(|_| (corner - plan(sk.points[far])).normalize_or_zero())
                .filter(|d| d.perp_dot(r).abs() > 0.2)
        };
        let (da, db) = (
            beside(a, before, (i + n - 1) % n),
            beside(b, after, (j + 1) % n),
        );
        // with an edge at each end to slide along, the edge may come to
        // the reference at any angle, taking its direction (MU48); alone,
        // it moves square to itself and must be near parallel
        let off = turn_to(t, r).abs();
        if (da.is_none() || db.is_none()) && off > PARALLEL_TOL_DEG {
            return Err(format!("That edge is {off:.0}° off the reference: Stretch takes a loose edge within {PARALLEL_TOL_DEG:.0}° of parallel. Extend brings an end at an angle to it."));
        }
        let slide = |corner: DVec2, along: Option<DVec2>| {
            let dir = along.unwrap_or(t.perp());
            along_to_line(corner, dir, r0, r1).map(|s| corner + dir * s)
        };
        let (Some(pa), Some(pb)) = (slide(a, da), slide(b, db)) else {
            return Err("That edge never meets the reference.".into());
        };
        if pa.distance(pb) < 1e-6 {
            return Err("That would leave the edge with no length.".into());
        }
        now.points[i] = back(pa);
        now.points[j] = back(pb);
    }
    if now.points == sk.points {
        return Err("Already there.".into());
    }
    now.check().map_err(|e| format!("Couldn't do that: {e}"))?;
    let crossed =
        |s: &creabim_core::Sketch| creabim_core::polygon::self_intersects(&s.outline(), s.closed);
    if crossed(&now) && !crossed(sk) {
        return Err(if extend {
            "That end would cross the outline itself: Stretch takes the whole edge there instead."
                .into()
        } else {
            "That would make the outline cross itself.".into()
        });
    }
    Ok(now)
}

/// What the next click would make of the outline under the pointer,
/// dashed, with the corners that move marked: so Stretch and Extend show
/// their result before they are asked for it (MU49). When it can't be
/// done, the reason by the pointer.
pub fn draw_outline_preview(
    painter: &egui::Painter,
    xf: &Xf,
    model: &creabim_core::Model,
    state: &AlignState,
    view: ElementId,
    hover: (ElementId, DVec2, DVec2, bool),
    at: DVec2,
) {
    let Some((v, r0, r1, _)) = state.reference else {
        return;
    };
    let extend = match state.mode {
        AlignMode::Extend => true,
        AlignMode::Stretch => false,
        AlignMode::Align | AlignMode::Corner => return,
    };
    let (id, t0, t1, centre) = hover;
    let Some(ElementKind::Sketch(sk)) = model.get(id).map(|e| &e.kind) else {
        return;
    };
    if v != view || centre || sk.plane.normal().z.abs() < 0.999 {
        return;
    }
    let accent = crate::theme::interaction();
    let plan = |p: DVec2| xf.to_screen(sk.plane.to_model(p).truncate());
    match sketch_edge_to(sk, (t0, t1), (r0, r1), at, extend) {
        Ok(now) => {
            let mut pts: Vec<egui::Pos2> = now.points.iter().map(|p| plan(*p)).collect();
            if now.closed {
                pts.push(pts[0]);
            }
            painter.add(egui::Shape::dashed_line(
                &pts,
                Stroke::new(2.0, accent),
                7.0,
                4.0,
            ));
            for (old, new) in sk.points.iter().zip(&now.points) {
                if old.distance(*new) > 1e-9 {
                    let (a, b) = (plan(*old), plan(*new));
                    painter.line_segment([a, b], Stroke::new(1.0, accent.gamma_multiply(0.7)));
                    painter.circle_filled(b, 4.0, accent);
                    painter.circle_stroke(b, 4.0, Stroke::new(1.0, egui::Color32::WHITE));
                }
            }
        }
        Err(why) => {
            let short = why.split([':', '.']).next().unwrap_or(&why).to_string();
            let at = xf.to_screen(at) + egui::vec2(14.0, 16.0);
            let galley = painter.layout_no_wrap(
                short,
                egui::FontId::proportional(11.5),
                egui::Color32::WHITE,
            );
            let back = egui::Rect::from_min_size(at, galley.size() + egui::vec2(10.0, 6.0));
            painter.rect_filled(back, 4.0, egui::Color32::from_black_alpha(190));
            painter.galley(at + egui::vec2(5.0, 3.0), galley, egui::Color32::WHITE);
        }
    }
}

/// The stretch of the face the edge `t` of `id` stands for (picked at
/// `at`) to the reference line `r`, as one undo step; walls joined to a
/// stretched wall end follow it. With it the message for the status bar;
/// else why it can't.
fn stretch_to(
    model: &creabim_core::Model,
    kind: PaneKind,
    items: &[PlanItem],
    id: ElementId,
    (t0, t1): (DVec2, DVec2),
    (r0, r1): (DVec2, DVec2),
    at: DVec2,
) -> Result<(Transaction, String), String> {
    let Some(e) = model.get(id) else {
        return Err("Click on the edge of what to stretch.".into());
    };
    let name = e.name.clone();
    if !pushpull::can_push(&e.kind) {
        return Err(format!(
            "{name} has no faces to stretch; Align moves it instead."
        ));
    }
    let (t, r) = ((t1 - t0).normalize_or_zero(), (r1 - r0).normalize_or_zero());
    let off = turn_to(t, r).abs();
    if off > PARALLEL_TOL_DEG {
        return Err(format!("That edge is {off:.0}° off the reference: Stretch takes edges within {PARALLEL_TOL_DEG:.0}° of parallel. Turn it with Align first, or use Extend for a wall or beam end at an angle."));
    }
    let (space, z) = match kind {
        PaneKind::Plan { level } => (Space::Plan, model.level(level).map_or(0.0, |l| l.elevation)),
        PaneKind::Section { start, end } => (Space::Section { start, end }, 0.0),
        _ => return Err("Stretch works in plans, sections and elevations.".into()),
    };
    let f = paneface::face_of_edge(items, space, id, t0, t1, at, z, 0.0);
    // the point of the edge nearest the click moves square to the edge
    // until it meets the reference (all of it does, when parallel)
    let q = t0 + t * (at - t0).dot(t).clamp(0.0, t0.distance(t1));
    let s = along_to_line(q, f.out, r0, r1).ok_or("That edge never meets the reference.")?;
    stretch_tx(model, id, &f.pick.face, s)
}

/// How far from `q` along the unit direction `out` the line through `r0`
/// and `r1` is; none when they run parallel.
fn along_to_line(q: DVec2, out: DVec2, r0: DVec2, r1: DVec2) -> Option<f64> {
    let n = (r1 - r0).normalize_or_zero().perp();
    let c = out.dot(n);
    (c.abs() > 1e-6).then(|| (r0 - q).dot(n) / c)
}

/// `face` of `id` pushed out `s` (pulled in when negative) by the
/// per-kind rules of `creabim_core::pushpull`, as one undo step; walls
/// joined to a stretched wall end follow it. With it the message for the
/// status bar; else why it can't.
pub fn stretch_tx(
    model: &creabim_core::Model,
    id: ElementId,
    face: &pushpull::FaceRef,
    s: f64,
) -> Result<(Transaction, String), String> {
    let Some(e) = model.get(id) else {
        return Err("Click on what to stretch.".into());
    };
    let name = e.name.clone();
    if !pushpull::can_push(&e.kind) {
        return Err(format!(
            "{name} has no faces to stretch; Align moves it instead."
        ));
    }
    if s.abs() < 1e-9 {
        return Err("Already there.".into());
    }
    let mut changed = pushpull::push_face(model, &pushpull::affected(model, id), face, s)
        .ok_or_else(|| {
            format!("{name} can't stretch that way that far: that face doesn't move, or the shape would turn inside out.")
        })?;
    let follow = creabim_core::joins::follow(model, &mut changed);
    let mut tx = Transaction::new("Stretch to reference");
    for n in changed {
        if let Some(o) = model.get(n.id).filter(|o| **o != n) {
            tx = tx.replace(o.clone(), n);
        }
    }
    for (o, n) in follow {
        tx = tx.replace(o, n);
    }
    if tx.ops.is_empty() {
        return Err(format!("{name} can't stretch that face."));
    }
    // which face moved, said plainly (MU33: a wall's bottom went 29 mm
    // under its slab and nobody could tell what had done it)
    let n = face.normal;
    let (what, way) = if n.z < -0.7 {
        ("bottom", if s > 0.0 { "down" } else { "up" })
    } else if n.z > 0.7 {
        ("top", if s > 0.0 { "up" } else { "down" })
    } else {
        ("side", if s > 0.0 { "out" } else { "in" })
    };
    Ok((tx, format!(
        "Stretched the {what} of {name} {} {way} to the reference (Ctrl+Z puts it back). Click more to stretch them to it; Esc lets it go.",
        crate::units::len(s.abs())
    )))
}

/// Draw the picked reference across the pane, and the edge the pointer is
/// over, so it is clear what the next click takes.
pub fn draw_picks(
    painter: &egui::Painter,
    xf: &Xf,
    state: &AlignState,
    view: ElementId,
    hover: Option<(ElementId, DVec2, DVec2, bool)>,
    centres: &[CentreLine],
) {
    let accent = crate::theme::interaction();
    // the centre lines of what the pointer is over, faintly
    let centre = crate::align3d::CENTRE;
    for (id, a, b) in centres {
        if hover.is_some_and(|h| h.0 == *id) {
            painter.add(egui::Shape::dashed_line(
                &[xf.to_screen(*a), xf.to_screen(*b)],
                Stroke::new(1.0, centre.gamma_multiply(0.6)),
                10.0,
                3.0,
            ));
        }
    }
    if let Some((v, a, b, _)) = state.reference {
        if v == view {
            let (sa, sb) = (xf.to_screen(a), xf.to_screen(b));
            let u = (sb - sa).normalized();
            if u.x.is_finite() {
                let reach = painter
                    .clip_rect()
                    .width()
                    .max(painter.clip_rect().height())
                    * 2.0;
                painter.add(egui::Shape::dashed_line(
                    &[sa - u * reach, sa + u * reach],
                    Stroke::new(1.5, snaps::GUIDE),
                    9.0,
                    4.0,
                ));
                painter.line_segment([sa, sb], Stroke::new(4.0, snaps::GUIDE.gamma_multiply(0.5)));
            }
        }
    }
    match hover {
        Some((_, a, b, true)) => {
            painter.add(egui::Shape::dashed_line(
                &[xf.to_screen(a), xf.to_screen(b)],
                Stroke::new(2.5, centre),
                10.0,
                3.0,
            ));
        }
        Some((_, a, b, false)) => {
            // one rule in every view (MU49): what a click would take as
            // the reference is in the guides' blue, what it would change
            // in the interaction colour; a light edge under it, to read
            // on any fill
            let ink = if state.reference.is_some_and(|r| r.0 == view) {
                accent
            } else {
                snaps::GUIDE
            };
            let line = [xf.to_screen(a), xf.to_screen(b)];
            painter.line_segment(line, Stroke::new(6.0, egui::Color32::from_white_alpha(200)));
            painter.line_segment(line, Stroke::new(3.5, ink));
        }
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_outline_edge_stretches_and_an_end_extends_to_a_reference() {
        use creabim_core::{Sketch, Workplane};
        let pts = vec![
            DVec2::ZERO,
            DVec2::new(4.0, 0.0),
            DVec2::new(4.0, 3.0),
            DVec2::new(0.0, 3.0),
        ];
        let sk = Sketch::polyline(None, Workplane::ground(0.0), pts.clone(), true);
        // the right-hand edge out to x = 6: its neighbours get longer
        let line = (DVec2::new(6.0, -1.0), DVec2::new(6.0, 9.0));
        let at = DVec2::new(4.0, 1.0);
        let out = sketch_edge_to(&sk, (pts[1], pts[2]), line, at, false).unwrap();
        assert!(out.points[1].distance(DVec2::new(6.0, 0.0)) < 1e-9);
        assert!(out.points[2].distance(DVec2::new(6.0, 3.0)) < 1e-9);
        assert_eq!((out.points[0], out.points[3]), (pts[0], pts[3]));
        // to a slanted line the edge goes too, its neighbours keeping
        // their direction (MU48)
        let slant = (DVec2::new(6.0, 0.0), DVec2::new(7.0, 3.0));
        let out = sketch_edge_to(&sk, (pts[1], pts[2]), slant, at, false).unwrap();
        assert!(out.points[1].distance(DVec2::new(6.0, 0.0)) < 1e-9);
        assert!(out.points[2].distance(DVec2::new(7.0, 3.0)) < 1e-9);
        // Extend: a reference met past an end takes that end out, even
        // with the click by the other one
        let past = (DVec2::new(-1.0, 4.0), DVec2::new(5.0, -2.0));
        let near_far = DVec2::new(4.0, 2.9);
        let out = sketch_edge_to(&sk, (pts[1], pts[2]), past, near_far, true).unwrap();
        assert!(out.points[1].distance(DVec2::new(4.0, -1.0)) < 1e-9);
        assert_eq!(out.points[2], pts[2]);
        // one that crosses the edge trims the end nearer the click
        let cut = (DVec2::new(0.0, 1.0), DVec2::new(9.0, 1.0));
        let out = sketch_edge_to(&sk, (pts[1], pts[2]), cut, near_far, true).unwrap();
        assert!(out.points[2].distance(DVec2::new(4.0, 1.0)) < 1e-9);
        let out = sketch_edge_to(&sk, (pts[1], pts[2]), cut, DVec2::new(4.0, 0.2), true).unwrap();
        assert!(out.points[1].distance(DVec2::new(4.0, 1.0)) < 1e-9);
        // an edge square to the reference does not stretch to it
        let at = DVec2::new(2.0, 0.0);
        assert!(sketch_edge_to(&sk, (pts[0], pts[1]), line, at, false).is_err());
        // an open line: the end nearer the click extends to the reference
        let open = Sketch::polyline(None, Workplane::ground(0.0), pts[..2].to_vec(), false);
        let at = DVec2::new(3.5, 0.0);
        let out = sketch_edge_to(&open, (pts[0], pts[1]), line, at, true).unwrap();
        assert!(out.points[0].distance(DVec2::ZERO) < 1e-9);
        assert!(out.points[1].distance(DVec2::new(6.0, 0.0)) < 1e-9);
        // and alone it stretches square to itself
        let level = (DVec2::new(-5.0, 2.0), DVec2::new(5.0, 2.0));
        let out = sketch_edge_to(&open, (pts[0], pts[1]), level, at, false).unwrap();
        assert!(out.points[0].distance(DVec2::new(0.0, 2.0)) < 1e-9);
        assert!(out.points[1].distance(DVec2::new(4.0, 2.0)) < 1e-9);
    }
    use creabim_core::Element;

    fn level(doc: &Document) -> ElementId {
        doc.model().levels().next().unwrap().0.id
    }

    /// A wall like the sample's first, from `a` to `b` on `level`.
    fn wall(doc: &Document, level: ElementId, a: DVec2, b: DVec2) -> Element {
        let mut w = doc
            .model()
            .iter()
            .find_map(|e| match &e.kind {
                ElementKind::Wall(w) => Some(w.clone()),
                _ => None,
            })
            .unwrap();
        (w.level, w.start, w.end) = (level, a, b);
        Element::new("Wall", ElementKind::Wall(w))
    }

    #[test]
    fn corner_trims_one_wall_and_extends_the_other_keeping_the_parts_clicked() {
        let mut doc = Document::from_model(creabim_core::sample::sample_house());
        let l = level(&doc);
        let v = |x, y| DVec2::new(x, y);
        // a runs along y = 0 from 20 to 30, past the corner at x = 25;
        // b runs up x = 25 from y = 2 to 6, short of it
        let a = wall(&doc, l, v(20.0, 0.0), v(30.0, 0.0));
        let b = wall(&doc, l, v(25.0, 2.0), v(25.0, 6.0));
        let (ia, ib) = (a.id, b.id);
        doc.commit(Transaction::new("Walls").insert(a).insert(b))
            .unwrap();
        let line = |doc: &Document, id| corner_line(doc.model().get(id).unwrap()).unwrap();
        // keep a's left part, b's (only) part
        let (tx, hit) = corner_tx(doc.model(), (ia, v(21.0, 0.0)), (ib, v(25.0, 5.0))).unwrap();
        assert!(hit.distance(v(25.0, 0.0)) < 1e-9);
        doc.commit(tx).unwrap();
        assert_eq!(line(&doc, ia), (v(20.0, 0.0), v(25.0, 0.0)), "a trimmed");
        assert_eq!(line(&doc, ib), (v(25.0, 0.0), v(25.0, 6.0)), "b extended");
        // one undo step for both
        doc.undo();
        assert_eq!(line(&doc, ia), (v(20.0, 0.0), v(30.0, 0.0)));
        assert_eq!(line(&doc, ib), (v(25.0, 2.0), v(25.0, 6.0)));
        // the other part of a kept instead
        let (tx, _) = corner_tx(doc.model(), (ia, v(29.0, 0.0)), (ib, v(25.0, 5.0))).unwrap();
        doc.commit(tx).unwrap();
        assert_eq!(line(&doc, ia), (v(25.0, 0.0), v(30.0, 0.0)));
        // parallel never meet; already met says so
        assert!(corner_ends(
            (v(0.0, 0.0), v(1.0, 0.0)),
            v(0.5, 0.0),
            (v(0.0, 1.0), v(1.0, 1.0)),
            v(0.5, 1.0)
        )
        .is_none());
        let again = corner_tx(doc.model(), (ia, v(29.0, 0.0)), (ib, v(25.0, 5.0)));
        assert!(again.is_err_and(|e| e.contains("already")));
        // two selected, no clicks: each keeps its longer side
        let (ka, kb) =
            longer_parts((v(0.0, 0.0), v(10.0, 0.0)), (v(3.0, -1.0), v(3.0, 5.0))).unwrap();
        assert_eq!((ka, kb), (v(10.0, 0.0), v(3.0, 5.0)));
    }

    #[test]
    fn turns_take_the_short_way_round() {
        assert!((turn_to(DVec2::X, DVec2::Y) - 90.0).abs() < 1e-9);
        assert!((turn_to(DVec2::X, DVec2::new(1.0, 1.0)) - 45.0).abs() < 1e-9);
        // lines have no direction: pointing back is parallel already
        assert!(turn_to(DVec2::X, -DVec2::X).abs() < 1e-9);
        assert!((turn_to(DVec2::X, DVec2::new(-1.0, 1.0)) + 45.0).abs() < 1e-9);
    }

    #[test]
    fn a_wall_turns_parallel_to_a_reference_and_moves_flush() {
        let mut doc = Document::from_model(creabim_core::sample::sample_house());
        let l = level(&doc);
        let w = wall(&doc, l, DVec2::new(20.0, 0.0), DVec2::new(24.0, 1.0));
        let id = w.id;
        doc.commit(Transaction::new("Wall").insert(w)).unwrap();
        // the reference: a line at 30° through (30, 5)
        let r0 = DVec2::new(30.0, 5.0);
        let r1 = r0 + DVec2::from_angle(30f64.to_radians());
        let ElementKind::Wall(w) = &doc.model().get(id).unwrap().kind else {
            unreachable!()
        };
        let (t0, t1) = (w.start, w.end);
        let mut e = doc.model().get(id).unwrap().clone();
        for t in align_moves(t0, t1, r0, r1, DVec2::new(22.0, 0.5)) {
            e = transformed(doc.model(), &e, &t).unwrap();
        }
        let ElementKind::Wall(a) = &e.kind else {
            unreachable!()
        };
        let r = (r1 - r0).normalize();
        // parallel, and both ends on the reference line
        assert!((a.end - a.start).normalize().perp_dot(r).abs() < 1e-9);
        for p in [a.start, a.end] {
            assert!((p - r0).perp_dot(r).abs() < 1e-9, "{p}");
        }
        // its length is kept
        assert!((a.length() - w.length()).abs() < 1e-9);
    }

    #[test]
    fn extend_stretches_the_nearer_end_to_the_reference_and_undoes() {
        let mut doc = Document::from_model(creabim_core::sample::sample_house());
        let l = level(&doc);
        let w = wall(&doc, l, DVec2::new(20.0, 0.0), DVec2::new(20.0, 3.0));
        let id = w.id;
        doc.commit(Transaction::new("Wall").insert(w)).unwrap();
        let view = doc
            .model()
            .iter()
            .find(|e| matches!(&e.kind, ElementKind::View(v) if matches!(v.kind, creabim_core::ViewKind::Plan { level, .. } if level == l)))
            .unwrap()
            .id;
        let mut state = PlanState::default();
        let mut sel = Selection::default();
        state.align.mode = AlignMode::Extend;
        // a slab edge along y = 5
        state.align.reference = Some((view, DVec2::new(15.0, 5.0), DVec2::new(25.0, 5.0), None));
        let items = vec![PlanItem::Line {
            id,
            pen: creabim_geom::plan::Pen::Cut,
            a: DVec2::new(20.0, 0.0),
            b: DVec2::new(20.0, 3.0),
        }];
        let kind = PaneKind::Plan { level: l };
        click(
            &mut doc,
            view,
            kind,
            &items,
            &[],
            DVec2::new(20.0, 2.8),
            None,
            0.1,
            0.01,
            &mut state,
            &mut sel,
        )
        .unwrap();
        let ElementKind::Wall(w) = &doc.model().get(id).unwrap().kind else {
            unreachable!()
        };
        assert!((w.end - DVec2::new(20.0, 5.0)).length() < 1e-9, "{}", w.end);
        assert_eq!(w.start, DVec2::new(20.0, 0.0));
        assert_eq!(doc.undo_label(), Some("Extend to reference"));
        doc.undo();
        let ElementKind::Wall(w) = &doc.model().get(id).unwrap().kind else {
            unreachable!()
        };
        assert_eq!(w.end, DVec2::new(20.0, 3.0));
        // the reference stays for the next pick
        assert!(state.align.reference.is_some());
    }

    /// The rectangle `lo`–`hi` drawn as `id`.s cut outline.
    fn fill(id: ElementId, lo: DVec2, hi: DVec2) -> PlanItem {
        PlanItem::Polygon {
            id,
            pen: creabim_geom::plan::Pen::Cut,
            points: vec![lo, DVec2::new(hi.x, lo.y), hi, DVec2::new(lo.x, hi.y)],
        }
    }

    #[test]
    fn stretch_moves_a_walls_side_to_the_reference_and_keeps_the_other() {
        let mut doc = Document::from_model(creabim_core::sample::sample_house());
        let l = level(&doc);
        let w = wall(&doc, l, DVec2::new(20.0, 0.0), DVec2::new(26.0, 0.0));
        let (id, t) = (
            w.id,
            match &w.kind {
                ElementKind::Wall(w) => w.thickness,
                _ => unreachable!(),
            },
        );
        doc.commit(Transaction::new("Wall").insert(w)).unwrap();
        let mut state = PlanState::default();
        let mut sel = Selection::default();
        state.align.mode = AlignMode::Stretch;
        let view = ElementId::new();
        // a reference 0.5 north of the wall's north face
        let north = t / 2.0;
        state.align.reference = Some((
            view,
            DVec2::new(0.0, north + 0.5),
            DVec2::new(1.0, north + 0.5),
            None,
        ));
        let items = vec![fill(id, DVec2::new(20.0, -north), DVec2::new(26.0, north))];
        click(
            &mut doc,
            view,
            PaneKind::Plan { level: l },
            &items,
            &[],
            DVec2::new(23.0, north),
            None,
            0.05,
            0.01,
            &mut state,
            &mut sel,
        )
        .unwrap();
        let ElementKind::Wall(after) = doc.model().get(id).unwrap().kind.clone() else {
            unreachable!()
        };
        // thicker by the gap, its south face where it was
        assert!((after.thickness - t - 0.5).abs() < 1e-9);
        assert!((after.start.y - after.thickness / 2.0 + north).abs() < 1e-9);
        assert_eq!(doc.undo_label(), Some("Stretch to reference"));
        // and its east end to a line at x = 28, in the same mode
        state.align.reference = Some((view, DVec2::new(28.0, 0.0), DVec2::new(28.0, 1.0), None));
        let n = after.thickness / 2.0 + after.start.y;
        let s = after.start.y - after.thickness / 2.0;
        let items = vec![fill(id, DVec2::new(20.0, s), DVec2::new(26.0, n))];
        click(
            &mut doc,
            view,
            PaneKind::Plan { level: l },
            &items,
            &[],
            DVec2::new(26.0, after.start.y),
            None,
            0.05,
            0.01,
            &mut state,
            &mut sel,
        )
        .unwrap();
        let ElementKind::Wall(after) = &doc.model().get(id).unwrap().kind else {
            unreachable!()
        };
        assert!((after.end.x - 28.0).abs() < 1e-9);
        assert!((after.start.x - 20.0).abs() < 1e-9);
    }

    #[test]
    fn stretch_takes_an_edge_nearly_parallel_to_the_reference() {
        let mut doc = Document::from_model(creabim_core::sample::sample_house());
        let l = level(&doc);
        let w = wall(&doc, l, DVec2::new(20.0, 0.0), DVec2::new(26.0, 0.0));
        let (id, t) = (
            w.id,
            match &w.kind {
                ElementKind::Wall(w) => w.thickness,
                _ => unreachable!(),
            },
        );
        doc.commit(Transaction::new("Wall").insert(w)).unwrap();
        let mut state = PlanState::default();
        let mut sel = Selection::default();
        state.align.mode = AlignMode::Stretch;
        let view = ElementId::new();
        let h = t / 2.0;
        let items = vec![fill(id, DVec2::new(20.0, -h), DVec2::new(26.0, h))];
        let stretch = |doc: &mut Document, state: &mut PlanState, sel: &mut Selection| {
            click(
                doc,
                view,
                PaneKind::Plan { level: l },
                &items,
                &[],
                DVec2::new(26.0, 0.0),
                None,
                0.05,
                0.01,
                state,
                sel,
            )
            .unwrap();
        };
        // a line 3° off upright through (28, 0): the end meets it there
        let r0 = DVec2::new(28.0, 0.0);
        let r1 = r0 + DVec2::from_angle(93f64.to_radians());
        state.align.reference = Some((view, r0, r1, None));
        stretch(&mut doc, &mut state, &mut sel);
        let ElementKind::Wall(after) = &doc.model().get(id).unwrap().kind else {
            unreachable!()
        };
        assert!((after.end.x - 28.0).abs() < 1e-9, "{}", after.end);
        assert!(state.status.as_deref().unwrap().starts_with("Stretched"));
        // 20° off is too far
        doc.undo();
        let r1 = r0 + DVec2::from_angle(110f64.to_radians());
        state.align.reference = Some((view, r0, r1, None));
        stretch(&mut doc, &mut state, &mut sel);
        assert!(state.status.as_deref().unwrap().contains("20°"));
        let ElementKind::Wall(after) = &doc.model().get(id).unwrap().kind else {
            unreachable!()
        };
        assert_eq!(after.end, DVec2::new(26.0, 0.0));
    }

    #[test]
    fn stretch_in_a_section_drops_a_slabs_underside_to_a_line() {
        let mut doc = Document::from_model(creabim_core::sample::sample_house());
        let (id, s) = doc
            .model()
            .iter()
            .find_map(|e| match &e.kind {
                ElementKind::Slab(s) => Some((e.id, s.clone())),
                _ => None,
            })
            .unwrap();
        let z0 = doc.model().level(s.level).unwrap().elevation;
        let (top, bottom) = (s.top(z0), s.top(z0) - s.thickness);
        // a section along x through the slab: it is cut from x = 0 to 10
        let kind = PaneKind::Section {
            start: DVec2::new(0.0, 3.0),
            end: DVec2::new(10.0, 3.0),
        };
        let items = vec![fill(id, DVec2::new(0.0, bottom), DVec2::new(10.0, top))];
        let mut state = PlanState::default();
        let mut sel = Selection::default();
        state.align.mode = AlignMode::Stretch;
        let view = ElementId::new();
        state.align.reference = Some((
            view,
            DVec2::new(-5.0, bottom - 0.3),
            DVec2::new(15.0, bottom - 0.3),
            None,
        ));
        click(
            &mut doc,
            view,
            kind,
            &items,
            &[],
            DVec2::new(5.0, bottom),
            None,
            0.05,
            0.01,
            &mut state,
            &mut sel,
        )
        .unwrap();
        let ElementKind::Slab(after) = &doc.model().get(id).unwrap().kind else {
            unreachable!()
        };
        assert!((after.thickness - s.thickness - 0.3).abs() < 1e-9);
        assert!((after.top(z0) - top).abs() < 1e-9);
        // and its east edge out to x = 12 in the same section
        state.align.reference = Some((view, DVec2::new(12.0, 0.0), DVec2::new(12.0, 1.0), None));
        let items = vec![fill(
            id,
            DVec2::new(0.0, bottom - 0.3),
            DVec2::new(10.0, top),
        )];
        click(
            &mut doc,
            view,
            kind,
            &items,
            &[],
            DVec2::new(10.0, top - 0.1),
            None,
            0.05,
            0.01,
            &mut state,
            &mut sel,
        )
        .unwrap();
        let ElementKind::Slab(after) = &doc.model().get(id).unwrap().kind else {
            unreachable!()
        };
        let east = after.boundary.iter().map(|p| p.x).fold(f64::MIN, f64::max);
        assert!((east - 12.0).abs() < 1e-9, "{:?}", after.boundary);
    }
    #[test]
    fn a_beams_centre_line_aligns_to_a_columns_in_plan() {
        let mut doc = Document::from_model(creabim_core::sample::sample_house());
        let l = level(&doc);
        let col = Element::new(
            "Post",
            ElementKind::Column(creabim_core::members::Column::new(
                l,
                DVec2::new(40.0, 5.0),
                creabim_core::members::Profile::Rectangle {
                    width: 0.3,
                    depth: 0.3,
                },
            )),
        );
        let beam = Element::new(
            "Beam",
            ElementKind::Beam(creabim_core::members::Beam::new(
                l,
                DVec2::new(41.0, 0.0),
                DVec2::new(41.0, 8.0),
            )),
        );
        let (cid, bid) = (col.id, beam.id);
        doc.commit(Transaction::new("Add").insert(col).insert(beam))
            .unwrap();
        // the plan's centre lines, as the pane finds them
        let scene = (*creabim_geom::shared_scene(doc.model())).clone();
        let centres: Vec<CentreLine> = [cid, bid]
            .iter()
            .flat_map(|id| crate::centres::of(doc.model(), &scene, *id))
            .filter_map(|c| {
                let (a, b) = crate::centres::edge_on(&c, -DVec3::Z, &|p| p.truncate())?;
                Some((c.id, a, b))
            })
            .collect();
        assert_eq!(centres.len(), 3, "the post's two and the beam's one");
        let items = vec![
            fill(cid, DVec2::new(39.85, 4.85), DVec2::new(40.15, 5.15)),
            fill(bid, DVec2::new(40.9, 0.0), DVec2::new(41.1, 8.0)),
        ];
        let mut state = PlanState::default();
        let mut sel = Selection::default();
        state.align.mode = AlignMode::Align;
        let view = ElementId::new();
        let kind = PaneKind::Plan { level: l };
        let go = |doc: &mut Document, state: &mut PlanState, sel: &mut Selection, at: DVec2| {
            click(
                doc, view, kind, &items, &centres, at, None, 0.05, 0.01, state, sel,
            )
            .unwrap();
        };
        // the post's centre line along y, then the beam's
        go(&mut doc, &mut state, &mut sel, DVec2::new(40.0, 5.1));
        assert!(
            state
                .status
                .as_deref()
                .unwrap()
                .contains("centre line of Post"),
            "{:?}",
            state.status
        );
        go(&mut doc, &mut state, &mut sel, DVec2::new(41.0, 3.0));
        let ElementKind::Beam(b) = &doc.model().get(bid).unwrap().kind else {
            unreachable!()
        };
        assert!((b.start.x - 40.0).abs() < 1e-9, "{}", b.start);
        assert!((b.end.x - 40.0).abs() < 1e-9, "{}", b.end);
        // and near its edge, the edge is still what is taken
        assert_eq!(
            pick_line(&items, &centres, DVec2::new(40.88, 3.0), 0.05).map(|l| l.3),
            Some(false)
        );
    }
}
