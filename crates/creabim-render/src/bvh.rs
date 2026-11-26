//! A bounding volume hierarchy: boxes in boxes round the triangles, so a
//! ray tests a few dozen triangles instead of all of them. Built by the
//! surface area heuristic over binned centroids.

use glam::Vec3;

#[derive(Clone, Copy, Debug)]
struct Node {
    lo: Vec3,
    hi: Vec3,
    /// For a leaf, its first triangle; else its second child (the first
    /// follows it).
    index: u32,
    /// Triangles in a leaf; zero for an inner node.
    count: u16,
    axis: u16,
}

pub struct Bvh {
    nodes: Vec<Node>,
    /// Triangle indices in leaf order.
    order: Vec<u32>,
    /// Corner and two edges of each triangle, in leaf order.
    v0: Vec<Vec3>,
    e1: Vec<Vec3>,
    e2: Vec<Vec3>,
}

#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub t: f32,
    pub tri: u32,
    pub u: f32,
    pub v: f32,
}

const BINS: usize = 16;
const LEAF: usize = 4;

#[derive(Clone, Copy)]
struct Aabb {
    lo: Vec3,
    hi: Vec3,
}
impl Aabb {
    const EMPTY: Aabb = Aabb {
        lo: Vec3::splat(f32::MAX),
        hi: Vec3::splat(f32::MIN),
    };
    fn grow(&mut self, p: Vec3) {
        self.lo = self.lo.min(p);
        self.hi = self.hi.max(p);
    }
    fn join(&mut self, o: &Aabb) {
        self.lo = self.lo.min(o.lo);
        self.hi = self.hi.max(o.hi);
    }
    fn area(&self) -> f32 {
        let d = (self.hi - self.lo).max(Vec3::ZERO);
        d.x * d.y + d.y * d.z + d.z * d.x
    }
}

impl Bvh {
    pub fn build(tris: &[[Vec3; 3]]) -> Bvh {
        let boxes: Vec<Aabb> = tris
            .iter()
            .map(|t| {
                let mut b = Aabb::EMPTY;
                t.iter().for_each(|p| b.grow(*p));
                b
            })
            .collect();
        let cent: Vec<Vec3> = boxes.iter().map(|b| (b.lo + b.hi) * 0.5).collect();
        let mut order: Vec<u32> = (0..tris.len() as u32).collect();
        let mut nodes = Vec::with_capacity(tris.len() / 2 + 1);
        nodes.push(Node {
            lo: Vec3::ZERO,
            hi: Vec3::ZERO,
            index: 0,
            count: 0,
            axis: 0,
        });
        // (node, start, end)
        let mut stack = vec![(0usize, 0usize, tris.len())];
        while let Some((ni, s, e)) = stack.pop() {
            let mut bb = Aabb::EMPTY;
            let mut cb = Aabb::EMPTY;
            for &i in &order[s..e] {
                bb.join(&boxes[i as usize]);
                cb.grow(cent[i as usize]);
            }
            nodes[ni].lo = bb.lo;
            nodes[ni].hi = bb.hi;
            let n = e - s;
            let leaf = |nodes: &mut Vec<Node>| {
                nodes[ni].index = s as u32;
                nodes[ni].count = n as u16;
            };
            if n <= LEAF {
                leaf(&mut nodes);
                continue;
            }
            // best split over binned centroids on each axis
            let mut best = (f32::MAX, 0usize, 0usize);
            let ext = cb.hi - cb.lo;
            for axis in 0..3 {
                if ext[axis] < 1e-7 {
                    continue;
                }
                let mut bins = [(Aabb::EMPTY, 0u32); BINS];
                let k = BINS as f32 / ext[axis];
                for &i in &order[s..e] {
                    let b = (((cent[i as usize][axis] - cb.lo[axis]) * k) as usize).min(BINS - 1);
                    bins[b].0.join(&boxes[i as usize]);
                    bins[b].1 += 1;
                }
                let mut right = [0f32; BINS];
                let mut acc = Aabb::EMPTY;
                let mut cnt = 0;
                for b in (1..BINS).rev() {
                    acc.join(&bins[b].0);
                    cnt += bins[b].1;
                    right[b] = if cnt > 0 {
                        acc.area() * cnt as f32
                    } else {
                        0.0
                    };
                }
                let mut acc = Aabb::EMPTY;
                let mut cnt = 0;
                for b in 0..BINS - 1 {
                    acc.join(&bins[b].0);
                    cnt += bins[b].1;
                    if cnt == 0 || cnt as usize == n {
                        continue;
                    }
                    let cost = acc.area() * cnt as f32 + right[b + 1];
                    if cost < best.0 {
                        best = (cost, axis, b);
                    }
                }
            }
            let leaf_cost = bb.area() * n as f32;
            if (best.0 == f32::MAX || best.0 >= leaf_cost) && n <= 16 {
                leaf(&mut nodes);
                continue;
            }
            let (axis, bin) = (best.1, best.2);
            let mid = if best.0 == f32::MAX {
                s + n / 2
            } else {
                let k = BINS as f32 / ext[axis];
                let slice = &mut order[s..e];
                let mut l = 0;
                for j in 0..slice.len() {
                    let i = slice[j] as usize;
                    let b = (((cent[i][axis] - cb.lo[axis]) * k) as usize).min(BINS - 1);
                    if b <= bin {
                        slice.swap(l, j);
                        l += 1;
                    }
                }
                s + l
            };
            let left = nodes.len();
            nodes.push(nodes[ni]);
            let right = nodes.len();
            nodes.push(nodes[ni]);
            nodes[ni].index = right as u32;
            nodes[ni].count = 0;
            nodes[ni].axis = axis as u16;
            // the left child is always `right - 1`
            stack.push((right, mid, e));
            stack.push((left, s, mid));
        }
        let v0 = order.iter().map(|&i| tris[i as usize][0]).collect();
        let e1 = order
            .iter()
            .map(|&i| tris[i as usize][1] - tris[i as usize][0])
            .collect();
        let e2 = order
            .iter()
            .map(|&i| tris[i as usize][2] - tris[i as usize][0])
            .collect();
        Bvh {
            nodes,
            order,
            v0,
            e1,
            e2,
        }
    }

