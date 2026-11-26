//! The structure: the 12 ft grid, footings and stem walls to frost depth,
//! the framed walls, the floors, the glulam line on grid 2 with its posts,
//! the structural ridge, the roofs, and the terrace canopy and porch
//! standing on piers.

use super::*;
use crate::footing::{self, FootingKind};
use crate::*;
use glam::DVec2;

/// The slabs reach the outer face of the framing.
pub const S: f64 = H - FRAME_OUT;
/// The wing roof's underside above the ground floor (9'-6").
pub const WING_SOFFIT: f64 = 9.5 * FT;
/// The canopy's underside above the ground floor (8'-6").
pub const CANOPY: f64 = 8.5 * FT;
/// The terrace and the porch, a 2 in step below the floor.
pub const TERRACE: f64 = -0.05;
/// The main roof's pitch: 8 in 12.
pub const PITCH: f64 = 33.690_067_5;

pub fn build(b: &mut Builder) {
    grids(b);
    foundations(b);
    roofs(b);
    walls(b);
    floors(b);
    frame(b);
    footings(b);
}

fn grids(b: &mut Builder) {
    let mut tx = Transaction::new("Ridgeline House: grids");
    let mut lines = vec![];
    for (i, label) in ["A", "B", "C", "D", "E"].into_iter().enumerate() {
        let x = 12.0 * i as f64;
        lines.push((label, p(x, -12.0), p(x, 30.0)));
    }
    for (i, label) in ["1", "2", "3"].into_iter().enumerate() {
        let y = 12.0 * i as f64;
        lines.push((label, p(-6.0, y), p(54.0, y)));
    }
    for (label, a, bb) in lines {
        tx = tx.insert(Element::new(
            format!("Grid {label}"),
            ElementKind::Grid(Grid {
                label: label.into(),
                start: a,
                end: bb,
                group: None,
                ..Grid::default()
            }),
        ));
    }
    b.commit(tx);
}

/// The lines the outside walls stand on, anticlockwise so the outside is
/// on their right: (key, from, to, on the wing).
pub fn perimeter() -> Vec<(&'static str, DVec2, DVec2, bool)> {
    vec![
        ("South", p(0.0, 0.0), p(36.0, 0.0), false),
        ("East", p(36.0, 0.0), p(36.0, 24.0), false),
        ("North", p(36.0, 24.0), p(0.0, 24.0), false),
        ("West", p(0.0, 24.0), p(0.0, 0.0), false),
        ("Wing South", p(36.0, 6.0), p(48.0, 6.0), true),
        ("Wing East", p(48.0, 6.0), p(48.0, 24.0), true),
        ("Wing North", p(48.0, 24.0), p(36.0, 24.0), true),
    ]
}

/// Stem walls from the footings up to the ground floor.
fn foundations(b: &mut Builder) {
    let (t, g) = (b.id("T.O. Footing"), b.id("Ground Floor"));
    let mut tx = Transaction::new("Ridgeline House: stem walls");
    for (key, a, bb, _) in perimeter() {
        let mut w = Wall::new(t, a, bb, 0.254, ft(4.0));
        w.top = WallTop::UpToLevel {
            level: g,
            offset: 0.0,
        };
        let key = format!("FDN {key}");
        let mut e = b.typed(
            Element::new(key.clone(), ElementKind::Wall(w)),
            "Foundation wall 10 in, insulated",
        );
        if let ElementKind::Wall(w) = &mut e.kind {
            w.height = ft(4.0);
        }
        tx = b.keep(&key, e, tx);
    }
    b.commit(tx);
}

