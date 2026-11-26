//! The ground a terrain's data describes.
//!
//! A terrain keeps what a survey gives: spot heights, contour lines,
//! break lines and an outline. From them:
//!
//! - **The survey's triangles** (a TIN): a Delaunay triangulation of the
//!   points inside the outline in which every line is kept as triangle
//!   edges, so no triangle crosses a contour, a kerb or a bank top.
//! - **The smooth ground**: over each triangle a cubic patch through its
//!   three corners with the slope the ground has there (fitted to the
//!   points around each corner), so the surface passes through every
//!   surveyed height exactly and curves between them. Along a break line
//!   the patches stay straight and the ground folds; either side of it the
//!   slopes are fitted separately.
//!
//! Everything drawn or measured (the 3D solid, section cuts, contours,
//! the height at a point) reads the same surface: the smooth one, or the
//! flat triangles when the terrain is set to show its survey.

use creabim_core::{GroundLineKind, Site, SiteForm};
use glam::{DVec2, DVec3};
use spade::{ConstrainedDelaunayTriangulation, HasPosition, Point2, Triangulation};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Points closer than this are one point.
const SNAP: f64 = 1e-3;

struct V {
    p: Point2<f64>,
}

impl HasPosition for V {
    type Scalar = f64;
    fn position(&self) -> Point2<f64> {
        self.p
    }
}

fn key(a: usize, b: usize) -> (usize, usize) {
    (a.min(b), a.max(b))
}

/// A terrain's ground: heights are above its level.
#[derive(Debug, Default)]
pub struct Ground {
    /// The survey's corners.
    pub verts: Vec<DVec3>,
    /// The survey's triangles, counter-clockwise.
    pub tris: Vec<[usize; 3]>,
    /// Edges the ground folds along (break lines).
    crease: HashSet<(usize, usize)>,
    /// Edges along contour lines: the ground stays smooth across them
    /// but runs straight along them, so a contour drawn as data comes
    /// back exactly where it was drawn.
    held: HashSet<(usize, usize)>,
    /// The slope at each corner of each triangle.
    grads: Vec<[DVec2; 3]>,
    /// At each corner, the lowest and highest heights among it and the
    /// corners it shares a triangle with: the smooth ground round it
    /// stays between them.
    span: Vec<(f64, f64)>,
    /// Read smoothly through the points, or as the flat triangles.
    pub smooth: bool,
    /// The surface as drawn: the triangles, each divided when smooth.
    pub fine: Fine,
    /// Triangles by grid cell, to find the one over a point.
    grid: Grid,
}

/// The surface as it is drawn: corners and triangles (counter-clockwise).
#[derive(Debug, Default)]
pub struct Fine {
    pub verts: Vec<DVec3>,
    pub tris: Vec<[u32; 3]>,
    /// Corners lying on a break line.
    pub sharp: Vec<bool>,
}

impl Fine {
    /// The corners of a triangle standing near upright (an upright cut's
    /// face) shade as that face alone: a normal averaged with the faces
    /// round a corner or a bend shaded them in dark wedges (owner, cut
    /// test).
    fn upright_sharp(&mut self) {
        for t in &self.tris {
            let [a, b, c] = t.map(|i| self.verts[i as usize]);
            let n = (b - a).cross(c - a);
            if n.length() > 1e-12 && n.z.abs() < 0.3 * n.length() {
                for i in t {
                    self.sharp[*i as usize] = true;
                }
            }
        }
    }
}

#[derive(Debug, Default)]
struct Grid {
    lo: DVec2,
    cell: f64,
    nx: usize,
    ny: usize,
    cells: Vec<Vec<u32>>,
}

/// One contour line: its height above the level, how many intervals up
/// from zero elevation it is, and its points.
#[derive(Clone, Debug, PartialEq)]
pub struct Contour {
    pub z: f64,
    pub index: i64,
    pub points: Vec<DVec2>,
    pub closed: bool,
}

/// The cubic patch over one triangle: the heights of its ten control
/// points.
struct Patch {
    f: [f64; 3],
    /// `e[i][0]` a third along from corner `i` to the next corner,
    /// `e[i][1]` a third along from corner `i` to the one before.
    e: [[f64; 2]; 3],
    mid: f64,
}

impl Patch {
    fn at(&self, u: f64, v: f64, w: f64) -> f64 {
        let b = [u, v, w];
        let mut z = 6.0 * self.mid * u * v * w;
        for i in 0..3 {
            let (n, p) = ((i + 1) % 3, (i + 2) % 3);
            z += self.f[i] * b[i].powi(3)
                + 3.0 * self.e[i][0] * b[i] * b[i] * b[n]
                + 3.0 * self.e[i][1] * b[i] * b[i] * b[p];
        }
        z
    }
}

/// How far apart a survey's points are, typically: the median distance
/// from a point to its nearest neighbour (of a sample), kept between
/// half a metre and ten.
fn survey_spacing(points: &[DVec3]) -> f64 {
    let step = (points.len() / 200).max(1);
    let mut near: Vec<f64> = points
        .iter()
        .step_by(step)
        .filter_map(|p| {
            points
                .iter()
                .map(|q| q.truncate().distance(p.truncate()))
                .filter(|d| *d > 1e-6)
                .min_by(|a, b| a.total_cmp(b))
        })
        .collect();
    if near.is_empty() {
        return 2.5;
    }
    near.sort_by(|a, b| a.total_cmp(b));
    near[near.len() / 2].clamp(0.5, 10.0)
}

