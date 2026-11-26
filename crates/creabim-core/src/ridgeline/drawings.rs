//! The views of the set, then what is drafted over them: details, tags,
//! notes and dimensions.

use super::*;
use crate::site::SitePreset;
use crate::visibility::Category as C;
use crate::*;
use glam::DVec2;

/// The views, by what the sheets call them.
pub struct Views {
    pub site: ElementId,
    pub foundation: ElementId,
    pub ground: ElementId,
    pub first: ElementId,
    pub roof: ElementId,
    pub ceilings: [ElementId; 2],
    /// A-A across the house through the stair, B-B along it.
    pub sections: [ElementId; 2],
    pub site_section: ElementId,
    /// South, east, north, west.
    pub elevations: [ElementId; 4],
    pub street: ElementId,
    pub garden: ElementId,
    pub schedules: Vec<ElementId>,
}

/// Plans at 1/4" = 1'-0".
pub const QUARTER: f64 = 48.0;

pub fn views(b: &mut Builder) -> Views {
    let (t, g, f, r) = (
        b.id("T.O. Footing"),
        b.id("Ground Floor"),
        b.id("First Floor"),
        b.id("Roof Plate"),
    );
    let mut tx = Transaction::new("Ridgeline House: views");
    let plan = |level: ElementId, scale: f64, order: u32, cut: f64, hide: &[C]| {
        let mut v = View::plan(level);
        v.scale = scale;
        v.browser_order = Some(order);
        if let ViewKind::Plan { cut_height, .. } = &mut v.kind {
            *cut_height = cut;
        }
        v.hidden.categories = hide.to_vec();
        v
    };
    let add = |b: &mut Builder, tx: Transaction, name: &str, v: View| {
        let e = Element::new(name, ElementKind::View(v));
        b.keep(name, e, tx)
    };
    // inside the house the ground and its planting stay out of the plans
    let indoors = [C::Site, C::Planting, C::Footings];
    // the site plan looks down on the roofs, with the lot's legal lines
    let below = [
        C::Walls,
        C::Doors,
        C::Windows,
        C::Objects,
        C::Railings,
        C::Stairs,
        C::Beams,
        C::Columns,
        C::Openings,
        C::Rooms,
        C::Ceilings,
        C::Footings,
        C::Grids,
    ];
    // the house on its lot: its ground floor's walls, cut, among the
    // lot's legal lines, levels and services. What stands outdoors shows:
    // the steps up from the court with their rail, the cars parked on it.
    // What is indoors of those kinds is hidden one by one as it is built
    // (`envelope::site_plan_outdoors_only`). Beams stay out: the canopy's
    // and the porch's are overhead, not site work
    let on_site = [
        C::Rooms,
        C::Ceilings,
        C::Footings,
        C::Grids,
        C::Beams,
        C::Tags,
    ];
    let mut site = plan(g, 120.0, 0, 1.2, &on_site);
    site.site = SitePreset::Permit.view();
    tx = add(b, tx, "Site plan", site);
    let mut foundation = plan(t, QUARTER, 1, 0.6, &[C::Site, C::Planting]);
    foundation.underlay = Underlay::Level(g);
    tx = add(b, tx, "Foundation plan", foundation);
    tx = add(
        b,
        tx,
        "Ground Floor plan",
        plan(g, QUARTER, 2, 1.2, &indoors),
    );
    tx = add(
        b,
        tx,
        "First Floor plan",
        plan(f, QUARTER, 3, 1.2, &indoors),
    );
    let mut over = below.to_vec();
    over.extend([C::Site, C::Planting, C::Slabs]);
    // the roof windows stay: they are what the roof plan shows of them
    over.retain(|c| *c != C::Grids && *c != C::Windows);
    // the roofs hang off the first floor: looked down on from over the ridge
    let _ = r;
    tx = add(b, tx, "Roof plan", plan(f, QUARTER, 4, 6.5, &over));
    for (i, (name, level)) in [
        ("Ground Floor ceiling plan", g),
        ("First Floor ceiling plan", f),
    ]
    .into_iter()
    .enumerate()
    {
        let mut c = plan(level, 64.0, i as u32, 2.25, &indoors);
        c.ceiling_plan = true;
        tx = add(b, tx, name, c);
    }
    for (name, a, bb, depth, cut, scale) in [
        // across the house through the stair and the living room, looking west
        (
            "Section A-A",
            p(17.0, 34.0),
            p(17.0, -14.0),
            ft(14.0),
            true,
            QUARTER,
        ),
        // along the house through the living room and the wing, looking north
        (
            "Section B-B",
            p(-8.0, 9.0),
            p(56.0, 9.0),
            ft(18.0),
            true,
            QUARTER,
        ),
        // the whole lot, north to south, through the house
        (
            "Site section",
            p(30.0, 70.0),
            p(30.0, -70.0),
            ft(20.0),
            true,
            120.0,
        ),
        (
            "South Elevation",
            p(-12.0, -24.0),
            p(60.0, -24.0),
            40.0,
            false,
            QUARTER,
        ),
        (
            "East Elevation",
            p(60.0, -16.0),
            p(60.0, 36.0),
            40.0,
            false,
            QUARTER,
        ),
        (
            "North Elevation",
            p(60.0, 36.0),
            p(-12.0, 36.0),
            40.0,
            false,
            QUARTER,
        ),
        (
            "West Elevation",
            p(-14.0, 36.0),
            p(-14.0, -16.0),
            40.0,
            false,
            QUARTER,
        ),
    ] {
        let mut view = View::section(a, bb, depth, cut);
        view.scale = scale;
        if cut {
            view.hidden.categories = vec![C::Openings, C::Planting];
        } else {
            view.style = ViewStyle::Shaded;
            view.hidden.categories = vec![C::Openings, C::Footings, C::Planting];
        }
        if scale < 100.0 {
            // a drawing of the building: the ground, without the survey's notes
            view.site.investigation = false;
            view.site.levels = false;
            view.site.services = false;
        }
        tx = add(b, tx, name, view);
    }
    let mut street = View::axonometric(35.0, 24.0);
    street.scale = 190.0;
    street.hidden.categories = vec![C::Openings, C::Footings, C::Grids];
    tx = add(b, tx, "3D from the south-east", street);
    let mut cover = View::axonometric(325.0, 26.0);
    cover.scale = 135.0;
    cover.hidden.categories = vec![C::Openings, C::Footings, C::Grids];
    tx = add(b, tx, "3D of the lot", cover);
    let mut garden = View::axonometric(325.0, 26.0);
    garden.scale = 190.0;
    garden.hidden.categories = vec![C::Openings, C::Footings, C::Grids];
    tx = add(b, tx, "3D from the south-west", garden);
    let mut schedules = vec![];
    for category in [
        ScheduleCategory::Doors,
        ScheduleCategory::Windows,
        ScheduleCategory::Rooms,
        ScheduleCategory::Walls,
        ScheduleCategory::Slabs,
        ScheduleCategory::Structure,
        ScheduleCategory::Stairs,
        ScheduleCategory::Furniture,
        ScheduleCategory::Fixtures,
        ScheduleCategory::Materials,
        ScheduleCategory::Drainage,
    ] {
        let name = category.schedule_name();
        tx = add(b, tx, &name, View::schedule(category));
        schedules.push(b.id(&name));
    }
    // the wall types, one row each, beside the wall-by-wall schedule
    let mut types = View::schedule(ScheduleCategory::Walls);
    if let ViewKind::Schedule { by_type, .. } = &mut types.kind {
        *by_type = true;
    }
    tx = add(b, tx, "Wall Type Schedule", types);
    b.commit(tx);
    Views {
        site: b.id("Site plan"),
        foundation: b.id("Foundation plan"),
        ground: b.id("Ground Floor plan"),
        first: b.id("First Floor plan"),
        roof: b.id("Roof plan"),
        ceilings: [
            b.id("Ground Floor ceiling plan"),
            b.id("First Floor ceiling plan"),
        ],
        sections: [b.id("Section A-A"), b.id("Section B-B")],
        site_section: b.id("Site section"),
        elevations: [
            b.id("South Elevation"),
            b.id("East Elevation"),
            b.id("North Elevation"),
            b.id("West Elevation"),
        ],
        street: b.id("3D from the south-east"),
        garden: b.id("3D from the south-west"),
        schedules,
    }
}

