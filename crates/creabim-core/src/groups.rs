//! Groups: sets of elements, as IFC keeps them (IfcGroup). A group has no
//! place and owns nothing: a piece can be in many groups at once, on any
//! level. CreaBIM keeps the groups a file brings in, shows them, and writes
//! them back as they came.
//!
//! Some files use groups for design options: Highland Haven keeps its four
//! houses (The Bark, The Kosh, The Rock, The Scupper) as four groups that
//! share most of their pieces. CreaBIM's own options stay as they are (a
//! piece is shared, or in one option: see [`crate::phases::Question`]), and
//! the file's way is translated at the boundary by [`make_options`]:
//!
//! - a piece in every one of the groups is shared;
//! - a piece in one of them is built in that option;
//! - a piece in some of them is copied, one copy per further option, each
//!   copy remembering the piece it stands for ([`OptionCopy`]).
//!
//! The groups stay in the project, each linked to the option made from it,
//! so the file's own membership is never lost: an export writes each group
//! back with what it held, and an unchanged copy as the piece it came from.
//! Nothing is turned into options without being asked (owner, Options from
//! IFC REV00, decision 2; copy per option, owner 2026-11-23).

use crate::phases::{self, OptionDef, Question};
use crate::{Element, ElementId, ElementKind, Model, Transaction};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

/// A group of elements.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupDef {
    /// Unique among the project's groups; never changes.
    pub id: u32,
    pub name: String,
    /// What it holds, in the order the file listed them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub members: Vec<ElementId>,
    /// The IfcGroup it came from, written back as it was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ifc: Option<IfcGroupSource>,
    /// The option made from it (see [`make_options`]): what that option
    /// builds is what the group holds when it is written back.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub option: Option<u32>,
}

/// What an IFC file said of a group beyond its name and members: kept so
/// the group is written back the same, and a Bonsai saved search keeps its
/// query.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IfcGroupSource {
    pub guid: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object_type: Option<String>,
}

impl IfcGroupSource {
    /// A Bonsai saved search: its description holds the query, and its
    /// members are what the query found when the file was saved.
    pub fn is_search(&self) -> bool {
        self.description
            .as_deref()
            .is_some_and(|d| d.trim_start().starts_with("{\"type\": \"BBIM_Search\""))
    }
}

impl GroupDef {
    pub fn is_search(&self) -> bool {
        self.ifc.as_ref().is_some_and(IfcGroupSource::is_search)
    }
}

/// A copy made so a piece of several options' groups can be in each
/// option: `copy` stands for `of` in its option. `stamp` is the copy's
/// shape when made ([`shape_stamp`]); while it is unchanged, an export
/// writes the piece once, as `of`, in every group that held it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptionCopy {
    pub copy: ElementId,
    pub of: ElementId,
    pub stamp: u64,
}

/// The project's groups.
pub fn groups(model: &Model) -> &[GroupDef] {
    model
        .project_info()
        .map(|(_, p)| p.groups.as_slice())
        .unwrap_or(&[])
}

/// The project's option copies.
pub fn copies(model: &Model) -> &[OptionCopy] {
    model
        .project_info()
        .map(|(_, p)| p.option_copies.as_slice())
        .unwrap_or(&[])
}

