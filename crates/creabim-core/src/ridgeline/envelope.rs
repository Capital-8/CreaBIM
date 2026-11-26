//! The envelope and the way up: doors and windows in the outside walls,
//! the dining room's glazing, the porch's slatted screen, two roof windows
//! over the landing, then the stair, its well and its guards.

use super::structure::{TERRACE, WING_SOFFIT};
use super::*;
use crate::cells::WindowStyle;
use crate::members::Railing;
use crate::*;
use glam::{DVec2, DVec3};

pub fn build(b: &mut Builder) {
    court_steps(b);
    openings(b);
    glazing_and_screens(b);
    roof_windows(b);
    stair(b);
    site_plan_outdoors_only(b);
}

/// The site plan shows what stands outdoors: the steps up from the court
/// and their rail, the cars parked on the court. The stair and guards
/// indoors, the furniture and fittings, and the figures are hidden in that
/// view one by one. Called again after each stage that adds such things.
pub fn site_plan_outdoors_only(b: &mut Builder) {
    // what stands outdoors is part of the site: it goes with the site
    // plan and leaves the floor plans, which show no site (MU136: the cars,
    // the court steps and the retaining wall were on the ground floor plan)
    let outdoors: Vec<Element> = b
        .doc
        .model()
        .iter()
        .filter(|e| {
            !e.on_site
                && match e.kind {
                    ElementKind::Stair(_) | ElementKind::Railing(_) => {
                        e.name.starts_with("Court steps")
                    }
                    ElementKind::Object(_) => e.name == "Car",
                    ElementKind::Wall(_) => e.name == "Court retaining wall",
                    _ => false,
                }
        })
        .cloned()
        .collect();
    if !outdoors.is_empty() {
        let mut tx = Transaction::new("Ridgeline House: outdoors is part of the site");
        for e in outdoors {
            let mut on = e.clone();
            on.on_site = true;
            tx = tx.replace(e, on);
        }
        b.commit(tx);
    }
    let before = b.doc.model().get(b.id("Site plan")).unwrap().clone();
    let mut after = before.clone();
    let ElementKind::View(v) = &mut after.kind else {
        return;
    };
    let indoors: Vec<ElementId> = b
        .doc
        .model()
        .iter()
        .filter(|e| match e.kind {
            ElementKind::Stair(_) | ElementKind::Railing(_) => !e.name.starts_with("Court steps"),
            ElementKind::Object(_) => e.name != "Car",
            _ => false,
        })
        .map(|e| e.id)
        .filter(|id| !v.hidden.elements.contains(id))
        .collect();
    if indoors.is_empty() {
        return;
    }
    v.hidden.elements.extend(indoors);
    b.commit(
        Transaction::new("Ridgeline House: the site plan, outdoors only").replace(before, after),
    );
}

/// A door, window or hole in `host`, `offset` from its start; its type
/// gives it its size.
pub fn opening(host: ElementId, kind: OpeningKind, offset: f64) -> Opening {
    Opening {
        host,
        kind,
        offset,
        sill: 0.0,
        width: 1.0,
        height: 2.1,
        material: None,
        frame: OpeningFrame::for_kind(kind),
        flip_hand: false,
        flip_facing: false,
        roof_at: None,
        roof_build: Default::default(),
        arrangement: None,
    }
}

/// Places `list` in walls: (name, host wall, kind, type, offset from the
/// wall's start in feet, hinge on the other jamb, face the other way).
pub fn place(
    b: &mut Builder,
    label: &str,
    list: &[(&str, &str, OpeningKind, &str, f64, bool, bool)],
) {
    let mut tx = Transaction::new(label);
    for &(name, host, kind, t, offset, hand, facing) in list {
        let mut o = opening(b.id(host), kind, ft(offset));
        o.flip_hand = hand;
        o.flip_facing = facing;
        let e = b.typed(Element::new(name, ElementKind::Opening(o)), t);
        tx = b.keep(name, e, tx);
    }
    b.commit(tx);
}

