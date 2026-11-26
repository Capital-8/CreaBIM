//! Settings carried from one element to others (Create similar, Pick up
//! and Apply; Mockup_Create_Similar_And_Pick_Up REV01, decided
//! 2026-11-21): how it is made and how it looks, never which one it is,
//! where it is or which group it is in. The settings come in groups the
//! user ticks; a group goes across only where the target has it too.

use crate::element::{Element, ElementKind, OpeningKind};
use crate::id::ElementId;
use crate::model::Model;
use crate::ops::Transaction;

/// A group of settings that goes across together.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Group {
    /// A door's or window's width and height; a beam's width and depth.
    Size,
    /// A window's sill.
    Sill,
    /// A door's or window's frame, panes and how it opens.
    Frame,
    /// How a door or window is made, and its trim.
    Make,
    /// What is declared for it: fire, acoustic, security, U-value.
    Ratings,
    /// A wall's or slab's thickness, assembly or curtain grid; a column's
    /// profile.
    Build,
    /// A wall's or column's height, base and top.
    Height,
    /// The line a wall is drawn along.
    Location,
    /// A slab's top above its level.
    Offset,
    /// A room's floor, wall and ceiling finishes.
    Finishes,
    /// What a room is used for and the department it belongs to.
    Use,
    /// A roof's shape and pitch.
    Pitch,
    /// How far a roof runs past its walls.
    Overhang,
    /// A stair's steps: the highest riser and the tread.
    Steps,
    /// A note's, tag's or dimension's text style and colour.
    Style,
    Material,
    Type,
    Phase,
}

impl Group {
    pub const ALL: [Group; 18] = [
        Group::Size,
        Group::Sill,
        Group::Frame,
        Group::Make,
        Group::Ratings,
        Group::Build,
        Group::Height,
        Group::Location,
        Group::Offset,
        Group::Finishes,
        Group::Use,
        Group::Pitch,
        Group::Overhang,
        Group::Steps,
        Group::Style,
        Group::Material,
        Group::Type,
        Group::Phase,
    ];

    pub fn label(self, kind: &ElementKind) -> &'static str {
        match (self, kind) {
            (Group::Build, ElementKind::Wall(_)) => "Thickness and build-up",
            (Group::Build, ElementKind::Slab(_)) => "Thickness and build-up",
            (Group::Build, ElementKind::Roof(_)) => "Thickness and build-up",
            (Group::Build, ElementKind::Ceiling(_)) => "Thickness",
            (Group::Build, ElementKind::Railing(_)) => "Rail and posts",
            (Group::Build, _) => "Profile",
            (Group::Size, ElementKind::Stair(_)) => "Width",
            (Group::Size, _) => "Size",
            (Group::Finishes, _) => "Finishes",
            (Group::Use, _) => "Use and department",
            (Group::Pitch, _) => "Shape and pitch",
            (Group::Overhang, _) => "Overhang",
            (Group::Steps, _) => "Riser and tread",
            (Group::Style, _) => "Text style and colour",
            (Group::Height, ElementKind::Railing(_) | ElementKind::Room(_)) => "Height",
            (Group::Sill, _) => "Sill",
            (Group::Frame, _) => "Frame, panes, opens",
            (Group::Make, _) => "Make and trim",
            (Group::Ratings, _) => "Declared ratings",
            (Group::Height, _) => "Height, base and top",
            (Group::Location, _) => "Location line",
            (Group::Offset, _) => "Height offset",
            (Group::Material, _) => "Material",
            (Group::Type, _) => "Type",
            (Group::Phase, _) => "Phase",
        }
    }

    /// Ticked at first: every group but phase (when it is built is not
    /// how it is made).
    pub fn on_at_first(self) -> bool {
        self != Group::Phase
    }
}

