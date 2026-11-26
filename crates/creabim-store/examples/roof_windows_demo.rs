//! Roof windows: a fixed, a top hung and a centre pivot roof window in
//! the front slope of a small gabled building, and a wide fixed one in
//! the back slope. A sheet shows the roof plan, the front elevation, a
//! section through the centre pivot window, the window schedule and a 3D
//! view.
//!
//! `cargo run -p creabim-store --example roof_windows_demo -- roof_windows.creabim`

use creabim_core::cells::WindowStyle;
use creabim_core::{
    Document, Element, ElementKind, Level, Opening, OpeningFrame, OpeningKind, Roof, RoofShape,
    ScheduleCategory, Sheet, Slab, Transaction, View, ViewKind, Viewport, Wall,
};
use creabim_geom::section::view_drawing;
use glam::DVec2;

fn v(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}

fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or("roof_windows.creabim".into());
    let mut doc = Document::new();

    let ground = Element::new(
        "Ground Floor",
        ElementKind::Level(Level {
            elevation: 0.0,
            height: 2.7,
        }),
    );
    let g = ground.id;
    doc.commit(Transaction::new("levels").insert(ground))
        .unwrap();

    // four outside walls, anticlockwise so the outside is on their right,
    // a floor, and a gabled roof with its ridge along the front
    let mut tx = Transaction::new("building");
    let mut roof = Roof::new(
        g,
        vec![
            v(-0.15, -0.15),
            v(10.15, -0.15),
            v(10.15, 7.15),
            v(-0.15, 7.15),
        ],
        RoofShape::Gable,
    );
    roof.base_offset = 2.7;
    roof.pitch = 40.0;
    roof.overhang = 0.4;
    roof.thickness = 0.3;
    roof.edge = 0;
    let roof = Element::new("Roof", ElementKind::Roof(roof));
    let rid = roof.id;
    tx = tx.insert(roof);
    for (name, a, b) in [
        ("Front wall", v(0.0, 0.0), v(10.0, 0.0)),
        ("East wall", v(10.0, 0.0), v(10.0, 7.0)),
        ("Back wall", v(10.0, 7.0), v(0.0, 7.0)),
        ("West wall", v(0.0, 7.0), v(0.0, 0.0)),
    ] {
        // the end walls rise to the roof's underside, the gables
        let mut w = Wall::new(g, a, b, 0.3, 2.7);
        if name.starts_with("East") || name.starts_with("West") {
            w.attach_top = vec![rid];
        }
        tx = tx.insert(Element::new(name, ElementKind::Wall(w)));
    }
    tx = tx.insert(Element::new(
        "Floor",
        ElementKind::Slab(Slab::new(
            g,
            vec![
                v(-0.15, -0.15),
                v(10.15, -0.15),
                v(10.15, 7.15),
                v(-0.15, 7.15),
            ],
            0.2,
        )),
    ));
    doc.commit(tx).unwrap();

    // the roof windows: (name, centre in plan, width, height, style)
    let windows = [
        (
            "Roof window, fixed",
            v(2.5, 1.6),
            0.78,
            1.4,
            WindowStyle::Fixed,
        ),
        (
            "Roof window, top hung",
            v(5.0, 1.6),
            0.78,
            1.4,
            WindowStyle::Awning,
        ),
        (
            "Roof window, centre pivot",
            v(7.5, 1.6),
            0.78,
            1.4,
            WindowStyle::HorizontalPivot,
        ),
        (
            "Roof window, back",
            v(5.0, 5.3),
            1.34,
            0.98,
            WindowStyle::Fixed,
        ),
    ];
    let mut tx = Transaction::new("roof windows");
    for (name, at, width, height, style) in windows {
        let mut frame = OpeningFrame::for_kind(OpeningKind::Window);
        frame.grid = style.grid(false, [1, 1]);
        tx = tx.insert(Element::new(
            name,
            ElementKind::Opening(Opening {
                host: rid,
                kind: OpeningKind::Window,
                offset: 0.0,
                sill: 0.0,
                width,
                height,
                material: None,
                frame,
                flip_hand: false,
                flip_facing: false,
                roof_at: Some(at),
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
    // the roof seen from above: the plan cut over the ridge
    let mut plan = View::plan(g);
    plan.scale = 50.0;
    if let ViewKind::Plan { cut_height, .. } = &mut plan.kind {
        *cut_height = 8.0;
    }
    tx = add(tx, "Roof plan", plan);
    let mut elev = View::section(v(-1.0, -4.0), v(11.0, -4.0), 30.0, false);
    elev.scale = 50.0;
    tx = add(tx, "Front elevation", elev);
    // across the building through the centre pivot window, looking east
    let mut sec = View::section(v(7.5, -1.5), v(7.5, 8.5), 6.0, true);
    sec.scale = 50.0;
    tx = add(tx, "Section A, through the centre pivot window", sec);
    tx = add(
        tx,
        "Window Schedule",
        View::schedule(ScheduleCategory::Windows),
    );
    let mut axo = View::axonometric(25.0, 35.0);
    axo.scale = 100.0;
    tx = add(tx, "3D", axo);
    doc.commit(tx).unwrap();

    // one A1 sheet: the roof plan and front elevation down the left, the
    // section, schedule and 3D on the right
    let sheet = Element::new("Roof windows", ElementKind::Sheet(Sheet::a("A210", "A1")));
    let sid = sheet.id;
    let mut tx = Transaction::new("sheet").insert(sheet);
    let at = [
        v(200.0, 430.0),
        v(200.0, 200.0),
        v(600.0, 420.0),
        v(600.0, 280.0),
        v(600.0, 140.0),
    ];
    for (k, (id, c)) in views.iter().zip(at).enumerate() {
        let d = view_drawing(doc.model(), *id).expect("drawn");
        let (lo, hi) = d.bounds().unwrap_or((DVec2::ZERO, DVec2::ZERO));
        // the plan cropped to the roof
        let crop = match k {
            0 => Some([v(-1.0, -1.0), v(11.0, 8.0)]),
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
