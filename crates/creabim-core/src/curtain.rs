//! Curtain walls: a wall made of a grid of mullions holding glass or solid
//! panels, as Revit's curtain walls or ArchiCAD's curtain wall tool.
//!
//! A curtain wall is an ordinary [`Wall`] with a [`CurtainGrid`], so it is
//! drawn with the Wall tool, joins, moves, has a level and a height like
//! any wall. Its grid lines run up the wall every so far along it (or
//! split it into so many bays) and across it every so far up. Mullions sit
//! on every grid line and round the edge; panels fill the cells between.
//! A door or bare hole hosted in a curtain wall takes the place of the
//! panels it covers, and the mullions through it are left out.
//!
//! This module lays the grid out in the wall's own coordinates (`u` along
//! the wall from its start, `v` up from its base); `creabim-geom` turns the
//! layout into meshes and plans, `creabim-ifc` into IfcMember and IfcPlate.
//!
//! A curtain wall drawn in, or moved into, another wall cuts its own
//! opening there, as a door or window does ([`voids_in`]): the host wall
//! loses the curtain wall's length and height, wherever the curtain wall
//! goes. Nothing is stored for it, so the void always follows.

use crate::{ElementId, ElementKind, Model, Opening, OpeningKind, Wall};
use serde::{Deserialize, Serialize};

/// How a curtain grid divides one direction.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum GridRule {
    /// A grid line every so many metres, from the start (or the base); the
    /// last bay takes what is left.
    Spacing(f64),
    /// So many equal bays.
    Count(u32),
    /// The fewest equal bays no wider than this (curtain grids mockup,
    /// decision 1).
    Fit(f64),
}

/// Where a spacing is set out from: where the odd bay goes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SetOut {
    /// From the start (or the base): the odd bay at the end.
    #[default]
    Start,
    /// A line on the middle: odd bays at both ends, the same.
    Centre,
    /// From the end (or the top): the odd bay at the start.
    End,
}

impl SetOut {
    pub const ALL: [SetOut; 3] = [SetOut::Start, SetOut::Centre, SetOut::End];

    pub fn label(self) -> &'static str {
        match self {
            SetOut::Start => "Start",
            SetOut::Centre => "Centre",
            SetOut::End => "End",
        }
    }
}

/// Grid lines set by hand in one direction, kept over the rule: lines
/// added, and lines of the rule taken away (a moved line is both).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HandLines {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub added: Vec<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed: Vec<f64>,
}

impl HandLines {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty()
    }
}

/// One segment of a grid line taken out, merging the two panels either
/// side: the line at `at` (along the wall for an upright, up it for a
/// transom), in the bay or row that holds `mid` (the other way).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Gap {
    /// An upright's segment (true) or a transom's.
    pub upright: bool,
    pub at: f64,
    pub mid: f64,
}

/// Lines the building puts on a curtain wall: transoms at the floors,
/// uprights on the structural grid (in its own coordinates).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Forced {
    pub us: Vec<f64>,
    pub vs: Vec<f64>,
}

/// Two grid lines this close are one.
const SAME_LINE: f64 = 0.001;

fn true_() -> bool {
    true
}

fn is_true(v: &bool) -> bool {
    *v
}

impl GridRule {
    /// The grid lines strictly inside `0..len`.
    pub fn lines(&self, len: f64) -> Vec<f64> {
        self.lines_from(len, SetOut::Start)
    }

    /// The grid lines strictly inside `0..len`, a spacing set out `from`
    /// the start, the middle or the end.
    pub fn lines_from(&self, len: f64, from: SetOut) -> Vec<f64> {
        match (*self, from) {
            (GridRule::Spacing(s), SetOut::End) if s > 1e-3 => {
                let mut out: Vec<f64> = GridRule::Spacing(s)
                    .lines(len)
                    .into_iter()
                    .map(|x| len - x)
                    .collect();
                out.reverse();
                return out;
            }
            (GridRule::Spacing(s), SetOut::Centre) if s > 1e-3 => {
                let c = len / 2.0;
                let k = ((c - 0.05) / s).floor().max(0.0) as i64;
                return (-k..=k.min(250))
                    .map(|i| c + i as f64 * s)
                    .filter(|x| *x > 0.05 && *x < len - 0.05)
                    .collect();
            }
            _ => {}
        }
        match *self {
            GridRule::Fit(m) if m > 1e-3 => {
                let n = ((len / m) - 1e-9).ceil().clamp(1.0, 200.0) as u32;
                GridRule::Count(n).lines(len)
            }
            GridRule::Fit(_) => vec![],
            GridRule::Spacing(s) if s > 1e-3 => {
                let mut out = vec![];
                let mut at = s;
                // a bay thinner than 5 cm at the end joins the one before
                while at < len - 0.05 && out.len() < 500 {
                    out.push(at);
                    at += s;
                }
                out
            }
            GridRule::Spacing(_) => vec![],
            GridRule::Count(n) => {
                let n = n.clamp(1, 200);
                (1..n).map(|i| len * i as f64 / n as f64).collect()
            }
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            GridRule::Spacing(_) => "Spacing",
            GridRule::Count(_) => "Count",
            GridRule::Fit(_) => "Fit",
        }
    }
}

/// What fills a curtain wall's cells.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PanelKind {
    #[default]
    Glass,
    /// An opaque panel, in the wall's material.
    Solid,
}

