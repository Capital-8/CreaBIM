//! Grid systems: named groups of grid lines with their own labelling, so
//! one project can carry building A's grid (A-1, A-2…) beside building B's
//! (B-A, B-B…) without the labels running into each other.
//!
//! A grid line belongs to at most one group ([`Grid::group`]). Its stored
//! label is the part after the group's prefix, so renaming the prefix
//! relabels every line of the group at once. Lines running north–south
//! (more along y than x) and lines running east–west are counted
//! separately, each in its own style: letters or numbers.

use crate::{Element, ElementId, ElementKind, Grid, Model, Transaction};
use glam::DVec2;
use serde::{Deserialize, Serialize};

/// How a set of grid lines is counted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LabelStyle {
    /// 1, 2, 3…
    #[default]
    Numbers,
    /// A, B, C… (I and O are skipped, as they read like 1 and 0), then AA, AB…
    Letters,
}

/// Letters used for grid labels: I and O are left out.
const LETTERS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ";

impl LabelStyle {
    pub const ALL: [LabelStyle; 2] = [LabelStyle::Numbers, LabelStyle::Letters];

    pub fn label(self) -> &'static str {
        match self {
            LabelStyle::Numbers => "1, 2, 3…",
            LabelStyle::Letters => "A, B, C…",
        }
    }

    /// The `n`th label, counting from 1.
    pub fn format(self, n: u32) -> String {
        match self {
            LabelStyle::Numbers => n.to_string(),
            LabelStyle::Letters => {
                // bijective base 24: A..Z, AA, AB…
                let base = LETTERS.len() as u32;
                let mut n = n.max(1);
                let mut out = Vec::new();
                while n > 0 {
                    n -= 1;
                    out.push(LETTERS[(n % base) as usize]);
                    n /= base;
                }
                out.reverse();
                String::from_utf8(out).unwrap_or_default()
            }
        }
    }

    /// Which label `text` is in this style, counting from 1.
    pub fn parse(self, text: &str) -> Option<u32> {
        let text = text.trim();
        match self {
            LabelStyle::Numbers => text.parse::<u32>().ok().filter(|n| *n > 0),
            LabelStyle::Letters => {
                if text.is_empty() {
                    return None;
                }
                let base = LETTERS.len() as u32;
                let mut n: u32 = 0;
                for c in text.bytes() {
                    let d = LETTERS.iter().position(|l| *l == c.to_ascii_uppercase())? as u32;
                    n = n.checked_mul(base)?.checked_add(d + 1)?;
                }
                Some(n)
            }
        }
    }
}

/// Where a grid system's colour comes from (MU67).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GridColour {
    /// The grid colour of the theme and paper: reads on all of them.
    #[default]
    Auto,
    /// The annotation colour, so grids sit with dimensions and tags.
    Annotation,
    /// The system's own colour.
    Custom,
}

impl GridColour {
    pub fn is_auto(&self) -> bool {
        *self == GridColour::Auto
    }
}

/// Which ends of a grid line carry a bubble.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Bubbles {
    Start,
    /// As older files drew them.
    #[default]
    End,
    Both,
    None,
}

impl Bubbles {
    pub const ALL: [Bubbles; 4] = [Bubbles::Start, Bubbles::End, Bubbles::Both, Bubbles::None];

    pub fn is_default(&self) -> bool {
        *self == Bubbles::End
    }

    pub fn label(self) -> &'static str {
        match self {
            Bubbles::Start => "Start",
            Bubbles::End => "End",
            Bubbles::Both => "Both",
            Bubbles::None => "None",
        }
    }

    pub fn at_start(self) -> bool {
        matches!(self, Bubbles::Start | Bubbles::Both)
    }

    pub fn at_end(self) -> bool {
        matches!(self, Bubbles::End | Bubbles::Both)
    }

    /// With the bubble at one end (`end`) turned on or off.
    pub fn with(self, end: bool, on: bool) -> Bubbles {
        let (mut s, mut e) = (self.at_start(), self.at_end());
        if end {
            e = on;
        } else {
            s = on;
        }
        match (s, e) {
            (true, true) => Bubbles::Both,
            (true, false) => Bubbles::Start,
            (false, true) => Bubbles::End,
            (false, false) => Bubbles::None,
        }
    }
}

pub(crate) fn no_elbows(e: &[f64; 2]) -> bool {
    e[0] == 0.0 && e[1] == 0.0
}

pub(crate) fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

/// A grid system's lines: straight in two directions (and diagonals), or
/// rings round its origin and spokes from it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Shape {
    #[default]
    Straight,
    Curved,
}

impl Shape {
    pub const ALL: [Shape; 2] = [Shape::Straight, Shape::Curved];

    pub fn label(self) -> &'static str {
        match self {
            Shape::Straight => "Straight",
            Shape::Curved => "Curved",
        }
    }
}

