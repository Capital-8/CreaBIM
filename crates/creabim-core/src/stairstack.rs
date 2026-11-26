//! Stacked stairs: one stair repeated up through typical floors.
//!
//! The stair at the foot of the stack is an ordinary stair. Each copy on
//! a floor above is a stair of its own (its own plan, checks, well and
//! IFC entity in its storey) whose `typical` names that foot. After any
//! edit, [`settle`] makes every stair of the stack the same as the one
//! edited, apart from its level, and gives each the same railings and
//! well. A copy released from the stack (`typical` cleared) is a free
//! stair again, as it stood.

use crate::members::{elevation, Stair, StairSide};
use crate::{Element, ElementId, ElementKind, Model, Op, Transaction};

/// The stair at the foot of `id`'s stack: itself when it isn't a copy.
pub fn foot(model: &Model, id: ElementId) -> ElementId {
    match model.get(id).map(|e| &e.kind) {
        Some(ElementKind::Stair(s)) => s
            .typical
            .filter(|t| matches!(model.get(*t).map(|e| &e.kind), Some(ElementKind::Stair(_))))
            .unwrap_or(id),
        _ => id,
    }
}

/// Every stair of `id`'s stack, lowest first; just `id` for a stair on
/// its own.
pub fn stack(model: &Model, id: ElementId) -> Vec<ElementId> {
    let f = foot(model, id);
    let mut out: Vec<(f64, ElementId)> = model
        .iter()
        .filter_map(|e| match &e.kind {
            ElementKind::Stair(s) if e.id == f || s.typical == Some(f) => {
                Some((elevation(model, s.level), e.id))
            }
            _ => None,
        })
        .collect();
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out.into_iter().map(|(_, id)| id).collect()
}

/// Whether `id` is stacked: a copy, or the foot of copies.
pub fn is_stacked(model: &Model, id: ElementId) -> bool {
    stack(model, id).len() > 1
}

/// The levels from `id`'s stack's foot up to `top`, not counting `top`:
/// where the stack's stairs start when it reaches `top`.
pub fn levels_to(model: &Model, id: ElementId, top: ElementId) -> Vec<ElementId> {
    let f = foot(model, id);
    let Some(ElementKind::Stair(s)) = model.get(f).map(|e| &e.kind) else {
        return vec![];
    };
    let (lo, hi) = (elevation(model, s.level), elevation(model, top));
    let mut ls: Vec<(f64, ElementId)> = model
        .levels()
        .map(|(e, l)| (l.elevation, e.id))
        .filter(|(z, _)| *z > lo + 1e-6 && *z < hi - 1e-6)
        .collect();
    ls.sort_by(|a, b| a.0.total_cmp(&b.0));
    ls.into_iter().map(|(_, id)| id).collect()
}

/// Repeat stair `id`'s stack up to level `top`: a copy on every level from
/// its foot up to the one below `top`, so the last arrives at `top`.
/// Copies already there stay, and so does a stair released from the
/// stack (no copy goes over it); copies on `top` or above go, with their
/// railings and wells.
pub fn repeat_to(model: &Model, id: ElementId, top: ElementId) -> Result<Transaction, String> {
    let f = foot(model, id);
    let (Some(fe), Some(_)) = (model.get(f), model.level(top)) else {
        return Err("no such stair or level".into());
    };
    let ElementKind::Stair(fs) = &fe.kind else {
        return Err("that isn't a stair".into());
    };
    if elevation(model, top) <= elevation(model, fs.level) + 1e-6 {
        return Err("pick a level above the stair".into());
    }
    let want = levels_to(model, f, top);
    let have: Vec<(ElementId, ElementId)> = stack(model, f)
        .into_iter()
        .filter(|s| *s != f)
        .filter_map(|s| match model.get(s).map(|e| &e.kind) {
            Some(ElementKind::Stair(x)) => Some((s, x.level)),
            _ => None,
        })
        .collect();
    // a storey released from the stack keeps its place: no copy over it
    let released = |level: ElementId| {
        model.iter().any(|e| {
            matches!(&e.kind, ElementKind::Stair(x)
                if e.id != f && x.typical.is_none() && x.level == level
                    && x.start.distance(fs.start) < 1e-3)
        })
    };
    let mut tx = Transaction::new("Repeat stair");
    for level in &want {
        if have.iter().any(|(_, l)| l == level) || released(*level) {
            continue;
        }
        let mut s = fs.clone();
        s.level = *level;
        s.typical = Some(f);
        let name = match model.get(*level) {
            Some(l) => format!("{} ({})", fe.name, l.name),
            None => fe.name.clone(),
        };
        tx = tx.insert(Element::new(name, ElementKind::Stair(s)));
    }
    let gone: Vec<ElementId> = have
        .iter()
        .filter(|(_, l)| !want.contains(l))
        .map(|(s, _)| *s)
        .collect();
    if !gone.is_empty() {
        tx = tx.remove_all_cascade(model, &gone);
    }
    Ok(tx)
}

