//! What fills a door or window opening: an outline, split into a grid of
//! cells by mullions (between columns) and transoms (between rows), each
//! cell a fixed light or a panel that opens with its own mechanism.
//!
//! A single door is one cell hung on its hinges; a pair is two cells in
//! two columns meeting with no mullion between; a patio door is a row of
//! fixed and sliding cells. A window of a fixed sidelight, two casements
//! and a fixed transom over them is a grid like any other.

use crate::element::{OpeningKind, Operation};
use serde::{Deserialize, Serialize};

/// The opening's shape in the wall: square-headed, or with an arched or
/// raked head over upright sides.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outline {
    #[default]
    Rectangle,
    /// A segmental arch, `rise` (metres) from its springing to its crown.
    Segmental { rise: f64 },
    /// A semicircular arch, its rise half the width.
    Round,
    /// A sloping head, `drop` (metres) lower at the end than at the start
    /// (lower at the start when negative).
    Raked { drop: f64 },
}

/// How many straight pieces an arch is drawn and built in.
pub const ARCH_PIECES: usize = 16;

impl Outline {
    pub fn is_rectangle(self) -> bool {
        self == Outline::Rectangle
    }

    /// The outline as it can be built in an opening `w` wide and `h`
    /// high: a rise of at most half the width (a round arch) and a head
    /// that leaves upright sides at least a tenth of the height.
    pub fn fitted(self, w: f64, h: f64) -> Outline {
        let most = (h * 0.9).max(0.0);
        match self {
            Outline::Rectangle => self,
            Outline::Round if w / 2.0 <= most => self,
            Outline::Round => Outline::Segmental { rise: most },
            Outline::Segmental { rise } => {
                let rise = if rise.is_finite() { rise } else { 0.0 };
                let rise = rise.min(w / 2.0).min(most);
                if rise < 0.005 {
                    Outline::Rectangle
                } else if (rise - w / 2.0).abs() < 1e-9 {
                    Outline::Round
                } else {
                    Outline::Segmental { rise }
                }
            }
            Outline::Raked { drop } => {
                let drop = if drop.is_finite() { drop } else { 0.0 };
                let drop = drop.clamp(-most, most);
                if drop.abs() < 0.005 {
                    Outline::Rectangle
                } else {
                    Outline::Raked { drop }
                }
            }
        }
    }

    /// How far the head rises above the lower of its springings.
    pub fn rise(self, w: f64) -> f64 {
        match self {
            Outline::Rectangle => 0.0,
            Outline::Segmental { rise } => rise,
            Outline::Round => w / 2.0,
            Outline::Raked { drop } => drop.abs(),
        }
    }

    /// Where the head starts at each side, up from the bottom.
    pub fn springs(self, w: f64, h: f64) -> (f64, f64) {
        match self.fitted(w, h) {
            Outline::Raked { drop } => (h - (-drop).max(0.0), h - drop.max(0.0)),
            o => (h - o.rise(w), h - o.rise(w)),
        }
    }

    /// The head of an opening `w` wide and `h` high, set in by `inset`
    /// (the frame's inside, for a frame that wide), as points `(x, z)`
    /// from its start to its end, between `x = inset` and `w - inset`.
    /// A rectangle's head is a straight line at `h - inset`.
    pub fn head(self, w: f64, h: f64, inset: f64) -> Vec<(f64, f64)> {
        let d = inset.clamp(0.0, w / 2.0 - 1e-6);
        match self.fitted(w, h) {
            Outline::Rectangle => vec![(d, h - d), (w - d, h - d)],
            Outline::Raked { drop } => {
                let (z0, z1) = (h - (-drop).max(0.0), h - drop.max(0.0));
                let slope = (z1 - z0) / w;
                let down = d * (1.0 + slope * slope).sqrt();
                let at = |x: f64| z0 + slope * x - down;
                vec![(d, at(d)), (w - d, at(w - d))]
            }
            o => {
                // a circle through both springings and the crown
                let r = o.rise(w);
                let radius = (w * w / 4.0 + r * r) / (2.0 * r);
                let cz = h - radius;
                let ri = (radius - d).max(1e-6);
                // where the inset circle meets the frame's insides
                let c = ((w / 2.0 - d) / ri).clamp(-1.0, 1.0);
                let a0 = c.acos();
                let n = ARCH_PIECES;
                (0..=n)
                    .map(|k| {
                        let a = a0 + (std::f64::consts::PI - 2.0 * a0) * k as f64 / n as f64;
                        (w / 2.0 - ri * a.cos(), cz + ri * a.sin())
                    })
                    .collect()
            }
        }
    }

    /// The height of the head set in by `inset` at `x` (held level past
    /// its ends), along the straight pieces of [`Outline::head`].
    pub fn head_z(self, w: f64, h: f64, inset: f64, x: f64) -> f64 {
        head_z_on(&self.head(w, h, inset), x)
    }
}

/// The height of a polyline `(x, z)`, rising in x, at `x`, held level
/// past its ends.
pub fn head_z_on(pts: &[(f64, f64)], x: f64) -> f64 {
    let Some(&(x0, z0)) = pts.first() else {
        return 0.0;
    };
    if x <= x0 {
        return z0;
    }
    for p in pts.windows(2) {
        let ((xa, za), (xb, zb)) = (p[0], p[1]);
        if x <= xb {
            let t = if xb - xa > 1e-12 {
                (x - xa) / (xb - xa)
            } else {
                1.0
            };
            return za + (zb - za) * t;
        }
    }
    pts.last().map_or(0.0, |p| p.1)
}

/// One column or row of the grid: its share of the clear width or height
/// (shares are relative: [1, 1] splits it equally), and whether a mullion
/// or transom stands between it and the one before (a pair of leaves or
/// sliding panels meet with none).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub share: f64,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub bar: bool,
}

impl Track {
    pub fn new(share: f64) -> Self {
        Self { share, bar: false }
    }
}