/// Which way a grid line runs in its system, which sets how it is counted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Direction {
    /// Across the system's east–west: counted west to east, in its
    /// `vertical` style (A, B, C… by default).
    Across,
    /// Along it: counted south to north, in its `horizontal` style.
    Along,
    /// Neither, by more than [`DIAGONAL_DEG`]: counted on its own, its
    /// labels marked (A', B'…), as IFC's W axes.
    Diagonal,
    /// A curved system's ring, counted outwards, in its `horizontal` style.
    Ring,
    /// A curved system's spoke, counted anticlockwise, in its `vertical`
    /// style.
    Spoke,
}

impl Direction {
    pub fn label(self) -> &'static str {
        match self {
            Direction::Across => "Across",
            Direction::Along => "Along",
            Direction::Diagonal => "Diagonal",
            Direction::Ring => "Ring",
            Direction::Spoke => "Spoke",
        }
    }
}

/// How far, in degrees, a line may lean off its system's directions and
/// still be counted with them.
pub const DIAGONAL_DEG: f64 = 20.0;

/// A grid system: its name is the element's name.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GridGroup {
    /// Put before every label of the group: "A-" gives A-1, A-2…
    #[serde(default)]
    pub prefix: String,
    /// How lines running north–south (along y) are counted, west to east.
    #[serde(default = "letters")]
    pub vertical: LabelStyle,
    /// How lines running east–west (along x) are counted, south to north.
    #[serde(default)]
    pub horizontal: LabelStyle,
    /// sRGB, 0..1: the group's lines and bubbles in 3D and in lists.
    #[serde(default = "default_color")]
    pub color: [f32; 3],
    /// Where its colour comes from (MU67): a grid colour that reads on the
    /// theme and paper, the annotation colour, or `color`.
    #[serde(default, skip_serializing_if = "GridColour::is_auto")]
    pub colour_mode: GridColour,
    /// Straight or curved (decision 4).
    #[serde(default, skip_serializing_if = "is_straight")]
    pub shape: Shape,
    /// Its own north, degrees anticlockwise: a turned building counts in
    /// its own directions (decision 4).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub turn: f64,
    /// Where its first lines are laid out from; a curved system's centre.
    #[serde(default, skip_serializing_if = "is_origin")]
    pub origin: DVec2,
    /// How diagonal lines are counted, and the mark after them (decision 3).
    #[serde(default = "letters")]
    pub diagonal: LabelStyle,
    #[serde(default = "prime")]
    pub diagonal_mark: String,
    /// The bubbles a new line of the system gets (decision 2).
    #[serde(default, skip_serializing_if = "Bubbles::is_default")]
    pub bubbles: Bubbles,
    /// The spacing Add lines lays out (decision 5), metres: across lines
    /// one after another, west to east (a curved system: spokes, degrees);
    /// along lines south to north (a curved system: rings, outwards).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub spacing_across: Vec<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub spacing_along: Vec<f64>,
    /// How far laid-out lines run past the last line crossing them.
    #[serde(default = "default_overhang")]
    pub overhang: f64,
}

fn letters() -> LabelStyle {
    LabelStyle::Letters
}

fn prime() -> String {
    "'".into()
}

fn default_overhang() -> f64 {
    1.5
}

fn is_straight(s: &Shape) -> bool {
    *s == Shape::Straight
}

fn is_origin(p: &DVec2) -> bool {
    *p == DVec2::ZERO
}

fn default_color() -> [f32; 3] {
    PALETTE[0]
}

/// Colours given to new groups in turn.
pub const PALETTE: [[f32; 3]; 6] = [
    [0.20, 0.42, 0.78],
    [0.82, 0.36, 0.16],
    [0.16, 0.60, 0.38],
    [0.62, 0.30, 0.70],
    [0.76, 0.60, 0.10],
    [0.15, 0.58, 0.66],
];

/// The colour of ungrouped grid lines.
pub const UNGROUPED_COLOR: [f32; 3] = [0.40, 0.46, 0.58];

impl Default for GridGroup {
    fn default() -> Self {
        Self {
            prefix: String::new(),
            vertical: LabelStyle::Letters,
            horizontal: LabelStyle::Numbers,
            color: default_color(),
            colour_mode: GridColour::Auto,
            shape: Shape::Straight,
            turn: 0.0,
            origin: DVec2::ZERO,
            diagonal: LabelStyle::Letters,
            diagonal_mark: prime(),
            bubbles: Bubbles::End,
            spacing_across: vec![],
            spacing_along: vec![],
            overhang: default_overhang(),
        }
    }
}

impl GridGroup {
    /// `p` in the system's own axes: from its origin, turned back by its
    /// turn.
    pub fn local(&self, p: DVec2) -> DVec2 {
        DVec2::from_angle(-self.turn.to_radians()).rotate(p - self.origin)
    }

    /// A point in the system's own axes, back in the plan.
    pub fn world(&self, p: DVec2) -> DVec2 {
        self.origin + DVec2::from_angle(self.turn.to_radians()).rotate(p)
    }