/// A height for a point of a terrain's boundary: the plane through the
/// survey points nearest it, carried on to it, within a little of their
/// own heights so a few points cannot throw it far.
fn edge_height(points: &[DVec3], c: DVec2) -> f64 {
    let mut near: Vec<(f64, DVec3)> = points
        .iter()
        .map(|p| (p.truncate().distance_squared(c), *p))
        .collect();
    let k = near.len().min(8);
    near.select_nth_unstable_by(k - 1, |a, b| a.0.total_cmp(&b.0));
    let near = &near[..k];
    if near[0].0 < 1e-12 {
        return near[0].1.z;
    }
    // least squares: z = a x + b y + c, about the point itself
    let (mut sxx, mut sxy, mut syy, mut sx, mut sy, mut n) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    let (mut sxz, mut syz, mut sz) = (0.0, 0.0, 0.0);
    for (_, p) in near {
        let (x, y, z) = (p.x - c.x, p.y - c.y, p.z);
        sxx += x * x;
        sxy += x * y;
        syy += y * y;
        sx += x;
        sy += y;
        n += 1.0;
        sxz += x * z;
        syz += y * z;
        sz += z;
    }
    let m = glam::DMat3::from_cols(
        DVec3::new(sxx, sxy, sx),
        DVec3::new(sxy, syy, sy),
        DVec3::new(sx, sy, n),
    );
    let (lo, hi) = near.iter().fold((f64::MAX, f64::MIN), |(lo, hi), (_, p)| {
        (lo.min(p.z), hi.max(p.z))
    });
    let room = 0.5 * (hi - lo);
    if m.determinant().abs() < 1e-9 {
        return creabim_core::site::surface_height(lo, points, c);
    }
    // the plane's height at the point itself is its constant term
    let z = (m.inverse() * DVec3::new(sxz, syz, sz)).z;
    z.clamp(lo - room, hi + room)
}

impl Ground {
    /// The ground of a terrain; none for anything else.
    pub fn build(site: &Site) -> Option<Ground> {
        let SiteForm::Terrain {
            boundary,
            top,
            points,
            data,
            ..
        } = &site.form
        else {
            return None;
        };
        let inside = |p: DVec2| {
            creabim_core::polygon::contains(boundary, p) || on_outline(boundary, p, SNAP)
        };
        // corners by position, so a point surveyed twice is one corner
        let mut verts: Vec<DVec3> = vec![];
        let mut seen: HashMap<(i64, i64), usize> = HashMap::new();
        let mut cdt = ConstrainedDelaunayTriangulation::<V>::new();
        let mut handles = vec![];
        let mut add = |p: DVec3, verts: &mut Vec<DVec3>| -> Option<usize> {
            let k = ((p.x / SNAP).round() as i64, (p.y / SNAP).round() as i64);
            if let Some(i) = seen.get(&k) {
                return Some(*i);
            }
            let h = cdt
                .insert(V {
                    p: Point2::new(p.x, p.y),
                })
                .ok()?;
            // spade gives a point it already has its old handle
            if h.index() < handles.len() {
                return Some(h.index());
            }
            handles.push(h);
            seen.insert(k, verts.len());
            verts.push(p);
            Some(verts.len() - 1)
        };
        // surveyed heights first: an outline corner on one takes its height
        let mut lines: Vec<(GroundLineKind, Vec<usize>)> = vec![];
        for p in points.iter().filter(|p| inside(p.truncate())) {
            add(*p, &mut verts);
        }
        for l in &data.lines {
            let mut run = vec![];
            let n = l.points.len();
            let count = if l.closed { n + 1 } else { n };
            for i in 0..count {
                let p = l.points[i % n];
                match inside(p.truncate()).then(|| add(p, &mut verts)).flatten() {
                    Some(v) => run.push(v),
                    // a line leaving the outline is cut there
                    None => lines.push((l.kind, std::mem::take(&mut run))),
                }
            }
            lines.push((l.kind, run));
        }
        let surveyed: Vec<DVec3> = verts.clone();
        let surveyed_edge = data.survey && surveyed.len() >= 3;
        let edge_height = |c: DVec2| {
            if surveyed_edge {
                edge_height(&surveyed, c)
            } else if data.survey && !surveyed.is_empty() {
                creabim_core::site::surface_height(*top, &surveyed, c)
            } else {
                *top
            }
        };
        // the boundary at about the survey's own spacing, each point
        // carrying on the slope of the survey nearest it: with its
        // corners alone, a survey stopping short of the boundary fell
        // away to one straight edge at a far average height, a lip all
        // along the terrain's side (owner, cut test)
        let along = if surveyed_edge {
            survey_spacing(&surveyed)
        } else {
            f64::INFINITY
        };
        let mut rim_points: Vec<DVec2> = vec![];
        for i in 0..boundary.len() {
            let (a, b) = (boundary[i], boundary[(i + 1) % boundary.len()]);
            let k = (a.distance(b) / along).ceil().clamp(1.0, 400.0) as usize;
            for j in 0..k {
                rim_points.push(a.lerp(b, j as f64 / k as f64));
            }
        }
        let ring: Vec<usize> = rim_points
            .iter()
            .filter_map(|c| add(c.extend(edge_height(*c)), &mut verts))
            .collect();
        if verts.len() < 3 {
            return None;
        }
        let n = ring.len();
        for i in 0..n {
            let (a, b) = (ring[i], ring[(i + 1) % n]);
            if a != b && cdt.can_add_constraint(handles[a], handles[b]) {
                cdt.add_constraint(handles[a], handles[b]);
            }
        }
        let mut sharp_runs = vec![];
        let mut held_runs = vec![];
        for (kind, run) in &lines {
            for w in run.windows(2) {
                if w[0] != w[1] && cdt.can_add_constraint(handles[w[0]], handles[w[1]]) {
                    cdt.add_constraint(handles[w[0]], handles[w[1]]);
                    match kind {
                        GroundLineKind::Break => sharp_runs.push((w[0], w[1])),
                        GroundLineKind::Contour => held_runs.push((w[0], w[1])),
                    }
                }
            }
        }
        let mut tris = vec![];
        for f in cdt.inner_faces() {
            let [a, b, c] = f.vertices().map(|v| v.fix().index());
            let (pa, pb, pc) = (
                verts[a].truncate(),
                verts[b].truncate(),
                verts[c].truncate(),
            );
            let area = (pb - pa).perp_dot(pc - pa);
            if area.abs() < 1e-9 || !creabim_core::polygon::contains(boundary, (pa + pb + pc) / 3.0)
            {
                continue;
            }
            tris.push(if area > 0.0 { [a, b, c] } else { [a, c, b] });
        }
        // a break line may have been laid through corners between its
        // ends: every triangle edge along it folds
        let edges: HashSet<(usize, usize)> = tris
            .iter()
            .flat_map(|t| (0..3).map(|i| key(t[i], t[(i + 1) % 3])))
            .collect();
        let along = |runs: Vec<(usize, usize)>| {
            let mut out = HashSet::new();
            for (a, b) in runs {
                if edges.contains(&key(a, b)) {
                    out.insert(key(a, b));
                    continue;
                }
                let (pa, pb) = (verts[a].truncate(), verts[b].truncate());
                for e in &edges {
                    let on = |i: usize| {
                        let p = verts[i].truncate();
                        let t = (p - pa).dot(pb - pa) / (pb - pa).length_squared();
                        (-1e-9..=1.0 + 1e-9).contains(&t) && (pa + (pb - pa) * t).distance(p) < SNAP
                    };
                    if on(e.0) && on(e.1) {
                        out.insert(*e);
                    }
                }
            }
            out
        };
        let crease = along(sharp_runs);
        let held = along(held_runs);
        let smooth = !data.faceted && !surveyed.is_empty();
        let mut g = Ground {
            verts,
            tris,
            crease,
            held,
            grads: vec![],
            span: vec![],
            smooth,
            fine: Fine::default(),
            grid: Grid::default(),
        };
        g.grads = g.slopes();
        g.span = g.verts.iter().map(|v| (v.z, v.z)).collect();
        for t in &g.tris {
            let (lo, hi) = t.iter().fold((f64::MAX, f64::MIN), |(lo, hi), v| {
                (lo.min(g.verts[*v].z), hi.max(g.verts[*v].z))
            });
            for v in t {
                g.span[*v] = (g.span[*v].0.min(lo), g.span[*v].1.max(hi));
            }
        }
        g.fine = g.divide();
        g.grid = g.bucket();
        Some(g)
    }

