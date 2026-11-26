//! Phases through IFC: a phased project written and read back keeps its
//! phases and every element's place in them; a file from elsewhere that
//! carries only the standard Status comes in as Existing, Demolition and
//! Proposed.

use creabim_core::phases::{self, Phasing};
use creabim_core::{Document, ElementId, ElementKind, Model, Transaction};
use creabim_ifc::{export_ifc4, import_ifc, ExportOptions};

/// The starter house with phases, one wall (and what is in it) taken
/// down, and a new wall built in Proposed. The two walls' ids.
fn phased() -> (Model, ElementId, ElementId) {
    let mut doc = Document::from_model(creabim_core::starter::starter_house());
    doc.commit(phases::start(doc.model(), true).unwrap())
        .unwrap();
    let m = doc.model();
    let wall = m
        .iter()
        .find(|e| matches!(e.kind, ElementKind::Wall(_)) && !m.dependents(e.id).is_empty())
        .unwrap()
        .clone();
    let (tx, _) = phases::remove_from(m, &[wall.id], Some(3));
    doc.commit(tx).unwrap();
    doc.set_working_phase(Some(3));
    let mut new = wall.clone();
    new.id = ElementId::new();
    new.name = "New wall".into();
    if let ElementKind::Wall(w) = &mut new.kind {
        let across = (w.end - w.start).perp().normalize_or_zero() * 1.5;
        w.start += across;
        w.end += across;
    }
    let id = new.id;
    doc.commit(Transaction::new("New wall").insert(new))
        .unwrap();
    (doc.model().clone(), wall.id, id)
}

#[test]
fn a_phased_project_comes_back_with_its_phases() {
    let (model, down, new) = phased();
    // a fourth phase, to see the list itself is carried
    let mut doc = Document::from_model(model);
    let (tx, _) = phases::add(doc.model(), "Loft", Some(3)).unwrap();
    doc.commit(tx).unwrap();
    let ifc = export_ifc4(doc.model(), &ExportOptions::default());
    let back = import_ifc(&ifc).unwrap().model;
    let names: Vec<(String, bool)> = phases::list(&back)
        .iter()
        .map(|p| (p.name.clone(), p.demolition))
        .collect();
    assert_eq!(
        names,
        [
            ("Existing".to_string(), false),
            ("Demolition".into(), true),
            ("Proposed".into(), false),
            ("Loft".into(), false)
        ]
    );
    let list = phases::list(&back);
    let id = |n: &str| list.iter().find(|p| p.name == n).map(|p| p.id);
    assert_eq!(
        back.get(down).expect("the wall taken down").phase.gone,
        id("Demolition")
    );
    assert_eq!(back.get(down).unwrap().phase.built, None);
    assert_eq!(
        back.get(new).expect("the new wall").phase,
        Phasing {
            built: id("Proposed"),
            ..Default::default()
        }
    );
    // and it reads the same from each phase
    let at = |m: &Model, p: &str, e: ElementId| {
        let l = phases::list(m);
        let i = l.iter().position(|x| x.name == p).unwrap();
        phases::status(l, m.get(e).unwrap().phase, i)
    };
    for (p, e) in [
        ("Existing", down),
        ("Demolition", down),
        ("Proposed", down),
        ("Proposed", new),
        ("Existing", new),
    ] {
        assert_eq!(at(&back, p, e), at(doc.model(), p, e), "{p}");
    }
}

#[test]
fn a_file_with_only_the_standard_status_comes_in_phased() {
    let (model, down, new) = phased();
    // as another program would write it: no CreaBIM set, just Status
    let ifc =
        export_ifc4(&model, &ExportOptions::default()).replace("CreaBIM_Phasing", "Someone_Elses");
    let back = import_ifc(&ifc).unwrap().model;
    let names: Vec<&str> = phases::list(&back)
        .iter()
        .map(|p| p.name.as_str())
        .collect();
    assert_eq!(names, ["Existing", "Demolition", "Proposed"]);
    assert_eq!(back.get(down).unwrap().phase.gone, Some(2));
    assert_eq!(back.get(new).unwrap().phase.built, Some(3));
    let kept = back
        .iter()
        .find(|e| matches!(e.kind, ElementKind::Wall(_)) && e.id != down && e.id != new)
        .unwrap();
    assert!(kept.phase.is_default(), "an existing wall kept");
}

