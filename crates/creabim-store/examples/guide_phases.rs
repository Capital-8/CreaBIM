//! The guide's Phases pictures: a small two-room house as surveyed
//! (Existing), its partition and part of its east wall taken down
//! (Demolition), and an extension built onto it (Proposed), with a plan
//! and a 3D view standing at each phase.
//!
//! `cargo run -p creabim-store --example guide_phases -- out.creabim [before-phases.creabim]`

use creabim_core::{
    phases, Document, Element, ElementId, ElementKind, Level, Opening, OpeningKind, Slab,
    Transaction, View, Wall,
};
use glam::DVec2;

const EXT: f64 = 0.3;
const INT: f64 = 0.12;
const H: f64 = 2.7;

fn v(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}

fn opening(host: ElementId, kind: OpeningKind, offset: f64, width: f64) -> Opening {
    Opening {
        host,
        kind,
        offset,
        sill: if kind == OpeningKind::Window {
            0.9
        } else {
            0.0
        },
        width,
        height: if kind == OpeningKind::Window {
            1.3
        } else {
            2.1
        },
        material: None,
        frame: creabim_core::OpeningFrame::for_kind(kind),
        flip_hand: false,
        flip_facing: false,
        roof_at: None,
        roof_build: Default::default(),
        arrangement: None,
    }
}

struct B {
    tx: Transaction,
    level: ElementId,
}

impl B {
    fn wall(&mut self, name: &str, a: DVec2, b: DVec2, t: f64) -> ElementId {
        let e = Element::new(name, ElementKind::Wall(Wall::new(self.level, a, b, t, H)));
        let id = e.id;
        self.tx = std::mem::replace(&mut self.tx, Transaction::new("")).insert(e);
        id
    }
    fn open(&mut self, name: &str, host: ElementId, kind: OpeningKind, offset: f64, width: f64) {
        let e = Element::new(
            name,
            ElementKind::Opening(opening(host, kind, offset, width)),
        );
        self.tx = std::mem::replace(&mut self.tx, Transaction::new("")).insert(e);
    }
    fn slab(&mut self, name: &str, a: DVec2, b: DVec2) {
        let s = Slab::new(self.level, creabim_core::polygon::rectangle(a, b), 0.2);
        self.tx = std::mem::replace(&mut self.tx, Transaction::new(""))
            .insert(Element::new(name, ElementKind::Slab(s)));
    }
}

fn main() {
    let out = std::env::args().nth(1).expect("the project to write");
    let mut doc = Document::from_model(creabim_core::Model::new());
    let lv = Element::new(
        "Ground floor",
        ElementKind::Level(Level {
            elevation: 0.0,
            height: 3.0,
        }),
    );
    let level = lv.id;
    doc.commit(Transaction::new("Level").insert(lv)).unwrap();

    // the house as it stands: 8 × 6 m, two rooms
    let mut b = B {
        tx: Transaction::new("The house as surveyed"),
        level,
    };
    let south = b.wall("South wall", v(0.0, 0.0), v(8.0, 0.0), EXT);
    let north = b.wall("North wall", v(8.0, 6.0), v(0.0, 6.0), EXT);
    let west = b.wall("West wall", v(0.0, 6.0), v(0.0, 0.0), EXT);
    b.wall("East wall, south", v(8.0, 0.0), v(8.0, 1.6), EXT);
    let east_mid = b.wall("East wall, middle", v(8.0, 1.6), v(8.0, 4.4), EXT);
    b.wall("East wall, north", v(8.0, 4.4), v(8.0, 6.0), EXT);
    let part = b.wall("Partition", v(4.4, 0.15), v(4.4, 5.85), INT);
    b.open("Front door", south, OpeningKind::Door, 1.6, 1.0);
    b.open("North window, east", north, OpeningKind::Window, 1.3, 1.4);
    b.open("North window, west", north, OpeningKind::Window, 5.3, 1.4);
    b.open("West window", west, OpeningKind::Window, 2.3, 1.4);
    b.open("East window", east_mid, OpeningKind::Window, 0.7, 1.4);
    b.open("Partition door", part, OpeningKind::Door, 3.9, 0.9);
    b.slab("Floor", v(0.0, 0.0), v(8.0, 6.0));
    doc.commit(b.tx).unwrap();

    // the same house before phases start, for the "Start phases" pictures
    if let Some(before) = std::env::args().nth(2) {
        creabim_store::save(&before, &doc).unwrap();
    }

    // phases, with what is drawn as the existing building
    doc.commit(phases::start(doc.model(), true).expect("no phases yet"))
        .unwrap();
    let list = phases::list(doc.model()).to_vec();
    let (demolition, proposed) = (list[1].id, list[2].id);

    // the partition and the east wall's middle come down, with what is in them
    let (tx, n) = phases::remove_from(doc.model(), &[part, east_mid], Some(demolition));
    doc.commit(tx).unwrap();
    println!("taken down: {n}");

    // the extension, built in Proposed
    doc.set_working_phase(Some(proposed));
    let mut b = B {
        tx: Transaction::new("The extension"),
        level,
    };
    b.wall("Extension, south", v(8.0, 1.2), v(11.6, 1.2), EXT);
    let east = b.wall("Extension, east", v(11.6, 1.2), v(11.6, 4.8), EXT);
    b.wall("Extension, north", v(11.6, 4.8), v(8.0, 4.8), EXT);
    b.open("Extension window", east, OpeningKind::Window, 0.9, 1.8);
    b.slab("Extension floor", v(8.0, 1.2), v(11.6, 4.8));
    doc.commit(b.tx).unwrap();
    doc.set_working_phase(None);

    // a plan and a 3D view at each phase
    let mut tx = Transaction::new("Phase views");
    for p in &list {
        let mut plan = View::plan(level);
        plan.phase = Some(p.id);
        tx = tx.insert(Element::new(
            format!("{} plan", p.name),
            ElementKind::View(plan),
        ));
        let mut axo = View::axonometric(215.0, 32.0);
        axo.phase = Some(p.id);
        tx = tx.insert(Element::new(
            format!("{} 3D", p.name),
            ElementKind::View(axo),
        ));
    }
    doc.commit(tx).unwrap();
    creabim_store::save(&out, &doc).unwrap();
    println!("wrote {out}");
}
