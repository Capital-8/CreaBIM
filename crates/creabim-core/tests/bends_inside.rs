//! Round a rounded bend, the inner edge of a road, a path or a trench
//! takes the radius less half the width, and a sharp corner where that is
//! nothing: never an edge that loops back on itself (owner, MU98_01 and
//! MU98_09: "radius dont work well on the internals").

use creabim_core::road::{offset_round, Road};
use creabim_core::walk::{Surface, Walk};
use glam::DVec2;

/// Consecutive points closer than a tenth of a millimetre made one: a
/// sharp inner corner repeats its point once for each of the arc's.
fn dedup(pts: &[DVec2]) -> Vec<DVec2> {
    let mut out: Vec<DVec2> = vec![];
    for p in pts {
        if out.last().is_none_or(|q| q.distance(*p) > 1e-4) {
            out.push(*p);
        }
    }
    out
}

/// Do segments ab and cd cross, other than at a shared end?
fn cross(a: DVec2, b: DVec2, c: DVec2, d: DVec2) -> bool {
    let o = |p: DVec2, q: DVec2, r: DVec2| (q - p).perp_dot(r - p);
    let (d1, d2, d3, d4) = (o(c, d, a), o(c, d, b), o(a, b, c), o(a, b, d));
    let eps = 1e-9;
    ((d1 > eps && d2 < -eps) || (d1 < -eps && d2 > eps))
        && ((d3 > eps && d4 < -eps) || (d3 < -eps && d4 > eps))
}

/// No two of its segments that are not neighbours cross.
fn simple(pts: &[DVec2], closed: bool) -> Result<(), String> {
    let p = dedup(pts);
    let mut segs: Vec<(DVec2, DVec2)> = p.windows(2).map(|w| (w[0], w[1])).collect();
    if closed && p.len() > 2 {
        segs.push((p[p.len() - 1], p[0]));
    }
    let n = segs.len();
    for i in 0..n {
        for j in i + 2..n {
            if closed && i == 0 && j == n - 1 {
                continue;
            }
            if cross(segs[i].0, segs[i].1, segs[j].0, segs[j].1) {
                return Err(format!("segments {i} and {j} cross"));
            }
        }
    }
    Ok(())
}

/// A line turning by `deg` at its middle, both stretches 30 m.
fn turn(deg: f64) -> Vec<DVec2> {
    let a = DVec2::ZERO;
    let b = DVec2::new(30.0, 0.0);
    let c = b + DVec2::from_angle(deg.to_radians()) * 30.0;
    vec![a, b, c]
}

const WIDTH: f64 = 6.0;

fn radii() -> [f64; 5] {
    [0.0, 0.001, WIDTH / 4.0, WIDTH / 2.0, 2.0 * WIDTH]
}

#[test]
fn every_edge_round_a_bend_is_simple() {
    for deg in [90.0, -90.0, 150.0, -150.0] {
        for r in radii() {
            let path = turn(deg);
            for d in [-WIDTH / 2.0, -0.3, 0.0, 0.3, WIDTH / 2.0, WIDTH / 2.0 + 2.0] {
                let e = offset_round(&path, r, d);
                simple(&e, false).unwrap_or_else(|m| panic!("turn {deg}°, R {r}, d {d}: {m}"));
            }
        }
    }
}

#[test]
fn the_inner_edge_is_never_a_reversed_arc() {
    // the inner edge only ever goes forward: each step along it points
    // the way the centreline's stretch it pairs with does, or nowhere
    for deg in [90.0, 150.0] {
        for r in radii() {
            let path = turn(deg);
            let mid = offset_round(&path, r, 0.0);
            let inner = offset_round(&path, r, WIDTH / 2.0);
            assert_eq!(mid.len(), inner.len(), "edges pair with the centreline");
            for k in 0..mid.len() - 1 {
                let along = mid[k + 1] - mid[k];
                let step = inner[k + 1] - inner[k];
                assert!(
                    step.dot(along) >= -1e-9,
                    "turn {deg}°, R {r}: step {k} of the inner edge runs backwards"
                );
            }
        }
    }
}

