//! Common properties (Common properties REV00, owner: A on all five):
//! declared ratings on the type with an own value per element, External
//! worked out, Load-bearing from the layers, written as IFC's common sets,
//! and a file's common sets read back into the fields.

use creabim_core::{starter::starter_house, Document, ElementKind};
use creabim_ifc::{export_ifc4, import_ifc, ExportOptions};

fn count(s: &str, needle: &str) -> usize {
    s.matches(needle).count()
}

#[test]
fn a_types_ratings_go_out_on_its_walls_and_come_back_as_theirs() {
    let mut doc = Document::from_model(starter_house());
    let wall = doc
        .model()
        .iter()
        .find(|e| matches!(e.kind, ElementKind::Wall(_)) && e.of_type.is_some())
        .expect("a typed wall")
        .clone();
    let ty = wall.of_type.as_ref().unwrap().id;
    let typed: Vec<_> = creabim_core::types::instances(doc.model(), ty);
    doc.edit(ty, "Ratings", |t| {
        if let ElementKind::ElementType(t) = &mut t.kind {
            t.ratings.fire = "REI 60".into();
            t.ratings.u_value = Some(0.28);
        }
    })
    .unwrap();
    let out = export_ifc4(doc.model(), &ExportOptions::default());
    // every wall of the type says it, and IsExternal (the house has rooms)
    assert_eq!(
        count(&out, "IFCLABEL('REI 60')"),
        typed.len(),
        "one a wall of the type"
    );
    assert!(out.contains("'IsExternal',$,IFCBOOLEAN("));
    assert!(out.contains("IFCTHERMALTRANSMITTANCEMEASURE(0.28)"));

    // read back: the wall's own, and not kept twice
    let back = import_ifc(&out).unwrap();
    let w = back.model.get(wall.id).expect("the wall comes back");
    let r = w
        .ratings
        .as_deref()
        .expect("its ratings come back as its own");
    assert_eq!(r.fire, "REI 60");
    assert_eq!(r.u_value, Some(0.28));
    let rec = back.model.ifc_data().get(&wall.id.ifc_guid());
    assert!(
        rec.is_none_or(|r| r
            .psets
            .iter()
            .all(|s| s.props.iter().all(|p| p.name != "FireRating"))),
        "the file's FireRating is the wall's now, not kept as well"
    );
    // and out again, once
    let again = export_ifc4(&back.model, &ExportOptions::default());
    assert_eq!(count(&again, "IFCLABEL('REI 60')"), typed.len());
}

#[test]
fn one_wall_set_external_keeps_it_and_its_neighbours_are_worked_out() {
    let mut doc = Document::from_model(starter_house());
    let walls: Vec<_> = doc
        .model()
        .iter()
        .filter(|e| matches!(e.kind, ElementKind::Wall(_)))
        .map(|e| e.id)
        .collect();
    let w = walls[0];
    let before = creabim_core::common::common(doc.model(), doc.model().get(w).unwrap())
        .external
        .unwrap()
        .0;
    doc.edit(w, "External", |e| {
        e.ratings = Some(Box::new(creabim_core::construction::Ratings {
            external: Some(!before),
            ..Default::default()
        }))
    })
    .unwrap();
    let c = creabim_core::common::common(doc.model(), doc.model().get(w).unwrap());
    assert_eq!(c.external, Some((!before, creabim_core::common::From::Own)));
    let other = creabim_core::common::common(doc.model(), doc.model().get(walls[1]).unwrap());
    assert_eq!(
        other.external.map(|x| x.1),
        Some(creabim_core::common::From::WorkedOut)
    );
}

#[test]
fn nothing_unknown_is_written_as_a_guess() {
    // no rooms: External is not known, so not written; nothing declared,
    // so no FireRating
    let m = creabim_core::sample::sample_house();
    if m.iter().any(|e| matches!(e.kind, ElementKind::Room(_))) {
        return;
    }
    let out = export_ifc4(&m, &ExportOptions::default());
    assert!(!out.contains("'IsExternal'"));
    assert!(!out.contains("'FireRating'"));
}
