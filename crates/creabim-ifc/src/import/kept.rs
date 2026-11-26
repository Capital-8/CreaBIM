//! The relationships and objects of a file that CreaBIM doesn't model,
//! kept to go back out (owner 2026-11-23, "Go ahead" on the side record):
//! the type of an element that came in as a mesh, the assembly it is part
//! of, the opening a mesh door fills, its material layer, profile or
//! constituent set, a Bonsai drawing and its sheet, the documents on an
//! element.
//!
//! Each relationship is kept with its ends: an object of the model by its
//! GlobalId, an object that did not come in (a type, an assembly, an
//! opening, a drawing, a group of drawings) as a kept entity with
//! everything it refers to. What the export writes itself (the project,
//! site, building, the representation contexts) is named, not copied; an
//! owner history is dropped. The export decides what goes back: CreaBIM's
//! own word first ([`crate::export`]).

use crate::step_read::{Entity, StepFile, Value};
use creabim_core::ifcdata::{KeptEntity, KeptRel};
use std::collections::{BTreeMap, HashSet};

/// Relationships kept.
const RELS: &[&str] = &[
    "IFCRELDEFINESBYTYPE",
    "IFCRELAGGREGATES",
    "IFCRELNESTS",
    "IFCRELVOIDSELEMENT",
    "IFCRELFILLSELEMENT",
    "IFCRELASSOCIATESMATERIAL",
    "IFCRELASSOCIATESDOCUMENT",
    "IFCRELCONTAINEDINSPATIALSTRUCTURE",
    "IFCRELASSIGNSTOGROUP",
    "IFCRELDEFINESBYPROPERTIES",
];

/// Where a relationship's ends are: (the relating end, the related end,
/// whether the related end is a list).
pub fn ends(class: &str) -> (usize, usize, bool) {
    match class {
        "IFCRELAGGREGATES" | "IFCRELNESTS" => (4, 5, true),
        "IFCRELVOIDSELEMENT" | "IFCRELFILLSELEMENT" => (4, 5, false),
        "IFCRELASSIGNSTOGROUP" => (6, 4, true),
        // DefinesByType, AssociatesMaterial/Document, Contained, DefinesByProperties
        _ => (5, 4, true),
    }
}

/// Content with a GlobalId that travels with what refers to it, not an
/// object in its own right.
fn is_content(class: &str) -> bool {
    class.starts_with("IFCPROPERTYSET")
        || class == "IFCELEMENTQUANTITY"
        || class.starts_with("IFCPROPERTYTEMPLATE")
}

/// Objects that did not come in but are kept, to carry the relationships
/// they are an end of.
fn keepable(class: &str) -> bool {
    class.ends_with("TYPE")
        || class.ends_with("STYLE")
            && !class.starts_with("IFCSURFACESTYLE")
            && class != "IFCPRESENTATIONSTYLE"
        || matches!(
            class,
            "IFCOPENINGELEMENT" | "IFCANNOTATION" | "IFCELEMENTASSEMBLY" | "IFCGROUP"
        )
}

/// An entity's GlobalId: 22 characters of IFC's base 64 in its first
/// argument (a material named "CASEWORK (PLACEHOLDER)" is 22 long too).
fn guid_of(e: &Entity) -> Option<&str> {
    e.arg(0).as_str().filter(|g| {
        g.len() == 22
            && g.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'$')
    })
}

/// What was kept, for the fidelity line.
#[derive(Debug, Default)]
pub struct Counts {
    pub rels: usize,
    pub types: usize,
    pub assemblies: usize,
    pub openings: usize,
    pub drawings: usize,
    pub documents: usize,
}

struct Keeper<'a> {
    file: &'a StepFile,
    model: &'a HashSet<String>,
    /// Groups the model keeps itself (their GlobalIds).
    groups: &'a HashSet<String>,
    two_x3: bool,
    kept: BTreeMap<u64, KeptEntity>,
    /// Entities being kept, against cycles.
    seen: HashSet<u64>,
    /// Kept content nothing kept refers to, written for its own sake.
    roots: Vec<u64>,
}

