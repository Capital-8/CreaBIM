//! Parametric patterns: a recipe (a generator and its numbers, or a tile
//! you drew) that lays out units (bricks, boards, tiles, drawn shapes) and
//! lines in a plane. One recipe serves every use of a pattern: the hatch
//! of a cut, the lines on a surface, and later the 3D relief, modelled
//! units and textures. The file keeps the recipe, never its lines.
//!
//! Sizes are in the pattern's own units ([`crate::styles::PatternUnits`]):
//! metres for a true-size pattern, millimetres of paper for a symbolic one.
//! The plane is the pattern's own: x along its courses, y up across them.

use glam::DVec2;
use serde::{Deserialize, Serialize};

/// How a pattern is made.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Recipe {
    /// Rectangular units in courses: brick and block bonds.
    Bond {
        #[serde(default)]
        bond: Bond,
        length: f64,
        height: f64,
        joint: f64,
    },
    /// Units `length` × `width` laid at right angles in a zigzag, its
    /// rows along x. `length` is rounded to a whole number of widths
    /// (with their joints) so the zigzag closes.
    Herringbone { length: f64, width: f64, joint: f64 },
    /// Pairs of units side by side, each pair turned against its
    /// neighbours: basketweave paving and parquet.
    Basketweave { length: f64, width: f64, joint: f64 },
    /// Boards `width` wide in rows, of random lengths from
    /// `min_length` to `length`, ends staggered.
    Boards {
        length: f64,
        min_length: f64,
        width: f64,
        joint: f64,
        #[serde(default)]
        seed: u32,
    },
    /// A grid of tiles; each row slides along by `offset` of a tile
    /// (0 is a grid, 0.5 a half bond).
    Tiles {
        width: f64,
        height: f64,
        joint: f64,
        #[serde(default)]
        offset: f64,
    },
    /// Hexagonal tiles `size` across their flats.
    Hexagons { size: f64, joint: f64 },
    /// Random-sized stones in courses: lengths about `length` (from half
    /// to one and a half of it), courses of one, one and a half or two
    /// times `height`.
    Ashlar {
        length: f64,
        height: f64,
        joint: f64,
        #[serde(default)]
        seed: u32,
    },
    /// A tile you drew, repeated.
    Drawn(DrawnTile),
}

/// The bond of a [`Recipe::Bond`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Bond {
    /// Every course slides half a unit: the usual brick bond.
    #[default]
    Stretcher,
    /// Units straight above each other.
    Stack,
    /// Stretchers and headers alternate along each course, each header
    /// centred on the stretcher below.
    Flemish,
    /// A course of stretchers, then a course of headers.
    English,
    /// Every course slides a third of a unit.
    Third,
}

impl Bond {
    pub const ALL: [Bond; 5] = [
        Bond::Stretcher,
        Bond::Stack,
        Bond::Flemish,
        Bond::English,
        Bond::Third,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Bond::Stretcher => "Stretcher (half)",
            Bond::Stack => "Stack",
            Bond::Flemish => "Flemish",
            Bond::English => "English",
            Bond::Third => "Third",
        }
    }
}

/// A tile you drew: its size, how it repeats, and its strokes.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DrawnTile {
    /// Width and height of one tile.
    pub size: [f64; 2],
    #[serde(default)]
    pub repeat: Repeat,
    #[serde(default)]
    pub strokes: Vec<DrawnStroke>,
    /// Dimensions kept with the tile while working on it: each from one
    /// point to another. Never drawn in plans, sections or elevations.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dims: Vec<[[f64; 2]; 2]>,
}

/// How a drawn tile repeats.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Repeat {
    /// Side by side in rows and columns.
    #[default]
    Grid,
    /// Every other row slides half a tile, like a bond.
    Offset,
    /// Every other tile along a row is mirrored.
    Mirror,
}

impl Repeat {
    pub const ALL: [Repeat; 3] = [Repeat::Grid, Repeat::Offset, Repeat::Mirror];

    pub fn label(self) -> &'static str {
        match self {
            Repeat::Grid => "Grid",
            Repeat::Offset => "Offset half a tile",
            Repeat::Mirror => "Mirror every other",
        }
    }
}

/// One stroke of a drawn tile: an open line or a closed shape, with the
/// job it does.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DrawnStroke {
    /// Its points, from the tile's lower left corner, in the pattern's
    /// units.
    pub points: Vec<[f64; 2]>,
    /// A closed shape (its last point joins its first).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub closed: bool,
    #[serde(default)]
    pub job: Job,
    /// How deep a groove, recess or hole goes, or how far a ridge or
    /// raised area stands out (metres); 0 takes the job's usual depth.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub depth: f64,
    /// A name to tell it apart in the list ("leaf", "wave").
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

