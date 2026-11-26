//! Stairs drawn by their lines, to try the stair system on: a straight
//! flight drawn by its left side along a wall, a half-turn stair in a
//! walled stair hall, an open stair with two quarter turns and an L on
//! winders, a stair curving round a quarter circle and a spiral, each with the railings the Stair tool gives it (a wall
//! handrail where a wall runs beside it, a guard on an open side). The
//! first floor has a well over each stair; there are sections along the
//! straight flight, through the open stair and up the winder stair.
//!
//! Behind them, a row of straight flights shows each way a stair can be
//! built: smooth and stepped concrete, a folded plate, side stringers,
//! cut stringers, a central spine, and treads cantilevered from a wall.
//! The curved stair is on side stringers and the spiral on a post.
//! Guards have balusters and newel posts; the open stair's are glass,
//! 1070 mm high, with a handrail at 900 on their posts.
//! The straight flight is finished in limestone, the half-turn stair in
//! oak treads with painted risers, and the winder stair in oak treads.
//!
//! `cargo run -p creabim-store --example stair_demo -- stairs.creabim`

use creabim_core::members::{self, RailExtend, StairFinish, StairLine};
use creabim_core::{
    Document, Element, ElementKind, Hatch, Level, Material, Railing, Shaft, Slab, Stair,
    Transaction, View, Wall,
};
use glam::DVec2;

fn v(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}

