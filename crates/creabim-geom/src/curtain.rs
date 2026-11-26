//! Curtain walls built from their grid (see [`creabim_core::curtain`]):
//! every mullion and panel a box in the wall's own frame, so the 3D view,
//! plans, sections and the IFC export share one description.

use crate::plan::{Pen, PlanItem};
use crate::TriMesh;
use creabim_core::curtain::{CurtainGrid, PanelKind};
use creabim_core::{ElementId, Opening, Wall};
use glam::{DVec2, DVec3};

/// What a box of a curtain wall is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CurtainPart {
    Mullion,
    Glass,
    /// An opaque panel.
    Panel,
    /// The sealed joint between flush-glazed panes.
    Seal,
}

/// One part as a box in wall coordinates: `x` along the wall from its
/// start, `y` across it from its centreline (left positive), `z` up from
/// its base.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurtainBox {
    pub part: CurtainPart,
    pub lo: DVec3,
    pub hi: DVec3,
    /// Under a roof the box slopes: its underside and top at its two
    /// ends, `[bottom at lo.x, bottom at hi.x, top at lo.x, top at hi.x]`
    /// (`lo.z` and `hi.z` then bound them).
    pub zs: Option<[f64; 4]>,
    /// Ends that run on into the next piece of the same part, and so
    /// draw no face (start, end).
    pub joined: [bool; 2],
    /// The stretch along the wall whose chord it lies on: its own, or,
    /// for a piece cut from a pane under a roof, the whole pane's (so a
    /// curved wall's pane stays one flat sheet).
    pub chord: (f64, f64),
}

impl CurtainBox {
    fn flat(part: CurtainPart, lo: DVec3, hi: DVec3) -> Self {
        Self {
            part,
            lo,
            hi,
            zs: None,
            joined: [false; 2],
            chord: (lo.x, hi.x),
        }
    }

    /// Its underside and top at `x` along the wall.
    pub fn z_at(&self, x: f64) -> (f64, f64) {
        match self.zs {
            None => (self.lo.z, self.hi.z),
            Some([b0, b1, t0, t1]) => {
                let w = self.hi.x - self.lo.x;
                let k = if w > 1e-12 {
                    ((x - self.lo.x) / w).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                (b0 + (b1 - b0) * k, t0 + (t1 - t0) * k)
            }
        }
    }
}

/// The top of a curtain wall attached to a roof or slab: its height over
/// the wall's base at points along it, straight between them.
#[derive(Clone, Debug, PartialEq)]
pub struct CurtainTop {
    pub pts: Vec<(f64, f64)>,
}

impl CurtainTop {
    pub fn at(&self, u: f64) -> f64 {
        let p = &self.pts;
        if u <= p[0].0 {
            return p[0].1;
        }
        for w in p.windows(2) {
            if u <= w[1].0 {
                let (a, b) = (w[0], w[1]);
                let t = if b.0 - a.0 > 1e-12 {
                    (u - a.0) / (b.0 - a.0)
                } else {
                    1.0
                };
                return a.1 + (b.1 - a.1) * t;
            }
        }
        p[p.len() - 1].1
    }

    pub fn highest(&self) -> f64 {
        self.pts.iter().map(|p| p.1).fold(f64::MIN, f64::max)
    }

