//! Property sets and classifications kept from a file go back out with the
//! objects that had them (owner 2026-11-23, "Side record").

use creabim_ifc::{export_ifc4, import_ifc, ExportOptions};

const FILE: &str = r"ISO-10303-21;
HEADER;FILE_DESCRIPTION((''),'2;1');FILE_NAME('','',(''),(''),'','','');FILE_SCHEMA(('IFC4'));ENDSEC;
DATA;
#1=IFCPROJECT('0YvctVUKr0kugbFTf53O9L',$,'P',$,$,$,$,(#11),#5);
#2=IFCSIUNIT(*,.LENGTHUNIT.,$,.METRE.);
#5=IFCUNITASSIGNMENT((#2));
#6=IFCCARTESIANPOINT((0.,0.,0.));
#7=IFCAXIS2PLACEMENT3D(#6,$,$);
#11=IFCGEOMETRICREPRESENTATIONCONTEXT($,'Model',3,1.E-05,#7,$);
#12=IFCLOCALPLACEMENT($,#7);
#13=IFCBUILDINGSTOREY('2FCZDorxHDT8NI01kdXi8P',$,'Ground',$,$,#12,$,$,.ELEMENT.,0.);
#20=IFCCARTESIANPOINT((0.,0.));
#21=IFCCARTESIANPOINT((4.,0.));
#22=IFCPOLYLINE((#20,#21));
#23=IFCSHAPEREPRESENTATION(#11,'Axis','Curve2D',(#22));
#24=IFCRECTANGLEPROFILEDEF(.AREA.,$,#25,4.,0.2);
#25=IFCAXIS2PLACEMENT2D(#26,$);
#26=IFCCARTESIANPOINT((2.,0.));
#27=IFCDIRECTION((0.,0.,1.));
#28=IFCEXTRUDEDAREASOLID(#24,#7,#27,3.);
#29=IFCSHAPEREPRESENTATION(#11,'Body','SweptSolid',(#28));
#30=IFCPRODUCTDEFINITIONSHAPE($,$,(#23,#29));
#31=IFCWALL('3vB2YO$MX4xv5uCqZZG05x',$,'W1',$,$,#12,#30,$,$);
#32=IFCWALL('1kTvXnbbzCWw8lcMd1dR4o',$,'W2',$,$,#12,#30,$,$);
#40=IFCPROPERTYSINGLEVALUE('FireRating',$,IFCLABEL('REI 60'),$);
#41=IFCPROPERTYSINGLEVALUE('Thermal',$,IFCTHERMALTRANSMITTANCEMEASURE(0.35),$);
#42=IFCPROPERTYENUMERATEDVALUE('Finish',$,(IFCLABEL('Plaster')),$);
#43=IFCPROPERTYSET('2Jx4u0OkzEVfC1Mp9z7GVt',$,'Acme_Wall',$,(#40,#41,#42));
#44=IFCRELDEFINESBYPROPERTIES('0e3cR$Q2T0fQGzD$rGGT1Y',$,$,$,(#31,#32),#43);
#50=IFCCLASSIFICATION('NBS','2015',$,'Uniclass',$,$,$);
#51=IFCCLASSIFICATIONREFERENCE($,'EF_25_10',$,#50,$,$);
#52=IFCRELASSOCIATESCLASSIFICATION('1nM8rWm0z3Ofp$pRk$JK5o',$,$,$,(#31),#51);
#60=IFCRELCONTAINEDINSPATIALSTRUCTURE('3Rz1bJ6oX5Ew0y9wQdJ2rT',$,$,$,(#31,#32),#13);
ENDSEC;
END-ISO-10303-21;
";

fn count(s: &str, needle: &str) -> usize {
    s.matches(needle).count()
}

