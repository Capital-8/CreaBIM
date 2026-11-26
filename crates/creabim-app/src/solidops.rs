//! Solid operations (wave 3, decision 4, A): select a shape (drawn with
//! Rectangle and Pull) and the elements it should work on; the strip
//! offers Add, Subtract and Intersect, and Reach. The shape stays as a
//! live operator: a see-through ghost, listed under each element's
//! Junctions and booleans, moved or reshaped later to change the result.
//! Reach runs it up or down without end, so one rectangle cuts a whole
//! stack of floors. Each element gets a [`Cut`]; one already made by the
//! same shape is changed instead.

use super::actionbar::Act;
use super::*;
use creabim_core::{BoolOp, Reach};

/// The operations, in the strip's order.
const OPS: [(BoolOp, &str, &str); 3] = [
    (BoolOp::Union, "Add", "solid-add"),
    (BoolOp::Difference, "Subtract", "solid-subtract"),
    (BoolOp::Intersection, "Intersect", "solid-intersect"),
];

const REACH_ICONS: [&str; 4] = ["reach-drawn", "reach-up", "reach-down", "reach-both"];

/// What a shape can add to, cut or intersect: what has a solid of its own.
pub(super) fn takes_booleans(k: &ElementKind) -> bool {
    matches!(
        k,
        ElementKind::Wall(_)
            | ElementKind::Slab(_)
            | ElementKind::Roof(_)
            | ElementKind::Column(_)
            | ElementKind::Beam(_)
            | ElementKind::Stair(_)
            | ElementKind::Footing(_)
            | ElementKind::Ceiling(_)
            | ElementKind::Part(_)
            | ElementKind::Object(_)
            | ElementKind::FreeMesh(_)
    )
}

impl CreaBimApp {
    /// The shape and what it works on, when the selection is one shape and
    /// one or more elements with a solid; with several shapes, the one
    /// picked last is the operator.
    pub(super) fn solid_op_pair(&self) -> Option<(ElementId, Vec<ElementId>)> {
        let model = self.doc.model();
        let ids = self.selection.all();
        if ids.len() < 2 {
            return None;
        }
        let is_shape = |id: &ElementId| {
            matches!(
                model.get(*id).map(|e| &e.kind),
                Some(ElementKind::FreeMesh(_))
            )
        };
        let shapes: Vec<ElementId> = ids.iter().copied().filter(is_shape).collect();
        let shape = match shapes[..] {
            [one] => one,
            [] => return None,
            _ => self.selection.active.filter(is_shape)?,
        };
        let targets: Vec<ElementId> = ids
            .iter()
            .copied()
            .filter(|id| *id != shape)
            .filter(|id| model.get(*id).is_some_and(|e| takes_booleans(&e.kind)))
            .collect();
        // all of the rest must take it, or the strip would mislead
        (!targets.is_empty() && targets.len() + 1 == ids.len()).then_some((shape, targets))
    }

    /// Why a shape selected with other elements gets no Add or Subtract:
    /// the first of them a shape can't work on (decision 4).
    pub(super) fn solid_op_blocker(&self) -> Option<String> {
        let model = self.doc.model();
        let ids = self.selection.all();
        if ids.len() < 2 {
            return None;
        }
        let is_shape = |id: &ElementId| {
            matches!(
                model.get(*id).map(|e| &e.kind),
                Some(ElementKind::FreeMesh(_))
            )
        };
        if !ids.iter().any(is_shape) || self.solid_op_pair().is_some() {
            return None;
        }
        let odd = ids
            .iter()
            .filter_map(|id| model.get(*id))
            .find(|e| !takes_booleans(&e.kind))?;
        let what = e_kind_name(&odd.kind);
        Some(format!(
            "{what} can't be added to or cut by a shape: leave {} out of the selection for Add, Subtract and Intersect.",
            odd.name
        ))
    }

    /// A boolean, as the activity log keeps it (MU71: it said only
    /// "Add"): the shape, its size and where it stands, the kind and how
    /// far it reaches, and each element it works on with how many layer
    /// meshes it is drawn as, so a report says what was asked of what.
    pub(super) fn log_boolean(
        &self,
        label: &str,
        shape: ElementId,
        targets: &[ElementId],
        reach: Reach,
    ) {
        let model = self.doc.model();
        let name = |id: ElementId| model.get(id).map_or("?".to_string(), |e| e.name.clone());
        let bounds = |id: ElementId| {
            let mut lo = glam::Vec3::splat(f32::MAX);
            let mut hi = glam::Vec3::splat(f32::MIN);
            for i in self.scene.iter().filter(|i| i.id == id) {
                if let Some((a, b)) = i.mesh.bounds() {
                    lo = lo.min(a);
                    hi = hi.max(b);
                }
            }
            (lo.x <= hi.x).then(|| {
                format!(
                    "{} x {} x {} m at ({:.2}, {:.2}, {:.2})",
                    (hi.x - lo.x) as f64,
                    (hi.y - lo.y) as f64,
                    (hi.z - lo.z) as f64,
                    lo.x,
                    lo.y,
                    lo.z
                )
            })
        };
        let on: Vec<String> = targets
            .iter()
            .map(|t| {
                let layers = self.scene.iter().filter(|i| i.id == *t).count();
                format!("{} ({layers} layer meshes)", name(*t))
            })
            .collect();
        crate::actlog::line(format!(
            "boolean: {label} | shape {} {} | reach {} | on {}",
            name(shape),
            bounds(shape).unwrap_or_default(),
            reach.label(),
            on.join(", ")
        ));
    }