    /// The line `d` below it, square to each slope: the underside of a
    /// head mullion `d` deep that follows it. Where the top steps (an
    /// eave), the step is left as it is.
    pub fn under(&self, d: f64) -> CurtainTop {
        // each stretch as a line v = a + s·u, dropped d square to itself
        let mut lines: Vec<(f64, f64, f64, f64)> = vec![]; // (s, a, u0, u1)
        for w in self.pts.windows(2) {
            let ((u0, v0), (u1, v1)) = (w[0], w[1]);
            if u1 - u0 < 1e-9 {
                continue;
            }
            let s = (v1 - v0) / (u1 - u0);
            if s.abs() > 6.0 {
                continue; // a step, not a slope
            }
            let a = v0 - s * u0 - d * (1.0 + s * s).sqrt();
            lines.push((s, a, u0, u1));
        }
        let (first, last) = (self.pts[0].0, self.pts[self.pts.len() - 1].0);
        if lines.is_empty() {
            return CurtainTop {
                pts: self.pts.iter().map(|&(u, v)| (u, v - d)).collect(),
            };
        }
        // the lowest line over each point, each reaching d past its stretch
        let low = |u: f64| -> f64 {
            lines
                .iter()
                .filter(|l| u >= l.2 - d - 1e-9 && u <= l.3 + d + 1e-9)
                .map(|l| l.1 + l.0 * u)
                .fold(f64::MAX, f64::min)
        };
        let mut us: Vec<f64> = self.pts.iter().map(|p| p.0).collect();
        for l in &lines {
            us.extend([l.2 - d, l.3 + d]);
        }
        for (i, a) in lines.iter().enumerate() {
            for b in &lines[i + 1..] {
                if (a.0 - b.0).abs() > 1e-9 {
                    us.push((b.1 - a.1) / (a.0 - b.0));
                }
            }
        }
        us.retain(|u| *u >= first && *u <= last);
        us.sort_by(f64::total_cmp);
        us.dedup_by(|a, b| (*a - *b).abs() < 1e-7);
        let pts = us
            .into_iter()
            .map(|u| {
                let v = low(u);
                (
                    u,
                    if v == f64::MAX {
                        self.at(u) - d
                    } else {
                        v.min(self.at(u) - d * 0.5)
                    },
                )
            })
            .collect();
        CurtainTop { pts }
    }
}

/// What a curtain wall's top follows when it is attached to a roof or
/// slab that is still there: its height along it, over its base.
pub fn curtain_top(model: &creabim_core::Model, wall: &Wall) -> Option<CurtainTop> {
    let prof = creabim_core::walls::top_profile(model, wall)?;
    let z0 = creabim_core::walls::base_z(model, wall);
    let len = wall.length();
    let mut us = vec![0.0, len];
    let bends = wall.bends();
    us.extend(bends.iter().copied());
    // where the top bends over a ridge or steps at an eave (as walls);
    // on a curved wall, along each straight stretch of its arc, as its
    // mullions are set, so a ridge met at a slant is still found
    let mut stops = vec![0.0];
    stops.extend(bends);
    stops.push(len);
    for s in stops.windows(2) {
        let (a, b) = (s[0], s[1]);
        let span = b - a;
        us.extend(
            prof.breaks(wall.place(a, 0.0), wall.place(b, 0.0))
                .into_iter()
                .flat_map(|k| [a + k * span - 0.001, a + k * span + 0.001])
                .filter(|u| *u > 0.0 && *u < len),
        );
    }
    us.sort_by(f64::total_cmp);
    us.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    // a curved wall's panes are flat, on chords inside its arc: under a
    // slope the roof over a chord can be lower than over the arc, so the
    // top is the lower of the two (a bay's sagitta in, or out)
    let sag = match (wall.curve(), &wall.curtain) {
        (creabim_core::sketch::Curve::Arc { radius, .. }, Some(g)) => {
            let bay = match g.vertical {
                creabim_core::GridRule::Spacing(s) | creabim_core::GridRule::Fit(s) => s,
                creabim_core::GridRule::Count(n) => len / n.max(1) as f64,
            }
            .min(len);
            let h = (bay / 2.0).min(radius);
            radius - (radius * radius - h * h).max(0.0).sqrt()
        }
        _ => 0.0,
    };
    let mut hit = false;
    let pts = us
        .into_iter()
        .map(|u| {
            let xy = wall.place(u, 0.0);
            let t = wall.tangent_at(u);
            // an end just past the roof's edge keeps its height
            let z = [0.0, 0.05, -0.05, 0.1, -0.1, 0.2, -0.2, 0.3, -0.3]
                .iter()
                .find_map(|o| prof.z_at(xy + t * *o));
            let z = z.map(|z| {
                [sag, -sag]
                    .iter()
                    .filter_map(|s| prof.z_at(wall.place(u, *s)))
                    .fold(z, f64::min)
            });
            hit |= z.is_some();
            (u, z.map_or(wall.height, |z| (z - z0).max(0.3)))
        })
        .collect();
    hit.then_some(CurtainTop { pts })
}

/// Where a curtain wall stands in the building: the roof over it, if it
/// is attached to one, and the lines the floors and the structural grid
/// put on it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Setting {
    pub top: Option<CurtainTop>,
    pub forced: creabim_core::curtain::Forced,
    /// How its start and end meet another curtain wall at a corner.
    pub ends: [EndJoin; 2],
}

/// How a curtain wall's end meets another at a corner (curtain grids,
/// decision 4): one of the two carries a square corner post, its edge
/// mullion run on across the other's depth; the other stops against it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum EndJoin {
    #[default]
    Free,
    /// The post: the edge mullion runs on this far past the end.
    Post(f64),
    /// Stopped against the other's post, this far in from the end.
    Butt(f64),
}

/// Which of two curtain walls meeting at a corner carries the post: the
/// first by where they lie, so both agree.
fn key(w: &Wall) -> [f64; 4] {
    [w.start.x, w.start.y, w.end.x, w.end.y]
}