/// A fingerprint of what `e` is, leaving out who it is and when it is
/// built: the same for a piece and its untouched copy, and the same from
/// one CreaBIM version to the next (FNV-1a over its written form).
pub fn shape_stamp(e: &Element) -> u64 {
    let text =
        serde_json::to_string(&(&e.kind, &e.of_type, &e.mark, e.on_site)).unwrap_or_default();
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// What `id` stands for in the file: the piece it is an untouched copy of,
/// when that piece is still there; else itself.
pub fn stands_for(model: &Model, id: ElementId) -> ElementId {
    copies(model)
        .iter()
        .find(|c| c.copy == id)
        .filter(|c| model.get(c.of).is_some())
        .filter(|c| model.get(c.copy).is_some_and(|e| shape_stamp(e) == c.stamp))
        .map(|c| c.of)
        .unwrap_or(id)
}

/// The name of an option made from group `name`: the set's number and
/// its underscore taken off ("1_The Bark" is "The Bark").
pub fn option_name(name: &str) -> String {
    match set_number(name) {
        Some((_, rest)) => rest.to_string(),
        None => name.trim().to_string(),
    }
}

fn set_number(name: &str) -> Option<(&str, &str)> {
    let (head, rest) = name.split_once('_')?;
    let rest = rest.trim();
    (!head.is_empty() && head.chars().all(|c| c.is_ascii_digit()) && !rest.is_empty())
        .then_some((head, rest))
}

/// Groups that look like the options of one design, each set listed by
/// group id: two or more groups not yet options, not saved searches, named
/// with the same number and an underscore ("1_The Bark", "1_The Kosh"),
/// each holding built things, and sharing most of them: at least half of
/// the smallest is in all of them.
pub fn option_candidates(model: &Model) -> Vec<Vec<u32>> {
    let mut by_set: BTreeMap<String, Vec<&GroupDef>> = BTreeMap::new();
    for g in groups(model) {
        if g.is_search() || g.option.is_some() {
            continue;
        }
        if !g
            .members
            .iter()
            .any(|m| model.get(*m).is_some_and(phases::phased))
        {
            continue;
        }
        if let Some((head, _)) = set_number(&g.name) {
            by_set.entry(head.to_string()).or_default().push(g);
        }
    }
    let mut out = vec![];
    for set in by_set.into_values().filter(|s| s.len() >= 2) {
        let sets: Vec<HashSet<ElementId>> = set
            .iter()
            .map(|g| g.members.iter().copied().collect())
            .collect();
        let smallest = sets.iter().map(HashSet::len).min().unwrap_or(0);
        let shared = sets[0]
            .iter()
            .filter(|m| sets[1..].iter().all(|s| s.contains(m)))
            .count();
        if smallest > 0 && shared * 2 >= smallest {
            out.push(set.iter().map(|g| g.id).collect());
        }
    }
    out
}

/// What [`make_options`] did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Made {
    /// The new question.
    pub question: u32,
    /// Pieces in every group: shared.
    pub shared: usize,
    /// Pieces in one group: built in its option.
    pub alone: usize,
    /// Copies made for pieces in some of the groups.
    pub copies: usize,
}