    #[inline(always)]
    fn slab(n: &Node, o: Vec3, inv: Vec3, tmax: f32) -> f32 {
        let t0 = (n.lo - o) * inv;
        let t1 = (n.hi - o) * inv;
        let tn = t0.min(t1).max_element().max(0.0);
        let tf = t0.max(t1).min_element().min(tmax);
        if tn <= tf {
            tn
        } else {
            f32::MAX
        }
    }

    #[inline(always)]
    fn tri(&self, j: usize, o: Vec3, d: Vec3, tmax: f32) -> Option<(f32, f32, f32)> {
        let (e1, e2) = (self.e1[j], self.e2[j]);
        let p = d.cross(e2);
        let det = e1.dot(p);
        if det.abs() < 1e-12 {
            return None;
        }
        let inv = 1.0 / det;
        let s = o - self.v0[j];
        let u = s.dot(p) * inv;
        if !(0.0..=1.0).contains(&u) {
            return None;
        }
        let q = s.cross(e1);
        let v = d.dot(q) * inv;
        if v < 0.0 || u + v > 1.0 {
            return None;
        }
        let t = e2.dot(q) * inv;
        (t > 1e-5 && t < tmax).then_some((t, u, v))
    }

    /// How far along the ray it reaches the box round every triangle
    /// (0 when it starts inside), or `None` when it passes it by.
    pub fn entry(&self, o: Vec3, d: Vec3) -> Option<f32> {
        if self.order.is_empty() {
            return None;
        }
        let inv = Vec3::new(1.0 / d.x, 1.0 / d.y, 1.0 / d.z);
        let t = Self::slab(&self.nodes[0], o, inv, f32::MAX);
        (t != f32::MAX).then_some(t)
    }

    /// The nearest triangle along the ray, if any is nearer than `tmax`.
    pub fn intersect(&self, o: Vec3, d: Vec3, tmax: f32) -> Option<Hit> {
        self.walk(o, d, tmax, |_, _| Visit::Closest)
    }

    /// How much light gets through to `tmax`: nothing past an opaque
    /// triangle, `glass` of it through each see-through one.
    pub fn transmit(&self, o: Vec3, d: Vec3, tmax: f32, see_through: impl Fn(u32) -> bool) -> f32 {
        let mut through = 1.0f32;
        let blocked = self
            .walk(o, d, tmax, |tri, _| {
                if see_through(tri) {
                    through *= 0.86;
                    Visit::Skip
                } else {
                    Visit::Stop
                }
            })
            .is_some();
        if blocked {
            0.0
        } else {
            through
        }
    }

    #[inline(always)]
    fn walk(
        &self,
        o: Vec3,
        d: Vec3,
        mut tmax: f32,
        mut on_hit: impl FnMut(u32, f32) -> Visit,
    ) -> Option<Hit> {
        // nothing to hit: a tree of no triangles has a root with no
        // children, and walking it took one from index 0 (a model with
        // nothing in it stopped the renderer's threads)
        if self.order.is_empty() {
            return None;
        }
        let inv = Vec3::new(1.0 / d.x, 1.0 / d.y, 1.0 / d.z);
        let neg = [d.x < 0.0, d.y < 0.0, d.z < 0.0];
        let mut best: Option<Hit> = None;
        let mut stack = [0u32; 64];
        let mut sp = 0;
        let mut ni = 0usize;
        if Self::slab(&self.nodes[0], o, inv, tmax) == f32::MAX {
            return None;
        }
        loop {
            let n = &self.nodes[ni];
            if n.count > 0 {
                let s = n.index as usize;
                for j in s..s + n.count as usize {
                    if let Some((t, u, v)) = self.tri(j, o, d, tmax) {
                        let tri = self.order[j];
                        match on_hit(tri, t) {
                            Visit::Closest => {
                                tmax = t;
                                best = Some(Hit { t, tri, u, v });
                            }
                            Visit::Stop => return Some(Hit { t, tri, u, v }),
                            Visit::Skip => {}
                        }
                    }
                }
            } else {
                let right = n.index as usize;
                let left = right - 1;
                let (near, far) = if neg[n.axis as usize] {
                    (right, left)
                } else {
                    (left, right)
                };
                let tn = Self::slab(&self.nodes[near], o, inv, tmax);
                let tf = Self::slab(&self.nodes[far], o, inv, tmax);
                match (tn < f32::MAX, tf < f32::MAX) {
                    (true, true) => {
                        stack[sp] = far as u32;
                        sp += 1;
                        ni = near;
                        continue;
                    }
                    (true, false) => {
                        ni = near;
                        continue;
                    }
                    (false, true) => {
                        ni = far;
                        continue;
                    }
                    _ => {}
                }
            }
            // pop the next node still in reach: tmax may have shrunk
            loop {
                if sp == 0 {
                    return best;
                }
                sp -= 1;
                ni = stack[sp] as usize;
                if Self::slab(&self.nodes[ni], o, inv, tmax) < f32::MAX {
                    break;
                }
            }
        }
    }
}

enum Visit {
    Closest,
    Stop,
    Skip,
}