/// What a stroke of a drawn pattern does. Every job draws as a line in
/// plans, sections and elevations; the jobs differ in 3D and when the
/// pattern is modelled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Job {
    /// A line on the face, nothing modelled.
    #[default]
    Line,
    /// Open lines: a channel cut along the line.
    Groove,
    /// Open lines: a bead standing out along the line.
    Ridge,
    /// Closed shapes: a piece of its own with a joint round it, like a
    /// tile. Counted.
    Unit,
    /// Closed shapes: the area pushed out of the face.
    Raised,
    /// Closed shapes: the area sunk into the face.
    Recessed,
    /// Closed shapes: cut right through. Drawn filled.
    Hole,
}

impl Job {
    /// The jobs an open line can do.
    pub const OPEN: [Job; 3] = [Job::Line, Job::Groove, Job::Ridge];
    /// The jobs a closed shape can do.
    pub const CLOSED: [Job; 5] = [Job::Line, Job::Unit, Job::Raised, Job::Recessed, Job::Hole];

    pub fn label(self) -> &'static str {
        match self {
            Job::Line => "Drawn line",
            Job::Groove => "Groove",
            Job::Ridge => "Ridge",
            Job::Unit => "Separate unit",
            Job::Raised => "Raised",
            Job::Recessed => "Recessed",
            Job::Hole => "Hole",
        }
    }

    /// The depth a stroke with this job and no depth of its own takes,
    /// metres.
    pub fn usual_depth(self) -> f64 {
        match self {
            Job::Line => 0.0,
            Job::Groove | Job::Ridge => 0.006,
            Job::Unit => 0.008,
            Job::Raised | Job::Recessed => 0.012,
            Job::Hole => 0.0,
        }
    }
}

/// What a recipe lays out over an area: closed shapes and open lines, each
/// with its job, in the pattern's plane and units.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Layout {
    pub shapes: Vec<(Vec<DVec2>, Job)>,
    pub lines: Vec<(Vec<DVec2>, Job)>,
}

impl Layout {
    fn unit(&mut self, pts: Vec<DVec2>) {
        self.shapes.push((pts, Job::Unit));
    }

    fn rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        self.unit(rect(x, y, w, h));
    }

    /// How many pieces it holds.
    pub fn len(&self) -> usize {
        self.shapes.len() + self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Every edge it draws: shape outlines closed, lines as they run.
    pub fn edges(&self) -> impl Iterator<Item = (DVec2, DVec2)> + '_ {
        let closed = self
            .shapes
            .iter()
            .flat_map(|(p, _)| (0..p.len()).map(move |i| (p[i], p[(i + 1) % p.len()])));
        let open = self
            .lines
            .iter()
            .flat_map(|(p, _)| p.windows(2).map(|w| (w[0], w[1])));
        closed.chain(open)
    }

    /// The units (closed shapes of the Unit job) whose centre lies inside
    /// `lo`..`hi`.
    pub fn count_units(&self, lo: DVec2, hi: DVec2) -> usize {
        self.shapes
            .iter()
            .filter(|(p, j)| *j == Job::Unit && !p.is_empty())
            .filter(|(p, _)| {
                let c = p.iter().copied().sum::<DVec2>() / p.len() as f64;
                c.x >= lo.x && c.x < hi.x && c.y >= lo.y && c.y < hi.y
            })
            .count()
    }
}

fn rect(x: f64, y: f64, w: f64, h: f64) -> Vec<DVec2> {
    vec![
        DVec2::new(x, y),
        DVec2::new(x + w, y),
        DVec2::new(x + w, y + h),
        DVec2::new(x, y + h),
    ]
}

/// A small, fast, repeatable random number source: the same seed always
/// lays the same boards and stones.
struct Rng(u32);

impl Rng {
    fn new(seed: u32, salt: i64) -> Self {
        let mut r = Rng(seed
            .wrapping_mul(0x9E37_79B9)
            .wrapping_add((salt as u32).wrapping_mul(0x85EB_CA6B))
            ^ 0x2545_F491);
        r.next();
        r
    }

    /// 0..1.
    fn next(&mut self) -> f64 {
        // mulberry32
        self.0 = self.0.wrapping_add(0x6D2B_79F5);
        let mut t = self.0;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        f64::from(t ^ (t >> 14)) / 4_294_967_296.0
    }
}