/// How a cell opens.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mechanism {
    /// A fixed light, or a fixed panel of a sliding door.
    #[default]
    Fixed,
    /// A leaf turning on hinges at one side: a door leaf, a casement.
    Hinged,
    /// A sash hinged at its top: an awning.
    TopHung,
    /// A sash hinged at its bottom: a hopper.
    BottomHung,
    /// Blades turning together: a louvre.
    Louvred,
    /// A leaf hung from a track on the wall's face.
    SurfaceSliding,
    /// A leaf sliding into a cavity in the wall.
    Pocket,
    /// A panel sliding on a track in the frame, past its neighbours.
    Sliding,
    /// A door leaf turning on a pivot in the floor and head, set in from
    /// its hinge side (an offset pivot door).
    Pivot,
    /// A leaf or sash turning on a pivot at its middle, about an upright
    /// axis (a centre pivot door, a vertical pivot window).
    CentrePivot,
    /// A door leaf on hinges that let it swing both ways (a café or
    /// kitchen door).
    DoubleAction,
    /// A door panel or window sash hinged to its neighbours, folding up
    /// against its hand's jamb with them.
    Folding,
    /// A sash that turns on side hinges like a casement or tilts in at
    /// its top on bottom hinges like a hopper, by its handle.
    TiltTurn,
    /// A sash sliding up or down in the frame, past its neighbour: a
    /// hung (sash) window.
    VerticalSliding,
    /// A sash turning on pivots at the middle of its sides, about a
    /// level axis.
    HorizontalPivot,
    /// A door of hinged horizontal panels running up on tracks to lie
    /// under the ceiling (a sectional garage door).
    Sectional,
    /// A curtain of slats rolling up into a coil box over the opening (a
    /// roller shutter door).
    Rolling,
    /// Four wings turning in a drum about an upright post (a revolving
    /// door).
    Revolving,
}

impl Mechanism {
    pub fn slides(self) -> bool {
        matches!(
            self,
            Mechanism::SurfaceSliding
                | Mechanism::Pocket
                | Mechanism::Sliding
                | Mechanism::VerticalSliding
        )
    }

    /// Does it fold away with its neighbours?
    pub fn folds(self) -> bool {
        self == Mechanism::Folding
    }

    /// Does it turn on hinges (a door leaf, a casement, an awning, a hopper)?
    pub fn hinged(self) -> bool {
        matches!(
            self,
            Mechanism::Hinged | Mechanism::TopHung | Mechanism::BottomHung
        )
    }
}

/// How a cell is shown in drawings and 3D.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Display {
    /// The drawing convention: a hinged door drawn open in plan, every
    /// other panel closed.
    #[default]
    Convention,
    Closed,
    Open,
}

/// One cell of the grid.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Cell {
    pub mechanism: Mechanism,
    /// Hinged (or sliding) towards the opening's end instead of its start.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub hand: bool,
    /// Opens to the other side from its opening's own: from the side a
    /// door opens to, or the outside a window faces (an inward casement).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub flipped: bool,
    /// Has the handle: the leaf opened first in a pair.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub active: bool,
    /// Which track a sliding panel runs on (0 or 1).
    #[serde(skip_serializing_if = "is_zero")]
    pub track: u8,
    #[serde(skip_serializing_if = "is_convention")]
    pub display: Display,
    /// Its glass split by glazing bars: panes across and up.
    #[serde(skip_serializing_if = "is_one_pane")]
    pub panes: [u32; 2],
    /// How many columns it spans from its own (a transom light across
    /// the whole opening); the cells it covers are kept but not built.
    #[serde(skip_serializing_if = "is_one")]
    pub across: u32,
}

fn is_one(v: &u32) -> bool {
    *v == 1
}

fn is_zero(v: &u8) -> bool {
    *v == 0
}

fn is_convention(d: &Display) -> bool {
    *d == Display::Convention
}

fn is_one_pane(p: &[u32; 2]) -> bool {
    *p == [1, 1]
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            mechanism: Mechanism::Fixed,
            hand: false,
            flipped: false,
            active: false,
            track: 0,
            display: Display::Convention,
            panes: [1, 1],
            across: 1,
        }
    }
}

impl Cell {
    pub fn new(mechanism: Mechanism) -> Self {
        Self {
            mechanism,
            ..Self::default()
        }
    }
}

/// The grid of cells inside an opening's frame: its columns from the
/// opening's start, its rows from the bottom, and its cells row by row
/// (bottom row first), `columns × rows` of them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Grid {
    pub outline: Outline,
    pub columns: Vec<Track>,
    pub rows: Vec<Track>,
    pub cells: Vec<Cell>,
}

impl Default for Grid {
    /// A single fixed cell.
    fn default() -> Self {
        Self::single(Cell::default())
    }
}

impl Grid {
    pub fn single(cell: Cell) -> Self {
        Self {
            outline: Outline::Rectangle,
            columns: vec![Track::new(1.0)],
            rows: vec![Track::new(1.0)],
            cells: vec![cell],
        }
    }

    /// One row of cells side by side, with the given shares and no
    /// mullions between them.
    pub fn row(cells: Vec<(f64, Cell)>) -> Self {
        Self {
            outline: Outline::Rectangle,
            columns: cells.iter().map(|(s, _)| Track::new(*s)).collect(),
            rows: vec![Track::new(1.0)],
            cells: cells.into_iter().map(|(_, c)| c).collect(),
        }
    }

    /// The cell in column `i`, row `j`.
    /// The grid seen from its other side, left for right: its columns in
    /// the other order, each cell where its mirror falls and hinged on its
    /// other jamb. A window flipped by hand (its `flip_hand`) draws this.
    pub fn mirrored(&self) -> Grid {
        let n = self.columns.len();
        if n == 0 {
            return self.clone();
        }
        let mut columns: Vec<Track> = self.columns.iter().rev().cloned().collect();
        // a bar stands on its column's left: between new k-1 and k is the
        // bar that stood between old n-k-1 and n-k
        for k in 0..n {
            columns[k].bar = if k == 0 {
                self.columns[0].bar
            } else {
                self.columns[n - k].bar
            };
        }
        let rows = self.rows.len();
        let mut cells = self.cells.clone();
        for j in 0..rows {
            let row: Vec<Cell> = (0..n)
                .map(|i| self.cells.get(j * n + i).copied().unwrap_or_default())
                .collect();
            let mut out: Vec<Cell> = vec![Cell::default(); n];
            // what covered places held, kept as they were
            for (i, c) in row.iter().enumerate() {
                if self.covered(i, j) {
                    out[n - 1 - i] = *c;
                }
            }
            for (i, c) in row.iter().enumerate() {
                if self.covered(i, j) {
                    continue;
                }
                let across = c.across.max(1) as usize;
                let at = n.saturating_sub(i + across);
                let mut c = *c;
                c.hand = !c.hand;
                out[at] = c;
            }
            for (i, c) in out.into_iter().enumerate() {
                if let Some(slot) = cells.get_mut(j * n + i) {
                    *slot = c;
                }
            }
        }
        Grid {
            outline: self.outline,
            columns,
            rows: self.rows.clone(),
            cells,
        }
    }