/// How `wall`'s two ends meet other curtain walls on its level.
fn end_joins(model: &creabim_core::Model, wall: &Wall) -> [EndJoin; 2] {
    let mut out = [EndJoin::Free; 2];
    let me = key(wall);
    for (k, p, t) in [
        (0, wall.start, wall.tangent_at(0.0)),
        (1, wall.end, wall.tangent_at(wall.length())),
    ] {
        for e in model.iter() {
            let creabim_core::ElementKind::Wall(o) = &e.kind else {
                continue;
            };
            if o.curtain.is_none() || o.level != wall.level || key(o) == me {
                continue;
            }
            let ot = if o.start.distance(p) < 1e-3 {
                o.tangent_at(0.0)
            } else if o.end.distance(p) < 1e-3 {
                o.tangent_at(o.length())
            } else {
                continue;
            };
            // a corner, not two walls running on in line
            if t.perp_dot(ot).abs() < 0.17 {
                continue;
            }
            let half = o
                .curtain
                .as_ref()
                .map_or(o.thickness, |g| g.mullion_depth.min(o.thickness.max(0.01)))
                / 2.0;
            let first = me
                .iter()
                .zip(key(o).iter())
                .find(|(a, b)| (*a - *b).abs() > 1e-9)
                .is_some_and(|(a, b)| a < b);
            out[k] = if first {
                EndJoin::Post(half)
            } else {
                EndJoin::Butt(half)
            };
        }
    }
    out
}

/// [`Setting`] of curtain wall `wall` in `model`.
pub fn setting(model: &creabim_core::Model, wall: &Wall) -> Setting {
    let top = curtain_top(model, wall);
    let height = top.as_ref().map_or(wall.height, |t| t.highest());
    Setting {
        forced: creabim_core::curtain::forced_lines(model, wall, height),
        top,
        ends: end_joins(model, wall),
    }
}

/// Every mullion and panel of a curtain wall, leaving room for its doors
/// and holes.
pub fn curtain_boxes(wall: &Wall, grid: &CurtainGrid, openings: &[&Opening]) -> Vec<CurtainBox> {
    curtain_boxes_under(wall, grid, openings, &Setting::default())
}

/// As [`curtain_boxes`], under the roof or slab `top` it is attached to:
/// the grid runs up to the highest point, every part stops under a head
/// mullion that follows the roof, and what lies above it is left out.
pub fn curtain_boxes_under(
    wall: &Wall,
    grid: &CurtainGrid,
    openings: &[&Opening],
    at: &Setting,
) -> Vec<CurtainBox> {
    let top = at.top.as_ref();
    let height = top.map_or(wall.height, |t| t.highest());
    let holes = creabim_core::curtain::holes_of(openings);
    let layout = grid.layout_with(wall.length(), height, &holes, &at.forced);
    let md = grid.mullion_depth.min(wall.thickness.max(0.01)) / 2.0;
    let pt = grid.panel_thickness.min(md * 2.0) / 2.0;
    let across = Across::of(grid, md, pt);
    let mut out = Vec::new();
    for m in &layout.mullions {
        let r = m.rect;
        let horizontal = m.run == creabim_core::curtain::MullionRun::Horizontal;
        // under a roof the head mullion takes the top edge's place
        if top.is_some() && horizontal && r.v1 >= height - 1e-6 {
            continue;
        }
        for (part, y0, y1, narrow) in across.pieces(horizontal) {
            let (mut u0, mut u1, mut v0, mut v1) = (r.u0, r.u1, r.v0, r.v1);
            if let Some(w) = narrow {
                // the plate or joint between panes: narrower, centred
                if horizontal {
                    let c = (v0 + v1) / 2.0;
                    (v0, v1) = (c - w / 2.0, c + w / 2.0);
                } else {
                    let c = (u0 + u1) / 2.0;
                    (u0, u1) = (c - w / 2.0, c + w / 2.0);
                }
            }
            out.push(CurtainBox::flat(
                part,
                DVec3::new(u0, y0, v0),
                DVec3::new(u1, y1, v1),
            ));
        }
    }
    // panes run into the mullions as far as the plate or joint
    let e = across.bite;
    for p in &layout.panels {
        let r = p.rect;
        out.push(CurtainBox::flat(
            match p.kind {
                PanelKind::Glass => CurtainPart::Glass,
                PanelKind::Solid => CurtainPart::Panel,
            },
            DVec3::new(r.u0 - e, across.glass.0, r.v0 - e),
            DVec3::new(r.u1 + e, across.glass.1, r.v1 + e),
        ));
    }
    out.retain(|b| (b.hi - b.lo).min_element() > 1e-6);
    corners(&mut out, &at.ends, wall.length(), grid.mullion_width);
    let Some(top) = top else {
        return out;
    };
    // everything stops under the head mullion
    let inner = top.under(grid.mullion_width);
    let mut cut = Vec::new();
    for b in out {
        clip_under(b, &inner, &mut cut);
    }
    // the head mullion, following the roof
    let mut us: Vec<f64> = top.pts.iter().chain(&inner.pts).map(|p| p.0).collect();
    us.sort_by(f64::total_cmp);
    us.dedup_by(|a, b| (*a - *b).abs() < 1e-7);
    let n = us.len();
    for (i, w) in us.windows(2).enumerate() {
        let (ua, ub) = (w[0], w[1]);
        let zs = [
            inner.at(ua).max(0.0),
            inner.at(ub).max(0.0),
            top.at(ua),
            top.at(ub),
        ];
        for (part, y0, y1) in across.head() {
            cut.push(CurtainBox {
                part,
                lo: DVec3::new(ua, y0, zs[0].min(zs[1])),
                hi: DVec3::new(ub, y1, zs[2].max(zs[3])),
                zs: Some(zs),
                joined: [i > 0, i + 2 < n],
                chord: (ua, ub),
            });
        }
    }
    cut
}

