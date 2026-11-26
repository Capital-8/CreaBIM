//! Doors and windows built inside their wall openings: a frame round the
//! edge, glass set back in it (split into panes by glazing bars), and for a
//! door a leaf with a handle. Every part is a box in the opening's own
//! frame, so the 3D view, drawings and IFC export share one description.

use crate::wall::Cut;
use crate::TriMesh;
use creabim_core::{Opening, OpeningFrame, OpeningKind, Operation, Wall};
use glam::{DVec2, DVec3};

/// What a part of a door or window is, which decides how it is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Part {
    /// Frame members and glazing bars, in the opening's material.
    Frame,
    /// A door's leaf, in the opening's material.
    Leaf,
    Glass,
    Handle,
    /// Linings, architraves and thresholds: shown in 3D, and in drawings
    /// at 1:20 and larger.
    Trim,
    /// A roof window's flashing collar, lapped into the roofing.
    Flashing,
    /// A roof window's insulation collar, round its frame in the roof.
    Insulation,
}

/// One part as a box in opening coordinates: `x` along the wall from the
/// opening's left edge, `y` across the wall from its centre line (towards
/// the wall's left, where a door swings), `z` up from the opening's bottom.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PartBox {
    pub part: Part,
    pub lo: DVec3,
    pub hi: DVec3,
}

/// One part shaped in elevation: an outline in `(x, z)` (opening
/// coordinates, convex unless `tris` says how it is split into
/// triangles) run across the wall from `y0` to `y1`. An arched or raked
/// head's frame, and whatever its curve cuts short below it.
#[derive(Clone, Debug, PartialEq)]
pub struct PartPrism {
    pub part: Part,
    pub outline: Vec<DVec2>,
    pub tris: Vec<[usize; 3]>,
    pub y0: f64,
    pub y1: f64,
}

/// One part shaped in section: an outline in `(y, z)` (across the wall
/// and up) run along the opening from `x0` to `x1`. A louvre's blade,
/// tilted and lapping the next (MU104).
#[derive(Clone, Debug, PartialEq)]
pub struct PartSlat {
    pub part: Part,
    pub outline: Vec<DVec2>,
    pub x0: f64,
    pub x1: f64,
}

/// A part as built: a box, shaped in elevation under an arched or raked
/// head, or shaped in section (a louvre's blade).
#[derive(Clone, Debug, PartialEq)]
pub enum Shaped {
    Box(PartBox),
    Prism(PartPrism),
    Slat(PartSlat),
    Plan(PartPlan),
}

/// A part shaped in plan, standing from `z0` to `z1`: a corner window's
/// member, glass or sill cut on the mitre where it meets the window round
/// the corner (part 9). `outline` is in opening coordinates (x along the
/// wall, y across it).
#[derive(Clone, Debug, PartialEq)]
pub struct PartPlan {
    pub part: Part,
    pub outline: Vec<DVec2>,
    pub z0: f64,
    pub z1: f64,
}

/// A glass louvre's blades (MU104, Mockup_Preview_And_Louvres_REV00 A):
/// 150 mm deep, 6 mm glass, tilted 15° from upright with the lower edge
/// to the outside to shed water, each lapping the next by about 15 mm
/// seen from the front. The blades are spread evenly over the cell, so
/// the first and the last stand the same small way off the sill and
/// the head, whatever the cell's height.
pub const BLADE_DEPTH: f64 = 0.150;
pub const BLADE_GLASS: f64 = 0.006;
pub const BLADE_TILT_DEG: f64 = 15.0;
pub const BLADE_LAP: f64 = 0.015;
/// The gap kept between the end blades and the frame above and below.
pub const BLADE_CLEAR: f64 = 0.008;

/// A blade's height and depth over all, its glass thickness included.
fn blade_extent() -> (f64, f64) {
    let (s, c) = BLADE_TILT_DEG.to_radians().sin_cos();
    (
        BLADE_DEPTH * c + BLADE_GLASS * s,
        BLADE_DEPTH * s + BLADE_GLASS * c,
    )
}

/// The blades of a louvred cell from `z0` to `z1`, each as the box round
/// it (bottom and top z, centred on y = 0): as many as lap the next by
/// close to [`BLADE_LAP`], evenly spread. None for a cell too low for
/// one.
pub fn louvre_blades(z0: f64, z1: f64) -> Vec<(f64, f64)> {
    let (h, _) = blade_extent();
    let room = z1 - z0 - 2.0 * BLADE_CLEAR;
    if room < h - 1e-9 {
        return vec![];
    }
    // as many blades as lap the next by 10 to 25 mm, the lap nearest
    // 15 mm, filling the cell; where no count does, a 15 mm lap and the
    // stack centred, the spare split between head and sill
    let mut best: Option<(usize, f64)> = None;
    for n in 2..200usize {
        let lap = h - (room - h) / (n - 1) as f64;
        if lap > 0.025 + 1e-12 {
            break;
        }
        if lap >= 0.010 - 1e-12
            && best.is_none_or(|(_, l)| (lap - BLADE_LAP).abs() < (l - BLADE_LAP).abs())
        {
            best = Some((n, lap));
        }
    }
    let (n, step) = match best {
        Some((n, lap)) => (n, h - lap),
        None => (
            ((room - h) / (h - BLADE_LAP)).floor() as usize + 1,
            h - BLADE_LAP,
        ),
    };
    let stack = h + step * (n - 1) as f64;
    let first = z0 + (z1 - z0 - stack) / 2.0;
    (0..n)
        .map(|k| (first + step * k as f64, first + step * k as f64 + h))
        .collect()
}

/// A blade's section in `(y, z)` from the box round it: the lower edge to
/// the outside, the upper inside, to shed rain. `out` is the outside's
/// side, +1 or -1 (a window is built facing +y, then turned to its wall).
pub fn blade_outline(ya: f64, yb: f64, za: f64, out: f64) -> Vec<DVec2> {
    let mirror = |q: DVec2| {
        if out < 0.0 {
            DVec2::new(ya + yb - q.x, q.y)
        } else {
            q
        }
    };
    let (s, c) = BLADE_TILT_DEG.to_radians().sin_cos();
    let t = BLADE_GLASS / 2.0;
    // the lower edge's middle, the run up the blade and its face normal
    let bottom = DVec2::new(yb - c * t, za + s * t);
    let up = DVec2::new(-s, c) * BLADE_DEPTH;
    let n = DVec2::new(c, s) * t;
    vec![bottom + n, bottom + up + n, bottom + up - n, bottom - n]
        .into_iter()
        .map(mirror)
        .collect()
}

/// Is this box the one round a louvre's blade?
fn is_blade(b: &PartBox) -> bool {
    let (h, d) = blade_extent();
    b.part == Part::Glass
        && ((b.hi.y - b.lo.y) - d).abs() < 1e-9
        && ((b.hi.z - b.lo.z) - h).abs() < 1e-9
}

impl Shaped {
    pub fn part(&self) -> Part {
        match self {
            Shaped::Box(b) => b.part,
            Shaped::Prism(p) => p.part,
            Shaped::Slat(p) => p.part,
            Shaped::Plan(p) => p.part,
        }
    }
}

/// A corner window's part cut on the mitre at each end of it that turns a
/// corner (part 9): its plan outline, or None when it isn't cut. The
/// mitre runs at 45 degrees through the corner point (the window's end on
/// its wall's line) towards the side the other wall runs off to, so the
/// two windows' members, glass and sills meet on it without overlapping.
pub fn mitred(o: &Opening, wall: &Wall, b: &PartBox) -> Option<Vec<DVec2>> {
    let a = o.arrangement.filter(|_| o.kind == OpeningKind::Window)?;
    if b.part == Part::Trim {
        return None;
    }
    let w = Fitted::of(o, wall)?.width();
    // each cut keeps where it is >= 0: the line through the corner point
    // halving the angle between this wall's body and the other's (+y is
    // the wall's left in opening coordinates)
    let cut = |end: bool| -> (f64, f64, f64) {
        let (c, body) = if end {
            (DVec2::new(w, 0.0), DVec2::NEG_X)
        } else {
            (DVec2::ZERO, DVec2::X)
        };
        let ang = a.angle_at(end);
        let other = DVec2::new(ang.cos(), ang.sin());
        let half = body + other;
        let mut nrm = if half.length() < 1e-6 {
            body
        } else {
            half.normalize().perp()
        };
        if nrm.dot(body) < 0.0 {
            nrm = -nrm;
        }
        (nrm.x, nrm.y, -nrm.dot(c))
    };
    let mut cuts: Vec<(f64, f64, f64)> = vec![];
    if a.corner_end && b.hi.x > w + 1e-6 {
        cuts.push(cut(true));
    }
    if a.corner_start && b.lo.x < -1e-6 {
        cuts.push(cut(false));
    }
    if cuts.is_empty() {
        return None;
    }
    let mut poly = vec![
        DVec2::new(b.lo.x, b.lo.y),
        DVec2::new(b.hi.x, b.lo.y),
        DVec2::new(b.hi.x, b.hi.y),
        DVec2::new(b.lo.x, b.hi.y),
    ];
    for (cx, cy, c0) in cuts {
        let f = |p: DVec2| cx * p.x + cy * p.y + c0;
        let mut next = vec![];
        for k in 0..poly.len() {
            let (p, q) = (poly[k], poly[(k + 1) % poly.len()]);
            let (fp, fq) = (f(p), f(q));
            if fp >= 0.0 {
                next.push(p);
            }
            if (fp >= 0.0) != (fq >= 0.0) {
                next.push(p + (q - p) * (fp / (fp - fq)));
            }
        }
        next.dedup_by(|q, p| q.distance(*p) < 1e-9);
        if next.len() > 1 && next[0].distance(next[next.len() - 1]) < 1e-9 {
            next.pop();
        }
        poly = next;
    }
    Some(poly)
}

/// Every part of a door or window as built: [`opening_boxes`], with an
/// arched or raked head's frame following its curve and everything
/// under it cut to the frame's inside.
pub fn opening_parts(o: &Opening, wall: &Wall) -> Vec<Shaped> {
    let boxes = opening_boxes(o, wall);
    let Some(f) = Fitted::of(o, wall) else {
        return vec![];
    };
    let outline = f.frame.grid.outline;
    // the outside's side: built facing +y, turned round when it faces the
    // other way (MU105: the blades shed rain into the room)
    let out_y = if o.faces_left(wall) { 1.0 } else { -1.0 };
    let slat = |b: &PartBox| {
        Shaped::Slat(PartSlat {
            part: b.part,
            outline: blade_outline(b.lo.y, b.hi.y, b.lo.z, out_y),
            x0: b.lo.x,
            x1: b.hi.x,
        })
    };
    if outline.is_rectangle() {
        return boxes
            .into_iter()
            .filter_map(|b| {
                if is_blade(&b) {
                    return Some(slat(&b));
                }
                // round a corner, cut on the mitre (or nothing left)
                match mitred(o, wall, &b) {
                    Some(pts) if pts.len() >= 3 => Some(Shaped::Plan(PartPlan {
                        part: b.part,
                        outline: pts,
                        z0: b.lo.z,
                        z1: b.hi.z,
                    })),
                    Some(_) => None,
                    None => Some(Shaped::Box(b)),
                }
            })
            .collect();
    }
    let (w, h, fw) = (f.width(), f.height(), f.frame.width);
    let inner = outline.head(w, h, fw);
    let outer = outline.head(w, h, 0.0);
    let g = |x: f64| creabim_core::cells::head_z_on(&inner, x);
    let mut out = vec![];
    let mut head_y = None;
    for b in boxes {
        if b.part == Part::Trim {
            out.push(Shaped::Box(b));
            continue;
        }
        // a blade under the curve stays whole if it fits, else goes
        if is_blade(&b) {
            if g(b.lo.x).min(g(b.hi.x)) >= b.hi.z - 1e-9 {
                out.push(slat(&b));
            }
            continue;
        }
        // the square head's members give way to the shaped one
        if b.part == Part::Frame && b.hi.z >= h - 1e-6 && b.lo.z >= h - fw - 1e-6 {
            head_y.get_or_insert((b.lo.y, b.hi.y));
            continue;
        }
        let (x0, x1) = (b.lo.x, b.hi.x);
        // the curve's corners over it, and where it crosses the box's top
        let mut xs: Vec<f64> = vec![x0, x1];
        xs.extend(inner.iter().map(|p| p.0).filter(|x| *x > x0 && *x < x1));
        xs.sort_by(f64::total_cmp);
        let mut cross = vec![];
        for p in xs.windows(2) {
            let (ga, gb) = (g(p[0]) - b.hi.z, g(p[1]) - b.hi.z);
            if ga * gb < 0.0 {
                cross.push(p[0] + (p[1] - p[0]) * ga / (ga - gb));
            }
        }
        xs.extend(cross);
        xs.sort_by(f64::total_cmp);
        if xs.iter().all(|x| g(*x) >= b.hi.z - 1e-9) {
            out.push(Shaped::Box(b));
            continue;
        }
        if xs.iter().all(|x| g(*x) <= b.lo.z + 1e-9) {
            continue;
        }
        let top = |x: f64| g(x).min(b.hi.z).max(b.lo.z);
        let mut pts = vec![DVec2::new(x0, b.lo.z), DVec2::new(x1, b.lo.z)];
        pts.extend(xs.iter().rev().map(|x| DVec2::new(*x, top(*x))));
        pts.dedup_by(|q, p| q.distance(*p) < 1e-9);
        if pts.len() > 2 && pts[0].distance(pts[pts.len() - 1]) < 1e-9 {
            pts.pop();
        }
        if pts.len() < 3 {
            continue;
        }
        let tris = (1..pts.len() - 1).map(|k| [0, k, k + 1]).collect();
        out.push(Shaped::Prism(PartPrism {
            part: b.part,
            outline: pts,
            tris,
            y0: b.lo.y,
            y1: b.hi.y,
        }));
    }
    // the head itself, from the outer curve down to the inner, sitting on
    // the jambs
    let (y0, y1) = head_y.unwrap_or((-f.frame.depth / 2.0, f.frame.depth / 2.0));
    let n = outer.len().min(inner.len());
    if n >= 2 {
        let (jl, jr) = (inner[0].1, inner[inner.len() - 1].1);
        let mut pts = vec![DVec2::new(0.0, jl)];
        pts.extend(outer.iter().map(|p| DVec2::new(p.0, p.1)));
        pts.push(DVec2::new(w, jr));
        pts.extend(inner.iter().rev().map(|p| DVec2::new(p.0, p.1)));
        let (o0, i_end) = (1, pts.len() - 1);
        let on = |k: usize| o0 + k;
        // inner[k] sits at i_end - k
        let inn = |k: usize| i_end - k;
        let mut tris = vec![[0, on(0), inn(0)], [on(n - 1), on(n - 1) + 1, inn(n - 1)]];
        for k in 0..n - 1 {
            tris.push([on(k), on(k + 1), inn(k + 1)]);
            tris.push([on(k), inn(k + 1), inn(k)]);
        }
        out.push(Shaped::Prism(PartPrism {
            part: Part::Frame,
            outline: pts,
            tris,
            y0,
            y1,
        }));
    }
    out
}

/// Glass thickness, kept thin but solid so drawings show it.
const GLASS: f64 = 0.02;

/// An all glass leaf's thickness: 12 mm toughened glass.
pub const ALL_GLASS: f64 = 0.012;

/// The opening as built: the rectangle it fills in the wall (clipped to
/// the wall) and its frame fitted to that size.
#[derive(Clone, Debug)]
pub struct Fitted {
    pub cut: Cut,
    pub frame: OpeningFrame,
}

impl Fitted {
    pub fn of(o: &Opening, wall: &Wall) -> Option<Fitted> {
        let cut = Cut::of(o, wall)?;
        let mut frame = o
            .frame
            .fitted(cut.u1 - cut.u0, cut.v1 - cut.v0, wall.thickness);
        // a window flipped by hand is its grid seen from the other side:
        // its sashes hinged on their other jambs, in plan and in 3D
        if o.kind == OpeningKind::Window && o.flip_hand {
            frame.grid = frame.grid.mirrored();
        }
        Some(Fitted { cut, frame })
    }

    pub fn width(&self) -> f64 {
        self.cut.u1 - self.cut.u0
    }

    pub fn height(&self) -> f64 {
        self.cut.v1 - self.cut.v0
    }

    /// A door leaf's thickness.
    pub fn leaf(&self) -> f64 {
        if self.frame.construction.leaf == creabim_core::construction::LeafStyle::AllGlass {
            return ALL_GLASS;
        }
        (self.frame.depth * 0.6).min(0.045)
    }
}

/// Every part of a door or window, in opening coordinates: what fills
/// the opening, and the trim round it.
pub fn opening_boxes(o: &Opening, wall: &Wall) -> Vec<PartBox> {
    let mut out = filling_boxes(o, wall);
    if !out.is_empty() {
        out.extend(trim_boxes(o, wall));
        corner_ends(o, wall, &mut out);
    }
    out
}

/// A window round a corner (doors and windows part 9), at each end of it
/// that reaches into the corner: its trims stop at that end, its outside
/// sill runs on round the corner to meet the other window's, and with a
/// glass joint its jamb there goes and its head, sill and glass run to
/// the end, to meet the other window's glass. With a post the two jambs
/// make the corner post.
fn corner_ends(o: &Opening, wall: &Wall, out: &mut Vec<PartBox>) {
    let Some(a) = o.arrangement.filter(|_| o.kind == OpeningKind::Window) else {
        return;
    };
    let Some(f) = Fitted::of(o, wall) else {
        return;
    };
    let (w, h, fw) = (f.width(), f.height(), f.frame.width);
    let near = |a: f64, b: f64| (a - b).abs() < 1e-6;
    for end in [false, true] {
        if !a.corner_at(end) {
            continue;
        }
        // worked at x = w: the start's corner mirrored there and back
        let mirror = |out: &mut [PartBox]| {
            if !end {
                for b in out.iter_mut() {
                    (b.lo.x, b.hi.x) = (w - b.hi.x, w - b.lo.x);
                }
            }
        };
        mirror(out);
        for b in out.iter_mut() {
            let gone = match b.part {
                // the lining and architrave up that side
                Part::Trim => b.lo.x >= w - 0.1,
                // the jamb, where the glass meets glass
                Part::Frame => {
                    a.joint == creabim_core::CornerJoint::Glass
                        && near(b.lo.x, w - fw)
                        && near(b.hi.x, w)
                        && b.lo.z < 1e-6
                        && near(b.hi.z, h)
                }
                _ => false,
            };
            if gone {
                // made empty here, dropped below
                b.hi = b.lo;
                continue;
            }
            match b.part {
                // the head's and sill's trims stop at the end
                Part::Trim if b.hi.x > w - 0.1 => b.hi.x = w,
                // head, sill member, glass and sashes to the end, glass
                // to glass
                _ if a.joint == creabim_core::CornerJoint::Glass && near(b.hi.x, w - fw) => {
                    b.hi.x = w
                }
                _ => {}
            }
            // what reaches the end runs on to the mitre, where `mitred`
            // cuts it: the jambs into one post, the glass and the outside
            // sill round the corner
            if b.part != Part::Trim && b.hi.x >= w - 1e-6 {
                b.hi.x = w + b.lo.y.abs().max(b.hi.y.abs()) + 1e-4;
            }
        }
        mirror(out);
    }
    out.retain(|b| (b.hi - b.lo).min_element() > 1e-6);
}

/// The lining, architrave and threshold of a door or window, on the
/// room side of its wall. Round an arched or raked head they stop where
/// it springs.
pub fn trim_boxes(o: &Opening, wall: &Wall) -> Vec<PartBox> {
    use creabim_core::construction::ARCHITRAVE_THICKNESS;
    let Some(f) = Fitted::of(o, wall) else {
        return vec![];
    };
    let trim = f.frame.construction.trim;
    if o.kind == OpeningKind::Hole || trim.is_none() {
        return vec![];
    }
    let (w, h) = (f.width(), f.height());
    let (fw, hd, face) = (f.frame.width, f.frame.depth / 2.0, wall.thickness / 2.0);
    let window = o.kind == OpeningKind::Window;
    let outline = f.frame.grid.outline;
    let square = outline.is_rectangle();
    let (s0, s1) = outline.springs(w, h);
    let b = DVec3::new;
    let mut out = vec![];
    let mut add = |lo: DVec3, hi: DVec3| {
        if (hi - lo).min_element() > 1e-6 {
            out.push(PartBox {
                part: Part::Trim,
                lo,
                hi,
            });
        }
    };
    // built on the +y side, turned to the room side below
    let l = trim.lining;
    if l > 0.0 && face > hd + 1e-6 {
        add(b(0.0, hd, 0.0), b(l, face, s0));
        add(b(w - l, hd, 0.0), b(w, face, s1));
        if square {
            add(b(l, hd, h - l), b(w - l, face, h));
        }
        if window {
            add(b(l, hd, 0.0), b(w - l, face, l));
        }
    }
    let a = trim.architrave;
    if a > 0.0 {
        let (y0, y1) = (face, face + ARCHITRAVE_THICKNESS);
        let z0 = if window { -a } else { 0.0 };
        add(b(-a, y0, z0), b(0.0, y1, s0));
        add(b(w, y0, z0), b(w + a, y1, s1));
        if square {
            add(b(-a, y0, h), b(w + a, y1, h + a));
        }
        if window {
            add(b(0.0, y0, -a), b(w, y1, 0.0));
        }
    }
    if trim.threshold > 0.0 && o.kind == OpeningKind::Door {
        add(b(fw, -face, 0.0), b(w - fw, face, trim.threshold));
    }
    // the room side: the wall's left unless it is flipped
    if wall.flip {
        for pb in &mut out {
            (pb.lo.y, pb.hi.y) = (-pb.hi.y, -pb.lo.y);
        }
    }
    out
}

/// What fills a door or window's opening, in opening coordinates.
fn filling_boxes(o: &Opening, wall: &Wall) -> Vec<PartBox> {
    let Some(f) = Fitted::of(o, wall) else {
        return vec![];
    };
    // a bare hole has nothing in it
    if o.kind == OpeningKind::Hole {
        return vec![];
    }
    if let Some((core, a)) = door_core(o, wall) {
        return assembly_boxes(&f, o, &core, &a, wall);
    }
    let (w, h) = (f.width(), f.height());
    let fw = f.frame.width;
    let hd = f.frame.depth / 2.0;
    let face = wall.thickness / 2.0;
    let mut out = Vec::new();
    let mut add = |part, lo: DVec3, hi: DVec3| {
        if (hi - lo).min_element() > 1e-6 {
            out.push(PartBox { part, lo, hi });
        }
    };
    let b = DVec3::new;
    let op = if o.kind == OpeningKind::Door {
        f.frame.operation(o.kind)
    } else {
        Operation::Standard
    };
    // a pocket door's leaf runs into the wall past its jamb: no jamb
    // there (built sliding towards x = 0)
    let pocket_lo = op.pocket();
    let pocket_hi = op == Operation::PocketPair;
    // jambs and head, round every opening
    if !pocket_lo {
        add(Part::Frame, b(0.0, -hd, 0.0), b(fw, hd, h));
    }
    if !pocket_hi {
        add(Part::Frame, b(w - fw, -hd, 0.0), b(w, hd, h));
    }
    let (h0, h1) = (
        if pocket_lo { 0.0 } else { fw },
        if pocket_hi { w } else { w - fw },
    );
    add(Part::Frame, b(h0, -hd, h - fw), b(h1, hd, h));
    match o.kind {
        OpeningKind::Window => {
            // the sill member, then glazing bars splitting the glass
            add(Part::Frame, b(fw, -hd, 0.0), b(w - fw, hd, fw));
            // the outside sill, from the frame out past the wall's face,
            // on the side the window faces out of (built facing +y)
            if face > hd + 0.005 {
                add(Part::Frame, b(0.0, hd, 0.0), b(w, face + SILL_NOSE, 0.03));
            }
            // its mullions and transoms, then each cell of its grid:
            // glass, or a sash that opens
            for (part, lo, hi) in grid_bars(&f, None) {
                add(part, lo, hi);
            }
            let sliding = f.frame.grid.cells.iter().any(|c| {
                matches!(
                    c.mechanism,
                    creabim_core::cells::Mechanism::Sliding
                        | creabim_core::cells::Mechanism::VerticalSliding
                )
            });
            for c in window_cells(&f) {
                for (part, lo, hi) in cell_parts(&f, &c, sliding) {
                    add(part, lo, hi);
                }
            }
        }
        OpeningKind::Door if op.slides() => {
            for (part, lo, hi) in sliding_parts(&f, op, face) {
                add(part, lo, hi);
            }
        }
        OpeningKind::Door if op.lifts() || op == Operation::Revolving => {
            for (part, lo, hi) in special_parts(&f, op, face) {
                add(part, lo, hi);
            }
        }
        OpeningKind::Door => {
            // the leaves, flush with the frame's face on the side they open
            // to (a pivot or double action leaf in the middle of the
            // frame): one, a pair split at the active leaf's share, or a
            // folding door's equal panels
            let gap = 0.004;
            let lt = f.leaf();
            let (c0, c1) = (fw, w - fw);
            let spans = leaf_spans(
                op,
                c0,
                c1,
                gap,
                f.frame.active_share(),
                f.frame.core().cells.len(),
            );
            let (y0, y1) = if op.pivots() || op.double_action() {
                (-lt / 2.0, lt / 2.0)
            } else {
                (hd - lt, hd)
            };
            // over a threshold, a little clear of it
            let foot = 0.01f64.max(f.frame.construction.trim.threshold + 0.005);
            for &(l0, l1) in &spans {
                // hung at the end nearer its side of the frame
                let start = l0 - c0 <= c1 - l1;
                for (part, lo, hi) in leaf_parts(
                    &f.frame.construction,
                    b(l0, y0, foot),
                    b(l1, y1, h - fw - gap),
                    start,
                ) {
                    add(part, lo, hi);
                }
            }
            // a lever handle on each face of the active leaf, on the side
            // away from its hinge (a double action leaf has push plates)
            let (x, wide) = handle_at(op, &spans);
            let z = 1.0f64.min(h * 0.45);
            if wide > 0.3
                && h > 1.2
                && f.frame.construction.leaf == creabim_core::construction::LeafStyle::AllGlass
            {
                // a long pull handle on each face, standing off the glass:
                // on each leaf of a pair, near where they meet
                let at: Vec<f64> = match spans.as_slice() {
                    [(_, a1), (b0, _)] if op.pair() => vec![a1 - 0.1, b0 + 0.1],
                    _ => vec![x - 0.03],
                };
                for x in at {
                    for p in pull_handle(x, y0, y1, h) {
                        add(p.0, p.1, p.2);
                    }
                }
            } else if wide > 0.3 && h > 1.2 && op.double_action() {
                for (r0, r1) in [(y1, y1 + 0.003), (y0 - 0.003, y0)] {
                    add(
                        Part::Handle,
                        b(x - 0.05, r0, z - 0.15),
                        b(x + 0.05, r1, z + 0.15),
                    );
                }
            } else if wide > 0.3 && h > 1.2 {
                let (f0, f1) = (y0, y1);
                for (r0, r1, a0, a1) in [
                    (f1, f1 + 0.012, f1 + 0.045, f1 + 0.062),
                    (f0 - 0.012, f0, f0 - 0.062, f0 - 0.045),
                ] {
                    add(
                        Part::Handle,
                        b(x - 0.025, r0, z - 0.04),
                        b(x + 0.025, r1, z + 0.04),
                    );
                    add(
                        Part::Handle,
                        b(x - 0.008, r0.min(a0), z - 0.008),
                        b(x + 0.008, r1.max(a1), z + 0.008),
                    );
                    add(
                        Part::Handle,
                        b(x - 0.13, a0, z - 0.01),
                        b(x + 0.01, a1, z + 0.01),
                    );
                }
            }
        }
        OpeningKind::Hole => {}
    }
    // built opening to (facing out of) +y, a door hinged at x = 0: turn it
    // round as the opening is set
    let across = !o.faces_left(wall);
    let along = o.kind == OpeningKind::Door && o.flip_hand;
    if across || along {
        for pb in &mut out {
            let (lo, hi) = (pb.lo, pb.hi);
            if along {
                (pb.lo.x, pb.hi.x) = (w - hi.x, w - lo.x);
            }
            if across {
                (pb.lo.y, pb.hi.y) = (-hi.y, -lo.y);
            }
        }
    }
    out
}