    pub fn cell(&self, i: usize, j: usize) -> Option<&Cell> {
        self.cells.get(j * self.columns.len() + i)
    }

    /// Is the place in column `i`, row `j` covered by a cell to its left
    /// spanning over it (so not a cell of its own)?
    pub fn covered(&self, i: usize, j: usize) -> bool {
        let n = self.columns.len();
        (0..i).any(|k| {
            self.cells
                .get(j * n + k)
                .is_some_and(|c| k + c.across.max(1) as usize > i)
        })
    }

    /// The opening as an assembly: a core (a door's leaves, a window's
    /// sashes) with a fixed sidelight each side or not and a transom light
    /// over them all or not, if its grid is laid out so. Every opening
    /// of one row and no mullions is a core alone.
    pub fn assembly(&self) -> Option<Assembly> {
        let n = self.columns.len();
        let rows = self.rows.len();
        if n == 0 || rows == 0 || rows > 2 || self.cells.len() < n * rows {
            return None;
        }
        // hung sashes, one above the other, are one window, not a sash
        // under a transom light
        if self
            .cells
            .iter()
            .any(|c| c.mechanism == Mechanism::VerticalSliding)
        {
            return None;
        }
        // the bottom row in runs between mullions
        let mut runs: Vec<(usize, usize)> = vec![];
        for (i, column) in self.columns[..n].iter().enumerate() {
            match runs.last_mut() {
                Some(r) if !column.bar => r.1 = i,
                _ => runs.push((i, i)),
            }
        }
        if self.cells[..n].iter().any(|c| c.across > 1) {
            return None;
        }
        let opens =
            |r: &(usize, usize)| (r.0..=r.1).any(|i| self.cells[i].mechanism != Mechanism::Fixed);
        let k = match runs.iter().filter(|r| opens(r)).count() {
            1 => runs.iter().position(opens)?,
            0 if runs.len() == 1 => 0,
            0 if runs.len() == 3 => 1,
            _ => return None,
        };
        let side = |r: &(usize, usize)| r.0 == r.1 && self.cells[r.0].mechanism == Mechanism::Fixed;
        if k > 1 || runs.len() - k > 2 || !runs.iter().enumerate().all(|(i, r)| i == k || side(r)) {
            return None;
        }
        let (a, b) = runs[k];
        let total: f64 = self.columns.iter().map(|t| t.share).sum();
        let rtotal: f64 = self.rows.iter().map(|t| t.share).sum();
        if total <= 0.0 || rtotal <= 0.0 {
            return None;
        }
        let transom = if rows == 2 {
            let t = self.cells[n];
            if t.across as usize != n {
                return None;
            }
            Some((self.rows[1].share / rtotal, t))
        } else {
            None
        };
        Some(Assembly {
            outline: self.outline,
            core: Grid {
                outline: Outline::Rectangle,
                columns: self.columns[a..=b]
                    .iter()
                    .enumerate()
                    .map(|(i, t)| Track {
                        share: t.share,
                        bar: i > 0 && t.bar,
                    })
                    .collect(),
                rows: vec![Track::new(1.0)],
                cells: self.cells[a..=b].to_vec(),
            },
            columns: (a, b),
            left: (a > 0).then(|| self.columns[0].share / total),
            right: (b + 1 < n).then(|| self.columns[n - 1].share / total),
            transom,
        })
    }

    /// The grid made whole: at least one column and row, positive shares,
    /// exactly one cell for each place (missing ones fixed).
    pub fn fitted(&self) -> Self {
        let tracks = |t: &[Track]| -> Vec<Track> {
            let mut t: Vec<Track> = t
                .iter()
                .map(|t| Track {
                    share: if t.share.is_finite() && t.share > 0.0 {
                        t.share
                    } else {
                        1.0
                    },
                    bar: t.bar,
                })
                .take(24)
                .collect();
            if t.is_empty() {
                t.push(Track::new(1.0));
            }
            t
        };
        let (columns, rows) = (tracks(&self.columns), tracks(&self.rows));
        let mut cells = self.cells.clone();
        cells.resize(columns.len() * rows.len(), Cell::default());
        for c in &mut cells {
            c.panes = [c.panes[0].clamp(1, 12), c.panes[1].clamp(1, 12)];
            c.track = c.track.min(1);
        }
        // a span stops at its row's end
        let n = columns.len();
        for (k, c) in cells.iter_mut().enumerate() {
            c.across = c.across.clamp(1, (n - k % n) as u32);
        }
        Self {
            outline: self.outline,
            columns,
            rows,
            cells,
        }
    }

