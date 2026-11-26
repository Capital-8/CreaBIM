//! A property: the lot's boundary, what each of its edges is (the front
//! on the street, the sides, the rear), the planning rules it is built to
//! (setbacks per edge, a height limit, how much of it may be covered and
//! built), and the buildable area those setbacks leave.
//!
//! A lot is a [`crate::SiteForm::Property`]; checking a building against
//! its rules needs the building's shapes, and is `creabim_geom::planning`.
//!
//! Bearings are as on a survey plat: from north, towards east or west,
//! `N 12°30'15" E`. North is the project's true north (Project
//! information), which is the drawing's up unless that turns it.

use glam::DVec2;
use serde::{Deserialize, Serialize};

/// What an edge of a lot is: it sets the edge's setback.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LotEdge {
    /// On the street.
    Front,
    #[default]
    Side,
    Rear,
    /// No setback (a party wall, a lane the rules don't count).
    Open,
}

impl LotEdge {
    pub const ALL: [LotEdge; 4] = [LotEdge::Front, LotEdge::Side, LotEdge::Rear, LotEdge::Open];

    pub fn label(self) -> &'static str {
        match self {
            LotEdge::Front => "Front",
            LotEdge::Side => "Side",
            LotEdge::Rear => "Rear",
            LotEdge::Open => "No setback",
        }
    }
}

/// The rules a lot is built to. A limit of 0 is none.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlanningRules {
    /// How far the building keeps from the front, the sides and the rear.
    pub front: f64,
    pub side: f64,
    pub rear: f64,
    /// The highest the building may reach above the project's zero.
    pub height: f64,
    /// How much of the lot the building may cover, 0 to 1.
    pub coverage: f64,
    /// The floor area allowed for each unit of lot area (floor area ratio).
    pub floor_ratio: f64,
    /// Each edge's bearing and length written along it in plans.
    pub bearings: bool,
    /// The setback lines and the buildable area drawn in plans.
    pub setbacks: bool,
    /// The lot has setbacks at all. Off, it has none: no setback lines,
    /// no buildable area short of the boundary, nothing checked against
    /// them; the front, side and rear keep their values for when it is
    /// turned back on (owner, cut test: not every site has setbacks).
    pub has_setbacks: bool,
    /// Angle planes (daylight planes): from `angle_from` above each edge
    /// with a setback, a plane rising inward at `angle` degrees from the
    /// horizontal that the building stays under. 0 is none.
    pub angle: f64,
    pub angle_from: f64,
    /// The envelope (the buildable area up to the limits) shown in 3D.
    pub envelope: bool,
    /// Strips along edges kept clear of building: a drain, a right of way.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub easements: Vec<Easement>,
    /// What single edges have of their own: a setback other than their
    /// kind's, a note written along their setback line.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub own: Vec<EdgeRule>,
}

/// What edge `edge` of a lot has of its own: a setback that is not its
/// kind's (a second street front, a wider side by a neighbour's window,
/// a flanking lane), and the note written on its setback line in place
/// of "Front", "Side" or "Rear".
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EdgeRule {
    pub edge: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setback: Option<f64>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
    /// Where its note was dragged to, from its place below the setback,
    /// in metres on the plan; none is that place (MU98_06).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note_off: Option<glam::DVec2>,
}

/// A strip `width` wide along edge `edge` of a lot, inside it, kept clear
/// of building.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Easement {
    pub name: String,
    pub edge: usize,
    pub width: f64,
}

impl Default for PlanningRules {
    fn default() -> Self {
        Self {
            front: 6.0,
            side: 3.0,
            rear: 6.0,
            height: 0.0,
            coverage: 0.0,
            floor_ratio: 0.0,
            bearings: true,
            setbacks: true,
            has_setbacks: true,
            angle: 0.0,
            angle_from: 2.5,
            envelope: true,
            easements: vec![],
            own: vec![],
        }
    }
}

impl PlanningRules {
    /// The setback of edge `i` of a lot of `n` edges: its own where it
    /// has one, else its kind's.
    pub fn setback_at(&self, edges: &[LotEdge], i: usize, n: usize) -> f64 {
        if !self.has_setbacks {
            return 0.0;
        }
        self.own
            .iter()
            .find(|o| o.edge == i)
            .and_then(|o| o.setback)
            .unwrap_or_else(|| self.setback(edge_kind(edges, i, n)))
    }