/// Where a hung door's leaves run along the clear width `c0`..`c1`, as
/// built (hinged at the start): one leaf, a pair split at the active
/// leaf's share, or a folding door's equal panels.
fn leaf_spans(
    op: Operation,
    c0: f64,
    c1: f64,
    gap: f64,
    share: f64,
    panels: usize,
) -> Vec<(f64, f64)> {
    if op.pair() {
        let m = c0 + (c1 - c0) * share;
        return vec![(c0 + gap, m - gap / 2.0), (m + gap / 2.0, c1 - gap)];
    }
    if !op.folds() {
        return vec![(c0 + gap, c1 - gap)];
    }
    // a folding door's panels: as many as its cells, equal, a hinge gap
    // between each
    let n = panels.max(2);
    let step = (c1 - c0) / n as f64;
    (0..n)
        .map(|i| {
            let a = c0 + step * i as f64;
            (
                a + if i == 0 { gap } else { gap / 2.0 },
                a + step - if i + 1 == n { gap } else { gap / 2.0 },
            )
        })
        .collect()
}

/// The parts of a sectional, roller or revolving door, as built (the
/// side it opens to, or lies on open, is +y).
fn special_parts(f: &Fitted, op: Operation, face: f64) -> Vec<(Part, DVec3, DVec3)> {
    let (w, h) = (f.width(), f.height());
    let fw = f.frame.width;
    let hd = f.frame.depth / 2.0;
    let b = DVec3::new;
    let mut out = vec![];
    let top = h - fw;
    match op {
        Operation::Sectional => {
            // panels about half a metre high, 40 mm thick, on the inside
            // face of the frame, lapping the jambs
            let n = (top / 0.5).round().max(3.0) as usize;
            let step = top / n as f64;
            let (y0, y1) = (hd, hd + 0.04);
            for k in 0..n {
                let z0 = step * k as f64 + if k == 0 { 0.005 } else { 0.002 };
                let z1 = step * (k + 1) as f64 - 0.002;
                out.push((Part::Leaf, b(fw * 0.5, y0, z0), b(w - fw * 0.5, y1, z1)));
            }
            // a pull handle on the bottom panel, inside
            let xm = w / 2.0;
            out.push((
                Part::Handle,
                b(xm - 0.08, y1, 0.15),
                b(xm + 0.08, y1 + 0.03, 0.18),
            ));
        }
        Operation::Roller => {
            // 75 mm slats in guides on the jambs, rolled up into a coil
            // box over the opening on the side it lies
            let n = (top / 0.075).floor().max(2.0) as usize;
            let step = top / n as f64;
            let (y0, y1) = (hd - 0.015, hd);
            for k in 0..n {
                let z0 = step * k as f64 + 0.001;
                out.push((Part::Leaf, b(fw, y0, z0), b(w - fw, y1, z0 + step - 0.002)));
            }
            let d = 0.3f64.min(top / 3.0);
            out.push((
                Part::Frame,
                b(0.0, face.max(hd), h),
                b(w, face.max(hd) + d, h + d),
            ));
        }
        Operation::Revolving => {
            // a post in the middle and four glazed wings, two across the
            // opening and two square to it, turning in a drum the width
            // of the opening
            let (c0, c1) = (fw, w - fw);
            let (cx, r) = ((c0 + c1) / 2.0, (c1 - c0) / 2.0);
            let (p, t, rail) = (0.03, 0.02, 0.1f64.min(top / 6.0));
            out.push((Part::Frame, b(cx - p, -p, 0.0), b(cx + p, p, top)));
            let mut wing = |lo: (f64, f64), hi: (f64, f64)| {
                let (x0, y0, x1, y1) = (lo.0, lo.1, hi.0, hi.1);
                out.push((Part::Leaf, b(x0, y0, 0.01), b(x1, y1, 0.01 + rail)));
                out.push((Part::Leaf, b(x0, y0, top - rail), b(x1, y1, top - 0.005)));
                out.push((Part::Glass, b(x0, y0, 0.01 + rail), b(x1, y1, top - rail)));
            };
            let (a, e) = (p + 0.005, r - 0.01);
            wing((cx + a, -t), (cx + e, t));
            wing((cx - e, -t), (cx - a, t));
            wing((cx - t, a), (cx + t, e));
            wing((cx - t, -e), (cx + t, -a));
        }
        _ => {}
    }
    out
}

/// Where along a door its handle goes, as built, and the width of the
/// leaf it is on: near the free edge of the active leaf.
fn handle_at(op: Operation, spans: &[(f64, f64)]) -> (f64, f64) {
    let Some(&(l0, l1)) = spans.first() else {
        return (0.0, 0.0);
    };
    match op {
        Operation::Bifold | Operation::Folding => {
            let (a, b) = spans[spans.len() - 1];
            (b - 0.07, b - a)
        }
        Operation::BifoldPair if spans.len() == 4 => {
            let (a, b) = spans[1];
            (b - 0.07, b - a)
        }
        _ => (l1 - 0.07, l1 - l0),
    }
}

/// A long pull handle on both faces of a glass leaf whose faces are at
/// `y0` and `y1`, centred along at `x`: an upright bar on two stand-offs,
/// 1200 mm long about handle height (shorter on a low leaf).
pub fn pull_handle(x: f64, y0: f64, y1: f64, h: f64) -> Vec<(Part, DVec3, DVec3)> {
    let b = DVec3::new;
    let (z0, z1) = (0.4f64.max(h * 0.2), (h - 0.3).min(1.6));
    if z1 - z0 < 0.3 {
        return vec![];
    }
    let mut out = vec![];
    for (face, dir) in [(y1, 1.0), (y0, -1.0)] {
        let (a, c) = (face + dir * 0.05, face + dir * 0.075);
        out.push((
            Part::Handle,
            b(x - 0.0125, a.min(c), z0),
            b(x + 0.0125, a.max(c), z1),
        ));
        for z in [z0 + 0.1, z1 - 0.1] {
            let (a, c) = (face, face + dir * 0.05);
            out.push((
                Part::Handle,
                b(x - 0.008, a.min(c), z - 0.008),
                b(x + 0.008, a.max(c), z + 0.008),
            ));
        }
    }
    out
}

/// A door leaf filling the box `lo`..`hi` (its thickness along y) as
/// `c` makes it: one slab, stiles and rails round panels, glass, panes or
/// slats, or a sheet of glass in its fittings. `start`: it is hung at its
/// start (x = lo.x), where an all glass leaf's single top patch goes.
pub fn leaf_parts(
    c: &creabim_core::construction::Construction,
    lo: DVec3,
    hi: DVec3,
    start: bool,
) -> Vec<(Part, DVec3, DVec3)> {
    use creabim_core::construction::{GlassFittings as G, LeafStyle as L};
    let style = c.leaf;
    let b = DVec3::new;
    let (w, h, t) = (hi.x - lo.x, hi.z - lo.z, hi.y - lo.y);
    let mut out = vec![];
    if style == L::Flush || w < 0.3 || h < 0.6 {
        return vec![(Part::Leaf, lo, hi)];
    }
    let ym = (lo.y + hi.y) / 2.0;
    // a member the whole thickness, and a panel, glass or slat sunk in it
    let mut member = |x0: f64, x1: f64, z0: f64, z1: f64| {
        out.push((Part::Leaf, b(x0, lo.y, z0), b(x1, hi.y, z1)));
    };
    if style == L::VisionPanel {
        // a slab round a narrow upright light, about eye height
        let (gw, z0, z1) = (
            0.15f64.min(w / 3.0),
            lo.z + 1.0f64.min(h * 0.5),
            lo.z + 1.7f64.min(h - 0.15),
        );
        let (x0, x1) = (lo.x + (w - gw) / 2.0, lo.x + (w + gw) / 2.0);
        if z1 - z0 < 0.2 {
            return vec![(Part::Leaf, lo, hi)];
        }
        member(lo.x, x0, lo.z, hi.z);
        member(x1, hi.x, lo.z, hi.z);
        member(x0, x1, lo.z, z0);
        member(x0, x1, z1, hi.z);
        out.push((
            Part::Glass,
            b(x0, ym - GLASS / 4.0, z0),
            b(x1, ym + GLASS / 4.0, z1),
        ));
        return out;
    }
    if style == L::AllGlass {
        // the glass the whole leaf, and its metal fittings clamped on it
        out.push((
            Part::Glass,
            b(lo.x, ym - t / 2.0, lo.z),
            b(hi.x, ym + t / 2.0, hi.z),
        ));
        let (f0, f1) = (ym - t / 2.0 - 0.012, ym + t / 2.0 + 0.012);
        let rail = 0.1f64.min(h / 8.0);
        let (pw, ph) = (0.18f64.min(w / 3.0), 0.06);
        let mut fit = |x0: f64, x1: f64, z0: f64, z1: f64| {
            out.push((Part::Handle, b(x0, f0, z0), b(x1, f1, z1)));
        };
        let patch_top = |fit: &mut dyn FnMut(f64, f64, f64, f64), at_start: bool| {
            if at_start {
                fit(lo.x, lo.x + pw, hi.z - ph, hi.z)
            } else {
                fit(hi.x - pw, hi.x, hi.z - ph, hi.z)
            }
        };
        match c.fittings {
            G::Patch => {
                for x in [lo.x, hi.x - pw] {
                    fit(x, x + pw, lo.z, lo.z + ph);
                    fit(x, x + pw, hi.z - ph, hi.z);
                }
            }
            G::BottomRail => {
                fit(lo.x, hi.x, lo.z, lo.z + rail);
                patch_top(&mut fit, start);
            }
            G::Rails => {
                fit(lo.x, hi.x, lo.z, lo.z + rail);
                fit(lo.x, hi.x, hi.z - rail, hi.z);
            }
        }
        return out;
    }
    if style.has_panes() {
        // stiles and rails, and bars between the panes, round one sheet
        let steel = style == L::SlimSteel;
        let (m, bottom, bar) = if steel {
            (0.04f64.min(w / 6.0), 0.06f64.min(h / 10.0), 0.02)
        } else {
            (0.1f64.min(w / 4.0), 0.2f64.min(h / 6.0), 0.025)
        };
        let (x0, x1) = (lo.x + m, hi.x - m);
        let (z0, z1) = (lo.z + bottom, hi.z - m);
        member(lo.x, x0, lo.z, hi.z);
        member(x1, hi.x, lo.z, hi.z);
        member(x0, x1, lo.z, z0);
        member(x0, x1, z1, hi.z);
        let [across, up] = c.panes_for(x1 - x0, z1 - z0);
        let d = t * 0.2;
        for k in 1..across {
            let x = x0 + (x1 - x0) * k as f64 / across as f64;
            out.push((
                Part::Leaf,
                b(x - bar / 2.0, lo.y + d, z0),
                b(x + bar / 2.0, hi.y - d, z1),
            ));
        }
        for k in 1..up {
            let z = z0 + (z1 - z0) * k as f64 / up as f64;
            out.push((
                Part::Leaf,
                b(x0, lo.y + d, z - bar / 2.0),
                b(x1, hi.y - d, z + bar / 2.0),
            ));
        }
        out.push((
            Part::Glass,
            b(x0, ym - GLASS / 4.0, z0),
            b(x1, ym + GLASS / 4.0, z1),
        ));
        return out;
    }
    let m = 0.1f64.min(w / 4.0);
    let bottom = 0.2f64.min(h / 6.0);
    let (x0, x1) = (lo.x + m, hi.x - m);
    let (z0, z1) = (lo.z + bottom, hi.z - m);
    member(lo.x, x0, lo.z, hi.z);
    member(x1, hi.x, lo.z, hi.z);
    member(x0, x1, lo.z, z0);
    member(x0, x1, z1, hi.z);
    let panel = |out: &mut Vec<(Part, DVec3, DVec3)>, a: f64, c: f64, za: f64, zc: f64| {
        let d = t * 0.3;
        out.push((Part::Leaf, b(a, lo.y + d, za), b(c, hi.y - d, zc)));
    };
    let glass = |out: &mut Vec<(Part, DVec3, DVec3)>, za: f64, zc: f64| {
        out.push((
            Part::Glass,
            b(x0, ym - GLASS / 4.0, za),
            b(x1, ym + GLASS / 4.0, zc),
        ));
    };
    match style {
        L::Glazed => glass(&mut out, z0, z1),
        L::Panelled => {
            // a lock rail across, a muntin up the middle: four panels
            let zm = lo.z + h * 0.42;
            let xm = (x0 + x1) / 2.0;
            let mw = m * 0.8;
            out.push((
                Part::Leaf,
                b(x0, lo.y, zm - m / 2.0),
                b(x1, hi.y, zm + m / 2.0),
            ));
            out.push((
                Part::Leaf,
                b(xm - mw / 2.0, lo.y, z0),
                b(xm + mw / 2.0, hi.y, zm - m / 2.0),
            ));
            out.push((
                Part::Leaf,
                b(xm - mw / 2.0, lo.y, zm + m / 2.0),
                b(xm + mw / 2.0, hi.y, z1),
            ));
            for (za, zc) in [(z0, zm - m / 2.0), (zm + m / 2.0, z1)] {
                panel(&mut out, x0, xm - mw / 2.0, za, zc);
                panel(&mut out, xm + mw / 2.0, x1, za, zc);
            }
        }
        L::PartGlazed => {
            let zm = lo.z + h * 0.45;
            out.push((
                Part::Leaf,
                b(x0, lo.y, zm - m / 2.0),
                b(x1, hi.y, zm + m / 2.0),
            ));
            panel(&mut out, x0, x1, z0, zm - m / 2.0);
            glass(&mut out, zm + m / 2.0, z1);
        }
        L::Louvred => {
            // slats 60 mm apart, a little thinner than the leaf
            let n = ((z1 - z0) / 0.06).floor().max(1.0) as usize;
            let step = (z1 - z0) / n as f64;
            let d = t * 0.15;
            for k in 0..n {
                let z = z0 + step * (k as f64 + 0.5);
                out.push((
                    Part::Leaf,
                    b(x0, lo.y + d, z - 0.01),
                    b(x1, hi.y - d, z + 0.01),
                ));
            }
        }
        L::Flush | L::VisionPanel | L::AllGlass | L::French | L::SlimSteel => unreachable!(),
    }
    out
}

/// How far a surface sliding leaf overlaps the opening at each side.
pub const SLIDE_OVERLAP: f64 = 0.05;

/// A sliding leaf or panel as it sits closed, in opening coordinates
/// before the door is turned to its hand and swing (built sliding towards
/// x = 0, on the +y side): its box, and how far it moves along x to open
/// (none for a fixed panel).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slide {
    pub leaf: PartBox,
    pub travel: f64,
    pub glazed: bool,
    /// The cell of the opening's grid it fills (`grid.cells[cell]`).
    pub cell: usize,
}

/// The sliding leaves or panels of a sliding door, built (see [`Slide`]).
fn slides_built(f: &Fitted, op: Operation, face: f64) -> Vec<Slide> {
    let (w, h) = (f.width(), f.height());
    let fw = f.frame.width;
    let lt = f.leaf();
    let gap = 0.004;
    let b = DVec3::new;
    let leaf = |lo: DVec3, hi: DVec3| PartBox {
        part: Part::Leaf,
        lo,
        hi,
    };
    let ov = SLIDE_OVERLAP;
    match op {
        Operation::SurfaceSliding | Operation::SurfaceSlidingPair => {
            // on the wall's face, just clear of it, rising past the head
            let (y0, y1) = (face + 0.012, face + 0.012 + lt);
            let top = h + 0.02;
            if op == Operation::SurfaceSliding {
                vec![Slide {
                    leaf: leaf(b(-ov, y0, 0.01), b(w + ov, y1, top)),
                    travel: -(w + ov),
                    glazed: false,
                    cell: 0,
                }]
            } else {
                let m = w / 2.0;
                vec![
                    Slide {
                        leaf: leaf(b(-ov, y0, 0.01), b(m - 0.002, y1, top)),
                        travel: -(m + ov),
                        glazed: false,
                        cell: 0,
                    },
                    Slide {
                        leaf: leaf(b(m + 0.002, y0, 0.01), b(w + ov, y1, top)),
                        travel: m + ov,
                        glazed: false,
                        cell: 1,
                    },
                ]
            }
        }
        Operation::Pocket | Operation::PocketPair => {
            // in the middle of the wall, closing on the jamb (or each other)
            let (y0, y1) = (-lt / 2.0, lt / 2.0);
            let top = h - fw - gap;
            if op == Operation::Pocket {
                vec![Slide {
                    leaf: leaf(b(0.0, y0, 0.01), b(w - fw - gap, y1, top)),
                    travel: -(w - fw - gap),
                    glazed: false,
                    cell: 0,
                }]
            } else {
                let m = w / 2.0;
                vec![
                    Slide {
                        leaf: leaf(b(0.0, y0, 0.01), b(m - 0.002, y1, top)),
                        travel: -(m - 0.002),
                        glazed: false,
                        cell: 0,
                    },
                    Slide {
                        leaf: leaf(b(m + 0.002, y0, 0.01), b(w, y1, top)),
                        travel: m - 0.002,
                        glazed: false,
                        cell: 1,
                    },
                ]
            }
        }
        Operation::MultiSlide => {
            // equal panels on two tracks, neighbours on different ones
            // so they overlap and pass
            let n = f.frame.panels().clamp(2, 4) as usize;
            let (c0, c1) = (fw, w - fw);
            let pw = (c1 - c0) / n as f64;
            let t = lt.min(f.frame.depth / 3.0);
            let fixed = |i: usize| f.frame.fixed() >> i & 1 == 1;
            (0..n)
                .map(|i| {
                    let y = if i % 2 == 0 { -t * 0.55 } else { t * 0.55 };
                    let lo = if i > 0 { c0 + pw * i as f64 - 0.02 } else { c0 };
                    let hi = if i + 1 < n {
                        c0 + pw * (i + 1) as f64 + 0.02
                    } else {
                        c1
                    };
                    // a sliding panel moves over its fixed neighbour, or
                    // else towards the opening's far end
                    let towards = if i > 0 && fixed(i - 1) {
                        -1.0
                    } else if (i + 1 < n && fixed(i + 1)) || (lo + hi) / 2.0 < w / 2.0 {
                        1.0
                    } else {
                        -1.0
                    };
                    Slide {
                        leaf: leaf(b(lo, y - t / 2.0, 0.02), b(hi, y + t / 2.0, h - fw)),
                        travel: if fixed(i) { 0.0 } else { towards * (pw - 0.04) },
                        glazed: true,
                        cell: i,
                    }
                })
                .collect()
        }
        _ => vec![],
    }
}

/// The parts of a sliding door inside its frame: its leaves (glazed
/// panels in a frame for a multi-panel slider), a track on the wall for
/// a surface slider, a flush pull on each leaf.
fn sliding_parts(f: &Fitted, op: Operation, face: f64) -> Vec<(Part, DVec3, DVec3)> {
    let (w, h) = (f.width(), f.height());
    let b = DVec3::new;
    let mut out = vec![];
    let slides = slides_built(f, op, face);
    for s in &slides {
        let (lo, hi) = (s.leaf.lo, s.leaf.hi);
        if s.glazed {
            // stiles and rails round the glass
            let m = 0.06f64.min((hi.x - lo.x) / 4.0);
            let gy = (lo.y + hi.y) / 2.0;
            out.push((Part::Leaf, lo, b(lo.x + m, hi.y, hi.z)));
            out.push((Part::Leaf, b(hi.x - m, lo.y, lo.z), hi));
            out.push((
                Part::Leaf,
                b(lo.x + m, lo.y, lo.z),
                b(hi.x - m, hi.y, lo.z + m * 1.5),
            ));
            out.push((
                Part::Leaf,
                b(lo.x + m, lo.y, hi.z - m),
                b(hi.x - m, hi.y, hi.z),
            ));
            out.push((
                Part::Glass,
                b(lo.x + m, gy - GLASS / 4.0, lo.z + m * 1.5),
                b(hi.x - m, gy + GLASS / 4.0, hi.z - m),
            ));
        } else {
            out.extend(leaf_parts(&f.frame.construction, lo, hi, true));
        }
        // a flush pull near the leading edge, on both faces
        if s.travel != 0.0 && h > 1.2 {
            let x = if s.travel < 0.0 {
                hi.x - 0.08
            } else {
                lo.x + 0.08
            };
            let z = 1.0f64.min(h * 0.45);
            for (y0, y1) in [(hi.y, hi.y + 0.006), (lo.y - 0.006, lo.y)] {
                out.push((
                    Part::Handle,
                    b(x - 0.015, y0, z - 0.1),
                    b(x + 0.015, y1, z + 0.1),
                ));
            }
        }
    }
    match op {
        // the track above, on the wall's face, as long as the leaves run
        Operation::SurfaceSliding | Operation::SurfaceSlidingPair => {
            let lo = slides
                .iter()
                .map(|s| s.leaf.lo.x + s.travel.min(0.0))
                .fold(f64::INFINITY, f64::min);
            let hi = slides
                .iter()
                .map(|s| s.leaf.hi.x + s.travel.max(0.0))
                .fold(f64::NEG_INFINITY, f64::max);
            let y1 = slides[0].leaf.hi.y + 0.012;
            out.push((Part::Handle, b(lo, face, h + 0.02), b(hi, y1, h + 0.08)));
        }
        // a sill track under the panels
        Operation::MultiSlide => {
            let hd = f.frame.depth / 2.0;
            out.push((
                Part::Frame,
                b(f.frame.width, -hd, 0.0),
                b(w - f.frame.width, hd, 0.02),
            ));
        }
        _ => {}
    }
    out
}

/// A sliding door's leaves or panels as built in its opening, turned to
/// its hand and swing as [`opening_boxes`] turns its parts: each box
/// closed, and the distance it moves along the opening to open (negative
/// towards the opening's start; 0 for a fixed panel).
pub fn sliding_leaves(o: &Opening, wall: &Wall) -> Vec<Slide> {
    // an assembly's door slides in its own part of the opening
    if let Some((core, a)) = door_core(o, wall) {
        let dx = core_dx(o, &core, wall);
        return sliding_leaves(&core, wall)
            .into_iter()
            .map(|mut s| {
                s.leaf.lo.x += dx;
                s.leaf.hi.x += dx;
                s.cell += a.columns.0;
                s
            })
            .collect();
    }
    if o.kind != OpeningKind::Door || !o.frame.operation(o.kind).slides() {
        return vec![];
    }
    let Some(f) = Fitted::of(o, wall) else {
        return vec![];
    };
    let w = f.width();
    let across = !o.faces_left(wall);
    let along = o.flip_hand;
    slides_built(&f, f.frame.operation(o.kind), wall.thickness / 2.0)
        .into_iter()
        .map(|mut s| {
            let (lo, hi) = (s.leaf.lo, s.leaf.hi);
            if along {
                (s.leaf.lo.x, s.leaf.hi.x) = (w - hi.x, w - lo.x);
                s.travel = -s.travel;
            }
            if across {
                (s.leaf.lo.y, s.leaf.hi.y) = (-hi.y, -lo.y);
            }
            s
        })
        .collect()
}

/// The cavities a pocket door needs in its wall, beside its opening:
/// each as the stretch of wall it runs along and up (wall coordinates)
/// and how far either side of the wall's centre line it reaches across.
pub fn pockets(o: &Opening, wall: &Wall) -> Vec<(Cut, (f64, f64))> {
    if let Some((core, _)) = door_core(o, wall) {
        return pockets(&core, wall);
    }
    let op = o.frame.operation(o.kind);
    if o.kind != OpeningKind::Door || !op.pocket() {
        return vec![];
    }
    let Some(f) = Fitted::of(o, wall) else {
        return vec![];
    };
    let half = f.leaf() / 2.0 + POCKET_PLAY;
    let t = wall.thickness / 2.0;
    // too thin a wall to hold a leaf with a skin each side
    if half > t - POCKET_SKIN {
        return vec![];
    }
    let c = f.cut;
    let len = wall.length();
    let v1 = c.v1 - f.frame.width;
    let mut out = vec![];
    for s in sliding_leaves(o, wall) {
        let (u0, u1) = if s.travel < 0.0 {
            (c.u0 + s.travel - POCKET_SKIN, c.u0 + 0.001)
        } else {
            (c.u1 - 0.001, c.u1 + s.travel + POCKET_SKIN)
        };
        let (u0, u1) = (u0.max(0.0), u1.min(len));
        if u1 > u0 + 1e-6 {
            out.push((
                Cut {
                    u0,
                    u1,
                    v0: c.v0,
                    v1,
                },
                (-half, half),
            ));
        }
    }
    out
}

