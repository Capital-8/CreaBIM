//! The starter house with two notes on the ground floor plan, to try
//! linked leaders on (MU135): one reading the wall its arrow points at
//! ({Name}, {Type}, {Material}, {Size}), one with its own text.
//!
//! `cargo run -p creabim-store --example notes_demo -- out.creabim`

use creabim_core::element::HostPoint;
use creabim_core::{Document, Element, ElementKind, Note, NoteReads, Transaction, ViewKind};
use glam::DVec2;

fn main() {
    let out = std::env::args().nth(1).expect("the project to write");
    let mut doc = Document::from_model(creabim_core::starter::starter_house());
    let m = doc.model();
    let (plan, level) = m
        .iter()
        .find_map(|e| match &e.kind {
            ElementKind::View(v) => match v.kind {
                ViewKind::Plan { level, .. } if e.name.contains("Ground") => Some((e.id, level)),
                _ => None,
            },
            _ => None,
        })
        .expect("a ground floor plan");
    let (wall, start, end) = m
        .iter()
        .find_map(|e| match &e.kind {
            ElementKind::Wall(w) if w.level == level => Some((e.id, w.start, w.end)),
            _ => None,
        })
        .expect("a wall on it");
    let mid = (start + end) / 2.0;
    let len = start.distance(end);
    let out_dir = (end - start).perp().normalize_or_zero();
    let at = mid + out_dir * 3.0 + DVec2::new(1.0, 0.0);
    let mut reading = Note::new(plan, at, "{Name}\n{Type} · {Material}\n{Size}");
    reading.leader = Some(HostPoint {
        host: Some(wall),
        ..HostPoint::free(DVec2::new(len / 2.0, 0.0))
    });
    reading.reads = Some(NoteReads {
        caps: true,
        ..Default::default()
    });
    reading.width = Some(70.0);
    let mut own = Note::new(
        plan,
        at + out_dir * 3.0,
        "Its own text, for comparing the rows",
    );
    own.leader = Some(HostPoint::free(mid + out_dir * 1.5));
    doc.commit(
        Transaction::new("Notes")
            .insert(Element::new("Reading note", ElementKind::Note(reading)))
            .insert(Element::new("Own note", ElementKind::Note(own))),
    )
    .unwrap();
    println!("plan {plan:?}, reading wall {wall:?}");
    creabim_store::save(&out, &doc).unwrap();
    // read it back and print what the plan draws for each note, as the
    // app's drawing does: the words filled from the wall, the arrow's end
    let back = creabim_store::load(&out).unwrap();
    let m = back.model();
    let Some(ElementKind::View(v)) = m.get(plan).map(|e| &e.kind) else {
        unreachable!()
    };
    for item in creabim_geom::annotate::annotations(m, plan, v) {
        if let creabim_geom::plan::PlanItem::Annotation { item, .. } = item {
            if let creabim_geom::annotation::Shape::Note { at, leader, .. } = item.shape {
                println!("note at {at:?}, arrow to {leader:?}:\n{}\n", item.text);
            }
        }
    }
}
