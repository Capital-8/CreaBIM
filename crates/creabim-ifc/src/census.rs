//! What an IFC file holds, counted so two files can be compared: the
//! source a model was imported from and the file CreaBIM wrote back.
//!
//! The census reads the STEP graph only ([`crate::step_read`]), so it
//! sees everything in the file, not just what the importer understands.
//! It is the measuring stick of `creabim ifc-check` (the IFC bench, stage 1
//! of the exchange roadmap) and of the "do no harm" tests: everything is
//! keyed by the GlobalId of the object that carries it, so a pset that
//! comes back on the same element counts as kept, wherever its record sits.

use crate::step_read::{StepFile, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Everything the bench measures in one file.
#[derive(Clone, Debug, Default)]
pub struct Census {
    pub schema: String,
    pub entities: usize,
    /// Every rooted object (anything with a GlobalId that is not a
    /// relationship or a property set): GlobalId → (class, name).
    pub objects: BTreeMap<String, (String, String)>,
    /// Property sets and quantity sets by holder: (GlobalId, set name).
    /// The holder is an occurrence or a type.
    pub psets: BTreeSet<(String, String)>,
    /// Each property or quantity: (holder, set, property) → its value.
    pub props: BTreeMap<(String, String, String), String>,
    /// Classification references: (GlobalId, "source:identification").
    pub classifications: BTreeSet<(String, String)>,
    /// Group memberships: (member GlobalId, "IFCGROUP 'name'").
    pub groups: BTreeSet<(String, String)>,
    /// Documents associated with objects: (GlobalId, document name).
    pub docs: BTreeSet<(String, String)>,
    /// Every IfcDocumentInformation: "SCOPE 'name'" (Bonsai's sheets and
    /// drawings carry the scope SHEET and DRAWING).
    pub doc_infos: BTreeSet<String>,
    /// Bonsai drawings: IfcAnnotation objects whose ObjectType is DRAWING.
    pub drawings: BTreeSet<String>,
    /// Objects with an associated material.
    pub materials: BTreeSet<String>,
    /// Occurrence → its type's name.
    pub types: BTreeMap<String, String>,
    /// Element → the name of the spatial structure that contains it.
    pub containment: BTreeMap<String, String>,
    /// (part, whole) from IfcRelAggregates and IfcRelNests.
    pub aggregates: BTreeSet<(String, String)>,
    /// (door or window, host wall or slab), through its opening.
    pub fills: BTreeSet<(String, String)>,
}

/// One compared measure: how many the source had, how many of those
/// came back (and, for values, came back the same), and what is new.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Measure {
    pub name: &'static str,
    pub source: usize,
    pub kept: usize,
    /// Of `kept`, how many came back with a different value or class.
    pub changed: usize,
    pub lost: usize,
    /// Only in the written file (CreaBIM's own psets, say).
    pub added: usize,
    /// A few of the lost keys, to say what went.
    pub lost_examples: Vec<String>,
    /// A few changed values, "key: before → after".
    pub changed_examples: Vec<String>,
}

impl Measure {
    /// The share of the source that came back, as a percentage; 100 when
    /// the source had none.
    pub fn kept_pct(&self) -> f64 {
        if self.source == 0 {
            100.0
        } else {
            100.0 * self.kept as f64 / self.source as f64
        }
    }
}

const EXAMPLES: usize = 5;

fn is_guid(v: &Value) -> Option<&str> {
    v.as_str().filter(|s| s.len() == 22)
}

fn is_pset_class(c: &str) -> bool {
    c == "IFCPROPERTYSET"
        || c == "IFCELEMENTQUANTITY"
        || c.ends_with("PROPERTIES")
        || c == "IFCPROPERTYSETDEFINITIONSET"
}

