//! What each step of moving a whole building costs the 3D scene, with its
//! wall and slab cuts made again (as a drop does) and taken along (as a
//! live move does). Opt-in, and only meaningful in an optimised build:
//!
//! ```text
//! cargo test --release -p creabim-geom --test perf_move -- --ignored --nocapture
//! ```
//!
//! This file is its own test program, so the switch it turns
//! ([`creabim_geom::scene::carry_cuts`]) touches no other test.

use creabim_core::transform::{transformed, Xform};
use creabim_core::{Document, ElementKind, Model, Transaction};
use glam::DVec3;
use std::time::Instant;

fn moved(model: &Model, by: DVec3) -> Model {
    let mut doc = Document::from_model(model.clone());
    let mut tx = Transaction::new("Move");
    for e in model.iter() {
        let moves = matches!(
            e.kind,
            ElementKind::Wall(_)
                | ElementKind::Slab(_)
                | ElementKind::Roof(_)
                | ElementKind::Column(_)
                | ElementKind::Beam(_)
                | ElementKind::Footing(_)
                | ElementKind::Stair(_)
                | ElementKind::Object(_)
                | ElementKind::Shaft(_)
                | ElementKind::Cut(_)
                | ElementKind::Railing(_)
                | ElementKind::FreeMesh(_)
                | ElementKind::Part(_)
        );
        if let Some(n) = transformed(model, e, &Xform::Move(by)).filter(|_| moves) {
            tx = tx.replace(e.clone(), n);
        }
    }
    doc.commit(tx).unwrap();
    doc.model().clone()
}

fn steps(model: &Model, from: f64) -> Vec<f64> {
    (1..=8)
        .map(|k| {
            let m = moved(
                model,
                DVec3::new(from + 0.137 * k as f64, -0.211 * k as f64, 0.0),
            );
            let t = Instant::now();
            let scene = creabim_geom::build_scene(&m);
            assert!(!scene.is_empty());
            t.elapsed().as_secs_f64() * 1000.0
        })
        .collect()
}

#[test]
#[ignore = "timings: run by hand in an optimised build"]
fn a_moving_house_step_by_step() {
    for (name, model) in [
        ("starter", creabim_core::starter::starter_house()),
        ("garden", creabim_core::garden::garden_house()),
        ("sample", creabim_core::sample::sample_house()),
    ] {
        let t = Instant::now();
        creabim_geom::build_scene(&model);
        let rest = t.elapsed().as_secs_f64() * 1000.0;
        let again = steps(&model, 3.0);
        creabim_geom::scene::carry_cuts(true);
        let along = steps(&model, 9.0);
        let used = creabim_geom::scene::carry_cuts(false);
        let line = |v: &[f64]| {
            v.iter()
                .map(|x| format!("{x:.0}"))
                .collect::<Vec<_>>()
                .join(" ")
        };
        println!("{name}: scene at rest {rest:.0} ms");
        println!("  cuts made again, ms a step: {}", line(&again));
        println!("  cuts taken along, ms a step: {}", line(&along));
        assert!(used, "{name}: a moving house takes cuts along");
    }
}
