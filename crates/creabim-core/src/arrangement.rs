//! Arrangements of ordinary windows (doors and windows part 9): windows
//! side by side in a ribbon, and windows round a corner, one in each wall,
//! meeting at a corner post or glass to glass. Each window keeps its own
//! frame and cells; the arrangement says which group it is in and which of
//! its ends reaches into a corner.

use crate::element::{
    Arrangement, ArrangementKind, CornerJoint, Element, ElementKind, Opening, OpeningKind,
};
use crate::id::ElementId;
use crate::model::Model;
use crate::ops::Transaction;
use glam::DVec2;

/// How close two wall ends are to meet at a corner.
const MEET: f64 = 0.02;

/// A group number no window uses yet.
pub fn next_group(model: &Model) -> u32 {
    model
        .iter()
        .filter_map(|e| match &e.kind {
            ElementKind::Opening(o) => o.arrangement.map(|a| a.group),
            _ => None,
        })
        .max()
        .unwrap_or(0)
        + 1
}

/// The windows of `id`'s group, itself included, in no set order.
pub fn group_of(model: &Model, id: ElementId) -> Vec<ElementId> {
    let Some(a) = window(model, id).and_then(|(_, o)| o.arrangement) else {
        return vec![id];
    };
    model
        .iter()
        .filter(|e| {
            matches!(&e.kind, ElementKind::Opening(o)
                if o.arrangement.is_some_and(|b| b.group == a.group && b.kind == a.kind))
        })
        .map(|e| e.id)
        .collect()
}

fn window(model: &Model, id: ElementId) -> Option<(&Element, &Opening)> {
    let e = model.get(id)?;
    match &e.kind {
        ElementKind::Opening(o) if o.kind == OpeningKind::Window && o.roof_at.is_none() => {
            Some((e, o))
        }
        _ => None,
    }
}

/// A copy of `e` with its own id, no mark of its own and no source.
fn copy_of(e: &Element, o: Opening) -> Element {
    let mut n = e.clone();
    n.id = ElementId::new();
    n.mark = None;
    n.source = None;
    n.kind = ElementKind::Opening(o);
    n
}

/// Another window beside `id`, the same, coupled to it in a ribbon: on the
/// side towards the wall's end, or the start when there is no room there.
pub fn add_beside(model: &Model, id: ElementId) -> Result<Transaction, String> {
    let (e, o) = window(model, id).ok_or("only a window in a wall makes a ribbon")?;
    let Some(ElementKind::Wall(w)) = model.get(o.host).map(|e| &e.kind) else {
        return Err("its wall is gone".into());
    };
    if o.arrangement
        .is_some_and(|a| a.kind != ArrangementKind::Ribbon)
    {
        return Err("it is already round a corner or in a bay".into());
    }
    let len = w.length();
    // past the last window of the ribbon on that side
    let mut group: Vec<Opening> = group_of(model, id)
        .into_iter()
        .filter_map(|g| window(model, g).map(|(_, o)| o.clone()))
        .filter(|g| g.host == o.host)
        .collect();
    group.sort_by(|a, b| a.offset.total_cmp(&b.offset));
    let first = group.first().map_or(o.offset, |g| g.offset);
    let last = group
        .last()
        .map_or(o.offset + o.width, |g| g.offset + g.width);
    let (lo, hi) = crate::walls::clear_span(model, o.host);
    // another door or window in the same wall that the new one would run into
    let clear = |at: f64| {
        !model.iter().any(|e| {
            matches!(&e.kind, ElementKind::Opening(p)
                if p.host == o.host && p.roof_at.is_none()
                    && p.offset < at + o.width - 1e-6 && at < p.offset + p.width - 1e-6)
        })
    };
    let at = if last + o.width <= hi.min(len) + 1e-9 && clear(last) {
        last
    } else if first - o.width >= lo.max(0.0) - 1e-9 && clear(first - o.width) {
        first - o.width
    } else {
        return Err("no room beside it in this wall".into());
    };
    let a = o.arrangement.unwrap_or(Arrangement {
        kind: ArrangementKind::Ribbon,
        group: next_group(model),
        corner_start: false,
        corner_end: false,
        joint: CornerJoint::Post,
        left_start: false,
        left_end: false,
        angle_start: None,
        angle_end: None,
    });
    let mut n = o.clone();
    n.offset = at;
    n.arrangement = Some(Arrangement {
        corner_start: false,
        corner_end: false,
        left_start: false,
        left_end: false,
        angle_start: None,
        angle_end: None,
        ..a
    });
    let mut tx = Transaction::new("Add a window to the ribbon").insert(copy_of(e, n));
    if o.arrangement.is_none() {
        let mut was = o.clone();
        was.arrangement = Some(a);
        let mut after = e.clone();
        after.kind = ElementKind::Opening(was);
        tx = tx.replace(e.clone(), after);
    }
    Ok(tx)
}