fn openings(b: &mut Builder) {
    use OpeningKind::{Door, Window};
    let slider = "Terrace slider 10'-0\" × 8'-0\"";
    let three = "Window 8'-0\" × 5'-0\", three lights";
    let pair = "Window 4'-0\" × 5'-0\", casement pair";
    let square = "Window 4'-0\" × 4'-0\", casement";
    let awning = "Window 2'-0\" × 3'-0\", awning";
    let tall = "Window 3'-0\" × 6'-0\", tilt-turn";
    place(
        b,
        "Ridgeline House: doors and windows",
        &[
            // ground floor: the living room opens south onto the terrace
            (
                "Living slider west",
                "GF South",
                Door,
                slider,
                1.0,
                false,
                false,
            ),
            (
                "Living slider east",
                "GF South",
                Door,
                slider,
                13.0,
                false,
                false,
            ),
            (
                "Living west window",
                "GF West",
                Window,
                "Window 6'-0\" × 6'-0\", fixed",
                14.0,
                false,
                false,
            ),
            (
                "Study window",
                "GF West",
                Window,
                square,
                1.75,
                false,
                false,
            ),
            (
                "Kitchen window",
                "GF North",
                Window,
                "Window 6'-0\" × 3'-4\", sliding",
                4.0,
                false,
                false,
            ),
            // the wing
            (
                "Entry door",
                "GF Wing South",
                Door,
                "Entry door 3'-6\" × 8'-0\"",
                4.0,
                false,
                false,
            ),
            (
                "Entry sidelight",
                "GF Wing South",
                Window,
                "Sidelight 1'-6\" × 8'-0\"",
                7.9,
                false,
                false,
            ),
            (
                "Mudroom window",
                "GF Wing East",
                Window,
                tall,
                3.0,
                false,
                false,
            ),
            (
                "Mechanical window",
                "GF Wing East",
                Window,
                awning,
                13.5,
                false,
                false,
            ),
            (
                "Powder room window",
                "GF Wing North",
                Window,
                awning,
                8.25,
                false,
                false,
            ),
            (
                "Service door",
                "GF Wing North",
                Door,
                "Service door 3'-0\" × 6'-8\"",
                1.75,
                true,
                false,
            ),
            // first floor: a wide window to each bedroom, south
            (
                "Bedroom 2 window",
                "FF South",
                Window,
                three,
                2.0,
                false,
                false,
            ),
            (
                "Bedroom 3 window",
                "FF South",
                Window,
                three,
                14.0,
                false,
                false,
            ),
            (
                "Primary bedroom window",
                "FF South",
                Window,
                three,
                26.0,
                false,
                false,
            ),
            (
                "Bedroom 2 west window",
                "FF West",
                Window,
                pair,
                14.0,
                false,
                false,
            ),
            (
                "Landing nook window",
                "FF West",
                Window,
                square,
                1.75,
                false,
                false,
            ),
            (
                "Stair window",
                "FF North",
                Window,
                square,
                19.0,
                false,
                false,
            ),
            (
                "Bathroom window",
                "FF North",
                Window,
                awning,
                3.0,
                false,
                false,
            ),
            (
                "En-suite window",
                "FF East",
                Window,
                awning,
                12.5,
                false,
                false,
            ),
            (
                "Primary bedroom east window",
                "FF East",
                Window,
                pair,
                6.5,
                false,
                false,
            ),
        ],
    );
}

/// How far short of the wing's corner the porch screen stops.
const SCREEN_GAP: f64 = 0.3;

fn glazing_and_screens(b: &mut Builder) {
    let g = b.id("Ground Floor");
    let mut tx = Transaction::new("Ridgeline House: glazing and screens");
    // the dining room's glazing sits in the south wall and cuts its own
    // opening there
    let mut cw = Wall::new(g, p(26.0, 0.0), p(34.0, 0.0), 0.15, ft(8.0));
    cw.material = None;
    let mut e = b.typed(
        Element::new("Dining glazing", ElementKind::Wall(cw)),
        "Dining glazing, 3 bays",
    );
    if let ElementKind::Wall(w) = &mut e.kind {
        w.height = ft(8.0);
    }
    tx = b.keep("Dining glazing", e, tx);
    // the porch's east side: cedar fins against the wind and the street
    let mut screen = Wall::new(
        g,
        p(48.0, 0.25),
        DVec2::new(ft(48.0), ft(6.0) - H - SCREEN_GAP),
        0.089,
        WING_SOFFIT - TERRACE,
    );
    screen.base_offset = TERRACE;
    screen.disallow_join = [true, true];
    let mut e = b.typed(
        Element::new("Porch screen", ElementKind::Wall(screen)),
        "Timber slat screen",
    );
    if let ElementKind::Wall(w) = &mut e.kind {
        w.height = WING_SOFFIT - TERRACE;
        w.base_offset = TERRACE;
    }
    tx = b.keep("Porch screen", e, tx);
    // a cedar jamb closes the screen against the wall. The screen itself
    // stops short of the corner: a wall ending on a corner, even one told
    // not to join, breaks the mitre of the two walls that meet there.
    let mut jamb = Column::new(
        g,
        DVec2::new(ft(48.0), ft(6.0) - H - SCREEN_GAP / 2.0),
        Profile::Rectangle {
            width: 0.089,
            depth: SCREEN_GAP,
        },
    );
    jamb.base_offset = TERRACE;
    jamb.height = Some(WING_SOFFIT - TERRACE);
    jamb.material = Some(b.id("Cedar cladding, vertical"));
    tx = b.keep(
        "Porch screen jamb",
        Element::new("Porch screen jamb", ElementKind::Column(jamb)),
        tx,
    );
    b.commit(tx);
}

