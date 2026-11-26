//! A car park: hard landscape that lays its own aisles, rows and bays
//! inside its outline, and counts them against what a permit asks for
//! (Mockup_Site_Car_Park REV02, P1).
//!
//! The rows run along its longest edge (or across it). From one side they
//! stack as modules: a row of bays, the aisle they open onto, a row facing
//! back. Two aisles or more are joined by a cross aisle at each end, so
//! cars can loop round. Where a road meets the outline is an entrance, and
//! the bays in its way are left out; where a path meets it is a door, and
//! the accessible bays go nearest that.

use glam::DVec2;
use serde::{Deserialize, Serialize};

use crate::polygon;

/// What a bay is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BayKind {
    Standard,
    /// With an access zone beside it.
    Accessible,
    /// With a charger.
    Ev,
    /// Parent and child.
    Parent,
    Motorbike,
    Delivery,
    /// Left out: a column, a hydrant, a tree kept.
    Out,
    /// A kerbed, planted island in a bay's place.
    Island,
}

impl BayKind {
    pub const ALL: [BayKind; 8] = [
        BayKind::Standard,
        BayKind::Accessible,
        BayKind::Ev,
        BayKind::Parent,
        BayKind::Motorbike,
        BayKind::Delivery,
        BayKind::Out,
        BayKind::Island,
    ];

    /// The kinds a schedule lists, in its order.
    pub const LISTED: [BayKind; 6] = [
        BayKind::Standard,
        BayKind::Ev,
        BayKind::Parent,
        BayKind::Accessible,
        BayKind::Motorbike,
        BayKind::Delivery,
    ];

    pub fn label(self) -> &'static str {
        match self {
            BayKind::Standard => "Standard",
            BayKind::Accessible => "Accessible",
            BayKind::Ev => "EV charging",
            BayKind::Parent => "Parent and child",
            BayKind::Motorbike => "Motorbike",
            BayKind::Delivery => "Delivery",
            BayKind::Out => "Left out",
            BayKind::Island => "Island",
        }
    }

    /// The letters written in the bay in plan; none for a standard bay.
    pub fn mark(self) -> &'static str {
        match self {
            BayKind::Standard | BayKind::Out | BayKind::Island => "",
            BayKind::Accessible => "A",
            BayKind::Ev => "EV",
            BayKind::Parent => "P",
            BayKind::Motorbike => "M",
            BayKind::Delivery => "D",
        }
    }

    /// Counted as a car's parking space against the requirement.
    pub fn for_cars(self) -> bool {
        matches!(
            self,
            BayKind::Standard | BayKind::Accessible | BayKind::Ev | BayKind::Parent
        )
    }
}

/// How many bays the car park has to have.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Default)]
pub enum Requirement {
    /// Nothing asked.
    #[default]
    None,
    /// A number, as the permit gives it.
    Number(u32),
    /// One bay per `per` m² of a building's `floor` area.
    PerFloor { floor: f64, per: f64 },
}

impl Requirement {
    pub fn required(&self) -> Option<u32> {
        match *self {
            Requirement::None => None,
            Requirement::Number(n) => Some(n),
            Requirement::PerFloor { floor, per } => {
                (per > 1e-9 && floor > 0.0).then(|| (floor / per - 1e-9).ceil().max(0.0) as u32)
            }
        }
    }
}

/// A bay changed by hand: the one whose middle is nearest `at` is `kind`,
/// however the car park is laid out again.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct BayChange {
    pub at: DVec2,
    pub kind: BayKind,
}

/// The parking of a hard landscape area.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CarPark {
    /// A bay across and along.
    pub bay_width: f64,
    pub bay_length: f64,
    /// The aisle between two rows, and the cross aisles.
    pub aisle: f64,
    /// Rows across the longest edge rather than along it.
    #[serde(default)]
    pub across: bool,
    /// How many accessible bays, and the access zone beside each.
    #[serde(default)]
    pub accessible: u32,
    pub access_zone: f64,
    #[serde(default)]
    pub ev: u32,
    #[serde(default)]
    pub parent: u32,
    /// What the permit asks: bays in all, and one accessible bay per so
    /// many (0 asks none).
    #[serde(default)]
    pub required: Requirement,
    #[serde(default)]
    pub accessible_per: u32,
    /// Bay numbers written in plan.
    #[serde(default = "yes")]
    pub numbers: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changes: Vec<BayChange>,
    /// Its aisles drawn by hand, the rows hanging off them; none, it lays
    /// its own.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aisles: Vec<Aisle>,
    /// Strips along its outline's edges, kept free of bays: a walkway,
    /// planting, a clearance.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub strips: Vec<EdgeStrip>,
    /// How far bays keep from the trunk of a tree standing in it; 0
    /// takes no notice of trees.
    #[serde(default = "tree_clear")]
    pub tree_clear: f64,
    /// The ground under it is worked to it: dug or made up to its
    /// underside, with banks where it meets the ground round it.
    #[serde(default = "yes")]
    pub ground: bool,
}

fn tree_clear() -> f64 {
    1.0
}

/// What a strip along an edge is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StripUse {
    /// A raised path for people on foot.
    Walkway,
    /// A kerbed, planted margin.
    Planting,
    /// Kept clear and level with the car park: beside a wall, a setback.
    Clear,
}

impl StripUse {
    pub const ALL: [StripUse; 3] = [StripUse::Walkway, StripUse::Planting, StripUse::Clear];

    pub fn label(self) -> &'static str {
        match self {
            StripUse::Walkway => "Walkway",
            StripUse::Planting => "Planting",
            StripUse::Clear => "Clearance",
        }
    }
}

/// A strip `width` deep along edge `edge` of the outline.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct EdgeStrip {
    pub edge: usize,
    pub width: f64,
    pub what: StripUse,
}

/// An aisle drawn by hand: a line with bends, rows of bays down its
/// sides.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Aisle {
    pub path: Vec<DVec2>,
    /// Its width; none takes the car park's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    #[serde(default)]
    pub one_way: bool,
    /// The inside kerb's radius at each bend.
    pub radius: f64,
    /// A row down its left and its right, looking along it.
    #[serde(default = "yes")]
    pub left: bool,
    #[serde(default = "yes")]
    pub right: bool,
    /// The bays' angle to it: 90 square on, 60 or 45 echelon, 0 parallel.
    pub angle: f64,
    /// A turning head at its start and its end.
    #[serde(default)]
    pub head_start: bool,
    #[serde(default)]
    pub head_end: bool,
    /// Rows drawn short by hand.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trims: Vec<RowTrim>,
}

/// A row kept to part of its stretch: the one down the `left` (or right)
/// of the aisle's stretch `seg`, its bays only from `from` to `to` along
/// that stretch, measured from the stretch's first point.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct RowTrim {
    pub seg: usize,
    pub left: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<f64>,
}

