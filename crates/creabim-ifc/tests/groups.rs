//! Groups through IFC (Options from IFC REV00, owner 2026-11-23): a file's
//! IfcGroups are kept and written back as they came, and groups used as
//! design options (Highland Haven's four houses) become CreaBIM options,
//! a piece in some of them copied into each, with the file's membership
//! written back unchanged.

use creabim_core::groups::{self, GroupDef, IfcGroupSource};
use creabim_core::phases;
use creabim_core::{Document, ElementId, ElementKind, Model, Transaction};
use creabim_ifc::{export_ifc4, import_ifc, ExportOptions};
use std::collections::{BTreeMap, BTreeSet};

/// The starter house, phased, with groups over its walls: "1_A" and "1_B"
/// and "1_C" share the first wall; the second is in A and B; the third in
/// A alone; a saved search holds the first two.
fn grouped() -> (Model, Vec<ElementId>) {
    let mut doc = Document::from_model(creabim_core::starter::starter_house());
    doc.commit(phases::start(doc.model(), true).unwrap())
        .unwrap();
    let m = doc.model();
    let walls: Vec<ElementId> = m
        .iter()
        .filter(|e| matches!(e.kind, ElementKind::Wall(_)))
        .map(|e| e.id)
        .take(4)
        .collect();
    let g = |id: u32, name: &str, members: &[usize], description: Option<&str>| GroupDef {
        id,
        name: name.into(),
        members: members.iter().map(|i| walls[*i]).collect(),
        ifc: Some(IfcGroupSource {
            guid: ElementId::new().ifc_guid(),
            description: description.map(str::to_string),
            object_type: None,
        }),
        option: None,
    };
    let (pid, _) = m.project_info().unwrap();
    let before = m.get(pid).unwrap().clone();
    let mut after = before.clone();
    if let ElementKind::Project(p) = &mut after.kind {
        p.groups = vec![
            g(1, "1_A", &[0, 1, 2], None),
            g(2, "1_B", &[0, 1], None),
            g(3, "1_C", &[0, 3], None),
            g(
                4,
                "Load bearing",
                &[0, 1],
                Some("{\"type\": \"BBIM_Search\", \"query\": \"IfcWall\"}"),
            ),
        ];
    }
    doc.commit(Transaction::new("Groups").replace(before, after))
        .unwrap();
    (doc.model().clone(), walls)
}

/// Each group of `m` by name, with its members' GlobalIds, as written.
fn membership(m: &Model) -> BTreeMap<String, BTreeSet<String>> {
    groups::groups(m)
        .iter()
        .map(|g| {
            (
                g.name.clone(),
                groups::holds(m, g).iter().map(|id| id.ifc_guid()).collect(),
            )
        })
        .collect()
}

#[test]
fn groups_come_back_as_they_went() {
    let (model, _) = grouped();
    let text = export_ifc4(&model, &ExportOptions::default());
    let back = import_ifc(&text).expect("imports").model;
    assert_eq!(membership(&back), membership(&model));
    let search = groups::groups(&back)
        .iter()
        .find(|g| g.name == "Load bearing")
        .unwrap();
    assert!(search.is_search(), "the saved search keeps its query");
    let a = groups::groups(&model)
        .iter()
        .find(|g| g.name == "1_A")
        .unwrap();
    let a_back = groups::groups(&back)
        .iter()
        .find(|g| g.name == "1_A")
        .unwrap();
    assert_eq!(
        a_back.ifc.as_ref().unwrap().guid,
        a.ifc.as_ref().unwrap().guid,
        "the group keeps its GlobalId"
    );
}

#[test]
fn an_import_offers_numbered_groups_as_options() {
    let (model, _) = grouped();
    let text = export_ifc4(&model, &ExportOptions::default());
    let r = import_ifc(&text).expect("imports");
    let line = r
        .fidelity
        .iter()
        .find(|l| l.starts_with("groups:"))
        .expect("a groups line");
    assert_eq!(line, "groups: 4 of 4 kept (1 Bonsai saved searches, with the members they found); look like design options: 1_A, 1_B, 1_C");
    assert!(
        !r.fidelity
            .iter()
            .any(|l| l.starts_with("not read yet") && l.contains("group")),
        "groups are read now"
    );
}