/// What keeps a pocket door from working where it stands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PocketProblem {
    /// The wall cannot hold the leaf with a skin each side: the least
    /// thickness that can.
    ThinWall { needs: f64 },
    /// The leaf is flat and the wall curved: it cannot slide into it.
    CurvedWall,
    /// The pocket would run past the wall's end by this much.
    PastEnd { short: f64 },
    /// The pocket runs into another door, window or hole in the wall.
    MeetsOpening,
}

/// The skin left each side of a pocket's cavity, at least.
const POCKET_SKIN: f64 = 0.02;
/// The play each side of the leaf in its cavity.
const POCKET_PLAY: f64 = 0.012;

/// The cells of an opening's grid that slide into a pocket in the wall.
pub fn pocket_cells(o: &Opening) -> Vec<usize> {
    if o.kind != OpeningKind::Door {
        return vec![];
    }
    let g = &o.frame.grid;
    (0..g.cells.len())
        .filter(|&i| g.cells[i].mechanism == creabim_core::cells::Mechanism::Pocket)
        .collect()
}

/// What is wrong with one pocket leaf where it stands, if anything: the
/// wall too thin or curved, no room to the wall's end, or another opening
/// in the way (`cell`: the grid cell that slides, see [`pocket_cells`];
/// `neighbours`: the wall's other openings; one that overlaps this door's
/// own opening is not counted, being a different trouble). A cell that
/// does not slide into a pocket has none. It only reports: [`pockets`]
/// still cuts what cavity there is room for.
pub fn pocket_problems(
    o: &Opening,
    cell: usize,
    wall: &Wall,
    neighbours: &[Opening],
) -> Vec<PocketProblem> {
    if !pocket_cells(o).contains(&cell) {
        return vec![];
    }
    if let Some((core, a)) = door_core(o, wall) {
        let Some(k) = cell.checked_sub(a.columns.0) else {
            return vec![];
        };
        return pocket_problems(&core, k, wall, neighbours);
    }
    let Some(f) = Fitted::of(o, wall) else {
        return vec![];
    };
    let mut out = vec![];
    // exactly when [`pockets`] cuts no cavity; what it needs is worked
    // out with the leaf as thick as a wall with room would give it
    if f.leaf() / 2.0 + POCKET_PLAY > wall.thickness / 2.0 - POCKET_SKIN {
        let c = f.cut;
        let roomy = Fitted {
            cut: c,
            frame: o.frame.fitted(c.u1 - c.u0, c.v1 - c.v0, 1.0),
        };
        out.push(PocketProblem::ThinWall {
            needs: roomy.leaf() + 2.0 * (POCKET_PLAY + POCKET_SKIN),
        });
    }
    if wall.is_arc() {
        out.push(PocketProblem::CurvedWall);
    }
    let c = f.cut;
    let len = wall.length();
    let (mut short, mut meets) = (0.0_f64, false);
    for s in sliding_leaves(o, wall)
        .into_iter()
        .filter(|s| s.cell == cell)
    {
        let (u0, u1) = if s.travel < 0.0 {
            (c.u0 + s.travel - POCKET_SKIN, c.u0)
        } else {
            (c.u1, c.u1 + s.travel + POCKET_SKIN)
        };
        short = short.max(-u0).max(u1 - len);
        for n in neighbours {
            let Some(nc) = Cut::of(n, wall) else {
                continue;
            };
            let own = nc.u0 < c.u1 - 1e-6 && nc.u1 > c.u0 + 1e-6;
            let along = nc.u0 < u1 - 1e-6 && nc.u1 > u0 + 1e-6;
            let up = nc.v0 < c.v1 - 1e-6 && nc.v1 > c.v0 + 1e-6;
            meets |= !own && along && up;
        }
    }
    if short > 1e-6 {
        out.push(PocketProblem::PastEnd { short });
    }
    if meets {
        out.push(PocketProblem::MeetsOpening);
    }
    out
}

/// A door leaf as hung: its box, the jamb it turns about and whether it
/// is the active leaf (the one with the handle, opened first).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hung {
    pub leaf: PartBox,
    /// Where along the opening (opening coordinates) its hinges are.
    pub hinge: f64,
    /// Which way along the opening the leaf runs from its hinge, closed.
    pub towards: f64,
    pub active: bool,
    /// How it turns.
    pub swing: Swing,
}

/// How a hung leaf turns about its hinge side.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Swing {
    /// On hinges at its edge, one way.
    Hinged,
    /// On a pivot this far in from its hinge edge.
    Pivot(f64),
    /// On hinges at its edge, both ways.
    DoubleAction,
    /// Folding with its neighbours; `back` panels fold the other way
    /// from the first (every second one).
    Fold { back: bool },
}

/// A door's leaves with their hinges, in opening coordinates: a single
/// leaf hung on its hand's jamb, a pair each on its own jamb with the
/// active leaf on the hand's side.
pub fn hung_leaves(o: &Opening, wall: &Wall) -> Vec<Hung> {
    // an assembly's door hangs in its own part of the opening
    if let Some((core, _)) = door_core(o, wall) {
        let dx = core_dx(o, &core, wall);
        return hung_leaves(&core, wall)
            .into_iter()
            .map(|mut h| {
                h.leaf.lo.x += dx;
                h.leaf.hi.x += dx;
                h.hinge += dx;
                h
            })
            .collect();
    }
    let op = o.frame.operation(o.kind);
    if o.kind != OpeningKind::Door || op.slides() || op.lifts() || op == Operation::Revolving {
        return vec![];
    }
    // each leaf as one slab, whatever it is made of
    let mut slab = o.clone();
    slab.frame.construction.leaf = creabim_core::construction::LeafStyle::Flush;
    let mut leaves: Vec<PartBox> = opening_boxes(&slab, wall)
        .into_iter()
        .filter(|b| b.part == Part::Leaf)
        .collect();
    leaves.sort_by(|a, b| a.lo.x.total_cmp(&b.lo.x));
    let n = leaves.len();
    let flip = o.flip_hand;
    leaves
        .into_iter()
        .enumerate()
        .map(|(i, leaf)| {
            let r = leaf.hi.x - leaf.lo.x;
            // which jamb it is hung from, which leaf has the handle, and
            // how it turns
            let (at_lo, active, swing) = match op {
                Operation::Bifold | Operation::Folding => {
                    // every panel folds towards the hand's jamb
                    let k = if flip { n - 1 - i } else { i };
                    (!flip, k + 1 == n, Swing::Fold { back: k % 2 == 1 })
                }
                Operation::BifoldPair => {
                    let k = if i < n / 2 { i } else { n - 1 - i };
                    let active = if flip { i == n - 2 } else { i == 1 };
                    (i < n / 2, active, Swing::Fold { back: k % 2 == 1 })
                }
                _ => {
                    // the leftmost leaf turns about its left edge unless the
                    // door is a single one flipped to its other hand
                    let at_lo = if n == 1 { !flip } else { i == 0 };
                    let swing = match op {
                        Operation::Pivot => Swing::Pivot((r / 6.0).max(0.1f64.min(r / 2.0))),
                        Operation::CentrePivot => Swing::Pivot(r / 2.0),
                        Operation::DoubleAction | Operation::DoubleActionPair => {
                            Swing::DoubleAction
                        }
                        _ => Swing::Hinged,
                    };
                    (at_lo, n == 1 || (i == 0) != flip, swing)
                }
            };
            Hung {
                leaf,
                hinge: if at_lo { leaf.lo.x } else { leaf.hi.x },
                towards: if at_lo { 1.0 } else { -1.0 },
                active,
                swing,
            }
        })
        .collect()
}

/// The opening marks an elevation or section draws on a door's leaves,
/// in model coordinates: for each leaf facing the viewer, a triangle from
/// its free edge's top and bottom corners to the middle of its hinge edge,
/// just in front of the leaf's face on the viewer's side, and whether
/// the leaf opens towards the viewer (drawn solid) or away (dashed).
/// `eye` points from the drawing towards the viewer.
pub fn leaf_marks(o: &Opening, wall: &Wall, level_z: f64, eye: DVec3) -> Vec<(bool, [DVec3; 3])> {
    leaf_marks_with(o, wall, level_z, eye, creabim_core::TriangleApex::Hinge)
}

/// [`leaf_marks`] with the triangle's apex where the project puts it:
/// at the middle of the hinge edge, or of the free edge (the handle's)
/// with its base on the hinge edge.
pub fn leaf_marks_with(
    o: &Opening,
    wall: &Wall,
    level_z: f64,
    eye: DVec3,
    apex: creabim_core::TriangleApex,
) -> Vec<(bool, [DVec3; 3])> {
    let Some(at) = to_model(o, wall, level_z) else {
        return vec![];
    };
    let ny = at(DVec3::Y) - at(DVec3::ZERO);
    let facing = ny.dot(eye);
    // seen edge on, a triangle would be a line
    if facing.abs() < 0.3 {
        return vec![];
    }
    let viewer_plus = facing > 0.0;
    // built opening to +y, turned round when it faces the wall's right
    let opens_plus = o.faces_left(wall);
    let toward = viewer_plus == opens_plus;
    let mut out = vec![];
    for h in hung_leaves(o, wall) {
        let l = h.leaf;
        let y = if viewer_plus {
            l.hi.y + 0.002
        } else {
            l.lo.y - 0.002
        };
        let free = if h.towards > 0.0 { l.hi.x } else { l.lo.x };
        let mid = (l.lo.z + l.hi.z) / 2.0;
        let mut tri = |toward: bool, edge: f64, axis: f64| {
            let (base, point) = match apex {
                creabim_core::TriangleApex::Hinge => (edge, axis),
                creabim_core::TriangleApex::Handle => (axis, edge),
            };
            out.push((
                toward,
                [
                    at(DVec3::new(base, y, l.hi.z)),
                    at(DVec3::new(point, y, mid)),
                    at(DVec3::new(base, y, l.lo.z)),
                ],
            ));
        };
        match h.swing {
            Swing::Hinged => tri(toward, free, h.hinge),
            // it opens towards whoever looks at it
            Swing::DoubleAction => tri(true, free, h.hinge),
            // the two parts either side of the pivot turn opposite ways,
            // both pointing at it
            Swing::Pivot(p) => {
                let axis = h.hinge + h.towards * p;
                tri(toward, free, axis);
                if p > 1e-9 {
                    tri(!toward, h.hinge, axis);
                }
            }
            // every second panel folds back the other way
            Swing::Fold { back } => tri(toward != back, free, h.hinge),
        }
    }
    out
}

/// The marks an elevation or section draws on a sliding door, in model
/// coordinates, each a line and whether it is dashed: an arrow on each
/// sliding leaf the way it opens, where it slides to as a dashed outline
/// (inside the wall for a pocket door), and an F on each fixed panel.
/// `eye` points from the drawing towards the viewer.
pub fn slide_marks(
    o: &Opening,
    wall: &Wall,
    level_z: f64,
    eye: DVec3,
) -> Vec<(bool, DVec3, DVec3)> {
    let Some(at) = to_model(o, wall, level_z) else {
        return vec![];
    };
    let ny = at(DVec3::Y) - at(DVec3::ZERO);
    let facing = ny.dot(eye);
    if facing.abs() < 0.3 {
        return vec![];
    }
    let side = if facing > 0.0 { 1.0 } else { -1.0 };
    let face = wall.thickness / 2.0;
    let leaves = sliding_leaves(o, wall);
    // in front of everything on the viewer's side, for the outlines
    let front = leaves
        .iter()
        .map(|s| {
            if side > 0.0 {
                s.leaf.hi.y
            } else {
                -s.leaf.lo.y
            }
        })
        .fold(face, f64::max)
        + 0.003;
    let mut out = vec![];
    let b = DVec3::new;
    for s in &leaves {
        let l = s.leaf;
        let y = if side > 0.0 {
            l.hi.y + 0.002
        } else {
            l.lo.y - 0.002
        };
        let (cx, cz) = ((l.lo.x + l.hi.x) / 2.0, (l.lo.z + l.hi.z) / 2.0);
        let wide = l.hi.x - l.lo.x;
        if s.travel == 0.0 {
            // F for fixed, reading the right way round
            let k = (wide * 0.12).min(0.12);
            let flip = reads_backwards(&at, eye);
            for (a, b2) in letter_f(cx, cz, k, flip) {
                out.push((false, at(b(a.0, y, a.1)), at(b(b2.0, y, b2.1))));
            }
            continue;
        }
        // the arrow, along the leaf at mid height
        let dir = s.travel.signum();
        let len = wide * 0.5;
        let (xa, xb) = (cx - dir * len / 2.0, cx + dir * len / 2.0);
        let k = (len * 0.15).min(0.1);
        out.push((false, at(b(xa, y, cz)), at(b(xb, y, cz))));
        for e in [-1.0, 1.0] {
            out.push((
                false,
                at(b(xb, y, cz)),
                at(b(xb - dir * k, y, cz + e * k * 0.6)),
            ));
        }
        // where it slides to, past the opening: dashed
        let (x0, x1) = (l.lo.x + s.travel, l.hi.x + s.travel);
        let yf = side * front;
        let c = [
            b(x0, yf, l.lo.z),
            b(x1, yf, l.lo.z),
            b(x1, yf, l.hi.z),
            b(x0, yf, l.hi.z),
        ];
        for i in 0..4 {
            out.push((true, at(c[i]), at(c[(i + 1) % 4])));
        }
    }
    out
}

/// The marks an elevation or section draws on a window's cells, in
/// model coordinates, each a line and whether it is dashed: a triangle
/// on each hinged sash (its apex at the hinges, or at the handle as the
/// project sets), solid where it opens towards the viewer and dashed
/// where it opens away; an arrow on a sliding sash and an F on the fixed
/// light beside it. `eye` points from the drawing towards the viewer.
pub fn window_marks(
    o: &Opening,
    wall: &Wall,
    level_z: f64,
    eye: DVec3,
    apex: creabim_core::TriangleApex,
) -> Vec<(bool, DVec3, DVec3)> {
    match to_model(o, wall, level_z) {
        Some(at) => window_marks_at(o, wall, &at, eye, apex),
        None => vec![],
    }
}

/// A roof window's marks, as [`window_marks`] draws a window's in a wall.
pub fn roof_window_marks(
    model: &creabim_core::Model,
    o: &Opening,
    eye: DVec3,
    apex: creabim_core::TriangleApex,
) -> Vec<(bool, DVec3, DVec3)> {
    match roof_window(model, o) {
        Some((local, wall, at)) => window_marks_at(&local, &wall, &at, eye, apex),
        None => vec![],
    }
}

/// The marks on a window's cells, its own coordinates mapped into the
/// model's by `at`.
fn window_marks_at(
    o: &Opening,
    wall: &Wall,
    at: &dyn Fn(DVec3) -> DVec3,
    eye: DVec3,
    apex: creabim_core::TriangleApex,
) -> Vec<(bool, DVec3, DVec3)> {
    use creabim_core::cells::Mechanism as M;
    if o.kind == OpeningKind::Hole {
        return vec![];
    }
    let Some(f) = Fitted::of(o, wall) else {
        return vec![];
    };
    // a window's cells, or the lights round an assembly's door
    let cells = match o.kind {
        OpeningKind::Window => window_cells(&f),
        _ => light_cells(o, wall),
    };
    if cells.is_empty() {
        return vec![];
    }
    let ny = at(DVec3::Y) - at(DVec3::ZERO);
    let facing = ny.dot(eye);
    if facing.abs() < 0.3 {
        return vec![];
    }
    let viewer_plus = facing > 0.0;
    // built facing out of +y, turned round when it faces the wall's right
    // (a door's lights face the outside, however the door opens)
    let out_plus = match o.kind {
        OpeningKind::Window => o.faces_left(wall),
        _ => lights_face_left(wall),
    };
    let hd = f.frame.depth / 2.0 + 0.003;
    let y = if viewer_plus { hd } else { -hd };
    let p = |x: f64, z: f64| at(DVec3::new(x, y, z));
    let sliding = cells.iter().any(|c| c.cell.mechanism == M::Sliding);
    let mut out = vec![];
    for c in &cells {
        let (x0, x1, z0, z1) = (c.x0, c.x1, c.z0, c.z1);
        let (xm, zm) = ((x0 + x1) / 2.0, (z0 + z1) / 2.0);
        let m = c.cell.mechanism;
        if m.hinged() {
            // opens out of the side it faces unless flipped
            let opens_plus = out_plus != c.cell.flipped;
            let dashed = viewer_plus != opens_plus;
            // the hinge edge's middle, and the opposite edge's ends
            let (hinge, far) = match m {
                M::TopHung => ((xm, z1), [(x0, z0), (x1, z0)]),
                M::BottomHung => ((xm, z0), [(x0, z1), (x1, z1)]),
                _ if c.cell.hand => ((x1, zm), [(x0, z1), (x0, z0)]),
                _ => ((x0, zm), [(x1, z1), (x1, z0)]),
            };
            let (point, base) = match apex {
                creabim_core::TriangleApex::Hinge => (hinge, far),
                creabim_core::TriangleApex::Handle => {
                    // the apex at the handle's edge, the base on the hinges
                    let mid = ((far[0].0 + far[1].0) / 2.0, (far[0].1 + far[1].1) / 2.0);
                    let base = match m {
                        M::TopHung => [(x0, z1), (x1, z1)],
                        M::BottomHung => [(x0, z0), (x1, z0)],
                        _ => [(hinge.0, z1), (hinge.0, z0)],
                    };
                    (mid, base)
                }
            };
            for e in base {
                out.push((dashed, p(e.0, e.1), p(point.0, point.1)));
            }
        } else if matches!(
            m,
            M::TiltTurn | M::HorizontalPivot | M::CentrePivot | M::Folding
        ) {
            let opens_plus = out_plus != c.cell.flipped;
            let dashed = viewer_plus != opens_plus;
            let mut tri = |dashed: bool, point: (f64, f64), base: [(f64, f64); 2]| {
                for e in base {
                    out.push((dashed, p(e.0, e.1), p(point.0, point.1)));
                }
            };
            match m {
                // turns as a casement and tilts as a hopper: both
                // triangles
                M::TiltTurn => {
                    if c.cell.hand {
                        tri(dashed, (x1, zm), [(x0, z1), (x0, z0)]);
                    } else {
                        tri(dashed, (x0, zm), [(x1, z1), (x1, z0)]);
                    }
                    tri(dashed, (xm, z0), [(x0, z1), (x1, z1)]);
                }
                // the halves either side of the pivots turn opposite
                // ways, both pointing at the axis: the lower half out
                M::HorizontalPivot => {
                    tri(dashed, (xm, zm), [(x0, z0), (x1, z0)]);
                    tri(!dashed, (xm, zm), [(x0, z1), (x1, z1)]);
                }
                M::CentrePivot => {
                    tri(dashed, (xm, zm), [(x1, z0), (x1, z1)]);
                    tri(!dashed, (xm, zm), [(x0, z0), (x0, z1)]);
                }
                // folding sashes: each points to its fold on the side
                // they stack, every second one folding back
                _ => {
                    let back = c.at.0 % 2 == 1;
                    if c.cell.hand {
                        tri(dashed != back, (x1, zm), [(x0, z1), (x0, z0)]);
                    } else {
                        tri(dashed != back, (x0, zm), [(x1, z1), (x1, z0)]);
                    }
                }
            }
        } else if m == M::VerticalSliding {
            // up for the lower sash, down for the upper
            let dir = if c.at.1 == 0 { 1.0 } else { -1.0 };
            let len = (z1 - z0) * 0.5;
            let (a, b) = (zm - dir * len / 2.0, zm + dir * len / 2.0);
            let k = (len * 0.15).min(0.1);
            out.push((false, p(xm, a), p(xm, b)));
            for e in [-1.0, 1.0] {
                out.push((false, p(xm, b), p(xm + e * k * 0.6, b - dir * k)));
            }
            // a guillotine's one sash rises into the wall over it: where
            // it goes, dashed, on the wall's face so the wall doesn't
            // hide it
            if f.frame.grid.rows.len() == 1 {
                let up = z1 - z0;
                let wy = (wall.thickness / 2.0 + 0.003).max(hd);
                let q = |x: f64, z: f64| at(DVec3::new(x, if viewer_plus { wy } else { -wy }, z));
                let c = [(x0, z1), (x1, z1), (x1, z1 + up), (x0, z1 + up)];
                for i in 0..4 {
                    let (a, b) = (c[i], c[(i + 1) % 4]);
                    out.push((true, q(a.0, a.1), q(b.0, b.1)));
                }
            }
        } else if m == M::Sliding {
            // slides over its neighbour
            let dir = if c.at.0 == 0 { 1.0 } else { -1.0 };
            let len = (x1 - x0) * 0.5;
            let (a, b) = (xm - dir * len / 2.0, xm + dir * len / 2.0);
            let k = (len * 0.15).min(0.1);
            out.push((false, p(a, zm), p(b, zm)));
            for e in [-1.0, 1.0] {
                out.push((false, p(b, zm), p(b - dir * k, zm + e * k * 0.6)));
            }
        } else if m == M::Fixed && sliding {
            let k = ((x1 - x0) * 0.12).min(0.12);
            for (a, b) in letter_f(xm, zm, k, reads_backwards(&at, eye)) {
                out.push((false, p(a.0, a.1), p(b.0, b.1)));
            }
        }
    }
    out
}

/// A window's cells in words for schedules: its style when it is one
/// ("Casement pair"), else each cell's mechanism.
pub fn window_text(o: &Opening) -> String {
    // a roof window, named as roof windows are
    if o.roof_at.is_some() {
        let s = creabim_core::cells::WindowStyle::of(&o.frame.grid);
        return match s {
            Some(s) => format!("Roof window, {}", s.roof_label().to_lowercase()),
            None => format!(
                "Roof window, {}",
                window_text(&Opening {
                    roof_at: None,
                    arrangement: None,
                    ..o.clone()
                })
                .to_lowercase()
            ),
        };
    }
    if let Some((square, head)) = squared(o) {
        return format!("{}{head}", window_text(&square));
    }
    use creabim_core::cells::{Mechanism as M, WindowStyle};
    let g = &o.frame.grid;
    let side = |inward: bool| if inward { ", in" } else { "" };
    let any_in = g
        .cells
        .iter()
        .any(|c| c.flipped && matches!(c.mechanism, M::Hinged | M::Folding));
    let style = |g: &creabim_core::cells::Grid| {
        WindowStyle::of(g).map(|s| match s {
            WindowStyle::Casement | WindowStyle::CasementPair | WindowStyle::Bifold => {
                format!("{}{}", s.label(), side(any_in))
            }
            _ => s.label().into(),
        })
    };
    if let Some(t) = style(g) {
        return t;
    }
    // a style with sidelights or a transom round it
    if let Some(a) = g.assembly().filter(|a| a.has_lights()) {
        if let Some(t) = style(&a.core) {
            return format!("{t}{}", lights_text(&a));
        }
    }
    // each kind of cell once, with how many there are, openers first
    let name = |m: M| match m {
        M::Hinged => "casement",
        M::TopHung => "awning",
        M::BottomHung => "hopper",
        M::Sliding | M::SurfaceSliding | M::Pocket => "sliding",
        M::Louvred => "louvred",
        M::Fixed => "fixed",
        M::Pivot | M::CentrePivot | M::HorizontalPivot => "pivot",
        M::DoubleAction => "double action",
        M::Folding => "folding",
        M::TiltTurn => "tilt and turn",
        M::VerticalSliding => "hung sash",
        M::Sectional => "sectional",
        M::Rolling => "roller",
        M::Revolving => "revolving",
    };
    let mut counts: Vec<(&str, usize)> = vec![];
    for c in &g.cells {
        let n = name(c.mechanism);
        match counts.iter_mut().find(|(k, _)| *k == n) {
            Some((_, k)) => *k += 1,
            None => counts.push((n, 1)),
        }
    }
    counts.sort_by_key(|(n, _)| *n == "fixed");
    let parts: Vec<String> = counts
        .iter()
        .map(|(n, k)| {
            if *k > 1 {
                format!("{n} ×{k}")
            } else {
                n.to_string()
            }
        })
        .collect();
    let mut t = parts.join(" + ");
    if let Some(c) = t.get_mut(0..1) {
        c.make_ascii_uppercase();
    }
    t
}

/// Does the opening's x axis run right to left for a viewer looking
/// back along `eye`?
fn reads_backwards(at: &dyn Fn(DVec3) -> DVec3, eye: DVec3) -> bool {
    let x = at(DVec3::X) - at(DVec3::ZERO);
    // the viewer's right: looking along -eye, with z up
    let right = (-eye).cross(DVec3::Z);
    x.dot(right) < 0.0
}

/// The strokes of an F `2k` tall centred on (`x`, `z`) in opening
/// coordinates, drawn mirrored along x when the view sees x backwards so
/// it reads the right way round.
fn letter_f(x: f64, z: f64, k: f64, flip: bool) -> [((f64, f64), (f64, f64)); 3] {
    let s = if flip { -1.0 } else { 1.0 };
    let (fx, fz) = (x - s * k * 0.4, z - k);
    [
        ((fx, fz), (fx, fz + 2.0 * k)),
        ((fx, fz + 2.0 * k), (fx + s * k, fz + 2.0 * k)),
        ((fx, fz + k), (fx + s * k * 0.8, fz + k)),
    ]
}

