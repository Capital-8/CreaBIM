//! Pivot, double action and folding doors: in the front wall of a small
//! building a tall offset pivot front door, a bifold pair and a five
//! panel folding door; in an inside partition a centre pivot door, a
//! double action door and pair (a kitchen's), a bifold wardrobe door and
//! a three panel folding door. A sheet shows the plan, a detail of the
//! folding doors, the front elevation, the partition seen from the front
//! room, the door schedule and a 3D view.
//!
//! `cargo run -p creabim-store --example folding_demo -- folding.creabim`

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
    let out = std::env::args().nth(1).unwrap_or("folding.creabim".into());
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
        ("Front wall", v(0.0, 0.0), v(13.0, 0.0), 0.3),
        ("East wall", v(13.0, 0.0), v(13.0, 6.0), 0.3),
        ("Back wall", v(13.0, 6.0), v(0.0, 6.0), 0.3),
        ("West wall", v(0.0, 6.0), v(0.0, 0.0), 0.3),
        ("Partition", v(0.15, 3.4), v(12.85, 3.4), 0.15),
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
                v(13.15, -0.15),
                v(13.15, 6.15),
                v(-0.15, 6.15),
            ],
            0.2,
        )),
    ));
    doc.commit(tx).unwrap();
    let (front, partition) = (front.unwrap(), partition.unwrap());

    // the doors: (name, wall, offset, width, height, operation, panels)
    let doors = [
        (
            "Front door, offset pivot",
            front,
            0.8,
            1.4,
            2.4,
            Operation::Pivot,
            1,
        ),
        (
            "Bifold pair",
            front,
            3.0,
            3.2,
            2.1,
            Operation::BifoldPair,
            4,
        ),
        (
            "Folding door, five panels",
            front,
            7.4,
            4.2,
            2.1,
            Operation::Folding,
            5,
        ),
        (
            "Centre pivot",
            partition,
            0.9,
            1.0,
            2.1,
            Operation::CentrePivot,
            1,
        ),
        (
            "Kitchen door",
            partition,
            3.0,
            0.9,
            2.1,
            Operation::DoubleAction,
            1,
        ),
        (
            "Kitchen pair",
            partition,
            4.8,
            1.6,
            2.1,
            Operation::DoubleActionPair,
            2,
        ),
        (
            "Wardrobe bifold",
            partition,
            7.4,
            1.0,
            2.1,
            Operation::Bifold,
            2,
        ),
        (
            "Folding door, three panels",
            partition,
            9.6,
            1.8,
            2.1,
            Operation::Folding,
            3,
        ),
    ];
    let mut tx = Transaction::new("doors");
    for (name, host, offset, width, height, operation, panels) in doors {
        tx = tx.insert(Element::new(
            name,
            ElementKind::Opening(Opening {
                host,
                kind: OpeningKind::Door,
                offset,
                sill: 0.0,
                width,
                height,
                material: None,
                frame: {
                    let mut f = OpeningFrame::of_operation(OpeningKind::Door, operation);
                    if operation == Operation::Folding {
                        f.set_folding_panels(OpeningKind::Door, panels);
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
    tx = add(tx, "Folding doors, plan detail", detail);
    let mut elev = View::section(v(-1.0, -4.0), v(14.0, -4.0), 30.0, false);
    elev.scale = 50.0;
    tx = add(tx, "Front elevation", elev);
    // in the front room, looking north at the partition
    let mut sec = View::section(v(-0.6, 2.4), v(13.6, 2.4), 2.0, true);
    sec.scale = 50.0;
    tx = add(tx, "Section A, the partition from the front room", sec);
    tx = add(tx, "Door Schedule", View::schedule(ScheduleCategory::Doors));
    let mut axo = View::axonometric(200.0, 30.0);
    axo.scale = 100.0;
    tx = add(tx, "3D", axo);
    doc.commit(tx).unwrap();

    // one A1 sheet: the plan, elevation and section down the left, the
    // detail, schedule and 3D on the right
    let sheet = Element::new(
        "Pivot, double action and folding doors",
        ElementKind::Sheet(Sheet::a("A207", "A1")),
    );
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
        // the plan cropped to the building, the detail to the folding
        // doors in the front wall
        let crop = match k {
            0 => Some([v(-1.0, -1.0), v(14.0, 7.0)]),
            1 => Some([v(7.1, -0.5), v(11.9, 1.7)]),
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