impl PanelKind {
    pub const ALL: [PanelKind; 2] = [PanelKind::Glass, PanelKind::Solid];

    pub fn label(self) -> &'static str {
        match self {
            PanelKind::Glass => "Glass",
            PanelKind::Solid => "Solid",
        }
    }
}

/// How a curtain wall's mullions are made: one swap from a quick block
/// to a built system (curtain mullions mockup, decision 1). The outside is
/// the wall's exterior face (its right).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MullionProfile {
    /// A plain block, the glass in the middle: for quick ideas.
    #[default]
    Box,
    /// A stick system: a body behind the glass, the glass held near the
    /// outside, a pressure plate and a snap cap proud of it.
    Stick,
    /// Flush glazing: the body behind, the glass flush outside, a sealed
    /// joint between panes and no cap.
    Flush,
}

impl MullionProfile {
    pub const ALL: [MullionProfile; 3] = [
        MullionProfile::Box,
        MullionProfile::Stick,
        MullionProfile::Flush,
    ];

    pub fn label(self) -> &'static str {
        match self {
            MullionProfile::Box => "Box",
            MullionProfile::Stick => "Stick",
            MullionProfile::Flush => "Flush",
        }
    }

    pub fn tip(self) -> &'static str {
        match self {
            MullionProfile::Box => {
                "A plain block with the glass in the middle: quick to read, for ideas"
            }
            MullionProfile::Stick => "A stick system: body behind the glass, a snap cap outside",
            MullionProfile::Flush => {
                "Flush glazing: glass flush outside, sealed joints, the frame inside"
            }
        }
    }
}

/// The grid, mullions and panels of a curtain wall.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CurtainGrid {
    /// Grid lines running up the wall, placed along it.
    pub vertical: GridRule,
    /// Grid lines running along the wall, placed up it.
    pub horizontal: GridRule,
    /// Face width of the mullions, seen from the front.
    pub mullion_width: f64,
    /// Depth of the mullions through the wall: the wall's thickness.
    pub mullion_depth: f64,
    #[serde(default)]
    pub panel: PanelKind,
    #[serde(default = "default_panel_thickness")]
    pub panel_thickness: f64,
    /// Cells that take the other kind of panel, as (bay along, row up).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub swapped: Vec<(u32, u32)>,
    /// How the mullions are made.
    #[serde(default, skip_serializing_if = "is_box")]
    pub profile: MullionProfile,
    /// A stick system's snap cap: how far it stands proud of the glass
    /// line, outside.
    #[serde(default = "default_cap", skip_serializing_if = "is_default_cap")]
    pub cap: f64,
    /// Flush glazing: the sealed joint between panes, seen from outside.
    #[serde(default = "default_joint", skip_serializing_if = "is_default_joint")]
    pub joint: f64,
    /// Transoms (the horizontal mullions) shallower than the mullions,
    /// from the outside face: `None` keeps them as deep.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transom_depth: Option<f64>,
    /// Where a spacing along and up is set out from.
    #[serde(default, skip_serializing_if = "is_start")]
    pub vertical_from: SetOut,
    #[serde(default, skip_serializing_if = "is_start")]
    pub horizontal_from: SetOut,
    /// Uprights and transoms set by hand, kept over the rules.
    #[serde(default, skip_serializing_if = "HandLines::is_empty")]
    pub vertical_hand: HandLines,
    #[serde(default, skip_serializing_if = "HandLines::is_empty")]
    pub horizontal_hand: HandLines,
    /// Segments taken out, merging panels.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gaps: Vec<Gap>,
    /// A transom where each floor meets it (decision 3).
    #[serde(default = "true_", skip_serializing_if = "is_true")]
    pub transoms_at_levels: bool,
    /// An upright wherever a structural grid line crosses it.
    #[serde(default = "true_", skip_serializing_if = "is_true")]
    pub mullions_on_axes: bool,
}

fn is_start(f: &SetOut) -> bool {
    *f == SetOut::Start
}

fn default_panel_thickness() -> f64 {
    0.024
}

fn is_box(p: &MullionProfile) -> bool {
    *p == MullionProfile::Box
}

fn default_cap() -> f64 {
    0.02
}

fn is_default_cap(v: &f64) -> bool {
    (*v - default_cap()).abs() < 1e-12
}

fn default_joint() -> f64 {
    0.02
}

fn is_default_joint(v: &f64) -> bool {
    (*v - default_joint()).abs() < 1e-12
}

impl Default for CurtainGrid {
    fn default() -> Self {
        Self {
            vertical: GridRule::Spacing(1.5),
            horizontal: GridRule::Spacing(1.5),
            mullion_width: 0.06,
            mullion_depth: 0.15,
            panel: PanelKind::Glass,
            panel_thickness: default_panel_thickness(),
            swapped: vec![],
            profile: MullionProfile::Box,
            cap: default_cap(),
            joint: default_joint(),
            transom_depth: None,
            vertical_from: SetOut::Start,
            horizontal_from: SetOut::Start,
            vertical_hand: HandLines::default(),
            horizontal_hand: HandLines::default(),
            gaps: vec![],
            transoms_at_levels: true,
            mullions_on_axes: true,
        }
    }
}

/// Which way a mullion runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MullionRun {
    Vertical,
    Horizontal,
}