/// The wall that meets `wall`'s start (`end` false) or end at a corner, on
/// its level, and whether it is that wall's end (true) or start there.
pub fn wall_at_corner(model: &Model, wall: ElementId, end: bool) -> Option<(ElementId, bool)> {
    let Some(ElementKind::Wall(w)) = model.get(wall).map(|e| &e.kind) else {
        return None;
    };
    let p = if end { w.end } else { w.start };
    let dir = w.direction();
    model.iter().find_map(|e| match &e.kind {
        ElementKind::Wall(v) if e.id != wall && v.level == w.level && v.curtain.is_none() => {
            // a corner, not two walls in line
            if v.direction().perp_dot(dir).abs() < 0.2 {
                return None;
            }
            if v.start.distance(p) < MEET {
                Some((e.id, false))
            } else if v.end.distance(p) < MEET {
                Some((e.id, true))
            } else {
                None
            }
        }
        _ => None,
    })
}

/// Where a window's end meets the wall's end at a corner: the offset that
/// puts that end of it on the corner.
pub fn corner_offset(len: f64, width: f64, end: bool) -> f64 {
    if end {
        len - width
    } else {
        0.0
    }
}

/// `id` taken to the corner at its wall's start (`end` false) or end, and a
/// window the same round the corner in the wall that meets it there,
/// joined at a corner post or glass to glass.
pub fn add_round_corner(
    model: &Model,
    id: ElementId,
    end: bool,
    joint: CornerJoint,
) -> Result<Transaction, String> {
    let (e, o) = window(model, id).ok_or("only a window in a wall turns a corner")?;
    if o.arrangement
        .is_some_and(|a| a.kind != ArrangementKind::Corner)
    {
        return Err("it is already in a ribbon or a bay".into());
    }
    if o.arrangement.is_some_and(|a| a.corner_at(end)) {
        return Err("it already turns that corner".into());
    }
    let Some(ElementKind::Wall(w)) = model.get(o.host).map(|e| &e.kind) else {
        return Err("its wall is gone".into());
    };
    let (other, other_end) =
        wall_at_corner(model, o.host, end).ok_or("no wall meets this one at that end")?;
    let Some(ElementKind::Wall(v)) = model.get(other).map(|e| &e.kind) else {
        return Err("that wall is gone".into());
    };
    if v.length() < o.width + 0.05 || w.length() < o.width + 0.05 {
        return Err("a wall at that corner is too short for the window".into());
    }
    let group = o.arrangement.map_or_else(|| next_group(model), |a| a.group);
    let mut here = o.clone();
    let mut a = here.arrangement.unwrap_or(Arrangement {
        kind: ArrangementKind::Corner,
        group,
        corner_start: false,
        corner_end: false,
        joint,
        left_start: false,
        left_end: false,
        angle_start: None,
        angle_end: None,
    });
    a.joint = joint;
    // which side each window's wall the other runs off to, for the mitre
    let (dw, dv) = (w.direction(), v.direction());
    let into_w = if end { -dw } else { dw };
    let into_v = if other_end { -dv } else { dv };
    let v_left_of_w = into_v.dot(dw.perp()) > 0.0;
    let w_left_of_v = into_w.dot(dv.perp()) > 0.0;
    if end {
        a.corner_end = true;
        a.left_end = v_left_of_w;
        a.angle_end = Some(turn(dw, into_v));
    } else {
        a.corner_start = true;
        a.left_start = v_left_of_w;
        a.angle_start = Some(turn(dw, into_v));
    }
    here.arrangement = Some(a);
    here.offset = corner_offset(w.length(), here.width, end);
    let mut there = o.clone();
    there.host = other;
    there.offset = corner_offset(v.length(), there.width, other_end);
    there.arrangement = Some(Arrangement {
        kind: ArrangementKind::Corner,
        group,
        corner_start: !other_end,
        corner_end: other_end,
        joint,
        left_start: !other_end && w_left_of_v,
        left_end: other_end && w_left_of_v,
        angle_start: (!other_end).then(|| turn(dv, into_w)),
        angle_end: other_end.then(|| turn(dv, into_w)),
    });
    // neither runs into another door or window
    let free = |host: ElementId, at: f64, w: f64| {
        !model.iter().any(|x| {
            x.id != id
                && matches!(&x.kind, ElementKind::Opening(p)
                    if p.host == host && p.roof_at.is_none()
                        && p.offset < at + w - 1e-6 && at < p.offset + p.width - 1e-6)
        })
    };
    if !free(here.host, here.offset, here.width) || !free(there.host, there.offset, there.width) {
        return Err("another door or window is in the way at that corner".into());
    }
    // facing out of the same side of the corner as this one
    there.flip_facing = facing_round(o, w, v, end, other_end);
    let mut after = e.clone();
    after.kind = ElementKind::Opening(here);
    Ok(Transaction::new("Turn the window round the corner")
        .replace(e.clone(), after)
        .insert(copy_of(e, there)))
}

