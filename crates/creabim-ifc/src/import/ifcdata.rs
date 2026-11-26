//! The property sets and classification references a file gives its
//! objects, kept beside the model so they go back out (owner 2026-11-23:
//! "Side record"). See [`creabim_core::ifcdata`].
//!
//! Kept: property sets of single and enumerated values, on the objects that
//! came in; classification references with their system. Left to CreaBIM:
//! its own sets (`CreaBIM_*`), which it writes from the model, and
//! quantities, which it works out. Bounded, list, table and complex
//! properties are counted as not kept.

use crate::step_read::{Entity, StepFile, Value};
use creabim_core::ifcdata::{ClassRef, IfcData, Prop, Pset};
use creabim_core::Model;
use std::collections::HashSet;

/// What was kept, and what was left, for the fidelity line.
#[derive(Debug, Default, PartialEq)]
pub struct Kept {
    pub sets: usize,
    pub props: usize,
    pub classes: usize,
    /// Properties of a kind not kept (bounded, list, table, complex...).
    pub skipped_props: usize,
    /// Property sets and classification references given to objects in
    /// the file, kept or not (on what didn't come in, or CreaBIM's own).
    pub sets_in_file: usize,
    pub classes_in_file: usize,
}

/// Read every property set and classification reference given to an
/// object of `model` (by the GlobalId it was read with).
pub fn read(file: &StepFile, model: &Model) -> (IfcData, Kept) {
    let wanted: HashSet<String> = model.iter().map(|e| e.id.ifc_guid()).collect();
    let mut data = IfcData::default();
    let mut kept = Kept::default();
    let guid = |r: u64| {
        file.get(r)
            .and_then(|e| e.arg(0).as_str())
            .filter(|g| wanted.contains(*g))
    };
    for e in file.entities.values() {
        match e.class.as_str() {
            "IFCRELDEFINESBYPROPERTIES" => {
                let sets: Vec<u64> = match e.arg(5) {
                    Value::Ref(r) => match file.get(*r) {
                        // IFC4: a set of sets in one record
                        Some(d) if d.is("IFCPROPERTYSETDEFINITIONSET") => d.arg(0).refs().collect(),
                        _ => vec![*r],
                    },
                    v => v.refs().collect(),
                };
                let sets: Vec<&Entity> = sets
                    .into_iter()
                    .filter_map(|s| file.get(s))
                    .filter(|s| s.is("IFCPROPERTYSET"))
                    .collect();
                kept.sets_in_file += sets.len() * e.arg(4).refs().count();
                let sets: Vec<Pset> = sets
                    .into_iter()
                    .filter_map(|s| pset(file, s, &mut kept))
                    .collect();
                if sets.is_empty() {
                    continue;
                }
                for o in e.arg(4).refs().filter_map(guid) {
                    let r = data.by_guid.entry(o.to_string()).or_default();
                    for s in &sets {
                        kept.sets += 1;
                        kept.props += s.props.len();
                        r.psets.push(s.clone());
                    }
                }
            }
            "IFCRELASSOCIATESCLASSIFICATION" => {
                kept.classes_in_file += e.arg(4).refs().count();
                let Some(c) = e
                    .arg(5)
                    .as_ref()
                    .and_then(|r| file.get(r))
                    .and_then(|r| class_ref(file, r))
                else {
                    continue;
                };
                for o in e.arg(4).refs().filter_map(guid) {
                    let r = data.by_guid.entry(o.to_string()).or_default();
                    if !r.classes.contains(&c) {
                        kept.classes += 1;
                        r.classes.push(c.clone());
                    }
                }
            }
            // a type's own sets (IfcTypeObject.HasPropertySets)
            c if c.ends_with("TYPE") || c.ends_with("STYLE") => {
                let sets: Vec<&Entity> = e
                    .arg(5)
                    .refs()
                    .filter_map(|s| file.get(s))
                    .filter(|s| s.is("IFCPROPERTYSET"))
                    .collect();
                if sets.is_empty() {
                    continue;
                }
                kept.sets_in_file += sets.len();
                let Some(g) = e.arg(0).as_str().filter(|g| wanted.contains(*g)) else {
                    continue;
                };
                let sets: Vec<Pset> = sets
                    .into_iter()
                    .filter_map(|s| pset(file, s, &mut kept))
                    .collect();
                for s in sets {
                    kept.sets += 1;
                    kept.props += s.props.len();
                    data.by_guid.entry(g.to_string()).or_default().psets.push(s);
                }
            }
            _ => {}
        }
    }
    data.by_guid.retain(|_, r| !r.is_empty());
    (data, kept)
}