/// Where a curtain wall's parts sit across it, by its mullion profile:
/// `y` from the outside face (negative, the wall's exterior on its right)
/// to the inside.
struct Across {
    profile: creabim_core::MullionProfile,
    /// Half the mullions' depth.
    md: f64,
    /// The transoms' depth, from the outside face (or centred, as a box).
    transom: f64,
    /// The panes' outside and inside faces.
    glass: (f64, f64),
    /// The cap's depth (stick) and the plate or joint's width.
    cap: f64,
    narrow: f64,
    /// How far panes run into a mullion past its face.
    bite: f64,
}

impl Across {
    fn of(grid: &CurtainGrid, md: f64, pt: f64) -> Self {
        use creabim_core::MullionProfile as P;
        let depth = 2.0 * md;
        let transom = grid.transom_depth.unwrap_or(depth).clamp(0.01, depth);
        let mw = grid.mullion_width;
        let out = -md;
        match grid.profile {
            P::Box => Self {
                profile: grid.profile,
                md,
                transom,
                glass: (-pt, pt),
                cap: 0.0,
                narrow: mw,
                bite: 0.0,
            },
            P::Stick => {
                // the cap, a 6 mm plate and gasket, then the glass
                let cap = grid.cap.clamp(0.002, depth * 0.4);
                let g0 = out + cap + 0.006;
                let g1 = (g0 + 2.0 * pt).min(md - 0.005);
                let narrow = (mw * 0.35).min(0.016);
                Self {
                    profile: grid.profile,
                    md,
                    transom: transom.max(g1 - out + 0.01),
                    glass: (g0, g1),
                    cap,
                    narrow,
                    bite: ((mw - narrow) / 2.0).max(0.0),
                }
            }
            P::Flush => {
                let g1 = (out + 2.0 * pt).min(md - 0.005);
                let narrow = grid.joint.min(mw);
                Self {
                    profile: grid.profile,
                    md,
                    transom: transom.max(g1 - out + 0.01),
                    glass: (out, g1),
                    cap: 0.0,
                    narrow,
                    bite: ((mw - narrow) / 2.0).max(0.0),
                }
            }
        }
    }

    /// A mullion's (or transom's) pieces: each one's part, its span
    /// across the wall, and the narrower face width of a plate or joint.
    fn pieces(&self, transom: bool) -> Vec<(CurtainPart, f64, f64, Option<f64>)> {
        use creabim_core::MullionProfile as P;
        let out = -self.md;
        let depth = if transom { self.transom } else { 2.0 * self.md };
        match self.profile {
            P::Box => vec![(CurtainPart::Mullion, -depth / 2.0, depth / 2.0, None)],
            P::Stick => vec![
                (CurtainPart::Mullion, out, out + self.cap, None),
                (
                    CurtainPart::Mullion,
                    out + self.cap,
                    self.glass.1,
                    Some(self.narrow),
                ),
                (CurtainPart::Mullion, self.glass.1, out + depth, None),
            ],
            P::Flush => vec![
                (
                    CurtainPart::Seal,
                    self.glass.0,
                    self.glass.1,
                    Some(self.narrow),
                ),
                (CurtainPart::Mullion, self.glass.1, out + depth, None),
            ],
        }
    }

    /// The head mullion under a roof: its cap, and its body over the
    /// glass line (the panes stop under it).
    fn head(&self) -> Vec<(CurtainPart, f64, f64)> {
        use creabim_core::MullionProfile as P;
        let (out, back) = (-self.md, self.md);
        match self.profile {
            P::Stick => vec![
                (CurtainPart::Mullion, out, out + self.cap),
                (CurtainPart::Mullion, out + self.cap, back),
            ],
            _ => vec![(CurtainPart::Mullion, out, back)],
        }
    }
}