#[test]
fn bonsais_status_lists_and_unmarked_elements_come_in_apart() {
    // Highland Haven (Bonsai): Status written as a list, EXISTING and
    // DEMOLISH, for what is there now and comes down; and most of the
    // house marked nothing at all. The list read as its first value kept
    // the 6 coming down standing, and the unmarked all read as Existing.
    // Now what comes down comes down, and what says nothing goes in a
    // phase of its own, first, until it is understood (owner 2026-11-23)
    let (model, down, new) = phased();
    let ifc = export_ifc4(&model, &ExportOptions::default())
        .replace("CreaBIM_Phasing", "Someone_Elses")
        .replace(
            "(IFCLABEL('DEMOLISH'))",
            "(IFCLABEL('EXISTING'),IFCLABEL('DEMOLISH'))",
        );
    assert!(ifc.contains("IFCLABEL('EXISTING'),IFCLABEL('DEMOLISH')"));
    // one standing wall that says nothing: its Status set taken away
    let kept = model
        .iter()
        .filter(|e| matches!(e.kind, ElementKind::Wall(_)) && e.id != down && e.id != new)
        .map(|e| e.id)
        .collect::<Vec<_>>();
    let (quiet, marked) = (kept[0], kept[1]);
    let record = |guid: &str| {
        ifc.lines()
            .find(|l| l.contains(&format!("('{guid}'")))
            .and_then(|l| l.split('=').next())
            .map(str::to_string)
            .unwrap()
    };
    let wall = record(&quiet.ifc_guid());
    let common: Vec<String> = ifc
        .lines()
        .filter(|l| l.contains("IFCPROPERTYSET(") && l.contains("'Pset_WallCommon'"))
        .filter_map(|l| l.split('=').next().map(str::to_string))
        .collect();
    let ifc: String = ifc
        .lines()
        .filter(|l| {
            !(l.contains("IFCRELDEFINESBYPROPERTIES(")
                && l.contains(&format!("({wall})"))
                && common.iter().any(|s| l.ends_with(&format!(",{s});"))))
        })
        .map(|l| format!("{l}\n"))
        .collect();
    let back = import_ifc(&ifc).unwrap().model;
    let list = phases::list(&back);
    let names: Vec<&str> = list.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(
        names,
        ["Not marked (IFC)", "Existing", "Demolition", "Proposed"]
    );
    let id = |n: &str| list.iter().find(|p| p.name == n).map(|p| p.id);
    let down = back.get(down).unwrap().phase;
    assert_eq!(
        (down.built, down.gone),
        (id("Existing"), id("Demolition")),
        "comes down"
    );
    assert_eq!(back.get(new).unwrap().phase.built, id("Proposed"));
    assert_eq!(
        back.get(marked).unwrap().phase.built,
        id("Existing"),
        "marked existing"
    );
    assert!(
        back.get(quiet).unwrap().phase.is_default(),
        "unmarked: the first phase, its own"
    );
}

#[test]
fn an_unphased_file_stays_unphased() {
    let model = creabim_core::starter::starter_house();
    let back = import_ifc(&export_ifc4(&model, &ExportOptions::default()))
        .unwrap()
        .model;
    assert!(phases::list(&back).is_empty());
    assert!(back.iter().all(|e| e.phase.is_default()));
}

#[test]
fn furniture_rooms_and_curtain_walls_keep_their_phase() {
    // they were written by writers of their own that added no phasing,
    // so a new kitchen came back as if it had always been there
    let (model, _, _) = phased();
    let mut doc = Document::from_model(model);
    let picks: Vec<ElementId> = ["IfcFurniture", "IfcSpace"]
        .iter()
        .filter_map(|c| {
            doc.model()
                .iter()
                .find(|e| e.ifc_class() == *c)
                .map(|e| e.id)
        })
        .collect();
    assert_eq!(picks.len(), 2, "the starter house has furniture and rooms");
    let mut tx = Transaction::new("new");
    for id in &picks {
        let mut e = doc.model().get(*id).unwrap().clone();
        let was = e.clone();
        e.phase = Phasing {
            built: Some(3),
            ..Default::default()
        };
        tx = tx.replace(was, e);
    }
    doc.commit(tx).unwrap();
    let back = import_ifc(&export_ifc4(doc.model(), &ExportOptions::default()))
        .unwrap()
        .model;
    for id in picks {
        let e = back.get(id).unwrap();
        assert_eq!(e.phase.built, Some(3), "{} kept its phase", e.ifc_class());
    }
}
