//! A site to try the ground on: the sample house on a levelled platform
//! cut into a slope, as a surveyor would hand it over. Writes the survey
//! as a CSV (to bring in with Import survey) and a project with the same
//! survey already brought in, with a section through it.
//!
//! `cargo run -p creabim-store --example site_demo -- out.creabim survey.csv`
//!
//! With `works` as a third word the survey is of the slope before anyone
//! built on it, and the design does the rest: a pad for the house, a
//! lower one for a car court, a drain trench, two layers of soil, and
//! the ground dug out under the slab.
//!
//! With `works` the soils report is in too: four boreholes, a trial pit
//! and a dynamic probe, logged as the report gives them, and the log of
//! the borehole under the house.
//!
//! The project is placed on the map where the survey is, and the street
//! below the site is written beside the CSV as a LandXML surface in the
//! same map coordinates: bring it in and it lands along the kerb.

use creabim_core::{Document, Element, ElementKind, Site, SiteForm, Transaction, View};
use creabim_exchange::survey;
use glam::{DVec2, DVec3};

/// The slope before anyone built on it.
fn natural(p: DVec2) -> f64 {
    -1.75
        + 0.09 * (p.y + 14.0)
        + 0.012 * p.x
        + 0.9 * (-(p - DVec2::new(19.0, 11.0)).length_squared() / 30.0).exp()
        - 0.5 * (-(p - DVec2::new(-5.0, -8.0)).length_squared() / 25.0).exp()
        + 0.12 * (p.x / 3.1).sin() * (p.y / 4.3).cos()
}

