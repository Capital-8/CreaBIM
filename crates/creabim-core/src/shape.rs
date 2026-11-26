//! The Shape: one free object that is lines, surfaces or solids, in one
//! or several pieces (MU43). It is the edit mesh of a [`crate::FreeMesh`]
//! with three things added here: lines that bound no face, a role
//! (solid, void or guide), and Pull as one operation whatever is pulled.
//!
//! Nothing here draws or picks: these are the operations the tools call.

use crate::{EditMesh, Element, ElementId, ElementKind, FreeMesh, Model, Transaction};
use glam::DVec3;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// How far off a face a point drawn on it may be and still count as on
/// it. The face under the pointer is found from the picture's own
/// geometry, which is less exact than the model's.
const ON_FACE: f64 = 0.005;

/// What a shape is for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    /// Part of the building: drawn, scheduled, exported.
    #[default]
    Solid,
    /// Cuts what it is joined to and is itself left out of drawings.
    Void,
    /// Something to draw by: shown, snapped to, never exported.
    Guide,
}

impl Role {
    pub const ALL: [Role; 3] = [Role::Solid, Role::Void, Role::Guide];

    pub fn label(self) -> &'static str {
        match self {
            Role::Solid => "Solid",
            Role::Void => "Void",
            Role::Guide => "Guide",
        }
    }

    pub fn is_solid(&self) -> bool {
        *self == Role::Solid
    }
}

/// What a shape's geometry amounts to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Nothing at all.
    Empty,
    /// Lines only.
    Lines,
    /// Faces that do not close a volume (with or without lines).
    Surface,
    /// Faces closing one or more volumes, every edge shared by two faces.
    Solid,
}

/// One connected part of a shape: faces and lines that share corners.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Piece {
    /// Indices into [`EditMesh::faces`].
    pub faces: Vec<usize>,
    /// Indices into [`EditMesh::edges`].
    pub edges: Vec<usize>,
    /// The corners it uses, ascending.
    pub vertices: Vec<u32>,
}

/// What Pull acts on.
#[derive(Clone, Debug, PartialEq)]
pub enum Pulled {
    /// A face, along its normal: a face of a solid grows or sinks; a flat
    /// face standing alone becomes a solid.
    Face(usize),
    /// Lines (indices into [`EditMesh::edges`]), along `direction`: each
    /// becomes a surface.
    Lines(Vec<usize>, DVec3),
}

fn edge_key(a: u32, b: u32) -> (u32, u32) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

impl EditMesh {
    /// How many faces use each edge of the faces.
    fn edge_uses(&self) -> HashMap<(u32, u32), u32> {
        let mut uses: HashMap<(u32, u32), u32> = HashMap::new();
        for f in &self.faces {
            for i in 0..f.len() {
                *uses
                    .entry(edge_key(f[i], f[(i + 1) % f.len()]))
                    .or_default() += 1;
            }
        }
        uses
    }

    /// Lines, a surface or a solid.
    pub fn kind(&self) -> Kind {
        if self.faces.is_empty() {
            return if self.edges.is_empty() {
                Kind::Empty
            } else {
                Kind::Lines
            };
        }
        let closed = self.edges.is_empty() && self.edge_uses().values().all(|n| *n == 2);
        if closed {
            Kind::Solid
        } else {
            Kind::Surface
        }
    }

    /// Its connected parts, in the order of their first corner. Corners
    /// used by nothing are in no piece.
    pub fn pieces(&self) -> Vec<Piece> {
        let n = self.vertices.len();
        let mut parent: Vec<usize> = (0..n).collect();
        fn find(parent: &mut [usize], mut i: usize) -> usize {
            while parent[i] != i {
                parent[i] = parent[parent[i]];
                i = parent[i];
            }
            i
        }
        let join = |parent: &mut Vec<usize>, a: u32, b: u32| {
            let (a, b) = (find(parent, a as usize), find(parent, b as usize));
            if a != b {
                let (lo, hi) = (a.min(b), a.max(b));
                parent[hi] = lo;
            }
        };
        let mut used = vec![false; n];
        for f in &self.faces {
            for &v in f {
                used[v as usize] = true;
                join(&mut parent, f[0], v);
            }
        }
        for e in &self.edges {
            used[e[0] as usize] = true;
            used[e[1] as usize] = true;
            join(&mut parent, e[0], e[1]);
        }
        let mut of_root: HashMap<usize, usize> = HashMap::new();
        let mut out: Vec<Piece> = Vec::new();
        for (v, is_used) in used.iter().enumerate() {
            if !*is_used {
                continue;
            }
            let root = find(&mut parent, v);
            let at = *of_root.entry(root).or_insert_with(|| {
                out.push(Piece::default());
                out.len() - 1
            });
            out[at].vertices.push(v as u32);
        }
        for (i, f) in self.faces.iter().enumerate() {
            let root = find(&mut parent, f[0] as usize);
            out[of_root[&root]].faces.push(i);
        }
        for (i, e) in self.edges.iter().enumerate() {
            let root = find(&mut parent, e[0] as usize);
            out[of_root[&root]].edges.push(i);
        }
        out
    }

    /// Every corner of the pieces these corners are in: what picking a
    /// piece takes (Blender's linked, a SketchUp triple-click).
    pub fn linked(&self, corners: &[u32]) -> Vec<u32> {
        let mut out: Vec<u32> = self
            .pieces()
            .into_iter()
            .filter(|p| corners.iter().any(|c| p.vertices.binary_search(c).is_ok()))
            .flat_map(|p| p.vertices)
            .collect();
        out.sort_unstable();
        out
    }

    /// The mesh of one piece alone, its corners renumbered.
    pub fn piece_mesh(&self, piece: &Piece) -> EditMesh {
        let renumber: HashMap<u32, u32> = piece
            .vertices
            .iter()
            .enumerate()
            .map(|(k, v)| (*v, k as u32))
            .collect();
        EditMesh {
            vertices: piece
                .vertices
                .iter()
                .map(|v| self.vertices[*v as usize])
                .collect(),
            faces: piece
                .faces
                .iter()
                .map(|f| self.faces[*f].iter().map(|v| renumber[v]).collect())
                .collect(),
            edges: piece
                .edges
                .iter()
                .map(|e| {
                    let e = self.edges[*e];
                    [renumber[&e[0]], renumber[&e[1]]]
                })
                .collect(),
        }
    }

    /// Each piece as a mesh of its own: Separate.
    pub fn separate(&self) -> Vec<EditMesh> {
        self.pieces().iter().map(|p| self.piece_mesh(p)).collect()
    }

    /// Several meshes as one, each a piece of it: Join. `offsets` moves
    /// each as it goes in (where its own shape stood from the first).
    pub fn joined(parts: &[(&EditMesh, DVec3)]) -> EditMesh {
        let mut out = EditMesh::default();
        for (m, offset) in parts {
            let base = out.vertices.len() as u32;
            out.vertices.extend(m.vertices.iter().map(|v| *v + *offset));
            out.faces
                .extend(m.faces.iter().map(|f| f.iter().map(|v| v + base).collect()));
            out.edges
                .extend(m.edges.iter().map(|e| [e[0] + base, e[1] + base]));
        }
        out
    }

    /// A line from `a` to `b`, its corners welded to ones already there
    /// (within a hundredth of a millimetre). Returns its index, or none
    /// for a line of no length or one already drawn.
    pub fn add_line(&mut self, a: DVec3, b: DVec3) -> Option<usize> {
        let corner = |p: DVec3, mesh: &mut EditMesh| -> u32 {
            match mesh.vertices.iter().position(|v| v.distance(p) < 1e-5) {
                Some(i) => i as u32,
                None => {
                    mesh.vertices.push(p);
                    (mesh.vertices.len() - 1) as u32
                }
            }
        };
        let (ia, ib) = (corner(a, self), corner(b, self));
        if ia == ib {
            return None;
        }
        let key = edge_key(ia, ib);
        if self.edges.iter().any(|e| edge_key(e[0], e[1]) == key)
            || self.edge_uses().contains_key(&key)
        {
            return None;
        }
        self.edges.push([ia, ib]);
        Some(self.edges.len() - 1)
    }

