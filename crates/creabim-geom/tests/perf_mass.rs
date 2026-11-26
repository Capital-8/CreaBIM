//! What masses and their modifiers cost: one tower with each modifier,
//! then many towers. Opt-in, and only meaningful in an optimised build:
//!
//! ```text
//! cargo test --release -p creabim-geom --test perf_mass -- --ignored --nocapture
//! ```
//!
//! Each line is one change to the model (a modifier's setting, or a move)
//! and what it sets off: the shape of each mass, the whole 3D scene, a
//! plan and a section of the starter house the towers stand beside. The
//! figures are recorded in `docs/PERFORMANCE.md`; the one budget held here
//! is that many towers of one shape cost little more than their triangles.

use creabim_core::modifiers::{Corners, Modifier, ModifierKind as K, OpenFaces};
use creabim_core::{Document, EditMesh, Element, ElementId, ElementKind, FreeMesh, Transaction};
use glam::{DVec2, DVec3};
use std::time::Instant;

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

fn view(doc: &Document, name: &str) -> ElementId {
    doc.model()
        .iter()
        .find(|e| e.name == name)
        .map(|e| e.id)
        .expect("a view of the starter house")
}

fn tower(level: ElementId, at: DVec3, mods: Vec<K>) -> Element {
    let mut f = FreeMesh::new(
        Some(level),
        at,
        EditMesh::cuboid(DVec3::new(20.0, 15.0, 45.0)),
    );
    f.modifiers = mods.into_iter().map(Modifier::new).collect();
    Element::new("Tower", ElementKind::FreeMesh(f))
}

/// The best of three changes, in ms: shape, scene, plan, section.
fn measure(
    what: &str,
    n: usize,
    mods: &dyn Fn() -> Vec<K>,
    change: &dyn Fn(&mut FreeMesh, usize),
) -> [f64; 4] {
    let base = creabim_core::starter::starter_house();
    let level = base.levels().next().expect("a level").0.id;
    let mut doc = Document::from_model(base);
    let mut tx = Transaction::new("Towers");
    let mut ids = vec![];
    for i in 0..n {
        let at = DVec3::new(
            30.0 + (i % 6) as f64 * 30.0,
            30.0 + (i / 6) as f64 * 30.0,
            0.0,
        );
        let e = tower(level, at, mods());
        ids.push(e.id);
        tx = tx.insert(e);
    }
    doc.commit(tx).unwrap();
    let plan = view(&doc, "Ground Floor plan");
    let section = view(&doc, "Section A-A");
    let mut best = [f64::INFINITY; 4];
    let (mut faces, mut tris) = (0, 0);
    for round in 0..4 {
        for id in &ids {
            doc.edit(*id, "Change", |e| {
                if let ElementKind::FreeMesh(f) = &mut e.kind {
                    change(f, round);
                }
            })
            .unwrap();
        }
        let m = doc.model();
        let t = Instant::now();
        faces = ids
            .iter()
            .map(|id| match &m.get(*id).unwrap().kind {
                ElementKind::FreeMesh(f) => f.evaluated().mesh.faces.len(),
                _ => 0,
            })
            .sum();
        let shape = ms(t);
        let t = Instant::now();
        let scene = creabim_geom::build_scene(m);
        let built = ms(t);
        tris = scene.iter().map(|i| i.mesh.indices.len() / 3).sum();
        let t = Instant::now();
        let _ = creabim_geom::section::view_drawing_with(m, plan, false);
        let planned = ms(t);
        let t = Instant::now();
        let _ = creabim_geom::section::view_drawing_with(m, section, false);
        let cut = ms(t);
        // the first round warms up
        if round > 0 {
            for (b, v) in best.iter_mut().zip([shape, built, planned, cut]) {
                *b = b.min(v);
            }
        }
    }
    println!(
        "{what:<34} {n:>3} towers {faces:>7} faces {tris:>8} tris | shape {:>6.1}  scene {:>6.1}  plan {:>6.1}  section {:>7.1} ms",
        best[0], best[1], best[2], best[3]
    );
    best
}