impl Keeper<'_> {
    /// The token for a reference to `id`, keeping what it needs.
    fn token(&mut self, id: u64) -> Option<String> {
        let e = self.file.get(id)?;
        let c = e.class.as_str();
        match c {
            "IFCOWNERHISTORY" => return Some("$".into()),
            "IFCPROJECT" => return Some("@project".into()),
            "IFCSITE" => return Some("@site".into()),
            "IFCBUILDING" => return Some("@building".into()),
            "IFCGEOMETRICREPRESENTATIONCONTEXT" => return Some("@model".into()),
            "IFCGEOMETRICREPRESENTATIONSUBCONTEXT" => {
                return Some(
                    match e.arg(0).as_str().unwrap_or("") {
                        "Axis" => "@axis",
                        "Annotation" | "Plan" | "FootPrint" => "@plan",
                        _ => "@body",
                    }
                    .into(),
                )
            }
            _ => {}
        }
        if !is_content(c) && !c.starts_with("IFCREL") {
            if let Some(g) = guid_of(e) {
                if self.model.contains(g) {
                    return Some(format!("@{g}"));
                }
                if c == "IFCGROUP" && self.groups.contains(g) {
                    return None;
                }
                if !keepable(c) {
                    return None;
                }
            }
        }
        self.keep(id).then(|| format!("#{id}"))
    }

    /// Keep entity `id` and what it refers to. False when it can't be.
    fn keep(&mut self, id: u64) -> bool {
        if self.kept.contains_key(&id) || self.seen.contains(&id) {
            return true;
        }
        self.seen.insert(id);
        let Some(e) = self.file.get(id) else {
            return false;
        };
        let mut args: Vec<String> = Vec::with_capacity(e.args.len());
        for a in &e.args {
            args.push(self.text(a));
        }
        pad_to_ifc4(&e.class, &mut args, self.two_x3);
        let guid = (!is_content(&e.class))
            .then(|| guid_of(e).map(str::to_string))
            .flatten();
        self.kept.insert(
            id,
            KeptEntity {
                id,
                class: e.class.clone(),
                args: args.join(","),
                guid,
                root: false,
            },
        );
        true
    }

    /// A value as STEP text, its references as tokens; a reference to
    /// what can't be kept is `$` (or left out of a list).
    fn text(&mut self, v: &Value) -> String {
        match v {
            Value::Ref(r) => self.token(*r).unwrap_or_else(|| "$".into()),
            Value::List(l) => {
                let items: Vec<String> = l
                    .iter()
                    .filter_map(|x| match x {
                        Value::Ref(r) => self.token(*r),
                        other => Some(self.text(other)),
                    })
                    .collect();
                format!("({})", items.join(","))
            }
            Value::Typed(t, inner) => match inner.as_ref() {
                Value::List(l) => format!(
                    "{t}({})",
                    l.iter().map(|x| self.text(x)).collect::<Vec<_>>().join(",")
                ),
                one => format!("{t}({})", self.text(one)),
            },
            other => super::ifcdata::step_text(other),
        }
    }
}

/// IFC2X3 records that IFC4 grew: padded with `$` to IFC4's length.
fn pad_to_ifc4(class: &str, args: &mut Vec<String>, two_x3: bool) {
    if !two_x3 {
        return;
    }
    let n = match class {
        "IFCMATERIAL" | "IFCMATERIALLAYERSET" => 3,
        "IFCMATERIALLAYER" => 7,
        "IFCMATERIALLAYERSETUSAGE" => 5,
        "IFCOPENINGELEMENT" => 9,
        "IFCFURNITURETYPE" => 11,
        c if c.ends_with("TYPE") => 10,
        _ => return,
    };
    while args.len() < n {
        args.push("$".into());
    }
}

