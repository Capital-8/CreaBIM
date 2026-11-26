//! Roads, drives and paths: a carriageway along a centreline, its level
//! given at each point of the line, falling across from its crown (or one
//! way) to kerbs and footpaths each side. A road is worked into the
//! terrain under it as a sloping pad, dug in and made up with banks.

use glam::{DVec2, DVec3};
use serde::{Deserialize, Serialize};

/// A road along `path` (each point in plan with its level above the
/// road's level, at the crown).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Road {
    pub path: Vec<DVec3>,
    /// The carriageway's width, kerb to kerb.
    pub width: f64,
    /// How it falls across, per cent.
    #[serde(default = "two_and_a_half")]
    pub fall: f64,
    /// Falls both ways from its middle. Otherwise one way, to its right.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub crowned: bool,
    /// How far the kerbs stand up; 0 is none (a shared surface).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub kerb: f64,
    /// A footpath this wide each side, at the kerb's top; 0 is none.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub footpath: f64,
    /// How deep its make-up is under the surface.
    #[serde(default = "point_three")]
    pub thickness: f64,
    /// Its banks into the ground: how far each runs out for each metre,
    /// where it is dug in and where it is made up.
    #[serde(default = "one_and_a_half")]
    pub cut: f64,
    #[serde(default = "two")]
    pub fill: f64,
    /// It is worked into the terrain under it.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub works: bool,
    /// Parking bays marked along its edges.
    #[serde(default, skip_serializing_if = "Parking::is_none")]
    pub parking: Parking,
    /// Gullies along its low edges, one every this many metres, with a
    /// channel by the kerb running to them; 0 is none.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub gullies: f64,
    /// Older files: its bends rounded to this radius on the centreline;
    /// 0 leaves them as sharp corners. Read only when `kerb_radius` is
    /// none; see [`Road::centre_radius`].
    #[serde(default, skip_serializing_if = "is_zero")]
    pub radius: f64,
    /// Its bends, as they are set out: the inside kerb's radius round
    /// each. The middle line takes it plus half the width, so a bend can
    /// never fold its inside edge (owner, cut test: 460 mm on the middle
    /// of a 5.5 m road piled the inside kerb onto one point). 0 is a sharp
    /// inside corner. None: an older road, by `radius`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kerb_radius: Option<f64>,
    /// Its sides cut and made up upright, whatever its banks: a retained
    /// face, for where a bank would run into another work.
    #[serde(default, skip_serializing_if = "Side::is_none")]
    pub held: Side,
    /// Where its levels come from.
    #[serde(default, skip_serializing_if = "Levels::is_points")]
    pub levels: Levels,
    /// Following the ground, its ends held at a level: its start's and
    /// its end's crown this far above the road's level (a drive meeting
    /// the garage floor). None follows the ground there too.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_at: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_at: Option<f64>,
    /// Following the ground: its crown's level every [`PROFILE_STEP`]
    /// along its middle from its start, worked out from the ground as
    /// it was (see the app's road fitting). Empty: by its points.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub profile: Vec<f64>,
    /// What `profile` was worked out from, so it is worked out again
    /// when the road or the ground under it changes.
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub profile_key: u64,
}

/// Where a road's levels come from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Levels {
    /// Each point's own level, straight grades between.
    #[default]
    Points,
    /// The ground as it was along its middle, smoothed over `ease`
    /// metres and held to `steepest` (rise over run).
    Ground { steepest: f64, ease: f64 },
}

impl Levels {
    pub fn is_points(&self) -> bool {
        matches!(self, Levels::Points)
    }

    /// Following the ground at most 1:8, eased over 10 m.
    pub fn ground() -> Self {
        Levels::Ground {
            steepest: 0.125,
            ease: 10.0,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Levels::Points => "At its points",
            Levels::Ground { .. } => "Follow the ground",
        }
    }
}

/// How far apart a road following the ground has its levels.
pub const PROFILE_STEP: f64 = 1.0;

/// A new road's inside kerb radius.
pub const KERB_RADIUS: f64 = 4.0;