/// The groups `e` has settings in.
pub fn groups(e: &Element) -> Vec<Group> {
    let mut out = vec![];
    match &e.kind {
        ElementKind::Opening(o) => {
            out.push(Group::Size);
            if o.kind == OpeningKind::Window && o.roof_at.is_none() {
                out.push(Group::Sill);
            }
            if o.kind != OpeningKind::Hole {
                out.extend([Group::Frame, Group::Make, Group::Ratings]);
            }
        }
        ElementKind::Wall(_) => out.extend([Group::Build, Group::Height, Group::Location]),
        ElementKind::Slab(_) => out.extend([Group::Build, Group::Offset]),
        ElementKind::Column(_) => out.extend([Group::Build, Group::Height]),
        ElementKind::Beam(_) => out.push(Group::Size),
        ElementKind::Room(_) => out.extend([Group::Finishes, Group::Use, Group::Height]),
        ElementKind::Roof(_) => out.extend([Group::Pitch, Group::Overhang, Group::Build]),
        ElementKind::Stair(_) => out.extend([Group::Size, Group::Steps]),
        ElementKind::Railing(_) => out.extend([Group::Height, Group::Build]),
        // a ceiling's height comes from its room and level: only how it
        // is made goes across
        ElementKind::Ceiling(_) => out.push(Group::Build),
        ElementKind::Note(n) if n.symbol.is_none() => out.push(Group::Style),
        ElementKind::Tag(_) | ElementKind::Dimension(_) => out.push(Group::Style),
        _ => {}
    }
    if crate::library::material_slot(e).is_some() {
        out.push(Group::Material);
    }
    if crate::types::TypeCategory::of(&e.kind).is_some() {
        out.push(Group::Type);
    }
    out.push(Group::Phase);
    out
}

/// Does `dst` take `g` from `src`: both have it, and for a door's or
/// window's frame and make, both are the same kind of opening.
pub fn applies(src: &Element, dst: &Element, g: Group) -> bool {
    if !groups(src).contains(&g) || !groups(dst).contains(&g) {
        return false;
    }
    match g {
        Group::Type => {
            crate::types::TypeCategory::of(&src.kind) == crate::types::TypeCategory::of(&dst.kind)
        }
        Group::Frame | Group::Make => matches!(
            (&src.kind, &dst.kind),
            (ElementKind::Opening(a), ElementKind::Opening(b)) if a.kind == b.kind
        ),
        Group::Material | Group::Phase => true,
        // text style and colour go between notes, tags and dimensions alike
        Group::Style => true,
        _ => std::mem::discriminant(&src.kind) == std::mem::discriminant(&dst.kind),
    }
}

/// `dst` given `src`'s settings in `on` (those it takes). Returns the
/// groups that changed it.
pub fn apply(src: &Element, dst: &mut Element, on: &[Group]) -> Vec<Group> {
    let mut changed = vec![];
    for &g in on {
        if !applies(src, dst, g) {
            continue;
        }
        let before = dst.clone();
        set(src, dst, g);
        if *dst != before {
            changed.push(g);
        }
    }
    changed
}

