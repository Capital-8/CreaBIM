//! Door leaves of each make along the inside of a small building's front
//! wall: flush, panelled, glazed, part glazed, louvred and with a vision
//! panel, with linings, architraves and thresholds round them and fire,
//! acoustic and other ratings declared. A sheet shows the plan, the
//! front wall from inside at 1:50 (no trim) and 1:20 (with it), a
//! section through a door at 1:20, the door schedule with its ratings,
//! and a 3D view.
//!
//! `cargo run -p creabim-store --example construction_demo -- construction.creabim`

use creabim_core::construction::{Construction, LeafStyle, Ratings, Trim};
use creabim_core::{
    Document, Element, ElementKind, Level, Opening, OpeningFrame, OpeningKind, ScheduleCategory,
    Sheet, Slab, Transaction, View, Viewport, Wall,
};
use creabim_geom::section::view_drawing;
use glam::DVec2;

fn v(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}

/// A door frame with a single leaf made as `leaf`, the trim round it and
/// the ratings declared for it.
fn made(leaf: LeafStyle, trim: Trim, ratings: Ratings) -> OpeningFrame {
    let mut f = OpeningFrame::for_kind(OpeningKind::Door);
    f.construction = Construction {
        leaf,
        trim,
        ratings,
        ..Construction::default()
    };
    f
}

fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or("construction.creabim".into());
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

    let trim = Trim {
        lining: 0.018,
        architrave: 0.07,
        threshold: 0.0,
    };
    let rated = |fire: &str, acoustic: &str| Ratings {
        fire: fire.into(),
        acoustic: acoustic.into(),
        ..Ratings::default()
    };
    let door = OpeningKind::Door;
    // (name, kind, offset, sill, width, height, frame)
    let openings = [
        (
            "Flush, fire rated",
            LeafStyle::Flush,
            rated("FD30", "Rw 32 dB"),
        ),
        ("Panelled", LeafStyle::Panelled, Ratings::default()),
        (
            "Glazed",
            LeafStyle::Glazed,
            Ratings {
                security: "RC2".into(),
                u_value: Some(1.4),
                accessible: true,
                ..Ratings::default()
            },
        ),
        ("Part glazed", LeafStyle::PartGlazed, rated("", "Rw 29 dB")),
        ("Louvred", LeafStyle::Louvred, Ratings::default()),
        ("Vision panel", LeafStyle::VisionPanel, rated("FD60", "")),
    ]
    .into_iter()
    .enumerate()
    .map(|(k, (name, leaf, ratings))| {
        let mut t = trim;
        // a threshold under the outer door only
        if k == 2 {
            t.threshold = 0.02;
        }
        (
            name,
            door,
            0.9 + 2.0 * k as f64,
            0.0,
            0.926,
            2.04,
            made(leaf, t, ratings),
        )
    })
    .collect::<Vec<_>>();
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
    // inside, looking back at the front wall: at 1:50, and at 1:20 with its trim
    let mut sec = View::section(v(13.6, 2.0), v(-0.6, 2.0), 2.5, true);
    sec.scale = 50.0;
    tx = add(tx, "Section A", sec);
    let mut sec = View::section(v(6.0, 2.0), v(0.6, 2.0), 2.5, true);
    sec.scale = 20.0;
    tx = add(tx, "Section B", sec);
    // through the glazed door's head and threshold, looking west
    let mut sec = View::section(v(5.4, -1.0), v(5.4, 1.2), 5.0, true);
    sec.scale = 20.0;
    tx = add(tx, "Section C", sec);
    tx = add(tx, "Door Schedule", View::schedule(ScheduleCategory::Doors));
    let mut axo = View::axonometric(200.0, 30.0);
    axo.scale = 100.0;
    tx = add(tx, "3D", axo);
    doc.commit(tx).unwrap();

    // one A1 sheet
    let sheet = Element::new(
        "Leaves, trim and ratings",
        ElementKind::Sheet(Sheet::a("A206", "A1")),
    );
    let sid = sheet.id;
    let mut tx = Transaction::new("sheet").insert(sheet);
    let at = [
        v(200.0, 470.0),
        v(200.0, 330.0),
        v(220.0, 190.0),
        v(560.0, 430.0),
        v(640.0, 270.0),
        v(620.0, 140.0),
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