/// Two roof windows in the north slope, over the landing and the stair.
fn roof_windows(b: &mut Builder) {
    let roof = b.id("Main roof");
    let bronze = Some(b.id("Aluminium, dark bronze"));
    let mut tx = Transaction::new("Ridgeline House: roof windows");
    for (name, at) in [
        ("Roof window, landing", p(9.0, 19.5)),
        ("Roof window, stair", p(17.0, 19.5)),
    ] {
        let mut frame = OpeningFrame::for_kind(OpeningKind::Window);
        frame.grid = WindowStyle::HorizontalPivot.grid(false, [1, 1]);
        let e = Element::new(
            name,
            ElementKind::Opening(Opening {
                host: roof,
                kind: OpeningKind::Window,
                offset: 0.0,
                sill: 0.0,
                width: 0.78,
                height: 1.4,
                material: bronze,
                frame,
                flip_hand: false,
                flip_facing: false,
                roof_at: Some(at),
                roof_build: Default::default(),
                arrangement: None,
            }),
        );
        tx = b.keep(name, e, tx);
    }
    b.commit(tx);
}

/// Four risers of 5 7/8 in up from the parking court to the house's pad,
/// in the gap of the retaining wall, a handrail on their open side.
fn court_steps(b: &mut Builder) {
    let g = b.id("Ground Floor");
    let rise = GRADE - super::site::COURT;
    let tread = inch(11.0);
    let risers = 4.0;
    let run = (risers - 1.0) * tread;
    let mut st = Stair::new(g, DVec2::new(ft(54.5) + run, ft(-14.0)), 180.0);
    st.width = ft(4.0);
    st.tread = tread;
    st.max_riser = rise / risers + 0.001;
    st.base_offset = super::site::COURT;
    st.height = Some(rise);
    st.material = Some(b.id("Concrete, board-formed"));
    st.rules = crate::stairrules::StairCode::Us;
    let mut tx = Transaction::new("Ridgeline House: court steps");
    tx = b.keep(
        "Court steps",
        Element::new("Court steps", ElementKind::Stair(st)),
        tx,
    );
    b.commit(tx);
    // four risers: a rail on the open (south) side
    let model = b.doc.model().clone();
    if let Some(mut rail) = Railing::along_stair(&model, b.id("Court steps"), false) {
        rail.material = Some(b.id("Aluminium, dark bronze"));
        rail.height = inch(36.0);
        let tx = Transaction::new("Ridgeline House: court steps rail")
            .insert(Element::new("Court steps rail", ElementKind::Railing(rail)));
        b.commit(tx);
    }
}

/// The stair's width (3'-3"), and where it runs: along the north wall,
/// rising west from x = 21'-6".
pub const STAIR_WIDTH: f64 = 0.99;
pub const STAIR_FOOT: f64 = 21.5;
/// The north wall's inside face.
pub fn north_face() -> f64 {
    ft(24.0) - H
}
/// The stair's open (south) edge.
pub fn stair_edge() -> f64 {
    north_face() - 0.03 - STAIR_WIDTH
}
/// Risers of 7 1/2 in and treads of 10 1/2 in: 16 up, 15 treads.
pub const TREAD: f64 = 10.5 * 0.0254;
/// Where the stair arrives upstairs, metres east of the origin.
pub fn stair_head() -> f64 {
    ft(STAIR_FOOT) - 15.0 * TREAD
}
/// The well's east end: the floor runs on over the first two treads,
/// which keeps 6'-8" of headroom over every tread.
pub fn well_end() -> f64 {
    ft(STAIR_FOOT) - 2.0 * TREAD
}