    /// The slope at each corner of each triangle: a plane fitted to the
    /// corners around it, on its side of any break line through it.
    fn slopes(&self) -> Vec<[DVec2; 3]> {
        let plane = |t: &[usize; 3]| {
            let [a, b, c] = t.map(|i| self.verts[i]);
            let n = (b - a).cross(c - a);
            if n.z.abs() < 1e-12 {
                DVec2::ZERO
            } else {
                DVec2::new(-n.x / n.z, -n.y / n.z)
            }
        };
        if !self.smooth {
            return self.tris.iter().map(|t| [plane(t); 3]).collect();
        }
        let mut around: Vec<Vec<usize>> = vec![vec![]; self.verts.len()];
        for (i, t) in self.tris.iter().enumerate() {
            for v in t {
                around[*v].push(i);
            }
        }
        let mut out = vec![[DVec2::ZERO; 3]; self.tris.len()];
        let mut done = vec![[false; 3]; self.tris.len()];
        for (ti, t) in self.tris.iter().enumerate() {
            for c in 0..3 {
                if done[ti][c] {
                    continue;
                }
                let v = t[c];
                // the fan of triangles reached from this one round the
                // corner without crossing a break line
                let mut fan = vec![ti];
                let mut k = 0;
                while k < fan.len() {
                    let cur = self.tris[fan[k]];
                    for other in &around[v] {
                        if fan.contains(other) {
                            continue;
                        }
                        let shared = self.tris[*other]
                            .iter()
                            .find(|w| **w != v && cur.contains(w));
                        if shared.is_some_and(|w| !self.crease.contains(&key(v, *w))) {
                            fan.push(*other);
                        }
                    }
                    k += 1;
                }
                let mut near: Vec<usize> = fan
                    .iter()
                    .flat_map(|f| self.tris[*f])
                    .filter(|w| *w != v)
                    .collect();
                near.sort_unstable();
                near.dedup();
                let g = self
                    .fit(v, &near)
                    .unwrap_or_else(|| plane(&self.tris[fan[0]]));
                for f in fan {
                    let corner = self.tris[f].iter().position(|w| *w == v).unwrap_or(0);
                    out[f][corner] = g;
                    done[f][corner] = true;
                }
            }
        }
        // then each slope refitted a few times so that the curve along
        // every edge bends as little as it can between its two ends
        // (the least-curvature cubic to a neighbour with slope g rises
        // 1.5 times the edge's rise less half of g along it)
        let folded: HashSet<usize> = self.crease.iter().flat_map(|e| [e.0, e.1]).collect();
        let mut first: Vec<Option<(usize, usize)>> = vec![None; self.verts.len()];
        for (ti, t) in self.tris.iter().enumerate() {
            for (c, v) in t.iter().enumerate() {
                first[*v].get_or_insert((ti, c));
            }
        }
        let mut ring: Vec<Vec<usize>> = vec![vec![]; self.verts.len()];
        for t in &self.tris {
            for i in 0..3 {
                ring[t[i]].push(t[(i + 1) % 3]);
                ring[t[i]].push(t[(i + 2) % 3]);
            }
        }
        for r in ring.iter_mut() {
            r.sort_unstable();
            r.dedup();
        }
        let mut g: Vec<DVec2> = first
            .iter()
            .map(|f| f.map_or(DVec2::ZERO, |(t, c)| out[t][c]))
            .collect();
        for _ in 0..6 {
            for v in 0..self.verts.len() {
                if folded.contains(&v) || ring[v].len() < 3 {
                    continue;
                }
                let o = self.verts[v];
                let (mut sxx, mut sxy, mut syy, mut sxz, mut syz) = (0.0, 0.0, 0.0, 0.0, 0.0);
                for n in &ring[v] {
                    let d = self.verts[*n] - o;
                    let e = d.truncate();
                    let w = 1.0 / e.length_squared().max(1e-12).powf(1.5);
                    // a neighbour on a break line has a slope each side
                    // of it: the edge to it is taken as it rises
                    let want = if folded.contains(n) {
                        d.z
                    } else {
                        1.5 * d.z - 0.5 * g[*n].dot(e)
                    };
                    sxx += w * e.x * e.x;
                    sxy += w * e.x * e.y;
                    syy += w * e.y * e.y;
                    sxz += w * e.x * want;
                    syz += w * e.y * want;
                }
                let det = sxx * syy - sxy * sxy;
                if det.abs() > 1e-9 * (sxx * syy).max(1e-12) {
                    g[v] = DVec2::new((sxz * syy - syz * sxy) / det, (syz * sxx - sxz * sxy) / det);
                }
            }
        }
        for (ti, t) in self.tris.iter().enumerate() {
            for (c, v) in t.iter().enumerate() {
                if !folded.contains(v) {
                    out[ti][c] = g[*v];
                }
            }
        }
        out
    }