    /// The closed loops the lines make, each as its corners in order, with
    /// the lines it is made of. Only simple loops: every corner of one
    /// joins exactly two of the lines.
    pub fn closed_loops(&self) -> Vec<(Vec<u32>, Vec<usize>)> {
        let mut at: HashMap<u32, Vec<usize>> = HashMap::new();
        for (i, e) in self.edges.iter().enumerate() {
            at.entry(e[0]).or_default().push(i);
            at.entry(e[1]).or_default().push(i);
        }
        let mut seen: HashSet<usize> = HashSet::new();
        let mut out = Vec::new();
        for start in 0..self.edges.len() {
            if seen.contains(&start) {
                continue;
            }
            let mut lines = vec![start];
            let mut corners = vec![self.edges[start][0]];
            let mut here = self.edges[start][1];
            let mut closed = false;
            loop {
                if here == corners[0] {
                    closed = true;
                    break;
                }
                let Some(next) = at.get(&here).filter(|v| v.len() == 2).and_then(|v| {
                    v.iter()
                        .copied()
                        .find(|l| *l != *lines.last().unwrap() && !lines.contains(l))
                }) else {
                    break;
                };
                corners.push(here);
                let e = self.edges[next];
                here = if e[0] == here { e[1] } else { e[0] };
                lines.push(next);
            }
            seen.extend(lines.iter().copied());
            let simple =
                closed && corners.len() >= 3 && at.get(&corners[0]).is_some_and(|v| v.len() == 2);
            if simple {
                out.push((corners, lines));
            }
        }
        out
    }

    /// Fill each closed loop of lines that lies in one plane with a face,
    /// the lines becoming its edges: what drawing a closed outline gives.
    /// The face looks along `up` (so an outline drawn on the ground faces
    /// up whichever way round it was drawn). Returns the new faces.
    pub fn fill_loops(&mut self, up: DVec3) -> Vec<usize> {
        let mut made = Vec::new();
        let mut gone: HashSet<usize> = HashSet::new();
        for (mut corners, lines) in self.closed_loops() {
            let pts: Vec<DVec3> = corners.iter().map(|v| self.vertices[*v as usize]).collect();
            let mut n = DVec3::ZERO;
            for i in 0..pts.len() {
                let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
                n += DVec3::new(
                    (a.y - b.y) * (a.z + b.z),
                    (a.z - b.z) * (a.x + b.x),
                    (a.x - b.x) * (a.y + b.y),
                );
            }
            let Some(unit) = n.try_normalize() else {
                continue;
            };
            let flat = pts.iter().all(|p| (*p - pts[0]).dot(unit).abs() < 1e-6);
            if !flat {
                continue;
            }
            if unit.dot(up) < 0.0 {
                corners.reverse();
            }
            self.faces.push(corners);
            made.push(self.faces.len() - 1);
            gone.extend(lines);
        }
        if !gone.is_empty() {
            let mut i = 0;
            self.edges.retain(|_| {
                i += 1;
                !gone.contains(&(i - 1))
            });
        }
        made
    }

    /// The corner of `face` at `p`: one it has, or a new one put on the
    /// edge `p` lies on, in every face and line that uses that edge. None
    /// if `p` is not on the face's outline.
    fn corner_on_outline(&mut self, face: usize, p: DVec3, tol: f64) -> Option<u32> {
        let f = self.faces.get(face)?.clone();
        if let Some(v) = f
            .iter()
            .find(|v| self.vertices[**v as usize].distance(p) < tol)
        {
            return Some(*v);
        }
        for i in 0..f.len() {
            let (a, b) = (f[i], f[(i + 1) % f.len()]);
            let (pa, pb) = (self.vertices[a as usize], self.vertices[b as usize]);
            let ab = pb - pa;
            let t = (p - pa).dot(ab) / ab.length_squared().max(1e-18);
            if t <= 0.0 || t >= 1.0 || (pa + ab * t).distance(p) >= tol {
                continue;
            }
            self.vertices.push(pa + ab * t);
            let v = (self.vertices.len() - 1) as u32;
            for g in &mut self.faces {
                for k in 0..g.len() {
                    let (x, y) = (g[k], g[(k + 1) % g.len()]);
                    if (x == a && y == b) || (x == b && y == a) {
                        g.insert(k + 1, v);
                        break;
                    }
                }
            }
            let key = edge_key(a, b);
            if let Some(e) = self.edges.iter().position(|e| edge_key(e[0], e[1]) == key) {
                let [x, y] = self.edges[e];
                self.edges[e] = [x, v];
                self.edges.push([v, y]);
            }
            return Some(v);
        }
        None
    }

    /// A line drawn across a face splits it in two (MU43, the cut line):
    /// `chain` runs on the face from a point of its outline to another,
    /// by any corners in between. The face keeps its number as one part;
    /// the other is new, and returned. None if the line does not lie on
    /// the face, does not start and end on its outline, leaves the face on
    /// the way, or only runs along an edge it already has.
    pub fn split_face(&mut self, face: usize, chain: &[DVec3]) -> Option<usize> {
        // drawn by hand on a face found by the pointer: within 5 mm of the
        // face it is on it, and an end within 10 mm of an edge is on it
        // (a hundredth of a millimetre refused lines that were plainly
        // across the face, MU45)
        const TOL: f64 = 0.01;
        if chain.len() < 2 || face >= self.faces.len() {
            return None;
        }
        let n = self.face_normal(face);
        let on = self.vertices[self.faces[face][0] as usize];
        if n == DVec3::ZERO || chain.iter().any(|p| (*p - on).dot(n).abs() > ON_FACE) {
            return None;
        }
        let chain: Vec<DVec3> = chain.iter().map(|p| *p - n * (*p - on).dot(n)).collect();
        let chain = &chain[..];
        // it stays inside the face: its corners between the ends, and the
        // middle of each of its pieces (a hollow face it would cross)
        let u = n.any_orthonormal_vector();
        let v = n.cross(u);
        let flat = |p: DVec3| glam::DVec2::new(p.dot(u), p.dot(v));
        let outline: Vec<glam::DVec2> = self.faces[face]
            .iter()
            .map(|i| flat(self.vertices[*i as usize]))
            .collect();
        let inside = |p: DVec3| crate::polygon::contains(&outline, flat(p));
        let inner = &chain[1..chain.len() - 1];
        if inner.iter().any(|p| !inside(*p))
            || chain.windows(2).any(|w| !inside((w[0] + w[1]) / 2.0))
        {
            return None;
        }
        let mut work = self.clone();
        let s = work.corner_on_outline(face, chain[0], TOL)?;
        let e = work.corner_on_outline(face, chain[chain.len() - 1], TOL)?;
        if s == e {
            return None;
        }
        let f = work.faces[face].clone();
        let m = f.len();
        let (i, j) = (
            f.iter().position(|x| *x == s)?,
            f.iter().position(|x| *x == e)?,
        );
        if inner.is_empty() && ((i + 1) % m == j || (j + 1) % m == i) {
            return None;
        }
        let mids: Vec<u32> = inner
            .iter()
            .map(|p| {
                work.vertices.push(*p);
                (work.vertices.len() - 1) as u32
            })
            .collect();
        // one part runs the outline from the start to the end and comes
        // back along the line; the other runs on from the end to the start
        let walk = |from: usize, to: usize| -> Vec<u32> {
            let mut out = vec![f[from]];
            let mut k = from;
            while k != to {
                k = (k + 1) % m;
                out.push(f[k]);
            }
            out
        };
        let mut a = walk(i, j);
        a.extend(mids.iter().rev());
        let mut b = walk(j, i);
        b.extend(mids.iter());
        if a.len() < 3 || b.len() < 3 {
            return None;
        }
        work.faces[face] = a;
        work.faces.push(b);
        *self = work;
        Some(self.faces.len() - 1)
    }

