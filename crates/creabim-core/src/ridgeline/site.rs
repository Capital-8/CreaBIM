//! The lot: 130 × 120 ft of ground falling to the south, the street along
//! its east side, the house's pad cut into the slope, a drive and a
//! parking court, the walk to the porch, drains, planting and the soils
//! report's two boreholes.

use super::*;
use crate::investigation::{FieldTest, ProbeKind, TestKind, WaterStrike};
use crate::lot::LotEdge;
use crate::services::{DrainUse, TankUse};
use crate::site::{Pattern, PlantForm};
use crate::*;
use glam::{DVec2, DVec3};

/// The ground as surveyed, metres above the ground floor: it falls about
/// 1 in 18 to the south, with a knoll in the north-west corner and a
/// hollow where the street's ditch runs.
pub fn natural(q: DVec2) -> f64 {
    let c = p(24.0, 12.0);
    let d = q - c;
    -0.3 + 0.055 * d.y + 0.012 * d.x + 0.6 * (-(q - p(-20.0, 45.0)).length_squared() / 60.0).exp()
        - 0.4 * (-(q - p(62.0, -44.0)).length_squared() / 50.0).exp()
        + 0.06 * (q.x / 3.1).sin() * (q.y / 4.3).cos()
}

/// The parking court's level.
pub const COURT: f64 = -0.8;

