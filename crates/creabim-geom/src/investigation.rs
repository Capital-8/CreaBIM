//! The soils report's boreholes, trial pits and probes as drawings: their
//! symbols on plan, their logs as columns of soil in 3D, the borehole log
//! a sheet carries, and the symbol each soil family is hatched with.
//! Nothing here interprets the ground between them; that is the ground
//! model's work.

use crate::plan::{Pen, PlanItem};
use crate::TriMesh;
use creabim_core::investigation::{Probe, ProbeKind, SoilKind, TestKind};
use creabim_core::pattern::{DrawnStroke, DrawnTile, Job, Recipe, Repeat};
use creabim_core::site::{Site, SiteForm};
use creabim_core::styles::{HatchPattern, PatternUnits};
use creabim_core::{members, ElementId, ElementKind, Model};
use glam::DVec2;

/// The size of an investigation point's symbol across, in metres on the
/// ground: 2.4 mm at 1:200.
const SYMBOL: f64 = 0.6;
/// A log's column in 3D, square across.
const COLUMN: f64 = 0.3;
/// Water in logs and 3D.
pub const WATER: [f32; 3] = [0.16, 0.50, 0.75];
/// Notes in grey.
const MUTED: [f32; 3] = [0.45, 0.48, 0.46];

/// The ground level of an investigation point: the report's, else the
/// existing ground's under it, else its level's elevation.
pub fn collar(model: &Model, s: &Site, p: &Probe) -> f64 {
    if let Some(g) = p.ground {
        return g;
    }
    crate::services::grounds_at(model, p.position)
        .map(|g| g.0)
        .unwrap_or_else(|| members::elevation(model, s.level))
}

/// The existing ground's level at an investigation point, if a terrain is
/// under it: to compare with the level the report gives.
pub fn survey_level(model: &Model, p: &Probe) -> Option<f64> {
    crate::services::grounds_at(model, p.position).map(|g| g.0)
}

/// The symbol a soil family is hatched with, the way soils reports draw
/// them, in paper millimetres so it reads the same at any scale.
pub fn soil_pattern(kind: SoilKind) -> HatchPattern {
    let line = |pts: &[[f64; 2]]| DrawnStroke {
        points: pts.to_vec(),
        closed: false,
        job: Job::Line,
        depth: 0.0,
        name: String::new(),
    };
    let shape = |pts: &[[f64; 2]]| DrawnStroke {
        closed: true,
        ..line(pts)
    };
    // a dot: a speck of line, drawn as a dot
    let dot = |x: f64, y: f64| line(&[[x, y], [x + 0.12, y]]);
    let ring = |cx: f64, cy: f64, r: f64| {
        let pts: Vec<[f64; 2]> = (0..10)
            .map(|k| {
                let a = k as f64 * std::f64::consts::TAU / 10.0;
                [cx + r * a.cos(), cy + r * a.sin()]
            })
            .collect();
        shape(&pts)
    };
    let (size, repeat, strokes) = match kind {
        SoilKind::Topsoil => (
            [3.0, 3.0],
            Repeat::Offset,
            vec![
                line(&[[0.6, 0.5], [0.6, 1.4]]),
                line(&[[1.1, 0.5], [1.1, 1.2]]),
            ],
        ),
        SoilKind::MadeGround => (
            [4.0, 4.0],
            Repeat::Offset,
            vec![
                shape(&[[0.4, 0.5], [1.6, 0.5], [1.0, 1.6]]),
                shape(&[[2.4, 2.3], [3.3, 2.3], [3.3, 3.2], [2.4, 3.2]]),
                dot(3.2, 0.9),
            ],
        ),
        SoilKind::Clay => (
            [3.0, 1.5],
            Repeat::Offset,
            vec![line(&[[0.3, 0.75], [2.0, 0.75]])],
        ),
        SoilKind::Silt => (
            [3.0, 1.5],
            Repeat::Offset,
            vec![line(&[[0.2, 0.75], [1.4, 0.75]]), dot(2.2, 0.75)],
        ),
        SoilKind::Sand => (
            [2.0, 2.0],
            Repeat::Offset,
            vec![dot(0.4, 0.5), dot(1.4, 1.5)],
        ),
        SoilKind::Gravel => (
            [3.0, 3.0],
            Repeat::Offset,
            vec![ring(0.9, 0.9, 0.5), ring(2.2, 2.3, 0.35)],
        ),
        SoilKind::Peat => (
            [3.0, 2.0],
            Repeat::Offset,
            vec![line(&[[0.5, 1.4], [1.0, 0.6], [1.5, 1.4]])],
        ),
        SoilKind::Rock => (
            [4.0, 2.0],
            Repeat::Offset,
            vec![
                line(&[[0.0, 0.0], [4.0, 0.0]]),
                line(&[[0.0, 0.0], [0.0, 2.0]]),
            ],
        ),
    };
    HatchPattern {
        units: PatternUnits::Paper,
        ..HatchPattern::from_recipe(
            kind.label(),
            Recipe::Drawn(DrawnTile {
                size,
                repeat,
                strokes,
                dims: vec![],
            }),
        )
    }
}

