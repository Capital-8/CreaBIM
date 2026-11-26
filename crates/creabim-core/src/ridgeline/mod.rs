//! Ridgeline House: the starter house of CreaBIM 1.2. A two-storey
//! cold-climate timber house on a sloping lot, built step by step the way
//! a user would build it. [`ridgeline_house`] is the finished house, as
//! *New project* opens it; [`build`] stops after each stage, for the
//! example that saves a milestone file of every one:
//!
//! `cargo run -p creabim-store --example showcase_house -- <folder>`
//!
//! The house is set out in feet on a 12 ft grid, x east and y north, the
//! origin on the south-west corner of the main block's wall centre lines.
//! The model itself is in metres, as every CreaBIM model is; the project's
//! units are feet and inches.
//!
//! The pictures of its hero views sheet are not drawn here: whoever has
//! them adds the sheet with [`hero_views_sheet`].

use crate::types::{ElementType, TypeParams};
use crate::{Document, Element, ElementId, ElementKind, Model, Transaction};
use glam::DVec2;
use std::collections::HashMap;

mod detailing;
mod drawings;
mod envelope;
mod interiors;
mod setup;
mod sheets;
mod site;
mod structure;

/// Metres in a foot.
pub const FT: f64 = 0.3048;
/// Feet to metres.
pub fn ft(x: f64) -> f64 {
    x * FT
}
/// Inches to metres.
pub fn inch(x: f64) -> f64 {
    x * 0.0254
}
/// A plan point given in feet.
pub fn p(x: f64, y: f64) -> DVec2 {
    DVec2::new(x * FT, y * FT)
}
/// A rectangle given in feet.
pub fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<DVec2> {
    crate::polygon::rectangle(p(x0, y0), p(x1, y1))
}

/// The exterior timber wall's thickness, and half of it.
pub const EXT: f64 = 0.257;
pub const H: f64 = EXT / 2.0;
/// How far the wall's framing sits in from its outer face: cladding,
/// cavity, exterior insulation and sheathing.
pub const FRAME_OUT: f64 = 0.019 + 0.019 + 0.051 + 0.011;
/// The partitions' thickness (2×4 studs and two boards), and half of it.
pub const PART: f64 = 0.121;
pub const HP: f64 = PART / 2.0;
/// Storey heights: ground floor to first floor, first floor to the plate.
pub const STOREY: f64 = 10.0 * FT;
pub const PLATE: f64 = 8.0 * FT;
/// The first floor's build-up.
pub const FLOOR: f64 = 0.356;
/// The finished ground beside the house, below the ground floor.
pub const GRADE: f64 = -0.2;

pub const PROJECT: &str = "Ridgeline House";
pub const ADDRESS: &str = "418 Ridgeline Road, Duluth, Minnesota";
pub const CLIENT: &str = "The Lindqvist family";

/// Builds the house step by step on one document.
pub struct Builder {
    pub doc: Document,
    /// Elements by the name they were given here.
    pub named: HashMap<String, ElementId>,
}

impl Builder {
    pub fn new() -> Self {
        Self {
            doc: Document::new(),
            named: HashMap::new(),
        }
    }

    pub fn id(&self, name: &str) -> ElementId {
        *self
            .named
            .get(name)
            .unwrap_or_else(|| panic!("the house has no {name}"))
    }

    pub fn commit(&mut self, tx: Transaction) {
        let label = tx.label.clone();
        self.doc
            .commit(tx)
            .unwrap_or_else(|e| panic!("{label}: {e:?}"));
    }

    /// Insert `e` under `key` in `tx`.
    pub fn keep(&mut self, key: &str, e: Element, tx: Transaction) -> Transaction {
        self.named.insert(key.to_string(), e.id);
        tx.insert(e)
    }

    /// A library material by name, added to the model if it is not there.
    pub fn material(&mut self, name: &str) -> ElementId {
        if let Some(id) = self.named.get(name) {
            return *id;
        }
        let (id, new) = crate::library::find_or_new(self.doc.model(), name)
            .unwrap_or_else(|| panic!("no library material {name}"));
        if let Some(e) = new {
            self.commit(Transaction::new("Add material").insert(e));
        }
        self.named.insert(name.into(), id);
        id
    }

    /// A type made of `params`, stamped onto elements later.
    pub fn add_type(&mut self, name: &str, description: &str, params: TypeParams) -> ElementId {
        let mut t = ElementType::new(params);
        t.description = description.into();
        let e = Element::new(name, ElementKind::ElementType(t));
        let id = e.id;
        self.named.insert(name.into(), id);
        self.commit(Transaction::new("Add type").insert(e));
        id
    }