impl Aisle {
    pub fn new(path: Vec<DVec2>) -> Self {
        Self {
            path,
            width: None,
            one_way: false,
            radius: 4.0,
            left: true,
            right: true,
            angle: 90.0,
            head_start: false,
            head_end: false,
            trims: vec![],
        }
    }

    /// The row down one side of stretch `seg` starts (or, `end`, ends)
    /// at `s` along the stretch.
    pub fn trim(&mut self, seg: usize, left: bool, end: bool, s: f64) {
        let k = match self
            .trims
            .iter()
            .position(|t| t.seg == seg && t.left == left)
        {
            Some(k) => k,
            None => {
                self.trims.push(RowTrim {
                    seg,
                    left,
                    from: None,
                    to: None,
                });
                self.trims.len() - 1
            }
        };
        let t = &mut self.trims[k];
        if end {
            t.to = Some(s.max(t.from.unwrap_or(0.0)));
        } else {
            t.from = Some(s.max(0.0).min(t.to.unwrap_or(f64::MAX)));
        }
    }
}

/// A row as laid out: which aisle, stretch and side it hangs off, the
/// stretch's start on its kerb line (`o`), the way along it (`d`), half
/// its depth back from the kerb (`back`), and where its bays start and
/// end along the stretch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RowAt {
    pub aisle: usize,
    pub seg: usize,
    pub left: bool,
    pub o: DVec2,
    pub d: DVec2,
    pub back: DVec2,
    pub from: f64,
    pub to: f64,
}

/// An aisle as laid out: its middle line, for its arrows.
#[derive(Clone, Debug, PartialEq)]
pub struct AisleLine {
    pub path: Vec<DVec2>,
    pub one_way: bool,
    pub width: f64,
}

fn yes() -> bool {
    true
}

impl Default for CarPark {
    fn default() -> Self {
        Self {
            bay_width: 2.5,
            bay_length: 5.0,
            aisle: 6.0,
            across: false,
            accessible: 0,
            access_zone: 1.2,
            ev: 0,
            parent: 0,
            required: Requirement::None,
            accessible_per: 0,
            numbers: true,
            changes: vec![],
            aisles: vec![],
            strips: vec![],
            tree_clear: 1.0,
            ground: true,
        }
    }
}

/// A bay as laid out: its corners (the edge from the first to the second
/// is the open one, on the aisle), what it is and what it is called.
#[derive(Clone, Debug, PartialEq)]
pub struct Bay {
    pub corners: [DVec2; 4],
    pub kind: BayKind,
    pub label: String,
    /// Its row, from 0 (row A).
    pub row: usize,
}

impl Bay {
    pub fn middle(&self) -> DVec2 {
        self.corners.iter().copied().sum::<DVec2>() / 4.0
    }
}

/// A car park laid out.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Layout {
    pub bays: Vec<Bay>,
    /// The hatched access zones beside accessible bays.
    pub zones: Vec<[DVec2; 4]>,
    /// The ground of every aisle, a stretch at a time.
    pub aisles: Vec<[DVec2; 4]>,
    /// The aisles' middle lines.
    pub lines: Vec<AisleLine>,
    /// Turning heads: pockets beside dead ends, kept clear.
    pub heads: Vec<[DVec2; 4]>,
    /// Bays made islands: kerbed and planted.
    pub islands: Vec<[DVec2; 4]>,
    /// Its aisles are drawn by hand.
    pub by_hand: bool,
    /// The strips along its edges: each one's edge, its ground and what
    /// it is.
    pub strips: Vec<(usize, Vec<DVec2>, StripUse)>,
    /// Its rows, each with where it hangs and how far it runs.
    pub rows_at: Vec<RowAt>,
    pub rows: usize,
    /// What could not be done as asked, for the panel.
    pub notes: Vec<String>,
}

impl Layout {
    pub fn count(&self, kind: BayKind) -> usize {
        self.bays.iter().filter(|b| b.kind == kind).count()
    }

    /// The bays that count as car spaces.
    pub fn cars(&self) -> usize {
        self.bays.iter().filter(|b| b.kind.for_cars()).count()
    }

