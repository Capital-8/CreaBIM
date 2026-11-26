//! Furniture and fixtures through IFC: object types as IfcFurnitureType,
//! IfcSanitaryTerminalType... with their parts as a mapped body and their
//! plan symbol as a mapped footprint; objects as their occurrences.

use creabim_core::objects::{self, SymbolItem};
use creabim_core::sample::furnished_house;
use creabim_core::{types, ElementKind, Model};
use creabim_ifc::{export_ifc4, import_ifc, ExportOptions};

/// (object name, type name, level, x, y, offset, turn) for every object, sorted.
fn placed(m: &Model) -> Vec<(String, String, String, i64, i64, i64, i64)> {
    let r = |v: f64| (v * 1000.0).round() as i64;
    let mut v: Vec<_> = m
        .iter()
        .filter_map(|e| {
            let ElementKind::Object(o) = &e.kind else {
                return None;
            };
            let (t, _) = types::type_of(m, e)?;
            let level = m.get(o.level)?.name.clone();
            Some((
                e.name.clone(),
                t.name.clone(),
                level,
                r(o.position.x),
                r(o.position.y),
                r(o.offset),
                r(o.rotation).rem_euclid(360_000),
            ))
        })
        .collect();
    v.sort();
    v
}

#[test]
fn furnished_house_round_trips_through_ifc() {
    let model = furnished_house();
    let before = placed(&model);
    assert!(before.len() >= 25, "{}", before.len());
    let ifc = export_ifc4(&model, &ExportOptions::default());
    for class in [
        "IFCFURNITURETYPE(",
        "IFCSANITARYTERMINALTYPE(",
        "IFCELECTRICAPPLIANCETYPE(",
        "IFCFURNITURE(",
        "IFCSANITARYTERMINAL(",
        "IFCELECTRICAPPLIANCE(",
        "IFCREPRESENTATIONMAP(",
        "IFCMAPPEDITEM(",
        "'FootPrint'",
        "IFCMATERIALCONSTITUENTSET(",
        ".WASHHANDBASIN.",
        ".SOFA.",
        ".BED.",
        // building services, with their types
        "IFCOUTLETTYPE(",
        "IFCOUTLET(",
        ".POWEROUTLET.",
        "IFCSWITCHINGDEVICETYPE(",
        ".TOGGLESWITCH.",
        "IFCLIGHTFIXTURETYPE(",
        "IFCLIGHTFIXTURE(",
        ".POINTSOURCE.",
        "IFCELECTRICDISTRIBUTIONBOARDTYPE(",
        ".CONSUMERUNIT.",
        "IFCSENSORTYPE(",
        ".SMOKESENSOR.",
        "IFCWASTETERMINALTYPE(",
        ".FLOORWASTE.",
        "IFCBOILERTYPE(",
        "IFCVALVETYPE(",
        ".FAUCET.",
        "IFCSPACEHEATERTYPE(",
        ".RADIATOR.",
        "IFCAIRTERMINALTYPE(",
        ".DIFFUSER.",
        ".GRILLE.",
        "IFCUNITARYEQUIPMENTTYPE(",
        ".SPLITSYSTEM.",
        "IFCFANTYPE(",
        "IFCFAN(",
    ] {
        assert!(ifc.contains(class), "{class}");
    }
    let back = import_ifc(&ifc).unwrap().model;
    // every object comes back where it was, of the same type
    assert_eq!(placed(&back), before);
    // and each type with its parts, their materials, its symbol and meaning
    for (id, name, t) in types::types(&model) {
        let creabim_core::TypeParams::Object(a) = &t.params else {
            continue;
        };
        let (_, _, tb) = types::types(&back)
            .into_iter()
            .find(|(i, _, _)| *i == id)
            .unwrap_or_else(|| panic!("{name} came back"));
        let creabim_core::TypeParams::Object(b) = &tb.params else {
            panic!("{name} is an object type");
        };
        assert_eq!(a.parts.len(), b.parts.len(), "{name}");
        assert_eq!(
            (a.category, a.mount, &a.ifc),
            (b.category, b.mount, &b.ifc),
            "{name}"
        );
        assert!((a.offset - b.offset).abs() < 1e-9, "{name}");
        assert_eq!(a.catalogue, b.catalogue, "{name}");
        assert!((a.size() - b.size()).length() < 1e-6, "{name}");
        for (pa, pb) in a.parts.iter().zip(&b.parts) {
            assert_eq!(pa.name, pb.name, "{name}");
            let mat = |m: &Model, id| m.get(id).map(|e| e.name.clone());
            assert_eq!(
                pa.material.and_then(|i| mat(&model, i)),
                pb.material.and_then(|i| mat(&back, i)),
                "{name} {}",
                pa.name
            );
        }
        let loops = |s: &[SymbolItem]| {
            s.iter()
                .filter(|i| matches!(i, SymbolItem::Loop(_)))
                .count()
        };
        assert_eq!(loops(&a.plan_symbol()), loops(&b.symbol), "{name}");
        assert_eq!(a.plan_symbol().len(), b.symbol.len(), "{name}");
    }
    // the materials keep their looks
    let ceramic = back
        .iter()
        .find(|e| e.name == "Sanitary ceramic, white")
        .expect("the basin's ceramic");
    let ElementKind::Material(m) = &ceramic.kind else {
        unreachable!()
    };
    assert!(m.color[0] > 0.9 && m.roughness < 0.3);
    // exporting again writes the same objects
    let again = export_ifc4(&back, &ExportOptions::default());
    assert_eq!(
        again.matches("IFCMAPPEDITEM(").count(),
        ifc.matches("IFCMAPPEDITEM(").count()
    );
    let _ = objects::IFC_CLASSES;
}

