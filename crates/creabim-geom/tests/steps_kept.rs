//! A terrain's works are worked in one after another, and the ground
//! after each is kept: changing a later work starts from the ground
//! before it. The ground must be the one worked out from nothing.

use creabim_core::{Element, ElementKind, Level, Model, Site, SiteForm};
use glam::{DVec2, DVec3};

fn model(second: DVec2, level: f64) -> Model {
    let lv = Element::new(
        "Ground Floor",
        ElementKind::Level(Level {
            elevation: 0.0,
            height: 3.0,
        }),
    );
    let sq = |c: DVec2, h: f64| {
        creabim_core::polygon::rectangle(c - DVec2::splat(h), c + DVec2::splat(h))
    };
    let mut t = Site::terrain(lv.id, sq(DVec2::ZERO, 30.0), 0.0, 1.0);
    if let SiteForm::Terrain { points, .. } = &mut t.form {
        for j in -2..=2 {
            for i in -2..=2 {
                let p = DVec2::new(i as f64 * 12.0, j as f64 * 12.0);
                points.push(DVec3::new(p.x, p.y, 0.04 * p.x + 0.02 * p.y));
            }
        }
    }
    let first = Site::pad(lv.id, sq(DVec2::new(-12.0, -8.0), 4.0), 0.5);
    let pit = Site::pit(lv.id, sq(DVec2::new(10.0, 12.0), 3.0), 1.2, 0.5);
    let moved = Site::pad(lv.id, sq(second, 4.0), level);
    let named = |n: &str, s: Site| Element::new(n, ElementKind::Site(s));
    Model::from_elements([
        lv,
        named("Terrain", t),
        named("Pad", first),
        named("Pit", pit),
        named("Pad", moved),
    ])
    .unwrap()
}

/// The ground's heights on a grid, its volumes and how many banks.
fn read(m: &Model) -> (Vec<f64>, f64, f64, usize) {
    let t = m
        .iter()
        .find_map(|e| match &e.kind {
            ElementKind::Site(s) if matches!(s.form, SiteForm::Terrain { .. }) => Some(s),
            _ => None,
        })
        .unwrap();
    let sh = creabim_geom::earth::shaped(m, t).unwrap();
    let mut zs = vec![];
    for j in -14..=14 {
        for i in -14..=14 {
            zs.push(
                sh.ground
                    .height(DVec2::new(i as f64 * 2.0, j as f64 * 2.0))
                    .unwrap(),
            );
        }
    }
    (zs, sh.cut, sh.fill, sh.banks.len())
}

#[test]
fn a_later_work_changed_gives_the_ground_worked_out_from_nothing() {
    // here the last pad is dragged and relevelled step by step, each
    // step starting from the ground the two works before it left
    let steps = [
        (DVec2::new(8.0, -10.0), -0.5),
        (DVec2::new(9.0, -10.5), -0.5),
        (DVec2::new(10.0, -11.0), -0.25),
        (DVec2::new(-10.0, -6.0), 1.0), // onto the first pad's bank
    ];
    let here: Vec<_> = steps.iter().map(|(p, z)| read(&model(*p, *z))).collect();
    // and each on a thread of its own, where nothing is kept
    for ((p, z), kept) in steps.into_iter().zip(here) {
        let fresh = std::thread::spawn(move || read(&model(p, z)))
            .join()
            .unwrap();
        assert_eq!(kept.3, fresh.3, "banks at {p}");
        assert!((kept.1 - fresh.1).abs() < 1e-9 && (kept.2 - fresh.2).abs() < 1e-9);
        for (a, b) in kept.0.iter().zip(&fresh.0) {
            assert!((a - b).abs() < 1e-9, "{a} against {b} at {p}");
        }
    }
}
