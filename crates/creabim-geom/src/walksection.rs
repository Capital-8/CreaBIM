//! A stair's walk-line section: the stair unrolled along its walk line, as
//! a drain's long section unrolls the drain. Every flight, landing and
//! winder shows at its true going and rise, with the pitch line through
//! the nosings, the floors under and over it cut, and the headroom line
//! the rule set asks for, the clash washed red where the floor comes too
//! low.

use crate::plan::{Pen, PlanItem};
use creabim_core::members::{level_above, Stair, StairBuild};
use creabim_core::stairwell;
use creabim_core::{ElementId, ElementKind, Model};
use glam::{DVec2, DVec3};

/// How often the floor over the stair is sampled, metres.
const STEP: f64 = 0.025;
/// How far the drawing runs on past each end of the stair, metres.
const REACH: f64 = 0.8;
/// Side samples for the headroom are this far in from each edge.
const EDGE: f64 = 0.05;
/// The clash between the headroom and the floor over it.
const CLASH: [f32; 3] = [0.88, 0.22, 0.2];
const RED: [f32; 3] = [0.8, 0.12, 0.08];
const MUTED: [f32; 3] = [0.45, 0.48, 0.46];
/// Treads of stringer, spine and cantilever stairs, cut.
const TREAD: f64 = 0.04;

/// A point along the walk line and two heights there: (along, low, high).
type Sample = (f64, f64, f64);

fn line(out: &mut Vec<PlanItem>, id: ElementId, pen: Pen, a: DVec2, b: DVec2) {
    out.push(PlanItem::Line { id, pen, a, b });
}

/// `pts` without the points that lie on a straight run, so a dashed line
/// over many short samples (round a spiral) dashes evenly.
fn straightened(pts: &[DVec2]) -> Vec<DVec2> {
    let mut out: Vec<DVec2> = vec![];
    for p in pts {
        if out.len() >= 2 {
            let (a, b) = (out[out.len() - 2], out[out.len() - 1]);
            let (d0, d1) = ((b - a).normalize_or_zero(), (*p - b).normalize_or_zero());
            if d0.dot(d1) > 1.0 - 1e-9 {
                out.pop();
            }
        }
        out.push(*p);
    }
    out
}

fn polyline(out: &mut Vec<PlanItem>, id: ElementId, pen: Pen, pts: &[DVec2]) {
    for w in pts.windows(2) {
        line(out, id, pen, w[0], w[1]);
    }
}

/// The unrolled walk line: the plan point and pitch height `u` along it,
/// carried on straight past either end.
struct Walk {
    pts: Vec<DVec3>,
    along: Vec<f64>,
}

impl Walk {
    fn total(&self) -> f64 {
        *self.along.last().unwrap_or(&0.0)
    }

    fn at(&self, u: f64) -> (DVec3, DVec2) {
        let total = self.total();
        if u < 0.0 || u > total {
            let (end, dir) = stairwell::at(&self.pts, &self.along, u.clamp(0.0, total));
            let past = if u < 0.0 { u } else { u - total };
            return (end + (dir * past).extend(0.0), dir);
        }
        stairwell::at(&self.pts, &self.along, u)
    }

    /// Where along it the pitch line first reaches height `z`.
    fn reaches(&self, z: f64) -> f64 {
        if self.pts.first().is_some_and(|p| p.z >= z - 1e-6) {
            return 0.0;
        }
        for i in 0..self.pts.len().saturating_sub(1) {
            let (a, b) = (self.pts[i].z, self.pts[i + 1].z);
            if a < z - 1e-6 && b >= z - 1e-6 {
                let f = ((z - a) / (b - a).max(1e-12)).clamp(0.0, 1.0);
                return self.along[i] + (self.along[i + 1] - self.along[i]) * f;
            }
        }
        self.total()
    }
}

/// The walk-line section of stair `id` at 1:`scale`.
pub fn walk_section(model: &Model, id: ElementId, scale: f64) -> crate::section::Drawing {
    let mut items = vec![];
    let Some(e) = model.get(id) else {
        return crate::section::Drawing { items };
    };
    let ElementKind::Stair(s) = &e.kind else {
        return crate::section::Drawing { items };
    };
    let (pts, along) = stairwell::pitch_line(model, s);
    if pts.len() < 2 {
        return crate::section::Drawing { items };
    }
    let walk = Walk { pts, along };
    draw(model, id, s, &walk, scale, &mut items);
    crate::section::Drawing { items }
}