    /// Edge `i`'s own setback, if it has one.
    pub fn own_setback(&self, i: usize) -> Option<f64> {
        self.own
            .iter()
            .find(|o| o.edge == i)
            .and_then(|o| o.setback)
    }

    /// The note on edge `i`'s setback line, if it has one.
    pub fn note_at(&self, i: usize) -> Option<&str> {
        self.own
            .iter()
            .find(|o| o.edge == i)
            .map(|o| o.note.trim())
            .filter(|t| !t.is_empty())
    }

    /// Give edge `i` a setback of its own, or with none put it back on
    /// its kind's.
    pub fn set_own_setback(&mut self, i: usize, setback: Option<f64>) {
        self.own_mut(i).setback = setback;
        self.tidy_own();
    }

    /// Write `note` on edge `i`'s setback line; empty takes it off.
    pub fn set_note(&mut self, i: usize, note: &str) {
        self.own_mut(i).note = note.to_string();
        self.tidy_own();
    }

    fn own_mut(&mut self, i: usize) -> &mut EdgeRule {
        if let Some(k) = self.own.iter().position(|o| o.edge == i) {
            return &mut self.own[k];
        }
        self.own.push(EdgeRule {
            edge: i,
            ..Default::default()
        });
        self.own.last_mut().expect("just pushed")
    }

    /// Where edge `i`'s note was dragged to from its place below the
    /// setback, if it was.
    pub fn note_off(&self, i: usize) -> Option<glam::DVec2> {
        self.own
            .iter()
            .find(|o| o.edge == i)
            .and_then(|o| o.note_off)
    }

    /// Put edge `i`'s note `off` from its place below the setback; none
    /// puts it back there.
    pub fn set_note_off(&mut self, i: usize, off: Option<glam::DVec2>) {
        self.own_mut(i).note_off = off.filter(|d| d.length() > 1e-6);
        self.tidy_own();
    }

    fn tidy_own(&mut self) {
        self.own
            .retain(|o| o.setback.is_some() || !o.note.is_empty());
        self.own.sort_by_key(|o| o.edge);
    }

    pub fn setback(&self, edge: LotEdge) -> f64 {
        match edge {
            LotEdge::Front => self.front,
            LotEdge::Side => self.side,
            LotEdge::Rear => self.rear,
            LotEdge::Open => 0.0,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        let ok = |v: f64| v.is_finite() && v >= 0.0;
        if ![
            self.front,
            self.side,
            self.rear,
            self.height,
            self.floor_ratio,
            self.angle_from,
        ]
        .into_iter()
        .all(ok)
            || !(ok(self.coverage) && self.coverage <= 1.0)
            || !(ok(self.angle) && self.angle < 89.0)
            || self
                .easements
                .iter()
                .any(|e| !(e.width.is_finite() && e.width > 0.0))
        {
            return Err(
                "setbacks and limits can't be negative, and coverage is at most 100%".into(),
            );
        }
        Ok(())
    }
}

/// What edge `i` of a lot of `n` corners is: as set, or by default the
/// first edge the front, on a four-sided lot the one opposite it the
/// rear, and the others sides.
pub fn edge_kind(edges: &[LotEdge], i: usize, n: usize) -> LotEdge {
    if let Some(e) = edges.get(i) {
        return *e;
    }
    match i {
        0 => LotEdge::Front,
        2 if n == 4 => LotEdge::Rear,
        _ => LotEdge::Side,
    }
}

/// A lot's boundary made counter-clockwise, its edges' kinds and its
/// easements following their edges.
pub fn make_ccw(
    boundary: &mut [DVec2],
    edges: &mut Vec<LotEdge>,
    easements: &mut [Easement],
    own: &mut [EdgeRule],
) {
    if crate::polygon::signed_area2(boundary) >= 0.0 {
        return;
    }
    let n = boundary.len();
    // fill in the defaults first, so they stay with their edges
    let mut all: Vec<LotEdge> = (0..n).map(|i| edge_kind(edges, i, n)).collect();
    boundary.reverse();
    // edge i (p_i to p_i+1) is now edge n-2-i; the closing edge stays last
    if n >= 2 {
        all[..n - 1].reverse();
        for e in easements.iter_mut().filter(|e| e.edge < n - 1) {
            e.edge = n - 2 - e.edge;
        }
        for o in own.iter_mut().filter(|o| o.edge < n - 1) {
            o.edge = n - 2 - o.edge;
        }
    }
    *edges = all;
}

/// The area the setbacks leave to build on: each edge moved in by its
/// setback, the corners where the moved edges meet. `None` when the
/// setbacks leave nothing, or the lot is too irregular for them to meet
/// cleanly. `boundary` is counter-clockwise.
pub fn buildable(
    boundary: &[DVec2],
    edges: &[LotEdge],
    rules: &PlanningRules,
) -> Option<Vec<DVec2>> {
    let n = boundary.len();
    if n < 3 {
        return None;
    }
    // each edge as a point on its moved line and its direction
    let lines: Vec<(DVec2, DVec2)> = (0..n)
        .map(|i| {
            let (a, b) = (boundary[i], boundary[(i + 1) % n]);
            let d = (b - a).normalize_or_zero();
            let inward = d.perp(); // left of a counter-clockwise edge
            (a + inward * rules.setback_at(edges, i, n), d)
        })
        .collect();
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let (p, d) = lines[(i + n - 1) % n];
        let (q, e) = lines[i];
        let cross = d.perp_dot(e);
        if cross.abs() < 1e-9 {
            // straight on: the moved lines meet where the corner moved in
            out.push(q);
            continue;
        }
        let t = (q - p).perp_dot(e) / cross;
        out.push(p + d * t);
    }
    let area = crate::polygon::signed_area2(&out);
    if area <= 1e-6 || crate::polygon::self_intersects(&out, true) {
        return None;
    }
    // every moved edge keeps its way round; one turned back means a
    // setback swallowed it
    for i in 0..n {
        let d0 = boundary[(i + 1) % n] - boundary[i];
        let d1 = out[(i + 1) % n] - out[i];
        if d0.dot(d1) < -1e-9 {
            return None;
        }
    }
    Some(out)
}