#[test]
fn lights_hang_from_the_ceiling_in_ifc_too() {
    let model = furnished_house();
    let ifc = export_ifc4(&model, &ExportOptions::default());
    let back = import_ifc(&ifc).unwrap().model;
    // each object stands at the same height in the file as in the model,
    // a pendant just under the first floor's slab
    let z = |m: &Model, name: &str| {
        let mut v: Vec<i64> = m
            .iter()
            .filter(|e| e.name.starts_with(name) && matches!(e.kind, ElementKind::Object(_)))
            .map(|e| (objects::base_z(m, e).unwrap() * 1000.0).round() as i64)
            .collect();
        v.sort();
        v
    };
    for name in [
        "Ceiling light",
        "Recessed downlight",
        "Double socket",
        "Smoke detector",
    ] {
        assert!(!z(&model, name).is_empty(), "{name}");
        assert_eq!(z(&model, name), z(&back, name), "{name}");
    }
    let pendant = back
        .iter()
        .find(|e| e.name.starts_with("Ceiling light") && matches!(e.kind, ElementKind::Object(_)))
        .unwrap();
    assert!(objects::hung(&back, pendant));
    let ElementKind::Object(o) = &pendant.kind else {
        unreachable!()
    };
    assert!(o.offset.abs() < 1e-9, "{}", o.offset);
}

/// A fitting's own light (Visuals REV02) goes out with its type and comes
/// back; a fitting with none still has none.
#[test]
fn a_fittings_own_light_round_trips_through_ifc() {
    use creabim_core::{Document, Light, Spread, Transaction, TypeParams};
    let mut doc = Document::from_model(furnished_house());
    let lights = |m: &Model| -> Vec<(String, Option<Light>)> {
        let mut v: Vec<_> = m
            .iter()
            .filter_map(|e| match &e.kind {
                ElementKind::ElementType(t) => match &t.params {
                    TypeParams::Object(o) if o.is_light_fitting() => {
                        Some((e.name.clone(), o.light.clone()))
                    }
                    _ => None,
                },
                _ => None,
            })
            .collect();
        v.sort_by(|a, b| a.0.cmp(&b.0));
        v
    };
    let before = lights(doc.model());
    assert!(
        before.len() >= 2 && before.iter().all(|(_, l)| l.is_none()),
        "{before:?}"
    );
    // the first fitting gets a light of its own
    let name = before[0].0.clone();
    let te = doc
        .model()
        .iter()
        .find(|e| e.name == name && matches!(e.kind, ElementKind::ElementType(_)))
        .unwrap()
        .clone();
    let mut after = te.clone();
    let own = Light {
        on: false,
        lumens: 2400.0,
        kelvin: 4000.0,
        beam: Spread::Narrow,
        group: "Kitchen downlights".into(),
    };
    if let ElementKind::ElementType(t) = &mut after.kind {
        if let TypeParams::Object(o) = &mut t.params {
            o.light = Some(own.clone());
        }
    }
    doc.commit(Transaction::new("light").replace(te, after))
        .unwrap();
    let ifc = export_ifc4(doc.model(), &ExportOptions::default());
    assert!(ifc.contains("'LightOutput'") && ifc.contains("Kitchen downlights"));
    let back = import_ifc(&ifc).expect("the file reads");
    let got = lights(&back.model);
    assert_eq!(got.len(), before.len());
    assert_eq!(got[0], (name, Some(own)));
    assert!(got[1..].iter().all(|(_, l)| l.is_none()), "{got:?}");
}