fn main() {
    let works = std::env::args().nth(3).as_deref() == Some("works");
    let mut rows = vec!["Point,Easting,Northing,Level,Code".to_string()];
    // the survey sits in map coordinates, as surveys do
    let origin = DVec3::new(512_360.0, 4_362_138.0, 312.0);
    let mut n = 100;
    let mut put = |p: DVec2, z: f64, code: &str| {
        n += 1;
        let q = p.extend(z) + origin;
        rows.push(format!("{n},{:.3},{:.3},{:.3},{code}", q.x, q.y, q.z));
    };
    // the platform the house stands on, and the foot of its banks
    let (lo, hi) = (DVec2::new(-3.0, -3.5), DVec2::new(13.0, 10.5));
    let ring = |grow: f64, step: f64| {
        let (a, b) = (lo - DVec2::splat(grow), hi + DVec2::splat(grow));
        let mut pts = vec![];
        let mut run = |from: DVec2, to: DVec2| {
            let k = ((to - from).length() / step).round().max(1.0) as usize;
            for i in 0..k {
                pts.push(from + (to - from) * (i as f64 / k as f64));
            }
        };
        run(a, DVec2::new(b.x, a.y));
        run(DVec2::new(b.x, a.y), b);
        run(b, DVec2::new(a.x, b.y));
        run(DVec2::new(a.x, b.y), a);
        pts.push(a);
        pts
    };
    if !works {
        for p in ring(0.0, 4.0) {
            put(p, -0.15, "PLAT");
        }
        for p in ring(1.8, 3.0) {
            put(p, natural(p), "TOE");
        }
    }
    // the kerb along the street, and the road's edge under it
    for i in 0..=8 {
        let x = -9.0 + i as f64 * 3.875;
        put(DVec2::new(x, -12.6), natural(DVec2::new(x, -12.6)), "KERB1");
    }
    for i in 0..=8 {
        let x = -9.0 + i as f64 * 3.875;
        put(
            DVec2::new(x, -12.75),
            natural(DVec2::new(x, -12.75)) - 0.12,
            "KERB2",
        );
    }
    // ground levels on a rough 3.5 m grid, off the platform and its banks
    let mut seed = 11u64;
    let mut rnd = || {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (seed >> 33) as f64 / (1u64 << 31) as f64 - 0.5
    };
    for j in 0..=8 {
        for i in 0..=9 {
            let edge = i == 0 || i == 9 || j == 0 || j == 8;
            let jitter = if edge {
                DVec2::ZERO
            } else {
                DVec2::new(rnd(), rnd()) * 1.6
            };
            let p = DVec2::new(-9.0 + i as f64 * 3.4444, -14.0 + j as f64 * 3.625) + jitter;
            let on_platform = !works && p.cmpgt(lo - 2.4).all() && p.cmplt(hi + 2.4).all();
            let by_kerb = (p.y + 12.7).abs() < 0.6;
            if !on_platform && !by_kerb {
                put(p, natural(p) - if p.y < -12.75 { 0.12 } else { 0.0 }, "GL");
            }
        }
    }
    if !works {
        put(DVec2::new(5.0, 3.5), -0.15, "GL");
    }
    let csv = rows.join("\n") + "\n";

    // the same survey brought in: read as the Import survey window reads it
    let table = survey::table_from_text(&csv);
    let cols = survey::guess_columns(&table).unwrap();
    let (pts, _) = survey::points(&table, cols, 1.0);
    let joined: Vec<String> = ["PLAT", "TOE", "KERB"].map(String::from).to_vec();
    let (mut spots, mut lines) = survey::assemble(&pts, &joined);
    survey::shift(&mut spots, &mut lines, origin);
    let m = creabim_core::sample::sample_house();
    let g = m.iter().find(|e| e.name == "Ground Floor").unwrap().id;
    let flat: Vec<DVec2> = spots.iter().map(|p| p.truncate()).collect();
    let mut site = Site::terrain(g, survey::outline_around(&flat, 0.0), 0.0, 1.0);
    if let SiteForm::Terrain {
        points,
        contour,
        data,
        ..
    } = &mut site.form
    {
        *points = spots;
        data.lines = lines;
        data.survey = true;
        data.labels = true;
        data.shift = origin;
        *contour = 0.25;
        data.major = 4;
        if works {
            data.pads.push(creabim_core::Pad::new(
                creabim_core::polygon::rectangle(DVec2::new(14.5, -11.5), DVec2::new(21.0, -5.5)),
                -1.0,
            ));
            // a drain trench across the slope below the house
            data.pads.push(creabim_core::Pad::dig(
                creabim_core::site::trench_outline(
                    &[
                        DVec2::new(-7.0, -8.5),
                        DVec2::new(5.0, -9.5),
                        DVec2::new(12.5, -9.0),
                    ],
                    0.6,
                ),
                1.0,
                0.0,
            ));
            data.strata = vec![
                creabim_core::Stratum {
                    name: "Topsoil".into(),
                    thickness: 0.3,
                },
                creabim_core::Stratum {
                    name: "Clay".into(),
                    thickness: 0.9,
                },
            ];
            data.under_buildings = true;
        }
    }
    let mut doc = Document::from_model(m);
    let mut tx = Transaction::new("site");
    if let Some((mat, new)) = creabim_core::library::find_or_new(doc.model(), "Grass") {
        site.material = Some(mat);
        if let Some(e) = new {
            tx = tx.insert(e);
        }
    }
    tx = tx.insert(Element::new("Audit terrain", ElementKind::Site(site)));
    if works {
        // the house's pad, its uphill (north) edge held by a retaining
        // wall: an ordinary wall outside the outline, the ground cut at
        // its back
        let mut pad = Site::pad(g, creabim_core::polygon::rectangle(lo, hi), -0.15);
        let t = 0.25;
        let mut wall = creabim_core::Wall::new(
            g,
            DVec2::new(hi.x, hi.y + t / 2.0),
            DVec2::new(lo.x, hi.y + t / 2.0),
            t,
            1.9,
        );
        wall.base_offset = -0.15;
        wall.location = creabim_core::LocationLine::InteriorFace;
        let wall = Element::new("Audit wall", ElementKind::Wall(wall));
        if let SiteForm::Pad {
            retained, walls, ..
        } = &mut pad.form
        {
            retained.push(2);
            walls.push((2, wall.id));
        }
        tx = tx.insert(wall);
        tx = tx.insert(Element::new("Audit pad", ElementKind::Site(pad)));
        // a garden path round from the drive, its bends rounded
        let mut path = creabim_core::walk::Walk::new(
            vec![
                DVec2::new(-8.0, -11.0),
                DVec2::new(-8.5, -2.0),
                DVec2::new(-6.0, 6.0),
                DVec2::new(-3.5, 9.0),
            ],
            creabim_core::walk::Surface::Gravel,
        );
        path.radius = 3.0;
        tx = tx.insert(Element::new(
            "Audit path",
            ElementKind::Site(Site {
                level: g,
                form: SiteForm::Walk(path),
                material: None,
            }),
        ));
        // a drive up from the street to the car court, on the ground as
        // it is at each point, and a foul drain from the house down
        let mut road = Site::road(
            g,
            vec![
                DVec2::new(-9.0, -13.0),
                DVec2::new(6.0, -13.2),
                DVec2::new(14.0, -12.0),
                DVec2::new(17.5, -9.0),
            ],
            3.5,
        );
        if let SiteForm::Road(r) = &mut road.form {
            for c in r.path.iter_mut() {
                c.z = natural(c.truncate());
            }
            r.radius = 6.0;
        }
        tx = tx.insert(Element::new("Audit drive", ElementKind::Site(road)));
        // one of everything else, each named for what it is, to look at
        // its Properties: found by name in the browser
        let named = |n: &str, s: Site| Element::new(n, ElementKind::Site(s));
        let sq = |c: DVec2, h: DVec2| creabim_core::polygon::rectangle(c - h, c + h);
        tx = tx.insert(named(
            "Audit pit",
            Site::pit(g, sq(DVec2::new(17.0, 4.0), DVec2::new(2.0, 1.5)), 1.2, 0.5),
        ));
        tx = tx.insert(named(
            "Audit trench",
            Site::trench(
                g,
                vec![
                    DVec2::new(15.0, 12.0),
                    DVec2::new(19.0, 13.0),
                    DVec2::new(21.0, 9.0),
                ],
                0.6,
                0.9,
            ),
        ));
        tx = tx.insert(named(
            "Audit property",
            Site::property(
                g,
                creabim_core::polygon::rectangle(DVec2::new(-9.5, -13.5), DVec2::new(22.0, 17.0)),
            ),
        ));
        tx = tx.insert(named(
            "Audit tank",
            Site::tank(
                g,
                DVec2::new(-7.5, 2.0),
                creabim_core::services::TankUse::Septic,
            ),
        ));
        tx = tx.insert(named(
            "Audit spot",
            Site::spot(g, DVec2::new(18.0, 15.0), natural(DVec2::new(18.0, 15.0))),
        ));
        tx = tx.insert(named(
            "Audit contour",
            Site::contour(
                g,
                vec![
                    DVec2::new(14.0, 15.5),
                    DVec2::new(18.0, 16.2),
                    DVec2::new(21.0, 15.0),
                ],
                0.6,
            ),
        ));
        tx = tx.insert(named(
            "Audit paving",
            Site::area(
                g,
                sq(DVec2::new(5.0, -5.5), DVec2::new(3.0, 1.2)),
                0.1,
                creabim_core::site::Pattern::Units {
                    size: DVec2::new(0.6, 0.4),
                    stagger: true,
                    angle: 0.0,
                },
            ),
        ));
        tx = tx.insert(named(
            "Audit shrub",
            Site::plant(
                g,
                DVec2::new(-6.0, 12.0),
                creabim_core::site::PlantForm::Shrub,
                1.5,
                1.2,
            ),
        ));
        tx = tx.insert(named(
            "Audit tree",
            Site::plant(
                g,
                DVec2::new(-7.0, 14.5),
                creabim_core::site::PlantForm::Tree,
                4.0,
                6.0,
            ),
        ));
        let drain = Site::drain(
            g,
            vec![
                DVec2::new(-1.0, -4.5),
                DVec2::new(-6.0, -7.0),
                DVec2::new(-8.5, -11.0),
            ],
            creabim_core::services::DrainUse::Foul,
        );
        tx = tx.insert(Element::new("Audit drain", ElementKind::Site(drain)));
        tx = boreholes(tx, g);
    }
    let mut v = View::section(DVec2::new(5.0, -14.5), DVec2::new(5.0, 17.5), 12.0, true);
    v.scale = 100.0;
    tx = tx.insert(Element::new("Site section", ElementKind::View(v)));
    doc.commit(tx).unwrap();
    // the project sits where the survey says: map data brought in later
    // (the street's LandXML below) lands beside it
    let mut info = doc
        .model()
        .project_info()
        .map(|(_, p)| p.clone())
        .unwrap_or_default();
    info.map = Some(creabim_core::MapPlace {
        crs: "EPSG:32613".into(),
        easting: origin.x,
        northing: origin.y,
    });
    info.elevation = origin.z;
    let placed = creabim_core::set_project(doc.model(), info);
    doc.commit(placed).unwrap();
    let out = std::env::args().nth(1).expect("the project to write");
    creabim_store::save(&out, &doc).unwrap();
    println!("saved {out}");
    if let Some(path) = std::env::args().nth(2) {
        std::fs::write(&path, csv).unwrap();
        println!("saved {path}");
        // the street below the site as a LandXML surface, in the same
        // map coordinates (northing, easting, height)
        let mut xml = String::from("<?xml version=\"1.0\"?>\n<LandXML xmlns=\"http://www.landxml.org/schema/LandXML-1.2\" version=\"1.2\">\n <Units><Metric linearUnit=\"meter\" areaUnit=\"squareMeter\" volumeUnit=\"cubicMeter\"/></Units>\n <CoordinateSystem epsgCode=\"32613\" name=\"WGS 84 / UTM zone 13N\"/>\n <Surfaces><Surface name=\"Street\">\n  <SourceData><Breaklines><Breakline brkType=\"standard\" name=\"Far kerb\"><PntList3D>");
        let at = |p: DVec2, z: f64| {
            let q = p.extend(z) + origin;
            format!("{:.3} {:.3} {:.3}", q.y, q.x, q.z)
        };
        let road = |p: DVec2| natural(DVec2::new(p.x, -12.75)) - 0.12 - 0.025 * (-12.75 - p.y);
        for i in 0..=8 {
            let p = DVec2::new(-9.0 + i as f64 * 3.875, -19.0);
            xml.push_str(&format!("{} ", at(p, road(p) + 0.12)));
        }
        xml.push_str("</PntList3D></Breakline></Breaklines></SourceData>\n  <Definition surfType=\"TIN\"><Pnts>\n");
        let mut id = 0;
        for j in 0..=5 {
            for i in 0..=8 {
                let p = DVec2::new(-9.0 + i as f64 * 3.875, -14.0 - j as f64 * 1.8);
                let kerb = if p.y < -19.0 { 0.12 } else { 0.0 };
                id += 1;
                xml.push_str(&format!(
                    "   <P id=\"{id}\">{}</P>\n",
                    at(p, road(p) + kerb)
                ));
            }
        }
        xml.push_str("  </Pnts></Definition>\n </Surface></Surfaces>\n</LandXML>\n");
        let street = std::path::Path::new(&path).with_extension("street.xml");
        std::fs::write(&street, xml).unwrap();
        println!("saved {}", street.display());
    }
}