    /// A grid as one of the operations doors and windows had before
    /// cells: how files from then convert, and what the operation menu
    /// sets. `share` is a pair's active leaf share, `panels` and `fixed`
    /// a multi-panel slider's panels and fixed bits, `panes` a window's
    /// glazing bars.
    pub fn of_operation(
        kind: OpeningKind,
        op: Operation,
        share: f64,
        panels: u32,
        fixed: u32,
        panes: [u32; 2],
    ) -> Self {
        let share = if share.is_finite() {
            share.clamp(0.5, 0.85)
        } else {
            0.5
        };
        let cell = |m: Mechanism| Cell {
            panes,
            ..Cell::new(m)
        };
        let pair = |m: Mechanism, s: f64| {
            Grid::row(vec![
                (
                    s,
                    Cell {
                        active: true,
                        ..cell(m)
                    },
                ),
                (
                    1.0 - s,
                    Cell {
                        hand: true,
                        ..cell(m)
                    },
                ),
            ])
        };
        match (kind, op) {
            (OpeningKind::Door, Operation::Standard) => Grid::single(Cell {
                active: true,
                ..cell(Mechanism::Hinged)
            }),
            (_, Operation::Standard) => Grid::single(cell(Mechanism::Fixed)),
            (_, Operation::DoubleSwing) => pair(Mechanism::Hinged, share),
            (_, Operation::SurfaceSliding) => Grid::single(Cell {
                active: true,
                ..cell(Mechanism::SurfaceSliding)
            }),
            (_, Operation::SurfaceSlidingPair) => pair(Mechanism::SurfaceSliding, 0.5),
            (_, Operation::Pocket) => Grid::single(Cell {
                active: true,
                ..cell(Mechanism::Pocket)
            }),
            (_, Operation::PocketPair) => pair(Mechanism::Pocket, 0.5),
            (_, Operation::Pivot) => Grid::single(Cell {
                active: true,
                ..cell(Mechanism::Pivot)
            }),
            (_, Operation::CentrePivot) => Grid::single(Cell {
                active: true,
                ..cell(Mechanism::CentrePivot)
            }),
            (_, Operation::DoubleAction) => Grid::single(Cell {
                active: true,
                ..cell(Mechanism::DoubleAction)
            }),
            (_, Operation::DoubleActionPair) => pair(Mechanism::DoubleAction, share),
            (_, Operation::Bifold | Operation::Folding) => {
                // folding up against the jamb at the start, the handle on
                // the leading panel, the last
                let n = if op == Operation::Bifold {
                    2
                } else {
                    panels.clamp(3, 6)
                };
                Grid::row(
                    (0..n)
                        .map(|i| {
                            (
                                1.0,
                                Cell {
                                    active: i + 1 == n,
                                    ..cell(Mechanism::Folding)
                                },
                            )
                        })
                        .collect(),
                )
            }
            (_, Operation::BifoldPair) => Grid::row(
                // two panels folding to each jamb, meeting in the middle,
                // the handle on the first meeting panel
                (0..4)
                    .map(|i| {
                        (
                            1.0,
                            Cell {
                                active: i == 1,
                                hand: i >= 2,
                                ..cell(Mechanism::Folding)
                            },
                        )
                    })
                    .collect(),
            ),
            (_, Operation::Sectional) => Grid::single(Cell {
                active: true,
                ..cell(Mechanism::Sectional)
            }),
            (_, Operation::Roller) => Grid::single(Cell {
                active: true,
                ..cell(Mechanism::Rolling)
            }),
            (_, Operation::Revolving) => Grid::single(Cell {
                active: true,
                ..cell(Mechanism::Revolving)
            }),
            (_, Operation::MultiSlide) => {
                let n = panels.clamp(2, 4);
                Grid::row(
                    (0..n)
                        .map(|i| {
                            let m = if fixed >> i & 1 == 1 {
                                Mechanism::Fixed
                            } else {
                                Mechanism::Sliding
                            };
                            (
                                1.0,
                                Cell {
                                    track: (i % 2) as u8,
                                    ..cell(m)
                                },
                            )
                        })
                        .collect(),
                )
            }
        }
    }

    /// Which of the operations before cells this grid is, if it is one
    /// of them: what drawings, schedules and IFC still build from until
    /// they build from cells directly.
    pub fn operation(&self, kind: OpeningKind) -> Option<Operation> {
        if !self.outline.is_rectangle() {
            return None;
        }
        let one_row = self.rows.len() == 1;
        let no_bars = self.columns.iter().skip(1).all(|t| !t.bar);
        if !one_row || !no_bars {
            return None;
        }
        let c = &self.cells;
        let m = |i: usize| c[i].mechanism;
        let plain = |i: usize| !c[i].flipped && c[i].display == Display::Convention;
        if !(0..c.len()).all(plain) {
            return None;
        }
        let pair = |mech: Mechanism| {
            c.len() == 2
                && m(0) == mech
                && m(1) == mech
                && c[0].active
                && !c[0].hand
                && !c[1].active
                && c[1].hand
        };
        match c.len() {
            1 => match (kind, m(0)) {
                (OpeningKind::Door, Mechanism::Hinged) if !c[0].hand => Some(Operation::Standard),
                (OpeningKind::Window | OpeningKind::Hole, Mechanism::Fixed) => {
                    Some(Operation::Standard)
                }
                (OpeningKind::Door, Mechanism::SurfaceSliding) if !c[0].hand => {
                    Some(Operation::SurfaceSliding)
                }
                (OpeningKind::Door, Mechanism::Pocket) if !c[0].hand => Some(Operation::Pocket),
                (OpeningKind::Door, Mechanism::Pivot) if !c[0].hand && c[0].active => {
                    Some(Operation::Pivot)
                }
                (OpeningKind::Door, Mechanism::CentrePivot) if !c[0].hand && c[0].active => {
                    Some(Operation::CentrePivot)
                }
                (OpeningKind::Door, Mechanism::DoubleAction) if !c[0].hand && c[0].active => {
                    Some(Operation::DoubleAction)
                }
                (OpeningKind::Door, Mechanism::Sectional) if !c[0].hand && c[0].active => {
                    Some(Operation::Sectional)
                }
                (OpeningKind::Door, Mechanism::Rolling) if !c[0].hand && c[0].active => {
                    Some(Operation::Roller)
                }
                (OpeningKind::Door, Mechanism::Revolving) if !c[0].hand && c[0].active => {
                    Some(Operation::Revolving)
                }
                _ => None,
            },
            2 if kind == OpeningKind::Door && pair(Mechanism::DoubleAction) => {
                Some(Operation::DoubleActionPair)
            }
            2 if kind == OpeningKind::Door && pair(Mechanism::Hinged) => {
                Some(Operation::DoubleSwing)
            }
            2 if kind == OpeningKind::Door
                && pair(Mechanism::SurfaceSliding)
                && self.columns[0].share == self.columns[1].share =>
            {
                Some(Operation::SurfaceSlidingPair)
            }
            2 if kind == OpeningKind::Door
                && pair(Mechanism::Pocket)
                && self.columns[0].share == self.columns[1].share =>
            {
                Some(Operation::PocketPair)
            }
            2..=6
                if kind == OpeningKind::Door
                    && c.iter()
                        .all(|c| c.mechanism == Mechanism::Folding && c.track == 0)
                    && self
                        .columns
                        .iter()
                        .all(|t| t.share == self.columns[0].share) =>
            {
                let n = c.len();
                let run = c
                    .iter()
                    .enumerate()
                    .all(|(i, c)| !c.hand && c.active == (i + 1 == n));
                let parting = n == 4
                    && c.iter()
                        .enumerate()
                        .all(|(i, c)| c.hand == (i >= 2) && c.active == (i == 1));
                match (n, run, parting) {
                    (2, true, _) => Some(Operation::Bifold),
                    (3.., true, _) => Some(Operation::Folding),
                    (_, _, true) => Some(Operation::BifoldPair),
                    _ => None,
                }
            }
            2..=4
                if kind == OpeningKind::Door
                    && c.iter().enumerate().all(|(i, c)| {
                        matches!(c.mechanism, Mechanism::Fixed | Mechanism::Sliding)
                            && c.track as usize == i % 2
                            && !c.hand
                            && !c.active
                    })
                    && self
                        .columns
                        .iter()
                        .all(|t| t.share == self.columns[0].share) =>
            {
                Some(Operation::MultiSlide)
            }
            _ => None,
        }
    }