#[test]
#[ignore = "timing: run with --release -- --ignored"]
fn masses_and_modifiers() {
    let slice = || K::Slice {
        heights: (1..15).map(|i| i as f64 * 3.0).collect(),
        gap: 0.05,
    };
    let panels = |w: f64, h: f64| K::Panelize {
        width: w,
        height: h,
        joint: 0.02,
        depth: 0.05,
    };
    let louvres = |spacing: f64| K::Louvres {
        sides: [true; 4],
        upright: true,
        spacing,
        depth: 0.3,
        thickness: 0.05,
        angle: 0.0,
    };
    let moved = |f: &mut FreeMesh, r: usize| f.origin.x += 0.1 * (r + 1) as f64;
    // a modifier's main setting nudged
    let nudge = |f: &mut FreeMesh, r: usize| {
        let step = 0.01 * (r + 1) as f64;
        match f.modifiers.last_mut().map(|m| &mut m.kind) {
            Some(K::Slice { gap, .. }) => *gap += step,
            Some(K::Panelize { width, .. }) => *width += step,
            Some(K::Louvres { spacing, .. }) => *spacing += step,
            Some(K::Bevel { radius, .. }) => *radius += step,
            Some(K::Shell { thickness, .. }) => *thickness += step,
            Some(K::Twist { angle }) => *angle += 1.0,
            Some(K::Array { count, .. }) | Some(K::RadialArray { count, .. }) => *count += 1,
            _ => f.origin.x += 0.1,
        }
    };
    measure("no tower (the starter house)", 0, &Vec::new, &moved);
    println!("one tower, one modifier, its setting changed:");
    measure("plain box, moved", 1, &Vec::new, &moved);
    measure("Slice by levels (14 floors)", 1, &|| vec![slice()], &nudge);
    measure("Panelize 1.2 x 3.0", 1, &|| vec![panels(1.2, 3.0)], &nudge);
    measure("Panelize 0.6 x 1.5", 1, &|| vec![panels(0.6, 1.5)], &nudge);
    measure("Louvres every 600", 1, &|| vec![louvres(0.6)], &nudge);
    measure("Louvres every 200", 1, &|| vec![louvres(0.2)], &nudge);
    measure(
        "Bevel r 2, 8 segments",
        1,
        &|| {
            vec![K::Bevel {
                radius: 2.0,
                segments: 8,
                chamfer: false,
                corners: Corners::All,
            }]
        },
        &nudge,
    );
    measure(
        "Shell 300",
        1,
        &|| {
            vec![K::Shell {
                thickness: 0.3,
                open: OpenFaces::Top,
            }]
        },
        &nudge,
    );
    measure(
        "Taper + Twist",
        1,
        &|| vec![K::Taper { x: 0.6, y: 0.6 }, K::Twist { angle: 45.0 }],
        &nudge,
    );
    measure(
        "Smooth, 4 levels",
        1,
        &|| {
            vec![K::Subdivision {
                levels: 4,
                simple: false,
            }]
        },
        &moved,
    );
    measure(
        "Array 10 x",
        1,
        &|| {
            vec![K::Array {
                count: 10,
                offset: DVec3::new(1.2, 0.0, 0.0),
                relative: true,
                fit_length: None,
                turn: 0.0,
                grow: 1.0,
            }]
        },
        &nudge,
    );
    measure(
        "Radial array 12",
        1,
        &|| {
            vec![K::RadialArray {
                count: 12,
                angle: 360.0,
                center: DVec2::new(40.0, 0.0),
                turn_copies: true,
            }]
        },
        &nudge,
    );
    measure(
        "Slice + Panelize + Louvres",
        1,
        &|| vec![slice(), panels(1.2, 3.0), louvres(0.6)],
        &nudge,
    );
    println!("many towers with Slice + Panelize, all moved:");
    let stack = || vec![slice(), panels(1.2, 3.0)];
    let twelve = measure("moved", 12, &stack, &moved);
    let many = measure("moved", 48, &stack, &moved);
    measure("moved", 96, &stack, &moved);
    println!("many towers with Slice + Panelize, every one changed:");
    measure("changed", 12, &stack, &nudge);
    measure("changed", 48, &stack, &nudge);
    // four times the towers, of one shape, are far from four times the
    // scene: their welding is kept (it was 100 ms against 300 ms)
    assert!(
        many[1] < twelve[1] * 2.5,
        "48 towers took {:.0} ms to build, 12 took {:.0} ms",
        many[1],
        twelve[1]
    );
}