    /// `e` stamped with the type named `t`.
    pub fn typed(&self, mut e: Element, t: &str) -> Element {
        let ok = crate::types::stamp(self.doc.model(), &mut e, self.id(t));
        assert!(ok, "{t} fits {}", e.name);
        e
    }
}

impl Default for Builder {
    fn default() -> Self {
        Self::new()
    }
}

/// The stages the house is built in, in order: the word that names each,
/// and the name of its milestone file.
pub const STAGES: [(&str, &str); 6] = [
    ("site", "01_site"),
    ("structure", "02_structure"),
    ("envelope", "03_envelope"),
    ("interiors", "04_interiors"),
    ("details", "05_details"),
    ("sheets", "06_sheets"),
];

/// Builds the house stage by stage, calling `after` with the house so far
/// and the stage's word once each is done, and stopping after the stage
/// named `until` (every stage when it names none).
pub fn build(until: &str, mut after: impl FnMut(&Builder, &str)) -> Builder {
    let mut b = Builder::new();
    setup::build(&mut b);
    // the views first, so every milestone opens on its drawings
    let views = drawings::views(&mut b);
    for (stage, _) in STAGES {
        match stage {
            "site" => site::build(&mut b),
            "structure" => structure::build(&mut b),
            "envelope" => envelope::build(&mut b),
            "interiors" => interiors::build(&mut b),
            "details" => {
                drawings::details(&mut b);
                drawings::annotate(&mut b, &views);
            }
            _ => sheets::build(&mut b, &views),
        }
        after(&b, stage);
        if stage == until {
            break;
        }
    }
    b
}

/// Ridgeline House, finished, with its views and sheets.
pub fn ridgeline_house() -> Model {
    build("", |_, _| {}).doc.model().clone()
}

/// A picture for the hero views sheet: its size in pixels and its pixels
/// as a title block keeps them (RGBA, deflated, in base64).
pub struct HeroPicture {
    pub width: u32,
    pub height: u32,
    pub rgba: String,
}

/// The hero views, in the order [`hero_views_sheet`] takes their
/// pictures: the file each is traced into, and its caption.
pub const HERO_VIEWS: [(&str, &str); 3] = [
    (
        "dusk-arrival",
        "1   THE ARRIVAL AT DUSK    21 December, 4:35 pm solar time, the house's own lights lit",
    ),
    (
        "daylight-south-west",
        "2   THE TERRACE SIDE    21 March, 2:30 pm",
    ),
    (
        "daylight-arrival",
        "3   THE ARRIVAL BY DAY    21 June, 9:30 am",
    ),
];