/// `poly` washed in its soil's colour and hatched with its symbol, in a
/// drawing of `k` paper millimetres per drawing unit.
pub fn soil_fill(out: &mut Vec<PlanItem>, id: ElementId, poly: &[DVec2], kind: SoilKind, k: f64) {
    out.push(PlanItem::Tint {
        id,
        points: poly.to_vec(),
        rgb: kind.color(),
    });
    for (a, b) in crate::looks::pattern_lines(&soil_pattern(kind), poly, k) {
        out.push(PlanItem::Line {
            id,
            pen: Pen::Fine,
            a,
            b,
        });
    }
}

fn line(out: &mut Vec<PlanItem>, id: ElementId, pen: Pen, a: DVec2, b: DVec2) {
    out.push(PlanItem::Line { id, pen, a, b });
}

fn ring(out: &mut Vec<PlanItem>, id: ElementId, pen: Pen, pts: &[DVec2]) {
    for i in 0..pts.len() {
        line(out, id, pen, pts[i], pts[(i + 1) % pts.len()]);
    }
}

fn circle(c: DVec2, r: f64, from: f64, to: f64, n: usize) -> Vec<DVec2> {
    (0..=n)
        .map(|k| c + DVec2::from_angle(from + (to - from) * k as f64 / n as f64) * r)
        .collect()
}