/// A rectangle on a curtain wall's face, in its own coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FaceRect {
    pub u0: f64,
    pub u1: f64,
    pub v0: f64,
    pub v1: f64,
}

impl FaceRect {
    fn overlaps(&self, o: &FaceRect) -> bool {
        self.u0 < o.u1 - 1e-6
            && o.u0 < self.u1 - 1e-6
            && self.v0 < o.v1 - 1e-6
            && o.v0 < self.v1 - 1e-6
    }
}

/// One mullion: a box `rect` on the face, `depth` through the wall.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mullion {
    pub run: MullionRun,
    pub rect: FaceRect,
}

/// One panel filling a cell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Panel {
    pub bay: u32,
    pub row: u32,
    pub kind: PanelKind,
    pub rect: FaceRect,
}

/// A curtain wall laid out: its grid lines, mullions and panels.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Layout {
    /// Grid lines along the wall, ends included.
    pub us: Vec<f64>,
    /// Grid lines up the wall, base and top included.
    pub vs: Vec<f64>,
    pub mullions: Vec<Mullion>,
    pub panels: Vec<Panel>,
}

impl CurtainGrid {
    pub fn validate(&self) -> Result<(), String> {
        let ok = |v: f64| v.is_finite() && v > 0.0;
        if !(ok(self.mullion_width) && ok(self.mullion_depth) && ok(self.panel_thickness)) {
            return Err("curtain wall mullions and panels need a positive size".into());
        }
        if !(ok(self.cap) && ok(self.joint) && self.transom_depth.is_none_or(ok)) {
            return Err("curtain wall caps, joints and transoms need a positive size".into());
        }
        for r in [self.vertical, self.horizontal] {
            if let GridRule::Spacing(s) | GridRule::Fit(s) = r {
                if !(s.is_finite() && s >= 0.05) {
                    return Err("curtain grid spacing must be at least 50 mm".into());
                }
            }
        }
        Ok(())
    }

    /// The panel kind of a cell, its swap applied.
    pub fn panel_of(&self, bay: u32, row: u32) -> PanelKind {
        let swapped = self.swapped.contains(&(bay, row));
        match (self.panel, swapped) {
            (PanelKind::Glass, true) => PanelKind::Solid,
            (PanelKind::Solid, true) => PanelKind::Glass,
            (k, false) => k,
        }
    }

    /// The grid laid out on a wall `len` long and `height` high, leaving
    /// room for `holes` (doors and bare openings, in the same coordinates):
    /// no mullion through one, and the cell round one filled to it (glass
    /// over a door, on a transom at its head).
    pub fn layout(&self, len: f64, height: f64, holes: &[FaceRect]) -> Layout {
        self.layout_with(len, height, holes, &Forced::default())
    }

    /// The grid lines one way: the rule's, those the building puts there
    /// (`forced`, taking the place of any rule line near them), then the
    /// ones set by hand. Ends included.
    pub fn grid_lines(&self, upright: bool, len: f64, forced: &[f64]) -> Vec<f64> {
        let (rule, from, hand, on) = if upright {
            (
                self.vertical,
                self.vertical_from,
                &self.vertical_hand,
                self.mullions_on_axes,
            )
        } else {
            (
                self.horizontal,
                self.horizontal_from,
                &self.horizontal_hand,
                self.transoms_at_levels,
            )
        };
        let mut xs = rule.lines_from(len, from);
        if on {
            // a forced line takes the place of a rule line close to it
            let near = (len / (xs.len() + 1) as f64 / 3.0).min(0.3);
            for f in forced.iter().filter(|f| **f > 0.05 && **f < len - 0.05) {
                xs.retain(|x| (x - f).abs() > near);
                xs.push(*f);
            }
        }
        xs.retain(|x| !hand.removed.iter().any(|r| (x - r).abs() < SAME_LINE));
        xs.extend(hand.added.iter().copied());
        xs.retain(|x| *x > 0.02 && *x < len - 0.02);
        xs.sort_by(f64::total_cmp);
        xs.dedup_by(|a, b| (*a - *b).abs() < SAME_LINE);
        let mut out = vec![0.0];
        out.extend(xs);
        out.push(len);
        out
    }