/// Bays along a road's side: which side, each bay's size and angle.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Parking {
    pub side: Side,
    /// A bay's width along the kerb (square to its own length).
    #[serde(default = "two_and_a_half")]
    pub width: f64,
    /// How long a bay is.
    #[serde(default = "five")]
    pub length: f64,
    /// The bays' angle to the road, degrees: 90 square on, 0 parallel.
    #[serde(default = "ninety")]
    pub angle: f64,
}

impl Default for Parking {
    fn default() -> Self {
        Self {
            side: Side::None,
            width: 2.5,
            length: 5.0,
            angle: 90.0,
        }
    }
}

impl Parking {
    pub fn is_none(&self) -> bool {
        self.side == Side::None
    }
}

/// Which side of a road.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    #[default]
    None,
    Left,
    Right,
    Both,
}

impl Side {
    pub const ALL: [Side; 4] = [Side::None, Side::Left, Side::Right, Side::Both];

    pub fn label(self) -> &'static str {
        match self {
            Side::None => "None",
            Side::Left => "Left",
            Side::Right => "Right",
            Side::Both => "Both sides",
        }
    }

    pub fn is_none(&self) -> bool {
        *self == Side::None
    }

    pub fn left(self) -> bool {
        matches!(self, Side::Left | Side::Both)
    }

    pub fn right(self) -> bool {
        matches!(self, Side::Right | Side::Both)
    }
}

fn yes() -> bool {
    true
}
fn is_true(b: &bool) -> bool {
    *b
}
fn is_zero(v: &f64) -> bool {
    *v == 0.0
}
fn is_zero_u64(v: &u64) -> bool {
    *v == 0
}
fn two_and_a_half() -> f64 {
    2.5
}
fn point_three() -> f64 {
    0.3
}
fn one_and_a_half() -> f64 {
    1.5
}
fn two() -> f64 {
    2.0
}
fn five() -> f64 {
    5.0
}
fn ninety() -> f64 {
    90.0
}

/// How far a road's channel runs in from its kerb.
pub const CHANNEL: f64 = 0.3;
/// How far a gully's middle is in from the kerb.
pub const GULLY_IN: f64 = 0.25;

/// One edge line of a road: its points in plan with their heights.
pub type Edge = Vec<DVec3>;

impl Road {
    /// A road along `path` at level 0, `width` wide, with the usual fall,
    /// kerbs and banks.
    pub fn new(path: Vec<DVec2>, width: f64) -> Self {
        Self {
            path: path.into_iter().map(|p| p.extend(0.0)).collect(),
            width,
            fall: 2.5,
            crowned: true,
            kerb: 0.1,
            footpath: 0.0,
            thickness: 0.3,
            cut: 1.5,
            fill: 2.0,
            works: true,
            parking: Parking::default(),
            gullies: 0.0,
            radius: 0.0,
            kerb_radius: Some(KERB_RADIUS),
            held: Side::None,
            start_at: None,
            end_at: None,
            levels: Levels::Points,
            profile: vec![],
            profile_key: 0,
        }
    }

    /// Its bends' radius on its middle line: the inside kerb's plus half
    /// its width; an older road's own, held to at least half its width
    /// so its inside edge never folds.
    pub fn centre_radius(&self) -> f64 {
        let half = self.width / 2.0;
        match self.kerb_radius {
            Some(k) => k.max(0.0) + half,
            None if self.radius > 0.0 => self.radius.max(half),
            None => 0.0,
        }
    }

    /// The inside kerb's radius it is set out to.
    pub fn kerb_radius(&self) -> f64 {
        match self.kerb_radius {
            Some(k) => k,
            None => (self.radius - self.width / 2.0).max(0.0),
        }
    }

