//! Arrangements of ordinary windows (doors and windows part 9): along the
//! front a ribbon of three windows frame to frame; at its east end a
//! window turning the corner glass to glass; at its west end one turning
//! the corner with a post. At the back an angled bay and a box bay, and on
//! the west a bow (part 9b). They are made as the Arrangement buttons in
//! Properties make them. A sheet shows the plan, the front and the east
//! side from outside, the window schedule with its Arrangement column and
//! 3D views from the front and the back.
//!
//! `cargo run -p creabim-store --example arrangements_demo -- arrangements.creabim`

use creabim_core::cells::WindowStyle;
use creabim_core::{
    arrangement, CornerJoint, Document, Element, ElementKind, Level, Opening, OpeningFrame,
    OpeningKind, ScheduleCategory, Sheet, Slab, Transaction, View, Viewport, Wall,
};
use creabim_geom::section::view_drawing;
use glam::DVec2;

fn v(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}

fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or("arrangements.creabim".into());
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

    // four outside walls, anticlockwise so the outside is on their right
    let mut tx = Transaction::new("walls");
    let mut front = None;
    let mut back = None;
    let mut west_wall = None;
    for (name, a, b) in [
        ("Front wall", v(0.0, 0.0), v(10.0, 0.0)),
        ("East wall", v(10.0, 0.0), v(10.0, 6.0)),
        ("Back wall", v(10.0, 6.0), v(0.0, 6.0)),
        ("West wall", v(0.0, 6.0), v(0.0, 0.0)),
    ] {
        let e = Element::new(name, ElementKind::Wall(Wall::new(g, a, b, 0.3, 3.0)));
        if name == "Front wall" {
            front = Some(e.id);
        }
        if name == "Back wall" {
            back = Some(e.id);
        }
        if name == "West wall" {
            west_wall = Some(e.id);
        }
        tx = tx.insert(e);
    }
    tx = tx.insert(Element::new(
        "Floor",
        ElementKind::Slab(Slab::new(
            g,
            vec![
                v(-0.15, -0.15),
                v(10.15, -0.15),
                v(10.15, 6.15),
                v(-0.15, 6.15),
            ],
            0.2,
        )),
    ));
    doc.commit(tx).unwrap();
    let front = front.unwrap();

    let fixed = || {
        let mut f = OpeningFrame::for_kind(OpeningKind::Window);
        f.grid = WindowStyle::Fixed.grid(false, [1, 1]);
        f
    };
    let window_in = |host, name: &str, offset: f64, width: f64| {
        Element::new(
            name,
            ElementKind::Opening(Opening {
                host,
                kind: OpeningKind::Window,
                offset,
                sill: 0.9,
                width,
                height: 1.4,
                material: None,
                frame: fixed(),
                flip_hand: false,
                flip_facing: false,
                roof_at: None,
                roof_build: Default::default(),
                arrangement: None,
            }),
        )
    };
    let window = |name: &str, offset: f64, width: f64| window_in(front, name, offset, width);
    let ribbon = window("Ribbon window", 3.5, 1.0);
    let east = window("Corner window, glass", 7.6, 1.6);
    let west = window("Corner window, post", 0.6, 1.4);
    // the bays' windows, before they become bays
    let (back, west_wall) = (back.unwrap(), west_wall.unwrap());
    let angled = window_in(back, "Angled bay", 1.6, 2.6);
    let boxed = window_in(back, "Box bay", 6.0, 1.8);
    let bow = window_in(west_wall, "Bow", 1.6, 2.6);
    let (an, bx, bw) = (angled.id, boxed.id, bow.id);
    doc.commit(
        Transaction::new("bay windows")
            .insert(angled)
            .insert(boxed)
            .insert(bow),
    )
    .unwrap();
    let (r, e, w) = (ribbon.id, east.id, west.id);
    doc.commit(
        Transaction::new("windows")
            .insert(ribbon)
            .insert(east)
            .insert(west),
    )
    .unwrap();
    // a ribbon of three, then the two corners turned, as Properties does
    for _ in 0..2 {
        let tx = arrangement::add_beside(doc.model(), r).unwrap();
        doc.commit(tx).unwrap();
    }
    let tx = arrangement::add_round_corner(doc.model(), e, true, CornerJoint::Glass).unwrap();
    doc.commit(tx).unwrap();
    let tx = arrangement::add_round_corner(doc.model(), w, false, CornerJoint::Post).unwrap();
    doc.commit(tx).unwrap();
    // part 9b: two bays and a bow
    use arrangement::BayShape;
    for (id, shape, joint) in [
        (an, BayShape::Angled, CornerJoint::Post),
        (bx, BayShape::Box, CornerJoint::Glass),
        (bw, BayShape::Bow(5), CornerJoint::Post),
    ] {
        let tx = arrangement::make_bay(doc.model(), id, shape, 0.6, joint).unwrap();
        doc.commit(tx).unwrap();
    }

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
    let mut front_elev = View::section(v(-2.0, -3.0), v(12.0, -3.0), 30.0, false);
    front_elev.scale = 50.0;
    tx = add(tx, "Front elevation", front_elev);
    let mut east_elev = View::section(v(13.0, -2.0), v(13.0, 8.0), 30.0, false);
    east_elev.scale = 50.0;
    tx = add(tx, "East elevation", east_elev);
    tx = add(
        tx,
        "Window Schedule",
        View::schedule(ScheduleCategory::Windows),
    );
    let mut axo = View::axonometric(25.0, 20.0);
    axo.scale = 60.0;
    tx = add(tx, "3D", axo);
    let mut axo_back = View::axonometric(205.0, 25.0);
    axo_back.scale = 60.0;
    tx = add(tx, "3D from the back", axo_back);
    doc.commit(tx).unwrap();

    // one A1 sheet
    let sheet = Element::new(
        "Ribbon, corner and bay windows",
        ElementKind::Sheet(Sheet::a("A213", "A1")),
    );
    let sid = sheet.id;
    let mut tx = Transaction::new("sheet").insert(sheet);
    // plan, front, east, schedule, 3D, 3D from the back: clear of each
    // other and of the title block at the foot on the right
    let at = [
        v(190.0, 430.0),
        v(190.0, 190.0),
        v(440.0, 190.0),
        v(690.0, 470.0),
        v(480.0, 430.0),
        v(690.0, 260.0),
    ];
    for (id, c) in views.iter().zip(at) {
        let d = view_drawing(doc.model(), *id).expect("drawn");
        let (lo, hi) = d.bounds().unwrap_or((DVec2::ZERO, DVec2::ZERO));
        tx = tx.insert(Element::new(
            "Viewport",
            ElementKind::Viewport(Viewport {
                sheet: sid,
                view: *id,
                center: c,
                target: (lo + hi) / 2.0,
                crop: None,
                fit: None,
                pinned: false,
            }),
        ));
    }
    doc.commit(tx).unwrap();
    creabim_store::save(&out, &doc).unwrap();
    println!("saved {out}");
}
