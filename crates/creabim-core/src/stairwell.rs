//! Headroom over a stair, and the well cut in the floor above it.
//!
//! Headroom is the clear height measured straight up from the pitch line
//! (the line through the nosings, level across landings) to the nearest
//! slab, beam or roof above. It is sampled along the walk line and near
//! both sides. A well is a shaft tied to its stair: its outline is the
//! stair's footprint from the first point where the floor above would come
//! lower than the headroom, plus any clearance, to the top. It follows the
//! stair as the stair changes and goes when the stair is deleted.

use crate::members::{self, Stair};
use crate::ops::Op;
use crate::slab::Shaft;
use crate::{Element, ElementId, ElementKind, Model, Transaction};
use glam::{DVec2, DVec3};

pub fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

/// How often headroom is sampled along the walk line, metres.
const STEP: f64 = 0.05;
/// Side samples are this far in from each edge.
const EDGE: f64 = 0.05;

/// The pitch line of a stair, absolute heights: plan points with the
/// height of the nosings over them, and the plan distance along it to each.
pub fn pitch_line(model: &Model, s: &Stair) -> (Vec<DVec3>, Vec<f64>) {
    let rise = s.rise(model);
    let base = s.base(model);
    let walk: Vec<DVec3> = s
        .layout(rise)
        .walk
        .into_iter()
        .map(|p| DVec3::new(p.x, p.y, p.z + base))
        .collect();
    let mut along = vec![0.0];
    for w in walk.windows(2) {
        let d = w[0].truncate().distance(w[1].truncate());
        along.push(along.last().unwrap() + d);
    }
    (walk, along)
}

/// The point of the pitch line `u` along it (plan distance), and which way
/// it runs there.
pub fn at(walk: &[DVec3], along: &[f64], u: f64) -> (DVec3, DVec2) {
    for i in 0..walk.len().saturating_sub(1) {
        if u <= along[i + 1] + 1e-12 || i + 2 == walk.len() {
            let len = (along[i + 1] - along[i]).max(1e-12);
            let t = ((u - along[i]) / len).clamp(0.0, 1.0);
            let dir = (walk[i + 1] - walk[i]).truncate().normalize_or_zero();
            return (walk[i].lerp(walk[i + 1], t), dir);
        }
    }
    (walk.first().copied().unwrap_or_default(), DVec2::X)
}

/// The lowest underside over plan point `p` higher than `z` (absolute):
/// slabs (outside their holes and shafts), beams and roofs. Infinite when
/// nothing is over it.
pub fn ceiling_over(model: &Model, p: DVec2, z: f64) -> f64 {
    over(model, p, z).map_or(f64::INFINITY, |o| o.under)
}

/// What is over a point: its underside and top (absolute) and which
/// element it is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Over {
    pub under: f64,
    pub top: f64,
    pub id: ElementId,
}