    /// [`CurtainGrid::layout`] with the lines the building puts there.
    pub fn layout_with(
        &self,
        len: f64,
        height: f64,
        holes: &[FaceRect],
        forced: &Forced,
    ) -> Layout {
        let us = self.grid_lines(true, len, &forced.us);
        let vs = self.grid_lines(false, height, &forced.vs);
        let mw = self.mullion_width;
        let half = mw / 2.0;
        // edge mullions sit inside the wall's outline
        let span = |xs: &[f64], i: usize, end: f64| -> (f64, f64) {
            let x = xs[i];
            if i == 0 {
                (0.0, mw.min(end))
            } else if i == xs.len() - 1 {
                ((end - mw).max(0.0), end)
            } else {
                (x - half, x + half)
            }
        };
        let (nb, nr) = (us.len() - 1, vs.len() - 1);
        // the cells between the mullions
        let cell = |bay: usize, row: usize| FaceRect {
            u0: span(&us, bay, len).1,
            u1: span(&us, bay + 1, len).0,
            v0: span(&vs, row, height).1,
            v1: span(&vs, row + 1, height).0,
        };
        let line_at =
            |xs: &[f64], at: f64| (1..xs.len() - 1).find(|i| (xs[*i] - at).abs() < SAME_LINE);
        let row_of =
            |xs: &[f64], mid: f64| (0..xs.len() - 1).find(|i| mid >= xs[*i] && mid <= xs[i + 1]);
        // segments taken out: (upright?, line index, the bay or row)
        let mut out_segs: Vec<(bool, usize, usize)> = vec![];
        for g in &self.gaps {
            let (lines, across) = if g.upright { (&us, &vs) } else { (&vs, &us) };
            if let (Some(i), Some(k)) = (line_at(lines, g.at), row_of(across, g.mid)) {
                out_segs.push((g.upright, i, k));
            }
        }
        let has_gap = |upright: bool, i: usize, k: usize| out_segs.contains(&(upright, i, k));
        let mut mullions = vec![];
        // a mullion piece, less what holes take out of it
        let push = |mullions: &mut Vec<Mullion>, run: MullionRun, r: FaceRect, cut: &[FaceRect]| {
            let mut pieces = vec![r];
            for h in holes.iter().chain(cut) {
                let mut next = vec![];
                for p in pieces {
                    if !p.overlaps(h) {
                        next.push(p);
                        continue;
                    }
                    match run {
                        MullionRun::Vertical => {
                            if h.v0 > p.v0 + 1e-6 {
                                next.push(FaceRect { v1: h.v0, ..p });
                            }
                            if h.v1 < p.v1 - 1e-6 {
                                next.push(FaceRect { v0: h.v1, ..p });
                            }
                        }
                        MullionRun::Horizontal => {
                            if h.u0 > p.u0 + 1e-6 {
                                next.push(FaceRect { u1: h.u0, ..p });
                            }
                            if h.u1 < p.u1 - 1e-6 {
                                next.push(FaceRect { u0: h.u1, ..p });
                            }
                        }
                    }
                }
                pieces = next;
            }
            for p in pieces {
                if p.u1 - p.u0 > 1e-6 && p.v1 - p.v0 > 1e-6 {
                    mullions.push(Mullion { run, rect: p });
                }
            }
        };
        for i in 0..us.len() {
            let (u0, u1) = span(&us, i, len);
            // its segments taken out, row by row
            let cut: Vec<FaceRect> = (0..nr)
                .filter(|k| has_gap(true, i, *k))
                .map(|k| {
                    let c = cell(0, k);
                    FaceRect {
                        u0,
                        u1,
                        v0: c.v0,
                        v1: c.v1,
                    }
                })
                .collect();
            push(
                &mut mullions,
                MullionRun::Vertical,
                FaceRect {
                    u0,
                    u1,
                    v0: 0.0,
                    v1: height,
                },
                &cut,
            );
        }
        // horizontal mullions run between the vertical ones
        for bay in 0..nb {
            let c = cell(bay, 0);
            if c.u1 <= c.u0 + 1e-6 {
                continue;
            }
            for j in 0..vs.len() {
                if has_gap(false, j, bay) {
                    continue;
                }
                let (v0, v1) = span(&vs, j, height);
                push(
                    &mut mullions,
                    MullionRun::Horizontal,
                    FaceRect {
                        u0: c.u0,
                        u1: c.u1,
                        v0,
                        v1,
                    },
                    &[],
                );
            }
        }
        // cells merged across the segments taken out
        let mut group: Vec<usize> = (0..nb * nr).collect();
        fn root(g: &mut [usize], mut i: usize) -> usize {
            while g[i] != i {
                g[i] = g[g[i]];
                i = g[i];
            }
            i
        }
        for &(upright, i, k) in &out_segs {
            let (a, b) = if upright {
                (k * nb + i - 1, k * nb + i)
            } else {
                ((i - 1) * nb + k, i * nb + k)
            };
            if a < nb * nr && b < nb * nr {
                let (ra, rb) = (root(&mut group, a), root(&mut group, b));
                group[ra.max(rb)] = ra.min(rb);
            }
        }
        let mut panels = vec![];
        let mut done = vec![false; nb * nr];
        for i in 0..nb * nr {
            if done[i] {
                continue;
            }
            let r = root(&mut group, i);
            let members: Vec<usize> = (0..nb * nr).filter(|j| root(&mut group, *j) == r).collect();
            let b0 = members.iter().map(|j| j % nb).min().unwrap_or(0);
            let b1 = members.iter().map(|j| j % nb).max().unwrap_or(0);
            let r0 = members.iter().map(|j| j / nb).min().unwrap_or(0);
            let r1 = members.iter().map(|j| j / nb).max().unwrap_or(0);
            let whole = FaceRect {
                u0: cell(b0, 0).u0,
                u1: cell(b1, 0).u1,
                v0: cell(0, r0).v0,
                v1: cell(0, r1).v1,
            };
            // merged when they make a rectangle with no hole in it
            let merged = members.len() > 1
                && members.len() == (b1 - b0 + 1) * (r1 - r0 + 1)
                && !holes.iter().any(|h| h.overlaps(&whole));
            if merged {
                for j in &members {
                    done[*j] = true;
                }
                panels.push(Panel {
                    bay: b0 as u32,
                    row: r0 as u32,
                    kind: self.panel_of(b0 as u32, r0 as u32),
                    rect: whole,
                });
                continue;
            }
            for j in members {
                if done[j] {
                    continue;
                }
                done[j] = true;
                let (bay, row) = (j % nb, j / nb);
                let c = cell(bay, row);
                if c.u1 > c.u0 + 1e-6 && c.v1 > c.v0 + 1e-6 {
                    self.fill_cell(bay, row, c, holes, &mut mullions, &mut panels);
                }
            }
        }
        Layout {
            us,
            vs,
            mullions,
            panels,
        }
    }

