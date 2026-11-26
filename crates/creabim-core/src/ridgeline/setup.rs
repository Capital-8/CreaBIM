//! The project's foundations: units, information, levels, the palette of
//! materials, the layered assemblies and the types everything is made of.

use super::*;
use crate::curtain::{CurtainGrid, GridRule, MullionProfile, PanelKind};
use crate::footing::FootingKind;
use crate::screen::ScreenPattern;
use crate::types::{
    BeamType, ColumnType, FootingType, OpeningType, SlabType, TypeParams, WallType,
};
use crate::*;
use glam::DVec2;

pub fn build(b: &mut Builder) {
    project(b);
    levels(b);
    materials(b);
    assemblies_and_types(b);
}

/// Feet and inches to a sixteenth, and what the title blocks read.
fn project(b: &mut Builder) {
    let mut units = Units::imperial();
    units.precision = 4;
    let tx = crate::units::set_units(b.doc.model(), units);
    b.commit(tx);
    let info = ProjectInfo {
        name: PROJECT.into(),
        number: "2026-014".into(),
        client: CLIENT.into(),
        status: "Permit set".into(),
        issue_date: "21 November 2026".into(),
        address: ADDRESS.into(),
        site_name: "Lot 7, Ridgeline Road".into(),
        latitude: Some(46.7867),
        longitude: Some(-92.1005),
        elevation: 372.0,
        building_name: "Residence".into(),
        building_use: "Single-family dwelling, IRC R-3".into(),
        notes: "Climate zone 6A. Frost depth 48 in. Ground snow load 60 psf, \
                roof design snow load 40 psf. Wind 115 mph, exposure B."
            .into(),
        ..Default::default()
    };
    let mut tx = set_project(b.doc.model(), info);
    tx.label = "Ridgeline House: project".into();
    b.commit(tx);
}

fn levels(b: &mut Builder) {
    let mut tx = Transaction::new("Ridgeline House: levels");
    for (name, elevation, height) in [
        ("T.O. Footing", ft(-4.0), ft(4.0)),
        ("Ground Floor", 0.0, STOREY),
        ("First Floor", STOREY, PLATE),
        ("Roof Plate", STOREY + PLATE, ft(8.0)),
    ] {
        let e = Element::new(name, ElementKind::Level(Level { elevation, height }));
        tx = b.keep(name, e, tx);
    }
    b.commit(tx);
}