fn set(src: &Element, dst: &mut Element, g: Group) {
    match g {
        Group::Material => {
            if let Some(m) = crate::library::material_slot(src) {
                crate::library::set_material(dst, m);
            }
            return;
        }
        Group::Type => {
            dst.of_type = src.of_type.clone();
            return;
        }
        Group::Phase => {
            dst.phase = src.phase;
            return;
        }
        _ => {}
    }
    match (&src.kind, &mut dst.kind) {
        (ElementKind::Opening(a), ElementKind::Opening(b)) => match g {
            Group::Size => {
                // its middle stays where it was, kept within its wall
                let mid = b.offset + b.width / 2.0;
                b.width = a.width;
                b.height = a.height;
                b.offset = mid - b.width / 2.0;
            }
            Group::Sill => b.sill = a.sill,
            Group::Frame => {
                let built = b.frame.construction.clone();
                b.frame = a.frame.clone();
                b.frame.construction = built;
            }
            Group::Make => {
                let ratings = b.frame.construction.ratings.clone();
                b.frame.construction = a.frame.construction.clone();
                b.frame.construction.ratings = ratings;
            }
            Group::Ratings => b.frame.construction.ratings = a.frame.construction.ratings.clone(),
            _ => {}
        },
        (ElementKind::Wall(a), ElementKind::Wall(b)) => match g {
            Group::Build => {
                b.thickness = a.thickness;
                b.assembly = a.assembly;
                b.curtain = a.curtain.clone();
            }
            Group::Height => {
                b.height = a.height;
                b.base_offset = a.base_offset;
                b.top = a.top;
            }
            Group::Location => b.location = a.location,
            _ => {}
        },
        (ElementKind::Slab(a), ElementKind::Slab(b)) => match g {
            Group::Build => {
                b.thickness = a.thickness;
                b.assembly = a.assembly;
            }
            Group::Offset => b.top_offset = a.top_offset,
            _ => {}
        },
        (ElementKind::Column(a), ElementKind::Column(b)) => match g {
            Group::Build => b.profile = a.profile,
            Group::Height => {
                b.height = a.height;
                b.base_offset = a.base_offset;
                b.top = a.top;
            }
            _ => {}
        },
        (ElementKind::Beam(a), ElementKind::Beam(b)) => {
            if g == Group::Size {
                b.width = a.width;
                b.depth = a.depth;
                b.section = a.section.clone();
            }
        }
        (ElementKind::Room(a), ElementKind::Room(b)) => match g {
            Group::Finishes => {
                b.floor_finish = a.floor_finish.clone();
                b.wall_finish = a.wall_finish.clone();
                b.ceiling_finish = a.ceiling_finish.clone();
            }
            Group::Use => {
                b.occupancy = a.occupancy.clone();
                b.department = a.department.clone();
            }
            Group::Height => b.height = a.height,
            _ => {}
        },
        (ElementKind::Roof(a), ElementKind::Roof(b)) => match g {
            Group::Pitch => {
                b.shape = a.shape;
                b.pitch = a.pitch;
            }
            Group::Overhang => b.overhang = a.overhang,
            Group::Build => {
                b.thickness = a.thickness;
                b.assembly = a.assembly;
            }
            _ => {}
        },
        (ElementKind::Stair(a), ElementKind::Stair(b)) => match g {
            Group::Size => b.width = a.width,
            Group::Steps => {
                b.max_riser = a.max_riser;
                b.tread = a.tread;
            }
            _ => {}
        },
        (ElementKind::Railing(a), ElementKind::Railing(b)) => match g {
            Group::Height => b.height = a.height,
            Group::Build => {
                b.rail_profile = a.rail_profile.clone();
                b.post_spacing = a.post_spacing;
            }
            _ => {}
        },
        (ElementKind::Ceiling(a), ElementKind::Ceiling(b)) => {
            if g == Group::Build {
                b.thickness = a.thickness;
            }
        }
        (a, b) if g == Group::Style => {
            if let Some((style, color)) = text_look(a) {
                if let Some((s, c)) = text_look_mut(b) {
                    *s = style;
                    *c = color;
                }
            }
        }
        _ => {}
    }
}

/// A note's, tag's or dimension's text style and colour.
fn text_look(k: &ElementKind) -> Option<(Option<ElementId>, Option<[f32; 3]>)> {
    match k {
        ElementKind::Note(n) => Some((n.style, n.color)),
        ElementKind::Tag(t) => Some((t.style, t.color)),
        ElementKind::Dimension(d) => Some((d.style, d.color)),
        _ => None,
    }
}

fn text_look_mut(k: &mut ElementKind) -> Option<(&mut Option<ElementId>, &mut Option<[f32; 3]>)> {
    match k {
        ElementKind::Note(n) => Some((&mut n.style, &mut n.color)),
        ElementKind::Tag(t) => Some((&mut t.style, &mut t.color)),
        ElementKind::Dimension(d) => Some((&mut d.style, &mut d.color)),
        _ => None,
    }
}