fn draw(
    model: &Model,
    id: ElementId,
    s: &Stair,
    walk: &Walk,
    scale: f64,
    items: &mut Vec<PlanItem>,
) {
    let mm = scale / 1000.0;
    let units = model.units();
    let text = |at: DVec2, t: String, align, bold: bool, rgb: Option<[f32; 3]>| PlanItem::Text {
        angle: 0.0,
        id,
        at,
        text: t,
        size: 2.5,
        align,
        mm,
        rgb,
        bold,
    };
    let rise = s.rise(model);
    let base = s.base(model);
    let top = base + rise;
    let layout = s.layout(rise);
    let n = layout.risers.max(1) as usize;
    let r = layout.riser;
    let total = walk.total();
    let v = DVec2::new;

    // each nosing, along the walk line: where the pitch line reaches it
    let nosing: Vec<f64> = (1..=n).map(|j| walk.reaches(base + j as f64 * r)).collect();
    let first = nosing[0];
    let last = *nosing.last().unwrap();

    let going = s.tread.max(0.05);
    // a tread at a landing's height is that landing
    let landing = |j: usize| {
        j + 1 < n
            && layout
                .landings
                .iter()
                .any(|l| (l.1 - (j + 1) as f64 * r).abs() < 1e-6)
    };
    // the steps: up each riser and along each tread; a ramp's slope runs
    // straight through its nosings, level only across its landings
    let ramp = s.is_ramp();
    let mut steps = vec![v(first, base)];
    for j in 0..n {
        let z = base + (j + 1) as f64 * r;
        steps.push(v(nosing[j], z));
        if j + 1 < n && (!ramp || landing(j)) {
            steps.push(v(nosing[j + 1], z));
        }
    }
    if ramp {
        steps = straightened(&steps);
    }
    let hatch = crate::plan::hatch_of(model, id);
    let colour = creabim_core::material_of(model, model.get(id).unwrap()).map(|(_, m)| m.color);
    // the floor the stair arrives on, if one is drawn: its slab and underside
    let reach_end = walk.at(total + 0.3).0.truncate();
    let arrive = model.iter().find_map(|f| match &f.kind {
        ElementKind::Slab(sl) if creabim_core::polygon::contains(&sl.outline(), reach_end) => {
            let t = sl.top(model.level(sl.level).map_or(0.0, |l| l.elevation));
            ((t - top).abs() < 0.05).then_some((f.id, t - sl.thickness))
        }
        _ => None,
    });
    match s.build {
        StairBuild::Concrete | StairBuild::Stepped | StairBuild::FoldedPlate => {
            let under_floor = arrive.map(|a| a.1);
            let soffit = concrete_soffit(&nosing, base, r, going, s.waist, &landing, under_floor);
            // filled in upright strips, each one convex: from the steps (or
            // the floor reached, past the top riser) down to the soffit
            let end = soffit.last().map_or(last, |p| p.x);
            let mut xs: Vec<f64> = nosing.clone();
            xs.extend(soffit.iter().map(|p| p.x));
            xs.push(first);
            xs.retain(|x| *x >= first - 1e-9 && *x <= end + 1e-9);
            xs.sort_by(|a, b| a.total_cmp(b));
            xs.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
            let upper = |x: f64| {
                if x >= last - 1e-9 {
                    return under_floor.unwrap_or(top);
                }
                if ramp {
                    return along_line(&steps, x);
                }
                let k = nosing.iter().rposition(|u| *u <= x + 1e-9).unwrap_or(0);
                base + (k + 1) as f64 * r
            };
            let lower = |x: f64| along_line(&soffit, x);
            for w in xs.windows(2) {
                let (x0, x1) = (w[0], w[1]);
                if x1 - x0 < 1e-9 {
                    continue;
                }
                let up = upper((x0 + x1) / 2.0);
                let (b0, b1) = (lower(x0), lower(x1));
                if b0 >= up - 1e-9 && b1 >= up - 1e-9 {
                    continue;
                }
                items.push(PlanItem::Fill {
                    id,
                    points: vec![v(x0, up), v(x1, up), v(x1, b1.min(up)), v(x0, b0.min(up))],
                    hatch,
                    rgb: colour,
                    material: None,
                });
            }
            polyline(items, id, Pen::Cut, &steps);
            let mut under: Vec<DVec2> = vec![v(first, base)];
            under.extend(soffit.iter().copied());
            polyline(items, id, Pen::Cut, &under);
            if under_floor.is_none() {
                line(items, id, Pen::Cut, v(last, top), *soffit.last().unwrap());
            }
        }
        _ => {
            // each tread cut, and what holds them seen beyond
            for j in 0..n {
                let z = base + (j + 1) as f64 * r;
                let x1 = if j + 1 < n {
                    nosing[j + 1]
                } else {
                    last + going
                };
                let x0 = nosing[j];
                if j + 1 == n {
                    // the top tread is the floor above
                    break;
                }
                let block = vec![v(x0, z), v(x1, z), v(x1, z - TREAD), v(x0, z - TREAD)];
                items.push(PlanItem::Fill {
                    id,
                    points: block.clone(),
                    hatch,
                    rgb: colour,
                    material: None,
                });
                polyline(
                    items,
                    id,
                    Pen::Cut,
                    &[block[0], block[1], block[2], block[3], block[0]],
                );
            }
            // a stringer or spine seen beyond, its edges parallel to the
            // pitch; cantilevered and spiral treads have nothing under them
            if matches!(
                s.build,
                StairBuild::Stringers | StairBuild::CutStringers | StairBuild::Spine
            ) {
                let depth = 0.25;
                line(
                    items,
                    id,
                    Pen::Projection,
                    v(first, base),
                    v(last, top - r - depth),
                );
                line(
                    items,
                    id,
                    Pen::Projection,
                    v(first, base + depth),
                    v(last, top),
                );
            }
        }
    }

    // the pitch line through the nosings
    let pitch: Vec<DVec2> = straightened(
        &walk
            .pts
            .iter()
            .zip(&walk.along)
            .map(|(p, u)| v(*u, p.z))
            .collect::<Vec<_>>(),
    );
    // drawn through the nosings of each flight, not over the landings
    let mut run: Vec<DVec2> = vec![];
    for (k, u) in nosing.iter().enumerate() {
        run.push(v(*u, base + (k + 1) as f64 * r));
        if landing(k) || k + 1 == n {
            polyline(items, id, Pen::Fine, &straightened(&run));
            run.clear();
        }
    }

    // the floor the stair starts from, and the one it reaches
    let left = -REACH;
    let right = total + REACH;
    line(items, id, Pen::Cut, v(left, base), v(first, base));
    match arrive {
        Some((fid, under)) => {
            let band = vec![v(last, top), v(right, top), v(right, under), v(last, under)];
            items.push(PlanItem::Fill {
                id: fid,
                points: band.clone(),
                hatch: crate::plan::hatch_of(model, fid),
                rgb: None,
                material: None,
            });
            polyline(items, fid, Pen::Cut, &[band[1], band[0], band[3], band[2]]);
        }
        None => line(items, id, Pen::Cut, v(last, top), v(right, top)),
    }

    // what is over the stair, cut: the floor above, beams, a roof
    let need = stairwell::need(model, s);
    let half = (s.width / 2.0 - EDGE).max(0.0);
    let count = ((right - left) / STEP).ceil() as usize;
    let mut runs: Vec<(ElementId, Vec<Sample>)> = vec![];
    let mut clash: Vec<Vec<Sample>> = vec![];
    let mut open = false;
    for k in 0..=count {
        let u = (left + k as f64 * STEP).min(right);
        let (p, dir) = walk.at(u);
        // the pitch height here, the floor's off the stair
        let z = if u < 0.0 {
            base
        } else if u > total {
            top
        } else {
            p.z
        };
        let o = stairwell::over(model, p.truncate(), z);
        // past the top the floor reached is underfoot, not overhead
        let o = o.filter(|o| u <= last || o.under > top + 0.5);
        match (o, runs.last_mut()) {
            (Some(o), Some((rid, run))) if open && *rid == o.id => run.push((u, o.under, o.top)),
            (Some(o), _) => runs.push((o.id, vec![(u, o.under, o.top)])),
            _ => {}
        }
        open = o.is_some();
        // the headroom, across the stair's width as the check measures it
        if u >= 0.0 && u <= total - 0.01 {
            let side = dir.perp() * half;
            let under = [p.truncate(), p.truncate() + side, p.truncate() - side]
                .into_iter()
                .map(|q| stairwell::ceiling_over(model, q, z))
                .fold(f64::INFINITY, f64::min);
            let clear = under - z;
            if clear < need - 5e-4 {
                match clash.last_mut() {
                    Some(c) if c.last().is_some_and(|q| u - q.0 < STEP * 1.5) => {
                        c.push((u, under, z + need))
                    }
                    _ => clash.push(vec![(u, under, z + need)]),
                }
            }
        }
    }
    for (rid, run) in &runs {
        if run.len() < 2 {
            continue;
        }
        let mut band: Vec<DVec2> = run.iter().map(|q| v(q.0, q.2)).collect();
        band.extend(run.iter().rev().map(|q| v(q.0, q.1)));
        items.push(PlanItem::Fill {
            id: *rid,
            points: band,
            hatch: crate::plan::hatch_of(model, *rid),
            rgb: None,
            material: None,
        });
        let under: Vec<DVec2> = run.iter().map(|q| v(q.0, q.1)).collect();
        let over: Vec<DVec2> = run.iter().map(|q| v(q.0, q.2)).collect();
        polyline(items, *rid, Pen::Cut, &under);
        polyline(items, *rid, Pen::Cut, &over);
        let (a, b) = (run[0], run[run.len() - 1]);
        // its cut ends, where it stops over a well (not at the drawing's edge)
        if a.0 > left + STEP {
            line(items, *rid, Pen::Cut, v(a.0, a.1), v(a.0, a.2));
        }
        if b.0 < right - STEP {
            line(items, *rid, Pen::Cut, v(b.0, b.1), v(b.0, b.2));
        }
    }

    // the headroom line over the pitch line, the clash washed red
    let head: Vec<DVec2> = pitch.iter().map(|p| *p + DVec2::Y * need).collect();
    polyline(items, id, Pen::Hidden, &head);
    for c in &clash {
        if c.len() < 2 {
            continue;
        }
        // what of the floor over it comes inside the headroom
        let mut wash: Vec<DVec2> = c.iter().map(|q| v(q.0, q.1)).collect();
        wash.extend(c.iter().rev().map(|q| {
            let top = stairwell::over(model, walk.at(q.0).0.truncate(), q.2 - need)
                .map_or(q.2, |o| o.top);
            v(q.0, q.2.min(top))
        }));
        items.push(PlanItem::Tint {
            id,
            points: wash,
            rgb: CLASH,
        });
    }
    items.push(text(
        head[0] + v(-1.5 * mm, 1.0 * mm),
        format!("Headroom {}", units.dimension(need)),
        crate::paper::Anchor::End,
        false,
        Some(MUTED),
    ));
    // the least headroom, measured where the check finds it
    let head_check = stairwell::headroom(model, s, need);
    if head_check.least.is_finite() {
        let p = head_check.at;
        let u = walk
            .pts
            .windows(2)
            .zip(walk.along.windows(2))
            .map(|(w, a)| {
                let d = (w[1] - w[0]).truncate();
                let t =
                    ((p - w[0]).truncate().dot(d) / d.length_squared().max(1e-12)).clamp(0.0, 1.0);
                let q = w[0].truncate() + d * t;
                (q.distance(p.truncate()), a[0] + (a[1] - a[0]) * t)
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map_or(0.0, |q| q.1);
        let (z, under) = (p.z, p.z + head_check.least);
        {
            let short = under - z < need - 5e-4;
            line(items, id, Pen::Symbol, v(u, z), v(u, under));
            items.push(text(
                v(u + 1.5 * mm, (z + under) / 2.0),
                units.dimension(under - z),
                crate::paper::Anchor::Start,
                true,
                short.then_some(RED),
            ));
        }
    }

    // the turns of the walk line, where a landing or winders turn it
    for i in 1..walk.pts.len().saturating_sub(1) {
        let a = (walk.pts[i] - walk.pts[i - 1])
            .truncate()
            .normalize_or_zero();
        let b = (walk.pts[i + 1] - walk.pts[i])
            .truncate()
            .normalize_or_zero();
        if a.dot(b) < 0.996 {
            let u = walk.along[i];
            line(
                items,
                id,
                Pen::Contour,
                v(u, walk.pts[i].z),
                v(u, walk.pts[i].z + need),
            );
        }
    }

    // the levels at its foot and head
    for (lid, z) in [(Some(s.level), base), (level_above(model, s.level), top)] {
        let name = lid
            .and_then(|l| model.get(l))
            .map(|l| l.name.clone())
            .unwrap_or_default();
        let x = right + 2.0 * mm;
        line(items, id, Pen::Fine, v(right, z), v(x + 22.0 * mm, z));
        items.push(text(
            v(x, z + 1.2 * mm),
            name,
            crate::paper::Anchor::Start,
            true,
            None,
        ));
        items.push(text(
            v(x, z - 4.0 * mm),
            units.elevation(z),
            crate::paper::Anchor::Start,
            false,
            None,
        ));
    }

    // what it is, under it
    let goings = layout.treads();
    let pitch_deg = (r / going).atan().to_degrees();
    items.push(text(
        v(left, base - 8.0 * mm),
        if ramp {
            format!(
                "{} · rise {} · walk line {}",
                crate::members::ramp_tag(s.tread / r.max(1e-9)),
                units.dimension(rise),
                units.length(total),
            )
        } else {
            format!(
                "{n} risers of {} · {goings} goings of {} · pitch {pitch_deg:.0}° · walk line {}",
                units.dimension(r),
                units.dimension(s.tread),
                units.length(total),
            )
        },
        crate::paper::Anchor::Start,
        false,
        None,
    ));
}

/// The soffit of a concrete stair along the walk line: under each flight a
/// line parallel to the pitch the waist below the roots of its steps, under
/// each landing a level slab the waist thick, each running on until it meets
/// the next, from where it leaves the floor to where it meets the underside
/// of the floor it arrives on (or under the top riser, with none).
fn concrete_soffit(
    nosing: &[f64],
    base: f64,
    r: f64,
    going: f64,
    waist: f64,
    landing: &dyn Fn(usize) -> bool,
    under_floor: Option<f64>,
) -> Vec<DVec2> {
    let n = nosing.len();
    let drop = waist / (r / going).atan().cos();
    let z = |k: usize| base + (k + 1) as f64 * r;
    // the flights, as the soffit points under their nosings, and the landings
    enum Piece {
        Flight(Vec<DVec2>),
        Landing(f64),
    }
    let mut pieces: Vec<Piece> = vec![];
    let mut k = 0;
    while k < n {
        if landing(k) {
            pieces.push(Piece::Landing(z(k) - waist));
            k += 1;
            continue;
        }
        let mut pts = vec![];
        while k < n && !landing(k) {
            pts.push(DVec2::new(nosing[k], z(k) - r - drop));
            k += 1;
        }
        // up to the landing's edge, the top of its last riser
        if k < n {
            pts.push(DVec2::new(nosing[k], z(k) - r - drop));
        }
        pieces.push(Piece::Flight(pts));
    }
    // where a flight's line, carried on straight past either end, reaches `h`
    let reach = |pts: &[DVec2], h: f64, at_end: bool| -> f64 {
        let (a, b) = if at_end {
            (pts[pts.len().saturating_sub(2)], pts[pts.len() - 1])
        } else {
            (pts[0], pts[1.min(pts.len() - 1)])
        };
        let slope = if (b.x - a.x).abs() > 1e-9 {
            (b.y - a.y) / (b.x - a.x)
        } else {
            r / going
        };
        let from = if at_end { b } else { a };
        from.x + (h - from.y) / slope.max(1e-6)
    };
    let mut out: Vec<DVec2> = vec![];
    for (i, piece) in pieces.iter().enumerate() {
        match piece {
            Piece::Flight(pts) => {
                // its start: off the floor, or out of the landing before it
                let from = match i.checked_sub(1).map(|p| &pieces[p]) {
                    Some(Piece::Landing(h)) => {
                        let x = reach(pts, *h, false);
                        out.push(DVec2::new(x, *h));
                        x
                    }
                    _ => {
                        let x = reach(pts, base, false).max(nosing[0]);
                        out.push(DVec2::new(x, base));
                        x
                    }
                };
                // its end: into the landing after it, or the floor reached
                let to = match pieces.get(i + 1) {
                    Some(Piece::Landing(h)) => Some((reach(pts, *h, true), *h)),
                    _ => under_floor.map(|h| (reach(pts, h, true), h)),
                };
                for p in pts {
                    if p.x > from + 1e-9 && to.is_none_or(|t| p.x < t.0 - 1e-9) {
                        out.push(*p);
                    }
                }
                if let Some((x, h)) = to {
                    out.push(DVec2::new(x, h));
                }
            }
            Piece::Landing(_) => {}
        }
    }
    out.dedup_by(|a, b| a.distance(*b) < 1e-9);
    out
}

/// The height of a polyline running left to right at `x`, held level past
/// its ends.
fn along_line(pts: &[DVec2], x: f64) -> f64 {
    if pts.is_empty() {
        return 0.0;
    }
    if x <= pts[0].x {
        return pts[0].y;
    }
    for w in pts.windows(2) {
        if x <= w[1].x {
            let f = (x - w[0].x) / (w[1].x - w[0].x).max(1e-12);
            return w[0].y + (w[1].y - w[0].y) * f;
        }
    }
    pts[pts.len() - 1].y
}

#[cfg(test)]
mod tests {
    use super::*;
    use creabim_core::members::StairLine;
    use creabim_core::{Document, Element, Level, Slab, Transaction};

    fn tinted(d: &crate::section::Drawing) -> usize {
        d.items
            .iter()
            .filter(|i| matches!(i, PlanItem::Tint { rgb, .. } if *rgb == CLASH))
            .count()
    }

    #[test]
    fn a_walk_section_steps_up_the_stair_and_reds_the_floor_too_low() {
        let mut doc = Document::new();
        let level = |name: &str, z: f64| {
            Element::new(
                name,
                ElementKind::Level(Level {
                    elevation: z,
                    height: 3.0,
                }),
            )
        };
        let (l0, l1) = (level("Ground", 0.0), level("First", 3.0));
        let floor = Slab::new(
            l1.id,
            vec![
                DVec2::new(-2.0, -3.0),
                DVec2::new(10.0, -3.0),
                DVec2::new(10.0, 3.0),
                DVec2::new(-2.0, 3.0),
            ],
            0.25,
        );
        let st = Stair::from_line(
            l0.id,
            &[DVec2::new(0.0, 0.0), DVec2::new(6.0, 0.0)],
            StairLine::Walk,
        )
        .unwrap();
        let stair = Element::new("Stair", ElementKind::Stair(st));
        let sid = stair.id;
        doc.commit(
            Transaction::new("t")
                .insert(l0)
                .insert(l1)
                .insert(Element::new("Floor", ElementKind::Slab(floor)))
                .insert(stair),
        )
        .unwrap();
        let d = walk_section(doc.model(), sid, 50.0);
        // its risers, as cut lines going straight up
        let up = d
            .items
            .iter()
            .filter(|i| {
                matches!(i, PlanItem::Line { pen: Pen::Cut, a, b, .. }
                    if (a.x - b.x).abs() < 1e-9 && (b.y - a.y - 3.0 / 17.0).abs() < 1e-6)
            })
            .count();
        assert_eq!(up, 17, "17 risers of 176");
        // the floor over the whole stair: its head hits it
        assert!(tinted(&d) > 0);
        let text: Vec<String> = d
            .items
            .iter()
            .filter_map(|i| match i {
                PlanItem::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect();
        assert!(
            text.iter().any(|t| t.starts_with("17 risers of 176")),
            "{text:?}"
        );
        assert!(text.iter().any(|t| t == "First"));
        // with a well cut, the head clears
        let tx = stairwell::cut_well(doc.model(), sid, 0.0).unwrap();
        doc.commit(tx).unwrap();
        let d = walk_section(doc.model(), sid, 50.0);
        assert_eq!(tinted(&d), 0);
    }

    #[test]
    fn a_concrete_soffit_runs_parallel_to_the_pitch_and_meets_the_landing() {
        // two flights of three risers either side of a 1 m landing
        let (r, g, w) = (0.175, 0.28, 0.15);
        let mut nosing = vec![0.0, g, 2.0 * g];
        nosing.extend([2.0 * g + 1.0, 3.0 * g + 1.0, 4.0 * g + 1.0]);
        let landing = |k: usize| k == 2;
        let soffit = concrete_soffit(&nosing, 0.0, r, g, w, &landing, Some(6.0 * r - 0.25));
        // left to right, never turning back
        assert!(
            soffit.windows(2).all(|p| p[1].x > p[0].x - 1e-9),
            "{soffit:?}"
        );
        // the landing's underside, level, the waist under it
        let flat = 3.0 * r - w;
        let level: Vec<&DVec2> = soffit
            .iter()
            .filter(|p| (p.y - flat).abs() < 1e-9)
            .collect();
        assert_eq!(level.len(), 2, "{soffit:?}");
        // the flight below meets it under the landing, not at its edge;
        // the one above, a waist deeper than the landing, just past it
        assert!(level[0].x > nosing[2] + 0.1, "{level:?}");
        assert!((level[1].x - nosing[3]).abs() < 0.1, "{level:?}");
        // and nothing dips below it in between
        assert!(soffit
            .iter()
            .filter(|p| p.x > level[0].x && p.x < level[1].x)
            .all(|p| p.y >= flat - 1e-9));
        // the top flight runs on to the underside of the floor it reaches
        let end = soffit.last().unwrap();
        assert!((end.y - (6.0 * r - 0.25)).abs() < 1e-9 && end.x > nosing[5]);
    }
}