#[test]
fn options_made_from_groups_write_the_groups_back_unchanged() {
    let (model, walls) = grouped();
    let before = membership(&model);
    let mut doc = Document::from_model(model);
    let (tx, made) = groups::make_options(doc.model(), &[1, 2, 3], "").expect("options made");
    doc.commit(tx).unwrap();
    assert_eq!(
        (made.shared, made.alone, made.copies),
        (1, 2, 1),
        "wall 1 copied into B"
    );
    let m = doc.model().clone();
    assert_eq!(
        membership(&m),
        before,
        "in CreaBIM the groups still hold the same pieces"
    );
    let text = export_ifc4(&m, &ExportOptions::default());
    // the copy is not written: the second wall appears once
    assert_eq!(
        text.matches(&format!("IFCWALL('{}'", walls[1].ifc_guid()))
            .count(),
        1,
        "the piece is written once"
    );
    let walls_before = export_ifc4(&grouped().0, &ExportOptions::default())
        .matches("IFCWALL(")
        .count();
    assert_eq!(
        text.matches("IFCWALL(").count(),
        walls_before,
        "and its copy is not"
    );
    let back = import_ifc(&text).expect("imports").model;
    assert_eq!(
        membership(&back),
        before,
        "the file's groups come back as they were"
    );
}

#[test]
fn a_copy_changed_in_its_option_goes_out_as_a_piece_of_its_own() {
    let (model, walls) = grouped();
    let mut doc = Document::from_model(model);
    let (tx, _) = groups::make_options(doc.model(), &[1, 2, 3], "").unwrap();
    doc.commit(tx).unwrap();
    let copy = groups::copies(doc.model())[0].copy;
    let tx = {
        let m = doc.model();
        let before = m.get(copy).unwrap().clone();
        let mut after = before.clone();
        if let ElementKind::Wall(w) = &mut after.kind {
            w.start.x += 0.25;
            w.end.x += 0.25;
        }
        Transaction::new("Moved in B").replace(before, after)
    };
    doc.commit(tx).unwrap();
    let text = export_ifc4(doc.model(), &ExportOptions::default());
    let back = import_ifc(&text).expect("imports").model;
    let got = membership(&back);
    assert!(
        got["1_B"].contains(&copy.ifc_guid()),
        "B holds its own, moved wall"
    );
    assert!(
        !got["1_B"].contains(&walls[1].ifc_guid()),
        "and not the original"
    );
    assert!(
        got["1_A"].contains(&walls[1].ifc_guid()),
        "A keeps the original"
    );
}

/// Bonsai's saved searches can hold element types too (Highland Haven's
/// "The Bark - House Only" holds 146): they go out and come back in the
/// group with the rest.
#[test]
fn a_group_holding_element_types_keeps_them() {
    let (model, walls) = grouped();
    let ty = model
        .iter()
        .find(|e| matches!(e.kind, ElementKind::ElementType(_)))
        .expect("the starter house has types")
        .id;
    let mut doc = Document::from_model(model);
    let (pid, _) = doc.model().project_info().unwrap();
    let before = doc.model().get(pid).unwrap().clone();
    let mut after = before.clone();
    if let ElementKind::Project(p) = &mut after.kind {
        p.groups.push(GroupDef {
            id: 9,
            name: "With types".into(),
            members: vec![walls[0], ty],
            ifc: None,
            option: None,
        });
    }
    doc.commit(Transaction::new("g").replace(before, after))
        .unwrap();
    let text = export_ifc4(doc.model(), &ExportOptions::default());
    let back = import_ifc(&text).expect("imports").model;
    assert_eq!(
        membership(&back)["With types"],
        membership(doc.model())["With types"]
    );
}