/// The ends where curtain walls meet: the post's edge mullion run on
/// across the other's depth; the other's edge mullion left out and its
/// transoms and panes run on to the post.
fn corners(out: &mut Vec<CurtainBox>, ends: &[EndJoin; 2], len: f64, mw: f64) {
    let eps = 1e-6;
    for (k, join) in ends.iter().enumerate() {
        let start = k == 0;
        // the edge mullion's pieces at this end, full width
        let edge = |b: &CurtainBox| {
            matches!(b.part, CurtainPart::Mullion | CurtainPart::Seal)
                && if start {
                    b.lo.x <= eps && b.hi.x <= mw + eps
                } else {
                    b.hi.x >= len - eps && b.lo.x >= len - mw - eps
                }
        };
        match *join {
            EndJoin::Free => {}
            EndJoin::Post(m) => {
                for b in out.iter_mut().filter(|b| edge(b)) {
                    if start {
                        b.lo.x -= m;
                    } else {
                        b.hi.x += m;
                    }
                    b.chord = (b.lo.x, b.hi.x);
                }
            }
            EndJoin::Butt(m) => {
                out.retain(|b| !edge(b));
                // what stopped at the edge mullion (the plates between
                // panes too) runs on to the post
                for b in out.iter_mut() {
                    if start && b.lo.x <= mw + eps {
                        b.lo.x = m;
                    } else if !start && b.hi.x >= len - mw - eps {
                        b.hi.x = len - m;
                    } else {
                        continue;
                    }
                    b.chord = (b.lo.x, b.hi.x);
                }
                out.retain(|b| b.hi.x - b.lo.x > 1e-6);
            }
        }
    }
}

/// Box `b` cut to what lies under `ceil`: sloped pieces where the line
/// crosses it, nothing where it lies wholly above.
fn clip_under(b: CurtainBox, ceil: &CurtainTop, out: &mut Vec<CurtainBox>) {
    let (x0, x1) = (b.lo.x, b.hi.x);
    let lowest = ceil
        .pts
        .iter()
        .filter(|p| p.0 > x0 && p.0 < x1)
        .map(|p| p.1)
        .fold(ceil.at(x0).min(ceil.at(x1)), f64::min);
    if lowest >= b.hi.z {
        out.push(b);
        return;
    }
    let mut xs = vec![x0, x1];
    xs.extend(ceil.pts.iter().map(|p| p.0).filter(|u| *u > x0 && *u < x1));
    // where the line crosses the box's top or underside
    for w in ceil.pts.windows(2) {
        let ((ua, va), (ub, vb)) = (w[0], w[1]);
        if (vb - va).abs() < 1e-12 {
            continue;
        }
        for z in [b.lo.z, b.hi.z] {
            let t = (z - va) / (vb - va);
            if t > 0.0 && t < 1.0 {
                let u = ua + (ub - ua) * t;
                if u > x0 && u < x1 {
                    xs.push(u);
                }
            }
        }
    }
    xs.sort_by(f64::total_cmp);
    xs.dedup_by(|a, b| (*a - *b).abs() < 1e-7);
    let start = out.len();
    let mut last_kept = false;
    for w in xs.windows(2) {
        let (ua, ub) = (w[0], w[1]);
        let (ta, tb) = (ceil.at(ua).min(b.hi.z), ceil.at(ub).min(b.hi.z));
        if ta.max(tb) <= b.lo.z + 1e-4 {
            last_kept = false;
            continue;
        }
        let (ta, tb) = (ta.max(b.lo.z), tb.max(b.lo.z));
        if last_kept {
            if let Some(prev) = out.last_mut() {
                prev.joined[1] = true;
            }
        }
        out.push(CurtainBox {
            part: b.part,
            lo: DVec3::new(ua, b.lo.y, b.lo.z),
            hi: DVec3::new(ub, b.hi.y, ta.max(tb)),
            zs: Some([b.lo.z, b.lo.z, ta, tb]),
            joined: [last_kept, false],
            chord: b.chord,
        });
        last_kept = true;
    }
    // a piece that is a whole box still keeps its square top
    if out.len() == start + 1 {
        let p = &mut out[start];
        if let Some([_, _, ta, tb]) = p.zs {
            if (ta - b.hi.z).abs() < 1e-9 && (tb - b.hi.z).abs() < 1e-9 {
                p.zs = None;
            }
        }
    }
}

