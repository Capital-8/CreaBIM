//! Sliding doors, each way they can run: multi-panel patio doors in the
//! front wall of a small building (four panels fixed, sliding, sliding,
//! fixed; two panels; three), and in an inside partition a surface
//! slider, a surface pair, a pocket door and a pocket pair. A sheet shows
//! the plan, a detail of the pocket doors in their cavities, the front
//! elevation, the partition seen from the front room, the door schedule
//! and a 3D view.
//!
//! `cargo run -p creabim-store --example sliding_demo -- sliding.creabim`

use creabim_core::{
    Document, Element, ElementKind, Level, Opening, OpeningFrame, OpeningKind, Operation,
    ScheduleCategory, Sheet, Slab, Transaction, View, Viewport, Wall,
};
use creabim_geom::section::view_drawing;
use glam::DVec2;

fn v(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or("sliding.creabim".into());
    let mut doc = Document::new();

    let ground = Element::new(
        "Ground Floor",
        ElementKind::Level(Level {
            elevation: 0.0,
            height: 3.0,
        }),
    );
    let g = ground.id;
    doc.commit(Transaction::new("levels").insert(ground))
        .unwrap();

    // four outside walls, anticlockwise so the outside is on their right,
    // a partition across the middle thick enough for pockets, and a floor
    let mut tx = Transaction::new("walls");
    let mut front = None;
    let mut partition = None;
    for (name, a, b, t) in [
        ("Front wall", v(0.0, 0.0), v(12.0, 0.0), 0.3),
        ("East wall", v(12.0, 0.0), v(12.0, 6.0), 0.3),
        ("Back wall", v(12.0, 6.0), v(0.0, 6.0), 0.3),
        ("West wall", v(0.0, 6.0), v(0.0, 0.0), 0.3),
        ("Partition", v(0.15, 3.4), v(11.85, 3.4), 0.15),
    ] {
        let e = Element::new(name, ElementKind::Wall(Wall::new(g, a, b, t, 3.0)));
        match name {
            "Front wall" => front = Some(e.id),
            "Partition" => partition = Some(e.id),
            _ => {}
        }
        tx = tx.insert(e);
    }
    tx = tx.insert(Element::new(
        "Floor",
        ElementKind::Slab(Slab::new(
            g,
            vec![
                v(-0.15, -0.15),
                v(12.15, -0.15),
                v(12.15, 6.15),
                v(-0.15, 6.15),
            ],
            0.2,
        )),
    ));
    doc.commit(tx).unwrap();
    let (front, partition) = (front.unwrap(), partition.unwrap());

    // the doors: (name, wall, offset, width, operation, panels, fixed)
    let doors = [
        (
            "Patio door, F S S F",
            front,
            0.9,
            3.6,
            Operation::MultiSlide,
            4,
            0b1001,
        ),
        (
            "Patio door, two panels",
            front,
            5.6,
            2.0,
            Operation::MultiSlide,
            2,
            0b01,
        ),
        (
            "Patio door, three panels",
            front,
            8.4,
            2.7,
            Operation::MultiSlide,
            3,
            0b100,
        ),
        (
            "Surface slider",
            partition,
            1.4,
            0.9,
            Operation::SurfaceSliding,
            2,
            1,
        ),
        (
            "Surface pair",
            partition,
            3.6,
            1.6,
            Operation::SurfaceSlidingPair,
            2,
            1,
        ),
        ("Pocket door", partition, 7.6, 0.9, Operation::Pocket, 2, 1),
        (
            "Pocket pair",
            partition,
            9.4,
            1.2,
            Operation::PocketPair,
            2,
            1,
        ),
    ];
    let mut tx = Transaction::new("doors");
    for (name, host, offset, width, operation, panels, fixed) in doors {
        tx = tx.insert(Element::new(
            name,
            ElementKind::Opening(Opening {
                host,
                kind: OpeningKind::Door,
                offset,
                sill: 0.0,
                width,
                height: 2.1,
                material: None,
                frame: {
                    let mut f = OpeningFrame::of_operation(OpeningKind::Door, operation);
                    if operation == Operation::MultiSlide {
                        f.set_panels(OpeningKind::Door, panels, fixed);
                    }
                    f
                },
                flip_hand: false,
                flip_facing: false,
                roof_at: None,
                roof_build: Default::default(),
                arrangement: None,
            }),
        ));
    }
    doc.commit(tx).unwrap();

    // the views
    let mut tx = Transaction::new("views");
    let mut views = vec![];
    let mut add = |tx: Transaction, name: &str, view: View| -> Transaction {
        let e = Element::new(name, ElementKind::View(view));
        views.push(e.id);
        tx.insert(e)
    };
    let mut plan = View::plan(g);
    plan.scale = 50.0;
    tx = add(tx, "Ground floor plan", plan);
    let mut detail = View::plan(g);
    detail.scale = 20.0;
    tx = add(tx, "Pocket doors, plan detail", detail);
    let mut elev = View::section(v(-1.0, -4.0), v(13.0, -4.0), 30.0, false);
    elev.scale = 50.0;
    tx = add(tx, "Front elevation", elev);
    // in the front room, looking north at the partition
    let mut sec = View::section(v(-0.6, 2.4), v(12.6, 2.4), 2.0, true);
    sec.scale = 50.0;
    tx = add(tx, "Section A, the partition from the front room", sec);
    tx = add(tx, "Door Schedule", View::schedule(ScheduleCategory::Doors));
    let mut axo = View::axonometric(200.0, 30.0);
    axo.scale = 100.0;
    tx = add(tx, "3D", axo);
    doc.commit(tx).unwrap();

    // one A1 sheet: the plan, elevation and section down the left, the
    // detail, schedule and 3D on the right
    let sheet = Element::new("Sliding doors", ElementKind::Sheet(Sheet::a("A202", "A1")));
    let sid = sheet.id;
    let mut tx = Transaction::new("sheet").insert(sheet);
    let at = [
        v(185.0, 455.0),
        v(600.0, 500.0),
        v(185.0, 285.0),
        v(185.0, 140.0),
        v(600.0, 380.0),
        v(600.0, 170.0),
    ];
    for (k, (id, c)) in views.iter().zip(at).enumerate() {
        let d = view_drawing(doc.model(), *id).expect("drawn");
        let (lo, hi) = d.bounds().unwrap_or((DVec2::ZERO, DVec2::ZERO));
        // the plan cropped to the building, the detail to the pockets
        let crop = match k {
            0 => Some([v(-1.0, -1.0), v(13.0, 7.0)]),
            1 => Some([v(6.7, 2.9), v(12.0, 3.9)]),
            _ => None,
        };
        let target = crop.map_or((lo + hi) / 2.0, |[a, b]| (a + b) / 2.0);
        tx = tx.insert(Element::new(
            "Viewport",
            ElementKind::Viewport(Viewport {
                sheet: sid,
                view: *id,
                center: c,
                target,
                crop,
                fit: None,
                pinned: false,
            }),
        ));
    }
    doc.commit(tx).unwrap();
    creabim_store::save(&out, &doc).unwrap();
    println!("saved {out}");
}