    /// Its bends whose straights are too short for its radius: each one's
    /// point and the inside kerb radius it is built to.
    pub fn bends_held(&self) -> Vec<(usize, f64)> {
        let r = self.centre_radius();
        let mut out = vec![];
        if r <= 1e-6 {
            return out;
        }
        let half = self.width / 2.0;
        for i in 1..self.path.len().saturating_sub(1) {
            let (a, b, c) = (
                self.path[i - 1].truncate(),
                self.path[i].truncate(),
                self.path[i + 1].truncate(),
            );
            let (u, v) = (b - a, c - b);
            let (lu, lv) = (u.length(), v.length());
            if lu < 1e-6 || lv < 1e-6 {
                continue;
            }
            let turn = (u / lu).perp_dot(v / lv).atan2((u / lu).dot(v / lv)).abs();
            if !(0.02..=3.0).contains(&turn) {
                continue;
            }
            let half_tan = (turn / 2.0).tan();
            let room = 0.45 * lu.min(lv);
            let got = (room / half_tan - half).max(0.0);
            // only where it gets less than it asks for
            if r * half_tan > room + 1e-9 && got < self.kerb_radius() - 0.005 {
                out.push((i, got));
            }
        }
        out
    }

    /// Its centreline as it is built: through its points, each bend
    /// rounded to its radius. Its level runs evenly from point to point
    /// and round each bend, or, following the ground, as its profile
    /// has it, with a point every [`PROFILE_STEP`].
    pub fn line(&self) -> Vec<DVec3> {
        self.built(0.0)
    }

    pub fn plan(&self) -> Vec<DVec2> {
        self.line().iter().map(|p| p.truncate()).collect()
    }

    /// How long its centreline is.
    pub fn length(&self) -> f64 {
        self.line()
            .windows(2)
            .map(|w| w[0].truncate().distance(w[1].truncate()))
            .sum()
    }

    /// The line `d` to the left of its middle (negative to its right) as
    /// it is built, one point for each of [`Road::line`]'s, each with the
    /// crown's level beside it. Where a tight bend gathers its inside
    /// edge onto one point, those points share one level, so no sliver
    /// stands up there (owner, cut test).
    pub fn built(&self, d: f64) -> Vec<DVec3> {
        let r = self.centre_radius();
        let centre = round_offset(&self.path, r, 0.0);
        let side = if d == 0.0 {
            centre.clone()
        } else {
            round_offset(&self.path, r, d)
        };
        let following = !self.profile.is_empty();
        let (mut centre, mut side) = if following {
            densify(&centre, &side, PROFILE_STEP)
        } else {
            (centre, side)
        };
        if following {
            let mut gone = 0.0;
            for i in 0..centre.len() {
                if i > 0 {
                    gone += centre[i - 1].truncate().distance(centre[i].truncate());
                }
                centre[i].z = self.profile_at(gone);
            }
        }
        for (p, c) in side.iter_mut().zip(&centre) {
            p.z = c.z;
        }
        // one level for each run of points on one spot
        let mut i = 0;
        while i < side.len() {
            let mut j = i + 1;
            while j < side.len() && side[j].truncate().distance(side[i].truncate()) < 1e-6 {
                j += 1;
            }
            if j - i > 1 {
                let z = side[i..j].iter().map(|p| p.z).sum::<f64>() / (j - i) as f64;
                for p in &mut side[i..j] {
                    p.z = z;
                }
            }
            i = j;
        }
        side
    }

    /// Its profile's level `t` along its middle.
    fn profile_at(&self, t: f64) -> f64 {
        let n = self.profile.len();
        if n == 0 {
            return 0.0;
        }
        let f = (t / PROFILE_STEP).max(0.0);
        let i = (f.floor() as usize).min(n - 1);
        let j = (i + 1).min(n - 1);
        let k = (f - i as f64).clamp(0.0, 1.0);
        self.profile[i] + (self.profile[j] - self.profile[i]) * k
    }

    /// Its steepest grade along its middle as built, rise over run,
    /// taken over 3 m or more so one step's rounding doesn't count.
    pub fn steepest(&self) -> f64 {
        if self.profile.is_empty() {
            return self.grades().iter().fold(0.0_f64, |a, g| a.max(g.abs()));
        }
        let line = self.line();
        let mut gone = vec![0.0];
        for w in line.windows(2) {
            gone.push(gone.last().unwrap() + w[0].truncate().distance(w[1].truncate()));
        }
        let mut most = 0.0_f64;
        for i in 0..line.len() {
            if let Some(j) = (i + 1..line.len()).find(|j| gone[*j] - gone[i] >= 3.0) {
                most = most.max((line[j].z - line[i].z).abs() / (gone[j] - gone[i]));
            }
        }
        most
    }

