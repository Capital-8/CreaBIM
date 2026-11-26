//! A stair stacked up through typical floors: a half-turn stair in a
//! walled core, drawn once on the ground floor with its railings and a
//! well, then repeated up to the roof. Every floor gets the same stair,
//! railings and well. The copy on the second floor is then given oak
//! treads, and every storey takes them. Last, the third floor's stair is
//! released and made steeper on its own.
//!
//! `cargo run -p creabim-store --example stack_demo -- stack.creabim`

use creabim_core::members::{self, RailExtend, StairFinish, StairLine};
use creabim_core::{
    stairstack, Document, Element, ElementKind, Hatch, Level, Material, Railing, Slab, Stair,
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
    let out = std::env::args().nth(1).unwrap_or("stack.creabim".into());
    let mut doc = Document::new();

    let names = [
        "Ground Floor",
        "First Floor",
        "Second Floor",
        "Third Floor",
        "Roof",
    ];
    let mut tx = Transaction::new("levels");
    let mut levels = vec![];
    for (k, n) in names.iter().enumerate() {
        let l = Element::new(
            *n,
            ElementKind::Level(Level {
                elevation: 3.0 * k as f64,
                height: 3.0,
            }),
        );
        levels.push(l.id);
        tx = tx.insert(l);
    }
    doc.commit(tx).unwrap();

    // the core's walls on every floor, and a floor over each
    let mut tx = Transaction::new("core");
    for (k, l) in levels.iter().enumerate() {
        if k + 1 < levels.len() {
            for (a, b) in [
                (v(0.0, -1.2), v(0.0, 2.35)),
                (v(0.0, 2.35), v(2.7, 2.35)),
                (v(2.7, 2.35), v(2.7, -1.2)),
            ] {
                tx = tx.insert(Element::new(
                    "Wall",
                    ElementKind::Wall(Wall::new(*l, a, b, 0.2, 3.0)),
                ));
            }
        }
        if k > 0 {
            let slab = Slab::new(*l, rect(v(-3.0, -4.0), v(6.0, 2.45)), 0.25);
            tx = tx.insert(Element::new(
                format!("{} slab", names[k]),
                ElementKind::Slab(slab),
            ));
        }
    }
    doc.commit(tx).unwrap();

    // the stair on the ground floor: up the west side, a half-landing,
    // back down the east
    let mut s = Stair::from_line(
        levels[0],
        &[v(0.6, -1.0), v(0.6, 1.75), v(2.1, 1.75), v(2.1, -1.0)],
        StairLine::Walk,
    )
    .unwrap();
    s.width = 1.0;
    s.tread = 0.28;
    s.max_riser = 0.18;
    s.trim(3.0);
    let e = Element::new("Core stair", ElementKind::Stair(s));
    let id = e.id;
    doc.commit(Transaction::new("stair").insert(e)).unwrap();
    let mut tx = Transaction::new("railings");
    for left in [true, false] {
        let model = doc.model();
        let ElementKind::Stair(s) = &model.get(id).unwrap().kind else {
            continue;
        };
        let mut r = Railing::along_stair(model, id, left).unwrap();
        r.on_wall = members::stair_side_on_wall(model, s, left);
        r.height = 0.9;
        r.extend = Some(RailExtend::default());
        r.rail_profile = Some("Handrail".into());
        if !r.on_wall {
            r.newels = true;
            r.infill = members::RailInfill::Balusters;
        }
        tx = tx.insert(Element::new("Railing", ElementKind::Railing(r)));
    }
    doc.commit(tx).unwrap();
    doc.commit(creabim_core::stairwell::cut_well(doc.model(), id, 0.0).unwrap())
        .unwrap();

    // up to the roof: a copy on every floor, with its railings and well
    doc.commit(stairstack::repeat_to(doc.model(), id, levels[4]).unwrap())
        .unwrap();
    let stack = stairstack::stack(doc.model(), id);
    // oak treads on the second floor's copy: every storey follows
    let oak = Element::new(
        "Oak",
        ElementKind::Material(Material::new(Hatch::Timber, [0.72, 0.52, 0.32])),
    );
    let oak_id = oak.id;
    doc.commit(Transaction::new("oak").insert(oak)).unwrap();
    doc.edit(stack[2], "Stair finish", |e| {
        if let ElementKind::Stair(s) = &mut e.kind {
            s.finish = StairFinish {
                tread: 0.025,
                tread_material: Some(oak_id),
                nosing: 0.02,
                ..Default::default()
            };
        }
    })
    .unwrap();
    // the third floor's released and steeper, its last flight longer
    doc.commit(stairstack::release(doc.model(), stack[3]).unwrap())
        .unwrap();
    doc.edit(stack[3], "Stair going", |e| {
        if let ElementKind::Stair(s) = &mut e.kind {
            s.tread = 0.26;
            s.max_riser = 0.19;
        }
    })
    .unwrap();

    let mut tx = Transaction::new("views");
    for (k, l) in levels.iter().enumerate().take(4) {
        let mut plan = View::plan(*l);
        plan.scale = 50.0;
        tx = tx.insert(Element::new(names[k], ElementKind::View(plan)));
    }
    // across the core, through both flights, every floor
    let mut sec = View::section(v(-1.0, 0.4), v(3.7, 0.4), 3.0, true);
    sec.scale = 50.0;
    tx = tx.insert(Element::new(
        "Section across the core",
        ElementKind::View(sec),
    ));
    // up the west flight on every floor
    let mut sec = View::section(v(0.6, -2.0), v(0.6, 3.2), 3.0, true);
    sec.scale = 50.0;
    tx = tx.insert(Element::new("Section up the core", ElementKind::View(sec)));
    let mut axo = View::axonometric(215.0, 30.0);
    axo.hidden
        .set_category(creabim_core::visibility::Category::Slabs, true);
    axo.hidden
        .set_category(creabim_core::visibility::Category::Walls, true);
    tx = tx.insert(Element::new("3D", ElementKind::View(axo)));
    tx = tx.insert(Element::new(
        "Stair Schedule",
        ElementKind::View(View::schedule(creabim_core::ScheduleCategory::Stairs)),
    ));
    doc.commit(tx).unwrap();
    let (tx, _) = creabim_geom::stairsheet::stair_sheet(doc.model(), id).unwrap();
    doc.commit(tx).unwrap();

    for s in stairstack::stack(doc.model(), id) {
        let e = doc.model().get(s).unwrap();
        let ElementKind::Stair(st) = &e.kind else {
            continue;
        };
        println!("{}: tread finish {}", e.name, st.finish.tread);
    }
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