fn roofs(b: &mut Builder) {
    let f = b.id("First Floor");
    let mut tx = Transaction::new("Ridgeline House: roofs");
    // the gable, its ridge along the house, eaves 2'-6" deep to shade the
    // south glass in summer
    let mut main = Roof::new(
        f,
        crate::polygon::rectangle(DVec2::splat(-H), p(36.0, 24.0) + DVec2::splat(H)),
        RoofShape::Gable,
    );
    main.pitch = PITCH;
    main.overhang = ft(2.5);
    main.base_offset = PLATE;
    // layers from the top: metal, underlayment, sheathing, vent, cellulose
    // (the I-joist rafters, whose tails run on to the fascia), polyiso,
    // vapour control and gypsum board, which stop at the walls' inner face
    for edge in 0..4 {
        for layer in [5, 6, 7] {
            crate::set_layer_edge(&mut main.layer_edges, layer, edge, -(ft(2.5) + EXT));
        }
    }
    let e = b.typed(
        Element::new("Main roof", ElementKind::Roof(main)),
        "Pitched roof R-60, standing seam",
    );
    tx = b.keep("Main roof", e, tx);
    // the wing's flat roof runs on south over the porch
    let x0 = ft(36.0) + H;
    let mut wing = Roof::new(
        f,
        crate::polygon::rectangle(DVec2::new(x0, ft(-1.0)), p(49.0, 25.0)),
        RoofShape::Flat,
    );
    wing.overhang = 0.0;
    wing.base_offset = WING_SOFFIT - STOREY;
    let e = b.typed(
        Element::new("Wing roof", ElementKind::Roof(wing)),
        "Flat roof R-60, membrane",
    );
    tx = b.keep("Wing roof", e, tx);
    // the terrace canopy: the brise-soleil over the living room's glass
    let mut canopy = Roof::new(
        f,
        crate::polygon::rectangle(p(-1.0, -7.0), DVec2::new(ft(25.0), -H)),
        RoofShape::Flat,
    );
    canopy.overhang = 0.0;
    canopy.base_offset = CANOPY - STOREY;
    let e = b.typed(
        Element::new("Terrace canopy", ElementKind::Roof(canopy)),
        "Canopy 4 in, standing seam",
    );
    tx = b.keep("Terrace canopy", e, tx);
    b.commit(tx);
}

/// The outside walls of both storeys.
fn walls(b: &mut Builder) {
    let (g, f) = (b.id("Ground Floor"), b.id("First Floor"));
    let (main_roof, wing_roof) = (b.id("Main roof"), b.id("Wing roof"));
    let mut tx = Transaction::new("Ridgeline House: outside walls");
    for (key, a, bb, wing) in perimeter() {
        let kind = if wing {
            "Exterior wall R-31, standing seam"
        } else {
            "Exterior wall R-31, cedar rainscreen"
        };
        let mut w = Wall::new(g, a, bb, EXT, STOREY);
        if wing {
            w.attach_top = vec![wing_roof];
        } else {
            w.top = WallTop::UpToLevel {
                level: f,
                offset: 0.0,
            };
        }
        let name = format!("GF {key}");
        let mut e = b.typed(Element::new(name.clone(), ElementKind::Wall(w)), kind);
        if let ElementKind::Wall(w) = &mut e.kind {
            w.height = STOREY;
        }
        tx = b.keep(&name, e, tx);
        if wing {
            continue;
        }
        // upstairs: the gables fill in up to the roof
        let mut w = Wall::new(f, a, bb, EXT, PLATE);
        w.attach_top = vec![main_roof];
        let name = format!("FF {key}");
        let mut e = b.typed(Element::new(name.clone(), ElementKind::Wall(w)), kind);
        if let ElementKind::Wall(w) = &mut e.kind {
            w.height = PLATE;
        }
        tx = b.keep(&name, e, tx);
    }
    b.commit(tx);
}