#[test]
fn property_sets_and_classifications_come_back_on_their_objects() {
    let r = import_ifc(FILE).unwrap();
    let data = r.model.ifc_data();
    let w1 = data.get("3vB2YO$MX4xv5uCqZZG05x").expect("W1 kept");
    assert_eq!(w1.psets[0].name, "Acme_Wall");
    assert_eq!(w1.psets[0].props.len(), 3);
    assert_eq!(w1.classes[0].code.as_deref(), Some("EF_25_10"));
    assert_eq!(w1.classes[0].system.as_deref(), Some("Uniclass"));
    assert!(
        r.fidelity.iter().any(
            |l| l.contains("2 of 2 property sets (6 values), 1 of 1 classification references")
        ),
        "{:?}",
        r.fidelity
    );

    let out = export_ifc4(&r.model, &ExportOptions::default());
    // the set once, shared by both walls, values as the file wrote them
    assert_eq!(count(&out, "'Acme_Wall'"), 1, "{out}");
    assert!(out.contains("IFCPROPERTYSINGLEVALUE('FireRating',$,IFCLABEL('REI 60'),$)"));
    assert!(out.contains("IFCTHERMALTRANSMITTANCEMEASURE(0.35)"));
    assert!(out.contains("IFCPROPERTYENUMERATEDVALUE('Finish',$,(IFCLABEL('Plaster')),$)"));
    assert!(out.contains("IFCCLASSIFICATIONREFERENCE($,'EF_25_10',$,#"));
    assert!(out.contains("IFCCLASSIFICATION('NBS','2015',$,'Uniclass'"));

    // and read again, the same
    let again = import_ifc(&out).unwrap();
    assert_eq!(
        again.model.ifc_data().get("3vB2YO$MX4xv5uCqZZG05x"),
        Some(w1)
    );
    assert_eq!(
        again
            .model
            .ifc_data()
            .get("1kTvXnbbzCWw8lcMd1dR4o")
            .unwrap()
            .psets,
        w1.psets
    );
}

#[test]
fn a_deleted_element_takes_its_record_with_it() {
    let r = import_ifc(FILE).unwrap();
    let mut doc = creabim_core::Document::from_model(r.model);
    let w2 = creabim_core::ElementId::from_ifc_guid("1kTvXnbbzCWw8lcMd1dR4o").unwrap();
    let tx = creabim_core::Transaction::new("Delete").remove_all_cascade(doc.model(), &[w2]);
    doc.commit(tx).unwrap();
    let out = export_ifc4(doc.model(), &ExportOptions::default());
    assert!(!out.contains("1kTvXnbbzCWw8lcMd1dR4o"));
    // the set still goes on W1, alone
    assert_eq!(count(&out, "'Acme_Wall'"), 1);
}

#[test]
fn a_common_set_creabim_writes_takes_the_files_other_properties() {
    // phased, CreaBIM writes each wall's Pset_WallCommon with its Status;
    // the file's Pset_WallCommon joins it rather than going out twice
    let file = FILE.replace("'Acme_Wall'", "'Pset_WallCommon'");
    let r = import_ifc(&file).unwrap();
    let mut doc = creabim_core::Document::from_model(r.model);
    let tx = creabim_core::phases::start(doc.model(), false).expect("phases start");
    doc.commit(tx).unwrap();
    let out = export_ifc4(doc.model(), &ExportOptions::default());
    assert_eq!(
        count(&out, "'Pset_WallCommon'"),
        2,
        "one set a wall:
{out}"
    );
    assert_eq!(count(&out, "'Status'"), 2, "CreaBIM's Status stands");
    assert_eq!(
        count(&out, "IFCLABEL('REI 60')"),
        2,
        "the file's FireRating joins each wall's set"
    );
    let again = import_ifc(&out).unwrap();
    // FireRating is a common property: back as the wall's own rating, not
    // kept beside it as well (Common properties REV00, decision 5)
    let id = creabim_core::ElementId::from_ifc_guid("3vB2YO$MX4xv5uCqZZG05x").unwrap();
    assert_eq!(
        again
            .model
            .get(id)
            .unwrap()
            .ratings
            .as_deref()
            .map(|r| r.fire.as_str()),
        Some("REI 60")
    );
    let kept = again.model.ifc_data().get("3vB2YO$MX4xv5uCqZZG05x");
    assert!(kept.is_none_or(|w| w
        .psets
        .iter()
        .all(|s| s.props.iter().all(|p| p.name != "FireRating"))));
}

#[test]
fn a_types_own_set_goes_back_on_the_type() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/bench/carried-data.ifc"
    );
    let r = import_ifc(&std::fs::read_to_string(path).unwrap()).unwrap();
    let t = r
        .model
        .ifc_data()
        .get("3dhw8zATb5NeCLhoPphVzY")
        .expect("the wall type's record");
    assert_eq!(t.psets[0].name, "Acme_TypeData");
    let out = export_ifc4(&r.model, &ExportOptions::default());
    // the type lists the set in its HasPropertySets, not by a relationship
    let set = out
        .lines()
        .find(|l| l.contains("'Acme_TypeData'"))
        .expect("written");
    let set_no = set.split('=').next().unwrap();
    let ty = out
        .lines()
        .find(|l| l.contains("IFCWALLTYPE('3dhw8zATb5NeCLhoPphVzY'"))
        .unwrap();
    assert!(ty.contains(set_no), "{ty}");
    let again = import_ifc(&out).unwrap();
    assert_eq!(
        again
            .model
            .ifc_data()
            .get("3dhw8zATb5NeCLhoPphVzY")
            .map(|r| &r.psets),
        Some(&t.psets)
    );
}
