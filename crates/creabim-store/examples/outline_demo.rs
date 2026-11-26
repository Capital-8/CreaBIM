//! Doors and windows with arched and raked heads: a pair of doors under
//! a fanlight in a round arch, a door under a segmental arch, a
//! round-headed window, a casement pair under a fanlight in a segmental
//! arch, and a window under a raked head, along the front wall of a small
//! building. A sheet shows the plan, the front elevation from outside and
//! from inside, a section through the entrance, the door and window
//! schedules and a 3D view.
//!
//! `cargo run -p creabim-store --example outline_demo -- outlines.creabim`

use creabim_core::cells::{Assembly, Cell, Mechanism, Outline, WindowStyle};
use creabim_core::{
    Document, Element, ElementKind, Level, Opening, OpeningFrame, OpeningKind, Operation,
    ScheduleCategory, Sheet, Slab, Transaction, View, Viewport, Wall,
};
use creabim_geom::section::view_drawing;
use glam::DVec2;

fn v(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}

/// A frame of `kind` whose core is `core`, with a fixed light over it of
/// `transom` (a share of the height) or none, in `outline`.
fn headed(
    kind: OpeningKind,
    core: creabim_core::cells::Grid,
    transom: Option<f64>,
    outline: Outline,
) -> OpeningFrame {
    let mut f = OpeningFrame::for_kind(kind);
    f.grid = Assembly {
        transom: transom.map(|s| (s, Cell::new(Mechanism::Fixed))),
        outline,
        ..Assembly::of(core)
    }
    .grid();
    f
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or("outlines.creabim".into());
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
    // and a floor
    let mut tx = Transaction::new("walls");
    let mut front = None;
    for (name, a, b, t) in [
        ("Front wall", v(0.0, 0.0), v(13.0, 0.0), 0.3),
        ("East wall", v(13.0, 0.0), v(13.0, 6.0), 0.3),
        ("Back wall", v(13.0, 6.0), v(0.0, 6.0), 0.3),
        ("West wall", v(0.0, 6.0), v(0.0, 0.0), 0.3),
    ] {
        let e = Element::new(name, ElementKind::Wall(Wall::new(g, a, b, t, 3.0)));
        if name == "Front wall" {
            front = Some(e.id);
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
    let front = front.unwrap();

    let door = OpeningKind::Door;
    let window = OpeningKind::Window;
    let pair = OpeningFrame::of_operation(door, Operation::DoubleSwing).grid;
    let single = OpeningFrame::of_operation(door, Operation::Standard).grid;
    // (name, kind, offset, sill, width, height, frame)
    let openings = [
        (
            "Entrance, pair under a round arch",
            door,
            1.0,
            0.0,
            1.6,
            2.9,
            headed(door, pair, Some(0.31), Outline::Round),
        ),
        (
            "Door under a segmental arch",
            door,
            3.8,
            0.0,
            1.0,
            2.5,
            headed(door, single, Some(0.16), Outline::Segmental { rise: 0.2 }),
        ),
        (
            "Round-headed window",
            window,
            6.0,
            0.8,
            0.9,
            1.9,
            headed(
                window,
                WindowStyle::Fixed.grid(false, [1, 1]),
                None,
                Outline::Round,
            ),
        ),
        (
            "Casement pair under a segmental arch",
            window,
            8.0,
            0.8,
            1.4,
            1.7,
            headed(
                window,
                WindowStyle::CasementPair.grid(false, [1, 1]),
                Some(0.25),
                Outline::Segmental { rise: 0.25 },
            ),
        ),
        (
            "Window under a raked head",
            window,
            10.6,
            0.8,
            1.4,
            1.6,
            headed(
                window,
                WindowStyle::Fixed.grid(false, [1, 1]),
                None,
                Outline::Raked { drop: 0.5 },
            ),
        ),
    ];
    let mut tx = Transaction::new("doors and windows");
    for (name, kind, offset, sill, width, height, frame) in openings {
        tx = tx.insert(Element::new(
            name,
            ElementKind::Opening(Opening {
                host: front,
                kind,
                offset,
                sill,
                width,
                height,
                material: None,
                frame,
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
    let mut elev = View::section(v(-1.0, -4.0), v(14.0, -4.0), 30.0, false);
    elev.scale = 50.0;
    tx = add(tx, "Front elevation", elev);
    // inside, looking back at the front wall
    let mut sec = View::section(v(13.6, 2.0), v(-0.6, 2.0), 2.5, true);
    sec.scale = 50.0;
    tx = add(tx, "Section A", sec);
    // up through the entrance's active leaf, looking west
    let mut sec = View::section(v(1.6, -1.6), v(1.6, 2.0), 5.0, true);
    sec.scale = 20.0;
    tx = add(tx, "Section B", sec);
    tx = add(tx, "Door Schedule", View::schedule(ScheduleCategory::Doors));
    tx = add(
        tx,
        "Window Schedule",
        View::schedule(ScheduleCategory::Windows),
    );
    let mut axo = View::axonometric(200.0, 30.0);
    axo.scale = 100.0;
    tx = add(tx, "3D", axo);
    doc.commit(tx).unwrap();

    // one A1 sheet
    let sheet = Element::new(
        "Arched and raked heads",
        ElementKind::Sheet(Sheet::a("A205", "A1")),
    );
    let sid = sheet.id;
    let mut tx = Transaction::new("sheet").insert(sheet);
    let at = [
        v(200.0, 470.0),
        v(200.0, 320.0),
        v(200.0, 190.0),
        v(520.0, 450.0),
        v(610.0, 330.0),
        v(610.0, 270.0),
        v(600.0, 150.0),
    ];
    for (k, (id, c)) in views.iter().zip(at).enumerate() {
        let d = view_drawing(doc.model(), *id).expect("drawn");
        let (lo, hi) = d.bounds().unwrap_or((DVec2::ZERO, DVec2::ZERO));
        let crop = (k == 0).then_some([v(-1.0, -1.0), v(14.0, 7.0)]);
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