/// How a door or window opens, in words for schedules: its operation,
/// and for a pair the leaf widths in millimetres, active leaf first.
pub fn operation_text(o: &Opening, wall: &Wall) -> String {
    if let Some((square, head)) = squared(o) {
        return format!("{}{head}", operation_text(&square, wall));
    }
    if o.kind == OpeningKind::Window {
        return window_text(o);
    }
    if let Some((core, a)) = door_core(o, wall) {
        return format!("{}{}", operation_text(&core, wall), lights_text(&a));
    }
    let name = o.frame.operation(o.kind).label(o.kind);
    if o.kind == OpeningKind::Door && o.frame.operation(o.kind) == Operation::MultiSlide {
        return format!("{name}, {}", slide_pattern(o, wall));
    }
    let op = o.frame.operation(o.kind);
    if op == Operation::Folding {
        return format!("{name}, {} panels", o.frame.core().cells.len());
    }
    let hung = hung_leaves(o, wall);
    if hung.len() < 2 || !op.pair() {
        return name.into();
    }
    let mm = |h: &Hung| ((h.leaf.hi.x - h.leaf.lo.x) * 1000.0).round();
    let mut widths: Vec<(bool, f64)> = hung.iter().map(|h| (h.active, mm(h))).collect();
    widths.sort_by_key(|(active, _)| !*active);
    let list: Vec<String> = widths.iter().map(|(_, w)| format!("{w}")).collect();
    format!("{name}, {}", list.join(" + "))
}

/// An arched or raked opening made square-headed, and its head in words
/// to follow its name: ", round-headed", ", segmental head", ", raked
/// head". None for a square-headed one.
fn squared(o: &Opening) -> Option<(Opening, &'static str)> {
    use creabim_core::cells::Outline;
    let words = match o.frame.grid.outline.fitted(o.width, o.height) {
        Outline::Rectangle => return None,
        Outline::Round => ", round-headed",
        Outline::Segmental { .. } => ", segmental head",
        Outline::Raked { .. } => ", raked head",
    };
    let mut square = o.clone();
    square.frame.grid.outline = Outline::Rectangle;
    Some((square, words))
}

/// The lights round an assembly's core in words, to follow its name:
/// ", with sidelight and transom", ", with sidelights", ", with
/// awning transom".
fn lights_text(a: &creabim_core::cells::Assembly) -> String {
    use creabim_core::cells::Mechanism as M;
    let mut parts = vec![];
    match (a.left.is_some(), a.right.is_some()) {
        (true, true) => parts.push("sidelights".to_string()),
        (true, false) | (false, true) => parts.push("sidelight".to_string()),
        _ => {}
    }
    if let Some((_, c)) = a.transom {
        parts.push(
            match c.mechanism {
                M::TopHung => "awning transom",
                M::BottomHung => "hopper transom",
                M::Louvred => "louvred transom",
                _ => "transom",
            }
            .to_string(),
        );
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!(", with {}", parts.join(" and "))
    }
}

/// A door assembly's door: the opening its leaves fill inside the whole
/// (a door of its own, framed by the mullions and transom round it), and
/// the assembly read from its grid. None for a door that is all door, a
/// window or a hole.
pub fn door_core(o: &Opening, wall: &Wall) -> Option<(Opening, creabim_core::cells::Assembly)> {
    if o.kind != OpeningKind::Door || o.frame.grid.operation(o.kind).is_some() {
        return None;
    }
    let f = Fitted::of(o, wall)?;
    let a = f.frame.grid.assembly()?;
    if !a.has_lights() || a.core.operation(OpeningKind::Door).is_none() {
        return None;
    }
    let (x0, x1, z1) = core_rect(&f, &a);
    let core = Opening {
        offset: f.cut.u0 + x0,
        width: x1 - x0,
        sill: f.cut.v0,
        height: z1,
        frame: OpeningFrame {
            grid: a.core.clone(),
            // the trim and ratings are the whole assembly's
            construction: creabim_core::construction::Construction {
                leaf: f.frame.construction.leaf,
                fittings: f.frame.construction.fittings,
                panes: f.frame.construction.panes,
                ..Default::default()
            },
            ..f.frame.clone()
        },
        ..o.clone()
    };
    Some((core, a))
}

/// How far an assembly's door starts along from the assembly's start.
fn core_dx(o: &Opening, core: &Opening, wall: &Wall) -> f64 {
    core.offset - Cut::of(o, wall).map_or(o.offset, |c| c.u0)
}

/// Where an assembly's core stands in it, in opening coordinates: from
/// and to along it, and up to the top of the transom bar over it (its
/// frame is the mullions and transom bar round it, or the outer frame).
fn core_rect(f: &Fitted, a: &creabim_core::cells::Assembly) -> (f64, f64, f64) {
    let (w, h) = (f.width(), f.height());
    let fw = f.frame.width;
    let g = &f.frame.grid;
    let xs = spans(&g.columns, fw, w - fw, fw);
    let zs = spans(&g.rows, fw, h - fw, fw);
    let (c0, c1) = a.columns;
    let x0 = if c0 == 0 { 0.0 } else { xs[c0].0 - fw };
    let x1 = if c1 + 1 >= xs.len() { w } else { xs[c1].1 + fw };
    let z1 = if zs.len() > 1 { zs[0].1 + fw } else { h };
    (x0, x1, z1)
}

/// Do the lights round an assembly's door face out of the wall's left?
/// They face the outside, as a window does, whichever way the door
/// opens: the wall's right unless the wall is flipped.
fn lights_face_left(wall: &Wall) -> bool {
    wall.flip
}

/// The lights round an assembly's door, as cells in the whole opening's
/// coordinates (before it is turned to its wall).
pub fn light_cells(o: &Opening, wall: &Wall) -> Vec<CellAt> {
    let (Some((_, a)), Some(f)) = (door_core(o, wall), Fitted::of(o, wall)) else {
        return vec![];
    };
    let (c0, c1) = a.columns;
    window_cells(&f)
        .into_iter()
        .filter(|c| !(c.at.1 == 0 && (c0..=c1).contains(&c.at.0)))
        .collect()
}

/// A door assembly's parts: its door, built as a door of its own and
/// moved into place, and round it the outer frame, mullions, transom
/// bar, sills and the lights.
fn assembly_boxes(
    f: &Fitted,
    o: &Opening,
    core: &Opening,
    a: &creabim_core::cells::Assembly,
    wall: &Wall,
) -> Vec<PartBox> {
    let dx = core_dx(o, core, wall);
    let mut out: Vec<PartBox> = opening_boxes(core, wall)
        .into_iter()
        .map(|mut b| {
            b.lo.x += dx;
            b.hi.x += dx;
            b
        })
        .collect();
    let (w, h) = (f.width(), f.height());
    let fw = f.frame.width;
    let hd = f.frame.depth / 2.0;
    let (x0, x1, z1) = core_rect(f, a);
    let b = DVec3::new;
    let mut parts: Vec<(Part, DVec3, DVec3)> = vec![];
    // the outer jambs, down to the floor beside a sidelight, else from
    // the door's head up
    for (lo, hi, beside) in [(0.0, fw, x0 > 0.0), (w - fw, w, x1 < w)] {
        let z0 = if beside { 0.0 } else { z1 };
        parts.push((Part::Frame, b(lo, -hd, z0), b(hi, hd, h)));
    }
    // the head: over the transom whole, else over the sidelights
    let head = if z1 < h - 1e-9 {
        vec![(fw, w - fw)]
    } else {
        vec![(fw, x0), (x1, w - fw)]
    };
    for (lo, hi) in head {
        parts.push((Part::Frame, b(lo, -hd, h - fw), b(hi, hd, h)));
    }
    // a sill under each sidelight
    for (lo, hi) in [(fw, x0), (x1, w - fw)] {
        parts.push((Part::Frame, b(lo, -hd, 0.0), b(hi, hd, fw)));
    }
    parts.extend(grid_bars(f, Some(((x0, x1), a.columns))));
    for c in light_cells(o, wall) {
        parts.extend(cell_parts(f, &c, false));
    }
    // the lights built facing out of +y as a window's, turned to face
    // the outside whichever way the door swings; the door's hand and
    // swing turn only the door
    let across = !lights_face_left(wall);
    for (part, mut lo, mut hi) in parts {
        if (hi - lo).min_element() <= 1e-6 {
            continue;
        }
        if across {
            (lo.y, hi.y) = (-hi.y, -lo.y);
        }
        out.push(PartBox { part, lo, hi });
    }
    out
}

/// How far a window's outside sill runs past the wall's face.
pub const SILL_NOSE: f64 = 0.04;

/// Is `b` the outside sill [`opening_boxes`] gives a window: the board
/// across its whole width at its foot, 30 mm thick?
fn is_outside_sill(b: &PartBox, o: &Opening) -> bool {
    b.part == Part::Frame
        && b.lo.x.abs() < 1e-9
        && (b.hi.x - o.width).abs() < 1e-9
        && b.lo.z.abs() < 1e-9
        && (b.hi.z - 0.03).abs() < 1e-9
}

/// One cell of a window's grid as built in its opening (facing out of
/// +y, before the window is turned to its wall): the rectangle inside
/// the frame and mullions it fills, in opening coordinates, and the cell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellAt {
    pub x0: f64,
    pub x1: f64,
    pub z0: f64,
    pub z1: f64,
    /// Its column and row.
    pub at: (usize, usize),
    pub cell: creabim_core::cells::Cell,
}

/// A track's cells' spans between `a` and `b`, a bar `bar` wide before
/// each track that has one.
fn spans(tracks: &[creabim_core::cells::Track], a: f64, b: f64, bar: f64) -> Vec<(f64, f64)> {
    let bars = tracks.iter().skip(1).filter(|t| t.bar).count() as f64;
    let total: f64 = tracks.iter().map(|t| t.share).sum();
    let room = (b - a - bars * bar).max(0.0);
    let mut x = a;
    let mut out = vec![];
    for (i, t) in tracks.iter().enumerate() {
        if i > 0 && t.bar {
            x += bar;
        }
        let w = room * t.share / total;
        out.push((x, x + w));
        x += w;
    }
    out
}

/// A window's cells inside its frame, laid out by its grid.
pub fn window_cells(f: &Fitted) -> Vec<CellAt> {
    let (w, h) = (f.width(), f.height());
    let fw = f.frame.width;
    let g = &f.frame.grid;
    let xs = spans(&g.columns, fw, w - fw, fw);
    let zs = spans(&g.rows, fw, h - fw, fw);
    let mut out = vec![];
    for (j, &(z0, z1)) in zs.iter().enumerate() {
        for (i, &(x0, _)) in xs.iter().enumerate() {
            if g.covered(i, j) {
                continue;
            }
            if let Some(cell) = g.cell(i, j) {
                // over every column it spans
                let last = (i + cell.across.max(1) as usize - 1).min(xs.len() - 1);
                out.push(CellAt {
                    x0,
                    x1: xs[last].1,
                    z0,
                    z1,
                    at: (i, j),
                    cell: *cell,
                });
            }
        }
    }
    out
}

/// A grid's mullions and transoms, as boxes: each mullion row by row
/// where no cell spans across it, each transom across the whole width.
/// `core`: an assembly's door, from and to along the opening and its
/// columns, which has its own frame, so no bar runs through or beside it.
fn grid_bars(f: &Fitted, core: Option<((f64, f64), (usize, usize))>) -> Vec<(Part, DVec3, DVec3)> {
    let (w, h) = (f.width(), f.height());
    let fw = f.frame.width;
    let hd = f.frame.depth / 2.0;
    let g = &f.frame.grid;
    let xs = spans(&g.columns, fw, w - fw, fw);
    let zs = spans(&g.rows, fw, h - fw, fw);
    let mut out = vec![];
    for (j, &(z0, z1)) in zs.iter().enumerate() {
        for i in 1..xs.len() {
            let by_core = core.is_some_and(|(_, (c0, c1))| j == 0 && i >= c0 && i <= c1 + 1);
            if g.columns[i].bar && !g.covered(i, j) && !by_core {
                out.push((
                    Part::Frame,
                    DVec3::new(xs[i - 1].1, -hd, z0),
                    DVec3::new(xs[i].0, hd, z1),
                ));
            }
        }
    }
    for j in 1..zs.len() {
        if g.rows[j].bar {
            let pieces = match core {
                Some(((x0, x1), _)) if j == 1 => vec![(fw, x0), (x1, w - fw)],
                _ => vec![(fw, w - fw)],
            };
            for (a, b) in pieces {
                out.push((
                    Part::Frame,
                    DVec3::new(a, -hd, zs[j - 1].1),
                    DVec3::new(b, hd, zs[j].0),
                ));
            }
        }
    }
    out
}

/// Glass in a rectangle split into panes by glazing bars `bar` wide,
/// centred on `y`.
fn glaze(
    out: &mut Vec<(Part, DVec3, DVec3)>,
    (x0, x1, z0, z1): (f64, f64, f64, f64),
    [na, nu]: [u32; 2],
    bar: f64,
    bd: f64,
    y: f64,
) {
    let b = DVec3::new;
    // pane edges along x and z: between bars, bars centred on the splits
    let edges = |a: f64, z: f64, n: u32| -> Vec<(f64, f64)> {
        let step = (z - a) / n as f64;
        (0..n)
            .map(|i| {
                let lo = a + step * i as f64 + if i > 0 { bar / 2.0 } else { 0.0 };
                let hi = a + step * (i + 1) as f64 - if i + 1 < n { bar / 2.0 } else { 0.0 };
                (lo, hi)
            })
            .collect()
    };
    let xs = edges(x0, x1, na.max(1));
    let zs = edges(z0, z1, nu.max(1));
    for pair in xs.windows(2) {
        out.push((
            Part::Frame,
            b(pair[0].1, y - bd, z0),
            b(pair[1].0, y + bd, z1),
        ));
    }
    for &(xa, xb) in &xs {
        for pair in zs.windows(2) {
            out.push((
                Part::Frame,
                b(xa, y - bd, pair[0].1),
                b(xb, y + bd, pair[1].0),
            ));
        }
        for &(za, zb) in &zs {
            out.push((
                Part::Glass,
                b(xa, y - GLASS / 2.0, za),
                b(xb, y + GLASS / 2.0, zb),
            ));
        }
    }
}

/// The parts of one window cell: glass for a fixed light; a sash round
/// its glass, with a handle inside, for one that opens; blades for a
/// louvre.
fn cell_parts(f: &Fitted, c: &CellAt, sliding: bool) -> Vec<(Part, DVec3, DVec3)> {
    use creabim_core::cells::Mechanism as M;
    use creabim_core::construction::LeafStyle as L;
    let fw = f.frame.width;
    let hd = f.frame.depth / 2.0;
    // a French or slim steel window: its glass in small panes (as many as
    // set, or about as its make gives where a cell has one), slim steel's
    // sashes and bars thin
    let make = f.frame.construction.leaf;
    let bar = match make {
        L::SlimSteel => 0.018f64.min(fw * 0.6),
        _ => fw * 0.6,
    };
    let panes_in = |w: f64, h: f64| -> [u32; 2] {
        if make.has_panes() && c.cell.panes == [1, 1] {
            let [a, u] = f.frame.construction.panes_for(w, h);
            [a as u32, u as u32]
        } else {
            c.cell.panes
        }
    };
    let bd = hd * 0.7;
    let b = DVec3::new;
    let mut out = vec![];
    let (x0, x1, z0, z1) = (c.x0, c.x1, c.z0, c.z1);
    match c.cell.mechanism {
        M::Fixed if c.cell.track == 0 && !sliding => {
            glaze(
                &mut out,
                (x0, x1, z0, z1),
                panes_in(x1 - x0, z1 - z0),
                bar,
                bd,
                0.0,
            );
        }
        M::Louvred => {
            // blades a tenth of a metre apart, in a frame of clips each side
            let clip = 0.02f64.min((x1 - x0) / 6.0);
            out.push((Part::Frame, b(x0, -bd, z0), b(x0 + clip, bd, z1)));
            out.push((Part::Frame, b(x1 - clip, -bd, z0), b(x1, bd, z1)));
            // glass blades, tilted and lapping, spread evenly (MU104); each
            // given here as the box round it, shaped in `opening_parts`
            let (_, d) = blade_extent();
            for (za, zb) in louvre_blades(z0, z1) {
                out.push((
                    Part::Glass,
                    b(x0 + clip, -d / 2.0, za),
                    b(x1 - clip, d / 2.0, zb),
                ));
            }
        }
        m => {
            // a sash in the frame: on its own track when it slides past
            // its neighbour, lapping it by 20 mm
            let (ya, yb) = if sliding {
                if c.cell.track == 0 {
                    (-hd * 0.9, -0.004)
                } else {
                    (0.004, hd * 0.9)
                }
            } else {
                (-hd * 0.8, hd * 0.8)
            };
            let lap = if sliding { 0.02 } else { 0.0 };
            let cols = f.frame.grid.columns.len();
            let rows = f.frame.grid.rows.len();
            let (x0, x1) = (
                if c.at.0 > 0 { x0 - lap } else { x0 },
                if c.at.0 + 1 < cols { x1 + lap } else { x1 },
            );
            // hung sashes lap at their meeting rails
            let hung = f
                .frame
                .grid
                .cells
                .iter()
                .any(|c| c.mechanism == M::VerticalSliding);
            let lap_z = if hung { lap } else { 0.0 };
            let (z0, z1) = (
                if c.at.1 > 0 { z0 - lap_z } else { z0 },
                if c.at.1 + 1 < rows { z1 + lap_z } else { z1 },
            );
            let sw = if make == L::SlimSteel { 0.03 } else { fw * 0.9 }
                .min((x1 - x0) / 4.0)
                .min((z1 - z0) / 4.0);
            out.push((Part::Leaf, b(x0, ya, z0), b(x0 + sw, yb, z1)));
            out.push((Part::Leaf, b(x1 - sw, ya, z0), b(x1, yb, z1)));
            out.push((Part::Leaf, b(x0 + sw, ya, z0), b(x1 - sw, yb, z0 + sw)));
            out.push((Part::Leaf, b(x0 + sw, ya, z1 - sw), b(x1 - sw, yb, z1)));
            let gy = (ya + yb) / 2.0;
            glaze(
                &mut out,
                (x0 + sw, x1 - sw, z0 + sw, z1 - sw),
                panes_in(x1 - x0 - 2.0 * sw, z1 - z0 - 2.0 * sw),
                bar,
                (yb - ya) / 2.0 * 0.7,
                gy,
            );
            // a handle on the inside face (-y): on the free edge of a
            // casement, the bottom of an awning, the top of a hopper, the
            // leading edge of a sliding sash
            let (hx, hz, upright) = match m {
                M::Hinged | M::TiltTurn | M::Folding => (
                    if c.cell.hand {
                        x0 + sw / 2.0
                    } else {
                        x1 - sw / 2.0
                    },
                    (z0 + z1) / 2.0,
                    true,
                ),
                M::TopHung | M::HorizontalPivot => ((x0 + x1) / 2.0, z0 + sw / 2.0, false),
                M::BottomHung => ((x0 + x1) / 2.0, z1 - sw / 2.0, false),
                // a sash lift on the meeting rail
                M::VerticalSliding if c.at.1 == 0 => ((x0 + x1) / 2.0, z1 - sw / 2.0, false),
                M::VerticalSliding => ((x0 + x1) / 2.0, z0 + sw / 2.0, false),
                M::CentrePivot => (x1 - sw / 2.0, (z0 + z1) / 2.0, true),
                _ => (
                    if c.at.0 == 0 {
                        x1 - sw / 2.0
                    } else {
                        x0 + sw / 2.0
                    },
                    (z0 + z1) / 2.0,
                    true,
                ),
            };
            if m != M::Fixed && !(m == M::Folding && !c.cell.active) {
                let (dx, dz) = if upright {
                    (0.012, 0.06)
                } else {
                    (0.06, 0.012)
                };
                out.push((
                    Part::Handle,
                    b(hx - dx, ya - 0.02, hz - dz),
                    b(hx + dx, ya, hz + dz),
                ));
            }
        }
    }
    out
}

/// Which way a door is hung, in words: its hinge side seen from the
/// side it opens to, and which side that is. A sliding door says which
/// way it slides, seen from the side its leaves are on (the inside for a
/// pocket door or a multi-panel slider unless flipped).
pub fn door_hand(o: &Opening, wall: &Wall) -> String {
    if let Some((core, _)) = door_core(o, wall) {
        return door_hand(&core, wall);
    }
    let side = if o.flip_facing { "out" } else { "in" };
    let op = o.frame.operation(o.kind);
    if o.kind == OpeningKind::Door && op.slides() {
        let leaves = sliding_leaves(o, wall);
        // seen from the side it faces, the wall's start is on the right
        // when that is the wall's left
        let start_right = o.faces_left(wall);
        let way = |t: f64| {
            if (t < 0.0) == start_right {
                "right"
            } else {
                "left"
            }
        };
        return match op {
            Operation::SurfaceSliding => format!("Slides {}, {side}", way(leaves[0].travel)),
            Operation::Pocket => format!("Slides {}", way(leaves[0].travel)),
            Operation::SurfaceSlidingPair => format!("Parting, {side}"),
            Operation::PocketPair => "Parting".into(),
            _ => format!("Sliding, {side}"),
        };
    }
    // looking at the side it opens to, the wall's start is on the right
    // when that is the wall's left
    let hinge_at_start = !o.flip_hand;
    let hinge = if hinge_at_start == o.faces_left(wall) {
        "Right"
    } else {
        "Left"
    };
    match op {
        Operation::Sectional | Operation::Roller => format!("Up, {side}"),
        Operation::Revolving => "Revolving".into(),
        Operation::BifoldPair => format!("Parting, {side}"),
        _ if op.folds() => format!("Folds {}, {side}", hinge.to_lowercase()),
        Operation::DoubleAction | Operation::DoubleActionPair => {
            format!("{hinge}, both ways")
        }
        _ => format!("{hinge}, {side}"),
    }
}

/// A multi-panel slider's panels read left to right from the side it
/// faces: F fixed, S sliding.
pub fn slide_pattern(o: &Opening, wall: &Wall) -> String {
    if let Some((core, _)) = door_core(o, wall) {
        return slide_pattern(&core, wall);
    }
    let mut leaves = sliding_leaves(o, wall);
    leaves.sort_by(|a, b| a.leaf.lo.x.total_cmp(&b.leaf.lo.x));
    if o.faces_left(wall) {
        leaves.reverse();
    }
    let marks: Vec<&str> = leaves
        .iter()
        .map(|s| if s.travel == 0.0 { "F" } else { "S" })
        .collect();
    marks.join(" ")
}

/// Opening coordinates → model coordinates, for an opening in `wall`
/// standing on a level at `level_z`.
pub fn to_model(o: &Opening, wall: &Wall, level_z: f64) -> Option<impl Fn(DVec3) -> DVec3> {
    let c = Cut::of(o, wall)?;
    // square to the wall at the opening's middle: on a curved wall the
    // door or window stands on the arc, along its tangent there
    let um = (c.u0 + c.u1) / 2.0;
    let d = wall.tangent_at(um);
    let n = DVec2::new(-d.y, d.x);
    let start = wall.place(um, 0.0) - d * um;
    let z0 = level_z + wall.base_offset + c.v0;
    Some(move |p: DVec3| (start + d * (c.u0 + p.x) + n * p.y).extend(z0 + p.z))
}

/// The opening's parts as meshes in model coordinates, one per kind of part.
pub fn opening_meshes(o: &Opening, wall: &Wall, level_z: f64) -> Vec<(Part, TriMesh)> {
    let Some(at) = to_model(o, wall, level_z) else {
        return vec![];
    };
    let mut out: Vec<(Part, TriMesh)> = Vec::new();
    for pb in opening_parts(o, wall) {
        let i = match out.iter().position(|(p, _)| *p == pb.part()) {
            Some(i) => i,
            None => {
                out.push((pb.part(), TriMesh::default()));
                out.len() - 1
            }
        };
        match &pb {
            Shaped::Box(b) => add_box(&mut out[i].1, b.lo, b.hi, &at),
            Shaped::Prism(p) => add_prism(&mut out[i].1, p, &at),
            Shaped::Slat(p) => add_slat(&mut out[i].1, p, &at),
            Shaped::Plan(p) => add_plan(&mut out[i].1, p, &at),
        }
    }
    out
}

/// A roof window as built: the window as it would be in a wall as thick
/// as its roof, in its own coordinates (x across the slope, y into the
/// roof, z up it, outside towards -y), and the map from those into the
/// model's. Its frame's outer face stands its upstand above the roof
/// finish (none: flush). None for a door or window in a wall.
pub fn roof_window(
    model: &creabim_core::Model,
    o: &Opening,
) -> Option<(Opening, Wall, impl Fn(DVec3) -> DVec3)> {
    let seat = creabim_core::roofwindow::seat(model, o)?;
    let r = creabim_core::roofwindow::roof_of(model, o)?;
    let wall = Wall::new(
        r.level,
        DVec2::ZERO,
        DVec2::new(o.width, 0.0),
        seat.thickness,
        o.height,
    );
    let local = Opening {
        offset: 0.0,
        sill: 0.0,
        flip_facing: false,
        roof_at: None,
        arrangement: None,
        ..o.clone()
    };
    let depth = Fitted::of(&local, &wall)?.frame.depth;
    let lift = roof_lift(&local, &wall, depth);
    Some((local, wall, move |p: DVec3| seat.at(p - DVec3::Y * lift)))
}

/// How far a roof window's own coordinates are moved out of the roof so
/// its frame's outer face stands its upstand above the roof finish.
pub fn roof_lift(local: &Opening, wall: &Wall, depth: f64) -> f64 {
    wall.thickness / 2.0 + local.roof_build.upstand.max(0.0) - depth / 2.0
}

/// How far below the roof finish its upstand sits on the roof: on the
/// deck, under the battens and roofing.
const UPSTAND_SEAT: f64 = 0.04;

/// How far up the slope a roof window's lining reaches by the roof's
/// underside, `d` deeper than where it starts: (head, sill). Splayed,
/// the head runs level into the room and the sill plumb; square, neither
/// moves. On a low slope the level head would run on: it stops at 1.2 m.
pub fn roof_splay(lining: creabim_core::RoofLining, pitch: f64, d: f64) -> (f64, f64) {
    let th = pitch.to_radians();
    match lining {
        creabim_core::RoofLining::Splayed if th > 0.05 && d > 0.0 => {
            ((d / th.tan()).min(1.2), -(d * th.tan()).min(1.2))
        }
        _ => (0.0, 0.0),
    }
}