    /// A pair's active leaf share of the clear width.
    pub fn active_share(&self) -> f64 {
        let total: f64 = self.columns.iter().map(|t| t.share).sum();
        match self.cells.iter().position(|c| c.active) {
            Some(i) if self.columns.len() == 2 && total > 0.0 => self.columns[i].share / total,
            _ => 0.5,
        }
    }

    /// A multi-panel slider's fixed panels, one bit each from the start.
    pub fn fixed_bits(&self) -> u32 {
        self.cells
            .iter()
            .take(32)
            .enumerate()
            .filter(|(_, c)| c.mechanism == Mechanism::Fixed)
            .fold(0, |b, (i, _)| b | 1 << i)
    }

    /// The first cell's glazing bars (every cell of a window before cells
    /// had the same).
    pub fn panes(&self) -> [u32; 2] {
        self.cells.first().map_or([1, 1], |c| c.panes)
    }
}

/// An opening read as a core with lights round it (see
/// [`Grid::assembly`]): the core's own grid, which columns of the whole
/// it fills, the sidelights' shares of the clear width, and the transom
/// light's share of the clear height with its cell, and the outline of
/// the whole.
#[derive(Clone, Debug, PartialEq)]
pub struct Assembly {
    pub outline: Outline,
    pub core: Grid,
    pub columns: (usize, usize),
    pub left: Option<f64>,
    pub right: Option<f64>,
    pub transom: Option<(f64, Cell)>,
}

impl Assembly {
    /// A core alone.
    pub fn of(core: Grid) -> Self {
        let n = core.columns.len();
        Self {
            outline: core.outline,
            core: Grid {
                outline: Outline::Rectangle,
                ..core
            },
            columns: (0, n.saturating_sub(1)),
            left: None,
            right: None,
            transom: None,
        }
    }

    /// Has it any light round its core?
    pub fn has_lights(&self) -> bool {
        self.left.is_some() || self.right.is_some() || self.transom.is_some()
    }

    /// The whole grid: the sidelights each side of the core behind a
    /// mullion, the transom over them all behind a transom bar, each
    /// light glazed as the core's first cell.
    pub fn grid(&self) -> Grid {
        let core = &self.core;
        let ctotal: f64 = core.columns.iter().map(|t| t.share).sum::<f64>().max(1e-9);
        let side = |s: Option<f64>| s.map(|s| s.clamp(0.02, 0.6));
        let (l, r) = (side(self.left), side(self.right));
        let rest = (1.0 - l.unwrap_or(0.0) - r.unwrap_or(0.0)).max(0.1);
        let light = Cell {
            panes: core.panes(),
            ..Cell::default()
        };
        let mut columns = vec![];
        let mut row = vec![];
        if let Some(l) = l {
            columns.push(Track::new(l));
            row.push(light);
        }
        for (i, t) in core.columns.iter().enumerate() {
            columns.push(Track {
                share: rest * t.share / ctotal,
                bar: if i == 0 { l.is_some() } else { t.bar },
            });
        }
        row.extend(core.cells.iter().copied());
        if let Some(r) = r {
            columns.push(Track {
                share: r,
                bar: true,
            });
            row.push(light);
        }
        let n = columns.len();
        let mut rows = vec![Track::new(1.0)];
        let mut cells = row;
        if let Some((t, cell)) = self.transom {
            let t = t.clamp(0.05, 0.6);
            rows = vec![
                Track::new(1.0 - t),
                Track {
                    share: t,
                    bar: true,
                },
            ];
            cells.push(Cell {
                across: n as u32,
                ..cell
            });
            cells.extend(std::iter::repeat_n(light, n - 1));
        }
        Grid {
            outline: self.outline,
            columns,
            rows,
            cells,
        }
    }
}

/// The windows the *Opens* menu offers, each a grid of cells.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WindowStyle {
    Fixed,
    Casement,
    CasementPair,
    Awning,
    Hopper,
    Sliding,
    Louvred,
    TiltTurn,
    SingleHung,
    DoubleHung,
    HorizontalPivot,
    VerticalPivot,
    Bifold,
    Guillotine,
}

impl WindowStyle {
    pub const ALL: [WindowStyle; 14] = [
        WindowStyle::Fixed,
        WindowStyle::Casement,
        WindowStyle::CasementPair,
        WindowStyle::Awning,
        WindowStyle::Hopper,
        WindowStyle::Sliding,
        WindowStyle::Louvred,
        WindowStyle::TiltTurn,
        WindowStyle::SingleHung,
        WindowStyle::DoubleHung,
        WindowStyle::HorizontalPivot,
        WindowStyle::VerticalPivot,
        WindowStyle::Bifold,
        WindowStyle::Guillotine,
    ];

