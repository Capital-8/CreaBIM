//! The site in sections, elevations and 3D as each view shows it: the
//! ground before or after its works, the ground as it was dashed under
//! the ground as built, cut and fill in colour, and level tags where the
//! works change the ground.

use crate::plan::{Pen, PlanItem};
use crate::section::SectionFrame;
use crate::SceneItem;
use creabim_core::{ElementId, ElementKind, GroundState, Model, Site, SiteForm, SiteView};
use glam::DVec2;

/// The light ink the ground as it was is written in.
pub const EXISTING_INK: [f32; 3] = [0.45, 0.45, 0.45];

fn site_of(model: &Model, id: ElementId) -> Option<&Site> {
    match model.get(id).map(|e| &e.kind) {
        Some(ElementKind::Site(s)) => Some(s),
        _ => None,
    }
}

/// `scene` as it was before the works: each terrain as surveyed, its
/// contours on the built ground gone, and no roads. Nothing changes for
/// a view showing the ground after its works.
pub fn scene_as_shown(model: &Model, scene: &mut Vec<SceneItem>, ground: GroundState) {
    if ground != GroundState::Before {
        return;
    }
    scene.retain(|i| match site_of(model, i.id) {
        Some(s) if matches!(s.form, SiteForm::Road(_)) => false,
        Some(s) if i.finish == Some(crate::scene::GROUND_LINES) => {
            !crate::earth::shaped(model, s).is_some_and(|sh| sh.changed())
        }
        _ => true,
    });
    for i in scene.iter_mut() {
        let Some(s) = site_of(model, i.id) else {
            continue;
        };
        if i.finish.is_none() {
            if let Some(m) = crate::parts::terrain_mesh_before(model, s) {
                i.mesh = m;
            }
        }
    }
}

/// The surveyed ground, see-through, where the works moved it: for a 3D
/// view showing both grounds.
pub fn ground_ghosts(model: &Model, scene: &[SceneItem]) -> Vec<SceneItem> {
    let mut seen = std::collections::HashSet::new();
    let mut out = vec![];
    for i in scene {
        if !seen.insert(i.id) {
            continue;
        }
        let Some(s) = site_of(model, i.id) else {
            continue;
        };
        if let Some(m) = crate::parts::ground_moved_mesh(model, s) {
            out.push(SceneItem {
                id: i.id,
                mesh: m,
                color: [0.62, 0.62, 0.64],
                surface: [0.9, 0.0, 0.35],
                ghost: false,
                layer: None,
                finish: Some(EXISTING_GROUND),
            });
        }
    }
    out
}

/// The finish of the surveyed ground shown see-through in 3D.
pub const EXISTING_GROUND: &str = "existing ground";

/// One sample across a terrain along a section line: how far along, the
/// ground as it was and as it is after the works.
#[derive(Clone, Copy, Debug)]
struct Sample {
    x: f64,
    was: f64,
    now: f64,
}

impl Sample {
    fn moved(&self) -> bool {
        (self.was - self.now).abs() > 0.01
    }
}

/// The runs along the section where the ground has its heights, each
/// sampled every tenth of a metre, with the terrain's level added.
fn profile(model: &Model, s: &Site, frame: &SectionFrame) -> Vec<Vec<Sample>> {
    let Some(sh) = crate::earth::shaped(model, s) else {
        return vec![];
    };
    let base = creabim_core::members::elevation(model, s.level);
    let n = ((frame.length / 0.1).ceil() as usize).clamp(1, 4000);
    let mut runs: Vec<Vec<Sample>> = vec![];
    let mut on = false;
    for i in 0..=n {
        let x = frame.length * i as f64 / n as f64;
        let p = frame.origin + frame.along * x;
        match (sh.existing.height(p), sh.ground.height(p)) {
            (Some(was), Some(now)) => {
                if !on {
                    runs.push(vec![]);
                    on = true;
                }
                runs.last_mut().unwrap().push(Sample {
                    x,
                    was: base + was,
                    now: base + now,
                });
            }
            _ => on = false,
        }
    }
    runs
}