/// Where a roof window's lining starts into its roof, from the roof's
/// middle plane (positive into the roof), and how far its head and sill
/// reach up the slope by the underside: the shape of the hole it needs.
pub fn roof_lining_reach(model: &creabim_core::Model, o: &Opening) -> Option<(f64, f64, f64)> {
    let (local, wall, _) = roof_window(model, o)?;
    let f = Fitted::of(&local, &wall)?;
    let (depth, t) = (f.frame.depth, wall.thickness);
    let lift = roof_lift(&local, &wall, depth);
    let y0 = (depth / 2.0).max(-t / 2.0 + UPSTAND_SEAT + lift);
    let y1 = t / 2.0 + lift;
    let pitch = creabim_core::roofwindow::seat(model, o)?.pitch();
    let (head, sill) = roof_splay(local.roof_build.lining, pitch, y1 - y0);
    Some((y0 - lift, head, sill))
}

/// A roof window's parts round its frame, in its own coordinates: the
/// upstand under its frame down to the deck, the flashing collar on the
/// roofing round it, the insulation collar round it in the roof, and its
/// lining from there to the roof's underside, splayed or square.
/// `pitch` is its slope in degrees.
pub fn roof_build_parts(local: &Opening, wall: &Wall, pitch: f64) -> Vec<Shaped> {
    let Some(f) = Fitted::of(local, wall) else {
        return vec![];
    };
    let rb = local.roof_build;
    let (w, h, t) = (local.width, local.height, wall.thickness);
    let (depth, fw) = (f.frame.depth, f.frame.width);
    let lift = roof_lift(local, wall, depth);
    // the roof's faces in these coordinates
    let (finish, deck, under) = (
        -t / 2.0 + lift,
        -t / 2.0 + UPSTAND_SEAT + lift,
        t / 2.0 + lift,
    );
    let b = DVec3::new;
    let mut out = vec![];
    let ring = |out: &mut Vec<Shaped>, part: Part, y0: f64, y1: f64, inset: f64, outset: f64| {
        if y1 <= y0 + 1e-6 {
            return;
        }
        let (a, z) = (-outset, inset);
        for (lo, hi) in [
            (b(a, y0, a), b(w - a, y1, z)),
            (b(a, y0, h - z), b(w - a, y1, h - a)),
            (b(a, y0, z), b(z, y1, h - z)),
            (b(w - z, y0, z), b(w - a, y1, h - z)),
        ] {
            out.push(Shaped::Box(PartBox { part, lo, hi }));
        }
    };
    // the upstand: the frame's rim carried down onto the deck
    let frame_in = depth / 2.0;
    ring(&mut out, Part::Frame, frame_in, deck, fw, 0.0);
    // the flashing collar on the roofing round it, and up the upstand
    if rb.flashing {
        let lap = 0.12;
        ring(&mut out, Part::Flashing, finish - 0.003, finish, 0.0, lap);
        if rb.upstand > 0.0 {
            let top = -depth / 2.0 + 0.02;
            ring(
                &mut out,
                Part::Flashing,
                top.min(finish - 0.003),
                finish - 0.003,
                0.0,
                0.003,
            );
        }
    }
    // the lining, from the upstand's foot (or the frame) to the underside
    let y0 = frame_in.max(deck);
    let y1 = under;
    let lt = 0.012;
    if y1 > y0 + 1e-6 && w > 2.0 * lt && h > 2.0 * lt {
        // splayed: the head runs level into the room, the sill plumb
        let (dz_head, dz_sill) = roof_splay(rb.lining, pitch, y1 - y0);
        let v = DVec2::new;
        let shaped = |part: Part, outline: Vec<DVec2>, x0: f64, x1: f64| {
            Shaped::Slat(PartSlat {
                part,
                outline,
                x0,
                x1,
            })
        };
        let slat = |outline: Vec<DVec2>, x0: f64, x1: f64| shaped(Part::Trim, outline, x0, x1);
        // the insulation collar behind the lining, in the roof's build-up,
        // following its splay (square round the frame, it stood in the
        // splayed hole)
        if rb.collar {
            let (c, k) = (0.05, under - 0.015);
            let at = |dz: f64, y: f64| dz * (y - y0) / (y1 - y0);
            let (hs, he) = (at(dz_head, y0), at(dz_head, k));
            let (ss, se) = (at(dz_sill, y0), at(dz_sill, k));
            let side = vec![
                v(y0, ss - c),
                v(k, se - c),
                v(k, h + he + c),
                v(y0, h + hs + c),
            ];
            out.push(shaped(Part::Insulation, side.clone(), -c, 0.0));
            out.push(shaped(Part::Insulation, side, w, w + c));
            out.push(shaped(
                Part::Insulation,
                vec![
                    v(y0, h + hs),
                    v(k, h + he),
                    v(k, h + he + c),
                    v(y0, h + hs + c),
                ],
                0.0,
                w,
            ));
            out.push(shaped(
                Part::Insulation,
                vec![v(y0, ss - c), v(k, se - c), v(k, se), v(y0, ss)],
                0.0,
                w,
            ));
        }
        // the sides follow the head and sill
        let side = vec![v(y0, 0.0), v(y1, dz_sill), v(y1, h + dz_head), v(y0, h)];
        out.push(slat(side.clone(), 0.0, lt));
        out.push(slat(side, w - lt, w));
        // the head and the sill boards, between the sides
        out.push(slat(
            vec![
                v(y0, h - lt),
                v(y1, h + dz_head - lt),
                v(y1, h + dz_head),
                v(y0, h),
            ],
            lt,
            w - lt,
        ));
        out.push(slat(
            vec![v(y0, 0.0), v(y1, dz_sill), v(y1, dz_sill + lt), v(y0, lt)],
            lt,
            w - lt,
        ));
    }
    out
}

/// A roof window's glazing as `n` panes across its glass's depth: each
/// pane 4 mm, the unit as deep as its glass or deeper for three.
fn panes(g: &PartBox, n: u8) -> Vec<PartBox> {
    let n = n.clamp(1, 3) as usize;
    if n == 1 {
        return vec![*g];
    }
    let pane = 0.004;
    let mid = (g.lo.y + g.hi.y) / 2.0;
    let gap = (((g.hi.y - g.lo.y) - n as f64 * pane) / (n - 1) as f64).max(0.012);
    let total = n as f64 * pane + (n - 1) as f64 * gap;
    (0..n)
        .map(|i| {
            let y = mid - total / 2.0 + i as f64 * (pane + gap);
            PartBox {
                part: Part::Glass,
                lo: DVec3::new(g.lo.x, y, g.lo.z),
                hi: DVec3::new(g.hi.x, y + pane, g.hi.z),
            }
        })
        .collect()
}

/// A roof window's parts as meshes in model coordinates, one per kind of
/// part: the window with its glazing unit, its upstand, flashing and
/// insulation collars, and the lining round its hole.
pub fn roof_window_meshes(model: &creabim_core::Model, o: &Opening) -> Vec<(Part, TriMesh)> {
    let Some((local, wall, at)) = roof_window(model, o) else {
        return vec![];
    };
    let mut out: Vec<(Part, TriMesh)> = Vec::new();
    let mut put = |part: Part, s: Shaped| {
        let i = match out.iter().position(|(p, _)| *p == part) {
            Some(i) => i,
            None => {
                out.push((part, TriMesh::default()));
                out.len() - 1
            }
        };
        match &s {
            Shaped::Box(b) => add_box(&mut out[i].1, b.lo, b.hi, &at),
            Shaped::Prism(p) => add_prism(&mut out[i].1, p, &at),
            Shaped::Slat(p) => add_slat(&mut out[i].1, p, &at),
            Shaped::Plan(p) => add_plan(&mut out[i].1, p, &at),
        }
    };
    let pitch = creabim_core::roofwindow::seat(model, o).map_or(0.0, |s| s.pitch());
    for pb in opening_parts(&local, &wall) {
        // not a wall window's outside sill: on a slope it stood up from
        // the frame's lower edge and would hold the water back (MU127)
        if matches!(&pb, Shaped::Box(b) if is_outside_sill(b, &local)) {
            continue;
        }
        // the glass as its glazing unit's panes
        if let Shaped::Box(g) = &pb {
            if g.part == Part::Glass {
                for p in panes(g, local.roof_build.glazing) {
                    put(Part::Glass, Shaped::Box(p));
                }
                continue;
            }
        }
        put(pb.part(), pb);
    }
    for s in roof_build_parts(&local, &wall, pitch) {
        put(s.part(), s);
    }
    out
}

/// A roof window's outline on its roof's slope, in model coordinates: the
/// four corners of its frame.
pub fn roof_window_outline(model: &creabim_core::Model, o: &Opening) -> Option<[DVec3; 4]> {
    let (local, _, at) = roof_window(model, o)?;
    let (w, h) = (local.width, local.height);
    Some([
        at(DVec3::new(0.0, 0.0, 0.0)),
        at(DVec3::new(w, 0.0, 0.0)),
        at(DVec3::new(w, 0.0, h)),
        at(DVec3::new(0.0, 0.0, h)),
    ])
}

/// Where a roof window's lining meets its roof's underside, in plan: the
/// hole it needs there, its corners sill-side first. Splayed, the head
/// reaches further up the slope and the sill down it; square, the hole is
/// the frame's and this is none (Mockup_Tags_And_Splay_REV00, MU142).
pub fn roof_lining_hole(model: &creabim_core::Model, o: &Opening) -> Option<[DVec2; 4]> {
    let (local, wall, at) = roof_window(model, o)?;
    let f = Fitted::of(&local, &wall)?;
    let (w, h, t, depth) = (local.width, local.height, wall.thickness, f.frame.depth);
    let lift = roof_lift(&local, &wall, depth);
    // the lining runs from the upstand's foot (or the frame) to the
    // underside, as `roof_build_parts` builds it
    let (deck, under) = (-t / 2.0 + UPSTAND_SEAT + lift, t / 2.0 + lift);
    let y0 = (depth / 2.0).max(deck);
    let pitch = creabim_core::roofwindow::seat(model, o)?.pitch();
    let (head, sill) = roof_splay(local.roof_build.lining, pitch, under - y0);
    if head.abs() < 1e-9 && sill.abs() < 1e-9 {
        return None;
    }
    let p = |x: f64, z: f64| at(DVec3::new(x, under, z)).truncate();
    Some([p(0.0, sill), p(w, sill), p(w, h + head), p(0.0, h + head)])
}

/// A roof window looked up at from a ceiling plan under its roof: its
/// frame and glass as seen, and with a splayed lining the hole the lining
/// makes at the ceiling as a fine line, with a short line from each of the
/// frame's head corners to it: the splay seen from below (S1 of
/// Mockup_Tags_And_Splay_REV00, owner A on all, MU142).
pub fn roof_window_ceiling(
    model: &creabim_core::Model,
    id: creabim_core::ElementId,
    o: &Opening,
) -> Vec<crate::plan::PlanItem> {
    use crate::plan::{Pen, PlanItem};
    let mut out = roof_window_plan(model, id, o);
    let (Some(hole), Some((local, _, at))) = (roof_lining_hole(model, o), roof_window(model, o))
    else {
        return out;
    };
    out.push(PlanItem::Polygon {
        id,
        pen: Pen::Fine,
        points: hole.to_vec(),
    });
    // from the frame's head corners to the hole's head edge, drawn in a
    // little from its ends so the two read as a splay
    let (w, h) = (local.width, local.height);
    let (k0, k1) = (
        at(DVec3::new(0.0, 0.0, h)).truncate(),
        at(DVec3::new(w, 0.0, h)).truncate(),
    );
    let (h0, h1) = (hole[3], hole[2]);
    let inset = 0.14;
    for (from, to) in [(k0, h0 + (h1 - h0) * inset), (k1, h1 + (h0 - h1) * inset)] {
        if from.distance(to) > 1e-6 {
            out.push(PlanItem::Line {
                id,
                pen: Pen::Fine,
                a: from,
                b: to,
            });
        }
    }
    out
}

/// A roof window overhead in a floor plan under its roof, dashed as what
/// is overhead is: its frame and, with a splayed lining, the hole the
/// lining makes at the ceiling, which shows how much of the room it lights.
/// No glass (Mockup_Tags_And_Splay_REV00, owner A on all, MU142).
pub fn roof_window_overhead(
    model: &creabim_core::Model,
    id: creabim_core::ElementId,
    o: &Opening,
) -> Vec<crate::plan::PlanItem> {
    use crate::plan::{Pen, PlanItem};
    let mut out: Vec<PlanItem> = roof_window_plan(model, id, o).into_iter().take(1).collect();
    if let Some(hole) = roof_lining_hole(model, o) {
        out.push(PlanItem::Polygon {
            id,
            pen: Pen::Hidden,
            points: hole.to_vec(),
        });
    }
    for it in &mut out {
        if let PlanItem::Polygon { pen, .. } = it {
            *pen = Pen::Hidden;
        }
    }
    out
}

/// A roof window seen from above, in plan: its frame's outline and, inside
/// it, the sash or glass, each as it falls in plan.
pub fn roof_window_plan(
    model: &creabim_core::Model,
    id: creabim_core::ElementId,
    o: &Opening,
) -> Vec<crate::plan::PlanItem> {
    use crate::plan::{Pen, PlanItem};
    let Some((local, wall, at)) = roof_window(model, o) else {
        return vec![];
    };
    let Some(f) = Fitted::of(&local, &wall) else {
        return vec![];
    };
    let (w, h, fw) = (local.width, local.height, f.frame.width);
    let ring = |x0: f64, z0: f64, x1: f64, z1: f64| {
        [(x0, z0), (x1, z0), (x1, z1), (x0, z1)]
            .iter()
            .map(|&(x, z)| at(DVec3::new(x, 0.0, z)).truncate())
            .collect::<Vec<DVec2>>()
    };
    let mut out = vec![PlanItem::Polygon {
        id,
        pen: Pen::Projection,
        points: ring(0.0, 0.0, w, h),
    }];
    if w > 2.0 * fw && h > 2.0 * fw {
        out.push(PlanItem::Polygon {
            id,
            pen: Pen::Projection,
            points: ring(fw, fw, w - fw, h - fw),
        });
    }
    out
}

/// A shaped part as a closed solid, its corners mapped through `at` (a
/// rigid map).
/// A blade's section run along the opening (see [`PartSlat`]).
pub(crate) fn add_slat(m: &mut TriMesh, p: &PartSlat, at: &dyn Fn(DVec3) -> DVec3) {
    let o = at(DVec3::ZERO);
    let axis = |v: DVec3| at(v) - o;
    let (ex, ey, ez) = (axis(DVec3::X), axis(DVec3::Y), axis(DVec3::Z));
    let pt = |q: DVec2, x: f64| at(DVec3::new(x, q.x, q.y));
    let n = p.outline.len();
    let tris: Vec<[usize; 3]> = (1..n.saturating_sub(1)).map(|k| [0, k, k + 1]).collect();
    for (x, nn) in [(p.x0, -ex), (p.x1, ex)] {
        let pts: Vec<DVec3> = p.outline.iter().map(|q| pt(*q, x)).collect();
        m.add_polygon(&pts, &tris, nn);
    }
    let area: f64 = (0..n)
        .map(|k| p.outline[k].perp_dot(p.outline[(k + 1) % n]))
        .sum();
    let turn = if area >= 0.0 { 1.0 } else { -1.0 };
    for k in 0..n {
        let (a, b) = (p.outline[k], p.outline[(k + 1) % n]);
        let d = b - a;
        if d.length() < 1e-12 {
            continue;
        }
        let nn = (d.y * ey - d.x * ez) * turn;
        m.add_quad([pt(a, p.x0), pt(b, p.x0), pt(b, p.x1), pt(a, p.x1)], nn);
    }
}

/// A part shaped in plan, its outline stood from z0 to z1.
pub(crate) fn add_plan(m: &mut TriMesh, p: &PartPlan, at: &dyn Fn(DVec3) -> DVec3) {
    let o = at(DVec3::ZERO);
    let axis = |v: DVec3| at(v) - o;
    let (ex, ey, ez) = (axis(DVec3::X), axis(DVec3::Y), axis(DVec3::Z));
    let pt = |q: DVec2, z: f64| at(DVec3::new(q.x, q.y, z));
    let n = p.outline.len();
    let tris: Vec<[usize; 3]> = (1..n.saturating_sub(1)).map(|k| [0, k, k + 1]).collect();
    for (z, nn) in [(p.z0, -ez), (p.z1, ez)] {
        let pts: Vec<DVec3> = p.outline.iter().map(|q| pt(*q, z)).collect();
        m.add_polygon(&pts, &tris, nn);
    }
    let area: f64 = (0..n)
        .map(|k| p.outline[k].perp_dot(p.outline[(k + 1) % n]))
        .sum();
    let turn = if area >= 0.0 { 1.0 } else { -1.0 };
    for k in 0..n {
        let (a, b) = (p.outline[k], p.outline[(k + 1) % n]);
        let d = b - a;
        if d.length() < 1e-12 {
            continue;
        }
        let nn = (d.y * ex - d.x * ey) * turn;
        m.add_quad([pt(a, p.z0), pt(b, p.z0), pt(b, p.z1), pt(a, p.z1)], nn);
    }
}

pub(crate) fn add_prism(m: &mut TriMesh, p: &PartPrism, at: &dyn Fn(DVec3) -> DVec3) {
    let o = at(DVec3::ZERO);
    let axis = |v: DVec3| at(v) - o;
    let (ex, ey, ez) = (axis(DVec3::X), axis(DVec3::Y), axis(DVec3::Z));
    let pt = |q: DVec2, y: f64| at(DVec3::new(q.x, y, q.y));
    for (y, n) in [(p.y0, -ey), (p.y1, ey)] {
        let pts: Vec<DVec3> = p.outline.iter().map(|q| pt(*q, y)).collect();
        m.add_polygon(&pts, &p.tris, n);
    }
    // round its edge, each side facing out of the outline
    let area: f64 = (0..p.outline.len())
        .map(|k| p.outline[k].perp_dot(p.outline[(k + 1) % p.outline.len()]))
        .sum();
    let turn = if area >= 0.0 { 1.0 } else { -1.0 };
    for k in 0..p.outline.len() {
        let (a, b) = (p.outline[k], p.outline[(k + 1) % p.outline.len()]);
        let d = b - a;
        if d.length() < 1e-12 {
            continue;
        }
        let n = (d.y * ex - d.x * ez) * turn;
        m.add_quad([pt(a, p.y0), pt(b, p.y0), pt(b, p.y1), pt(a, p.y1)], n);
    }
}

/// A closed box from `lo` to `hi`, its corners mapped through `at` (a
/// rigid map, so the faces stay square).
pub(crate) fn add_box(m: &mut TriMesh, lo: DVec3, hi: DVec3, at: &dyn Fn(DVec3) -> DVec3) {
    let c = |x: f64, y: f64, z: f64| at(DVec3::new(x, y, z));
    let o = at(DVec3::ZERO);
    let axis = |v: DVec3| at(v) - o;
    let (ex, ey, ez) = (axis(DVec3::X), axis(DVec3::Y), axis(DVec3::Z));
    let (a, b) = (lo, hi);
    m.add_quad(
        [
            c(a.x, a.y, a.z),
            c(a.x, b.y, a.z),
            c(a.x, b.y, b.z),
            c(a.x, a.y, b.z),
        ],
        -ex,
    );
    m.add_quad(
        [
            c(b.x, a.y, a.z),
            c(b.x, b.y, a.z),
            c(b.x, b.y, b.z),
            c(b.x, a.y, b.z),
        ],
        ex,
    );
    m.add_quad(
        [
            c(a.x, a.y, a.z),
            c(b.x, a.y, a.z),
            c(b.x, a.y, b.z),
            c(a.x, a.y, b.z),
        ],
        -ey,
    );
    m.add_quad(
        [
            c(a.x, b.y, a.z),
            c(b.x, b.y, a.z),
            c(b.x, b.y, b.z),
            c(a.x, b.y, b.z),
        ],
        ey,
    );
    m.add_quad(
        [
            c(a.x, a.y, a.z),
            c(b.x, a.y, a.z),
            c(b.x, b.y, a.z),
            c(a.x, b.y, a.z),
        ],
        -ez,
    );
    m.add_quad(
        [
            c(a.x, a.y, b.z),
            c(b.x, a.y, b.z),
            c(b.x, b.y, b.z),
            c(a.x, b.y, b.z),
        ],
        ez,
    );
}