/// The `flip_facing` that turns the window round the corner to the same
/// side of the corner as `o`: if `o` faces out of the corner's outside
/// (away from the other wall's body) so does the new one, else both face
/// into its inside.
fn facing_round(
    o: &Opening,
    w: &crate::element::Wall,
    v: &crate::element::Wall,
    end: bool,
    other_end: bool,
) -> bool {
    let (dw, dv) = (w.direction(), v.direction());
    // from the corner along each wall, into its body
    let into_w = if end { -dw } else { dw };
    let into_v = if other_end { -dv } else { dv };
    let out_o = if o.faces_left(w) {
        dw.perp()
    } else {
        -dw.perp()
    };
    let outside = out_o.dot(into_v) < 0.0;
    // the new window faces v's left when that points away from w's body
    // on the outside (or into it on the inside)
    let left = (dv.perp().dot(into_w) < 0.0) == outside;
    // a window faces left unless flipped, when its wall is flipped
    v.flip != left
}

/// A corner window's offset kept on its corner as its wall changes length.
pub fn keep_on_corner(len: f64, o: &mut Opening) {
    let Some(a) = o.arrangement else { return };
    if a.corner_end {
        o.offset = corner_offset(len, o.width, true);
    } else if a.corner_start {
        o.offset = 0.0;
    }
}

/// Every window of `id`'s corner meeting with `joint`.
pub fn set_joint(model: &Model, id: ElementId, joint: CornerJoint) -> Transaction {
    let mut tx = Transaction::new("Change the corner joint");
    for g in group_of(model, id) {
        let Some(e) = model.get(g) else { continue };
        let ElementKind::Opening(o) = &e.kind else {
            continue;
        };
        let Some(mut a) = o.arrangement else { continue };
        if a.joint == joint {
            continue;
        }
        a.joint = joint;
        let mut after = e.clone();
        if let ElementKind::Opening(o) = &mut after.kind {
            o.arrangement = Some(a);
        }
        tx = tx.replace(e.clone(), after);
    }
    tx
}

/// `id` on its own again, where it stands; a group left with one window
/// leaves it too.
pub fn leave(model: &Model, id: ElementId) -> Transaction {
    let group = group_of(model, id);
    let rest: Vec<ElementId> = group.iter().copied().filter(|g| *g != id).collect();
    let mut tx = Transaction::new("Take the window out of its group");
    let alone = |e: &Element| {
        let mut after = e.clone();
        if let ElementKind::Opening(o) = &mut after.kind {
            o.arrangement = None;
        }
        after
    };
    let mut ids = vec![id];
    if rest.len() == 1 {
        ids.extend(rest);
    }
    for g in ids {
        if let Some(e) = model.get(g) {
            tx = tx.replace(e.clone(), alone(e));
        }
    }
    tx
}

/// The angle of `d` from direction `along`, towards its left.
fn turn(along: DVec2, d: DVec2) -> f64 {
    along.perp_dot(d).atan2(along.dot(d))
}

/// The shapes a bay takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BayShape {
    /// Square sides: a box bay (oriel).
    Box,
    /// Sides at 45 degrees: the canted bay.
    Angled,
    /// A curve of `n` facets: a bow window.
    Bow(u32),
}

