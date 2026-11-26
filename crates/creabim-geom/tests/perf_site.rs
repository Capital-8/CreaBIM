//! What the ground costs to work out: a survey of a given size built
//! once, then a pad moved step by step (as a drag would), then a spot
//! height changed. Opt-in, and only meaningful in an optimised build:
//!
//! ```text
//! cargo test --release -p creabim-geom --test perf_site -- --ignored --nocapture
//! ```

use creabim_core::{Element, ElementKind, Level, Model, Pad, Site, SiteForm};
use glam::{DVec2, DVec3};
use std::time::Instant;

/// A square of rolling ground `side` metres across, surveyed on a grid of
/// `n` by `n` points.
fn survey(side: f64, n: usize) -> (Model, Site) {
    let lv = Element::new(
        "Ground",
        ElementKind::Level(Level {
            elevation: 0.0,
            height: 3.0,
        }),
    );
    let mut t = Site::terrain(
        lv.id,
        creabim_core::polygon::rectangle(DVec2::ZERO, DVec2::new(side, side)),
        0.0,
        1.0,
    );
    if let SiteForm::Terrain { points, data, .. } = &mut t.form {
        let step = side / (n - 1) as f64;
        for i in 0..n {
            for j in 0..n {
                let (x, y) = (i as f64 * step, j as f64 * step);
                // a fall to the south with two swells, and a little
                // scatter so no four points are on a plane
                let z = y * 0.06
                    + 1.5 * (x / side * 6.0).sin() * (y / side * 4.0).cos()
                    + 0.02 * ((i * 31 + j * 17) % 7) as f64;
                points.push(DVec3::new(x, y, z));
            }
        }
        data.survey = true;
    }
    (Model::from_elements([lv]).unwrap(), t)
}

fn with_pad(t: &Site, at: DVec2, extra: usize) -> Site {
    with_pad_placed(t, at, extra, false)
}

/// The pad that moves, with `extra` other works worked in before it
/// (`last`) or after it.
fn with_pad_placed(t: &Site, at: DVec2, extra: usize, last: bool) -> Site {
    let mut t = t.clone();
    if let SiteForm::Terrain { data, .. } = &mut t.form {
        if !last {
            data.pads.push(Pad::new(
                creabim_core::polygon::rectangle(at, at + DVec2::new(12.0, 9.0)),
                3.0,
            ));
        }
        // other works that do not move, well away from it
        for k in 0..extra {
            let o = DVec2::new(10.0 + 20.0 * k as f64, 10.0);
            data.pads.push(Pad::dig(
                creabim_core::polygon::rectangle(o, o + DVec2::new(6.0, 4.0)),
                1.2,
                0.5,
            ));
        }
        if last {
            data.pads.push(Pad::new(
                creabim_core::polygon::rectangle(at, at + DVec2::new(12.0, 9.0)),
                3.0,
            ));
        }
    }
    t
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

fn run(side: f64, n: usize) {
    let (m, t) = survey(side, n);
    let t0 = Instant::now();
    let g = creabim_geom::ground::of(&t).expect("a ground");
    let first = ms(t0);
    let t0 = Instant::now();
    let lines = g.contours(0.5, 0.0);
    let contours = ms(t0);
    let mid = DVec2::splat(side * 0.5);

    // a pad dragged in eight steps: each step is a new ground
    let mut drag = vec![];
    for k in 0..8 {
        let p = with_pad(&t, mid + DVec2::new(0.4 * k as f64, 0.3 * k as f64), 0);
        let t0 = Instant::now();
        let s = creabim_geom::earth::shaped(&m, &p).expect("shaped");
        assert!(s.changed());
        drag.push(ms(t0));
    }
    // the same with four other works on the ground
    let mut busy = vec![];
    for k in 0..8 {
        let p = with_pad(&t, mid + DVec2::new(0.4 * k as f64, 0.3 * k as f64), 4);
        let t0 = Instant::now();
        let _ = creabim_geom::earth::shaped(&m, &p).expect("shaped");
        busy.push(ms(t0));
    }
    // the same, the pad that moves worked in after the four others: the
    // ground they left is kept
    let mut after = vec![];
    for k in 0..8 {
        let p = with_pad_placed(
            &t,
            mid + DVec2::new(0.4 * k as f64, 0.3 * k as f64),
            4,
            true,
        );
        let t0 = Instant::now();
        let _ = creabim_geom::earth::shaped(&m, &p).expect("shaped");
        after.push(ms(t0));
    }
    // a spot height changed: the survey itself is new
    let mut spot = vec![];
    for k in 0..4 {
        let mut p = t.clone();
        if let SiteForm::Terrain { points, .. } = &mut p.form {
            let i = points.len() / 2;
            points[i].z += 0.1 * (k + 1) as f64;
        }
        let t0 = Instant::now();
        let _ = creabim_geom::ground::of(&p).expect("a ground");
        spot.push(ms(t0));
    }
    let mean = |v: &[f64]| v.iter().sum::<f64>() / v.len() as f64;
    let worst = |v: &[f64]| v.iter().cloned().fold(0.0, f64::max);
    println!(
        "{:>6} points ({side:.0} m): ground {first:7.1} ms | contours {contours:6.1} ms ({} lines) | pad step {:7.1} (worst {:7.1}) | with 4 works {:7.1} (worst {:7.1}) | last of 5 {:7.1} (first {:7.1}) | spot {:7.1}",
        n * n,
        lines.len(),
        mean(&drag),
        worst(&drag),
        mean(&busy),
        worst(&busy),
        mean(&after[1..]),
        after[0],
        mean(&spot),
    );
}

#[test]
#[ignore = "a timing, run by hand in a release build"]
fn what_the_ground_costs() {
    for (side, n) in [(60.0, 12), (120.0, 32), (200.0, 71), (300.0, 142)] {
        run(side, n);
    }
}
