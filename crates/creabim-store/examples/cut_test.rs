//! A made-up site to try upright cuts and bank corners on: a bumpy hill
//! and, worked into it, every kind of corner a pad, a pit or a trench
//! can have. Written fresh, nothing taken from the audit file.
//!
//! `cargo run -p creabim-store --example cut_test -- out.creabim`
//!
//! - A: a pit, one edge upright, on the slope.
//! - B: a pit turned 30°, two edges side by side upright.
//! - C: an L-shaped pad cut into the hill (an inside corner), one edge
//!   held by a retaining wall.
//! - D: a five-sided pad, half dug in and half made up, with a sharp
//!   60° corner and a wide 135° one, banks all round.
//! - E: a pit with all four edges upright (a box).
//! - F: a trench with two bends, upright sides.
//! - G: a pad with two upright edges either side of a sloping one.
//! - R: a road down the hill past C, its bends tight (0.46 m on its
//!   middle, as the owner set one), each point at the ground.
//! - Trees on the slope, one on the knoll.

use creabim_core::{Document, Element, ElementKind, Level, Site, SiteForm, Transaction};
use glam::{DVec2, DVec3};

/// The hill: rising to the north-east, with a knoll and a hollow and a
/// ripple, so no two corners sit on the same slope.
fn hill(p: DVec2) -> f64 {
    0.07 * p.y + 0.03 * p.x + 1.1 * (-(p - DVec2::new(14.0, 10.0)).length_squared() / 40.0).exp()
        - 0.7 * (-(p - DVec2::new(-12.0, -6.0)).length_squared() / 30.0).exp()
        + 0.15 * (p.x / 2.7).sin() * (p.y / 3.9).cos()
}