/// Take stair `id` out of its stack, as it stands. Releasing the foot
/// releases nothing: its copies keep following it (release them instead).
pub fn release(model: &Model, id: ElementId) -> Option<Transaction> {
    let e = model.get(id)?;
    let ElementKind::Stair(s) = &e.kind else {
        return None;
    };
    s.typical?;
    let mut after = e.clone();
    if let ElementKind::Stair(x) = &mut after.kind {
        x.typical = None;
    }
    Some(Transaction::new("Release stair").replace(e.clone(), after))
}

/// `from`'s shape for the stair `to` of the same stack: everything but
/// its level and the stair it copies.
fn copied(from: &Stair, to: &Stair) -> Stair {
    let mut s = from.clone();
    s.level = to.level;
    s.typical = to.typical;
    s
}

/// The railing following side `left` of stair `id`.
fn railing_on(model: &Model, id: ElementId, left: bool) -> Option<&Element> {
    model.iter().find(|e| {
        matches!(&e.kind, ElementKind::Railing(r)
            if r.stair == Some(StairSide { stair: id, left }))
    })
}

/// The well tied to stair `id`.
fn well_of(model: &Model, id: ElementId) -> Option<&Element> {
    model
        .iter()
        .find(|e| matches!(&e.kind, ElementKind::Shaft(sh) if sh.stair == Some(id)))
}