    /// The cuts `shape` makes on `targets`, by target.
    fn cuts_by(&self, shape: ElementId, targets: &[ElementId]) -> Vec<(ElementId, Element)> {
        let model = self.doc.model();
        model
            .iter()
            .filter_map(|e| match &e.kind {
                ElementKind::Cut(c) if c.cutter == shape && targets.contains(&c.target) => {
                    Some((c.target, e.clone()))
                }
                _ => None,
            })
            .collect()
    }

    /// The operation and the reach the shape already works with on every
    /// one of `targets`, if one.
    fn solid_now(&self, shape: ElementId, targets: &[ElementId]) -> (Option<BoolOp>, Reach) {
        let cuts = self.cuts_by(shape, targets);
        let all = targets.iter().all(|t| cuts.iter().any(|(c, _)| c == t));
        let of = |e: &Element| match &e.kind {
            ElementKind::Cut(c) => Some((c.op, c.reach)),
            _ => None,
        };
        let first = cuts.first().and_then(|(_, e)| of(e));
        let same = cuts.iter().all(|(_, e)| of(e) == first);
        match (all && same, first) {
            (true, Some((op, reach))) => (Some(op), reach),
            _ => (None, self.solid_reach),
        }
    }

    /// The strip's rows for a shape and what it works on.
    pub(super) fn solid_op_acts(&self, shape: ElementId, targets: &[ElementId]) -> Vec<Act> {
        let (op, reach) = self.solid_now(shape, targets);
        let n = targets.len();
        let them = if n == 1 { "it" } else { "them" };
        let mut acts: Vec<Act> = OPS
            .iter()
            .enumerate()
            .map(|(i, (o, label, icon))| {
                let hint = match o {
                    BoolOp::Union => format!("Join the shape on to {them}"),
                    BoolOp::Difference => format!("Cut the shape out of {them}"),
                    BoolOp::Intersection => format!("Keep only what {them} and the shape both hold"),
                };
                Act {
                    on: op == Some(*o),
                    ..Act::new(format!("solid:op:{i}"), *label, icon).hint(format!(
                        "{hint}. The shape stays, as a see-through ghost: move or reshape it to change the result"
                    ))
                }
            })
            .collect();
        acts.push(Act::new("head:reach", "Reach", ""));
        acts.extend(Reach::ALL.iter().enumerate().map(|(i, r)| {
            let hint = match r {
                Reach::AsDrawn => "The shape works as far as it is drawn",
                Reach::Up => "Its top runs on up without end",
                Reach::Down => "Its bottom runs on down without end",
                Reach::Both => "It runs on up and down without end: one rectangle cuts every floor",
            };
            Act {
                on: reach == *r,
                ..Act::new(format!("solid:reach:{i}"), r.label(), REACH_ICONS[i]).hint(hint)
            }
        }));
        acts
    }

    /// Run a solid operation row of the strip.
    pub(super) fn run_solid_op(&mut self, id: &str) {
        let Some((shape, targets)) = self.solid_op_pair() else {
            return;
        };
        let cuts = self.cuts_by(shape, &targets);
        if let Some(i) = id
            .strip_prefix("solid:reach:")
            .and_then(|i| i.parse::<usize>().ok())
        {
            let Some(r) = Reach::ALL.get(i).copied() else {
                return;
            };
            self.solid_reach = r;
            if cuts.is_empty() {
                self.status = format!("Reach: {}. Now pick Add, Subtract or Intersect.", r.label());
                return;
            }
            let mut tx = Transaction::new("Reach");
            for (_, before) in &cuts {
                let mut after = before.clone();
                if let ElementKind::Cut(c) = &mut after.kind {
                    c.reach = r;
                }
                tx = tx.replace(before.clone(), after);
            }
            let res = self.doc.commit(tx);
            self.report(res, format!("Reach: {}.", r.label()));
            return;
        }
        let Some((op, label, _)) = id
            .strip_prefix("solid:op:")
            .and_then(|i| i.parse::<usize>().ok())
            .and_then(|i| OPS.get(i).copied())
        else {
            return;
        };
        let reach = self.solid_now(shape, &targets).1;
        let mut tx = Transaction::new(label);
        for t in &targets {
            match cuts.iter().find(|(c, _)| c == t) {
                Some((_, before)) => {
                    let mut after = before.clone();
                    if let ElementKind::Cut(c) = &mut after.kind {
                        (c.op, c.reach) = (op, reach);
                    }
                    tx = tx.replace(before.clone(), after);
                }
                None => {
                    tx = tx.insert(Element::new(
                        "Cut",
                        ElementKind::Cut(Cut {
                            target: *t,
                            cutter: shape,
                            op,
                            reach,
                        }),
                    ));
                }
            }
        }
        let n = targets.len();
        let what = plural("element", n);
        let res = self.doc.commit(tx);
        self.log_boolean(label, shape, &targets, reach);
        self.report(
            res,
            format!(
                "{label}: the shape works on {n} {what}. It stays as a ghost: move or reshape it to change the result; Junctions and booleans lists it."
            ),
        );
    }
}