/// Four honest materials carry the house: warm vertical cedar, dark
/// standing-seam metal, board-formed concrete and bronze aluminium. The
/// rest is what is inside the build-ups.
fn materials(b: &mut Builder) {
    let m = |hatch, colour: [f32; 3], category: &str, description: &str, rough: f32, metal: f32| {
        Material {
            category: category.into(),
            description: description.into(),
            roughness: rough,
            metallic: metal,
            ..Material::new(hatch, colour)
        }
    };
    let own = [
        (
            "Cedar cladding, vertical",
            m(
                Hatch::Timber,
                [0.66, 0.45, 0.27],
                "timber",
                "Western red cedar, 1×6 vertical boards, clear oil",
                0.75,
                0.0,
            ),
        ),
        (
            "Standing seam, charcoal",
            m(
                Hatch::Solid,
                [0.17, 0.18, 0.19],
                "steel",
                "24 ga steel standing seam, matte charcoal",
                0.55,
                0.5,
            ),
        ),
        (
            "Concrete, board-formed",
            m(
                Hatch::Concrete,
                [0.63, 0.62, 0.59],
                "concrete",
                "Cast-in-place concrete, board-formed face, 4000 psi",
                0.9,
                0.0,
            ),
        ),
        (
            "Aluminium, dark bronze",
            m(
                Hatch::Solid,
                [0.17, 0.15, 0.13],
                "aluminium",
                "Thermally broken aluminium, dark bronze anodised",
                0.4,
                0.7,
            ),
        ),
        (
            "Stud frame 2×6, mineral wool",
            m(
                Hatch::Diagonal,
                [0.93, 0.84, 0.55],
                "timber",
                "2×6 studs at 16 in o.c., R-23 mineral wool batts",
                0.9,
                0.0,
            ),
        ),
        (
            "Insulation, XPS",
            m(
                Hatch::Cross,
                [0.74, 0.85, 0.95],
                "insulation",
                "Extruded polystyrene, R-5 per inch",
                0.9,
                0.0,
            ),
        ),
        (
            "Insulation, cellulose",
            m(
                Hatch::Cross,
                [0.24, 0.24, 0.25],
                "insulation",
                "Dense-pack cellulose between I-joist rafters",
                0.9,
                0.0,
            ),
        ),
        (
            "Insulation, polyiso",
            m(
                Hatch::Cross,
                [0.22, 0.22, 0.23],
                "insulation",
                "Polyisocyanurate roof boards, R-6 per inch",
                0.9,
                0.0,
            ),
        ),
        (
            "Membrane, vapour control",
            m(
                Hatch::Solid,
                [0.2, 0.45, 0.75],
                "membrane",
                "Smart vapour retarder, taped",
                0.6,
                0.0,
            ),
        ),
        (
            "Fir, clear finish",
            m(
                Hatch::Timber,
                [0.78, 0.60, 0.40],
                "timber",
                "Douglas fir glulam and posts, clear finish",
                0.6,
                0.0,
            ),
        ),
    ];
    let mut tx = Transaction::new("Ridgeline House: materials");
    for (name, mat) in own {
        let e = Element::new(name, ElementKind::Material(mat));
        tx = b.keep(name, e, tx);
    }
    b.commit(tx);
    for name in [
        "Insulation, mineral wool",
        "Insulation, PIR",
        "OSB",
        "Plywood",
        "Plasterboard",
        "Timber, softwood",
        "Timber, oak",
        "Parquet, oak",
        "Concrete, cast in place",
        "Concrete, polished",
        "Gravel",
        "Membrane, bitumen",
        "Membrane, EPDM",
        "Paint, white",
        "Timber decking",
        "Paving, concrete flags",
        "Grass",
        "Ceramic tile",
        "Asphalt",
    ] {
        b.material(name);
    }
    // rates for the take-off, in US dollars, supplied and fixed: per
    // square metre, cubic metre or metre, as the take-off measures
    use crate::takeoff::{Rate, RateUnit::*};
    for (name, amount, per) in [
        ("Cedar cladding, vertical", 130.0, Area),
        ("Standing seam, charcoal", 160.0, Area),
        ("Concrete, board-formed", 420.0, Volume),
        ("Concrete, cast in place", 320.0, Volume),
        ("Stud frame 2×6, mineral wool", 68.0, Area),
        ("Insulation, mineral wool", 210.0, Volume),
        ("Insulation, XPS", 250.0, Volume),
        ("Insulation, cellulose", 95.0, Volume),
        ("Insulation, polyiso", 330.0, Volume),
        ("Membrane, vapour control", 6.0, Area),
        ("Membrane, bitumen", 9.0, Area),
        ("Membrane, EPDM", 32.0, Area),
        ("OSB", 11.0, Area),
        ("Plywood", 22.0, Area),
        ("Plasterboard", 9.0, Area),
        ("Timber, softwood", 780.0, Volume),
        ("Timber, oak", 3200.0, Volume),
        ("Fir, clear finish", 1900.0, Volume),
        ("Parquet, oak", 110.0, Area),
        ("Timber decking", 140.0, Area),
        ("Gravel", 60.0, Volume),
        ("Paint, white", 5.0, Area),
    ] {
        let id = b.id(name);
        b.doc
            .edit(id, "Ridgeline House: rates", |e| {
                if let ElementKind::Material(m) = &mut e.kind {
                    m.rate = Some(Rate { amount, per });
                }
            })
            .expect("a material takes its rate");
    }
}