/// After `tx`: copies whose foot went take the lowest of them as their
/// foot; then in each stack `tx` touched, every stair takes the shape of
/// the one edited, and the railings and well it has.
pub fn settle(model: &Model, tx: &Transaction) -> Transaction {
    let mut out = Transaction::new("Stacked stairs");
    // a foot deleted: the lowest copy takes its place (only then: a copy
    // may be built before its foot, as from a file)
    let removed = tx
        .ops
        .iter()
        .any(|op| matches!(op, Op::Remove(e) if matches!(e.kind, ElementKind::Stair(_))));
    let mut orphans: Vec<(ElementId, f64, ElementId)> = model
        .iter()
        .filter_map(|e| match &e.kind {
            ElementKind::Stair(s)
                if s.typical.is_some_and(|t| {
                    !matches!(model.get(t).map(|x| &x.kind), Some(ElementKind::Stair(_)))
                }) =>
            {
                Some((s.typical.unwrap(), elevation(model, s.level), e.id))
            }
            _ => None,
        })
        .collect();
    if removed && !orphans.is_empty() {
        orphans.sort_by(|a, b| a.1.total_cmp(&b.1));
        let mut new_foot: Vec<(ElementId, ElementId)> = vec![];
        for (old, _, id) in &orphans {
            let to = match new_foot.iter().find(|(o, _)| o == old) {
                Some((_, f)) => Some(*f),
                None => {
                    new_foot.push((*old, *id));
                    None
                }
            };
            let e = model.get(*id).unwrap();
            let mut after = e.clone();
            if let ElementKind::Stair(s) = &mut after.kind {
                s.typical = to;
            }
            out = out.replace(e.clone(), after);
        }
        // the rest settles on the next edit
        return out;
    }

    // the stair each op speaks for, as it is now
    let mut sources: Vec<ElementId> = vec![];
    for op in &tx.ops {
        let (e, inserted) = match op {
            Op::Insert(e) => (e, true),
            Op::Replace { after, .. } => (after, false),
            Op::Remove(e) => (e, false),
        };
        let id = match &e.kind {
            // a new copy takes after its foot
            ElementKind::Stair(s) if inserted => s.typical.unwrap_or(e.id),
            ElementKind::Stair(_) => e.id,
            ElementKind::Railing(r) => match r.stair {
                Some(side) => side.stair,
                None => continue,
            },
            ElementKind::Shaft(sh) => match sh.stair {
                Some(s) => s,
                None => continue,
            },
            _ => continue,
        };
        let Some(ElementKind::Stair(s)) = model.get(id).map(|e| &e.kind) else {
            continue;
        };
        // a stair just released speaks for no stack
        if s.typical.is_none() && !is_stacked(model, id) {
            continue;
        }
        let f = foot(model, id);
        if !sources.iter().any(|x| foot(model, *x) == f) {
            sources.push(id);
        }
    }

    for src in sources {
        let Some(ElementKind::Stair(from)) = model.get(src).map(|e| &e.kind) else {
            continue;
        };
        for id in stack(model, src) {
            if id == src {
                continue;
            }
            let e = model.get(id).unwrap();
            let ElementKind::Stair(to) = &e.kind else {
                continue;
            };
            let want = copied(from, to);
            if &want != to {
                let mut after = e.clone();
                after.kind = ElementKind::Stair(want.clone());
                out = out.replace(e.clone(), after);
            }
            // its railings, side by side
            for left in [true, false] {
                let theirs = railing_on(model, id, left);
                match (railing_on(model, src, left), theirs) {
                    (Some(r), None) => {
                        let mut c = r.clone();
                        c.id = ElementId::new();
                        if let ElementKind::Railing(x) = &mut c.kind {
                            x.level = want.level;
                            x.stair = Some(StairSide { stair: id, left });
                        }
                        out = out.insert(c);
                    }
                    (Some(r), Some(t)) => {
                        let mut c = r.clone();
                        c.id = t.id;
                        c.name = t.name.clone();
                        if let ElementKind::Railing(x) = &mut c.kind {
                            x.level = want.level;
                            x.stair = Some(StairSide { stair: id, left });
                        }
                        if &c != t {
                            out = out.replace(t.clone(), c);
                        }
                    }
                    (None, Some(t)) => out = out.remove_cascade(model, t.id),
                    (None, None) => {}
                }
            }
            // and its well: the outline is the well's own, redrawn by
            // stairwell::settle; here only whether it has one, and its
            // clearance
            match (well_of(model, src), well_of(model, id)) {
                (Some(w), None) => {
                    let ElementKind::Shaft(sh) = &w.kind else {
                        continue;
                    };
                    // where the floor above is clear, it needs none
                    if let Ok(cut) = crate::stairwell::cut_well(model, id, sh.clearance) {
                        out.ops.extend(cut.ops);
                    }
                }
                (Some(w), Some(t)) => {
                    let (ElementKind::Shaft(a), ElementKind::Shaft(b)) = (&w.kind, &t.kind) else {
                        continue;
                    };
                    if (a.clearance - b.clearance).abs() > 1e-9 {
                        let mut c = t.clone();
                        if let ElementKind::Shaft(x) = &mut c.kind {
                            x.clearance = a.clearance;
                        }
                        out = out.replace(t.clone(), c);
                    }
                }
                (None, Some(t)) => out = out.remove_cascade(model, t.id),
                (None, None) => {}
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::members::{Railing, StairLine};
    use crate::{Document, Level, Slab};
    use glam::DVec2;

    /// Four storeys 3 m apart with a floor on each above the ground, and a
    /// stair with a railing on its left on the ground; the document, the
    /// levels lowest first and the stair.
    fn house() -> (Document, Vec<ElementId>, ElementId) {
        let mut doc = Document::new();
        let mut tx = Transaction::new("t");
        let mut levels = vec![];
        for (i, name) in ["Ground", "First", "Second", "Roof"].iter().enumerate() {
            let l = Element::new(
                *name,
                ElementKind::Level(Level {
                    elevation: 3.0 * i as f64,
                    height: 3.0,
                }),
            );
            levels.push(l.id);
            if i > 0 {
                let floor = Slab::new(
                    l.id,
                    vec![
                        DVec2::new(-2.0, -3.0),
                        DVec2::new(10.0, -3.0),
                        DVec2::new(10.0, 3.0),
                        DVec2::new(-2.0, 3.0),
                    ],
                    0.25,
                );
                tx = tx
                    .insert(l)
                    .insert(Element::new("Floor", ElementKind::Slab(floor)));
            } else {
                tx = tx.insert(l);
            }
        }
        let s = Stair::from_line(
            levels[0],
            &[DVec2::new(0.0, 0.0), DVec2::new(6.0, 0.0)],
            StairLine::Walk,
        )
        .unwrap();
        let st = Element::new("Stair", ElementKind::Stair(s));
        let id = st.id;
        tx = tx.insert(st);
        doc.commit(tx).unwrap();
        let rail = Railing::along_stair(doc.model(), id, true).unwrap();
        doc.commit(
            Transaction::new("r").insert(Element::new("Railing", ElementKind::Railing(rail))),
        )
        .unwrap();
        (doc, levels, id)
    }

    fn stair(m: &Model, id: ElementId) -> Stair {
        match &m.get(id).unwrap().kind {
            ElementKind::Stair(s) => s.clone(),
            _ => panic!(),
        }
    }

    fn rails(m: &Model, id: ElementId) -> Vec<&crate::members::Railing> {
        m.iter()
            .filter_map(|e| match &e.kind {
                ElementKind::Railing(r) if r.stair.is_some_and(|s| s.stair == id) => Some(r),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_stair_repeats_up_to_a_level_and_one_edit_changes_every_storey() {
        let (mut doc, levels, id) = house();
        let tx = repeat_to(doc.model(), id, levels[3]).unwrap();
        doc.commit(tx).unwrap();
        let m = doc.model();
        let all = stack(m, id);
        // ground, first and second: the last arrives at the roof
        assert_eq!(all.len(), 3);
        assert_eq!(all[0], id);
        for (k, s) in all.iter().enumerate() {
            assert_eq!(stair(m, *s).level, levels[k]);
            // each with its railing on the left
            assert_eq!(rails(m, *s).len(), 1, "storey {k}");
        }
        assert_eq!(m.get(all[1]).unwrap().name, "Stair (First)");

        // widen the copy on the second floor: all three widen
        let top = all[2];
        doc.edit(top, "w", |e| {
            if let ElementKind::Stair(s) = &mut e.kind {
                s.width = 1.2;
            }
        })
        .unwrap();
        for s in stack(doc.model(), id) {
            assert!((stair(doc.model(), s).width - 1.2).abs() < 1e-9);
        }
        // a railing's height on the foot goes to every storey
        let r0 = doc
            .model()
            .iter()
            .find(|e| matches!(&e.kind, ElementKind::Railing(r) if r.stair.unwrap().stair == id))
            .unwrap()
            .id;
        doc.edit(r0, "h", |e| {
            if let ElementKind::Railing(r) = &mut e.kind {
                r.height = 1.07;
            }
        })
        .unwrap();
        for s in stack(doc.model(), id) {
            assert!((rails(doc.model(), s)[0].height - 1.07).abs() < 1e-9);
        }
        // and a well cut on the foot is cut on every floor
        let cut = crate::stairwell::cut_well(doc.model(), id, 0.0).unwrap();
        doc.commit(cut).unwrap();
        let wells = doc
            .model()
            .iter()
            .filter(|e| matches!(&e.kind, ElementKind::Shaft(sh) if sh.stair.is_some()))
            .count();
        assert_eq!(wells, 3);

        // released, the first floor's stair keeps its shape and goes its own way
        let first = all[1];
        doc.commit(release(doc.model(), first).unwrap()).unwrap();
        doc.edit(first, "w", |e| {
            if let ElementKind::Stair(s) = &mut e.kind {
                s.width = 0.9;
            }
        })
        .unwrap();
        assert!((stair(doc.model(), id).width - 1.2).abs() < 1e-9);
        assert_eq!(stack(doc.model(), id).len(), 2);
        // repeating again leaves the released storey alone
        doc.commit(repeat_to(doc.model(), id, levels[3]).unwrap())
            .unwrap();
        assert_eq!(stack(doc.model(), id).len(), 2);

        // back down to the first floor: the second floor's copy goes, with
        // its railing and well
        doc.commit(repeat_to(doc.model(), id, levels[1]).unwrap())
            .unwrap();
        assert_eq!(stack(doc.model(), id), vec![id]);
        assert!(doc.model().get(top).is_none());
        assert!(rails(doc.model(), top).is_empty());
    }

    #[test]
    fn when_the_foot_goes_the_lowest_copy_takes_its_place() {
        let (mut doc, levels, id) = house();
        doc.commit(repeat_to(doc.model(), id, levels[3]).unwrap())
            .unwrap();
        let all = stack(doc.model(), id);
        doc.commit(Transaction::new("d").remove_cascade(doc.model(), id))
            .unwrap();
        let m = doc.model();
        assert_eq!(stair(m, all[1]).typical, None);
        assert_eq!(stair(m, all[2]).typical, Some(all[1]));
        assert_eq!(stack(m, all[2]), vec![all[1], all[2]]);
    }
}