/// The line edge `i` of a lot is on, moved `by` inward: a point on it
/// and its direction. `boundary` is counter-clockwise.
fn moved_edge(boundary: &[DVec2], i: usize, by: f64) -> (DVec2, DVec2) {
    let n = boundary.len();
    let (a, b) = (boundary[i % n], boundary[(i + 1) % n]);
    let d = (b - a).normalize_or_zero();
    (a + d.perp() * by, d)
}

/// Where two lines (a point and a direction each) meet; `None` when they
/// run side by side.
fn meet(p: DVec2, d: DVec2, q: DVec2, e: DVec2) -> Option<DVec2> {
    let cross = d.perp_dot(e);
    (cross.abs() > 1e-9).then(|| p + d * ((q - p).perp_dot(e) / cross))
}

/// An easement's strip: the edge, and the edge moved in by its width,
/// ended on the edges either side. `None` for an edge the lot hasn't.
pub fn easement_outline(boundary: &[DVec2], e: &Easement) -> Option<Vec<DVec2>> {
    let n = boundary.len();
    if e.edge >= n || n < 3 {
        return None;
    }
    let (a, b) = (boundary[e.edge], boundary[(e.edge + 1) % n]);
    let (p, d) = moved_edge(boundary, e.edge, e.width);
    let (q0, d0) = moved_edge(boundary, e.edge + n - 1, 0.0);
    let (q1, d1) = moved_edge(boundary, e.edge + 1, 0.0);
    let start = meet(p, d, q0, d0)?;
    let end = meet(p, d, q1, d1)?;
    Some(vec![a, b, end, start])
}

/// How far inside edge `i`'s line `p` is.
fn inward(boundary: &[DVec2], i: usize, p: DVec2) -> f64 {
    let (a, d) = moved_edge(boundary, i, 0.0);
    d.perp().dot(p - a)
}

/// The highest the building may reach over `p` above the project's
/// zero: the height limit, and under each angle plane. `None` with no
/// limit on height.
pub fn envelope_top(
    boundary: &[DVec2],
    edges: &[LotEdge],
    rules: &PlanningRules,
    p: DVec2,
) -> Option<f64> {
    let n = boundary.len();
    let mut top = (rules.height > 0.0).then_some(rules.height);
    if rules.angle > 0.0 {
        let slope = rules.angle.to_radians().tan();
        for i in 0..n {
            if edge_kind(edges, i, n) == LotEdge::Open {
                continue;
            }
            let z = rules.angle_from + inward(boundary, i, p).max(0.0) * slope;
            top = Some(top.map_or(z, |t: f64| t.min(z)));
        }
    }
    top
}

