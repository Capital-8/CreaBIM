//! A car park end to end: its bays in plan, its markings in 3D and its
//! schedule, with a road for its entrance and a path for its door
//! (Mockup_Site_Car_Park REV02, P1).

use creabim_core::carpark::{CarPark, Requirement};
use creabim_core::{
    Document, Element, ElementKind, Level, ScheduleCategory, Site, SiteForm, Transaction,
};
use glam::DVec2;

fn model() -> (creabim_core::Model, creabim_core::ElementId) {
    let lv = Element::new(
        "Ground Floor",
        ElementKind::Level(Level {
            elevation: 0.0,
            height: 3.0,
        }),
    );
    let g = lv.id;
    let mut park = Site::area(
        g,
        creabim_core::polygon::rectangle(DVec2::ZERO, DVec2::new(47.0, 32.0)),
        0.15,
        creabim_core::site::Pattern::None,
    );
    if let SiteForm::Area { parking, .. } = &mut park.form {
        *parking = Some(CarPark {
            accessible: 2,
            ev: 4,
            accessible_per: 30,
            required: Requirement::PerFloor {
                floor: 1500.0,
                per: 30.0,
            },
            ..CarPark::default()
        });
    }
    let park = Element::new("Shop car park", ElementKind::Site(park));
    let id = park.id;
    let ground = Element::new(
        "Ground",
        ElementKind::Site(Site::terrain(
            g,
            creabim_core::polygon::rectangle(DVec2::new(-15.0, -20.0), DVec2::new(62.0, 55.0)),
            0.0,
            2.0,
        )),
    );
    let tx = Transaction::new("car park")
        .insert(lv)
        .insert(ground)
        .insert(park)
        .insert(Element::new(
            "Drive",
            ElementKind::Site(Site::road(
                g,
                vec![DVec2::new(3.0, -14.0), DVec2::new(3.0, 0.0)],
                6.0,
            )),
        ))
        .insert(Element::new(
            "Path",
            ElementKind::Site(Site::walk(
                g,
                vec![DVec2::new(11.0, 32.0), DVec2::new(11.0, 38.0)],
                creabim_core::walk::Surface::Paving,
            )),
        ));
    let mut doc = Document::from_model(creabim_core::Model::new());
    doc.commit(tx).unwrap();
    (doc.model().clone(), id)
}

#[test]
fn a_car_park_counts_draws_and_schedules_its_bays() {
    let (m, id) = model();
    let ElementKind::Site(s) = &m.get(id).unwrap().kind else {
        panic!()
    };
    assert_eq!(s.label(), "Car park");
    let lay = s.car_park_layout(&m).unwrap();
    assert_eq!(lay.cars(), 58);
    assert!(lay.notes.is_empty(), "{:?}", lay.notes);
    // the accessible pair is by the path's end, on the far edge
    for b in lay
        .bays
        .iter()
        .filter(|b| b.kind == creabim_core::carpark::BayKind::Accessible)
    {
        assert!(b.middle().distance(DVec2::new(11.0, 32.0)) < 8.0);
    }
    // the ground under it is dug to its underside, so it never shows
    // through: 150 mm below its top inside, untouched well outside
    let inside = creabim_geom::earth::ground_at(&m, DVec2::new(20.0, 16.0)).unwrap();
    assert!((inside - (-0.15)).abs() < 0.02, "{inside}");
    let outside = creabim_geom::earth::ground_at(&m, DVec2::new(-10.0, -10.0)).unwrap();
    assert!(outside.abs() < 0.02, "{outside}");
    // 3D: its slab, its white lines, its blue accessible markings
    assert_eq!(creabim_geom::parts::site_meshes(&m, s).len(), 3);
    // the schedule: standard, EV, accessible, and its total against 50
    let sch = creabim_geom::schedule::schedule(&m, ScheduleCategory::Parking);
    assert_eq!(sch.rows.len(), 4);
    assert_eq!(sch.rows[2].cells[1], "Accessible");
    assert_eq!(sch.rows[2].cells[3], "A1–A2");
    assert_eq!(sch.rows[2].cells[5], "2");
    assert_eq!(sch.totals[4], "58");
    assert_eq!(sch.totals[5], "50");
}
