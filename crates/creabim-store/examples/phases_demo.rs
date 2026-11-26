//! The starter house with phases, to try them on: the house as it stands
//! (Existing), a wall with what is in it taken down (Demolition), and a
//! new wall beside it (Proposed), with a plan and a 3D view standing at
//! each phase.
//!
//! `cargo run -p creabim-store --example phases_demo -- out.creabim`

use creabim_core::{
    phases, Document, Element, ElementId, ElementKind, Transaction, View, ViewKind,
};
use glam::DVec2;

fn main() {
    let out = std::env::args().nth(1).expect("the project to write");
    let mut doc = Document::from_model(creabim_core::starter::starter_house());
    doc.commit(phases::start(doc.model(), true).expect("no phases yet"))
        .unwrap();
    let m = doc.model();
    // a wall with a door or window in it comes down
    let wall = m
        .iter()
        .find(|e| matches!(e.kind, ElementKind::Wall(_)) && !m.dependents(e.id).is_empty())
        .expect("a wall with openings")
        .clone();
    let (tx, taken) = phases::remove_from(m, &[wall.id], Some(3));
    doc.commit(tx).unwrap();
    println!("{} taken down with {} in all", wall.name, taken);
    // a new one a metre and a half off it, drawn in Proposed
    doc.set_working_phase(Some(3));
    let mut new = wall.clone();
    new.id = ElementId::new();
    new.name = "New wall".into();
    if let ElementKind::Wall(w) = &mut new.kind {
        let across = (w.end - w.start).perp().normalize_or_zero() * 1.5;
        w.start += across;
        w.end += across;
    }
    doc.commit(Transaction::new("New wall").insert(new))
        .unwrap();
    doc.set_working_phase(None);
    // a plan and a 3D view at each phase
    let level = match &wall.kind {
        ElementKind::Wall(w) => w.level,
        _ => unreachable!(),
    };
    let mut tx = Transaction::new("Phase views");
    for p in phases::list(doc.model()).to_vec() {
        let mut plan = View::plan(level);
        plan.phase = Some(p.id);
        tx = tx.insert(Element::new(
            format!("{} plan", p.name),
            ElementKind::View(plan),
        ));
        let mut axo = View::axonometric(35.0, 30.0);
        axo.phase = Some(p.id);
        tx = tx.insert(Element::new(
            format!("{} 3D", p.name),
            ElementKind::View(axo),
        ));
    }
    doc.commit(tx).unwrap();
    for e in doc.model().iter() {
        if let ElementKind::View(v) = &e.kind {
            if matches!(v.kind, ViewKind::Plan { .. }) {
                println!("{:22} stands at {:?}", e.name, v.phase);
            }
        }
    }
    let _ = DVec2::ZERO;
    creabim_store::save(&out, &doc).unwrap();
}