/// The bay `id` makes, `depth` out from its wall on the side it faces out
/// of: the wall opened over the window's width, a run of short walls the
/// same as it out round the bay, the same window in each (meeting at
/// posts, or glass to glass), and the floor on the level carried out to the
/// bay's outside. The window becomes the bay's windows.
pub fn make_bay(
    model: &Model,
    id: ElementId,
    shape: BayShape,
    depth: f64,
    joint: CornerJoint,
) -> Result<Transaction, String> {
    let (e, o) = window(model, id).ok_or("only a window in a wall makes a bay")?;
    if o.arrangement.is_some() {
        return Err("take it out of its group first".into());
    }
    let host_e = model.get(o.host).ok_or("its wall is gone")?;
    let ElementKind::Wall(w) = &host_e.kind else {
        return Err("only a window in a wall makes a bay".into());
    };
    if w.is_arc() {
        return Err("a bay needs a straight wall".into());
    }
    if depth < 0.2 {
        return Err("a bay comes out 200 mm at least".into());
    }
    let u = w.direction();
    let len = w.length();
    let (a, b) = (o.offset, o.offset + o.width);
    let t = w.thickness;
    if a < t || b > len - t {
        return Err("too near the wall's end for a bay".into());
    }
    // no other door or window where the wall opens
    if model.iter().any(|x| {
        x.id != id
            && matches!(&x.kind, ElementKind::Opening(p)
                if p.host == o.host && p.roof_at.is_none() && p.offset < b && a < p.offset + p.width)
    }) {
        return Err("another door or window is in the way".into());
    }
    let n = if o.faces_left(w) { u.perp() } else { -u.perp() };
    let (pa, pb) = (w.start + u * a, w.start + u * b);
    // the bay's line, from the wall at A out round to the wall at B
    let mut chain = vec![pa];
    match shape {
        BayShape::Box => {
            chain.push(pa + n * depth);
            chain.push(pb + n * depth);
        }
        BayShape::Angled => {
            if b - a < 2.0 * depth + 0.4 {
                return Err("the window is too narrow for a bay that deep".into());
            }
            chain.push(pa + n * depth + u * depth);
            chain.push(pb + n * depth - u * depth);
        }
        BayShape::Bow(k) => {
            let k = k.clamp(3, 9);
            let c = b - a;
            let r = (c * c / 4.0 + depth * depth) / (2.0 * depth);
            let centre = (pa + pb) / 2.0 + n * (depth - r);
            let (sa, sb) = (pa - centre, pb - centre);
            let (a0, a1) = (sa.y.atan2(sa.x), sb.y.atan2(sb.x));
            // the short way round, through the bow's outer point
            let mut sweep = a1 - a0;
            let mid = (pa + pb) / 2.0 + n * depth - centre;
            let am = mid.y.atan2(mid.x);
            let on = |x: f64| {
                let mut d = (am - a0) - x;
                while d > std::f64::consts::PI {
                    d -= std::f64::consts::TAU;
                }
                while d < -std::f64::consts::PI {
                    d += std::f64::consts::TAU;
                }
                d.abs() < x.abs().max(1e-9)
            };
            while sweep > std::f64::consts::PI {
                sweep -= std::f64::consts::TAU;
            }
            while sweep < -std::f64::consts::PI {
                sweep += std::f64::consts::TAU;
            }
            if !on(sweep / 2.0) && sweep.abs() > 1e-9 {
                sweep -= std::f64::consts::TAU * sweep.signum();
            }
            for i in 1..k {
                let ang = a0 + sweep * i as f64 / k as f64;
                chain.push(centre + DVec2::new(ang.cos(), ang.sin()) * r);
            }
        }
    }
    chain.push(pb);
    let facets: Vec<(DVec2, DVec2)> = chain.windows(2).map(|p| (p[0], p[1])).collect();
    if facets.iter().any(|(p, q)| p.distance(*q) < 0.3) {
        return Err("the bay's sides would be too short".into());
    }
    let group = next_group(model);
    let kind = if matches!(shape, BayShape::Bow(_)) {
        ArrangementKind::Bow
    } else {
        ArrangementKind::Bay
    };
    let mut tx = Transaction::new(if kind == ArrangementKind::Bow {
        "Make a bow window"
    } else {
        "Make a bay window"
    });
    // the wall: this one to A, a new one from B on, its doors and windows
    // past the bay going with it
    let mut head = host_e.clone();
    let mut tail = host_e.clone();
    tail.id = ElementId::new();
    tail.mark = None;
    tail.source = None;
    if let (ElementKind::Wall(h), ElementKind::Wall(tl)) = (&mut head.kind, &mut tail.kind) {
        h.end = pa;
        tl.start = pb;
    }
    // the new wall first, then what moves into it
    tx = tx.replace(host_e.clone(), head).insert(tail.clone());
    for x in model.iter() {
        if let ElementKind::Opening(p) = &x.kind {
            if p.host == o.host && p.roof_at.is_none() && p.offset >= b - 1e-9 {
                let mut after = x.clone();
                if let ElementKind::Opening(q) = &mut after.kind {
                    q.host = tail.id;
                    q.offset -= b;
                }
                tx = tx.replace(x.clone(), after);
            }
        }
    }
    // the bay's walls, the same as the wall, and a window in each
    let walls: Vec<Element> = facets
        .iter()
        .enumerate()
        .map(|(i, (p, q))| {
            let mut f = host_e.clone();
            f.id = ElementId::new();
            f.name = format!("{} bay {}", host_e.name, i + 1);
            f.mark = None;
            f.source = None;
            if let ElementKind::Wall(fw) = &mut f.kind {
                fw.start = *p;
                fw.end = *q;
            }
            f
        })
        .collect();
    let last = walls.len() - 1;
    // the stand-off from the house wall: as far as its body reaches into
    // the side at the angle they meet, and a 50 mm jamb
    let pier = |f: &Element| match &f.kind {
        ElementKind::Wall(fw) => t / 2.0 / u.perp_dot(fw.direction()).abs().max(0.3) + 0.05,
        _ => t,
    };
    let flen = |f: &Element| match &f.kind {
        ElementKind::Wall(fw) => fw.length(),
        _ => 0.0,
    };
    // which sides are long enough for a window, standing off the house
    // wall by a pier at the ends that meet it (a shallow box bay's sides
    // stay solid)
    let fits: Vec<bool> = (0..walls.len())
        .map(|i| {
            let piers = (i == 0) as u8 as f64 + (i == last) as u8 as f64;
            flen(&walls[i]) - piers * pier(&walls[i]) >= 0.3
        })
        .collect();
    let mut wins = vec![];
    for (i, f) in walls.iter().enumerate() {
        let ElementKind::Wall(fw) = &f.kind else {
            unreachable!()
        };
        let flen = fw.length();
        let d = fw.direction();
        if !fits[i] {
            continue;
        }
        // into each corner where the side beyond has a window too; else it
        // stands off the wall there by a pier
        let start = i > 0 && fits[i - 1];
        let end = i < last && fits[i + 1];
        let pier = pier(f);
        let width = flen - if start { 0.0 } else { pier } - if end { 0.0 } else { pier };
        if width < 0.3 {
            continue;
        }
        let prev = (i > 0).then(|| walls[i - 1].clone());
        let next = (i < last).then(|| walls[i + 1].clone());
        let dir_of = |x: &Option<Element>| match x.as_ref().map(|x| &x.kind) {
            Some(ElementKind::Wall(v)) => Some(v.direction()),
            _ => None,
        };
        let mut win = o.clone();
        win.host = f.id;
        win.offset = if start { 0.0 } else { pier };
        win.width = width;
        // facing out of the bay, as the window did out of the wall
        win.flip_facing = o.flip_facing;
        win.arrangement = Some(Arrangement {
            kind,
            group,
            corner_start: start,
            corner_end: end,
            joint,
            left_start: false,
            left_end: false,
            // the facet before runs back from this one's start, the next
            // on from its end
            angle_start: dir_of(&prev).filter(|_| start).map(|pd| turn(d, -pd)),
            angle_end: dir_of(&next).filter(|_| end).map(|nd| turn(d, nd)),
        });
        wins.push(copy_of(e, win));
    }
    // the walls first, then the windows in them
    for f in walls {
        tx = tx.insert(f);
    }
    for x in wins {
        tx = tx.insert(x);
    }
    tx = tx.remove_cascade(model, id);
    // the floor carried out to the bay's outside face
    if let Some(slab_tx) = bay_floor(model, w, n, &chain, t) {
        for op in slab_tx.ops {
            tx.ops.push(op);
        }
    }
    Ok(tx)
}