    /// A closed outline drawn inside a face becomes a face of its own
    /// (MU43): `ring` lies on the face, wholly inside it, touching neither
    /// its outline nor itself. The new face is returned, to pull in for a
    /// recess or out for a boss. The face round it keeps its number; a
    /// mesh face has no hole, so it comes back on itself along one seam
    /// from its outline to the ring (never drawn: both sides are it).
    pub fn inset_loop(&mut self, face: usize, ring: &[DVec3]) -> Option<usize> {
        let k = ring.len();
        if k < 3 || face >= self.faces.len() {
            return None;
        }
        let n = self.face_normal(face);
        let on = self.vertices[self.faces[face][0] as usize];
        if n == DVec3::ZERO || ring.iter().any(|p| (*p - on).dot(n).abs() > ON_FACE) {
            return None;
        }
        let ring: Vec<DVec3> = ring.iter().map(|p| *p - n * (*p - on).dot(n)).collect();
        let ring = &ring[..];
        let u = n.any_orthonormal_vector();
        let v = n.cross(u);
        let flat = |p: DVec3| glam::DVec2::new(p.dot(u), p.dot(v));
        let f = self.faces[face].clone();
        let m = f.len();
        let outer: Vec<glam::DVec2> = f.iter().map(|i| flat(self.vertices[*i as usize])).collect();
        let mut inner: Vec<glam::DVec2> = ring.iter().map(|p| flat(*p)).collect();
        let mut ring: Vec<DVec3> = ring.to_vec();
        // the same way round as the face, so it faces the same way
        if crate::polygon::signed_area2(&inner) * crate::polygon::signed_area2(&outer) < 0.0 {
            inner.reverse();
            ring.reverse();
        }
        if crate::polygon::signed_area2(&inner).abs() < 1e-9 {
            return None;
        }
        // two segments that properly cross, or touch
        let side = |a: glam::DVec2, b: glam::DVec2, p: glam::DVec2| (b - a).perp_dot(p - a);
        let meet = |a: glam::DVec2, b: glam::DVec2, c: glam::DVec2, d: glam::DVec2| {
            let (s1, s2, s3, s4) = (side(a, b, c), side(a, b, d), side(c, d, a), side(c, d, b));
            let within = |a: glam::DVec2, b: glam::DVec2, p: glam::DVec2| {
                (p - a).dot(b - a) >= -1e-9 && (p - b).dot(a - b) >= -1e-9
            };
            (s1 * s2 < 0.0 && s3 * s4 < 0.0)
                || (s1.abs() < 1e-9 && within(a, b, c))
                || (s2.abs() < 1e-9 && within(a, b, d))
                || (s3.abs() < 1e-9 && within(c, d, a))
                || (s4.abs() < 1e-9 && within(c, d, b))
        };
        // wholly inside, clear of the outline, and not crossing itself
        if inner.iter().any(|p| !crate::polygon::contains(&outer, *p)) {
            return None;
        }
        for i in 0..k {
            let (a, b) = (inner[i], inner[(i + 1) % k]);
            if (0..m).any(|j| meet(a, b, outer[j], outer[(j + 1) % m])) {
                return None;
            }
            for j in i + 2..k {
                if (j + 1) % k != i && meet(a, b, inner[j], inner[(j + 1) % k]) {
                    return None;
                }
            }
        }
        // the seam: the nearest corner of the outline to a corner of the
        // ring that sees it (nothing of either crosses the way between)
        let mut pairs: Vec<(f64, usize, usize)> = (0..m)
            .flat_map(|i| (0..k).map(move |j| (i, j)))
            .map(|(i, j)| (outer[i].distance(inner[j]), i, j))
            .collect();
        pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
        let sees = |i: usize, j: usize| {
            let (a, b) = (outer[i], inner[j]);
            let clear_of = |pts: &[glam::DVec2], skip: glam::DVec2| {
                let c = pts.len();
                (0..c).all(|t| {
                    let (p, q) = (pts[t], pts[(t + 1) % c]);
                    p == skip || q == skip || !meet(a, b, p, q)
                })
            };
            clear_of(&outer, a) && clear_of(&inner, b)
        };
        let (_, i, j) = pairs.into_iter().find(|(_, i, j)| sees(*i, *j))?;
        let base = self.vertices.len() as u32;
        self.vertices.extend(ring.iter().copied());
        let r = |t: usize| base + (t % k) as u32;
        // round the outline to the seam, in along it, round the ring the
        // other way, back out, and on round the outline
        let mut around: Vec<u32> = f[..=i].to_vec();
        around.extend((0..=k).map(|t| r(j + k - t)));
        around.extend(f[i..].iter().copied());
        self.faces[face] = around;
        self.faces.push((0..k).map(r).collect());
        Some(self.faces.len() - 1)
    }

    /// How far behind `face` the shape's far side is, where the face would
    /// come out if pulled straight in: the nearest face that looks the
    /// other way and holds the whole of this one. None for a face with
    /// nothing behind it, or one that is itself a whole side.
    pub fn through(&self, face: usize) -> Option<(usize, f64)> {
        let n = self.face_normal(face);
        let f = self.faces.get(face)?;
        if n == DVec3::ZERO || self.stands_square(face) {
            return None;
        }
        let pts: Vec<DVec3> = f.iter().map(|v| self.vertices[*v as usize]).collect();
        let u = n.any_orthonormal_vector();
        let v = n.cross(u);
        let flat = |p: DVec3| glam::DVec2::new(p.dot(u), p.dot(v));
        let mut best: Option<(usize, f64)> = None;
        for (g, loop_) in self.faces.iter().enumerate() {
            if g == face || self.face_normal(g).dot(n) > -0.999 {
                continue;
            }
            let depth = (pts[0] - self.vertices[loop_[0] as usize]).dot(n);
            if depth < 1e-6 || best.is_some_and(|b| b.1 <= depth) {
                continue;
            }
            let outline: Vec<glam::DVec2> = loop_
                .iter()
                .map(|i| flat(self.vertices[*i as usize]))
                .collect();
            if pts
                .iter()
                .all(|p| crate::polygon::contains(&outline, flat(*p)))
            {
                best = Some((g, depth));
            }
        }
        best
    }

    /// Pull a face right through the shape: a hole from this side to the
    /// far one, its walls where the face's edges ran. False, and nothing
    /// changed, if there is no far side to come out of (see
    /// [`EditMesh::through`]).
    pub fn pull_through(&mut self, face: usize) -> bool {
        let Some((far, depth)) = self.through(face) else {
            return false;
        };
        let mut work = self.clone();
        // in to the far side: walls, and the face lying on the far one
        work.extrude_face(face, -depth);
        let ring: Vec<u32> = work.faces[face].clone();
        let at: Vec<DVec3> = ring.iter().map(|v| work.vertices[*v as usize]).collect();
        // the same outline cut in the far face
        let Some(inner) = work.inset_loop(far, &at) else {
            return false;
        };
        // its corners are the walls' own: one corner where there were two
        let twin: HashMap<u32, u32> = work.faces[inner]
            .iter()
            .filter_map(|v| {
                let p = work.vertices[*v as usize];
                let k = at.iter().position(|q| q.distance(p) < 1e-6)?;
                Some((*v, ring[k]))
            })
            .collect();
        if twin.len() != ring.len() {
            return false;
        }
        for v in &mut work.faces[far] {
            if let Some(w) = twin.get(v) {
                *v = *w;
            }
        }
        // both ends open: the face that was pulled, and its print on the
        // far side
        let (a, b) = (face.max(inner), face.min(inner));
        work.faces.remove(a);
        work.faces.remove(b);
        if work.validate().is_err() {
            return false;
        }
        *self = work;
        true
    }

    /// Pull: one operation whatever is pulled. Returns the faces to carry
    /// on with (the pulled face under its number, or the new surfaces).
    pub fn pull(&mut self, what: &Pulled, distance: f64) -> Vec<usize> {
        match what {
            Pulled::Face(face) => {
                self.pull_face(*face, distance);
                vec![*face]
            }
            Pulled::Lines(lines, direction) => {
                let by = direction.normalize_or_zero() * distance;
                self.pull_lines(lines, by)
            }
        }
    }