impl Recipe {
    /// A short name for its kind, for lists.
    pub fn label(&self) -> &'static str {
        match self {
            Recipe::Bond { .. } => "Bond",
            Recipe::Herringbone { .. } => "Herringbone",
            Recipe::Basketweave { .. } => "Basketweave",
            Recipe::Boards { .. } => "Boards",
            Recipe::Tiles { .. } => "Tiles",
            Recipe::Hexagons { .. } => "Hexagons",
            Recipe::Ashlar { .. } => "Random stone",
            Recipe::Drawn(_) => "Drawn tile",
        }
    }

    /// Whether every size is a positive, finite number.
    pub fn is_valid(&self) -> bool {
        let pos = |v: f64| v.is_finite() && v > 1e-9;
        let joint = |v: f64| v.is_finite() && v >= 0.0;
        match *self {
            Recipe::Bond {
                length,
                height,
                joint: j,
                ..
            } => pos(length) && pos(height) && joint(j),
            Recipe::Herringbone {
                length,
                width,
                joint: j,
            }
            | Recipe::Basketweave {
                length,
                width,
                joint: j,
            } => pos(length) && pos(width) && joint(j),
            Recipe::Boards {
                length,
                min_length,
                width,
                joint: j,
                ..
            } => pos(length) && pos(min_length) && pos(width) && joint(j),
            Recipe::Tiles {
                width,
                height,
                joint: j,
                offset,
            } => pos(width) && pos(height) && joint(j) && offset.is_finite(),
            Recipe::Hexagons { size, joint: j } => pos(size) && joint(j),
            Recipe::Ashlar {
                length,
                height,
                joint: j,
                ..
            } => pos(length) && pos(height) && joint(j),
            Recipe::Drawn(ref t) => {
                pos(t.size[0])
                    && pos(t.size[1])
                    && t.strokes.iter().all(|s| {
                        s.points
                            .iter()
                            .all(|p| p[0].is_finite() && p[1].is_finite())
                            && s.depth.is_finite()
                    })
            }
        }
    }

    /// About how many pieces it lays per unit of area, to refuse a layout
    /// too fine to draw before making it.
    fn per_area(&self) -> f64 {
        let a = match *self {
            Recipe::Bond {
                length,
                height,
                joint,
                ..
            } => (length + joint) * (height + joint) / 2.0,
            Recipe::Herringbone {
                length,
                width,
                joint,
            }
            | Recipe::Basketweave {
                length,
                width,
                joint,
            } => (length + joint) * (width + joint),
            Recipe::Boards {
                min_length,
                width,
                joint,
                ..
            } => (min_length + joint) * (width + joint),
            Recipe::Tiles {
                width,
                height,
                joint,
                ..
            } => (width + joint) * (height + joint),
            Recipe::Hexagons { size, joint } => (size + joint).powi(2) * 0.8,
            Recipe::Ashlar {
                length,
                height,
                joint,
                ..
            } => (length * 0.5 + joint) * (height + joint),
            Recipe::Drawn(ref t) => t.size[0] * t.size[1] / (t.strokes.len().max(1) as f64),
        };
        1.0 / a.max(1e-12)
    }

    /// The layout over `lo`..`hi` (the pattern's plane), or none when it
    /// would be more than `limit` pieces or the recipe is not valid.
    pub fn layout(&self, lo: DVec2, hi: DVec2, limit: usize) -> Option<Layout> {
        if !self.is_valid() || !(lo.is_finite() && hi.is_finite()) || hi.x < lo.x || hi.y < lo.y {
            return None;
        }
        let area = (hi.x - lo.x + 1e-9) * (hi.y - lo.y + 1e-9);
        if area * self.per_area() > limit as f64 {
            return None;
        }
        let mut out = Layout::default();
        match *self {
            Recipe::Bond {
                bond,
                length,
                height,
                joint,
            } => bond_layout(&mut out, bond, length, height, joint, lo, hi),
            Recipe::Herringbone {
                length,
                width,
                joint,
            } => herringbone(&mut out, length, width, joint, lo, hi),
            Recipe::Basketweave {
                length,
                width,
                joint,
            } => basketweave(&mut out, length, width, joint, lo, hi),
            Recipe::Boards {
                length,
                min_length,
                width,
                joint,
                seed,
            } => {
                let pitch = width + joint;
                let (lmin, lmax) = (min_length.min(length), length.max(min_length));
                for r in (lo.y / pitch).floor() as i64 - 1..=(hi.y / pitch).ceil() as i64 {
                    let y = r as f64 * pitch;
                    random_run(seed, r, lo.x, hi.x, lmin, lmax, joint, |x, l| {
                        out.rect(x, y, l, width)
                    });
                }
            }
            Recipe::Tiles {
                width,
                height,
                joint,
                offset,
            } => {
                let (px, py) = (width + joint, height + joint);
                for r in (lo.y / py).floor() as i64 - 1..=(hi.y / py).ceil() as i64 {
                    let slide = (r as f64 * offset).rem_euclid(1.0) * px;
                    let c0 = ((lo.x - slide) / px).floor() as i64 - 1;
                    let c1 = ((hi.x - slide) / px).ceil() as i64;
                    for c in c0..=c1 {
                        out.rect(slide + c as f64 * px, r as f64 * py, width, height);
                    }
                }
            }
            Recipe::Hexagons { size, joint } => {
                // flat-topped hexagons: across the flats `size`
                let s = size / 3f64.sqrt();
                let (dx, dy) = (1.5 * s + joint * 3f64.sqrt() / 2.0, size + joint);
                for c in (lo.x / dx).floor() as i64 - 1..=(hi.x / dx).ceil() as i64 + 1 {
                    let up = if c.rem_euclid(2) == 1 { dy / 2.0 } else { 0.0 };
                    for r in (lo.y / dy).floor() as i64 - 1..=(hi.y / dy).ceil() as i64 + 1 {
                        let ctr = DVec2::new(c as f64 * dx, r as f64 * dy + up);
                        out.unit(
                            (0..6)
                                .map(|k| {
                                    ctr + DVec2::from_angle(k as f64 * std::f64::consts::FRAC_PI_3)
                                        * s
                                })
                                .collect(),
                        );
                    }
                }
            }
            Recipe::Ashlar {
                length,
                height,
                joint,
                seed,
            } => {
                // courses of random height from a band below `lo`, so the
                // same stones appear wherever the area starts
                let band = 4.0 * (2.0 * height + joint);
                let b0 = (lo.y / band).floor() as i64;
                for b in b0..=(hi.y / band).ceil() as i64 {
                    let mut g = Rng::new(seed, b);
                    let mut y = b as f64 * band;
                    let top = (b + 1) as f64 * band;
                    while y < top - 1e-9 {
                        let mut h = height * [1.0, 1.5, 2.0][(g.next() * 3.0) as usize % 3];
                        if y + h + joint > top - height * 0.5 {
                            h = top - y - joint;
                        }
                        if y + h > lo.y && y < hi.y {
                            let row = (y * 1e4).round() as i64;
                            random_run(
                                seed ^ 0xA5A5,
                                row,
                                lo.x,
                                hi.x,
                                length * 0.5,
                                length * 1.5,
                                joint,
                                |x, l| out.rect(x, y, l, h),
                            );
                        }
                        y += h + joint;
                    }
                }
            }
            Recipe::Drawn(ref t) => drawn(&mut out, t, lo, hi),
        }
        (out.len() <= limit).then_some(out)
    }
}