/// What `g` of `e` is, in a few words, for the held list.
pub fn value(model: &Model, e: &Element, g: Group) -> String {
    let mm = |v: f64| format!("{:.0}", v * 1000.0);
    let name = |id: Option<ElementId>| {
        id.and_then(|i| model.get(i))
            .map_or_else(|| "None".to_string(), |x| x.name.clone())
    };
    match (g, &e.kind) {
        (Group::Material, _) => match crate::library::material_slot(e) {
            Some(Some(m)) => name(Some(m)),
            _ => "By type".into(),
        },
        (Group::Type, _) => name(e.of_type.as_ref().map(|t| t.id)),
        (Group::Phase, _) => {
            let list = crate::phases::list(model);
            e.phase
                .built
                .and_then(|b| list.iter().find(|p| p.id == b))
                .or_else(|| list.first())
                .map_or_else(|| "First".into(), |p| p.name.clone())
        }
        (Group::Size, ElementKind::Opening(o)) => format!("{} × {}", mm(o.width), mm(o.height)),
        (Group::Size, ElementKind::Beam(b)) => format!("{} × {}", mm(b.width), mm(b.depth)),
        (Group::Sill, ElementKind::Opening(o)) => mm(o.sill),
        (Group::Frame, ElementKind::Opening(o)) => {
            format!(
                "{} · {}",
                mm(o.frame.width),
                o.frame.operation(o.kind).short_label(o.kind)
            )
        }
        (Group::Make, ElementKind::Opening(o)) => {
            use crate::construction::LeafStyle;
            match (o.kind, o.frame.construction.leaf) {
                // a window's plain make is Standard, as Properties says
                (OpeningKind::Window, LeafStyle::Flush) => "Standard".into(),
                (_, l) => l.label().to_string(),
            }
        }
        (Group::Ratings, ElementKind::Opening(o)) => {
            let r = &o.frame.construction.ratings;
            let n = [
                !r.fire.is_empty(),
                !r.acoustic.is_empty(),
                !r.security.is_empty(),
                r.u_value.is_some(),
                r.accessible,
            ]
            .iter()
            .filter(|x| **x)
            .count();
            if n == 0 {
                "None".into()
            } else {
                n.to_string()
            }
        }
        (Group::Build, ElementKind::Wall(w)) => match w.assembly {
            Some(a) => name(Some(a)),
            None => mm(w.thickness),
        },
        (Group::Build, ElementKind::Slab(s)) => match s.assembly {
            Some(a) => name(Some(a)),
            None => mm(s.thickness),
        },
        (Group::Build, ElementKind::Column(_)) => "Its profile".into(),
        (Group::Height, ElementKind::Wall(w)) => mm(w.height),
        (Group::Height, ElementKind::Column(c)) => {
            c.height.map_or_else(|| "To the level".into(), mm)
        }
        (Group::Location, ElementKind::Wall(w)) => crate::walls::location_label(model, w),
        (Group::Offset, ElementKind::Slab(s)) => mm(s.top_offset),
        (Group::Finishes, ElementKind::Room(r)) => {
            let f: Vec<&str> = [&r.floor_finish, &r.wall_finish, &r.ceiling_finish]
                .iter()
                .map(|s| if s.is_empty() { "—" } else { s.as_str() })
                .collect();
            f.join(" / ")
        }
        (Group::Use, ElementKind::Room(r)) => {
            let u = [r.occupancy.as_str(), r.department.as_str()]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" · ");
            if u.is_empty() {
                "None".into()
            } else {
                u
            }
        }
        (Group::Height, ElementKind::Room(r)) => {
            r.height.map_or_else(|| "To the ceiling".into(), mm)
        }
        (Group::Pitch, ElementKind::Roof(r)) => format!("{:?} {:.0}°", r.shape, r.pitch),
        (Group::Overhang, ElementKind::Roof(r)) => mm(r.overhang),
        (Group::Build, ElementKind::Roof(r)) => match r.assembly {
            Some(a) => name(Some(a)),
            None => mm(r.thickness),
        },
        (Group::Size, ElementKind::Stair(st)) => mm(st.width),
        (Group::Steps, ElementKind::Stair(st)) => {
            format!("{} / {}", mm(st.max_riser), mm(st.tread))
        }
        (Group::Height, ElementKind::Railing(r)) => mm(r.height),
        (Group::Build, ElementKind::Railing(r)) => {
            format!(
                "{} · {}",
                r.rail_profile.as_deref().unwrap_or("Plain"),
                mm(r.post_spacing)
            )
        }
        (Group::Build, ElementKind::Ceiling(c)) => mm(c.thickness),
        (Group::Style, k) => match text_look(k) {
            Some((st, c)) => {
                let base = st.map_or_else(|| "Default".to_string(), |i| name(Some(i)));
                if c.is_some() {
                    format!("{base}, own colour")
                } else {
                    base
                }
            }
            None => String::new(),
        },
        _ => String::new(),
    }
}