fn turned(c: DVec2, half: DVec2, deg: f64) -> Vec<DVec2> {
    let r = DVec2::from_angle(deg.to_radians());
    [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
        .iter()
        .map(|(x, y)| c + r.rotate(DVec2::new(x * half.x, y * half.y)))
        .collect()
}

fn main() {
    let out = std::env::args().nth(1).expect("the project to write");
    let lv = Element::new(
        "Ground Floor",
        ElementKind::Level(Level {
            elevation: 0.0,
            height: 3.0,
        }),
    );
    let g = lv.id;
    let sq = |c: DVec2, h: DVec2| creabim_core::polygon::rectangle(c - h, c + h);
    // the survey: a 2.5 m grid over the hill, as a surveyor would hand it
    let mut ground = Site::terrain(g, sq(DVec2::ZERO, DVec2::new(30.0, 22.0)), 0.0, 3.0);
    if let SiteForm::Terrain {
        points,
        data,
        contour,
        ..
    } = &mut ground.form
    {
        let mut y = -22.0;
        while y <= 22.0 + 1e-9 {
            let mut x = -30.0;
            while x <= 30.0 + 1e-9 {
                points.push(DVec3::new(x, y, hill(DVec2::new(x, y))));
                x += 2.5;
            }
            y += 2.5;
        }
        *contour = 0.25;
        data.major = 4;
        // as a surveyor hands it: the ground carries on to the boundary
        // from the points nearest it
        data.survey = true;
    }
    let named = |n: &str, s: Site| Element::new(n, ElementKind::Site(s));
    let upright = |mut s: Site, edges: &[usize]| {
        if let SiteForm::Pad { retained, .. } = &mut s.form {
            retained.extend_from_slice(edges);
        }
        s
    };
    let mut tx = Transaction::new("cut test")
        .insert(lv.clone())
        .insert(named("Test hill", ground));
    // A: a pit, its south edge upright
    tx = tx.insert(named(
        "A pit, one upright",
        upright(
            Site::pit(
                g,
                sq(DVec2::new(-18.0, 8.0), DVec2::new(2.5, 1.8)),
                1.3,
                0.6,
            ),
            &[0],
        ),
    ));
    // B: a pit turned 30°, two edges side by side upright
    tx = tx.insert(named(
        "B pit turned, two upright",
        upright(
            Site::pit(
                g,
                turned(DVec2::new(-8.0, 9.0), DVec2::new(2.2, 1.6), 30.0),
                1.1,
                0.7,
            ),
            &[0, 1],
        ),
    ));
    // C: an L-shaped pad dug into the hill, an inside corner; its north
    // edge held by a retaining wall
    let l_shape = vec![
        DVec2::new(2.0, 2.0),
        DVec2::new(10.0, 2.0),
        DVec2::new(10.0, 6.0),
        DVec2::new(6.0, 6.0),
        DVec2::new(6.0, 10.0),
        DVec2::new(2.0, 10.0),
    ];
    let level_c = hill(DVec2::new(6.0, 6.0)) - 0.4;
    let mut pad_c = Site::pad(g, l_shape, level_c);
    let t = 0.25;
    let mut wall = creabim_core::Wall::new(
        g,
        DVec2::new(6.0, 10.0 + t / 2.0),
        DVec2::new(2.0, 10.0 + t / 2.0),
        t,
        2.4,
    );
    wall.base_offset = level_c;
    wall.location = creabim_core::LocationLine::InteriorFace;
    let wall = Element::new("C retaining wall", ElementKind::Wall(wall));
    if let SiteForm::Pad {
        retained,
        walls,
        cut,
        fill,
        ..
    } = &mut pad_c.form
    {
        retained.push(4);
        walls.push((4, wall.id));
        *cut = 1.0;
        *fill = 1.5;
    }
    tx = tx.insert(wall).insert(named("C L-shaped pad, wall", pad_c));
    // D: five sides, a 60° corner and a 135° one; half dug, half made up
    let a = DVec2::new(14.0, -14.0);
    let five = vec![
        a,
        DVec2::new(26.0, -14.0),
        DVec2::new(26.0, -9.0),
        DVec2::new(21.0, -5.0),
        a + DVec2::from_angle(60f64.to_radians()) * 10.0,
    ];
    let level_d = hill(DVec2::new(20.0, -9.5));
    let mut pad_d = Site::pad(g, five, level_d);
    if let SiteForm::Pad { cut, fill, .. } = &mut pad_d.form {
        *cut = 1.5;
        *fill = 2.0;
    }
    tx = tx.insert(named("D five-sided pad, cut and fill", pad_d));
    // E: a pit, every edge upright
    tx = tx.insert(named(
        "E pit, a box",
        upright(
            Site::pit(
                g,
                sq(DVec2::new(-16.0, -10.0), DVec2::new(1.8, 1.4)),
                1.0,
                0.5,
            ),
            &[0, 1, 2, 3],
        ),
    ));
    // F: a trench with two bends, upright sides
    tx = tx.insert(named(
        "F trench, two bends",
        Site::trench(
            g,
            vec![
                DVec2::new(-6.0, -16.0),
                DVec2::new(2.0, -13.0),
                DVec2::new(5.0, -6.0),
                DVec2::new(10.0, -5.0),
            ],
            0.8,
            1.0,
        ),
    ));
    // G: a pad cut into the knoll, its east and west edges upright, its
    // north edge between them banked
    let level_g = hill(DVec2::new(14.0, 14.0)) - 0.9;
    let mut pad_g = Site::pad(g, sq(DVec2::new(14.0, 15.0), DVec2::new(3.0, 2.0)), level_g);
    if let SiteForm::Pad { retained, cut, .. } = &mut pad_g.form {
        retained.extend_from_slice(&[1, 3]);
        *cut = 1.0;
    }
    tx = tx.insert(named("G pad, uprights either side", pad_g));
    // R: a road zig-zagging down past C's wall, each point at the ground
    let path = [
        DVec2::new(4.0, 21.0),
        DVec2::new(5.0, 15.0),
        DVec2::new(11.5, 9.0),
        DVec2::new(15.0, 9.5),
        DVec2::new(17.0, 1.0),
        DVec2::new(21.0, -4.0),
    ];
    let mut road = Site::road(g, path.to_vec(), 5.5);
    if let SiteForm::Road(r) = &mut road.form {
        for (c, q) in r.path.iter_mut().zip(&path) {
            c.z = hill(*q);
        }
        r.kerb = 0.1;
        // as an older file has it: 0.46 m on its middle
        r.kerb_radius = None;
        r.radius = 0.46;
    }
    tx = tx.insert(named("R road down the hill", road));
    for (i, at) in [
        DVec2::new(-4.0, 16.0),
        DVec2::new(14.0, 10.0),
        DVec2::new(24.0, 4.0),
    ]
    .iter()
    .enumerate()
    {
        let shape = creabim_core::tree::TreeShape {
            seed: i as u32 + 7,
            ..creabim_core::tree::TreePreset::Shade.shape().0
        };
        tx = tx.insert(named("Tree", Site::tree(g, *at, shape, 4.0, 6.0)));
    }
    // a lot over the middle of the hill, and a section across C's wall
    // and the road
    tx = tx.insert(named(
        "Property",
        Site::property(g, sq(DVec2::new(4.0, 2.0), DVec2::new(20.0, 16.0))),
    ));
    let mut section =
        creabim_core::View::section(DVec2::new(-2.0, 10.15), DVec2::new(26.0, 10.15), 20.0, true);
    section.scale = 100.0;
    tx = tx.insert(Element::new("Section A", ElementKind::View(section)));
    let mut doc = Document::from_model(creabim_core::Model::new());
    doc.commit(tx).unwrap();
    creabim_store::save(&out, &doc).unwrap();
    println!("saved {out}");
}