/// The angle planes in effect: for each edge with a setback, the plane's
/// `(normal, offset)` with the building kept where `normal · x <= offset`.
pub fn angle_planes(
    boundary: &[DVec2],
    edges: &[LotEdge],
    rules: &PlanningRules,
) -> Vec<(glam::DVec3, f64)> {
    let n = boundary.len();
    if rules.angle <= 0.0 {
        return vec![];
    }
    let slope = rules.angle.to_radians().tan();
    (0..n)
        .filter(|i| edge_kind(edges, *i, n) != LotEdge::Open)
        .map(|i| {
            // z - from - slope * inward(p) <= 0
            let (a, d) = moved_edge(boundary, i, 0.0);
            let m = d.perp();
            let normal = glam::DVec3::new(-slope * m.x, -slope * m.y, 1.0);
            (normal, rules.angle_from - slope * m.dot(a))
        })
        .collect()
}

/// The area of a ring of corners.
pub fn area(ring: &[DVec2]) -> f64 {
    crate::polygon::signed_area2(ring).abs() / 2.0
}

/// The direction an azimuth (degrees clockwise from north) points in the
/// drawing, with true north `true_north` degrees anticlockwise of up.
pub fn direction(azimuth: f64, true_north: f64) -> DVec2 {
    let a = (90.0 + true_north - azimuth).to_radians();
    DVec2::new(a.cos(), a.sin())
}

/// The azimuth (degrees clockwise from north, 0 to 360) of direction `d`.
pub fn azimuth(d: DVec2, true_north: f64) -> f64 {
    (90.0 + true_north - d.y.atan2(d.x).to_degrees()).rem_euclid(360.0)
}

/// An azimuth as a quadrant bearing: `N 12°30'15" E`.
pub fn bearing_text(azimuth: f64) -> String {
    let a = azimuth.rem_euclid(360.0);
    let (from, angle, to) = match a {
        a if a <= 90.0 => ('N', a, 'E'),
        a if a <= 180.0 => ('S', 180.0 - a, 'E'),
        a if a <= 270.0 => ('S', a - 180.0, 'W'),
        a => ('N', 360.0 - a, 'W'),
    };
    let secs = (angle * 3600.0).round() as i64;
    format!(
        "{from} {}°{:02}'{:02}\" {to}",
        secs / 3600,
        secs / 60 % 60,
        secs % 60
    )
}

/// An angle typed as decimal degrees (`12.5`) or degrees, minutes and
/// seconds (`12°30'15"`, `12 30 15`, `12-30-15`, `12d30m15s`).
fn parse_angle(s: &str) -> Option<f64> {
    let parts: Vec<f64> = s
        .split(|c: char| !(c.is_ascii_digit() || c == '.'))
        .filter(|p| !p.is_empty())
        .map(|p| p.parse().ok())
        .collect::<Option<_>>()?;
    match parts[..] {
        [d] => Some(d),
        [d, m] if m < 60.0 => Some(d + m / 60.0),
        [d, m, s] if m < 60.0 && s < 60.0 => Some(d + m / 60.0 + s / 3600.0),
        _ => None,
    }
}

/// A bearing typed as a quadrant bearing (`N 12°30' E`, `S45W`) or an
/// azimuth in degrees clockwise from north (`192.5`): the azimuth.
pub fn parse_bearing(s: &str) -> Option<f64> {
    let t = s.trim().to_ascii_uppercase();
    let first = t.chars().next()?;
    if first == 'N' || first == 'S' {
        let last = t.chars().last()?;
        if last != 'E' && last != 'W' {
            return None;
        }
        let angle = parse_angle(&t[1..t.len() - 1])?;
        if angle > 90.0 {
            return None;
        }
        return Some(match (first, last) {
            ('N', 'E') => angle,
            ('S', 'E') => 180.0 - angle,
            ('S', _) => 180.0 + angle,
            _ => (360.0 - angle).rem_euclid(360.0),
        });
    }
    let a = parse_angle(&t)?;
    (a < 360.0).then_some(a)
}

