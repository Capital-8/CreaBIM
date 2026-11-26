//! The common properties of walls, slabs, roofs, columns, beams, doors and
//! windows (Common properties REV00, owner: A on all five): what IFC's
//! Pset_WallCommon and the like carry, and schedules list.
//!
//! - Fire, acoustic, security, U-value and accessible are **declared**:
//!   information only, carried as given, never checked. They live on the
//!   type ([`crate::types::ElementType::ratings`]); an element may set its
//!   own ([`crate::Element::ratings`]), field by field. A door or window
//!   keeps its own in its construction, as before.
//! - **Load-bearing** is the type's switch, set or not; not set, it is
//!   worked out from the layers: a core of masonry, concrete, stone or
//!   steel bears; a stud or timber frame may or may not (an outer wall of
//!   a timber house carries the floor above, a partition doesn't), so it
//!   is not known and not written until set; a core of neither (plaster,
//!   insulation) does not. Columns and beams bear.
//! - **External** is worked out from the model: a wall with the outside
//!   on one side (no room there), a slab or column outside every room, a
//!   roof, a door or window in an external wall. With no rooms drawn it is
//!   not known, and nothing is written. One element may say Yes or No.

use crate::construction::Ratings;
use crate::{Element, ElementKind, Model};
use glam::DVec2;

/// Where a value came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum From {
    /// The element's type.
    Type,
    /// The element itself.
    Own,
    /// Worked out from the model.
    WorkedOut,
}

/// An element's common properties as they stand: declared values (its
/// own over its type's), load-bearing and external with where each came
/// from.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Common {
    pub declared: Ratings,
    pub load_bearing: Option<(bool, From)>,
    pub external: Option<(bool, From)>,
}

/// Can `kind` carry common properties?
pub fn rated(kind: &ElementKind) -> bool {
    matches!(
        kind,
        ElementKind::Wall(_)
            | ElementKind::Slab(_)
            | ElementKind::Roof(_)
            | ElementKind::Column(_)
            | ElementKind::Beam(_)
    ) || matches!(kind, ElementKind::Opening(o) if o.kind != crate::OpeningKind::Hole)
}

/// `own` over `base`, field by field: an empty field is `base`'s.
pub fn merged(base: &Ratings, own: &Ratings) -> Ratings {
    let s = |o: &String, b: &String| {
        if o.trim().is_empty() {
            b.clone()
        } else {
            o.clone()
        }
    };
    Ratings {
        fire: s(&own.fire, &base.fire),
        acoustic: s(&own.acoustic, &base.acoustic),
        security: s(&own.security, &base.security),
        u_value: own.u_value.or(base.u_value),
        accessible: own.accessible || base.accessible,
        load_bearing: own.load_bearing.or(base.load_bearing),
        external: own.external.or(base.external),
    }
}

/// The type's ratings for `e`, if it has a type.
pub fn type_ratings<'a>(model: &'a Model, e: &Element) -> Option<&'a Ratings> {
    crate::types::type_of(model, e).map(|(_, t)| &t.ratings)
}

/// The common properties of `e`, worked out where they are not set.
pub fn common(model: &Model, e: &Element) -> Common {
    if !rated(&e.kind) {
        return Common::default();
    }
    let base = type_ratings(model, e).cloned().unwrap_or_default();
    let own = match &e.kind {
        // a door or window keeps its own in its construction, as before
        ElementKind::Opening(o) => merged(
            &o.frame.construction.ratings,
            e.ratings.as_deref().unwrap_or(&Ratings::default()),
        ),
        _ => e.ratings.as_deref().cloned().unwrap_or_default(),
    };
    let declared = merged(&base, &own);
    let load_bearing = match (own.load_bearing, base.load_bearing) {
        (Some(v), _) => Some((v, From::Own)),
        (None, Some(v)) => Some((v, From::Type)),
        _ => bears(model, e).map(|v| (v, From::WorkedOut)),
    };
    let external = match own.external {
        Some(v) => Some((v, From::Own)),
        None => external(model, e).map(|v| (v, From::WorkedOut)),
    };
    Common {
        declared: Ratings {
            load_bearing: None,
            external: None,
            ..declared
        },
        load_bearing,
        external,
    }
}