    /// Do the faces around this one stand square to it, all its edges
    /// shared? Then pulling it only lengthens them.
    fn stands_square(&self, face: usize) -> bool {
        let n = self.face_normal(face);
        let f = &self.faces[face];
        let mine: HashSet<(u32, u32)> = (0..f.len())
            .map(|i| edge_key(f[i], f[(i + 1) % f.len()]))
            .collect();
        let mut met: HashSet<(u32, u32)> = HashSet::new();
        for (k, g) in self.faces.iter().enumerate() {
            if k == face {
                continue;
            }
            let shares: Vec<(u32, u32)> = (0..g.len())
                .map(|i| edge_key(g[i], g[(i + 1) % g.len()]))
                .filter(|e| mine.contains(e))
                .collect();
            if shares.is_empty() {
                // a face that only touches it at a corner would be bent
                if g.iter().any(|v| f.contains(v)) && self.face_normal(k).dot(n).abs() > 1e-6 {
                    return false;
                }
                continue;
            }
            if self.face_normal(k).dot(n).abs() > 1e-6 {
                return false;
            }
            met.extend(shares);
        }
        met.len() == mine.len()
    }

    /// A face along its normal. Where the faces around it stand square to
    /// it (a box, a prism), they lengthen and no edge is added. Otherwise
    /// it grows or sinks with new sides, as [`EditMesh::extrude_face`] has
    /// it. A flat face standing alone (none of its edges shared) becomes a
    /// solid: it keeps a copy of itself where it was, turned to face out,
    /// as the other end.
    pub fn pull_face(&mut self, face: usize, distance: f64) {
        if face >= self.faces.len() || distance.abs() < 1e-9 {
            return;
        }
        let uses = self.edge_uses();
        let f = &self.faces[face];
        let alone = (0..f.len()).all(|i| uses[&edge_key(f[i], f[(i + 1) % f.len()])] == 1);
        if !alone {
            if self.stands_square(face) {
                let by = self.face_normal(face) * distance;
                let verts = self.faces[face].clone();
                self.translate_vertices(&verts, by);
            } else {
                self.extrude_face(face, distance);
            }
            return;
        }
        let base = self.faces[face].clone();
        if distance > 0.0 {
            // the copy left behind looks back the way the face came
            self.extrude_face(face, distance);
            self.faces.push(base.into_iter().rev().collect());
        } else {
            // pulled backwards: the face that moves is the far end, turned
            // to face away, and the one left behind keeps facing forwards
            let n = self.face_normal(face);
            let moved: Vec<u32> = base
                .iter()
                .map(|&v| {
                    self.vertices.push(self.vertices[v as usize] + n * distance);
                    (self.vertices.len() - 1) as u32
                })
                .collect();
            for i in 0..base.len() {
                let j = (i + 1) % base.len();
                self.faces.push(vec![base[j], base[i], moved[i], moved[j]]);
            }
            self.faces.push(base);
            self.faces[face] = moved.into_iter().rev().collect();
        }
    }

    /// Faces lying in one plane that meet along an edge become one face,
    /// as SketchUp has it: after a pull the new sides run on from the old
    /// ones, and the line between them means nothing (MU45, MU47: "it
    /// doesn't consider this to be 1 face?"). Only where one of the two
    /// is new or moved since `before`: two faces the user cut apart and
    /// left where they were stay two. Returns how many faces went.
    pub fn merge_flat(&mut self, before: &EditMesh) -> usize {
        let key = |m: &EditMesh, f: &Vec<u32>| {
            let mut k: Vec<[i64; 3]> = f
                .iter()
                .map(|v| {
                    m.vertices[*v as usize]
                        .to_array()
                        .map(|c| (c * 1e6).round() as i64)
                })
                .collect();
            k.sort_unstable();
            k
        };
        let old: HashSet<Vec<[i64; 3]>> = before.faces.iter().map(|f| key(before, f)).collect();
        let simple = |f: &Vec<u32>| {
            let mut seen = HashSet::new();
            f.len() >= 3 && f.iter().all(|v| seen.insert(*v))
        };
        let mut gone = 0;
        'again: loop {
            let fresh: Vec<bool> = self
                .faces
                .iter()
                .map(|f| !old.contains(&key(self, f)))
                .collect();
            // each edge, and the faces on it with the way they run it
            let mut on: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
            let mut used: HashMap<u32, usize> = HashMap::new();
            for (fi, f) in self.faces.iter().enumerate() {
                for i in 0..f.len() {
                    on.entry(edge_key(f[i], f[(i + 1) % f.len()]))
                        .or_default()
                        .push(fi);
                    *used.entry(f[i]).or_default() += 1;
                }
            }
            let mut pairs: Vec<(usize, usize)> = on
                .values()
                .filter(|v| v.len() == 2 && v[0] != v[1])
                .map(|v| (v[0].min(v[1]), v[0].max(v[1])))
                .collect();
            pairs.sort_unstable();
            pairs.dedup();
            for (a, b) in pairs {
                if !(fresh[a] || fresh[b]) || !simple(&self.faces[a]) || !simple(&self.faces[b]) {
                    continue;
                }
                let (na, nb) = (self.face_normal(a), self.face_normal(b));
                let pa = self.vertices[self.faces[a][0] as usize];
                let flat = na.dot(nb) > 1.0 - 1e-9
                    && self.faces[b]
                        .iter()
                        .all(|v| (self.vertices[*v as usize] - pa).dot(na).abs() < 1e-6);
                if !flat {
                    continue;
                }
                let (ra, rb) = (self.faces[a].clone(), self.faces[b].clone());
                let n = ra.len();
                let in_b: HashSet<(u32, u32)> = (0..rb.len())
                    .map(|i| (rb[i], rb[(i + 1) % rb.len()]))
                    .collect();
                // the edges of `a` that `b` runs the other way
                let shared: Vec<bool> = (0..n)
                    .map(|i| in_b.contains(&(ra[(i + 1) % n], ra[i])))
                    .collect();
                let count = shared.iter().filter(|s| **s).count();
                if count == 0 || count == n {
                    continue;
                }
                // one run of them, from corner `p` to corner `q` of `a`
                let Some(start) = (0..n).find(|i| shared[*i] && !shared[(*i + n - 1) % n]) else {
                    continue;
                };
                if (0..count).any(|k| !shared[(start + k) % n]) {
                    continue;
                }
                let (p, q) = (ra[start], ra[(start + count) % n]);
                // a corner inside the run that another face stands on stays
                let inside: Vec<u32> = (1..count).map(|k| ra[(start + k) % n]).collect();
                if inside.iter().any(|v| used[v] > 2) {
                    continue;
                }
                let mut ring: Vec<u32> = (0..=n - count)
                    .map(|k| ra[(start + count + k) % n])
                    .collect();
                debug_assert_eq!((ring[0], *ring.last().unwrap()), (q, p));
                let Some(at_p) = rb.iter().position(|v| *v == p) else {
                    continue;
                };
                let m = rb.len();
                ring.extend((1..m - count).map(|k| rb[(at_p + k) % m]));
                if !simple(&ring) {
                    continue;
                }
                self.faces[a] = ring;
                self.faces.remove(b);
                gone += 1;
                continue 'again;
            }
            break;
        }
        if gone > 0 {
            self.drop_straight_corners();
            self.drop_unused_vertices();
        }
        gone
    }

    /// Corners lying on a straight run in every face they are in say
    /// nothing: they go.
    fn drop_straight_corners(&mut self) {
        loop {
            let mut straight: HashMap<u32, bool> = HashMap::new();
            for f in &self.faces {
                let n = f.len();
                for i in 0..n {
                    let (a, v, b) = (f[(i + n - 1) % n], f[i], f[(i + 1) % n]);
                    let (pa, pv, pb) = (
                        self.vertices[a as usize],
                        self.vertices[v as usize],
                        self.vertices[b as usize],
                    );
                    let (u, w) = (pv - pa, pb - pv);
                    let on_line = n > 3
                        && u.cross(w).length() < 1e-9 * (1.0 + u.length() * w.length())
                        && u.dot(w) > 0.0;
                    let e = straight.entry(v).or_insert(true);
                    *e = *e && on_line;
                }
            }
            for e in &self.edges {
                straight.insert(e[0], false);
                straight.insert(e[1], false);
            }
            // one at a time: taking one out changes what its neighbours see
            let Some(v) = straight.iter().filter(|(_, s)| **s).map(|(v, _)| *v).min() else {
                break;
            };
            for f in &mut self.faces {
                f.retain(|x| *x != v);
            }
        }
    }

    /// Vertices no face and no line uses go, and the rest are numbered
    /// again.
    fn drop_unused_vertices(&mut self) {
        let mut keep = vec![false; self.vertices.len()];
        for v in self
            .faces
            .iter()
            .flatten()
            .chain(self.edges.iter().flatten())
        {
            keep[*v as usize] = true;
        }
        let mut to = vec![0u32; keep.len()];
        let mut next = 0u32;
        for (i, k) in keep.iter().enumerate() {
            to[i] = next;
            if *k {
                next += 1;
            }
        }
        let mut i = 0;
        self.vertices.retain(|_| {
            i += 1;
            keep[i - 1]
        });
        for v in self
            .faces
            .iter_mut()
            .flatten()
            .chain(self.edges.iter_mut().flatten())
        {
            *v = to[*v as usize];
        }
    }

    /// Lines carried by `by`: each becomes a four-cornered surface between
    /// where it was and where it went, lines that share a corner sharing
    /// the edge between their surfaces. The pulled lines stop being loose
    /// lines (they are edges of the new faces). Returns the new faces.
    pub fn pull_lines(&mut self, lines: &[usize], by: DVec3) -> Vec<usize> {
        if by.length() < 1e-9 {
            return vec![];
        }
        let mut picked: Vec<usize> = lines
            .iter()
            .copied()
            .filter(|l| *l < self.edges.len())
            .collect();
        picked.sort_unstable();
        picked.dedup();
        let mut far: HashMap<u32, u32> = HashMap::new();
        let mut made = Vec::new();
        for &l in &picked {
            let [a, b] = self.edges[l];
            let mut lift = |v: u32, mesh: &mut EditMesh| -> u32 {
                *far.entry(v).or_insert_with(|| {
                    mesh.vertices.push(mesh.vertices[v as usize] + by);
                    (mesh.vertices.len() - 1) as u32
                })
            };
            let (fa, fb) = (lift(a, self), lift(b, self));
            self.faces.push(vec![a, b, fb, fa]);
            made.push(self.faces.len() - 1);
        }
        let gone: HashSet<usize> = picked.into_iter().collect();
        let mut i = 0;
        self.edges.retain(|_| {
            i += 1;
            !gone.contains(&(i - 1))
        });
        made
    }
}