/// A lot typed as a traverse: from `start`, one leg per line, a bearing
/// and then a distance (`N 12°30' E 24.35`, `192.5 18`). The legs need
/// not come back to the start: the last corner joins the first.
/// Returns the corners and how far the last leg ends from the start (its
/// misclosure). `to_model` turns a typed distance into model metres.
pub fn traverse(
    text: &str,
    start: DVec2,
    true_north: f64,
    to_model: impl Fn(f64) -> f64,
) -> Result<(Vec<DVec2>, f64), String> {
    let mut pts = vec![start];
    for (k, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // the distance is the last word (a unit after it dropped); the
        // bearing the rest
        let line = line.trim_end_matches('m').trim_end();
        let (bearing, dist) = line
            .rsplit_once(|c: char| c.is_whitespace() || c == ',' || c == ';' || c == '\t')
            .ok_or_else(|| {
                format!(
                    "line {}: a bearing and a distance, like N 12°30' E 24.35",
                    k + 1
                )
            })?;
        let dist: f64 = dist
            .trim()
            .parse()
            .map_err(|_| format!("line {}: \"{}\" is not a distance", k + 1, dist.trim()))?;
        let az = parse_bearing(bearing.trim_end_matches([',', ';']))
            .ok_or_else(|| format!("line {}: \"{}\" is not a bearing", k + 1, bearing.trim()))?;
        if !(dist.is_finite() && dist > 0.0) {
            return Err(format!("line {}: the distance must be above zero", k + 1));
        }
        let last = *pts.last().unwrap();
        pts.push(last + direction(az, true_north) * to_model(dist));
    }
    // a closing leg typed back to the start is the closing edge itself
    let miss = pts.last().unwrap().distance(start);
    if pts.len() > 3 && miss < 0.05 {
        pts.pop();
    }
    if pts.len() < 3 {
        return Err("a lot needs at least two legs".into());
    }
    Ok((pts, miss))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_edge_can_have_a_setback_and_a_note_of_its_own() {
        // 20 by 30: front 6, sides 3, rear 6 leave 14 by 18
        let lot = square(20.0, 30.0);
        let mut rules = PlanningRules::default();
        let b = buildable(&lot, &[], &rules).unwrap();
        assert!((area(&b) - 14.0 * 18.0).abs() < 1e-9);
        // the east side (edge 1) keeps 5 m of its own: 12 by 18
        rules.set_own_setback(1, Some(5.0));
        assert_eq!(rules.setback_at(&[], 1, 4), 5.0);
        assert_eq!(
            rules.setback_at(&[], 3, 4),
            3.0,
            "the other side keeps its kind's"
        );
        let b = buildable(&lot, &[], &rules).unwrap();
        assert!((area(&b) - 12.0 * 18.0).abs() < 1e-9);
        // a note is kept beside it, and an edge with neither is not kept
        rules.set_note(1, "Fire lane");
        rules.set_note(2, "  ");
        assert_eq!(rules.note_at(1), Some("Fire lane"));
        assert_eq!(rules.note_at(2), None);
        rules.set_own_setback(1, None);
        assert_eq!(rules.own.len(), 2);
        rules.set_note(1, "");
        rules.set_note(2, "");
        assert!(rules.own.is_empty());
        // drawn the other way round, an edge's own follows it
        let mut cw: Vec<DVec2> = lot.iter().rev().copied().collect();
        let (mut edges, mut own) = (
            vec![],
            vec![EdgeRule {
                edge: 0,
                setback: Some(9.0),
                note: String::new(),
                note_off: None,
            }],
        );
        make_ccw(&mut cw, &mut edges, &mut [], &mut own);
        assert_eq!(own[0].edge, 2);
    }

    fn square(w: f64, d: f64) -> Vec<DVec2> {
        vec![
            DVec2::ZERO,
            DVec2::new(w, 0.0),
            DVec2::new(w, d),
            DVec2::new(0.0, d),
        ]
    }

    #[test]
    fn setbacks_leave_the_buildable_area_by_edge() {
        let lot = square(20.0, 30.0);
        let rules = PlanningRules::default(); // front 6, side 3, rear 6
        let b = buildable(&lot, &[], &rules).unwrap();
        // front is the first edge (along y = 0), rear the third
        assert!((b[0] - DVec2::new(3.0, 6.0)).length() < 1e-9);
        assert!((b[2] - DVec2::new(17.0, 24.0)).length() < 1e-9);
        assert!((area(&b) - 14.0 * 18.0).abs() < 1e-9);
        // an open edge keeps nothing back
        let open = [LotEdge::Front, LotEdge::Open, LotEdge::Rear, LotEdge::Side];
        let b = buildable(&lot, &open, &rules).unwrap();
        assert!((area(&b) - 17.0 * 18.0).abs() < 1e-9);
        // setbacks that meet leave nothing
        let deep = PlanningRules {
            front: 16.0,
            rear: 16.0,
            ..rules
        };
        assert!(buildable(&lot, &[], &deep).is_none());
    }

    #[test]
    fn angle_planes_and_easements_keep_off_the_edges() {
        let lot = square(20.0, 30.0);
        let rules = PlanningRules {
            height: 9.0,
            angle: 45.0,
            angle_from: 3.0,
            ..PlanningRules::default()
        };
        // 4 m in from the front, 10 from either side: under the front plane
        let p = DVec2::new(10.0, 4.0);
        assert!((envelope_top(&lot, &[], &rules, p).unwrap() - 7.0).abs() < 1e-9);
        // in the middle the height limit is lower than every plane
        let mid = DVec2::new(10.0, 15.0);
        assert!((envelope_top(&lot, &[], &rules, mid).unwrap() - 9.0).abs() < 1e-9);
        // the planes agree with the function
        for (nrm, off) in angle_planes(&lot, &[], &rules) {
            let on = p.extend(envelope_top(&lot, &[], &rules, p).unwrap());
            assert!(nrm.dot(on) <= off + 1e-9);
        }
        assert_eq!(envelope_top(&lot, &[], &PlanningRules::default(), p), None);
        // a 3 m strip along the rear
        let e = Easement {
            name: "Drain".into(),
            edge: 2,
            width: 3.0,
        };
        let s = easement_outline(&lot, &e).unwrap();
        assert!((area(&s) - 60.0).abs() < 1e-9);
        assert!(s.iter().all(|q| q.y >= 27.0 - 1e-9));
    }

    #[test]
    fn a_clockwise_lot_keeps_its_edges() {
        let mut lot = square(20.0, 30.0);
        lot.reverse(); // clockwise: (0,30) (20,30) (20,0) (0,0)
        lot.rotate_left(3); // starts at (0,0) along the left side
        let mut edges = vec![LotEdge::Side, LotEdge::Rear, LotEdge::Side, LotEdge::Front];
        let front = (lot[3], lot[0]);
        make_ccw(&mut lot, &mut edges, &mut [], &mut []);
        let n = lot.len();
        let i = edges.iter().position(|e| *e == LotEdge::Front).unwrap();
        let (a, b) = (lot[i], lot[(i + 1) % n]);
        assert!((a == front.1 && b == front.0) || (a == front.0 && b == front.1));
        assert!(crate::polygon::signed_area2(&lot) > 0.0);
    }

    #[test]
    fn bearings_read_and_write_as_on_a_plat() {
        assert_eq!(bearing_text(12.504166666), "N 12°30'15\" E");
        assert_eq!(bearing_text(180.0 + 45.0), "S 45°00'00\" W");
        assert_eq!(parse_bearing("N 12°30'15\" E").unwrap(), 12.504166666666666);
        assert_eq!(parse_bearing("s45w"), Some(225.0));
        assert_eq!(parse_bearing("N 10 W"), Some(350.0));
        assert_eq!(parse_bearing("192.5"), Some(192.5));
        assert_eq!(parse_bearing("N 95 E"), None);
        // north is up; east is +x
        let e = direction(90.0, 0.0);
        assert!((e - DVec2::X).length() < 1e-12);
        assert!((azimuth(DVec2::new(0.0, -1.0), 0.0) - 180.0).abs() < 1e-9);
        // true north turned 10° anticlockwise of up
        assert!((azimuth(direction(33.0, 10.0), 10.0) - 33.0).abs() < 1e-9);
    }

    #[test]
    fn a_traverse_closes_on_itself() {
        let text = "N 90 E 20\nN 0 E 30\n# a note\nS 90 W 20.00 m\nS 0 E 30";
        let (pts, miss) = traverse(text, DVec2::ZERO, 0.0, |d| d).unwrap();
        assert_eq!(pts.len(), 4);
        assert!(miss < 1e-9);
        assert!((pts[2] - DVec2::new(20.0, 30.0)).length() < 1e-9);
        let (_, miss) = traverse("90 20\n0 30\n270 19.9", DVec2::ZERO, 0.0, |d| d).unwrap();
        assert!((miss - 30.0f64.hypot(0.1)).abs() < 1e-9);
        assert!(traverse("N 90 E twenty", DVec2::ZERO, 0.0, |d| d).is_err());
    }
}