fn assemblies_and_types(b: &mut Builder) {
    use LayerFunction::*;
    let l = |b: &Builder, f: LayerFunction, m: &str, t: f64| Layer::new(f, Some(b.id(m)), t);
    let air = |t: f64| Layer::new(AirGap, None, t);
    // outside to inside, top to bottom
    let timber_wall = |b: &Builder, face: &str| {
        vec![
            l(b, Finish, face, 0.019),
            air(0.019),
            l(b, Insulation, "Insulation, mineral wool", 0.051),
            l(b, Substrate, "OSB", 0.011),
            l(b, Structure, "Stud frame 2×6, mineral wool", 0.140),
            l(b, Membrane, "Membrane, vapour control", 0.001),
            l(b, Finish, "Plasterboard", 0.016),
        ]
    };
    let assemblies: Vec<(&str, AssemblyUse, &str, Vec<Layer>)> = vec![
        (
            "Exterior wall R-31, cedar rainscreen",
            AssemblyUse::Wall,
            "Cedar on 3/4 in vented cavity, 2 in mineral wool, sheathing, 2×6 studs with R-23 batts, vapour control, gypsum board",
            timber_wall(b, "Cedar cladding, vertical"),
        ),
        (
            "Exterior wall R-31, standing seam",
            AssemblyUse::Wall,
            "Charcoal standing seam on 3/4 in vented cavity, 2 in mineral wool, sheathing, 2×6 studs with R-23 batts, vapour control, gypsum board",
            timber_wall(b, "Standing seam, charcoal"),
        ),
        (
            "Foundation wall 10 in, insulated",
            AssemblyUse::Wall,
            "8 in board-formed concrete stem wall, 2 in XPS on its inside face",
            vec![
                l(b, Structure, "Concrete, board-formed", 0.203),
                l(b, Insulation, "Insulation, XPS", 0.051),
            ],
        ),
        (
            "Partition 2×4",
            AssemblyUse::Wall,
            "5/8 in gypsum board both sides of 2×4 studs",
            vec![
                l(b, Finish, "Plasterboard", 0.016),
                l(b, Structure, "Timber, softwood", 0.089),
                l(b, Finish, "Plasterboard", 0.016),
            ],
        ),
        (
            "Partition 2×6, plumbing",
            AssemblyUse::Wall,
            "5/8 in gypsum board both sides of 2×6 studs: wet walls and pocket doors",
            vec![
                l(b, Finish, "Plasterboard", 0.016),
                l(b, Structure, "Timber, softwood", 0.140),
                l(b, Finish, "Plasterboard", 0.016),
            ],
        ),
        (
            "Slab on grade R-20",
            AssemblyUse::Floor,
            "Oak on a 4 in concrete slab with radiant tubing, vapour barrier, 4 in XPS, 6 in drainage gravel",
            vec![
                l(b, Finish, "Parquet, oak", 0.019),
                l(b, Structure, "Concrete, cast in place", 0.102),
                l(b, Membrane, "Membrane, vapour control", 0.001),
                l(b, Insulation, "Insulation, XPS", 0.102),
                l(b, Substrate, "Gravel", 0.152),
            ],
        ),
        (
            "Upper floor, I-joist",
            AssemblyUse::Floor,
            "Oak on 3/4 in plywood on 11 7/8 in I-joists at 16 in o.c., gypsum board ceiling",
            vec![
                l(b, Finish, "Parquet, oak", 0.019),
                l(b, Substrate, "Plywood", 0.019),
                l(b, Structure, "Timber, softwood", 0.302),
                l(b, Finish, "Plasterboard", 0.016),
            ],
        ),
        (
            "Pitched roof R-60, standing seam",
            AssemblyUse::Roof,
            "Standing seam on underlayment and sheathing, vent space, 14 in I-joist rafters dense-packed with cellulose, 1 1/2 in polyiso, vapour control, gypsum board",
            vec![
                l(b, Finish, "Standing seam, charcoal", 0.025),
                l(b, Membrane, "Membrane, bitumen", 0.002),
                l(b, Substrate, "OSB", 0.016),
                air(0.038),
                l(b, Insulation, "Insulation, cellulose", 0.356),
                l(b, Insulation, "Insulation, polyiso", 0.038),
                l(b, Membrane, "Membrane, vapour control", 0.001),
                l(b, Finish, "Plasterboard", 0.016),
            ],
        ),
        (
            "Flat roof R-60, membrane",
            AssemblyUse::Roof,
            "EPDM on 8 in tapered polyiso on sheathing, 2×10 joists with batts, gypsum board",
            vec![
                l(b, Finish, "Membrane, EPDM", 0.003),
                l(b, Insulation, "Insulation, polyiso", 0.203),
                l(b, Substrate, "Plywood", 0.019),
                l(b, Structure, "Stud frame 2×6, mineral wool", 0.235),
                l(b, Finish, "Plasterboard", 0.016),
            ],
        ),
    ];
    let mut tx = Transaction::new("Ridgeline House: assemblies");
    let mut totals = HashMap::new();
    for (name, usage, description, layers) in assemblies {
        let mut a = Assembly::new(usage, layers);
        a.description = description.into();
        totals.insert(name, a.total());
        let e = Element::new(name, ElementKind::Assembly(a));
        tx = b.keep(name, e, tx);
    }
    b.commit(tx);
    assert!((totals["Exterior wall R-31, cedar rainscreen"] - EXT).abs() < 1e-9);
    assert!((totals["Partition 2×4"] - PART).abs() < 1e-9);
    assert!((totals["Upper floor, I-joist"] - FLOOR).abs() < 1e-9);

    // types take their assembly's name
    let walls = [
        ("Exterior wall R-31, cedar rainscreen", "The main block"),
        ("Exterior wall R-31, standing seam", "The entry wing"),
        (
            "Foundation wall 10 in, insulated",
            "Stem walls to frost depth",
        ),
        ("Partition 2×4", "Room partitions"),
        ("Partition 2×6, plumbing", "Wet walls and pocket doors"),
    ];
    for (name, description) in walls {
        let params = TypeParams::Wall(WallType {
            thickness: totals[name],
            assembly: Some(b.id(name)),
            material: None,
            location: LocationLine::Centre,
            height: None,
            curtain: None,
            screen: None,
        });
        b.add_type(name, description, params);
    }
    let cedar = Some(b.id("Cedar cladding, vertical"));
    let bronze = Some(b.id("Aluminium, dark bronze"));
    let fir = Some(b.id("Fir, clear finish"));
    // vertical 2×4 fins on edge, 5 1/2 in apart, in a frame
    b.add_type(
        "Timber slat screen",
        "Vertical cedar fins, 1 1/2 in wide at 5 1/2 in centres: the brise-soleil and the stair screen",
        TypeParams::Wall(WallType {
            thickness: 0.089,
            assembly: None,
            material: cedar,
            location: LocationLine::Centre,
            height: None,
            curtain: None,
            screen: Some(Box::new(ScreenPattern {
                unit: DVec2::new(0.14, 1.0),
                holes: vec![[DVec2::new(0.038, 0.0), DVec2::new(0.14, 1.0)]],
                frame: 0.089,
                stagger: false,
            })),
        }),
    );
    b.add_type(
        "Dining glazing, 3 bays",
        "Three tall bays of triple glazing in bronze aluminium",
        TypeParams::Wall(WallType {
            thickness: 0.15,
            assembly: None,
            material: bronze,
            location: LocationLine::Centre,
            height: None,
            curtain: Some(CurtainGrid {
                vertical: GridRule::Count(3),
                horizontal: GridRule::Count(1),
                mullion_width: 0.06,
                mullion_depth: 0.15,
                panel: PanelKind::Glass,
                profile: MullionProfile::Stick,
                transom_depth: Some(0.1),
                ..Default::default()
            }),
            screen: None,
        }),
    );

    let slab = |b: &Builder, a: &str| SlabType {
        thickness: totals[a],
        assembly: Some(b.id(a)),
        material: None,
    };
    let plain = |t: f64, m: Option<ElementId>| SlabType {
        thickness: t,
        assembly: None,
        material: m,
    };
    let s = TypeParams::Slab(slab(b, "Slab on grade R-20"));
    b.add_type("Slab on grade R-20", "Ground floor", s);
    let s = TypeParams::Slab(slab(b, "Upper floor, I-joist"));
    b.add_type("Upper floor, I-joist", "First floor", s);
    let deck = Some(b.id("Timber decking"));
    b.add_type(
        "Deck 2 in, hardwood",
        "Deck boards on sleepers",
        TypeParams::Slab(plain(0.05, deck)),
    );
    let conc = Some(b.id("Concrete, board-formed"));
    b.add_type(
        "Porch slab 5 in, concrete",
        "Broom-finished concrete on insulation",
        TypeParams::Slab(plain(0.127, conc)),
    );
    let r = TypeParams::Roof(slab(b, "Pitched roof R-60, standing seam"));
    b.add_type("Pitched roof R-60, standing seam", "Main roof, 8:12", r);
    let r = TypeParams::Roof(slab(b, "Flat roof R-60, membrane"));
    b.add_type("Flat roof R-60, membrane", "Wing roof, 1/4 in per foot", r);
    let seam = Some(b.id("Standing seam, charcoal"));
    b.add_type(
        "Canopy 4 in, standing seam",
        "The terrace canopy: metal on purlins",
        TypeParams::Roof(plain(0.1, seam)),
    );

    // doors
    let white = Some(b.id("Paint, white"));
    let door = |w: f64, h: f64, op: Operation, m: Option<ElementId>| {
        TypeParams::Door(OpeningType {
            width: w,
            height: h,
            sill: 0.0,
            frame: OpeningFrame::of_operation(OpeningKind::Door, op),
            material: m,
        })
    };
    b.add_type(
        "Entry door 3'-6\" × 8'-0\"",
        "Insulated cedar plank door, U-0.20",
        door(inch(42.0), ft(8.0), Operation::Standard, cedar),
    );
    b.add_type(
        "Service door 3'-0\" × 6'-8\"",
        "Insulated steel door",
        door(inch(36.0), inch(80.0), Operation::Standard, bronze),
    );
    b.add_type(
        "Interior door 2'-8\" × 6'-8\"",
        "Solid core, painted",
        door(inch(32.0), inch(80.0), Operation::Standard, white),
    );
    b.add_type(
        "Pocket door 2'-6\" × 6'-8\"",
        "Solid core, painted, in a 2×6 wall",
        door(inch(30.0), inch(80.0), Operation::Pocket, white),
    );
    b.add_type(
        "Closet doors 4'-0\" × 6'-8\"",
        "Bifold pair",
        door(inch(48.0), inch(80.0), Operation::BifoldPair, white),
    );
    let mut slider = OpeningFrame::of_operation(OpeningKind::Door, Operation::MultiSlide);
    slider.set_panels(OpeningKind::Door, 4, 0b1001);
    b.add_type(
        "Terrace slider 10'-0\" × 8'-0\"",
        "Four-panel lift-slide, triple glazed, U-0.17: fixed, sliding, sliding, fixed",
        TypeParams::Door(OpeningType {
            width: ft(10.0),
            height: ft(8.0),
            sill: 0.0,
            frame: slider,
            material: bronze,
        }),
    );

    // windows: triple glazed, bronze aluminium-clad
    use crate::cells::WindowStyle;
    let window = |w: f64, h: f64, sill: f64, style: WindowStyle, panes: [u32; 2]| {
        let mut frame = OpeningFrame {
            depth: 0.11,
            ..OpeningFrame::for_kind(OpeningKind::Window)
        };
        if style == WindowStyle::Fixed {
            frame = frame.with_panes(panes[0], panes[1]);
        } else {
            frame.grid = style.grid(false, panes);
        }
        TypeParams::Window(OpeningType {
            width: w,
            height: h,
            sill,
            frame,
            material: bronze,
        })
    };
    for (name, description, w, h, sill, style, panes) in [
        (
            "Window 8'-0\" × 5'-0\", three lights",
            "Bedrooms: fixed",
            ft(8.0),
            ft(5.0),
            ft(2.0),
            WindowStyle::Fixed,
            [3, 1],
        ),
        (
            "Window 6'-0\" × 6'-0\", fixed",
            "Living room, west",
            ft(6.0),
            ft(6.0),
            ft(2.0),
            WindowStyle::Fixed,
            [2, 1],
        ),
        (
            "Window 4'-0\" × 5'-0\", casement pair",
            "Bedrooms, the gable ends",
            ft(4.0),
            ft(5.0),
            ft(2.0),
            WindowStyle::CasementPair,
            [1, 1],
        ),
        (
            "Window 4'-0\" × 4'-0\", casement",
            "Study and landing",
            ft(4.0),
            ft(4.0),
            ft(3.0),
            WindowStyle::Casement,
            [1, 1],
        ),
        (
            "Window 6'-0\" × 3'-4\", sliding",
            "Over the kitchen counter, clear of the sink",
            ft(6.0),
            inch(40.0),
            inch(44.0),
            WindowStyle::Sliding,
            [1, 1],
        ),
        (
            "Window 2'-0\" × 3'-0\", awning",
            "Bathrooms, obscured glass",
            ft(2.0),
            ft(3.0),
            ft(4.0),
            WindowStyle::Awning,
            [1, 1],
        ),
        (
            "Window 3'-0\" × 6'-0\", tilt-turn",
            "The stair and the mudroom",
            ft(3.0),
            ft(6.0),
            ft(2.0),
            WindowStyle::TiltTurn,
            [1, 1],
        ),
        (
            "Sidelight 1'-6\" × 8'-0\"",
            "Beside the entry door",
            ft(1.5),
            ft(7.85),
            ft(0.15),
            WindowStyle::Fixed,
            [1, 1],
        ),
    ] {
        b.add_type(name, description, window(w, h, sill, style, panes));
    }

    // structure
    b.add_type(
        "Glulam beam 5 1/2\" × 15\"",
        "24F-V4 Douglas fir, carries the first floor on grid 2",
        TypeParams::Beam(BeamType {
            width: 0.14,
            depth: 0.381,
            material: fir,
        }),
    );
    b.add_type(
        "Glulam ridge 5 1/2\" × 18\"",
        "24F-V4 Douglas fir structural ridge",
        TypeParams::Beam(BeamType {
            width: 0.14,
            depth: 0.457,
            material: fir,
        }),
    );
    b.add_type(
        "Timber beam 4×10",
        "Canopy and porch beams",
        TypeParams::Beam(BeamType {
            width: 0.089,
            depth: 0.235,
            material: fir,
        }),
    );
    b.add_type(
        "Timber post 6×6",
        "Douglas fir, on steel standoff bases",
        TypeParams::Column(ColumnType {
            profile: Profile::Rectangle {
                width: 0.14,
                depth: 0.14,
            },
            material: fir,
        }),
    );
    let cast = Some(b.id("Concrete, cast in place"));
    b.add_type(
        "Concrete pier 12\" dia.",
        "Sonotube pier to frost depth",
        TypeParams::Column(ColumnType {
            profile: Profile::Round { diameter: 0.305 },
            material: cast,
        }),
    );
    let footing = |kind, width, length, thickness| {
        TypeParams::Footing(FootingType {
            kind,
            width,
            length,
            thickness,
            material: cast,
        })
    };
    b.add_type(
        "Strip footing 20\" × 10\"",
        "Under the stem walls, two #5 continuous",
        footing(FootingKind::Strip, inch(20.0), 0.0, inch(10.0)),
    );
    b.add_type(
        "Pad footing 30\" × 30\" × 12\"",
        "Under the interior posts",
        footing(FootingKind::Pad, inch(30.0), inch(30.0), inch(12.0)),
    );
    b.add_type(
        "Pad footing 24\" × 24\" × 10\"",
        "Under the piers",
        footing(FootingKind::Pad, inch(24.0), inch(24.0), inch(10.0)),
    );
}
