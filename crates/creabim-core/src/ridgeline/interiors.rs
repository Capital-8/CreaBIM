//! Inside: the partitions and their doors, the rooms, the ceilings, the
//! furniture and the fixtures.
//!
//! Every wet room sits against one heated service chase in the north-east
//! corner: the kitchen and the powder room below, both bathrooms and the
//! laundry above. No pipe runs in an outside wall or an unheated floor.

use super::envelope::{north_face, place, stair_edge, well_end};
use super::site::COURT;
use super::structure::{TERRACE, WING_SOFFIT};
use super::*;
use crate::objects::Mount;
use crate::types::{OpeningType, TypeParams};
use crate::*;
use glam::DVec2;

/// The ground floor's ceiling: the underside of the first floor.
pub const GF_CEILING: f64 = STOREY - FLOOR;

pub fn build(b: &mut Builder) {
    partitions(b);
    doors(b);
    rooms(b);
    ceilings(b);
    furnish(b);
    // the furniture stays off the site plan, the parked cars stay on it
    super::envelope::site_plan_outdoors_only(b);
}

fn partitions(b: &mut Builder) {
    let (g, f) = (b.id("Ground Floor"), b.id("First Floor"));
    let (main_roof, wing_roof) = (b.id("Main roof"), b.id("Wing roof"));
    let thin = "Partition 2×4";
    let wet = "Partition 2×6, plumbing";
    // the laundry's west wall stands on the well's end
    let lx = well_end() / FT + 0.25;
    // (key, level, from, to, type, ends that do not join)
    let list: Vec<(&str, ElementId, DVec2, DVec2, &str)> = vec![
        // ground floor: only the study and the chase are closed off
        ("GF Study South", g, p(0.0, 16.5), p(8.5, 16.5), thin),
        ("GF Study East", g, p(8.5, 16.5), p(8.5, 24.0), thin),
        ("GF Chase West", g, p(34.0, 15.5), p(34.0, 17.5), thin),
        ("GF Chase South", g, p(34.0, 15.5), p(36.0, 15.5), thin),
        ("GF Chase North", g, p(34.0, 17.5), p(36.0, 17.5), thin),
        // the wing
        ("GF Mudroom North", g, p(36.0, 16.0), p(48.0, 16.0), thin),
        ("GF Powder East", g, p(41.5, 16.0), p(41.5, 24.0), wet),
        // first floor: bedrooms south of the corridor, wet rooms north-east
        // the corridor runs on east past bedroom 3 to the primary bedroom's
        // door (owner, MU141)
        ("FF Corridor South", f, p(0.0, 16.5), p(27.27, 16.5), thin),
        ("FF Bedroom 2 East", f, p(12.0, 0.0), p(12.0, 16.5), thin),
        ("FF Bedroom 3 East", f, p(24.0, 0.0), p(24.0, 16.354), thin),
        ("FF Primary North", f, p(27.5, 11.0), p(36.0, 11.0), thin),
        ("FF Bath West", f, p(27.5, 11.0), p(27.5, 24.0), wet),
        ("FF Wet wall", f, p(27.5, 17.5), p(36.0, 17.5), wet),
        ("FF Laundry West", f, p(lx, 20.0), p(lx, 24.0), thin),
        ("FF Laundry South", f, p(lx, 20.0), p(24.0, 20.0), thin),
        ("FF Laundry East", f, p(24.0, 20.0), p(24.0, 24.0), thin),
        ("FF Chase West", f, p(34.0, 15.5), p(34.0, 17.5), thin),
        ("FF Chase South", f, p(34.0, 15.5), p(36.0, 15.5), thin),
    ];
    let mut tx = Transaction::new("Ridgeline House: partitions");
    for (key, level, a, bb, kind) in list {
        let wing = key.contains("Mudroom") || key.contains("Powder");
        let height = if level == f {
            PLATE
        } else if wing {
            WING_SOFFIT
        } else {
            GF_CEILING
        };
        let mut w = Wall::new(level, a, bb, PART, height);
        if level == f {
            // up to the roof in the vaulted bedrooms; the flat ceilings
            // north of the corridor are attached to later
            w.attach_top = vec![main_roof];
        } else if wing {
            w.attach_top = vec![wing_roof];
        }
        let mut e = b.typed(Element::new(key, ElementKind::Wall(w)), kind);
        if let ElementKind::Wall(w) = &mut e.kind {
            w.height = height;
        }
        tx = b.keep(key, e, tx);
    }
    // the stair's screen: cedar fins from floor to ceiling along its open side
    let y = stair_edge() - 0.03 - 0.045;
    let mut screen = Wall::new(
        g,
        DVec2::new(ft(8.5) + HP, y),
        DVec2::new(ft(super::envelope::STAIR_FOOT), y),
        0.089,
        GF_CEILING,
    );
    screen.disallow_join = [true, true];
    let mut e = b.typed(
        Element::new("Stair screen", ElementKind::Wall(screen)),
        "Timber slat screen",
    );
    if let ElementKind::Wall(w) = &mut e.kind {
        w.height = GF_CEILING;
    }
    tx = b.keep("Stair screen", e, tx);
    b.commit(tx);
}

