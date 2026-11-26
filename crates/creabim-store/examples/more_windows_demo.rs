//! More windows: a tilt and turn sash, a single hung and a double hung
//! sash window, a horizontal and a vertical pivot window and a bifold
//! window along the front wall of a small building. A sheet shows the
//! plan, the front elevation from outside and from inside, sections
//! through the double hung window and the horizontal pivot, the window
//! schedule and a 3D view.
//!
//! `cargo run -p creabim-store --example more_windows_demo -- more_windows.creabim`

use creabim_core::cells::WindowStyle;
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
    let out = std::env::args()
        .nth(1)
        .unwrap_or("more_windows.creabim".into());
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
        ("Front wall", v(0.0, 0.0), v(12.0, 0.0), 0.3),
        ("East wall", v(12.0, 0.0), v(12.0, 6.0), 0.3),
        ("Back wall", v(12.0, 6.0), v(0.0, 6.0), 0.3),
        ("West wall", v(0.0, 6.0), v(0.0, 0.0), 0.3),
    ] {
        let e = Element::new(name, ElementKind::Wall(Wall::new(g, a, b, t, 3.0)));
        if name == "Front wall" {
            front = Some(e.id)
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
    let front = front.unwrap();

    // the windows: (name, offset, width, sill, height, style)
    let styles = [
        ("Tilt and turn", 0.6, 0.9, 0.9, 1.2, WindowStyle::TiltTurn),
        ("Single hung", 2.1, 0.9, 0.75, 1.5, WindowStyle::SingleHung),
        ("Double hung", 3.6, 0.9, 0.75, 1.5, WindowStyle::DoubleHung),
        (
            "Horizontal pivot",
            5.1,
            1.2,
            0.9,
            1.2,
            WindowStyle::HorizontalPivot,
        ),
        (
            "Vertical pivot",
            6.9,
            1.0,
            0.75,
            1.5,
            WindowStyle::VerticalPivot,
        ),
        ("Bifold", 8.6, 1.6, 0.9, 1.2, WindowStyle::Bifold),
    ];
    let mut tx = Transaction::new("windows");
    for (name, offset, width, sill, height, style) in styles {
        let mut frame = OpeningFrame::for_kind(OpeningKind::Window);
        frame.grid = style.grid(false, [1, 1]);
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
    let mut elev = View::section(v(-1.0, -4.0), v(13.0, -4.0), 30.0, false);
    elev.scale = 50.0;
    tx = add(tx, "Front elevation", elev);
    // inside, looking back at the front wall: the outward sashes now
    // open away, their triangles dashed
    let mut sec = View::section(v(12.6, 2.0), v(-0.6, 2.0), 2.5, true);
    sec.scale = 50.0;
    tx = add(tx, "Section A, inside looking at the front wall", sec);
    // up through the double hung window and the horizontal pivot,
    // looking west
    let mut sec = View::section(v(4.05, -1.6), v(4.05, 2.0), 5.0, true);
    sec.scale = 20.0;
    tx = add(tx, "Section B, through the double hung window", sec);
    let mut sec = View::section(v(5.7, -1.6), v(5.7, 2.0), 5.0, true);
    sec.scale = 20.0;
    tx = add(tx, "Section C, through the horizontal pivot", sec);
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
    let sheet = Element::new("More windows", ElementKind::Sheet(Sheet::a("A208", "A1")));
    let sid = sheet.id;
    let mut tx = Transaction::new("sheet").insert(sheet);
    let at = [
        v(185.0, 470.0),
        v(185.0, 330.0),
        v(185.0, 190.0),
        v(540.0, 470.0),
        v(720.0, 470.0),
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
