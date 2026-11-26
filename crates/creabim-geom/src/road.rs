//! A road's solid: its surface across (footpath, kerb, carriageway with
//! its crown or one-way fall, kerb, footpath) carried along its
//! centreline, its make-up under it.

use crate::mesh::TriMesh;
use creabim_core::road::Road;
use glam::{DVec2, DVec3};

/// Across the road from its left back to its right back: each point's
/// offset from the centreline (left positive) and its height above the
/// centreline's level.
pub fn profile(r: &Road) -> Vec<(f64, f64)> {
    let half = r.width / 2.0;
    let f = r.fall / 100.0;
    let (zl, zr) = if r.crowned {
        (-half * f, -half * f)
    } else {
        (half * f, -half * f)
    };
    let mut p = vec![];
    if r.footpath > 0.0 {
        p.push((half + r.footpath, zl + r.kerb));
    }
    if r.kerb > 0.0 {
        p.push((half, zl + r.kerb));
    }
    p.push((half, zl));
    if r.crowned {
        p.push((0.0, 0.0));
    }
    p.push((-half, zr));
    if r.kerb > 0.0 {
        p.push((-half, zr + r.kerb));
    }
    if r.footpath > 0.0 {
        p.push((-half - r.footpath, zr + r.kerb));
    }
    p
}

/// The road's solid, its level `base` above the project's zero.
pub fn mesh(r: &Road, base: f64) -> TriMesh {
    let mut m = TriMesh::default();
    if r.path.len() < 2 {
        return m;
    }
    // as it is built: its bends rounded to its radius
    let line = r.line();
    let plan = r.plan();
    let prof = profile(r);
    // each profile point's line along the road
    let lines: Vec<Vec<DVec3>> = prof
        .iter()
        .map(|(d, dz)| {
            r.built(*d)
                .into_iter()
                .map(|p| p.with_z(base + p.z + dz))
                .collect()
        })
        .collect();
    let down = DVec3::Z * r.thickness;
    let n = line.len();
    let k = prof.len();
    let centre = |i: usize| line[i].truncate();
    for i in 0..n - 1 {
        for j in 0..k - 1 {
            let q = [
                lines[j][i],
                lines[j + 1][i],
                lines[j + 1][i + 1],
                lines[j][i + 1],
            ];
            let mut nrm = (q[1] - q[0]).cross(q[3] - q[0]);
            if nrm.length_squared() < 1e-12 {
                nrm = (q[2] - q[1]).cross(q[3] - q[1]);
            }
            if nrm.length_squared() < 1e-12 {
                continue;
            }
            if nrm.z.abs() > 0.3 * nrm.length() {
                // the surface: up
                if nrm.z < 0.0 {
                    nrm = -nrm;
                }
            } else {
                // a kerb's face: towards the carriageway
                let mid = (q[0] + q[2]).truncate() / 2.0;
                let c = (centre(i) + centre(i + 1)) / 2.0;
                if nrm.truncate().dot(c - mid) < 0.0 {
                    nrm = -nrm;
                }
            }
            m.add_quad(q, nrm);
            // its underside, the make-up's depth below
            let b = q.map(|p| p - down);
            m.add_quad([b[0], b[3], b[2], b[1]], -nrm.with_z(nrm.z.abs()));
        }
        // the sides, down from the backs
        for (j, sign) in [(0, 1.0), (k - 1, -1.0)] {
            let (a, b) = (lines[j][i], lines[j][i + 1]);
            let out = (b - a).truncate().perp().normalize_or_zero() * sign;
            m.add_quad([a, b, b - down, a - down], out.extend(0.0));
        }
    }
    // the ends
    for (i, sign) in [(0, -1.0), (n - 1, 1.0)] {
        let along = if i == 0 {
            plan[1] - plan[0]
        } else {
            plan[n - 1] - plan[n - 2]
        };
        let out = (along.normalize_or_zero() * sign).extend(0.0);
        for j in 0..k - 1 {
            let (a, b) = (lines[j][i], lines[j + 1][i]);
            if a.distance(b) < 1e-9 {
                continue;
            }
            m.add_quad([a, b, b - down, a - down], out);
        }
    }
    m
}

/// Where along the centreline (`t` from 0 to its length) a point is, and
/// its level there, for marking chainages and levels.
pub fn at_chainage(r: &Road, t: f64) -> Option<(DVec2, DVec2, f64)> {
    let mut gone = 0.0;
    for w in r.line().windows(2) {
        let (a, b) = (w[0], w[1]);
        let len = a.truncate().distance(b.truncate());
        if len < 1e-9 {
            continue;
        }
        if t <= gone + len + 1e-9 {
            let s = ((t - gone) / len).clamp(0.0, 1.0);
            let p = a.lerp(b, s);
            return Some((p.truncate(), (b - a).truncate() / len, p.z));
        }
        gone += len;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_road_is_a_closed_solid_with_its_surface_up() {
        let mut r = Road::new(vec![DVec2::ZERO, DVec2::new(20.0, 0.0)], 6.0);
        r.footpath = 1.8;
        let m = mesh(&r, 10.0);
        assert!(!m.is_empty());
        // the crown's at 10 and the make-up 0.3 under the lowest edge
        let zs: Vec<f32> = m.positions.iter().map(|p| p[2]).collect();
        let hi = zs.iter().copied().fold(f32::MIN, f32::max);
        let lo = zs.iter().copied().fold(f32::MAX, f32::min);
        assert!((hi as f64 - 10.025).abs() < 1e-4, "{hi}");
        assert!((lo as f64 - (10.0 - 0.075 - 0.3)).abs() < 1e-4, "{lo}");
        assert_eq!(profile(&r).len(), 7);
        let (p, d, z) = at_chainage(&r, 5.0).unwrap();
        assert!((p.x - 5.0).abs() < 1e-9 && d.x > 0.99 && z.abs() < 1e-9);
    }
}
