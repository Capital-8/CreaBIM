//! Reading a terrain's form: its ground shaded by slope, by the way it
//! faces (aspect) or by height, each band's area, and which way water
//! runs off it with its low points, where it would pond.
//!
//! The ground read is the one drawn: the proposed ground, with its pads
//! and digs worked in.

use crate::ground::Ground;
use creabim_core::GroundAnalysis;
use glam::{DVec2, DVec3};

/// A band of the shading: what it covers, its colour, and how much of
/// the ground (in plan) is in it.
#[derive(Clone, Debug, PartialEq)]
pub struct Band {
    pub label: String,
    pub rgb: [f32; 3],
    pub area: f64,
}

/// The ground shaded: each drawn triangle (in plan) with its band.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shading {
    pub tris: Vec<([DVec2; 3], usize)>,
    pub bands: Vec<Band>,
}

/// Slope bands: the steepest each holds (per cent), what it is, colour.
const SLOPES: [(f64, &str, [f32; 3]); 7] = [
    (2.0, "0–2% flat", [0.55, 0.78, 0.92]),
    (5.0, "2–5% gentle", [0.55, 0.82, 0.55]),
    (10.0, "5–10%", [0.80, 0.88, 0.45]),
    (15.0, "10–15%", [0.98, 0.84, 0.40]),
    (25.0, "15–25% steep", [0.96, 0.60, 0.32]),
    (33.4, "25–33%, to 1 in 3", [0.88, 0.32, 0.28]),
    (f64::INFINITY, "over 1 in 3", [0.62, 0.30, 0.62]),
];

/// The eight ways ground can face, from north round by east, and flat.
const ASPECTS: [(&str, [f32; 3]); 9] = [
    ("Faces north", [0.86, 0.30, 0.30]),
    ("Faces north-east", [0.96, 0.60, 0.30]),
    ("Faces east", [0.96, 0.86, 0.36]),
    ("Faces south-east", [0.56, 0.82, 0.40]),
    ("Faces south", [0.40, 0.80, 0.78]),
    ("Faces south-west", [0.46, 0.66, 0.92]),
    ("Faces west", [0.38, 0.42, 0.86]),
    ("Faces north-west", [0.80, 0.44, 0.80]),
    ("Flat (under 1%)", [0.80, 0.80, 0.80]),
];

/// How steep, per cent, a triangle is.
fn slope(t: &[DVec3; 3]) -> (f64, DVec2) {
    let n = (t[1] - t[0]).cross(t[2] - t[0]);
    if n.z.abs() < 1e-12 {
        return (f64::INFINITY, DVec2::ZERO);
    }
    // the gradient: dz/dx, dz/dy
    let g = DVec2::new(-n.x / n.z, -n.y / n.z);
    (g.length() * 100.0, g)
}

fn plan_area(t: &[DVec2; 3]) -> f64 {
    (t[1] - t[0]).perp_dot(t[2] - t[0]).abs() / 2.0
}

/// A colour between `a` and `b`.
fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [0, 1, 2].map(|k| a[k] + (b[k] - a[k]) * t)
}