/// Words a bearing material's name or category carries.
const BEARING: &[&str] = &[
    "concrete",
    "masonry",
    "block",
    "brick",
    "stone",
    "steel",
    "clt",
    "cross-laminated",
    "rammed",
    "adobe",
    "earth",
];

/// Words a frame core carries: it may bear or not, so it is not guessed.
const FRAME: &[&str] = &["stud", "timber", "wood", "frame", "lumber", "joist"];

/// Whether a material bears: yes, no, or not known (a frame).
fn heavy(model: &Model, material: Option<crate::ElementId>) -> Option<bool> {
    let Some(m) = material.and_then(|m| model.get(m)) else {
        return Some(false);
    };
    let mut text = m.name.to_lowercase();
    if let ElementKind::Material(mat) = &m.kind {
        text.push(' ');
        text.push_str(&mat.category.to_lowercase());
    }
    // a solid timber panel bears, though it says "timber"
    if ["clt", "cross-laminated", "cross laminated"]
        .iter()
        .any(|w| text.contains(w))
    {
        return Some(true);
    }
    if FRAME.iter().any(|w| text.contains(w)) {
        return None;
    }
    Some(BEARING.iter().any(|w| text.contains(w)))
}

/// Load-bearing, worked out: columns and beams bear; a wall or slab bears
/// when its structure layer (or its one material) is of a bearing
/// material. Roofs, doors and windows: not worked out.
fn bears(model: &Model, e: &Element) -> Option<bool> {
    let (assembly, material) = match &e.kind {
        ElementKind::Column(_) | ElementKind::Beam(_) => return Some(true),
        ElementKind::Wall(w) if w.curtain.is_some() => return Some(false),
        ElementKind::Wall(w) => (w.assembly, w.material),
        ElementKind::Slab(s) => (s.assembly, s.material),
        _ => return None,
    };
    let core = assembly
        .and_then(|a| model.get(a))
        .and_then(|a| match &a.kind {
            ElementKind::Assembly(asm) => asm
                .layers
                .iter()
                .find(|l| l.function == crate::assembly::LayerFunction::Structure)
                .map(|l| l.material),
            _ => None,
        });
    heavy(model, core.flatten().or(material))
}

/// The room outlines on `level`.
fn rooms_on(model: &Model, level: crate::ElementId) -> Vec<Vec<DVec2>> {
    model
        .iter()
        .filter_map(|r| match &r.kind {
            ElementKind::Room(room) if room.level == level => crate::room::outline(model, r.id)
                .or_else(|| (!room.boundary.is_empty()).then(|| room.boundary.clone())),
            _ => None,
        })
        .filter(|o| o.len() >= 3)
        .collect()
}

fn in_a_room(rooms: &[Vec<DVec2>], p: DVec2) -> bool {
    rooms.iter().any(|o| crate::polygon::contains(o, p))
}