fn doors(b: &mut Builder) {
    use OpeningKind::Door;
    let bronze = Some(b.id("Aluminium, dark bronze"));
    let mut frame = OpeningFrame::for_kind(Door).with_panes(1, 1);
    frame.depth = 0.11;
    b.add_type(
        "Vestibule door 3'-0\" × 7'-0\", glazed",
        "The mudroom is an air lock: a second, glazed door into the house",
        TypeParams::Door(OpeningType {
            width: inch(36.0),
            height: ft(7.0),
            sill: 0.0,
            frame,
            material: bronze,
        }),
    );
    let swing = "Interior door 2'-8\" × 6'-8\"";
    let pocket = "Pocket door 2'-6\" × 6'-8\"";
    place(
        b,
        "Ridgeline House: inside doors",
        &[
            (
                "Vestibule door",
                "GF East",
                Door,
                "Vestibule door 3'-0\" × 7'-0\", glazed",
                8.77,
                false,
                true,
            ),
            (
                "Study door",
                "GF Study South",
                Door,
                swing,
                5.635,
                true,
                false,
            ),
            (
                "Powder room door",
                "GF Mudroom North",
                Door,
                swing,
                2.551,
                true,
                false,
            ),
            (
                "Mechanical room door",
                "GF Mudroom North",
                Door,
                swing,
                7.6,
                true,
                false,
            ),
            (
                "Bedroom 2 door",
                "FF Corridor South",
                Door,
                swing,
                8.906,
                true,
                true,
            ),
            (
                "Bedroom 3 door",
                "FF Corridor South",
                Door,
                swing,
                12.428,
                false,
                true,
            ),
            // the primary bedroom is entered from the corridor's end (MU141)
            (
                "Primary bedroom door",
                "FF Corridor South",
                Door,
                swing,
                24.322,
                true,
                true,
            ),
            (
                "En-suite door",
                "FF Primary North",
                Door,
                pocket,
                5.578,
                false,
                false,
            ),
            (
                "Bathroom door",
                "FF Bath West",
                Door,
                pocket,
                9.6,
                false,
                false,
            ),
            (
                "Laundry door",
                "FF Laundry South",
                Door,
                swing,
                0.55,
                false,
                true,
            ),
        ],
    );
}

