use glam::{DVec2, DVec3};
use serde::{Deserialize, Serialize};

/// A polygon mesh for Blender-style editing: vertices plus faces as vertex
/// loops, counter-clockwise when seen from outside.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EditMesh {
    pub vertices: Vec<DVec3>,
    pub faces: Vec<Vec<u32>>,
    /// Lines that bound no face, each from one corner to another: a shape
    /// can be lines alone, or carry lines beside its faces (MU43). Empty
    /// in every mesh made before, and left out of their files.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edges: Vec<[u32; 2]>,
}

impl EditMesh {
    /// An axis-aligned box with its base centred on the origin.
    pub fn cuboid(size: DVec3) -> Self {
        let (x, y, z) = (size.x / 2.0, size.y / 2.0, size.z);
        let vertices = vec![
            DVec3::new(-x, -y, 0.0),
            DVec3::new(x, -y, 0.0),
            DVec3::new(x, y, 0.0),
            DVec3::new(-x, y, 0.0),
            DVec3::new(-x, -y, z),
            DVec3::new(x, -y, z),
            DVec3::new(x, y, z),
            DVec3::new(-x, y, z),
        ];
        let faces = vec![
            vec![0, 3, 2, 1], // bottom
            vec![4, 5, 6, 7], // top
            vec![0, 1, 5, 4], // front (-y)
            vec![1, 2, 6, 5], // right (+x)
            vec![2, 3, 7, 6], // back (+y)
            vec![3, 0, 4, 7], // left (-x)
        ];
        Self {
            vertices,
            faces,
            edges: vec![],
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.faces.is_empty() && self.edges.is_empty() {
            return Err("mesh has no faces".into());
        }
        for (i, e) in self.edges.iter().enumerate() {
            if e[0] == e[1] {
                return Err(format!("line {i} has no length"));
            }
            if let Some(v) = e.iter().find(|&&v| v as usize >= self.vertices.len()) {
                return Err(format!("line {i} uses missing vertex {v}"));
            }
        }
        for (i, f) in self.faces.iter().enumerate() {
            if f.len() < 3 {
                return Err(format!("face {i} has fewer than 3 vertices"));
            }
            if let Some(v) = f.iter().find(|&&v| v as usize >= self.vertices.len()) {
                return Err(format!("face {i} uses missing vertex {v}"));
            }
        }
        if self.vertices.iter().any(|v| !v.is_finite()) {
            return Err("mesh has a non-finite vertex".into());
        }
        Ok(())
    }

    /// Newell's method: robust for any planar or near-planar polygon.
    pub fn face_normal(&self, face: usize) -> DVec3 {
        let loop_ = &self.faces[face];
        let mut n = DVec3::ZERO;
        for (i, &a) in loop_.iter().enumerate() {
            let p = self.vertices[a as usize];
            let q = self.vertices[loop_[(i + 1) % loop_.len()] as usize];
            n += DVec3::new(
                (p.y - q.y) * (p.z + q.z),
                (p.z - q.z) * (p.x + q.x),
                (p.x - q.x) * (p.y + q.y),
            );
        }
        n.normalize_or_zero()
    }

    /// Triangles covering face `face`, as vertex indices, counter-clockwise
    /// seen from outside. There are always `len - 2`, so triangle k of a
    /// flattened mesh maps back to its face.
    pub fn face_triangles(&self, face: usize) -> Vec<[u32; 3]> {
        let f = &self.faces[face];
        self.face_corners(face)
            .into_iter()
            .map(|t| [f[t[0]], f[t[1]], f[t[2]]])
            .collect()
    }

    /// Like [`EditMesh::face_triangles`], as positions in the face's loop.
    /// Concave faces (an L-shaped extrusion's cap) are ear-clipped; convex
    /// ones are fanned from the first corner.
    pub fn face_corners(&self, face: usize) -> Vec<[usize; 3]> {
        let f = &self.faces[face];
        let m = f.len();
        let fan = || -> Vec<[usize; 3]> { (1..m - 1).map(|i| [0, i, i + 1]).collect() };
        let n = self.face_normal(face);
        if m <= 3 || n == DVec3::ZERO {
            return fan();
        }
        let u = n.any_orthonormal_vector();
        let v = n.cross(u);
        let pts: Vec<DVec2> = f
            .iter()
            .map(|&i| {
                let p = self.vertices[i as usize];
                DVec2::new(p.dot(u), p.dot(v))
            })
            .collect();
        let convex = (0..m).all(|i| {
            let (a, b, c) = (pts[i], pts[(i + 1) % m], pts[(i + 2) % m]);
            (b - a).perp_dot(c - b) >= -1e-12
        });
        if convex {
            return fan();
        }
        let tris = crate::polygon::triangulate(&pts);
        if tris.len() == m - 2 {
            tris
        } else {
            fan()
        }
    }

    pub fn face_center(&self, face: usize) -> DVec3 {
        let loop_ = &self.faces[face];
        loop_
            .iter()
            .map(|&v| self.vertices[v as usize])
            .sum::<DVec3>()
            / loop_.len() as f64
    }

    pub fn translate_vertices(&mut self, vertices: &[u32], delta: DVec3) {
        let mut moved = vertices.to_vec();
        moved.sort_unstable();
        moved.dedup();
        for v in moved {
            self.vertices[v as usize] += delta;
        }
    }

    /// Extrude one face along its normal by `distance`, adding a side quad
    /// per edge, like Blender's E key on a single face.
    pub fn extrude_face(&mut self, face: usize, distance: f64) {
        let offset = self.face_normal(face) * distance;
        let old = self.faces[face].clone();
        // a face round a hole comes back on itself along a seam: a corner
        // it passes twice is lifted once, and the seam grows no side
        let mut lifted: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
        let new: Vec<u32> = old
            .iter()
            .map(|&v| {
                *lifted.entry(v).or_insert_with(|| {
                    self.vertices.push(self.vertices[v as usize] + offset);
                    (self.vertices.len() - 1) as u32
                })
            })
            .collect();
        let m = old.len();
        let seam = |a: u32, b: u32| (0..m).any(|k| old[k] == b && old[(k + 1) % m] == a);
        for i in 0..m {
            let j = (i + 1) % m;
            if seam(old[i], old[j]) {
                continue;
            }
            self.faces.push(vec![old[i], old[j], new[j], new[i]]);
        }
        self.faces[face] = new;
    }

    /// Inset a face by `by` (Blender's I): a smaller face of the same shape
    /// `by` inside its edges, joined to them by a ring of faces. The face
    /// keeps its number, now the inner one, to push in or pull out next: a
    /// window reveal, a recessed panel, a parapet (inset the roof, push it
    /// down). False if `by` would close it up.
    pub fn inset_face(&mut self, face: usize, by: f64) -> bool {
        let old = self.faces[face].clone();
        let k = old.len();
        if k < 3 || by <= 0.0 {
            return false;
        }
        let n = self.face_normal(face);
        let at = |i: usize| self.vertices[old[i % k] as usize];
        let mut inner = Vec::with_capacity(k);
        for i in 0..k {
            let (p, v, q) = (at(i + k - 1), at(i), at(i + 1));
            let (Some(e1), Some(e2)) = ((v - p).try_normalize(), (q - v).try_normalize()) else {
                return false;
            };
            // inward is to the left of each edge, seen along the normal
            let (in1, in2) = (n.cross(e1), n.cross(e2));
            let Some(b) = (in1 + in2).try_normalize() else {
                return false;
            };
            let cos = b.dot(in1);
            if cos < 0.05 {
                return false;
            }
            inner.push(v + b * (by / cos));
        }
        // too far: the inner face would turn inside out
        // every inner edge runs the way its outer edge does
        let kept = (0..k).all(|i| {
            let j = (i + 1) % k;
            (inner[j] - inner[i]).dot(at(j) - at(i)) > 1e-9
        });
        if !kept {
            return false;
        }
        let new: Vec<u32> = inner
            .into_iter()
            .map(|p| {
                self.vertices.push(p);
                (self.vertices.len() - 1) as u32
            })
            .collect();
        for i in 0..k {
            let j = (i + 1) % k;
            self.faces.push(vec![old[i], old[j], new[j], new[i]]);
        }
        self.faces[face] = new;
        true
    }

    /// Inset each of `faces` on its own by `by`, then sink (negative) or
    /// raise the new inner faces by `depth`, with sides square to them: a
    /// recess or a raised panel in one go (MU49). Each face keeps its
    /// number, now the inner one. False, and nothing changed, when `by`
    /// would close a face up or make its inner outline cross itself.
    pub fn inset_faces(&mut self, faces: &[usize], by: f64, depth: f64) -> bool {
        let mut fs = faces.to_vec();
        fs.sort_unstable();
        fs.dedup();
        if fs.is_empty() || fs.iter().any(|f| *f >= self.faces.len()) {
            return false;
        }
        let before = self.clone();
        for f in &fs {
            if !(self.inset_face(*f, by) && self.ring_is_simple(*f)) {
                *self = before;
                return false;
            }
        }
        if depth.abs() > 1e-9 {
            for f in &fs {
                self.extrude_face(*f, depth);
            }
        }
        true
    }

    /// Outset `face` by `by`: a rim around it, cut into the faces next to
    /// it, parallel to the face and `by` behind it. The face keeps its
    /// size and its number. False, and nothing changed, when a corner of
    /// the face has more or fewer than one edge leaving it, when that edge
    /// does not go behind the face, or when `by` is further than it runs.
    pub fn outset_face(&mut self, face: usize, by: f64) -> bool {
        let Some(ring) = self.faces.get(face).cloned() else {
            return false;
        };
        let k = ring.len();
        if k < 3 || by <= 0.0 {
            return false;
        }
        let n = self.face_normal(face);
        // each corner slides down the one edge that leaves the face
        let mut moved = Vec::with_capacity(k);
        for i in 0..k {
            let (v, before, after) = (ring[i], ring[(i + k - 1) % k], ring[(i + 1) % k]);
            let mut away: Vec<u32> = vec![];
            for (g, f) in self.faces.iter().enumerate() {
                if g == face {
                    continue;
                }
                let m = f.len();
                for j in 0..m {
                    if f[j] != v {
                        continue;
                    }
                    for w in [f[(j + m - 1) % m], f[(j + 1) % m]] {
                        if w != before && w != after && !away.contains(&w) {
                            away.push(w);
                        }
                    }
                }
            }
            let [w] = away[..] else {
                return false;
            };
            let d = self.vertices[w as usize] - self.vertices[v as usize];
            let len = d.length();
            let Some(dir) = d.try_normalize() else {
                return false;
            };
            let down = -dir.dot(n);
            if down < 0.05 {
                return false;
            }
            let t = by / down;
            if t > len - 1e-6 {
                return false;
            }
            moved.push(self.vertices[v as usize] + dir * t);
        }
        // every side of the face has one face across it
        for i in 0..k {
            let (a, b) = (ring[i], ring[(i + 1) % k]);
            let across = self
                .faces
                .iter()
                .enumerate()
                .filter(|(g, f)| {
                    *g != face && (0..f.len()).any(|j| f[j] == b && f[(j + 1) % f.len()] == a)
                })
                .count();
            if across != 1 {
                return false;
            }
        }
        let first = self.vertices.len() as u32;
        self.vertices.extend(moved);
        let new = |v: u32| ring.iter().position(|r| *r == v).map(|i| first + i as u32);
        let mut rim = vec![];
        for (g, f) in self.faces.iter_mut().enumerate() {
            if g == face {
                continue;
            }
            let m = f.len();
            for j in 0..m {
                let (a, b) = (f[j], f[(j + 1) % m]);
                if let (Some(na), Some(nb)) = (new(a), new(b)) {
                    rim.push(vec![a, b, nb, na]);
                }
            }
            for v in f.iter_mut() {
                if let Some(nv) = new(*v) {
                    *v = nv;
                }
            }
        }
        self.faces.extend(rim);
        true
    }

    /// Outset each of `faces`, then sink or raise them by `depth` as
    /// [`Self::inset_faces`] does. False, and nothing changed, when one
    /// of them cannot take it.
    pub fn outset_faces(&mut self, faces: &[usize], by: f64, depth: f64) -> bool {
        let mut fs = faces.to_vec();
        fs.sort_unstable();
        fs.dedup();
        if fs.is_empty() {
            return false;
        }
        let before = self.clone();
        for f in &fs {
            if !self.outset_face(*f, by) {
                *self = before;
                return false;
            }
        }
        if depth.abs() > 1e-9 {
            for f in &fs {
                self.extrude_face(*f, depth);
            }
        }
        true
    }

    /// How far `faces` can be outset, up to `want`, as
    /// [`Self::inset_reach`].
    pub fn outset_reach(&self, faces: &[usize], want: f64) -> f64 {
        let fits = |t: f64| t > 0.0 && self.clone().outset_faces(faces, t, 0.0);
        if fits(want) {
            return want;
        }
        let (mut lo, mut hi) = (0.0, want);
        for _ in 0..16 {
            let mid = (lo + hi) / 2.0;
            if fits(mid) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        lo
    }

    /// How far `faces` can be inset, up to `want`: `want` itself when it
    /// fits, else the most that still leaves every face (to a millimetre
    /// or so). Zero when nothing fits.
    pub fn inset_reach(&self, faces: &[usize], want: f64) -> f64 {
        let fits = |t: f64| t > 0.0 && self.clone().inset_faces(faces, t, 0.0);
        if fits(want) {
            return want;
        }
        let (mut lo, mut hi) = (0.0, want);
        for _ in 0..16 {
            let mid = (lo + hi) / 2.0;
            if fits(mid) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        lo
    }

    /// Does the outline of `face` keep clear of itself, seen square on?
    fn ring_is_simple(&self, face: usize) -> bool {
        let n = self.face_normal(face).abs();
        let flat: Vec<glam::DVec2> = self.faces[face]
            .iter()
            .map(|v| {
                let q = self.vertices[*v as usize];
                if n.x >= n.y && n.x >= n.z {
                    glam::DVec2::new(q.y, q.z)
                } else if n.y >= n.z {
                    glam::DVec2::new(q.z, q.x)
                } else {
                    glam::DVec2::new(q.x, q.y)
                }
            })
            .collect();
        !crate::polygon::self_intersects(&flat, true)
    }

    /// Scale a face's vertices about its centre, Blender's S key on a face.
    pub fn scale_face(&mut self, face: usize, factor: f64) {
        let c = self.face_center(face);
        for &v in &self.faces[face].clone() {
            let p = &mut self.vertices[v as usize];
            *p = c + (*p - c) * factor;
        }
    }

    /// One level of Catmull-Clark subdivision. Every face becomes quads, which
    /// is the basis for SubD modelling.
    pub fn subdivide(&self) -> EditMesh {
        self.subdivide_with(true)
    }

    /// One level of subdivision: every face into quads, rounded
    /// (Catmull-Clark) when `smooth`, else left where it was.
    pub fn subdivide_with(&self, smooth: bool) -> EditMesh {
        use std::collections::HashMap;
        let nv = self.vertices.len();
        let face_points: Vec<DVec3> = (0..self.faces.len()).map(|f| self.face_center(f)).collect();

        // edge (min,max) -> adjacent faces
        let mut edge_faces: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
        for (fi, f) in self.faces.iter().enumerate() {
            for i in 0..f.len() {
                let (a, b) = (f[i], f[(i + 1) % f.len()]);
                edge_faces.entry((a.min(b), a.max(b))).or_default().push(fi);
            }
        }

        let mut vertices = Vec::with_capacity(nv + edge_faces.len() + self.faces.len());
        // original vertices first; their smoothed positions are filled in below
        vertices.extend(self.vertices.iter().copied());

        let mut edge_index: HashMap<(u32, u32), u32> = HashMap::new();
        let mut edge_mid: HashMap<(u32, u32), DVec3> = HashMap::new();
        let mut keys: Vec<_> = edge_faces.keys().copied().collect();
        keys.sort_unstable();
        for key in keys {
            let faces = &edge_faces[&key];
            let (a, b) = (self.vertices[key.0 as usize], self.vertices[key.1 as usize]);
            edge_mid.insert(key, (a + b) / 2.0);
            let p = if faces.len() == 2 && smooth {
                (a + b + face_points[faces[0]] + face_points[faces[1]]) / 4.0
            } else {
                (a + b) / 2.0 // boundary edge
            };
            edge_index.insert(key, vertices.len() as u32);
            vertices.push(p);
        }

        // move original vertices
        let mut v_faces: Vec<Vec<usize>> = vec![vec![]; nv];
        let mut v_edges: Vec<Vec<(u32, u32)>> = vec![vec![]; nv];
        for (fi, f) in self.faces.iter().enumerate() {
            for &v in f {
                v_faces[v as usize].push(fi);
            }
        }
        for &key in edge_faces.keys() {
            v_edges[key.0 as usize].push(key);
            v_edges[key.1 as usize].push(key);
        }
        for v in (0..nv).filter(|_| smooth) {
            let p = self.vertices[v];
            let boundary: Vec<_> = v_edges[v]
                .iter()
                .filter(|e| edge_faces[e].len() == 1)
                .collect();
            vertices[v] = if !boundary.is_empty() {
                let mids: DVec3 = boundary.iter().map(|e| edge_mid[e]).sum();
                (p * 2.0 + mids) / (2.0 + boundary.len() as f64)
            } else if !v_faces[v].is_empty() {
                let n = v_faces[v].len() as f64;
                let f: DVec3 = v_faces[v].iter().map(|&fi| face_points[fi]).sum::<DVec3>() / n;
                let r: DVec3 =
                    v_edges[v].iter().map(|e| edge_mid[e]).sum::<DVec3>() / v_edges[v].len() as f64;
                (f + r * 2.0 + p * (n - 3.0)) / n
            } else {
                p
            };
        }

        let mut faces = Vec::new();
        for (fi, f) in self.faces.iter().enumerate() {
            let center = vertices.len() as u32;
            vertices.push(face_points[fi]);
            for i in 0..f.len() {
                let prev = f[(i + f.len() - 1) % f.len()];
                let cur = f[i];
                let next = f[(i + 1) % f.len()];
                let e_prev = edge_index[&(prev.min(cur), prev.max(cur))];
                let e_next = edge_index[&(cur.min(next), cur.max(next))];
                faces.push(vec![cur, e_next, center, e_prev]);
            }
        }
        EditMesh {
            vertices,
            faces,
            edges: vec![],
        }
    }

    /// Signed volume; positive when faces point outward.
    pub fn volume(&self) -> f64 {
        let mut v = 0.0;
        for f in &self.faces {
            let p0 = self.vertices[f[0] as usize];
            for i in 1..f.len() - 1 {
                let p1 = self.vertices[f[i] as usize];
                let p2 = self.vertices[f[i + 1] as usize];
                v += p0.dot(p1.cross(p2)) / 6.0;
            }
        }
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outset_cuts_a_rim_around_the_face_and_leaves_its_size() {
        let mut m = EditMesh::cuboid(DVec3::new(4.0, 2.0, 1.0));
        let top = (0..6).find(|f| m.face_normal(*f).z > 0.9).unwrap();
        let (volume, z, was) = (m.volume(), m.face_center(top).z, m.faces[top].clone());
        assert!(m.outset_faces(&[top], 0.25, 0.0));
        m.validate().unwrap();
        // the face is as it was; four rim faces, each 0.25 high, on the sides
        assert_eq!(m.faces[top], was);
        assert_eq!(m.faces.len(), 6 + 4);
        assert!((m.volume() - volume).abs() < 1e-9);
        for f in 6..10 {
            assert!(m.face_normal(f).z.abs() < 1e-9, "a rim face is upright");
            assert!((m.face_center(f).z - (z - 0.125)).abs() < 1e-9);
        }
        // every edge still has a face each side of it
        for f in &m.faces {
            for j in 0..f.len() {
                let (a, b) = (f[j], f[(j + 1) % f.len()]);
                let back = m
                    .faces
                    .iter()
                    .filter(|g| (0..g.len()).any(|i| g[i] == b && g[(i + 1) % g.len()] == a))
                    .count();
                assert_eq!(back, 1, "edge {a}-{b}");
            }
        }
        // raised with it: the top goes up 0.5, the volume by its area times that
        let mut up = EditMesh::cuboid(DVec3::new(4.0, 2.0, 1.0));
        assert!(up.outset_faces(&[top], 0.25, 0.5));
        assert!((up.volume() - (volume + 8.0 * 0.5)).abs() < 1e-9);
        // the sides are 1 high: 1.2 is too far, and nothing changes
        let mut b = EditMesh::cuboid(DVec3::new(4.0, 2.0, 1.0));
        let kept = b.clone();
        assert!(!b.outset_faces(&[top], 1.2, 0.0));
        assert_eq!(b.faces, kept.faces);
        let most = kept.outset_reach(&[top], 1.2);
        assert!(most > 0.99 && most < 1.0, "{most}");
    }

    #[test]
    fn inset_with_a_depth_makes_a_recess_and_stops_where_a_face_would_close() {
        let mut m = EditMesh::cuboid(DVec3::new(4.0, 2.0, 1.0));
        let top = (0..6).find(|f| m.face_normal(*f).z > 0.9).unwrap();
        let side = (0..6).find(|f| m.face_normal(*f).x > 0.9).unwrap();
        let (volume, z) = (m.volume(), m.face_center(top).z);
        // a recess 0.5 in from the edges of the top, 0.25 deep
        assert!(m.inset_faces(&[top], 0.5, -0.25));
        m.validate().unwrap();
        assert!((m.face_center(top).z - (z - 0.25)).abs() < 1e-9);
        assert!((m.volume() - (volume - 3.0 * 1.0 * 0.25)).abs() < 1e-9);
        // two faces at once, each on its own
        let mut two = EditMesh::cuboid(DVec3::new(4.0, 2.0, 1.0));
        assert!(two.inset_faces(&[top, side], 0.2, 0.0));
        two.validate().unwrap();
        assert_eq!(two.faces.len(), 6 + 4 + 4);
        // the top is 2 wide: 1.0 from each side closes it, and nothing changes
        let mut box_ = EditMesh::cuboid(DVec3::new(4.0, 2.0, 1.0));
        let was = box_.clone();
        assert!(!box_.inset_faces(&[top], 1.2, 0.0));
        assert_eq!(box_.faces, was.faces);
        assert_eq!(box_.vertices.len(), was.vertices.len());
        // as far as it goes: just under 1.0, and 0.3 as asked when it fits
        let most = was.inset_reach(&[top], 1.2);
        assert!(most > 0.99 && most < 1.0, "{most}");
        assert_eq!(was.inset_reach(&[top], 0.3), 0.3);
    }

    #[test]
    fn inset_leaves_a_ring_and_a_smaller_face() {
        let mut m = EditMesh::cuboid(DVec3::splat(2.0));
        let top = (0..m.faces.len())
            .find(|f| m.face_normal(*f).z > 0.9)
            .unwrap();
        let faces = m.faces.len();
        assert!(m.inset_face(top, 0.25));
        assert_eq!(m.faces.len(), faces + 4);
        // the inner face is 1.5 by 1.5, still on top
        let xs: Vec<f64> = m.faces[top]
            .iter()
            .map(|v| m.vertices[*v as usize].x)
            .collect();
        let w = xs.iter().cloned().fold(f64::MIN, f64::max)
            - xs.iter().cloned().fold(f64::MAX, f64::min);
        assert!((w - 1.5).abs() < 1e-9);
        assert!(m.face_normal(top).z > 0.9);
        // and too far closes it up: refused
        assert!(!m.inset_face(top, 1.0));
    }

    #[test]
    fn cuboid_has_outward_faces() {
        let m = EditMesh::cuboid(DVec3::new(2.0, 3.0, 4.0));
        assert!((m.volume() - 24.0).abs() < 1e-9);
        assert_eq!(m.face_normal(1), DVec3::Z);
        assert_eq!(m.face_normal(0), -DVec3::Z);
    }

    #[test]
    fn extrude_top_face_grows_volume() {
        let mut m = EditMesh::cuboid(DVec3::ONE);
        m.extrude_face(1, 2.0);
        assert_eq!(m.faces.len(), 10);
        assert!((m.volume() - 3.0).abs() < 1e-9);
        m.validate().unwrap();
    }

    #[test]
    fn scale_face_tapers() {
        let mut m = EditMesh::cuboid(DVec3::ONE);
        m.scale_face(1, 0.0); // pyramid
        assert!((m.volume() - 1.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn subdivide_cube_gives_24_quads_and_shrinks() {
        let m = EditMesh::cuboid(DVec3::ONE);
        let s = m.subdivide();
        assert_eq!(s.faces.len(), 24);
        assert_eq!(s.vertices.len(), 8 + 12 + 6);
        assert!(s.faces.iter().all(|f| f.len() == 4));
        assert!(s.volume() > 0.0 && s.volume() < 1.0);
        s.validate().unwrap();
    }
}