fn floors(b: &mut Builder) {
    let (g, f) = (b.id("Ground Floor"), b.id("First Floor"));
    let mut tx = Transaction::new("Ridgeline House: floors");
    let o = DVec2::splat(S);
    let gf = Slab::new(
        g,
        vec![
            DVec2::new(-S, -S),
            p(36.0, 0.0) + DVec2::new(S, -S),
            p(36.0, 6.0) + DVec2::new(S, -S),
            p(48.0, 6.0) + DVec2::new(S, -S),
            p(48.0, 24.0) + o,
            p(0.0, 24.0) + DVec2::new(-S, S),
        ],
        0.376,
    );
    let e = b.typed(
        Element::new("Ground floor slab", ElementKind::Slab(gf)),
        "Slab on grade R-20",
    );
    tx = b.keep("Ground floor slab", e, tx);
    let mut ff = Slab::new(f, crate::polygon::rectangle(-o, p(36.0, 24.0) + o), FLOOR);
    // the service chase: every stack and duct in one heated riser
    ff.holes = vec![chase()];
    let e = b.typed(
        Element::new("First floor", ElementKind::Slab(ff)),
        "Upper floor, I-joist",
    );
    tx = b.keep("First floor", e, tx);
    // the porch: concrete, a step down, under the wing's roof
    let mut porch = Slab::new(
        g,
        crate::polygon::rectangle(
            DVec2::new(ft(36.0) + H, ft(-0.5)),
            DVec2::new(ft(48.5), ft(6.0) - H),
        ),
        0.127,
    );
    porch.top_offset = TERRACE;
    let e = b.typed(
        Element::new("Porch", ElementKind::Slab(porch)),
        "Porch slab 5 in, concrete",
    );
    tx = b.keep("Porch", e, tx);
    // the terrace: a hardwood deck off the living room, under the canopy
    let mut deck = Slab::new(
        g,
        crate::polygon::rectangle(p(-1.0, -12.0), DVec2::new(ft(36.0) + H, -H)),
        0.05,
    );
    deck.top_offset = TERRACE;
    let e = b.typed(
        Element::new("Terrace", ElementKind::Slab(deck)),
        "Deck 2 in, hardwood",
    );
    tx = b.keep("Terrace", e, tx);
    b.commit(tx);

    // the first floor bears on the walls under it
    let mut tx = Transaction::new("Ridgeline House: junctions");
    for wall in ["GF South", "GF East", "GF North", "GF West"] {
        let mut rule = JoinRule::new(b.id("First floor"), b.id(wall));
        rule.winner = JoinWinner::First;
        tx = tx.insert(Element::new("Junction", ElementKind::Join(rule)));
    }
    b.commit(tx);
}

/// The service chase's outline: on the kitchen's east wall, and upstairs
/// in the wet wall between the en-suite and the bathroom.
pub fn chase() -> Vec<DVec2> {
    rect(34.2, 15.7, 35.5, 17.3)
}