/// Turn groups `ids` into a new question named `name` (none: "Design
/// options"), one option per group in that order, the first its pick. See
/// the module's notes for what becomes of each piece. Doors and windows
/// copied with their wall go into the wall's copy. `None` for a project
/// without phases (start them first), or fewer than two groups.
pub fn make_options(model: &Model, ids: &[u32], name: &str) -> Option<(Transaction, Made)> {
    if phases::list(model).is_empty() {
        return None;
    }
    let (pid, _) = model.project_info()?;
    let all = groups(model);
    let chosen: Vec<&GroupDef> = ids
        .iter()
        .filter_map(|id| all.iter().find(|g| g.id == *id && g.option.is_none()))
        .collect();
    if chosen.len() < 2 {
        return None;
    }
    let next = phases::questions(model)
        .iter()
        .chain(phases::shelf(model))
        .flat_map(|q| std::iter::once(q.id).chain(q.options.iter().map(|o| o.id)))
        .max()
        .unwrap_or(0)
        + 1;
    let options: Vec<OptionDef> = chosen
        .iter()
        .enumerate()
        .map(|(i, g)| OptionDef {
            id: next + 1 + i as u32,
            name: option_name(&g.name),
        })
        .collect();
    let mut qname = name.trim().to_string();
    if qname.is_empty() {
        qname = "Design options".into();
    }
    let taken: HashSet<&str> = phases::questions(model)
        .iter()
        .map(|q| q.name.as_str())
        .collect();
    if taken.contains(qname.as_str()) {
        let base = qname.clone();
        let mut n = 2;
        while taken.contains(format!("{base} {n}").as_str()) {
            n += 1;
        }
        qname = format!("{base} {n}");
    }
    let mut made = Made {
        question: next,
        ..Default::default()
    };
    let question = Question {
        id: next,
        name: qname,
        pick: options[0].id,
        options: options.clone(),
    };

    // each built piece's options, in the groups' order
    let mut held: BTreeMap<ElementId, Vec<u32>> = BTreeMap::new();
    for (g, o) in chosen.iter().zip(&options) {
        for m in &g.members {
            if model.get(*m).is_some_and(phases::phased) {
                let v = held.entry(*m).or_default();
                if !v.contains(&o.id) {
                    v.push(o.id);
                }
            }
        }
    }
    let n = options.len();
    let mut tx = Transaction::new("Make options");
    let mut new_copies = vec![];
    // (original, option) -> its copy, so a door copied with its wall goes in the wall's copy
    let mut copy_of: HashMap<(ElementId, u32), ElementId> = HashMap::new();
    let mut later_openings = vec![];
    for (m, set) in &held {
        let e = model.get(*m).expect("checked above");
        if set.len() == n {
            made.shared += 1;
            continue;
        }
        let mut first = e.clone();
        first.phase.option = Some(set[0]);
        if first != *e {
            tx = tx.replace(e.clone(), first);
        }
        if set.len() == 1 {
            made.alone += 1;
            continue;
        }
        for o in &set[1..] {
            let mut c = e.clone();
            c.id = ElementId::new();
            c.phase.option = Some(*o);
            copy_of.insert((*m, *o), c.id);
            if matches!(c.kind, ElementKind::Opening(_)) {
                later_openings.push((c, *m));
            } else {
                new_copies.push(OptionCopy {
                    copy: c.id,
                    of: *m,
                    stamp: shape_stamp(&c),
                });
                tx = tx.insert(c);
            }
            made.copies += 1;
        }
    }
    for (mut c, of) in later_openings {
        if let (ElementKind::Opening(op), Some(o)) = (&mut c.kind, c.phase.option) {
            if let Some(host) = copy_of.get(&(op.host, o)) {
                op.host = *host;
            }
        }
        new_copies.push(OptionCopy {
            copy: c.id,
            of,
            stamp: shape_stamp(&c),
        });
        tx = tx.insert(c);
    }
    let before = model.get(pid)?.clone();
    let mut after = before.clone();
    if let ElementKind::Project(p) = &mut after.kind {
        for (g, o) in chosen.iter().zip(&options) {
            if let Some(x) = p.groups.iter_mut().find(|x| x.id == g.id) {
                x.option = Some(o.id);
            }
        }
        p.questions.push(question);
        p.option_copies.extend(new_copies);
    }
    tx = tx.replace(before, after);
    Some((tx, made))
}

/// What group `g` holds now, as written to a file: its own members still
/// there; for a group an option was made from, what that option builds and
/// what all options share that the group held, with untouched copies
/// written as the piece they stand for.
pub fn holds(model: &Model, g: &GroupDef) -> Vec<ElementId> {
    let mut out: Vec<ElementId> = vec![];
    let mut seen = HashSet::new();
    let mut push = |id: ElementId, out: &mut Vec<ElementId>| {
        if seen.insert(id) {
            out.push(id);
        }
    };
    let Some(o) = g.option.filter(|o| phases::option(model, *o).is_some()) else {
        for m in g.members.iter().filter(|m| model.get(**m).is_some()) {
            push(*m, &mut out);
        }
        return out;
    };
    let listed: HashSet<ElementId> = g.members.iter().copied().collect();
    // the file's order first, then what was drawn in the option since
    for m in &g.members {
        if let Some(e) = model.get(*m) {
            if e.phase.option == Some(o) || (e.phase.option.is_none() && phases::phased(e)) {
                push(*m, &mut out);
            } else if let Some(c) = copies(model).iter().find(|c| {
                c.of == *m && model.get(c.copy).is_some_and(|x| x.phase.option == Some(o))
            }) {
                push(stands_for(model, c.copy), &mut out);
            }
        }
    }
    for e in model
        .iter()
        .filter(|e| e.phase.option == Some(o) && !listed.contains(&e.id))
    {
        push(stands_for(model, e.id), &mut out);
    }
    out
}

