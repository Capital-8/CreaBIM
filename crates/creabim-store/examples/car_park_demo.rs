//! A shop's car park to try the car park tool on: a 47 × 32 m hard
//! landscape area that lays its own bays, a drive meeting one of its
//! cross aisles (its entrance), a path meeting its far edge (the door),
//! and its bay schedule (Mockup_Site_Car_Park REV02, P1).
//!
//! `cargo run -p creabim-store --example car_park_demo -- out.creabim`

use creabim_core::carpark::{CarPark, Requirement, StripUse};
use creabim_core::{Document, Element, ElementKind, Level, Site, SiteForm, Transaction};
use glam::DVec2;

fn main() {
    let out = std::env::args().nth(1).expect("the project to write");
    let lv = Element::new(
        "Ground Floor",
        ElementKind::Level(Level {
            elevation: 0.0,
            height: 3.0,
        }),
    );
    let g = lv.id;
    let rect = |a: DVec2, b: DVec2| creabim_core::polygon::rectangle(a, b);
    let named = |name: &str, s: Site| Element::new(name, ElementKind::Site(s));
    let mut tx = Transaction::new("Car park demo").insert(lv);
    tx = tx.insert(named(
        "Ground",
        Site::terrain(
            g,
            rect(DVec2::new(-15.0, -20.0), DVec2::new(62.0, 55.0)),
            0.0,
            2.0,
        ),
    ));
    let mut park = Site::area(
        g,
        rect(DVec2::ZERO, DVec2::new(47.0, 32.0)),
        0.15,
        creabim_core::site::Pattern::None,
    );
    if let SiteForm::Area { top, parking, .. } = &mut park.form {
        *top = 0.0;
        let mut cp = CarPark {
            accessible: 2,
            ev: 4,
            accessible_per: 30,
            required: Requirement::PerFloor {
                floor: 1500.0,
                per: 30.0,
            },
            ..CarPark::default()
        };
        // a walkway along the far edge, by the path to the door, and
        // planting down the east side
        cp.set_strip(2, Some((StripUse::Walkway, 2.0)));
        cp.set_strip(1, Some((StripUse::Planting, 1.5)));
        *parking = Some(cp);
    }
    tx = tx.insert(named("Shop car park", park));
    // the drive in from the street, to the end of a cross aisle
    tx = tx.insert(named(
        "Drive",
        Site::road(g, vec![DVec2::new(3.0, -14.0), DVec2::new(3.0, 0.0)], 6.0),
    ));
    // the path to the shop's door, from the car park's far edge
    tx = tx.insert(named(
        "Path to the door",
        Site::walk(
            g,
            vec![DVec2::new(11.0, 32.0), DVec2::new(11.0, 38.0)],
            creabim_core::walk::Surface::Paving,
        ),
    ));
    // a tree kept in the far row: the bays step round its trunk
    let shape = creabim_core::tree::TreeShape {
        seed: 7,
        ..creabim_core::tree::TreePreset::Shade.shape().0
    };
    tx = tx.insert(named(
        "Kept tree",
        Site::tree(g, DVec2::new(34.5, 26.5), shape, 4.0, 6.0),
    ));
    // the four simple plants in a row behind it, and a section along them
    for (name, form, x, canopy, height) in [
        ("Tree", creabim_core::PlantForm::Tree, 8.0, 5.0, 7.0),
        ("Conifer", creabim_core::PlantForm::Conifer, 17.0, 4.0, 8.0),
        ("Palm", creabim_core::PlantForm::Palm, 26.0, 4.5, 7.0),
        ("Shrub", creabim_core::PlantForm::Shrub, 33.0, 2.4, 1.4),
    ] {
        tx = tx.insert(named(
            name,
            Site::plant(g, DVec2::new(x, 47.0), form, canopy, height),
        ));
    }
    let mut section =
        creabim_core::View::section(DVec2::new(2.0, 44.0), DVec2::new(38.0, 44.0), 12.0, true);
    section.scale = 100.0;
    tx = tx.insert(Element::new("Plants section", ElementKind::View(section)));
    // one through the tree's trunk, to see it cut
    let mut cut =
        creabim_core::View::section(DVec2::new(2.0, 47.0), DVec2::new(38.0, 47.0), 12.0, true);
    cut.scale = 100.0;
    tx = tx.insert(Element::new("Plants cut", ElementKind::View(cut)));
    tx = tx.insert(Element::new(
        creabim_core::ScheduleCategory::Parking.schedule_name(),
        ElementKind::View(creabim_core::View::schedule(
            creabim_core::ScheduleCategory::Parking,
        )),
    ));
    let mut doc = Document::from_model(creabim_core::Model::new());
    doc.commit(tx).unwrap();
    let model = doc.model();
    for e in model.iter() {
        if let ElementKind::Site(s) = &e.kind {
            if let Some(lay) = s.car_park_layout(model) {
                println!(
                    "{}: {} rows, {} car bays, notes {:?}",
                    e.name,
                    lay.rows,
                    lay.cars(),
                    lay.notes
                );
            }
        }
    }
    creabim_store::save(&out, &doc).unwrap();
}
