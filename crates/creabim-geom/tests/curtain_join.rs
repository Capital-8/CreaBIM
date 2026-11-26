//! A wall made a curtain wall in the starter house (1.1.0: the app froze,
//! its memory past 8 GB, as join cuts multiplied a slab's triangles).

use creabim_core::*;

#[test]
fn a_starter_wall_made_a_curtain_wall_builds_without_runaway_cuts() {
    let mut doc = Document::from_model(starter::starter_house());
    let id = doc
        .model()
        .iter()
        .find(|e| e.name == "GF North")
        .map(|e| e.id)
        .expect("the starter house's GF North");
    // as the Curtain wall switch in Properties does it
    doc.edit(id, "Curtain wall", |e| {
        if let ElementKind::Wall(w) = &mut e.kind {
            let c = CurtainGrid {
                mullion_depth: w.thickness.clamp(0.05, 0.3),
                ..Default::default()
            };
            w.thickness = c.mullion_depth;
            w.curtain = Some(c);
            w.assembly = None;
        }
    })
    .unwrap();
    let model = doc.model().clone();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let scene = creabim_geom::scene::build_scene(&model);
        let _ = tx.send(scene.iter().map(|i| i.mesh.indices.len() / 3).max());
    });
    let largest = rx
        .recv_timeout(std::time::Duration::from_secs(120))
        .expect("the scene builds")
        .expect("a scene");
    assert!(largest <= 60_000, "a piece of {largest} triangles");
}