/// Lay random lengths `lmin`..`lmax` with `joint` between along a row
/// over `x0`..`x1`, the same pieces wherever the stretch starts: the row
/// is cut into blocks, each laid from its own seed, the blocks' ends
/// staggered row by row.
#[allow(clippy::too_many_arguments)]
fn random_run(
    seed: u32,
    row: i64,
    x0: f64,
    x1: f64,
    lmin: f64,
    lmax: f64,
    joint: f64,
    mut emit: impl FnMut(f64, f64),
) {
    let block = 6.0 * (lmax + joint);
    let phase = Rng::new(seed, row.wrapping_mul(7919)).next() * block;
    let b0 = ((x0 - phase) / block).floor() as i64 - 1;
    let b1 = ((x1 - phase) / block).ceil() as i64;
    for b in b0..=b1 {
        let start = phase + b as f64 * block;
        let end = start + block;
        let mut g = Rng::new(seed, row.wrapping_mul(1_000_003).wrapping_add(b));
        let mut x = start;
        let mut pieces: Vec<(f64, f64)> = vec![];
        while x < end - joint - 1e-9 {
            let mut l = lmin + (lmax - lmin) * g.next();
            let left = end - joint - x;
            if left - l < lmin + joint {
                // what is left: one last piece, or two halves if too long
                l = if left <= lmax {
                    left
                } else {
                    (left - joint) / 2.0
                };
            }
            pieces.push((x, l));
            x += l + joint;
        }
        for (x, l) in pieces {
            if x + l > x0 && x < x1 {
                emit(x, l);
            }
        }
    }
}

fn bond_layout(
    out: &mut Layout,
    bond: Bond,
    length: f64,
    height: f64,
    joint: f64,
    lo: DVec2,
    hi: DVec2,
) {
    let (pl, ph) = (length + joint, height + joint);
    let header = ((length - joint) / 2.0).max(1e-6);
    for r in (lo.y / ph).floor() as i64 - 1..=(hi.y / ph).ceil() as i64 {
        let y = r as f64 * ph;
        let odd = r.rem_euclid(2) == 1;
        // (unit lengths along the course, where the course starts)
        let (units, start): (Vec<f64>, f64) = match bond {
            Bond::Stretcher => (vec![length], if odd { pl / 2.0 } else { 0.0 }),
            Bond::Stack => (vec![length], 0.0),
            Bond::Third => (vec![length], (r as f64 / 3.0).fract() * pl),
            Bond::English => {
                if odd {
                    (vec![header], header / 2.0 + joint / 2.0)
                } else {
                    (vec![length], 0.0)
                }
            }
            Bond::Flemish => {
                // stretcher, header, ...; the next course's header sits
                // centred on this course's stretcher
                let s = if odd {
                    length / 2.0 - header / 2.0 - (length + joint)
                } else {
                    0.0
                };
                (vec![length, header], s)
            }
        };
        let period: f64 = units.iter().map(|u| u + joint).sum();
        let mut x = start + ((lo.x - start) / period).floor() * period - period;
        while x < hi.x {
            for u in &units {
                if x + u > lo.x && x < hi.x {
                    out.rect(x, y, *u, height);
                }
                x += u + joint;
            }
        }
    }
}