/// A value as text, for comparing: numbers to 6 significant digits,
/// typed wrappers (`IFCLABEL('x')`) by what they hold.
pub fn value_text(v: &Value) -> String {
    match v {
        Value::Null | Value::Derived => "$".into(),
        Value::Int(i) => i.to_string(),
        Value::Real(r) => {
            if *r == 0.0 {
                "0".into()
            } else {
                let s = format!("{:.6e}", r);
                // 1.500000e0 → 1.5e0
                let (m, e) = s.split_once('e').unwrap_or((&s, "0"));
                let m = m.trim_end_matches('0').trim_end_matches('.');
                format!("{m}e{e}")
            }
        }
        Value::Str(s) => s.clone(),
        Value::Enum(e) => format!(".{e}."),
        Value::Binary(b) => b.clone(),
        Value::Ref(r) => format!("#{r}"),
        Value::List(l) => {
            let parts: Vec<String> = l.iter().map(value_text).collect();
            format!("({})", parts.join(","))
        }
        Value::Typed(_, inner) => value_text(inner),
    }
}

impl Census {
    /// Count what `file` holds.
    pub fn of(file: &StepFile) -> Census {
        let mut c = Census {
            schema: file.schemas.first().cloned().unwrap_or_default(),
            entities: file.entities.len(),
            ..Default::default()
        };
        let guid_of = |id: u64| -> Option<String> {
            file.get(id)
                .and_then(|e| is_guid(e.arg(0)))
                .map(str::to_string)
        };
        let name_of = |id: u64| -> String {
            file.get(id)
                .map(|e| e.arg(2).as_str().unwrap_or("").to_string())
                .unwrap_or_default()
        };
        // a pset's (name, properties) and each property's value
        let read_set = |id: u64| -> Option<(String, Vec<(String, String)>)> {
            let e = file.get(id)?;
            if !is_pset_class(&e.class) {
                return None;
            }
            let name = match e.arg(2).as_str() {
                Some(n) => n.to_string(),
                // predefined sets (door lining...) have no name: their class
                None => e.class.clone(),
            };
            let list = if e.is("IFCELEMENTQUANTITY") {
                e.arg(5)
            } else {
                e.arg(4)
            };
            let mut props = vec![];
            for p in list.refs().filter_map(|r| file.get(r)) {
                let Some(pn) = p.arg(0).as_str() else {
                    continue;
                };
                let v = if p.class.starts_with("IFCQUANTITY") {
                    p.arg(3)
                } else {
                    p.arg(2)
                };
                props.push((pn.to_string(), value_text(v)));
            }
            Some((name, props))
        };
        let add_set = |c: &mut Census, holder: &str, set: u64| {
            if let Some((name, props)) = read_set(set) {
                for (pn, v) in props {
                    c.props.insert((holder.to_string(), name.clone(), pn), v);
                }
                c.psets.insert((holder.to_string(), name));
            }
        };

        let mut ids: Vec<&u64> = file.entities.keys().collect();
        ids.sort_unstable();
        for &id in ids {
            let e = &file.entities[&id];
            let cl = e.class.as_str();
            if cl.starts_with("IFCREL") {
                continue;
            }
            if cl == "IFCDOCUMENTINFORMATION" {
                let name = e.arg(1).as_str().unwrap_or("");
                let scope = e.arg(6).as_str().unwrap_or("(no scope)");
                c.doc_infos.insert(format!("{scope} '{name}'"));
                continue;
            }
            let Some(g) = is_guid(e.arg(0)) else { continue };
            if is_pset_class(cl) {
                continue;
            }
            c.objects
                .insert(g.to_string(), (cl.to_string(), name_of(id)));
            if cl == "IFCANNOTATION" && e.arg(4).as_str() == Some("DRAWING") {
                c.drawings.insert(g.to_string());
            }
            // a type's own property sets (IfcTypeObject.HasPropertySets)
            if cl.ends_with("TYPE") || cl.ends_with("STYLE") {
                for set in e.arg(5).refs() {
                    add_set(&mut c, g, set);
                }
            }
        }

        // openings → their host, for the fills
        let mut opening_host: HashMap<u64, u64> = HashMap::new();
        for e in file.entities.values() {
            if e.is("IFCRELVOIDSELEMENT") {
                if let (Some(h), Some(o)) = (e.arg(4).as_ref(), e.arg(5).as_ref()) {
                    opening_host.insert(o, h);
                }
            }
        }

        let refs_or_one = |v: &Value| -> Vec<u64> {
            match v {
                Value::Ref(r) => vec![*r],
                _ => v.refs().collect(),
            }
        };
        for e in file.entities.values() {
            match e.class.as_str() {
                "IFCRELDEFINESBYPROPERTIES" => {
                    let sets = refs_or_one(e.arg(5));
                    for o in e.arg(4).refs().filter_map(guid_of) {
                        for &s in &sets {
                            // IFC4 allows a set of sets in one record
                            match file.get(s) {
                                Some(d) if d.is("IFCPROPERTYSETDEFINITIONSET") => {
                                    for s2 in d.arg(0).refs() {
                                        add_set(&mut c, &o, s2);
                                    }
                                }
                                _ => add_set(&mut c, &o, s),
                            }
                        }
                    }
                }
                "IFCRELDEFINESBYTYPE" => {
                    let t = e.arg(5).as_ref().map(name_of).unwrap_or_default();
                    for o in e.arg(4).refs().filter_map(guid_of) {
                        c.types.insert(o, t.clone());
                    }
                }
                "IFCRELASSOCIATESCLASSIFICATION" => {
                    let key = e
                        .arg(5)
                        .as_ref()
                        .and_then(|r| file.get(r))
                        .map(|r| classification_key(file, r))
                        .unwrap_or_default();
                    for o in e.arg(4).refs().filter_map(guid_of) {
                        c.classifications.insert((o, key.clone()));
                    }
                }
                "IFCRELASSIGNSTOGROUP" | "IFCRELASSIGNSTOGROUPBYFACTOR" => {
                    let key = e
                        .arg(6)
                        .as_ref()
                        .and_then(|r| file.get(r))
                        .map(|g| format!("{} '{}'", g.class, g.arg(2).as_str().unwrap_or("")))
                        .unwrap_or_default();
                    for o in e.arg(4).refs().filter_map(guid_of) {
                        c.groups.insert((o, key.clone()));
                    }
                }
                "IFCRELASSOCIATESDOCUMENT" => {
                    let key = e
                        .arg(5)
                        .as_ref()
                        .and_then(|r| file.get(r))
                        .map(|d| document_key(file, d))
                        .unwrap_or_default();
                    for o in e.arg(4).refs().filter_map(guid_of) {
                        c.docs.insert((o, key.clone()));
                    }
                }
                "IFCRELASSOCIATESMATERIAL" => {
                    for o in e.arg(4).refs().filter_map(guid_of) {
                        c.materials.insert(o);
                    }
                }
                "IFCRELCONTAINEDINSPATIALSTRUCTURE" => {
                    let s = e.arg(5).as_ref().map(name_of).unwrap_or_default();
                    for o in e.arg(4).refs().filter_map(guid_of) {
                        c.containment.insert(o, s.clone());
                    }
                }
                "IFCRELAGGREGATES" | "IFCRELNESTS" => {
                    if let Some(whole) = e.arg(4).as_ref().and_then(guid_of) {
                        for part in e.arg(5).refs().filter_map(guid_of) {
                            c.aggregates.insert((part, whole.clone()));
                        }
                    }
                }
                "IFCRELFILLSELEMENT" => {
                    let host = e
                        .arg(4)
                        .as_ref()
                        .and_then(|o| opening_host.get(&o).copied())
                        .and_then(guid_of);
                    if let (Some(h), Some(f)) = (host, e.arg(5).as_ref().and_then(guid_of)) {
                        c.fills.insert((f, h));
                    }
                }
                _ => {}
            }
        }
        c
    }

