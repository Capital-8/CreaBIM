//! While a drag is live the ground waits: it is the one last worked out,
//! and is worked out again on the drop. This file is its own test
//! program, so the switch it turns ([`creabim_geom::earth::hold`])
//! touches no other test.

use creabim_core::{
    Document, Element, ElementKind, Level, Model, Site, SiteForm, Transaction, Xform,
};
use glam::{DVec2, DVec3};

fn terrain(m: &Model) -> &Site {
    m.iter()
        .find_map(|e| match &e.kind {
            ElementKind::Site(s) if matches!(s.form, SiteForm::Terrain { .. }) => Some(s),
            _ => None,
        })
        .unwrap()
}

#[test]
fn the_ground_waits_while_a_pit_is_dragged_and_follows_on_the_drop() {
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
    let ground = Element::new(
        "Terrain",
        ElementKind::Site(Site::terrain(lv.id, sq(DVec2::ZERO, 20.0), 0.0, 1.0)),
    );
    let pit = Element::new(
        "Pit",
        ElementKind::Site(Site::pit(lv.id, sq(DVec2::new(-8.0, 0.0), 2.0), 1.0, 0.5)),
    );
    let id = pit.id;
    let mut doc = Document::from_model(Model::from_elements([lv, ground, pit]).unwrap());
    let at = |m: &Model, p: DVec2| {
        creabim_geom::earth::shaped(m, terrain(m))
            .unwrap()
            .ground
            .height(p)
            .unwrap()
    };
    let (west, east) = (DVec2::new(-8.0, 0.0), DVec2::new(8.0, 0.0));
    assert!((at(doc.model(), west) + 1.0).abs() < 1e-6);
    let own = creabim_geom::earth::work_alone(doc.model(), terrain(doc.model()), id).unwrap();
    assert!(own.cut > 16.0 && own.fill < 1e-9, "{own:?}");

    // the drag starts, and the pit is taken 16 m east
    assert!(!creabim_geom::earth::hold(true));
    let before = doc.model().get(id).unwrap().clone();
    let after = creabim_core::transform::transformed(
        doc.model(),
        &before,
        &Xform::Move(DVec3::new(16.0, 0.0, 0.0)),
    )
    .unwrap();
    doc.commit(Transaction::new("Move").replace(before, after))
        .unwrap();
    // the ground has not moved yet, nor have its volumes
    assert!((at(doc.model(), west) + 1.0).abs() < 1e-6);
    assert!(at(doc.model(), east).abs() < 1e-6);
    assert_eq!(
        creabim_geom::earth::work_alone(doc.model(), terrain(doc.model()), id),
        Some(own)
    );
    // the drop: something was held, so it is all to be drawn again
    assert!(creabim_geom::earth::hold(false));
    assert!(at(doc.model(), west).abs() < 1e-6);
    assert!((at(doc.model(), east) + 1.0).abs() < 1e-6);
    // with nothing held since, turning it off again asks for nothing
    assert!(!creabim_geom::earth::hold(true));
    assert!(!creabim_geom::earth::hold(false));
}