fn herringbone(out: &mut Layout, length: f64, width: f64, joint: f64, lo: DVec2, hi: DVec2) {
    // a staircase of a unit along x and a unit up y, each step one width
    // (with its joint) on; the staircases repeat 2 lengths apart along x
    let q = width + joint;
    let m = ((length + joint) / q).round().max(2.0);
    let l = m * q - joint;
    let t = 2.0 * m * q;
    let i0 = ((lo.y - l) / q).floor() as i64 - 1;
    let i1 = (hi.y / q).ceil() as i64 + 1;
    for i in i0..=i1 {
        let b = i as f64 * q;
        let k0 = ((lo.x - b - l) / t).floor() as i64 - 1;
        let k1 = ((hi.x - b) / t).ceil() as i64 + 1;
        for k in k0..=k1 {
            let x = b + k as f64 * t;
            out.rect(x, b, l, width);
            out.rect(x, b + q, width, l);
        }
    }
}

fn basketweave(out: &mut Layout, length: f64, width: f64, joint: f64, lo: DVec2, hi: DVec2) {
    // a square cell of length × length holding parallel units; cells
    // alternate lying and standing
    let n = ((length + joint) / (width + joint)).round().max(1.0);
    let cell = length + joint;
    let w = (cell - n * joint) / n;
    for cx in (lo.x / cell).floor() as i64 - 1..=(hi.x / cell).ceil() as i64 {
        for cy in (lo.y / cell).floor() as i64 - 1..=(hi.y / cell).ceil() as i64 {
            let (x0, y0) = (cx as f64 * cell, cy as f64 * cell);
            for k in 0..n as i64 {
                let o = k as f64 * (w + joint);
                if (cx + cy).rem_euclid(2) == 0 {
                    out.rect(x0, y0 + o, length, w);
                } else {
                    out.rect(x0 + o, y0, w, length);
                }
            }
        }
    }
}

fn drawn(out: &mut Layout, t: &DrawnTile, lo: DVec2, hi: DVec2) {
    let [w, h] = t.size;
    for r in (lo.y / h).floor() as i64 - 1..=(hi.y / h).ceil() as i64 {
        let slide = if t.repeat == Repeat::Offset && r.rem_euclid(2) == 1 {
            w / 2.0
        } else {
            0.0
        };
        for c in ((lo.x - slide) / w).floor() as i64 - 1..=((hi.x - slide) / w).ceil() as i64 {
            let o = DVec2::new(slide + c as f64 * w, r as f64 * h);
            let mirror = t.repeat == Repeat::Mirror && c.rem_euclid(2) == 1;
            for s in &t.strokes {
                if s.points.len() < 2 || (s.closed && s.points.len() < 3) {
                    continue;
                }
                let pts: Vec<DVec2> = s
                    .points
                    .iter()
                    .map(|p| o + DVec2::new(if mirror { w - p[0] } else { p[0] }, p[1]))
                    .collect();
                if s.closed {
                    out.shapes.push((pts, s.job));
                } else {
                    out.lines.push((pts, s.job));
                }
            }
        }
    }
}

/// A recipe's ready-made patterns: the Library tab of the Patterns window.
pub struct Preset {
    pub name: &'static str,
    /// The list it is grouped under.
    pub group: &'static str,
    pub recipe: fn() -> Recipe,
}