/// The groups `e` is in, by name: those that hold it, or the piece it is
/// an untouched copy of.
pub fn of(model: &Model, e: ElementId) -> Vec<String> {
    let me = stands_for(model, e);
    groups(model)
        .iter()
        .filter(|g| match g.option {
            Some(_) => holds(model, g).contains(&me),
            None => g.members.contains(&e),
        })
        .map(|g| g.name.clone())
        .collect()
}

/// The members of group `id` still in the model, for selecting them.
pub fn members(model: &Model, id: u32) -> Vec<ElementId> {
    let Some(g) = groups(model).iter().find(|g| g.id == id) else {
        return vec![];
    };
    match g.option {
        // the option's pieces, copies included: what is seen
        Some(o) => model
            .iter()
            .filter(|e| {
                e.phase.option == Some(o) || (e.phase.option.is_none() && g.members.contains(&e.id))
            })
            .map(|e| e.id)
            .collect(),
        None => g
            .members
            .iter()
            .copied()
            .filter(|m| model.get(*m).is_some())
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editmesh::EditMesh;
    use crate::project::ProjectInfo;
    use crate::{Document, Element, FreeMesh};
    use glam::DVec3;

    struct Fixture {
        doc: Document,
        pieces: Vec<ElementId>,
    }

    /// Five blocks, and the groups `spec` names: each group's members by
    /// index into the blocks.
    fn fixture(spec: &[(&str, &[usize])]) -> Fixture {
        let slabs: Vec<Element> = (0..5)
            .map(|i| {
                let at = DVec3::new(i as f64 * 3.0, 0.0, 0.0);
                Element::new(
                    format!("s{i}"),
                    ElementKind::FreeMesh(FreeMesh::new(
                        None,
                        at,
                        EditMesh::cuboid(DVec3::new(2.0, 2.0, 0.2)),
                    )),
                )
            })
            .collect();
        let pieces: Vec<ElementId> = slabs.iter().map(|e| e.id).collect();
        let groups = spec
            .iter()
            .enumerate()
            .map(|(i, (name, m))| GroupDef {
                id: i as u32 + 1,
                name: name.to_string(),
                members: m.iter().map(|k| pieces[*k]).collect(),
                ifc: None,
                option: None,
            })
            .collect();
        let info = ProjectInfo {
            phases: phases::standard(),
            groups,
            ..Default::default()
        };
        let mut tx =
            Transaction::new("t").insert(Element::new("Project", ElementKind::Project(info)));
        for s in slabs {
            tx = tx.insert(s);
        }
        let mut doc = Document::new();
        doc.commit(tx).unwrap();
        Fixture { doc, pieces }
    }

    #[test]
    fn numbered_groups_sharing_most_pieces_look_like_options() {
        let f = fixture(&[
            ("1_The Bark", &[0, 1, 2]),
            ("1_The Kosh", &[0, 1, 3]),
            ("FRAMING", &[0, 1, 2, 3]),
            ("2_The Drumlin", &[4]),
        ]);
        assert_eq!(option_candidates(f.doc.model()), vec![vec![1, 2]]);
    }

    #[test]
    fn groups_that_share_little_are_not_offered() {
        let f = fixture(&[("1_A", &[0, 1, 2]), ("1_B", &[2, 3, 4])]);
        assert!(option_candidates(f.doc.model()).is_empty());
    }

    #[test]
    fn a_piece_in_some_options_is_copied_into_each() {
        // 0 in all three, 1 in A and B, 2 in A alone, 4 in none
        let mut f = fixture(&[("1_A", &[0, 1, 2]), ("1_B", &[0, 1]), ("1_C", &[0, 3])]);
        let before = f.doc.model().len();
        let (tx, made) = make_options(f.doc.model(), &[1, 2, 3], "").expect("options made");
        f.doc.commit(tx).unwrap();
        assert_eq!((made.shared, made.alone, made.copies), (1, 2, 1));
        let m = f.doc.model();
        assert_eq!(m.len(), before + 1, "one copy, nothing else added");
        let q = phases::questions(m)
            .iter()
            .find(|q| q.id == made.question)
            .expect("the question");
        assert_eq!(q.name, "Design options");
        let [a, b, c] = [q.options[0].id, q.options[1].id, q.options[2].id];
        let opt = |i: usize| m.get(f.pieces[i]).unwrap().phase.option;
        assert_eq!(opt(0), None, "in every group: shared");
        assert_eq!(opt(1), Some(a), "the piece itself goes to the first");
        assert_eq!(opt(2), Some(a));
        assert_eq!(opt(3), Some(c));
        assert_eq!(opt(4), None, "in no group: shared");
        let copy = copies(m)[0];
        assert_eq!(copy.of, f.pieces[1]);
        assert_eq!(m.get(copy.copy).unwrap().phase.option, Some(b));
        // each option shows its own house, plus what all share
        let shows = |o: u32, id: ElementId| {
            phases::effective(m.get(id).unwrap().phase, &phases::chosen(m, &[o])).is_some()
        };
        assert!(shows(b, copy.copy) && !shows(a, copy.copy));
        assert!(shows(a, f.pieces[1]) && !shows(b, f.pieces[1]));
        assert!(shows(c, f.pieces[0]) && shows(c, f.pieces[4]));
    }

    #[test]
    fn untouched_the_groups_are_written_back_as_they_came() {
        let mut f = fixture(&[
            ("1_A", &[0, 1, 2]),
            ("1_B", &[0, 1]),
            ("1_C", &[0, 3]),
            ("FRAMING", &[1, 4]),
        ]);
        let (tx, _) = make_options(f.doc.model(), &[1, 2, 3], "House").unwrap();
        f.doc.commit(tx).unwrap();
        let m = f.doc.model();
        let held = |name: &str| {
            let g = groups(m).iter().find(|g| g.name == name).unwrap();
            let mut v = holds(m, g);
            v.sort();
            v
        };
        let ids = |k: &[usize]| {
            let mut v: Vec<ElementId> = k.iter().map(|i| f.pieces[*i]).collect();
            v.sort();
            v
        };
        assert_eq!(held("1_A"), ids(&[0, 1, 2]));
        assert_eq!(
            held("1_B"),
            ids(&[0, 1]),
            "the copy is written as the piece it stands for"
        );
        assert_eq!(held("1_C"), ids(&[0, 3]));
        assert_eq!(held("FRAMING"), ids(&[1, 4]));
    }

    #[test]
    fn a_changed_copy_is_its_own_piece() {
        let mut f = fixture(&[("1_A", &[0, 1]), ("1_B", &[0, 2]), ("1_C", &[0, 1])]);
        let (tx, _) = make_options(f.doc.model(), &[1, 2, 3], "").unwrap();
        f.doc.commit(tx).unwrap();
        let copy = copies(f.doc.model())[0].copy;
        assert_eq!(stands_for(f.doc.model(), copy), f.pieces[1]);
        let tx = {
            let m = f.doc.model();
            let before = m.get(copy).unwrap().clone();
            let mut after = before.clone();
            after.name = "renamed only".into();
            Transaction::new("rename").replace(before, after)
        };
        f.doc.commit(tx).unwrap();
        assert_eq!(
            stands_for(f.doc.model(), copy),
            f.pieces[1],
            "a new name is not a new shape"
        );
        let tx = {
            let m = f.doc.model();
            let before = m.get(copy).unwrap().clone();
            let mut after = before.clone();
            after.mark = Some("S-9".into());
            Transaction::new("mark").replace(before, after)
        };
        f.doc.commit(tx).unwrap();
        assert_eq!(
            stands_for(f.doc.model(), copy),
            copy,
            "changed: written as itself"
        );
    }

    #[test]
    fn option_names_lose_their_set_number() {
        assert_eq!(option_name("1_The Bark"), "The Bark");
        assert_eq!(
            option_name("BBIM_Linked_Aggregate"),
            "BBIM_Linked_Aggregate"
        );
        assert_eq!(option_name("_End Module"), "_End Module");
    }
}
