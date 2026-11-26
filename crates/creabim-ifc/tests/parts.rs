//! Extrusions, sweeps, voids, screen walls and site elements in IFC: the
//! classes they export as, voids as IfcOpeningElements in what they cut,
//! and a native round trip through IFCX.

use creabim_core::garden::garden_house;
use creabim_core::{Element, ElementKind, Model};
use creabim_ifc::{export_ifc4, export_ifcx, import_ifc, import_ifcx, ExportOptions};

fn natives(m: &Model) -> Vec<Element> {
    let mut v: Vec<Element> = m
        .iter()
        .filter(|e| matches!(e.kind, ElementKind::Part(_) | ElementKind::Site(_)))
        .cloned()
        .collect();
    v.sort_by_key(|e| e.id.0);
    v
}

fn count(text: &str, entity: &str) -> usize {
    text.lines().filter(|l| l.contains(entity)).count()
}

#[test]
fn parts_and_site_export_as_their_classes() {
    let m = garden_house();
    let text = export_ifc4(&m, &ExportOptions::default());
    // the stack as the class picked for it, the hearth as a proxy
    assert!(text
        .lines()
        .any(|l| l.contains("IFCCHIMNEY(") && l.contains("'Chimney stack'")));
    assert!(text
        .lines()
        .any(|l| l.contains("IFCBUILDINGELEMENTPROXY(") && l.contains("'Hearth'")));
    assert!(text
        .lines()
        .any(|l| l.contains("IFCCOVERING(") && l.contains("'Coping'")));
    // terrain, plants and hard landscape
    assert!(text
        .lines()
        .any(|l| l.contains("IFCGEOGRAPHICELEMENT(") && l.contains(".TERRAIN.")));
    assert!(text
        .lines()
        .any(|l| l.contains("IFCGEOGRAPHICELEMENT(") && l.contains("'Oak'")));
    assert!(text
        .lines()
        .any(|l| l.contains("IFCCIVILELEMENT(") && l.contains("'Patio'")));
    // the firebox is no element of its own: one opening in each thing it cuts
    assert!(!text.contains("'Firebox'"));
    let firebox_openings = text
        .lines()
        .filter(|l| l.contains("IFCOPENINGELEMENT(") && l.contains("'Firebox in "))
        .count();
    assert_eq!(firebox_openings, 2);
    assert!(count(&text, "IFCRELVOIDSELEMENT(") >= 2);
}

#[test]
fn the_step_file_reads_back() {
    let m = garden_house();
    let r = import_ifc(&export_ifc4(&m, &ExportOptions::default())).unwrap();
    // without a native property set yet the parts come back as shapes,
    // but nothing is lost
    for name in [
        "Chimney stack",
        "Coping",
        "Ground",
        "Patio",
        "Oak",
        "Garden screen",
    ] {
        assert!(
            r.model.iter().any(|e| e.name == name),
            "{name}: {:?}",
            r.warnings
        );
    }
}

#[test]
fn parts_and_site_round_trip_natively_through_ifcx() {
    let m = garden_house();
    let r = import_ifcx(&export_ifcx(&m)).unwrap();
    let (before, after) = (natives(&m), natives(&r.model));
    assert_eq!(before.len(), after.len(), "{:?}", r.warnings);
    for (a, b) in before.iter().zip(&after) {
        assert_eq!(a.name, b.name);
        assert_eq!(a.kind, b.kind, "{} comes back as the same element", a.name);
    }
    // the coping still follows its wall, and the void still cuts
    let cuts = creabim_core::parts::void_cuts(&r.model);
    assert_eq!(cuts.len(), creabim_core::parts::void_cuts(&m).len());
    let screen = r.model.iter().find(|e| e.name == "Garden screen").unwrap();
    assert!(creabim_core::screen::is_screen(&r.model, screen.id));
}

#[test]
fn a_located_site_and_its_earthworks_are_written() {
    use creabim_core::{Pad, SiteForm, Transaction};
    let m = garden_house();
    // nothing worked into the ground, nowhere in the world: nothing said
    let plain = export_ifc4(&m, &ExportOptions::default());
    assert_eq!(count(&plain, "'CreaBIM_Earthworks'"), 0);
    let mut doc = creabim_core::Document::from_model(m);
    let mut tx = Transaction::new("site");
    // a pad dug half a metre into the terrain
    let before = doc
        .model()
        .iter()
        .find(|e| matches!(&e.kind, ElementKind::Site(s) if matches!(s.form, SiteForm::Terrain { .. })))
        .unwrap()
        .clone();
    let mut after = before.clone();
    let ElementKind::Site(s) = &mut after.kind else {
        panic!()
    };
    let SiteForm::Terrain {
        boundary,
        top,
        data,
        ..
    } = &mut s.form
    else {
        panic!()
    };
    let c = boundary.iter().copied().sum::<glam::DVec2>() / boundary.len() as f64;
    data.pads.push(Pad::new(
        creabim_core::polygon::rectangle(c - glam::DVec2::splat(1.0), c + glam::DVec2::splat(1.0)),
        *top - 0.5,
    ));
    tx = tx.replace(before, after);
    // and the project in Toronto
    let info = creabim_core::ProjectInfo {
        latitude: Some(43.6532),
        longitude: Some(-79.3832),
        elevation: 76.0,
        map: Some(creabim_core::MapPlace {
            crs: "epsg:32617".into(),
            easting: 630_100.0,
            northing: 4_834_600.5,
        }),
        ..Default::default()
    };
    doc.commit(tx.insert(Element::new("Project", ElementKind::Project(info))))
        .unwrap();
    let text = export_ifc4(doc.model(), &ExportOptions::default());
    let site = text.lines().find(|l| l.contains("IFCSITE(")).unwrap();
    assert!(site.contains("(43,39,11,520000)"), "{site}");
    assert!(site.contains("(-79,-22,-59,-520000)"), "{site}");
    assert!(site.contains(",76."), "{site}");
    // on the map: the origin's coordinates in the named system
    assert_eq!(count(&plain, "IFCMAPCONVERSION("), 0);
    assert!(text.contains("IFCPROJECTEDCRS('EPSG:32617',"), "no crs");
    let conv = text
        .lines()
        .find(|l| l.contains("IFCMAPCONVERSION("))
        .unwrap();
    assert!(conv.contains(",630100.,4834600.5,76.,1.,0.,$"), "{conv}");
    assert_eq!(count(&text, "'CreaBIM_Earthworks'"), 1);
    let cut = text.lines().find(|l| l.contains("'CutVolume'")).unwrap();
    // 2 m by 2 m by half a metre, and its banks
    let v: f64 = cut.split(',').nth(3).unwrap().parse().unwrap();
    assert!(v > 2.0 && v < 3.5, "{cut}");
    // and it still reads back
    assert!(import_ifc(&text).is_ok());
}
