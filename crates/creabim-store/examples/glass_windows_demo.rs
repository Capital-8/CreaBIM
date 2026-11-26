//! French and slim steel windows along a front wall: French casements,
//! a pair and a fixed light, and slim steel casements, a fixed light and
//! a fixed light over an awning. A sheet shows the plan, the front from
//! outside, the front from inside, the window schedule and a 3D view.
//!
//! `cargo run -p creabim-store --example glass_windows_demo -- glass_windows.creabim`

use creabim_core::cells::WindowStyle;
use creabim_core::construction::{Construction, LeafStyle};
use creabim_core::{
    Document, Element, ElementKind, Level, Opening, OpeningFrame, OpeningKind, ScheduleCategory,
    Sheet, Slab, Transaction, View, Viewport, Wall,
};
use creabim_geom::section::view_drawing;
use glam::DVec2;

fn v(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}

/// A window frame opening as `style`, made as `make`.
fn made(style: WindowStyle, make: LeafStyle) -> OpeningFrame {
    let mut f = OpeningFrame::for_kind(OpeningKind::Window);
    f.grid = style.grid(false, [1, 1]);
    f.construction = Construction {
        leaf: make,
        ..Construction::default()
    };
    if make == LeafStyle::SlimSteel {
        f.width = 0.04;
    }
    f
}

fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or("glass_windows.creabim".into());
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

    use LeafStyle as L;
    use WindowStyle as Ws;
    // (name, offset, width, sill, height, frame)
    let windows = [
        (
            "French casement",
            0.6,
            0.7,
            0.9,
            1.3,
            made(Ws::Casement, L::French),
        ),
        (
            "French casement pair",
            1.9,
            1.3,
            0.9,
            1.3,
            made(Ws::CasementPair, L::French),
        ),
        (
            "French, fixed",
            3.8,
            1.2,
            0.9,
            1.3,
            made(Ws::Fixed, L::French),
        ),
        (
            "Slim steel casement",
            5.6,
            0.7,
            0.9,
            1.3,
            made(Ws::Casement, L::SlimSteel),
        ),
        (
            "Slim steel, fixed",
            6.9,
            1.6,
            0.9,
            1.3,
            made(Ws::Fixed, L::SlimSteel),
        ),
        (
            "Slim steel awning",
            9.1,
            1.2,
            0.9,
            1.3,
            made(Ws::Awning, L::SlimSteel),
        ),
        (
            "Slim steel casement pair",
            11.0,
            1.4,
            0.9,
            1.3,
            made(Ws::CasementPair, L::SlimSteel),
        ),
    ];
    let mut tx = Transaction::new("windows");
    for (name, offset, width, sill, height, frame) in windows {
        tx = tx.insert(Element::new(
            name,
            ElementKind::Opening(Opening {
                host: front,
                kind: OpeningKind::Window,
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
    // the front from outside, and from inside looking back at it
    let mut elev = View::section(v(-0.6, -3.0), v(14.6, -3.0), 30.0, false);
    elev.scale = 50.0;
    tx = add(tx, "Front elevation", elev);
    let mut sec = View::section(v(14.6, 2.0), v(-0.6, 2.0), 2.5, true);
    sec.scale = 50.0;
    tx = add(tx, "Inside, looking at the front", sec);
    tx = add(
        tx,
        "Window Schedule",
        View::schedule(ScheduleCategory::Windows),
    );
    let mut axo = View::axonometric(25.0, 20.0);
    axo.scale = 60.0;
    tx = add(tx, "3D", axo);
    doc.commit(tx).unwrap();

    // one A1 sheet
    let sheet = Element::new(
        "French and slim steel windows",
        ElementKind::Sheet(Sheet::a("A212", "A1")),
    );
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