fn rooms(b: &mut Builder) {
    let (g, f) = (b.id("Ground Floor"), b.id("First Floor"));
    // the open plan is one space: lines mark where its rooms meet
    let mut tx = Transaction::new("Ridgeline House: room separators");
    let sep = |level, a: DVec2, bb: DVec2| {
        Element::new(
            "Room separator",
            ElementKind::RoomSeparator(crate::room::Separator { level, a, b: bb }),
        )
    };
    tx = tx.insert(sep(g, p(24.0, 0.0), p(24.0, 24.0)));
    tx = tx.insert(sep(g, p(24.0, 13.0), p(36.0, 13.0)));
    // the stair's foot, where the screen ends
    tx = tx.insert(sep(
        g,
        DVec2::new(ft(super::envelope::STAIR_FOOT), stair_edge() - 0.1),
        DVec2::new(ft(super::envelope::STAIR_FOOT), ft(24.0)),
    ));
    b.commit(tx);

    let porch = crate::polygon::rectangle(
        DVec2::new(ft(36.0) + H, ft(-0.5)),
        DVec2::new(ft(48.5), ft(6.0) - H),
    );
    let terrace = crate::polygon::rectangle(p(-1.0, -12.0), DVec2::new(ft(36.0) + H, -H));
    // (level, number, name, its point, a drawn outline, occupancy, floor, walls)
    type Entry<'a> = (
        ElementId,
        &'a str,
        &'a str,
        DVec2,
        Vec<DVec2>,
        &'a str,
        &'a str,
    );
    #[rustfmt::skip]
    let list: Vec<Entry> = vec![
        (g, "101", "Living", p(10.0, 8.0), vec![], "Living", "Oak, oiled"),
        (g, "102", "Dining", p(30.0, 6.0), vec![], "Living", "Oak, oiled"),
        (g, "103", "Kitchen", p(30.0, 18.0), vec![], "Kitchen", "Oak, oiled"),
        (g, "104", "Study", p(4.0, 20.0), vec![], "Study", "Oak, oiled"),
        (g, "105", "Stair", p(15.0, 22.0), vec![], "Circulation", "Oak treads"),
        (g, "106", "Mudroom", p(42.0, 11.0), vec![], "Circulation", "Slate tile"),
        (g, "107", "Powder room", p(38.5, 20.0), vec![], "Sanitary", "Porcelain tile"),
        (g, "108", "Mechanical", p(45.0, 20.0), vec![], "Utility", "Sealed concrete"),
        (g, "109", "Porch", p(42.0, 3.0), porch, "Outdoor", "Broom-finished concrete"),
        (g, "110", "Terrace", p(12.0, -6.0), terrace, "Outdoor", "Ipe decking"),
        (f, "201", "Bedroom 2", p(6.0, 8.0), vec![], "Bedroom", "Oak, oiled"),
        (f, "202", "Bedroom 3", p(18.0, 8.0), vec![], "Bedroom", "Oak, oiled"),
        (f, "203", "Primary bedroom", p(30.0, 5.5), vec![], "Bedroom", "Oak, oiled"),
        (f, "204", "En-suite", p(31.2, 12.8), vec![], "Sanitary", "Porcelain tile"),
        (f, "205", "Bathroom", p(30.2, 21.8), vec![], "Sanitary", "Porcelain tile"),
        (f, "206", "Laundry", p(22.0, 22.0), vec![], "Utility", "Porcelain tile"),
        (f, "207", "Landing", p(4.0, 20.0), vec![], "Circulation", "Oak, oiled"),
    ];
    let mut tx = Transaction::new("Ridgeline House: rooms");
    for (level, number, name, at, outline, occupancy, floor) in list {
        let outdoor = occupancy == "Outdoor";
        let mut r = Room::new(level, at);
        r.number = number.into();
        r.boundary = outline;
        r.occupancy = occupancy.into();
        r.floor_finish = floor.into();
        if outdoor {
            r.department = "Outdoor".into();
            r.height = Some(2.6);
            r.ceiling_finish = "Cedar soffit".into();
        } else {
            r.department = if level == g { "Day" } else { "Night" }.into();
            r.wall_finish = if occupancy == "Sanitary" {
                "Tile to 48 in, paint over".into()
            } else {
                "Gypsum board, painted".into()
            };
            r.ceiling_finish = match name {
                "Bedroom 2" | "Bedroom 3" | "Primary bedroom" => "Vaulted, gypsum board".into(),
                "Landing" => "Vaulted, gypsum board; flat over the passage".into(),
                _ => "Gypsum board, painted".into(),
            };
        }
        let key = format!("Room {number}");
        tx = b.keep(&key, Element::new(name, ElementKind::Room(r)), tx);
    }
    b.commit(tx);
}

