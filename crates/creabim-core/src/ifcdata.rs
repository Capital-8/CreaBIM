//! IFC data CreaBIM doesn't model, kept so it goes back out (owner
//! 2026-11-23, "side record"): each object's property sets and
//! classification references, as the file wrote them, keyed by the
//! object's GlobalId.
//!
//! It sits beside the model, not in its elements: no element changes shape
//! for it, and nothing in CreaBIM reads it but the IFC export and the
//! Properties panel. It is read-only after the import, so the model's
//! copies (undo, export) share one copy of it. An element deleted here
//! simply isn't written, and its record with it.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One property of a set, with its value as the file wrote it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Prop {
    pub name: String,
    /// The value as STEP text, e.g. `IFCLABEL('Brick')`, `.T.`, or for a
    /// list of enumerated values `(IFCLABEL('A'),IFCLABEL('B'))`. Written
    /// back as it is.
    pub value: String,
    /// What kind of property: `single`, `enumerated` (`value` is the list).
    #[serde(default, skip_serializing_if = "is_single")]
    pub kind: String,
}

fn is_single(k: &str) -> bool {
    k.is_empty() || k == "single"
}

/// A property set as the file wrote it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pset {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub props: Vec<Prop>,
}

/// A classification reference: "EF_25_10" in Uniclass 2015, say.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClassRef {
    /// The code (Identification, or ItemReference in IFC2X3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    /// The system it belongs to: "Uniclass", "NL-SfB".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edition: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// What one object carried.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Record {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub psets: Vec<Pset>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub classes: Vec<ClassRef>,
}

impl Record {
    pub fn is_empty(&self) -> bool {
        self.psets.is_empty() && self.classes.is_empty()
    }

    pub fn props(&self) -> usize {
        self.psets.iter().map(|p| p.props.len()).sum()
    }
}

/// Every object's record, by GlobalId; and the relationships and objects
/// the file had that CreaBIM doesn't model (a wall's type when the wall
/// came in as a mesh, an assembly, the opening a mesh door fills, a
/// material layer set, a Bonsai drawing and its sheet), kept as the file
/// wrote them to go back out.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct IfcData {
    pub by_guid: BTreeMap<String, Record>,
    /// Entities kept as written, each with everything it refers to.
    pub kept: Vec<KeptEntity>,
    /// Relationships kept, their ends by GlobalId or kept entity.
    pub rels: Vec<KeptRel>,
    /// The GlobalIds of the file's project, site and building
    /// (`project`, `site`, `building`), which the export writes again, so
    /// what the file tied to them (Bonsai's sheets, to the project) finds
    /// them.
    pub anchors: BTreeMap<String, String>,
}

/// An entity kept as the file wrote it. In `args`, `#n` is kept entity
/// `n`, `@` + a GlobalId an object of the model, and `@project`, `@site`,
/// `@building`, `@model`, `@body`, `@axis`, `@plan` what the export writes
/// itself.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KeptEntity {
    pub id: u64,
    pub class: String,
    pub args: String,
    /// Its GlobalId, for an object (a type, a drawing): the export uses
    /// the model's own when it wrote one of that id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guid: Option<String>,
    /// Written for its own sake, though nothing kept refers to it (a
    /// Bonsai sheet's layout file, which refers to its sheet).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub root: bool,
}

/// A relationship kept: its class (`IFCRELDEFINESBYTYPE`...), GlobalId and
/// name, the one end it relates (`relating`) and the many (`related`),
/// each a token as in [`KeptEntity::args`]. `extra` is the rest of a
/// group assignment (its RelatedObjectsType).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KeptRel {
    pub class: String,
    pub guid: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub relating: String,
    pub related: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub extra: String,
}

impl IfcData {
    pub fn is_empty(&self) -> bool {
        self.by_guid.is_empty()
            && self.kept.is_empty()
            && self.rels.is_empty()
            && self.anchors.is_empty()
    }