/// Keep the file's relationships that touch the model or a kept object,
/// with the objects and content they need. `model` holds the GlobalIds of
/// the model's elements, `groups` those of the groups it keeps itself.
pub fn read(
    file: &StepFile,
    model: &HashSet<String>,
    groups: &HashSet<String>,
) -> (Vec<KeptEntity>, Vec<KeptRel>, Counts) {
    let schema = file
        .schemas
        .first()
        .map(|s| s.to_ascii_uppercase())
        .unwrap_or_default();
    let mut k = Keeper {
        file,
        model,
        groups,
        two_x3: schema.starts_with("IFC2X"),
        kept: BTreeMap::new(),
        seen: HashSet::new(),
        roots: Vec::new(),
    };
    let mut ids: Vec<u64> = file.entities.keys().copied().collect();
    ids.sort_unstable();
    // every Bonsai drawing (an annotation of type DRAWING) is kept,
    // related or not
    for &id in &ids {
        let e = &file.entities[&id];
        if e.class == "IFCANNOTATION" && guid_of(e).is_some_and(|g| !model.contains(g)) {
            k.keep(id);
        }
    }
    let kept_object = |k: &Keeper, t: &str| {
        t.starts_with('#')
            && k.kept
                .get(&t[1..].parse::<u64>().unwrap_or(0))
                .is_some_and(|e| e.guid.is_some())
    };
    let mut rels = Vec::new();
    // first what relates the model's objects (and keeps the types,
    // assemblies, openings they need), then what only touches kept
    // objects (their containment, their property sets)
    let second = |c: &str| {
        matches!(
            c,
            "IFCRELCONTAINEDINSPATIALSTRUCTURE" | "IFCRELDEFINESBYPROPERTIES"
        )
    };
    let order: Vec<u64> = ids
        .iter()
        .copied()
        .filter(|id| !second(&file.entities[id].class))
        .chain(
            ids.iter()
                .copied()
                .filter(|id| second(&file.entities[id].class)),
        )
        .collect();
    for id in order {
        let e = &file.entities[&id];
        if !RELS.contains(&e.class.as_str()) {
            continue;
        }
        let class = e.class.as_str();
        // CreaBIM writes the model's own containment and its own groups;
        // property sets on the model's objects are kept on their records
        let (ia, ib, many) = ends(class);
        let relating_raw = e.arg(ia).as_ref();
        if class == "IFCRELASSIGNSTOGROUP" {
            let g = relating_raw.and_then(|r| file.get(r)).and_then(guid_of);
            if g.is_some_and(|g| groups.contains(g)) {
                continue;
            }
        }
        let related_raw: Vec<u64> = if many {
            e.arg(ib).refs().collect()
        } else {
            e.arg(ib).as_ref().into_iter().collect()
        };
        let Some(relating_raw) = relating_raw else {
            continue;
        };
        // the ends that are the model's, before keeping anything for this
        // the project, site and building are written again: anchored too
        let touches_model = |r: u64| {
            file.get(r).is_some_and(|e| {
                matches!(e.class.as_str(), "IFCPROJECT" | "IFCSITE" | "IFCBUILDING")
                    || guid_of(e).is_some_and(|g| model.contains(g))
            })
        };
        let only_kept_side = second(class);
        if only_kept_side
            && !related_raw
                .iter()
                .any(|r| k.kept.get(r).is_some_and(|x| x.guid.is_some()))
        {
            continue;
        }
        if !only_kept_side
            && !touches_model(relating_raw)
            && !related_raw.iter().any(|r| touches_model(*r))
        {
            // nothing of the model on either end: kept only for what is
            // kept anyway (a drawing's group, a type's own type...)
            let kept_end = |r: u64| k.kept.get(&r).is_some_and(|x| x.guid.is_some());
            if !kept_end(relating_raw) && !related_raw.iter().any(|r| kept_end(*r)) {
                continue;
            }
        }
        let Some(relating) = k.token(relating_raw) else {
            continue;
        };
        let mut related: Vec<String> = related_raw.iter().filter_map(|r| k.token(*r)).collect();
        if only_kept_side {
            related.retain(|t| kept_object(&k, t));
        }
        if related.is_empty() || relating == "$" {
            continue;
        }
        let extra = if class == "IFCRELASSIGNSTOGROUP" {
            k.text(e.arg(5))
        } else {
            String::new()
        };
        rels.push(KeptRel {
            class: class.to_string(),
            guid: guid_of(e).unwrap_or("").to_string(),
            name: e.arg(2).as_str().map(str::to_string),
            relating,
            related,
            extra,
        });
    }
    // every document record is kept, tied to anything or not (a sheet
    // may hang off nothing but its own layout and drawing files)
    for &id in &ids {
        if file.entities[&id].class == "IFCDOCUMENTINFORMATION" && !k.kept.contains_key(&id) {
            k.seen.remove(&id);
            k.keep(id);
            k.roots.push(id);
        }
    }
    // what refers to a kept document with nothing referring to it: a
    // Bonsai sheet's LAYOUT, TITLEBLOCK and SHEET files, a drawing's
    // document, the relationships between documents
    loop {
        let before = k.kept.len();
        for &id in &ids {
            let e = &file.entities[&id];
            let target = match e.class.as_str() {
                "IFCDOCUMENTREFERENCE" => e.arg(4).as_ref(),
                "IFCDOCUMENTINFORMATIONRELATIONSHIP" => e.arg(2).as_ref(),
                _ => None,
            };
            if target.is_some_and(|t| k.kept.contains_key(&t)) && !k.kept.contains_key(&id) {
                k.seen.remove(&id);
                k.keep(id);
                k.roots.push(id);
            }
        }
        if k.kept.len() == before {
            break;
        }
    }
    let mut c = Counts {
        rels: rels.len(),
        ..Counts::default()
    };
    for e in k.kept.values().filter(|e| e.guid.is_some()) {
        match e.class.as_str() {
            "IFCOPENINGELEMENT" => c.openings += 1,
            // a Bonsai drawing is an annotation of type DRAWING
            "IFCANNOTATION" if e.args.contains("'DRAWING'") => c.drawings += 1,
            "IFCELEMENTASSEMBLY" => c.assemblies += 1,
            cl if cl.ends_with("TYPE") || cl.ends_with("STYLE") => c.types += 1,
            _ => {}
        }
    }
    c.documents = k
        .kept
        .values()
        .filter(|e| e.class == "IFCDOCUMENTINFORMATION" || e.class == "IFCDOCUMENTREFERENCE")
        .count();
    // content kept for its own sake is written even with nothing
    // referring to it
    let roots: HashSet<u64> = k.roots.iter().copied().collect();
    let mut kept: Vec<KeptEntity> = k.kept.into_values().collect();
    for e in kept.iter_mut().filter(|e| roots.contains(&e.id)) {
        e.root = true;
    }
    (kept, rels, c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_as_long_as_a_global_id_is_not_one() {
        let e = Entity {
            class: "IFCMATERIAL".into(),
            args: vec![Value::Str("CASEWORK (PLACEHOLDER)".into())],
        };
        assert_eq!(guid_of(&e), None);
        let w = Entity {
            class: "IFCWALL".into(),
            args: vec![Value::Str("118CUWRkzBuA8zcQgsJeVP".into())],
        };
        assert_eq!(guid_of(&w), Some("118CUWRkzBuA8zcQgsJeVP"));
    }
}