/// The patterns CreaBIM ships, true size, in metres.
pub const PRESETS: &[Preset] = &[
    Preset {
        name: "Brick, stretcher bond",
        group: "Masonry",
        recipe: || Recipe::Bond {
            bond: Bond::Stretcher,
            length: 0.215,
            height: 0.065,
            joint: 0.01,
        },
    },
    Preset {
        name: "Brick, Flemish bond",
        group: "Masonry",
        recipe: || Recipe::Bond {
            bond: Bond::Flemish,
            length: 0.215,
            height: 0.065,
            joint: 0.01,
        },
    },
    Preset {
        name: "Brick, English bond",
        group: "Masonry",
        recipe: || Recipe::Bond {
            bond: Bond::English,
            length: 0.215,
            height: 0.065,
            joint: 0.01,
        },
    },
    Preset {
        name: "Block, stretcher bond",
        group: "Masonry",
        recipe: || Recipe::Bond {
            bond: Bond::Stretcher,
            length: 0.44,
            height: 0.215,
            joint: 0.01,
        },
    },
    Preset {
        name: "Block, stack bond",
        group: "Masonry",
        recipe: || Recipe::Bond {
            bond: Bond::Stack,
            length: 0.44,
            height: 0.215,
            joint: 0.01,
        },
    },
    Preset {
        name: "Random stone",
        group: "Masonry",
        recipe: || Recipe::Ashlar {
            length: 0.4,
            height: 0.15,
            joint: 0.012,
            seed: 9,
        },
    },
    Preset {
        name: "Tiles 300",
        group: "Tiles",
        recipe: || Recipe::Tiles {
            width: 0.3,
            height: 0.3,
            joint: 0.003,
            offset: 0.0,
        },
    },
    Preset {
        name: "Tiles 600 × 300, half bond",
        group: "Tiles",
        recipe: || Recipe::Tiles {
            width: 0.6,
            height: 0.3,
            joint: 0.003,
            offset: 0.5,
        },
    },
    Preset {
        name: "Hexagon tiles 200",
        group: "Tiles",
        recipe: || Recipe::Hexagons {
            size: 0.2,
            joint: 0.004,
        },
    },
    Preset {
        name: "Herringbone pavers",
        group: "Paving",
        recipe: || Recipe::Herringbone {
            length: 0.2,
            width: 0.1,
            joint: 0.005,
        },
    },
    Preset {
        name: "Basketweave pavers",
        group: "Paving",
        recipe: || Recipe::Basketweave {
            length: 0.2,
            width: 0.1,
            joint: 0.005,
        },
    },
    Preset {
        name: "Setts 100",
        group: "Paving",
        recipe: || Recipe::Tiles {
            width: 0.1,
            height: 0.1,
            joint: 0.01,
            offset: 0.5,
        },
    },
    Preset {
        name: "Boards 145",
        group: "Timber",
        recipe: || Recipe::Boards {
            length: 2.4,
            min_length: 0.9,
            width: 0.145,
            joint: 0.004,
            seed: 11,
        },
    },
    Preset {
        name: "Cladding 150, vertical",
        group: "Timber",
        recipe: || Recipe::Tiles {
            width: 0.15,
            height: 6.0,
            joint: 0.01,
            offset: 0.0,
        },
    },
    Preset {
        name: "Parquet herringbone",
        group: "Timber",
        recipe: || Recipe::Herringbone {
            length: 0.42,
            width: 0.07,
            joint: 0.0,
        },
    },
    Preset {
        name: "Slates 500 × 250",
        group: "Roofing",
        recipe: || Recipe::Tiles {
            width: 0.25,
            height: 0.2,
            joint: 0.003,
            offset: 0.5,
        },
    },
    Preset {
        name: "Leaf and wave",
        group: "Drawn",
        recipe: leaf_and_wave,
    },
    Preset {
        name: "Perforated circles",
        group: "Drawn",
        recipe: perforated,
    },
];

/// A drawn sample: a raised leaf on a diagonal and a grooved wave that
/// runs on from tile to tile.
pub fn leaf_and_wave() -> Recipe {
    let s = 0.3;
    let (a, b) = (DVec2::new(0.18, 0.23) * s, DVec2::new(0.78, 0.68) * s);
    let (d, n) = (b - a, (b - a).perp().normalize());
    let w = 0.15 * s;
    let mut leaf = vec![];
    for i in 0..=12 {
        let t = i as f64 / 12.0;
        let p = a + d * t + n * w * (std::f64::consts::PI * t).sin();
        leaf.push([round(p.x), round(p.y)]);
    }
    for i in (1..12).rev() {
        let t = i as f64 / 12.0;
        let p = a + d * t - n * w * (std::f64::consts::PI * t).sin();
        leaf.push([round(p.x), round(p.y)]);
    }
    let wave = (0..=16)
        .map(|i| {
            let t = i as f64 / 16.0;
            [
                round(t * s),
                round((0.12 + 0.07 * (2.0 * std::f64::consts::PI * t).sin()) * s),
            ]
        })
        .collect();
    Recipe::Drawn(DrawnTile {
        size: [s, s],
        repeat: Repeat::Grid,
        dims: vec![],
        strokes: vec![
            DrawnStroke {
                points: leaf,
                closed: true,
                job: Job::Raised,
                depth: 0.012,
                name: "leaf".into(),
            },
            DrawnStroke {
                points: wave,
                closed: false,
                job: Job::Groove,
                depth: 0.006,
                name: "wave".into(),
            },
        ],
    })
}

/// A drawn sample: round holes in a staggered grid, a perforated screen.
pub fn perforated() -> Recipe {
    let s = 0.1;
    let circle = |c: DVec2, r: f64| -> Vec<[f64; 2]> {
        (0..16)
            .map(|k| {
                let p = c + DVec2::from_angle(k as f64 * std::f64::consts::TAU / 16.0) * r;
                [round(p.x), round(p.y)]
            })
            .collect()
    };
    Recipe::Drawn(DrawnTile {
        size: [s, s],
        repeat: Repeat::Offset,
        dims: vec![],
        strokes: vec![DrawnStroke {
            points: circle(DVec2::splat(s / 2.0), 0.03),
            closed: true,
            job: Job::Hole,
            depth: 0.0,
            name: "hole".into(),
        }],
    })
}