#[test]
fn the_inner_edge_takes_the_radius_less_half_the_width() {
    // a wide bend: the inner edge is an arc of R - w/2 round the same centre
    let path = turn(90.0);
    let r = 10.0;
    let mid = offset_round(&path, r, 0.0);
    let inner = offset_round(&path, r, WIDTH / 2.0);
    let outer = offset_round(&path, r, -WIDTH / 2.0);
    // the arc's centre: 10 m in from where the bend starts, on the left
    let centre = DVec2::new(30.0 - r, r);
    let on_arc = |pts: &[DVec2], want: f64| {
        pts.iter()
            .filter(|p| p.x > 30.0 - r + 1e-6 && p.y < r - 1e-6)
            .all(|p| (p.distance(centre) - want).abs() < 1e-6)
    };
    assert!(on_arc(&mid, r));
    assert!(on_arc(&inner, r - WIDTH / 2.0));
    assert!(on_arc(&outer, r + WIDTH / 2.0));
    // a tight one: the inner edge is the sharp corner of its two sides
    let tight = offset_round(&path, 1.0, WIDTH / 2.0);
    let corner = DVec2::new(30.0 - WIDTH / 2.0, WIDTH / 2.0);
    assert!(tight.iter().any(|p| p.distance(corner) < 1e-6));
    assert!(tight
        .iter()
        .all(|p| p.x <= corner.x + 1e-6 || p.y >= corner.y - 1e-6));
}

#[test]
fn a_roads_edges_kerbs_channels_and_outline_are_simple() {
    for deg in [90.0, -90.0, 150.0, -150.0] {
        for r in radii() {
            let mut road = Road::new(turn(deg), WIDTH);
            road.kerb_radius = Some(r);
            road.footpath = 1.8;
            road.gullies = 20.0;
            let what = |m: &str| format!("road, turn {deg}°, R {r}: {m}");
            let (l, rt) = road.edges();
            simple(&l.iter().map(|p| p.truncate()).collect::<Vec<_>>(), false)
                .unwrap_or_else(|m| panic!("{}", what(&format!("left edge {m}"))));
            simple(&rt.iter().map(|p| p.truncate()).collect::<Vec<_>>(), false)
                .unwrap_or_else(|m| panic!("{}", what(&format!("right edge {m}"))));
            for ch in road.channels() {
                simple(&ch, false).unwrap_or_else(|m| panic!("{}", what(&format!("channel {m}"))));
            }
            let (ring, zs) = road.outline();
            assert_eq!(ring.len(), zs.len());
            simple(&ring, true).unwrap_or_else(|m| panic!("{}", what(&format!("outline {m}"))));
        }
    }
}

#[test]
fn a_paths_edges_and_outline_are_simple() {
    for deg in [90.0, -90.0, 150.0, -150.0] {
        for r in radii() {
            let mut w = Walk::new(turn(deg), Surface::Gravel);
            w.width = WIDTH;
            w.edging = 0.05;
            w.radius = r;
            let (l, rt) = w.edges(WIDTH / 2.0);
            assert_eq!(l.len(), w.line().len());
            simple(&l, false).unwrap_or_else(|m| panic!("path {deg}° R {r}: left {m}"));
            simple(&rt, false).unwrap_or_else(|m| panic!("path {deg}° R {r}: right {m}"));
            simple(&w.outline(), true).unwrap_or_else(|m| panic!("path {deg}° R {r}: outline {m}"));
        }
    }
}

#[test]
fn a_trenchs_outline_is_simple() {
    for deg in [90.0, -90.0, 150.0, -150.0] {
        for r in radii() {
            let ring = creabim_core::site::trench_round(&turn(deg), r, WIDTH);
            simple(&ring, true).unwrap_or_else(|m| panic!("trench {deg}° R {r}: {m}"));
            assert!(creabim_core::polygon::signed_area2(&ring) > 0.0);
        }
    }
}