    /// Each stretch's grade along it, rise over run (+ climbing from its
    /// first point to its next).
    pub fn grades(&self) -> Vec<f64> {
        self.path
            .windows(2)
            .map(|w| {
                let run = w[0].truncate().distance(w[1].truncate());
                if run > 1e-9 {
                    (w[1].z - w[0].z) / run
                } else {
                    0.0
                }
            })
            .collect()
    }

    /// The left and right edges of the carriageway at kerb's foot, each
    /// from the first point to the last, with their heights.
    pub fn edges(&self) -> (Edge, Edge) {
        self.edges_at(self.width / 2.0, 0.0)
    }

    /// The outer edges: the footpaths' backs, or the carriageway's edges
    /// raised by the kerb when there are none.
    pub fn outer(&self) -> (Edge, Edge) {
        self.edges_at(self.width / 2.0 + self.footpath, self.kerb)
    }

    /// The line `d` to the left of its centreline (negative to its
    /// right) as it is built, round its bends too: see [`Road::built`].
    pub fn side_line(&self, d: f64) -> Vec<DVec2> {
        self.built(d).into_iter().map(|p| p.truncate()).collect()
    }

    /// Lines `off` out from the centreline each side, `up` above the
    /// carriageway's edge there.
    fn edges_at(&self, off: f64, up: f64) -> (Edge, Edge) {
        let half = self.width / 2.0;
        let f = self.fall / 100.0;
        let (dl, dr) = if self.crowned {
            (-half * f, -half * f)
        } else {
            (half * f, -half * f)
        };
        let l = self.built(off);
        let r = self.built(-off);
        (
            l.iter().map(|p| p.with_z(p.z + dl + up)).collect(),
            r.iter().map(|p| p.with_z(p.z + dr + up)).collect(),
        )
    }

    /// The sides its water runs to: both for a crowned road, else its
    /// right.
    pub fn low_sides(&self) -> (bool, bool) {
        (self.crowned, true)
    }

    /// The channel each low side: a line by the kerb, in plan, from its
    /// first point to its last. None without gullies.
    pub fn channels(&self) -> Vec<Vec<DVec2>> {
        if self.gullies <= 0.0 {
            return vec![];
        }
        let off = (self.width / 2.0 - CHANNEL).max(0.0);
        let (l, r) = self.low_sides();
        let mut out = vec![];
        if l {
            out.push(self.side_line(off));
        }
        if r {
            out.push(self.side_line(-off));
        }
        out
    }

    /// Its gullies: each one's place in plan with the height of the road's
    /// edge there, along its low sides, one every `gullies` metres from
    /// half that from its high end, and one at its low end.
    pub fn gully_points(&self) -> Vec<DVec3> {
        if self.gullies <= 0.0 {
            return vec![];
        }
        let (le, re) = self.edges_at(self.width / 2.0 - GULLY_IN, 0.0);
        let (l, r) = self.low_sides();
        let mut out = vec![];
        for (on, mut edge) in [(l, le), (r, re)] {
            if !on || edge.len() < 2 {
                continue;
            }
            // from its high end down, so the last is at its low end
            if edge[0].z < edge[edge.len() - 1].z {
                edge.reverse();
            }
            let lens: Vec<f64> = edge
                .windows(2)
                .map(|w| w[0].truncate().distance(w[1].truncate()))
                .collect();
            let total: f64 = lens.iter().sum();
            let mut at = (self.gullies / 2.0).min(total);
            loop {
                // the point `at` along the edge
                let mut run = at;
                for (i, len) in lens.iter().enumerate() {
                    if run <= *len + 1e-9 {
                        out.push(edge[i].lerp(edge[i + 1], run / len.max(1e-9)));
                        break;
                    }
                    run -= len;
                }
                if at >= total - 1e-9 {
                    break;
                }
                at = (at + self.gullies).min(total);
            }
        }
        out
    }