/// The corners of a line: Douglas–Peucker to `tol`, its ends kept.
fn corners(pts: &[DVec2], tol: f64) -> Vec<usize> {
    fn walk(pts: &[DVec2], a: usize, b: usize, tol: f64, out: &mut Vec<usize>) {
        if b <= a + 1 {
            return;
        }
        let (pa, pb) = (pts[a], pts[b]);
        let d = (pb - pa).normalize_or_zero();
        let (mut far, mut at) = (0.0, a);
        for (k, p) in pts.iter().enumerate().take(b).skip(a + 1) {
            let off = (*p - pa).perp_dot(d).abs();
            if off > far {
                far = off;
                at = k;
            }
        }
        if far > tol {
            walk(pts, a, at, tol, out);
            out.push(at);
            walk(pts, at, b, tol, out);
        }
    }
    if pts.is_empty() {
        return vec![];
    }
    let mut out = vec![0];
    walk(pts, 0, pts.len() - 1, tol, &mut out);
    if pts.len() > 1 {
        out.push(pts.len() - 1);
    }
    out
}

/// The site in a section or elevation along `frame` drawn at `scale`,
/// as view setting `sv` shows it, worked into `items` (the drawing
/// [`crate::section::section_view_in`] made from the scene as shown).
pub fn site_in_section(
    model: &Model,
    scene: &[SceneItem],
    frame: &SectionFrame,
    sv: &SiteView,
    scale: f64,
    items: &mut Vec<PlanItem>,
) {
    let mm = scale / 1000.0;
    for e in model.iter() {
        let ElementKind::Site(s) = &e.kind else {
            continue;
        };
        if !matches!(s.form, SiteForm::Terrain { .. }) || !scene.iter().any(|i| i.id == e.id) {
            continue;
        }
        if !crate::earth::shaped(model, s).is_some_and(|sh| sh.changed()) {
            continue;
        }
        let id = e.id;
        // the ground as it was, dashed by the section itself under the
        // cut: before the works it is the ground drawn, and with both it
        // is drawn grey, in elevations too
        if sv.ground != GroundState::After {
            items.retain(
                |it| !matches!(it, PlanItem::Line { id: i, pen: Pen::Hidden, .. } if *i == id),
            );
        }
        if sv.ground == GroundState::Before {
            // the soil layers under the ground as it was, whole
            items.retain(|it| match it {
                PlanItem::Line {
                    id: i,
                    pen: Pen::Contour,
                    ..
                } => *i != id,
                PlanItem::Text { id: i, .. } => *i != id,
                _ => true,
            });
            if let Some(sh) = crate::earth::shaped(model, s) {
                items.extend(crate::section::soil_layers(
                    model, id, s, &sh, frame, scale, true,
                ));
            }
            continue;
        }
        let runs = profile(model, s, frame);
        if sv.ground == GroundState::Both {
            for run in &runs {
                for w in run.windows(2) {
                    if w[0].moved() && w[1].moved() {
                        items.push(PlanItem::Line {
                            id,
                            pen: Pen::Existing,
                            a: DVec2::new(w[0].x, w[0].was),
                            b: DVec2::new(w[1].x, w[1].was),
                        });
                    }
                }
            }
        }
        // dug (below the ground as it was) and made up, washed over the
        // ground's cut but under what is drawn after it
        if sv.cut_fill {
            let mut tints = vec![];
            for run in &runs {
                let mut k = 0;
                while k < run.len() {
                    let sign = |q: &Sample| {
                        if !q.moved() {
                            0
                        } else if q.now < q.was {
                            -1
                        } else {
                            1
                        }
                    };
                    let sg = sign(&run[k]);
                    let mut j = k;
                    while j + 1 < run.len() && sign(&run[j + 1]) == sg {
                        j += 1;
                    }
                    if sg != 0 {
                        // from the daylight either side, so the wash meets
                        // the ground lines
                        let (a, b) = (k.saturating_sub(1), (j + 1).min(run.len() - 1));
                        let piece = &run[a..=b];
                        let mut pts: Vec<DVec2> =
                            piece.iter().map(|q| DVec2::new(q.x, q.now)).collect();
                        pts.extend(piece.iter().rev().map(|q| DVec2::new(q.x, q.was)));
                        let deep = piece.iter().map(|q| q.now - q.was).fold(0.0_f64, |m, d| {
                            if d.abs() > m.abs() {
                                d
                            } else {
                                m
                            }
                        });
                        tints.push(PlanItem::Tint {
                            id,
                            points: pts,
                            rgb: crate::parts::cut_fill_rgb(deep),
                        });
                    }
                    k = j + 1;
                }
            }
            let at = items
                .iter()
                .rposition(|it| {
                    matches!(it, PlanItem::Polygon { id: i, .. } | PlanItem::Fill { id: i, .. } if *i == id)
                })
                .map_or(0, |i| i + 1);
            items.splice(at..at, tints);
        }
        // level tags where the ground changes: at each corner of the
        // ground as built, its level bold, the ground as it was light
        // under it where they differ
        if sv.levels {
            let gap = 8.0 * mm;
            for run in &runs {
                let Some(first) = run.iter().position(Sample::moved) else {
                    continue;
                };
                let last = run.iter().rposition(Sample::moved).unwrap_or(first);
                let (a, b) = (first.saturating_sub(1), (last + 1).min(run.len() - 1));
                let piece = &run[a..=b];
                let pts: Vec<DVec2> = piece.iter().map(|q| DVec2::new(q.x, q.now)).collect();
                let mut placed: Option<f64> = None;
                for k in corners(&pts, 0.05) {
                    let q = piece[k];
                    if placed.is_some_and(|x| q.x - x < gap) {
                        continue;
                    }
                    placed = Some(q.x);
                    level_tag(model, id, q, mm, sv.tag(id, q.x), items);
                }
            }
        }
    }
}