/// A box with a sloped top or underside, its faces turned to face out.
fn add_sloped(m: &mut TriMesh, b: &CurtainBox, at: &dyn Fn(DVec3) -> DVec3) {
    let [b0, b1, t0, t1] = b.zs.unwrap_or([b.lo.z, b.lo.z, b.hi.z, b.hi.z]);
    let (x0, x1, y0, y1) = (b.lo.x, b.hi.x, b.lo.y, b.hi.y);
    let p = |x: f64, y: f64, z: f64| at(DVec3::new(x, y, z));
    let o = at(DVec3::ZERO);
    let ex = at(DVec3::X) - o;
    let ey = at(DVec3::Y) - o;
    let face = |m: &mut TriMesh, q: [DVec3; 4], out: DVec3| {
        let n = (q[2] - q[0]).cross(q[3] - q[1]);
        let n = if n.length_squared() < 1e-24 {
            out
        } else if n.dot(out) < 0.0 {
            -n
        } else {
            n
        };
        m.add_quad(q, n);
    };
    // front and back
    face(
        m,
        [p(x0, y0, b0), p(x1, y0, b1), p(x1, y0, t1), p(x0, y0, t0)],
        -ey,
    );
    face(
        m,
        [p(x0, y1, b0), p(x1, y1, b1), p(x1, y1, t1), p(x0, y1, t0)],
        ey,
    );
    // underside and top
    let up = at(DVec3::Z) - o;
    face(
        m,
        [p(x0, y0, b0), p(x1, y0, b1), p(x1, y1, b1), p(x0, y1, b0)],
        -up,
    );
    face(
        m,
        [p(x0, y0, t0), p(x1, y0, t1), p(x1, y1, t1), p(x0, y1, t0)],
        up,
    );
    if !b.joined[0] && t0 - b0 > 1e-9 {
        face(
            m,
            [p(x0, y0, b0), p(x0, y1, b0), p(x0, y1, t0), p(x0, y0, t0)],
            -ex,
        );
    }
    if !b.joined[1] && t1 - b1 > 1e-9 {
        face(
            m,
            [p(x1, y0, b1), p(x1, y1, b1), p(x1, y1, t1), p(x1, y0, t1)],
            ex,
        );
    }
}

/// Wall coordinates to model coordinates for a wall on a level at `level_z`.
pub fn to_model(wall: &Wall, level_z: f64) -> impl Fn(DVec3) -> DVec3 {
    let d = wall.direction();
    let n = DVec2::new(-d.y, d.x);
    let start = wall.start;
    let z0 = level_z + wall.base_offset;
    move |p: DVec3| (start + d * p.x + n * p.y).extend(z0 + p.z)
}

/// Wall coordinates to model coordinates for one box of the wall, from
/// `u0` to `u1` along it: on a curved wall the box runs straight along the
/// chord of its stretch of arc, as curved curtain walls are built (flat
/// glass between mullions set round the curve). Its length along the wall
/// maps onto the chord, so neighbours meet at the mullions.
pub fn box_to_model(wall: &Wall, u0: f64, u1: f64, level_z: f64) -> impl Fn(DVec3) -> DVec3 {
    let (a, c) = (wall.place(u0, 0.0), wall.place(u1, 0.0));
    let d = (c - a)
        .try_normalize()
        .unwrap_or_else(|| wall.tangent_at(u0));
    let n = DVec2::new(-d.y, d.x);
    let k = if u1 - u0 > 1e-9 {
        a.distance(c) / (u1 - u0)
    } else {
        1.0
    };
    let z0 = level_z + wall.base_offset;
    move |p: DVec3| (a + d * ((p.x - u0) * k) + n * p.y).extend(z0 + p.z)
}

/// The curtain wall's parts as meshes in model coordinates, one per kind.
pub fn curtain_meshes(
    wall: &Wall,
    grid: &CurtainGrid,
    openings: &[&Opening],
    level_z: f64,
) -> Vec<(CurtainPart, TriMesh)> {
    curtain_meshes_under(wall, grid, openings, level_z, &Setting::default())
}

/// As [`curtain_meshes`], under the roof or slab `top` it is attached to.
pub fn curtain_meshes_under(
    wall: &Wall,
    grid: &CurtainGrid,
    openings: &[&Opening],
    level_z: f64,
    at: &Setting,
) -> Vec<(CurtainPart, TriMesh)> {
    let mut out: Vec<(CurtainPart, TriMesh)> = Vec::new();
    for b in curtain_boxes_under(wall, grid, openings, at) {
        let at = box_to_model(wall, b.chord.0, b.chord.1, level_z);
        let i = match out.iter().position(|(p, _)| *p == b.part) {
            Some(i) => i,
            None => {
                out.push((b.part, TriMesh::default()));
                out.len() - 1
            }
        };
        if b.zs.is_none() && b.joined == [false; 2] {
            crate::opening::add_box(&mut out[i].1, b.lo, b.hi, &at);
        } else {
            add_sloped(&mut out[i].1, &b, &at);
        }
    }
    out
}

/// A curtain wall in a plan cut `cut_v` above its base: the mullions cut
/// (filled), glass as a thin pair of lines, solid panels cut.
pub fn curtain_plan(
    id: ElementId,
    wall: &Wall,
    grid: &CurtainGrid,
    openings: &[&Opening],
    cut_v: f64,
) -> Vec<PlanItem> {
    curtain_plan_under(id, wall, grid, openings, cut_v, &Setting::default())
}