/// An investigation point on plan: the usual symbol (a circle with two
/// quarters filled for a borehole, a square crossed for a trial pit, a
/// triangle for a probe), its name, and its ground level and depth.
pub fn probe_plan(
    model: &Model,
    out: &mut Vec<PlanItem>,
    id: ElementId,
    s: &Site,
    p: &Probe,
    levels: bool,
) {
    let c = p.position;
    let r = SYMBOL / 2.0;
    let ink = [0.1, 0.1, 0.1];
    match p.kind {
        ProbeKind::Borehole => {
            let rim = circle(c, r, 0.0, std::f64::consts::TAU, 24);
            out.push(PlanItem::Tint {
                id,
                points: rim.clone(),
                rgb: [1.0, 1.0, 1.0],
            });
            // the north-east and south-west quarters filled
            for q in [0.0, std::f64::consts::PI] {
                let mut wedge = vec![c];
                wedge.extend(circle(c, r, q, q + std::f64::consts::FRAC_PI_2, 8));
                out.push(PlanItem::Tint {
                    id,
                    points: wedge,
                    rgb: ink,
                });
            }
            for w in rim.windows(2) {
                line(out, id, Pen::Symbol, w[0], w[1]);
            }
        }
        ProbeKind::TrialPit => {
            let sq: Vec<DVec2> = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
                .iter()
                .map(|(x, y)| c + DVec2::new(*x, *y) * r * 0.9)
                .collect();
            out.push(PlanItem::Tint {
                id,
                points: sq.clone(),
                rgb: [1.0, 1.0, 1.0],
            });
            ring(out, id, Pen::Symbol, &sq);
            line(out, id, Pen::Symbol, sq[0], sq[2]);
        }
        ProbeKind::Probe => {
            let tri: Vec<DVec2> = (0..3)
                .map(|k| {
                    c + DVec2::from_angle(
                        std::f64::consts::FRAC_PI_2 + k as f64 * std::f64::consts::TAU / 3.0,
                    ) * r
                })
                .collect();
            out.push(PlanItem::Tint {
                id,
                points: tri.clone(),
                rgb: [1.0, 1.0, 1.0],
            });
            ring(out, id, Pen::Symbol, &tri);
        }
    }
    let name = model.get(id).map(|e| e.name.clone()).unwrap_or_default();
    let text = |at: DVec2, text: String, bold: bool| PlanItem::Text {
        angle: 0.0,
        id,
        at,
        text,
        size: crate::parts::GROUND_TEXT_MM,
        align: crate::paper::Anchor::Start,
        mm: 0.1,
        rgb: None,
        bold,
    };
    // one line beside the symbol, so it reads at any scale: its name,
    // then with levels on its ground level and how deep it went
    let right = c + DVec2::new(r * 1.7, 0.0);
    if levels {
        let g = collar(model, s, p);
        out.push(text(
            right,
            format!(
                "{name}  {} · to {}",
                creabim_core::site_level(model, g),
                depth_text(model, p.depth)
            ),
            false,
        ));
    } else {
        out.push(text(right, name, true));
    }
}

/// A depth as logs write it: metres to two places.
fn depth_text(model: &Model, d: f64) -> String {
    if model.units().is_imperial() {
        model.units().length(d)
    } else {
        format!("{d:.2} m")
    }
}

/// An investigation point in 3D: a slim column of its layers, each in
/// its soil's colour, from its ground level down; undescribed depth grey.
pub fn probe_meshes(model: &Model, s: &Site, p: &Probe) -> Vec<(TriMesh, [f32; 3], bool)> {
    let top = collar(model, s, p);
    let h = COLUMN / 2.0;
    let square = [
        p.position + DVec2::new(-h, -h),
        p.position + DVec2::new(h, -h),
        p.position + DVec2::new(h, h),
        p.position + DVec2::new(-h, h),
    ];
    let mut out = vec![];
    let mut down = 0.0;
    for (from, l) in p.spans() {
        let to = l.to.min(p.depth);
        if to > from {
            out.push((
                crate::scene::slab_mesh(&square, top - from, to - from),
                l.soil.color(),
                false,
            ));
        }
        down = to;
    }
    if p.depth > down + 1e-6 {
        out.push((
            crate::scene::slab_mesh(&square, top - down, p.depth - down),
            [0.62, 0.62, 0.60],
            false,
        ));
    }
    // the water struck, as a thin blue disc round the column
    if let Some((struck, rest)) = p.water_level() {
        let z = top - rest.unwrap_or(struck);
        let w = h * 2.2;
        let disc = [
            p.position + DVec2::new(-w, -w),
            p.position + DVec2::new(w, -w),
            p.position + DVec2::new(w, w),
            p.position + DVec2::new(-w, w),
        ];
        out.push((crate::scene::slab_mesh(&disc, z + 0.01, 0.02), WATER, false));
    }
    out
}

/// The investigation point a view's log is of: its element, site and
/// probe.
fn probe_of(model: &Model, id: ElementId) -> Option<(&creabim_core::Element, &Site, &Probe)> {
    let e = model.get(id)?;
    let ElementKind::Site(s) = &e.kind else {
        return None;
    };
    let SiteForm::Investigation(p) = &s.form else {
        return None;
    };
    Some((e, s, p))
}