    pub fn label(self) -> &'static str {
        match self {
            WindowStyle::Fixed => "Fixed",
            WindowStyle::Casement => "Casement",
            WindowStyle::CasementPair => "Casement pair",
            WindowStyle::Awning => "Awning",
            WindowStyle::Hopper => "Hopper",
            WindowStyle::Sliding => "Horizontal sliding",
            WindowStyle::Louvred => "Louvred",
            WindowStyle::TiltTurn => "Tilt and turn",
            WindowStyle::SingleHung => "Single hung",
            WindowStyle::DoubleHung => "Double hung",
            WindowStyle::HorizontalPivot => "Horizontal pivot",
            WindowStyle::VerticalPivot => "Vertical pivot",
            WindowStyle::Bifold => "Bifold",
            WindowStyle::Guillotine => "Guillotine",
        }
    }

    /// The ways a roof window opens: fixed, top hung (an awning, hinged
    /// at its head) and centre pivot (turning on pivots at the middle of
    /// its sides).
    pub const ROOF: [WindowStyle; 3] = [
        WindowStyle::Fixed,
        WindowStyle::Awning,
        WindowStyle::HorizontalPivot,
    ];

    /// Its name on a roof window, as makers name them.
    pub fn roof_label(self) -> &'static str {
        match self {
            WindowStyle::Awning => "Top hung",
            WindowStyle::HorizontalPivot => "Centre pivot",
            s => s.label(),
        }
    }

    /// A short form of its name that fits a narrow menu button (the menu
    /// itself lists the full names).
    pub fn short_label(self) -> &'static str {
        match self {
            WindowStyle::CasementPair => "Casements",
            WindowStyle::Sliding => "Sliding",
            WindowStyle::TiltTurn => "Tilt-turn",
            WindowStyle::SingleHung => "Hung",
            WindowStyle::DoubleHung => "Both hung",
            WindowStyle::HorizontalPivot => "Level pivot",
            WindowStyle::VerticalPivot => "Side pivot",
            _ => self.label(),
        }
    }

    /// Its grid, its casements opening inward when `inward` (the
    /// project's convention), each cell glazed `panes`.
    pub fn grid(self, inward: bool, panes: [u32; 2]) -> Grid {
        let cell = |m: Mechanism| Cell {
            panes,
            ..Cell::new(m)
        };
        match self {
            WindowStyle::Fixed => Grid::single(cell(Mechanism::Fixed)),
            WindowStyle::Casement => Grid::single(Cell {
                flipped: inward,
                ..cell(Mechanism::Hinged)
            }),
            WindowStyle::CasementPair => Grid::row(vec![
                (
                    1.0,
                    Cell {
                        flipped: inward,
                        active: true,
                        ..cell(Mechanism::Hinged)
                    },
                ),
                (
                    1.0,
                    Cell {
                        flipped: inward,
                        hand: true,
                        ..cell(Mechanism::Hinged)
                    },
                ),
            ]),
            WindowStyle::Awning => Grid::single(cell(Mechanism::TopHung)),
            // a hopper opens into the room
            WindowStyle::Hopper => Grid::single(Cell {
                flipped: true,
                ..cell(Mechanism::BottomHung)
            }),
            WindowStyle::Sliding => Grid::row(vec![
                (1.0, cell(Mechanism::Fixed)),
                (
                    1.0,
                    Cell {
                        track: 1,
                        ..cell(Mechanism::Sliding)
                    },
                ),
            ]),
            WindowStyle::Louvred => Grid::single(Cell {
                panes: [1, 1],
                ..cell(Mechanism::Louvred)
            }),
            // a tilt and turn sash opens into the room
            WindowStyle::TiltTurn => Grid::single(Cell {
                flipped: true,
                ..cell(Mechanism::TiltTurn)
            }),
            // two sashes one above the other, the lower one inside: the
            // lower slides up, and in a double hung the upper slides down
            WindowStyle::SingleHung | WindowStyle::DoubleHung => Grid {
                outline: Outline::Rectangle,
                columns: vec![Track::new(1.0)],
                rows: vec![Track::new(1.0), Track::new(1.0)],
                cells: vec![
                    cell(Mechanism::VerticalSliding),
                    Cell {
                        track: 1,
                        ..cell(if self == WindowStyle::DoubleHung {
                            Mechanism::VerticalSliding
                        } else {
                            Mechanism::Fixed
                        })
                    },
                ],
            },
            // one sash rising into the wall over the window (a servery)
            WindowStyle::Guillotine => Grid::single(cell(Mechanism::VerticalSliding)),
            WindowStyle::HorizontalPivot => Grid::single(cell(Mechanism::HorizontalPivot)),
            WindowStyle::VerticalPivot => Grid::single(cell(Mechanism::CentrePivot)),
            // two sashes folding against the start, the handle on the
            // second
            WindowStyle::Bifold => Grid::row(vec![
                (
                    1.0,
                    Cell {
                        flipped: inward,
                        ..cell(Mechanism::Folding)
                    },
                ),
                (
                    1.0,
                    Cell {
                        flipped: inward,
                        active: true,
                        ..cell(Mechanism::Folding)
                    },
                ),
            ]),
        }
    }

    /// The style a window's grid is, if it is one of them (whichever way
    /// its casements open, however its cells are glazed).
    pub fn of(grid: &Grid) -> Option<WindowStyle> {
        WindowStyle::ALL.into_iter().find(|s| {
            [false, true].iter().any(|&inward| {
                let g = s.grid(inward, [1, 1]);
                g.rows.len() == grid.rows.len()
                    && g.columns.len() == grid.columns.len()
                    && g.columns
                        .iter()
                        .zip(&grid.columns)
                        .all(|(a, b)| a.bar == b.bar)
                    && g.rows.iter().zip(&grid.rows).all(|(a, b)| a.bar == b.bar)
                    && g.cells.iter().zip(&grid.cells).all(|(a, b)| {
                        // a slider is one whichever of its panels is fixed
                        let same = a.mechanism == b.mechanism
                            || (*s == WindowStyle::Sliding
                                && matches!(b.mechanism, Mechanism::Fixed | Mechanism::Sliding));
                        same && a.hand == b.hand && a.track == b.track
                    })
                    && (*s != WindowStyle::Sliding
                        || grid.cells.iter().any(|c| c.mechanism == Mechanism::Sliding))
            })
        })
    }
}

#[cfg(test)]
mod mirror_tests {
    use super::*;