/// A level tag on the ground at `q`, as any tag is drawn (MU142, owner:
/// one tag language): its level after the works in the tag text style and
/// the annotation colour, a leader down to the point it measures, ending
/// in the style's mark, through the bend put in it, and the level before
/// the works as a smaller line over it where the works moved the ground.
fn level_tag(
    model: &Model,
    id: ElementId,
    q: Sample,
    mm: f64,
    moved: Option<creabim_core::TagMove>,
    items: &mut Vec<PlanItem>,
) {
    let num = |z: f64| creabim_core::site_level(model, z);
    let look = crate::annotate::look_of(model, None, None);
    let top = q.now.max(q.was) + 3.0 * mm;
    let text = num(q.now);
    let mut was = look;
    was.size *= 0.8;
    let was_text = format!("was {}", num(q.was));
    let over = DVec2::new(0.0, (look.size + was.size) * 0.75 * mm);
    // the whole tag, its was-line over it where there is one: its middle
    // from its level's, and its half size
    let (lift, half) = {
        let w = look
            .width(&text)
            .max(if q.moved() { was.width(&was_text) } else { 0.0 });
        let h = 0.75 * look.size * mm + if q.moved() { over.y / 2.0 } else { 0.0 };
        (
            if q.moved() { over / 2.0 } else { DVec2::ZERO },
            DVec2::new(w * mm / 2.0 + 0.5 * mm, h),
        )
    };
    // where its words go: moved by hand in this view, or, where they
    // would land on something cut (a wall) or on a tag put before it,
    // aside to the clear side (owner, cut test: a tag on a retaining wall;
    // MU142: one moved aside landed on the next)
    let at = DVec2::new(q.x, top + 7.5 * mm);
    let at = match moved {
        Some(m) => at + m.by,
        None => clear_of(model, items, id, at + lift, half, mm) - lift,
    };
    let foot = DVec2::new(q.x, q.now);
    let mut level = crate::annotate::room_tag_item(id, text, at, look, mm);
    if let PlanItem::Annotation { item, .. } = &mut level {
        if let crate::annotation::Shape::Note { leader, elbow, .. } = &mut item.shape {
            *leader = Some(foot);
            *elbow = moved.and_then(|m| m.elbow).map(|e| foot + e);
        }
    }
    items.push(level);
    if q.moved() {
        items.push(crate::annotate::room_tag_item(
            id,
            was_text,
            at + over,
            was,
            mm,
        ));
    }
}