/// IfcPropertySet: GlobalId, OwnerHistory, Name, Description, HasProperties.
fn pset(file: &StepFile, s: &Entity, kept: &mut Kept) -> Option<Pset> {
    let name = s.arg(2).as_str()?.to_string();
    if name.starts_with("CreaBIM_") {
        return None;
    }
    let mut props = Vec::new();
    for p in s.arg(4).refs().filter_map(|r| file.get(r)) {
        let pname = p.arg(0).as_str().unwrap_or("").to_string();
        match p.class.as_str() {
            // Name, Description, NominalValue, Unit
            "IFCPROPERTYSINGLEVALUE" => props.push(Prop {
                name: pname,
                value: step_text(p.arg(2)),
                kind: String::new(),
            }),
            // Name, Description, EnumerationValues, EnumerationReference
            "IFCPROPERTYENUMERATEDVALUE" => props.push(Prop {
                name: pname,
                value: step_text(p.arg(2)),
                kind: "enumerated".into(),
            }),
            _ => kept.skipped_props += 1,
        }
    }
    Some(Pset {
        name,
        description: s.arg(3).as_str().map(str::to_string),
        props,
    })
}

/// IfcClassificationReference: Location, Identification (IFC2X3
/// ItemReference), Name, ReferencedSource; an IfcClassification given
/// directly names only the system.
fn class_ref(file: &StepFile, r: &Entity) -> Option<ClassRef> {
    let s = |v: &Value| v.as_str().map(str::to_string);
    if r.is("IFCCLASSIFICATION") {
        // Source, Edition, EditionDate, Name
        return Some(ClassRef {
            code: None,
            name: None,
            location: None,
            system: s(r.arg(3)),
            edition: s(r.arg(1)),
            source: s(r.arg(0)),
        });
    }
    if !r.is("IFCCLASSIFICATIONREFERENCE") {
        return None;
    }
    let mut c = ClassRef {
        code: s(r.arg(1)),
        name: s(r.arg(2)),
        location: s(r.arg(0)),
        system: None,
        edition: None,
        source: None,
    };
    // the system: an IfcClassification, or a parent reference's system
    let mut src = file.deref(r.arg(3));
    for _ in 0..8 {
        match src {
            Some(x) if x.is("IFCCLASSIFICATION") => {
                c.system = s(x.arg(3));
                c.edition = s(x.arg(1));
                c.source = s(x.arg(0));
                break;
            }
            Some(x) if x.is("IFCCLASSIFICATIONREFERENCE") => src = file.deref(x.arg(3)),
            _ => break,
        }
    }
    Some(c)
}

/// A value back as STEP text: what the file wrote, with references (a unit,
/// a table) left out as `$`, which nothing here can carry.
pub fn step_text(v: &Value) -> String {
    match v {
        Value::Null | Value::Ref(_) => "$".into(),
        Value::Derived => "*".into(),
        Value::Int(i) => i.to_string(),
        Value::Real(r) => real(*r),
        Value::Str(s) => crate::step::string(s),
        Value::Enum(e) => format!(".{e}."),
        Value::Binary(b) => format!("\"{b}\""),
        Value::List(l) => format!(
            "({})",
            l.iter().map(step_text).collect::<Vec<_>>().join(",")
        ),
        Value::Typed(t, inner) => match inner.as_ref() {
            // several parameters come as a list; one is written bare
            Value::List(l) => format!(
                "{t}({})",
                l.iter().map(step_text).collect::<Vec<_>>().join(",")
            ),
            one => format!("{t}({})", step_text(one)),
        },
    }
}

/// A STEP real: always with its point ("2.", "1.5E-05").
fn real(r: f64) -> String {
    if !r.is_finite() {
        return "0.".into();
    }
    let s = format!("{r:?}").to_uppercase();
    match s.split_once('E') {
        Some((m, e)) if !m.contains('.') => format!("{m}.E{e}"),
        Some(_) => s,
        None if s.contains('.') => s,
        None => format!("{s}."),
    }
}