    /// The slope of the plane through corner `v` that best fits the
    /// corners `near` it (nearer ones count for more).
    fn fit(&self, v: usize, near: &[usize]) -> Option<DVec2> {
        let o = self.verts[v];
        let (mut sxx, mut sxy, mut syy, mut sxz, mut syz) = (0.0, 0.0, 0.0, 0.0, 0.0);
        for n in near {
            let d = self.verts[*n] - o;
            let w = 1.0 / (d.x * d.x + d.y * d.y).max(1e-12);
            sxx += w * d.x * d.x;
            sxy += w * d.x * d.y;
            syy += w * d.y * d.y;
            sxz += w * d.x * d.z;
            syz += w * d.y * d.z;
        }
        let det = sxx * syy - sxy * sxy;
        // corners in one line give no plane
        if near.len() < 2 || det.abs() < 1e-9 * (sxx * syy).max(1e-12) {
            return None;
        }
        Some(DVec2::new(
            (sxz * syy - syz * sxy) / det,
            (syz * sxx - sxz * sxy) / det,
        ))
    }

    fn patch(&self, ti: usize) -> Patch {
        let t = self.tris[ti];
        let p = t.map(|i| self.verts[i]);
        let f = [p[0].z, p[1].z, p[2].z];
        let mut e = [[0.0; 2]; 3];
        for i in 0..3 {
            for (slot, j) in [(0, (i + 1) % 3), (1, (i + 2) % 3)] {
                let straight = f[i] + (f[j] - f[i]) / 3.0;
                let edge = key(t[i], t[j]);
                e[i][slot] =
                    if !self.smooth || self.crease.contains(&edge) || self.held.contains(&edge) {
                        straight
                    } else {
                        // off the straight line by no more than a part of
                        // the edge's length: a slope fitted across a
                        // cliff must not throw the ground up beside it
                        let along = (p[j] - p[i]).truncate();
                        let limit = 0.15 * along.length();
                        // and never above the highest height surveyed
                        // round its corner nor below the lowest: a few
                        // spot heights far apart cannot raise a hill
                        // (or sink a hollow) nobody surveyed
                        let (lo, hi) = self.span[t[i]];
                        // and along an edge that rises it climbs no
                        // faster than half as much again as the edge rises, nor
                        // turns back, beyond what the heights round the
                        // corner leave room for: a steep slope beside a
                        // spot height does not carry on past it
                        let (low, high) = (f[i].min(f[j]), f[i].max(f[j]));
                        let (down, up) = (0.75 * (low - lo), 0.75 * (hi - high));
                        let rise = 1.5 * (f[j] - f[i]) / 3.0;
                        (f[i] + self.grads[ti][i].dot(along) / 3.0)
                            .clamp(straight - limit, straight + limit)
                            .clamp(f[i] + rise.min(0.0) - down, f[i] + rise.max(0.0) + up)
                            .clamp(lo, hi)
                    };
            }
        }
        let edge = e.iter().flatten().sum::<f64>() / 6.0;
        let corner = (f[0] + f[1] + f[2]) / 3.0;
        let (lo, hi) = t.iter().fold((f64::MAX, f64::MIN), |(lo, hi), v| {
            (lo.min(self.span[*v].0), hi.max(self.span[*v].1))
        });
        Patch {
            f,
            e,
            mid: (edge + (edge - corner) / 2.0).clamp(lo, hi),
        }
    }

    /// The surface as drawn: each triangle divided and lifted onto its
    /// patch when smooth, the triangles themselves when not.
    fn divide(&self) -> Fine {
        let n: usize = match (self.smooth, self.tris.len()) {
            (false, _) => 1,
            // a few large triangles (a ground drawn from a handful of
            // spot levels) are divided until a piece is about a metre
            // and a half, so its contours curve; never more than 12 a
            // side, nor more than 16,000 pieces in all (every piece is
            // drawn, snapped to and picked in 3D: 60,000 made hovering
            // over a surveyed ground slow)
            (true, t @ 0..=4000) => {
                let longest = self
                    .tris
                    .iter()
                    .flat_map(|tr| {
                        (0..3).map(|i| {
                            self.verts[tr[i]]
                                .truncate()
                                .distance(self.verts[tr[(i + 1) % 3]].truncate())
                        })
                    })
                    .fold(0.0, f64::max);
                let wanted = (longest / 1.5).ceil() as usize;
                let room = ((16_000.0 / t.max(1) as f64).sqrt()).floor() as usize;
                wanted.clamp(4, 12).min(room.max(4))
            }
            (true, 4001..=30000) => 2,
            _ => 1,
        };
        let mut fine = Fine {
            verts: self.verts.clone(),
            tris: vec![],
            sharp: vec![false; self.verts.len()],
        };
        for e in &self.crease {
            fine.sharp[e.0] = true;
            fine.sharp[e.1] = true;
        }
        if n == 1 {
            fine.tris = self.tris.iter().map(|t| t.map(|i| i as u32)).collect();
            fine.upright_sharp();
            return fine;
        }
        // points along an edge are shared by the triangles either side
        let mut on_edge: HashMap<(usize, usize, usize), u32> = HashMap::new();
        for (ti, t) in self.tris.iter().enumerate() {
            let patch = self.patch(ti);
            let p = t.map(|i| self.verts[i].truncate());
            // a triangle standing near upright (an upright cut's face) is
            // all but a line in plan: any curve fitted over it folds its
            // pieces back and forth, dark wedges at the face's corners
            // and bends (owner, cut test). It stays flat.
            let q = t.map(|i| self.verts[i]);
            let normal = (q[1] - q[0]).cross(q[2] - q[0]);
            let upright = normal.z.abs() < 0.3 * normal.length();
            let f = q.map(|v| v.z);
            let mut local = vec![0u32; (n + 1) * (n + 2) / 2];
            let at = |i: usize, j: usize| i * (2 * n + 3 - i) / 2 + j;
            for i in 0..=n {
                for j in 0..=n - i {
                    let k = n - i - j;
                    // i steps towards corner 1, j towards corner 2
                    let (u, v, w) = (
                        k as f64 / n as f64,
                        i as f64 / n as f64,
                        j as f64 / n as f64,
                    );
                    let corner = [(k, 0), (i, 1), (j, 2)]
                        .into_iter()
                        .find(|(c, _)| *c == n)
                        .map(|(_, c)| t[c]);
                    let id = if let Some(c) = corner {
                        c as u32
                    } else {
                        // on an edge: between two corners, counted from
                        // the lower-numbered one
                        let edge = if j == 0 {
                            Some((t[0], t[1], i))
                        } else if k == 0 {
                            Some((t[1], t[2], j))
                        } else if i == 0 {
                            Some((t[2], t[0], k))
                        } else {
                            None
                        };
                        let xy = p[0] * u + p[1] * v + p[2] * w;
                        let make = |fine: &mut Fine, sharp: bool| {
                            let z = if upright {
                                f[0] * u + f[1] * v + f[2] * w
                            } else {
                                patch.at(u, v, w)
                            };
                            fine.verts.push(xy.extend(z));
                            fine.sharp.push(sharp);
                            fine.verts.len() as u32 - 1
                        };
                        match edge {
                            Some((a, b, steps)) => {
                                let (lo, hi) = key(a, b);
                                let s = if a == lo { steps } else { n - steps };
                                let sharp = self.crease.contains(&(lo, hi));
                                *on_edge
                                    .entry((lo, hi, s))
                                    .or_insert_with(|| make(&mut fine, sharp))
                            }
                            None => make(&mut fine, false),
                        }
                    };
                    local[at(i, j)] = id;
                }
            }
            for i in 0..n {
                for j in 0..n - i {
                    fine.tris
                        .push([local[at(i, j)], local[at(i + 1, j)], local[at(i, j + 1)]]);
                    if i + j + 1 < n {
                        fine.tris.push([
                            local[at(i + 1, j)],
                            local[at(i + 1, j + 1)],
                            local[at(i, j + 1)],
                        ]);
                    }
                }
            }
        }
        fine.upright_sharp();
        fine
    }

