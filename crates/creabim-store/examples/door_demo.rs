//! Doors, each way they can be hung: a single door and a row of double
//! doors in the front wall of a small building, with equal leaves,
//! unequal leaves (the active leaf two thirds of the width), a pair
//! opening out and a pair with its active leaf on the other jamb; and a
//! pair in an inside partition. A sheet shows the plan, the front
//! elevation with each leaf's opening triangle, a section through the
//! unequal pair, a section looking at the front wall from inside and the
//! door schedule.
//!
//! `cargo run -p creabim-store --example door_demo -- doors.creabim`

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
    let out = std::env::args().nth(1).unwrap_or("doors.creabim".into());
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
    // a partition across the middle and a floor
    let mut tx = Transaction::new("walls");
    let mut front = None;
    let mut partition = None;
    for (name, a, b, t) in [
        ("Front wall", v(0.0, 0.0), v(12.0, 0.0), 0.3),
        ("East wall", v(12.0, 0.0), v(12.0, 6.0), 0.3),
        ("Back wall", v(12.0, 6.0), v(0.0, 6.0), 0.3),
        ("West wall", v(0.0, 6.0), v(0.0, 0.0), 0.3),
        ("Partition", v(0.15, 3.4), v(11.85, 3.4), 0.12),
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

    // the doors: (name, wall, offset, width, operation, active share, out, other hand)
    let doors = [
        (
            "Single door",
            front,
            1.0,
            0.9,
            Operation::Standard,
            0.5,
            false,
            false,
        ),
        (
            "Double door, equal leaves",
            front,
            2.7,
            1.6,
            Operation::DoubleSwing,
            0.5,
            false,
            false,
        ),
        (
            "Double door, 1000 + 500",
            front,
            5.1,
            1.62,
            Operation::DoubleSwing,
            0.67,
            false,
            false,
        ),
        (
            "Double door opening out",
            front,
            7.6,
            1.8,
            Operation::DoubleSwing,
            0.5,
            true,
            false,
        ),
        (
            "Double door, other hand",
            front,
            10.0,
            1.5,
            Operation::DoubleSwing,
            0.6,
            false,
            true,
        ),
        (
            "Inner double door",
            partition,
            4.6,
            1.4,
            Operation::DoubleSwing,
            0.5,
            false,
            false,
        ),
    ];
    let mut tx = Transaction::new("doors");
    for (name, host, offset, width, operation, share, out, hand) in doors {
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
                    f.set_active_share(share);
                    f
                },
                flip_hand: hand,
                flip_facing: out,
                roof_at: None,
                roof_build: Default::default(),
                arrangement: None,
            }),
        ));
    }
    doc.commit(tx).unwrap();

    // the views: the plan, the front elevation, sections through two
    // pairs, the door schedule and a 3D view
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
    let mut elev = View::section(v(-1.0, -4.0), v(13.0, -4.0), 30.0, false);
    elev.scale = 50.0;
    tx = add(tx, "Front elevation", elev);
    // up through the unequal pair, looking west
    let mut sec = View::section(v(5.6, -1.6), v(5.6, 4.6), 5.0, true);
    sec.scale = 25.0;
    tx = add(tx, "Section A, through the 1000 + 500 pair", sec);
    // inside, looking back at the front wall: the leaves that open
    // towards the viewer now, their triangles solid
    let mut sec = View::section(v(12.6, 1.6), v(-0.6, 1.6), 2.0, true);
    sec.scale = 50.0;
    tx = add(tx, "Section B, inside looking at the front wall", sec);
    tx = add(tx, "Door Schedule", View::schedule(ScheduleCategory::Doors));
    let mut axo = View::axonometric(200.0, 30.0);
    axo.scale = 100.0;
    tx = add(tx, "3D", axo);
    doc.commit(tx).unwrap();

    // one A1 sheet: the plan and elevation down the left, the sections,
    // 3D and schedule on the right
    let sheet = Element::new("Doors", ElementKind::Sheet(Sheet::a("A201", "A1")));
    let sid = sheet.id;
    let mut tx = Transaction::new("sheet").insert(sheet);
    // where each goes (mm on the sheet), and the plan cropped to the
    // building and its section marks
    let at = [
        v(185.0, 455.0),
        v(185.0, 285.0),
        v(575.0, 470.0),
        v(575.0, 315.0),
        v(560.0, 150.0),
        v(185.0, 130.0),
    ];
    for (k, (id, c)) in views.iter().zip(at).enumerate() {
        let d = view_drawing(doc.model(), *id).expect("drawn");
        let (lo, hi) = d.bounds().unwrap_or((DVec2::ZERO, DVec2::ZERO));
        let crop = (k == 0).then_some([v(-1.0, -2.3), v(13.0, 7.0)]);
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
