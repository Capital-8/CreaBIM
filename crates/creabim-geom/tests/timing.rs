//! Stopwatch for the drawings and the 3D scene of Ridgeline (MU142 lag
//! report). Ignored: run with `--ignored --nocapture` in the fast profile.

use creabim_core::{ElementKind, ViewKind};
use std::time::Instant;

#[test]
#[ignore]
fn ridgeline_timings() {
    let m = creabim_core::ridgeline::ridgeline_house();
    // the 3D scene, three times (the first warms caches)
    for run in 0..3 {
        let t = Instant::now();
        let scene = creabim_geom::scene::build_scene(&m);
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        let t = Instant::now();
        let mut edges = 0;
        for it in &scene {
            edges += creabim_geom::section::feature_edges(it).len();
        }
        let ems = t.elapsed().as_secs_f64() * 1000.0;
        eprintln!(
            "scene run {run}: {} items, {ms:.0} ms; edges {edges} in {ems:.0} ms",
            scene.len()
        );
        // the bodies alone, before walls and slabs are cut into each other
        let t = Instant::now();
        let bare = creabim_geom::scene::build_scene_for_exchange(&m);
        eprintln!(
            "  unjoined: {} items in {:.0} ms",
            bare.len(),
            t.elapsed().as_secs_f64() * 1000.0
        );
        // each kind of element built alone (wall joins worked out each time)
        if run == 2 {
            let mut by_kind: std::collections::BTreeMap<String, (usize, f64)> = Default::default();
            let mut slow: Vec<(f64, String)> = vec![];
            for e in m.iter() {
                let only: std::collections::HashSet<_> = [e.id].into_iter().collect();
                let t = Instant::now();
                let s = creabim_geom::scene::build_scene_for_exchange_of(&m, &only);
                if s.is_empty() {
                    continue;
                }
                let k = by_kind.entry(kind_name(Some(&e.kind))).or_default();
                k.0 += 1;
                let ms = t.elapsed().as_secs_f64() * 1000.0;
                k.1 += ms;
                slow.push((ms, format!("{} ({})", e.name, kind_name(Some(&e.kind)))));
            }
            slow.sort_by(|a, b| b.0.total_cmp(&a.0));
            for (ms, n) in slow.iter().take(12) {
                eprintln!("  slowest {n}: {ms:.1} ms");
            }
            for (k, (n, ms)) in &by_kind {
                eprintln!("  alone {k}: {n} in {ms:.0} ms");
            }
        }
        if run == 2 {
            // what carries the triangles, by element kind and by element
            let mut by_kind: std::collections::BTreeMap<String, (usize, usize)> =
                Default::default();
            let mut by_el: Vec<(usize, String)> = vec![];
            for it in &scene {
                let n = it.mesh.indices.len() / 3;
                let e = m.get(it.id);
                let kind = e.map_or("?".into(), |e| {
                    format!("{:?}", std::mem::discriminant(&e.kind))
                });
                let name = e.map_or("?".into(), |e| e.name.clone());
                let k = by_kind.entry(kind_name(e.map(|e| &e.kind))).or_default();
                k.0 += 1;
                k.1 += n;
                let _ = kind;
                by_el.push((n, name));
            }
            let mut ks: Vec<_> = by_kind.into_iter().collect();
            ks.sort_by_key(|k| std::cmp::Reverse(k.1 .1));
            for (k, (c, n)) in ks.iter().take(10) {
                eprintln!("  kind {k}: {c} items, {n} triangles");
            }
            by_el.sort_by_key(|e| std::cmp::Reverse(e.0));
            for (n, name) in by_el.iter().take(10) {
                eprintln!("  element {name}: {n} triangles");
            }
        }
    }
    // as the app builds it: each time on a thread of its own
    let m2 = std::sync::Arc::new(m.clone());
    for run in 0..3 {
        let m2 = m2.clone();
        let ms = std::thread::spawn(move || {
            let t = Instant::now();
            let _ = creabim_geom::scene::build_scene(&m2);
            t.elapsed().as_secs_f64() * 1000.0
        })
        .join()
        .unwrap();
        eprintln!("scene on a new thread, run {run}: {ms:.0} ms");
    }
    // a drag in the Ground Floor plan: a chair turned a little each step,
    // the plan built again after each (the view worked in is built on the
    // spot, on the frame)
    {
        let mut doc = creabim_core::Document::from_model(m.clone());
        let plan = m.iter().find(|e| e.name == "Ground Floor plan").unwrap().id;
        let chair = m.iter().find(|e| e.name == "Dining chair").unwrap().id;
        for step in 0..4 {
            let before = doc.model().get(chair).unwrap().clone();
            let mut moved = before.clone();
            if let ElementKind::Object(o) = &mut moved.kind {
                o.rotation += 5.0;
            }
            doc.commit(creabim_core::Transaction::new("turn").replace(before, moved))
                .unwrap();
            let t = Instant::now();
            let d = creabim_geom::section::view_drawing(doc.model(), plan);
            eprintln!(
                "drag step {step}: plan in {:.0} ms ({} items)",
                t.elapsed().as_secs_f64() * 1000.0,
                d.map_or(0, |d| d.items.len())
            );
        }
    }
    // every plan and section drawing
    for e in m.iter() {
        let ElementKind::View(v) = &e.kind else {
            continue;
        };
        if !matches!(v.kind, ViewKind::Plan { .. } | ViewKind::Section { .. }) {
            continue;
        }
        let t = Instant::now();
        let d = creabim_geom::section::view_drawing(&m, e.id);
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        let t = Instant::now();
        let _ = creabim_geom::section::view_drawing(&m, e.id);
        let ms2 = t.elapsed().as_secs_f64() * 1000.0;
        eprintln!(
            "drawing {:<24} {ms:>7.0} ms, again {ms2:>7.0} ms, {} items",
            e.name,
            d.map_or(0, |d| d.items.len())
        );
    }
}

fn kind_name(k: Option<&ElementKind>) -> String {
    let Some(k) = k else { return "?".into() };
    let s = format!("{k:?}");
    s.split(|c: char| !c.is_alphanumeric())
        .next()
        .unwrap_or("?")
        .to_string()
}