    /// Compare a source file's census with the one of the file written
    /// from it.
    pub fn compare(&self, out: &Census) -> Vec<Measure> {
        let objects_class: BTreeMap<String, String> = self
            .objects
            .iter()
            .map(|(g, (cl, _))| (g.clone(), cl.clone()))
            .collect();
        let out_class: BTreeMap<String, String> = out
            .objects
            .iter()
            .map(|(g, (cl, _))| (g.clone(), cl.clone()))
            .collect();
        let bonsai = |s: &BTreeSet<(String, String)>| -> BTreeSet<(String, String)> {
            s.iter()
                .filter(|(_, n)| n.starts_with("BBIM_"))
                .cloned()
                .collect()
        };
        let sheets = |s: &BTreeSet<String>| -> BTreeSet<String> {
            s.iter()
                .filter(|n| n.starts_with("SHEET "))
                .cloned()
                .collect()
        };
        let set = |s: &BTreeSet<(String, String)>| -> BTreeSet<String> {
            s.iter().map(|(a, b)| format!("{a} {b}")).collect()
        };
        let one = |s: &BTreeSet<String>| s.clone();
        vec![
            compare_map("objects (GlobalId, class)", &objects_class, &out_class),
            compare_set("property sets", &set(&self.psets), &set(&out.psets)),
            compare_set(
                "BBIM_ property sets",
                &set(&bonsai(&self.psets)),
                &set(&bonsai(&out.psets)),
            ),
            compare_map(
                "properties (value)",
                &keyed(&self.props),
                &keyed(&out.props),
            ),
            compare_set(
                "classification refs",
                &set(&self.classifications),
                &set(&out.classifications),
            ),
            compare_set("group memberships", &set(&self.groups), &set(&out.groups)),
            compare_set("documents on objects", &set(&self.docs), &set(&out.docs)),
            compare_set(
                "document records",
                &one(&self.doc_infos),
                &one(&out.doc_infos),
            ),
            compare_set(
                "Bonsai sheets",
                &sheets(&self.doc_infos),
                &sheets(&out.doc_infos),
            ),
            compare_set("Bonsai drawings", &one(&self.drawings), &one(&out.drawings)),
            compare_set(
                "materials on objects",
                &one(&self.materials),
                &one(&out.materials),
            ),
            compare_map("type of each object", &self.types, &out.types),
            compare_map(
                "storey of each element",
                &self.containment,
                &out.containment,
            ),
            compare_set(
                "parts of wholes",
                &set(&self.aggregates),
                &set(&out.aggregates),
            ),
            compare_set(
                "doors and windows in hosts",
                &set(&self.fills),
                &set(&out.fills),
            ),
        ]
    }

