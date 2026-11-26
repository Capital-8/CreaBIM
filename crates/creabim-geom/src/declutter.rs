//! Keeping a plan's text readable: room names, notes, tags and plain
//! labels are nudged off the lines and other text they would sit on, to
//! the nearest clear spot, with a leader back when they move far.

use crate::annotate::{dimension_geometry, tag_geometry};
use crate::annotation::Shape;
use crate::plan::{is_bubble, Pen, PlanItem};
use glam::DVec2;

/// A box on the drawing, in drawing units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextBox {
    pub lo: DVec2,
    pub hi: DVec2,
}

impl TextBox {
    fn around(pts: impl IntoIterator<Item = DVec2>) -> Option<Self> {
        let mut it = pts.into_iter();
        let first = it.next()?;
        let (lo, hi) = it.fold((first, first), |(lo, hi), p| (lo.min(p), hi.max(p)));
        Some(Self { lo, hi })
    }

    pub(crate) fn moved(self, d: DVec2) -> Self {
        Self {
            lo: self.lo + d,
            hi: self.hi + d,
        }
    }

    fn shrunk(self, by: f64) -> Self {
        let c = (self.lo + self.hi) / 2.0;
        Self {
            lo: (self.lo + DVec2::splat(by)).min(c),
            hi: (self.hi - DVec2::splat(by)).max(c),
        }
    }

    pub fn overlaps(&self, o: &TextBox) -> bool {
        self.lo.x < o.hi.x && o.lo.x < self.hi.x && self.lo.y < o.hi.y && o.lo.y < self.hi.y
    }

    fn centre(&self) -> DVec2 {
        (self.lo + self.hi) / 2.0
    }

    /// Does segment `a` → `b` pass through the box?
    pub fn crossed_by(&self, a: DVec2, b: DVec2) -> bool {
        // Liang-Barsky clipping
        let d = b - a;
        let (mut t0, mut t1) = (0.0f64, 1.0f64);
        for (p, q) in [
            (-d.x, a.x - self.lo.x),
            (d.x, self.hi.x - a.x),
            (-d.y, a.y - self.lo.y),
            (d.y, self.hi.y - a.y),
        ] {
            if p.abs() < 1e-15 {
                if q < 0.0 {
                    return false;
                }
            } else {
                let r = q / p;
                if p < 0.0 {
                    t0 = t0.max(r);
                } else {
                    t1 = t1.min(r);
                }
            }
        }
        t0 < t1
    }
}

/// Size of plain labels' text, paper mm (as [`crate::paper::draw`] draws them).
const LABEL_SIZE: f64 = 2.5;

