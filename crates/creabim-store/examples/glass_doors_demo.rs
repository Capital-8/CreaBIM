//! Glass doors along a shopfront: all glass doors in each of the three
//! fitting types (F, corner patches; BP, a bottom rail; A/P, top and
//! bottom rails), a pair of French doors and slim steel doors, single and
//! a pair. A sheet shows the plan, the front from outside, the front from
//! inside, the door schedule and a 3D view.
//!
//! `cargo run -p creabim-store --example glass_doors_demo -- glass_doors.creabim`

use creabim_core::construction::{Construction, GlassFittings, LeafStyle};
use creabim_core::{
    Document, Element, ElementKind, Level, Opening, OpeningFrame, OpeningKind, Operation,
    ScheduleCategory, Sheet, Slab, Transaction, View, Viewport, Wall,
};
use creabim_geom::section::view_drawing;
use glam::DVec2;

fn v(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}

/// A door frame opening as `op`, its leaves made as `leaf`, held by
/// `fittings` if all glass.
fn made(op: Operation, leaf: LeafStyle, fittings: GlassFittings) -> OpeningFrame {
    let mut f = OpeningFrame::of_operation(OpeningKind::Door, op);
    f.construction = Construction {
        leaf,
        fittings,
        ..Construction::default()
    };
    if leaf == LeafStyle::AllGlass {
        // a slim channel round frameless glass
        f.width = 0.03;
    }
    f
}

fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or("glass_doors.creabim".into());
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
    for (name, a, b) in [
        ("Front wall", v(0.0, 0.0), v(14.0, 0.0)),
        ("East wall", v(14.0, 0.0), v(14.0, 6.0)),
        ("Back wall", v(14.0, 6.0), v(0.0, 6.0)),
        ("West wall", v(0.0, 6.0), v(0.0, 0.0)),
    ] {
        let e = Element::new(name, ElementKind::Wall(Wall::new(g, a, b, 0.3, 3.0)));
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
                v(14.15, -0.15),
                v(14.15, 6.15),
                v(-0.15, 6.15),
            ],
            0.2,
        )),
    ));
    doc.commit(tx).unwrap();
    let front = front.unwrap();

    use GlassFittings as G;
    use LeafStyle as L;
    // (name, offset, width, frame)
    let doors = [
        (
            "All glass, F",
            0.6,
            0.9,
            made(Operation::DoubleAction, L::AllGlass, G::Patch),
        ),
        (
            "All glass, BP",
            2.3,
            0.9,
            made(Operation::Standard, L::AllGlass, G::BottomRail),
        ),
        (
            "All glass pair, A/P",
            4.0,
            1.8,
            made(Operation::DoubleActionPair, L::AllGlass, G::Rails),
        ),
        (
            "French doors",
            6.6,
            1.5,
            made(Operation::DoubleSwing, L::French, G::Patch),
        ),
        (
            "Slim steel",
            8.9,
            0.9,
            made(Operation::Standard, L::SlimSteel, G::Patch),
        ),
        (
            "Slim steel pair",
            10.6,
            1.6,
            made(Operation::DoubleSwing, L::SlimSteel, G::Patch),
        ),
    ];
    let mut tx = Transaction::new("doors");
    for (name, offset, width, frame) in doors {
        tx = tx.insert(Element::new(
            name,
            ElementKind::Opening(Opening {
                host: front,
                kind: OpeningKind::Door,
                offset,
                sill: 0.0,
                width,
                height: 2.2,
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
    // the front from outside, and from inside looking back at it
    let mut elev = View::section(v(-0.6, -3.0), v(14.6, -3.0), 30.0, false);
    elev.scale = 50.0;
    tx = add(tx, "Front elevation", elev);
    let mut sec = View::section(v(14.6, 2.0), v(-0.6, 2.0), 2.5, true);
    sec.scale = 50.0;
    tx = add(tx, "Inside, looking at the front", sec);
    tx = add(tx, "Door Schedule", View::schedule(ScheduleCategory::Doors));
    let mut axo = View::axonometric(25.0, 20.0);
    axo.scale = 60.0;
    tx = add(tx, "3D", axo);
    doc.commit(tx).unwrap();

    // one A1 sheet
    let sheet = Element::new("Glass doors", ElementKind::Sheet(Sheet::a("A211", "A1")));
    let sid = sheet.id;
    let mut tx = Transaction::new("sheet").insert(sheet);
    let at = [
        v(220.0, 470.0),
        v(220.0, 330.0),
        v(220.0, 180.0),
        v(620.0, 450.0),
        v(620.0, 220.0),
    ];
    for (k, (id, c)) in views.iter().zip(at).enumerate() {
        let d = view_drawing(doc.model(), *id).expect("drawn");
        let (lo, hi) = d.bounds().unwrap_or((DVec2::ZERO, DVec2::ZERO));
        // the plan cropped to the front and a little of the room
        let crop = (k == 0).then_some([v(-0.5, -1.5), v(14.5, 2.5)]);
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