    /// Kept entity `id`.
    pub fn entity(&self, token: &str) -> Option<&KeptEntity> {
        let id: u64 = token.strip_prefix('#')?.parse().ok()?;
        self.kept.iter().find(|k| k.id == id)
    }

    /// The name a kept object was written with (its third argument).
    pub fn name_of(&self, token: &str) -> Option<String> {
        let e = self.entity(token)?;
        let n = display(split_top(&e.args).get(2)?);
        (!n.is_empty()).then_some(n)
    }

    /// The relationships of `class` kept with `guid` among their related.
    pub fn rels_of<'a>(
        &'a self,
        guid: &'a str,
        class: &'a str,
    ) -> impl Iterator<Item = &'a KeptRel> + 'a {
        let token = format!("@{guid}");
        self.rels
            .iter()
            .filter(move |r| r.class == class && r.related.contains(&token))
    }

    /// The documents the file gave object `guid`, or the type the file
    /// gave it: each by its location and name (a reference's own name,
    /// else its document's, else its file's name).
    pub fn documents_of(&self, guid: &str) -> Vec<crate::docrefs::DocRef> {
        let mut holders = vec![format!("@{guid}")];
        holders.extend(
            self.rels_of(guid, "IFCRELDEFINESBYTYPE")
                .map(|r| r.relating.clone()),
        );
        let mut out: Vec<crate::docrefs::DocRef> = Vec::new();
        for r in self.rels.iter().filter(|r| {
            r.class == "IFCRELASSOCIATESDOCUMENT" && r.related.iter().any(|t| holders.contains(t))
        }) {
            let Some(d) = self.document(&r.relating) else {
                continue;
            };
            if !out.contains(&d) {
                out.push(d);
            }
        }
        out
    }

    /// A kept IfcDocumentReference (Location, Identification, Name,
    /// Description, ReferencedDocument) or IfcDocumentInformation
    /// (Identification, Name, ..., its references found by their
    /// ReferencedDocument) as a document.
    fn document(&self, token: &str) -> Option<crate::docrefs::DocRef> {
        let e = self.entity(token)?;
        let a = split_top(&e.args);
        let arg = |i: usize| a.get(i).map(|v| display(v)).unwrap_or_default();
        let (location, mut name) = match e.class.as_str() {
            "IFCDOCUMENTREFERENCE" => {
                let mut name = arg(2);
                if name.is_empty() {
                    name = a
                        .get(4)
                        .and_then(|t| self.entity(t.trim()))
                        .map(|i| display(split_top(&i.args).get(1).copied().unwrap_or("$")))
                        .unwrap_or_default();
                }
                (arg(0), name)
            }
            "IFCDOCUMENTINFORMATION" => {
                let loc = self
                    .kept
                    .iter()
                    .filter(|k| k.class == "IFCDOCUMENTREFERENCE")
                    .find(|k| split_top(&k.args).get(4).map(|t| t.trim()) == Some(token))
                    .map(|k| display(split_top(&k.args).first().copied().unwrap_or("$")))
                    .unwrap_or_default();
                (loc, arg(1))
            }
            _ => return None,
        };
        if name.is_empty() {
            name = location
                .rsplit(['/', '\\'])
                .next()
                .unwrap_or("")
                .to_string();
        }
        if name.is_empty() && location.is_empty() {
            return None;
        }
        Some(crate::docrefs::DocRef { name, location })
    }

    pub fn get(&self, guid: &str) -> Option<&Record> {
        self.by_guid.get(guid)
    }
}

/// The record of element `e`: an element read from IFC has the object's
/// GlobalId as its id (see [`crate::ElementId::from_ifc_guid`]).
pub fn of<'a>(model: &'a crate::Model, e: &crate::Element) -> Option<&'a Record> {
    let data = model.ifc_data();
    if data.is_empty() {
        return None;
    }
    data.get(&e.id.ifc_guid())
}

