//! Earthworks are elements worked into the terrain under them, and a
//! project from before that opens the same: works a terrain kept in its
//! own list become elements, and a pad element from before pits and
//! trenches is worked in where it always was.

use creabim_core::{Document, Element, ElementKind, Level, Model, Pad, Site, SiteForm};
use glam::DVec2;

fn level() -> Element {
    Element::new(
        "Ground Floor",
        ElementKind::Level(Level {
            elevation: 0.0,
            height: 3.0,
        }),
    )
}

fn sq(c: DVec2, h: f64) -> Vec<DVec2> {
    creabim_core::polygon::rectangle(c - DVec2::splat(h), c + DVec2::splat(h))
}

fn terrain(m: &Model) -> &Site {
    m.iter()
        .find_map(|e| match &e.kind {
            ElementKind::Site(s) if matches!(s.form, SiteForm::Terrain { .. }) => Some(s),
            _ => None,
        })
        .unwrap()
}

/// A terrain that keeps a pad and a pit in its own list, as files did.
fn earlier() -> Model {
    let lv = level();
    let mut t = Site::terrain(lv.id, sq(DVec2::ZERO, 20.0), 0.0, 1.0);
    if let SiteForm::Terrain { data, .. } = &mut t.form {
        data.pads
            .push(Pad::new(sq(DVec2::new(-8.0, 0.0), 4.0), 0.85));
        data.pads
            .push(Pad::dig(sq(DVec2::new(8.0, 0.0), 2.0), 1.2, 0.5));
    }
    Model::from_elements([lv, Element::new("Terrain", ElementKind::Site(t))]).unwrap()
}

#[test]
fn the_ground_is_the_same_once_its_works_are_elements() {
    let m = earlier();
    let before = creabim_geom::earth::shaped(&m, terrain(&m)).unwrap();
    let mut doc = Document::from_model(m);
    doc.commit(creabim_core::site::earlier_pads(doc.model()).expect("works to convert"))
        .unwrap();
    let m = doc.model();
    assert_eq!(
        m.iter()
            .filter(|e| matches!(&e.kind, ElementKind::Site(s) if s.work_kind().is_some()))
            .count(),
        2
    );
    let after = creabim_geom::earth::shaped(m, terrain(m)).unwrap();
    assert!(after.changed());
    for i in 0..=40 {
        for j in 0..=40 {
            let p = DVec2::new(-20.0 + i as f64, -20.0 + j as f64);
            let (a, b) = (before.ground.height(p), after.ground.height(p));
            assert!(
                a.zip(b).is_some_and(|(a, b)| (a - b).abs() < 1e-9),
                "{a:?} {b:?} at {p}"
            );
        }
    }
    assert!((before.cut - after.cut).abs() < 1e-6 && (before.fill - after.fill).abs() < 1e-6);
    // on the pad, in the pit, and away from both
    assert!((after.ground.height(DVec2::new(-8.0, 0.0)).unwrap() - 0.85).abs() < 1e-6);
    assert!((after.ground.height(DVec2::new(8.0, 0.0)).unwrap() + 1.2).abs() < 1e-6);
    assert!(after.ground.height(DVec2::new(0.0, 15.0)).unwrap().abs() < 1e-6);
}

#[test]
fn a_pad_element_from_an_earlier_file_levels_the_ground_where_it_did() {
    let lv = level();
    let ground = Element::new(
        "Terrain",
        ElementKind::Site(Site::terrain(lv.id, sq(DVec2::ZERO, 20.0), 0.0, 1.0)),
    );
    // as an earlier file holds it: 1 m up on a 150 mm base course, banks
    // of 1 in 1.5 dug and 1 in 2 made up, and none of the later fields
    let old = format!(
        r#"{{"level":{},"form":{{"Pad":{{"boundary":[[-5.0,-5.0],[5.0,-5.0],[5.0,5.0],[-5.0,5.0]],"top":1.0,"base":0.15,"cut":1.5,"fill":2.0}}}}}}"#,
        serde_json::to_string(&lv.id).unwrap()
    );
    let pad: Site = serde_json::from_str(&old).expect("an earlier pad still reads");
    let pad = Element::new("Pad", ElementKind::Site(pad));
    let m = Model::from_elements([lv, ground, pad]).unwrap();
    // nothing to convert: it is an earthwork as it stands
    assert!(creabim_core::site::earlier_pads(&m).is_none());
    let s = creabim_geom::earth::shaped(&m, terrain(&m)).expect("the ground as designed");
    assert!(s.changed());
    // inside it: the underside of the base course
    let z = s.ground.height(DVec2::ZERO).unwrap();
    assert!((z - 0.85).abs() < 1e-6, "{z}");
    // its made-up bank falls 1 in 2 from its edge
    let bank = s.ground.height(DVec2::new(6.0, 0.0)).unwrap();
    assert!((bank - 0.35).abs() < 0.05, "{bank}");
    assert!(s.ground.height(DVec2::new(15.0, 0.0)).unwrap().abs() < 1e-6);
    assert!(s.fill > 80.0 && s.cut < 1e-6, "{} {}", s.fill, s.cut);
}

#[test]
fn an_earthwork_moved_takes_the_ground_with_it() {
    let lv = level();
    let ground = Element::new(
        "Terrain",
        ElementKind::Site(Site::terrain(lv.id, sq(DVec2::ZERO, 20.0), 0.0, 1.0)),
    );
    let pit = Element::new(
        "Pit",
        ElementKind::Site(Site::pit(lv.id, sq(DVec2::new(-8.0, 0.0), 2.0), 1.0, 0.5)),
    );
    let trench = Element::new(
        "Trench",
        ElementKind::Site(Site::trench(
            lv.id,
            vec![DVec2::new(0.0, 10.0), DVec2::new(10.0, 10.0)],
            0.6,
            0.8,
        )),
    );
    let id = pit.id;
    let mut doc = Document::from_model(Model::from_elements([lv, ground, pit, trench]).unwrap());
    let at = |m: &Model, p: DVec2| {
        creabim_geom::earth::shaped(m, terrain(m))
            .unwrap()
            .ground
            .height(p)
            .unwrap()
    };
    assert!((at(doc.model(), DVec2::new(-8.0, 0.0)) + 1.0).abs() < 1e-6);
    assert!((at(doc.model(), DVec2::new(5.0, 10.0)) + 0.8).abs() < 1e-6);
    // moved 16 m east, as Move does to any element
    let before = doc.model().get(id).unwrap().clone();
    let after = creabim_core::transform::transformed(
        doc.model(),
        &before,
        &creabim_core::Xform::Move(glam::DVec3::new(16.0, 0.0, 0.0)),
    )
    .expect("an earthwork moves");
    doc.commit(creabim_core::Transaction::new("Move").replace(before, after))
        .unwrap();
    assert!(at(doc.model(), DVec2::new(-8.0, 0.0)).abs() < 1e-6);
    assert!((at(doc.model(), DVec2::new(8.0, 0.0)) + 1.0).abs() < 1e-6);
    // deleted, the ground closes
    doc.commit(creabim_core::Transaction::new("Delete").remove_cascade(doc.model(), id))
        .unwrap();
    assert!(at(doc.model(), DVec2::new(8.0, 0.0)).abs() < 1e-6);
}