fn ceilings(b: &mut Builder) {
    let (g, f) = (b.id("Ground Floor"), b.id("First Floor"));
    let board = b.material("Plasterboard");
    let cedar = b.id("Cedar cladding, vertical");
    let mut tx = Transaction::new("Ridgeline House: ceilings");
    let mut flat = vec![];
    // upstairs the bedrooms and the landing are vaulted to the roof: the
    // roof is the insulated, warm layer (R-60 in the rafters, board under
    // them), so a flat ceiling only closes off a pocket inside the house.
    // Over the landing it shut out the two roof windows (owner, MU130):
    // vaulted, they light the landing and, down the stair well, the hall
    // below. The wet rooms keep flat ceilings at the plate, their ducts
    // and fans run over them
    for number in ["204", "205", "206"] {
        let mut c = Ceiling::in_room(f, b.id(&format!("Room {number}")), PLATE - 0.013);
        c.thickness = 0.013;
        c.material = Some(board);
        let e = Element::new("Gypsum board ceiling", ElementKind::Ceiling(c));
        flat.push(e.id);
        tx = tx.insert(e);
    }
    // the passage east of the landing, to the primary bedroom and the
    // bathroom, is out from under the roof windows: a flat ceiling at the
    // plate carries its smoke alarm, its downlights and the supply duct
    // to its diffuser
    {
        let mut c = Ceiling::drawn(
            f,
            crate::polygon::rectangle(
                DVec2::new(ft(24.0) + HP, ft(11.0) + HP),
                DVec2::new(ft(27.5) - HP, north_face()),
            ),
            PLATE - 0.013,
        );
        c.thickness = 0.013;
        c.material = Some(board);
        tx = tx.insert(Element::new(
            "Gypsum board ceiling, passage",
            ElementKind::Ceiling(c),
        ));
    }
    // the ground floor's ceilings are the underside of the floor above
    for number in ["101", "102", "103", "104"] {
        let mut c = Ceiling::in_room(g, b.id(&format!("Room {number}")), GF_CEILING - 0.013);
        c.thickness = 0.013;
        c.material = Some(board);
        tx = tx.insert(Element::new(
            "Gypsum board ceiling",
            ElementKind::Ceiling(c),
        ));
    }
    // the wing: board at 9 ft, a tile ceiling the services run over in
    // the powder room
    for (number, height, grid) in [
        ("106", ft(9.0), 0.0),
        ("108", ft(9.0), 0.0),
        ("107", ft(8.0), 0.6),
    ] {
        let mut c = Ceiling::in_room(g, b.id(&format!("Room {number}")), height);
        c.thickness = 0.016;
        c.grid = grid;
        c.material = Some(board);
        let name = if grid > 0.0 {
            "Suspended tile ceiling"
        } else {
            "Gypsum board ceiling"
        };
        tx = tx.insert(Element::new(name, ElementKind::Ceiling(c)));
    }
    // a dropped soffit over the kitchen's north counter, for the hood's duct
    {
        let mut c = Ceiling::in_room(g, b.id("Room 103"), ft(8.0));
        c.room = None;
        c.boundary = crate::polygon::rectangle(
            DVec2::new(ft(24.5), north_face() - ft(2.2)),
            DVec2::new(ft(34.0) - HP, north_face()),
        );
        c.thickness = 0.016;
        c.material = Some(board);
        tx = tx.insert(Element::new("Kitchen soffit", ElementKind::Ceiling(c)));
    }
    // cedar soffits outside, under the canopy and the porch roof
    for (number, height) in [
        ("109", WING_SOFFIT - 0.02),
        ("110", structure::CANOPY - 0.02),
    ] {
        let room = b.id(&format!("Room {number}"));
        let mut c = Ceiling::in_room(g, room, height);
        if number == "110" {
            c.room = None;
            c.boundary = crate::polygon::rectangle(p(-1.0, -7.0), DVec2::new(ft(25.0), -H));
        }
        c.thickness = 0.019;
        c.material = Some(cedar);
        tx = tx.insert(Element::new("Cedar soffit", ElementKind::Ceiling(c)));
    }
    b.commit(tx);

    // the partitions between the wet rooms stop at their flat ceilings;
    // those that face the vaulted landing (the bathroom's west wall, the
    // laundry's three) run on up to the roof and close the space over
    // the ceilings off from it
    for key in ["FF Wet wall", "FF Chase West", "FF Chase South"] {
        let id = b.id(key);
        let flat = flat.clone();
        b.doc
            .edit(id, "Ridgeline House: partitions to the ceiling", |e| {
                if let ElementKind::Wall(w) = &mut e.kind {
                    w.attach_top = flat.clone();
                }
            })
            .expect("the partitions take the ceilings");
    }
    // where the passage's ceiling meets the vault, a bulkhead closes the
    // space over it: from the plate up to the roof, between the corridor's
    // wall and the laundry's. It stands on the roof plate's level, so it
    // does not divide the landing below it into two rooms
    let top = b.id("Roof Plate");
    let mut w = Wall::new(top, p(24.0, 16.5), p(24.0, 20.0), PART, 1.0);
    w.base_offset = -0.013;
    w.attach_top = vec![b.id("Main roof")];
    let e = b.typed(
        Element::new("FF Passage bulkhead", ElementKind::Wall(w)),
        "Partition 2×4",
    );
    let tx = b.keep(
        "FF Passage bulkhead",
        e,
        Transaction::new("Ridgeline House: passage bulkhead"),
    );
    b.commit(tx);
    let _ = TERRACE;
}

