//! Windows, each way they open: a fixed light, a casement, a casement
//! pair, an awning, a hopper, a horizontal slider and a louvre along the
//! front wall of a small building, and on its side wall a window of
//! several cells, a fixed sidelight and two casements under a fixed
//! transom, built as one grid. A sheet shows the plan, the front
//! elevation from outside and from inside, a section through the
//! awning, the window schedule and a 3D view.
//!
//! `cargo run -p creabim-store --example window_demo -- windows.creabim`

use creabim_core::cells::{Cell, Grid, Mechanism, Track, WindowStyle};
use creabim_core::{
    Document, Element, ElementKind, Level, Opening, OpeningFrame, OpeningKind, ScheduleCategory,
    Sheet, Slab, Transaction, View, Viewport, Wall,
};
use creabim_geom::section::view_drawing;
use glam::DVec2;

fn v(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or("windows.creabim".into());
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
    let mut side = None;
    for (name, a, b, t) in [
        ("Front wall", v(0.0, 0.0), v(12.0, 0.0), 0.3),
        ("East wall", v(12.0, 0.0), v(12.0, 6.0), 0.3),
        ("Back wall", v(12.0, 6.0), v(0.0, 6.0), 0.3),
        ("West wall", v(0.0, 6.0), v(0.0, 0.0), 0.3),
    ] {
        let e = Element::new(name, ElementKind::Wall(Wall::new(g, a, b, t, 3.0)));
        match name {
            "Front wall" => front = Some(e.id),
            "East wall" => side = Some(e.id),
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
    let (front, side) = (front.unwrap(), side.unwrap());

    // the windows: (name, wall, offset, width, grid)
    let styles = [
        ("Fixed light", 0.6, WindowStyle::Fixed),
        ("Casement", 2.0, WindowStyle::Casement),
        ("Casement pair", 3.4, WindowStyle::CasementPair),
        ("Awning", 5.3, WindowStyle::Awning),
        ("Hopper", 6.8, WindowStyle::Hopper),
        ("Horizontal slider", 8.2, WindowStyle::Sliding),
        ("Louvre", 10.4, WindowStyle::Louvred),
    ];
    let mut tx = Transaction::new("windows");
    for (name, offset, style) in styles {
        let width = match style {
            WindowStyle::CasementPair | WindowStyle::Sliding => 1.4,
            _ => 0.9,
        };
        let mut frame = OpeningFrame::for_kind(OpeningKind::Window);
        frame.grid = style.grid(false, [1, 1]);
        tx = tx.insert(Element::new(
            name,
            ElementKind::Opening(Opening {
                host: front,
                kind: OpeningKind::Window,
                offset,
                sill: 0.9,
                width,
                height: 1.2,
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
    // on the side wall, one window of several cells: a fixed sidelight
    // and two casements, a fixed transom over all three
    let mut frame = OpeningFrame::for_kind(OpeningKind::Window);
    let casement = |hand: bool| Cell {
        hand,
        ..Cell::new(Mechanism::Hinged)
    };
    frame.grid = Grid {
        columns: vec![
            Track::new(0.6),
            Track {
                share: 0.8,
                bar: true,
            },
            Track::new(0.8),
        ],
        rows: vec![
            Track::new(1.4),
            Track {
                share: 0.5,
                bar: true,
            },
        ],
        cells: vec![
            Cell::new(Mechanism::Fixed),
            casement(false),
            casement(true),
            Cell::new(Mechanism::Fixed),
            Cell::new(Mechanism::Fixed),
            Cell::new(Mechanism::Fixed),
        ],
        ..Grid::default()
    }
    .fitted();
    tx = tx.insert(Element::new(
        "Sidelight, casements and transom",
        ElementKind::Opening(Opening {
            host: side,
            kind: OpeningKind::Window,
            offset: 1.8,
            sill: 0.6,
            width: 2.2,
            height: 1.9,
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
    let mut elev = View::section(v(-1.0, -4.0), v(13.0, -4.0), 30.0, false);
    elev.scale = 50.0;
    tx = add(tx, "Front elevation", elev);
    // inside, looking back at the front wall: the outward sashes now
    // open away, their triangles dashed
    let mut sec = View::section(v(12.6, 2.0), v(-0.6, 2.0), 2.5, true);
    sec.scale = 50.0;
    tx = add(tx, "Section A, inside looking at the front wall", sec);
    // up through the awning, looking west
    let mut sec = View::section(v(5.75, -1.6), v(5.75, 2.0), 5.0, true);
    sec.scale = 20.0;
    tx = add(tx, "Section B, through the awning", sec);
    // the side wall from outside
    let mut east = View::section(v(16.0, -1.0), v(16.0, 7.0), 30.0, false);
    east.scale = 50.0;
    tx = add(tx, "East elevation", east);
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
    let sheet = Element::new("Windows", ElementKind::Sheet(Sheet::a("A203", "A1")));
    let sid = sheet.id;
    let mut tx = Transaction::new("sheet").insert(sheet);
    let at = [
        v(185.0, 470.0),
        v(185.0, 330.0),
        v(185.0, 190.0),
        v(560.0, 470.0),
        v(690.0, 470.0),
        v(600.0, 300.0),
        v(600.0, 130.0),
    ];
    for (k, (id, c)) in views.iter().zip(at).enumerate() {
        let d = view_drawing(doc.model(), *id).expect("drawn");
        let (lo, hi) = d.bounds().unwrap_or((DVec2::ZERO, DVec2::ZERO));
        let crop = (k == 0).then_some([v(-1.0, -1.0), v(13.0, 7.0)]);
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