/// A kept value as a person reads it: `IFCLABEL('Brick')` is Brick,
/// `.T.` Yes, `IFCLENGTHMEASURE(2.4)` 2.4, a list its items joined.
pub fn display(value: &str) -> String {
    let v = value.trim();
    match v {
        "$" => return String::new(),
        ".T." => return "Yes".into(),
        ".F." => return "No".into(),
        ".U." => return "Unknown".into(),
        _ => {}
    }
    // a list: (a,b)
    if let Some(inner) = v.strip_prefix('(').and_then(|x| x.strip_suffix(')')) {
        return split_top(inner)
            .iter()
            .map(|x| display(x))
            .collect::<Vec<_>>()
            .join(", ");
    }
    // a typed value: NAME(inner)
    if let Some(open) = v.find('(') {
        if v.ends_with(')')
            && v[..open]
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return display(&v[open + 1..v.len() - 1]);
        }
    }
    if let Some(text) = v.strip_prefix('\'').and_then(|x| x.strip_suffix('\'')) {
        return decode(&text.replace("''", "'"));
    }
    if let Some(e) = v.strip_prefix('.').and_then(|x| x.strip_suffix('.')) {
        return e.to_string();
    }
    v.trim_end_matches('.').to_string()
}

/// Split `a,b` at the commas outside brackets and quotes.
fn split_top(s: &str) -> Vec<&str> {
    let (mut out, mut depth, mut quoted, mut start) = (Vec::new(), 0i32, false, 0);
    for (i, ch) in s.char_indices() {
        match ch {
            '\'' => quoted = !quoted,
            '(' if !quoted => depth += 1,
            ')' if !quoted => depth -= 1,
            ',' if !quoted && depth == 0 => {
                out.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&s[start..]);
    out
}

/// STEP's `\X2\00E9\X0\` encoding of text beyond ASCII, read back.
fn decode(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(at) = rest.find("\\X2\\") {
        out.push_str(&rest[..at]);
        let after = &rest[at + 4..];
        let end = after.find("\\X0\\").unwrap_or(after.len());
        let units: Vec<u16> = after.as_bytes()[..end]
            .chunks(4)
            .filter_map(|c| u16::from_str_radix(std::str::from_utf8(c).ok()?, 16).ok())
            .collect();
        out.push_str(&String::from_utf16_lossy(&units));
        rest = after.get(end + 4..).unwrap_or("");
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod display_tests {
    use super::display;

    #[test]
    fn values_read_as_a_person_would() {
        assert_eq!(display("IFCLABEL('Brick')"), "Brick");
        assert_eq!(display(".T."), "Yes");
        assert_eq!(display("IFCLENGTHMEASURE(2.4)"), "2.4");
        assert_eq!(display("IFCREAL(3.)"), "3");
        assert_eq!(display("(IFCLABEL('A'),IFCLABEL('B, C'))"), "A, B, C");
        assert_eq!(display("IFCLABEL('Ba\\X2\\00E9\\X0\\ton')"), "Baéton");
        assert_eq!(display("IFCTEXT('it''s')"), "it's");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Element, ElementKind, Model};

    #[test]
    fn a_models_copies_share_its_record_and_an_element_finds_its_own() {
        let mut m = Model::default();
        let e = Element::new("w", ElementKind::Project(Default::default()));
        let guid = e.id.ifc_guid();
        m.insert(e.clone()).unwrap();
        let mut d = IfcData::default();
        d.by_guid.insert(
            guid,
            Record {
                psets: vec![Pset {
                    name: "Acme".into(),
                    description: None,
                    props: vec![Prop {
                        name: "Fire".into(),
                        value: "IFCLABEL('60')".into(),
                        kind: String::new(),
                    }],
                }],
                classes: vec![],
            },
        );
        m.set_ifc_data(d);
        let copy = m.clone();
        assert!(
            std::ptr::eq(m.ifc_data(), copy.ifc_data()),
            "one shared copy"
        );
        assert_eq!(
            of(&copy, &e).unwrap().psets[0].props[0].value,
            "IFCLABEL('60')"
        );
        let other = Element::new("x", ElementKind::Project(Default::default()));
        assert!(of(&m, &other).is_none());
    }
}