/// `g` (heights above its level, `base` the level's elevation) shaded by
/// `how`; `north` is true north's turn from up, degrees anticlockwise,
/// and `interval` the height bands' (0 is eight bands over its range).
pub fn shade(g: &Ground, how: GroundAnalysis, north: f64, base: f64, interval: f64) -> Shading {
    let tris: Vec<[DVec3; 3]> = g
        .fine
        .tris
        .iter()
        .map(|t| t.map(|i| g.fine.verts[i as usize]))
        .collect();
    let flat = |t: &[DVec3; 3]| t.map(|p| p.truncate());
    let mut out = Shading::default();
    match how {
        GroundAnalysis::None => return out,
        GroundAnalysis::Slope => {
            out.bands = SLOPES
                .iter()
                .map(|(_, l, c)| Band {
                    label: l.to_string(),
                    rgb: *c,
                    area: 0.0,
                })
                .collect();
            for t in &tris {
                let (pc, _) = slope(t);
                let k = SLOPES
                    .iter()
                    .position(|b| pc < b.0)
                    .unwrap_or(SLOPES.len() - 1);
                out.tris.push((flat(t), k));
            }
        }
        GroundAnalysis::Aspect => {
            out.bands = ASPECTS
                .iter()
                .map(|(l, c)| Band {
                    label: l.to_string(),
                    rgb: *c,
                    area: 0.0,
                })
                .collect();
            for t in &tris {
                let (pc, grad) = slope(t);
                // it faces the way it falls
                let k = if pc < 1.0 {
                    8
                } else {
                    let az = creabim_core::lot::azimuth(-grad, north);
                    ((az + 22.5) / 45.0).floor() as usize % 8
                };
                out.tris.push((flat(t), k));
            }
        }
        GroundAnalysis::Height => {
            let (lo, hi) = g.range();
            if hi.partial_cmp(&lo) != Some(std::cmp::Ordering::Greater) {
                return out;
            }
            let step = if interval > 0.0 {
                interval
            } else {
                (hi - lo) / 8.0
            };
            let first = (lo / step).floor();
            let n = (((hi / step).ceil() - first) as usize).clamp(1, 40);
            let low = [0.40_f32, 0.62, 0.45];
            let mid = [0.90_f32, 0.84, 0.52];
            let high = [0.62_f32, 0.40, 0.28];
            out.bands = (0..n)
                .map(|k| {
                    let a = (first + k as f64) * step;
                    let t = if n > 1 {
                        k as f32 / (n - 1) as f32
                    } else {
                        0.5
                    };
                    let rgb = if t < 0.5 {
                        mix(low, mid, t * 2.0)
                    } else {
                        mix(mid, high, t * 2.0 - 1.0)
                    };
                    Band {
                        label: format!("{:.2} to {:.2}", base + a, base + a + step),
                        rgb,
                        area: 0.0,
                    }
                })
                .collect();
            for t in &tris {
                let z = (t[0].z + t[1].z + t[2].z) / 3.0;
                let k = (((z / step).floor() - first).max(0.0) as usize).min(n - 1);
                out.tris.push((flat(t), k));
            }
        }
    }
    for (t, k) in &out.tris {
        out.bands[*k].area += plan_area(t);
    }
    out
}

/// Which way water runs off `g`: arrows downhill on a grid `spacing`
/// apart inside `outline` (each its middle and the way it points), and
/// the low points where it gathers.
pub fn flow(g: &Ground, outline: &[DVec2], spacing: f64) -> (Vec<(DVec2, DVec2)>, Vec<DVec2>) {
    let (lo, hi) = outline.iter().fold(
        (DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY)),
        |(a, b), p| (a.min(*p), b.max(*p)),
    );
    if !lo.is_finite() || spacing <= 0.0 {
        return (vec![], vec![]);
    }
    let h = |p: DVec2| {
        creabim_core::polygon::contains(outline, p)
            .then(|| g.height(p))
            .flatten()
    };
    let (nx, ny) = (
        ((hi.x - lo.x) / spacing).floor() as i64,
        ((hi.y - lo.y) / spacing).floor() as i64,
    );
    let at = |i: i64, j: i64| lo + DVec2::new(i as f64 + 0.5, j as f64 + 0.5) * spacing;
    let mut arrows = vec![];
    let mut lows = vec![];
    let d = spacing * 0.25;
    for j in 0..ny.max(0) {
        for i in 0..nx.max(0) {
            let p = at(i, j);
            let Some(z) = h(p) else { continue };
            let gx = match (h(p + DVec2::X * d), h(p - DVec2::X * d)) {
                (Some(a), Some(b)) => (a - b) / (2.0 * d),
                _ => continue,
            };
            let gy = match (h(p + DVec2::Y * d), h(p - DVec2::Y * d)) {
                (Some(a), Some(b)) => (a - b) / (2.0 * d),
                _ => continue,
            };
            let grad = DVec2::new(gx, gy);
            if grad.length() > 0.005 {
                arrows.push((p, -grad.normalize()));
            }
            // a low point: lower than all eight round it, all on the ground
            let round: Vec<Option<f64>> = (-1..=1)
                .flat_map(|dj| (-1..=1).map(move |di| (di, dj)))
                .filter(|(di, dj)| (*di, *dj) != (0, 0))
                .map(|(di, dj)| h(at(i + di, j + dj)))
                .collect();
            if round.iter().all(|q| q.is_some_and(|q| q > z + 1e-4)) {
                lows.push(p);
            }
        }
    }
    (arrows, lows)
}