/// A tag's words at `at`, moved aside when they would lie on something
/// cut or on a tag of `own` put before it: to whichever side is clear,
/// else up over it. Of the places clear of those, the first that is clear
/// of the building's lines too (a kitchen's units seen beyond the cut)
/// wins: beside it, then stacked over its neighbour, then further along
/// (MU143: the tag at a wall's foot, kept off the wall and the next tag,
/// landed on the kitchen's lines).
fn clear_of(
    model: &Model,
    items: &[PlanItem],
    own: ElementId,
    at: DVec2,
    half: DVec2,
    mm: f64,
) -> DVec2 {
    let boxes: Vec<(DVec2, DVec2)> = items
        .iter()
        .filter_map(|it| match it {
            PlanItem::Polygon { id, points, .. } | PlanItem::Fill { id, points, .. }
                if *id != own =>
            {
                let lo = points.iter().copied().reduce(DVec2::min)?;
                let hi = points.iter().copied().reduce(DVec2::max)?;
                Some((lo, hi))
            }
            // the level tags put before it, a little room round them
            PlanItem::Annotation { id, .. } if *id == own => {
                let (b, _) = crate::declutter::text_box(it, mm)?;
                Some((b.lo - DVec2::splat(0.5 * mm), b.hi + DVec2::splat(0.5 * mm)))
            }
            _ => None,
        })
        .collect();
    // what is drawn of the building: not the ground's own lines, nor the
    // levels' and grids' lines that run across the whole drawing
    let datum = |id: &ElementId| {
        matches!(
            model.get(*id).map(|e| &e.kind),
            Some(ElementKind::Level(_) | ElementKind::Grid(_) | ElementKind::Site(_)) | None
        )
    };
    let lines: Vec<(DVec2, DVec2)> = items
        .iter()
        .filter_map(|it| match it {
            PlanItem::Line { id, a, b, .. } if *id != own && !datum(id) => Some((*a, *b)),
            _ => None,
        })
        .collect();
    let hits = |c: DVec2| {
        let (lo, hi) = (c - half, c + half);
        boxes
            .iter()
            .any(|(a, b)| lo.x < b.x && hi.x > a.x && lo.y < b.y && hi.y > a.y)
    };
    let crosses = |c: DVec2| {
        let (lo, hi) = (c - half, c + half);
        lines.iter().any(|(a, b)| segment_meets_box(*a, *b, lo, hi))
    };
    let side = 2.0 * half.x + 2.0 * mm;
    let rise = 2.0 * half.y + 1.5 * mm;
    let places: Vec<DVec2> = [
        (0.0, 0.0),
        (-1.0, 0.0),
        (1.0, 0.0),
        (-1.0, 1.0),
        (1.0, 1.0),
        (-2.0, 0.0),
        (2.0, 0.0),
        (-1.0, 2.0),
        (1.0, 2.0),
    ]
    .iter()
    .map(|(x, y)| at + DVec2::new(x * side, y * rise))
    .collect();
    if let Some(c) = places.iter().find(|c| !hits(**c) && !crosses(**c)) {
        return *c;
    }
    // nowhere clear of the lines: clear of what is cut and the tags, as
    // before
    for c in [places[0], places[1], places[2], places[5], places[6]] {
        if !hits(c) {
            return c;
        }
    }
    // hemmed in both sides: up until it is clear of them all
    let mut up = at;
    for _ in 0..60 {
        up += DVec2::Y * 2.0 * mm;
        if !hits(up) {
            return up;
        }
    }
    at
}