/// As [`curtain_plan`], under the roof or slab `top` it is attached to.
pub fn curtain_plan_under(
    id: ElementId,
    wall: &Wall,
    grid: &CurtainGrid,
    openings: &[&Opening],
    cut_v: f64,
    at: &Setting,
) -> Vec<PlanItem> {
    let mut out = Vec::new();
    for b in curtain_boxes_under(wall, grid, openings, at) {
        if !(b.lo.z < cut_v && cut_v < b.hi.z) {
            continue;
        }
        // a sloped piece is cut only where the plane passes through it
        let (mut x0, mut x1) = (b.lo.x, b.hi.x);
        if b.zs.is_some() {
            let inside = |x: f64| {
                let (lo, hi) = b.z_at(x);
                lo < cut_v && cut_v < hi
            };
            let n = 32;
            let xs: Vec<f64> = (0..=n)
                .map(|i| b.lo.x + (b.hi.x - b.lo.x) * i as f64 / n as f64)
                .filter(|x| inside(*x))
                .collect();
            let (Some(a), Some(c)) = (xs.first(), xs.last()) else {
                continue;
            };
            (x0, x1) = (*a, *c);
            if x1 - x0 < 1e-6 {
                continue;
            }
        }
        let m = box_to_model(wall, b.chord.0, b.chord.1, 0.0);
        let at = |u: f64, s: f64| m(DVec3::new(u, s, 0.0)).truncate();
        let points = vec![
            at(x0, b.lo.y),
            at(x1, b.lo.y),
            at(x1, b.hi.y),
            at(x0, b.hi.y),
        ];
        let pen = match b.part {
            CurtainPart::Glass => Pen::Symbol,
            CurtainPart::Mullion | CurtainPart::Panel | CurtainPart::Seal => Pen::Cut,
        };
        out.push(PlanItem::Polygon { id, pen, points });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use creabim_core::{GridRule, OpeningKind};

    fn wall() -> Wall {
        let mut w = Wall::new(
            ElementId::new(),
            DVec2::ZERO,
            DVec2::new(3.0, 0.0),
            0.15,
            3.0,
        );
        w.curtain = Some(CurtainGrid {
            vertical: GridRule::Count(3),
            horizontal: GridRule::Count(2),
            ..Default::default()
        });
        w
    }

    #[test]
    fn mullions_and_glass_fill_the_wall() {
        let w = wall();
        let g = w.curtain.clone().unwrap();
        let parts = curtain_meshes(&w, &g, &[], 0.0);
        let glass = parts.iter().find(|p| p.0 == CurtainPart::Glass).unwrap();
        // six panes of glass between 60 mm mullions
        // (edge mullions are a whole width inside the wall, inner ones centred)
        let widths = 0.91 + 0.94 + 0.91;
        let area = widths * 2.0 * 1.41;
        assert!((glass.1.volume() - area * 0.024).abs() < 1e-4);
        let (lo, hi) = parts[0].1.bounds().unwrap();
        assert!(lo.x.abs() < 1e-6 && (hi.x - 3.0).abs() < 1e-6 && (hi.z - 3.0).abs() < 1e-6);
        // the plan at 1.2 m cuts four uprights and three panes
        let plan = curtain_plan(ElementId::new(), &w, &g, &[], 1.2);
        let cut = plan
            .iter()
            .filter(|i| matches!(i, PlanItem::Polygon { pen: Pen::Cut, .. }))
            .count();
        assert_eq!((cut, plan.len()), (4, 7));
    }

    #[test]
    fn a_curved_curtain_wall_sets_its_mullions_round_the_arc() {
        // a half circle of radius 1.5 round (1.5, 0)
        let mut w = wall();
        w.bulge = 1.0;
        let g = w.curtain.clone().unwrap();
        let c = DVec2::new(1.5, 0.0);
        for b in curtain_boxes(&w, &g, &[]) {
            let at = box_to_model(&w, b.lo.x, b.hi.x, 0.0);
            // each box's two ends, on its centreline, sit on the arc
            for x in [b.lo.x, b.hi.x] {
                let p = at(DVec3::new(x, 0.0, 0.0)).truncate();
                assert!((p.distance(c) - 1.5).abs() < 1e-9, "{p}");
            }
        }
    }

    #[test]
    fn under_a_gable_every_part_stops_at_the_roof_and_a_head_follows_it() {
        // 6 m wide, eaves at 3 m, ridge at 4.5 m in the middle
        let mut w = wall();
        w.end = DVec2::new(6.0, 0.0);
        let g = w.curtain.clone().unwrap();
        let top = CurtainTop {
            pts: vec![(0.0, 3.0), (2.999, 4.4995), (3.001, 4.4995), (6.0, 3.0)],
        };
        let at = Setting {
            top: Some(top.clone()),
            ..Default::default()
        };
        let boxes = curtain_boxes_under(&w, &g, &[], &at);
        for b in &boxes {
            for x in [b.lo.x, (b.lo.x + b.hi.x) / 2.0, b.hi.x] {
                let (_, t) = b.z_at(x);
                assert!(t <= top.at(x) + 1e-6, "{:?} pokes out at {x}: {t}", b.part);
            }
        }
        // glass stops a mullion's width under the roof, square to its slope
        let inner = top.under(g.mullion_width);
        let drop = top.at(1.5) - inner.at(1.5);
        assert!((drop - 0.06 * 1.25f64.sqrt()).abs() < 1e-3, "{drop}");
        for b in boxes.iter().filter(|b| b.part == CurtainPart::Glass) {
            let (_, t) = b.z_at(b.lo.x);
            assert!(t <= inner.at(b.lo.x) + 1e-6);
        }
        // the head runs the whole way along under the roof
        let head: f64 = boxes
            .iter()
            .filter(|b| b.part == CurtainPart::Mullion && b.zs.is_some())
            .filter(|b| (b.z_at(b.lo.x).1 - top.at(b.lo.x)).abs() < 1e-6)
            .map(|b| b.hi.x - b.lo.x)
            .sum();
        assert!((head - 6.0).abs() < 1e-6, "{head}");
        // and nothing is left of the flat top edge at the ridge's height
        assert!(!boxes
            .iter()
            .any(|b| b.zs.is_none() && b.part == CurtainPart::Mullion && b.hi.z > 4.0));
        // the meshes build, closed enough to have a volume
        let parts = curtain_meshes_under(&w, &g, &[], 0.0, &at);
        assert!(parts.iter().all(|p| p.1.volume() > 0.0));
    }

    /// Two boxes overlap (more than touching).
    fn overlap(a: &CurtainBox, b: &CurtainBox) -> bool {
        (0..3).all(|k| a.lo[k] < b.hi[k] - 1e-7 && b.lo[k] < a.hi[k] - 1e-7)
    }

    #[test]
    fn each_profile_builds_its_parts_without_one_inside_another() {
        use creabim_core::MullionProfile;
        for profile in MullionProfile::ALL {
            let mut w = wall();
            let mut g = w.curtain.clone().unwrap();
            g.profile = profile;
            g.transom_depth = Some(0.1);
            w.curtain = Some(g.clone());
            let boxes = curtain_boxes(&w, &g, &[]);
            let glass: Vec<_> = boxes
                .iter()
                .filter(|b| b.part == CurtainPart::Glass)
                .collect();
            let frame: Vec<_> = boxes
                .iter()
                .filter(|b| b.part != CurtainPart::Glass)
                .collect();
            assert_eq!(glass.len(), 6, "{profile:?}");
            for p in &glass {
                for f in &frame {
                    assert!(!overlap(p, f), "{profile:?}: glass {p:?} runs into {f:?}");
                }
            }
            let front = frame.iter().map(|b| b.lo.y).fold(f64::MAX, f64::min);
            let pane = glass[0];
            match profile {
                // the glass in the middle
                MullionProfile::Box => assert!((pane.lo.y + pane.hi.y).abs() < 1e-9),
                // a cap in front of the glass, the glass running in behind it
                MullionProfile::Stick => {
                    assert!(front < pane.lo.y - 0.01);
                    assert!(pane.hi.x - pane.lo.x > 0.94);
                }
                // the glass is the outside face, the joints dark between
                MullionProfile::Flush => {
                    assert!((pane.lo.y + 0.075).abs() < 1e-9);
                    assert!(boxes.iter().any(|b| b.part == CurtainPart::Seal));
                }
            }
            // transoms shallower than the mullions
            let deepest = |pick: &dyn Fn(&CurtainBox) -> bool| {
                frame
                    .iter()
                    .filter(|b| pick(b))
                    .map(|b| b.hi.y)
                    .fold(f64::MIN, f64::max)
            };
            let upright = deepest(&|b: &CurtainBox| b.hi.z - b.lo.z > 2.0);
            let across = deepest(&|b: &CurtainBox| b.hi.x - b.lo.x > 0.5 && b.hi.z - b.lo.z < 0.2);
            assert!(
                across < upright - 0.02,
                "{profile:?}: {across} against {upright}"
            );
            // and every part builds a closed solid
            for (part, mesh) in curtain_meshes(&w, &g, &[], 0.0) {
                assert!(mesh.volume() > 0.0, "{profile:?} {part:?}");
            }
        }
    }

    #[test]
    fn a_door_replaces_its_panel() {
        let w = wall();
        let g = w.curtain.clone().unwrap();
        let door = Opening {
            host: ElementId::new(),
            kind: OpeningKind::Door,
            offset: 1.03,
            sill: 0.0,
            width: 0.94,
            height: 2.1,
            material: None,
            frame: creabim_core::OpeningFrame::for_kind(OpeningKind::Door),
            flip_hand: false,
            flip_facing: false,
            roof_at: None,
            roof_build: Default::default(),
            arrangement: None,
        };
        let boxes = curtain_boxes(&w, &g, &[&door]);
        let glass = boxes
            .iter()
            .filter(|b| b.part == CurtainPart::Glass)
            .count();
        // four panes beside it, and one over it on a transom at its head
        assert_eq!(glass, 5);
    }
}