    /// Objects by class, sorted by class.
    pub fn classes(&self) -> BTreeMap<String, usize> {
        let mut m = BTreeMap::new();
        for (cl, _) in self.objects.values() {
            *m.entry(cl.clone()).or_insert(0) += 1;
        }
        m
    }
}

fn keyed(m: &BTreeMap<(String, String, String), String>) -> BTreeMap<String, String> {
    m.iter()
        .map(|((g, s, p), v)| (format!("{g} {s}.{p}"), v.clone()))
        .collect()
}

fn classification_key(file: &StepFile, r: &crate::step_read::Entity) -> String {
    // IFC4 IfcClassificationReference: Location, Identification, Name,
    // ReferencedSource; IFC2X3 has ItemReference where IFC4 has
    // Identification. An IfcClassification itself names the system.
    if r.is("IFCCLASSIFICATION") {
        let n = r.arg(3).as_str().or(r.arg(0).as_str()).unwrap_or("");
        return format!("{n}:");
    }
    let ident = r.arg(1).as_str().or(r.arg(2).as_str()).unwrap_or("");
    let source = file
        .deref(r.arg(3))
        .map(|s| {
            if s.is("IFCCLASSIFICATION") {
                s.arg(3).as_str().unwrap_or("").to_string()
            } else {
                s.arg(2).as_str().unwrap_or("").to_string()
            }
        })
        .unwrap_or_default();
    format!("{source}:{ident}")
}

fn document_key(file: &StepFile, d: &crate::step_read::Entity) -> String {
    if d.is("IFCDOCUMENTINFORMATION") {
        return d.arg(1).as_str().unwrap_or("").to_string();
    }
    // IfcDocumentReference: Location, Identification, Name, Description,
    // ReferencedDocument
    let own = d
        .arg(2)
        .as_str()
        .or(d.arg(1).as_str())
        .or(d.arg(0).as_str());
    match own {
        Some(n) => n.to_string(),
        None => file
            .deref(d.arg(4))
            .and_then(|i| i.arg(1).as_str())
            .unwrap_or("")
            .to_string(),
    }
}

fn compare_set(name: &'static str, a: &BTreeSet<String>, b: &BTreeSet<String>) -> Measure {
    let kept = a.intersection(b).count();
    Measure {
        name,
        source: a.len(),
        kept,
        changed: 0,
        lost: a.len() - kept,
        added: b.len() - kept,
        lost_examples: a.difference(b).take(EXAMPLES).cloned().collect(),
        changed_examples: vec![],
    }
}