/// Sheet A-902, the hero views: `pictures` (one per [`HERO_VIEWS`]) kept
/// in a copy of the project's title block, so the project and its PDF
/// carry them. `None` when the model has no title block or no sheet.
pub fn hero_views_sheet(m: &Model, pictures: [HeroPicture; 3]) -> Option<Transaction> {
    use crate::titleblock::{TextAlign, TitleItem};
    let mut t = m.iter().find_map(|e| match &e.kind {
        ElementKind::SheetTemplate(t) => Some(t.clone()),
        _ => None,
    })?;
    let any = m.iter().find_map(|e| match &e.kind {
        ElementKind::Sheet(s) => Some(s.clone()),
        _ => None,
    })?;
    // lower left and upper right, in mm from the sheet's bottom right corner
    let places = [
        ((-884.0, 215.0), (-404.0, 515.0)),
        ((-394.0, 336.25), (-108.0, 515.0)),
        ((-394.0, 140.0), (-108.0, 318.75)),
    ];
    let text = |at: DVec2, size: f64, text: &str| TitleItem::Text {
        at,
        size,
        text: text.into(),
        align: TextAlign::Start,
        angle: 0.0,
        colour: None,
        font: None,
    };
    for ((pic, (_, caption)), (lo, hi)) in pictures.into_iter().zip(HERO_VIEWS).zip(places) {
        t.items.push(TitleItem::Image {
            lo: DVec2::new(lo.0, lo.1),
            hi: DVec2::new(hi.0, hi.1),
            width: pic.width,
            height: pic.height,
            rgba: pic.rgba,
            opacity: 1.0,
        });
        t.items
            .push(text(DVec2::new(lo.0, lo.1 - 7.0), 3.8, caption));
    }
    t.items.push(text(
        DVec2::new(-884.0, 190.0),
        3.2,
        "Traced by CreaBIM's own path tracer from the model: the sun where Duluth's latitude puts it, the lamps the project's light fittings.",
    ));
    let master = Element::new("Ridgeline hero views", ElementKind::SheetTemplate(t));
    let mut sheet = crate::Sheet {
        number: "A-902".into(),
        template: Some(master.id),
        ..any
    };
    sheet.guides.clear();
    Some(
        Transaction::new("Hero views sheet")
            .insert(master)
            .insert(Element::new("Hero Views", ElementKind::Sheet(sheet))),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count(m: &Model, f: impl Fn(&ElementKind) -> bool) -> usize {
        m.iter().filter(|e| f(&e.kind)).count()
    }

    /// Each object of `m` on `level` named `name…`: its footprint's corners
    /// and the way it faces (a turn of 0 faces south).
    fn placed(m: &Model, level: &str, name: &str) -> Vec<([DVec2; 4], DVec2)> {
        m.iter()
            .filter(|e| e.name.starts_with(name))
            .filter_map(|e| {
                let ElementKind::Object(o) = &e.kind else {
                    return None;
                };
                if m.get(o.level).map(|l| l.name.as_str()) != Some(level) {
                    return None;
                }
                let (lo, hi) = crate::objects::type_of(m, e)?.bounds();
                let corners = [(lo.x, lo.y), (hi.x, lo.y), (hi.x, hi.y), (lo.x, hi.y)]
                    .map(|(x, y)| o.to_plan(DVec2::new(x, y)));
                let (s, c) = o.rotation.to_radians().sin_cos();
                Some((corners, DVec2::new(s, -c)))
            })
            .collect()
    }

    /// A bed and its bedside tables stand clear of the radiator by them
    /// (owner, MU141: the beds touched them).
    #[test]
    fn the_beds_stand_clear_of_the_radiators() {
        let m = ridgeline_house();
        let boxed = |c: &[DVec2; 4]| {
            c.iter()
                .fold((c[0], c[0]), |(a, b), p| (a.min(*p), b.max(*p)))
        };
        let mut beds = placed(&m, "First Floor", "Double bed");
        beds.extend(placed(&m, "First Floor", "Bedside table"));
        let rads = placed(&m, "First Floor", "Radiator");
        assert!(beds.len() >= 3 && rads.len() >= 3);
        for (bed, _) in &beds {
            let (blo, bhi) = boxed(bed);
            for (rad, _) in &rads {
                let (rlo, rhi) = boxed(rad);
                let gap = (rlo - bhi).max(blo - rhi).max_element();
                assert!(
                    gap > 0.025,
                    "a bed {:.0} mm from a radiator: {:.2?} {:.2?} and {:.2?} {:.2?} ft",
                    gap * 1000.0,
                    blo / FT,
                    bhi / FT,
                    rlo / FT,
                    rhi / FT
                );
            }
        }
    }

    /// The two armchairs by the east slider face the low table between
    /// them, as seats serve what they are set round (owner, MU141: they
    /// faced away from it).
    #[test]
    fn the_armchairs_face_the_table_they_serve() {
        let m = ridgeline_house();
        let table = p(18.1, 6.8);
        let chairs: Vec<_> = placed(&m, "Ground Floor", "Armchair")
            .into_iter()
            .filter(|(c, _)| ((c[0] + c[2]) / 2.0).distance(table) < ft(6.0))
            .collect();
        assert_eq!(chairs.len(), 2);
        for (c, facing) in chairs {
            let to = (table - (c[0] + c[2]) / 2.0).normalize();
            let off = facing.angle_to(to).to_degrees().abs();
            assert!(off < 20.0, "an armchair faces {off:.0}° off the table");
        }
    }

    /// The finished house has its site, its structure, its rooms and its
    /// fifteen drawn sheets, and nothing sits on a copy of itself.
    #[test]
    fn the_house_builds_whole() {
        let m = ridgeline_house();
        assert!(m.len() > 700, "{} elements", m.len());
        assert_eq!(count(&m, |k| matches!(k, ElementKind::Sheet(_))), 15);
        assert_eq!(count(&m, |k| matches!(k, ElementKind::Level(_))), 4);
        assert!(count(&m, |k| matches!(k, ElementKind::Wall(_))) >= 40);
        assert!(count(&m, |k| matches!(k, ElementKind::Room(_))) >= 17);
        assert!(count(&m, |k| matches!(k, ElementKind::Site(_))) >= 30);
        assert!(count(&m, |k| matches!(k, ElementKind::Footing(_))) >= 13);
        assert!(crate::checks::stacked_copies(&m).is_empty());
    }

    /// The details draft nothing the model does not bear out: no eave
    /// set, no ledger under the canopy, no sill plate where the section
    /// passes through the slider; and the details sheet says the details
    /// are work in progress (owner, 2026-11-22).
    #[test]
    fn the_details_leave_out_what_the_model_does_not_carry() {
        let m = ridgeline_house();
        let notes: Vec<&str> = m
            .iter()
            .filter_map(|e| match &e.kind {
                ElementKind::Note(n) => Some(n.text.as_str()),
                _ => None,
            })
            .collect();
        for gone in [
            "FASCIA",
            "GUTTER",
            "SOFFIT",
            "LEDGER",
            "BASE FLASHING",
            "PT 2×6 SILL",
            "APRON",
            "BAFFLE",
        ] {
            assert!(
                !notes.iter().any(|t| t.contains(gone)),
                "a note still names {gone}"
            );
        }
        // the wall's layers are named once, where a wall is cut
        assert_eq!(
            notes
                .iter()
                .filter(|t| t.contains("CEDAR, VERTICAL"))
                .count(),
            1
        );
        assert_eq!(
            notes
                .iter()
                .filter(|t| t.contains("work in progress"))
                .count(),
            1
        );
        // 32 pieces were drafted; the seven that floated are gone
        assert_eq!(count(&m, |k| matches!(k, ElementKind::Detail(_))), 25);
    }

    /// Owner, MU130: "we provided sky light and shut it out with the
    /// ceilings". No ceiling hangs under either roof window, so each one
    /// lights the room under it; every fitting hung from a ceiling upstairs
    /// still has one over it, and the landing stays one room.
    #[test]
    fn the_roof_windows_light_the_landing() {
        let m = ridgeline_house();
        let f = m.iter().find(|e| e.name == "First Floor").unwrap().id;
        for name in ["Roof window, landing", "Roof window, stair"] {
            let e = m.iter().find(|e| e.name == name).unwrap();
            let ElementKind::Opening(o) = &e.kind else {
                panic!()
            };
            let at = o.roof_at.unwrap();
            assert!(
                crate::room::ceiling_over(&m, f, at, f64::MAX).is_none(),
                "a ceiling under the {name}"
            );
        }
        for e in m.iter() {
            let ElementKind::Object(o) = &e.kind else {
                continue;
            };
            if o.level == f && crate::objects::hung(&m, e) {
                assert!(
                    crate::room::ceiling_over(&m, f, o.position, f64::MAX).is_some(),
                    "{} hangs from no ceiling at {:?}",
                    e.name,
                    o.position / FT
                );
            }
        }
        let landing = m.iter().filter(|e| e.name == "Landing").count();
        assert_eq!(landing, 1);
    }

    /// Owner, MU130: "look at that slab in the wall ... the wall will be
    /// continuous in that area". The first floor does not run into the
    /// north wall over the stair well; elsewhere it bears on that wall.
    #[test]
    fn the_north_wall_runs_on_past_the_stair_well() {
        let m = ridgeline_house();
        let floor = m
            .iter()
            .find_map(|e| match &e.kind {
                ElementKind::Slab(s) if e.name == "First floor" => Some(s),
                _ => None,
            })
            .unwrap();
        let in_wall = |x: f64| DVec2::new(x, ft(24.0) - 0.05);
        let covers = |q: DVec2| {
            crate::polygon::contains(&floor.boundary, q)
                && !floor.holes.iter().any(|h| crate::polygon::contains(h, q))
        };
        let mid = (envelope::stair_head() + envelope::well_end()) / 2.0;
        assert!(
            !covers(in_wall(mid)),
            "the floor runs into the wall at the well"
        );
        assert!(
            covers(in_wall(ft(30.0))),
            "the floor bears on the wall beside the well"
        );
        assert!(
            covers(in_wall(ft(4.0))),
            "the floor bears on the wall beside the well"
        );
    }

    /// The site plan shows what stands outdoors (owner's markups: the
    /// court's steps could be neither seen nor selected there; "fix it
    /// everywhere there are similar items"): the steps and their rail,
    /// the two parked cars. The stair and guards indoors, the furniture
    /// and the figures are hidden there, and nothing else of those kinds
    /// is hidden by its category.
    #[test]
    fn the_site_plan_shows_what_stands_outdoors() {
        use crate::visibility::Category as C;
        let m = ridgeline_house();
        let view = m
            .iter()
            .find_map(|e| match &e.kind {
                ElementKind::View(v) if e.name == "Site plan" => Some(v),
                _ => None,
            })
            .unwrap();
        for c in [
            C::Stairs,
            C::Railings,
            C::Objects,
            C::Walls,
            C::Slabs,
            C::Columns,
            C::Site,
            C::Planting,
        ] {
            assert!(!view.hidden.categories.contains(&c), "{c:?} is hidden");
        }
        let mut shown = vec![];
        for e in m.iter() {
            if !matches!(
                e.kind,
                ElementKind::Stair(_) | ElementKind::Railing(_) | ElementKind::Object(_)
            ) {
                continue;
            }
            if !view.hidden.elements.contains(&e.id) {
                shown.push(e.name.as_str());
            }
        }
        shown.sort();
        assert_eq!(shown, ["Car", "Car", "Court steps", "Court steps rail"]);
    }

    /// What stands outdoors is part of the site, so the floor plans,
    /// which show no site, leave it out (MU136: the cars, the court steps
    /// and the retaining wall were drawn on the ground floor plan).
    #[test]
    fn the_floor_plans_leave_the_outdoors_to_the_site_plan() {
        use crate::visibility::{Category, WhyHidden};
        let m = ridgeline_house();
        let plan = m
            .iter()
            .find_map(|e| match &e.kind {
                ElementKind::View(v)
                    if matches!(v.kind, crate::ViewKind::Plan { .. })
                        && e.name.starts_with("Ground") =>
                {
                    Some(v)
                }
                _ => None,
            })
            .expect("a ground floor plan");
        let outdoors: Vec<&Element> = m
            .iter()
            // the placed cars, not the car's type, which is named the same
            .filter(|e| e.can_be_site())
            .filter(|e| {
                e.name == "Car"
                    || e.name.starts_with("Court steps")
                    || e.name == "Court retaining wall"
            })
            .collect();
        let names: Vec<&str> = outdoors.iter().map(|e| e.name.as_str()).collect();
        for n in [
            "Car",
            "Court steps",
            "Court steps rail",
            "Court retaining wall",
        ] {
            assert!(names.contains(&n), "{n} among {names:?}");
        }
        for e in outdoors {
            assert!(e.is_site_part(), "{} is part of the site", e.name);
            assert_eq!(
                plan.hidden.why(&m, e.id),
                Some(WhyHidden::Category(Category::Site)),
                "{} left off the ground floor plan",
                e.name
            );
        }
    }

    /// Every stage adds to the one before, and stopping after a stage
    /// leaves the later ones out.
    #[test]
    fn the_stages_build_in_order() {
        let mut sizes = vec![];
        let whole = build("", |b, stage| {
            sizes.push((stage.to_string(), b.doc.model().len()))
        });
        let names: Vec<&str> = sizes.iter().map(|s| s.0.as_str()).collect();
        assert_eq!(names, STAGES.map(|s| s.0));
        assert!(sizes.windows(2).all(|w| w[0].1 < w[1].1), "{sizes:?}");
        let site = build("site", |_, _| {});
        assert_eq!(site.doc.model().len(), sizes[0].1);
        assert!(site.doc.model().len() < whole.doc.model().len());
    }

    /// The hero views sheet takes its three pictures and is the
    /// sixteenth sheet.
    #[test]
    fn the_hero_views_sheet_is_the_sixteenth() {
        let m = ridgeline_house();
        let picture = || HeroPicture {
            width: 2,
            height: 1,
            rgba: String::new(),
        };
        let tx = hero_views_sheet(&m, [picture(), picture(), picture()]).unwrap();
        let mut doc = Document::from_model(m);
        doc.commit(tx).unwrap();
        let m = doc.model();
        assert_eq!(count(m, |k| matches!(k, ElementKind::Sheet(_))), 16);
        let images = m
            .iter()
            .filter_map(|e| match &e.kind {
                ElementKind::SheetTemplate(t) if e.name == "Ridgeline hero views" => Some(t),
                _ => None,
            })
            .flat_map(|t| &t.items)
            .filter(|i| matches!(i, crate::titleblock::TitleItem::Image { .. }))
            .count();
        assert_eq!(images, 3);
    }
}