/// The text box of a label, tag or note, and whether the pass may move it.
pub fn text_box(item: &PlanItem, mm: f64) -> Option<(TextBox, bool)> {
    match item {
        PlanItem::Label { at, text, .. } => {
            if is_bubble(text) {
                let r = 2.5 * crate::plan::bubble_scale(text) * mm;
                return Some((
                    TextBox {
                        lo: *at - DVec2::splat(r),
                        hi: *at + DVec2::splat(r),
                    },
                    false,
                ));
            }
            let half = DVec2::new(
                crate::paper::text_width(text, LABEL_SIZE) / 2.0,
                LABEL_SIZE * 0.6,
            ) * mm;
            Some((
                TextBox {
                    lo: *at - half,
                    hi: *at + half,
                },
                true,
            ))
        }
        PlanItem::Tag {
            anchor,
            at,
            elbow,
            text,
            round,
            look,
            ..
        } => {
            let g = tag_geometry(*anchor, *at, *elbow, text, *round, look, mm);
            let b = TextBox::around(g.outline.iter().copied()).or_else(|| {
                let half = DVec2::new(look.width(text) / 2.0, look.size * 0.6) * mm;
                Some(TextBox {
                    lo: *at - half,
                    hi: *at + half,
                })
            })?;
            Some((b, true))
        }
        PlanItem::Annotation { item: a, .. } => {
            let m = a.marks(mm);
            let pts = m.texts.iter().flat_map(|t| {
                let w = a.look.width(&t.text) * mm;
                let h = a.look.size * 0.6 * mm;
                let (dir, up) = (
                    DVec2::from_angle(t.angle),
                    DVec2::from_angle(t.angle).perp(),
                );
                let x0 = match t.anchor {
                    crate::paper::Anchor::Start => 0.0,
                    crate::paper::Anchor::Middle => -w / 2.0,
                    crate::paper::Anchor::End => -w,
                };
                let o = t.at + dir * x0;
                [
                    o - up * h,
                    o + dir * w + up * h,
                    o + dir * w - up * h,
                    o + up * h,
                ]
            });
            let b = TextBox::around(pts)?;
            Some((b, matches!(a.shape, Shape::Note { .. })))
        }
        PlanItem::Dimension {
            a,
            b,
            offset,
            text,
            look,
            witness,
            label,
            ..
        } => {
            let g =
                crate::annotate::dimension_layout(*a, *b, *offset, *witness, look, mm, text, label);
            let w = look.width(text) * mm;
            let h = look.size * 0.6 * mm;
            let (dir, up) = (
                DVec2::from_angle(g.angle),
                DVec2::from_angle(g.angle).perp(),
            );
            let c = g.text_at;
            let b = TextBox::around([
                c - dir * w / 2.0 - up * h,
                c + dir * w / 2.0 - up * h,
                c + dir * w / 2.0 + up * h,
                c - dir * w / 2.0 + up * h,
            ])?;
            Some((b, false))
        }
        _ => None,
    }
}