    /// The labels of the bays of `kind`, shortened to runs ("1–45").
    pub fn labels(&self, kind: BayKind) -> String {
        #[allow(clippy::type_complexity)]
        let mut runs: Vec<(String, String, Option<(String, u32)>)> = vec![];
        for b in self.bays.iter().filter(|b| b.kind == kind) {
            let split = b
                .label
                .find(|c: char| c.is_ascii_digit())
                .unwrap_or(b.label.len());
            let key = b.label[..split].to_string();
            let n: Option<u32> = b.label[split..].parse().ok();
            let joined = match (runs.last_mut(), n) {
                (Some((_, last, Some((k, m)))), Some(n)) if *k == key && *m + 1 == n => {
                    *last = b.label.clone();
                    *m = n;
                    true
                }
                _ => false,
            };
            if !joined {
                runs.push((b.label.clone(), b.label.clone(), n.map(|n| (key, n))));
            }
        }
        runs.iter()
            .map(|(a, b, _)| {
                if a == b {
                    a.clone()
                } else {
                    format!("{a}–{b}")
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// The letter of a row: A, B … Z, AA, AB …
pub fn row_letter(i: usize) -> String {
    let mut i = i;
    let mut s = String::new();
    loop {
        s.insert(0, (b'A' + (i % 26) as u8) as char);
        if i < 26 {
            break;
        }
        i = i / 26 - 1;
    }
    s
}

/// The step the outline is felt along a row at.
const FEEL: f64 = 0.1;

impl CarPark {
    pub fn validate(&self) -> Result<(), String> {
        let ok = |v: f64, lo: f64, hi: f64, what: &str| {
            if v.is_finite() && v >= lo && v <= hi {
                Ok(())
            } else {
                Err(format!(
                    "a car park's {what} must be {lo} to {hi} m, got {v}"
                ))
            }
        };
        ok(self.bay_width, 0.8, 6.0, "bay width")?;
        ok(self.bay_length, 1.5, 20.0, "bay length")?;
        ok(self.aisle, 2.0, 20.0, "aisle")?;
        ok(self.access_zone, 0.0, 3.0, "access zone")?;
        if let Requirement::PerFloor { floor, per } = self.required {
            if !(floor.is_finite() && per.is_finite() && floor >= 0.0 && per > 0.0) {
                return Err(
                    "a car park's requirement needs a floor area and an area per bay".into(),
                );
            }
        }
        if self.changes.iter().any(|c| !c.at.is_finite()) {
            return Err("a changed bay's place must be finite".into());
        }
        for st in &self.strips {
            ok(st.width, 0.1, 30.0, "edge strip")?;
        }
        if !(self.tree_clear.is_finite() && self.tree_clear >= 0.0) {
            return Err("a car park's clearance to trees must be 0 or more".into());
        }
        for a in &self.aisles {
            if a.path.len() < 2 || a.path.iter().any(|p| !p.is_finite()) {
                return Err("an aisle needs two finite points".into());
            }
            if let Some(w) = a.width {
                ok(w, 2.0, 20.0, "aisle")?;
            }
            if !(a.radius.is_finite() && a.radius >= 0.0 && a.angle.is_finite()) {
                return Err("an aisle's radius and angle must be finite".into());
            }
        }
        Ok(())
    }

    /// The bay `at` is `kind` from now on; none lets the layout decide
    /// again.
    pub fn change(&mut self, at: DVec2, kind: Option<BayKind>) {
        let near = self.bay_width.min(self.bay_length) / 2.0;
        self.changes.retain(|c| c.at.distance(at) > near);
        if let Some(kind) = kind {
            self.changes.push(BayChange { at, kind });
        }
    }

    /// Its bays inside `boundary` less `holes`. `entrances` are where
    /// roads meet it, `doors` where paths do. With aisles drawn by hand
    /// the rows hang off those; with none it lays its own.
    pub fn layout(
        &self,
        boundary: &[DVec2],
        holes: &[Vec<DVec2>],
        entrances: &[DVec2],
        doors: &[DVec2],
    ) -> Layout {
        self.layout_clear(boundary, holes, entrances, doors, &[])
    }

    /// [`Self::layout`], with ground the bays keep off besides (round a
    /// tree trunk).
    pub fn layout_clear(
        &self,
        boundary: &[DVec2],
        holes: &[Vec<DVec2>],
        entrances: &[DVec2],
        doors: &[DVec2],
        clear: &[Vec<DVec2>],
    ) -> Layout {
        let mut out = Layout::default();
        if boundary.len() < 3 || self.validate().is_err() {
            return out;
        }
        out.by_hand = !self.aisles.is_empty();
        // its edge strips take their ground first: the bays are laid in
        // what is left
        let inner = self.inset(boundary);
        let n = boundary.len();
        for st in self.strips.iter().filter(|s| s.edge < n) {
            let (i, j) = (st.edge, (st.edge + 1) % n);
            out.strips.push((
                i,
                vec![boundary[i], boundary[j], inner[j], inner[i]],
                st.what,
            ));
        }
        let boundary = &inner[..];
        let aisles = if out.by_hand {
            self.aisles.clone()
        } else {
            match self.own_aisles(boundary, holes) {
                Ok(a) => a,
                Err(note) => {
                    out.notes.push(note);
                    return out;
                }
            }
        };
        let mut blocked = holes.to_vec();
        blocked.extend(clear.iter().cloned());
        let stretches = self.hang_rows(&aisles, boundary, &blocked, entrances, &mut out);
        if out.bays.is_empty() {
            out.notes.push("No bay fits inside its outline.".into());
            return out;
        }
        self.special_bays(&stretches, entrances, doors, &mut out);
        out
    }

    /// The aisles it lays by itself: along its longest edge (or across
    /// it), a row, its aisle and a row facing back, again and again; two
    /// aisles or more joined by a cross aisle at each end.
    pub fn own_aisles(
        &self,
        boundary: &[DVec2],
        holes: &[Vec<DVec2>],
    ) -> Result<Vec<Aisle>, String> {
        let n = boundary.len();
        let (l, aisle) = (self.bay_length, self.aisle);
        // its frame: u along the rows, v across them
        let mut u = DVec2::X;
        let mut longest = 0.0;
        for i in 0..n {
            let e = boundary[(i + 1) % n] - boundary[i];
            if e.length() > longest + 1e-9 {
                longest = e.length();
                u = e / longest;
            }
        }
        if self.across {
            u = u.perp();
        }
        let v = u.perp();
        let at = |s: f64, t: f64| u * s + v * t;
        let (mut s0, mut s1, mut t0, mut t1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for p in boundary {
            s0 = s0.min(p.dot(u));
            s1 = s1.max(p.dot(u));
            t0 = t0.min(p.dot(v));
            t1 = t1.max(p.dot(v));
        }
        let inside = |p: DVec2| {
            polygon::contains(boundary, p) && !holes.iter().any(|h| polygon::contains(h, p))
        };
        // the aisles' middles across, stacked from one side; the last
        // one may have a row on one side only
        let mut mids: Vec<(f64, bool)> = vec![];
        let mut t = t0;
        let eps = 1e-6;
        while t + 2.0 * l + aisle <= t1 + eps {
            mids.push((t + l + aisle / 2.0, true));
            t += 2.0 * l + aisle;
        }
        if t + l + aisle <= t1 + eps {
            mids.push((t + l + aisle / 2.0, false));
        }
        if mids.is_empty() {
            return Err(format!(
                "Too narrow for a row and its aisle: it needs {:.1} m across.",
                l + aisle
            ));
        }
        let cross = if mids.len() >= 2 { aisle } else { 0.0 };
        let mut out = vec![];
        for (mid, both) in &mids {
            // where the aisle's own width lies inside, along
            let (a, b) = (mid - aisle / 2.0, mid + aisle / 2.0);
            let mut start: Option<f64> = None;
            let k = ((s1 - s0) / FEEL).ceil() as usize;
            for i in 0..=k {
                let s = s0 + FEEL * (i as f64 + 0.5);
                let ok = s < s1 && [a + 0.02, *mid, b - 0.02].iter().all(|t| inside(at(s, *t)));
                match (ok, start) {
                    (true, None) => start = Some(s - FEEL / 2.0),
                    (false, Some(from)) => {
                        let (from, to) = (from + cross / 2.0, s - FEEL / 2.0 - cross / 2.0);
                        if to - from > self.bay_width {
                            out.push(Aisle {
                                // a row on the far side only where it fits
                                left: *both,
                                right: true,
                                ..Aisle::new(vec![at(from, *mid), at(to, *mid)])
                            });
                        }
                        start = None;
                    }
                    _ => {}
                }
            }
        }
        if cross > 0.0 {
            let (lo, hi) = (mids[0].0, mids[mids.len() - 1].0);
            for s in [s0 + cross / 2.0, s1 - cross / 2.0] {
                out.push(Aisle {
                    left: false,
                    right: false,
                    ..Aisle::new(vec![at(s, lo), at(s, hi)])
                });
            }
        }
        Ok(out)
    }

    /// The rows along `aisles`: bays down each side of each straight
    /// stretch, wherever they fit inside the outline, clear of every
    /// aisle and of each other. Returns each unbroken stretch of square
    /// bays, for the accessible ones to go in.
    fn hang_rows(
        &self,
        aisles: &[Aisle],
        boundary: &[DVec2],
        holes: &[Vec<DVec2>],
        entrances: &[DVec2],
        out: &mut Layout,
    ) -> Vec<(Frame, Vec<(usize, f64)>)> {
        let (w, l) = (self.bay_width, self.bay_length);
        let inside = |p: DVec2| {
            polygon::contains(boundary, p) && !holes.iter().any(|h| polygon::contains(h, p))
        };
        // every aisle's own ground, which no bay may stand on
        let mut ways: Vec<Vec<DVec2>> = vec![];
        for a in aisles {
            let half = a.width.unwrap_or(self.aisle) / 2.0;
            let m = a.path.len();
            for i in 0..m.saturating_sub(1) {
                let (p, q) = (a.path[i], a.path[i + 1]);
                let len = p.distance(q);
                if len < 1e-6 {
                    continue;
                }
                let d = (q - p) / len;
                // through a bend, to cover its corner
                let (p, q) = (
                    if i > 0 { p - d * half } else { p },
                    if i + 2 < m { q + d * half } else { q },
                );
                let side = d.perp() * half;
                let quad = [p - side, q - side, q + side, p + side];
                out.aisles.push(quad);
                ways.push(quad.to_vec());
            }
            out.lines.push(AisleLine {
                path: a.path.clone(),
                one_way: a.one_way,
                width: half * 2.0,
            });
        }
        // a road's way in from the outline to the nearest aisle
        for e in entrances {
            let near = ways
                .iter()
                .flat_map(|q| (0..4).map(move |k| nearest_on(q[k], q[(k + 1) % 4], *e)))
                .min_by(|a, b| a.distance(*e).total_cmp(&b.distance(*e)));
            let reach = 2.0 * l + self.aisle;
            match near {
                Some(p) if p.distance(*e) <= reach => {
                    let len = p.distance(*e);
                    if len > 0.05 {
                        let d = (p - *e) / len;
                        let side = d.perp() * self.aisle / 2.0;
                        ways.push(vec![*e - side, p - side, p + side, *e + side]);
                    }
                }
                _ => out.notes.push(
                    "A road meets it where no aisle is near: move the road's end to an aisle."
                        .into(),
                ),
            }
        }
        // turning heads: a pocket beside a dead end, kept clear
        for (k, a) in aisles.iter().enumerate() {
            let m = a.path.len();
            if m < 2 {
                continue;
            }
            let half = a.width.unwrap_or(self.aisle) / 2.0;
            for (on, end, back) in [
                (a.head_start, a.path[0], a.path[1]),
                (a.head_end, a.path[m - 1], a.path[m - 2]),
            ] {
                let len = end.distance(back);
                if len < 1e-6 {
                    continue;
                }
                let d = (back - end) / len;
                if on {
                    let reach = (2.0 * half).min(len);
                    let pocket = [d.perp(), -d.perp()].into_iter().find_map(|n| {
                        let q = [
                            end + n * half,
                            end + d * reach + n * half,
                            end + d * reach + n * (half + l),
                            end + n * (half + l),
                        ];
                        let m = (q[0] + q[2]) / 2.0;
                        let ok = q.iter().all(|p| inside(*p + (m - *p) * 0.02)) && inside(m);
                        ok.then_some(q)
                    });
                    match pocket {
                        Some(q) => {
                            out.heads.push(q);
                            ways.push(q.to_vec());
                        }
                        None => out.notes.push(format!(
                            "Aisle {}: no room for a turning head at its end.",
                            k + 1
                        )),
                    }
                    continue;
                }
                // a dead end: not at the outline (a way in or out) and not
                // on another aisle
                let at_edge = (0..boundary.len()).any(|i| {
                    nearest_on(boundary[i], boundary[(i + 1) % boundary.len()], end).distance(end)
                        < 1.5
                }) || !inside(end);
                let joins = aisles.iter().enumerate().any(|(j, o)| {
                    j != k
                        && o.path.windows(2).any(|s| {
                            nearest_on(s[0], s[1], end).distance(end)
                                < o.width.unwrap_or(self.aisle) / 2.0 + 0.5
                        })
                });
                let long = a.path.windows(2).map(|s| s[0].distance(s[1])).sum::<f64>()
                    > 4.0 * w + 2.0 * half;
                if !at_edge && !joins && long && (a.left || a.right) {
                    out.notes.push(format!(
                        "Aisle {} ends in a dead end: give it a turning head.",
                        k + 1
                    ));
                }
            }
        }

        let mut stretches: Vec<(Frame, Vec<(usize, f64)>)> = vec![];
        let mut boxes: Vec<(DVec2, DVec2)> = vec![];
        let mut row = 0;
        for (ai, a) in aisles.iter().enumerate() {
            let half = a.width.unwrap_or(self.aisle) / 2.0;
            let m = a.path.len();
            let angle = a.angle.clamp(0.0, 90.0).to_radians();
            for i in 0..m.saturating_sub(1) {
                let (p, q) = (a.path[i], a.path[i + 1]);
                let len = p.distance(q);
                if len < 1e-6 {
                    continue;
                }
                let d = (q - p) / len;
                // the straight part: clear of the bends' curves
                let tangent = |j: usize| -> f64 {
                    if j == 0 || j + 1 >= m {
                        return 0.0;
                    }
                    let (x, y) = (a.path[j] - a.path[j - 1], a.path[j + 1] - a.path[j]);
                    let turn = x.angle_to(y).abs();
                    ((a.radius.max(0.0) + half) * (turn / 2.0).tan()).min(len / 2.0)
                };
                let (from, to) = (tangent(i), len - tangent(i + 1));
                for (on, n, left) in [(a.left, d.perp(), true), (a.right, -d.perp(), false)] {
                    if !on {
                        continue;
                    }
                    // drawn short by hand
                    let trim = a.trims.iter().find(|t| t.seg == i && t.left == left);
                    let (from, to) = (
                        trim.and_then(|t| t.from).map_or(from, |v| v.max(from)),
                        trim.and_then(|t| t.to).map_or(to, |v| v.min(to)),
                    );
                    let frame = if angle < 1f64.to_radians() {
                        // parallel: along the kerb
                        Frame {
                            o: p + n * half,
                            d,
                            axis: n,
                            depth: w,
                            step: l,
                            square: false,
                        }
                    } else {
                        Frame {
                            o: p + n * half,
                            d,
                            axis: d * angle.cos() + n * angle.sin(),
                            depth: l,
                            step: w / angle.sin(),
                            square: angle > 89f64.to_radians(),
                        }
                    };
                    let mut s = from;
                    let mut stretch: Vec<(usize, f64)> = vec![];
                    let mut any = false;
                    let mut span: Option<(f64, f64)> = None;
                    while s + frame.step <= to + 1e-6 {
                        let c = frame.bay(s, s + frame.step);
                        let mid = (c[0] + c[2]) / 2.0;
                        // a little inside itself, so touching is not on
                        let small: Vec<DVec2> = c.iter().map(|p| *p + (mid - *p) * 0.03).collect();
                        let (lo, hi) = small
                            .iter()
                            .fold((small[0], small[0]), |(a, b), p| (a.min(*p), b.max(*p)));
                        let fits = small.iter().chain([&mid]).all(|p| inside(*p))
                            && !holes.iter().any(|h| polygon::overlap(&small, h))
                            && !ways.iter().any(|q| polygon::overlap(&small, q))
                            && !boxes.iter().zip(&out.bays).any(|((a, b), bay)| {
                                lo.x < b.x
                                    && hi.x > a.x
                                    && lo.y < b.y
                                    && hi.y > a.y
                                    && polygon::overlap(&small, &bay.corners)
                            });
                        if !fits {
                            if !stretch.is_empty() {
                                stretches.push((frame, std::mem::take(&mut stretch)));
                            }
                            // feel along for the next place one fits
                            s += 0.25;
                            continue;
                        }
                        stretch.push((out.bays.len(), s));
                        let (blo, bhi) = c
                            .iter()
                            .fold((c[0], c[0]), |(a, b), p| (a.min(*p), b.max(*p)));
                        boxes.push((blo, bhi));
                        out.bays.push(Bay {
                            corners: c,
                            kind: BayKind::Standard,
                            label: String::new(),
                            row,
                        });
                        any = true;
                        span = Some((span.map_or(s, |v| v.0), s + frame.step));
                        s += frame.step;
                    }
                    if !stretch.is_empty() {
                        stretches.push((frame, stretch));
                    }
                    if any {
                        row += 1;
                    }
                    if let Some((from, to)) = span {
                        out.rows_at.push(RowAt {
                            aisle: ai,
                            seg: i,
                            left,
                            o: frame.o,
                            d,
                            back: (frame.axis - d * frame.axis.dot(d)) * frame.depth / 2.0,
                            from,
                            to,
                        });
                    }
                }
            }
        }
        out.rows = row;
        stretches
    }

    /// The accessible, parent and charging bays by their counts, then
    /// what was changed by hand, and every bay's number.
    fn special_bays(
        &self,
        stretches: &[(Frame, Vec<(usize, f64)>)],
        entrances: &[DVec2],
        doors: &[DVec2],
        out: &mut Layout,
    ) {
        let (w, l) = (self.bay_width, self.bay_length);
        // accessible bays nearest the door (a path's end), else the
        // entrance, else the first bay; in twos, sharing a zone
        let door = doors
            .first()
            .or(entrances.first())
            .copied()
            .unwrap_or_else(|| out.bays[0].middle());
        let mut gone: Vec<usize> = vec![];
        let mut left = self.accessible;
        let zone = self.access_zone;
        while left > 0 {
            let group = left.min(2) as usize;
            let need = ((group as f64 * (w + zone)) / w - 1e-6).ceil() as usize;
            let mut best: Option<(f64, usize, usize)> = None;
            for (si, (frame, idx)) in stretches.iter().enumerate() {
                if idx.len() < need || !frame.square {
                    continue;
                }
                for j in 0..=idx.len() - need {
                    let window = &idx[j..j + need];
                    let taken = window
                        .iter()
                        .any(|(i, _)| gone.contains(i) || out.bays[*i].kind != BayKind::Standard);
                    // side by side, with no gap between them
                    let apart = window
                        .windows(2)
                        .any(|p| (p[1].1 - p[0].1 - w).abs() > 0.01);
                    if taken || apart {
                        continue;
                    }
                    let mid = (out.bays[window[0].0].middle()
                        + out.bays[window[need - 1].0].middle())
                        / 2.0;
                    let d = mid.distance(door);
                    if best.is_none_or(|(bd, ..)| d < bd - 1e-9) {
                        best = Some((d, si, j));
                    }
                }
            }
            let Some((_, si, j)) = best else {
                out.notes.push(format!(
                    "No room for {left} more accessible bay{}: each pair needs {need} square bays side by side.",
                    if left == 1 { "" } else { "s" }
                ));
                break;
            };
            let (frame, idx) = &stretches[si];
            let window: Vec<(usize, f64)> = idx[j..j + need].to_vec();
            let x0 = window[0].1;
            // the first of them take the accessible bays; the rest go
            for (g, (i, _)) in window.iter().enumerate() {
                if g < group {
                    let a = x0 + g as f64 * (w + zone);
                    out.bays[*i].corners = frame.bay(a, a + w);
                    out.bays[*i].kind = BayKind::Accessible;
                    out.zones.push(frame.bay(a + w, a + w + zone));
                } else {
                    gone.push(*i);
                }
            }
            left -= group as u32;
        }

        // parents next nearest the door; chargers at the far end of the
        // row farthest from it
        let free = |bays: &[Bay], gone: &[usize]| -> Vec<usize> {
            (0..bays.len())
                .filter(|i| !gone.contains(i) && bays[*i].kind == BayKind::Standard)
                .collect()
        };
        let mut near = free(&out.bays, &gone);
        near.sort_by(|a, b| {
            out.bays[*a]
                .middle()
                .distance(door)
                .total_cmp(&out.bays[*b].middle().distance(door))
        });
        for i in near.into_iter().take(self.parent as usize) {
            out.bays[i].kind = BayKind::Parent;
        }
        let mut far = free(&out.bays, &gone);
        let row_far = |r: usize| {
            out.bays
                .iter()
                .filter(|b| b.row == r)
                .map(|b| b.middle().distance(door))
                .fold(0.0, f64::max)
        };
        let far_rows: Vec<f64> = (0..out.rows).map(row_far).collect();
        far.sort_by(|a, b| {
            let (ba, bb) = (&out.bays[*a], &out.bays[*b]);
            far_rows[bb.row].total_cmp(&far_rows[ba.row]).then(
                bb.middle()
                    .distance(door)
                    .total_cmp(&ba.middle().distance(door)),
            )
        });
        for i in far.into_iter().take(self.ev as usize) {
            out.bays[i].kind = BayKind::Ev;
        }

        // what was changed by hand
        let reach = w.max(l) / 2.0;
        for c in &self.changes {
            let nearest = (0..out.bays.len())
                .filter(|i| !gone.contains(i))
                .min_by(|a, b| {
                    out.bays[*a]
                        .middle()
                        .distance(c.at)
                        .total_cmp(&out.bays[*b].middle().distance(c.at))
                });
            if let Some(i) = nearest {
                // an accessible bay is made by the count, with its zone
                if out.bays[i].middle().distance(c.at) <= reach
                    && out.bays[i].kind != BayKind::Accessible
                    && c.kind != BayKind::Accessible
                {
                    out.bays[i].kind = c.kind;
                }
            }
        }

        // the bays taken by accessible ones' zones and those left out
        // go; a bay made an island is kerbed and planted
        let mut i = 0;
        let mut islands = vec![];
        out.bays.retain(|b| {
            let keep = !gone.contains(&i) && b.kind != BayKind::Out && b.kind != BayKind::Island;
            if !gone.contains(&i) && b.kind == BayKind::Island {
                islands.push(b.corners);
            }
            i += 1;
            keep
        });
        out.islands = islands;
        // numbered along each row; the accessible ones on their own
        let (mut k, mut a) = (0, 0);
        for b in out.bays.iter_mut() {
            if b.kind == BayKind::Accessible {
                a += 1;
                b.label = format!("A{a}");
            } else {
                k += 1;
                b.label = k.to_string();
            }
        }
    }

    /// How deep the strip along edge `i` is; 0 with none.
    pub fn strip_width(&self, i: usize) -> f64 {
        self.strips
            .iter()
            .find(|s| s.edge == i)
            .map_or(0.0, |s| s.width)
    }

    /// Edge `i` has a strip of `what`, `width` deep; none takes it away.
    pub fn set_strip(&mut self, i: usize, strip: Option<(StripUse, f64)>) {
        self.strips.retain(|s| s.edge != i);
        if let Some((what, width)) = strip {
            self.strips.push(EdgeStrip {
                edge: i,
                width,
                what,
            });
            self.strips.sort_by_key(|s| s.edge);
        }
    }

    /// The outline with each edge moved in by its strip.
    pub fn inset(&self, boundary: &[DVec2]) -> Vec<DVec2> {
        let n = boundary.len();
        if n < 3 || self.strips.is_empty() {
            return boundary.to_vec();
        }
        // each edge's line, moved in (the outline runs counter-clockwise)
        let line = |i: usize| {
            let (a, b) = (boundary[i], boundary[(i + 1) % n]);
            let d = (b - a).normalize_or_zero();
            (a + d.perp() * self.strip_width(i), d)
        };
        (0..n)
            .map(|i| {
                // where the edge before and the edge from this corner meet
                let ((p, d), (q, e)) = (line((i + n - 1) % n), line(i));
                let den = d.perp_dot(e);
                if den.abs() < 1e-9 {
                    return q;
                }
                p + d * ((q - p).perp_dot(e) / den)
            })
            .collect()
    }

    /// Corner `i` was added to the outline, splitting edge `i`: both
    /// halves keep its strip.
    pub fn corner_added(&mut self, i: usize) {
        let split = self.strips.iter().find(|s| s.edge == i).copied();
        for s in self.strips.iter_mut().filter(|s| s.edge > i) {
            s.edge += 1;
        }
        if let Some(s) = split {
            self.strips.push(EdgeStrip { edge: i + 1, ..s });
        }
        self.strips.sort_by_key(|s| s.edge);
    }

    /// Corner `i` of the outline was taken away: the edge that began
    /// there goes, with its strip.
    pub fn corner_removed(&mut self, i: usize) {
        self.strips.retain(|s| s.edge != i);
        for s in self.strips.iter_mut().filter(|s| s.edge > i) {
            s.edge -= 1;
        }
    }

    /// Aisle `k`'s line gained or lost a point: its rows' own lengths,
    /// which went by its stretches, are let go.
    pub fn aisle_reshaped(&mut self, k: usize) {
        if let Some(a) = self.aisles.get_mut(k) {
            a.trims.clear();
        }
    }

    /// Its aisles become lines to draw and bend by hand: the ones it lays
    /// by itself, as they stand.
    pub fn take_aisles(&mut self, boundary: &[DVec2], holes: &[Vec<DVec2>]) {
        if self.aisles.is_empty() {
            if let Ok(a) = self.own_aisles(boundary, holes) {
                self.aisles = a;
            }
        }
    }

    /// What its count says against what is asked: each line, and whether
    /// it falls short.
    pub fn verdict(&self, layout: &Layout) -> Vec<(String, bool)> {
        let mut out = vec![];
        let cars = layout.cars() as u32;
        let s = |n: u32| if n == 1 { "" } else { "s" };
        match self.required.required() {
            Some(need) => out.push((
                format!("{cars} bay{} for {need} required.", s(cars)),
                cars < need,
            )),
            None => out.push((format!("{cars} bay{}.", s(cars)), false)),
        }
        if self.accessible_per > 0 {
            let need = (cars as f64 / self.accessible_per as f64 - 1e-9)
                .ceil()
                .max(0.0) as u32;
            let have = layout.count(BayKind::Accessible) as u32;
            out.push((
                format!(
                    "{have} accessible for {need} required (1 per {} bays).",
                    self.accessible_per
                ),
                have < need,
            ));
        }
        out
    }
}

/// A row's frame: `o` on the aisle's edge where the row starts, `d`
/// along it, `axis` the way a bay runs back from the aisle, `depth` long;
/// each bay `step` along. `square` for bays square to the aisle.
#[derive(Clone, Copy, Debug)]
struct Frame {
    o: DVec2,
    d: DVec2,
    axis: DVec2,
    depth: f64,
    step: f64,
    square: bool,
}

impl Frame {
    /// The bay from `x0` to `x1` along the row, its open edge first.
    fn bay(&self, x0: f64, x1: f64) -> [DVec2; 4] {
        let (a, b) = (self.o + self.d * x0, self.o + self.d * x1);
        [b, a, a + self.axis * self.depth, b + self.axis * self.depth]
    }
}

/// The point of the stretch from `a` to `b` nearest `p`.
fn nearest_on(a: DVec2, b: DVec2, p: DVec2) -> DVec2 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-12)).clamp(0.0, 1.0);
    a + ab * t
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(w: f64, d: f64) -> Vec<DVec2> {
        vec![
            DVec2::ZERO,
            DVec2::new(w, 0.0),
            DVec2::new(w, d),
            DVec2::new(0.0, d),
        ]
    }

    #[test]
    fn the_mockups_outline_holds_60_plain_bays() {
        // 47 x 32 m: two modules, a cross aisle each end, 14 bays a row
        // (35 m between the cross aisles)
        let cp = CarPark::default();
        let lay = cp.layout(&rect(47.0, 32.0), &[], &[], &[]);
        assert_eq!(lay.rows, 4);
        assert_eq!(lay.bays.len(), 60);
        assert_eq!(lay.aisles.len(), 4);
        assert!(lay.notes.is_empty(), "{:?}", lay.notes);
        // every bay inside, none overlapping a cross aisle
        for b in &lay.bays {
            for p in b.corners {
                assert!(p.x >= 3.0 - 1e-6 && p.x <= 44.0 + 1e-6, "{p}");
            }
        }
        assert_eq!(lay.bays[0].label, "1");
        assert_eq!(lay.bays[59].label, "60");
    }

    #[test]
    fn one_aisle_needs_no_cross_aisles() {
        let lay = CarPark::default().layout(&rect(25.0, 16.0), &[], &[], &[]);
        assert_eq!(lay.rows, 2);
        assert_eq!(lay.bays.len(), 20);
        assert_eq!(lay.aisles.len(), 1);
    }

    #[test]
    fn a_single_row_where_only_one_fits() {
        let lay = CarPark::default().layout(&rect(25.0, 12.0), &[], &[], &[]);
        assert_eq!(lay.rows, 1);
        assert_eq!(lay.bays.len(), 10);
        let narrow = CarPark::default().layout(&rect(25.0, 9.0), &[], &[], &[]);
        assert!(narrow.bays.is_empty());
        assert_eq!(narrow.notes.len(), 1);
    }

    #[test]
    fn accessible_bays_pair_up_nearest_the_door() {
        let cp = CarPark {
            accessible: 2,
            ev: 4,
            ..CarPark::default()
        };
        // the door at the top left, past row D's start
        let door = DVec2::new(8.0, 32.0);
        let lay = cp.layout(&rect(47.0, 32.0), &[], &[], &[door]);
        // a pair takes three bays' room: 60 - 3 + 2
        assert_eq!(lay.bays.len(), 59);
        assert_eq!(lay.count(BayKind::Accessible), 2);
        assert_eq!(lay.zones.len(), 2);
        assert_eq!(lay.count(BayKind::Ev), 4);
        for b in lay.bays.iter().filter(|b| b.kind == BayKind::Accessible) {
            assert!(b.middle().distance(door) < 9.0, "{}", b.middle());
        }
        // the chargers are in the row farthest from the door
        for b in lay.bays.iter().filter(|b| b.kind == BayKind::Ev) {
            assert!(b.middle().y < 5.0);
        }
        assert_eq!(lay.labels(BayKind::Accessible), "A1–A2");
        assert_eq!(lay.cars(), 59);
    }

    #[test]
    fn a_road_meeting_a_row_clears_its_way_in() {
        let cp = CarPark::default();
        // a road arriving at the middle of the bottom edge, onto row A
        let lay = cp.layout(&rect(47.0, 32.0), &[], &[DVec2::new(23.5, 0.0)], &[]);
        assert!(lay.notes.is_empty(), "{:?}", lay.notes);
        // 6 m of row A gives way: two to four bays
        assert!(
            lay.bays.len() <= 58 && lay.bays.len() >= 56,
            "{}",
            lay.bays.len()
        );
        // and one arriving at a corner clears the corner bays in its way
        let lay = cp.layout(&rect(47.0, 32.0), &[], &[DVec2::new(3.0, 0.0)], &[]);
        assert!(
            lay.bays.len() < 60 && lay.bays.len() >= 57,
            "{}",
            lay.bays.len()
        );
    }

    #[test]
    fn a_bay_changed_by_hand_stays_changed() {
        let mut cp = CarPark::default();
        let lay = cp.layout(&rect(47.0, 32.0), &[], &[], &[]);
        let at = lay.bays[5].middle();
        cp.change(at, Some(BayKind::Out));
        let lay = cp.layout(&rect(47.0, 32.0), &[], &[], &[]);
        assert_eq!(lay.bays.len(), 59);
        cp.change(at, Some(BayKind::Delivery));
        assert_eq!(cp.changes.len(), 1);
        // a little bigger outline: the bay is still the nearest one
        let lay = cp.layout(&rect(47.2, 32.0), &[], &[], &[]);
        assert_eq!(lay.count(BayKind::Delivery), 1);
        assert!(lay.cars() >= 59);
        cp.change(at, None);
        assert!(cp.changes.is_empty());
    }

    #[test]
    fn holes_and_odd_outlines_keep_bays_inside() {
        // an L: the top right quarter missing
        let l = vec![
            DVec2::ZERO,
            DVec2::new(47.0, 0.0),
            DVec2::new(47.0, 16.0),
            DVec2::new(24.0, 16.0),
            DVec2::new(24.0, 32.0),
            DVec2::new(0.0, 32.0),
        ];
        let lay = CarPark::default().layout(&l, &[], &[], &[]);
        assert!(!lay.bays.is_empty());
        for b in &lay.bays {
            let m = b.middle();
            assert!(polygon::contains(&l, m), "{m}");
        }
        // a hole takes the bays it lies on
        let hole = vec![rect(5.0, 5.0)
            .iter()
            .map(|p| *p + DVec2::new(15.0, 0.0))
            .collect()];
        let with = CarPark::default().layout(&rect(47.0, 32.0), &hole, &[], &[]);
        assert!(with.bays.len() < 60);
    }

    #[test]
    fn the_count_reads_against_what_is_asked() {
        let cp = CarPark {
            accessible: 2,
            accessible_per: 30,
            required: Requirement::PerFloor {
                floor: 1500.0,
                per: 30.0,
            },
            ..CarPark::default()
        };
        let lay = cp.layout(&rect(47.0, 32.0), &[], &[], &[]);
        let v = cp.verdict(&lay);
        assert_eq!(v[0], ("59 bays for 50 required.".to_string(), false));
        assert_eq!(
            v[1],
            (
                "2 accessible for 2 required (1 per 30 bays).".to_string(),
                false
            )
        );
        let short = CarPark {
            required: Requirement::Number(80),
            ..CarPark::default()
        };
        assert!(short.verdict(&short.layout(&rect(47.0, 32.0), &[], &[], &[]))[0].1);
        assert_eq!(row_letter(0), "A");
        assert_eq!(row_letter(27), "AB");
    }

    #[test]
    fn rows_hang_off_an_aisle_drawn_by_hand() {
        // two bays by a house: a short drive with a row of two on one side
        let lot = rect(12.0, 8.0);
        let mut cp = CarPark {
            aisles: vec![Aisle {
                left: false,
                width: Some(3.2),
                ..Aisle::new(vec![DVec2::new(1.6, 0.0), DVec2::new(1.6, 6.0)])
            }],
            ..CarPark::default()
        };
        let lay = cp.layout(&lot, &[], &[], &[]);
        assert!(lay.by_hand);
        // the drive runs up (+y): its right is +x, where 6 m takes two bays
        assert_eq!(lay.bays.len(), 2, "{:?}", lay.notes);
        for b in &lay.bays {
            assert!(b.middle().x > 3.2 && b.middle().x < 8.3, "{}", b.middle());
        }
        // both sides off: none
        cp.aisles[0].right = false;
        assert!(cp.layout(&lot, &[], &[], &[]).bays.is_empty());
    }

    #[test]
    fn a_bent_aisle_keeps_bays_off_its_bend_and_each_other() {
        let lot = rect(40.0, 40.0);
        let cp = CarPark {
            aisles: vec![Aisle::new(vec![
                DVec2::new(0.0, 12.0),
                DVec2::new(28.0, 12.0),
                DVec2::new(28.0, 40.0),
            ])],
            ..CarPark::default()
        };
        let lay = cp.layout(&lot, &[], &[], &[]);
        assert!(lay.bays.len() > 20, "{}", lay.bays.len());
        // no two bays share ground, and none stands on the aisle
        for (i, a) in lay.bays.iter().enumerate() {
            let inner = |b: &Bay| -> Vec<DVec2> {
                let m = b.middle();
                b.corners.iter().map(|p| *p + (m - *p) * 0.05).collect()
            };
            for b in &lay.bays[i + 1..] {
                assert!(
                    !polygon::overlap(&inner(a), &inner(b)),
                    "{} {}",
                    a.middle(),
                    b.middle()
                );
            }
            for q in &lay.aisles {
                assert!(!polygon::overlap(&inner(a), q), "{}", a.middle());
            }
        }
    }

    #[test]
    fn angled_and_parallel_rows_and_a_turning_head() {
        let lot = rect(40.0, 20.0);
        let line = vec![DVec2::new(0.0, 10.0), DVec2::new(34.0, 10.0)];
        let count = |angle: f64| {
            let cp = CarPark {
                aisles: vec![Aisle {
                    angle,
                    ..Aisle::new(line.clone())
                }],
                ..CarPark::default()
            };
            cp.layout(&lot, &[], &[], &[]).bays.len()
        };
        let (square, sixty, parallel) = (count(90.0), count(60.0), count(0.0));
        assert_eq!(square, 26);
        assert!(sixty < square && sixty >= 20, "{sixty}");
        assert_eq!(parallel, 12);
        // a dead end is noted; a turning head answers it and takes bays
        let mut cp = CarPark {
            aisles: vec![Aisle::new(line.clone())],
            ..CarPark::default()
        };
        let lay = cp.layout(&lot, &[], &[], &[]);
        assert!(
            lay.notes.iter().any(|n| n.contains("dead end")),
            "{:?}",
            lay.notes
        );
        cp.aisles[0].head_end = true;
        let lay = cp.layout(&lot, &[], &[], &[]);
        assert!(lay.notes.is_empty(), "{:?}", lay.notes);
        assert_eq!(lay.heads.len(), 1);
        assert!(lay.bays.len() < square);
    }

    #[test]
    fn taking_the_aisles_over_keeps_the_layout_and_an_island_takes_a_bay() {
        let lot = rect(47.0, 32.0);
        let mut cp = CarPark::default();
        let before = cp.layout(&lot, &[], &[], &[]);
        cp.take_aisles(&lot, &[]);
        assert_eq!(cp.aisles.len(), 4);
        let after = cp.layout(&lot, &[], &[], &[]);
        assert!(after.by_hand);
        assert_eq!(after.bays.len(), before.bays.len());
        cp.change(after.bays[3].middle(), Some(BayKind::Island));
        let lay = cp.layout(&lot, &[], &[], &[]);
        assert_eq!(lay.islands.len(), 1);
        assert_eq!(lay.bays.len(), before.bays.len() - 1);
    }

    #[test]
    fn a_row_drawn_short_keeps_its_ends() {
        let lot = rect(40.0, 20.0);
        let mut cp = CarPark {
            aisles: vec![Aisle::new(vec![
                DVec2::new(0.0, 10.0),
                DVec2::new(40.0, 10.0),
            ])],
            ..CarPark::default()
        };
        let full = cp.layout(&lot, &[], &[], &[]);
        assert_eq!(full.bays.len(), 32);
        assert_eq!(full.rows_at.len(), 2);
        let left = full.rows_at.iter().find(|r| r.left).unwrap();
        assert!((left.from - 0.0).abs() < 1e-6 && (left.to - 40.0).abs() < 1e-6);
        // the left row from 10 m to 21 m: four bays, the rest of a fifth unused
        cp.aisles[0].trim(0, true, false, 10.0);
        cp.aisles[0].trim(0, true, true, 21.0);
        let lay = cp.layout(&lot, &[], &[], &[]);
        assert_eq!(lay.bays.len(), 16 + 4);
        let left = lay.rows_at.iter().find(|r| r.left).unwrap();
        assert!(
            (left.from - 10.0).abs() < 1e-6 && (left.to - 20.0).abs() < 1e-6,
            "{left:?}"
        );
        // a point added to the aisle lets the rows' lengths go
        cp.aisle_reshaped(0);
        assert_eq!(cp.layout(&lot, &[], &[], &[]).bays.len(), 32);
    }

    #[test]
    fn edge_strips_take_their_ground_and_bays_step_round_what_is_kept_clear() {
        let lot = rect(47.0, 32.0);
        let mut cp = CarPark::default();
        // a 2 m walkway along the far edge (edge 2: from (47,32) to (0,32))
        cp.set_strip(2, Some((StripUse::Walkway, 2.0)));
        let inner = cp.inset(&lot);
        assert!((inner[2].y - 30.0).abs() < 1e-9 && (inner[3].y - 30.0).abs() < 1e-9);
        assert!((inner[0].y - 0.0).abs() < 1e-9);
        let lay = cp.layout(&lot, &[], &[], &[]);
        assert_eq!(lay.strips.len(), 1);
        // 30 m deep: one module and a row with its aisle
        assert_eq!(lay.rows, 3);
        for b in &lay.bays {
            for p in b.corners {
                assert!(p.y <= 30.0 + 1e-6, "{p}");
            }
        }
        // what is kept clear round a tree takes the bays it touches
        let plain = CarPark::default();
        let all = plain.layout(&lot, &[], &[], &[]).bays.len();
        let trunk: Vec<DVec2> = rect(2.0, 2.0)
            .iter()
            .map(|p| *p + DVec2::new(20.0, 1.0))
            .collect();
        let round = plain.layout_clear(&lot, &[], &[], &[], &[trunk]).bays.len();
        assert!(round < all && round >= all - 2, "{round} of {all}");
        // a corner added on the strip's edge: both halves keep it
        cp.corner_added(2);
        assert_eq!(cp.strips.len(), 2);
        cp.corner_removed(3);
        assert_eq!(cp.strips.len(), 1);
    }
}