    /// A cell's panel, or round one hole in it: glass over the hole on a
    /// transom at its head, under it on one at its sill, and beside it on
    /// uprights at its sides. A cell with several holes is left open.
    fn fill_cell(
        &self,
        bay: usize,
        row: usize,
        rect: FaceRect,
        holes: &[FaceRect],
        mullions: &mut Vec<Mullion>,
        panels: &mut Vec<Panel>,
    ) {
        let kind = self.panel_of(bay as u32, row as u32);
        let mk = |r: FaceRect| Panel {
            bay: bay as u32,
            row: row as u32,
            kind,
            rect: r,
        };
        let mw = self.mullion_width;
        let min = 0.05;
        let inside: Vec<&FaceRect> = holes.iter().filter(|h| h.overlaps(&rect)).collect();
        match inside.as_slice() {
            [] => panels.push(mk(rect)),
            [h] => {
                let (hv0, hv1) = (h.v0.max(rect.v0), h.v1.min(rect.v1));
                if rect.v1 - (h.v1 + mw) > min {
                    mullions.push(Mullion {
                        run: MullionRun::Horizontal,
                        rect: FaceRect {
                            v0: h.v1,
                            v1: h.v1 + mw,
                            ..rect
                        },
                    });
                    panels.push(mk(FaceRect {
                        v0: h.v1 + mw,
                        ..rect
                    }));
                }
                if (h.v0 - mw) - rect.v0 > min {
                    mullions.push(Mullion {
                        run: MullionRun::Horizontal,
                        rect: FaceRect {
                            v0: h.v0 - mw,
                            v1: h.v0,
                            ..rect
                        },
                    });
                    panels.push(mk(FaceRect {
                        v1: h.v0 - mw,
                        ..rect
                    }));
                }
                if (h.u0 - mw) - rect.u0 > min && hv1 > hv0 {
                    mullions.push(Mullion {
                        run: MullionRun::Vertical,
                        rect: FaceRect {
                            u0: h.u0 - mw,
                            u1: h.u0,
                            v0: hv0,
                            v1: hv1,
                        },
                    });
                    panels.push(mk(FaceRect {
                        u1: h.u0 - mw,
                        v0: hv0,
                        v1: hv1,
                        ..rect
                    }));
                }
                if rect.u1 - (h.u1 + mw) > min && hv1 > hv0 {
                    mullions.push(Mullion {
                        run: MullionRun::Vertical,
                        rect: FaceRect {
                            u0: h.u1,
                            u1: h.u1 + mw,
                            v0: hv0,
                            v1: hv1,
                        },
                    });
                    panels.push(mk(FaceRect {
                        u0: h.u1 + mw,
                        v0: hv0,
                        v1: hv1,
                        ..rect
                    }));
                }
            }
            _ => {}
        }
    }

    /// The layout of a curtain wall, with its doors and holes.
    pub fn wall_layout(&self, wall: &Wall, openings: &[&Opening]) -> Layout {
        self.layout(wall.length(), wall.height, &holes_of(openings))
    }
}

/// Doors and holes as rectangles on a curtain wall's face.
pub fn holes_of(openings: &[&Opening]) -> Vec<FaceRect> {
    openings
        .iter()
        .map(|o| FaceRect {
            u0: o.offset,
            u1: o.offset + o.width,
            v0: o.sill,
            v1: o.sill + o.height,
        })
        .collect()
}

/// The lines the building puts on curtain wall `wall`, `height` high: a
/// transom at each level between its base and top, an upright where a
/// straight grid line crosses it (a straight curtain wall).
pub fn forced_lines(model: &Model, wall: &Wall, height: f64) -> Forced {
    let mut f = Forced::default();
    let base = crate::walls::base_z(model, wall);
    for e in model.iter() {
        match &e.kind {
            ElementKind::Level(l) => {
                let v = l.elevation - base;
                if v > 0.3 && v < height - 0.3 {
                    f.vs.push(v);
                }
            }
            ElementKind::Grid(g) if !wall.is_arc() => {
                if let crate::sketch::Curve::Line(a, b) = crate::grids::curve(g) {
                    let (p, d) = (wall.start, wall.end - wall.start);
                    let e2 = b - a;
                    let den = d.perp_dot(e2);
                    if den.abs() < 1e-9 {
                        continue;
                    }
                    let t = (a - p).perp_dot(e2) / den;
                    let s = (a - p).perp_dot(d) / den;
                    if (0.0..=1.0).contains(&t) && (-1e-9..=1.0 + 1e-9).contains(&s) {
                        f.us.push(t * wall.length());
                    }
                }
            }
            _ => {}
        }
    }
    f
}

/// How far off a host wall's direction a curtain wall may run and still
/// sit in it (the sine of about one degree).
const PARALLEL: f64 = 0.0175;