/// Separate: a shape of several pieces becomes one shape a piece. The
/// first keeps the element (its name, what is joined to it); the others
/// are new, named after it. None if it is not a shape of several pieces.
pub fn separate(model: &Model, id: ElementId) -> Option<(Transaction, Vec<ElementId>)> {
    let e = model.get(id)?;
    let ElementKind::FreeMesh(f) = &e.kind else {
        return None;
    };
    let parts = f.mesh.separate();
    if parts.len() < 2 {
        return None;
    }
    let mut tx = Transaction::new("Separate shape");
    let mut ids = vec![id];
    for (i, mesh) in parts.into_iter().enumerate() {
        let mut piece = f.clone();
        piece.mesh = mesh;
        if i == 0 {
            let mut after = e.clone();
            after.kind = ElementKind::FreeMesh(piece);
            tx = tx.replace(e.clone(), after);
        } else {
            let new = Element::new(
                format!("{} {}", e.name, i + 1),
                ElementKind::FreeMesh(piece),
            );
            ids.push(new.id);
            tx = tx.insert(new);
        }
    }
    Some((tx, ids))
}

/// A drawn line that crosses a face of a shape, from its outline to its
/// outline, cuts that face in two (SketchUp's line on a face); a closed
/// outline drawn inside a face becomes a face of its own. Returns the
/// shape as it was, as it is now, and the new face. The drawing itself is
/// then not needed. None if `sketch` does neither on a face of a shape
/// without modifiers.
pub fn imprint(model: &Model, sketch: ElementId) -> Option<(Element, Element, usize)> {
    let ElementKind::Sketch(sk) = &model.get(sketch)?.kind else {
        return None;
    };
    let level = |l: Option<ElementId>| l.and_then(|l| model.level(l)).map_or(0.0, |l| l.elevation);
    let lift = DVec3::Z * level(sk.level);
    let line: Vec<DVec3> = sk.model_points().into_iter().map(|p| p + lift).collect();
    if line.len() < 2 {
        return None;
    }
    for e in model.iter() {
        let ElementKind::FreeMesh(f) = &e.kind else {
            continue;
        };
        if !f.modifiers.is_empty() {
            continue;
        }
        let base = f.origin + DVec3::Z * level(f.level);
        let local: Vec<DVec3> = line.iter().map(|p| f.to_local(*p - base)).collect();
        for face in 0..f.mesh.faces.len() {
            // an open line cuts across; a closed outline is a face inside
            let mut mesh = f.mesh.clone();
            let made = if sk.closed {
                mesh.inset_loop(face, &local)
            } else {
                mesh.split_face(face, &local)
            };
            let Some(new) = made else {
                continue;
            };
            if mesh.validate().is_err() {
                continue;
            }
            let mut after = e.clone();
            if let ElementKind::FreeMesh(g) = &mut after.kind {
                g.mesh = mesh;
            }
            return Some((e.clone(), after, new));
        }
    }
    None
}