/// The lines a label should keep off, with how much sitting on each
/// costs: cut lines most, then the rest.
fn obstacles(items: &[PlanItem], mm: f64) -> Vec<(DVec2, DVec2, f64, usize)> {
    let mut out = Vec::new();
    for (k, it) in items.iter().enumerate() {
        match it {
            PlanItem::Line { pen, a, b, .. } => {
                let w = match pen {
                    Pen::Guide | Pen::Underlay | Pen::Cutter => continue,
                    Pen::Cut => 3.0,
                    Pen::Grid => 2.0,
                    _ => 1.0,
                };
                out.push((*a, *b, w, k));
            }
            PlanItem::Polygon { pen, points, .. } => {
                if *pen == Pen::Guide || *pen == Pen::Underlay {
                    continue;
                }
                let w = if *pen == Pen::Cut { 3.0 } else { 1.0 };
                let n = points.len();
                for i in 0..n {
                    out.push((points[i], points[(i + 1) % n], w, k));
                }
            }
            PlanItem::Arc {
                pen,
                center,
                radius,
                start,
                end,
                ..
            } if *pen != Pen::Underlay => {
                let p = |i: usize| {
                    *center + DVec2::from_angle(start + (end - start) * i as f64 / 8.0) * *radius
                };
                for i in 0..8 {
                    out.push((p(i), p(i + 1), 1.0, k));
                }
            }
            PlanItem::Dimension {
                a,
                b,
                offset,
                look,
                witness,
                ..
            } => {
                for (p, q) in dimension_geometry(*a, *b, *offset, *witness, look, mm).lines {
                    out.push((p, q, 1.0, k));
                }
            }
            PlanItem::Tag {
                anchor,
                at,
                elbow,
                text,
                round,
                look,
                ..
            } => {
                let leader = tag_geometry(*anchor, *at, *elbow, text, *round, look, mm).leader;
                for w in leader.windows(2) {
                    out.push((w[0], w[1], 1.0, k));
                }
            }
            PlanItem::Annotation { item, .. } => {
                for l in item.marks(mm).lines {
                    for w in l.windows(2) {
                        out.push((w[0], w[1], 1.0, k));
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// How far a label may move to find a clear spot, paper mm.
const REACH: f64 = 14.0;
/// A label moved further than this, paper mm, gets a leader back.
pub(crate) const LEADER_PAST: f64 = 9.0;

/// Move labels, notes and tags off lines and each other. `mm` is drawing
/// units per paper mm. Grid and section bubbles and dimension text stay
/// where they are and are kept clear of.
pub fn place_labels(items: &mut Vec<PlanItem>, mm: f64) {
    place_labels_with(
        items,
        mm,
        &std::collections::HashSet::new(),
        &std::collections::HashSet::new(),
    );
}

/// The box a plain text covers, in drawing units.
fn plain_text_box(item: &PlanItem) -> Option<TextBox> {
    let PlanItem::Text {
        at,
        text,
        size,
        align,
        mm,
        ..
    } = item
    else {
        return None;
    };
    let w = crate::paper::text_width(text, *size) * mm;
    let h = size * 0.6 * mm;
    let x0 = match align {
        crate::paper::Anchor::Start => 0.0,
        crate::paper::Anchor::Middle => -w / 2.0,
        crate::paper::Anchor::End => -w,
    };
    Some(TextBox {
        lo: DVec2::new(at.x + x0, at.y - h),
        hi: DVec2::new(at.x + x0 + w, at.y + h),
    })
}

/// [`place_labels`], with the plain texts of the elements in `blocks`
/// (a chamber's name and levels, a tank's) moved too, each stack of lines
/// as one block. Other plain texts stay and are kept clear of.
pub fn place_labels_with(
    items: &mut Vec<PlanItem>,
    mm: f64,
    blocks: &std::collections::HashSet<creabim_core::ElementId>,
    lead_out: &std::collections::HashSet<creabim_core::ElementId>,
) {
    place_labels_pinned(
        items,
        mm,
        blocks,
        lead_out,
        &std::collections::HashSet::new(),
    );
}

/// [`place_labels_with`], with the items of `pinned` (tags put by hand)
/// left where they are and kept clear of, as fixed text is.
pub fn place_labels_pinned(
    items: &mut Vec<PlanItem>,
    mm: f64,
    blocks: &std::collections::HashSet<creabim_core::ElementId>,
    lead_out: &std::collections::HashSet<creabim_core::ElementId>,
    pinned: &std::collections::HashSet<creabim_core::ElementId>,
) {
    place_labels_in_rooms(items, mm, blocks, lead_out, pinned, &Default::default());
}

/// [`place_labels_pinned`], with each room tag's room (`rooms`, by tag):
/// a tag nudged but still wholly inside its room gets no leader back. Every
/// tag nudged off another label drew one, so a plan opened with leaders
/// on tags that sat in their rooms (MU142, owner: "they still start with
/// leader").
pub fn place_labels_in_rooms(
    items: &mut Vec<PlanItem>,
    mm: f64,
    blocks: &std::collections::HashSet<creabim_core::ElementId>,
    lead_out: &std::collections::HashSet<creabim_core::ElementId>,
    pinned: &std::collections::HashSet<creabim_core::ElementId>,
    rooms: &std::collections::HashMap<creabim_core::ElementId, Vec<DVec2>>,
) {
    let segs = obstacles(items, mm);
    // the building's extent (its cut lines), for what goes outside it
    let extent = items
        .iter()
        .filter_map(|it| match it {
            PlanItem::Line {
                pen: Pen::Cut,
                a,
                b,
                ..
            } => Some([*a, *b]),
            _ => None,
        })
        .flatten()
        .fold(None::<(DVec2, DVec2)>, |acc, p| match acc {
            None => Some((p, p)),
            Some((a, b)) => Some((a.min(p), b.max(p))),
        });
    // what is placed so far: fixed text first
    let mut taken: Vec<(TextBox, usize)> = Vec::new();
    let mut movable: Vec<(usize, TextBox)> = Vec::new();
    // stacks of plain lines that move together
    let mut stacks: Vec<(Vec<usize>, TextBox)> = Vec::new();
    for (k, it) in items.iter().enumerate() {
        if let Some(b) = plain_text_box(it) {
            if !blocks.contains(&it.id()) {
                taken.push((b, k));
                continue;
            }
            // the line under the last of this element's stack joins it
            let joins = stacks.last().is_some_and(|(ks, g)| {
                let last = ks[ks.len() - 1];
                last + 1 == k
                    && items[last].id() == it.id()
                    && (b.lo.x - g.lo.x).abs() < 1e-6
                    && b.hi.y >= g.lo.y - (b.hi.y - b.lo.y)
            });
            if joins {
                let (ks, g) = stacks.last_mut().unwrap();
                ks.push(k);
                *g = TextBox {
                    lo: g.lo.min(b.lo),
                    hi: g.hi.max(b.hi),
                };
            } else {
                stacks.push((vec![k], b));
            }
            continue;
        }
        match text_box(it, mm) {
            Some((b, false)) => taken.push((b, k)),
            Some((b, true)) if pinned.contains(&it.id()) => taken.push((b, k)),
            Some((b, true)) => movable.push((k, b)),
            None => {}
        }
    }
    // a tag written as several lines (a room's name, area, number) moves
    // as one block: line by line, one line was nudged off its column
    // (MU31: KITCHEN stood right of its area)
    let mut groups: Vec<(Vec<usize>, TextBox)> = Vec::new();
    for (k, b) in movable {
        let block = matches!(items[k], PlanItem::Annotation { .. });
        let same = groups.iter_mut().find(|(ks, _)| {
            block
                && matches!(items[ks[0]], PlanItem::Annotation { .. })
                && items[ks[0]].id() == items[k].id()
        });
        match same {
            Some((ks, g)) => {
                ks.push(k);
                *g = TextBox {
                    lo: g.lo.min(b.lo),
                    hi: g.hi.max(b.hi),
                };
            }
            None => groups.push((vec![k], b)),
        }
    }
    let mut movable = groups;
    movable.extend(stacks);
    // the bigger texts first (room names, notes), tags after
    movable.sort_by(|a, b| {
        let area = |t: &TextBox| (t.hi - t.lo).x * (t.hi - t.lo).y;
        area(&b.1).total_cmp(&area(&a.1))
    });
    let pad = 0.4 * mm;
    let mut leaders: Vec<PlanItem> = Vec::new();
    for (ks, b) in movable {
        let k = ks[0];
        let own = items[k].id();
        // only what is near matters
        let reach = REACH * mm + (b.hi - b.lo).length();
        let near = TextBox {
            lo: b.lo - DVec2::splat(reach),
            hi: b.hi + DVec2::splat(reach),
        };
        let local: Vec<(DVec2, DVec2, f64)> = segs
            .iter()
            .filter(|(p, q, w, i)| {
                !ks.contains(i)
                    && !(own == items[*i].id() && is_own_mark(&items[k]))
                    // a path's own joints and pattern under its name
                    && !(own == items[*i].id() && blocks.contains(&own) && *w <= 1.0)
                    && near.crossed_by(*p, *q)
            })
            .map(|(p, q, w, _)| (*p, *q, *w))
            .collect();
        let cost = |c: &TextBox| -> f64 {
            let c2 = c.shrunk(-pad);
            let lines: f64 = local
                .iter()
                .filter(|(p, q, _)| c2.crossed_by(*p, *q))
                .map(|s| s.2)
                .sum();
            let texts = taken.iter().filter(|(t, _)| t.overlaps(&c2)).count() as f64;
            lines + texts * 10.0
        };
        let here = cost(&b);
        if here == 0.0 {
            taken.push((b, k));
            continue;
        }
        let h = (b.hi.y - b.lo.y).max(1.0 * mm);
        let step = h * 0.5;
        let mut best = (here, DVec2::ZERO);
        let rings = (REACH * mm / step).ceil() as i32;
        'search: for r in 1..=rings {
            let dist = r as f64 * step;
            let around = (8 * r).min(48);
            for i in 0..around {
                let d = DVec2::from_angle(i as f64 / around as f64 * std::f64::consts::TAU) * dist;
                // a slight preference for staying close
                let c = cost(&b.moved(d)) + dist / (REACH * mm) * 0.5;
                if c < best.0 - 1e-9 {
                    best = (c, d);
                    if c < 0.5 {
                        break 'search;
                    }
                }
            }
        }
        let mut d = best.1;
        // a room tag too big for its room, on another text with no
        // clear spot near: outside the plan at its nearest edge, on a
        // leader (MU131)
        if best.0 >= 10.0 && lead_out.contains(&own) {
            if let Some((lo, hi)) = extent {
                let c = b.centre();
                let h = b.hi.y - b.lo.y;
                let up = (hi.y - c.y) < (c.y - lo.y);
                let gap = 6.0 * mm;
                let x = c.x.clamp(
                    lo.x + (b.hi.x - b.lo.x) / 2.0,
                    hi.x - (b.hi.x - b.lo.x) / 2.0,
                );
                let mut y = if up {
                    hi.y + gap + h / 2.0
                } else {
                    lo.y - gap - h / 2.0
                };
                let free = |y: f64| {
                    let t = b.moved(DVec2::new(x, y) - c);
                    !taken.iter().any(|(o, _)| o.overlaps(&t.shrunk(-pad)))
                };
                let mut tries = 0;
                while !free(y) && tries < 40 {
                    y += if up { h + pad } else { -(h + pad) };
                    tries += 1;
                }
                d = DVec2::new(x, y) - c;
            }
        }
        if d == DVec2::ZERO {
            taken.push((b, k));
            continue;
        }
        let nb = b.moved(d);
        // (its middle in the room: a small room's tag may lap its walls)
        let in_room = rooms
            .get(&own)
            .is_some_and(|o| creabim_core::polygon::contains(o, nb.centre()));
        let far = d.length() > LEADER_PAST * mm && !in_room;
        // one leader for the block, from its first line
        for (n, &j) in ks.iter().enumerate() {
            shift(
                &mut items[j],
                d,
                b.centre(),
                far && n == 0,
                &mut leaders,
                nb,
            );
        }
        taken.push((nb, k));
    }
    items.extend(leaders);
}

/// Is the item a tag or note, whose own leader and outline it may sit on?
fn is_own_mark(item: &PlanItem) -> bool {
    matches!(item, PlanItem::Tag { .. } | PlanItem::Annotation { .. })
}

/// Move an item's text by `d`. Moved `far`, a note gets a leader to
/// where its text was and a plain label a line back to it; a tag keeps
/// its own leader to what it tags.
fn shift(
    item: &mut PlanItem,
    d: DVec2,
    was: DVec2,
    far: bool,
    leaders: &mut Vec<PlanItem>,
    now: TextBox,
) {
    match item {
        PlanItem::Label { id, at, .. } => {
            *at += d;
            if far {
                leaders.push(PlanItem::Line {
                    id: *id,
                    pen: Pen::Fine,
                    a: edge_toward(&now, was),
                    b: was,
                });
            }
        }
        PlanItem::Tag { at, .. } => *at += d,
        PlanItem::Text { id, at, .. } => {
            *at += d;
            if far {
                leaders.push(PlanItem::Line {
                    id: *id,
                    pen: Pen::Fine,
                    a: edge_toward(&now, was),
                    b: was,
                });
            }
        }
        PlanItem::Annotation { item, .. } => {
            if let Shape::Note { at, leader, .. } = &mut item.shape {
                *at += d;
                if far && leader.is_none() {
                    *leader = Some(was);
                }
            }
        }
        _ => {}
    }
}

/// Where a line from the box's centre toward `p` leaves the box.
fn edge_toward(b: &TextBox, p: DVec2) -> DVec2 {
    let c = b.centre();
    let v = p - c;
    let half = (b.hi - b.lo) / 2.0;
    let k = [
        if v.x.abs() > 1e-12 {
            half.x / v.x.abs()
        } else {
            f64::MAX
        },
        if v.y.abs() > 1e-12 {
            half.y / v.y.abs()
        } else {
            f64::MAX
        },
    ]
    .into_iter()
    .fold(f64::MAX, f64::min)
    .min(1.0);
    c + v * k
}

/// What a label, tag, note or dimension reads, for reports.
pub fn label_text(item: &PlanItem) -> String {
    match item {
        PlanItem::Label { text, .. }
        | PlanItem::Tag { text, .. }
        | PlanItem::Dimension { text, .. } => text.clone(),
        PlanItem::Annotation { item, .. } => item.text.replace('\n', " "),
        _ => String::new(),
    }
}

/// Pairs of label boxes that overlap: none after [`place_labels`], where
/// there was room.
pub fn overlapping_labels(items: &[PlanItem], mm: f64) -> Vec<(usize, usize)> {
    let boxes: Vec<(usize, TextBox)> = items
        .iter()
        .enumerate()
        .filter_map(|(k, it)| text_box(it, mm).map(|(b, _)| (k, b.shrunk(0.2 * mm))))
        .collect();
    let mut out = Vec::new();
    for i in 0..boxes.len() {
        for j in i + 1..boxes.len() {
            if boxes[i].1.overlaps(&boxes[j].1) {
                out.push((boxes[i].0, boxes[j].0));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use creabim_core::{ElementId, ElementKind, Model};

    #[test]
    fn a_label_on_a_line_moves_off_it() {
        let id = ElementId::NIL;
        let mut items = vec![
            PlanItem::Line {
                id,
                pen: Pen::Grid,
                a: DVec2::new(-10.0, 0.0),
                b: DVec2::new(10.0, 0.0),
            },
            PlanItem::Label {
                id,
                at: DVec2::ZERO,
                text: "LOGGIA".into(),
            },
        ];
        let mm = 0.05;
        place_labels(&mut items, mm);
        let (b, _) = text_box(&items[1], mm).unwrap();
        assert!(!b.crossed_by(DVec2::new(-10.0, 0.0), DVec2::new(10.0, 0.0)));
        // close by, so no leader
        assert_eq!(items.len(), 2);
    }

    fn plans_on_sheets(m: &Model) -> Vec<ElementId> {
        let mut out = vec![];
        for e in m.iter() {
            if let ElementKind::Viewport(vp) = &e.kind {
                if matches!(
                    m.get(vp.view).map(|v| &v.kind),
                    Some(ElementKind::View(v)) if matches!(v.kind, creabim_core::ViewKind::Plan { .. })
                ) && !out.contains(&vp.view)
                {
                    out.push(vp.view);
                }
            }
        }
        out
    }

    #[test]
    fn no_two_labels_overlap_on_the_starter_house_sheets() {
        let m = creabim_core::starter::starter_house();
        let plans = plans_on_sheets(&m);
        // four floor plans and the two ceiling plans of A105 (#70)
        assert_eq!(plans.len(), 6);
        for view in plans {
            let ElementKind::View(v) = &m.get(view).unwrap().kind else {
                unreachable!()
            };
            let mm = v.scale / 1000.0;
            let items = crate::section::view_drawing(&m, view).unwrap().items;
            let clash = overlapping_labels(&items, mm);
            let name = &m.get(view).unwrap().name;
            assert!(
                clash.is_empty(),
                "{name}: {:?}",
                clash
                    .iter()
                    .map(|(a, b)| (format!("{:?}", items[*a]), format!("{:?}", items[*b])))
                    .collect::<Vec<_>>()
            );
            // room names and notes keep off grid and section lines
            for it in &items {
                let PlanItem::Annotation { item, .. } = it else {
                    continue;
                };
                if !matches!(item.shape, Shape::Note { .. }) {
                    continue;
                }
                let (b, _) = text_box(it, mm).unwrap();
                for other in &items {
                    if let PlanItem::Line {
                        id,
                        pen: Pen::Grid | Pen::Symbol,
                        a,
                        b: e,
                    } = other
                    {
                        let line = matches!(
                            m.get(*id).map(|x| &x.kind),
                            Some(ElementKind::Grid(_) | ElementKind::View(_))
                        );
                        assert!(
                            !line || !b.shrunk(0.3 * mm).crossed_by(*a, *e),
                            "{name}: {:?} sits on a grid or section line",
                            item.text
                        );
                    }
                }
            }
        }
    }
}