    fn bucket(&self) -> Grid {
        let Some(first) = self.verts.first().map(|v| v.truncate()) else {
            return Grid::default();
        };
        let (lo, hi) = self.verts.iter().fold((first, first), |(lo, hi), v| {
            (lo.min(v.truncate()), hi.max(v.truncate()))
        });
        let size = (hi - lo).max(DVec2::splat(1e-6));
        let per_side = (self.tris.len() as f64).sqrt().ceil().clamp(1.0, 256.0);
        let cell = (size.max_element() / per_side).max(1e-6);
        let (nx, ny) = (
            (size.x / cell).ceil().max(1.0) as usize,
            (size.y / cell).ceil().max(1.0) as usize,
        );
        let mut cells = vec![vec![]; nx * ny];
        for (i, t) in self.tris.iter().enumerate() {
            let p = t.map(|v| self.verts[v].truncate());
            let (a, b) = (p[0].min(p[1]).min(p[2]), p[0].max(p[1]).max(p[2]));
            let ix = |x: f64| (((x - lo.x) / cell) as usize).min(nx - 1);
            let iy = |y: f64| (((y - lo.y) / cell) as usize).min(ny - 1);
            for y in iy(a.y)..=iy(b.y) {
                for x in ix(a.x)..=ix(b.x) {
                    cells[y * nx + x].push(i as u32);
                }
            }
        }
        Grid {
            lo,
            cell,
            nx,
            ny,
            cells,
        }
    }

    /// The ground's height above its level at `p`; none outside it.
    pub fn height(&self, p: DVec2) -> Option<f64> {
        let g = &self.grid;
        if g.cells.is_empty() {
            return None;
        }
        let q = (p - g.lo) / g.cell;
        if q.x < -1e-9 || q.y < -1e-9 {
            return None;
        }
        let (x, y) = ((q.x as usize).min(g.nx - 1), (q.y as usize).min(g.ny - 1));
        for ti in &g.cells[y * g.nx + x] {
            let t = self.tris[*ti as usize];
            let [a, b, c] = t.map(|v| self.verts[v].truncate());
            let area = (b - a).perp_dot(c - a);
            let u = (b - p).perp_dot(c - p) / area;
            let v = (c - p).perp_dot(a - p) / area;
            let w = 1.0 - u - v;
            if u >= -1e-9 && v >= -1e-9 && w >= -1e-9 {
                return Some(self.patch(*ti as usize).at(u, v, w));
            }
        }
        None
    }