/// The soils report's investigation points, logged as it gives them, and
/// the log of the borehole under the house.
fn boreholes(mut tx: Transaction, level: creabim_core::ElementId) -> Transaction {
    use creabim_core::investigation::{FieldTest, ProbeKind, TestKind, WaterStrike};
    let report = "GI-2026-031, Geo Consult";
    // (name, kind, where, depth, rows as the report prints them, water, SPTs)
    type Row<'a> = (
        &'a str,
        ProbeKind,
        DVec2,
        f64,
        &'a str,
        Option<(f64, f64)>,
        &'a [(f64, f64, &'a str)],
    );
    let points: [Row; 6] = [
        (
            "BH1",
            ProbeKind::Borehole,
            DVec2::new(-6.0, -6.5),
            8.0,
            "0.00 0.30 GT1 TOPSOIL: dark brown sandy loam with rootlets\n\
             0.30 1.40 GT2 MADE GROUND: brown sandy gravel with brick and concrete fragments\n\
             1.40 3.60 GT3 Firm brown mottled grey silty CLAY, medium plasticity\n\
             3.60 5.20 GT4 Dense orange-brown fine to medium SAND\n\
             5.20 8.00 GT5 Weathered SANDSTONE, recovered as angular gravel",
            Some((3.4, 3.0)),
            &[(1.5, 6.0, ""), (2.5, 11.0, ""), (4.0, 34.0, ""), (5.5, 50.0, "50/180 mm")],
        ),
        (
            "BH2",
            ProbeKind::Borehole,
            DVec2::new(17.0, -3.0),
            8.0,
            "0.00 0.25 GT1 TOPSOIL: dark brown sandy loam\n\
             0.25 2.90 GT3 Firm to stiff brown silty CLAY\n\
             2.90 4.10 GT4 Dense orange-brown fine SAND\n\
             4.10 8.00 GT5 Weathered SANDSTONE becoming moderately strong",
            Some((2.8, 2.5)),
            &[(1.0, 9.0, ""), (2.0, 14.0, ""), (3.5, 38.0, ""), (5.0, 50.0, "50/120 mm")],
        ),
        (
            "BH3",
            ProbeKind::Borehole,
            DVec2::new(5.0, 3.5),
            10.0,
            "0.00 0.25 GT1 TOPSOIL: dark brown sandy loam with rootlets\n\
             0.25 1.05 GT2 MADE GROUND: brown sandy gravel with brick and concrete fragments\n\
             1.05 3.05 GT3 Firm brown mottled grey silty CLAY, medium plasticity\n\
             3.05 5.25 GT4 Dense orange-brown fine to medium SAND\n\
             5.25 10.00 GT5 Weathered SANDSTONE, recovered as angular gravel; moderately strong below 7.0 m",
            Some((3.1, 2.8)),
            &[(1.5, 9.0, ""), (2.5, 12.0, ""), (3.5, 31.0, ""), (4.5, 38.0, ""), (6.0, 50.0, "50/220 mm"), (8.0, 50.0, "50/90 mm")],
        ),
        (
            "BH4",
            ProbeKind::Borehole,
            DVec2::new(-4.5, 15.0),
            6.0,
            "0.00 0.35 GT1 TOPSOIL: dark brown clayey loam\n\
             0.35 1.60 GT2 MADE GROUND: soft brown clay with ash and brick\n\
             1.60 3.00 GT3 Soft becoming firm grey silty CLAY\n\
             3.00 6.00 GT4 Medium dense brown clayey SAND",
            Some((2.2, 1.9)),
            &[(1.0, 4.0, ""), (2.0, 7.0, ""), (3.5, 21.0, ""), (5.0, 27.0, "")],
        ),
        (
            "TP1",
            ProbeKind::TrialPit,
            DVec2::new(18.5, 13.5),
            2.4,
            "0.00 0.30 GT1 TOPSOIL: dark brown sandy loam\n\
             0.30 2.40 GT3 Firm brown silty CLAY with occasional gravel",
            None,
            &[],
        ),
        (
            "DP1",
            ProbeKind::Probe,
            DVec2::new(8.5, -10.5),
            4.0,
            "",
            None,
            &[],
        ),
    ];
    let mut bh3 = None;
    for (name, kind, at, depth, rows, water, spt) in points {
        let mut site = Site::probe(level, at, kind);
        if let SiteForm::Investigation(p) = &mut site.form {
            p.depth = depth;
            p.read_layers(rows);
            p.report = report.into();
            p.date = "02–06 Oct 2026".into();
            p.method = match kind {
                ProbeKind::Borehole => "Cable percussion, 150 mm".into(),
                ProbeKind::TrialPit => "Machine dug, 0.6 m bucket".into(),
                ProbeKind::Probe => "Dynamic probe, super heavy".into(),
            };
            if let Some((struck, rest)) = water {
                p.water.push(WaterStrike {
                    depth: struck,
                    rest: Some(rest),
                    note: "after 20 min".into(),
                });
            }
            for (d, n, text) in spt {
                p.tests.push(FieldTest {
                    depth: *d,
                    kind: TestKind::Spt,
                    value: *n,
                    text: text.to_string(),
                });
            }
            if kind == ProbeKind::Probe {
                for k in 1..=8 {
                    let d = k as f64 * 0.5;
                    p.tests.push(FieldTest {
                        depth: d,
                        kind: TestKind::Dynamic,
                        value: (2.0 + d * d * 1.6).round(),
                        text: String::new(),
                    });
                }
            }
        }
        let e = Element::new(name, ElementKind::Site(site));
        if name == "BH3" {
            bh3 = Some(e.id);
        }
        tx = tx.insert(e);
    }
    if let Some(id) = bh3 {
        tx = tx.insert(Element::new(
            "BH3 log",
            ElementKind::View(View::borehole_log(id)),
        ));
    }
    tx
}