fn round(v: f64) -> f64 {
    (v * 1e5).round() / 1e5
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area() -> (DVec2, DVec2) {
        (DVec2::ZERO, DVec2::new(2.6, 1.5))
    }

    fn brick(bond: Bond) -> Recipe {
        Recipe::Bond {
            bond,
            length: 0.215,
            height: 0.065,
            joint: 0.01,
        }
    }

    #[test]
    fn a_stretcher_bond_lays_its_bricks_a_course_at_a_time() {
        // 10 bricks a course over 2.25 m, 20 courses over 1.5 m
        let (lo, hi) = (DVec2::ZERO, DVec2::new(2.25, 1.5));
        let l = brick(Bond::Stretcher).layout(lo, hi, 100_000).unwrap();
        assert_eq!(l.count_units(lo, hi), 200);
    }

    #[test]
    fn courses_stagger_by_half_a_brick() {
        let l = brick(Bond::Stretcher)
            .layout(DVec2::ZERO, DVec2::new(1.0, 0.15), 1000)
            .unwrap();
        let starts = |y: f64| {
            let mut xs: Vec<f64> = l
                .shapes
                .iter()
                .filter(|(p, _)| (p[0].y - y).abs() < 1e-9)
                .map(|(p, _)| p[0].x.rem_euclid(0.225))
                .collect();
            xs.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
            xs[0]
        };
        assert!((starts(0.0) - 0.0).abs() < 1e-9);
        assert!((starts(0.075) - 0.1125).abs() < 1e-9);
    }

    #[test]
    fn flemish_headers_sit_centred_on_the_stretcher_below() {
        let l = brick(Bond::Flemish)
            .layout(DVec2::ZERO, DVec2::new(1.0, 0.15), 1000)
            .unwrap();
        let header = (0.215 - 0.01) / 2.0;
        let below: Vec<f64> = l
            .shapes
            .iter()
            .filter(|(p, _)| p[0].y.abs() < 1e-9 && (p[1].x - p[0].x - 0.215).abs() < 1e-9)
            .map(|(p, _)| (p[0].x + p[1].x) / 2.0)
            .collect();
        let above: Vec<f64> = l
            .shapes
            .iter()
            .filter(|(p, _)| {
                (p[0].y - 0.075).abs() < 1e-9 && (p[1].x - p[0].x - header).abs() < 1e-9
            })
            .map(|(p, _)| (p[0].x + p[1].x) / 2.0)
            .collect();
        assert!(!above.is_empty());
        for c in above {
            assert!(below.iter().any(|b| (b - c).abs() < 1e-9), "{c}");
        }
    }

    #[test]
    fn herringbone_units_never_overlap() {
        let r = Recipe::Herringbone {
            length: 0.2,
            width: 0.1,
            joint: 0.0,
        };
        let l = r.layout(DVec2::ZERO, DVec2::splat(1.0), 10_000).unwrap();
        let boxes: Vec<(DVec2, DVec2)> = l.shapes.iter().map(|(p, _)| (p[0], p[2])).collect();
        for (i, a) in boxes.iter().enumerate() {
            for b in &boxes[i + 1..] {
                let ox = a.1.x.min(b.1.x) - a.0.x.max(b.0.x);
                let oy = a.1.y.min(b.1.y) - a.0.y.max(b.0.y);
                assert!(ox <= 1e-9 || oy <= 1e-9, "{a:?} {b:?}");
            }
        }
        // and they cover the area: 50 units a square metre
        let n = l.count_units(DVec2::ZERO, DVec2::ONE);
        assert!((44..=56).contains(&n), "{n}");
    }

    #[test]
    fn boards_and_stones_are_the_same_for_the_same_seed() {
        let (lo, hi) = area();
        for r in [
            Recipe::Boards {
                length: 2.4,
                min_length: 0.9,
                width: 0.145,
                joint: 0.004,
                seed: 3,
            },
            Recipe::Ashlar {
                length: 0.4,
                height: 0.15,
                joint: 0.012,
                seed: 3,
            },
        ] {
            let a = r.layout(lo, hi, 100_000).unwrap();
            assert_eq!(a, r.layout(lo, hi, 100_000).unwrap());
            // the part inside a smaller window is the same stones
            let small = r.layout(DVec2::new(1.0, 0.5), hi, 100_000).unwrap();
            let inside = |l: &Layout| {
                let mut v: Vec<(i64, i64)> = l
                    .shapes
                    .iter()
                    .filter(|(p, _)| p[0].x > 1.2 && p[0].y > 0.7 && p[2].x < 2.4 && p[2].y < 1.3)
                    .map(|(p, _)| ((p[0].x * 1e6) as i64, (p[0].y * 1e6) as i64))
                    .collect();
                v.sort();
                v
            };
            assert_eq!(inside(&a), inside(&small));
        }
    }

    #[test]
    fn boards_keep_their_lengths_in_range() {
        let r = Recipe::Boards {
            length: 2.4,
            min_length: 0.9,
            width: 0.145,
            joint: 0.004,
            seed: 1,
        };
        let (lo, hi) = area();
        for (p, _) in r.layout(lo, hi, 100_000).unwrap().shapes {
            let l = p[1].x - p[0].x;
            assert!((0.9 - 1e-9..=2.4 + 1e-9).contains(&l), "{l}");
        }
    }

    #[test]
    fn a_drawn_tile_repeats_and_keeps_its_jobs() {
        let r = leaf_and_wave();
        let l = r.layout(DVec2::ZERO, DVec2::new(1.2, 0.6), 10_000).unwrap();
        assert!(l.shapes.iter().all(|(_, j)| *j == Job::Raised));
        assert!(l.lines.iter().all(|(_, j)| *j == Job::Groove));
        // 4 × 2 tiles at least, with a ring round them
        assert!(l.shapes.len() >= 8);
        // mirrored every other tile: the leaf leans the other way
        let Recipe::Drawn(mut t) = r else {
            unreachable!()
        };
        t.repeat = Repeat::Mirror;
        let m = Recipe::Drawn(t)
            .layout(DVec2::ZERO, DVec2::new(0.6, 0.3), 1000)
            .unwrap();
        let lean = |p: &Vec<DVec2>| (p[12] - p[0]).x.signum();
        let leans: Vec<f64> = m.shapes.iter().map(|(p, _)| lean(p)).collect();
        assert!(leans.contains(&1.0) && leans.contains(&-1.0));
    }

    #[test]
    fn too_fine_a_pattern_is_refused_not_drawn() {
        let r = brick(Bond::Stretcher);
        assert!(r
            .layout(DVec2::ZERO, DVec2::splat(1000.0), 50_000)
            .is_none());
        let bad = Recipe::Tiles {
            width: 0.0,
            height: 0.3,
            joint: 0.0,
            offset: 0.0,
        };
        assert!(bad.layout(DVec2::ZERO, DVec2::ONE, 1000).is_none());
    }

    #[test]
    fn every_preset_lays_out() {
        for p in PRESETS {
            let r = (p.recipe)();
            let l = r.layout(DVec2::ZERO, DVec2::new(2.0, 2.0), 100_000);
            assert!(l.is_some_and(|l| !l.is_empty()), "{}", p.name);
        }
    }

    #[test]
    fn recipes_round_trip_through_json() {
        for p in PRESETS {
            let r = (p.recipe)();
            let s = serde_json::to_string(&r).unwrap();
            let back: Recipe = serde_json::from_str(&s).unwrap();
            assert_eq!(r, back, "{s}");
        }
    }
}