    /// Its whole outline (with the footpaths), counter-clockwise, with the
    /// height of each corner: what is worked into the ground.
    pub fn outline(&self) -> (Vec<DVec2>, Vec<f64>) {
        let (l, r) = self.outer();
        let mut ring: Vec<DVec3> = r;
        ring.extend(l.into_iter().rev());
        // a point gathered several times round a tight bend is one corner
        ring.dedup_by(|b, a| a.truncate().distance(b.truncate()) < 1e-6);
        while ring.len() > 3 && ring[0].truncate().distance(ring[ring.len() - 1].truncate()) < 1e-6
        {
            ring.pop();
        }
        // the right edge forward and the left back is counter-clockwise
        // when the road runs any way at all
        if crate::polygon::signed_area2(&ring.iter().map(|p| p.truncate()).collect::<Vec<_>>())
            < 0.0
        {
            ring.reverse();
        }
        (
            ring.iter().map(|p| p.truncate()).collect(),
            ring.iter().map(|p| p.z).collect(),
        )
    }

    /// The bays along it: each one's four corners in plan, on each side
    /// it has them, marked on the carriageway in from its edge from its
    /// start (a car park is a wide road with bays down its sides).
    pub fn bays(&self) -> Vec<[DVec2; 4]> {
        let p = &self.parking;
        if p.is_none() || !(p.width > 0.1 && p.length > 0.1) {
            return vec![];
        }
        let plan = self.plan();
        let a = p.angle.clamp(0.0, 90.0).to_radians();
        let parallel = a < 1e-3;
        // along the kerb each bay takes its width over the sine (its
        // length when parallel), and leans on by its length's cosine
        let (pitch, lean) = if parallel {
            (p.length, 0.0)
        } else {
            (p.width / a.sin(), p.length * a.cos())
        };
        let mut out = vec![];
        for (on, sign) in [(p.side.left(), 1.0), (p.side.right(), -1.0)] {
            if !on {
                continue;
            }
            let kerb = offset(&plan, sign * self.width / 2.0);
            for w in kerb.windows(2) {
                let d = w[1] - w[0];
                let len = d.length();
                if len < 1e-6 {
                    continue;
                }
                let along = d / len;
                // marked on the carriageway, in from its edge
                let outward = -along.perp() * sign;
                let reach = if parallel {
                    outward * p.width
                } else {
                    (along * a.cos() + outward * a.sin()) * p.length
                };
                let n = ((len - lean) / pitch + 1e-9).floor().max(0.0) as usize;
                for k in 0..n {
                    let s = w[0] + along * (k as f64 * pitch);
                    let e = s + along * pitch;
                    out.push([s, e, e + reach, s + reach]);
                }
            }
        }
        out
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.path.len() < 2 || self.path.iter().any(|p| !p.is_finite()) {
            return Err("a road needs two finite points".into());
        }
        let ok = |v: f64| v.is_finite() && v >= 0.0;
        if !(self.width.is_finite() && self.width > 0.0) {
            return Err("a road's width must be positive".into());
        }
        if !(ok(self.fall)
            && ok(self.kerb)
            && ok(self.footpath)
            && ok(self.cut)
            && ok(self.fill)
            && self.thickness.is_finite()
            && self.thickness > 0.0)
        {
            return Err("a road's fall, kerb, footpath, depth and banks can't be negative".into());
        }
        if !(self.gullies == 0.0 || (self.gullies.is_finite() && self.gullies >= 2.0)) {
            return Err(format!(
                "a road's gullies must be 2 m apart or more, or 0 for none, got {}",
                self.gullies
            ));
        }
        let p = &self.parking;
        if !(p.width.is_finite() && p.length.is_finite() && p.angle.is_finite()) {
            return Err("parking bays need finite sizes".into());
        }
        Ok(())
    }
}

/// `path` with each inner corner rounded to `radius`: an arc tangent to
/// the two stretches, in pieces of ten degrees or less, its heights
/// running evenly from one stretch to the other. Where a stretch is too
/// short for the radius, the arc takes at most 45% of it.
pub fn round_line(path: &[DVec3], radius: f64) -> Vec<DVec3> {
    round_offset(path, radius, 0.0)
}

/// [`round_line`] for a line in plan.
pub fn round_plan(path: &[DVec2], radius: f64) -> Vec<DVec2> {
    let lifted: Vec<DVec3> = path.iter().map(|p| p.extend(0.0)).collect();
    round_line(&lifted, radius)
        .into_iter()
        .map(|p| p.truncate())
        .collect()
}