    /// The lowest and highest of the surface, above its level.
    pub fn range(&self) -> (f64, f64) {
        self.fine
            .verts
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
                (lo.min(v.z), hi.max(v.z))
            })
    }

    /// The surface's triangles as drawn, in model coordinates (`base`
    /// the level's elevation).
    pub fn triangles(&self, base: f64) -> Vec<[DVec3; 3]> {
        let up = DVec3::Z * base;
        self.fine
            .tris
            .iter()
            .map(|t| t.map(|i| self.fine.verts[i as usize] + up))
            .collect()
    }

    /// A normal at each corner of the drawn surface: the ground's own
    /// where it is smooth, none along a break line (each triangle there
    /// keeps its own, so the fold shows).
    pub fn normals(&self) -> Vec<Option<DVec3>> {
        let mut sum = vec![DVec3::ZERO; self.fine.verts.len()];
        for t in &self.fine.tris {
            let [a, b, c] = t.map(|i| self.fine.verts[i as usize]);
            let n = (b - a).cross(c - a);
            for i in t {
                sum[*i as usize] += n;
            }
        }
        sum.iter()
            .zip(&self.fine.sharp)
            .map(|(n, sharp)| (self.smooth && !sharp).then(|| n.normalize_or(DVec3::Z)))
            .collect()
    }

    /// The surface's outer edge as drawn, in order round the terrain:
    /// pairs of corners with the ground on their left.
    pub fn rim(&self) -> Vec<(DVec3, DVec3)> {
        let mut count: HashMap<(u32, u32), (u32, u32, u8)> = HashMap::new();
        for t in &self.fine.tris {
            for i in 0..3 {
                let (a, b) = (t[i], t[(i + 1) % 3]);
                let e = count.entry((a.min(b), a.max(b))).or_insert((a, b, 0));
                e.2 += 1;
            }
        }
        let mut rim: Vec<(u32, u32)> = count
            .values()
            .filter(|e| e.2 == 1)
            .map(|e| (e.0, e.1))
            .collect();
        rim.sort_unstable();
        rim.iter()
            .map(|(a, b)| (self.fine.verts[*a as usize], self.fine.verts[*b as usize]))
            .collect()
    }

    /// Contours every `interval` of elevation (`base` the level's), each
    /// a run of points. The lines of a smooth ground are rounded through
    /// their points; they keep their corners on break lines.
    pub fn contours(&self, interval: f64, base: f64) -> Vec<Contour> {
        if interval <= 0.0 || !interval.is_finite() {
            return vec![];
        }
        let (lo, hi) = self.range();
        if !(lo.is_finite() && hi.is_finite()) || (hi - lo) / interval > 4000.0 {
            return vec![];
        }
        // segments by level: each end is the edge of the drawn surface it
        // crosses, so the pieces of one line share ends exactly
        type End = (u32, u32);
        let mut by_level: HashMap<i64, Vec<(End, End)>> = HashMap::new();
        let mut where_at: HashMap<(i64, End), (DVec2, bool)> = HashMap::new();
        let v = &self.fine.verts;
        for t in &self.fine.tris {
            let z = t.map(|i| v[i as usize].z + base);
            let (tlo, thi) = (z[0].min(z[1]).min(z[2]), z[0].max(z[1]).max(z[2]));
            let mut k = (tlo / interval).ceil() as i64;
            while (k as f64) * interval <= thi {
                // a hair above the level, so a corner lying exactly on it
                // counts as below and a flat triangle gives no line
                let h = (k as f64) * interval + 1e-7;
                let mut ends = vec![];
                for i in 0..3 {
                    let (a, b) = (t[i], t[(i + 1) % 3]);
                    let (za, zb) = (z[i], z[(i + 1) % 3]);
                    if (za > h) != (zb > h) {
                        let e = (a.min(b), a.max(b));
                        where_at.entry((k, e)).or_insert_with(|| {
                            let s = (h - za) / (zb - za);
                            let (pa, pb) = (v[a as usize], v[b as usize]);
                            let sharp = self.fine.sharp[a as usize] && self.fine.sharp[b as usize];
                            ((pa + (pb - pa) * s).truncate(), sharp)
                        });
                        ends.push(e);
                    }
                }
                if ends.len() == 2 {
                    by_level.entry(k).or_default().push((ends[0], ends[1]));
                }
                k += 1;
            }
        }
        let mut out = vec![];
        let mut levels: Vec<i64> = by_level.keys().copied().collect();
        levels.sort_unstable();
        for k in levels {
            let segs = &by_level[&k];
            let mut at: HashMap<End, Vec<usize>> = HashMap::new();
            for (i, s) in segs.iter().enumerate() {
                at.entry(s.0).or_default().push(i);
                at.entry(s.1).or_default().push(i);
            }
            let mut used = vec![false; segs.len()];
            for start in 0..segs.len() {
                if used[start] {
                    continue;
                }
                used[start] = true;
                let mut run = std::collections::VecDeque::from([segs[start].0, segs[start].1]);
                for front in [false, true] {
                    loop {
                        let end = if front { run[0] } else { run[run.len() - 1] };
                        let Some(next) = at[&end].iter().copied().find(|i| !used[*i]) else {
                            break;
                        };
                        used[next] = true;
                        let other = if segs[next].0 == end {
                            segs[next].1
                        } else {
                            segs[next].0
                        };
                        if front {
                            run.push_front(other);
                        } else {
                            run.push_back(other);
                        }
                    }
                }
                let closed = run.len() > 3 && run[0] == run[run.len() - 1];
                if closed {
                    run.pop_back();
                }
                let pts: Vec<(DVec2, bool)> = run.iter().map(|e| where_at[&(k, *e)]).collect();
                let points = if self.smooth {
                    round(&pts, closed)
                } else {
                    pts.iter().map(|p| p.0).collect()
                };
                out.push(Contour {
                    z: (k as f64) * interval - base,
                    index: k,
                    points: thin(&points, 0.004),
                    closed,
                });
            }
        }
        out
    }
}

/// Is `p` on the outline, within `tol`?
fn on_outline(outline: &[DVec2], p: DVec2, tol: f64) -> bool {
    let n = outline.len();
    (0..n).any(|i| {
        let (a, b) = (outline[i], outline[(i + 1) % n]);
        let d = b - a;
        let t = ((p - a).dot(d) / d.length_squared().max(1e-18)).clamp(0.0, 1.0);
        (a + d * t).distance(p) < tol
    })
}

/// A line through `pts` as a curve (Catmull-Rom, which passes through
/// every point), keeping a corner wherever a point is marked sharp.
fn round(pts: &[(DVec2, bool)], closed: bool) -> Vec<DVec2> {
    let n = pts.len();
    if n < 3 {
        return pts.iter().map(|p| p.0).collect();
    }
    let get = |i: isize| -> DVec2 {
        if closed {
            pts[i.rem_euclid(n as isize) as usize].0
        } else {
            pts[i.clamp(0, n as isize - 1) as usize].0
        }
    };
    let spans = if closed { n } else { n - 1 };
    let mut out = vec![];
    for i in 0..spans {
        let (a, b) = (i as isize, i as isize + 1);
        let (p1, p2) = (get(a), get(b));
        // at a sharp point the curve leaves straight
        let p0 = if pts[i].1 { p1 - (p2 - p1) } else { get(a - 1) };
        let p3 = if pts[(i + 1) % n].1 {
            p2 + (p2 - p1)
        } else {
            get(b + 1)
        };
        out.push(p1);
        let turn = (p1 - p0)
            .angle_to(p2 - p1)
            .abs()
            .max((p2 - p1).angle_to(p3 - p2).abs());
        let steps = if turn < 0.03 {
            1
        } else if turn < 0.3 {
            3
        } else {
            5
        };
        for s in 1..steps {
            let t = s as f64 / steps as f64;
            let (t2, t3) = (t * t, t * t * t);
            out.push(
                0.5 * ((2.0 * p1)
                    + (p2 - p0) * t
                    + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
                    + (3.0 * p1 - p0 - 3.0 * p2 + p3) * t3),
            );
        }
    }
    if !closed {
        out.push(pts[n - 1].0);
    }
    out
}