/// The common properties CreaBIM keeps as its own fields (Common
/// properties REV00, decision 5): read into the elements, then left out of
/// the kept sets so they are written from the model only.
pub const COMMON_PROPS: &[&str] = &[
    "IsExternal",
    "LoadBearing",
    "FireRating",
    "AcousticRating",
    "SecurityRating",
    "ThermalTransmittance",
    "HandicapAccessible",
];

/// Whether `set` is one of IFC's common sets (Pset_WallCommon...).
pub fn is_common_set(set: &str) -> bool {
    set.starts_with("Pset_") && set.ends_with("Common")
}

/// Each object's common properties as the file gives them, by GlobalId.
pub fn common_ratings(
    file: &StepFile,
) -> std::collections::HashMap<String, creabim_core::construction::Ratings> {
    use creabim_core::construction::Ratings;
    let mut out: std::collections::HashMap<String, Ratings> = std::collections::HashMap::new();
    let flag = |v: &Value| -> Option<bool> {
        let inner = match v {
            Value::Typed(_, x) => x.as_ref(),
            x => x,
        };
        match inner.as_enum()? {
            "T" => Some(true),
            "F" => Some(false),
            _ => None,
        }
    };
    for e in file
        .entities
        .values()
        .filter(|e| e.is("IFCRELDEFINESBYPROPERTIES"))
    {
        let Some(set) = e
            .arg(5)
            .as_ref()
            .and_then(|s| file.get(s))
            .filter(|s| s.is("IFCPROPERTYSET"))
        else {
            continue;
        };
        if !set.arg(2).as_str().is_some_and(is_common_set) {
            continue;
        }
        let mut r = Ratings::default();
        for p in set
            .arg(4)
            .refs()
            .filter_map(|x| file.get(x))
            .filter(|p| p.is("IFCPROPERTYSINGLEVALUE"))
        {
            let v = p.arg(2);
            match p.arg(0).as_str().unwrap_or("") {
                "IsExternal" => r.external = flag(v),
                "LoadBearing" => r.load_bearing = flag(v),
                "FireRating" => r.fire = v.as_str().unwrap_or("").to_string(),
                "AcousticRating" => r.acoustic = v.as_str().unwrap_or("").to_string(),
                "SecurityRating" => r.security = v.as_str().unwrap_or("").to_string(),
                "ThermalTransmittance" => {
                    r.u_value = v.as_f64().filter(|u| u.is_finite() && *u > 0.0)
                }
                "HandicapAccessible" => r.accessible = flag(v) == Some(true),
                _ => {}
            }
        }
        if r.is_none() {
            continue;
        }
        for o in e
            .arg(4)
            .refs()
            .filter_map(|x| file.get(x))
            .filter_map(|o| o.arg(0).as_str())
        {
            out.insert(o.to_string(), r.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::step_read::parse;

    #[test]
    fn values_go_back_as_the_file_wrote_them() {
        let f = parse(
            "ISO-10303-21;HEADER;FILE_SCHEMA(('IFC4'));ENDSEC;DATA;\n\
             #1=IFCPROPERTYSINGLEVALUE('a',$,IFCLENGTHMEASURE(2.),$);\n\
             #2=IFCPROPERTYSINGLEVALUE('b',$,IFCBOOLEAN(.T.),$);\n\
             #3=IFCPROPERTYSINGLEVALUE('c',$,IFCLABEL('Ba\\X2\\00E9\\X0\\ton'),$);\n\
             #4=IFCPROPERTYSINGLEVALUE('d',$,IFCREAL(1.5E-05),$);\n\
             ENDSEC;END-ISO-10303-21;",
        )
        .unwrap();
        let v = |i| step_text(f.get(i).unwrap().arg(2));
        assert_eq!(v(1), "IFCLENGTHMEASURE(2.0)");
        assert_eq!(v(2), "IFCBOOLEAN(.T.)");
        assert_eq!(v(3), "IFCLABEL('Ba\\X2\\00E9\\X0\\ton')");
        assert_eq!(v(4), "IFCREAL(1.5E-5)");
    }
}