/// The lowest slab, beam or roof over plan point `p` whose underside is
/// higher than `z` (absolute), skipping slabs and roofs where `p` is in
/// one of their holes or shafts.
pub fn over(model: &Model, p: DVec2, z: f64) -> Option<Over> {
    let mut low: Option<Over> = None;
    let mut take = |under: f64, top: f64, id: ElementId| {
        if under > z + 0.02 && low.is_none_or(|o| under < o.under) {
            low = Some(Over { under, top, id });
        }
    };
    for e in model.iter() {
        match &e.kind {
            ElementKind::Slab(s) => {
                if !crate::polygon::contains(&s.outline(), p) {
                    continue;
                }
                if crate::slab::holes_of(model, e)
                    .iter()
                    .any(|h| crate::polygon::contains(h, p))
                {
                    continue;
                }
                let top = s.top(model.level(s.level).map_or(0.0, |l| l.elevation));
                take(top - s.thickness, top, e.id);
            }
            ElementKind::Beam(b) => {
                if !crate::polygon::contains(&b.outline(), p) {
                    continue;
                }
                let (a, c) = b.axis(model);
                let d = (c - a).truncate();
                let t = if d.length_squared() > 1e-12 {
                    ((p - a.truncate()).dot(d) / d.length_squared()).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let top = a.z + (c.z - a.z) * t;
                take(top - b.depth, top, e.id);
            }
            ElementKind::Roof(r) => {
                if !crate::polygon::contains(&r.eave_outline(), p) {
                    continue;
                }
                if crate::slab::holes_of(model, e)
                    .iter()
                    .any(|h| crate::polygon::contains(h, p))
                {
                    continue;
                }
                let under = r.base(model) + r.underside_at(p);
                take(under, under + r.thickness, e.id);
            }
            _ => {}
        }
    }
    low
}

/// The headroom over a stair.
#[derive(Clone, Debug, PartialEq)]
pub struct Headroom {
    /// The least clear height found, metres (infinite: nothing above).
    pub least: f64,
    /// Where, on the pitch line (absolute).
    pub at: DVec3,
    /// The stretches of the walk line with less than `need` over them: the
    /// pitch line's points from where it starts to fall short to where it
    /// is clear again.
    pub low: Vec<Vec<DVec3>>,
    pub need: f64,
}

/// The headroom over stair `s`, against `need`.
pub fn headroom(model: &Model, s: &Stair, need: f64) -> Headroom {
    let (walk, along) = pitch_line(model, s);
    let total = *along.last().unwrap_or(&0.0);
    let half = (s.width / 2.0 - EDGE).max(0.0);
    let mut out = Headroom {
        least: f64::INFINITY,
        at: walk.first().copied().unwrap_or_default(),
        low: vec![],
        need,
    };
    if walk.len() < 2 {
        return out;
    }
    let n = (total / STEP).ceil().max(1.0) as usize;
    let mut run: Vec<DVec3> = vec![];
    for k in 0..=n {
        // stop just short of the top nosing: past it is the floor above
        let u = (k as f64 * STEP).min(total - 0.01).max(0.0);
        let (p, dir) = at(&walk, &along, u);
        let side = dir.perp() * half;
        let clear = [p.truncate(), p.truncate() + side, p.truncate() - side]
            .into_iter()
            .map(|q| ceiling_over(model, q, p.z) - p.z)
            .fold(f64::INFINITY, f64::min);
        if clear < out.least {
            out.least = clear;
            out.at = p;
        }
        if clear < need - 5e-4 {
            run.push(p);
        } else if !run.is_empty() {
            out.low.push(std::mem::take(&mut run));
        }
    }
    if !run.is_empty() {
        out.low.push(run);
    }
    out
}

/// The well a stair needs in the floor above: its footprint from the first
/// point where the floor's underside comes within `need` of the pitch
/// line, `clearance` further back, to the top. None when the floor above
/// is clear of the whole stair (or there is none).
pub fn well_outline(model: &Model, s: &Stair, need: f64, clearance: f64) -> Option<Vec<DVec2>> {
    let (walk, along) = pitch_line(model, s);
    let total = *along.last()?;
    if walk.len() < 2 || total < 1e-6 {
        return None;
    }
    // the floor above: the lowest slab over the top of the flight, whose
    // underside is above the bottom of the stair
    let base = s.base(model);
    let (top, _) = at(&walk, &along, (total - 0.3).max(0.0));
    let floor = model
        .iter()
        .filter_map(|e| match &e.kind {
            ElementKind::Slab(sl) if crate::polygon::contains(&sl.outline(), top.truncate()) => {
                let t = sl.top(model.level(sl.level).map_or(0.0, |l| l.elevation));
                let under = t - sl.thickness;
                (under > base + 0.5).then_some(under)
            }
            _ => None,
        })
        .fold(f64::INFINITY, f64::min);
    let under = if floor.is_finite() {
        floor
    } else {
        // no slab drawn yet: the level above, a typical floor thick
        members::level_above(model, s.level)
            .and_then(|l| model.level(l))
            .map(|l| l.elevation - 0.25)?
    };
    // the first point the floor would come too low over
    let n = (total / 0.01).ceil() as usize;
    let first = (0..=n)
        .map(|k| (k as f64 * 0.01).min(total))
        .find(|u| under - at(&walk, &along, *u).0.z < need - 5e-4)?;
    let from = (first - clearance).max(0.0);
    // the walk line from there to the top, as a strip the stair's width
    let mut line = vec![at(&walk, &along, from).0.truncate()];
    for (i, p) in walk.iter().enumerate() {
        if along[i] > from + 1e-9 {
            line.push(p.truncate());
        }
    }
    line.dedup_by(|a, b| a.distance(*b) < 1e-6);
    if line.len() < 2 {
        return None;
    }
    let half = s.width / 2.0;
    let (left, mut right) = strip_sides(&line, half);
    if left.len() < 2 || right.len() < 2 {
        return None;
    }
    if let Some(m) = s.mirror(s.rise(model)) {
        return split_well(&m, left, right);
    }
    right.reverse();
    let mut poly = left;
    poly.extend(right);
    if crate::polygon::signed_area2(&poly) < 0.0 {
        poly.reverse();
    }
    (poly.len() >= 3 && crate::polygon::signed_area2(&poly).abs() > 1e-6).then_some(poly)
}

/// A split stair's well: the strip over the drawn arm (its two sides,
/// both from the foot up) and its mirror image, as one outline. Where the
/// strip starts on the shared flight its side along the mirror line is
/// shared, and the two join there; else they are joined by a bridge of no
/// width.
/// The two sides of a strip `half` either side of `line`, each running
/// forward only. A side moved in towards the centre of a tight curve (a
/// spiral's inside, a winder's corner) folds back on itself wherever the
/// line's points lie closer together than the curve allows; those points
/// are left out, so the well's outline never turns back or crosses
/// itself (MU100, MU101: a spiral's well had 16 turn-backs, and every
/// outline drawn round it threw lines across the plan).
fn strip_sides(line: &[DVec2], half: f64) -> (Vec<DVec2>, Vec<DVec2>) {
    let n = line.len();
    let tangent =
        |i: usize| (line[(i + 1).min(n - 1)] - line[i.saturating_sub(1)]).normalize_or_zero();
    let forward = |side: Vec<DVec2>| {
        let mut out: Vec<DVec2> = vec![];
        for (i, p) in side.into_iter().enumerate() {
            match out.last() {
                Some(q) if (p - *q).dot(tangent(i)) <= 1e-4 => {}
                _ => out.push(p),
            }
        }
        out
    };
    (
        forward(crate::offset::offset_path(line, half)),
        forward(crate::offset::offset_path(line, -half)),
    )
}

fn split_well(m: &members::Mirror, left: Vec<DVec2>, right: Vec<DVec2>) -> Option<Vec<DVec2>> {
    let (ax, ot) = if m.left { (left, right) } else { (right, left) };
    if ax.len() < 2 || ot.len() < 2 {
        return None;
    }
    let on_axis = |p: &DVec2| (*p - m.origin).perp_dot(m.dir).abs() < 1e-6;
    let k = ax.iter().take_while(|p| on_axis(p)).count();
    let ccw = |mut p: Vec<DVec2>| {
        p.dedup_by(|a, b| a.distance(*b) < 1e-9);
        if p.len() > 1 && p[0].distance(p[p.len() - 1]) < 1e-9 {
            p.pop();
        }
        // no corner where the halves meet in a straight line
        let mut k = 0;
        while p.len() > 3 && k < p.len() {
            let n = p.len();
            let (a, b, c) = (p[(k + n - 1) % n], p[k], p[(k + 1) % n]);
            if (b - a).perp_dot(c - b).abs() < 1e-9 && (b - a).dot(c - b) > 0.0 {
                p.remove(k);
            } else {
                k += 1;
            }
        }
        if crate::polygon::signed_area2(&p) < 0.0 {
            p.reverse();
        }
        (p.len() >= 3 && crate::polygon::signed_area2(&p).abs() > 1e-6).then_some(p)
    };
    if k >= 2 {
        // up the outer side, back down the mirror-line side to where it
        // leaves the line, up its image, down the image's outer side
        let mut poly = ot.clone();
        poly.extend(ax[k - 1..].iter().rev().copied());
        poly.extend(ax[k - 1..].iter().map(|p| m.pt(*p)));
        poly.extend(ot.iter().rev().map(|p| m.pt(*p)));
        return ccw(poly);
    }
    let mut half = ot.clone();
    half.extend(ax.iter().rev().copied());
    let half = ccw(half)?;
    let mut image: Vec<DVec2> = half.iter().map(|p| m.pt(*p)).collect();
    image.reverse();
    // bridge from the half's vertex nearest the image
    let (i, j) = (0..half.len())
        .flat_map(|i| (0..image.len()).map(move |j| (i, j)))
        .min_by(|a, b| {
            half[a.0]
                .distance(image[a.1])
                .total_cmp(&half[b.0].distance(image[b.1]))
        })?;
    let mut poly: Vec<DVec2> = half[i..].iter().chain(&half[..=i]).copied().collect();
    poly.extend(image[j..].iter().chain(&image[..=j]).copied());
    let mut out = poly;
    out.dedup_by(|a, b| a.distance(*b) < 1e-9);
    (out.len() >= 6).then_some(out)
}

/// The headroom the stair's rule set asks for.
pub fn need(model: &Model, s: &Stair) -> f64 {
    s.rules.rules(model).headroom
}

/// A well for stair `id`: a shaft tied to it, from its level up to the
/// level above, `clearance` further down the stair than the headroom asks.
pub fn cut_well(model: &Model, id: ElementId, clearance: f64) -> Result<Transaction, String> {
    let Some(ElementKind::Stair(s)) = model.get(id).map(|e| &e.kind) else {
        return Err("that isn't a stair".into());
    };
    if model
        .iter()
        .any(|e| matches!(&e.kind, ElementKind::Shaft(sh) if sh.stair == Some(id)))
    {
        return Err("this stair has its well already".into());
    }
    let outline = well_outline(model, s, need(model, s), clearance)
        .ok_or("the floor above is clear of this stair: it needs no well")?;
    let mut sh = Shaft::new(s.level, outline, members::level_above(model, s.level));
    sh.stair = Some(id);
    sh.clearance = clearance;
    Ok(Transaction::new("Cut a well").insert(Element::new("Stair well", ElementKind::Shaft(sh))))
}

/// The wells an earlier version cut with an outline that crosses itself
/// (round a spiral, a curved flight or winders), cut again from their
/// stairs: one step, for when a project is opened. None when every well
/// is sound (MU102).
pub fn mend_wells(model: &Model) -> Option<Transaction> {
    let mut out = Transaction::new("Stair wells recut");
    for e in model.iter() {
        let ElementKind::Shaft(sh) = &e.kind else {
            continue;
        };
        if !crate::polygon::self_intersects(&sh.boundary, true) {
            continue;
        }
        let Some(ElementKind::Stair(s)) = sh.stair.and_then(|id| model.get(id)).map(|x| &x.kind)
        else {
            continue;
        };
        let Some(outline) = well_outline(model, s, need(model, s), sh.clearance) else {
            continue;
        };
        let mut after = e.clone();
        if let ElementKind::Shaft(x) = &mut after.kind {
            x.boundary = outline;
        }
        out = out.replace(e.clone(), after);
    }
    (!out.ops.is_empty()).then_some(out)
}

/// After `tx`: every well tied to a stair takes the outline its stair now
/// needs, and every crop box kept round a stair its new box. Runs when a
/// stair, slab, level, well or such a crop box changed.
pub fn settle(model: &Model, tx: &Transaction) -> Transaction {
    let mut out = Transaction::new("Stair wells");
    let touched = tx.ops.iter().any(|op| {
        let e = match op {
            Op::Insert(e) | Op::Remove(e) => e,
            Op::Replace { after, .. } => after,
        };
        matches!(
            &e.kind,
            ElementKind::Stair(_) | ElementKind::Slab(_) | ElementKind::Level(_)
        ) || matches!(&e.kind, ElementKind::Shaft(sh) if sh.stair.is_some())
            || matches!(&e.kind, ElementKind::CropBox(c) if c.fit.is_some())
            || matches!(&e.kind, ElementKind::Project(_))
    });
    if !touched {
        return out;
    }
    for e in model.iter() {
        let ElementKind::Shaft(sh) = &e.kind else {
            continue;
        };
        let Some(ElementKind::Stair(s)) = sh.stair.and_then(|id| model.get(id)).map(|x| &x.kind)
        else {
            continue;
        };
        let Some(outline) = well_outline(model, s, need(model, s), sh.clearance) else {
            continue;
        };
        let same = outline.len() == sh.boundary.len()
            && outline
                .iter()
                .zip(&sh.boundary)
                .all(|(a, b)| a.distance(*b) < 1e-6);
        if same && sh.level == s.level {
            continue;
        }
        let mut after = e.clone();
        if let ElementKind::Shaft(x) = &mut after.kind {
            x.boundary = outline;
            x.level = s.level;
        }
        out = out.replace(e.clone(), after);
    }
    // a crop box kept round a stair follows it too
    for e in model.iter() {
        let ElementKind::CropBox(c) = &e.kind else {
            continue;
        };
        let Some(ElementKind::Stair(s)) = c.fit.and_then(|id| model.get(id)).map(|x| &x.kind)
        else {
            continue;
        };
        let Some((lo, hi)) = s.cut_out(model) else {
            continue;
        };
        if lo.distance(c.min) < 1e-6 && hi.distance(c.max) < 1e-6 {
            continue;
        }
        let mut after = e.clone();
        if let ElementKind::CropBox(x) = &mut after.kind {
            x.min = lo;
            x.max = hi;
        }
        out = out.replace(e.clone(), after);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::members::StairLine;
    use crate::{Document, Level, Slab};

    /// Two levels 3 m apart, a floor slab over the first, and a straight
    /// stair up between them; the document and the stair's id.
    fn stair_under_a_floor() -> (Document, ElementId) {
        let mut doc = Document::new();
        let g = Element::new(
            "Ground",
            ElementKind::Level(Level {
                elevation: 0.0,
                height: 3.0,
            }),
        );
        let f = Element::new(
            "First",
            ElementKind::Level(Level {
                elevation: 3.0,
                height: 3.0,
            }),
        );
        let floor = Slab::new(
            f.id,
            vec![
                DVec2::new(-2.0, -3.0),
                DVec2::new(10.0, -3.0),
                DVec2::new(10.0, 3.0),
                DVec2::new(-2.0, 3.0),
            ],
            0.25,
        );
        let s = Stair::from_line(
            g.id,
            &[DVec2::new(0.0, 0.0), DVec2::new(6.0, 0.0)],
            StairLine::Walk,
        )
        .unwrap();
        let st = Element::new("Stair", ElementKind::Stair(s));
        let id = st.id;
        let tx = Transaction::new("t")
            .insert(g)
            .insert(f)
            .insert(Element::new("Floor", ElementKind::Slab(floor)))
            .insert(st);
        doc.commit(tx).unwrap();
        (doc, id)
    }

    fn stair(doc: &Document, id: ElementId) -> Stair {
        match &doc.model().get(id).unwrap().kind {
            ElementKind::Stair(s) => s.clone(),
            _ => panic!("a stair"),
        }
    }

    /// How many corners of a closed outline turn back on the edge before.
    fn turn_backs(b: &[DVec2]) -> usize {
        let n = b.len();
        (0..n)
            .filter(|&i| {
                let (a, p, c) = (b[(i + n - 1) % n], b[i], b[(i + 1) % n]);
                (p - a).normalize_or_zero().dot((c - p).normalize_or_zero()) < -0.5
            })
            .count()
    }

    #[test]
    fn a_wells_outline_never_turns_back_round_a_spiral_or_a_winder() {
        // MU100, MU101: the outline round a spiral's well folded back 16
        // times and crossed itself; a hover over it threw lines across
        // the plan
        let (mut doc, id) = stair_under_a_floor();
        let g = stair(&doc, id).level;
        let c = DVec2::new(4.0, 0.0);
        let at = |deg: f64| c + DVec2::from_angle(deg.to_radians()) * 0.65;
        let mut spiral =
            Stair::from_line(g, &[at(0.0), at(150.0), at(300.0)], StairLine::Walk).unwrap();
        spiral.width = 0.9;
        spiral.tread = 0.2;
        spiral.set_bend(0, 150.0);
        spiral.set_bend(1, 150.0);
        let mut winder = Stair::from_line(
            g,
            &[
                DVec2::new(0.0, 0.0),
                DVec2::new(2.5, 0.0),
                DVec2::new(2.5, 2.5),
            ],
            StairLine::Walk,
        )
        .unwrap();
        winder.at_turns = members::TurnKind::Winders(0);
        for (name, s) in [("spiral", spiral), ("winder", winder)] {
            let before = doc.model().get(id).unwrap().clone();
            let mut after = before.clone();
            after.kind = ElementKind::Stair(s);
            doc.commit(Transaction::new("s").replace(before, after))
                .unwrap();
            let st = stair(&doc, id);
            let b = well_outline(doc.model(), &st, need(doc.model(), &st), 0.0)
                .unwrap_or_else(|| panic!("{name}: a well"));
            assert_eq!(turn_backs(&b), 0, "{name}: {b:?}");
            assert!(!crate::polygon::self_intersects(&b, true), "{name}: {b:?}");
            let short = (0..b.len())
                .map(|i| b[i].distance(b[(i + 1) % b.len()]))
                .fold(f64::INFINITY, f64::min);
            assert!(short > 1e-4, "{name}: an edge of no length");
        }
    }

    #[test]
    fn a_well_whose_outline_crosses_itself_is_cut_again_on_opening() {
        let (mut doc, id) = stair_under_a_floor();
        doc.commit(cut_well(doc.model(), id, 0.0).unwrap()).unwrap();
        // a sound well is left alone
        assert!(mend_wells(doc.model()).is_none());
        // one as an earlier version left it: a bow-tie
        let w = doc
            .model()
            .iter()
            .find(|e| matches!(&e.kind, ElementKind::Shaft(_)))
            .unwrap()
            .clone();
        let mut bad = w.clone();
        let ElementKind::Shaft(sh) = &mut bad.kind else {
            panic!()
        };
        let good = sh.boundary.clone();
        // a spike from its second corner out across its last edge
        let (a, b2, d) = (good[0], good[1], *good.last().unwrap());
        sh.boundary.insert(2, (a + d) / 2.0 - (b2 - a) * 0.3);
        assert!(crate::polygon::self_intersects(&sh.boundary, true));
        // (put in past the document, which would settle it at once)
        let mut m = doc.model().clone();
        Transaction::new("old")
            .replace(w, bad)
            .apply(&mut m)
            .unwrap();
        let tx = mend_wells(&m).expect("a well to mend");
        tx.apply(&mut m).unwrap();
        let mended = m.iter().find_map(|e| match &e.kind {
            ElementKind::Shaft(s) => Some(s.boundary.clone()),
            _ => None,
        });
        assert_eq!(mended.unwrap(), good);
    }

    fn well(doc: &Document) -> Option<Shaft> {
        doc.model().iter().find_map(|e| match &e.kind {
            ElementKind::Shaft(s) => Some(s.clone()),
            _ => None,
        })
    }

    #[test]
    fn a_split_stair_is_its_arm_and_the_arm_mirrored() {
        let (mut doc, id) = stair_under_a_floor();
        // east up the middle, a landing, back west on the north: the
        // mirror line is the shared flight's south edge
        doc.edit(id, "split", |e| {
            if let ElementKind::Stair(s) = &mut e.kind {
                s.set_line(&[
                    DVec2::new(0.0, 0.0),
                    DVec2::new(2.75, 0.0),
                    DVec2::new(2.75, 1.5),
                    DVec2::new(0.0, 1.5),
                ]);
                s.split = true;
            }
        })
        .unwrap();
        let s = stair(&doc, id);
        let rise = s.rise(doc.model());
        let m = s.mirror(rise).unwrap();
        assert!(!m.left, "{m:?}");
        assert!((m.origin.y + 0.5).abs() < 1e-9, "{m:?}");
        assert!((m.pt(DVec2::new(1.0, 1.5)).y + 2.5).abs() < 1e-9);
        assert_eq!(s.ifc_type(rise), "DOUBLE_RETURN_STAIR");
        // the footprint holds both arms
        let (lo, hi) = s.footprint(doc.model()).unwrap();
        assert!(lo.y < -2.9 && hi.y > 1.9, "{lo} {hi}");
        // twice the treads of one arm
        let mut one = s.clone();
        one.split = false;
        assert!((s.quantities(rise).treads - 2.0 * one.quantities(rise).treads).abs() < 1e-9);
        // no rail down the middle: the right side's starts at the landing's far corner
        let rail = members::stair_rail(&s, rise, false, false, Some(Default::default()));
        let first = rail[0].truncate();
        // the landing's far edge: past the shared flight and its landing
        assert!(first.x > m.shared + 1.0 - 0.1, "{first}");
        // on the mirror line itself, where it meets its mirror image
        assert!((first.y - m.origin.y).abs() < 1e-9, "{first}");
        // and the left side's still runs from the foot
        let rail = members::stair_rail(&s, rise, true, false, None);
        assert!(rail[0].x < 0.1);
        // one well over both arms
        doc.commit(cut_well(doc.model(), id, 0.0).unwrap()).unwrap();
        let w = well(&doc).unwrap();
        for p in [
            DVec2::new(2.0, 1.5),
            DVec2::new(2.0, -2.5),
            DVec2::new(2.0, -0.5),
        ] {
            assert!(
                crate::polygon::contains(&w.boundary, p),
                "{p}: {:?}",
                w.boundary
            );
        }
        assert!(!crate::polygon::self_intersects(&w.boundary, true));
    }

    #[test]
    fn a_floor_over_a_stair_takes_its_headroom_until_a_well_is_cut() {
        let (mut doc, id) = stair_under_a_floor();
        let s = stair(&doc, id);
        // under a 250 mm floor at 3 m the head hits the slab: at the top
        // nosing the pitch line is nearly 3 m up
        let h = headroom(doc.model(), &s, 1.95);
        assert!(h.least < 0.5, "{h:?}");
        assert_eq!(h.low.len(), 1);
        doc.commit(cut_well(doc.model(), id, 0.0).unwrap()).unwrap();
        let h = headroom(doc.model(), &stair(&doc, id), 1.95);
        assert!(h.least >= 1.95 - 1e-3, "{h:?}");
        assert!(h.low.is_empty());
        // the well starts where the pitch line comes within 1950 of the
        // floor's underside (2750): at 800 up, about 4½ treads in
        let w = well(&doc).unwrap();
        let x0 = w.boundary.iter().map(|p| p.x).fold(f64::MAX, f64::min);
        assert!(x0 > 0.9 && x0 < 1.6, "{x0}");
        // and a second one isn't cut
        assert!(cut_well(doc.model(), id, 0.0).is_err());
    }

    #[test]
    fn a_well_follows_its_stair_and_goes_with_it() {
        let (mut doc, id) = stair_under_a_floor();
        doc.commit(cut_well(doc.model(), id, 0.3).unwrap()).unwrap();
        let before = well(&doc).unwrap().boundary;
        // a wider stair: a wider well
        let e = doc.model().get(id).unwrap().clone();
        let mut after = e.clone();
        if let ElementKind::Stair(s) = &mut after.kind {
            s.width = 1.4;
        }
        doc.commit(Transaction::new("wider").replace(e, after))
            .unwrap();
        let w = well(&doc).unwrap();
        let span = |b: &[DVec2]| {
            b.iter().map(|p| p.y).fold(f64::MIN, f64::max)
                - b.iter().map(|p| p.y).fold(f64::MAX, f64::min)
        };
        assert!((span(&before) - 1.0).abs() < 1e-6);
        assert!((span(&w.boundary) - 1.4).abs() < 1e-6);
        assert!(headroom(doc.model(), &stair(&doc, id), 1.95).low.is_empty());
        // deleting the stair takes its well
        let tx = Transaction::new("delete").remove_all_cascade(doc.model(), &[id]);
        doc.commit(tx).unwrap();
        assert!(well(&doc).is_none());
    }
}