/// Break `text` into lines of at most `n` characters, at spaces.
fn wrap(text: &str, n: usize) -> Vec<String> {
    let mut lines = vec![];
    let mut cur = String::new();
    for w in text.split_whitespace() {
        if !cur.is_empty() && cur.chars().count() + 1 + w.chars().count() > n {
            lines.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(w);
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

/// The paper millimetres a log gives each metre of depth: deep holes
/// squeezed so a log fits a sheet.
pub fn log_mm_per_m(depth: f64) -> f64 {
    if depth <= 6.0 {
        25.0
    } else if depth <= 12.0 {
        15.0
    } else if depth <= 25.0 {
        8.0
    } else {
        200.0 / depth.max(1.0)
    }
}

/// The borehole log of investigation point `id`, as a drawing of `scale`
/// (1:`scale`) laid out in paper millimetres: a heading with its
/// position, ground level, method and report; then columns of depth,
/// level, the soil symbols, each layer's description, the water struck,
/// and the tests with a graph of their values.
pub fn log_drawing(model: &Model, id: ElementId, scale: f64) -> crate::section::Drawing {
    let mut items = vec![];
    let Some((e, s, p)) = probe_of(model, id) else {
        return crate::section::Drawing { items };
    };
    let k = 1000.0 / scale; // paper mm per drawing unit
    let mm = 1.0 / k; // drawing units per paper mm
    let at = |x: f64, y: f64| DVec2::new(x, y) * mm;
    let top = collar(model, s, p);
    let per = log_mm_per_m(p.depth);
    // columns, paper mm from the left
    let (c_dep, c_lev, c_leg, c_des, c_wat, c_tst, c_gra, c_end) =
        (0.0, 14.0, 30.0, 46.0, 128.0, 140.0, 158.0, 196.0);
    let head = 26.0; // the heading's height
    let cols = 7.0; // the column titles' height
    let y0 = -(head + cols); // depth 0
    let yd = |d: f64| y0 - d * per;
    let bottom = yd(p.depth);
    let text = |at: DVec2, t: String, size: f64, align, bold: bool, rgb: Option<[f32; 3]>| {
        PlanItem::Text {
            angle: 0.0,
            id,
            at,
            text: t,
            size,
            align,
            mm,
            rgb,
            bold,
        }
    };
    use crate::paper::Anchor::{End, Middle, Start};
    // frame and rules
    let frame = [
        at(0.0, 0.0),
        at(c_end, 0.0),
        at(c_end, bottom - 14.0),
        at(0.0, bottom - 14.0),
    ];
    ring(&mut items, id, Pen::Cut, &frame);
    line(&mut items, id, Pen::Cut, at(0.0, -head), at(c_end, -head));
    line(&mut items, id, Pen::Projection, at(0.0, y0), at(c_end, y0));
    line(&mut items, id, Pen::Cut, at(0.0, bottom), at(c_end, bottom));
    for x in [c_lev, c_leg, c_des, c_wat, c_tst, c_gra] {
        line(&mut items, id, Pen::Fine, at(x, -head), at(x, bottom));
    }
    // heading
    items.push(text(at(4.0, -9.0), e.name.clone(), 5.0, Start, true, None));
    items.push(text(
        at(4.0, -15.0),
        format!("{} log", p.kind.label()),
        2.2,
        Start,
        false,
        Some(MUTED),
    ));
    let pos = if model.units().is_imperial() {
        format!(
            "E {}  N {}",
            model.units().length(p.position.x),
            model.units().length(p.position.y)
        )
    } else {
        format!("E {:.2}  N {:.2}", p.position.x, p.position.y)
    };
    let mut ground = creabim_core::site_level(model, top);
    if p.ground.is_none() {
        ground.push_str(" (from the survey)");
    }
    let field = |v: &str| {
        if v.trim().is_empty() {
            "—".to_string()
        } else {
            v.to_string()
        }
    };
    let facts = [
        ("Position", pos),
        ("Ground level", ground),
        ("Depth", depth_text(model, p.depth)),
        ("Method", field(&p.method)),
        ("Date", field(&p.date)),
        ("Report", field(&p.report)),
    ];
    for (i, (a, v)) in facts.iter().enumerate() {
        let x = 40.0 + (i % 3) as f64 * 52.0;
        let y = -8.0 - (i / 3) as f64 * 8.0;
        items.push(text(
            at(x, y),
            a.to_string(),
            1.8,
            Start,
            false,
            Some(MUTED),
        ));
        let v: String = v.chars().take(30).collect();
        items.push(text(at(x, y - 3.4), v, 2.2, Start, true, None));
    }
    // column titles
    let ty = -head - cols / 2.0 - 0.8;
    for (x, t) in [
        (c_dep + 1.5, "Depth m"),
        (c_lev + 1.5, "Level"),
        (c_leg + 1.5, "Legend"),
        (c_des + 1.5, "Description"),
        (c_wat + 1.0, "Water"),
        (c_tst + 1.5, "Tests"),
    ] {
        items.push(text(at(x, ty), t.into(), 1.8, Start, true, Some(MUTED)));
    }
    let tests: Vec<_> = p.tests.iter().collect();
    let scale_max = tests
        .iter()
        .map(|t| t.value)
        .fold(0.0_f64, f64::max)
        .max(10.0);
    let scale_max = (scale_max / 10.0).ceil() * 10.0;
    let gx = |v: f64| c_gra + 3.0 + (c_end - c_gra - 6.0) * (v / scale_max).clamp(0.0, 1.0);
    let graph_kind = tests.first().map(|t| t.kind).unwrap_or(TestKind::Spt);
    items.push(text(
        at(c_gra + 1.5, ty),
        graph_kind.label().into(),
        1.8,
        Start,
        true,
        Some(MUTED),
    ));
    // depth ticks every metre (every 2 or 5 on deep holes)
    let step = if p.depth > 25.0 {
        5.0
    } else if p.depth > 12.0 {
        2.0
    } else {
        1.0
    };
    let mut d = 0.0;
    while d <= p.depth + 1e-9 {
        let y = yd(d);
        line(&mut items, id, Pen::Fine, at(c_lev - 2.0, y), at(c_lev, y));
        items.push(text(
            at(c_dep + 1.5, y - 0.7),
            format!("{d:.1}"),
            2.0,
            Start,
            false,
            None,
        ));
        d += step;
    }
    // graph gridlines
    for i in 0..=4 {
        let v = scale_max * i as f64 / 4.0;
        line(
            &mut items,
            id,
            Pen::Contour,
            at(gx(v), y0),
            at(gx(v), bottom),
        );
        items.push(text(
            at(gx(v), y0 + 1.2),
            format!("{v:.0}"),
            1.6,
            Middle,
            false,
            Some(MUTED),
        ));
    }
    // layers
    let mut last_text = y0;
    for (from, l) in p.spans() {
        let to = l.to.min(p.depth);
        if to <= from {
            continue;
        }
        let (ya, yb) = (yd(from), yd(to));
        let cell = [at(c_leg, ya), at(c_des, ya), at(c_des, yb), at(c_leg, yb)];
        soil_fill(&mut items, id, &cell, l.soil, k);
        ring(&mut items, id, Pen::Projection, &cell);
        line(
            &mut items,
            id,
            Pen::Projection,
            at(c_lev, yb),
            at(c_wat, yb),
        );
        // the level and depth of its foot
        items.push(text(
            at(c_lev + 1.5, yb + 1.6),
            creabim_core::site_level(model, top - to),
            1.9,
            Start,
            false,
            None,
        ));
        items.push(text(
            at(c_leg - 1.5, yb + 1.6),
            format!("{to:.2}"),
            1.9,
            End,
            true,
            None,
        ));
        // its description, from its top, as far as it has room
        let mut y = (ya - 2.2).min(last_text - 2.8);
        let mut body = String::new();
        if !l.unit.is_empty() {
            body.push_str(&l.unit);
            body.push_str("  ");
        }
        body.push_str(if l.description.is_empty() {
            l.soil.label()
        } else {
            &l.description
        });
        for (j, ln) in wrap(&body, 44).into_iter().enumerate() {
            if j > 0 && y < yb + 1.0 {
                break;
            }
            items.push(text(
                at(c_des + 1.5, y),
                ln,
                2.0,
                Start,
                j == 0 && !l.unit.is_empty(),
                None,
            ));
            last_text = y;
            y -= 2.8;
        }
    }
    if p.layers.is_empty() {
        items.push(text(
            at(c_des + 1.5, y0 - 4.0),
            "No layers described yet".into(),
            2.0,
            Start,
            false,
            Some(MUTED),
        ));
    }
    items.push(text(
        at(c_des + 1.5, bottom - 4.5),
        format!(
            "End of {} at {}",
            p.kind.label().to_lowercase(),
            depth_text(model, p.depth)
        ),
        2.2,
        Start,
        true,
        None,
    ));
    // water: struck (filled) and where it rose to (open)
    for w in &p.water {
        let y = yd(w.depth);
        let x = c_wat + 6.0;
        items.push(PlanItem::Tint {
            id,
            points: vec![at(x - 1.8, y + 2.2), at(x + 1.8, y + 2.2), at(x, y)],
            rgb: WATER,
        });
        line(&mut items, id, Pen::Fine, at(c_wat, y), at(c_tst, y));
        items.push(text(
            at(c_wat + 0.8, y - 2.6),
            format!("{:.2}", w.depth),
            1.6,
            Start,
            false,
            Some(WATER),
        ));
        if let Some(r) = w.rest {
            let y = yd(r);
            ring(
                &mut items,
                id,
                Pen::Fine,
                &[at(x - 1.8, y + 2.2), at(x + 1.8, y + 2.2), at(x, y)],
            );
            items.push(text(
                at(c_wat + 0.8, y + 3.0),
                format!("{r:.2}"),
                1.6,
                Start,
                false,
                Some(WATER),
            ));
        }
    }
    // tests: depth and value, and a graph of the main kind's
    let mut pts = vec![];
    for t in &tests {
        let y = yd(t.depth);
        let v = if t.text.is_empty() {
            format!("{:.0}", t.value)
        } else {
            t.text.clone()
        };
        items.push(text(
            at(c_tst + 1.0, y - 0.7),
            format!("{:.1}", t.depth),
            1.8,
            Start,
            false,
            None,
        ));
        items.push(text(at(c_gra - 1.0, y - 0.7), v, 1.8, End, true, None));
        if t.kind == graph_kind {
            pts.push(at(gx(t.value), y));
        }
    }
    for w in pts.windows(2) {
        line(&mut items, id, Pen::Cut, w[0], w[1]);
    }
    for q in &pts {
        let dot = circle(*q, 0.8 * mm, 0.0, std::f64::consts::TAU, 10);
        items.push(PlanItem::Tint {
            id,
            points: dot,
            rgb: [0.12, 0.56, 0.53],
        });
    }
    // notes under the frame
    let note = if p.notes.trim().is_empty() {
        "Depths below the ground level above; levels as the project's. As the soils report logged it."
            .to_string()
    } else {
        p.notes.clone()
    };
    items.push(text(
        at(2.0, bottom - 10.0),
        note.chars().take(140).collect(),
        1.8,
        Start,
        false,
        Some(MUTED),
    ));
    crate::section::Drawing { items }
}

#[cfg(test)]
mod tests {
    use super::*;
    use creabim_core::investigation::{FieldTest, LogLayer, WaterStrike};
    use creabim_core::{Document, Element, Level, Transaction};

    fn borehole() -> (Document, ElementId) {
        let mut doc = Document::new();
        let lv = Element::new(
            "Ground",
            ElementKind::Level(Level {
                elevation: 0.0,
                height: 3.0,
            }),
        );
        let lid = lv.id;
        let mut site = Site::probe(lid, DVec2::new(30.0, 20.0), ProbeKind::Borehole);
        if let SiteForm::Investigation(p) = &mut site.form {
            p.ground = Some(11.51);
            p.depth = 10.0;
            p.layers = vec![
                LogLayer {
                    to: 0.25,
                    soil: SoilKind::Topsoil,
                    unit: "GT1".into(),
                    description: "TOPSOIL".into(),
                },
                LogLayer {
                    to: 3.05,
                    soil: SoilKind::Clay,
                    unit: "GT3".into(),
                    description: "Firm brown silty CLAY".into(),
                },
                LogLayer {
                    to: 10.0,
                    soil: SoilKind::Rock,
                    unit: String::new(),
                    description: "Weathered SANDSTONE".into(),
                },
            ];
            p.water = vec![WaterStrike {
                depth: 3.1,
                rest: Some(2.8),
                note: String::new(),
            }];
            p.tests = vec![
                FieldTest {
                    depth: 1.5,
                    kind: TestKind::Spt,
                    value: 9.0,
                    text: String::new(),
                },
                FieldTest {
                    depth: 6.0,
                    kind: TestKind::Spt,
                    value: 50.0,
                    text: "50/220 mm".into(),
                },
            ];
        }
        let e = Element::new("BH3", ElementKind::Site(site));
        let id = e.id;
        doc.commit(Transaction::new("t").insert(lv).insert(e))
            .unwrap();
        (doc, id)
    }

    #[test]
    fn a_log_writes_each_layer_its_levels_and_the_tests() {
        let (doc, id) = borehole();
        let d = log_drawing(doc.model(), id, 50.0);
        let texts: Vec<String> = d
            .items
            .iter()
            .filter_map(|i| match i {
                PlanItem::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect();
        for want in [
            "BH3",
            "GT3 Firm brown silty CLAY",
            "3.05",
            "8.46",
            "50/220 mm",
            "End of borehole at 10.00 m",
        ] {
            assert!(
                texts.iter().any(|t| t == want),
                "{want} missing from {texts:?}"
            );
        }
        // every layer is hatched: its tint and some symbol lines
        let tints = d
            .items
            .iter()
            .filter(|i| matches!(i, PlanItem::Tint { rgb, .. } if *rgb == SoilKind::Clay.color()))
            .count();
        assert_eq!(tints, 1);
    }

    #[test]
    fn every_soil_symbol_draws_lines() {
        let sq = [
            DVec2::ZERO,
            DVec2::new(20.0, 0.0),
            DVec2::new(20.0, 20.0),
            DVec2::new(0.0, 20.0),
        ];
        for kind in SoilKind::ALL {
            let n = crate::looks::pattern_lines(&soil_pattern(kind), &sq, 1.0).len();
            assert!(n > 10, "{} drew {n} lines", kind.label());
        }
    }

    #[test]
    fn a_borehole_shows_its_layers_in_3d_and_its_name_on_plan() {
        let (doc, id) = borehole();
        let m = doc.model();
        let ElementKind::Site(s) = &m.get(id).unwrap().kind else {
            unreachable!()
        };
        let p = s.as_probe().unwrap();
        let meshes = probe_meshes(m, s, p);
        assert_eq!(meshes.len(), 4, "three layers and the water");
        let mut out = vec![];
        probe_plan(m, &mut out, id, s, p, true);
        assert!(out
            .iter()
            .any(|i| matches!(i, PlanItem::Text { text, .. } if text.starts_with("BH3 "))));
        assert!(out
            .iter()
            .any(|i| matches!(i, PlanItem::Text { text, .. } if text.contains("to 10.00 m"))));
    }
}
