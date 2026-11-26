//! Documents on elements (Element documents REV00, owner: A on all five):
//! an element's and a type's documents written as IfcDocumentReference on
//! IfcRelAssociatesDocument, one reference shared by what has it, and a
//! file's documents listed by name when read back.

use creabim_core::docrefs::{self, DocRef, From};
use creabim_core::{starter::starter_house, Document, ElementKind};
use creabim_ifc::{export_ifc4, import_ifc, ExportOptions};

#[test]
fn documents_go_out_once_on_what_has_them_and_come_back_by_name() {
    let mut doc = Document::from_model(starter_house());
    let walls: Vec<_> = doc
        .model()
        .iter()
        .filter(|e| matches!(e.kind, ElementKind::Wall(_)))
        .map(|e| e.id)
        .collect();
    let typed = doc
        .model()
        .iter()
        .find(|e| e.of_type.is_some())
        .unwrap()
        .clone();
    let ty = typed.of_type.as_ref().unwrap().id;
    let sheet = DocRef {
        name: "Block data sheet".into(),
        location: "docs/block.pdf".into(),
    };
    for w in &walls[..2] {
        let s = sheet.clone();
        doc.edit(*w, "Add document", move |e| e.documents.push(s))
            .unwrap();
    }
    doc.edit(ty, "Add document", |t| {
        t.documents
            .push(DocRef::from_link("https://example.com/guide"))
    })
    .unwrap();

    let out = export_ifc4(doc.model(), &ExportOptions::default());
    assert_eq!(
        out.matches("IFCDOCUMENTREFERENCE('docs/block.pdf',$,'Block data sheet',$,$)")
            .count(),
        1,
        "one reference, shared"
    );
    let num = out
        .lines()
        .find(|l| l.contains("'docs/block.pdf'"))
        .and_then(|l| l.split('=').next())
        .unwrap()
        .to_string();
    let rel = out
        .lines()
        .find(|l| l.contains("IFCRELASSOCIATESDOCUMENT") && l.ends_with(&format!(",{num});")))
        .expect("associated");
    assert_eq!(
        rel.matches('#').count(),
        1 + 2 + 1,
        "the rel, both walls, the reference: {rel}"
    );
    assert_eq!(
        out.matches("'https://example.com/guide'").count(),
        1,
        "the type's, once"
    );

    // read back: the file's now, on the walls and (through the type) on
    // every wall of the type
    let back = import_ifc(&out).unwrap();
    let w = back.model.get(walls[0]).unwrap();
    let got = docrefs::of(&back.model, w);
    assert!(got.contains(&(sheet.clone(), From::File)), "{got:?}");
    let of_type = back.model.get(typed.id).unwrap();
    let names: Vec<_> = docrefs::of(&back.model, of_type)
        .into_iter()
        .map(|(d, _)| d.name)
        .collect();
    assert!(names.contains(&"example.com".to_string()), "{names:?}");
    // and out again, once
    let again = export_ifc4(&back.model, &ExportOptions::default());
    assert_eq!(again.matches("'docs/block.pdf'").count(), 1);
}

#[test]
fn a_bonsai_reference_without_a_name_takes_its_documents() {
    // a wall of a real export, given a document the way Bonsai writes it:
    // the reference unnamed, its document information named
    let m = starter_house();
    let out = export_ifc4(&m, &ExportOptions::default());
    let wall = out.lines().find(|l| l.contains("=IFCWALL(")).unwrap();
    let num = wall.split('=').next().unwrap();
    let guid = wall.split('\'').nth(1).unwrap();
    let extra = format!(
        "#999901=IFCDOCUMENTINFORMATION('X','Casement data sheet',$,$,$,$,'DATASHEET',$,$,$,$,$,$,$,$,$,$);
         #999902=IFCDOCUMENTREFERENCE('docs/casement.pdf','X',$,$,#999901);
         #999903=IFCRELASSOCIATESDOCUMENT('0ydYodiAbDUAUIOh_4ZagO',$,$,$,({num}),#999902);
"
    );
    let at = out.rfind("ENDSEC;").unwrap();
    let ifc = format!("{}{extra}{}", &out[..at], &out[at..]);
    let r = import_ifc(&ifc).expect("reads");
    let w = r
        .model
        .iter()
        .find(|e| e.id.ifc_guid() == guid)
        .expect("the wall comes in");
    let got = docrefs::of(&r.model, w);
    assert_eq!(
        got,
        vec![(
            DocRef {
                name: "Casement data sheet".into(),
                location: "docs/casement.pdf".into()
            },
            From::File
        )]
    );
}