/// The opening curtain wall `cw` cuts in wall `host`, if it sits in it:
/// running along it, its centreline within the host's thickness, and
/// overlapping it along its length and up its height. The opening is
/// where they overlap, in the host's own coordinates.
/// [`void_of`] for curved walls: only a curtain wall on the same arc as
/// its host sits in it (the other half of a circle shares the host's chord
/// line, and cut it through); the hole runs round the arc.
fn arc_void(model: &Model, host_id: ElementId, host: &Wall, cw: &Wall) -> Option<Opening> {
    use crate::sketch::Curve;
    let (
        Curve::Arc {
            center: c1,
            radius: r1,
            ..
        },
        Curve::Arc {
            center: c2,
            radius: r2,
            ..
        },
    ) = (host.curve(), cw.curve())
    else {
        return None;
    };
    if c1.distance(c2) > 1e-3 || (r1 - r2).abs() > host.thickness / 2.0 + 1e-3 {
        return None;
    }
    // its middle on the host's stretch of the circle, not past its ends
    let mid = cw.place(cw.length() / 2.0, 0.0);
    if host.along(mid).1 > host.thickness / 2.0 + 1e-3 {
        return None;
    }
    let (a, b) = (host.along(cw.start).0, host.along(cw.end).0);
    let (u0, u1) = (a.min(b), a.max(b));
    let (h0, c0) = (
        crate::walls::base_z(model, host),
        crate::walls::base_z(model, cw),
    );
    let v0 = c0.max(h0) - h0;
    let v1 = void_top(model, host, cw, c0, h0);
    if u1 - u0 < 0.05 || v1 - v0 < 0.05 {
        return None;
    }
    Some(Opening {
        host: host_id,
        kind: OpeningKind::Hole,
        offset: u0,
        sill: v0,
        width: u1 - u0,
        height: v1 - v0,
        material: None,
        frame: crate::OpeningFrame::for_kind(OpeningKind::Hole),
        flip_hand: false,
        flip_facing: false,
        roof_at: None,
        roof_build: Default::default(),
        arrangement: None,
    })
}

/// How high the hole curtain wall `cw` cuts in `host` runs, over the
/// host's base `h0` (`c0` is the curtain wall's base). A curtain wall
/// attached to the roof opens the host up to the roof; one that is not
/// leaves the host closed over it, even where the host's own top follows
/// a roof (else the hole ran on up to the roof).
fn void_top(model: &Model, host: &Wall, cw: &Wall, c0: f64, h0: f64) -> f64 {
    let full = host.height;
    let own = c0 + cw.height - h0;
    if crate::walls::top_profile(model, cw).is_some() {
        return full;
    }
    if crate::walls::top_profile(model, host).is_some() && own >= full - 1e-6 {
        return full - 0.001;
    }
    own.min(full)
}

pub fn void_of(model: &Model, host_id: ElementId, host: &Wall, cw: &Wall) -> Option<Opening> {
    if cw.curtain.is_none() || host.curtain.is_some() || host.length() < 1e-6 {
        return None;
    }
    if host.is_arc() || cw.is_arc() {
        return arc_void(model, host_id, host, cw);
    }
    let d = host.direction();
    let e = cw.direction();
    if d.perp_dot(e).abs() > PARALLEL {
        return None;
    }
    let mid = (cw.start + cw.end) / 2.0;
    let off = (mid - host.start).perp_dot(d).abs();
    if off > host.thickness / 2.0 + 1e-3 {
        return None;
    }
    let along = |p: glam::DVec2| (p - host.start).dot(d);
    let (a, b) = (along(cw.start), along(cw.end));
    let u0 = a.min(b).max(0.0);
    let u1 = a.max(b).min(host.length());
    let (h0, c0) = (
        crate::walls::base_z(model, host),
        crate::walls::base_z(model, cw),
    );
    let v0 = c0.max(h0) - h0;
    let v1 = void_top(model, host, cw, c0, h0);
    if u1 - u0 < 0.05 || v1 - v0 < 0.05 {
        return None;
    }
    Some(Opening {
        host: host_id,
        kind: OpeningKind::Hole,
        offset: u0,
        sill: v0,
        width: u1 - u0,
        height: v1 - v0,
        material: None,
        frame: crate::OpeningFrame::for_kind(OpeningKind::Hole),
        flip_hand: false,
        flip_facing: false,
        roof_at: None,
        roof_build: Default::default(),
        arrangement: None,
    })
}

/// The openings the curtain walls sitting in wall `host` cut in it, each
/// with the curtain wall that fills it.
pub fn voids_in(model: &Model, host: ElementId) -> Vec<(ElementId, Opening)> {
    let Some(ElementKind::Wall(h)) = model.get(host).map(|e| &e.kind) else {
        return vec![];
    };
    if h.curtain.is_some() {
        return vec![];
    }
    model
        .iter()
        .filter(|e| e.id != host)
        .filter_map(|e| match &e.kind {
            ElementKind::Wall(cw) => Some((e.id, void_of(model, host, h, cw)?)),
            _ => None,
        })
        .collect()
}

/// The walls curtain wall `cw` sits in, each with the opening it cuts.
pub fn hosts_of(model: &Model, cw: ElementId) -> Vec<(ElementId, Opening)> {
    let Some(ElementKind::Wall(c)) = model.get(cw).map(|e| &e.kind) else {
        return vec![];
    };
    if c.curtain.is_none() {
        return vec![];
    }
    model
        .iter()
        .filter(|e| e.id != cw)
        .filter_map(|e| match &e.kind {
            ElementKind::Wall(h) => Some((e.id, void_of(model, e.id, h, c)?)),
            _ => None,
        })
        .collect()
}