    #[test]
    fn a_grid_mirrored_swaps_its_columns_and_hinges() {
        let hinged = Cell {
            mechanism: Mechanism::Hinged,
            hand: false,
            ..Cell::default()
        };
        let fixed = Cell::default();
        let mut g = Grid::single(hinged);
        g.columns = vec![
            Track {
                share: 2.0,
                bar: false,
            },
            Track {
                share: 1.0,
                bar: true,
            },
        ];
        g.cells = vec![hinged, fixed];
        let m = g.mirrored();
        assert_eq!(m.columns[0].share, 1.0);
        assert_eq!(m.columns[1].share, 2.0);
        assert!(m.columns[1].bar, "the bar between them stays between them");
        assert_eq!(m.cell(1, 0).unwrap().mechanism, Mechanism::Hinged);
        assert!(m.cell(1, 0).unwrap().hand, "hinged on its other jamb");
        assert_eq!(m.cell(0, 0).unwrap().mechanism, Mechanism::Fixed);
        // twice is the grid again
        assert_eq!(m.mirrored(), g);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sliding_window_stays_one_whichever_panel_is_fixed() {
        let mut g = WindowStyle::Sliding.grid(false, [1, 1]);
        assert_eq!(WindowStyle::of(&g), Some(WindowStyle::Sliding));
        g.cells[0].mechanism = Mechanism::Sliding;
        g.cells[1].mechanism = Mechanism::Fixed;
        assert_eq!(WindowStyle::of(&g), Some(WindowStyle::Sliding));
        g.cells[0].mechanism = Mechanism::Sliding;
        g.cells[1].mechanism = Mechanism::Sliding;
        assert_eq!(WindowStyle::of(&g), Some(WindowStyle::Sliding));
        // both fixed is a fixed light in two panes, not a slider
        g.cells[0].mechanism = Mechanism::Fixed;
        g.cells[1].mechanism = Mechanism::Fixed;
        assert_eq!(WindowStyle::of(&g), None);
    }

    #[test]
    fn every_operation_before_cells_comes_back_from_its_grid() {
        let door = OpeningKind::Door;
        for op in Operation::for_kind(door) {
            let g = Grid::of_operation(door, *op, 0.6, 3, 0b101, [1, 1]);
            assert_eq!(g.operation(door), Some(*op), "{op:?}");
        }
        let g = Grid::of_operation(OpeningKind::Window, Operation::Standard, 0.5, 2, 1, [3, 2]);
        assert_eq!(g.operation(OpeningKind::Window), Some(Operation::Standard));
        assert_eq!(g.panes(), [3, 2]);
        // a pair's share and a slider's fixed panels survive
        let g = Grid::of_operation(door, Operation::DoubleSwing, 0.67, 2, 1, [1, 1]);
        assert!((g.active_share() - 0.67).abs() < 1e-12);
        let g = Grid::of_operation(door, Operation::MultiSlide, 0.5, 4, 0b1001, [1, 1]);
        assert_eq!((g.cells.len(), g.fixed_bits()), (4, 0b1001));
    }

    #[test]
    fn pivot_double_action_and_folding_doors_are_grids_and_read_back() {
        let door = OpeningKind::Door;
        for (op, n) in [
            (Operation::Pivot, 1),
            (Operation::CentrePivot, 1),
            (Operation::DoubleAction, 1),
            (Operation::DoubleActionPair, 2),
            (Operation::Bifold, 2),
            (Operation::BifoldPair, 4),
            (Operation::Folding, 5),
            (Operation::Sectional, 1),
            (Operation::Roller, 1),
            (Operation::Revolving, 1),
        ] {
            let g = Grid::of_operation(door, op, 0.6, 5, 0, [1, 1]);
            assert_eq!(g.cells.len(), n, "{op:?}");
            assert_eq!(g.operation(door), Some(op), "{op:?}");
            // saved and read back
            let j = serde_json::to_string(&g).unwrap();
            assert_eq!(serde_json::from_str::<Grid>(&j).unwrap(), g);
        }
        // a folding door has 3 to 6 panels, a pair keeps its share
        let g = Grid::of_operation(door, Operation::Folding, 0.5, 9, 0, [1, 1]);
        assert_eq!(g.cells.len(), 6);
        let g = Grid::of_operation(door, Operation::Folding, 0.5, 1, 0, [1, 1]);
        assert_eq!(g.cells.len(), 3);
        let g = Grid::of_operation(door, Operation::DoubleActionPair, 0.67, 2, 0, [1, 1]);
        assert!((g.active_share() - 0.67).abs() < 1e-12);
        // a folding run with its handle on the wrong panel is just cells
        let mut g = Grid::of_operation(door, Operation::Bifold, 0.5, 2, 0, [1, 1]);
        g.cells[0].active = true;
        assert_eq!(g.operation(door), None);
        // the new mechanisms are saved by name
        let c = Cell::new(Mechanism::CentrePivot);
        assert_eq!(
            serde_json::to_string(&c).unwrap(),
            r#"{"mechanism":"centre-pivot"}"#
        );
    }

    #[test]
    fn window_styles_are_grids_and_read_back() {
        for s in WindowStyle::ALL {
            for inward in [false, true] {
                let g = s.grid(inward, [2, 1]);
                assert_eq!(WindowStyle::of(&g), Some(s), "{s:?}");
                assert_eq!(g.fitted(), g);
            }
        }
        // casements follow the project's side; a hopper always opens in
        assert!(!WindowStyle::Casement.grid(false, [1, 1]).cells[0].flipped);
        assert!(WindowStyle::Casement.grid(true, [1, 1]).cells[0].flipped);
        assert!(WindowStyle::Hopper.grid(false, [1, 1]).cells[0].flipped);
        // a guillotine is one rising sash, read as nothing else
        let g = WindowStyle::Guillotine.grid(false, [1, 1]);
        assert_eq!(
            (g.rows.len(), g.cells[0].mechanism),
            (1, Mechanism::VerticalSliding)
        );
        // hung sashes are two rows, the lower inside the upper, with no
        // transom between; a tilt and turn sash always opens in
        let g = WindowStyle::DoubleHung.grid(false, [1, 1]);
        assert_eq!((g.columns.len(), g.rows.len()), (1, 2));
        assert!(!g.rows[1].bar);
        assert_eq!((g.cells[0].track, g.cells[1].track), (0, 1));
        assert!(g.cells.iter().all(|c| c.mechanism.slides()));
        assert!(g.assembly().is_none(), "not a sash under a transom");
        let g = WindowStyle::SingleHung.grid(false, [1, 1]);
        assert_eq!(g.cells[1].mechanism, Mechanism::Fixed);
        assert!(g.assembly().is_none());
        assert!(WindowStyle::TiltTurn.grid(false, [1, 1]).cells[0].flipped);
        // and none of them is mistaken for another
        let mut seen = std::collections::HashSet::new();
        for s in WindowStyle::ALL {
            assert!(seen.insert(format!("{:?}", s.grid(false, [1, 1]))), "{s:?}");
        }
        // the new mechanisms are saved by name
        for (m, name) in [
            (Mechanism::TiltTurn, "tilt-turn"),
            (Mechanism::VerticalSliding, "vertical-sliding"),
            (Mechanism::HorizontalPivot, "horizontal-pivot"),
        ] {
            assert_eq!(
                serde_json::to_string(&Cell::new(m)).unwrap(),
                format!(r#"{{"mechanism":"{name}"}}"#)
            );
        }
        // a plain window is still the fixed light it always was
        assert_eq!(
            WindowStyle::Fixed.grid(false, [1, 1]),
            Grid::of_operation(OpeningKind::Window, Operation::Standard, 0.5, 2, 1, [1, 1])
        );
    }

    #[test]
    fn a_door_with_sidelights_and_a_transom_reads_back_as_its_parts() {
        let door = OpeningKind::Door;
        let core = Grid::of_operation(door, Operation::DoubleSwing, 0.67, 2, 1, [1, 1]);
        let a = Assembly {
            left: Some(0.2),
            transom: Some((0.25, Cell::default())),
            ..Assembly::of(core.clone())
        };
        let g = a.grid();
        assert_eq!((g.columns.len(), g.rows.len(), g.cells.len()), (3, 2, 6));
        assert!(g.columns[1].bar && !g.columns[2].bar && g.rows[1].bar);
        // the transom is one light over all three columns
        assert_eq!(g.cells[3].across, 3);
        assert!(!g.covered(0, 1) && g.covered(1, 1) && g.covered(2, 1));
        assert_eq!(g.fitted(), g);
        // no operation from before cells; read back, the same parts
        assert_eq!(g.operation(door), None);
        let back = g.assembly().unwrap();
        assert_eq!(back.columns, (1, 2));
        assert!((back.left.unwrap() - 0.2).abs() < 1e-12 && back.right.is_none());
        assert!((back.transom.unwrap().0 - 0.25).abs() < 1e-12);
        // the core is still the pair it was, with its share
        assert_eq!(back.core.operation(door), Some(Operation::DoubleSwing));
        assert!((back.core.active_share() - 0.67).abs() < 1e-12);
        assert_eq!(back.grid(), g);
        // a plain door is a core alone
        let plain = core.assembly().unwrap();
        assert!(!plain.has_lights() && plain.core == core);
        // a patio door's fixed panels are its own, not sidelights
        let patio = Grid::of_operation(door, Operation::MultiSlide, 0.5, 4, 0b1001, [1, 1]);
        assert_eq!(patio.assembly().unwrap().columns, (0, 3));
    }

    #[test]
    fn an_arch_springs_from_its_sides_and_rises_to_its_crown() {
        let close =
            |a: (f64, f64), b: (f64, f64)| (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9;
        let crown = |p: &[(f64, f64)]| p.iter().map(|q| q.1).fold(f64::MIN, f64::max);
        let round = Outline::Round.head(1.2, 2.4, 0.0);
        assert_eq!(round.len(), ARCH_PIECES + 1);
        assert!(close(round[0], (0.0, 1.8)) && close(round[ARCH_PIECES], (1.2, 1.8)));
        assert!((crown(&round) - 2.4).abs() < 1e-9);
        // its frame's inside meets the jambs' insides where it springs
        let inside = Outline::Round.head(1.2, 2.4, 0.06);
        assert!(close(inside[0], (0.06, 1.8)));
        assert!((crown(&inside) - 2.34).abs() < 1e-9);
        let seg = Outline::Segmental { rise: 0.3 }.head(1.2, 2.4, 0.0);
        assert!(close(seg[0], (0.0, 2.1)) && (crown(&seg) - 2.4).abs() < 1e-9);
        // raked, lower at its start
        assert_eq!(Outline::Raked { drop: -0.4 }.springs(1.0, 2.4), (2.0, 2.4));
        // too high a rise for the opening is cut down to fit
        assert_eq!(
            Outline::Round.fitted(3.0, 1.0),
            Outline::Segmental { rise: 0.9 }
        );
        assert_eq!(
            Outline::Segmental { rise: 0.6 }.fitted(1.2, 2.4),
            Outline::Round
        );
        assert_eq!(
            Outline::Raked { drop: 0.0 }.fitted(1.2, 2.4),
            Outline::Rectangle
        );
    }

    #[test]
    fn an_outline_is_saved_by_name_and_kept_round_an_assembly() {
        assert_eq!(
            serde_json::to_string(&Outline::Rectangle).unwrap(),
            "\"rectangle\""
        );
        let seg = Outline::Segmental { rise: 0.3 };
        let j = serde_json::to_string(&seg).unwrap();
        assert_eq!(j, r#"{"segmental":{"rise":0.3}}"#);
        assert_eq!(serde_json::from_str::<Outline>(&j).unwrap(), seg);
        // a door under a fanlight in a round arch reads back whole
        let mut g = Assembly {
            transom: Some((0.3, Cell::default())),
            ..Assembly::of(Grid::of_operation(
                OpeningKind::Door,
                Operation::DoubleSwing,
                0.5,
                0,
                0,
                [1, 1],
            ))
        }
        .grid();
        g.outline = Outline::Round;
        let a = g.assembly().unwrap();
        assert_eq!(a.outline, Outline::Round);
        assert!(a.core.outline.is_rectangle());
        assert_eq!(
            a.core.operation(OpeningKind::Door),
            Some(Operation::DoubleSwing)
        );
        assert_eq!(a.grid(), g);
        // an arched grid is no operation from before cells
        assert_eq!(g.operation(OpeningKind::Door), None);
    }

    #[test]
    fn a_window_of_sidelights_and_a_transom_round_a_casement_pair() {
        let core = WindowStyle::CasementPair.grid(false, [1, 1]);
        let a = Assembly {
            left: Some(0.25),
            right: Some(0.25),
            transom: Some((0.3, Cell::new(Mechanism::TopHung))),
            ..Assembly::of(core)
        };
        let g = a.grid();
        let back = g.assembly().unwrap();
        assert_eq!(back.columns, (1, 2));
        assert_eq!(WindowStyle::of(&back.core), Some(WindowStyle::CasementPair));
        assert_eq!(back.transom.unwrap().1.mechanism, Mechanism::TopHung);
        // two opening runs between mullions are no assembly
        let mut two = g.clone();
        two.cells[0].mechanism = Mechanism::Hinged;
        assert_eq!(two.assembly(), None);
    }

    #[test]
    fn a_grid_with_a_mullion_or_two_rows_is_no_old_operation() {
        let mut g =
            Grid::of_operation(OpeningKind::Door, Operation::DoubleSwing, 0.5, 2, 1, [1, 1]);
        g.columns[1].bar = true;
        assert_eq!(g.operation(OpeningKind::Door), None);
        let g = Grid {
            rows: vec![Track::new(1.0), Track::new(0.3)],
            ..Grid::default()
        }
        .fitted();
        assert_eq!(g.cells.len(), 2);
        assert_eq!(g.operation(OpeningKind::Window), None);
    }
}