/// `pts` without the points that lie within `tol` of the line through
/// their neighbours (Douglas-Peucker).
fn thin(pts: &[DVec2], tol: f64) -> Vec<DVec2> {
    if pts.len() < 3 {
        return pts.to_vec();
    }
    let mut keep = vec![false; pts.len()];
    keep[0] = true;
    keep[pts.len() - 1] = true;
    let mut stack = vec![(0, pts.len() - 1)];
    while let Some((a, b)) = stack.pop() {
        let (pa, d) = (pts[a], pts[b] - pts[a]);
        let len = d.length().max(1e-12);
        let (mut far, mut at) = (tol, None);
        for (i, p) in pts.iter().enumerate().take(b).skip(a + 1) {
            let dist = if len < 1e-9 {
                p.distance(pa)
            } else {
                (d.perp_dot(*p - pa) / len).abs()
            };
            if dist > far {
                (far, at) = (dist, Some(i));
            }
        }
        if let Some(i) = at {
            keep[i] = true;
            stack.push((a, i));
            stack.push((i, b));
        }
    }
    pts.iter()
        .zip(keep)
        .filter_map(|(p, k)| k.then_some(*p))
        .collect()
}

/// Shared by every thread: the app builds each scene and drawing on a
/// thread of its own, which found a cache of its own empty every time
/// (MU142 lag: the scene 240 ms on a new thread, 140 ms warm).
static CACHE: std::sync::Mutex<Vec<(u64, Arc<Ground>)>> = std::sync::Mutex::new(Vec::new());