/// What an element is, for a sentence: "A door", "A dimension".
fn e_kind_name(k: &ElementKind) -> String {
    let n = format!("{k:?}");
    let word: String = n.split(['(', '{', ' ']).next().unwrap_or("").to_string();
    // CamelCase to words
    let mut out = String::new();
    for (i, c) in word.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push(' ');
        }
        out.push(c.to_ascii_lowercase());
    }
    let a = if out.starts_with(['a', 'e', 'i', 'o', 'u']) {
        "An"
    } else {
        "A"
    };
    format!("{a} {out}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shape_subtracts_from_two_walls_live_and_reach_runs_it_up() {
        let mut app = crate::app::test_app();
        let model = app.doc.model();
        let walls: Vec<ElementId> = model
            .iter()
            .filter(|e| matches!(e.kind, ElementKind::Wall(_)))
            .take(2)
            .map(|e| e.id)
            .collect();
        let level = model.levels().next().unwrap().0.id;
        // a 1 m cube where the first wall stands
        let ElementKind::Wall(w) = &model.get(walls[0]).unwrap().kind else {
            unreachable!()
        };
        let at = (w.start + w.end) / 2.0;
        let f = creabim_core::FreeMesh::new(
            Some(level),
            at.extend(0.0),
            creabim_core::EditMesh::cuboid(glam::DVec3::splat(1.0)),
        );
        let shape = Element::new("Shape", ElementKind::FreeMesh(f));
        let sid = shape.id;
        app.doc
            .commit(Transaction::new("Shape").insert(shape))
            .unwrap();
        app.selection.extend([walls[0], walls[1], sid], false);
        let (got, mut on) = app.solid_op_pair().expect("a shape and two walls");
        on.sort_by_key(|i| i.0);
        let mut want = walls.clone();
        want.sort_by_key(|i| i.0);
        assert_eq!((got, on), (sid, want));
        // Subtract: one live cut each, one undo step
        app.run_solid_op("solid:op:1");
        let cuts = app.cuts_by(sid, &walls);
        assert_eq!(cuts.len(), 2);
        assert_eq!(
            app.solid_now(sid, &walls),
            (Some(BoolOp::Difference), Reach::AsDrawn)
        );
        // Reach both changes those cuts, not new ones
        app.run_solid_op("solid:reach:3");
        assert_eq!(app.cuts_by(sid, &walls).len(), 2);
        assert_eq!(app.solid_now(sid, &walls).1, Reach::Both);
        // Add instead: the same cuts change, reach kept
        app.run_solid_op("solid:op:0");
        assert_eq!(app.cuts_by(sid, &walls).len(), 2);
        assert_eq!(
            app.solid_now(sid, &walls),
            (Some(BoolOp::Union), Reach::Both)
        );
        // the strip lights what is on
        let acts = app.solid_op_acts(sid, &walls);
        assert!(acts.iter().any(|a| a.id == "solid:op:0" && a.on));
        assert!(acts.iter().any(|a| a.id == "solid:reach:3" && a.on));
        // a door is not a solid to cut: no strip rows then
        app.selection.extend([sid], false);
        assert!(app.solid_op_pair().is_none());
    }

    #[test]
    fn reach_runs_a_cutter_top_and_bottom_on() {
        use creabim_geom::mesh::TriMesh;
        let m = TriMesh {
            positions: vec![
                [0.0, 0.0, 1.0],
                [1.0, 0.0, 1.0],
                [0.0, 0.0, 2.0],
                [1.0, 0.0, 2.0],
            ],
            indices: vec![0, 1, 2, 1, 3, 2],
            ..Default::default()
        };
        let up = creabim_geom::scene::reached_tool(&m, Reach::Up);
        assert!(up.positions[2][2] > 500.0 && up.positions[0][2] == 1.0);
        let both = creabim_geom::scene::reached_tool(&m, Reach::Both);
        assert!(both.positions[0][2] < -500.0 && both.positions[3][2] > 500.0);
        assert_eq!(
            creabim_geom::scene::reached_tool(&m, Reach::AsDrawn).positions,
            m.positions
        );
    }
}