/// The floor on the wall's level carried out under a bay whose line runs
/// `chain` (A out round to B, on the walls' centre lines): its outline
/// gains the bay's outside, where its edge runs along the wall's outside
/// face; else a floor of its own for the bay.
fn bay_floor(
    model: &Model,
    w: &crate::element::Wall,
    n: DVec2,
    chain: &[DVec2],
    t: f64,
) -> Option<Transaction> {
    let h = t / 2.0;
    // each facet's outside line, and where neighbours meet
    let centre = chain.iter().copied().sum::<DVec2>() / chain.len() as f64;
    let lines: Vec<(DVec2, DVec2)> = chain
        .windows(2)
        .map(|p| {
            let d = (p[1] - p[0]).normalize();
            let mut out = d.perp();
            if out.dot((p[0] + p[1]) / 2.0 - centre) < 0.0 {
                out = -out;
            }
            (p[0] + out * h, d)
        })
        .collect();
    let meet = |(p, d): (DVec2, DVec2), (q, e): (DVec2, DVec2)| -> DVec2 {
        let den = d.perp_dot(e);
        if den.abs() < 1e-9 {
            return q;
        }
        p + d * ((q - p).perp_dot(e) / den)
    };
    let face = (
        chain[0] + n * h,
        (chain[chain.len() - 1] - chain[0]).normalize(),
    );
    let mut outline = vec![meet(face, lines[0])];
    for k in 0..lines.len() - 1 {
        outline.push(meet(lines[k], lines[k + 1]));
    }
    outline.push(meet(lines[lines.len() - 1], face));
    // the level's floor along the wall's outside face
    let slab = model.iter().find_map(|e| match &e.kind {
        ElementKind::Slab(s) if s.level == w.level && s.top_offset.abs() < 1e-6 => {
            let m = s.boundary.len();
            (0..m)
                .find(|&k| {
                    let (p, q) = (s.boundary[k], s.boundary[(k + 1) % m]);
                    let on = |x: DVec2| {
                        let pq = q - p;
                        let tt = (x - p).dot(pq) / pq.length_squared().max(1e-12);
                        (-1e-6..=1.0 + 1e-6).contains(&tt) && (p + pq * tt).distance(x) < 0.03
                    };
                    on(outline[0])
                        && on(outline[outline.len() - 1])
                        && s.bulges.get(k).is_none_or(|b| b.abs() < 1e-9)
                        && s.layer_edges.is_empty()
                })
                .map(|k| (e, s, k))
        }
        _ => None,
    });
    if let Some((e, s, k)) = slab {
        let m = s.boundary.len();
        let p = s.boundary[k];
        let q = s.boundary[(k + 1) % m];
        let mut pts = outline.clone();
        // in the edge's own direction
        if pts[0].distance(p) > pts[pts.len() - 1].distance(p) {
            pts.reverse();
        }
        let _ = q;
        let mut after = e.clone();
        if let ElementKind::Slab(ns) = &mut after.kind {
            for (j, x) in pts.into_iter().enumerate() {
                ns.boundary.insert(k + 1 + j, x);
            }
            if !ns.bulges.is_empty() {
                let add = ns.boundary.len() - s.boundary.len();
                for _ in 0..add {
                    ns.bulges.insert((k + 1).min(ns.bulges.len()), 0.0);
                }
            }
        }
        return Some(Transaction::new("floor").replace(e.clone(), after));
    }
    // no floor to carry on: one of its own, as thick as the level's
    let thickness = model.iter().find_map(|e| match &e.kind {
        ElementKind::Slab(s) if s.level == w.level => Some(s.thickness),
        _ => None,
    })?;
    let mut poly = outline;
    let area: f64 = (0..poly.len())
        .map(|k| poly[k].perp_dot(poly[(k + 1) % poly.len()]))
        .sum();
    if area < 0.0 {
        poly.reverse();
    }
    Some(Transaction::new("floor").insert(Element::new(
        "Bay floor",
        ElementKind::Slab(crate::element::Slab::new(w.level, poly, thickness)),
    )))
}