/// The hole an opening cuts, through the whole wall, as a box: what a
/// bare hole shows (see-through) and is picked by, and what the placement
/// ghost shows around a door or window (A7-4, MU18).
pub fn cut_box_mesh(o: &Opening, wall: &Wall, level_z: f64) -> Option<crate::TriMesh> {
    let c = crate::wall::Cut::of(o, wall)?;
    let t = wall.thickness / 2.0;
    // round the arc on a curved wall
    let n = if wall.is_arc() { 8 } else { 1 };
    let us: Vec<f64> = (0..=n)
        .map(|i| c.u0 + (c.u1 - c.u0) * i as f64 / n as f64)
        .collect();
    let mut outline: Vec<glam::DVec2> = us.iter().map(|u| wall.place(*u, -t)).collect();
    outline.extend(us.iter().rev().map(|u| wall.place(*u, t)));
    let z0 = level_z + wall.base_offset;
    Some(crate::scene::slab_mesh(&outline, z0 + c.v1, c.v1 - c.v0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use creabim_core::ElementId;

    /// MU142 (Mockup_Tags_And_Splay_REV00, A on all): a splayed roof
    /// window shows its lining in 2D. Looked up at from the ceiling plan
    /// under it: the hole at the ceiling as a fine line, longer up and
    /// down the slope than the frame and as wide, and two splay lines
    /// from the frame's head. Overhead in the floor plan: the frame and
    /// that hole dashed, no glass. Lined square, neither draws a hole.
    #[test]
    fn a_splayed_roof_window_shows_its_lining_in_plans() {
        use crate::plan::{Pen, PlanItem};
        use creabim_core::ElementKind;
        let mut m = creabim_core::ridgeline::ridgeline_house();
        let named = |m: &creabim_core::Model, n: &str| m.iter().find(|e| e.name == n).unwrap().id;
        let (ceiling, floor) = (
            named(&m, "First Floor ceiling plan"),
            named(&m, "First Floor plan"),
        );
        let (id, o) = m
            .iter()
            .find_map(|e| match &e.kind {
                ElementKind::Opening(o) if o.roof_at.is_some() => Some((e.id, o.clone())),
                _ => None,
            })
            .expect("a roof window");
        let of = |m: &creabim_core::Model, view: ElementId| -> Vec<PlanItem> {
            crate::section::view_drawing(m, view)
                .unwrap()
                .items
                .into_iter()
                .filter(|i| i.id() == id)
                .collect()
        };
        let pens = |items: &[PlanItem]| -> Vec<(bool, Pen)> {
            items
                .iter()
                .filter_map(|i| match i {
                    PlanItem::Polygon { pen, .. } => Some((true, *pen)),
                    PlanItem::Line { pen, .. } => Some((false, *pen)),
                    _ => None,
                })
                .collect()
        };
        let hole = roof_lining_hole(&m, &o).expect("splayed");
        let frame = roof_window_outline(&m, &o).unwrap().map(|p| p.truncate());
        // as wide as the frame, longer along the slope
        assert!((hole[0].distance(hole[1]) - frame[0].distance(frame[1])).abs() < 1e-6);
        assert!(hole[1].distance(hole[2]) > frame[1].distance(frame[2]) + 0.1);
        let up = pens(&of(&m, ceiling));
        assert!(up.contains(&(true, Pen::Fine)), "the hole: {up:?}");
        assert_eq!(
            up.iter().filter(|p| **p == (false, Pen::Fine)).count(),
            2,
            "the splay: {up:?}"
        );
        // frame and glass, seen
        assert_eq!(
            up.iter().filter(|p| **p == (true, Pen::Projection)).count(),
            2,
            "{up:?}"
        );
        let down = pens(&of(&m, floor));
        assert_eq!(
            down,
            vec![(true, Pen::Hidden), (true, Pen::Hidden)],
            "frame and hole dashed, no glass"
        );
        // lined square: no hole, no splay
        let e = m.get(id).unwrap().clone();
        let mut sq = e.clone();
        if let ElementKind::Opening(x) = &mut sq.kind {
            x.roof_build.lining = creabim_core::RoofLining::Square;
        }
        creabim_core::Transaction::new("square")
            .replace(e, sq)
            .apply(&mut m)
            .unwrap();
        assert!(!pens(&of(&m, ceiling)).iter().any(|p| p.1 == Pen::Fine));
        assert_eq!(pens(&of(&m, floor)), vec![(true, Pen::Hidden)]);
    }

    fn wall() -> Wall {
        Wall {
            level: ElementId::new(),
            start: DVec2::new(0.0, 0.0),
            end: DVec2::new(5.0, 0.0),
            thickness: 0.3,
            height: 3.0,
            base_offset: 0.0,
            material: None,
            assembly: None,
            flip: false,
            location: Default::default(),
            top: Default::default(),
            disallow_join: [false; 2],
            attach_top: vec![],
            curtain: None,
            layer_ends: vec![],
            bulge: 0.0,
        }
    }

    fn opening(kind: OpeningKind, frame: OpeningFrame) -> Opening {
        Opening {
            host: ElementId::new(),
            kind,
            offset: 1.0,
            sill: if kind == OpeningKind::Door { 0.0 } else { 0.9 },
            width: 1.2,
            height: if kind == OpeningKind::Door { 2.1 } else { 1.3 },
            material: None,
            frame,
            flip_hand: false,
            flip_facing: false,
            roof_at: None,
            roof_build: Default::default(),
            arrangement: None,
        }
    }

    fn count(boxes: &[PartBox], part: Part) -> usize {
        boxes.iter().filter(|b| b.part == part).count()
    }

    /// A corner of two walls with a window turned round it by the core's
    /// own call: the windows and their walls.
    fn corner_pair(joint: creabim_core::CornerJoint, end: bool) -> Vec<(Opening, Wall)> {
        use creabim_core::{Element, ElementKind, Level, Model, Transaction};
        let mut m = Model::default();
        let l = Element::new(
            "L",
            ElementKind::Level(Level {
                elevation: 0.0,
                height: 3.0,
            }),
        );
        let lid = l.id;
        let mk = |a: DVec2, b: DVec2| {
            let mut w = wall();
            w.level = lid;
            w.start = a;
            w.end = b;
            Element::new("W", ElementKind::Wall(w))
        };
        // anticlockwise, inside on the left: a 5 m wall, then one up from
        // its end and one down to its start
        let (a, b, c) = (
            mk(DVec2::new(0.0, 0.0), DVec2::new(5.0, 0.0)),
            mk(DVec2::new(5.0, 0.0), DVec2::new(5.0, 4.0)),
            mk(DVec2::new(0.0, 4.0), DVec2::new(0.0, 0.0)),
        );
        let mut o = opening(
            OpeningKind::Window,
            OpeningFrame::for_kind(OpeningKind::Window),
        );
        o.host = a.id;
        let win = Element::new("Win", ElementKind::Opening(o));
        let id = win.id;
        Transaction::new("s")
            .insert(l)
            .insert(a)
            .insert(b)
            .insert(c)
            .insert(win)
            .apply(&mut m)
            .unwrap();
        creabim_core::arrangement::add_round_corner(&m, id, end, joint)
            .unwrap()
            .apply(&mut m)
            .unwrap();
        m.iter()
            .filter_map(|e| match &e.kind {
                ElementKind::Opening(o) => match &m.get(o.host).unwrap().kind {
                    ElementKind::Wall(w) => Some((o.clone(), w.clone())),
                    _ => None,
                },
                _ => None,
            })
            .collect()
    }

    /// Each part's plan outline in the world, with its height range.
    fn world_plan(o: &Opening, w: &Wall) -> Vec<(Part, Vec<DVec2>, f64, f64)> {
        let at = to_model(o, w, 0.0).unwrap();
        let to = |q: DVec2| at(DVec3::new(q.x, q.y, 0.0)).truncate();
        opening_parts(o, w)
            .into_iter()
            .filter_map(|s| match s {
                Shaped::Box(b) => Some((
                    b.part,
                    vec![
                        to(DVec2::new(b.lo.x, b.lo.y)),
                        to(DVec2::new(b.hi.x, b.lo.y)),
                        to(DVec2::new(b.hi.x, b.hi.y)),
                        to(DVec2::new(b.lo.x, b.hi.y)),
                    ],
                    b.lo.z,
                    b.hi.z,
                )),
                Shaped::Plan(p) => Some((
                    p.part,
                    p.outline.iter().map(|q| to(*q)).collect(),
                    p.z0,
                    p.z1,
                )),
                _ => None,
            })
            .collect()
    }

    /// The area two convex outlines share.
    fn shared_area(a: &[DVec2], b: &[DVec2]) -> f64 {
        let area = |p: &[DVec2]| {
            (0..p.len())
                .map(|k| p[k].perp_dot(p[(k + 1) % p.len()]))
                .sum::<f64>()
                / 2.0
        };
        let turn = if area(b) >= 0.0 { 1.0 } else { -1.0 };
        let mut poly = a.to_vec();
        for k in 0..b.len() {
            let (e0, e1) = (b[k], b[(k + 1) % b.len()]);
            let f = |p: DVec2| (e1 - e0).perp_dot(p - e0) * turn;
            let mut next = vec![];
            for i in 0..poly.len() {
                let (p, q) = (poly[i], poly[(i + 1) % poly.len()]);
                let (fp, fq) = (f(p), f(q));
                if fp >= 0.0 {
                    next.push(p);
                }
                if (fp >= 0.0) != (fq >= 0.0) {
                    next.push(p + (q - p) * (fp / (fp - fq)));
                }
            }
            poly = next;
            if poly.len() < 3 {
                return 0.0;
            }
        }
        area(&poly).abs()
    }

    /// Part 9b: the windows of a box, angled and bow bay meet their
    /// neighbours on the mitre at every corner angle, nothing of one
    /// running into the next.
    #[test]
    fn bay_windows_meet_on_the_mitre_at_every_angle() {
        use creabim_core::arrangement::{make_bay, BayShape};
        use creabim_core::{CornerJoint, Element, ElementKind, Level, Model, Transaction};
        for shape in [BayShape::Box, BayShape::Angled, BayShape::Bow(5)] {
            for joint in [CornerJoint::Post, CornerJoint::Glass] {
                let mut m = Model::default();
                let l = Element::new(
                    "L",
                    ElementKind::Level(Level {
                        elevation: 0.0,
                        height: 3.0,
                    }),
                );
                let lid = l.id;
                let pts = [
                    DVec2::new(0.0, 0.0),
                    DVec2::new(6.0, 0.0),
                    DVec2::new(6.0, 4.0),
                    DVec2::new(0.0, 4.0),
                ];
                let mut tx = Transaction::new("s").insert(l);
                let mut south = None;
                for i in 0..4 {
                    let mut w = wall();
                    w.level = lid;
                    w.start = pts[i];
                    w.end = pts[(i + 1) % 4];
                    let e = Element::new("W", ElementKind::Wall(w));
                    if i == 0 {
                        south = Some(e.id);
                    }
                    tx = tx.insert(e);
                }
                let mut o = opening(
                    OpeningKind::Window,
                    OpeningFrame::for_kind(OpeningKind::Window),
                );
                o.host = south.unwrap();
                o.offset = 1.8;
                o.width = 2.4;
                let win = Element::new("Win", ElementKind::Opening(o));
                let id = win.id;
                tx.insert(win).apply(&mut m).unwrap();
                make_bay(&m, id, shape, 0.6, joint)
                    .unwrap()
                    .apply(&mut m)
                    .unwrap();
                let bay: Vec<(Opening, Wall)> = m
                    .iter()
                    .filter_map(|e| match &e.kind {
                        ElementKind::Opening(o) => match &m.get(o.host).unwrap().kind {
                            ElementKind::Wall(w) => Some((o.clone(), w.clone())),
                            _ => None,
                        },
                        _ => None,
                    })
                    .collect();
                let plans: Vec<_> = bay.iter().map(|(o, w)| world_plan(o, w)).collect();
                for i in 0..plans.len() {
                    for j in i + 1..plans.len() {
                        for (pa, a, a0, a1) in &plans[i] {
                            for (pb, b, b0, b1) in &plans[j] {
                                if a1.min(*b1) - a0.max(*b0) <= 1e-6 {
                                    continue;
                                }
                                let shared = shared_area(a, b);
                                assert!(
                                    shared < 1e-7,
                                    "{shape:?} {joint:?}: {pa:?} and {pb:?} overlap by {shared} m2"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    /// Part 9 (owner: "what can we do about their joins?"): two windows
    /// round a corner meet on the mitre. No member, glass or sill of one
    /// runs into the other's, at either corner of a wall, with a post or
    /// glass to glass; and they close the corner (the outside sills meet
    /// at the outer corner of the walls).
    #[test]
    fn windows_round_a_corner_meet_on_the_mitre_without_overlapping() {
        use creabim_core::CornerJoint;
        for joint in [CornerJoint::Post, CornerJoint::Glass] {
            for end in [true, false] {
                let pair = corner_pair(joint, end);
                assert_eq!(pair.len(), 2);
                let (pa, pb) = (
                    world_plan(&pair[0].0, &pair[0].1),
                    world_plan(&pair[1].0, &pair[1].1),
                );
                for (part_a, a, a0, a1) in &pa {
                    for (part_b, b, b0, b1) in &pb {
                        if a1.min(*b1) - a0.max(*b0) <= 1e-6 {
                            continue;
                        }
                        let shared = shared_area(a, b);
                        assert!(
                            shared < 1e-7,
                            "{joint:?} end {end}: {part_a:?} and {part_b:?} overlap by {shared} m2"
                        );
                    }
                }
                // the corner post: two jambs with a post, none glass to glass
                let full = |o: &Opening, w: &Wall| {
                    let h = Fitted::of(o, w).unwrap().height();
                    opening_parts(o, w)
                        .iter()
                        .filter(|s| match s {
                            Shaped::Plan(p) => {
                                p.part == Part::Frame && p.z0 < 1e-6 && (p.z1 - h).abs() < 1e-6
                            }
                            _ => false,
                        })
                        .count()
                };
                let jambs = full(&pair[0].0, &pair[0].1) + full(&pair[1].0, &pair[1].1);
                assert_eq!(
                    jambs,
                    if joint == CornerJoint::Post { 2 } else { 0 },
                    "{joint:?} end {end}"
                );
                // both outside sills reach the walls' outer corner, their
                // nose past it, and meet there
                let n = 0.15 + SILL_NOSE;
                let corner = if end {
                    DVec2::new(5.0 + n, -n)
                } else {
                    DVec2::new(-n, -n)
                };
                let at_corner = pa
                    .iter()
                    .chain(&pb)
                    .filter(|(part, pts, z0, z1)| {
                        *part == Part::Frame
                            && *z0 < 1e-6
                            && *z1 <= 0.031
                            && pts.iter().any(|q| q.distance(corner) < 1e-6)
                    })
                    .count();
                assert_eq!(
                    at_corner, 2,
                    "{joint:?} end {end}: the sills don't meet at the outer corner"
                );
            }
        }
    }

    #[test]
    fn an_arched_window_follows_its_head_and_its_glass_fills_the_arch() {
        use creabim_core::cells::Outline;
        let w = wall();
        let mut o = opening(
            OpeningKind::Window,
            OpeningFrame::for_kind(OpeningKind::Window),
        );
        o.frame.grid.outline = Outline::Round;
        let parts = opening_parts(&o, &w);
        let f = Fitted::of(&o, &w).unwrap();
        let (ww, h) = (f.width(), f.height());
        let outer = Outline::Round.head(ww, h, 0.0);
        let top = |x: f64| creabim_core::cells::head_z_on(&outer, x);
        // nothing pokes out past the arch
        for p in &parts {
            match p {
                Shaped::Box(b) => {
                    assert!(
                        b.hi.z <= top(b.lo.x) + 1e-9 && b.hi.z <= top(b.hi.x) + 1e-9,
                        "{b:?}"
                    )
                }
                Shaped::Prism(p) => {
                    for q in &p.outline {
                        assert!(q.y <= top(q.x) + 1e-9, "{p:?}");
                    }
                }
                Shaped::Plan(p) => panic!("no mitre without a corner: {p:?}"),
                Shaped::Slat(p) => {
                    for q in &p.outline {
                        assert!(q.y <= top(p.x0).min(top(p.x1)) + 1e-9, "{p:?}");
                    }
                }
            }
        }
        // a curved head, and the glass shaped to its inside
        let prisms: Vec<&PartPrism> = parts
            .iter()
            .filter_map(|p| match p {
                Shaped::Prism(p) => Some(p),
                _ => None,
            })
            .collect();
        assert!(prisms
            .iter()
            .any(|p| p.part == Part::Frame && p.outline.len() > 30));
        let glass = prisms.iter().find(|p| p.part == Part::Glass).unwrap();
        let crown = glass.outline.iter().map(|q| q.y).fold(0.0, f64::max);
        assert!((crown - (h - f.frame.width)).abs() < 1e-6, "{crown}");
        // each part a closed solid
        for (part, m) in opening_meshes(&o, &w, 0.0) {
            assert!(m.is_closed(), "{part:?}");
        }
        // a square window is all boxes, as before
        o.frame.grid.outline = Outline::Rectangle;
        assert!(opening_parts(&o, &w)
            .iter()
            .all(|p| matches!(p, Shaped::Box(_))));
    }

    #[test]
    fn louvre_blades_lap_evenly_and_stand_clear_of_head_and_sill() {
        let (h, d) = blade_extent();
        for height in [0.30, 0.6, 0.9, 1.1, 1.3, 1.8, 2.4] {
            let z0 = 0.05;
            let z1 = z0 + height;
            let bl = louvre_blades(z0, z1);
            assert!(!bl.is_empty(), "{height}");
            // the first and last the same way off sill and head
            let below = bl[0].0 - z0;
            let above = z1 - bl[bl.len() - 1].1;
            assert!((below - above).abs() < 1e-9, "{height}: {below} {above}");
            assert!(
                below >= BLADE_CLEAR - 1e-9 && above >= BLADE_CLEAR - 1e-9,
                "{height}"
            );
            // each laps the next by close to 15 mm, all by the same
            let laps: Vec<f64> = bl.windows(2).map(|w| w[0].1 - w[1].0).collect();
            for l in &laps {
                assert!((l - laps[0]).abs() < 1e-9);
                assert!(
                    *l >= 0.010 - 1e-9 && *l <= 0.025 + 1e-9,
                    "{height}: lap {l}"
                );
            }
            // the shapes: inside their boxes, no blade crossing another
            let shapes: Vec<Vec<DVec2>> = bl
                .iter()
                .map(|(za, _)| blade_outline(-d / 2.0, d / 2.0, *za, 1.0))
                .collect();
            for (k, sh) in shapes.iter().enumerate() {
                for q in sh {
                    assert!(q.x >= -d / 2.0 - 1e-9 && q.x <= d / 2.0 + 1e-9, "{q:?}");
                    assert!(q.y >= bl[k].0 - 1e-9 && q.y <= bl[k].0 + h + 1e-9, "{q:?}");
                }
                if k > 0 {
                    // the blade above lies outside the one below where they lap
                    let low = &shapes[k - 1];
                    let up = sh;
                    let face_out_low = (low[0], low[1]);
                    let side =
                        |p: DVec2| (face_out_low.1 - face_out_low.0).perp_dot(p - face_out_low.0);
                    for q in up {
                        assert!(side(*q) <= 1e-9, "blade {k} crosses the one below");
                    }
                }
            }
        }
    }

    #[test]
    fn louvre_blades_shed_rain_to_the_side_the_outside_sill_is_on() {
        // MU105: on a wall facing the other way the blades were reversed
        for flip in [false, true] {
            let mut o = window(creabim_core::cells::WindowStyle::Louvred, false);
            o.flip_facing = flip;
            let w = wall();
            let parts = opening_parts(&o, &w);
            // the outside sill: the frame part lowest and furthest out
            let sill = parts
                .iter()
                .filter_map(|p| match p {
                    Shaped::Box(b) if b.part == Part::Frame && b.hi.z <= 0.031 => {
                        Some((b.lo.y + b.hi.y) / 2.0)
                    }
                    _ => None,
                })
                .max_by(|a, b| a.abs().total_cmp(&b.abs()))
                .expect("an outside sill");
            for p in &parts {
                if let Shaped::Slat(sl) = p {
                    let low = sl
                        .outline
                        .iter()
                        .min_by(|a, b| a.y.total_cmp(&b.y))
                        .unwrap();
                    let high = sl
                        .outline
                        .iter()
                        .max_by(|a, b| a.y.total_cmp(&b.y))
                        .unwrap();
                    assert!(
                        (low.x - high.x) * sill.signum() > 0.03,
                        "flip {flip}: lower edge {low:?} not toward the sill at {sill}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_louvred_window_is_built_of_tilted_blades_in_closed_solids() {
        let o = window(creabim_core::cells::WindowStyle::Louvred, false);
        let w = wall();
        let slats: Vec<PartSlat> = opening_parts(&o, &w)
            .into_iter()
            .filter_map(|p| match p {
                Shaped::Slat(s) => Some(s),
                _ => None,
            })
            .collect();
        assert!(slats.len() >= 5, "{}", slats.len());
        // tilted (which way: see the test on the outside sill)
        let s = &slats[0];
        let low = s.outline.iter().min_by(|a, b| a.y.total_cmp(&b.y)).unwrap();
        let high = s.outline.iter().max_by(|a, b| a.y.total_cmp(&b.y)).unwrap();
        assert!((low.x - high.x).abs() > 0.03, "{low:?} {high:?}");
        for (part, m) in opening_meshes(&o, &w, 0.0) {
            assert!(m.is_closed(), "{part:?}");
        }
    }

    #[test]
    fn an_all_glass_leaf_is_glass_in_its_fittings_with_a_pull_handle() {
        use creabim_core::construction::{GlassFittings as G, LeafStyle as L};
        let w = wall();
        let mut o = opening(OpeningKind::Door, OpeningFrame::for_kind(OpeningKind::Door));
        o.frame.construction.leaf = L::AllGlass;
        for (fit, metal) in [(G::Patch, 4), (G::BottomRail, 2), (G::Rails, 2)] {
            o.frame.construction.fittings = fit;
            let boxes = opening_boxes(&o, &w);
            // no timber leaf: a sheet of glass 12 mm thick
            assert_eq!(count(&boxes, Part::Leaf), 0, "{fit:?}");
            let glass = boxes.iter().find(|b| b.part == Part::Glass).unwrap();
            assert!((glass.hi.y - glass.lo.y - ALL_GLASS).abs() < 1e-9);
            assert!(glass.hi.z - glass.lo.z > 1.9);
            // its fittings, and an upright pull handle on each face
            let handle: Vec<&PartBox> = boxes.iter().filter(|b| b.part == Part::Handle).collect();
            let bars = handle.iter().filter(|b| b.hi.z - b.lo.z > 0.9).count();
            assert_eq!(bars, 2, "{fit:?}");
            assert_eq!(handle.len(), metal + 2 + 4, "{fit:?}");
            assert_eq!(hung_leaves(&o, &w).len(), 1);
        }
        // a rail runs the leaf's width; a patch is short
        o.frame.construction.fittings = G::Rails;
        let boxes = opening_boxes(&o, &w);
        let widest = boxes
            .iter()
            .filter(|b| b.part == Part::Handle)
            .map(|b| b.hi.x - b.lo.x)
            .fold(0.0, f64::max);
        assert!(widest > 0.7, "{widest}");
        // a pair has a pull handle on each leaf
        o.frame = OpeningFrame::of_operation(OpeningKind::Door, Operation::DoubleActionPair);
        o.frame.construction.leaf = L::AllGlass;
        o.width = 1.8;
        let bars = opening_boxes(&o, &w)
            .iter()
            .filter(|b| b.part == Part::Handle && b.hi.z - b.lo.z > 0.9)
            .count();
        assert_eq!(bars, 4);
    }

    #[test]
    fn french_and_slim_steel_windows_divide_their_glass_into_panes() {
        use creabim_core::cells::WindowStyle;
        use creabim_core::construction::LeafStyle as L;
        let w = wall();
        let mut frame = OpeningFrame::for_kind(OpeningKind::Window);
        frame.grid = WindowStyle::Casement.grid(false, [1, 1]);
        let mut o = opening(OpeningKind::Window, frame);
        let glass = |o: &Opening| {
            opening_boxes(o, &w)
                .iter()
                .filter(|b| b.part == Part::Glass)
                .count()
        };
        // as before: one pane
        assert_eq!(glass(&o), 1);
        o.frame.construction.leaf = L::French;
        o.frame.construction.panes = Some([2, 3]);
        assert_eq!(glass(&o), 6);
        // panes set on the cell itself still win
        o.frame.set_panes(1, 2);
        assert_eq!(glass(&o), 2);
        o.frame.set_panes(1, 1);
        // slim steel: thinner sash members
        let sash = |o: &Opening| {
            opening_boxes(o, &w)
                .iter()
                .filter(|b| b.part == Part::Leaf)
                .map(|b| b.hi.x - b.lo.x)
                .fold(f64::MAX, f64::min)
        };
        let french = sash(&o);
        o.frame.construction.leaf = L::SlimSteel;
        assert!(sash(&o) < french, "{} {french}", sash(&o));
        assert_eq!(glass(&o), 6);
    }

    #[test]
    fn french_and_slim_steel_leaves_divide_their_glass_into_panes() {
        use creabim_core::construction::LeafStyle as L;
        let w = wall();
        let mut o = opening(OpeningKind::Door, OpeningFrame::for_kind(OpeningKind::Door));
        let bars = |o: &Opening| {
            opening_boxes(o, &w)
                .iter()
                .filter(|b| b.part == Part::Leaf)
                .count()
                - 4
        };
        for style in [L::French, L::SlimSteel] {
            o.frame.construction.leaf = style;
            o.frame.construction.panes = Some([3, 4]);
            // two bars up, three across, inside the stiles and rails
            assert_eq!(bars(&o), 2 + 3, "{style:?}");
            o.frame.construction.panes = None;
            assert!(bars(&o) >= 2, "{style:?}");
        }
        // slim steel's stiles are thinner
        let stile = |style| {
            let mut o = o.clone();
            o.frame.construction.leaf = style;
            let b = opening_boxes(&o, &w);
            let glass = b.iter().find(|b| b.part == Part::Glass).unwrap();
            let leaf = b
                .iter()
                .filter(|b| b.part == Part::Leaf)
                .map(|b| b.lo.x)
                .fold(f64::MAX, f64::min);
            glass.lo.x - leaf
        };
        assert!(stile(L::SlimSteel) < stile(L::French) / 2.0);
    }

    #[test]
    fn leaves_are_made_as_their_style_says_inside_their_slab() {
        use creabim_core::construction::LeafStyle as L;
        let w = wall();
        let flush = opening(OpeningKind::Door, OpeningFrame::for_kind(OpeningKind::Door));
        let slab = opening_boxes(&flush, &w);
        let leaf = *slab.iter().find(|b| b.part == Part::Leaf).unwrap();
        for (style, glass) in [
            (L::Panelled, false),
            (L::Glazed, true),
            (L::PartGlazed, true),
            (L::Louvred, false),
            (L::VisionPanel, true),
            (L::French, true),
            (L::SlimSteel, true),
        ] {
            let mut o = flush.clone();
            o.frame.construction.leaf = style;
            let boxes = opening_boxes(&o, &w);
            let parts: Vec<&PartBox> = boxes
                .iter()
                .filter(|b| matches!(b.part, Part::Leaf | Part::Glass))
                .collect();
            assert!(parts.len() > 3, "{style:?}");
            // still one leaf, hung and scheduled as before
            assert_eq!(hung_leaves(&o, &w).len(), 1, "{style:?}");
            assert_eq!(operation_text(&o, &w), "Single hinged");
            assert_eq!(
                parts.iter().any(|b| b.part == Part::Glass),
                glass,
                "{style:?}"
            );
            // all within the flush leaf's slab
            for p in parts {
                assert!(
                    (p.lo - leaf.lo).min_element() > -1e-9
                        && (leaf.hi - p.hi).min_element() > -1e-9,
                    "{style:?} {p:?}"
                );
            }
        }
    }

    #[test]
    fn trim_goes_on_the_room_side_and_a_threshold_lifts_the_leaf() {
        use creabim_core::construction::Trim;
        let mut w = wall();
        let mut o = opening(OpeningKind::Door, OpeningFrame::for_kind(OpeningKind::Door));
        o.frame.construction.trim = Trim {
            lining: 0.018,
            architrave: 0.07,
            threshold: 0.02,
        };
        let boxes = opening_boxes(&o, &w);
        let trims: Vec<&PartBox> = boxes.iter().filter(|b| b.part == Part::Trim).collect();
        // two jamb linings and a head; two architraves and a head; a threshold
        assert_eq!(trims.len(), 7);
        // the room is on the wall's left: everything but the threshold there
        let face = w.thickness / 2.0;
        let arch: Vec<&&PartBox> = trims.iter().filter(|b| b.lo.y >= face - 1e-9).collect();
        assert_eq!(arch.len(), 3);
        assert!(arch.iter().any(|b| (b.lo.x + 0.07).abs() < 1e-9));
        assert!(arch
            .iter()
            .any(|b| (b.hi.z - (o.height + 0.07)).abs() < 1e-9));
        let leaf = boxes.iter().find(|b| b.part == Part::Leaf).unwrap();
        assert!(leaf.lo.z > 0.02);
        // a flipped wall has its room on the right
        w.flip = true;
        let boxes = opening_boxes(&o, &w);
        assert_eq!(
            boxes
                .iter()
                .filter(|b| b.part == Part::Trim && b.hi.y <= -face + 1e-9)
                .count(),
            3
        );
        // and none of it when there is none
        o.frame.construction.trim = Trim::default();
        assert!(opening_boxes(&o, &wall())
            .iter()
            .all(|b| b.part != Part::Trim));
    }

    #[test]
    fn a_window_has_a_frame_round_set_back_glass() {
        let o = opening(
            OpeningKind::Window,
            OpeningFrame::for_kind(OpeningKind::Window),
        );
        let boxes = opening_boxes(&o, &wall());
        // four frame members and the outside sill
        assert_eq!(count(&boxes, Part::Frame), 5);
        assert_eq!(count(&boxes, Part::Glass), 1);
        let glass = boxes.iter().find(|b| b.part == Part::Glass).unwrap();
        // the glass fills the frame's inside, behind its faces
        assert!((glass.lo.x - 0.06).abs() < 1e-9 && (glass.hi.x - 1.14).abs() < 1e-9);
        assert!(glass.hi.y < 0.05 && glass.lo.y > -0.05);
        // everything stays inside the opening
        for b in &boxes {
            assert!(b.lo.x >= -1e-9 && b.hi.x <= 1.2 + 1e-9);
            assert!(b.lo.z >= -1e-9 && b.hi.z <= 1.3 + 1e-9);
        }
    }

    #[test]
    fn panes_split_the_glass_with_bars() {
        let frame = OpeningFrame::for_kind(OpeningKind::Window).with_panes(3, 2);
        let boxes = opening_boxes(&opening(OpeningKind::Window, frame), &wall());
        assert_eq!(count(&boxes, Part::Glass), 6);
        // the frame and sill, two upright bars, a cross bar in each column
        assert_eq!(count(&boxes, Part::Frame), 5 + 2 + 3);
        let glass: f64 = boxes
            .iter()
            .filter(|b| b.part == Part::Glass)
            .map(|b| (b.hi.x - b.lo.x) * (b.hi.z - b.lo.z))
            .sum();
        let bar = 0.06 * 0.6;
        let inside = (1.2 - 0.12 - 2.0 * bar) * (1.3 - 0.12 - bar);
        assert!((glass - inside).abs() < 1e-9);
    }

    #[test]
    fn a_door_has_a_leaf_and_handles_but_no_sill() {
        let o = opening(OpeningKind::Door, OpeningFrame::for_kind(OpeningKind::Door));
        let boxes = opening_boxes(&o, &wall());
        assert_eq!(count(&boxes, Part::Frame), 3);
        assert_eq!(count(&boxes, Part::Leaf), 1);
        assert_eq!(count(&boxes, Part::Glass), 0);
        assert!(count(&boxes, Part::Handle) >= 2);
        let meshes = opening_meshes(&o, &wall(), 0.0);
        // every part is a closed box
        for (_, m) in &meshes {
            assert!(m.volume() > 0.0);
        }
    }

    fn double(share: f64) -> Opening {
        let mut frame = OpeningFrame::of_operation(OpeningKind::Door, Operation::DoubleSwing);
        frame.set_active_share(share);
        Opening {
            width: 1.6,
            ..opening(OpeningKind::Door, frame)
        }
    }

    #[test]
    fn a_double_door_has_two_leaves_split_at_the_active_share() {
        let o = double(2.0 / 3.0);
        let boxes = opening_boxes(&o, &wall());
        assert_eq!(count(&boxes, Part::Leaf), 2);
        let hung = hung_leaves(&o, &wall());
        assert_eq!(hung.len(), 2);
        let wide = |h: &Hung| h.leaf.hi.x - h.leaf.lo.x;
        // 1.6 less two 60 mm jambs, split two to one
        let clear = 1.6 - 0.12;
        assert!((wide(&hung[0]) - clear * 2.0 / 3.0).abs() < 0.01);
        assert!((wide(&hung[1]) - clear / 3.0).abs() < 0.01);
        // each turns about its own jamb, the wider one active
        assert!(hung[0].active && !hung[1].active);
        assert!(hung[0].hinge < 0.1 && hung[1].hinge > 1.5);
        assert_eq!((hung[0].towards, hung[1].towards), (1.0, -1.0));
        // the handle is on the active leaf, near the meeting stiles
        let handle = boxes.iter().find(|b| b.part == Part::Handle).unwrap();
        assert!(handle.lo.x > 0.8 && handle.hi.x < hung[0].leaf.hi.x + 1e-9);
        // flipping the hand puts the active leaf on the other jamb
        let mut f = o.clone();
        f.flip_hand = true;
        let hung = hung_leaves(&f, &wall());
        assert!(!hung[0].active && hung[1].active);
        assert!((wide(&hung[1]) - clear * 2.0 / 3.0).abs() < 0.01);
    }

    #[test]
    fn equal_leaves_and_out_of_range_shares() {
        let hung = hung_leaves(&double(0.5), &wall());
        assert!(
            (hung[0].leaf.hi.x - hung[0].leaf.lo.x - (hung[1].leaf.hi.x - hung[1].leaf.lo.x)).abs()
                < 1e-9
        );
        // an active leaf is never the smaller one, nor nearly all the door
        let hung = hung_leaves(&double(0.2), &wall());
        assert!(
            (hung[0].leaf.hi.x - hung[0].leaf.lo.x - (hung[1].leaf.hi.x - hung[1].leaf.lo.x)).abs()
                < 1e-9
        );
        let hung = hung_leaves(&double(0.99), &wall());
        assert!(hung[1].leaf.hi.x - hung[1].leaf.lo.x > 0.2);
    }

    #[test]
    fn a_single_door_is_hung_on_its_hand() {
        let mut o = opening(OpeningKind::Door, OpeningFrame::for_kind(OpeningKind::Door));
        let h = hung_leaves(&o, &wall());
        assert_eq!(h.len(), 1);
        assert!(h[0].active && h[0].hinge < 0.1 && h[0].towards == 1.0);
        o.flip_hand = true;
        let h = hung_leaves(&o, &wall());
        assert!(h[0].hinge > 1.1 && h[0].towards == -1.0);
    }

    #[test]
    fn leaf_marks_point_at_the_hinge_and_say_which_way_it_opens() {
        // a pair opening to the wall's left (+y here)
        let o = double(0.5);
        let from_left = leaf_marks(&o, &wall(), 0.0, DVec3::Y);
        assert_eq!(from_left.len(), 2);
        assert!(from_left.iter().all(|(toward, _)| *toward));
        let from_right = leaf_marks(&o, &wall(), 0.0, -DVec3::Y);
        assert!(from_right.iter().all(|(toward, _)| !*toward));
        // the first leaf's apex is on its hinge jamb, near the opening's start
        let [a, apex, b] = from_left[0].1;
        assert!(apex.x < a.x && apex.x < b.x);
        assert!((apex.x - 1.06).abs() < 0.01, "{apex}");
        assert!((apex.z - (a.z + b.z) / 2.0).abs() < 1e-9);
        // and the second's on the far jamb
        let apex2 = from_left[1].1[1];
        assert!((apex2.x - 2.536).abs() < 0.01, "{apex2}");
        // in front of the leaf on the viewer's side
        assert!(apex.y > 0.0);
        // looked at edge on: nothing
        assert!(leaf_marks(&o, &wall(), 0.0, DVec3::X).is_empty());
        // windows get none yet
        let w = opening(
            OpeningKind::Window,
            OpeningFrame::for_kind(OpeningKind::Window),
        );
        assert!(leaf_marks(&w, &wall(), 0.0, DVec3::Y).is_empty());
    }

    /// A pair (1000 active + 500) with a sidelight on its start side and
    /// a transom light over both, 2.0 wide and 2.6 high.
    fn assembly_door() -> Opening {
        use creabim_core::cells::{Assembly, Cell};
        let mut frame = OpeningFrame::of_operation(OpeningKind::Door, Operation::DoubleSwing);
        frame.set_active_share(0.667);
        frame.grid = Assembly {
            left: Some(0.25),
            transom: Some((0.2, Cell::default())),
            ..Assembly::of(frame.grid.clone())
        }
        .grid();
        Opening {
            width: 2.0,
            height: 2.6,
            ..opening(OpeningKind::Door, frame)
        }
    }

    #[test]
    fn a_door_with_a_sidelight_and_transom_hangs_its_leaves_in_its_own_part() {
        let o = assembly_door();
        let w = wall();
        let (core, a) = door_core(&o, &w).expect("an assembly");
        assert_eq!(a.columns, (1, 2));
        // the door stands right of the sidelight and under the transom
        let f = Fitted::of(&o, &w).unwrap();
        let (x0, x1, z1) = core_rect(&f, &a);
        assert!((core.offset - (o.offset + x0)).abs() < 1e-9);
        assert!(x0 > 0.4 && x1 == 2.0 && z1 > 1.9 && z1 < 2.2);
        // two leaves, both inside the door's part
        let hung = hung_leaves(&o, &w);
        assert_eq!(hung.len(), 2);
        assert!(hung
            .iter()
            .all(|h| h.leaf.lo.x >= x0 && h.leaf.hi.x <= x1 + 1e-9));
        assert!(hung.iter().all(|h| h.leaf.hi.z < z1));
        // the leaves keep the core's share: about two thirds and one third
        assert_eq!(
            operation_text(&o, &w),
            format!("{}, with sidelight and transom", operation_text(&core, &w))
        );
        let boxes = opening_boxes(&o, &w);
        assert_eq!(count(&boxes, Part::Leaf), 2);
        // glass in the sidelight (left of the door) and in the transom
        let glass: Vec<&PartBox> = boxes.iter().filter(|b| b.part == Part::Glass).collect();
        assert!(glass.iter().any(|b| b.hi.x <= x0 && b.lo.z < 1.0));
        assert!(glass
            .iter()
            .any(|b| b.lo.z >= z1 - 1e-9 && b.hi.x - b.lo.x > 1.5));
        // flipping its hand hangs the door on its other jamb but leaves the
        // sidelight where it is
        let flipped = Opening {
            flip_hand: true,
            ..o.clone()
        };
        let fb = opening_boxes(&flipped, &w);
        assert!(fb
            .iter()
            .any(|b| b.part == Part::Glass && b.hi.x <= x0 && b.lo.z < 1.0));
        let fh = hung_leaves(&flipped, &w);
        assert!(fh.iter().all(|h| h.leaf.lo.x >= x0 - 1e-9));
        assert_ne!(
            fh.iter().find(|h| h.active).unwrap().hinge,
            hung.iter().find(|h| h.active).unwrap().hinge
        );
        // the outer frame stands beside the sidelight down to the floor,
        // and the door has its own jambs
        assert!(boxes
            .iter()
            .any(|b| b.part == Part::Frame && b.lo.x == 0.0 && b.lo.z == 0.0));
        // drawings see the door the same, with the lights round it
        assert!(!light_cells(&o, &w).is_empty());
        assert_eq!(door_hand(&o, &w), door_hand(&core, &w));
    }

    #[test]
    fn a_sliding_door_with_a_sidelight_slides_and_pockets_in_its_own_part() {
        use creabim_core::cells::Assembly;
        let mut frame = OpeningFrame::of_operation(OpeningKind::Door, Operation::Pocket);
        frame.grid = Assembly {
            right: Some(0.3),
            ..Assembly::of(frame.grid.clone())
        }
        .grid();
        let o = Opening {
            width: 1.4,
            ..opening(OpeningKind::Door, frame)
        };
        let w = wall();
        assert_eq!(o.frame.operation(o.kind), Operation::Pocket);
        let s = sliding_leaves(&o, &w);
        assert_eq!(s.len(), 1);
        // the leaf is the assembly's first cell, and slides into the wall
        // past the door's start, away from the sidelight
        assert_eq!(s[0].cell, 0);
        assert!(s[0].travel < 0.0);
        assert_eq!(pockets(&o, &w).len(), 1);
        assert_eq!(pocket_cells(&o), vec![0]);
        assert!(pocket_problems(&o, 0, &w, &[]).is_empty());
        // changing what the door does keeps its sidelight
        let mut o2 = o.clone();
        o2.frame
            .set_operation(OpeningKind::Door, Operation::DoubleSwing);
        assert!(o2.frame.assembly().is_some_and(|a| a.right.is_some()));
        assert_eq!(hung_leaves(&o2, &w).len(), 2);
    }

    #[test]
    fn a_window_names_its_lights() {
        use creabim_core::cells::{Assembly, Cell, Mechanism, WindowStyle};
        let mut o = opening(
            OpeningKind::Window,
            OpeningFrame::for_kind(OpeningKind::Window),
        );
        o.frame.grid = Assembly {
            left: Some(0.2),
            right: Some(0.2),
            transom: Some((0.25, Cell::new(Mechanism::TopHung))),
            ..Assembly::of(WindowStyle::CasementPair.grid(false, [1, 1]))
        }
        .grid();
        assert_eq!(
            window_text(&o),
            "Casement pair, with sidelights and awning transom"
        );
        // the transom is one light over all four columns: one sash
        let f = Fitted::of(&o, &wall()).unwrap();
        let cells = window_cells(&f);
        assert_eq!(cells.len(), 5);
        let top = cells.iter().find(|c| c.at.1 == 1).unwrap();
        assert!((top.x1 - top.x0) > 1.0);
        // its triangle: an awning's, the apex at its top
        let marks = window_marks(
            &o,
            &wall(),
            0.0,
            DVec3::Y,
            creabim_core::TriangleApex::Hinge,
        );
        assert_eq!(marks.len(), 6);
    }

    #[test]
    fn pivot_double_action_and_folding_doors_hang_their_leaves() {
        let door = |op: Operation, width: f64| Opening {
            width,
            ..opening(
                OpeningKind::Door,
                OpeningFrame::of_operation(OpeningKind::Door, op),
            )
        };
        let w = wall();
        // a pivot leaf sits in the middle of the frame, turning about a
        // pivot a sixth of the way in
        let o = door(Operation::Pivot, 1.2);
        let h = hung_leaves(&o, &w);
        assert_eq!(h.len(), 1);
        let l = h[0].leaf;
        assert!((l.lo.y + l.hi.y).abs() < 1e-9);
        let Swing::Pivot(p) = h[0].swing else {
            panic!("{:?}", h[0].swing)
        };
        assert!((p - (l.hi.x - l.lo.x) / 6.0).abs() < 1e-9);
        assert_eq!(operation_text(&o, &w), "Offset pivot");
        assert_eq!(door_hand(&o, &w), "Right, in");
        // two triangles in elevation, pointing at the pivot from both
        // edges, one each way
        let marks = leaf_marks(&o, &w, 0.0, DVec3::Y);
        assert_eq!(marks.len(), 2);
        assert_ne!(marks[0].0, marks[1].0);
        let o = door(Operation::CentrePivot, 1.2);
        let h = hung_leaves(&o, &w);
        assert!(
            matches!(h[0].swing, Swing::Pivot(p) if (p - (h[0].leaf.hi.x - h[0].leaf.lo.x) / 2.0).abs() < 1e-9)
        );
        // double action: in the middle of the frame, push plates, both ways
        let o = door(Operation::DoubleAction, 0.9);
        let h = hung_leaves(&o, &w);
        assert_eq!((h.len(), h[0].swing), (1, Swing::DoubleAction));
        assert_eq!(door_hand(&o, &w), "Right, both ways");
        assert_eq!(count(&opening_boxes(&o, &w), Part::Handle), 2);
        let marks = leaf_marks(&o, &w, 0.0, -DVec3::Y);
        assert!(marks.iter().all(|m| m.0), "solid from either side");
        let o = door(Operation::DoubleActionPair, 1.6);
        assert_eq!(hung_leaves(&o, &w).len(), 2);
        assert!(operation_text(&o, &w).starts_with("Double action pair, "));
        // folding doors: equal panels, every one folding to the hand's
        // jamb, the handle on the leading panel
        let o = door(Operation::Bifold, 1.0);
        let h = hung_leaves(&o, &w);
        assert_eq!(h.len(), 2);
        assert!(h.iter().all(|h| h.towards > 0.0));
        assert_eq!(h.iter().position(|h| h.active), Some(1));
        assert_eq!(operation_text(&o, &w), "Bifold");
        assert_eq!(door_hand(&o, &w), "Folds right, in");
        let mut f = o.clone();
        f.flip_hand = true;
        let h = hung_leaves(&f, &w);
        assert!(h.iter().all(|h| h.towards < 0.0));
        assert_eq!(h.iter().position(|h| h.active), Some(0));
        let o = door(Operation::BifoldPair, 2.0);
        let h = hung_leaves(&o, &w);
        let towards: Vec<f64> = h.iter().map(|h| h.towards).collect();
        assert_eq!(towards, [1.0, 1.0, -1.0, -1.0]);
        assert_eq!(h.iter().position(|h| h.active), Some(1));
        assert_eq!(door_hand(&o, &w), "Parting, in");
        let mut o = door(Operation::Folding, 3.0);
        o.frame.set_folding_panels(OpeningKind::Door, 5);
        let h = hung_leaves(&o, &w);
        assert_eq!(h.len(), 5);
        let wide: Vec<f64> = h.iter().map(|h| h.leaf.hi.x - h.leaf.lo.x).collect();
        assert!(wide.iter().all(|x| (x - wide[1]).abs() < 0.003), "{wide:?}");
        assert_eq!(operation_text(&o, &w), "Folding, 5 panels");
        // its panels fold alternately, so the marks alternate
        let marks = leaf_marks(&o, &w, 0.0, DVec3::Y);
        assert_eq!(marks.len(), 5);
        assert!(marks.windows(2).all(|p| p[0].0 != p[1].0));
        // a single handle, on the last panel
        let handles: Vec<PartBox> = opening_boxes(&o, &w)
            .into_iter()
            .filter(|b| b.part == Part::Handle)
            .collect();
        assert!(handles.iter().all(|b| b.lo.x > h[4].leaf.lo.x - 0.2));
    }

    #[test]
    fn tilt_turn_hung_pivot_and_bifold_windows_are_built_and_marked() {
        use creabim_core::cells::WindowStyle;
        let w = wall();
        let window = |s: WindowStyle| {
            let mut frame = OpeningFrame::for_kind(OpeningKind::Window);
            frame.grid = s.grid(false, [1, 1]);
            Opening {
                width: 1.2,
                height: 1.5,
                ..opening(OpeningKind::Window, frame)
            }
        };
        let marks = |o: &Opening, eye: DVec3| {
            window_marks(o, &w, 0.0, eye, creabim_core::TriangleApex::Hinge)
        };
        // tilt and turn: a casement's triangle and a hopper's, both
        // dashed from outside (it opens in), both solid from inside
        let o = window(WindowStyle::TiltTurn);
        let out = if o.faces_left(&w) {
            DVec3::Y
        } else {
            -DVec3::Y
        };
        let m = marks(&o, out);
        assert_eq!(m.len(), 4);
        assert!(m.iter().all(|l| l.0));
        assert!(marks(&o, -out).iter().all(|l| !l.0));
        assert_eq!(window_text(&o), "Tilt and turn");
        // hung sashes: two sashes, one in front of the other, lapping
        // at the meeting rail; an arrow each way on a double hung
        let o = window(WindowStyle::DoubleHung);
        let sashes: Vec<PartBox> = opening_boxes(&o, &w)
            .into_iter()
            .filter(|b| b.part == Part::Leaf)
            .collect();
        let (lower, upper): (Vec<&PartBox>, Vec<&PartBox>) =
            sashes.iter().partition(|b| b.lo.z < 0.75);
        let top_of_lower = lower.iter().map(|b| b.hi.z).fold(0.0, f64::max);
        let foot_of_upper = upper.iter().map(|b| b.lo.z).fold(9.0, f64::min);
        assert!(top_of_lower > foot_of_upper, "they lap");
        let ly = lower[0].lo.y + lower[0].hi.y;
        let uy = upper[0].lo.y + upper[0].hi.y;
        assert!(ly * uy < 0.0, "on tracks either side of the middle");
        assert_eq!(marks(&o, out).len(), 6);
        assert_eq!(marks(&window(WindowStyle::SingleHung), out).len(), 3);
        assert_eq!(window_text(&o), "Double hung");
        // pivots: two triangles meeting at the axis, one each way
        for s in [WindowStyle::HorizontalPivot, WindowStyle::VerticalPivot] {
            let m = marks(&window(s), out);
            assert_eq!(m.len(), 4, "{s:?}");
            assert_eq!(m.iter().filter(|l| l.0).count(), 2, "{s:?}");
        }
        // bifold: a triangle on each sash, alternately solid and dashed,
        // and one handle
        let o = window(WindowStyle::Bifold);
        let m = marks(&o, out);
        assert_eq!(m.len(), 4);
        assert_ne!(m[0].0, m[2].0);
        assert_eq!(count(&opening_boxes(&o, &w), Part::Handle), 1);
    }

    #[test]
    fn sectional_roller_and_revolving_doors_and_guillotine_windows() {
        use creabim_core::cells::WindowStyle;
        let w = wall();
        let door = |op: Operation, width: f64, height: f64| Opening {
            width,
            height,
            ..opening(
                OpeningKind::Door,
                OpeningFrame::of_operation(OpeningKind::Door, op),
            )
        };
        // none of them hangs a leaf or swings
        for op in [
            Operation::Sectional,
            Operation::Roller,
            Operation::Revolving,
        ] {
            let o = door(op, 2.4, 2.2);
            assert!(hung_leaves(&o, &w).is_empty(), "{op:?}");
            assert!(leaf_marks(&o, &w, 0.0, DVec3::Y).is_empty(), "{op:?}");
        }
        // a sectional door: panels about half a metre high, one pull
        let o = door(Operation::Sectional, 2.6, 2.2);
        let b = opening_boxes(&o, &w);
        assert_eq!(count(&b, Part::Leaf), 4);
        assert_eq!(count(&b, Part::Handle), 1);
        assert_eq!(operation_text(&o, &w), "Sectional overhead");
        assert_eq!(door_hand(&o, &w), "Up, in");
        // a roller shutter: 75 mm slats, and a coil box over the head
        let o = door(Operation::Roller, 3.0, 2.6);
        let b = opening_boxes(&o, &w);
        assert!(count(&b, Part::Leaf) >= 30, "{}", count(&b, Part::Leaf));
        let head = b.iter().map(|b| b.hi.z).fold(0.0, f64::max);
        assert!(head > 2.6 + 0.2, "the coil box sits over the head: {head}");
        assert_eq!(count(&b, Part::Handle), 0);
        assert_eq!(operation_text(&o, &w), "Roller shutter");
        // a revolving door: four glazed wings round a post
        let o = door(Operation::Revolving, 2.0, 2.3);
        assert_eq!(count(&opening_boxes(&o, &w), Part::Glass), 4);
        assert_eq!(door_hand(&o, &w), "Revolving");
        // a guillotine: one sash sliding up, marked with its arrow and,
        // dashed, where it rises to
        let mut frame = OpeningFrame::for_kind(OpeningKind::Window);
        frame.grid = WindowStyle::Guillotine.grid(false, [1, 1]);
        let o = Opening {
            width: 1.5,
            height: 0.7,
            ..opening(OpeningKind::Window, frame)
        };
        let m = window_marks(&o, &w, 0.0, DVec3::Y, creabim_core::TriangleApex::Hinge);
        assert_eq!(m.len(), 7);
        assert_eq!(m.iter().filter(|l| l.0).count(), 4);
        assert!(m
            .iter()
            .any(|l| l.1.z > o.height + 0.3 || l.2.z > o.height + 0.3));
        assert_eq!(window_text(&o), "Guillotine");
    }

    #[test]
    fn a_roof_window_is_set_in_its_slope_and_cut_through_its_roof() {
        use creabim_core::{Element, ElementKind, Transaction};
        let mut m = creabim_core::sample::layered_house();
        let roof = m
            .iter()
            .find(|e| matches!(e.kind, ElementKind::Roof(_)))
            .unwrap()
            .id;
        // lined square, so its hole is a box square to the slope
        let o = Opening {
            roof_build: creabim_core::RoofBuild {
                lining: creabim_core::RoofLining::Square,
                ..Default::default()
            },
            ..creabim_core::roofwindow::new_on(&m, roof).unwrap()
        };
        let before = crate::members::member_mesh(&m, m.get(roof).unwrap())
            .unwrap()
            .0;
        let w = Element::new("Roof window", ElementKind::Opening(o.clone()));
        Transaction::new("t").insert(w).apply(&mut m).unwrap();
        // the roof stays one closed solid, less the hole square to its
        // slope
        let after = crate::members::member_mesh(&m, m.get(roof).unwrap())
            .unwrap()
            .0;
        assert!(after.is_closed());
        let seat = creabim_core::roofwindow::seat(&m, &o).unwrap();
        let hole = o.width * o.height * seat.thickness;
        let lost = before.volume() - after.volume();
        assert!((lost - hole).abs() < 0.02 * hole, "{lost} vs {hole}");
        // the window: glass in the slope over where it was placed, and a
        // lining round the hole
        let parts = roof_window_meshes(&m, &o);
        let glass = &parts.iter().find(|(p, _)| *p == Part::Glass).unwrap().1;
        let (lo, hi) = glass.bounds().unwrap();
        let mid = ((lo + hi) / 2.0).as_dvec3();
        // (it stands on its upstand, out of the roof square to the slope:
        // taken back onto the roof's middle plane, it is where it was put)
        let on = mid - seat.out * (mid - seat.origin).dot(seat.out);
        assert!(on.truncate().distance(o.roof_at.unwrap()) < 0.1, "{mid}");
        assert!(parts.iter().any(|(p, _)| *p == Part::Trim));
        // nothing of it stands up from the roof past its frame, which
        // stands its upstand above the roofing: no wall window's sill at
        // its lower edge
        let proud = parts
            .iter()
            .flat_map(|(_, m)| m.positions.iter())
            .map(|p| (glam::Vec3::from(*p).as_dvec3() - seat.origin).dot(seat.out))
            .fold(f64::MIN, f64::max);
        assert!(
            proud <= seat.thickness / 2.0 + o.roof_build.upstand + 1e-4,
            "{proud}"
        );
        // its plan: the frame and the glass inside it
        assert_eq!(roof_window_plan(&m, roof, &o).len(), 2);
        // its marks seen from outside: a centre pivot's two triangles
        let eye = seat.out;
        let marks = roof_window_marks(&m, &o, eye, creabim_core::TriangleApex::Hinge);
        assert_eq!(marks.len(), 4);
        assert_eq!(window_text(&o), "Roof window, centre pivot");
    }

    #[test]
    fn a_roof_window_is_built_as_roof_windows_are() {
        // Mockup_Roof_Window_REV00 (owner MU127: "a generic vertical wall
        // window just applied to the roof"), approved A on all five
        use creabim_core::{Element, ElementKind, RoofLining, Transaction};
        let mut m = creabim_core::sample::layered_house();
        let roof = m
            .iter()
            .find(|e| matches!(e.kind, ElementKind::Roof(_)))
            .unwrap()
            .id;
        let o = creabim_core::roofwindow::new_on(&m, roof).unwrap();
        // the defaults: 90 mm upstand, double glazed, flashing and
        // insulation collars, a splayed lining
        let d = o.roof_build;
        assert_eq!(
            (d.upstand, d.glazing, d.flashing, d.collar, d.lining),
            (0.09, 2, true, true, RoofLining::Splayed)
        );
        Transaction::new("t")
            .insert(Element::new("Roof window", ElementKind::Opening(o.clone())))
            .apply(&mut m)
            .unwrap();
        let seat = creabim_core::roofwindow::seat(&m, &o).unwrap();
        let t = seat.thickness;
        let pts = |o: &Opening, part: Part| -> Vec<DVec3> {
            roof_window_meshes(&m, o)
                .into_iter()
                .filter(|(p, _)| *p == part)
                .flat_map(|(_, mm)| {
                    mm.positions
                        .into_iter()
                        .map(|q| glam::Vec3::from(q).as_dvec3())
                })
                .collect()
        };
        // out of the roof's middle plane, positive outwards
        let out = |q: DVec3| (q - seat.origin).dot(seat.out);
        // its frame stands 90 mm above the roofing
        let frame_top = pts(&o, Part::Frame)
            .into_iter()
            .map(out)
            .fold(f64::MIN, f64::max);
        assert!((frame_top - (t / 2.0 + 0.09)).abs() < 1e-6, "{frame_top}");
        // and its upstand carries the frame down onto the deck
        let frame_foot = pts(&o, Part::Frame)
            .into_iter()
            .map(out)
            .fold(f64::MAX, f64::min);
        assert!(frame_foot <= t / 2.0 - UPSTAND_SEAT + 1e-6, "{frame_foot}");
        // a flush one sits down on the roofing
        let flush = Opening {
            roof_build: creabim_core::RoofBuild { upstand: 0.0, ..d },
            ..o.clone()
        };
        let top = pts(&flush, Part::Frame)
            .into_iter()
            .map(out)
            .fold(f64::MIN, f64::max);
        assert!((top - t / 2.0).abs() < 1e-6, "{top}");
        // double glazing: twice the panes of single
        let single = Opening {
            roof_build: creabim_core::RoofBuild { glazing: 1, ..d },
            ..o.clone()
        };
        let glass = pts(&o, Part::Glass).len();
        assert!(
            glass > 0 && glass == 2 * pts(&single, Part::Glass).len(),
            "{glass}"
        );
        // the flashing lies on the roofing and up the upstand, no higher
        let fl: Vec<f64> = pts(&o, Part::Flashing).into_iter().map(out).collect();
        assert!(!fl.is_empty());
        assert!(
            fl.iter()
                .all(|&v| v >= t / 2.0 - 1e-6 && v <= t / 2.0 + 0.09 + 1e-6),
            "{fl:?}"
        );
        let none = Opening {
            roof_build: creabim_core::RoofBuild {
                flashing: false,
                collar: false,
                ..d
            },
            ..o.clone()
        };
        assert!(pts(&none, Part::Flashing).is_empty() && pts(&none, Part::Insulation).is_empty());
        // the insulation collar is inside the roof
        let ins: Vec<f64> = pts(&o, Part::Insulation).into_iter().map(out).collect();
        assert!(
            !ins.is_empty() && ins.iter().all(|&v| v.abs() <= t / 2.0 + 1e-6),
            "{ins:?}"
        );
        // the lining: its head level and its sill plumb, from the frame
        // side to the ceiling side
        let lining = |o: &Opening| {
            let p = pts(o, Part::Trim);
            let inner = p.iter().map(|q| out(*q)).fold(f64::MIN, f64::max);
            let ceiling: Vec<DVec3> = p
                .iter()
                .copied()
                .filter(|q| out(*q) < -t / 2.0 + 1e-6)
                .collect();
            let frame_side: Vec<DVec3> = p
                .iter()
                .copied()
                .filter(|q| out(*q) > inner - 1e-6)
                .collect();
            (frame_side, ceiling)
        };
        let top = |v: &[DVec3]| v.iter().map(|q| q.z).fold(f64::MIN, f64::max);
        let h = DVec3::new(seat.up.x, seat.up.y, 0.0).normalize();
        let back = |v: &[DVec3]| v.iter().map(|q| q.dot(h)).fold(f64::MAX, f64::min);
        let (a, c) = lining(&o);
        assert!(!a.is_empty() && !c.is_empty());
        assert!(
            (top(&a) - top(&c)).abs() < 0.005,
            "head level: {} {}",
            top(&a),
            top(&c)
        );
        assert!(
            (back(&a) - back(&c)).abs() < 0.005,
            "sill plumb: {} {}",
            back(&a),
            back(&c)
        );
        // square to the glass instead, the head drops into the room
        let square = Opening {
            roof_build: creabim_core::RoofBuild {
                lining: RoofLining::Square,
                ..d
            },
            ..o.clone()
        };
        let (a, c) = lining(&square);
        assert!(top(&c) < top(&a) - 0.05, "{} {}", top(&a), top(&c));
        // the roof's hole follows the splay: still one closed solid, and
        // more of the roof taken out than a square hole
        let roof_mesh = |o: &Opening| {
            let mut mm = m.clone();
            let before = mm
                .iter()
                .find(|e| matches!(&e.kind, ElementKind::Opening(p) if p.roof_at.is_some()))
                .unwrap()
                .clone();
            let mut after = before.clone();
            after.kind = ElementKind::Opening(o.clone());
            Transaction::new("t")
                .replace(before, after)
                .apply(&mut mm)
                .unwrap();
            crate::members::member_mesh(&mm, mm.get(roof).unwrap())
                .unwrap()
                .0
        };
        let (splayed, squared) = (roof_mesh(&o), roof_mesh(&square));
        assert!(splayed.is_closed() && squared.is_closed());
        assert!(
            splayed.volume() < squared.volume() - 0.01,
            "{} {}",
            splayed.volume(),
            squared.volume()
        );
        // and the file keeps only what differs from the defaults
        let json = serde_json::to_string(&o).unwrap();
        assert!(!json.contains("roof_build"), "{json}");
        let json = serde_json::to_string(&single).unwrap();
        let back_in: Opening = serde_json::from_str(&json).unwrap();
        assert_eq!(back_in.roof_build, single.roof_build);
    }

    #[test]
    fn a_roof_window_laid_over_another_leaves_its_roof_whole() {
        use creabim_core::{Element, ElementKind, Transaction};
        let mut m = creabim_core::sample::layered_house();
        let roof = m
            .iter()
            .find(|e| matches!(e.kind, ElementKind::Roof(_)))
            .unwrap()
            .id;
        let a = creabim_core::roofwindow::new_on(&m, roof).unwrap();
        let mut b = a.clone();
        b.roof_at = Some(a.roof_at.unwrap() + glam::DVec2::new(0.2, 0.1));
        let vol = |m: &creabim_core::Model| {
            let r = crate::members::member_mesh(m, m.get(roof).unwrap())
                .unwrap()
                .0;
            assert!(r.is_closed());
            r.volume()
        };
        Transaction::new("a")
            .insert(Element::new("A", ElementKind::Opening(a)))
            .apply(&mut m)
            .unwrap();
        let one = vol(&m);
        // the second, overlapping the first, is left uncut
        Transaction::new("b")
            .insert(Element::new("B", ElementKind::Opening(b)))
            .apply(&mut m)
            .unwrap();
        assert!((vol(&m) - one).abs() < 1e-6);
    }

    fn slider(op: Operation) -> Opening {
        let frame = OpeningFrame::of_operation(OpeningKind::Door, op);
        Opening {
            width: 1.0,
            ..opening(OpeningKind::Door, frame)
        }
    }

    #[test]
    fn a_surface_slider_hangs_on_the_wall_face_and_slides_clear() {
        let o = slider(Operation::SurfaceSliding);
        let s = sliding_leaves(&o, &wall());
        assert_eq!(s.len(), 1);
        let l = s[0].leaf;
        // on the face it opens to (+y), lapping the opening each side
        assert!(l.lo.y > 0.15 && l.hi.y < 0.25);
        assert!(l.lo.x < 0.0 && l.hi.x > 1.0);
        // slides towards the opening's start, clear of it
        assert!(s[0].travel < -1.0);
        assert!(l.hi.x + s[0].travel <= 0.0);
        // hinged-door marks are not for it; a track above
        assert!(hung_leaves(&o, &wall()).is_empty());
        let boxes = opening_boxes(&o, &wall());
        assert!(boxes.iter().any(|b| b.part == Part::Handle && b.lo.z > 2.1));
        // flipping the hand slides it the other way, the swing the other face
        let mut f = o.clone();
        f.flip_hand = true;
        f.flip_facing = true;
        let s = sliding_leaves(&f, &wall());
        assert!(s[0].travel > 1.0 && s[0].leaf.hi.y < -0.15);
        assert_eq!(door_hand(&o, &wall()), "Slides right, in");
        assert_eq!(door_hand(&f, &wall()), "Slides right, out");
        // the pair parts from the middle
        let s = sliding_leaves(&slider(Operation::SurfaceSlidingPair), &wall());
        assert_eq!(s.len(), 2);
        assert!(s[0].travel < 0.0 && s[1].travel > 0.0);
    }

    #[test]
    fn a_pocket_door_runs_into_a_cavity_in_its_wall() {
        let o = slider(Operation::Pocket);
        let boxes = opening_boxes(&o, &wall());
        // no jamb on the pocket side, the leaf in the middle of the wall
        assert_eq!(count(&boxes, Part::Frame), 2);
        let s = sliding_leaves(&o, &wall());
        assert!(s[0].leaf.lo.y.abs() < 0.03 && s[0].leaf.hi.y.abs() < 0.03);
        let p = pockets(&o, &wall());
        assert_eq!(p.len(), 1);
        let (c, (h0, h1)) = p[0];
        // beside the opening at 1.0, as long as the leaf, the height of it
        assert!(c.u1 > 1.0 && c.u1 < 1.01);
        assert!(c.u0 < 1.0 - 0.9 + 0.01 && c.u0 > 0.0);
        assert!(c.v0 == 0.0 && c.v1 > 2.0);
        assert!(h0 < -0.02 && h1 > 0.02 && h1 < 0.15);
        // a pair has a pocket each side
        let p = pockets(&slider(Operation::PocketPair), &wall());
        assert_eq!(p.len(), 2);
        assert!(p[0].0.u1 <= 1.01 && p[1].0.u0 >= 1.99);
        // too thin a wall for one: no cavity
        let thin = Wall {
            thickness: 0.07,
            ..wall()
        };
        assert!(pockets(&o, &thin).is_empty());
    }

    #[test]
    fn a_multi_panel_slider_has_fixed_and_sliding_panels() {
        let mut frame = OpeningFrame::for_kind(OpeningKind::Door);
        frame.set_panels(OpeningKind::Door, 4, 0b1001);
        let o = Opening {
            width: 3.0,
            ..opening(OpeningKind::Door, frame.clone())
        };
        let s = sliding_leaves(&o, &wall());
        assert_eq!(s.len(), 4);
        let travel: Vec<f64> = s
            .iter()
            .map(|s| {
                if s.travel == 0.0 {
                    0.0
                } else {
                    s.travel.signum()
                }
            })
            .collect();
        // the middle two slide over the fixed ones beside them
        assert_eq!(travel, vec![0.0, -1.0, 1.0, 0.0]);
        // neighbours on different tracks
        assert!(s[0].leaf.lo.y * s[1].leaf.lo.y < 0.0);
        assert!(s.iter().all(|s| s.glazed));
        assert_eq!(slide_pattern(&o, &wall()), "F S S F");
        assert_eq!(operation_text(&o, &wall()), "Multi-panel sliding, F S S F");
        let boxes = opening_boxes(&o, &wall());
        assert_eq!(count(&boxes, Part::Glass), 4);
        // read from the side it faces: panel 1 is on the right
        let o3 = Opening {
            frame: {
                let mut f = frame.clone();
                f.set_panels(OpeningKind::Door, 3, 0b001);
                f
            },
            ..o
        };
        assert_eq!(slide_pattern(&o3, &wall()), "S S F");
    }

    #[test]
    fn a_pocket_with_room_in_a_thick_straight_wall_has_no_problems() {
        let o = slider(Operation::Pocket);
        assert!(pocket_problems(&o, 0, &wall(), &[]).is_empty());
        // nor has a door that is not a pocket door, wherever it stands
        let mut thin = wall();
        thin.thickness = 0.06;
        assert!(pocket_problems(&slider(Operation::SurfaceSliding), 0, &thin, &[]).is_empty());
    }

    #[test]
    fn a_pocket_says_when_its_wall_is_too_thin_or_curved() {
        let o = slider(Operation::Pocket);
        let mut thin = wall();
        thin.thickness = 0.06;
        // exactly when no cavity is cut
        assert!(pockets(&o, &thin).is_empty());
        let p = pocket_problems(&o, 0, &thin, &[]);
        assert!(
            matches!(p[..], [PocketProblem::ThinWall { needs }] if needs > 0.06 && needs < 0.15)
        );
        // a wall just thick enough gets its cavity and no complaint
        let PocketProblem::ThinWall { needs } = p[0] else {
            unreachable!()
        };
        let mut enough = wall();
        enough.thickness = needs + 0.001;
        assert_eq!(pockets(&o, &enough).len(), 1);
        assert!(pocket_problems(&o, 0, &enough, &[]).is_empty());
        let mut curved = wall();
        curved.bulge = 0.2;
        assert_eq!(
            pocket_problems(&o, 0, &curved, &[]),
            vec![PocketProblem::CurvedWall]
        );
    }

    #[test]
    fn a_pocket_says_when_it_has_no_room() {
        // a wall that stops 0.3 m into the pocket
        let mut o = slider(Operation::Pocket);
        let travel = sliding_leaves(&o, &wall())[0].travel;
        let mut w = wall();
        let c = Cut::of(&o, &w).unwrap();
        if travel < 0.0 {
            let cut = c.u0 + travel + 0.3;
            w.start.x += cut;
            o.offset -= cut;
        } else {
            w.end.x = c.u1 + travel - 0.3;
        }
        let p = pocket_problems(&o, 0, &w, &[]);
        assert!(
            matches!(p[..], [PocketProblem::PastEnd { short }] if (short - 0.32).abs() < 0.02),
            "{p:?}"
        );

        // another opening where the leaf would go
        let o = slider(Operation::Pocket);
        let w = wall();
        let c = Cut::of(&o, &w).unwrap();
        let mut n = opening(
            OpeningKind::Window,
            OpeningFrame::for_kind(OpeningKind::Window),
        );
        n.width = 0.4;
        n.sill = 0.9;
        n.height = 0.6;
        let mid = if travel < 0.0 { c.u0 - 0.5 } else { c.u1 + 0.5 };
        n.offset = o.offset + (mid - (c.u0 + c.u1) / 2.0);
        let nc = Cut::of(&n, &w).unwrap();
        assert!(
            nc.u1 < c.u0 || nc.u0 > c.u1,
            "the window stands beside the door"
        );
        assert_eq!(
            pocket_problems(&o, 0, &w, &[n.clone()]),
            vec![PocketProblem::MeetsOpening]
        );
        // clear above the door's head, it is not in the way
        n.sill = 2.4;
        n.height = 0.4;
        assert!(pocket_problems(&o, 0, &w, &[n.clone()]).is_empty());
        // on the other side of the door, neither
        n.sill = 0.9;
        n.offset = o.offset - (mid - (c.u0 + c.u1) / 2.0);
        assert!(pocket_problems(&o, 0, &w, &[n]).is_empty());
    }

    #[test]
    fn each_leaf_of_a_pocket_pair_is_checked_on_its_own() {
        // a pair whose wall stops 0.2 m into the pocket on one side only
        let mut o = slider(Operation::PocketPair);
        assert_eq!(pocket_cells(&o), vec![0, 1]);
        let leaves = sliding_leaves(&o, &wall());
        assert_eq!(leaves.len(), 2);
        let short = leaves.iter().find(|s| s.travel < 0.0).unwrap();
        let mut w = wall();
        let c = Cut::of(&o, &w).unwrap();
        let cut = c.u0 + short.travel + 0.2;
        w.start.x += cut;
        o.offset -= cut;
        let p = pocket_problems(&o, short.cell, &w, &[]);
        assert!(
            matches!(p[..], [PocketProblem::PastEnd { short }] if (short - 0.22).abs() < 0.02),
            "{p:?}"
        );
        // the other leaf slides the other way, into wall it has
        assert!(pocket_problems(&o, 1 - short.cell, &w, &[]).is_empty());
        // a cell that does not exist, or a door with no pocket, has none
        assert!(pocket_problems(&o, 5, &w, &[]).is_empty());
        assert!(pocket_cells(&slider(Operation::SurfaceSlidingPair)).is_empty());
    }

    #[test]
    fn sliders_get_arrows_and_dashed_open_positions_in_elevation() {
        let o = slider(Operation::Pocket);
        let marks = slide_marks(&o, &wall(), 0.0, DVec3::Y);
        // an arrow of three lines, the open position's four dashed edges
        assert_eq!(marks.iter().filter(|m| !m.0).count(), 3);
        assert_eq!(marks.iter().filter(|m| m.0).count(), 4);
        // the outline is out over the pocket, in front of the wall's face
        let xs: Vec<f64> = marks.iter().filter(|m| m.0).map(|m| m.1.x).collect();
        assert!(xs.iter().all(|x| *x < 1.0 + 1e-6));
        assert!(marks.iter().filter(|m| m.0).all(|m| m.1.y > 0.15));
        // edge on, nothing
        assert!(slide_marks(&o, &wall(), 0.0, DVec3::X).is_empty());
    }

    /// A pair from a file before cells, its share now column sizes,
    /// schedules the same leaf widths as it did then (part 1's 1620 door
    /// at 67 %).
    #[test]
    fn a_pair_from_before_cells_schedules_the_same_leaf_widths() {
        let host = serde_json::to_string(&creabim_core::ElementId::new()).unwrap();
        let o: Opening = serde_json::from_str(&format!(
            r#"{{"host":{host},"kind":"Door","offset":1.0,"sill":0.0,"width":1.62,"height":2.1,"frame":{{"operation":"DoubleSwing","active_share":0.67}}}}"#
        ))
        .unwrap();
        assert_eq!(operation_text(&o, &wall()), "Double hinged, 999 + 489");
        let mut f = o.clone();
        f.flip_hand = true;
        assert_eq!(operation_text(&f, &wall()), "Double hinged, 999 + 489");
    }

    #[test]
    fn the_triangle_can_point_at_the_handle_instead() {
        let o = opening(OpeningKind::Door, OpeningFrame::for_kind(OpeningKind::Door));
        let hinge = leaf_marks(&o, &wall(), 0.0, DVec3::Y);
        let handle = leaf_marks_with(
            &o,
            &wall(),
            0.0,
            DVec3::Y,
            creabim_core::TriangleApex::Handle,
        );
        let ([a, p, _], [b, q, _]) = (hinge[0].1, handle[0].1);
        // the apex and the base swap edges; the same side, the same pen
        assert!((p.x - b.x).abs() < 1e-9 && (q.x - a.x).abs() < 1e-9);
        assert_eq!(hinge[0].0, handle[0].0);
    }

    fn window(style: creabim_core::cells::WindowStyle, inward: bool) -> Opening {
        let mut frame = OpeningFrame::for_kind(OpeningKind::Window);
        frame.grid = style.grid(inward, [1, 1]);
        opening(OpeningKind::Window, frame)
    }

    #[test]
    fn opening_windows_have_sashes_with_handles() {
        use creabim_core::cells::WindowStyle as S;
        let boxes = opening_boxes(&window(S::Casement, false), &wall());
        // a sash of four members round its glass, a handle inside
        assert_eq!(count(&boxes, Part::Leaf), 4);
        assert_eq!(count(&boxes, Part::Glass), 1);
        let handle: Vec<&PartBox> = boxes.iter().filter(|b| b.part == Part::Handle).collect();
        assert_eq!(handle.len(), 1);
        // on the free edge (hinged at the start), on the inside face; the
        // window here faces the wall's right, so its inside is +y
        assert!(handle[0].lo.x > 1.0 && handle[0].lo.y > 0.0);
        // a pair: two sashes; a louvre: blades a tenth of a metre apart
        let boxes = opening_boxes(&window(S::CasementPair, false), &wall());
        assert_eq!(count(&boxes, Part::Leaf), 8);
        let boxes = opening_boxes(&window(S::Louvred, false), &wall());
        let blades = count(&boxes, Part::Glass);
        assert!((9..=12).contains(&blades), "{blades}");
        // a sliding window: a fixed light and a sash on different tracks
        let boxes = opening_boxes(&window(S::Sliding, false), &wall());
        assert_eq!(count(&boxes, Part::Leaf), 8);
        // a plain window is unchanged: frame, sill, outside sill, glass
        let boxes = opening_boxes(&window(S::Fixed, false), &wall());
        assert_eq!(count(&boxes, Part::Leaf), 0);
        assert_eq!(count(&boxes, Part::Glass), 1);
    }

    #[test]
    fn a_mullion_splits_a_window_into_its_cells() {
        use creabim_core::cells::{Cell, Grid, Mechanism, Track};
        let mut frame = OpeningFrame::for_kind(OpeningKind::Window);
        frame.grid = Grid {
            columns: vec![
                Track::new(1.0),
                Track {
                    share: 2.0,
                    bar: true,
                },
            ],
            rows: vec![
                Track::new(1.0),
                Track {
                    share: 0.5,
                    bar: true,
                },
            ],
            cells: vec![
                Cell::new(Mechanism::Fixed),
                Cell::new(Mechanism::Hinged),
                Cell::new(Mechanism::Fixed),
                Cell::new(Mechanism::TopHung),
            ],
            ..Grid::default()
        };
        let o = Opening {
            width: 1.86,
            ..opening(OpeningKind::Window, frame)
        };
        let f = Fitted::of(&o, &wall()).unwrap();
        let cells = window_cells(&f);
        assert_eq!(cells.len(), 4);
        // 1.86 less two jambs and a mullion, split one to two
        let w0 = cells[0].x1 - cells[0].x0;
        let w1 = cells[1].x1 - cells[1].x0;
        assert!((w0 - 0.56).abs() < 1e-9 && (w1 - 1.12).abs() < 1e-9);
        assert!((cells[1].x0 - cells[0].x1 - 0.06).abs() < 1e-9);
        // a mullion and a transom among the frame's boxes
        let boxes = opening_boxes(&o, &wall());
        let tall = boxes
            .iter()
            .filter(|b| {
                b.part == Part::Frame
                    && (b.hi.x - b.lo.x - 0.06).abs() < 1e-9
                    && b.hi.z - b.lo.z > 1.0
            })
            .count();
        assert!(tall >= 1);
        assert_eq!(window_text(&o), "Casement + awning + fixed ×2");
    }

    #[test]
    fn window_triangles_point_at_the_hinges_and_say_which_way_they_open() {
        use creabim_core::cells::WindowStyle as S;
        use creabim_core::TriangleApex as A;
        let w = wall();
        // this window faces the wall's right (-y): seen from there, an
        // outward casement opens towards the viewer
        let out = -DVec3::Y;
        let m = window_marks(&window(S::Casement, false), &w, 0.0, out, A::Hinge);
        assert_eq!(m.len(), 2);
        assert!(m.iter().all(|l| !l.0));
        // the apex at the hinge jamb, at mid height
        let apex = m[0].2;
        assert!((apex.x - 1.06).abs() < 1e-6 && (apex.z - (0.9 + 0.65)).abs() < 1e-6);
        // from inside, dashed; an inward casement the other way round
        assert!(
            window_marks(&window(S::Casement, false), &w, 0.0, -out, A::Hinge)
                .iter()
                .all(|l| l.0)
        );
        assert!(
            window_marks(&window(S::Casement, true), &w, 0.0, out, A::Hinge)
                .iter()
                .all(|l| l.0)
        );
        // an awning's apex at its top, a hopper's at its bottom
        let top = window_marks(&window(S::Awning, false), &w, 0.0, out, A::Hinge)[0].2;
        let bottom = window_marks(&window(S::Hopper, false), &w, 0.0, out, A::Hinge)[0].2;
        assert!(top.z > 2.0 && bottom.z < 1.0);
        // pointing at the handle instead
        let h = window_marks(&window(S::Casement, false), &w, 0.0, out, A::Handle)[0].2;
        assert!(h.x > 2.0);
        // a sliding window: an arrow and an F; a fixed light: nothing
        assert_eq!(
            window_marks(&window(S::Sliding, false), &w, 0.0, out, A::Hinge).len(),
            6
        );
        assert!(window_marks(&window(S::Fixed, false), &w, 0.0, out, A::Hinge).is_empty());
        assert_eq!(
            window_text(&window(S::CasementPair, true)),
            "Casement pair, in"
        );
    }

    #[test]
    fn the_frame_fits_a_thin_wall() {
        let mut w = wall();
        w.thickness = 0.05;
        let o = opening(
            OpeningKind::Window,
            OpeningFrame::for_kind(OpeningKind::Window),
        );
        for b in opening_boxes(&o, &w) {
            assert!(b.lo.y >= -0.025 - 1e-9 && b.hi.y <= 0.025 + 1e-9);
        }
    }

    fn leaf(o: &Opening) -> PartBox {
        *opening_boxes(o, &wall())
            .iter()
            .find(|b| b.part == Part::Leaf)
            .unwrap()
    }

    #[test]
    fn doors_flip_hand_and_facing() {
        let mut o = opening(OpeningKind::Door, OpeningFrame::for_kind(OpeningKind::Door));
        // into the interior (the left of an unflipped wall), hinged at its start
        let l = leaf(&o);
        assert!(l.hi.y > 0.0 && l.lo.y > -0.01);
        assert_eq!(door_hand(&o, &wall()), "Right, in");
        let handle = |o: &Opening| {
            opening_boxes(o, &wall())
                .iter()
                .find(|b| b.part == Part::Handle)
                .unwrap()
                .lo
                .x
        };
        assert!(handle(&o) > 0.6, "the handle is away from the hinge");
        o.flip_hand = true;
        assert!(handle(&o) < 0.6);
        assert!((leaf(&o).lo.x - (1.2 - l.hi.x)).abs() < 1e-9);
        assert_eq!(door_hand(&o, &wall()), "Left, in");
        o.flip_facing = true;
        let l = leaf(&o);
        assert!(l.lo.y < 0.0 && l.hi.y < 0.01, "now to the wall's right");
        assert_eq!(door_hand(&o, &wall()), "Right, out");
    }

    #[test]
    fn a_window_faces_out_of_the_exterior_unless_flipped() {
        let mut o = opening(
            OpeningKind::Window,
            OpeningFrame::for_kind(OpeningKind::Window),
        );
        let sill = |o: &Opening| {
            opening_boxes(o, &wall())
                .into_iter()
                .filter(|b| b.part == Part::Frame)
                .min_by(|a, b| a.lo.y.min(-a.hi.y).total_cmp(&b.lo.y.min(-b.hi.y)))
                .unwrap()
        };
        // the exterior is the wall's right: the sill runs out past it
        assert!((sill(&o).lo.y + 0.15 + SILL_NOSE).abs() < 1e-9);
        o.flip_facing = true;
        assert!((sill(&o).hi.y - 0.15 - SILL_NOSE).abs() < 1e-9);
    }
}
