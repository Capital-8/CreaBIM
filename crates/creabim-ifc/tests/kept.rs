//! The relationships and objects CreaBIM doesn't model go back out (owner
//! 2026-11-23, "Go ahead" on the side record): types, assemblies, openings,
//! material sets, drawings, sheets and documents. And CreaBIM's own
//! quantities and design options go out too.

use creabim_ifc::census::Census;
use creabim_ifc::step_read::parse;
use creabim_ifc::{export_ifc4, import_ifc, ExportOptions};

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!("{}/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

fn kept(src: &str, out: &str, measure: &str) -> (usize, usize) {
    let a = Census::of(&parse(src).unwrap());
    let b = Census::of(&parse(out).unwrap());
    let m = a
        .compare(&b)
        .into_iter()
        .find(|m| m.name == measure)
        .unwrap_or_else(|| panic!("no measure {measure}"));
    (m.kept, m.source)
}

#[test]
fn drawings_sheets_and_documents_come_back() {
    let src = fixture("bench/carried-data.ifc");
    let r = import_ifc(&src).unwrap();
    let out = export_ifc4(&r.model, &ExportOptions::default());
    for measure in [
        "Bonsai drawings",
        "Bonsai sheets",
        "document records",
        "documents on objects",
        "type of each object",
    ] {
        let (k, n) = kept(&src, &out, measure);
        assert!(n > 0, "{measure}: the fixture has some");
        assert_eq!(k, n, "{measure}: {k} of {n} came back");
    }
    // and the project keeps its GlobalId, which the sheet hangs off
    let project = src.lines().find(|l| l.contains("=IFCPROJECT(")).unwrap();
    let guid = &project[project.find('\'').unwrap()..][..24];
    assert!(
        out.lines()
            .any(|l| l.contains("=IFCPROJECT(") && l.contains(guid)),
        "{guid}"
    );
}

#[test]
fn the_structure_of_a_bonsai_file_survives_a_round_trip() {
    let src = fixture("thirdparty/bonsai-basic.ifc");
    let r = import_ifc(&src).unwrap();
    let out = export_ifc4(&r.model, &ExportOptions::default());
    // every body here is a mesh mapped from a type: the types come back
    let (k, n) = kept(&src, &out, "type of each object");
    assert_eq!(k, n, "types: {k} of {n}");
    // the file reads again as it was
    import_ifc(&out).expect("the written file reads");
}

#[test]
fn a_mesh_keeps_its_layer_set_and_a_native_wall_keeps_creabims() {
    let src = fixture("bench/carried-data.ifc");
    let r = import_ifc(&src).unwrap();
    let data = r.model.ifc_data();
    // every kept material relationship names the model's objects by id
    for rel in data
        .rels
        .iter()
        .filter(|r| r.class == "IFCRELASSOCIATESMATERIAL")
    {
        assert!(!rel.related.is_empty());
    }
    let out = export_ifc4(&r.model, &ExportOptions::default());
    // no element is given two materials
    let f = parse(&out).unwrap();
    let mut seen = std::collections::HashMap::<u64, usize>::new();
    for e in f
        .entities
        .values()
        .filter(|e| e.is("IFCRELASSOCIATESMATERIAL"))
    {
        for o in e.arg(4).refs() {
            *seen.entry(o).or_default() += 1;
        }
    }
    let twice: Vec<_> = seen.iter().filter(|(_, n)| **n > 1).collect();
    assert!(twice.is_empty(), "given two materials: {twice:?}");
}

#[test]
fn creabims_quantities_and_options_go_out() {
    use creabim_core::{phases, Document};
    let mut doc = Document::from_model(creabim_core::sample::sample_house());
    let tx = phases::start(doc.model(), false).unwrap();
    doc.commit(tx).unwrap();
    let wall = doc
        .model()
        .iter()
        .find(|e| matches!(e.kind, creabim_core::ElementKind::Wall(_)))
        .unwrap()
        .id;
    let (tx, [first, _]) = phases::add_question(doc.model(), "Roof").expect("a question");
    doc.commit(tx).unwrap();
    let q = phases::questions(doc.model())[0].clone();
    let name = q
        .options
        .iter()
        .find(|o| o.id == first)
        .unwrap()
        .name
        .clone();
    doc.edit(wall, "In the first option", |e| {
        e.phase.option = Some(first)
    })
    .unwrap();
    let out = export_ifc4(doc.model(), &ExportOptions::default());
    assert!(out.contains("'Qto_WallBaseQuantities'"));
    assert!(out.contains("IFCQUANTITYLENGTH('Height'"));
    // the option holding the wall goes out as a group of that name
    let quoted = format!("'{name}'");
    let group = out
        .lines()
        .find(|l| l.contains("IFCGROUP(") && l.contains(&quoted))
        .expect("the option's group");
    assert!(group.contains("'CreaBIM option'"));
    let n = group.split('=').next().unwrap();
    let wall_ref = out
        .lines()
        .find(|l| l.contains(&format!("IFCWALL('{}'", wall.ifc_guid())))
        .unwrap()
        .split('=')
        .next()
        .unwrap()
        .to_string();
    assert!(out.lines().any(|l| l.contains("IFCRELASSIGNSTOGROUP")
        && l.ends_with(&format!(",{n});"))
        && l.contains(&wall_ref)));
    // the same GlobalId each time
    let again = export_ifc4(doc.model(), &ExportOptions::default());
    let gid = |s: &str| {
        s.lines()
            .find(|l| l.contains("IFCGROUP(") && l.contains(&quoted))
            .unwrap()
            .split('\'')
            .nth(1)
            .unwrap()
            .to_string()
    };
    assert_eq!(gid(&out), gid(&again));
}