/// What applying `on` from `src` would change on `dst`: (group's label,
/// now, after), for the pointer's preview.
pub fn changes(
    model: &Model,
    src: &Element,
    dst: &Element,
    on: &[Group],
) -> Vec<(&'static str, String, String)> {
    let mut after = dst.clone();
    let changed = apply(src, &mut after, on);
    changed
        .into_iter()
        .map(|g| {
            (
                g.label(&dst.kind),
                value(model, dst, g),
                value(model, &after, g),
            )
        })
        .collect()
}

/// One step giving each of `targets` (but `src` itself) `src`'s settings
/// in `on`, and how many it changed.
pub fn apply_to(
    model: &Model,
    src: &Element,
    targets: &[ElementId],
    on: &[Group],
) -> (Transaction, usize) {
    let mut tx = Transaction::new(format!("Apply settings of {}", src.name));
    let mut n = 0;
    for id in targets {
        if *id == src.id {
            continue;
        }
        let Some(e) = model.get(*id) else { continue };
        let mut after = e.clone();
        if apply(src, &mut after, on).is_empty() {
            continue;
        }
        if let ElementKind::Opening(o) = &mut after.kind {
            if o.roof_at.is_none() {
                crate::walls::clamp_opening(model, o);
            }
        }
        tx = tx.replace(e.clone(), after);
        n += 1;
    }
    (tx, n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::{Level, Opening, Wall};
    use glam::DVec2;

    fn model() -> (Model, ElementId, ElementId, ElementId, ElementId) {
        let mut m = Model::default();
        let l = Element::new(
            "L",
            ElementKind::Level(Level {
                elevation: 0.0,
                height: 3.0,
            }),
        );
        let w = Element::new(
            "W",
            ElementKind::Wall(Wall::new(
                l.id,
                DVec2::ZERO,
                DVec2::new(10.0, 0.0),
                0.2,
                2.7,
            )),
        );
        let win = |name: &str, kind, offset, width| {
            Element::new(
                name,
                ElementKind::Opening(Opening {
                    host: w.id,
                    kind,
                    offset,
                    sill: 0.9,
                    width,
                    height: 1.2,
                    material: None,
                    frame: crate::OpeningFrame::for_kind(kind),
                    flip_hand: false,
                    flip_facing: false,
                    roof_at: None,
                    roof_build: Default::default(),
                    arrangement: None,
                }),
            )
        };
        let mut a = win("A", OpeningKind::Window, 1.0, 1.6);
        if let ElementKind::Opening(o) = &mut a.kind {
            o.sill = 0.6;
            o.height = 1.5;
            o.frame.construction.ratings.fire = "FD30".into();
        }
        a.phase.built = Some(2);
        let mut b = win("B", OpeningKind::Window, 5.0, 1.0);
        b.mark = Some("W9".into());
        let d = win("D", OpeningKind::Door, 8.0, 0.9);
        let ids = (w.id, a.id, b.id, d.id);
        Transaction::new("s")
            .insert(l)
            .insert(w)
            .insert(a)
            .insert(b)
            .insert(d)
            .apply(&mut m)
            .unwrap();
        (m, ids.0, ids.1, ids.2, ids.3)
    }

    #[test]
    fn apply_carries_how_it_is_made_never_which_one_or_where() {
        let (mut m, wall, a, b, _) = model();
        let src = m.get(a).unwrap().clone();
        let on: Vec<Group> = Group::ALL.into_iter().filter(|g| g.on_at_first()).collect();
        let (tx, n) = apply_to(&m, &src, &[b], &on);
        assert_eq!(n, 1);
        tx.apply(&mut m).unwrap();
        let e = m.get(b).unwrap();
        let ElementKind::Opening(o) = &e.kind else {
            unreachable!()
        };
        assert_eq!((o.width, o.height, o.sill), (1.6, 1.5, 0.6));
        assert_eq!(o.frame.construction.ratings.fire, "FD30");
        // its middle stays at 5.5, its wall, mark and name its own
        assert!((o.offset + o.width / 2.0 - 5.5).abs() < 1e-9);
        assert_eq!(o.host, wall);
        assert_eq!(e.mark.as_deref(), Some("W9"));
        assert_eq!(e.name, "B");
        // phase is off at first
        assert_eq!(e.phase.built, None);
    }

    #[test]
    fn a_door_takes_from_a_window_only_what_both_have() {
        let (m, _, a, _, d) = model();
        let (src, dst) = (m.get(a).unwrap(), m.get(d).unwrap());
        assert!(applies(src, dst, Group::Size));
        assert!(applies(src, dst, Group::Ratings));
        assert!(!applies(src, dst, Group::Sill));
        assert!(!applies(src, dst, Group::Frame));
        assert!(!applies(src, dst, Group::Make));
        // a door and a window are different types
        assert!(!applies(src, dst, Group::Type));
        let mut after = dst.clone();
        let changed = apply(src, &mut after, &Group::ALL);
        let ElementKind::Opening(o) = &after.kind else {
            unreachable!()
        };
        assert_eq!(o.kind, OpeningKind::Door);
        assert_eq!(
            o.frame.operation(OpeningKind::Door),
            crate::OpeningFrame::for_kind(OpeningKind::Door).operation(OpeningKind::Door)
        );
        assert!(changed.contains(&Group::Size) && !changed.contains(&Group::Frame));
    }

    #[test]
    fn the_preview_says_what_changes() {
        let (m, _, a, b, _) = model();
        let c = changes(
            &m,
            m.get(a).unwrap(),
            m.get(b).unwrap(),
            &[Group::Size, Group::Sill, Group::Material],
        );
        assert_eq!(
            c,
            vec![
                ("Size", "1000 × 1200".to_string(), "1600 × 1500".to_string()),
                ("Sill", "900".to_string(), "600".to_string())
            ]
        );
        // nothing to change: nothing said
        assert!(changes(&m, m.get(a).unwrap(), m.get(a).unwrap(), &Group::ALL).is_empty());
    }

    #[test]
    fn walls_take_a_walls_build_up_height_and_line() {
        let (mut m, wall, _, _, _) = model();
        let l = match &m.get(wall).unwrap().kind {
            ElementKind::Wall(w) => w.level,
            _ => unreachable!(),
        };
        let mut src = Wall::new(l, DVec2::new(0.0, 5.0), DVec2::new(4.0, 5.0), 0.35, 3.2);
        src.location = crate::LocationLine::ExteriorFace;
        let s = Element::new("S", ElementKind::Wall(src));
        let sid = s.id;
        Transaction::new("s").insert(s).apply(&mut m).unwrap();
        let (tx, n) = apply_to(
            &m,
            m.get(sid).unwrap(),
            &[wall],
            &[Group::Build, Group::Height, Group::Location],
        );
        assert_eq!(n, 1);
        tx.apply(&mut m).unwrap();
        let ElementKind::Wall(w) = &m.get(wall).unwrap().kind else {
            unreachable!()
        };
        assert_eq!(
            (w.thickness, w.height, w.location),
            (0.35, 3.2, crate::LocationLine::ExteriorFace)
        );
        assert_eq!((w.start, w.end), (DVec2::ZERO, DVec2::new(10.0, 0.0)));
    }

    /// Pick up and Apply on the starter house's rooms, roofs, stairs,
    /// railings, ceilings and annotations: each takes what it has in
    /// common with the source, never where it is or which one it is
    /// (MU135: they carried only type, material and phase).
    #[test]
    fn rooms_roofs_stairs_railings_ceilings_and_text_take_their_settings() {
        let m = crate::starter::starter_house();
        let two = |f: fn(&ElementKind) -> bool| {
            let v: Vec<Element> = m.iter().filter(|e| f(&e.kind)).cloned().collect();
            assert!(!v.is_empty());
            (v[0].clone(), v.last().unwrap().clone())
        };
        let all = |e: &Element| groups(e);
        // rooms: finishes and use, never the name, number or outline
        let (mut a, mut b) = two(|k| matches!(k, ElementKind::Room(_)));
        if let ElementKind::Room(r) = &mut a.kind {
            r.floor_finish = "Porcelain tile".into();
            r.wall_finish = "Tiled to 1200".into();
            r.department = "Flat 2".into();
        }
        let (name, number) = (
            b.name.clone(),
            match &b.kind {
                ElementKind::Room(r) => r.number.clone(),
                _ => unreachable!(),
            },
        );
        let changed = apply(&a, &mut b, &all(&a));
        assert!(
            changed.contains(&Group::Finishes) && changed.contains(&Group::Use),
            "{changed:?}"
        );
        let ElementKind::Room(r) = &b.kind else {
            unreachable!()
        };
        assert_eq!(
            (r.floor_finish.as_str(), r.department.as_str()),
            ("Porcelain tile", "Flat 2")
        );
        assert_eq!((b.name.clone(), r.number.clone()), (name, number));
        // roofs: shape, pitch, overhang
        let (mut a, mut b) = two(|k| matches!(k, ElementKind::Roof(_)));
        if let ElementKind::Roof(r) = &mut a.kind {
            r.pitch = 42.0;
            r.overhang = 0.65;
        }
        apply(&a, &mut b, &all(&a));
        let ElementKind::Roof(r) = &b.kind else {
            unreachable!()
        };
        assert_eq!((r.pitch, r.overhang), (42.0, 0.65));
        // stairs: width and steps
        let (a, _) = two(|k| matches!(k, ElementKind::Stair(_)));
        let mut b = a.clone();
        if let ElementKind::Stair(st) = &mut b.kind {
            st.width = 0.8;
            st.tread = 0.3;
        }
        apply(&a, &mut b, &all(&a));
        assert_eq!(
            b.kind, a.kind,
            "the stair takes the other's width and steps"
        );
        // railings: height, rail and posts
        let (a, _) = two(|k| matches!(k, ElementKind::Railing(_)));
        let mut b = a.clone();
        if let ElementKind::Railing(r) = &mut b.kind {
            r.height = 1.1;
            r.post_spacing = 2.5;
        }
        apply(&a, &mut b, &all(&a));
        assert_eq!(b.kind, a.kind);
        // ceilings: thickness, never height
        let (a, _) = two(|k| matches!(k, ElementKind::Ceiling(_)));
        let mut b = a.clone();
        if let ElementKind::Ceiling(c) = &mut b.kind {
            c.thickness = 0.05;
            c.height = 2.2;
        }
        apply(&a, &mut b, &all(&a));
        let (ElementKind::Ceiling(ca), ElementKind::Ceiling(cb)) = (&a.kind, &b.kind) else {
            unreachable!()
        };
        assert_eq!(cb.thickness, ca.thickness);
        assert_eq!(cb.height, 2.2, "its height stays its own");
        // text: a note's style and colour go to a dimension
        let style = ElementId::new();
        let (mut note, _) = two(|k| matches!(k, ElementKind::Note(n) if n.symbol.is_none()));
        if let ElementKind::Note(n) = &mut note.kind {
            n.style = Some(style);
            n.color = Some([1.0, 0.0, 0.0]);
        }
        let (_, mut dim) = two(|k| matches!(k, ElementKind::Dimension(_)));
        assert_eq!(apply(&note, &mut dim, &all(&note)), vec![Group::Style]);
        let ElementKind::Dimension(d) = &dim.kind else {
            unreachable!()
        };
        assert_eq!((d.style, d.color), (Some(style), Some([1.0, 0.0, 0.0])));
    }
}