fn rect(a: DVec2, b: DVec2) -> Vec<DVec2> {
    vec![a, v(b.x, a.y), b, v(a.x, b.y)]
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or("stairs.creabim".into());
    let mut doc = Document::new();

    let mut tx = Transaction::new("levels");
    let ground = Element::new(
        "Ground Floor",
        ElementKind::Level(Level {
            elevation: 0.0,
            height: 3.0,
        }),
    );
    let first = Element::new(
        "First Floor",
        ElementKind::Level(Level {
            elevation: 3.0,
            height: 2.8,
        }),
    );
    let (g, f) = (ground.id, first.id);
    tx = tx.insert(ground).insert(first);
    doc.commit(tx).unwrap();

    // walls: one along the straight flight, three round the stair hall
    let mut tx = Transaction::new("walls");
    for (a, b) in [
        (v(0.0, 0.0), v(7.0, 0.0)),
        (v(9.0, -1.2), v(9.0, 2.35)),
        (v(9.0, 2.35), v(11.7, 2.35)),
        (v(11.7, 2.35), v(11.7, -1.2)),
        // the wall the cantilevered treads come out of
        (v(38.4, 6.2), v(38.4, 11.8)),
    ] {
        tx = tx.insert(Element::new(
            "Wall",
            ElementKind::Wall(Wall::new(g, a, b, 0.2, 3.0)),
        ));
    }
    doc.commit(tx).unwrap();

    // finishes: oak treads, painted risers, stone
    let mut tx = Transaction::new("materials");
    let mat = |name: &str, hatch: Hatch, color: [f32; 3]| {
        Element::new(name, ElementKind::Material(Material::new(hatch, color)))
    };
    let oak = mat("Oak", Hatch::Timber, [0.72, 0.52, 0.32]);
    let paint = mat("Painted softwood", Hatch::Diagonal, [0.95, 0.95, 0.93]);
    let stone = mat("Limestone", Hatch::Cross, [0.86, 0.82, 0.72]);
    let (oak, paint, stone) = {
        let ids = (oak.id, paint.id, stone.id);
        tx = tx.insert(oak).insert(paint).insert(stone);
        ids
    };
    doc.commit(tx).unwrap();

    // the stairs, by their lines
    // a spiral round (34, 0), its walk line 0.65 m out: three arcs that go
    // round further than its 17 risers need (trimmed below)
    let spiral: Vec<DVec2> = (0..=3)
        .map(|k| v(34.0, 0.0) + DVec2::from_angle(1.75 * k as f64).rotate(v(0.65, 0.0)))
        .collect();
    let mut lines: Vec<(&str, StairLine, Vec<DVec2>)> = vec![
        // its left side along the wall's face: 0.1 m below the centreline
        (
            "Straight stair",
            StairLine::Left,
            vec![v(0.6, -0.1), v(6.8, -0.1)],
        ),
        // up the hall's west side, a half-landing at the end, back down the east
        (
            "Half-turn stair",
            StairLine::Walk,
            vec![v(9.6, -1.0), v(9.6, 1.75), v(11.1, 1.75), v(11.1, -1.0)],
        ),
        // open, two quarter turns
        (
            "Open stair",
            StairLine::Walk,
            vec![v(14.0, -1.0), v(15.9, -1.0), v(15.9, 1.4), v(14.0, 1.4)],
        ),
        // an L turning on winders: three of 30° each
        (
            "Winder stair",
            StairLine::Walk,
            vec![v(19.6, -1.0), v(22.4, -1.0), v(22.4, 1.0)],
        ),
        // a quarter circle of radius 3 on its walk line, then straight on
        (
            "Curved stair",
            StairLine::Walk,
            vec![v(25.5, -1.0), v(28.5, 2.0), v(28.5, 3.5)],
        ),
        ("Spiral stair", StairLine::Walk, spiral.clone()),
        // south of the hall, outside: an entrance ramp up 750 mm, turning
        // on a landing
        (
            "Entrance ramp",
            StairLine::Walk,
            vec![v(1.0, -6.0), v(8.5, -6.0), v(8.5, -11.0)],
        ),
        // north of the hall, a split stair: one arm drawn (up the middle,
        // a landing, back down the west), its mirror image the east arm
        (
            "Split stair",
            StairLine::Walk,
            vec![v(20.0, 6.5), v(20.0, 9.25), v(18.5, 9.25), v(18.5, 6.5)],
        ),
    ];
    // east of them, the row of builds, each flight 2 m from the next,
    // going north
    use members::StairBuild as B;
    let row = [
        B::Concrete,
        B::Stepped,
        B::FoldedPlate,
        B::Stringers,
        B::CutStringers,
        B::Spine,
    ];
    for (k, b) in row.iter().enumerate() {
        let x = 41.0 + 2.0 * k as f64;
        lines.push((b.label(), StairLine::Walk, vec![v(x, 6.5), v(x, 11.5)]));
    }
    // at the west end, its left side on the wall's east face
    lines.push((
        B::Cantilever.label(),
        StairLine::Left,
        vec![v(38.5, 6.5), v(38.5, 11.5)],
    ));
    let mut tx = Transaction::new("stairs");
    let mut ids = vec![];
    for (name, drawn, pts) in &lines {
        let mut s = Stair::from_line(g, pts, *drawn).unwrap();
        s.width = 1.0;
        s.tread = 0.28;
        s.max_riser = 0.18;
        if *name == "Winder stair" {
            s.at_turns = members::TurnKind::Winders(0);
            s.finish = StairFinish {
                tread: 0.025,
                tread_material: Some(oak),
                nosing: 0.02,
                ..Default::default()
            };
        }
        // the hall stair in oak with painted risers, the straight flight
        // in stone
        if *name == "Half-turn stair" {
            s.finish = StairFinish {
                tread: 0.025,
                tread_material: Some(oak),
                riser: 0.018,
                riser_material: Some(paint),
                nosing: 0.02,
            };
        }
        if *name == "Straight stair" {
            s.finish = StairFinish {
                tread: 0.03,
                tread_material: Some(stone),
                riser: 0.02,
                riser_material: Some(stone),
                nosing: 0.0,
            };
        }
        if *name == "Curved stair" {
            s.set_bend(0, 90.0);
            s.build = B::Stringers;
        }
        if let Some(b) = B::ALL.into_iter().find(|b| b.label() == *name) {
            s.build = b;
        }
        if *name == "Spiral stair" {
            s.width = 0.9;
            s.tread = 0.2;
            s.line = pts.iter().map(|p| s.to_local(*p)).collect();
            for k in 0..3 {
                s.set_bend(k, 1.75f64.to_degrees());
            }
            s.build = B::Post;
        }
        if *name == "Split stair" {
            s.split = true;
        }
        if *name == "Entrance ramp" {
            s.width = 1.5;
            s.height = Some(0.75);
            s.make_ramp(1.0 / 12.0);
        }
        // trimmed at its last riser, as the tool leaves it
        s.trim(s.height.unwrap_or(3.0));
        let e = Element::new(*name, ElementKind::Stair(s));
        ids.push(e.id);
        tx = tx.insert(e);
    }
    doc.commit(tx).unwrap();

    // the railings the tool gives them
    let mut tx = Transaction::new("railings");
    for id in &ids {
        for left in [true, false] {
            let model = doc.model();
            let ElementKind::Stair(s) = &model.get(*id).unwrap().kind else {
                continue;
            };
            let mut r = Railing::along_stair(model, *id, left).unwrap();
            r.on_wall = members::stair_side_on_wall(model, s, left);
            r.height = 0.9;
            r.extend = Some(RailExtend::default());
            r.rail_profile = Some("Handrail".into());
            // guards as the Stair tool makes them: balusters and newels;
            // the open stair's in glass, taller than its handrail
            if !r.on_wall {
                r.newels = true;
                r.infill = members::RailInfill::Balusters;
                if model.get(*id).unwrap().name == "Open stair" {
                    r.infill = members::RailInfill::Glass;
                    r.guard_height = Some(1.07);
                }
            }
            tx = tx.insert(Element::new("Railing", ElementKind::Railing(r)));
        }
    }
    doc.commit(tx).unwrap();

    // the first floor, with a well over each stair
    let mut tx = Transaction::new("floor");
    let mut slab = Slab::new(f, rect(v(-0.5, -3.0), v(55.0, 12.5)), 0.25);
    slab.top_offset = 0.0;
    tx = tx.insert(Element::new("First floor", ElementKind::Slab(slab)));
    // the straight stair's well drawn by hand, too short: the floor comes
    // down over the head two thirds of the way up
    tx = tx.insert(Element::new(
        "Stair Well",
        ElementKind::Shaft(Shaft::new(g, rect(v(3.0, -1.1), v(5.1, -0.1)), Some(f))),
    ));
    doc.commit(tx).unwrap();
    // every other stair's well cut by Cut a well: tied to its stair, sized
    // from the headroom
    for id in &ids {
        let name = &doc.model().get(*id).unwrap().name;
        if name == "Straight stair" || name == "Entrance ramp" {
            continue;
        }
        let tx = creabim_core::stairwell::cut_well(doc.model(), *id, 0.0).unwrap();
        doc.commit(tx).unwrap();
    }

    let mut tx = Transaction::new("views");
    let mut plan = View::plan(g);
    plan.scale = 50.0;
    tx = tx.insert(Element::new("Ground Floor", ElementKind::View(plan)));
    tx = tx.insert(Element::new(
        "First Floor",
        ElementKind::View(View::plan(f)),
    ));
    let mut sec = View::section(v(-0.5, -0.6), v(8.0, -0.6), 3.0, true);
    sec.scale = 50.0;
    tx = tx.insert(Element::new(
        "Section along the straight stair",
        ElementKind::View(sec),
    ));
    // up the half-turn stair's first flight: oak treads, painted risers
    let mut sec = View::section(v(9.6, -1.5), v(9.6, 2.6), 3.0, true);
    sec.scale = 20.0;
    tx = tx.insert(Element::new(
        "Section up the hall stair",
        ElementKind::View(sec),
    ));
    // across the open stair's first landing: the soffits meet it flush
    let mut sec = View::section(v(13.4, -1.0), v(17.0, -1.0), 3.0, true);
    sec.scale = 50.0;
    tx = tx.insert(Element::new(
        "Section through the open stair",
        ElementKind::View(sec),
    ));
    // up the winder stair's first flight and through its winders
    let mut sec = View::section(v(19.0, -0.9), v(23.4, -0.9), 3.0, true);
    sec.scale = 50.0;
    tx = tx.insert(Element::new(
        "Section up the winder stair",
        ElementKind::View(sec),
    ));
    // up the middle of the curved stair, and across the spiral's centre
    let mut sec = View::section(v(24.8, 0.3), v(30.0, 0.3), 3.0, true);
    sec.scale = 50.0;
    tx = tx.insert(Element::new(
        "Section across the curved stair",
        ElementKind::View(sec),
    ));
    let mut sec = View::section(v(32.5, -0.05), v(35.5, -0.05), 3.0, true);
    sec.scale = 50.0;
    tx = tx.insert(Element::new(
        "Section through the spiral",
        ElementKind::View(sec),
    ));
    // across the split stair's three flights
    let mut sec = View::section(v(16.8, 7.6), v(24.2, 7.6), 3.0, true);
    sec.scale = 50.0;
    tx = tx.insert(Element::new(
        "Section across the split stair",
        ElementKind::View(sec),
    ));
    // across the row of builds, halfway up
    let mut sec = View::section(v(37.5, 9.0), v(52.5, 9.0), 3.0, true);
    sec.scale = 50.0;
    tx = tx.insert(Element::new(
        "Section across the ways of building",
        ElementKind::View(sec),
    ));
    // each stair unrolled along its walk line, with its headroom: the
    // straight stair's short well shows red
    for name in [
        "Straight stair",
        "Half-turn stair",
        "Winder stair",
        "Spiral stair",
        "Entrance ramp",
    ] {
        let id = ids
            .iter()
            .find(|id| doc.model().get(**id).unwrap().name == name)
            .copied()
            .unwrap();
        tx = tx.insert(Element::new(
            format!("{name} walk-line section"),
            ElementKind::View(View::walk_section(id)),
        ));
    }
    // the take-off: concrete, boards and the finishes, by material
    tx = tx.insert(Element::new(
        "Take-off",
        ElementKind::View(View::schedule(creabim_core::ScheduleCategory::Materials)),
    ));
    // the floor above left out, to see the stairs and their railings
    let mut axo = View::axonometric(205.0, 38.0);
    axo.hidden
        .set_category(creabim_core::visibility::Category::Slabs, true);
    tx = tx.insert(Element::new("3D", ElementKind::View(axo)));
    // every stair with its rise, going, headroom and checks
    tx = tx.insert(Element::new(
        "Stair Schedule",
        ElementKind::View(View::schedule(creabim_core::ScheduleCategory::Stairs)),
    ));
    doc.commit(tx).unwrap();
    // a sheet for the half-turn and the straight stair: both plans, the
    // walk-line section, a 3D view cut out round it and the schedule
    for name in [
        "Half-turn stair",
        "Straight stair",
        "Entrance ramp",
        "Split stair",
    ] {
        let id = ids
            .iter()
            .find(|id| doc.model().get(**id).unwrap().name == name)
            .copied()
            .unwrap();
        let (tx, _) = creabim_geom::stairsheet::stair_sheet(doc.model(), id).unwrap();
        doc.commit(tx).unwrap();
    }

    // what each stair fails of its rule set
    for (id, bad) in creabim_core::stairrules::failing_stairs(doc.model()) {
        let name = &doc.model().get(id).unwrap().name;
        let what: Vec<String> = bad
            .iter()
            .map(|c| format!("{} {} ({})", c.what, c.value, c.limit))
            .collect();
        println!("{name}: {}", what.join("; "));
    }
    creabim_store::save(&out, &doc).unwrap();
    println!("saved {out}");
}