/// The two walls' meeting point at a corner, for drawings.
pub fn corner_point(model: &Model, wall: ElementId, end: bool) -> Option<DVec2> {
    let Some(ElementKind::Wall(w)) = model.get(wall).map(|e| &e.kind) else {
        return None;
    };
    Some(if end { w.end } else { w.start })
}

/// The schedule's name for a window's arrangement: "Ribbon 1", "Corner 2,
/// glass", or empty when it stands on its own.
pub fn label(o: &Opening) -> String {
    o.arrangement.map(|a| a.label()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::{Level, Slab, Wall};

    /// A 6 x 4 room drawn anticlockwise (inside on the walls' left), and a
    /// 1.2 m window in its south wall at `offset`.
    fn room(offset: f64) -> (Model, [ElementId; 4], ElementId) {
        let mut m = Model::default();
        let l = Element::new(
            "L0",
            ElementKind::Level(Level {
                elevation: 0.0,
                height: 3.0,
            }),
        );
        let p = [
            DVec2::new(0.0, 0.0),
            DVec2::new(6.0, 0.0),
            DVec2::new(6.0, 4.0),
            DVec2::new(0.0, 4.0),
        ];
        let walls: Vec<Element> = (0..4)
            .map(|i| {
                Element::new(
                    format!("W{i}"),
                    ElementKind::Wall(Wall::new(l.id, p[i], p[(i + 1) % 4], 0.2, 2.7)),
                )
            })
            .collect();
        let ids = [walls[0].id, walls[1].id, walls[2].id, walls[3].id];
        let win = Element::new(
            "Window",
            ElementKind::Opening(Opening {
                host: ids[0],
                kind: OpeningKind::Window,
                offset,
                sill: 0.9,
                width: 1.2,
                height: 1.2,
                material: None,
                frame: crate::OpeningFrame::for_kind(OpeningKind::Window),
                flip_hand: false,
                flip_facing: false,
                roof_at: None,
                roof_build: Default::default(),
                arrangement: None,
            }),
        );
        let wid = win.id;
        let mut tx = Transaction::new("setup").insert(l);
        for w in walls {
            tx = tx.insert(w);
        }
        tx.insert(win).apply(&mut m).unwrap();
        (m, ids, wid)
    }

    fn windows(m: &Model) -> Vec<Opening> {
        m.iter()
            .filter_map(|e| match &e.kind {
                ElementKind::Opening(o) => Some(o.clone()),
                _ => None,
            })
            .collect()
    }

    /// Does `o` face out of the room (away from its middle)?
    fn faces_out(m: &Model, o: &Opening) -> bool {
        let Some(ElementKind::Wall(w)) = m.get(o.host).map(|e| &e.kind) else {
            unreachable!()
        };
        let d = w.direction();
        let n = if o.faces_left(w) { d.perp() } else { -d.perp() };
        let at = w.start + d * (o.offset + o.width / 2.0);
        n.dot(at - DVec2::new(3.0, 2.0)) > 0.0
    }

    #[test]
    fn a_ribbon_grows_window_by_window_and_stops_at_the_corner() {
        let (mut m, _, id) = room(1.0);
        add_beside(&m, id).unwrap().apply(&mut m).unwrap();
        let mut ws = windows(&m);
        ws.sort_by(|a, b| a.offset.total_cmp(&b.offset));
        assert_eq!(ws.len(), 2);
        // abutting, frame to frame, in one group
        assert!((ws[1].offset - 2.2).abs() < 1e-9, "{}", ws[1].offset);
        let (a, b) = (ws[0].arrangement.unwrap(), ws[1].arrangement.unwrap());
        assert_eq!((a.kind, a.group), (ArrangementKind::Ribbon, b.group));
        // a third and a fourth, to 5.8: clear of the corner wall's face
        // at 5.9; then no room either side (1.0 before the first)
        add_beside(&m, id).unwrap().apply(&mut m).unwrap();
        add_beside(&m, id).unwrap().apply(&mut m).unwrap();
        let ends: Vec<f64> = windows(&m).iter().map(|o| o.offset + o.width).collect();
        assert!(ends.iter().all(|e| *e <= 5.9 + 1e-9), "{ends:?}");
        assert!(add_beside(&m, id).is_err());
        assert_eq!(next_group(&m), 2);
        // from 1.4: three to 5.0, then the next goes before the first
        let (mut m, _, id) = room(1.4);
        for _ in 0..3 {
            add_beside(&m, id).unwrap().apply(&mut m).unwrap();
        }
        let mut at: Vec<f64> = windows(&m).iter().map(|o| o.offset).collect();
        at.sort_by(f64::total_cmp);
        assert!(
            (at[0] - 0.2).abs() < 1e-9 && (at[3] - 3.8).abs() < 1e-9,
            "{at:?}"
        );
    }

    #[test]
    fn a_ribbon_does_not_run_into_another_window() {
        // a window at 3.0 stops a ribbon from 1.0 growing past 2.2...3.4
        let (mut m, walls, id) = room(1.0);
        let other = m.get(id).unwrap().clone();
        let mut other = copy_of(&other, {
            let ElementKind::Opening(o) = &other.kind else {
                unreachable!()
            };
            Opening {
                offset: 3.0,
                ..o.clone()
            }
        });
        other.name = "Other".into();
        Transaction::new("other")
            .insert(other)
            .apply(&mut m)
            .unwrap();
        assert!(add_beside(&m, id).is_err());
        let _ = walls;
    }

    #[test]
    fn a_window_turns_the_corner_facing_out_on_both_walls() {
        let (mut m, walls, id) = room(2.0);
        add_round_corner(&m, id, true, CornerJoint::Glass)
            .unwrap()
            .apply(&mut m)
            .unwrap();
        let ws = windows(&m);
        let south = ws.iter().find(|o| o.host == walls[0]).unwrap();
        let east = ws.iter().find(|o| o.host == walls[1]).unwrap();
        // into the corner: the south one's end at the wall's end, the east
        // one's start at the east wall's start
        assert!((south.offset + south.width - 6.0).abs() < 1e-9);
        assert!(east.offset.abs() < 1e-9);
        let (a, b) = (south.arrangement.unwrap(), east.arrangement.unwrap());
        assert!(a.corner_end && !a.corner_start && b.corner_start && !b.corner_end);
        assert_eq!((a.group, a.joint), (b.group, CornerJoint::Glass));
        // the room is to every wall's left: so is the other wall
        assert!(a.left_end && b.left_start);
        assert!(faces_out(&m, south) && faces_out(&m, east));
        // moved along, it stays on its corner
        let mut moved = south.clone();
        moved.offset = 1.0;
        crate::walls::clamp_opening(&m, &mut moved);
        assert!((moved.offset - 4.8).abs() < 1e-9, "{}", moved.offset);
    }

    #[test]
    fn round_the_start_and_facing_in_it_faces_in_round_the_corner_too() {
        let (mut m, walls, id) = room(2.0);
        // facing the room: the one round the corner faces it too
        let e = m.get(id).unwrap().clone();
        let mut after = e.clone();
        if let ElementKind::Opening(o) = &mut after.kind {
            o.flip_facing = true;
        }
        Transaction::new("flip")
            .replace(e, after)
            .apply(&mut m)
            .unwrap();
        add_round_corner(&m, id, false, CornerJoint::Post)
            .unwrap()
            .apply(&mut m)
            .unwrap();
        let ws = windows(&m);
        let south = ws.iter().find(|o| o.host == walls[0]).unwrap();
        // the west wall ends at the south wall's start
        let west = ws.iter().find(|o| o.host == walls[3]).unwrap();
        assert!(south.offset.abs() < 1e-9);
        assert!((west.offset + west.width - 4.0).abs() < 1e-9);
        assert!(west.arrangement.unwrap().corner_end);
        assert!(!faces_out(&m, south) && !faces_out(&m, west));
        // and it can't turn the same corner twice
        assert!(add_round_corner(&m, id, false, CornerJoint::Post).is_err());
        assert_eq!(label(south), "Corner 1");
    }

    #[test]
    fn a_corner_is_refused_where_a_window_is_in_the_way() {
        let (mut m, walls, id) = room(2.0);
        let e = m.get(id).unwrap().clone();
        let ElementKind::Opening(o) = &e.kind else {
            unreachable!()
        };
        let blocker = copy_of(
            &e,
            Opening {
                host: walls[1],
                offset: 0.5,
                ..o.clone()
            },
        );
        Transaction::new("b").insert(blocker).apply(&mut m).unwrap();
        assert!(add_round_corner(&m, id, true, CornerJoint::Post).is_err());
        // the other corner is free
        assert!(add_round_corner(&m, id, false, CornerJoint::Post).is_ok());
    }

    /// Part 9b: a bay opens the wall over the window and runs out round it,
    /// a window in each side meeting its neighbours at the corners, facing
    /// out; the floor runs out under it.
    #[test]
    fn bays_and_bows_open_the_wall_and_window_each_side() {
        for (shape, sides) in [
            (BayShape::Box, 3),
            (BayShape::Angled, 3),
            (BayShape::Bow(5), 5),
        ] {
            let (mut m, walls, id) = room(2.0);
            // a 2.4 m window in the 6 m south wall, a floor to the outside faces
            let e = m.get(id).unwrap().clone();
            let mut wide = e.clone();
            if let ElementKind::Opening(o) = &mut wide.kind {
                o.width = 2.4;
            }
            let lvl = match &m.get(walls[0]).unwrap().kind {
                ElementKind::Wall(w) => w.level,
                _ => unreachable!(),
            };
            let floor = Element::new(
                "Floor",
                ElementKind::Slab(Slab::new(
                    lvl,
                    vec![
                        DVec2::new(-0.1, -0.1),
                        DVec2::new(6.1, -0.1),
                        DVec2::new(6.1, 4.1),
                        DVec2::new(-0.1, 4.1),
                    ],
                    0.2,
                )),
            );
            let fid = floor.id;
            Transaction::new("w")
                .replace(e, wide)
                .insert(floor)
                .apply(&mut m)
                .unwrap();
            make_bay(&m, id, shape, 0.6, CornerJoint::Post)
                .unwrap()
                .apply(&mut m)
                .unwrap();
            assert!(m.get(id).is_none(), "the window became the bay's");
            let bay: Vec<Opening> = windows(&m);
            assert_eq!(bay.len(), sides, "{shape:?}");
            let walls_now = m
                .iter()
                .filter(|e| matches!(e.kind, ElementKind::Wall(_)))
                .count();
            // the south wall in two, and a wall for each side
            assert_eq!(walls_now, 4 + 1 + sides, "{shape:?}");
            for o in &bay {
                let a = o.arrangement.unwrap();
                assert_eq!(a.group, 1);
                assert!(faces_out_of(&m, o, DVec2::new(3.0, 2.0)), "{shape:?} {o:?}");
            }
            // the inner windows meet at both ends, the outer ones at one
            let corners: usize = bay
                .iter()
                .map(|o| o.arrangement.unwrap())
                .map(|a| a.corner_start as usize + a.corner_end as usize)
                .sum();
            assert_eq!(corners, 2 * (sides - 1), "{shape:?}");
            // nothing runs past its side's ends
            for o in &bay {
                let Some(ElementKind::Wall(w)) = m.get(o.host).map(|e| &e.kind) else {
                    unreachable!()
                };
                assert!(o.offset >= -1e-9 && o.offset + o.width <= w.length() + 1e-9);
            }
            // the floor runs out under the bay
            let ElementKind::Slab(s) = &m.get(fid).unwrap().kind else {
                unreachable!()
            };
            assert!(
                crate::polygon::contains(&s.boundary, DVec2::new(3.0, -0.4)),
                "{shape:?} {:?}",
                s.boundary
            );
            assert!(!crate::polygon::contains(
                &s.boundary,
                DVec2::new(1.0, -0.4)
            ));
        }
        // too narrow for an angled bay that deep
        let (m, _, id) = room(2.0);
        assert!(make_bay(&m, id, BayShape::Angled, 0.6, CornerJoint::Post).is_err());
    }

    /// Does `o` face away from `inside`?
    fn faces_out_of(m: &Model, o: &Opening, inside: DVec2) -> bool {
        let Some(ElementKind::Wall(w)) = m.get(o.host).map(|e| &e.kind) else {
            unreachable!()
        };
        let d = w.direction();
        let n = if o.faces_left(w) { d.perp() } else { -d.perp() };
        let at = w.start + d * (o.offset + o.width / 2.0);
        n.dot(at - inside) > 0.0
    }

    /// A box bay too shallow for windows in its sides: they stay solid and
    /// the front window stands off them, turning no corner.
    #[test]
    fn a_shallow_box_bays_sides_stay_solid() {
        let (mut m, _, id) = room(1.0);
        let e = m.get(id).unwrap().clone();
        let mut wide = e.clone();
        if let ElementKind::Opening(o) = &mut wide.kind {
            o.width = 2.0;
        }
        Transaction::new("w")
            .replace(e, wide)
            .apply(&mut m)
            .unwrap();
        make_bay(&m, id, BayShape::Box, 0.4, CornerJoint::Post)
            .unwrap()
            .apply(&mut m)
            .unwrap();
        let ws = windows(&m);
        assert_eq!(ws.len(), 1);
        let a = ws[0].arrangement.unwrap();
        assert!(!a.corner_start && !a.corner_end, "{a:?}");
        assert!(ws[0].offset > 0.2);
    }

    #[test]
    fn a_window_past_the_bay_moves_to_the_wall_beyond_it() {
        let (mut m, walls, id) = room(1.0);
        let e = m.get(id).unwrap().clone();
        let ElementKind::Opening(o) = &e.kind else {
            unreachable!()
        };
        let past = copy_of(
            &e,
            Opening {
                offset: 4.0,
                ..o.clone()
            },
        );
        let pid = past.id;
        Transaction::new("p").insert(past).apply(&mut m).unwrap();
        make_bay(&m, id, BayShape::Box, 0.6, CornerJoint::Post)
            .unwrap()
            .apply(&mut m)
            .unwrap();
        let ElementKind::Opening(p) = &m.get(pid).unwrap().kind else {
            unreachable!()
        };
        assert_ne!(p.host, walls[0]);
        let Some(ElementKind::Wall(w)) = m.get(p.host).map(|e| &e.kind) else {
            unreachable!()
        };
        // the wall beyond starts at the bay's far side, 2.2 along
        assert!(
            (w.start.x - 2.2).abs() < 1e-9 && (p.offset - 1.8).abs() < 1e-9,
            "{:?} {}",
            w.start,
            p.offset
        );
    }

    #[test]
    fn a_corner_needs_a_wall_meeting_there() {
        let (mut m, walls, id) = room(2.0);
        Transaction::new("rm")
            .remove_cascade(&m, walls[1])
            .apply(&mut m)
            .unwrap();
        assert!(add_round_corner(&m, id, true, CornerJoint::Post).is_err());
    }
}