/// One pattern of the library: what it is called, its group, a line on
/// what it is for, and the pattern itself.
#[derive(Clone, Debug)]
pub struct LibraryEntry {
    pub name: String,
    pub group: String,
    pub about: String,
    /// True size, made by a recipe (bonds, tiles, boards…), or drawn from
    /// lines and a tile (the Hatch Atlas's).
    pub true_size: bool,
    pub pattern: crate::styles::HatchPattern,
}

/// The Hatch Atlas's patterns kept as recommended (owner, 2026-11-18,
/// Hatch_Comparison_REV00): 37 of its 48, where CreaBIM had none as good.
const ATLAS: &str = include_str!("../packs/hatch_atlas.json");

/// The pattern library: the recipes, then the atlas's patterns, grouped.
pub fn library() -> &'static [LibraryEntry] {
    static LIB: std::sync::OnceLock<Vec<LibraryEntry>> = std::sync::OnceLock::new();
    LIB.get_or_init(|| {
        let mut out: Vec<LibraryEntry> = PRESETS
            .iter()
            .map(|pr| {
                let mut p = crate::styles::HatchPattern::from_recipe(pr.name, (pr.recipe)());
                // brick and block give way to the brick hatch at 1:50 and
                // smaller, as drawings are usually drawn
                if pr.group == "Masonry" {
                    p.small_scale = Some(crate::styles::SmallScale {
                        below: 50.0,
                        pattern: crate::Hatch::Brick.label().to_string(),
                    });
                }
                LibraryEntry {
                    name: pr.name.to_string(),
                    group: pr.group.to_string(),
                    about: String::new(),
                    true_size: true,
                    pattern: p,
                }
            })
            .collect();
        #[derive(Deserialize)]
        struct Atlas {
            patterns: Vec<serde_json::Value>,
        }
        if let Ok(a) = serde_json::from_str::<Atlas>(ATLAS) {
            for v in a.patterns {
                let group = v["group"].as_str().unwrap_or("Atlas").to_string();
                let about = v["use"].as_str().unwrap_or("").to_string();
                if let Ok(p) = serde_json::from_value::<crate::styles::HatchPattern>(v) {
                    out.push(LibraryEntry {
                        name: p.name.clone(),
                        group,
                        about,
                        true_size: p.recipe.is_some(),
                        pattern: p,
                    });
                }
            }
        }
        out
    })
}

#[cfg(test)]
mod library_tests {
    #[test]
    fn the_library_holds_the_recipes_and_the_kept_atlas_patterns() {
        let lib = super::library();
        assert_eq!(lib.len(), super::PRESETS.len() + 37);
        // names are unique, so "Add to the project" finds the right one
        let mut names: Vec<&str> = lib.iter().map(|e| e.name.as_str()).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), lib.len());
        assert!(lib
            .iter()
            .any(|e| e.name == "Rammed earth" && e.group == "Natural and new"));
        assert!(
            !lib.iter().any(|e| e.name == "Insulation, batt"),
            "waits for its fix"
        );
    }
}
