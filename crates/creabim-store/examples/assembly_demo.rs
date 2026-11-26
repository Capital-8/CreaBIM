//! Doors and windows with lights round them: an entrance of a sidelight
//! and an unequal pair under a transom light, a door between two
//! sidelights, a door under an awning transom, a casement pair between
//! sidelights under an awning transom, and a casement under a fixed
//! transom, along the front wall of a small building. A sheet shows the
//! plan, the front elevation from outside and from inside, a section
//! through the entrance, the door and window schedules and a 3D view.
//!
//! `cargo run -p creabim-store --example assembly_demo -- assemblies.creabim`

use creabim_core::cells::{Assembly, Cell, Mechanism, WindowStyle};
use creabim_core::{
    Document, Element, ElementKind, Level, Opening, OpeningFrame, OpeningKind, Operation,
    ScheduleCategory, Sheet, Slab, Transaction, View, Viewport, Wall,
};
use creabim_geom::section::view_drawing;
use glam::DVec2;

fn v(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}

/// A frame of `kind` whose core is `core`, with sidelights of `side`
/// (a share of the width) at the start, end or both, and a transom light
/// of `transom` (a share of the height) over it.
fn assembly(
    kind: OpeningKind,
    core: creabim_core::cells::Grid,
    side: (Option<f64>, Option<f64>),
    transom: Option<(f64, Mechanism)>,
) -> OpeningFrame {
    let mut f = OpeningFrame::for_kind(kind);
    f.grid = Assembly {
        left: side.0,
        right: side.1,
        transom: transom.map(|(s, m)| (s, Cell::new(m))),
        ..Assembly::of(core)
    }
    .grid();
    f
}

fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or("assemblies.creabim".into());
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
    // the entrance's pair: about 1000 active and 500
    let mut pair = OpeningFrame::of_operation(door, Operation::DoubleSwing);
    pair.set_active_share(0.667);
    let single = OpeningFrame::of_operation(door, Operation::Standard).grid;
    // (name, kind, offset, sill, width, height, frame)
    let openings = [
        (
            "Entrance, sidelight, pair and transom",
            door,
            0.8,
            0.0,
            2.1,
            2.7,
            assembly(
                door,
                pair.grid,
                (Some(0.24), None),
                Some((0.2, Mechanism::Fixed)),
            ),
        ),
        (
            "Door between sidelights",
            door,
            3.7,
            0.0,
            1.9,
            2.1,
            assembly(door, single.clone(), (Some(0.25), Some(0.25)), None),
        ),
        (
            "Door under an awning transom",
            door,
            6.4,
            0.0,
            1.0,
            2.7,
            assembly(door, single, (None, None), Some((0.2, Mechanism::TopHung))),
        ),
        (
            "Casement pair, sidelights and awning transom",
            window,
            8.2,
            0.6,
            2.4,
            1.9,
            assembly(
                window,
                WindowStyle::CasementPair.grid(false, [1, 1]),
                (Some(0.22), Some(0.22)),
                Some((0.25, Mechanism::TopHung)),
            ),
        ),
        (
            "Casement under a fixed transom",
            window,
            11.4,
            0.6,
            0.9,
            1.9,
            assembly(
                window,
                WindowStyle::Casement.grid(false, [1, 1]),
                (None, None),
                Some((0.25, Mechanism::Fixed)),
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
    let mut sec = View::section(v(1.9, -1.6), v(1.9, 2.0), 5.0, true);
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
        "Sidelights and transoms",
        ElementKind::Sheet(Sheet::a("A204", "A1")),
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