/// A cache shared by every thread, held only to look up or put away.
pub(crate) fn kept<T>(m: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// The ground of a terrain, kept from one drawing to the next while its
/// data stays the same.
pub fn of(site: &Site) -> Option<Arc<Ground>> {
    let k = data_key(site)?;
    {
        let mut c = kept(&CACHE);
        if let Some(i) = c.iter().position(|e| e.0 == k) {
            let hit = c.remove(i);
            c.push(hit.clone());
            return Some(hit.1);
        }
    }
    let g = Arc::new(Ground::build(site)?);
    let mut c = kept(&CACHE);
    c.retain(|e| e.0 != k);
    if c.len() >= 12 {
        c.remove(0);
    }
    c.push((k, g.clone()));
    Some(g)
}

/// What a terrain's ground depends on, as one number: two terrains with
/// the same have the same ground.
pub fn data_key(site: &Site) -> Option<u64> {
    use std::hash::{Hash, Hasher};
    let SiteForm::Terrain {
        boundary,
        top,
        points,
        data,
        ..
    } = &site.form
    else {
        return None;
    };
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let mut f = |v: f64| v.to_bits().hash(&mut h);
    f(*top);
    boundary.iter().for_each(|p| {
        f(p.x);
        f(p.y)
    });
    f(f64::NAN);
    for p in points
        .iter()
        .chain(data.lines.iter().flat_map(|l| l.points.iter()))
    {
        f(p.x);
        f(p.y);
        f(p.z);
    }
    for l in &data.lines {
        (l.points.len(), l.kind == GroundLineKind::Break, l.closed).hash(&mut h);
    }
    (data.survey, data.faceted).hash(&mut h);
    Some(h.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use creabim_core::{ElementId, GroundLine};

    fn square(size: f64) -> Site {
        Site::terrain(
            ElementId::new(),
            creabim_core::polygon::rectangle(DVec2::ZERO, DVec2::splat(size)),
            0.0,
            0.5,
        )
    }

    fn data(s: &mut Site) -> (&mut Vec<DVec3>, &mut creabim_core::GroundData) {
        match &mut s.form {
            SiteForm::Terrain { points, data, .. } => (points, data),
            _ => unreachable!(),
        }
    }

    #[test]
    fn the_ground_passes_through_every_surveyed_height() {
        let mut s = square(20.0);
        let pts = [
            DVec3::new(5.0, 5.0, 1.0),
            DVec3::new(15.0, 6.0, 2.5),
            DVec3::new(9.0, 14.0, -0.7),
            DVec3::new(16.0, 16.0, 3.0),
        ];
        data(&mut s).0.extend(pts);
        for faceted in [false, true] {
            data(&mut s).1.faceted = faceted;
            let g = Ground::build(&s).unwrap();
            for p in pts {
                let h = g.height(p.truncate()).unwrap();
                assert!((h - p.z).abs() < 1e-9, "{h} at {p}");
            }
            // and the outline's corners stay at the terrain's top
            assert!(g.height(DVec2::ZERO).unwrap().abs() < 1e-9);
            assert!(g.height(DVec2::new(30.0, 5.0)).is_none());
        }
    }

    #[test]
    fn a_plane_stays_a_plane_when_smoothed() {
        // a survey of a plane: the smooth ground adds no bumps
        let mut s = square(10.0);
        let plane = |p: DVec2| 0.1 * p.x - 0.2 * p.y + 3.0;
        let (points, d) = data(&mut s);
        d.survey = true;
        for i in 0..=5 {
            for j in 0..=5 {
                let p = DVec2::new(i as f64 * 2.0, j as f64 * 2.0);
                points.push(p.extend(plane(p)));
            }
        }
        let g = Ground::build(&s).unwrap();
        assert!(g.smooth);
        for p in [
            DVec2::new(3.3, 4.7),
            DVec2::new(0.4, 9.1),
            DVec2::new(7.7, 7.2),
        ] {
            assert!((g.height(p).unwrap() - plane(p)).abs() < 1e-9);
        }
        // contours of a plane are straight: two points each
        let c = g.contours(0.5, 0.0);
        assert!(!c.is_empty());
        assert!(c.iter().all(|c| c.points.len() == 2), "{c:?}");
    }

    #[test]
    fn a_few_spot_heights_raise_no_hill_above_them() {
        // the first thing anyone tries: an outline at the level and four
        // spot heights, two low and two high (the stage 0 walk: the
        // ground reached 4.56 m from spots of 0.50 to 2.00 m)
        let mut s = square(30.0);
        let (points, _) = data(&mut s);
        points.push(DVec3::new(3.0, 3.0, 0.5));
        points.push(DVec3::new(27.0, 3.0, 0.5));
        points.push(DVec3::new(3.0, 27.0, 2.0));
        points.push(DVec3::new(27.0, 27.0, 2.0));
        let g = Ground::build(&s).unwrap();
        assert!(g.smooth);
        for i in 0..=60 {
            for j in 0..=60 {
                let z = g
                    .height(DVec2::new(i as f64 * 0.5, j as f64 * 0.5))
                    .unwrap();
                assert!((-1e-9..=2.0 + 1e-9).contains(&z), "{z} at {i} {j}");
            }
        }
        // and it still passes through each spot height
        assert!((g.height(DVec2::new(27.0, 27.0)).unwrap() - 2.0).abs() < 1e-9);
        assert!((g.height(DVec2::new(3.0, 3.0)).unwrap() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn the_smooth_ground_curves_between_points_and_the_facets_do_not() {
        // a rounded hill sampled coarsely
        let hill = |p: DVec2| 3.0 * (-(p - DVec2::splat(10.0)).length_squared() / 120.0).exp();
        let mut s = square(20.0);
        let (points, d) = data(&mut s);
        d.survey = true;
        for i in 0..=4 {
            for j in 0..=4 {
                let p = DVec2::new(i as f64 * 5.0, j as f64 * 5.0);
                points.push(p.extend(hill(p)));
            }
        }
        let err = |g: &Ground| {
            let mut worst: f64 = 0.0;
            for i in 0..40 {
                for j in 0..40 {
                    let p = DVec2::new(0.25 + i as f64 * 0.5, 0.25 + j as f64 * 0.5);
                    worst = worst.max((g.height(p).unwrap() - hill(p)).abs());
                }
            }
            worst
        };
        let smooth = Ground::build(&s).unwrap();
        data(&mut s).1.faceted = true;
        let flat = Ground::build(&s).unwrap();
        assert!(
            err(&smooth) < err(&flat) * 0.5,
            "{} {}",
            err(&smooth),
            err(&flat)
        );
        // the smooth surface is drawn finer than the survey's triangles
        assert!(smooth.fine.tris.len() > flat.fine.tris.len() * 10);
        assert_eq!(flat.fine.tris.len(), flat.tris.len());
    }

    #[test]
    fn no_triangle_crosses_a_line_and_the_ground_folds_along_a_break_line() {
        let mut s = square(20.0);
        let (points, d) = data(&mut s);
        // a kerb across the middle, 0.15 up on its far side
        d.lines.push(GroundLine {
            kind: GroundLineKind::Break,
            points: vec![DVec3::new(0.0, 10.0, 0.0), DVec3::new(20.0, 10.0, 0.0)],
            closed: false,
        });
        d.lines.push(GroundLine {
            kind: GroundLineKind::Break,
            points: vec![DVec3::new(0.0, 10.1, 0.15), DVec3::new(20.0, 10.1, 0.15)],
            closed: false,
        });
        points.push(DVec3::new(10.0, 15.0, 0.15));
        points.push(DVec3::new(10.0, 5.0, 0.0));
        let g = Ground::build(&s).unwrap();
        for t in &g.tris {
            let ys = t.map(|i| g.verts[i].y);
            let below = ys.iter().all(|y| *y <= 10.0 + 1e-9);
            let between = ys.iter().all(|y| (10.0 - 1e-9..=10.1 + 1e-9).contains(y));
            let above = ys.iter().all(|y| *y >= 10.1 - 1e-9);
            assert!(below || between || above, "{ys:?}");
        }
        // flat right up to the kerb, the step taken between the lines
        assert!(g.height(DVec2::new(7.0, 9.99)).unwrap().abs() < 1e-6);
        assert!((g.height(DVec2::new(7.0, 10.11)).unwrap() - 0.15).abs() < 1e-3);
        assert!((g.height(DVec2::new(7.0, 10.05)).unwrap() - 0.075).abs() < 1e-6);
    }

    #[test]
    fn contours_drawn_as_data_come_back_as_contours() {
        // two closed rings at 1 and 2: the ground between them gives
        // closed contours at 1.5, inside the outer ring
        let ring = |r: f64, z: f64| GroundLine {
            kind: GroundLineKind::Contour,
            points: (0..24)
                .map(|i| {
                    let a = i as f64 * std::f64::consts::TAU / 24.0;
                    (DVec2::splat(15.0) + DVec2::from_angle(a) * r).extend(z)
                })
                .collect(),
            closed: true,
        };
        let mut s = square(30.0);
        let (_, d) = data(&mut s);
        d.lines.push(ring(10.0, 1.0));
        d.lines.push(ring(5.0, 2.0));
        let g = Ground::build(&s).unwrap();
        let c: Vec<Contour> = g
            .contours(0.5, 0.0)
            .into_iter()
            .filter(|c| c.index == 3)
            .collect();
        assert_eq!(c.len(), 1, "{c:?}");
        assert!(c[0].closed);
        for p in &c[0].points {
            let r = p.distance(DVec2::splat(15.0));
            assert!((5.0..=10.0).contains(&r), "{r}");
            assert!((g.height(*p).unwrap() - 1.5).abs() < 0.05);
        }
    }

    #[test]
    fn a_survey_takes_its_edge_from_the_data_and_a_drawn_terrain_keeps_its_top() {
        let mut s = square(10.0);
        let (points, _) = data(&mut s);
        points.push(DVec3::new(5.0, 5.0, 4.0));
        let drawn = Ground::build(&s).unwrap();
        assert_eq!(drawn.height(DVec2::ZERO), Some(0.0));
        data(&mut s).1.survey = true;
        let survey = Ground::build(&s).unwrap();
        assert!((survey.height(DVec2::ZERO).unwrap() - 4.0).abs() < 1e-9);
    }

    #[test]
    fn a_big_survey_builds_quickly() {
        let mut s = square(200.0);
        let (points, d) = data(&mut s);
        d.survey = true;
        let mut seed = 7u64;
        let mut rnd = || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (seed >> 33) as f64 / (1u64 << 31) as f64
        };
        for _ in 0..5000 {
            let p = DVec2::new(rnd() * 200.0, rnd() * 200.0);
            points.push(p.extend((p.x / 30.0).sin() * 3.0 + p.y * 0.02));
        }
        let t = std::time::Instant::now();
        let g = Ground::build(&s).unwrap();
        let c = g.contours(0.5, 0.0);
        assert!(g.tris.len() > 9000);
        assert!(!c.is_empty());
        assert!(t.elapsed().as_secs_f64() < 5.0, "{:?}", t.elapsed());
    }
}