/// External, worked out; none when the level has no rooms to tell by.
fn external(model: &Model, e: &Element) -> Option<bool> {
    match &e.kind {
        ElementKind::Roof(_) => Some(true),
        ElementKind::Wall(w) => {
            let rooms = rooms_on(model, w.level);
            if rooms.is_empty() {
                return None;
            }
            let d = w.end - w.start;
            if d.length() < 1e-6 {
                return None;
            }
            let n = DVec2::new(-d.y, d.x).normalize();
            let reach = w.thickness / 2.0 + 0.25;
            // a few points along it, each side: outside when one side is
            // in no room along most of it
            let mut outside = 0;
            let samples = [0.25, 0.5, 0.75];
            for t in samples {
                let p = w.start + d * t;
                let a = in_a_room(&rooms, p + n * reach);
                let b = in_a_room(&rooms, p - n * reach);
                if a != b {
                    outside += 1;
                }
            }
            Some(outside * 2 > samples.len())
        }
        ElementKind::Slab(s) => {
            // a slab nobody stands on indoors: a terrace, a balcony
            let rooms = rooms_on(model, s.level);
            if rooms.is_empty() || s.boundary.len() < 3 {
                return None;
            }
            let c = s.boundary.iter().copied().sum::<DVec2>() / s.boundary.len() as f64;
            Some(!in_a_room(&rooms, c))
        }
        ElementKind::Column(c) => {
            let rooms = rooms_on(model, c.level);
            (!rooms.is_empty()).then(|| !in_a_room(&rooms, c.position))
        }
        ElementKind::Beam(b) => {
            let rooms = rooms_on(model, b.level);
            (!rooms.is_empty()).then(|| !in_a_room(&rooms, (b.start + b.end) / 2.0))
        }
        // a door or window: as the wall it is in
        ElementKind::Opening(o) => {
            let host = model.get(o.host)?;
            let c = common(model, host);
            c.external.map(|(v, _)| v)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn own_values_sit_over_the_types_field_by_field() {
        let base = Ratings {
            fire: "REI 60".into(),
            u_value: Some(0.28),
            load_bearing: Some(true),
            ..Default::default()
        };
        let own = Ratings {
            acoustic: "Rw 52 dB".into(),
            u_value: Some(0.2),
            ..Default::default()
        };
        let m = merged(&base, &own);
        assert_eq!(m.fire, "REI 60");
        assert_eq!(m.acoustic, "Rw 52 dB");
        assert_eq!(m.u_value, Some(0.2));
        assert_eq!(m.load_bearing, Some(true));
    }

    #[test]
    fn the_starter_house_works_out_its_outer_walls_as_external() {
        let m = crate::starter::starter_house();
        let walls: Vec<&Element> = m
            .iter()
            .filter(|e| matches!(e.kind, ElementKind::Wall(_)))
            .collect();
        let ext: Vec<Option<bool>> = walls
            .iter()
            .map(|w| common(&m, w).external.map(|(v, _)| v))
            .collect();
        assert!(
            ext.contains(&Some(true)),
            "some walls face the outside: {ext:?}"
        );
        assert!(
            ext.iter().all(|x| x.is_some()),
            "rooms are drawn, so each wall is known: {ext:?}"
        );
    }

    #[test]
    fn a_stud_frame_core_is_not_guessed() {
        // a stud frame may bear (an outer wall carrying the floor above)
        // or not (a partition): nothing is said until set
        let mut m = crate::starter::starter_house();
        let w = m
            .iter()
            .find(|e| e.name == "FF South")
            .expect("FF South")
            .clone();
        assert_eq!(
            common(&m, &w).load_bearing.map(|x| x.0),
            Some(true),
            "a block core bears"
        );
        let ElementKind::Wall(wl) = &w.kind else {
            unreachable!()
        };
        let Some(ElementKind::Assembly(a)) =
            wl.assembly.and_then(|a| m.get(a)).map(|a| a.kind.clone())
        else {
            panic!("an assembly")
        };
        let core = a
            .layers
            .iter()
            .find(|l| l.function == crate::assembly::LayerFunction::Structure)
            .and_then(|l| l.material)
            .unwrap();
        let before = m.get(core).unwrap().clone();
        let mut mat = before.clone();
        mat.name = "Stud frame, 2x6 at 16\" o.c.".into();
        if let ElementKind::Material(x) = &mut mat.kind {
            x.category = "wood".into();
        }
        crate::Transaction::new("Stud")
            .replace(before.clone(), mat.clone())
            .apply(&mut m)
            .unwrap();
        assert_eq!(common(&m, &w).load_bearing, None);
        // a cross-laminated timber panel does bear
        let mut clt = mat.clone();
        clt.name = "Cross-laminated timber panels".into();
        crate::Transaction::new("CLT")
            .replace(mat, clt)
            .apply(&mut m)
            .unwrap();
        assert_eq!(common(&m, &w).load_bearing.map(|x| x.0), Some(true));
    }

    #[test]
    fn with_no_rooms_external_is_not_known() {
        let m = crate::sample::sample_house();
        if m.iter().any(|e| matches!(e.kind, ElementKind::Room(_))) {
            return;
        }
        let w = m
            .iter()
            .find(|e| matches!(e.kind, ElementKind::Wall(_)))
            .unwrap();
        assert_eq!(common(&m, w).external, None);
    }
}