/// Join: several shapes become one, each a piece of it. The first keeps
/// the element; the others go into it where they stand, and are removed.
/// Modifiers are applied first, on every shape with any (the joined shape
/// has none): one stack over all the pieces would change the others
/// (MU48: seven walled shapes would not join). Shapes with something
/// joined to them (a cut) are refused with the reason.
pub fn join(model: &Model, ids: &[ElementId]) -> Result<(Transaction, ElementId), String> {
    let mut shapes: Vec<(&Element, &FreeMesh)> = Vec::new();
    for id in ids {
        match model.get(*id) {
            Some(e) => match &e.kind {
                ElementKind::FreeMesh(f) => shapes.push((e, f)),
                _ => return Err(format!("{} is not a shape: only shapes join.", e.name)),
            },
            None => continue,
        }
    }
    if shapes.len() < 2 {
        return Err("Select two or more shapes to join.".into());
    }
    let (keep, first) = shapes[0];
    let level = |f: &FreeMesh| {
        f.level
            .and_then(|l| model.level(l))
            .map_or(0.0, |l| l.elevation)
    };
    let base = first.origin + DVec3::Z * level(first);
    let applied = shapes.iter().any(|(_, f)| !f.modifiers.is_empty());
    let made = |f: &FreeMesh| {
        if f.modifiers.is_empty() {
            f.mesh.clone()
        } else {
            crate::modifiers::evaluate(&f.mesh, &f.modifiers).mesh
        }
    };
    let mut mesh = made(first);
    let mut gone = Vec::new();
    for (e, f) in &shapes[1..] {
        if !model.dependents(e.id).is_empty() {
            return Err(format!(
                "{} has something joined to it (a cut): remove that, then join.",
                e.name
            ));
        }
        let at = f.origin + DVec3::Z * level(f);
        let mut moved = made(f);
        for v in &mut moved.vertices {
            *v = first.to_local(at + f.to_model(*v) - base);
        }
        mesh = EditMesh::joined(&[(&mesh, DVec3::ZERO), (&moved, DVec3::ZERO)]);
        gone.push(e.id);
    }
    let mut after = keep.clone();
    if mesh.validate().is_err() {
        return Err("Their modifiers leave a shape that can't be joined: apply them one by one to see which.".into());
    }
    if let ElementKind::FreeMesh(f) = &mut after.kind {
        f.mesh = mesh;
        if applied {
            f.modifiers.clear();
        }
    }
    let tx = Transaction::new("Join shapes")
        .replace(keep.clone(), after)
        .remove_all_cascade(model, &gone);
    Ok((tx, keep.id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faces_left_in_one_plane_by_a_pull_become_one() {
        // MU47: a box, its top cut in two, one half pulled up
        let mut m = EditMesh::cuboid(DVec3::new(4.0, 2.0, 1.0));
        let top = (0..6).find(|f| m.face_normal(*f).z > 0.9).unwrap();
        let z = m.face_center(top).z;
        let new = m
            .split_face(top, &[DVec3::new(0.0, -1.0, z), DVec3::new(0.0, 1.0, z)])
            .unwrap();
        // the two halves were cut apart on purpose and nothing moved: two
        let cut = m.clone();
        assert_eq!(m.merge_flat(&cut), 0);
        assert_eq!(m.faces.len(), 7);
        m.pull_face(new, 0.5);
        let volume = m.volume();
        let loose = m.faces.len();
        // its end and its two long sides run on from the box's own
        assert_eq!(m.merge_flat(&cut), 3);
        assert_eq!(m.faces.len(), loose - 3);
        assert_eq!(m.faces.len(), 8, "floor, two tops, the step, four sides");
        assert_eq!(m.kind(), Kind::Solid);
        m.validate().unwrap();
        assert!((m.volume() - volume).abs() < 1e-9);
        // no corner left in the middle of a straight edge, none unused
        assert_eq!(m.vertices.len(), 12);
        let used: HashSet<u32> = m.faces.iter().flatten().copied().collect();
        assert_eq!(used.len(), m.vertices.len());
        // a box pulled longer has nothing to merge
        let mut b = EditMesh::cuboid(DVec3::new(4.0, 2.0, 1.0));
        let before = b.clone();
        b.pull_face(top, 1.0);
        assert_eq!(b.merge_flat(&before), 0);
        assert_eq!(b.faces.len(), 6);
    }

    #[test]
    fn a_line_across_a_face_splits_it_and_each_part_pulls_on_its_own() {
        let mut m = EditMesh::cuboid(DVec3::new(4.0, 2.0, 1.0));
        let top = (0..6).find(|f| m.face_normal(*f).z > 0.9).unwrap();
        let z = m.face_center(top).z;
        let volume = m.volume();
        // across the top, from the middle of one long edge to the other
        let new = m
            .split_face(top, &[DVec3::new(0.0, -1.0, z), DVec3::new(0.0, 1.0, z)])
            .unwrap();
        assert_eq!(m.faces.len(), 7);
        assert_eq!(m.vertices.len(), 10, "a corner on each edge it meets");
        assert_eq!(m.kind(), Kind::Solid, "the sides took the new corners too");
        assert!((m.volume() - volume).abs() < 1e-9);
        for f in [top, new] {
            assert_eq!(m.faces[f].len(), 4);
            assert!(m.face_normal(f).z > 0.99, "both parts face up");
        }
        // one half pulled up: a step, and still a solid
        m.pull_face(new, 0.5);
        assert_eq!(m.kind(), Kind::Solid);
        assert!((m.volume() - (volume + 2.0 * 2.0 * 0.5)).abs() < 1e-9);

        // by a corner in between: an L cut out of the top
        let mut l = EditMesh::cuboid(DVec3::new(4.0, 2.0, 1.0));
        let new = l
            .split_face(
                top,
                &[
                    DVec3::new(0.0, -1.0, z),
                    DVec3::new(0.0, 0.0, z),
                    DVec3::new(2.0, 0.0, z),
                ],
            )
            .unwrap();
        assert_eq!(l.faces[top].len() + l.faces[new].len(), 4 + 2 + 2 + 2);
        assert_eq!(l.kind(), Kind::Solid);
        assert!((l.volume() - volume).abs() < 1e-9);

        // not a cut: off the face, short of its outline, along an edge
        let mut c = EditMesh::cuboid(DVec3::new(4.0, 2.0, 1.0));
        let before = c.clone();
        let p = |x: f64, y: f64, dz: f64| DVec3::new(x, y, z + dz);
        assert!(c
            .split_face(top, &[p(0.0, -1.0, 0.3), p(0.0, 1.0, 0.3)])
            .is_none());
        assert!(c
            .split_face(top, &[p(0.0, -1.0, 0.0), p(0.0, 0.5, 0.0)])
            .is_none());
        assert!(c
            .split_face(top, &[p(-2.0, -1.0, 0.0), p(2.0, -1.0, 0.0)])
            .is_none());
        assert!(c
            .split_face(top, &[p(0.0, -1.0, 0.0), p(1.0, -1.0, 0.0)])
            .is_none());
        assert_eq!(c, before, "a line that cuts nothing changes nothing");
        // corner to corner is a cut
        assert!(c
            .split_face(top, &[p(-2.0, -1.0, 0.0), p(2.0, 1.0, 0.0)])
            .is_some());
    }

    #[test]
    fn an_outline_inside_a_face_is_a_face_to_sink_or_raise() {
        let box_ = EditMesh::cuboid(DVec3::new(4.0, 2.0, 1.0));
        let top = (0..6).find(|f| box_.face_normal(*f).z > 0.9).unwrap();
        let z = box_.face_center(top).z;
        let volume = box_.volume();
        let p = |x: f64, y: f64| DVec3::new(x, y, z);
        // a 1 by 1 square in the top, drawn either way round
        for reversed in [false, true] {
            let mut ring = vec![p(-0.5, -0.5), p(0.5, -0.5), p(0.5, 0.5), p(-0.5, 0.5)];
            if reversed {
                ring.reverse();
            }
            let mut m = box_.clone();
            let inner = m.inset_loop(top, &ring).unwrap();
            assert_eq!(m.faces.len(), 7);
            assert_eq!(
                m.kind(),
                Kind::Solid,
                "the seam is shared by the face with itself"
            );
            assert!((m.volume() - volume).abs() < 1e-9);
            assert!(m.face_normal(inner).z > 0.99);
            assert!(m.face_normal(top).z > 0.99);
            // it is drawn right: triangles covering the top less the square
            let area: f64 = m
                .face_corners(top)
                .iter()
                .map(|t| {
                    let q: Vec<DVec3> = t
                        .iter()
                        .map(|i| m.vertices[m.faces[top][*i] as usize])
                        .collect();
                    (q[1] - q[0]).cross(q[2] - q[0]).length() / 2.0
                })
                .sum();
            assert!((area - (8.0 - 1.0)).abs() < 1e-9, "{area}");
            // sunk: a recess
            let mut sunk = m.clone();
            sunk.pull_face(inner, -0.3);
            assert_eq!(sunk.kind(), Kind::Solid);
            assert!(
                (sunk.volume() - (volume - 0.3)).abs() < 1e-9,
                "{}",
                sunk.volume()
            );
            // the face round it raised: the square stays down, no wall
            // grows along the seam
            let mut raised = m.clone();
            raised.pull_face(top, 0.5);
            assert_eq!(raised.kind(), Kind::Solid);
            assert!(
                (raised.volume() - (volume + 7.0 * 0.5)).abs() < 1e-9,
                "{}",
                raised.volume()
            );
        }
        // not inside: touching the outline, crossing it, off the face
        let mut m = box_.clone();
        assert!(m
            .inset_loop(
                top,
                &[p(-2.0, -0.5), p(0.5, -0.5), p(0.5, 0.5), p(-2.0, 0.5)]
            )
            .is_none());
        assert!(m
            .inset_loop(top, &[p(1.0, -0.5), p(3.0, -0.5), p(3.0, 0.5), p(1.0, 0.5)])
            .is_none());
        assert!(m
            .inset_loop(top, &[p(9.0, 9.0), p(10.0, 9.0), p(10.0, 10.0)])
            .is_none());
        assert_eq!(m, box_);
        // a second outline in the same face
        let a = m
            .inset_loop(
                top,
                &[p(-1.5, -0.5), p(-0.5, -0.5), p(-0.5, 0.5), p(-1.5, 0.5)],
            )
            .unwrap();
        let b = m
            .inset_loop(top, &[p(0.5, -0.5), p(1.5, -0.5), p(1.5, 0.5), p(0.5, 0.5)])
            .unwrap();
        assert_ne!(a, b);
        assert_eq!(m.kind(), Kind::Solid);
        assert!((m.volume() - volume).abs() < 1e-9);
    }

    #[test]
    fn an_outline_pulled_right_through_opens_a_hole() {
        let mut m = EditMesh::cuboid(DVec3::new(4.0, 2.0, 1.0));
        let top = (0..6).find(|f| m.face_normal(*f).z > 0.9).unwrap();
        assert!(m.through(top).is_none(), "a whole side has no far side");
        let z = m.face_center(top).z;
        let p = |x: f64, y: f64| DVec3::new(x, y, z);
        let inner = m
            .inset_loop(
                top,
                &[p(-0.5, -0.5), p(0.5, -0.5), p(0.5, 0.5), p(-0.5, 0.5)],
            )
            .unwrap();
        let (_, depth) = m.through(inner).unwrap();
        assert!((depth - 1.0).abs() < 1e-9);
        let faces = m.faces.len();
        assert!(m.pull_through(inner));
        // four walls more, the two ends gone
        assert_eq!(m.faces.len(), faces + 4 - 1);
        assert_eq!(m.kind(), Kind::Solid, "a hole through is still closed");
        assert!((m.volume() - (8.0 - 1.0)).abs() < 1e-9, "{}", m.volume());
    }

    #[test]
    fn a_line_or_outline_a_little_off_the_face_still_lands_on_it() {
        let box_ = EditMesh::cuboid(DVec3::new(4.0, 2.0, 1.0));
        let top = (0..6).find(|f| box_.face_normal(*f).z > 0.9).unwrap();
        let z = box_.face_center(top).z;
        // 2 mm above the face, its ends 4 mm short of the edges
        let mut m = box_.clone();
        let new = m
            .split_face(
                top,
                &[
                    DVec3::new(0.0, -0.996, z + 0.002),
                    DVec3::new(0.0, 0.996, z + 0.002),
                ],
            )
            .unwrap();
        assert_eq!(m.kind(), Kind::Solid);
        assert!((m.volume() - box_.volume()).abs() < 1e-9);
        assert!(m.faces[new]
            .iter()
            .all(|v| (m.vertices[*v as usize].z - z).abs() < 1e-12));
        let mut m = box_.clone();
        let q = |x: f64, y: f64| DVec3::new(x, y, z - 0.003);
        assert!(m
            .inset_loop(
                top,
                &[q(-0.5, -0.5), q(0.5, -0.5), q(0.5, 0.5), q(-0.5, 0.5)]
            )
            .is_some());
        assert_eq!(m.kind(), Kind::Solid);
        // 3 cm off is not on it
        let mut m = box_.clone();
        assert!(m
            .split_face(
                top,
                &[
                    DVec3::new(0.0, -1.0, z + 0.03),
                    DVec3::new(0.0, 1.0, z + 0.03)
                ]
            )
            .is_none());
    }

    #[test]
    fn a_line_drawn_across_a_shape_cuts_its_face() {
        let mut f = FreeMesh::new(
            None,
            DVec3::new(10.0, 5.0, 0.0),
            EditMesh::cuboid(DVec3::new(4.0, 2.0, 1.0)),
        );
        f.rotation = 90.0;
        let shape = Element::new("Shape", ElementKind::FreeMesh(f));
        // on its top (z = 1), across the short way: turned, that is along X
        let plane = crate::Workplane::ground(1.0);
        let across = crate::Sketch::polyline(
            None,
            plane,
            vec![glam::DVec2::new(9.0, 5.0), glam::DVec2::new(11.0, 5.0)],
            false,
        );
        let beside = crate::Sketch::polyline(
            None,
            plane,
            vec![glam::DVec2::new(20.0, 5.0), glam::DVec2::new(22.0, 5.0)],
            false,
        );
        let (a, b) = (
            Element::new("Line", ElementKind::Sketch(across)),
            Element::new("Line", ElementKind::Sketch(beside)),
        );
        let model = Model::from_elements([shape.clone(), a.clone(), b.clone()]).unwrap();
        let (before, after, new) = imprint(&model, a.id).unwrap();
        assert_eq!(before.id, shape.id);
        let ElementKind::FreeMesh(g) = &after.kind else {
            panic!()
        };
        assert_eq!(g.mesh.faces.len(), 7);
        assert!(g.to_model(g.mesh.face_normal(new)).z > 0.99);
        assert!(
            imprint(&model, b.id).is_none(),
            "a line beside it cuts nothing"
        );
    }

    #[test]
    fn shapes_join_where_they_stand_and_separate_again() {
        let mut a = FreeMesh::new(None, DVec3::ZERO, EditMesh::cuboid(DVec3::ONE));
        a.rotation = 90.0;
        let mut b = FreeMesh::new(
            None,
            DVec3::new(5.0, 0.0, 2.0),
            EditMesh::cuboid(DVec3::new(2.0, 1.0, 1.0)),
        );
        b.scale = 2.0;
        let (ea, eb) = (
            Element::new("A", ElementKind::FreeMesh(a)),
            Element::new("B", ElementKind::FreeMesh(b.clone())),
        );
        let mut doc =
            crate::Document::from_model(Model::from_elements([ea.clone(), eb.clone()]).unwrap());
        let (tx, kept) = join(doc.model(), &[ea.id, eb.id]).unwrap();
        doc.commit(tx).unwrap();
        assert_eq!(kept, ea.id);
        assert!(doc.model().get(eb.id).is_none());
        let ElementKind::FreeMesh(j) = &doc.model().get(kept).unwrap().kind else {
            panic!()
        };
        assert_eq!(j.mesh.pieces().len(), 2);
        assert!((j.mesh.volume() - (1.0 + 2.0 * 8.0)).abs() < 1e-9);
        // B's corners are where they were in the model
        let was: Vec<DVec3> = b
            .mesh
            .vertices
            .iter()
            .map(|v| b.origin + b.to_model(*v))
            .collect();
        let now: Vec<DVec3> = j.mesh.vertices[8..]
            .iter()
            .map(|v| j.origin + j.to_model(*v))
            .collect();
        for (w, n) in was.iter().zip(&now) {
            assert!(w.distance(*n) < 1e-9, "{w} {n}");
        }
        // and apart again: two shapes, the first under the old id
        let (tx, ids) = separate(doc.model(), kept).unwrap();
        doc.commit(tx).unwrap();
        assert_eq!(ids.len(), 2);
        assert_eq!(ids[0], kept);
        assert_eq!(doc.model().get(ids[1]).unwrap().name, "A 2");
        assert!(
            separate(doc.model(), kept).is_none(),
            "one piece stays as it is"
        );
        // only shapes join
        assert!(join(doc.model(), &[kept]).is_err());
    }

    fn square_of_lines() -> EditMesh {
        let mut m = EditMesh::default();
        let p = [
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(4.0, 0.0, 0.0),
            DVec3::new(4.0, 3.0, 0.0),
            DVec3::new(0.0, 3.0, 0.0),
        ];
        for i in 0..4 {
            assert!(m.add_line(p[i], p[(i + 1) % 4]).is_some());
        }
        m
    }

    #[test]
    fn a_shape_is_lines_a_surface_or_a_solid() {
        assert_eq!(EditMesh::default().kind(), Kind::Empty);
        let mut m = square_of_lines();
        assert_eq!(m.kind(), Kind::Lines);
        assert!(m.validate().is_ok(), "lines alone are a shape");
        assert_eq!(m.fill_loops(DVec3::Z).len(), 1);
        assert_eq!(m.kind(), Kind::Surface);
        assert!(m.edges.is_empty(), "the lines became the face's edges");
        m.pull_face(0, 2.5);
        assert_eq!(m.kind(), Kind::Solid);
        assert!((m.volume() - 4.0 * 3.0 * 2.5).abs() < 1e-9);
        assert_eq!(EditMesh::cuboid(DVec3::ONE).kind(), Kind::Solid);
    }

    #[test]
    fn a_line_drawn_twice_or_of_no_length_is_not_added() {
        let mut m = square_of_lines();
        assert_eq!(m.vertices.len(), 4, "corners are welded");
        assert!(m.add_line(DVec3::ZERO, DVec3::new(4.0, 0.0, 0.0)).is_none());
        assert!(m.add_line(DVec3::new(4.0, 0.0, 0.0), DVec3::ZERO).is_none());
        assert!(m.add_line(DVec3::ONE, DVec3::ONE).is_none());
        assert_eq!(m.edges.len(), 4);
    }

    #[test]
    fn an_outline_faces_up_whichever_way_it_was_drawn() {
        for reversed in [false, true] {
            let mut m = EditMesh::default();
            let mut p = [
                DVec3::new(0.0, 0.0, 1.0),
                DVec3::new(2.0, 0.0, 1.0),
                DVec3::new(2.0, 2.0, 1.0),
                DVec3::new(0.0, 2.0, 1.0),
            ];
            if reversed {
                p.reverse();
            }
            for i in 0..4 {
                m.add_line(p[i], p[(i + 1) % 4]);
            }
            let made = m.fill_loops(DVec3::Z);
            assert_eq!(made.len(), 1);
            assert!(m.face_normal(made[0]).z > 0.99);
        }
    }

    #[test]
    fn an_open_run_of_lines_and_a_bent_loop_are_not_filled() {
        let mut m = EditMesh::default();
        m.add_line(DVec3::ZERO, DVec3::X);
        m.add_line(DVec3::X, DVec3::new(1.0, 1.0, 0.0));
        assert!(m.closed_loops().is_empty());
        assert!(m.fill_loops(DVec3::Z).is_empty());
        // a closed loop that is not flat stays lines
        let mut bent = EditMesh::default();
        let p = [DVec3::ZERO, DVec3::X, DVec3::new(1.0, 1.0, 0.5), DVec3::Y];
        for i in 0..4 {
            bent.add_line(p[i], p[(i + 1) % 4]);
        }
        assert_eq!(bent.closed_loops().len(), 1);
        assert!(bent.fill_loops(DVec3::Z).is_empty());
        assert_eq!(bent.kind(), Kind::Lines);
    }

    #[test]
    fn a_flat_face_pulled_either_way_is_a_solid_facing_out() {
        for distance in [2.0, -2.0] {
            let mut m = square_of_lines();
            m.fill_loops(DVec3::Z);
            m.pull(&Pulled::Face(0), distance);
            assert_eq!(m.kind(), Kind::Solid, "pulled {distance}");
            // faces out: a positive volume
            assert!((m.volume() - 24.0).abs() < 1e-9, "{}", m.volume());
            // the pulled face keeps its number and is at the far end
            let z = m.face_center(0).z;
            assert!((z - distance).abs() < 1e-9, "{z}");
        }
    }

    #[test]
    fn a_box_pulled_stays_a_box_and_a_tapered_face_gets_new_sides() {
        let mut a = EditMesh::cuboid(DVec3::new(2.0, 2.0, 1.0));
        let before = a.volume();
        for face in 0..6 {
            let mut b = a.clone();
            b.pull_face(face, 0.5);
            assert_eq!(b.vertices.len(), 8, "no edge is added to a box");
            assert_eq!(b.faces.len(), 6);
            assert!(b.volume() > before + 0.9, "face {face}");
            assert_eq!(b.kind(), Kind::Solid);
        }
        // its top made smaller: the sides lean, so a pull makes new ones
        let top = (0..6).find(|f| a.face_normal(*f).z > 0.9).unwrap();
        a.scale_face(top, 0.5);
        let mut b = a.clone();
        let mut c = a.clone();
        b.pull_face(top, 0.5);
        c.extrude_face(top, 0.5);
        assert_eq!(b, c);
        assert_eq!(b.vertices.len(), 12);
        assert_eq!(b.kind(), Kind::Solid);
    }

    #[test]
    fn lines_pulled_become_surfaces_that_share_their_edges() {
        // an L of two lines, pulled up: two walls meeting at a shared edge
        let mut m = EditMesh::default();
        let a = m.add_line(DVec3::ZERO, DVec3::new(3.0, 0.0, 0.0)).unwrap();
        let b = m
            .add_line(DVec3::new(3.0, 0.0, 0.0), DVec3::new(3.0, 2.0, 0.0))
            .unwrap();
        let made = m.pull(&Pulled::Lines(vec![a, b], DVec3::Z), 2.7);
        assert_eq!(made.len(), 2);
        assert!(m.edges.is_empty());
        assert_eq!(m.kind(), Kind::Surface);
        assert_eq!(m.vertices.len(), 6, "the corner's lifted point is shared");
        assert_eq!(m.pieces().len(), 1);
        let area: f64 = made
            .iter()
            .map(|f| {
                let v: Vec<DVec3> = m.faces[*f]
                    .iter()
                    .map(|i| m.vertices[*i as usize])
                    .collect();
                (v[1] - v[0]).cross(v[3] - v[0]).length()
            })
            .sum();
        assert!((area - (3.0 + 2.0) * 2.7).abs() < 1e-9);
        // a line left out stays a line
        let mut m = EditMesh::default();
        let a = m.add_line(DVec3::ZERO, DVec3::X).unwrap();
        m.add_line(DVec3::new(5.0, 0.0, 0.0), DVec3::new(6.0, 0.0, 0.0));
        m.pull_lines(&[a], DVec3::Z);
        assert_eq!((m.faces.len(), m.edges.len()), (1, 1));
    }

    #[test]
    fn pieces_separate_and_join_back() {
        let box_a = EditMesh::cuboid(DVec3::ONE);
        let mut lines = EditMesh::default();
        lines.add_line(DVec3::ZERO, DVec3::X);
        let both = EditMesh::joined(&[
            (&box_a, DVec3::ZERO),
            (&box_a, DVec3::new(5.0, 0.0, 0.0)),
            (&lines, DVec3::new(0.0, 9.0, 0.0)),
        ]);
        assert!(both.validate().is_ok());
        let pieces = both.pieces();
        assert_eq!(pieces.len(), 3);
        assert_eq!(pieces[0].faces.len(), 6);
        assert_eq!(pieces[2].edges.len(), 1);
        // a corner of the second box picks that box, all of it and no more
        assert_eq!(both.linked(&[9]), (8..16).collect::<Vec<u32>>());
        assert_eq!(both.linked(&[0, 17]).len(), 10);
        assert!(both.linked(&[]).is_empty());
        let parts = both.separate();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], box_a);
        assert_eq!(parts[1].kind(), Kind::Solid);
        assert!((parts[1].vertices[0].x - 4.5).abs() < 1e-9);
        assert_eq!(parts[2].kind(), Kind::Lines);
        // joined back, they are the same shape
        let again = EditMesh::joined(&parts.iter().map(|p| (p, DVec3::ZERO)).collect::<Vec<_>>());
        assert_eq!(again, both);
    }

    #[test]
    fn a_role_is_solid_unless_said_and_older_files_open_as_solid() {
        assert_eq!(Role::default(), Role::Solid);
        let f = crate::FreeMesh::new(None, DVec3::ZERO, EditMesh::cuboid(DVec3::ONE));
        assert_eq!(f.role, Role::Solid);
        let json = serde_json::to_string(&f).unwrap();
        assert!(!json.contains("role"), "a solid's file is as it was");
        assert!(!json.contains("edges"), "a mesh without lines too");
        let back: crate::FreeMesh = serde_json::from_str(&json).unwrap();
        assert_eq!(back, f);
        let mut v = f.clone();
        v.role = Role::Void;
        let back: crate::FreeMesh =
            serde_json::from_str(&serde_json::to_string(&v).unwrap()).unwrap();
        assert_eq!(back.role, Role::Void);
    }
}