fn compare_map(
    name: &'static str,
    a: &BTreeMap<String, String>,
    b: &BTreeMap<String, String>,
) -> Measure {
    let mut m = Measure {
        name,
        source: a.len(),
        ..Default::default()
    };
    for (k, v) in a {
        match b.get(k) {
            Some(w) => {
                m.kept += 1;
                if w != v {
                    m.changed += 1;
                    if m.changed_examples.len() < EXAMPLES {
                        m.changed_examples.push(format!("{k}: {v} → {w}"));
                    }
                }
            }
            None => {
                m.lost += 1;
                if m.lost_examples.len() < EXAMPLES {
                    m.lost_examples.push(k.clone());
                }
            }
        }
    }
    m.added = b.keys().filter(|k| !a.contains_key(*k)).count();
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::step_read::parse;

    const FILE: &str = "ISO-10303-21;HEADER;FILE_SCHEMA(('IFC4'));ENDSEC;DATA;
#1=IFCWALL('0aaaaaaaaaaaaaaaaaaaaa',$,'W1',$,$,$,$,$,$);
#2=IFCPROPERTYSINGLEVALUE('FireRating',$,IFCLABEL('EI60'),$);
#3=IFCPROPERTYSET('0bbbbbbbbbbbbbbbbbbbbb',$,'Acme_Wall',$,(#2));
#4=IFCRELDEFINESBYPROPERTIES('0ccccccccccccccccccccc',$,$,$,(#1),#3);
#5=IFCCLASSIFICATION('CSI',$,$,'Uniclass',$,$,$);
#6=IFCCLASSIFICATIONREFERENCE($,'EF_25_10',$,#5,$,$);
#7=IFCRELASSOCIATESCLASSIFICATION('0ddddddddddddddddddddd',$,$,$,(#1),#6);
#8=IFCGROUP('0eeeeeeeeeeeeeeeeeeeee',$,'Fire walls',$,$);
#9=IFCRELASSIGNSTOGROUP('0fffffffffffffffffffff',$,$,$,(#1),$,#8);
#10=IFCDOCUMENTINFORMATION('A101','Ground plan',$,$,$,$,'SHEET',$,$,$,$,$,$,$,$,$,$);
#11=IFCANNOTATION('0ggggggggggggggggggggg',$,'PLAN',$,'DRAWING',$,$);
ENDSEC;END-ISO-10303-21;";

    #[test]
    fn counts_what_the_importer_leaves_behind() {
        let c = Census::of(&parse(FILE).unwrap());
        let w = "0aaaaaaaaaaaaaaaaaaaaa".to_string();
        assert!(c.psets.contains(&(w.clone(), "Acme_Wall".into())));
        assert_eq!(
            c.props[&(w.clone(), "Acme_Wall".into(), "FireRating".into())],
            "EI60"
        );
        assert!(c
            .classifications
            .contains(&(w.clone(), "Uniclass:EF_25_10".into())));
        assert!(c
            .groups
            .contains(&(w.clone(), "IFCGROUP 'Fire walls'".into())));
        assert!(c.doc_infos.contains("SHEET 'Ground plan'"));
        assert_eq!(c.drawings.len(), 1);
        // the pset itself is not an object; the group and annotation are
        assert_eq!(c.objects.len(), 3);
    }

    #[test]
    fn a_file_compared_with_itself_loses_nothing() {
        let c = Census::of(&parse(FILE).unwrap());
        for m in c.compare(&c) {
            assert_eq!((m.lost, m.changed, m.added), (0, 0, 0), "{}", m.name);
        }
    }

    #[test]
    fn a_file_compared_with_an_empty_one_loses_everything() {
        let c = Census::of(&parse(FILE).unwrap());
        let m = c.compare(&Census::default());
        let psets = m.iter().find(|m| m.name == "property sets").unwrap();
        assert_eq!((psets.source, psets.lost), (1, 1));
        let sheets = m.iter().find(|m| m.name == "Bonsai sheets").unwrap();
        assert_eq!(sheets.lost, 1);
    }
}