/// Whether the segment `a`–`b` passes through the box `lo`–`hi`.
fn segment_meets_box(a: DVec2, b: DVec2, lo: DVec2, hi: DVec2) -> bool {
    // clipped to the box, slab by slab
    let d = b - a;
    let (mut t0, mut t1) = (0.0_f64, 1.0_f64);
    for k in 0..2 {
        if d[k].abs() < 1e-12 {
            if a[k] < lo[k] || a[k] > hi[k] {
                return false;
            }
            continue;
        }
        let (mut s0, mut s1) = ((lo[k] - a[k]) / d[k], (hi[k] - a[k]) / d[k]);
        if s0 > s1 {
            std::mem::swap(&mut s0, &mut s1);
        }
        t0 = t0.max(s0);
        t1 = t1.min(s1);
        if t0 > t1 {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use creabim_core::{Element, Level, Pad, View};
    use glam::DVec3;

    /// A terrain rising 1 in 10 to the north, a pad levelled at 2 m in
    /// its middle, and a section across it from south to north.
    fn model(sv: SiteView) -> (Model, ElementId, ElementId) {
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
        if let SiteForm::Terrain { points, data, .. } = &mut t.form {
            for i in 0..=8 {
                for j in 0..=8 {
                    let (x, y) = (i as f64 * 5.0, j as f64 * 5.0);
                    points.push(DVec3::new(x, y, y * 0.1));
                }
            }
            data.survey = true;
            data.faceted = true;
            data.pads.push(Pad::new(
                creabim_core::polygon::rectangle(DVec2::new(15.0, 15.0), DVec2::new(25.0, 25.0)),
                2.0,
            ));
        }
        let te = Element::new("Terrain", ElementKind::Site(t));
        let tid = te.id;
        let mut v = View::section(DVec2::new(20.0, -2.0), DVec2::new(20.0, 42.0), 30.0, true);
        v.scale = 100.0;
        v.site = sv;
        // seen from the west: the section runs north along x = 20
        let ve = Element::new("Section", ElementKind::View(v));
        let vid = ve.id;
        (Model::from_elements([lv, te, ve]).unwrap(), tid, vid)
    }

    fn drawn(sv: SiteView) -> (Vec<PlanItem>, ElementId) {
        let (m, t, v) = model(sv);
        (crate::section::view_drawing(&m, v).unwrap().items, t)
    }

    #[test]
    fn a_section_shows_the_ground_before_after_or_both_with_its_levels() {
        // the terrain is the only site element
        let of = |items: &[PlanItem], f: &dyn Fn(&PlanItem) -> bool| {
            items
                .iter()
                .filter(|i| f(i) && !matches!(i, PlanItem::Annotation { .. }))
                .count()
        };
        let pen = |p: Pen| move |i: &PlanItem| matches!(i, PlanItem::Line { pen, .. } if *pen == p);
        let tint = |i: &PlanItem| matches!(i, PlanItem::Tint { .. });
        // a level tag: a tag's note with its leader down to the ground
        // (MU142: tags, in the tag style and the annotation colour)
        let levels = |items: &[PlanItem]| -> Vec<(String, DVec2, Option<[f32; 3]>)> {
            items
                .iter()
                .filter_map(|i| match i {
                    PlanItem::Annotation { item, .. } => match item.shape {
                        crate::annotation::Shape::Note {
                            leader: Some(foot), ..
                        } => Some((item.text.clone(), foot, item.look.rgb)),
                        _ => None,
                    },
                    _ => None,
                })
                .collect()
        };
        let was = |items: &[PlanItem]| {
            items
                .iter()
                .filter(|i| matches!(i, PlanItem::Annotation { item, .. } if item.text.starts_with("was ")))
                .count()
        };

        // after, as every section has drawn it: the old ground dashed,
        // and now the levels where the pad changes the ground
        let (after, _) = drawn(SiteView::default());
        assert!(of(&after, &pen(Pen::Hidden)) > 0);
        assert_eq!(of(&after, &pen(Pen::Existing)), 0);
        assert_eq!(of(&after, &tint), 0);
        let tags = levels(&after);
        // the daylights and the tops of bank, as room allows
        assert!(tags.len() >= 3, "{tags:?}");
        assert!(was(&after) >= 2);
        // no level left as plain text, each in the annotation colour, its
        // leader ending on the ground it measures
        assert!(!after.iter().any(|i| matches!(i, PlanItem::Text { .. })));
        for (text, foot, rgb) in &tags {
            assert!(rgb.is_none(), "{text}: a colour of its own");
            assert!(
                foot.y < 2.6 && foot.y > -0.5,
                "{text}: its leader ends at {foot}"
            );
        }
        // the pad's own level is among them
        assert!(tags
            .iter()
            .any(|(t, ..)| t.contains("2.00") || t.contains("2000")));

        // both: the old ground grey instead, and cut and fill washed
        let (both, _) = drawn(SiteView {
            ground: GroundState::Both,
            cut_fill: true,
            ..SiteView::default()
        });
        assert_eq!(of(&both, &pen(Pen::Hidden)), 0);
        assert!(of(&both, &pen(Pen::Existing)) > 0);
        // dug at the north end of the pad, made up at the south
        assert!(of(&both, &tint) >= 2);

        // before: no old ground, no tags, and the cut is the surveyed
        // ground's: at the pad's middle it is 2 m up, the ground's level
        let (before, _) = drawn(SiteView {
            ground: GroundState::Before,
            ..SiteView::default()
        });
        assert_eq!(of(&before, &pen(Pen::Hidden)), 0);
        assert!(levels(&before).is_empty());
        // the cut's top along the pad follows the slope, not the level
        // the highest the ground's cut reaches `x` along
        let top_at = |items: &[PlanItem], x: f64| {
            let mut top = f64::MIN;
            for i in items {
                let PlanItem::Fill { points, .. } = i else {
                    continue;
                };
                for k in 0..points.len() {
                    let (a, b) = (points[k], points[(k + 1) % points.len()]);
                    if (a.x - x) * (b.x - x) <= 0.0 && (b.x - a.x).abs() > 1e-9 {
                        top = top.max(a.y + (b.y - a.y) * (x - a.x) / (b.x - a.x));
                    }
                }
            }
            top
        };
        // 25 m along the section (y = 23 on site) the pad sits at 2, the
        // ground at 2.3
        let (b, a) = (top_at(&before, 25.0), top_at(&after, 25.0));
        assert!((b - 2.3).abs() < 0.05, "{b}");
        assert!((a - 2.0).abs() < 0.05, "{a}");
    }

    /// MU142 (seen in wave3l, Ridgeline's Site section): a level tag moved
    /// aside off a wall landed on the next tag. Level tags move off each
    /// other too (the approved mock-up: "the level tags still move off
    /// each other on their own"): no two overlap, a tag's was-line and all.
    #[test]
    fn level_tags_do_not_lie_on_each_other() {
        let m = creabim_core::ridgeline::ridgeline_house();
        let view = m.iter().find(|e| e.name == "Site section").unwrap().id;
        let d = crate::section::view_drawing(&m, view).unwrap();
        let mm = match &m.get(view).unwrap().kind {
            ElementKind::View(v) => v.scale / 1000.0,
            _ => unreachable!(),
        };
        let boxes: Vec<(String, crate::declutter::TextBox)> = d
            .items
            .iter()
            .filter(|i| {
                matches!(i, PlanItem::Annotation { id, .. }
                    if matches!(m.get(*id).map(|e| &e.kind), Some(ElementKind::Site(s)) if matches!(s.form, SiteForm::Terrain { .. })))
            })
            .filter_map(|i| {
                let PlanItem::Annotation { item, .. } = i else { return None };
                Some((item.text.clone(), crate::declutter::text_box(i, mm)?.0))
            })
            .collect();
        assert!(boxes.len() >= 3, "{}", boxes.len());
        for (k, (ta, a)) in boxes.iter().enumerate() {
            for (tb, b) in &boxes[k + 1..] {
                assert!(!a.overlaps(b), "{ta} lies on {tb}: {a:?} {b:?}");
            }
        }
    }

    /// MU143 (the owner's open list): in Ridgeline's Site section the tag
    /// at the left wall's foot, kept off the wall and the tag beside it,
    /// sat over the kitchen's lines. A level tag keeps off the building's
    /// lines where there is room: here, stacked over its neighbour.
    #[test]
    fn level_tags_keep_off_the_buildings_lines() {
        let m = creabim_core::ridgeline::ridgeline_house();
        let view = m.iter().find(|e| e.name == "Site section").unwrap().id;
        let d = crate::section::view_drawing(&m, view).unwrap();
        let mm = match &m.get(view).unwrap().kind {
            ElementKind::View(v) => v.scale / 1000.0,
            _ => unreachable!(),
        };
        let built = |id: &ElementId| {
            !matches!(
                m.get(*id).map(|e| &e.kind),
                Some(ElementKind::Level(_) | ElementKind::Grid(_) | ElementKind::Site(_)) | None
            )
        };
        let lines: Vec<(DVec2, DVec2)> = d
            .items
            .iter()
            .filter_map(|i| match i {
                PlanItem::Line { id, a, b, .. } if built(id) => Some((*a, *b)),
                _ => None,
            })
            .collect();
        let mut tags = 0;
        for i in &d.items {
            let PlanItem::Annotation { id, item } = i else {
                continue;
            };
            if !matches!(m.get(*id).map(|e| &e.kind), Some(ElementKind::Site(s)) if matches!(s.form, SiteForm::Terrain { .. }))
            {
                continue;
            }
            tags += 1;
            let b = crate::declutter::text_box(i, mm).unwrap().0;
            let on: Vec<_> = lines
                .iter()
                .filter(|(a, c)| segment_meets_box(*a, *c, b.lo, b.hi))
                .collect();
            assert!(
                on.is_empty(),
                "{} lies on {} of the building's lines, e.g. {:?}",
                item.text,
                on.len(),
                on[0]
            );
        }
        assert!(tags >= 3, "{tags}");
    }

    #[test]
    fn the_3d_ground_before_its_works_is_the_surveyed_one() {
        let (m, t, _) = model(SiteView::default());
        let mut scene = crate::scene::drawn_scene(&m);
        let top = |s: &[SceneItem]| {
            s.iter()
                .filter(|i| i.id == t && i.finish.is_none())
                .flat_map(|i| i.mesh.positions.iter())
                .filter(|p| {
                    (14.9..25.1).contains(&p[0]) && (14.9..25.1).contains(&p[1]) && p[2] > 0.5
                })
                .map(|p| p[2])
                .fold(f32::MAX, f32::min)
        };
        // the pad's level, banks aside
        assert!(top(&scene) > 1.9, "{}", top(&scene));
        let ghosts = ground_ghosts(&m, &scene);
        assert_eq!(ghosts.len(), 1);
        scene_as_shown(&m, &mut scene, GroundState::Before);
        // the slope it was, down to 1.5 at the pad's south edge
        assert!((top(&scene) - 1.5).abs() < 0.05, "{}", top(&scene));
    }
}
