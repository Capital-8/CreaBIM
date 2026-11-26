//! Large and specialist doors and a guillotine window: a sectional
//! overhead door, a roller shutter and a revolving door in the front wall
//! of a small building, and a guillotine servery window in a partition
//! inside. A sheet shows the plan, the front elevation, the front wall
//! from inside, a section through the sectional door, the schedules and
//! a 3D view.
//!
//! `cargo run -p creabim-store --example specialist_demo -- specialist.creabim`

use creabim_core::cells::WindowStyle;
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
    let out = std::env::args()
        .nth(1)
        .unwrap_or("specialist.creabim".into());
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
        ("Partition", v(0.15, 4.0), v(12.85, 4.0), 0.15),
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

    // the doors: (name, wall, offset, width, height, operation)
    let doors = [
        (
            "Garage door, sectional",
            front,
            0.8,
            2.6,
            2.2,
            Operation::Sectional,
        ),
        ("Roller shutter", front, 4.6, 3.0, 2.6, Operation::Roller),
        ("Revolving door", front, 9.4, 2.0, 2.3, Operation::Revolving),
    ];
    let mut tx = Transaction::new("doors");
    for (name, host, offset, width, height, operation) in doors {
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
                frame: OpeningFrame::of_operation(OpeningKind::Door, operation),
                flip_hand: false,
                flip_facing: false,
                roof_at: None,
                roof_build: Default::default(),
                arrangement: None,
            }),
        ));
    }
    // a guillotine servery window in the partition
    let mut frame = OpeningFrame::for_kind(OpeningKind::Window);
    frame.grid = WindowStyle::Guillotine.grid(false, [1, 1]);
    tx = tx.insert(Element::new(
        "Servery, guillotine",
        ElementKind::Opening(Opening {
            host: partition,
            kind: OpeningKind::Window,
            offset: 5.5,
            sill: 0.9,
            width: 1.5,
            height: 0.7,
            material: None,
            frame,
            flip_hand: false,
            flip_facing: false,
            roof_at: None,
            roof_build: Default::default(),
            arrangement: None,
        }),
    ));
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
    // up through the sectional door, looking west: its panels and where
    // they lie open
    let mut sec = View::section(v(2.1, -1.5), v(2.1, 4.6), 5.0, true);
    sec.scale = 50.0;
    tx = add(tx, "Section B, through the sectional door", sec);
    let mut elev = View::section(v(-1.0, -4.0), v(14.0, -4.0), 30.0, false);
    elev.scale = 50.0;
    tx = add(tx, "Front elevation", elev);
    // in the front room, looking south at the front wall
    let mut sec = View::section(v(13.6, 2.0), v(-0.6, 2.0), 2.5, true);
    sec.scale = 50.0;
    tx = add(tx, "Section A, inside looking at the front wall", sec);
    tx = add(tx, "Door Schedule", View::schedule(ScheduleCategory::Doors));
    tx = add(
        tx,
        "Window Schedule",
        View::schedule(ScheduleCategory::Windows),
    );
    // and looking north at the partition: the guillotine hatch, and
    // where its sash rises to
    let mut sec = View::section(v(-0.6, 3.0), v(13.6, 3.0), 1.5, true);
    sec.scale = 50.0;
    tx = add(tx, "Section C, looking at the servery", sec);
    let mut axo = View::axonometric(200.0, 30.0);
    axo.scale = 100.0;
    tx = add(tx, "3D", axo);
    doc.commit(tx).unwrap();

    // one A1 sheet: the plan, elevation and section down the left, the
    // detail, schedule and 3D on the right
    let sheet = Element::new(
        "Large and specialist doors",
        ElementKind::Sheet(Sheet::a("A209", "A1")),
    );
    let sid = sheet.id;
    let mut tx = Transaction::new("sheet").insert(sheet);
    let at = [
        v(185.0, 455.0),
        v(580.0, 470.0),
        v(185.0, 285.0),
        v(185.0, 140.0),
        v(600.0, 360.0),
        v(600.0, 300.0),
        v(600.0, 225.0),
        v(600.0, 115.0),
    ];
    for (k, (id, c)) in views.iter().zip(at).enumerate() {
        let d = view_drawing(doc.model(), *id).expect("drawn");
        let (lo, hi) = d.bounds().unwrap_or((DVec2::ZERO, DVec2::ZERO));
        // the plan cropped to the building
        let crop = match k {
            0 => Some([v(-1.0, -1.0), v(14.0, 7.0)]),
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