    /// Which way `g` runs in this system.
    pub fn direction(&self, g: &Grid) -> Direction {
        if self.shape == Shape::Curved {
            return if g.bulge != 0.0 {
                Direction::Ring
            } else {
                Direction::Spoke
            };
        }
        let d = self.local(g.end) - self.local(g.start);
        let deg = d.y.abs().atan2(d.x.abs()).to_degrees();
        if deg >= 90.0 - DIAGONAL_DEG {
            Direction::Across
        } else if deg <= DIAGONAL_DEG {
            Direction::Along
        } else {
            Direction::Diagonal
        }
    }

    /// The style and mark lines running `dir` are labelled in.
    pub fn style(&self, dir: Direction) -> (LabelStyle, &str) {
        match dir {
            Direction::Across | Direction::Spoke => (self.vertical, ""),
            Direction::Along | Direction::Ring => (self.horizontal, ""),
            Direction::Diagonal => (self.diagonal, self.diagonal_mark.as_str()),
        }
    }

    /// Where `g` comes in the count of its direction: west to east, south
    /// to north, outwards, anticlockwise.
    pub fn order(&self, g: &Grid, dir: Direction) -> f64 {
        let m = self.local((g.start + g.end) / 2.0);
        match dir {
            Direction::Across | Direction::Diagonal => m.x,
            Direction::Along => m.y,
            Direction::Ring => g.start.distance(self.origin),
            Direction::Spoke => {
                // the far end's bearing from the centre
                let far = if g.start.distance(self.origin) > g.end.distance(self.origin) {
                    g.start
                } else {
                    g.end
                };
                self.local(far).to_angle().rem_euclid(std::f64::consts::TAU)
            }
        }
    }

    /// The style lines from `start` to `end` are counted in (straight).
    pub fn style_for(&self, start: DVec2, end: DVec2) -> LabelStyle {
        let g = Grid {
            start,
            end,
            ..Grid::default()
        };
        self.style(self.direction(&g)).0
    }
}

/// `n` in `style`, with `mark` after it.
fn format_marked(style: LabelStyle, mark: &str, n: u32) -> String {
    format!("{}{mark}", style.format(n))
}

/// Which label `text` is, in `style` with `mark` after it.
fn parse_marked(style: LabelStyle, mark: &str, text: &str) -> Option<u32> {
    let t = text.trim();
    let t = if mark.is_empty() {
        t
    } else {
        t.strip_suffix(mark)?
    };
    style.parse(t)
}

/// A grid line's shape: a line, or an arc when it bulges.
pub fn curve(g: &Grid) -> crate::sketch::Curve {
    crate::sketch::Curve::arc(g.start, g.end, g.bulge)
}

/// The line as a polyline, arcs in short pieces.
pub fn points(g: &Grid) -> Vec<DVec2> {
    curve(g).points()
}

/// Which way the line leaves at its start (`end` false) or its end,
/// pointing out of it.
pub fn outward(g: &Grid, end: bool) -> DVec2 {
    let c = curve(g);
    let (p, q) = if end {
        (c.at(1.0), c.at(0.999))
    } else {
        (c.at(0.0), c.at(0.001))
    };
    (p - q).normalize_or_zero()
}

/// How far out a bubble's centre sits past its line's end, metres, at
/// 1:1 (drawings scale it with the bubble).
pub const BUBBLE_OUT: f64 = 0.6;

/// The bubble at one end of `g`: its centre and the leader from the line's
/// end to it, which jogs aside on an elbow.
pub fn bubble(g: &Grid, end: bool, out: f64) -> (DVec2, Vec<DVec2>) {
    let c = curve(g);
    let p = c.at(if end { 1.0 } else { 0.0 });
    let t = outward(g, end);
    let side = g.elbows[usize::from(end)];
    if side == 0.0 {
        return (p + t * out, vec![p]);
    }
    let n = t.perp();
    let knee = p + t * out * 0.4;
    let jog = knee + n * side;
    (jog + t * out * 0.6, vec![p, knee, jog])
}

/// `g` with `trim` metres taken off its start and its end (a plan's own
/// ends, decision 6); unchanged when that would leave nothing.
pub fn trimmed(g: &Grid, trim: [f64; 2]) -> Grid {
    if trim == [0.0, 0.0] {
        return g.clone();
    }
    let c = curve(g);
    let len = match c {
        crate::sketch::Curve::Line(a, b) => a.distance(b),
        crate::sketch::Curve::Arc { radius, sweep, .. } => radius * sweep.abs(),
    };
    if len <= 1e-9 || trim[0] + trim[1] >= len - 0.01 {
        return g.clone();
    }
    let (t0, t1) = (trim[0] / len, 1.0 - trim[1] / len);
    let mut out = g.clone();
    out.start = c.at(t0);
    out.end = c.at(t1);
    // the same arc, shorter: a quarter of its smaller sweep
    if let crate::sketch::Curve::Arc { sweep, .. } = c {
        out.bulge = (sweep * (t1 - t0) / 4.0).tan();
    }
    out
}