/// Posts, beams and piers.
fn frame(b: &mut Builder) {
    let (t, g, f) = (
        b.id("T.O. Footing"),
        b.id("Ground Floor"),
        b.id("First Floor"),
    );
    let mut tx = Transaction::new("Ridgeline House: frame");
    let post = |b: &Builder, key: &str, level, at: DVec2, base: f64, height: f64, kind: &str| {
        let size = if kind.starts_with("Concrete") {
            Profile::Round { diameter: 0.305 }
        } else {
            Profile::Rectangle {
                width: 0.14,
                depth: 0.14,
            }
        };
        let mut c = Column::new(level, at, size);
        c.base_offset = base;
        c.height = Some(height);
        let mut e = b.typed(Element::new(key, ElementKind::Column(c)), kind);
        if let ElementKind::Column(c) = &mut e.kind {
            c.height = Some(height);
            c.base_offset = base;
        }
        e
    };
    let beam = |b: &Builder, key: &str, level, a: DVec2, bb: DVec2, down: f64, kind: &str| {
        let mut bm = Beam::new(level, a, bb);
        bm.top_offset = down;
        b.typed(Element::new(key, ElementKind::Beam(bm)), kind)
    };
    // grid 2: a glulam under the first floor on two fir posts
    let under = STOREY - FLOOR - 0.381;
    for (key, x) in [("Post B2", 12.0), ("Post C2", 24.0)] {
        let e = post(b, key, g, p(x, 12.0), 0.0, under, "Timber post 6×6");
        tx = b.keep(key, e, tx);
    }
    let e = beam(
        b,
        "Floor beam, grid 2",
        g,
        DVec2::new(H, ft(12.0)),
        DVec2::new(ft(36.0) - H, ft(12.0)),
        FLOOR,
        "Glulam beam 5 1/2\" × 15\"",
    );
    tx = b.keep("Floor beam, grid 2", e, tx);
    // the ridge: a glulam from gable to gable, under the rafters
    let rise = (ft(12.0) + H) * PITCH.to_radians().tan();
    let e = beam(
        b,
        "Ridge beam",
        f,
        DVec2::new(H, ft(12.0)),
        DVec2::new(ft(36.0) - H, ft(12.0)),
        -(rise - 0.06),
        "Glulam ridge 5 1/2\" × 18\"",
    );
    tx = b.keep("Ridge beam", e, tx);

    // the terrace canopy: three posts on piers, a beam along them and a
    // rafter beam back to the wall at each
    let post_top = CANOPY - 0.235;
    for (i, x) in [0.0, 12.0, 24.0].into_iter().enumerate() {
        let at = p(x, -6.0);
        let key = format!("Canopy post {}", i + 1);
        let e = post(
            b,
            &key,
            g,
            at,
            TERRACE,
            post_top - TERRACE,
            "Timber post 6×6",
        );
        tx = b.keep(&key, e, tx);
        let key = format!("Canopy pier {}", i + 1);
        let e = post(
            b,
            &key,
            t,
            at,
            0.0,
            ft(4.0) + TERRACE - 0.05,
            "Concrete pier 12\" dia.",
        );
        tx = b.keep(&key, e, tx);
        let key = format!("Canopy rafter beam {}", i + 1);
        let e = beam(
            b,
            &key,
            g,
            at + DVec2::new(0.0, 0.07),
            DVec2::new(at.x, -H),
            STOREY - CANOPY,
            "Timber beam 4×10",
        );
        tx = b.keep(&key, e, tx);
    }
    let e = beam(
        b,
        "Canopy beam",
        g,
        p(-1.0, -6.0),
        p(25.0, -6.0),
        STOREY - CANOPY,
        "Timber beam 4×10",
    );
    tx = b.keep("Canopy beam", e, tx);

    // the porch: one post at its corner, a beam under the roof's edge
    let corner = p(48.0, 0.0);
    let top = WING_SOFFIT - 0.235;
    let e = post(
        b,
        "Porch post",
        g,
        corner,
        TERRACE,
        top - TERRACE,
        "Timber post 6×6",
    );
    tx = b.keep("Porch post", e, tx);
    let e = post(
        b,
        "Porch pier",
        t,
        corner,
        0.0,
        ft(4.0) + TERRACE - 0.127,
        "Concrete pier 12\" dia.",
    );
    tx = b.keep("Porch pier", e, tx);
    let e = beam(
        b,
        "Porch beam",
        g,
        DVec2::new(ft(36.0) + H, 0.0),
        p(48.5, 0.0),
        STOREY - WING_SOFFIT,
        "Timber beam 4×10",
    );
    tx = b.keep("Porch beam", e, tx);
    b.commit(tx);
}

/// Strips under the stem walls and pads under the posts and piers.
fn footings(b: &mut Builder) {
    let model = b.doc.model().clone();
    let t = b.id("T.O. Footing");
    let hosts: Vec<ElementId> = model
        .iter()
        .filter(|e| match &e.kind {
            ElementKind::Wall(w) => w.level == t,
            // the piers, and the two posts that stand on the slab
            ElementKind::Column(c) => c.level == t || e.name.starts_with("Post "),
            _ => false,
        })
        .map(|e| e.id)
        .collect();
    let strip = b.id("Strip footing 20\" × 10\"");
    let big = b.id("Pad footing 30\" × 30\" × 12\"");
    let small = b.id("Pad footing 24\" × 24\" × 10\"");
    let (mut tx, _) = footing::under(&model, &hosts, &|e| {
        let (kind, host) = match &e.kind {
            ElementKind::Footing(f) => (f.kind, f.host),
            _ => return,
        };
        let on_pier = host
            .and_then(|h| model.get(h))
            .is_some_and(|h| matches!(&h.kind, ElementKind::Column(c) if c.level == t));
        let ty = match kind {
            FootingKind::Strip => strip,
            FootingKind::Pad if on_pier => small,
            FootingKind::Pad => big,
        };
        types::stamp(&model, e, ty);
        if let ElementKind::Footing(f) = &mut e.kind {
            // the interior posts' pads sit under the slab
            f.below = if kind == FootingKind::Pad && !on_pier {
                0.376
            } else {
                0.0
            };
        }
    });
    tx.label = "Ridgeline House: footings".into();
    b.commit(tx);
}