fn stair(b: &mut Builder) {
    let (g, f) = (b.id("Ground Floor"), b.id("First Floor"));
    let oak = b.material("Timber, oak");
    let mut tx = Transaction::new("Ridgeline House: stair");
    let yc = north_face() - 0.03 - STAIR_WIDTH / 2.0;
    let mut st = Stair::new(g, DVec2::new(ft(STAIR_FOOT), yc), 180.0);
    st.width = STAIR_WIDTH;
    st.tread = TREAD;
    st.max_riser = inch(7.75);
    st.material = Some(oak);
    st.rules = crate::stairrules::StairCode::Us;
    tx = b.keep("Stair", Element::new("Stair", ElementKind::Stair(st)), tx);
    let (x0, x1, y0, y1) = (stair_head(), well_end(), stair_edge() - 0.03, north_face());
    let well = vec![
        DVec2::new(x0, y0),
        DVec2::new(x1, y0),
        DVec2::new(x1, y1),
        DVec2::new(x0, y1),
    ];
    let shaft = Element::new(
        "Stair well",
        ElementKind::Shaft(Shaft::new(g, well, Some(f))),
    );
    tx = b.keep("Stair well", shaft, tx);
    b.commit(tx);
    // where the well meets the north wall there is no floor to carry: the
    // first floor stops at the well's edge, open to the outside of the
    // studs, and the wall runs on, one height of studs, past the well
    // (owner, MU130). Elsewhere the floor and its rim board stand on the
    // wall under it
    let top = ft(24.0) + super::structure::S;
    let floor = b.id("First floor");
    b.doc
        .edit(
            floor,
            "Ridgeline House: the first floor stops at the well",
            |e| {
                if let ElementKind::Slab(sl) = &mut e.kind {
                    let at = sl
                        .boundary
                        .iter()
                        .position(|q| (q.y - top).abs() < 1e-9 && q.x > x1)
                        .expect("the floor's north-east corner");
                    let notch = [
                        DVec2::new(x1, top),
                        DVec2::new(x1, y0),
                        DVec2::new(x0, y0),
                        DVec2::new(x0, top),
                    ];
                    for (i, q) in notch.into_iter().enumerate() {
                        sl.boundary.insert(at + 1 + i, q);
                    }
                }
            },
        )
        .expect("the first floor takes the notch");

    // a round rail the hand closes round (IRC: 1 1/4 to 2 in across)
    let grip = "Handrail, round 1 1/2 in";
    let round = crate::profiles::NamedProfile {
        name: grip.into(),
        points: (0..24)
            .map(|i| {
                let a = i as f64 / 24.0 * std::f64::consts::TAU;
                DVec2::new(a.cos(), a.sin()) * inch(0.75)
            })
            .collect(),
        ..Default::default()
    };
    let tx = crate::profiles::set_profiles(
        b.doc.model(),
        vec![round],
        "Ridgeline House: handrail profile",
    );
    b.commit(tx);
    let fir = Some(b.id("Fir, clear finish"));
    let model = b.doc.model().clone();
    let mut tx = Transaction::new("Ridgeline House: guards");
    if let Some(mut rail) = Railing::along_stair(&model, b.id("Stair"), true) {
        rail.material = fir;
        // IRC: 34 to 38 in above the nosings, nothing a 4 in sphere passes
        rail.height = inch(36.0);
        rail.rail_profile = Some(grip.into());
        rail.infill = crate::members::RailInfill::Balusters;
        tx = tx.insert(Element::new("Stair handrail", ElementKind::Railing(rail)));
    }
    // upstairs: a guard along the well, up to the laundry's wall
    let at = |x: f64, y: f64| DVec3::new(x, y, 0.0);
    let mut guard = Railing::new(f, vec![at(x0, y0 - 0.03), at(x1, y0 - 0.03)]);
    guard.height = inch(36.0);
    guard.infill = crate::members::RailInfill::Balusters;
    guard.material = fir;
    tx = tx.insert(Element::new("Landing guard", ElementKind::Railing(guard)));
    b.commit(tx);
}