/// The line `d` to the left of `path` (negative to its right) as it is
/// built with its bends rounded to `radius`: one point for each of
/// [`round_plan`]'s, so edges and the centreline pair up.
///
/// It is made from the corners, not by moving the rounded line: round a
/// bend the outer edge takes the radius plus `d`, the inner the radius
/// less `d`, and where that is nothing or less the inner edge is a sharp
/// corner. Moving the rounded line instead loops the inner edge back on
/// itself once `d` passes the radius (owner, MU98).
pub fn offset_round(path: &[DVec2], radius: f64, d: f64) -> Vec<DVec2> {
    let lifted: Vec<DVec3> = path.iter().map(|p| p.extend(0.0)).collect();
    round_offset(&lifted, radius, d)
        .into_iter()
        .map(|p| p.truncate())
        .collect()
}

/// [`offset_round`] with heights: each point takes the height of the
/// centreline's point it pairs with.
pub fn round_offset(path: &[DVec3], radius: f64, d: f64) -> Vec<DVec3> {
    let n = path.len();
    if n < 2 {
        return path.to_vec();
    }
    let plan: Vec<DVec2> = path.iter().map(|p| p.truncate()).collect();
    let mitre = |i: usize| mitred(&plan, i, d);
    let mut out = vec![mitre(0).extend(path[0].z)];
    for i in 1..n - 1 {
        let (a, b, c) = (path[i - 1], path[i], path[i + 1]);
        let (u, v) = (b.truncate() - a.truncate(), c.truncate() - b.truncate());
        let (lu, lv) = (u.length(), v.length());
        if radius <= 1e-6 || lu < 1e-6 || lv < 1e-6 {
            out.push(mitre(i).extend(b.z));
            continue;
        }
        let (u, v) = (u / lu, v / lv);
        let turn = u.perp_dot(v).atan2(u.dot(v));
        if turn.abs() < 0.02 || turn.abs() > 3.0 {
            // straight on, or doubling back: nothing to round
            out.push(mitre(i).extend(b.z));
            continue;
        }
        // how far back from the corner the arc starts
        let t = (radius * (turn.abs() / 2.0).tan()).min(0.45 * lu.min(lv));
        let r = t / (turn.abs() / 2.0).tan();
        let start = b.truncate() - u * t;
        let (zs, ze) = (b.z + (a.z - b.z) * t / lu, b.z + (c.z - b.z) * t / lv);
        let side = if turn > 0.0 { u.perp() } else { -u.perp() };
        let centre = start + side * r;
        // this edge's radius round the bend: less on the inside, more on
        // the outside; none is a sharp corner, never a reversed arc
        let here = r - d * turn.signum();
        let steps = ((turn.abs().to_degrees() / 10.0).ceil() as usize).max(2);
        for k in 0..=steps {
            let f = k as f64 / steps as f64;
            let p = centre + DVec2::from_angle(turn * f).rotate(start - centre);
            let q = if here > 1e-9 {
                centre + (p - centre) * (here / r)
            } else {
                mitre(i)
            };
            out.push(q.extend(zs + (ze - zs) * f));
        }
    }
    out.push(mitre(n - 1).extend(path[n - 1].z));
    out
}

/// `centre` and its paired `side` with points put in wherever the centre
/// runs further than `step`, at the same places along both.
fn densify(centre: &[DVec3], side: &[DVec3], step: f64) -> (Vec<DVec3>, Vec<DVec3>) {
    let mut c = vec![];
    let mut s = vec![];
    for i in 0..centre.len() {
        if i > 0 {
            let len = centre[i - 1].truncate().distance(centre[i].truncate());
            let k = (len / step).ceil() as usize;
            for j in 1..k {
                let f = j as f64 / k as f64;
                c.push(centre[i - 1].lerp(centre[i], f));
                s.push(side[i - 1].lerp(side[i], f));
            }
        }
        c.push(centre[i]);
        s.push(side[i]);
    }
    (c, s)
}