/// Does `g` show on `level`?
pub fn on_level(g: &Grid, level: ElementId) -> bool {
    g.levels.is_empty() || g.levels.contains(&level)
}

/// Does a line run more along y than along x?
pub fn runs_north_south(start: DVec2, end: DVec2) -> bool {
    let d = end - start;
    d.y.abs() >= d.x.abs()
}

/// Every grid group in the model, in model order.
pub fn groups(model: &Model) -> Vec<(&Element, &GridGroup)> {
    model
        .iter()
        .filter_map(|e| match &e.kind {
            ElementKind::GridGroup(g) => Some((e, g)),
            _ => None,
        })
        .collect()
}

/// The group a grid line belongs to, if it (still) exists.
pub fn group_of<'a>(model: &'a Model, grid: &Grid) -> Option<(&'a Element, &'a GridGroup)> {
    let e = model.get(grid.group?)?;
    match &e.kind {
        ElementKind::GridGroup(g) => Some((e, g)),
        _ => None,
    }
}

/// The scheme `g` is counted in: its group's, or the default.
fn scheme_of(model: &Model, group: Option<ElementId>) -> GridGroup {
    group
        .and_then(|g| match &model.get(g)?.kind {
            ElementKind::GridGroup(gg) => Some(gg.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

/// Which way `g` runs in its system.
pub fn direction_of(model: &Model, g: &Grid) -> Direction {
    scheme_of(model, g.group).direction(g)
}

/// A grid line's label as drawn: its group's prefix, then its own label.
pub fn display_label(model: &Model, grid: &Grid) -> String {
    match group_of(model, grid) {
        Some((_, g)) => format!("{}{}", g.prefix, grid.label),
        None => grid.label.clone(),
    }
}

/// The colour a grid line is drawn in: its group's, or the ungrouped one.
pub fn color_of(model: &Model, grid: &Grid) -> [f32; 3] {
    group_of(model, grid).map_or(UNGROUPED_COLOR, |(_, g)| g.color)
}

/// The group, if it is one.
fn real_group(model: &Model, group: Option<ElementId>) -> Option<ElementId> {
    group.filter(|g| {
        matches!(
            model.get(*g).map(|e| &e.kind),
            Some(ElementKind::GridGroup(_))
        )
    })
}

/// The label a new line from `start` to `end` gets in `group` (`None` for
/// ungrouped lines): one past the highest label of the lines of that group
/// running the same way. Without a prefix, as stored.
pub fn next_label(model: &Model, group: Option<ElementId>, start: DVec2, end: DVec2) -> String {
    next_label_for(
        model,
        &Grid {
            start,
            end,
            group,
            ..Grid::default()
        },
    )
}

/// [`next_label`] for a line as it is (an arc included).
pub fn next_label_for(model: &Model, g: &Grid) -> String {
    let group = real_group(model, g.group);
    let scheme = scheme_of(model, group);
    let dir = scheme.direction(g);
    let (style, mark) = scheme.style(dir);
    let highest = model
        .iter()
        .filter_map(|e| match &e.kind {
            ElementKind::Grid(o) if o.group == group => Some(o),
            _ => None,
        })
        .filter(|o| scheme.direction(o) == dir)
        .filter_map(|o| parse_marked(style, mark, &o.label))
        .max()
        .unwrap_or(0);
    format_marked(style, mark, highest + 1)
}

/// A new grid line in `group`, labelled to follow on from its group, with
/// the bubbles its group gives new lines.
pub fn new_grid(model: &Model, group: Option<ElementId>, start: DVec2, end: DVec2) -> Element {
    new_line(
        model,
        Grid {
            start,
            end,
            group,
            ..Grid::default()
        },
    )
}

/// A new grid line shaped as `g` (an arc, its levels): its group checked,
/// its label and bubbles given by its group.
pub fn new_line(model: &Model, mut g: Grid) -> Element {
    g.group = real_group(model, g.group);
    g.bubbles = scheme_of(model, g.group).bubbles;
    g.label = next_label_for(model, &g);
    let name = format!("Grid {}", display_label(model, &g));
    Element::new(name, ElementKind::Grid(g))
}

/// A new, empty grid group with a name, prefix and colour no other group has.
pub fn new_group(model: &Model) -> Element {
    let existing = groups(model);
    let n = existing.len();
    let taken = |prefix: &str| existing.iter().any(|(_, g)| g.prefix == prefix);
    let mut k = n as u32 + 1;
    while taken(&format!("{}-", LabelStyle::Letters.format(k))) {
        k += 1;
    }
    let letter = LabelStyle::Letters.format(k);
    Element::new(
        format!("Building {letter} grid"),
        ElementKind::GridGroup(GridGroup {
            prefix: format!("{letter}-"),
            color: PALETTE[n % PALETTE.len()],
            ..GridGroup::default()
        }),
    )
}

/// Relabel the lines of `group` (`None`: the ungrouped ones) in order,
/// each direction on its own: across lines west to east, along lines
/// south to north, diagonals west to east, rings outwards, spokes
/// anticlockwise, all in the system's own axes. Returns one undo step,
/// empty when nothing changes.
pub fn renumber(model: &Model, group: Option<ElementId>) -> Transaction {
    let scheme = scheme_of(model, group);
    let mut tx = Transaction::new("Renumber grid lines");
    for dir in [
        Direction::Across,
        Direction::Along,
        Direction::Diagonal,
        Direction::Ring,
        Direction::Spoke,
    ] {
        let mut lines: Vec<(&Element, &Grid)> = model
            .iter()
            .filter_map(|e| match &e.kind {
                ElementKind::Grid(g) if g.group == group => Some((e, g)),
                _ => None,
            })
            .filter(|(_, g)| scheme.direction(g) == dir)
            .collect();
        lines.sort_by(|a, b| scheme.order(a.1, dir).total_cmp(&scheme.order(b.1, dir)));
        let (style, mark) = scheme.style(dir);
        for (i, (e, g)) in lines.into_iter().enumerate() {
            let label = format_marked(style, mark, i as u32 + 1);
            if g.label != label {
                let mut after = e.clone();
                if let ElementKind::Grid(ag) = &mut after.kind {
                    ag.label = label;
                    after.name = format!("Grid {}", display_label(model, ag));
                }
                tx = tx.replace(e.clone(), after);
            }
        }
    }
    tx
}

/// Running sums of `steps`, from 0: where laid-out lines go.
fn offsets(steps: &[f64]) -> Vec<f64> {
    let mut at = 0.0;
    let mut out = vec![0.0];
    for s in steps {
        at += s;
        out.push(at);
    }
    out
}

/// The lines Add lines lays out for `group` from its spacing (decision 5):
/// a straight system's across lines at its across spacing and along lines
/// at its along spacing, each running past the last line crossing it by
/// the overhang; a curved system's rings at its along spacing (outwards
/// from the first, which is one step out) and spokes at its across
/// spacing (degrees, anticlockwise from east), each spanning the others.
/// Lines already there (same ends, within a millimetre) are not made
/// again. One undo step; empty when there is nothing to add.
/// `s` is the system as it is now (being edited, it may differ from the
/// model's).
pub fn lay_out(model: &Model, group: ElementId, s: &GridGroup) -> Transaction {
    let mut tx = Transaction::new("Add grid lines");
    let mut lines: Vec<Grid> = vec![];
    let line = |a: DVec2, b: DVec2, bulge: f64| Grid {
        start: a,
        end: b,
        bulge,
        group: Some(group),
        ..Grid::default()
    };
    match s.shape {
        Shape::Straight => {
            let xs = offsets(&s.spacing_across);
            let ys = offsets(&s.spacing_along);
            let (x0, x1) = (xs[0] - s.overhang, xs[xs.len() - 1] + s.overhang);
            let (y0, y1) = (ys[0] - s.overhang, ys[ys.len() - 1] + s.overhang);
            if !s.spacing_across.is_empty() {
                for x in &xs {
                    lines.push(line(
                        s.world(DVec2::new(*x, y0)),
                        s.world(DVec2::new(*x, y1)),
                        0.0,
                    ));
                }
            }
            if !s.spacing_along.is_empty() {
                for y in &ys {
                    lines.push(line(
                        s.world(DVec2::new(x0, *y)),
                        s.world(DVec2::new(x1, *y)),
                        0.0,
                    ));
                }
            }
        }
        Shape::Curved => {
            // rings: the first one step out from the centre
            let rs: Vec<f64> = offsets(&s.spacing_along).into_iter().skip(1).collect();
            let angles: Vec<f64> = if s.spacing_across.is_empty() {
                vec![0.0, 90.0]
            } else {
                offsets(&s.spacing_across)
            };
            let (a0, a1) = (angles[0], angles[angles.len() - 1]);
            let (r_in, r_out) = match (rs.first(), rs.last()) {
                (Some(i), Some(o)) => ((i - s.overhang).max(0.0), o + s.overhang),
                _ => (0.0, 10.0),
            };
            let sweep = (a1 - a0).to_radians();
            for r in &rs {
                let p = |deg: f64| s.world(DVec2::from_angle(deg.to_radians()) * *r);
                lines.push(line(
                    p(a0 - 5.0),
                    p(a1 + 5.0),
                    ((sweep + 10f64.to_radians()) / 4.0).tan(),
                ));
            }
            if !s.spacing_across.is_empty() {
                for a in &angles {
                    let d = DVec2::from_angle(a.to_radians());
                    lines.push(line(s.world(d * r_in), s.world(d * r_out), 0.0));
                }
            }
        }
    }
    let mut m = model.clone();
    for g in lines {
        let exists = m.iter().any(|e| match &e.kind {
            ElementKind::Grid(o) => {
                o.group == g.group
                    && ((o.start.distance(g.start) < 1e-3 && o.end.distance(g.end) < 1e-3)
                        || (o.start.distance(g.end) < 1e-3 && o.end.distance(g.start) < 1e-3))
            }
            _ => false,
        });
        if exists {
            continue;
        }
        let e = new_line(&m, g);
        // each line counts on from the ones just added
        if m.insert(e.clone()).is_ok() {
            tx = tx.insert(e);
        }
    }
    tx
}

/// A spacing list as typed ("4000, 3400, 5000" or "3 x 4000"), each value
/// read by `read` (the project's lengths, or degrees); None if a part
/// doesn't read.
pub fn parse_spacing(text: &str, read: impl Fn(&str) -> Option<f64>) -> Option<Vec<f64>> {
    let mut out = vec![];
    for part in text.split([',', ';']) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        // "3 x 4000": a count, then the step
        let (n, v) = match part.split_once(['x', 'X', '\u{d7}', '*']) {
            Some((n, v)) if n.trim().parse::<u32>().is_ok() => {
                (n.trim().parse::<u32>().ok()?, v.trim())
            }
            _ => (1, part),
        };
        let v = read(v)?;
        if v <= 0.0 || n == 0 || n > 200 {
            return None;
        }
        for _ in 0..n {
            out.push(v);
        }
    }
    Some(out)
}

/// Ops that take the grid lines of the groups in `removed` out of their
/// group, for a transaction that deletes those groups: the lines stay.
pub fn release_lines(model: &Model, removed: &[ElementId]) -> Vec<crate::Op> {
    model
        .iter()
        .filter(|e| !removed.contains(&e.id))
        .filter_map(|e| match &e.kind {
            ElementKind::Grid(g) if g.group.is_some_and(|gr| removed.contains(&gr)) => {
                let mut after = e.clone();
                if let ElementKind::Grid(ag) = &mut after.kind {
                    // keep the label as it read, prefix included
                    ag.label = display_label(model, g);
                    ag.group = None;
                }
                Some(crate::Op::Replace {
                    before: e.clone(),
                    after,
                })
            }
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(model: &mut Model, group: Option<ElementId>, a: DVec2, b: DVec2) -> Element {
        let e = new_grid(model, group, a, b);
        model.insert(e.clone()).unwrap();
        e
    }

    fn label(model: &Model, e: &Element) -> String {
        match &model.get(e.id).unwrap().kind {
            ElementKind::Grid(g) => display_label(model, g),
            _ => unreachable!(),
        }
    }

    #[test]
    fn letters_skip_i_and_o_and_roll_over() {
        let l = LabelStyle::Letters;
        assert_eq!(l.format(1), "A");
        assert_eq!(l.format(8), "H");
        assert_eq!(l.format(9), "J");
        assert_eq!(l.format(24), "Z");
        assert_eq!(l.format(25), "AA");
        for n in 1..2000 {
            assert_eq!(l.parse(&l.format(n)), Some(n));
            assert_eq!(
                LabelStyle::Numbers.parse(&LabelStyle::Numbers.format(n)),
                Some(n)
            );
        }
        assert_eq!(l.parse("I"), None);
        assert_eq!(LabelStyle::Numbers.parse("A"), None);
    }

    #[test]
    fn labels_continue_within_each_group_and_direction() {
        let mut m = Model::new();
        let a = new_group(&m);
        m.insert(a.clone()).unwrap();
        let b = new_group(&m);
        m.insert(b.clone()).unwrap();
        // the second group gets its own prefix and colour
        let (ElementKind::GridGroup(ga), ElementKind::GridGroup(gb)) = (&a.kind, &b.kind) else {
            unreachable!()
        };
        assert_eq!(ga.prefix, "A-");
        assert_eq!(gb.prefix, "B-");
        assert_ne!(ga.color, gb.color);

        let ns = |x: f64| (DVec2::new(x, -1.0), DVec2::new(x, 10.0));
        let ew = |y: f64| (DVec2::new(-1.0, y), DVec2::new(10.0, y));
        let a1 = line(&mut m, Some(a.id), ns(0.0).0, ns(0.0).1);
        let a2 = line(&mut m, Some(a.id), ns(5.0).0, ns(5.0).1);
        let a3 = line(&mut m, Some(a.id), ew(0.0).0, ew(0.0).1);
        let b1 = line(&mut m, Some(b.id), ns(30.0).0, ns(30.0).1);
        let u1 = line(&mut m, None, ew(40.0).0, ew(40.0).1);
        assert_eq!(label(&m, &a1), "A-A");
        assert_eq!(label(&m, &a2), "A-B");
        assert_eq!(label(&m, &a3), "A-1");
        assert_eq!(label(&m, &b1), "B-A");
        assert_eq!(label(&m, &u1), "1");

        // a new prefix relabels the whole group
        let mut renamed = m.get(a.id).unwrap().clone();
        if let ElementKind::GridGroup(g) = &mut renamed.kind {
            g.prefix = "North ".into();
        }
        m.replace(renamed).unwrap();
        assert_eq!(label(&m, &a2), "North B");
        assert_eq!(label(&m, &b1), "B-A");
    }

    #[test]
    fn renumber_orders_lines_and_deleting_a_group_keeps_its_lines() {
        let mut m = Model::new();
        let g = new_group(&m);
        m.insert(g.clone()).unwrap();
        let far = line(
            &mut m,
            Some(g.id),
            DVec2::new(9.0, 0.0),
            DVec2::new(9.0, 5.0),
        );
        let near = line(
            &mut m,
            Some(g.id),
            DVec2::new(1.0, 0.0),
            DVec2::new(1.0, 5.0),
        );
        assert_eq!(label(&m, &far), "A-A");
        renumber(&m, Some(g.id)).apply(&mut m).unwrap();
        assert_eq!(label(&m, &near), "A-A");
        assert_eq!(label(&m, &far), "A-B");

        let tx = Transaction::new("Delete").remove_all_cascade(&m, &[g.id]);
        tx.apply(&mut m).unwrap();
        assert!(m.get(g.id).is_none());
        match &m.get(far.id).unwrap().kind {
            ElementKind::Grid(gr) => {
                assert_eq!(gr.group, None);
                assert_eq!(gr.label, "A-B");
            }
            _ => unreachable!(),
        }
    }

    fn grid_of(model: &Model, e: &Element) -> Grid {
        match &model.get(e.id).unwrap().kind {
            ElementKind::Grid(g) => g.clone(),
            _ => unreachable!(),
        }
    }

    fn set_group(model: &mut Model, id: ElementId, f: impl FnOnce(&mut GridGroup)) {
        let mut e = model.get(id).unwrap().clone();
        if let ElementKind::GridGroup(g) = &mut e.kind {
            f(g);
        }
        model.replace(e).unwrap();
    }

    #[test]
    fn a_turned_system_counts_in_its_own_axes_and_diagonals_on_their_own() {
        let mut m = Model::new();
        let s = new_group(&m);
        m.insert(s.clone()).unwrap();
        // the system turned 30°: its "across" lines lean 30° off north
        set_group(&mut m, s.id, |g| g.turn = 30.0);
        let ElementKind::GridGroup(gg) = m.get(s.id).unwrap().kind.clone() else {
            unreachable!()
        };
        let up = |x: f64| (gg.world(DVec2::new(x, -5.0)), gg.world(DVec2::new(x, 5.0)));
        let a = line(&mut m, Some(s.id), up(0.0).0, up(0.0).1);
        let b = line(&mut m, Some(s.id), up(4.0).0, up(4.0).1);
        let one = line(
            &mut m,
            Some(s.id),
            gg.world(DVec2::new(-5.0, 0.0)),
            gg.world(DVec2::new(9.0, 0.0)),
        );
        // 45° off the system's axes: a diagonal, counted on its own
        let d1 = line(
            &mut m,
            Some(s.id),
            gg.world(DVec2::new(0.0, 0.0)),
            gg.world(DVec2::new(6.0, 6.0)),
        );
        assert_eq!(label(&m, &a), "A-A");
        assert_eq!(label(&m, &b), "A-B");
        assert_eq!(label(&m, &one), "A-1");
        assert_eq!(label(&m, &d1), "A-A'");
        assert_eq!(direction_of(&m, &grid_of(&m, &d1)), Direction::Diagonal);
        // a second diagonal follows on; renumbering leaves A, B, 1 alone
        let d2 = line(
            &mut m,
            Some(s.id),
            gg.world(DVec2::new(3.0, 0.0)),
            gg.world(DVec2::new(9.0, 6.0)),
        );
        assert_eq!(label(&m, &d2), "A-B'");
        let tx = renumber(&m, Some(s.id));
        assert!(tx.ops.is_empty(), "already in order");
    }

    #[test]
    fn add_lines_lays_out_the_spacing_once_straight_and_curved() {
        let mut m = Model::new();
        let s = new_group(&m);
        m.insert(s.clone()).unwrap();
        set_group(&mut m, s.id, |g| {
            g.spacing_across = vec![4.0, 3.4];
            g.spacing_along = vec![5.0];
            g.bubbles = Bubbles::Both;
        });
        let ElementKind::GridGroup(gg) = m.get(s.id).unwrap().kind.clone() else {
            unreachable!()
        };
        let tx = lay_out(&m, s.id, &gg);
        assert_eq!(tx.ops.len(), 5, "three across, two along");
        tx.apply(&mut m).unwrap();
        let lines: Vec<Grid> = m
            .iter()
            .filter_map(|e| match &e.kind {
                ElementKind::Grid(g) => Some(g.clone()),
                _ => None,
            })
            .collect();
        let mut labels: Vec<String> = lines.iter().map(|g| g.label.clone()).collect();
        labels.sort();
        assert_eq!(labels, ["1", "2", "A", "B", "C"]);
        assert!(lines.iter().all(|g| g.bubbles == Bubbles::Both));
        // the across lines run past the along lines by the overhang
        let c = lines.iter().find(|g| g.label == "C").unwrap();
        assert!((c.start.x - 7.4).abs() < 1e-9 && (c.start.y + 1.5).abs() < 1e-9);
        // again: nothing new
        assert!(lay_out(&m, s.id, &gg).ops.is_empty());

        // curved: rings round the origin, spokes from it
        let r = new_group(&m);
        m.insert(r.clone()).unwrap();
        set_group(&mut m, r.id, |g| {
            g.shape = Shape::Curved;
            g.origin = DVec2::new(100.0, 0.0);
            g.spacing_along = vec![3.0, 3.0];
            g.spacing_across = vec![30.0, 30.0];
        });
        let ElementKind::GridGroup(rg) = m.get(r.id).unwrap().kind.clone() else {
            unreachable!()
        };
        let tx = lay_out(&m, r.id, &rg);
        tx.apply(&mut m).unwrap();
        let mine: Vec<Grid> = m
            .iter()
            .filter_map(|e| match &e.kind {
                ElementKind::Grid(g) if g.group == Some(r.id) => Some(g.clone()),
                _ => None,
            })
            .collect();
        let rings: Vec<&Grid> = mine.iter().filter(|g| g.bulge != 0.0).collect();
        let spokes: Vec<&Grid> = mine.iter().filter(|g| g.bulge == 0.0).collect();
        assert_eq!((rings.len(), spokes.len()), (2, 3));
        for g in &rings {
            // an arc round the centre: both ends and its middle as far out
            let mid = curve(g).at(0.5);
            let rr = g.start.distance(rg.origin);
            assert!((g.end.distance(rg.origin) - rr).abs() < 1e-6);
            assert!((mid.distance(rg.origin) - rr).abs() < 1e-6, "{rr}");
        }
        let mut ring_labels: Vec<String> = rings.iter().map(|g| g.label.clone()).collect();
        ring_labels.sort();
        assert_eq!(ring_labels, ["1", "2"], "rings counted outwards");
    }

    #[test]
    fn bubbles_elbows_trims_and_spacing_text() {
        let g = Grid {
            start: DVec2::ZERO,
            end: DVec2::new(10.0, 0.0),
            bubbles: Bubbles::End,
            elbows: [0.0, 1.0],
            ..Grid::default()
        };
        assert_eq!(g.bubbles.with(false, true), Bubbles::Both);
        assert_eq!(Bubbles::Both.with(true, false), Bubbles::Start);
        assert_eq!(Bubbles::Start.with(false, false), Bubbles::None);
        // the end bubble jogs 1 m to the left of the way out
        let (at, leader) = bubble(&g, true, 0.6);
        assert_eq!(leader.len(), 3);
        assert!((at.y - 1.0).abs() < 1e-9 && at.x > 10.0);
        // no elbow at the start: straight out
        let (s, _) = bubble(&g, false, 0.6);
        assert!((s - DVec2::new(-0.6, 0.0)).length() < 1e-9);
        // trimmed in one plan
        let t = trimmed(&g, [2.0, 1.0]);
        assert!((t.start.x - 2.0).abs() < 1e-9 && (t.end.x - 9.0).abs() < 1e-9);
        assert_eq!(trimmed(&g, [6.0, 6.0]).start, g.start, "never to nothing");
        // an arc trimmed stays on its circle
        let arc = Grid {
            bulge: 0.4,
            ..g.clone()
        };
        let (c, r) = match curve(&arc) {
            crate::sketch::Curve::Arc { center, radius, .. } => (center, radius),
            _ => unreachable!(),
        };
        let ta = trimmed(&arc, [1.0, 1.0]);
        for p in [ta.start, ta.end, curve(&ta).at(0.5)] {
            assert!((p.distance(c) - r).abs() < 1e-6);
        }
        // levels
        let lv = ElementId::new();
        assert!(on_level(&g, lv));
        let only = Grid {
            levels: vec![ElementId::new()],
            ..g.clone()
        };
        assert!(!on_level(&only, lv));
        // typed spacing
        let mm = |s: &str| s.trim().parse::<f64>().ok().map(|v| v / 1000.0);
        assert_eq!(
            parse_spacing("4000, 3400,5000", mm),
            Some(vec![4.0, 3.4, 5.0])
        );
        assert_eq!(parse_spacing("3 x 4000", mm), Some(vec![4.0; 3]));
        assert_eq!(parse_spacing("4000, abc", mm), None);
        assert_eq!(parse_spacing("", mm), Some(vec![]));
    }

    #[test]
    fn grids_and_views_from_older_files_still_read() {
        let old = r#"{"label":"A","start":[0.0,0.0],"end":[0.0,5.0]}"#;
        let g: Grid = serde_json::from_str(old).unwrap();
        assert_eq!(g.group, None);
        // an ungrouped line writes no group
        assert!(!serde_json::to_string(&g).unwrap().contains("group"));
        let group: GridGroup = serde_json::from_str("{}").unwrap();
        assert_eq!(group, GridGroup::default());
    }
}