/// Every opening through wall `id` as built: its doors, windows and holes,
/// then the voids the curtain walls in it cut.
pub fn wall_openings(model: &Model, id: ElementId) -> Vec<Opening> {
    let mut out: Vec<Opening> = model
        .dependents(id)
        .into_iter()
        .filter_map(|d| match &model.get(d)?.kind {
            ElementKind::Opening(o) => Some(o.clone()),
            _ => None,
        })
        .collect();
    out.extend(voids_in(model, id).into_iter().map(|(_, o)| o));
    // a screen wall's holes
    out.extend(crate::screen::screen_openings(model, id));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_curtain_half_of_a_circle_cuts_nothing_from_the_solid_half() {
        use crate::{Element, Level};
        use glam::DVec2;
        let lv = Element::new(
            "L0",
            ElementKind::Level(Level {
                elevation: 0.0,
                height: 3.0,
            }),
        );
        let (a, b) = (DVec2::new(-3.0, 0.0), DVec2::new(3.0, 0.0));
        let mut solid = Wall::new(lv.id, a, b, 0.3, 3.0);
        solid.bulge = 1.0;
        let mut glass = Wall::new(lv.id, b, a, 0.15, 3.0);
        glass.bulge = 1.0;
        glass.curtain = Some(CurtainGrid::default());
        let (s, g) = (
            Element::new("Wall", ElementKind::Wall(solid)),
            Element::new("Glass", ElementKind::Wall(glass)),
        );
        let sid = s.id;
        let m = Model::from_elements([lv, s, g]).unwrap();
        assert!(voids_in(&m, sid).is_empty());
    }

    /// A level with a 10 m wall along x and a 3 m curtain wall at `at`
    /// along it, `off` to its side.
    fn wall_and_curtain(at: f64, off: f64) -> (Model, ElementId, ElementId) {
        use crate::{Element, Level};
        use glam::DVec2;
        let lv = Element::new(
            "L0",
            ElementKind::Level(Level {
                elevation: 0.0,
                height: 3.0,
            }),
        );
        let host = Element::new(
            "Wall",
            ElementKind::Wall(Wall::new(
                lv.id,
                DVec2::ZERO,
                DVec2::new(10.0, 0.0),
                0.3,
                3.0,
            )),
        );
        let mut w = Wall::new(
            lv.id,
            DVec2::new(at, off),
            DVec2::new(at + 3.0, off),
            0.15,
            2.5,
        );
        w.curtain = Some(CurtainGrid::default());
        let cw = Element::new("Storefront", ElementKind::Wall(w));
        let (h, c) = (host.id, cw.id);
        (Model::from_elements([lv, host, cw]).unwrap(), h, c)
    }

    #[test]
    fn a_curtain_wall_in_a_wall_cuts_its_own_opening_which_follows_it() {
        let (m, host, cw) = wall_and_curtain(2.0, 0.05);
        let v = voids_in(&m, host);
        assert_eq!(v.len(), 1);
        let (filler, o) = &v[0];
        assert_eq!(*filler, cw);
        assert!((o.offset - 2.0).abs() < 1e-9 && (o.width - 3.0).abs() < 1e-9);
        assert!(o.sill.abs() < 1e-9 && (o.height - 2.5).abs() < 1e-9);
        assert_eq!(hosts_of(&m, cw).len(), 1);
        assert_eq!(wall_openings(&m, host).len(), 1);
        // moved along, the opening goes with it; past the end it is cut short
        let (m, host, _) = wall_and_curtain(8.5, 0.0);
        let o = &voids_in(&m, host)[0].1;
        assert!((o.offset - 8.5).abs() < 1e-9 && (o.width - 1.5).abs() < 1e-9);
        // out of the wall, or beside it, it stands on its own
        let (m, host, cw) = wall_and_curtain(2.0, 0.6);
        assert!(voids_in(&m, host).is_empty() && hosts_of(&m, cw).is_empty());
        let (m, host, _) = wall_and_curtain(12.0, 0.0);
        assert!(voids_in(&m, host).is_empty());
        // a curtain wall hosts no curtain-wall voids itself
        let (m, _, cw) = wall_and_curtain(2.0, 0.0);
        assert!(voids_in(&m, cw).is_empty());
    }

    #[test]
    fn spacing_and_count_place_grid_lines() {
        assert_eq!(GridRule::Spacing(1.5).lines(4.0), vec![1.5, 3.0]);
        // a last bay of 3 cm joins the one before
        assert_eq!(GridRule::Spacing(1.0).lines(3.03), vec![1.0, 2.0]);
        assert_eq!(GridRule::Count(4).lines(4.0), vec![1.0, 2.0, 3.0]);
        assert!(GridRule::Count(1).lines(4.0).is_empty());
    }

    #[test]
    fn a_grid_has_mullions_round_and_through_and_a_panel_per_cell() {
        let g = CurtainGrid {
            vertical: GridRule::Count(3),
            horizontal: GridRule::Count(2),
            ..Default::default()
        };
        let l = g.layout(3.0, 3.0, &[]);
        assert_eq!(l.panels.len(), 6);
        // 4 uprights, and 3 rows of transoms in each of 3 bays
        assert_eq!(l.mullions.len(), 4 + 9);
        let p = l.panels[0].rect;
        assert!((p.u0 - 0.06).abs() < 1e-9 && (p.u1 - 0.97).abs() < 1e-9);
        assert!(l.panels.iter().all(|p| p.kind == PanelKind::Glass));
    }

    #[test]
    fn fit_and_set_out_place_the_odd_bay_where_asked() {
        // the fewest equal bays no wider than 1.5 m on 7.2 m: five of 1.44
        let fit = GridRule::Fit(1.5).lines(7.2);
        assert_eq!(fit.len(), 4);
        assert!((fit[0] - 1.44).abs() < 1e-9);
        // 1.5 m from the end: the odd 0.3 m bay at the start
        let end = GridRule::Spacing(1.5).lines_from(7.2, SetOut::End);
        assert!((end[0] - 1.2).abs() < 1e-9 && (end[end.len() - 1] - 5.7).abs() < 1e-9);
        // a line on the middle: the same odd bay at both ends
        let c = GridRule::Spacing(1.5).lines_from(7.2, SetOut::Centre);
        assert!(c.iter().any(|x| (x - 3.6).abs() < 1e-9));
        assert!((c[0] - (7.2 - c[c.len() - 1])).abs() < 1e-9);
    }

    #[test]
    fn lines_by_hand_and_from_the_building_are_kept_over_the_rule() {
        let mut g = CurtainGrid {
            vertical: GridRule::Count(4),
            horizontal: GridRule::Count(2),
            ..Default::default()
        };
        // a floor at 3.0 m takes the place of the rule's line near it
        let f = Forced {
            us: vec![],
            vs: vec![2.9],
        };
        let vs = g.grid_lines(false, 6.0, &f.vs);
        assert_eq!(vs, vec![0.0, 2.9, 6.0]);
        // off, the rule's line stands
        g.transoms_at_levels = false;
        assert_eq!(g.grid_lines(false, 6.0, &f.vs), vec![0.0, 3.0, 6.0]);
        // an upright moved by hand: 2.0 taken away, 2.4 put in
        g.vertical_hand = HandLines {
            added: vec![2.4],
            removed: vec![2.0],
        };
        assert_eq!(g.grid_lines(true, 8.0, &[]), vec![0.0, 2.4, 4.0, 6.0, 8.0]);
    }

    #[test]
    fn a_segment_taken_out_merges_the_panels_either_side() {
        let g = CurtainGrid {
            vertical: GridRule::Count(3),
            horizontal: GridRule::Count(2),
            gaps: vec![Gap {
                upright: true,
                at: 1.0,
                mid: 0.5,
            }],
            ..Default::default()
        };
        let l = g.layout(3.0, 3.0, &[]);
        // six cells, two of them one pane now
        assert_eq!(l.panels.len(), 5);
        let wide = l
            .panels
            .iter()
            .find(|p| p.rect.u1 - p.rect.u0 > 1.5)
            .unwrap();
        assert!(wide.rect.v1 < 1.6);
        // and no upright left between them in that row
        assert!(!l.mullions.iter().any(|m| m.run == MullionRun::Vertical
            && m.rect.u0 < 1.0
            && m.rect.u1 > 1.0
            && m.rect.v0 < 0.5
            && m.rect.v1 > 0.5));
    }

    #[test]
    fn a_door_takes_the_panels_and_mullions_it_covers() {
        let g = CurtainGrid {
            vertical: GridRule::Count(3),
            horizontal: GridRule::Count(2),
            swapped: vec![(2, 1)],
            ..Default::default()
        };
        // a door in the middle bay, through the transom at 1.5 m
        let door = FaceRect {
            u0: 1.03,
            u1: 1.97,
            v0: 0.0,
            v1: 2.1,
        };
        let l = g.layout(3.0, 3.0, &[door]);
        // the four bays beside it, and glass over the door on a transom
        // at its head (it was left open before)
        assert_eq!(l.panels.len(), 5);
        assert!(l
            .panels
            .iter()
            .any(|p| (p.bay, p.row) == (1, 1) && (p.rect.v0 - 2.16).abs() < 1e-9));
        assert!(l.mullions.iter().any(|m| m.run == MullionRun::Horizontal
            && (m.rect.v0 - 2.1).abs() < 1e-9
            && (m.rect.u0 - 1.03).abs() < 1e-9));
        assert!(!l.mullions.iter().any(|m| m.rect.overlaps(&door)));
        assert_eq!(
            l.panels
                .iter()
                .find(|p| (p.bay, p.row) == (2, 1))
                .unwrap()
                .kind,
            PanelKind::Solid
        );
    }

    #[test]
    fn walls_from_older_files_are_not_curtain_walls() {
        let level = "00000000-0000-0000-0000-000000000001";
        let wall = format!(
            r#"{{"level":"{level}","start":[0.0,0.0],"end":[4.0,0.0],"thickness":0.2,"height":3.0,"base_offset":0.0}}"#
        );
        let w: Wall = serde_json::from_str(&wall).unwrap();
        assert!(w.curtain.is_none());
        assert!(!serde_json::to_string(&w).unwrap().contains("curtain"));
        let mut w = w;
        w.curtain = Some(CurtainGrid::default());
        let back: Wall = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
        assert_eq!(back, w);
    }
}