pub fn build(b: &mut Builder) {
    let g = b.id("Ground Floor");
    // the survey: levels on a rough 12 ft grid, as a surveyor walks it
    let (lo, hi) = (p(-56.0, -74.0), p(128.0, 74.0));
    let mut seed = 7u64;
    let mut rnd = || {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (seed >> 33) as f64 / (1u64 << 31) as f64 - 0.5
    };
    let mut spots = vec![];
    let (nx, ny) = (15, 12);
    for j in 0..=ny {
        for i in 0..=nx {
            let edge = i == 0 || i == nx || j == 0 || j == ny;
            let jitter = if edge {
                DVec2::ZERO
            } else {
                DVec2::new(rnd(), rnd()) * 1.4
            };
            let q =
                lo + (hi - lo) * DVec2::new(i as f64 / nx as f64, j as f64 / ny as f64) + jitter;
            spots.push(DVec3::new(q.x, q.y, natural(q)));
        }
    }
    let mut ground = Site::terrain(g, crate::polygon::rectangle(lo, hi), 0.0, 2.0);
    if let SiteForm::Terrain {
        points,
        contour,
        data,
        ..
    } = &mut ground.form
    {
        *points = spots;
        // one-foot contours, every fifth heavier and labelled
        *contour = FT;
        data.major = 5;
        data.survey = true;
        data.labels = true;
        data.under_buildings = true;
        data.strata = vec![
            Stratum {
                name: "Topsoil".into(),
                thickness: 0.25,
            },
            Stratum {
                name: "Glacial till: stiff sandy clay".into(),
                thickness: 1.9,
            },
        ];
    }
    ground.material = Some(b.id("Grass"));
    let mut tx = Transaction::new("Ridgeline House: the ground");
    tx = b.keep(
        "Ground",
        Element::new("Ground", ElementKind::Site(ground)),
        tx,
    );

    // the lot, its street edge the front
    let mut lot = Site::property(g, rect(-45.0, -60.0, 85.0, 60.0));
    if let SiteForm::Property { edges, rules, .. } = &mut lot.form {
        *edges = vec![LotEdge::Side, LotEdge::Front, LotEdge::Side, LotEdge::Rear];
        rules.front = ft(25.0);
        rules.side = ft(10.0);
        rules.rear = ft(25.0);
        rules.height = ft(35.0);
        rules.coverage = 0.3;
    }
    tx = b.keep("Lot 7", Element::new("Lot 7", ElementKind::Site(lot)), tx);
    b.commit(tx);

    // the street, and the drive in from it to the court
    let mut tx = Transaction::new("Ridgeline House: street and drive");
    let on_ground = |s: &mut Site| {
        if let SiteForm::Road(r) = &mut s.form {
            for c in r.path.iter_mut() {
                c.z = natural(c.truncate());
            }
        }
    };
    let mut street = Site::road(
        g,
        vec![
            p(101.0, -74.0),
            p(100.0, -20.0),
            p(101.0, 30.0),
            p(103.0, 74.0),
        ],
        ft(24.0),
    );
    on_ground(&mut street);
    if let SiteForm::Road(r) = &mut street.form {
        r.radius = 60.0;
        r.footpath = 1.5;
    }
    tx = b.keep(
        "Ridgeline Road",
        Element::new("Ridgeline Road", ElementKind::Site(street)),
        tx,
    );
    let mut drive = Site::road(g, vec![p(89.0, -23.0), p(79.0, -23.0)], ft(12.0));
    if let SiteForm::Road(r) = &mut drive.form {
        r.path[0].z = natural(p(89.0, -23.0));
        r.path[1].z = COURT;
        r.kerb = 0.0;
        r.crowned = false;
    }
    tx = b.keep("Drive", Element::new("Drive", ElementKind::Site(drive)), tx);
    b.commit(tx);

    // earthworks: the house's pad a step below the floor, the court lower
    let mut tx = Transaction::new("Ridgeline House: pads");
    let house = Site::pad(g, rect(-6.0, -18.0, 54.5, 30.0), GRADE);
    tx = b.keep(
        "House pad",
        Element::new("House pad", ElementKind::Site(house)),
        tx,
    );
    let court_outline = rect(54.5, -34.0, 79.0, -12.0);
    // its formation level, under the paving
    let mut court = Site::pad(g, court_outline.clone(), COURT - 0.1);
    // its west edge (edge 3, from its north-west corner south) stands
    // upright: the house's pad is 2 ft above it there
    let t = 0.254;
    let concrete = b.id("Concrete, board-formed");
    let mut wall = Wall::new(
        g,
        DVec2::new(ft(54.5) - t / 2.0, ft(-16.2)),
        DVec2::new(ft(54.5) - t / 2.0, ft(-34.0)),
        t,
        GRADE + 0.15 - (COURT - 0.1),
    );
    wall.base_offset = COURT - 0.1;
    wall.material = Some(concrete);
    let wall = Element::new("Court retaining wall", ElementKind::Wall(wall));
    if let SiteForm::Pad {
        retained, walls, ..
    } = &mut court.form
    {
        retained.push(3);
        walls.push((3, wall.id));
    }
    tx = b.keep("Court retaining wall", wall, tx);
    tx = b.keep(
        "Court pad",
        Element::new("Court pad", ElementKind::Site(court)),
        tx,
    );
    let mut paving = Site::area(
        g,
        court_outline,
        0.1,
        // 24 by 12 in concrete pavers, stretcher bond: a unit a site plan
        // can show (4 by 8 in blocks closed into a patch, owner MU125)
        Pattern::Units {
            size: DVec2::new(inch(24.0), inch(12.0)),
            stagger: true,
            angle: 0.0,
        },
    );
    if let SiteForm::Area { top, .. } = &mut paving.form {
        *top = COURT;
    }
    paving.material = Some(b.material("Block paving"));
    tx = b.keep(
        "Parking court",
        Element::new("Parking court", ElementKind::Site(paving)),
        tx,
    );
    // the way up from the court: steps through a board-formed retaining
    // wall to the house's pad, then a paved path to the porch
    let steps_y = (-16.0, -12.0);
    let mut path = Site::area(
        g,
        vec![
            p(40.764, steps_y.0),
            p(54.5, steps_y.0),
            p(54.5, steps_y.1),
            // its leg to the porch centred on the entry door and its
            // sidelight (MU141)
            p(44.764, steps_y.1),
            p(44.764, -0.5),
            p(40.764, -0.5),
        ],
        0.1,
        Pattern::Units {
            size: DVec2::new(0.6, 0.6),
            stagger: false,
            angle: 0.0,
        },
    );
    if let SiteForm::Area { top, .. } = &mut path.form {
        *top = GRADE + 0.02;
    }
    path.material = Some(b.material("Paving, concrete flags"));
    tx = b.keep(
        "Entry path",
        Element::new("Entry path", ElementKind::Site(path)),
        tx,
    );
    b.commit(tx);

    // services: the foul drain from the stack to the street's sewer, the
    // roof water to a cistern below the terrace
    let mut tx = Transaction::new("Ridgeline House: drains");
    let named = |n: &str, s: Site| Element::new(n, ElementKind::Site(s));
    // the sewer leaves below the frost
    let mut foul = Site::drain(
        g,
        vec![p(35.0, 26.5), p(60.0, 24.0), p(87.0, 16.0)],
        DrainUse::Foul,
    );
    if let SiteForm::Drain(d) = &mut foul.form {
        d.depth = 1.5;
    }
    tx = tx.insert(named("Foul drain", foul));
    // it runs down the slope, so it falls with it: at the usual 1 in 80
    // the ground would drop away from under the pipe
    let mut roof_water = Site::drain(
        g,
        vec![p(-3.5, 25.0), p(-8.0, 4.0), p(-16.0, -22.0)],
        DrainUse::Surface,
    );
    if let SiteForm::Drain(d) = &mut roof_water.form {
        d.fall = 6.5;
        d.depth = 1.0;
    }
    tx = tx.insert(named("Roof water drain", roof_water));
    tx = tx.insert(named(
        "Rainwater cistern",
        Site::tank(g, p(-18.0, -27.0), TankUse::Rainwater),
    ));
    b.commit(tx);

    // planting: a windbreak of conifers along the north boundary,
    // deciduous shade to the south-west, shrubs by the entry
    let mut tx = Transaction::new("Ridgeline House: planting");
    for (i, (x, y, canopy, height)) in [
        (-36.0, 52.0, 4.2, 10.0),
        (-20.0, 54.0, 4.8, 12.0),
        (-3.0, 51.0, 4.0, 9.5),
        (15.0, 54.0, 4.6, 11.5),
        (33.0, 52.0, 4.2, 10.5),
        (52.0, 54.0, 4.8, 12.5),
        (70.0, 51.0, 4.0, 9.0),
    ]
    .into_iter()
    .enumerate()
    {
        tx = tx.insert(named(
            &format!("White spruce {}", i + 1),
            Site::plant(g, p(x, y), PlantForm::Conifer, canopy, height),
        ));
    }
    for (name, x, y, canopy, height) in [
        ("Sugar maple 1", -30.0, -28.0, 7.5, 9.0),
        ("Sugar maple 2", -12.0, -46.0, 6.5, 8.0),
        ("Paper birch 1", 16.0, -42.0, 5.0, 8.5),
        ("Paper birch 2", 70.0, 30.0, 5.0, 8.0),
        ("Paper birch 3", 64.0, 8.0, 4.5, 7.5),
    ] {
        let shape = TreeShape::new_at(canopy, p(x, y));
        tx = tx.insert(named(name, Site::tree(g, p(x, y), shape, canopy, height)));
    }
    for (i, (x, y)) in [
        (51.5, 3.0),
        (52.5, 9.0),
        (51.0, 15.0),
        (-4.0, -12.0),
        (28.0, -13.5),
        (33.0, -13.0),
        (52.5, -6.5),
    ]
    .into_iter()
    .enumerate()
    {
        tx = tx.insert(named(
            &format!("Dogwood {}", i + 1),
            Site::plant(g, p(x, y), PlantForm::Shrub, 1.6, 1.3),
        ));
    }
    b.commit(tx);

    // the soils report
    let mut tx = Transaction::new("Ridgeline House: boreholes");
    let report = "GI-2026-118, North Shore Geotechnical";
    for (name, at, depth, rows, water, spt) in [
        (
            "BH1",
            p(18.0, 12.0),
            6.0,
            "0.00 0.25 GT1 TOPSOIL: dark brown silty loam with rootlets\n\
             0.25 2.15 GT2 Stiff reddish-brown sandy CLAY (glacial till), occasional cobbles\n\
             2.15 4.40 GT3 Very stiff grey silty CLAY with gravel\n\
             4.40 6.00 GT4 Weathered GABBRO bedrock",
            Some((3.6, 3.3)),
            [(1.0, 14.0), (2.0, 22.0), (3.5, 36.0), (4.5, 50.0)],
        ),
        (
            "BH2",
            p(66.0, -24.0),
            5.0,
            "0.00 0.30 GT1 TOPSOIL: dark brown silty loam\n\
             0.30 2.60 GT2 Stiff reddish-brown sandy CLAY (glacial till)\n\
             2.60 5.00 GT3 Very stiff grey silty CLAY with gravel",
            Some((2.9, 2.6)),
            [(1.0, 11.0), (2.0, 19.0), (3.0, 31.0), (4.5, 44.0)],
        ),
    ] {
        let mut site = Site::probe(g, at, ProbeKind::Borehole);
        if let SiteForm::Investigation(pr) = &mut site.form {
            pr.depth = depth;
            pr.read_layers(rows);
            pr.report = report.into();
            pr.date = "14 Sep 2026".into();
            pr.method = "Hollow-stem auger, 6 in".into();
            if let Some((struck, rest)) = water {
                pr.water.push(WaterStrike {
                    depth: struck,
                    rest: Some(rest),
                    note: "after 30 min".into(),
                });
            }
            for (d, n) in spt {
                pr.tests.push(FieldTest {
                    depth: d,
                    kind: TestKind::Spt,
                    value: n,
                    text: String::new(),
                });
            }
        }
        tx = b.keep(name, Element::new(name, ElementKind::Site(site)), tx);
    }
    b.commit(tx);
    let log = Element::new(
        "BH1 log",
        ElementKind::View(View::borehole_log(b.id("BH1"))),
    );
    let tx = b.keep(
        "BH1 log",
        log,
        Transaction::new("Ridgeline House: borehole log"),
    );
    b.commit(tx);

    // where the survey says the lot is
    let mut info = b
        .doc
        .model()
        .project_info()
        .map(|(_, i)| i.clone())
        .unwrap_or_default();
    info.map = Some(MapPlace {
        crs: "EPSG:26915".into(),
        easting: 568_620.0,
        northing: 5_181_540.0,
    });
    let tx = set_project(b.doc.model(), info);
    b.commit(tx);
}