/// Furniture and fixtures from the starter catalogue: (object, level, a
/// point near where it goes in feet, its turn when it is not placed
/// against the nearest wall). A turn of 0 faces south, 90 east.
fn furnish(b: &mut Builder) {
    let (g, f) = (b.id("Ground Floor"), b.id("First Floor"));
    type Item = (&'static str, ElementId, (f64, f64), Option<f64>);
    #[rustfmt::skip]
    let plan: Vec<Item> = vec![
        // living: seating in the west bay, facing the terrace
        ("sofa-3", g, (6.6, 9.52), Some(0.0)),
        ("coffee-table", g, (6.6, 5.52), Some(0.0)),
        ("armchair", g, (3.0, 4.92), Some(90.0)),
        ("armchair", g, (10.9, 4.92), Some(270.0)),
        // two chairs by the east slider, out of the way to the stair, each
        // facing the table between them (MU141: they faced away from it)
        ("armchair", g, (15.68, 3.65), Some(135.0)),
        ("armchair", g, (21.07, 4.0), Some(225.0)),
        ("coffee-table", g, (18.1, 6.8), Some(0.0)),
        // dining, in the east bay behind the tall glazing, clear of the way
        // past the island (MU141)
        ("dining-table", g, (29.17, 5.48), Some(0.0)),
        ("chair", g, (27.47, 3.88), Some(180.0)),
        ("chair", g, (29.17, 3.88), Some(180.0)),
        ("chair", g, (30.87, 3.88), Some(180.0)),
        ("chair", g, (27.47, 7.08), Some(0.0)),
        ("chair", g, (29.17, 7.08), Some(0.0)),
        ("chair", g, (30.87, 7.08), Some(0.0)),
        ("chair", g, (25.47, 5.48), Some(90.0)),
        ("chair", g, (32.87, 5.48), Some(270.0)),
        // kitchen: the run along the north wall, the sink under its
        // window, the range at the east end under the soffit's hood, a
        // short run on the east wall either side of the chase, and an
        // island with stools, 42 in clear all round
        ("base-cabinet-600", g, (26.37, 22.55), Some(0.0)),
        ("sink-unit-1000", g, (29.0, 22.55), Some(0.0)),
        ("base-cabinet-600", g, (31.63, 22.55), Some(0.0)),
        ("cooker-600", g, (33.6, 22.55), Some(0.0)),
        ("wall-cabinet-600", g, (34.6, 23.0), Some(0.0)),
        ("base-cabinet-600", g, (34.56, 20.7), Some(270.0)),
        ("base-cabinet-600", g, (34.56, 18.73), Some(270.0)),
        // the fridge ends the east run (MU141)
        ("fridge-freezer", g, (34.37, 14.17), Some(270.0)),
        ("base-cabinet-600", g, (25.63, 17.0), Some(180.0)),
        ("base-cabinet-600", g, (27.6, 17.0), Some(180.0)),
        ("base-cabinet-600", g, (29.57, 17.0), Some(180.0)),
        // stools at the island, one to each cabinet (MU141)
        ("chair", g, (25.63, 15.0), Some(180.0)),
        ("chair", g, (27.6, 15.0), Some(180.0)),
        ("chair", g, (29.57, 15.0), Some(180.0)),
        // study
        ("desk", g, (0.8, 20.5), None),
        ("chair", g, (3.4, 20.5), Some(270.0)),
        ("wardrobe", g, (7.9, 21.5), None),
        // mudroom, powder room, mechanical room
        ("wardrobe", g, (38.1, 7.6), Some(180.0)),
        ("wc", g, (39.75, 23.4), None),
        ("washbasin", g, (36.6, 20.5), None),
        ("boiler", g, (47.5, 17.6), None),
        ("consumer-unit", g, (47.0, 16.4), None),
        // bedroom 2
        // the bed clear of the radiator (MU141)
        ("double-bed", f, (11.4, 7.75), None),
        ("bedside-table", f, (11.4, 3.95), None),
        ("bedside-table", f, (11.4, 11.55), None),
        ("wardrobe", f, (2.0, 16.1), None),
        ("wardrobe", f, (5.4, 16.1), None),
        ("desk", f, (0.7, 3.0), None),
        ("chair", f, (3.3, 3.0), Some(270.0)),
        // bedroom 3
        ("double-bed", f, (12.6, 7.75), None),
        ("bedside-table", f, (12.6, 3.95), None),
        ("bedside-table", f, (12.6, 11.55), None),
        ("wardrobe", f, (18.6, 16.1), None),
        ("wardrobe", f, (22.0, 16.1), None),
        ("desk", f, (23.4, 3.5), None),
        ("chair", f, (20.8, 3.5), Some(90.0)),
        // primary bedroom, its bed against the west wall, the wardrobe on
        // the east wall; no armchair, which blocked the way (MU141)
        ("double-bed", f, (24.6, 5.5), None),
        ("bedside-table", f, (24.6, 2.0), None),
        ("bedside-table", f, (24.6, 9.0), None),
        ("wardrobe", f, (34.55, 4.86), Some(270.0)),
        // the en-suite: the shower against the wet wall away from the door,
        // the WC on the wet wall by the chase, the basin on the east wall
        // (MU141)
        ("shower", f, (29.4, 15.75), Some(0.0)),
        ("wc", f, (32.38, 16.05), Some(0.0)),
        ("washbasin", f, (34.82, 13.99), Some(270.0)),
        // the bathroom: a 5 ft bath across its east end, its taps at the
        // wet wall; the WC and basin on the wet wall
        ("bathtub", f, (34.35, 20.6), Some(90.0)),
        ("wc", f, (31.4, 18.95), Some(180.0)),
        ("washbasin", f, (29.22, 18.54), Some(180.0)),
        // laundry: washer and dryer bays
        ("base-cabinet-600", f, (23.0, 22.9), Some(0.0)),
        // the landing's desk under the west window
        ("desk", f, (0.7, 20.5), None),
        ("chair", f, (3.3, 20.5), Some(270.0)),
        // electrical: the panel is in the mechanical room
        ("switch-2-gang", g, (36.4, 7.8), None),
        ("switch-2-gang", g, (35.52, 12.7), Some(270.0)),
        ("switch-1-gang", g, (4.2, 16.7), None),
        ("switch-1-gang", g, (37.2, 16.2), None),
        ("switch-1-gang", g, (43.2, 16.2), None),
        ("socket-double", g, (0.4, 3.0), None),
        ("socket-double", g, (0.48, 12.9), Some(90.0)),
        ("socket-double", g, (12.0, 0.48), Some(180.0)),
        ("socket-double", g, (24.6, 0.48), Some(180.0)),
        ("socket-double", g, (35.52, 3.0), Some(270.0)),
        ("socket-double", g, (26.6, 23.5), None),
        ("socket-double", g, (34.0, 21.0), None),
        ("socket-double", g, (4.0, 23.5), None),
        ("socket-double", g, (47.5, 8.0), None),
        ("socket-single", g, (47.5, 22.5), None),
        ("ceiling-light", g, (7.0, 4.92), Some(0.0)),
        ("ceiling-light", g, (18.0, 8.0), Some(0.0)),
        ("ceiling-light", g, (29.17, 5.48), Some(0.0)),
        ("ceiling-light", g, (4.2, 20.2), Some(0.0)),
        ("ceiling-light", g, (42.0, 11.0), Some(0.0)),
        ("downlight", g, (25.63, 17.0), Some(0.0)),
        ("downlight", g, (27.6, 17.0), Some(0.0)),
        ("downlight", g, (29.6, 17.0), Some(0.0)),
        ("downlight", g, (27.0, 21.0), Some(0.0)),
        ("downlight", g, (31.5, 21.0), Some(0.0)),
        ("downlight", g, (38.7, 20.0), Some(0.0)),
        ("downlight", g, (44.8, 20.0), Some(0.0)),
        ("downlight", g, (22.5, 21.5), Some(0.0)),
        ("smoke-detector", g, (22.0, 14.0), Some(0.0)),
        ("smoke-detector", g, (42.0, 13.5), Some(0.0)),
        ("extract-fan", g, (38.6, 23.6), None),
        ("extract-fan", g, (47.6, 19.0), None),
        ("outside-tap", g, (-0.6, 2.0), None),
        ("outside-tap", g, (48.6, 22.0), None),
        ("switch-1-gang", f, (7.6, 16.7), None),
        ("switch-1-gang", f, (17.0, 16.3), None),
        ("switch-1-gang", f, (27.7, 11.6), None),
        ("switch-1-gang", f, (27.3, 23.2), None),
        ("socket-double", f, (0.48, 12.0), Some(90.0)),
        ("socket-double", f, (11.0, 0.48), Some(180.0)),
        ("socket-double", f, (13.0, 0.48), Some(180.0)),
        ("socket-double", f, (23.6, 12.5), None),
        ("socket-double", f, (25.0, 0.48), Some(180.0)),
        ("socket-double", f, (35.55, 2.11), Some(270.0)),
        ("socket-double", f, (4.0, 23.5), None),
        ("wall-light", f, (11.6, 5.35), None),
        ("wall-light", f, (11.6, 10.15), None),
        ("wall-light", f, (12.4, 5.35), None),
        ("wall-light", f, (12.4, 10.15), None),
        ("wall-light", f, (24.4, 3.4), None),
        ("wall-light", f, (24.4, 7.6), None),
        ("wall-light", f, (0.5, 12.6), Some(90.0)),
        ("wall-light", f, (23.6, 14.5), None),
        // the landing is vaulted: its lights are on the corridor's wall,
        // as the bedrooms' are on theirs
        ("wall-light", f, (3.5, 16.9), None),
        ("wall-light", f, (12.5, 16.9), None),
        ("wall-light", f, (21.0, 16.9), None),
        ("downlight", f, (25.8, 17.5), Some(0.0)),
        ("downlight", f, (25.8, 22.0), Some(0.0)),
        ("downlight", f, (31.0, 13.2), Some(0.0)),
        ("downlight", f, (30.5, 21.6), Some(0.0)),
        ("downlight", f, (22.0, 21.8), Some(0.0)),
        // the hall's smoke alarm and the supply diffuser: on the passage's
        // flat ceiling
        ("smoke-detector", f, (25.8, 15.0), Some(0.0)),
        ("extract-grille", f, (32.2, 22.6), Some(0.0)),
        ("extract-grille", f, (29.6, 13.2), Some(0.0)),
        ("supply-diffuser", f, (25.8, 20.0), Some(0.0)),
        // under the shower
        ("floor-drain", f, (29.4, 15.75), Some(0.0)),
        // bedrooms 2 and 3: on the partition, clear of the bedside tables
        // (MU141)
        ("radiator", f, (11.6, 13.85), None),
        ("radiator", f, (12.4, 13.85), None),
        ("radiator", f, (26.82, 0.57), Some(180.0)),
        ("radiator", f, (4.0, 23.5), None),
    ];
    for (key, lvl, at, turn) in plan {
        let at = p(at.0, at.1);
        let c = crate::catalogue::starter(&format!("starter/{key}")).unwrap_or_else(|| {
            let keys: Vec<String> = crate::catalogue::starters()
                .into_iter()
                .map(|c| c.key)
                .collect();
            panic!("no starter {key}: {keys:?}")
        });
        let (q, r) = match turn {
            Some(r) => (crate::objects::centred(&c.object, at, r), r),
            None => {
                let Some(spot) = crate::objects::against_wall_clear(
                    b.doc.model(),
                    lvl,
                    at,
                    0.9,
                    &c.object,
                    c.object.offset,
                ) else {
                    println!(
                        "  note: no wall near {key} at {:.1},{:.1} ft",
                        at.x / FT,
                        at.y / FT
                    );
                    continue;
                };
                if spot.blocked {
                    println!(
                        "  note: {key} at {:.1},{:.1} ft is in the way of an opening",
                        at.x / FT,
                        at.y / FT
                    );
                }
                (spot.at, spot.rotation)
            }
        };
        let _ = Mount::Ceiling;
        let (tx, _) = crate::objects::place(b.doc.model(), &c, lvl, q, r);
        b.commit(tx);
    }
    // a car in the court and the family on the terrace, for scale
    for (key, at, turn, z) in [
        ("car", (63.0, -27.0), 90.0, COURT),
        ("car", (72.0, -19.0), 90.0, COURT),
        ("person", (14.0, -8.0), 200.0, TERRACE),
        ("child", (17.0, -9.0), 150.0, TERRACE),
        ("person", (43.0, 2.5), 0.0, TERRACE),
    ] {
        let Some(c) = crate::catalogue::starter(&format!("starter/{key}")) else {
            println!("  note: no starter {key}");
            continue;
        };
        let q = crate::objects::centred(&c.object, p(at.0, at.1), turn);
        let (tx, id) = crate::objects::place(b.doc.model(), &c, g, q, turn);
        b.commit(tx);
        b.doc
            .edit(id, "Ridgeline House: entourage", |e| {
                if let ElementKind::Object(o) = &mut e.kind {
                    o.offset = z;
                }
            })
            .unwrap();
    }
}