/// A good spacing for flow arrows over an outline: some twenty across.
pub fn flow_spacing(outline: &[DVec2]) -> f64 {
    let (lo, hi) = outline.iter().fold(
        (DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY)),
        |(a, b), p| (a.min(*p), b.max(*p)),
    );
    ((hi - lo).max_element() / 20.0).clamp(0.5, 20.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use creabim_core::{ElementId, Site};

    /// A terrain falling 1 in 10 to the south, with a hollow in it.
    fn sloping(hollow: bool) -> Ground {
        let mut s = Site::terrain(
            ElementId::new(),
            vec![
                DVec2::ZERO,
                DVec2::new(20.0, 0.0),
                DVec2::new(20.0, 20.0),
                DVec2::new(0.0, 20.0),
            ],
            0.0,
            0.5,
        );
        if let creabim_core::SiteForm::Terrain { points, data, .. } = &mut s.form {
            for j in 0..=4 {
                for i in 0..=4 {
                    let p = DVec2::new(i as f64 * 5.0, j as f64 * 5.0);
                    let mut z = p.y * 0.1;
                    if hollow && (i, j) == (2, 2) {
                        z -= 2.0;
                    }
                    points.push(p.extend(z));
                }
            }
            data.survey = true;
            data.faceted = true;
        }
        Ground::build(&s).unwrap()
    }

    #[test]
    fn a_slope_of_one_in_ten_is_in_its_band_and_faces_south() {
        let g = sloping(false);
        let s = shade(&g, GroundAnalysis::Slope, 0.0, 0.0, 0.0);
        let total: f64 = s.bands.iter().map(|b| b.area).sum();
        assert!((total - 400.0).abs() < 1e-6);
        // 10% is in the 10–15% band
        assert!((s.bands[3].area - 400.0).abs() < 1e-6, "{:?}", s.bands);
        // it falls to the south, so it faces south
        let a = shade(&g, GroundAnalysis::Aspect, 0.0, 0.0, 0.0);
        assert!((a.bands[4].area - 400.0).abs() < 1e-6);
        // with true north turned 90° anticlockwise of up, ground falling
        // down the drawing faces west
        let a = shade(&g, GroundAnalysis::Aspect, 90.0, 0.0, 0.0);
        assert!((a.bands[6].area - 400.0).abs() < 1e-6, "{:?}", a.bands);
        // height bands of 0.5 m over 2 m of fall: four
        let hgt = shade(&g, GroundAnalysis::Height, 0.0, 100.0, 0.5);
        assert_eq!(hgt.bands.len(), 4);
        assert!(hgt.bands[0].label.starts_with("100.00"));
    }

    #[test]
    fn water_runs_downhill_and_gathers_in_the_hollow() {
        let g = sloping(false);
        let outline = [
            DVec2::ZERO,
            DVec2::new(20.0, 0.0),
            DVec2::new(20.0, 20.0),
            DVec2::new(0.0, 20.0),
        ];
        let (arrows, lows) = flow(&g, &outline, 2.0);
        assert!(!arrows.is_empty() && lows.is_empty());
        assert!(arrows.iter().all(|(_, d)| d.y < -0.99));
        let g = sloping(true);
        let (_, lows) = flow(&g, &outline, 1.0);
        assert!(
            lows.iter()
                .any(|p| p.distance(DVec2::new(10.0, 10.0)) < 1.5),
            "{lows:?}"
        );
    }
}