/// Point `i` of `line` moved `d` to its left, its corner mitred (held to
/// four times `d` out at a sharp one).
fn mitred(line: &[DVec2], i: usize, d: f64) -> DVec2 {
    let n = line.len();
    let dir = |i: usize| (line[i + 1] - line[i]).normalize_or_zero();
    let a = if i == 0 { dir(0) } else { dir(i - 1) };
    let b = if i + 1 == n { dir(n - 2) } else { dir(i) };
    let (na, nb) = (a.perp(), b.perp());
    let m = (na + nb).normalize_or_zero();
    let cos = m.dot(na);
    let m = if m == DVec2::ZERO || cos.abs() < 1e-6 {
        na
    } else {
        m * (1.0 / cos).min(4.0)
    };
    line[i] + m * d
}

/// `line` moved `d` to its left (negative to its right), its corners
/// mitred (and held to four times `d` out at sharp ones). For a line
/// with rounded bends use [`offset_round`] on its corners instead.
pub fn offset(line: &[DVec2], d: f64) -> Vec<DVec2> {
    if line.len() < 2 {
        return line.to_vec();
    }
    (0..line.len()).map(|i| mitred(line, i, d)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn straight() -> Road {
        let mut r = Road::new(vec![DVec2::ZERO, DVec2::new(40.0, 0.0)], 6.0);
        r.path[1].z = 2.0;
        r
    }

    #[test]
    fn a_road_has_its_edges_and_its_grade() {
        let r = straight();
        assert!((r.length() - 40.0).abs() < 1e-9);
        assert!((r.grades()[0] - 0.05).abs() < 1e-9);
        let (l, rt) = r.edges();
        // 3 m out each side, 2.5% lower than the crown
        assert!((l[0].y - 3.0).abs() < 1e-9 && (rt[0].y + 3.0).abs() < 1e-9);
        assert!((l[0].z + 0.075).abs() < 1e-9 && (rt[1].z - (2.0 - 0.075)).abs() < 1e-9);
        let (ring, zs) = r.outline();
        assert_eq!(ring.len(), 4);
        assert!(crate::polygon::signed_area2(&ring) > 0.0);
        assert!((crate::polygon::signed_area2(&ring) / 2.0 - 240.0).abs() < 1e-6);
        // the kerb's top is the outline's height
        assert!(zs.iter().all(|z| *z > -0.075 + 0.09 - 1e-9));
    }

    #[test]
    fn gullies_line_its_low_sides() {
        let mut r = straight();
        r.gullies = 10.0;
        assert!(r.validate().is_ok());
        let g = r.gully_points();
        // a crowned road: both sides, 5, 15, 25 and 35 m down from its
        // high end and one at its low end
        assert_eq!(g.len(), 10, "{g:?}");
        assert!(g.iter().any(|p| p.x.abs() < 1e-9));
        assert_eq!(r.channels().len(), 2);
        r.crowned = false;
        assert_eq!(r.gully_points().len(), 5);
        r.gullies = 1.0;
        assert!(r.validate().is_err());
    }

    #[test]
    fn a_bend_is_rounded_to_its_radius() {
        let mut r = Road::new(
            vec![DVec2::ZERO, DVec2::new(20.0, 0.0), DVec2::new(20.0, 20.0)],
            4.0,
        );
        r.path[1].z = 1.0;
        r.path[2].z = 2.0;
        r.kerb_radius = None;
        let sharp = r.length();
        assert_eq!(r.line().len(), 3);
        // a 4 m kerb on a 4 m road is 6 m on its middle
        r.kerb_radius = Some(4.0);
        assert!((r.centre_radius() - 6.0).abs() < 1e-12);
        let line = r.line();
        assert!(line.len() > 8, "the corner is an arc of pieces");
        // it starts and ends where it did, and cuts the corner
        assert_eq!((line[0], line[line.len() - 1]), (r.path[0], r.path[2]));
        assert!(r.length() < sharp - 1.0);
        // every piece of the arc is 6 m from its centre, (14, 6)
        let c = DVec2::new(14.0, 6.0);
        for p in &line[1..line.len() - 1] {
            assert!((p.truncate().distance(c) - 6.0).abs() < 1e-9, "{p}");
        }
        // its level runs on evenly: 0.7 where the arc starts, 1.3 where it ends
        assert!((line[1].z - 0.7).abs() < 1e-9 && (line[line.len() - 2].z - 1.3).abs() < 1e-9);
        // its edges and its outline follow
        assert_eq!(r.edges().0.len(), line.len());
        // a radius too big for a short stretch takes what fits
        r.kerb_radius = Some(98.0);
        let tight = r.line();
        assert!(
            (tight[1].truncate().x - 11.0).abs() < 1e-9,
            "45% of the stretch"
        );
        assert!(r.validate().is_ok());
    }

    #[test]
    fn a_bend_keeps_its_width() {
        let mut r = Road::new(
            vec![DVec2::ZERO, DVec2::new(20.0, 0.0), DVec2::new(20.0, 20.0)],
            6.0,
        );
        // an older road with sharp corners
        r.kerb_radius = None;
        let (l, _) = r.edges();
        // the inside of the bend: mitred to (17, 3)
        assert!(
            l[1].truncate().distance(DVec2::new(17.0, 3.0)) < 1e-9,
            "{l:?}"
        );
    }

    #[test]
    fn a_tight_bend_gathers_its_inside_edge_at_one_level() {
        // the owner's: 460 mm on the middle of a 5.5 m road, climbing
        let mut r = Road::new(
            vec![DVec2::ZERO, DVec2::new(6.0, 0.0), DVec2::new(2.0, 8.0)],
            5.5,
        );
        r.kerb_radius = None;
        r.radius = 0.46;
        r.path[1].z = 1.0;
        r.path[2].z = 2.5;
        // held to half its width on its middle: the inside kerb is a
        // corner, never a fold
        assert!((r.centre_radius() - 2.75).abs() < 1e-12);
        for d in [2.75, -2.75, 2.85, -2.85] {
            let side = r.built(d);
            for w in side.windows(2) {
                if w[0].truncate().distance(w[1].truncate()) < 1e-6 {
                    assert!((w[0].z - w[1].z).abs() < 1e-12, "a sliver at {d}: {w:?}");
                }
            }
        }
        let (ring, zs) = r.outline();
        assert_eq!(ring.len(), zs.len());
        for i in 0..ring.len() {
            assert!(
                ring[i].distance(ring[(i + 1) % ring.len()]) > 1e-6,
                "a repeated corner"
            );
        }
        // too short a straight for its radius: said, with what it got
        r.kerb_radius = Some(20.0);
        let held = r.bends_held();
        assert_eq!(held.len(), 1);
        assert!(held[0].1 < 20.0);
    }

    #[test]
    fn a_road_following_the_ground_takes_its_profile() {
        let mut r = Road::new(vec![DVec2::ZERO, DVec2::new(10.0, 0.0)], 5.0);
        r.levels = Levels::ground();
        r.profile = (0..=10).map(|i| (i as f64 * 0.3).sin()).collect();
        let line = r.line();
        // a point every metre, each at its profile
        assert_eq!(line.len(), 11);
        for (i, p) in line.iter().enumerate() {
            assert!((p.z - (i as f64 * 0.3).sin()).abs() < 1e-9);
        }
        let (l, rt) = r.edges();
        assert_eq!((l.len(), rt.len()), (11, 11));
        assert!(r.steepest() > 0.2);
    }

    #[test]
    fn bays_line_the_kerb() {
        let mut r = straight();
        r.parking.side = Side::Left;
        let bays = r.bays();
        // 40 m of kerb takes sixteen 2.5 m bays square on
        assert_eq!(bays.len(), 16);
        assert!((bays[0][3].y + 2.0).abs() < 1e-9, "{:?}", bays[0]);
        r.parking.side = Side::Both;
        assert_eq!(r.bays().len(), 32);
        r.parking.angle = 0.0;
        r.parking.side = Side::Right;
        r.parking.length = 6.0;
        // parallel: 6 m each, 2.5 m out
        let bays = r.bays();
        assert_eq!(bays.len(), 6);
        assert!((bays[0][3].y + 0.5).abs() < 1e-9, "{:?}", bays[0]);
    }
}