/// Where the wall section C-C is cut: through the west living-room slider
/// and bedroom 2's window, from inside to outside, looking east.
pub const WALL_SECTION_X: f64 = 6.0;
/// Its start, north of the south wall, in feet; along the section the
/// wall's centre line is this far from the start.
pub const WALL_SECTION_FROM: f64 = 7.0;

/// A detail view: its name and the corners of its callout box, in the
/// wall section's drawing (metres along it, and height).
pub type DetailBox = (&'static str, (f64, f64), (f64, f64));

/// The bedroom window's sill and head above the ground floor: the head
/// at 7'-0" upstairs leaves room for its header under the plates.
pub const SILL: f64 = STOREY + 2.0 * FT;
pub const HEAD: f64 = SILL + 5.0 * FT;

/// The wall's centre line along the section, metres.
pub fn wall_at() -> f64 {
    ft(WALL_SECTION_FROM)
}

pub fn detail_boxes() -> [DetailBox; 4] {
    let w = wall_at();
    let top = STOREY + PLATE;
    [
        ("Detail 1 Foundation", (w - 0.75, -1.65), (w + 0.75, 0.45)),
        (
            "Detail 2 Floor and slider head",
            (w - 0.75, STOREY - 0.85),
            (w + 0.75, STOREY + 0.3),
        ),
        (
            "Detail 3 Window sill",
            (w - 0.6, SILL - 0.35),
            (w + 0.6, SILL + 0.35),
        ),
        (
            "Detail 4 Window head and eaves",
            (w - 0.75, HEAD - 0.3),
            (w + 1.25, top + 0.95),
        ),
    ]
}

/// The wall section and the details called out from it.
pub fn details(b: &mut Builder) {
    let mut tx = Transaction::new("Ridgeline House: wall section");
    let x = WALL_SECTION_X;
    let mut section = View::section(p(x, WALL_SECTION_FROM), p(x, -9.0), 0.6, true);
    section.scale = 24.0;
    section.hidden.categories = vec![C::Openings, C::Objects, C::Planting, C::Rooms];
    section.site.investigation = false;
    section.site.levels = false;
    section.site.services = false;
    let parent = section.clone();
    tx = b.keep(
        "Section C-C",
        Element::new("Section C-C", ElementKind::View(section)),
        tx,
    );
    b.commit(tx);
    let sid = b.id("Section C-C");
    let mut tx = Transaction::new("Ridgeline House: details");
    for (name, lo, hi) in detail_boxes() {
        let mut d = View::callout(
            sid,
            &parent,
            DVec2::new(lo.0, lo.1),
            DVec2::new(hi.0, hi.1),
            8.0,
        );
        d.site = parent.site.clone();
        d.hidden = parent.hidden.clone();
        tx = b.keep(name, Element::new(name, ElementKind::View(d)), tx);
    }
    b.commit(tx);
    super::detailing::draft(b);
}

/// Room tags, door and window tags, notes and dimensions.
pub fn annotate(b: &mut Builder, views: &Views) {
    let (g, f) = (b.id("Ground Floor"), b.id("First Floor"));
    let mut tx = Transaction::new("Ridgeline House: annotation styles");
    let room_style = TextStyle {
        size: 3.0,
        bold: true,
        purpose: StyleUse::Notes,
        ..Default::default()
    };
    tx = b.keep(
        "Room name",
        Element::new("Room name", ElementKind::TextStyle(room_style)),
        tx,
    );
    for (name, s) in crate::annotation::starter_text_styles() {
        tx = b.keep(name, Element::new(name, ElementKind::TextStyle(s)), tx);
    }
    b.commit(tx);
    let room = Some(b.id("Room name"));

    // rooms: name, number and area, read live from the walls round them
    let model = b.doc.model().clone();
    let mut tx = Transaction::new("Ridgeline House: room tags");
    for e in model.iter() {
        let ElementKind::Room(r) = &e.kind else {
            continue;
        };
        let view = if r.level == f {
            views.first
        } else {
            views.ground
        };
        let mut t = Tag::new(view, e.id, DVec2::ZERO);
        t.style = room;
        tx = tx.insert(Element::new("Room tag", ElementKind::Tag(t)));
    }
    // each ceiling's height, in the ceiling plans (MU142)
    for t in crate::room::ceiling_tags_missing(&model, None, None) {
        tx = tx.insert(t);
    }
    // doors and windows, in the plan of their storey
    for e in model.iter() {
        let ElementKind::Opening(o) = &e.kind else {
            continue;
        };
        if o.kind == OpeningKind::Hole || o.roof_at.is_some() {
            continue;
        }
        let Some(ElementKind::Wall(w)) = model.get(o.host).map(|h| &h.kind) else {
            continue;
        };
        let view = if w.level == f {
            views.first
        } else if w.level == g {
            views.ground
        } else {
            continue;
        };
        // the outside is to a wall's right; windows are tagged outside,
        // doors on the side they open to
        let right = -w.direction().perp();
        let exterior = w.thickness > 0.2 || w.curtain.is_some();
        let out = if exterior {
            right * 0.75
        } else {
            let left = !o.faces_left(w);
            (if left { right } else { -right }) * 0.5
        };
        tx = tx.insert(Element::new(
            "Tag",
            ElementKind::Tag(Tag::new(view, e.id, out)),
        ));
    }
    b.commit(tx);

    // dimensions: the grid and the overall size, outside the walls
    let dim = |view: ElementId, a: DVec2, bb: DVec2, offset: f64| {
        Element::new(
            "Dimension",
            ElementKind::Dimension(Dimension::linear(
                view,
                HostPoint::free(a),
                HostPoint::free(bb),
                offset,
            )),
        )
    };
    let mut tx = Transaction::new("Ridgeline House: dimensions");
    for (view, east, south_off) in [
        (views.ground, 48.0, 15.0),
        (views.foundation, 48.0, 10.0),
        (views.first, 36.0, 6.0),
    ] {
        // along the south: grid to grid, then overall
        let xs: Vec<f64> = (0..=((east / 12.0) as usize))
            .map(|i| 12.0 * i as f64)
            .collect();
        for w in xs.windows(2) {
            tx = tx.insert(dim(view, p(w[0], 0.0), p(w[1], 0.0), -ft(south_off)));
        }
        tx = tx.insert(dim(view, p(0.0, 0.0), p(east, 0.0), -ft(south_off + 2.5)));
        // up the east side
        for w in [0.0, 12.0, 24.0].windows(2) {
            tx = tx.insert(dim(view, p(east, w[0]), p(east, w[1]), -ft(7.0)));
        }
        tx = tx.insert(dim(view, p(east, 0.0), p(east, 24.0), -ft(9.5)));
    }
    // the ground floor: the wing's set-back, and the rooms' key sizes
    tx = tx.insert(dim(views.ground, p(48.0, 0.0), p(48.0, 6.0), -ft(4.5)));
    tx = tx.insert(dim(views.ground, p(48.0, 6.0), p(48.0, 24.0), -ft(4.5)));
    tx = tx.insert(dim(views.ground, p(0.0, 24.0), p(8.5, 24.0), ft(4.0)));
    tx = tx.insert(dim(views.ground, p(8.5, 24.0), p(36.0, 24.0), ft(4.0)));
    tx = tx.insert(dim(views.ground, p(36.0, 24.0), p(41.5, 24.0), ft(4.0)));
    tx = tx.insert(dim(views.ground, p(41.5, 24.0), p(48.0, 24.0), ft(4.0)));
    tx = tx.insert(dim(views.ground, p(0.0, 24.0), p(0.0, 16.5), -ft(5.0)));
    tx = tx.insert(dim(views.ground, p(0.0, 16.5), p(0.0, 0.0), -ft(5.0)));
    // the first floor: partitions along the north and the west
    for w in [0.0, 12.0, 24.0, 27.5, 36.0].windows(2) {
        tx = tx.insert(dim(views.first, p(w[0], 24.0), p(w[1], 24.0), ft(6.0)));
    }
    for w in [24.0, 16.5, 0.0].windows(2) {
        tx = tx.insert(dim(views.first, p(0.0, w[0]), p(0.0, w[1]), -ft(6.0)));
    }
    // storey heights beside the sections
    for section in views.sections {
        let lv = |name: &str, h: f64| HostPoint {
            host: Some(b.id(name)),
            local: DVec2::new(0.8, h),
            from_end: false,
            body: None,
        };
        for (a, c) in [
            ("T.O. Footing", "Ground Floor"),
            ("Ground Floor", "First Floor"),
            ("First Floor", "Roof Plate"),
        ] {
            tx = tx.insert(Element::new(
                "Dimension",
                ElementKind::Dimension(Dimension::linear(section, lv(a, 0.0), lv(c, 0.0), 0.0)),
            ));
        }
    }
    b.commit(tx);

    // what the plans need saying
    #[allow(clippy::type_complexity)]
    let notes: Vec<(ElementId, (f64, f64), &str, Option<(f64, f64)>)> = vec![
        (
            views.foundation,
            (52.0, -4.0),
            "STRIP FOOTINGS 20\" × 10\"\n4000 psi concrete, 2 #5 continuous\nT.O. footing -4'-0\"\nbearing 50 in below finished grade",
            Some((48.0, 12.0)),
        ),
        (
            views.foundation,
            (-22.0, 20.0),
            "8\" STEM WALL, BOARD-FORMED\n2\" XPS inside face, R-10\n#4 at 16\" o.c. each way",
            Some((0.0, 18.0)),
        ),
        (
            views.foundation,
            (4.0, 18.0),
            "4\" SLAB ON GRADE, RADIANT\n15 mil vapour barrier\n4\" XPS, R-20\n6\" drainage gravel",
            None,
        ),
        (
            views.foundation,
            (14.0, 7.0),
            "PAD 30\" × 30\" × 12\"\nunder each 6×6 post",
            Some((12.0, 12.0)),
        ),
        (
            views.foundation,
            (-20.0, -9.0),
            "12\" DIA. PIERS TO FROST DEPTH\non 24\" × 24\" × 10\" pads",
            Some((0.0, -6.0)),
        ),
        (
            views.roof,
            (6.0, 6.0),
            "STANDING SEAM, CHARCOAL\n8 : 12\nR-60 assembly\n40 psf roof snow load",
            None,
        ),
        (views.roof, (22.0, 21.0), "STANDING SEAM\n8 : 12", None),
        (
            views.roof,
            (38.5, 13.0),
            "EPDM ON TAPERED POLYISO\n1/4\" per foot to scuppers\nR-60",
            None,
        ),
        (views.roof, (5.0, -4.0), "TERRACE CANOPY\nstanding seam on purlins", None),
        (
            views.roof,
            (20.0, 30.0),
            "ROOF WINDOWS 30\" × 55\"\ncentre pivot, triple glazed",
            Some((17.0, 19.5)),
        ),
        (
            views.ground,
            (26.5, 11.0),
            "GLULAM 5 1/2\" × 15\" OVER\non 6×6 posts",
            Some((20.0, 12.0)),
        ),
        (
            views.ground,
            (38.5, 27.5),
            "SERVICE CHASE\nstacks and ducts, heated",
            Some((34.9, 16.7)),
        ),
        (
            views.first,
            (39.0, 21.5),
            "SERVICE CHASE\nover the kitchen's",
            Some((34.9, 16.7)),
        ),
        (views.first, (6.0, 12.6), "RIDGE BEAM OVER\nvaulted to the roof", None),
    ];
    let mut tx = Transaction::new("Ridgeline House: notes");
    for (view, at, text, leader) in notes {
        let mut n = Note::new(view, p(at.0, at.1), text);
        n.leader = leader.map(|l| HostPoint::free(p(l.0, l.1)));
        tx = tx.insert(Element::new("Note", ElementKind::Note(n)));
    }
    b.commit(tx);
}
