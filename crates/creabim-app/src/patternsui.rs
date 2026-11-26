//! The Patterns window: the project's hatch and surface patterns beside
//! the Materials window. A pattern is one recipe (line families, a
//! generator such as a brick bond, or a tile you drew) used four ways:
//! hatching cut faces, drawn true size on surfaces, modelled, and as a
//! texture. Materials pick their patterns by name; every edit here is an
//! ordinary edit of the project's style pack, so it undoes and saves.

use super::*;
use creabim_core::patfile;
use creabim_core::pattern::{self, Bond, DrawnStroke, DrawnTile, Job, Recipe, Repeat};
use creabim_core::styles::{self, HatchPattern, PatternUnits, SmallScale, StylePack};
use creabim_core::Hatch;
use egui::Pos2;
use std::sync::Mutex;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum PatTab {
    #[default]
    Project,
    Library,
}

/// What the pointer does on the drawn-tile canvas.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
enum DrawTool {
    #[default]
    Select,
    Line,
    Shape,
    Rectangle,
    Circle,
    /// A stroke's copy beside it, out or in (REV01, E).
    Offset,
    /// Two clicks: the distance, nothing kept (REV01, D).
    Measure,
    /// Two clicks: a dimension kept with the tile (REV01, D).
    Dimension,
}

impl DrawTool {
    const ALL: [DrawTool; 8] = [
        DrawTool::Select,
        DrawTool::Line,
        DrawTool::Shape,
        DrawTool::Rectangle,
        DrawTool::Circle,
        DrawTool::Offset,
        DrawTool::Measure,
        DrawTool::Dimension,
    ];

    fn label(self) -> &'static str {
        match self {
            DrawTool::Select => "Select",
            DrawTool::Line => "Line",
            DrawTool::Shape => "Shape",
            DrawTool::Rectangle => "Rectangle",
            DrawTool::Circle => "Circle",
            DrawTool::Offset => "Offset",
            DrawTool::Measure => "Measure",
            DrawTool::Dimension => "Dimension",
        }
    }

    fn hint(self) -> &'static str {
        match self {
            DrawTool::Select => "Click a stroke to pick it; drag a corner, or the stroke to move it. Delete removes it, Shift+D copies it.",
            DrawTool::Line => "Click the points of an open line; double-click, Enter or a right-click ends it; Backspace takes a point back.",
            DrawTool::Shape => {
                "Click the corners of a closed shape; click the first again to close it."
            }
            DrawTool::Rectangle => "Drag a rectangle, or click two corners.",
            DrawTool::Circle => "Drag from its centre out to its edge, or click both.",
            DrawTool::Offset => "Click a stroke, then move out or in and click, or type the distance and press Enter. Ctrl keeps going.",
            DrawTool::Measure => "Click two points: the distance, both offsets and the angle. Nothing is kept; Esc clears it.",
            DrawTool::Dimension => "Click two points: a dimension kept with the tile, never drawn in plans. Click its number to resize.",
        }
    }

    /// Its key on the Patterns area, as in the drawings.
    fn key(self) -> &'static str {
        match self {
            DrawTool::Select => "V",
            DrawTool::Line => "L",
            DrawTool::Shape => "P",
            DrawTool::Rectangle => "R",
            DrawTool::Circle => "C",
            DrawTool::Offset => "O",
            DrawTool::Measure => "M",
            DrawTool::Dimension => "D",
        }
    }
}

/// Which part of the Patterns page is drawn: all of it (the old
/// window), the canvas (the Patterns area) or its settings (Properties).
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
enum Part {
    #[default]
    All,
    Canvas,
    Props,
}

/// The Patterns window's own state, for the session.
#[derive(Default)]
pub(super) struct PatternsUi {
    pub open: bool,
    part: Part,
    tab: PatTab,
    filter: String,
    /// The pattern being edited, by name.
    current: Option<String>,
    /// The library pattern shown.
    preset: usize,
    /// The name as typed, before it is committed (old name, typed).
    name_edit: Option<(String, String)>,
    tool: DrawTool,
    /// The stroke picked on the canvas.
    stroke: Option<usize>,
    /// Points of the line or shape being drawn.
    draft: Vec<DVec2>,
    /// The point being dragged: stroke, point.
    grip: Option<(usize, usize)>,
    /// Where a rectangle or circle drag started.
    press: Option<DVec2>,
    /// Shown at 1:`scale` on the canvas when not drawing.
    scale: f64,
    /// Bring the window to the front next frame.
    raise: bool,
    /// The Patterns area's view: zoom (1 fits), pan in tile units, and
    /// whether the repeat is shown round the tile (owner 2026-11-18).
    view_zoom: f32,
    view_pan: DVec2,
    repeat: bool,
    /// The Patterns area's camera, as a Profile view's: the point at its
    /// centre and pixels a metre; none fits the tile.
    cam: Option<(DVec2, f32)>,
    /// The tile while a drag edits it: drawn as it goes, saved once when
    /// the drag ends (a save a frame rebuilt the drawings of every
    /// material using the pattern).
    live: Option<DrawnTile>,
    /// A stroke being moved by a drag: it, the point grabbed, its points
    /// before.
    moving: Option<(usize, DVec2, Vec<[f64; 2]>)>,
    /// The pass the pointer was last over the area, for the app's keys to
    /// stand aside (as a Profile view's).
    pub hovered_pass: u64,
    /// The pattern shown laid, at its angle and scale, rather than its
    /// tile being drawn on (REV01: Edit / Seen).
    seen: bool,
    /// Sizes typed while drawing: the field being typed, and both fields
    /// (length and angle, width and height, diameter and radius).
    typed: Option<(usize, [String; 2])>,
    /// Measure's last reading: from, to.
    measured: Option<(DVec2, DVec2)>,
    /// A dimension's number being typed: which, the text.
    dim_edit: Option<(usize, String)>,
    /// Offset's stroke, picked.
    offset_src: Option<usize>,
    /// What the right-click menu is about.
    menu_on: Option<MenuOn>,
}

/// What a right-click on the Patterns area is on (REV01, B).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum MenuOn {
    Paper,
    Stroke(usize),
    Point(usize, usize),
    Dim(usize),
}

/// What its menu asked for.
#[derive(Clone, Copy, PartialEq, Debug)]
enum MenuAct {
    Duplicate,
    Delete,
    Job(Job),
    Mirror(bool),
    OpenClose,
    Offset,
    AddPoint,
    DeletePoint,
    Split,
    DimType,
    DimDelete,
    Repeat,
    Frame,
    Seen,
    ClearDims,
}

/// Characters typed this frame into `buf`: a number, and for a length
/// its unit (215, 21.5cm, 8' 6"); for an angle only the number.
fn take_typed(ui: &egui::Ui, buf: &mut String, angle: bool) {
    let text: String = ui.input(|i| {
        i.events
            .iter()
            .filter_map(|e| match e {
                egui::Event::Text(t) => Some(t.clone()),
                _ => None,
            })
            .collect()
    });
    for c in text.chars() {
        match c {
            '0'..='9' | '.' | '-' => buf.push(c),
            ',' => buf.push('.'),
            '\'' | '"' | '/' | ' ' if !angle && !buf.is_empty() => buf.push(c),
            'm' | 'c' | 'f' | 't' | 'i' | 'n' if !angle && !buf.is_empty() => buf.push(c),
            _ => {}
        }
    }
}

/// Dimension `di` of tile `t` made `len` long: the strokes ending at its
/// far end stretch, every point of theirs at or past that end moving
/// along it, and the dimensions on those points follow.
fn resize_dim(t: &mut DrawnTile, di: usize, len: f64) {
    let Some([a, b]) = t.dims.get(di).copied() else {
        return;
    };
    let (a, b) = (DVec2::from(a), DVec2::from(b));
    let Some(dir) = (b - a).try_normalize() else {
        return;
    };
    let d = dir * (len - a.distance(b));
    let past = b.dot(dir) - 1e-7;
    let mut moved: Vec<DVec2> = vec![];
    for s in &mut t.strokes {
        if !s.points.iter().any(|q| DVec2::from(*q).distance(b) < 1e-6) {
            continue;
        }
        for q in &mut s.points {
            let v = DVec2::from(*q);
            if v.dot(dir) >= past {
                moved.push(v);
                *q = (v + d).into();
            }
        }
    }
    moved.push(b);
    for dm in &mut t.dims {
        for e in dm.iter_mut() {
            let v = DVec2::from(*e);
            if moved.iter().any(|m| m.distance(v) < 1e-6) {
                *e = (v + d).into();
            }
        }
    }
}

/// Stroke `s` offset by `by` (closed: out when positive; open: to its
/// left), or none when that undoes it.
fn offset_stroke(s: &DrawnStroke, by: f64) -> Option<DrawnStroke> {
    let pts: Vec<DVec2> = s.points.iter().map(|q| DVec2::from(*q)).collect();
    let out = if s.closed {
        creabim_core::members::offset_polygon(&pts, by)
    } else {
        creabim_core::offset::offset_path(&pts, by)
    };
    if s.closed && creabim_core::polygon::signed_area2(&out).abs() < 1e-12 {
        return None;
    }
    Some(DrawnStroke {
        points: out.into_iter().map(Into::into).collect(),
        name: String::new(),
        ..s.clone()
    })
}

/// How far, and to which side, `p` is from stroke `s`: out of a closed
/// one positive; for an open one, to its left positive.
fn side_distance(s: &DrawnStroke, p: DVec2) -> f64 {
    let pts: Vec<DVec2> = s.points.iter().map(|q| DVec2::from(*q)).collect();
    creabim_core::offset::side_of(&pts, s.closed, p)
}

/// Asked from outside the window (a material's Edit… button, the
/// Graphic styles window): open it, at a pattern when named.
static REQUEST: Mutex<Option<Option<String>>> = Mutex::new(None);

/// Open the Patterns window, at pattern `name` when given.
pub(super) fn request_open(name: Option<&str>) {
    if let Ok(mut r) = REQUEST.lock() {
        *r = Some(name.map(str::to_string));
    }
}

fn c32(c: [f32; 3]) -> Color32 {
    Color32::from_rgb(
        (c[0].clamp(0.0, 1.0) * 255.0) as u8,
        (c[1].clamp(0.0, 1.0) * 255.0) as u8,
        (c[2].clamp(0.0, 1.0) * 255.0) as u8,
    )
}

/// Paint pattern `p` into `rect`, `side` model metres across, as it
/// draws at 1:`scale` (holes filled, the rest as lines).
fn paint_pattern(painter: &egui::Painter, rect: Rect, p: &HatchPattern, scale: f64, side: f64) {
    // a square has no longest side: the preview runs along x
    let flat;
    let p = if p.along_longest {
        flat = HatchPattern {
            along_longest: false,
            ..p.clone()
        };
        &flat
    } else {
        p
    };
    // drawn as on white paper, then turned over to dark paper with the
    // drawings (DS MU21): greys turn over, a coloured background is kept
    super::paint_drawing(painter, || {
        paint_pattern_light(painter, rect, p, scale, side)
    });
}

fn paint_pattern_light(
    painter: &egui::Painter,
    rect: Rect,
    p: &HatchPattern,
    scale: f64,
    side: f64,
) {
    let ink = Color32::from_gray(45);
    let paper = p.fill.map_or(Color32::WHITE, |c| c32(c.0));
    painter.rect_filled(rect, 0.0, paper);
    let to = |q: [f32; 2]| {
        egui::pos2(
            rect.min.x + q[0] * rect.width(),
            rect.max.y - q[1] * rect.height(),
        )
    };
    let stroke = egui::Stroke::new(1.0, ink);
    let clip = painter.with_clip_rect(rect);
    with_laid(p, scale, side, |laid| {
        for pts in &laid.holes {
            let ps: Vec<_> = pts.iter().map(|q| to(*q)).collect();
            clip.add(egui::Shape::convex_polygon(
                ps,
                ink.gamma_multiply(0.8),
                egui::Stroke::NONE,
            ));
        }
        for (a, b) in &laid.lines {
            let (a, b) = (to(*a), to(*b));
            if a.distance(b) < 0.8 {
                clip.circle_filled(a, 0.7, ink);
            } else {
                clip.line_segment([a, b], stroke);
            }
        }
    });
}

/// A pattern laid on a square, in fractions of its side: what painting
/// it needs, kept between frames.
struct Laid {
    p: HatchPattern,
    scale: f64,
    side: f64,
    holes: Vec<Vec<[f32; 2]>>,
    lines: Vec<([f32; 2], [f32; 2])>,
}

thread_local! {
    /// The patterns laid lately (owner 2026-11-18: "Seen at" lagged; the
    /// area and the swatches laid every pattern again each frame).
    static LAID: std::cell::RefCell<Vec<Laid>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Run `f` on pattern `p` laid on a square `side` metres across at
/// 1:`scale`, laying it only when it isn't kept already.
fn with_laid(p: &HatchPattern, scale: f64, side: f64, f: impl FnOnce(&Laid)) {
    LAID.with(|kept| {
        let mut kept = kept.borrow_mut();
        let at = kept
            .iter()
            .position(|l| l.scale == scale && l.side == side && l.p == *p);
        let at = match at {
            Some(i) => i,
            None => {
                let k = 1000.0 / scale;
                let poly = [
                    DVec2::ZERO,
                    DVec2::new(side, 0.0),
                    DVec2::new(side, side),
                    DVec2::new(0.0, side),
                ];
                let unit = |q: DVec2| [(q.x / side) as f32, (q.y / side) as f32];
                let mut holes = vec![];
                if p.recipe.is_some() {
                    if let Some(l) = creabim_geom::looks::recipe_layout(p, &poly, k) {
                        for (pts, job) in &l.shapes {
                            if *job == Job::Hole && pts.len() >= 3 {
                                holes.push(pts.iter().map(|q| unit(*q)).collect());
                            }
                        }
                    }
                }
                let lines = creabim_geom::looks::pattern_lines(p, &poly, k)
                    .into_iter()
                    .map(|(a, b)| (unit(a), unit(b)))
                    .collect();
                if kept.len() >= 32 {
                    kept.remove(0);
                }
                kept.push(Laid {
                    p: p.clone(),
                    scale,
                    side,
                    holes,
                    lines,
                });
                kept.len() - 1
            }
        };
        f(&kept[at]);
    });
}

/// A square swatch of a pattern as it draws at 1:`scale`: `size` points
/// showing 20 mm of paper.
pub(super) fn swatch(ui: &mut egui::Ui, p: &HatchPattern, scale: f64, size: f32) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let side = 20.0 / (1000.0 / scale);
    paint_pattern(ui.painter(), r, p, scale, side);
    ui.painter().rect_stroke(
        r,
        2.0,
        egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
        egui::StrokeKind::Inside,
    );
    resp
}

/// Pattern `p` filling `rect` (any proportions), about 40 mm of paper
/// across, at the scale its swatches use: a Libraries tile.
pub(super) fn paint_in(ui: &mut egui::Ui, rect: Rect, p: &HatchPattern) {
    let scale = swatch_scale(p);
    let side = 40.0 / (1000.0 / scale);
    let sq = Rect::from_min_size(rect.min, Vec2::splat(rect.width().max(rect.height())));
    let clip = ui.painter().with_clip_rect(rect);
    paint_pattern(&clip, sq, p, scale, side);
}

/// The scale a swatch of `p` is drawn at: true-size patterns at 1:20
/// (their coursing shows), paper ones at 1:100.
fn swatch_scale(p: &HatchPattern) -> f64 {
    if p.recipe.is_some() || p.units == PatternUnits::Model {
        20.0
    } else {
        100.0
    }
}

/// A length in metres, edited in millimetres, as a Properties row: the
/// field runs to the row's end, its number in the middle (DSM09).
fn mm(ui: &mut egui::Ui, label: &str, v: &mut f64, min: f64) -> bool {
    let mut x = *v * 1000.0;
    let changed = crate::props::row(ui, label, |ui| {
        crate::props::fill(
            ui,
            crate::units::model_mm(&mut x).range(min * 1000.0..=100_000.0),
        )
        .changed()
    });
    if changed {
        *v = x / 1000.0;
    }
    changed
}

/// The random lay-out's number, the dice at the row's end (as a tree's
/// seed).
fn seed(ui: &mut egui::Ui, s: &mut u32) -> bool {
    crate::props::row(ui, "Variation", |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            let w = (ui.available_width() - 24.0).max(40.0);
            let h = ui.spacing().interact_size.y;
            let mut c = ui
                .add_sized([w, h], egui::DragValue::new(s).range(0..=9999))
                .on_hover_text("Another number lays the random lengths out differently")
                .changed();
            if crate::props::icon_button(
                ui,
                "dice",
                "Shuffle: another lay-out of the random lengths",
            )
            .clicked()
            {
                *s = s.wrapping_mul(1_103_515_245).wrapping_add(12_345) % 10_000;
                c = true;
            }
            c
        })
        .inner
    })
}

/// The recipes a pattern can be made by, with a starting recipe each.
const MADE_BY: [&str; 9] = [
    "Line families",
    "Bond",
    "Herringbone",
    "Basketweave",
    "Boards",
    "Tiles",
    "Hexagons",
    "Random stone",
    "Drawn tile",
];

fn starting_recipe(kind: &str) -> Option<Recipe> {
    Some(match kind {
        "Bond" => Recipe::Bond {
            bond: Bond::Stretcher,
            length: 0.215,
            height: 0.065,
            joint: 0.01,
        },
        "Herringbone" => Recipe::Herringbone {
            length: 0.2,
            width: 0.1,
            joint: 0.003,
        },
        "Basketweave" => Recipe::Basketweave {
            length: 0.2,
            width: 0.1,
            joint: 0.003,
        },
        "Boards" => Recipe::Boards {
            length: 2.4,
            min_length: 0.9,
            width: 0.145,
            joint: 0.002,
            seed: 1,
        },
        "Tiles" => Recipe::Tiles {
            width: 0.3,
            height: 0.3,
            joint: 0.003,
            offset: 0.0,
        },
        "Hexagons" => Recipe::Hexagons {
            size: 0.2,
            joint: 0.003,
        },
        "Random stone" => Recipe::Ashlar {
            length: 0.45,
            height: 0.15,
            joint: 0.012,
            seed: 1,
        },
        "Drawn tile" => Recipe::Drawn(DrawnTile {
            size: [0.3, 0.3],
            repeat: Repeat::Grid,
            dims: vec![],
            strokes: vec![DrawnStroke {
                points: vec![[0.05, 0.05], [0.25, 0.05], [0.25, 0.25], [0.05, 0.25]],
                closed: true,
                job: Job::Raised,
                depth: 0.0,
                name: "square".into(),
            }],
        }),
        _ => return None,
    })
}

/// The recipe's sizes. True when one changed.
fn recipe_fields(ui: &mut egui::Ui, r: &mut Recipe) -> bool {
    let mut changed = false;
    match r {
        Recipe::Bond {
            bond,
            length,
            height,
            joint,
        } => {
            crate::props::Combo::salted("Bond", "pat_bond")
                .selected_text(bond.label())
                .show_ui(ui, |ui| {
                    for b in Bond::ALL {
                        changed |= ui.selectable_value(bond, b, b.label()).changed();
                    }
                });
            changed |= mm(ui, "Unit length", length, 0.005);
            changed |= mm(ui, "Unit height", height, 0.005);
            changed |= mm(ui, "Joint", joint, 0.0);
        }
        Recipe::Herringbone {
            length,
            width,
            joint,
        }
        | Recipe::Basketweave {
            length,
            width,
            joint,
        } => {
            changed |= mm(ui, "Unit length", length, 0.005);
            changed |= mm(ui, "Unit width", width, 0.005);
            changed |= mm(ui, "Joint", joint, 0.0);
        }
        Recipe::Boards {
            length,
            min_length,
            width,
            joint,
            seed: s,
        } => {
            changed |= mm(ui, "Longest", length, 0.05);
            changed |= mm(ui, "Shortest", min_length, 0.05);
            changed |= mm(ui, "Board width", width, 0.01);
            changed |= mm(ui, "Joint", joint, 0.0);
            changed |= seed(ui, s);
            if *min_length > *length {
                *min_length = *length;
            }
        }
        Recipe::Tiles {
            width,
            height,
            joint,
            offset,
        } => {
            changed |= mm(ui, "Tile width", width, 0.005);
            changed |= mm(ui, "Tile height", height, 0.005);
            changed |= mm(ui, "Joint", joint, 0.0);
            let mut pc = *offset * 100.0;
            if crate::props::row(ui, "Rows slide", |ui| {
                crate::props::fill(
                    ui,
                    egui::Slider::new(&mut pc, 0.0..=50.0)
                        .suffix(" %")
                        .max_decimals(0),
                )
                .on_hover_text("0 is a grid, 50 % a half bond")
                .changed()
            }) {
                *offset = pc / 100.0;
                changed = true;
            }
        }
        Recipe::Hexagons { size, joint } => {
            changed |= mm(ui, "Across flats", size, 0.01);
            changed |= mm(ui, "Joint", joint, 0.0);
        }
        Recipe::Ashlar {
            length,
            height,
            joint,
            seed: s,
        } => {
            changed |= mm(ui, "Stone length", length, 0.02);
            changed |= mm(ui, "Course height", height, 0.02);
            changed |= mm(ui, "Joint", joint, 0.0);
            changed |= seed(ui, s);
        }
        Recipe::Drawn(t) => {
            changed |= mm(ui, "Width", &mut t.size[0], 0.005);
            changed |= mm(ui, "Height", &mut t.size[1], 0.005);
            crate::props::Combo::salted("Repeats", "pat_repeat")
                .selected_text(t.repeat.label())
                .show_ui(ui, |ui| {
                    for x in Repeat::ALL {
                        changed |= ui.selectable_value(&mut t.repeat, x, x.label()).changed();
                    }
                });
        }
    }
    changed
}

/// The strokes of a drawn tile: name, job, depth. True when one changed.
fn stroke_list(ui: &mut egui::Ui, t: &mut DrawnTile, picked: &mut Option<usize>) -> bool {
    let mut changed = false;
    let mut remove = None;
    if t.strokes.is_empty() {
        ui.label(
            egui::RichText::new(
                "Nothing drawn yet: pick Line or Shape above the tile and draw in it.",
            )
            .weak(),
        );
    }
    for (i, s) in t.strokes.iter_mut().enumerate() {
        ui.push_id(("stroke", i), |ui| {
            crate::props::flow(ui, |ui| {
                let on = *picked == Some(i);
                let what = if s.closed { "Shape" } else { "Line" };
                if ui
                    .selectable_label(on, what)
                    .on_hover_text(if s.closed {
                        "Closed shape"
                    } else {
                        "Open line"
                    })
                    .clicked()
                {
                    *picked = Some(i);
                }
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut s.name)
                            .vertical_align(egui::Align::Center)
                            .hint_text(if s.closed { "shape" } else { "line" })
                            .desired_width(64.0),
                    )
                    .changed()
                {
                    changed = true;
                }
                let jobs: &[Job] = if s.closed { &Job::CLOSED } else { &Job::OPEN };
                egui::ComboBox::from_id_salt("job")
                    .icon(crate::props::combo_icon)
                    .width(98.0)
                    .selected_text(s.job.label())
                    .show_ui(ui, |ui| {
                        for j in jobs {
                            changed |= ui.selectable_value(&mut s.job, *j, j.label()).changed();
                        }
                    });
                if !matches!(s.job, Job::Line | Job::Hole) {
                    let mut d = if s.depth > 0.0 {
                        s.depth
                    } else {
                        s.job.usual_depth()
                    } * 1000.0;
                    if ui
                        .add(crate::units::model_mm(&mut d).range(0.5..=200.0))
                        .on_hover_text(match s.job {
                            Job::Ridge | Job::Raised => "How far it stands out",
                            _ => "How deep it goes",
                        })
                        .changed()
                    {
                        s.depth = d / 1000.0;
                        changed = true;
                    }
                }
                if crate::props::icon_button(ui, "trash-2", "Delete the stroke").clicked() {
                    remove = Some(i);
                }
            });
        });
    }
    if let Some(i) = remove {
        t.strokes.remove(i);
        *picked = None;
        changed = true;
    }
    changed
}

/// The strokes of a drawn tile in Properties: one row a stroke, its
/// little picture filled as its job draws it, then the picked one's
/// settings, a row each. True when one changed.
fn stroke_rows(
    ui: &mut egui::Ui,
    t: &mut DrawnTile,
    picked: &mut Option<usize>,
) -> Option<&'static str> {
    let mut changed = None;
    if t.strokes.is_empty() {
        crate::props::empty(ui, "Nothing drawn yet: pick Line (L) or Shape (P) in the area's header and draw on the tile.");
        return None;
    }
    for (i, s) in t.strokes.iter().enumerate() {
        let what = if !s.name.is_empty() {
            s.name.clone()
        } else if s.closed {
            format!("Shape {}", i + 1)
        } else {
            format!("Line {}", i + 1)
        };
        let text = format!("{what}  ·  {}", s.job.label());
        let pts: Vec<DVec2> = s.points.iter().map(|q| DVec2::from(*q)).collect();
        let (closed, job) = (s.closed, s.job);
        let r = crate::props::picture_option(ui, *picked == Some(i), &text, |pa, r, st| {
            let lo = pts.iter().fold(DVec2::splat(f64::MAX), |a, q| a.min(*q));
            let hi = pts.iter().fold(DVec2::splat(f64::MIN), |a, q| a.max(*q));
            let span = (hi - lo).max_element().max(1e-6);
            let k = (r.width() - 2.0) as f64 / span;
            let to = |q: DVec2| {
                r.center()
                    + Vec2::new(
                        ((q.x - (lo.x + hi.x) / 2.0) * k) as f32,
                        (-(q.y - (lo.y + hi.y) / 2.0) * k) as f32,
                    )
            };
            if closed {
                if let Some(f) = job_fill(job) {
                    fill_shape(pa, &pts, &to, f);
                }
                pa.add(egui::Shape::closed_line(
                    pts.iter().map(|q| to(*q)).collect(),
                    egui::Stroke::new(1.2, st.color),
                ));
            } else {
                pa.add(egui::Shape::line(
                    pts.iter().map(|q| to(*q)).collect(),
                    egui::Stroke::new(1.2, st.color),
                ));
            }
        });
        if r.clicked() {
            *picked = Some(i);
        }
    }
    // the picked stroke's settings
    let mut remove = None;
    if let Some(i) = picked.filter(|i| *i < t.strokes.len()) {
        let s = &mut t.strokes[i];
        crate::props::sub_box(ui, "pat.stroke", "Picked stroke", None, true, |ui| {
            if crate::props::row(ui, "Name", |ui| {
                crate::props::text_field(
                    ui,
                    egui::TextEdit::singleline(&mut s.name).hint_text(if s.closed {
                        "shape"
                    } else {
                        "line"
                    }),
                    f32::INFINITY,
                )
                .changed()
            }) {
                changed = Some("Stroke name");
            }
            let jobs: &[Job] = if s.closed { &Job::CLOSED } else { &Job::OPEN };
            crate::props::Combo::salted("Job", "pat_stroke_job")
                .selected_text(s.job.label())
                .show_ui(ui, |ui| {
                    for j in jobs {
                        if ui.selectable_value(&mut s.job, *j, j.label()).changed() {
                            changed = Some("Stroke job");
                        }
                    }
                });
            if !matches!(s.job, Job::Line | Job::Hole) {
                let mut d = if s.depth > 0.0 {
                    s.depth
                } else {
                    s.job.usual_depth()
                } * 1000.0;
                let what = match s.job {
                    Job::Ridge | Job::Raised => "Stands out",
                    _ => "Depth",
                };
                if crate::props::row(ui, what, |ui| {
                    crate::props::fill(ui, crate::units::model_mm(&mut d).range(0.5..=200.0))
                        .changed()
                }) {
                    s.depth = d / 1000.0;
                    changed = Some("Stroke depth");
                }
            }
            if crate::props::action(
                ui,
                "",
                "Delete stroke",
                "Or press Delete on the canvas",
                true,
            )
            .clicked()
            {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove {
        t.strokes.remove(i);
        *picked = None;
        changed = Some("Delete stroke");
    }
    crate::props::info(
        ui,
        "Filled on the canvas: holes (cut through, drawn solid in plans and sections), raised and recessed areas. Outlines only: drawn lines and separate units. The job says what it becomes in 3D.",
        "",
    );
    changed
}

/// An angle as typed: no trailing zeros (-75, 22.5).
fn fmt_angle(a: f64) -> String {
    let t = format!("{a:.1}");
    t.strip_suffix(".0").map_or(t.clone(), str::to_string)
}

/// A round step for snapping in a tile `size` across.
fn snap_step(size: f64) -> f64 {
    [0.001, 0.002, 0.005, 0.01, 0.02, 0.05, 0.1, 0.2, 0.5]
        .into_iter()
        .find(|s| size / s <= 40.0)
        .unwrap_or(1.0)
}

fn seg_dist(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_sq().max(1e-6)).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

impl PatternsUi {
    /// The drawn tile's canvas: the tile and its repeats round it, strokes
    /// to pick and points to drag. True when the tile changed.
    fn tile_canvas(&mut self, ui: &mut egui::Ui, p: &mut HatchPattern, side: f32) -> bool {
        self.tile_canvas_sized(ui, p, Vec2::splat(side), false)
    }

    /// The drawn tile on a canvas `size` across: the tile centred, the
    /// repeat around it to the canvas's edges (any proportions: in the
    /// Patterns area it fills the area, owner markup 2026-11-18).
    fn tile_canvas_sized(
        &mut self,
        ui: &mut egui::Ui,
        p: &mut HatchPattern,
        size: Vec2,
        area: bool,
    ) -> bool {
        let mut changed = false;
        let ghost = p.clone();
        let Some(Recipe::Drawn(t)) = p.recipe.as_mut() else {
            return false;
        };
        let (resp, painter) = ui.allocate_painter(size, Sense::click_and_drag());
        let rect = resp.rect;
        let [w, h] = t.size;
        let big = w.max(h).max(1e-3);
        let short = size.x.min(size.y) as f64;
        // the area: the tile alone, fitted with room round it, or the
        // repeat (three tiles across); the window: two tiles across
        let fit = if !area {
            short / (2.0 * big)
        } else if self.repeat {
            short * 0.92 / (3.0 * big)
        } else {
            short * 0.72 / big
        };
        if area {
            if self.view_zoom <= 0.0 {
                self.view_zoom = 1.0;
            }
            // pan: the wheel's sideways roll, a middle or right drag
            if resp.hovered() {
                let (roll, scroll) = crate::planview::wheel(ui);
                let px_now = fit * self.view_zoom as f64;
                if scroll != 0.0 {
                    // zoom about the pointer
                    if let Some(at) = resp.hover_pos() {
                        let before = (at - rect.center()) / self.view_zoom;
                        self.view_zoom =
                            (self.view_zoom * (1.0 + scroll * 0.002)).clamp(0.15, 40.0);
                        let after = (at - rect.center()) / self.view_zoom;
                        let d = (after - before) * self.view_zoom;
                        self.view_pan += DVec2::new(-d.x as f64, d.y as f64) / px_now;
                    }
                }
                self.view_pan -= DVec2::new(roll.x as f64, -roll.y as f64) / px_now;
            }
            if resp.dragged_by(egui::PointerButton::Middle)
                || resp.dragged_by(egui::PointerButton::Secondary)
            {
                let d = resp.drag_delta();
                self.view_pan -=
                    DVec2::new(d.x as f64, -d.y as f64) / (fit * self.view_zoom as f64);
            }
        }
        let px = if area {
            fit * self.view_zoom as f64
        } else {
            fit
        };
        let (sx, sy) = (size.x as f64 / px, size.y as f64 / px);
        let span = sx.max(sy);
        let centre = DVec2::new(w / 2.0, h / 2.0) + if area { self.view_pan } else { DVec2::ZERO };
        let lo = DVec2::new(centre.x - sx / 2.0, centre.y - sy / 2.0);
        let to = |q: DVec2| {
            egui::pos2(
                rect.min.x + ((q.x - lo.x) * px) as f32,
                rect.max.y - ((q.y - lo.y) * px) as f32,
            )
        };
        let from = |s: Pos2| {
            DVec2::new(
                lo.x + (s.x - rect.min.x) as f64 / px,
                lo.y + (rect.max.y - s.y) as f64 / px,
            )
        };
        let vis = ui.visuals().clone();
        let bg = ghost.fill.map_or(Color32::from_gray(250), |c| c32(c.0));
        painter.rect_filled(rect, 4.0, bg);
        let clip = painter.with_clip_rect(rect);
        // the snapping grid, faint
        let step = snap_step(w.max(h));
        let grid = Color32::from_gray(215);
        let (mut gx, gx1) = ((lo.x / step).floor() * step, lo.x + sx);
        while gx <= gx1 {
            let a = to(DVec2::new(gx, lo.y));
            clip.line_segment(
                [a, egui::pos2(a.x, rect.min.y)],
                egui::Stroke::new(0.5, grid),
            );
            gx += step;
        }
        let (mut gy, gy1) = ((lo.y / step).floor() * step, lo.y + sy);
        while gy <= gy1 {
            let a = to(DVec2::new(lo.x, gy));
            clip.line_segment(
                [a, egui::pos2(rect.max.x, a.y)],
                egui::Stroke::new(0.5, grid),
            );
            gy += step;
        }
        // the repeats, as the pattern lays them (in the area, when Repeat
        // is on)
        let ink = Color32::from_gray(150);
        let show_repeat = !area || self.repeat;
        if let Some(l) = ghost
            .recipe
            .as_ref()
            .filter(|_| show_repeat)
            .and_then(|r| r.layout(lo, lo + DVec2::splat(span), 20_000))
        {
            for (pts, job) in &l.shapes {
                let ps: Vec<Pos2> = pts.iter().map(|q| to(*q)).collect();
                let fill = match job {
                    Job::Hole => Some(Color32::from_gray(120)),
                    Job::Raised => Some(Color32::from_rgba_unmultiplied(120, 150, 110, 60)),
                    Job::Recessed => Some(Color32::from_rgba_unmultiplied(60, 60, 60, 40)),
                    _ => None,
                };
                if let Some(f) = fill {
                    clip.add(egui::Shape::convex_polygon(
                        ps.clone(),
                        f,
                        egui::Stroke::NONE,
                    ));
                }
                clip.add(egui::Shape::closed_line(ps, egui::Stroke::new(1.0, ink)));
            }
            for (pts, job) in &l.lines {
                let ps: Vec<Pos2> = pts.iter().map(|q| to(*q)).collect();
                let wdt = if *job == Job::Line { 1.0 } else { 2.0 };
                clip.add(egui::Shape::line(ps, egui::Stroke::new(wdt, ink)));
            }
        }
        // the tile itself
        let tile = Rect::from_two_pos(to(DVec2::ZERO), to(DVec2::new(w, h)));
        clip.rect_stroke(
            tile,
            0.0,
            egui::Stroke::new(1.5, vis.selection.bg_fill),
            egui::StrokeKind::Middle,
        );
        // the strokes, editable
        let accent = vis.selection.stroke.color;
        let dark = Color32::from_gray(40);
        for (i, s) in t.strokes.iter().enumerate() {
            let ps: Vec<Pos2> = s.points.iter().map(|q| to(DVec2::from(*q))).collect();
            let on = self.stroke == Some(i);
            let st = egui::Stroke::new(if on { 2.5 } else { 1.6 }, if on { accent } else { dark });
            if s.closed {
                clip.add(egui::Shape::closed_line(ps.clone(), st));
            } else {
                clip.add(egui::Shape::line(ps.clone(), st));
            }
            // a circle's many points would hide it: grips on corners only
            if on && ps.len() <= 16 {
                for q in &ps {
                    clip.rect_filled(
                        Rect::from_center_size(*q, Vec2::splat(if on { 7.0 } else { 5.0 })),
                        1.0,
                        if on { accent } else { Color32::WHITE },
                    );
                    clip.rect_stroke(
                        Rect::from_center_size(*q, Vec2::splat(if on { 7.0 } else { 5.0 })),
                        1.0,
                        egui::Stroke::new(1.0, dark),
                        egui::StrokeKind::Middle,
                    );
                }
            }
        }
        // snapping: to a point within 8 px, else the grid
        let verts: Vec<DVec2> = t
            .strokes
            .iter()
            .flat_map(|s| s.points.iter().map(|q| DVec2::from(*q)))
            .collect();
        let snap = |s: Pos2| -> DVec2 {
            if let Some(q) = verts.iter().find(|q| to(**q).distance(s) < 8.0) {
                return *q;
            }
            let q = from(s);
            DVec2::new((q.x / step).round() * step, (q.y / step).round() * step)
        };
        let hover = resp.hover_pos();
        let keys = ui.input(|i| {
            (
                i.key_pressed(egui::Key::Enter),
                i.key_pressed(egui::Key::Escape),
                i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace),
            )
        });
        let focus_free = !ui.ctx().egui_wants_keyboard_input();
        match self.tool {
            // the old window draws only; these are the area's
            DrawTool::Offset | DrawTool::Measure | DrawTool::Dimension => {}
            DrawTool::Select => {
                if resp.drag_started() {
                    if let Some(at) = resp.interact_pointer_pos() {
                        self.grip = t.strokes.iter().enumerate().find_map(|(i, s)| {
                            s.points
                                .iter()
                                .position(|q| to(DVec2::from(*q)).distance(at) < 8.0)
                                .map(|j| (i, j))
                        });
                        if let Some((i, _)) = self.grip {
                            self.stroke = Some(i);
                        }
                    }
                }
                if let (Some((i, j)), Some(at)) = (self.grip, resp.interact_pointer_pos()) {
                    if resp.dragged() {
                        let q = from(at);
                        let q =
                            DVec2::new((q.x / step).round() * step, (q.y / step).round() * step);
                        if let Some(pt) = t.strokes.get_mut(i).and_then(|s| s.points.get_mut(j)) {
                            if DVec2::from(*pt) != q {
                                *pt = q.into();
                                changed = true;
                            }
                        }
                    }
                }
                if resp.drag_stopped() {
                    self.grip = None;
                }
                if resp.clicked() {
                    if let Some(at) = resp.interact_pointer_pos() {
                        self.stroke = t.strokes.iter().enumerate().find_map(|(i, s)| {
                            let ps: Vec<Pos2> =
                                s.points.iter().map(|q| to(DVec2::from(*q))).collect();
                            let n = ps.len();
                            let segs = if s.closed { n } else { n.saturating_sub(1) };
                            (0..segs)
                                .any(|k| seg_dist(at, ps[k], ps[(k + 1) % n]) < 6.0)
                                .then_some(i)
                        });
                    }
                }
                if keys.2 && focus_free && resp.hovered() {
                    if let Some(i) = self.stroke.take() {
                        if i < t.strokes.len() {
                            t.strokes.remove(i);
                            changed = true;
                        }
                    }
                }
            }
            DrawTool::Line | DrawTool::Shape => {
                let closing = self.tool == DrawTool::Shape;
                if resp.clicked() {
                    if let Some(at) = resp.interact_pointer_pos() {
                        let q = snap(at);
                        if closing && self.draft.len() >= 3 && to(self.draft[0]).distance(at) < 8.0
                        {
                            let pts = std::mem::take(&mut self.draft);
                            t.strokes.push(DrawnStroke {
                                points: pts.into_iter().map(Into::into).collect(),
                                closed: true,
                                job: Job::Unit,
                                ..Default::default()
                            });
                            self.stroke = Some(t.strokes.len() - 1);
                            changed = true;
                        } else if self.draft.last().is_none_or(|l| l.distance(q) > 1e-9) {
                            self.draft.push(q);
                        }
                    }
                }
                let finish =
                    resp.double_clicked() || resp.secondary_clicked() || (keys.0 && focus_free);
                if finish && self.draft.len() >= 2 {
                    let pts = std::mem::take(&mut self.draft);
                    let closed = closing && pts.len() >= 3;
                    t.strokes.push(DrawnStroke {
                        points: pts.into_iter().map(Into::into).collect(),
                        closed,
                        job: if closed { Job::Unit } else { Job::Line },
                        ..Default::default()
                    });
                    self.stroke = Some(t.strokes.len() - 1);
                    changed = true;
                }
                if !self.draft.is_empty() {
                    let mut ps: Vec<Pos2> = self.draft.iter().map(|q| to(*q)).collect();
                    if let Some(hp) = hover {
                        ps.push(to(snap(hp)));
                    }
                    clip.add(egui::Shape::line(ps, egui::Stroke::new(1.5, accent)));
                }
            }
            DrawTool::Rectangle | DrawTool::Circle => {
                if resp.drag_started() {
                    self.press = resp.interact_pointer_pos().map(snap);
                }
                let now = resp.interact_pointer_pos().or(hover).map(snap);
                let tool = self.tool;
                let shape = |a: DVec2, b: DVec2| -> Vec<DVec2> {
                    if tool == DrawTool::Rectangle {
                        vec![a, DVec2::new(b.x, a.y), b, DVec2::new(a.x, b.y)]
                    } else {
                        let r = a.distance(b);
                        (0..32)
                            .map(|k| {
                                let t = k as f64 / 32.0 * std::f64::consts::TAU;
                                a + DVec2::new(t.cos(), t.sin()) * r
                            })
                            .collect()
                    }
                };
                if let (Some(a), Some(b)) = (self.press, now) {
                    let ps: Vec<Pos2> = shape(a, b).into_iter().map(to).collect();
                    clip.add(egui::Shape::closed_line(ps, egui::Stroke::new(1.5, accent)));
                    if resp.drag_stopped() {
                        let big = (b - a).abs();
                        let ok = if tool == DrawTool::Rectangle {
                            big.x > 1e-6 && big.y > 1e-6
                        } else {
                            big.length() > 1e-6
                        };
                        if ok {
                            t.strokes.push(DrawnStroke {
                                points: shape(a, b).into_iter().map(Into::into).collect(),
                                closed: true,
                                job: if tool == DrawTool::Circle {
                                    Job::Hole
                                } else {
                                    Job::Unit
                                },
                                ..Default::default()
                            });
                            self.stroke = Some(t.strokes.len() - 1);
                            changed = true;
                        }
                        self.press = None;
                    }
                }
            }
        }
        if changed {
            // the strokes above were painted before this change
            ui.ctx().request_repaint();
        }
        if keys.1 && focus_free {
            self.draft.clear();
            self.press = None;
        }
        painter.rect_stroke(
            rect,
            4.0,
            egui::Stroke::new(1.0, vis.widgets.noninteractive.bg_stroke.color),
            egui::StrokeKind::Inside,
        );
        changed
    }
}

/// How a stroke's job shows on the canvas: the fill of a closed shape
/// (none for a drawn line or a separate unit, drawn as outlines), as the
/// repeat shows it.
fn job_fill(job: Job) -> Option<Color32> {
    match job {
        Job::Hole => Some(Color32::from_gray(120)),
        Job::Raised => Some(Color32::from_rgba_unmultiplied(120, 150, 110, 70)),
        Job::Recessed => Some(Color32::from_rgba_unmultiplied(60, 60, 60, 45)),
        _ => None,
    }
}

/// Fill the shape with corners `world` (any shape, not only convex ones)
/// as `to` places them on screen.
fn fill_shape(painter: &egui::Painter, world: &[DVec2], to: &dyn Fn(DVec2) -> Pos2, col: Color32) {
    if world.len() < 3 {
        return;
    }
    let mut mesh = egui::Mesh::default();
    for q in world {
        mesh.colored_vertex(to(*q), col);
    }
    for [a, b, c] in creabim_core::polygon::triangulate(world) {
        mesh.add_triangle(a as u32, b as u32, c as u32);
    }
    painter.add(egui::Shape::mesh(mesh));
}

/// How far the repeat runs round the tile in the Patterns area: two tiles
/// each way (five by five), however far out the view is zoomed (owner
/// 2026-11-18: never to infinity).
const REPEAT_REACH: f64 = 2.0;

impl PatternsUi {
    /// The Patterns area's canvas, as a Profile view: the drawn tile at
    /// the centre of the whole area, the grid, the zoom column, wheel and
    /// right-drag to zoom and pan; snaps to corners, the tile's edges and
    /// its neighbours' corners, with level, plumb and angle guides; the
    /// familiar keys (L, P, R, C, Esc, Delete, F, Shift+D). True when the
    /// tile changed.
    fn area_canvas(&mut self, ui: &mut egui::Ui, p: &mut HatchPattern, size: Vec2) -> bool {
        let mut changed = false;
        // a drag in progress shows as it goes
        if let (Some(l), Some(Recipe::Drawn(t))) = (&self.live, p.recipe.as_mut()) {
            *t = l.clone();
        }
        let ghost = p.clone();
        let Some(Recipe::Drawn(t)) = p.recipe.as_mut() else {
            return false;
        };
        let (resp, painter) = ui.allocate_painter(size, Sense::click_and_drag());
        let body = resp.rect;
        if ui.rect_contains_pointer(body) {
            self.hovered_pass = ui.ctx().cumulative_pass_nr();
        }
        let pal = crate::theme::Palette::of(ui.ctx());
        let [w, h] = t.size;
        let big = w.max(h).max(1e-3);
        let tile_c = DVec2::new(w / 2.0, h / 2.0);
        // the scale that shows `tiles` tiles across the shorter side
        let short = body.width().min(body.height()).max(80.0) as f64;
        let fit_k = move |tiles: f64| (short * 0.86 / (tiles * big)) as f32;
        let repeat = self.repeat;
        let fit = move || (tile_c, fit_k(if repeat { 3.0 } else { 1.3 }));
        // never further out than the repeat reaches, nor in past a
        // millimetre a dozen pixels
        let (k_min, k_max) = (fit_k(2.0 * REPEAT_REACH + 2.0), fit_k(1.0).max(12_000.0));
        let (mut center, mut scale) = self.cam.unwrap_or_else(fit);
        if resp.hovered() {
            let (pan, scroll) = crate::planview::wheel(ui);
            if pan != Vec2::ZERO {
                center -= DVec2::new(pan.x as f64, -pan.y as f64) / scale as f64;
            }
            if scroll != 0.0 {
                if let Some(hp) = resp.hover_pos() {
                    let off = DVec2::new(
                        (hp.x - body.center().x) as f64,
                        -(hp.y - body.center().y) as f64,
                    );
                    let at = center + off / scale as f64;
                    scale = (scale * (1.0 + scroll / 300.0).clamp(0.5, 2.0)).clamp(k_min, k_max);
                    center = at - off / scale as f64;
                }
            }
        }
        // Ctrl + middle-drag zooms, a right or middle drag pans (as Profiles)
        if resp.dragged_by(egui::PointerButton::Middle) && ui.input(|i| i.modifiers.command) {
            if let Some(hp) = ui.input(|i| i.pointer.press_origin()) {
                let off = DVec2::new(
                    (hp.x - body.center().x) as f64,
                    -(hp.y - body.center().y) as f64,
                );
                let at = center + off / scale as f64;
                scale = (scale * (1.0 - resp.drag_delta().y / 200.0).clamp(0.5, 2.0))
                    .clamp(k_min, k_max);
                center = at - off / scale as f64;
            }
        } else if resp.dragged_by(egui::PointerButton::Secondary)
            || resp.dragged_by(egui::PointerButton::Middle)
        {
            let d = resp.drag_delta();
            center -= DVec2::new(d.x as f64, -d.y as f64) / scale as f64;
        }
        // the zoom column's frame-selection: the picked stroke
        let fit_picked = self
            .stroke
            .and_then(|i| t.strokes.get(i))
            .filter(|s| !s.points.is_empty())
            .map(|s| {
                let lo = s
                    .points
                    .iter()
                    .fold(DVec2::splat(f64::MAX), |a, q| a.min(DVec2::from(*q)));
                let hi = s
                    .points
                    .iter()
                    .fold(DVec2::splat(f64::MIN), |a, q| a.max(DVec2::from(*q)));
                let span = (hi - lo).max(DVec2::splat(big / 50.0));
                let k = ((body.width() - 160.0).max(60.0) as f64 / span.x)
                    .min((body.height() - 120.0).max(60.0) as f64 / span.y);
                ((lo + hi) / 2.0, (k as f32).clamp(k_min, k_max))
            });
        let (nav, look) =
            crate::planview::nav_input(ui, body, ElementId::NIL, fit, fit_picked, center, scale);
        if let Some((c, k)) = nav {
            center = c;
            scale = k.clamp(k_min, k_max);
        }
        // F or the keypad's . frames the picked stroke, else the tile
        let keys_here = resp.hovered() && !ui.ctx().egui_wants_keyboard_input();
        if keys_here
            && ui.input(|i| {
                !i.modifiers.any()
                    && (i.key_pressed(egui::Key::F) || i.key_pressed(egui::Key::Period))
            })
        {
            (center, scale) = fit_picked.unwrap_or_else(fit);
        }
        // the view stays over the tile and its repeat
        let reach = DVec2::new(w, h) * (REPEAT_REACH + 1.0);
        center = center.clamp(tile_c - reach, tile_c + reach);
        self.cam = Some((center, scale));
        let to = |q: DVec2| {
            body.center()
                + Vec2::new(
                    ((q.x - center.x) * scale as f64) as f32,
                    (-(q.y - center.y) * scale as f64) as f32,
                )
        };
        let from = |s: Pos2| {
            center
                + DVec2::new(
                    (s.x - body.center().x) as f64,
                    -(s.y - body.center().y) as f64,
                ) / scale as f64
        };
        // the paper: the pattern's background, else the theme's
        let paper = ghost
            .fill
            .map_or(if pal.dark { pal.panel2 } else { Color32::WHITE }, |c| {
                c32(c.0)
            });
        painter.rect_filled(body, 0.0, paper);
        let clip = painter.with_clip_rect(body);
        // the grid: the snapping step, stronger every five, finer steps
        // left out when too close to read
        let step = snap_step(big);
        let (wlo, whi) = (from(body.left_bottom()), from(body.right_top()));
        let mut g = step;
        while g * (scale as f64) < 7.0 {
            g *= 5.0;
        }
        let thin = egui::Stroke::new(1.0, pal.border.gamma_multiply(0.45));
        let strong = egui::Stroke::new(1.0, pal.border);
        let mut x = (wlo.x / g).floor() * g;
        while x <= whi.x {
            let s = if ((x / g).round() as i64) % 5 == 0 {
                strong
            } else {
                thin
            };
            clip.line_segment([to(DVec2::new(x, wlo.y)), to(DVec2::new(x, whi.y))], s);
            x += g;
        }
        let mut y = (wlo.y / g).floor() * g;
        while y <= whi.y {
            let s = if ((y / g).round() as i64) % 5 == 0 {
                strong
            } else {
                thin
            };
            clip.line_segment([to(DVec2::new(wlo.x, y)), to(DVec2::new(whi.x, y))], s);
            y += g;
        }
        // the repeat, as the pattern lays it, five tiles by five at most
        let ink = Color32::from_gray(150);
        if self.repeat {
            let block_lo = -DVec2::new(w, h) * REPEAT_REACH;
            let block_hi = DVec2::new(w, h) * (REPEAT_REACH + 1.0);
            let lo = wlo.max(block_lo);
            let hi = whi.min(block_hi);
            if lo.x < hi.x && lo.y < hi.y {
                if let Some(l) = ghost.recipe.as_ref().and_then(|r| r.layout(lo, hi, 4_000)) {
                    let rc =
                        clip.with_clip_rect(Rect::from_two_pos(to(lo), to(hi)).intersect(body));
                    for (pts, job) in &l.shapes {
                        if let Some(f) = job_fill(*job) {
                            fill_shape(&rc, pts, &to, f);
                        }
                        rc.add(egui::Shape::closed_line(
                            pts.iter().map(|q| to(*q)).collect(),
                            egui::Stroke::new(1.0, ink),
                        ));
                    }
                    for (pts, job) in &l.lines {
                        let wdt = if *job == Job::Line { 1.0 } else { 2.0 };
                        rc.add(egui::Shape::line(
                            pts.iter().map(|q| to(*q)).collect(),
                            egui::Stroke::new(wdt, ink),
                        ));
                    }
                }
            }
        }
        // the tile
        let vis = ui.visuals().clone();
        let tile = Rect::from_two_pos(to(DVec2::ZERO), to(DVec2::new(w, h)));
        clip.rect_stroke(
            tile,
            0.0,
            egui::Stroke::new(1.5, vis.selection.bg_fill),
            egui::StrokeKind::Middle,
        );
        // the strokes, editable, filled as their job shows
        let accent = vis.selection.stroke.color;
        let dark = if pal.dark && ghost.fill.is_none() {
            Color32::from_gray(225)
        } else {
            Color32::from_gray(40)
        };
        for (i, s) in t.strokes.iter().enumerate() {
            let world: Vec<DVec2> = s.points.iter().map(|q| DVec2::from(*q)).collect();
            let ps: Vec<Pos2> = world.iter().map(|q| to(*q)).collect();
            let on = self.stroke == Some(i);
            if s.closed {
                if let Some(f) = job_fill(s.job) {
                    fill_shape(&clip, &world, &to, f);
                }
            }
            let st = egui::Stroke::new(if on { 2.5 } else { 1.6 }, if on { accent } else { dark });
            if s.closed {
                clip.add(egui::Shape::closed_line(ps.clone(), st));
            } else {
                clip.add(egui::Shape::line(ps.clone(), st));
            }
            if on && ps.len() <= 16 {
                for q in &ps {
                    let r = Rect::from_center_size(*q, Vec2::splat(7.0));
                    clip.rect_filled(r, 1.0, accent);
                    clip.rect_stroke(
                        r,
                        1.0,
                        egui::Stroke::new(1.0, dark),
                        egui::StrokeKind::Middle,
                    );
                }
            }
        }
        // snaps, magnet-like as a Profile view's: points first (corners,
        // the tile's corners, edge middles and centre, the neighbouring
        // tiles' copies of every corner, so a stroke joins its repeat),
        // then lines (level and plumb guides through corners, angles from
        // the point worked from, the strokes' edges), then the grid. Alt
        // holds them off, to the grid only.
        let free = ui.input(|i| i.modifiers.alt);
        let tol = 9.0 / scale as f64;
        let mut corners: Vec<DVec2> = vec![
            DVec2::ZERO,
            DVec2::new(w, 0.0),
            DVec2::new(0.0, h),
            DVec2::new(w, h),
            DVec2::new(w / 2.0, 0.0),
            DVec2::new(w / 2.0, h),
            DVec2::new(0.0, h / 2.0),
            DVec2::new(w, h / 2.0),
            tile_c,
        ];
        let skip = self.grip;
        let moving_i = self.moving.as_ref().map(|m| m.0);
        for (si, s) in t.strokes.iter().enumerate() {
            if Some(si) == moving_i {
                continue;
            }
            for (ci, q) in s.points.iter().enumerate() {
                if Some((si, ci)) == skip {
                    continue;
                }
                let q = DVec2::from(*q);
                for dx in [-1.0, 0.0, 1.0] {
                    for dy in [-1.0, 0.0, 1.0] {
                        corners.push(q + DVec2::new(dx * w, dy * h));
                    }
                }
            }
        }
        corners.extend(self.draft.iter().copied());
        let edges: Vec<(DVec2, DVec2)> = t
            .strokes
            .iter()
            .enumerate()
            .filter(|(si, _)| Some(*si) != moving_i)
            .flat_map(|(_, s)| {
                let n = s.points.len();
                let k = if s.closed { n } else { n.saturating_sub(1) };
                (0..k).map(move |e| (DVec2::from(s.points[e]), DVec2::from(s.points[(e + 1) % n])))
            })
            .chain([
                (DVec2::ZERO, DVec2::new(w, 0.0)),
                (DVec2::new(w, 0.0), DVec2::new(w, h)),
                (DVec2::new(w, h), DVec2::new(0.0, h)),
                (DVec2::new(0.0, h), DVec2::ZERO),
            ])
            .collect();
        let track_from: Option<DVec2> = match self.tool {
            DrawTool::Line | DrawTool::Shape => self.draft.last().copied(),
            DrawTool::Rectangle | DrawTool::Circle => self.press,
            DrawTool::Measure | DrawTool::Dimension => self.draft.first().copied(),
            DrawTool::Select | DrawTool::Offset => None,
        };
        let round = |v: f64| (v / step).round() * step;
        type Pick = Option<(f64, DVec2, Vec<(DVec2, DVec2)>)>;
        let snap = |q: Pos2| -> (DVec2, bool, Vec<(DVec2, DVec2)>) {
            let wq = from(q);
            if free {
                return (DVec2::new(round(wq.x), round(wq.y)), false, vec![]);
            }
            let take = |p: DVec2, temp: Vec<(DVec2, DVec2)>, best: &mut Pick| {
                let d = p.distance(wq);
                if d < tol && best.as_ref().is_none_or(|b| d < b.0) {
                    *best = Some((d, p, temp));
                }
            };
            let mut best: Pick = None;
            for c in &corners {
                take(*c, vec![], &mut best);
            }
            let near_x: Vec<DVec2> = corners
                .iter()
                .copied()
                .filter(|c| (c.x - wq.x).abs() < tol)
                .collect();
            let near_y: Vec<DVec2> = corners
                .iter()
                .copied()
                .filter(|c| (c.y - wq.y).abs() < tol)
                .collect();
            if best.is_none() {
                // where a plumb guide through one corner meets a level one
                for a in &near_x {
                    for b in &near_y {
                        let p = DVec2::new(a.x, b.y);
                        if p.distance(*a) > 1e-9 && p.distance(*b) > 1e-9 {
                            take(p, vec![(*a, p), (*b, p)], &mut best);
                        }
                    }
                }
            }
            if let Some((_, p, temp)) = best {
                return (p, true, temp);
            }
            let mut line: Pick = None;
            for c in &near_x {
                let p = DVec2::new(c.x, round(wq.y));
                take(p, vec![(*c, p)], &mut line);
            }
            for c in &near_y {
                let p = DVec2::new(round(wq.x), c.y);
                take(p, vec![(*c, p)], &mut line);
            }
            if let Some(f) = track_from {
                // every 15°, and parallel and square to every edge
                let mut dirs: Vec<DVec2> = (0..12)
                    .map(|k| DVec2::from_angle((k as f64 * 15.0).to_radians()))
                    .collect();
                for (a, b) in &edges {
                    let d = (*b - *a).normalize_or_zero();
                    dirs.push(d);
                    dirs.push(d.perp());
                }
                for d in dirs {
                    if d == DVec2::ZERO {
                        continue;
                    }
                    let tt = (wq - f).dot(d);
                    if (f + d * tt).distance(wq) < tol && tt.abs() > tol {
                        let p = f + d * round(tt);
                        take(p, vec![(f, p)], &mut line);
                    }
                }
            }
            if let Some((_, p, temp)) = line {
                return (p, true, temp);
            }
            // on an edge
            let mut on: Pick = None;
            for (a, b) in &edges {
                let ab = *b - *a;
                let tt = ((wq - *a).dot(ab) / ab.length_squared().max(1e-12)).clamp(0.0, 1.0);
                take(*a + ab * tt, vec![], &mut on);
            }
            if let Some((_, p, _)) = on {
                return (p, true, vec![]);
            }
            (DVec2::new(round(wq.x), round(wq.y)), false, vec![])
        };
        let hover = resp.hover_pos();
        let snapped = hover.map(snap);
        // a stroke under the pointer: on an edge, or inside a shape
        let outlines: Vec<(Vec<DVec2>, bool)> = t
            .strokes
            .iter()
            .map(|s| (s.points.iter().map(|q| DVec2::from(*q)).collect(), s.closed))
            .collect();
        let stroke_at = |at: Pos2| -> Option<usize> {
            let wq = from(at);
            outlines
                .iter()
                .enumerate()
                .rev()
                .find_map(|(i, (pts, closed))| {
                    let ps: Vec<Pos2> = pts.iter().map(|q| to(*q)).collect();
                    let n = ps.len();
                    let segs = if *closed { n } else { n.saturating_sub(1) };
                    let on_edge = (0..segs).any(|k| seg_dist(at, ps[k], ps[(k + 1) % n]) < 6.0);
                    let inside = *closed && creabim_core::polygon::contains(pts, wq);
                    (on_edge || inside).then_some(i)
                })
        };
        let keys = ui.input(|i| {
            (
                i.key_pressed(egui::Key::Enter),
                i.key_pressed(egui::Key::Escape),
                i.key_pressed(egui::Key::Delete),
                i.key_pressed(egui::Key::Backspace),
            )
        });
        // sizes typed while drawing, as the plan's drags take them (REV01,
        // C): a number goes into the chip at the pointer, Tab to the next,
        // Enter places it
        let (mut k_enter, mut k_esc, mut k_del, mut k_back) = keys;
        let started = match self.tool {
            DrawTool::Line | DrawTool::Shape | DrawTool::Measure | DrawTool::Dimension => {
                !self.draft.is_empty()
            }
            DrawTool::Rectangle | DrawTool::Circle => self.press.is_some(),
            DrawTool::Offset => self.offset_src.is_some(),
            DrawTool::Select => false,
        };
        let sizes = matches!(
            self.tool,
            DrawTool::Line
                | DrawTool::Shape
                | DrawTool::Rectangle
                | DrawTool::Circle
                | DrawTool::Offset
        );
        if !started || !sizes {
            self.typed = None;
        }
        if self
            .dim_edit
            .as_ref()
            .is_some_and(|(k, _)| *k >= t.dims.len())
        {
            self.dim_edit = None;
        }
        if let Some((di, buf)) = self.dim_edit.as_mut() {
            // a dimension's number being typed: Enter resizes what it measures
            take_typed(ui, buf, false);
            if k_back {
                buf.pop();
            }
            if k_enter {
                if let Some(v) = crate::units::parse(buf).filter(|v| *v > 1e-6) {
                    resize_dim(t, *di, v);
                    changed = true;
                }
                self.dim_edit = None;
            } else if k_esc {
                self.dim_edit = None;
            }
            (k_enter, k_esc, k_del, k_back) = (false, false, false, false);
        } else if started && sizes && keys_here {
            let ty = self
                .typed
                .get_or_insert_with(|| (0, [String::new(), String::new()]));
            if self.tool != DrawTool::Offset
                && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Tab))
            {
                ty.0 = 1 - ty.0;
            }
            let angle = ty.0 == 1 && matches!(self.tool, DrawTool::Line | DrawTool::Shape);
            take_typed(ui, &mut ty.1[ty.0], angle);
            if ty.1.iter().any(|v| !v.is_empty()) {
                if k_back {
                    ty.1[ty.0].pop();
                    k_back = false;
                }
                if k_esc {
                    self.typed = None;
                    k_esc = false;
                }
            }
        }
        let typing = self
            .typed
            .as_ref()
            .is_some_and(|t| t.1.iter().any(|v| !v.is_empty()));
        let place_typed = typing && k_enter && keys_here;
        if place_typed {
            k_enter = false;
        }
        let keys = (k_enter, k_esc, k_del, k_back);
        let pointer = snapped.as_ref().map(|s| s.0);
        // where the typed sizes put the next point (the pointer gives the
        // direction or the side); the pointer itself when nothing is typed
        let aim: Option<DVec2> = if !typing || self.tool == DrawTool::Offset {
            pointer
        } else {
            let v = self.typed.clone().map(|t| t.1).unwrap_or_default();
            let len = |s: &str| crate::units::parse(s).filter(|x| x.is_finite() && *x >= 0.0);
            match self.tool {
                DrawTool::Line | DrawTool::Shape => self.draft.last().map(|&f| {
                    let p = pointer.unwrap_or(f + DVec2::X);
                    let dir = match v[1].trim().parse::<f64>() {
                        Ok(a) => DVec2::from_angle(a.to_radians()),
                        Err(_) => (p - f).try_normalize().unwrap_or(DVec2::X),
                    };
                    f + dir * len(&v[0]).unwrap_or_else(|| (p - f).dot(dir).max(0.0))
                }),
                DrawTool::Rectangle => self.press.map(|a| {
                    let p = pointer.unwrap_or(a + DVec2::ONE);
                    let sx = if p.x < a.x { -1.0 } else { 1.0 };
                    let sy = if p.y < a.y { -1.0 } else { 1.0 };
                    let wv = len(&v[0]).unwrap_or((p.x - a.x).abs());
                    let hv = len(&v[1]).unwrap_or((p.y - a.y).abs());
                    a + DVec2::new(sx * wv, sy * hv)
                }),
                DrawTool::Circle => self.press.map(|a| {
                    let p = pointer.unwrap_or(a + DVec2::X);
                    let dir = (p - a).try_normalize().unwrap_or(DVec2::X);
                    a + dir
                        * len(&v[1])
                            .or(len(&v[0]).map(|d| d / 2.0))
                            .unwrap_or(p.distance(a))
                }),
                _ => pointer,
            }
        };
        // the tool keys, as in the drawings: L line, P shape, R rectangle,
        // C circle; Esc ends what is drawn, then goes back to Select
        if keys_here && !typing && self.dim_edit.is_none() {
            let pick = ui.input(|i| {
                if i.modifiers.any() {
                    return None;
                }
                [
                    (egui::Key::L, DrawTool::Line),
                    (egui::Key::P, DrawTool::Shape),
                    (egui::Key::R, DrawTool::Rectangle),
                    (egui::Key::C, DrawTool::Circle),
                    (egui::Key::V, DrawTool::Select),
                    (egui::Key::O, DrawTool::Offset),
                    (egui::Key::M, DrawTool::Measure),
                    (egui::Key::D, DrawTool::Dimension),
                ]
                .into_iter()
                .find(|(k, _)| i.key_pressed(*k))
                .map(|(_, t)| t)
            });
            if let Some(tool) = pick {
                // Offset starts from the stroke picked, when there is one
                self.offset_src = if tool == DrawTool::Offset {
                    self.stroke
                } else {
                    None
                };
                self.tool = tool;
                self.draft.clear();
                self.press = None;
                self.measured = None;
            }
            if keys.1 {
                if self.measured.is_some() {
                    self.measured = None;
                } else if self.offset_src.is_some() {
                    self.offset_src = None;
                } else if self.draft.is_empty() && self.press.is_none() {
                    self.tool = DrawTool::Select;
                    self.stroke = None;
                } else {
                    self.draft.clear();
                    self.press = None;
                }
            }
        }
        let push = |t: &mut DrawnTile,
                    pts: Vec<DVec2>,
                    closed: bool,
                    job: Job,
                    picked: &mut Option<usize>| {
            t.strokes.push(DrawnStroke {
                points: pts.into_iter().map(Into::into).collect(),
                closed,
                job,
                ..Default::default()
            });
            *picked = Some(t.strokes.len() - 1);
        };
        // the dimensions kept with the tile: drawn off what they measure,
        // their numbers to click and retype (REV01, D)
        let dim_ink = pal.accent;
        let mut dim_hits: Vec<(usize, Rect)> = vec![];
        for (k, [a, b]) in t.dims.iter().enumerate() {
            let (a, b) = (DVec2::from(*a), DVec2::from(*b));
            let (sa, sb) = (to(a), to(b));
            let u = (sb - sa).normalized();
            if !u.x.is_finite() {
                continue;
            }
            let n = Vec2::new(u.y, -u.x) * 16.0;
            let st = egui::Stroke::new(1.0, dim_ink);
            clip.line_segment([sa + n * 0.25, sa + n * 1.25], st);
            clip.line_segment([sb + n * 0.25, sb + n * 1.25], st);
            clip.line_segment([sa + n, sb + n], st);
            for e in [sa + n, sb + n] {
                let tk = (u + Vec2::new(-u.y, u.x)).normalized() * 4.0;
                clip.line_segment([e - tk, e + tk], egui::Stroke::new(1.4, dim_ink));
            }
            let editing = self
                .dim_edit
                .as_ref()
                .filter(|(i, _)| *i == k)
                .map(|(_, b)| format!("{b}|"));
            let text = editing
                .clone()
                .unwrap_or_else(|| crate::units::len(a.distance(b)));
            let ink = if editing.is_some() { pal.text } else { dim_ink };
            let g = clip.layout_no_wrap(text, egui::FontId::proportional(11.5), ink);
            let mid = sa + n * 1.9 + (sb - sa) / 2.0;
            let r = Rect::from_center_size(mid, g.size() + Vec2::new(8.0, 4.0));
            if editing.is_some() {
                clip.rect(
                    r,
                    4.0,
                    pal.panel2,
                    egui::Stroke::new(1.0, pal.accent),
                    egui::StrokeKind::Inside,
                );
            }
            clip.galley(r.min + Vec2::new(4.0, 2.0), g, ink);
            dim_hits.push((k, r));
        }
        // a right-click when nothing is being drawn: a menu for what is
        // under the pointer (REV01, B); while drawing it still finishes
        let idle =
            !started && self.grip.is_none() && self.moving.is_none() && self.dim_edit.is_none();
        if idle && resp.secondary_clicked() {
            if let Some(at) = resp.interact_pointer_pos() {
                let dim = dim_hits
                    .iter()
                    .find(|(_, r)| r.expand(3.0).contains(at))
                    .map(|d| d.0);
                let point = (0..t.strokes.len()).rev().find_map(|i| {
                    t.strokes[i]
                        .points
                        .iter()
                        .position(|q| to(DVec2::from(*q)).distance(at) < 8.0)
                        .map(|j| (i, j))
                });
                self.menu_on = Some(match (dim, point, stroke_at(at)) {
                    (Some(k), _, _) => MenuOn::Dim(k),
                    (_, Some((i, j)), _) => MenuOn::Point(i, j),
                    (_, _, Some(i)) => MenuOn::Stroke(i),
                    _ => MenuOn::Paper,
                });
                if let Some(MenuOn::Stroke(i) | MenuOn::Point(i, _)) = self.menu_on {
                    self.stroke = Some(i);
                }
            }
        }
        // a click on a dimension's number: type its length
        if self.tool == DrawTool::Select && resp.clicked() {
            if let Some(at) = resp.interact_pointer_pos() {
                if let Some((k, _)) = dim_hits.iter().find(|(_, r)| r.expand(3.0).contains(at)) {
                    self.dim_edit = Some((*k, String::new()));
                }
            }
        }
        let mut act: Option<MenuAct> = None;
        if idle {
            let on = self.menu_on;
            let tile_text = format!("Tile {} × {}", crate::units::len(w), crate::units::len(h));
            let (repeat_on, seen_s) = (
                self.repeat,
                if self.scale <= 0.0 { 20.0 } else { self.scale },
            );
            let has_dims = !t.dims.is_empty();
            let strokes = &t.strokes;
            resp.context_menu(|ui| {
                ui.set_min_width(200.0);
                let item = |ui: &mut egui::Ui, label: &str, key: &str, enabled: bool| {
                    ui.add_enabled(enabled, egui::Button::new(label).shortcut_text(key))
                        .clicked()
                };
                let head = |ui: &mut egui::Ui, text: String| {
                    ui.label(
                        egui::RichText::new(text.to_uppercase())
                            .size(10.5)
                            .strong()
                            .weak(),
                    );
                };
                match on {
                    Some(MenuOn::Stroke(i)) if i < strokes.len() => {
                        let s = &strokes[i];
                        let name = if s.name.is_empty() {
                            format!("{} {}", if s.closed { "Shape" } else { "Line" }, i + 1)
                        } else {
                            s.name.clone()
                        };
                        head(ui, format!("{name} · {}", s.job.label()));
                        if item(ui, "Offset…", "O", true) {
                            act = Some(MenuAct::Offset);
                        }
                        if item(ui, "Duplicate", "Shift+D", true) {
                            act = Some(MenuAct::Duplicate);
                        }
                        ui.menu_button("Job", |ui| {
                            let jobs: &[Job] = if s.closed { &Job::CLOSED } else { &Job::OPEN };
                            for j in jobs {
                                if ui.selectable_label(s.job == *j, j.label()).clicked() {
                                    act = Some(MenuAct::Job(*j));
                                }
                            }
                        });
                        ui.menu_button("Mirror across the tile", |ui| {
                            if ui.button("Left to right").clicked() {
                                act = Some(MenuAct::Mirror(true));
                            }
                            if ui.button("Top to bottom").clicked() {
                                act = Some(MenuAct::Mirror(false));
                            }
                        });
                        let open_close = if s.closed {
                            "Open the shape"
                        } else {
                            "Close the line"
                        };
                        if item(ui, open_close, "", s.closed || s.points.len() >= 3) {
                            act = Some(MenuAct::OpenClose);
                        }
                        ui.separator();
                        if item(ui, "Delete", "Del", true) {
                            act = Some(MenuAct::Delete);
                        }
                    }
                    Some(MenuOn::Point(i, j)) if i < strokes.len() => {
                        let s = &strokes[i];
                        let n = s.points.len();
                        head(ui, format!("Point {} of {n}", j + 1));
                        if item(ui, "Add a point after it", "", s.closed || j + 1 < n) {
                            act = Some(MenuAct::AddPoint);
                        }
                        if item(ui, "Delete the point", "", n > if s.closed { 3 } else { 2 }) {
                            act = Some(MenuAct::DeletePoint);
                        }
                        ui.separator();
                        if item(
                            ui,
                            "Split the line here",
                            "",
                            !s.closed && j > 0 && j + 1 < n,
                        ) {
                            act = Some(MenuAct::Split);
                        }
                    }
                    Some(MenuOn::Dim(_)) => {
                        head(ui, "Dimension".into());
                        if item(ui, "Type its length…", "", true) {
                            act = Some(MenuAct::DimType);
                        }
                        if item(ui, "Delete the dimension", "", true) {
                            act = Some(MenuAct::DimDelete);
                        }
                    }
                    _ => {
                        head(ui, tile_text);
                        if item(ui, "Repeat", if repeat_on { "on" } else { "off" }, true) {
                            act = Some(MenuAct::Repeat);
                        }
                        if item(ui, "Frame the tile", "F", true) {
                            act = Some(MenuAct::Frame);
                        }
                        if item(ui, &format!("Seen at 1:{seen_s}"), "Tab", true) {
                            act = Some(MenuAct::Seen);
                        }
                        ui.separator();
                        if item(ui, "Clear the dimensions", "", has_dims) {
                            act = Some(MenuAct::ClearDims);
                        }
                    }
                }
                if act.is_some() {
                    ui.close();
                }
            });
        }
        if let Some(a) = act {
            let tile_mid = DVec2::new(w, h) / 2.0;
            match (a, self.menu_on) {
                (MenuAct::Duplicate, Some(MenuOn::Stroke(i))) => {
                    if let Some(mut s) = t.strokes.get(i).cloned() {
                        let d = DVec2::splat(step * 2.0);
                        s.points = s
                            .points
                            .iter()
                            .map(|q| (DVec2::from(*q) + d).into())
                            .collect();
                        t.strokes.push(s);
                        self.stroke = Some(t.strokes.len() - 1);
                        changed = true;
                    }
                }
                (MenuAct::Delete, Some(MenuOn::Stroke(i))) if i < t.strokes.len() => {
                    t.strokes.remove(i);
                    self.stroke = None;
                    changed = true;
                }
                (MenuAct::Job(j), Some(MenuOn::Stroke(i))) => {
                    if let Some(s) = t.strokes.get_mut(i) {
                        s.job = j;
                        changed = true;
                    }
                }
                (MenuAct::Mirror(across_x), Some(MenuOn::Stroke(i))) => {
                    if let Some(s) = t.strokes.get_mut(i) {
                        for q in &mut s.points {
                            let v = DVec2::from(*q);
                            *q = if across_x {
                                [2.0 * tile_mid.x - v.x, v.y]
                            } else {
                                [v.x, 2.0 * tile_mid.y - v.y]
                            };
                        }
                        s.points.reverse();
                        changed = true;
                    }
                }
                (MenuAct::OpenClose, Some(MenuOn::Stroke(i))) => {
                    if let Some(s) = t.strokes.get_mut(i) {
                        s.closed = !s.closed;
                        s.job = if s.closed { Job::Unit } else { Job::Line };
                        changed = true;
                    }
                }
                (MenuAct::Offset, Some(MenuOn::Stroke(i))) => {
                    self.tool = DrawTool::Offset;
                    self.offset_src = Some(i);
                }
                (MenuAct::AddPoint, Some(MenuOn::Point(i, j))) => {
                    if let Some(s) = t.strokes.get_mut(i) {
                        let n = s.points.len();
                        let (a, b) = (DVec2::from(s.points[j]), DVec2::from(s.points[(j + 1) % n]));
                        s.points.insert(j + 1, ((a + b) / 2.0).into());
                        changed = true;
                    }
                }
                (MenuAct::DeletePoint, Some(MenuOn::Point(i, j))) => {
                    if let Some(s) = t.strokes.get_mut(i) {
                        if j < s.points.len() {
                            s.points.remove(j);
                            changed = true;
                        }
                    }
                }
                (MenuAct::Split, Some(MenuOn::Point(i, j))) => {
                    if let Some(s) = t.strokes.get_mut(i) {
                        let mut rest = s.clone();
                        rest.points = s.points[j..].to_vec();
                        s.points.truncate(j + 1);
                        t.strokes.push(rest);
                        changed = true;
                    }
                }
                (MenuAct::DimType, Some(MenuOn::Dim(k))) => {
                    self.dim_edit = Some((k, String::new()))
                }
                (MenuAct::DimDelete, Some(MenuOn::Dim(k))) if k < t.dims.len() => {
                    t.dims.remove(k);
                    changed = true;
                }
                (MenuAct::Repeat, _) => {
                    self.repeat = !self.repeat;
                    self.cam = None;
                }
                (MenuAct::Frame, _) => self.cam = None,
                (MenuAct::Seen, _) => self.seen = true,
                (MenuAct::ClearDims, _) => {
                    t.dims.clear();
                    changed = true;
                }
                _ => {}
            }
            self.menu_on = None;
        }
        match self.tool {
            DrawTool::Select => {
                if resp.drag_started_by(egui::PointerButton::Primary) {
                    if let Some(at) = resp.interact_pointer_pos() {
                        // a corner of the picked stroke first, then any
                        let order: Vec<usize> =
                            self.stroke.into_iter().chain(0..t.strokes.len()).collect();
                        self.grip = order.iter().find_map(|&i| {
                            t.strokes
                                .get(i)?
                                .points
                                .iter()
                                .position(|q| to(DVec2::from(*q)).distance(at) < 8.0)
                                .map(|j| (i, j))
                        });
                        if let Some((i, _)) = self.grip {
                            self.stroke = Some(i);
                        } else if let Some(i) = stroke_at(at) {
                            // the whole stroke moves, by its snapped grab point
                            self.stroke = Some(i);
                            self.moving = Some((i, snap(at).0, t.strokes[i].points.clone()));
                        }
                    }
                }
                if resp.dragged_by(egui::PointerButton::Primary) {
                    if let (Some((i, j)), Some(at)) = (self.grip, resp.interact_pointer_pos()) {
                        let q = snap(at).0;
                        if let Some(pt) = t.strokes.get_mut(i).and_then(|s| s.points.get_mut(j)) {
                            *pt = q.into();
                        }
                        self.live = Some(t.clone());
                    } else if let (Some((i, grab, before)), Some(at)) =
                        (self.moving.clone(), resp.interact_pointer_pos())
                    {
                        let d = snap(at).0 - grab;
                        if let Some(s) = t.strokes.get_mut(i) {
                            s.points = before
                                .iter()
                                .map(|q| (DVec2::from(*q) + d).into())
                                .collect();
                        }
                        self.live = Some(t.clone());
                    }
                }
                if resp.drag_stopped() {
                    self.grip = None;
                    self.moving = None;
                    if self.live.take().is_some() {
                        changed = true;
                    }
                }
                if resp.clicked() {
                    self.stroke = resp.interact_pointer_pos().and_then(stroke_at);
                }
                if keys_here && (keys.2 || keys.3) {
                    if let Some(i) = self.stroke.take() {
                        if i < t.strokes.len() {
                            t.strokes.remove(i);
                            changed = true;
                        }
                    }
                }
                // Shift+D: a copy of the picked stroke, a grid step along
                if keys_here
                    && ui.input(|i| {
                        i.modifiers.shift && !i.modifiers.command && i.key_pressed(egui::Key::D)
                    })
                {
                    if let Some(mut s) = self.stroke.and_then(|i| t.strokes.get(i).cloned()) {
                        let d = DVec2::splat(step * 2.0);
                        s.points = s
                            .points
                            .iter()
                            .map(|q| (DVec2::from(*q) + d).into())
                            .collect();
                        t.strokes.push(s);
                        self.stroke = Some(t.strokes.len() - 1);
                        changed = true;
                    }
                }
            }
            DrawTool::Line | DrawTool::Shape => {
                let closing = self.tool == DrawTool::Shape;
                if resp.clicked() {
                    if let Some(at) = resp.interact_pointer_pos() {
                        let q = snap(at).0;
                        if closing && self.draft.len() >= 3 && to(self.draft[0]).distance(at) < 8.0
                        {
                            let pts = std::mem::take(&mut self.draft);
                            push(t, pts, true, Job::Unit, &mut self.stroke);
                            changed = true;
                        } else if self.draft.last().is_some_and(|l| l.distance(q) < 1e-9)
                            && self.draft.len() >= 2
                        {
                            // the last point clicked again ends it
                            let pts = std::mem::take(&mut self.draft);
                            let closed = closing && pts.len() >= 3;
                            push(
                                t,
                                pts,
                                closed,
                                if closed { Job::Unit } else { Job::Line },
                                &mut self.stroke,
                            );
                            changed = true;
                        } else if self.draft.last().is_none_or(|l| l.distance(q) > 1e-9) {
                            self.draft.push(q);
                            self.typed = None;
                        }
                    }
                }
                // the typed length and angle: Enter places the point
                if place_typed {
                    if let Some(q) =
                        aim.filter(|q| self.draft.last().is_none_or(|l| l.distance(*q) > 1e-9))
                    {
                        self.draft.push(q);
                    }
                    self.typed = None;
                }
                let finish =
                    resp.double_clicked() || resp.secondary_clicked() || (keys.0 && keys_here);
                if finish && self.draft.len() >= 2 {
                    let pts = std::mem::take(&mut self.draft);
                    let closed = closing && pts.len() >= 3;
                    push(
                        t,
                        pts,
                        closed,
                        if closed { Job::Unit } else { Job::Line },
                        &mut self.stroke,
                    );
                    changed = true;
                }
                // Backspace takes the last point back
                if keys_here && keys.3 {
                    self.draft.pop();
                }
                if !self.draft.is_empty() {
                    let mut ps: Vec<Pos2> = self.draft.iter().map(|q| to(*q)).collect();
                    if let Some(q) = aim {
                        ps.push(to(q));
                    }
                    clip.add(egui::Shape::line(ps, egui::Stroke::new(1.5, accent)));
                }
            }
            DrawTool::Rectangle | DrawTool::Circle => {
                let tool = self.tool;
                let shape = |a: DVec2, b: DVec2| -> Vec<DVec2> {
                    if tool == DrawTool::Rectangle {
                        vec![a, DVec2::new(b.x, a.y), b, DVec2::new(a.x, b.y)]
                    } else {
                        let r = a.distance(b);
                        (0..32)
                            .map(|k| {
                                a + DVec2::from_angle(k as f64 / 32.0 * std::f64::consts::TAU) * r
                            })
                            .collect()
                    }
                };
                let ok = |a: DVec2, b: DVec2| {
                    let d = (b - a).abs();
                    if tool == DrawTool::Rectangle {
                        d.x > 1e-6 && d.y > 1e-6
                    } else {
                        d.length() > 1e-6
                    }
                };
                // a drag, or two clicks
                let mut end = None;
                if resp.drag_started_by(egui::PointerButton::Primary) {
                    self.press = resp.interact_pointer_pos().map(|p| snap(p).0);
                }
                if resp.drag_stopped() {
                    end = resp.interact_pointer_pos().map(|p| snap(p).0);
                }
                if resp.clicked() {
                    if let Some(q) = resp.interact_pointer_pos().map(|p| snap(p).0) {
                        if self.press.is_none() {
                            self.press = Some(q);
                        } else {
                            end = Some(q);
                        }
                    }
                }
                // the typed sizes: Enter places it
                if place_typed && self.press.is_some() {
                    end = aim;
                }
                if let Some(a) = self.press {
                    if let Some(b) = end {
                        if ok(a, b) {
                            let job = if tool == DrawTool::Circle {
                                Job::Hole
                            } else {
                                Job::Unit
                            };
                            push(t, shape(a, b), true, job, &mut self.stroke);
                            changed = true;
                        }
                        self.press = None;
                        self.typed = None;
                    } else if let Some(b) = aim {
                        let ps: Vec<Pos2> = shape(a, b).into_iter().map(to).collect();
                        clip.add(egui::Shape::closed_line(ps, egui::Stroke::new(1.5, accent)));
                    }
                }
            }
            DrawTool::Offset => {
                // first the stroke, then out or in and a click, or a typed
                // distance (REV01, E)
                if self.offset_src.is_some_and(|i| i >= t.strokes.len()) {
                    self.offset_src = None;
                }
                match self.offset_src {
                    None => {
                        if resp.clicked() {
                            if let Some(i) = resp.interact_pointer_pos().and_then(stroke_at) {
                                self.offset_src = Some(i);
                                self.stroke = Some(i);
                            }
                        }
                    }
                    Some(i) => {
                        let src = t.strokes[i].clone();
                        // the pointer's side and distance, a snap step
                        let raw = pointer.map_or(0.0, |p| side_distance(&src, p));
                        let typed_by = self
                            .typed
                            .as_ref()
                            .and_then(|t| crate::units::parse(&t.1[0]))
                            .filter(|v| v.is_finite());
                        let by = match typed_by {
                            Some(v) => v.abs() * if raw < 0.0 { -1.0 } else { 1.0 },
                            None => round(raw),
                        };
                        let made = (by.abs() > 1e-9).then(|| offset_stroke(&src, by)).flatten();
                        if let Some(o) = &made {
                            let ps: Vec<Pos2> =
                                o.points.iter().map(|q| to(DVec2::from(*q))).collect();
                            let st = egui::Stroke::new(1.5, accent);
                            if o.closed {
                                crate::snaps::dashed_clipped(
                                    &clip,
                                    &[ps.clone(), vec![ps[0]]].concat(),
                                    st,
                                    6.0,
                                    4.0,
                                );
                            } else {
                                crate::snaps::dashed_clipped(&clip, &ps, st, 6.0, 4.0);
                            }
                            if let Some(hp) = hover {
                                let text = format!("Offset {}", crate::units::len(by.abs()));
                                let g = clip.layout_no_wrap(
                                    text,
                                    egui::FontId::proportional(11.5),
                                    pal.text,
                                );
                                let r = Rect::from_min_size(
                                    hp + Vec2::new(14.0, 14.0),
                                    g.size() + Vec2::new(10.0, 6.0),
                                );
                                clip.rect(
                                    r,
                                    5.0,
                                    pal.panel2,
                                    egui::Stroke::new(
                                        1.0,
                                        if typing { pal.accent } else { pal.border },
                                    ),
                                    egui::StrokeKind::Inside,
                                );
                                clip.galley(r.min + Vec2::new(5.0, 3.0), g, pal.text);
                            }
                        }
                        if resp.clicked() || place_typed {
                            if let Some(o) = made {
                                t.strokes.push(o);
                                changed = true;
                                // Ctrl keeps going, from the new one
                                if ui.input(|i| i.modifiers.command) {
                                    self.offset_src = Some(t.strokes.len() - 1);
                                } else {
                                    self.offset_src = None;
                                }
                                self.stroke = Some(t.strokes.len() - 1);
                            }
                            self.typed = None;
                        }
                    }
                }
            }
            DrawTool::Measure | DrawTool::Dimension => {
                // two clicks; Measure keeps nothing, Dimension keeps it with
                // the tile (REV01, D)
                if resp.clicked() {
                    if let Some(q) = resp.interact_pointer_pos().map(|p| snap(p).0) {
                        match self.draft.first().copied() {
                            Some(a) => {
                                if a.distance(q) > 1e-9 {
                                    if self.tool == DrawTool::Measure {
                                        self.measured = Some((a, q));
                                    } else {
                                        t.dims.push([a.into(), q.into()]);
                                        changed = true;
                                    }
                                }
                                self.draft.clear();
                            }
                            None => {
                                self.draft.push(q);
                                self.measured = None;
                            }
                        }
                    }
                }
                let reading = match (self.draft.first().copied(), pointer) {
                    (Some(a), Some(b)) => Some((a, b)),
                    _ => self.measured.filter(|_| self.tool == DrawTool::Measure),
                };
                if let Some((a, b)) = reading {
                    let (sa, sb) = (to(a), to(b));
                    crate::snaps::dashed_clipped(
                        &clip,
                        &[sa, sb],
                        egui::Stroke::new(1.4, accent),
                        5.0,
                        4.0,
                    );
                    for e in [sa, sb] {
                        clip.circle_filled(e, 3.0, accent);
                    }
                    let d = b - a;
                    let text = if self.tool == DrawTool::Measure {
                        format!(
                            "{}  ·  Δx {}  ·  Δy {}  ·  {}°",
                            crate::units::len(d.length()),
                            crate::units::len(d.x.abs()),
                            crate::units::len(d.y.abs()),
                            fmt_angle(d.y.atan2(d.x).to_degrees())
                        )
                    } else {
                        crate::units::len(d.length())
                    };
                    let g = clip.layout_no_wrap(text, egui::FontId::proportional(11.5), pal.text);
                    let r = Rect::from_min_size(
                        sb + Vec2::new(12.0, 12.0),
                        g.size() + Vec2::new(10.0, 6.0),
                    );
                    clip.rect(
                        r,
                        5.0,
                        pal.panel2,
                        egui::Stroke::new(1.0, pal.border),
                        egui::StrokeKind::Inside,
                    );
                    clip.galley(r.min + Vec2::new(5.0, 3.0), g, pal.text);
                }
            }
        }
        // the sizes chip at the pointer while drawing: what is typed, or
        // what the pointer gives, muted (REV01, C)
        if started
            && matches!(
                self.tool,
                DrawTool::Line | DrawTool::Shape | DrawTool::Rectangle | DrawTool::Circle
            )
        {
            let base = match self.tool {
                DrawTool::Line | DrawTool::Shape => self.draft.last().copied(),
                _ => self.press,
            };
            if let (Some(b0), Some(q)) = (base, aim) {
                let d = q - b0;
                let (labels, live) = match self.tool {
                    DrawTool::Line | DrawTool::Shape => (
                        ["L", "∠"],
                        [
                            crate::units::len(d.length()),
                            format!("{}°", fmt_angle(d.y.atan2(d.x).to_degrees())),
                        ],
                    ),
                    DrawTool::Rectangle => (
                        ["W", "H"],
                        [crate::units::len(d.x.abs()), crate::units::len(d.y.abs())],
                    ),
                    _ => (
                        ["Ø", "r"],
                        [
                            crate::units::len(2.0 * d.length()),
                            crate::units::len(d.length()),
                        ],
                    ),
                };
                let (field, typed) = self
                    .typed
                    .clone()
                    .unwrap_or((0, [String::new(), String::new()]));
                let mut at = to(q) + Vec2::new(14.0, 14.0);
                for k in 0..2 {
                    let shown = if typed[k].is_empty() {
                        live[k].clone()
                    } else {
                        typed[k].clone()
                    };
                    let cursor = if k == field && typing { "|" } else { "" };
                    let text = format!("{} {shown}{cursor}", labels[k]);
                    let ink = if typed[k].is_empty() {
                        pal.muted
                    } else {
                        pal.text
                    };
                    let g = clip.layout_no_wrap(text, egui::FontId::proportional(11.5), ink);
                    let r = Rect::from_min_size(at, g.size() + Vec2::new(10.0, 6.0));
                    let edge = if k == field && typing {
                        pal.accent
                    } else {
                        pal.border
                    };
                    clip.rect(
                        r,
                        5.0,
                        pal.panel2,
                        egui::Stroke::new(1.0, edge),
                        egui::StrokeKind::Inside,
                    );
                    clip.galley(r.min + Vec2::new(5.0, 3.0), g, ink);
                    at.x = r.right() + 4.0;
                }
            }
        }
        // the snapped point, with the guides it follows dashed
        let drawing = self.tool != DrawTool::Select || self.grip.is_some() || self.moving.is_some();
        if let (Some((q, true, temp)), true) = (&snapped, drawing) {
            for (a, b) in temp {
                let (sa, sb) = (to(*a), to(*b));
                let u = (sb - sa).normalized();
                let more = if u.x.is_finite() {
                    u * 30.0
                } else {
                    Vec2::ZERO
                };
                crate::snaps::dashed_clipped(
                    &clip,
                    &[sa, sb + more],
                    egui::Stroke::new(1.1, crate::snaps::GUIDE),
                    5.0,
                    4.0,
                );
            }
            clip.rect_stroke(
                Rect::from_center_size(to(*q), Vec2::splat(10.0)),
                1.0,
                egui::Stroke::new(1.5, accent),
                egui::StrokeKind::Middle,
            );
        }
        // the tool's hint, at the foot
        clip.text(
            body.left_bottom() + Vec2::new(10.0, -8.0),
            egui::Align2::LEFT_BOTTOM,
            if started && matches!(self.tool, DrawTool::Line | DrawTool::Shape) {
                "Type a length  ·  Tab: angle  ·  Enter places the point  ·  Alt: no snaps"
                    .to_string()
            } else if started && self.tool == DrawTool::Rectangle {
                "Type the width  ·  Tab: height  ·  Enter places it  ·  Alt: no snaps".to_string()
            } else if started && self.tool == DrawTool::Circle {
                "Type the diameter  ·  Tab: radius  ·  Enter places it  ·  Alt: no snaps"
                    .to_string()
            } else if started && self.tool == DrawTool::Offset {
                "Out or in with the pointer  ·  type a distance  ·  Enter  ·  Ctrl keeps going"
                    .to_string()
            } else {
                format!("{}  ·  Alt: no snaps", self.tool.hint())
            },
            egui::FontId::proportional(12.0),
            pal.muted,
        );
        if let Some(look) = look {
            crate::planview::nav_draw(ui, &look);
        }
        if changed || self.live.is_some() {
            ui.ctx().request_repaint();
        }
        changed
    }
}

/// A free name like `base`: `base`, `base 2`…
pub(super) fn free_name(pack: &StylePack, base: &str) -> String {
    let taken = |n: &str| pack.pattern(n).is_some();
    if !taken(base) {
        return base.to_string();
    }
    (2..)
        .map(|i| format!("{base} {i}"))
        .find(|n| !taken(n))
        .unwrap_or_else(|| base.to_string())
}

enum PAsk {
    Commit(&'static str, Box<StylePack>),
    Rename(String, String),
    Import,
    Export,
    /// Open a material that uses the pattern in the Materials window.
    OpenMaterial(ElementId),
}

impl CreaBimApp {
    /// Open a pattern to draw and change: in the Patterns area (owner
    /// 2026-11-18: what you edit gets a viewing space), one already
    /// showing it, else the area in front turns into it.
    pub(super) fn open_patterns(&mut self, name: Option<String>) {
        use crate::areas::Shows;
        self.pat.tab = PatTab::Project;
        if let Some(i) = self
            .areas
            .root
            .areas()
            .iter()
            .position(|s| *s == Shows::Patterns)
        {
            self.areas.focus = i;
        } else {
            let i = self.areas.focus;
            if let Some(s) = self.areas.root.get_mut(i) {
                *s = Shows::Patterns;
            }
            self.store_areas();
        }
        if let Some(n) = name {
            self.pat.tab = PatTab::Project;
            self.pat.current = Some(n);
            self.pat.name_edit = None;
            self.pat.stroke = None;
            self.pat.draft.clear();
        }
    }

    /// The Patterns window, when open (or asked for).
    pub(super) fn patterns_window(&mut self, ctx: &egui::Context) {
        if let Some(req) = REQUEST.lock().ok().and_then(|mut r| r.take()) {
            self.open_patterns(req);
        }
        if !self.pat.open {
            return;
        }
        let mut open = true;
        let mut asks = vec![];
        let area = self.mat.area.unwrap_or_else(|| ctx.content_rect());
        let (_, pack) = self.edited_pack();
        let shown = crate::app::shell::window("Patterns")
            .open(&mut open)
            .default_pos(area.min + Vec2::new(48.0, 40.0))
            .constrain_to(area)
            .default_size([900.0, (area.height() - 60.0).clamp(420.0, 560.0)])
            .min_width(760.0)
            .collapsible(false)
            .show(ctx, |ui| {
                // one row: the project / library switch as a segmented
                // control, Find on the right (DS-01 4.8)
                crate::props::flow(ui, |ui| {
                    let h = ui.spacing().interact_size.y;
                    ui.allocate_ui(Vec2::new(220.0, h), |ui| {
                        crate::props::segment(
                            ui,
                            &mut self.pat.tab,
                            &[
                                (
                                    PatTab::Project,
                                    "In this project",
                                    "the patterns this project's style pack holds",
                                ),
                                (PatTab::Library, "Library", "patterns to add to the project"),
                            ],
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        crate::props::text_field(
                            ui,
                            egui::TextEdit::singleline(&mut self.pat.filter)
                                .hint_text("Find patterns"),
                            180.0,
                        );
                    });
                });
                ui.separator();
                // drawn in the room the window has, never growing it (MU62)
                crate::library::fit(ui, |ui| match self.pat.tab {
                    PatTab::Project => self.patterns_project(ui, &pack, &mut asks),
                    PatTab::Library => self.patterns_library(ui, &pack, &mut asks),
                });
            });
        // asked for from another window: in front of it
        if std::mem::take(&mut self.pat.raise) {
            if let Some(r) = shown {
                ctx.move_to_top(r.response.layer_id);
            }
        }
        self.pat.open = open;
        for a in asks {
            match a {
                PAsk::Commit(label, p) => {
                    let (id, _) = self.edited_pack();
                    self.commit_pattern(id, *p, label);
                }
                PAsk::Rename(old, new) => {
                    let (id, p) = self.edited_pack();
                    let tx = styles::rename_pattern(self.doc.model(), id, p, &old, &new);
                    let r = self.doc.commit(tx);
                    self.report(r, format!("Renamed pattern {old} to {new}."));
                    self.pat.current = Some(new);
                }
                PAsk::Import => self.import_pat_dialog(),
                PAsk::Export => self.export_pat_dialog(),
                PAsk::OpenMaterial(id) => self.open_materials(Some(id)),
            }
        }
    }

    /// The Patterns area: the project's patterns and the one open, its tile
    /// drawn as large as the area allows.
    pub(super) fn patterns_area(&mut self, ui: &mut egui::Ui, body: Rect) {
        let p = self.shell.theme.palette();
        ui.painter().rect_filled(body, 0.0, p.panel2);
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(body)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        child.spacing_mut().item_spacing = Vec2::ZERO;
        let (_, pack) = self.edited_pack();
        let mut asks = vec![];
        self.pat.part = Part::Canvas;
        crate::library::fit(&mut child, |ui| self.patterns_project(ui, &pack, &mut asks));
        self.pat.part = Part::All;
        self.patterns_asks(asks);
    }

    /// Is the pointer over the Patterns area (its keys win there)?
    pub(super) fn pattern_view_hovered(&self, ctx: &egui::Context) -> bool {
        let p = self.pat.hovered_pass;
        p > 0 && p + 1 >= ctx.cumulative_pass_nr()
    }

    /// Is a Patterns area the one in front?
    pub(super) fn patterns_in_front(&self) -> bool {
        matches!(
            self.areas.root.areas().get(self.areas.focus),
            Some(crate::areas::Shows::Patterns)
        )
    }

    /// Properties › Pattern: the open pattern's settings, as Profiles and
    /// Masters keep theirs (owner 2026-11-18: the area is where it is
    /// drawn and seen).
    pub(super) fn pattern_props(&mut self, ui: &mut egui::Ui) {
        let (_, pack) = self.edited_pack();
        let mut asks = vec![];
        self.pattern_props_ds(ui, &pack, &mut asks);
        self.patterns_asks(asks);
    }

    /// Properties › Pattern in the house layout (owner markup 2026-11-18:
    /// "polish and make DS style"): sections as a Profile's, one row a
    /// setting, labels in their column, nothing past the panel's edge.
    fn pattern_props_ds(&mut self, ui: &mut egui::Ui, pack: &StylePack, asks: &mut Vec<PAsk>) {
        let names = pack.pattern_names();
        let current = self
            .pat
            .current
            .clone()
            .filter(|c| pack.pattern(c).is_some())
            .or_else(|| pack.patterns.first().map(|p| p.name.clone()))
            .unwrap_or_else(|| Hatch::Brick.label().to_string());
        let mut edited = pack.clone();
        let mut label: &'static str = "";
        let users = styles::pattern_users(self.doc.model(), &current);
        let at = edited
            .patterns
            .iter()
            .position(|p| p.name.eq_ignore_ascii_case(&current));
        // the pattern: its name, what makes it, what uses it
        let open = self.prop_header(ui, "pat_about", "Pattern", true, None);
        let pat = &mut self.pat;
        crate::props::fold(ui, open, |ui| {
            match at {
                None => {
                    crate::props::readout(
                        ui,
                        "Name",
                        current.clone(),
                        "A built-in pattern keeps its name",
                    );
                    crate::props::readout(
                        ui,
                        "Made by",
                        "Built in",
                        "A symbolic hatch on paper, the same at every scale",
                    );
                }
                Some(i) => {
                    let p = &mut edited.patterns[i];
                    let (old, mut typed) = pat
                        .name_edit
                        .clone()
                        .filter(|(o, _)| o.eq_ignore_ascii_case(&p.name))
                        .unwrap_or_else(|| (p.name.clone(), p.name.clone()));
                    let r = crate::props::row(ui, "Name", |ui| {
                        crate::props::text_field(
                            ui,
                            egui::TextEdit::singleline(&mut typed),
                            f32::INFINITY,
                        )
                    });
                    let done = r.lost_focus();
                    pat.name_edit = Some((old.clone(), typed.clone()));
                    let typed = typed.trim().to_string();
                    if done && !typed.is_empty() && typed != old {
                        if !(pack.pattern(&typed).is_some() && !typed.eq_ignore_ascii_case(&old)) {
                            asks.push(PAsk::Rename(old.clone(), typed));
                        }
                        pat.name_edit = None;
                    }
                    let now = match &p.recipe {
                        None => "Line families",
                        Some(Recipe::Ashlar { .. }) => "Random stone",
                        Some(Recipe::Drawn(_)) => "Drawn tile",
                        Some(r) => r.label(),
                    };
                    crate::props::Combo::salted("Made by", "pat_made_by_ds")
                        .selected_text(now)
                        .show_ui(ui, |ui| {
                            for kind in MADE_BY {
                                if ui.selectable_label(kind == now, kind).clicked() && kind != now {
                                    match starting_recipe(kind) {
                                        Some(r) => {
                                            p.recipe = Some(r);
                                            p.units = PatternUnits::Model;
                                        }
                                        None => {
                                            p.recipe = None;
                                            if p.lines.is_empty() {
                                                p.lines =
                                                    styles::builtin_pattern(Hatch::Diagonal).lines;
                                                p.units = PatternUnits::Paper;
                                            }
                                        }
                                    }
                                    label = "Pattern recipe";
                                }
                            }
                        });
                }
            }
            match users.as_slice() {
                [] => crate::props::readout(
                    ui,
                    "Used by",
                    "No material yet",
                    "A material uses it when its cut or surface pattern is this one",
                ),
                [(id, name)] => {
                    if crate::props::relation(ui, "Used by", name, "Open it in Materials", None)
                        .select
                    {
                        asks.push(PAsk::OpenMaterial(*id));
                    }
                }
                many => {
                    let shown = format!("{} materials", many.len());
                    let all = many
                        .iter()
                        .map(|u| u.1.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    if crate::props::relation(ui, "Used by", &shown, &all, None).select {
                        asks.push(PAsk::OpenMaterial(many[0].0));
                    }
                }
            }
            if at.is_none() {
                crate::props::info(ui, "Built in: make an editable copy to change it. Saved under the same name, it replaces the built-in one for every material using it.", "");
                if crate::props::action(ui, "", "Make an editable copy", "", true).clicked() {
                    if let Some(p) = pack.pattern(&current) {
                        edited.patterns.push(p.into_owned());
                        label = "Edit built-in pattern";
                    }
                }
            }
        });
        if let Some(i) = at {
            let drawn = matches!(edited.patterns[i].recipe, Some(Recipe::Drawn(_)));
            // its size: the tile, the units, or the lines
            let title = match &edited.patterns[i].recipe {
                None => "Lines",
                Some(Recipe::Drawn(_)) => "Tile",
                Some(_) => "Units",
            };
            let open = self.prop_header(ui, "pat_size", title, true, None);
            crate::props::fold(ui, open, |ui| {
                let p = &mut edited.patterns[i];
                match p.recipe.as_mut() {
                    Some(r) => {
                        if recipe_fields(ui, r) {
                            label = "Pattern size";
                        }
                        if !r.is_valid() {
                            ui.colored_label(
                                ui.visuals().warn_fg_color,
                                "Some size is zero: nothing is drawn.",
                            );
                        }
                    }
                    None => {
                        if super::stylesui::line_families(ui, p) {
                            label = "Edit pattern lines";
                        }
                    }
                }
            });
            if edited.patterns[i].recipe.is_some() {
                // how it is laid on a face
                let open = self.prop_header(ui, "pat_lay", "Laying", true, None);
                crate::props::fold(ui, open, |ui| {
                    let p = &mut edited.patterns[i];
                    if crate::props::row(ui, "Angle", |ui| {
                        crate::props::fill(
                            ui,
                            egui::DragValue::new(&mut p.angle)
                                .range(-180.0..=180.0)
                                .speed(1.0)
                                .suffix("°"),
                        )
                        .changed()
                    }) {
                        label = "Pattern angle";
                    }
                    // both on/off settings as check rows, so their boxes
                    // line up (owner 2026-11-18: they didn't)
                    if crate::props::check(ui, "Courses", &mut p.along_longest, "Along longest side")
                        .on_hover_text("Courses run along each area's longest edge, from its end, instead of across the drawing: brick along a wall in plan")
                        .changed()
                    {
                        label = "Pattern direction";
                    }
                    let mut filled = p.fill.is_some();
                    if crate::props::check(ui, "Background", &mut filled, "Fill behind lines")
                        .on_hover_text("A colour behind the lines; without it the paper shows")
                        .changed()
                    {
                        p.fill = filled.then_some(creabim_core::styles::Rgb([0.85; 3]));
                        label = "Pattern background";
                    }
                    if let Some(c) = p.fill.as_mut() {
                        // the picker rounds through its own colour space:
                        // keep what it gives only when it was used
                        let mut edit = c.0;
                        if crate::props::row(ui, "Colour", |ui| {
                            crate::props::colour_button_rgb(ui, &mut edit).changed()
                        }) {
                            c.0 = edit;
                            label = "Pattern background";
                        }
                    }
                });
                // at small scales, a symbolic pattern stands in
                let open = self.prop_header(ui, "pat_small", "At small scales", false, None);
                crate::props::fold(ui, open, |ui| {
                    let p = &mut edited.patterns[i];
                    let mut on = p.small_scale.is_some();
                    if crate::props::switch(ui, &mut on, "Simplify")
                        .on_hover_text("Drawings smaller than a scale draw a symbolic pattern instead: coursing at 1:20, the brick hatch at 1:100")
                        .changed()
                    {
                        p.small_scale = on.then(|| SmallScale { below: 50.0, pattern: Hatch::Brick.label().to_string() });
                        label = "Pattern at small scales";
                    }
                    let mut s = p.small_scale.clone().unwrap_or(SmallScale {
                        below: 50.0,
                        pattern: Hatch::Brick.label().to_string(),
                    });
                    let mut moved = false;
                    ui.add_enabled_ui(on, |ui| {
                        moved |= crate::props::row(ui, "From 1:", |ui| {
                            crate::props::fill(
                                ui,
                                egui::DragValue::new(&mut s.below)
                                    .range(2.0..=5000.0)
                                    .speed(1.0),
                            )
                            .changed()
                        });
                        crate::props::Combo::salted("Draw as", "pat_small_ds")
                            .selected_text(s.pattern.clone())
                            .show_ui(ui, |ui| {
                                for n in names.iter().filter(|n| !n.eq_ignore_ascii_case(&current))
                                {
                                    if ui
                                        .selectable_label(s.pattern.eq_ignore_ascii_case(n), n)
                                        .clicked()
                                    {
                                        s.pattern = n.clone();
                                        moved = true;
                                    }
                                }
                            });
                    });
                    if on && moved {
                        p.small_scale = Some(s);
                        label = "Pattern at small scales";
                    }
                });
            }
            if drawn {
                let n = match &edited.patterns[i].recipe {
                    Some(Recipe::Drawn(t)) => t.strokes.len(),
                    _ => 0,
                };
                let open =
                    self.prop_header(ui, "pat_strokes", &format!("Strokes  {n}"), true, None);
                let pat = &mut self.pat;
                crate::props::fold(ui, open, |ui| {
                    if let Some(Recipe::Drawn(t)) = edited.patterns[i].recipe.as_mut() {
                        if let Some(what) = stroke_rows(ui, t, &mut pat.stroke) {
                            label = what;
                        }
                    }
                });
            }
        }
        // as it draws, at two scales, side by side across the panel
        let open = self.prop_header(ui, "pat_draws", "As it draws", true, None);
        let shown_p = at
            .map(|i| edited.patterns[i].clone())
            .or_else(|| pack.pattern(&current).map(|p| p.into_owned()));
        let (seen_at, seen) = (
            self.pat.scale,
            self.pat.seen || !self.pattern_open_is_drawn(),
        );
        let mut look_at = None;
        crate::props::fold(ui, open, |ui| {
            let Some(p) = shown_p else { return };
            // a pair across the whole width: squares when narrow, wider than
            // tall as the panel grows, never over 2:1 or 120 px tall
            // (REV01, F; owner: it adapts to the panel)
            let gap = 6.0;
            let w = ((ui.available_width() - gap) / 2.0).max(40.0);
            let h = w.min(120.0).max(w / 2.0);
            // the same paper on both, at their scales: 20 mm across 110 px
            let paper_mm = 20.0 * (w as f64 / 110.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for sc in [20.0, 100.0] {
                    let (r, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
                    let sym = p.small_scale_name(sc).and_then(|n| pack.pattern(n));
                    let shown = sym.clone().map_or_else(|| p.clone(), |s| s.into_owned());
                    let big = r.width().max(r.height());
                    let sq = Rect::from_min_size(r.min, Vec2::splat(big));
                    let side = paper_mm * (big / w) as f64 * sc / 1000.0;
                    paint_pattern(&ui.painter().with_clip_rect(r), sq, &shown, sc, side);
                    // the caption inside the picture: the scale, and the
                    // symbol standing in when there is one
                    let v = ui.visuals().clone();
                    let band = Rect::from_min_max(egui::pos2(r.left(), r.bottom() - 18.0), r.max);
                    ui.painter()
                        .rect_filled(band, 0.0, v.panel_fill.gamma_multiply(0.85));
                    let lit = seen && (seen_at - sc).abs() < 0.5;
                    let right = match &sym {
                        Some(s) => s.name.clone(),
                        None if lit => "seen".into(),
                        None => String::new(),
                    };
                    let font = egui::FontId::proportional(11.0);
                    ui.painter().text(
                        band.left_center() + Vec2::new(6.0, 0.0),
                        egui::Align2::LEFT_CENTER,
                        format!("1:{sc}"),
                        font.clone(),
                        v.text_color(),
                    );
                    if !right.is_empty() {
                        let g = ui.painter().layout(
                            right,
                            font,
                            v.weak_text_color(),
                            band.width() - 46.0,
                        );
                        ui.painter().galley(
                            egui::pos2(
                                band.right() - 6.0 - g.size().x,
                                band.center().y - g.size().y / 2.0,
                            ),
                            g,
                            v.weak_text_color(),
                        );
                    }
                    let edge = if lit {
                        egui::Stroke::new(2.0, v.selection.bg_fill)
                    } else {
                        v.widgets.noninteractive.bg_stroke
                    };
                    ui.painter()
                        .rect_stroke(r, 4.0, edge, egui::StrokeKind::Inside);
                    if resp
                        .on_hover_text(format!("Show it in the area, seen at 1:{sc}"))
                        .clicked()
                    {
                        look_at = Some(sc);
                    }
                }
            });
        });
        if let Some(sc) = look_at {
            self.pat.scale = sc;
            self.pat.seen = true;
            self.pat.draft.clear();
        }
        if !label.is_empty() && edited != *pack {
            asks.push(PAsk::Commit(label, Box::new(edited)));
        }
    }

    /// Is the open pattern drawn (its tile tools apply)?
    fn pattern_open_is_drawn(&self) -> bool {
        let (_, pack) = self.edited_pack();
        self.pat
            .current
            .as_ref()
            .and_then(|c| {
                pack.patterns
                    .iter()
                    .find(|p| p.name.eq_ignore_ascii_case(c))
            })
            .is_some_and(|p| matches!(p.recipe, Some(Recipe::Drawn(_))))
    }

    /// The Patterns area's header, in the owner's order (Mockup_Patterns_
    /// Area_REV01, A): after the type and the pattern's name, + (a click
    /// makes a drawn tile; its arrow the other kinds and what to do with
    /// this pattern), the mode (Edit or Seen at a scale, as a view's
    /// Object/Edit), then on a drawn tile in Edit its tools, Repeat and,
    /// with a stroke picked, its trash.
    pub(super) fn patterns_controls(&self) -> Vec<serde_json::Value> {
        use serde_json::json;
        let mut v = vec![
            json!({"kind": "split", "id": "pat.newkind:Drawn tile", "key": "pat.new", "icon": "plus",
            "label": "New drawn tile",
            "hint": "A click makes a new drawn tile; the arrow: the other kinds, duplicate, delete, import, export"}),
        ];
        let drawn = self.pattern_open_is_drawn();
        let editing = drawn && !self.pat.seen;
        let s = if self.pat.scale <= 0.0 {
            20.0
        } else {
            self.pat.scale
        };
        v.push(json!({"kind": "menu", "key": "pat.mode", "icon": "", "label": "Edit or Seen",
            "value": if editing { "Edit".to_string() } else { format!("Seen at 1:{s}") },
            "hint": "Edit draws on the tile, square; Seen shows the pattern laid, at its angle, at a scale. Tab switches"}));
        if editing {
            v.push(json!({"kind": "sep"}));
            for (t, icon) in [
                (DrawTool::Select, "mouse-pointer-2"),
                (DrawTool::Line, "slash"),
                (DrawTool::Shape, "pentagon"),
                (DrawTool::Rectangle, "square"),
                (DrawTool::Circle, "circle"),
                (DrawTool::Offset, ""),
                (DrawTool::Measure, "ruler"),
                (DrawTool::Dimension, "dimension"),
            ] {
                // the exact-size tools after the drawing ones (REV01, A)
                if t == DrawTool::Offset {
                    v.push(json!({"kind": "sep"}));
                }
                let icon = if icon.is_empty() { "offset" } else { icon };
                v.push(json!({"kind": "toggle", "id": format!("pat.tool:{}", t.label().to_lowercase()), "icon": icon,
                    "label": format!("{} ({})", t.label(), t.key()), "on": self.pat.tool == t, "hint": t.hint()}));
            }
            v.push(json!({"kind": "sep"}));
            v.push(json!({"kind": "toggle", "id": "pat.repeat", "icon": "grid-3x3", "label": "Repeat", "on": self.pat.repeat,
                "hint": "Show how the tile repeats round it, two tiles each way (off: the tile alone, to design it)"}));
            if self.pat.stroke.is_some() {
                v.push(json!({"kind": "button", "id": "pat.strokedel", "icon": "trash-2", "label": "Delete the picked stroke",
                    "hint": "Or press Delete"}));
            }
        }
        v
    }

    /// The Patterns area's menus: its name lists the project's patterns.
    pub(super) fn patterns_menu(&self, key: &str) -> Option<Vec<crate::ui_model::Item>> {
        use crate::ui_model::Item;
        let (_, pack) = self.edited_pack();
        Some(match key {
            "pat.pick" => {
                let cur = self.pat.current.clone().unwrap_or_default();
                let mut items = vec![];
                for (head, own) in [("This project", true), ("Built in", false)] {
                    items.push(Item::Header(head.into()));
                    for n in pack.pattern_names() {
                        let is_own = pack
                            .patterns
                            .iter()
                            .any(|p| p.name.eq_ignore_ascii_case(&n));
                        if is_own == own {
                            items.push(
                                Item::new(format!("pat.open:{n}"), &n)
                                    .checked(n.eq_ignore_ascii_case(&cur)),
                            );
                        }
                    }
                }
                items
            }
            "pat.new" => {
                // grouped and explained, your own first (REV01, A)
                let kind =
                    |k: &str, hint: &str| Item::new(format!("pat.newkind:{k}"), k).hint(hint);
                let own = pack.patterns.iter().any(|p| {
                    self.pat
                        .current
                        .as_deref()
                        .is_some_and(|c| p.name.eq_ignore_ascii_case(c))
                });
                vec![
                    Item::Header("Your own".into()),
                    kind("Drawn tile", "Draw one tile; it repeats. A breeze block, a cast motif"),
                    Item::Header("Lines".into()),
                    kind("Line families", "Sets of parallel lines, as CAD .pat hatches; imported .pat files land here"),
                    Item::Header("Laid units, true size".into()),
                    kind("Bond", "Brick and block courses"),
                    kind("Herringbone", "Paving, parquet"),
                    kind("Basketweave", "Paving, parquet"),
                    kind("Boards", "Decking, floorboards, cladding"),
                    kind("Tiles", "Floor and wall tiles"),
                    kind("Hexagons", "Hex tiles and pavers"),
                    kind("Random stone", "Rubble walls, flagstones"),
                    Item::Separator,
                    Item::new("pat.dup", "Duplicate this pattern").icon("copy").enabled(own),
                    Item::new("pat.delete", "Delete this pattern")
                        .icon("trash-2")
                        .enabled(own)
                        .hint("Only when no material uses it"),
                    Item::Separator,
                    Item::new("pat.import", "Import .pat…").icon("download").hint("Hatch patterns from AutoCAD or Revit"),
                    Item::new("pat.export", "Export .pat…")
                        .icon("upload")
                        .hint("The project's patterns that can be written as lines"),
                    Item::new("libraries", "Library…").icon("library").hint("More patterns to add"),
                ]
            }
            "pat.mode" => {
                let s = if self.pat.scale <= 0.0 {
                    20.0
                } else {
                    self.pat.scale
                };
                let drawn = self.pattern_open_is_drawn();
                let mut items = vec![Item::new("pat.edit", "Edit the tile")
                    .checked(drawn && !self.pat.seen)
                    .enabled(drawn)
                    .hint(if drawn {
                        "Draw on the tile, square"
                    } else {
                        "Only a drawn tile is drawn on"
                    })];
                items.push(Item::Header("Seen at".into()));
                for k in [20.0, 50.0, 100.0] {
                    items.push(
                        Item::new(format!("pat.scale:{k}"), format!("1:{k}"))
                            .checked((!drawn || self.pat.seen) && (s - k).abs() < 0.5),
                    );
                }
                items
            }
            _ => return None,
        })
    }

    /// The Patterns area's buttons.
    pub(super) fn patterns_action(&mut self, head: &str, arg: &str) -> bool {
        match head {
            "pat.tool" => {
                if let Some(t) = DrawTool::ALL
                    .into_iter()
                    .find(|t| t.label().eq_ignore_ascii_case(arg))
                {
                    self.pat.offset_src = if t == DrawTool::Offset {
                        self.pat.stroke
                    } else {
                        None
                    };
                    self.pat.measured = None;
                    self.pat.tool = t;
                    self.pat.draft.clear();
                }
            }
            "pat.scale" => {
                self.pat.scale = arg.parse().unwrap_or(20.0);
                self.pat.seen = true;
                self.pat.draft.clear();
            }
            "pat.edit" => self.pat.seen = false,
            "pat.strokedel" => {
                let (id, mut pack) = self.edited_pack();
                let cur = self.pat.current.clone().unwrap_or_default();
                let picked = self.pat.stroke.take();
                if let (Some(i), Some(Recipe::Drawn(t))) = (
                    picked,
                    pack.patterns
                        .iter_mut()
                        .find(|p| p.name.eq_ignore_ascii_case(&cur))
                        .and_then(|p| p.recipe.as_mut()),
                ) {
                    if i < t.strokes.len() {
                        t.strokes.remove(i);
                        self.commit_pattern(id, pack, "Delete stroke");
                    }
                }
            }
            "pat.repeat" => {
                self.pat.repeat = !self.pat.repeat;
                // the view fits again to what is shown
                self.pat.cam = None;
            }
            "pat.fit" => self.pat.cam = None,
            "pat.open" => {
                self.pat.cam = None;
                self.pat.current = Some(arg.to_string());
                self.pat.seen = false;
                self.pat.name_edit = None;
                self.pat.stroke = None;
                self.pat.draft.clear();
            }
            "pat.newkind" => {
                let (id, mut pack) = self.edited_pack();
                let name = free_name(
                    &pack,
                    if arg == "Drawn tile" {
                        "Drawn pattern"
                    } else {
                        arg
                    },
                );
                let p = match starting_recipe(arg) {
                    Some(r) => HatchPattern::from_recipe(&name, r),
                    None => {
                        let mut p = styles::builtin_pattern(Hatch::Diagonal);
                        p.name = name.clone();
                        p
                    }
                };
                pack.patterns.push(p);
                self.pat.current = Some(name);
                self.pat.seen = false;
                if arg == "Drawn tile" {
                    // Select first, as everywhere (owner 2026-11-18)
                    self.pat.tool = DrawTool::Select;
                }
                self.commit_pattern(id, pack, "New pattern");
            }
            "pat.dup" => {
                let (id, mut pack) = self.edited_pack();
                let cur = self.pat.current.clone().unwrap_or_default();
                if let Some(mut p) = pack
                    .patterns
                    .iter()
                    .find(|p| p.name.eq_ignore_ascii_case(&cur))
                    .cloned()
                {
                    p.name = free_name(&pack, &format!("{} copy", p.name));
                    self.pat.current = Some(p.name.clone());
                    pack.patterns.push(p);
                    self.commit_pattern(id, pack, "Duplicate pattern");
                } else {
                    self.status =
                        "A built-in pattern: Make an editable copy in Properties first.".into();
                }
            }
            "pat.delete" => {
                let (id, mut pack) = self.edited_pack();
                let cur = self.pat.current.clone().unwrap_or_default();
                let used = styles::pattern_users(self.doc.model(), &cur).len();
                match pack
                    .patterns
                    .iter()
                    .position(|p| p.name.eq_ignore_ascii_case(&cur))
                {
                    None => self.status = "Built-in patterns stay.".into(),
                    Some(_) if used > 0 => {
                        self.status = format!(
                            "{used} material(s) use {cur}: give them another pattern first."
                        )
                    }
                    Some(i) => {
                        pack.patterns.remove(i);
                        self.pat.current = None;
                        self.commit_pattern(id, pack, "Delete pattern");
                    }
                }
            }
            "pat.import" => self.import_pat_dialog(),
            "pat.export" => self.export_pat_dialog(),
            _ => return false,
        }
        true
    }

    /// Save an edit of the patterns. Only dragging or typing a value folds
    /// into the last undo step; a stroke drawn, moved or deleted, a job
    /// picked, a pattern added or removed is a step of its own (owner
    /// 2026-11-18: undo didn't bring a deleted stroke back, it had been
    /// folded into the whole drawing session).
    fn commit_pattern(&mut self, id: Option<ElementId>, pack: StylePack, label: &str) {
        const FOLDS: [&str; 8] = [
            "Pattern background",
            "Pattern size",
            "Pattern angle",
            "Edit pattern lines",
            "Pattern recipe",
            "Stroke depth",
            "Stroke name",
            "Pattern at small scales",
        ];
        let Some(id) = id.filter(|_| !FOLDS.contains(&label)) else {
            self.commit_pack(id, pack, label);
            return;
        };
        let r = self.doc.edit(id, label, |e| {
            e.name = pack.name.clone();
            e.kind = ElementKind::StylePack(Box::new(pack.clone()));
        });
        self.report(r, format!("{label}."));
    }

    /// The area's name in its header: the pattern open.
    pub(super) fn patterns_title(&self) -> String {
        self.pat
            .current
            .clone()
            .unwrap_or_else(|| "Patterns".into())
    }

    fn patterns_asks(&mut self, asks: Vec<PAsk>) {
        for a in asks {
            match a {
                PAsk::Commit(label, p) => {
                    let (id, _) = self.edited_pack();
                    self.commit_pattern(id, *p, label);
                }
                PAsk::Rename(old, new) => {
                    let (id, p) = self.edited_pack();
                    let tx = styles::rename_pattern(self.doc.model(), id, p, &old, &new);
                    let r = self.doc.commit(tx);
                    self.report(r, format!("Renamed pattern {old} to {new}."));
                    self.pat.current = Some(new);
                }
                PAsk::Import => self.import_pat_dialog(),
                PAsk::Export => self.export_pat_dialog(),
                PAsk::OpenMaterial(id) => self.open_materials(Some(id)),
            }
        }
    }

    fn patterns_project(&mut self, ui: &mut egui::Ui, pack: &StylePack, asks: &mut Vec<PAsk>) {
        let f = self.pat.filter.trim().to_lowercase();
        let names = pack.pattern_names();
        let current = self
            .pat
            .current
            .clone()
            .filter(|c| pack.pattern(c).is_some())
            .or_else(|| pack.patterns.first().map(|p| p.name.clone()))
            .unwrap_or_else(|| Hatch::Brick.label().to_string());
        if self.pat.current.as_deref() != Some(current.as_str()) {
            self.pat.current = Some(current.clone());
        }
        let height = ui.available_height().max(380.0);
        let mut edited = pack.clone();
        let mut label: &'static str = "";
        let part = self.pat.part;
        ui.horizontal_top(|ui| {
            // the list (the area's is its name's menu)
            if part == Part::All { ui.vertical(|ui| {
                ui.set_width(220.0);
                egui::ScrollArea::vertical()
                    .id_salt("pat_list")
                    .max_height(height - 40.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let group = |n: &str| -> &'static str {
                            match pack.patterns.iter().find(|p| p.name.eq_ignore_ascii_case(n)) {
                                None => "Built in",
                                Some(p) => match &p.recipe {
                                    Some(Recipe::Drawn(_)) => "Drawn",
                                    Some(_) => "Generated",
                                    None => "Line patterns",
                                },
                            }
                        };
                        for g in ["Generated", "Drawn", "Line patterns", "Built in"] {
                            let rows: Vec<&String> = names
                                .iter()
                                .filter(|n| group(n) == g && (f.is_empty() || n.to_lowercase().contains(&f)))
                                .collect();
                            if rows.is_empty() {
                                continue;
                            }
                            // the library's accordion (MU42); a search shows
                            // every group's matches
                            let open = crate::library::group(ui, "patterns", g, rows.len());
                            // the rows fold smoothly (AMD-017); a search shows them all
                            crate::props::fold(ui, open || !f.is_empty(), |ui| {
                            for n in rows {
                                let Some(p) = pack.pattern(n) else { continue };
                                let users = styles::pattern_users(self.doc.model(), n).len();
                                let sp = p.into_owned();
                                let sc = swatch_scale(&sp);
                                // its swatch on paper, as it draws
                                let mark = move |pa: &egui::Painter, r: egui::Rect| {
                                    pa.rect_filled(r, 2.0, Color32::WHITE);
                                    paint_pattern(pa, r, &sp, sc, 20.0 / (1000.0 / sc));
                                    pa.rect_stroke(r, 2.0, egui::Stroke::new(1.0, Color32::from_black_alpha(60)), egui::StrokeKind::Inside);
                                };
                                let note = if users > 0 { users.to_string() } else { String::new() };
                                if crate::library::item(ui, n.eq_ignore_ascii_case(&current), n, &note, Some(&mark))
                                    .on_hover_text(match users {
                                        0 => "No material uses it".to_string(),
                                        1 => "Used by 1 material".to_string(),
                                        k => format!("Used by {k} materials"),
                                    })
                                    .clicked()
                                {
                                    self.pat.current = Some(n.clone());
                                    self.pat.name_edit = None;
                                    self.pat.stroke = None;
                                    self.pat.draft.clear();
                                }
                            }
                            });
                        }
                    });
                crate::library::bar(ui, |ui| {
                    let new = crate::library::new_button(ui, "A new pattern: pick what it is made by");
                    egui::Popup::menu(&new).show(|ui| {
                        for kind in MADE_BY {
                            if ui.button(kind).clicked() {
                                let name = free_name(&edited, if kind == "Drawn tile" { "Drawn pattern" } else { kind });
                                let p = match starting_recipe(kind) {
                                    Some(r) => HatchPattern::from_recipe(&name, r),
                                    None => {
                                        let mut p = styles::builtin_pattern(Hatch::Diagonal);
                                        p.name = name.clone();
                                        p
                                    }
                                };
                                edited.patterns.push(p);
                                self.pat.current = Some(name);
                                if kind == "Drawn tile" {
                                    self.pat.tool = DrawTool::Shape;
                                }
                                label = "New pattern";
                                ui.close();
                            }
                        }
                    });
                    let at = edited.patterns.iter().position(|p| p.name.eq_ignore_ascii_case(&current));
                    if crate::library::duplicate_button(ui, at.is_some())
                        .on_disabled_hover_text("A built-in pattern: Make an editable copy first")
                        .clicked()
                    {
                        if let Some(i) = at {
                            let mut p = edited.patterns[i].clone();
                            p.name = free_name(&edited, &format!("{} copy", p.name));
                            self.pat.current = Some(p.name.clone());
                            edited.patterns.push(p);
                            label = "Duplicate pattern";
                        }
                    }
                    let used = styles::pattern_users(self.doc.model(), &current).len();
                    let why = if at.is_none() {
                        "Built-in patterns stay"
                    } else if used > 0 {
                        "Materials use it: give them another pattern first"
                    } else {
                        "This pattern"
                    };
                    if crate::library::delete_button(ui, at.is_some() && used == 0, why).clicked() {
                        if let Some(i) = at {
                            edited.patterns.remove(i);
                            self.pat.current = None;
                            label = "Delete pattern";
                        }
                    }
                    // what brings patterns in and takes them out
                    crate::library::right(ui, |ui| {
                        if crate::props::icon_button(
                            ui,
                            "upload",
                            "Export .pat: the project's line patterns, and generated ones that can be written as lines, as an AutoCAD/Revit .pat file",
                        )
                        .clicked()
                        {
                            asks.push(PAsk::Export);
                        }
                        if crate::props::icon_button(ui, "download", "Import .pat: hatch patterns from an AutoCAD or Revit .pat file").clicked() {
                            asks.push(PAsk::Import);
                        }
                    });
                });
            });
            ui.separator(); }
            let Some(i) = edited.patterns.iter().position(|p| p.name.eq_ignore_ascii_case(&current)) else {
                // a built-in one: shown, copied to edit
                ui.vertical(|ui| {
                    let Some(p) = pack.pattern(&current) else { return };
                    if part == Part::Props {
                        ui.label(egui::RichText::new(&current).strong());
                        ui.label(egui::RichText::new("Built in: a symbolic hatch on paper, the same at every scale. Make an editable copy to change it.").weak());
                        if ui.button("Make an editable copy").clicked() {
                            edited.patterns.push(p.into_owned());
                            label = "Edit built-in pattern";
                        }
                        return;
                    }
                    if part == Part::Canvas {
                        // the whole area, the pattern on paper (its settings,
                        // and making a copy, are in Properties)
                        let r = ui.available_rect_before_wrap();
                        let (r, _) = ui.allocate_exact_size(r.size(), Sense::hover());
                        let side = (r.width().max(r.height()) as f64 / 120.0) * 2.0;
                        let sq = Rect::from_center_size(r.center(), Vec2::splat(r.width().max(r.height())));
                        let clip = ui.painter().with_clip_rect(r);
                        paint_pattern(&clip, sq, &p, 100.0, side);
                        return;
                    }
                    ui.heading(&current);
                    ui.label(egui::RichText::new("Built in: a symbolic hatch on paper, the same at every scale.").weak());
                    ui.add_space(6.0);
                    let s = 300.0_f32.min(ui.available_width() - 20.0);
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(s), Sense::hover());
                    paint_pattern(ui.painter(), r, &p, 100.0, 20.0 / 10.0 * (s as f64 / 120.0));
                    ui.add_space(6.0);
                    if ui
                        .button("Make an editable copy")
                        .on_hover_text("Saved in the project under the same name, it replaces the built-in one for every material that uses it")
                        .clicked()
                    {
                        edited.patterns.push(p.into_owned());
                        label = "Edit built-in pattern";
                    }
                });
                return;
            };
            // the canvas
            let p = &mut edited.patterns[i];
            let drawn = matches!(p.recipe, Some(Recipe::Drawn(_)));
            if part != Part::Props { ui.vertical(|ui| {
                // as large as the room allows: in the Patterns area, all of it
                if part == Part::Canvas {
                    // the whole area: the tile centred, its repeat to the
                    // edges, or the pattern seen at a scale
                    let room = ui.available_rect_before_wrap().size();
                    // Tab switches Edit and Seen on a drawn tile, as Object
                    // and Edit mode in a view (REV01, B)
                    let over = ui.rect_contains_pointer(ui.available_rect_before_wrap());
                    if over {
                        // the app's keys stand aside in Seen too
                        self.pat.hovered_pass = ui.ctx().cumulative_pass_nr();
                    }
                    let busy = !self.pat.draft.is_empty() || self.pat.press.is_some() || self.pat.dim_edit.is_some()
                        || self.pat.offset_src.is_some();
                    if drawn && over && !busy && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Tab)) {
                        self.pat.seen = !self.pat.seen;
                        self.pat.draft.clear();
                    }
                    if drawn && !self.pat.seen {
                        let at = ui.available_rect_before_wrap();
                        if self.pat.area_canvas(ui, p, room) {
                            label = "Draw pattern";
                        }
                        // the tile is drawn square; how it is laid shows in
                        // Seen (REV01, B)
                        if p.angle.abs() > 0.05 {
                            let text = format!("Laid at {}° · Tab shows it", fmt_angle(p.angle));
                            let pal = ui.visuals().clone();
                            let g = ui.painter().layout_no_wrap(text, egui::FontId::proportional(11.5), pal.text_color());
                            let r = Rect::from_min_size(at.left_top() + Vec2::new(10.0, 10.0), g.size() + Vec2::new(14.0, 8.0));
                            ui.painter().rect(r, 5.0, pal.extreme_bg_color, pal.widgets.noninteractive.bg_stroke, egui::StrokeKind::Inside);
                            ui.painter().galley(r.min + Vec2::new(7.0, 4.0), g, pal.text_color());
                        }
                    } else {
                        if self.pat.scale <= 0.0 {
                            self.pat.scale = 20.0;
                        }
                        let (r, _) = ui.allocate_exact_size(room, Sense::hover());
                        let big = r.width().max(r.height());
                        let sq = Rect::from_center_size(r.center(), Vec2::splat(big));
                        let paper = 80.0 * (big as f64 / 320.0);
                        let shown = match p.small_scale_name(self.pat.scale).and_then(|n| pack.pattern(n)) {
                            Some(sym) => sym.into_owned(),
                            None => p.clone(),
                        };
                        let clip = ui.painter().with_clip_rect(r);
                        paint_pattern(&clip, sq, &shown, self.pat.scale, paper * self.pat.scale / 1000.0);
                    }
                    return;
                }
                let side = if part == Part::Canvas {
                    0.0
                } else {
                    (ui.available_width() - 380.0).clamp(220.0, 440.0).min(height - 110.0)
                };
                ui.set_width(side);
                if drawn {
                    if part == Part::All { crate::props::flow(ui, |ui| {
                        for t in DrawTool::ALL {
                            if ui
                                .selectable_value(&mut self.pat.tool, t, t.label())
                                .on_hover_text(t.hint())
                                .changed()
                            {
                                self.pat.draft.clear();
                            }
                        }
                    }); }
                    if self.pat.tile_canvas(ui, p, side) {
                        label = "Draw pattern";
                    }
                    ui.label(egui::RichText::new(self.pat.tool.hint()).weak().size(11.0));
                } else {
                    if self.pat.scale <= 0.0 {
                        self.pat.scale = 20.0;
                    }
                    if part == Part::All { crate::props::flow(ui, |ui| {
                        ui.label("Seen at");
                        let h = ui.spacing().interact_size.y;
                        ui.allocate_ui(Vec2::new(180.0, h), |ui| {
                            crate::props::segment(
                                ui,
                                &mut self.pat.scale,
                                &[(20.0, "1:20", ""), (50.0, "1:50", ""), (100.0, "1:100", "")],
                            );
                        });
                    }); }
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(side), Sense::hover());
                    // the canvas shows 80 mm of paper
                    let paper = 80.0 * (side as f64 / 320.0);
                    let shown = match p.small_scale_name(self.pat.scale).and_then(|n| pack.pattern(n)) {
                        Some(sym) => sym.into_owned(),
                        None => p.clone(),
                    };
                    paint_pattern(ui.painter(), r, &shown, self.pat.scale, paper * self.pat.scale / 1000.0);
                    ui.painter().rect_stroke(r, 4.0, egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color), egui::StrokeKind::Inside);
                    ui.label(
                        egui::RichText::new(format!("{} across at 1:{}", crate::units::len(paper * self.pat.scale / 1000.0), self.pat.scale))
                            .weak()
                            .size(11.0),
                    );
                }
            }); }
            if part == Part::All {
                ui.separator();
            }
            if part == Part::Canvas {
                return;
            }
            // the properties
            ui.vertical(|ui| {
                egui::ScrollArea::vertical()
                    .id_salt("pat_props")
                    .max_height(if part == Part::Props { f32::INFINITY } else { height - 10.0 })
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        // the name, committed when typing stops
                        let (old, mut typed) = self
                            .pat
                            .name_edit
                            .clone()
                            .filter(|(o, _)| o.eq_ignore_ascii_case(&p.name))
                            .unwrap_or_else(|| (p.name.clone(), p.name.clone()));
                        let r = crate::props::row(ui, "Name", |ui| {
                            crate::props::text_field(ui, egui::TextEdit::singleline(&mut typed), f32::INFINITY)
                        });
                        let done = r.lost_focus();
                        self.pat.name_edit = Some((old.clone(), typed.clone()));
                        let typed = typed.trim().to_string();
                        if done && !typed.is_empty() && typed != old {
                            if pack.pattern(&typed).is_some() && !typed.eq_ignore_ascii_case(&old) {
                                self.pat.name_edit = None;
                            } else {
                                asks.push(PAsk::Rename(old.clone(), typed));
                                self.pat.name_edit = None;
                            }
                        }
                        let users = styles::pattern_users(self.doc.model(), &p.name);
                        {
                            let now = match &p.recipe {
                                None => "Line families",
                                Some(r) => match r {
                                    Recipe::Ashlar { .. } => "Random stone",
                                    Recipe::Drawn(_) => "Drawn tile",
                                    r => r.label(),
                                },
                            };
                            // nine makers: a drop-down, the field to the row's end
                            crate::props::Combo::salted("Made by", "pat_made_by")
                                .selected_text(now)
                                .show_ui(ui, |ui| {
                                    for kind in MADE_BY {
                                        if ui.selectable_label(kind == now, kind).clicked() && kind != now {
                                            match starting_recipe(kind) {
                                                Some(r) => {
                                                    p.recipe = Some(r);
                                                    p.units = PatternUnits::Model;
                                                }
                                                None => {
                                                    p.recipe = None;
                                                    if p.lines.is_empty() {
                                                        p.lines = styles::builtin_pattern(Hatch::Diagonal).lines;
                                                        p.units = PatternUnits::Paper;
                                                    }
                                                }
                                            }
                                            label = "Pattern recipe";
                                        }
                                    }
                                });
                        }
                        if let Some(r) = p.recipe.as_mut() {
                            if recipe_fields(ui, r) {
                                label = "Pattern size";
                            }
                            if crate::props::row(ui, "Angle", |ui| {
                                crate::props::fill(ui, egui::DragValue::new(&mut p.angle).range(-180.0..=180.0).speed(1.0).suffix("°"))
                                    .changed()
                            }) {
                                label = "Pattern angle";
                            }
                            if crate::props::switch(ui, &mut p.along_longest, "Along the longest side")
                                .on_hover_text("Courses run along each area's longest edge, from its end, instead of across the drawing: brick along a wall in plan")
                                .changed()
                            {
                                label = "Pattern direction";
                            }
                            if crate::props::row(ui, "Background", |ui| {
                                super::stylesui::colour_pick(ui, &mut p.fill, true, "A fill behind the lines; none is white")
                            }) {
                                label = "Pattern background";
                            }
                            // at small scales: a fold of its own, a control a row;
                            // switched off, its fields stay in sight, greyed
                            crate::props::sub_box(ui, "pat.small", "At small scales", None, true, |ui| {
                                let mut on = p.small_scale.is_some();
                                if crate::props::switch(ui, &mut on, "Simplify")
                                    .on_hover_text("Drawings smaller than a scale draw a symbolic pattern instead: coursing at 1:20, the brick hatch at 1:100")
                                    .changed()
                                {
                                    p.small_scale = on.then(|| SmallScale { below: 50.0, pattern: Hatch::Brick.label().to_string() });
                                    label = "Pattern at small scales";
                                }
                                let mut s = p
                                    .small_scale
                                    .clone()
                                    .unwrap_or(SmallScale { below: 50.0, pattern: Hatch::Brick.label().to_string() });
                                let mut moved = false;
                                ui.add_enabled_ui(on, |ui| {
                                    moved |= crate::props::row(ui, "From 1:", |ui| {
                                        crate::props::fill(ui, egui::DragValue::new(&mut s.below).range(2.0..=5000.0).speed(1.0)).changed()
                                    });
                                    crate::props::Combo::salted("Draw as", "pat_small")
                                        .selected_text(s.pattern.clone())
                                        .show_ui(ui, |ui| {
                                            for n in names.iter().filter(|n| !n.eq_ignore_ascii_case(&current)) {
                                                if ui.selectable_label(s.pattern.eq_ignore_ascii_case(n), n).clicked() {
                                                    s.pattern = n.clone();
                                                    moved = true;
                                                }
                                            }
                                        });
                                });
                                if on && moved {
                                    p.small_scale = Some(s);
                                    label = "Pattern at small scales";
                                }
                            });
                            if let Some(Recipe::Drawn(t)) = p.recipe.as_mut() {
                                ui.add_space(6.0);
                                ui.label(egui::RichText::new("STROKES").size(10.5).strong().color(ui.visuals().weak_text_color()));
                                if stroke_list(ui, t, &mut self.pat.stroke) {
                                    label = "Pattern strokes";
                                }
                                ui.label(
                                    egui::RichText::new(
                                        "Every stroke draws as a line in plans, sections and elevations. Its job says what it becomes in 3D: grooves and ridges along lines; separate units, raised or recessed areas and holes for shapes.",
                                    )
                                    .weak()
                                    .size(11.0),
                                );
                            }
                            if !p.recipe.as_ref().is_some_and(|r| r.is_valid()) {
                                ui.colored_label(ui.visuals().warn_fg_color, "Some size is zero: nothing is drawn.");
                            }
                        } else if super::stylesui::line_families(ui, p) {
                            label = "Edit pattern lines";
                        }
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new("AS IT DRAWS").size(10.5).strong().color(ui.visuals().weak_text_color()));
                        crate::props::flow(ui, |ui| {
                            for s in [20.0, 100.0] {
                                ui.vertical(|ui| {
                                    let shown = match p.small_scale_name(s).and_then(|n| pack.pattern(n)) {
                                        Some(sym) => sym.into_owned(),
                                        None => p.clone(),
                                    };
                                    swatch(ui, &shown, s, 72.0);
                                    ui.label(egui::RichText::new(format!("1:{s}")).weak().size(11.0));
                                });
                            }
                        });
                        // what uses it, at the foot (DS-01 4.8): a chip that
                        // opens the material, or how many there are
                        ui.add_space(8.0);
                        match users.as_slice() {
                            [] => crate::props::readout(
                                ui,
                                "Used by",
                                "No material yet",
                                "A material uses it when its cut or surface pattern is this one",
                            ),
                            [(id, name)] => {
                                if crate::props::relation(ui, "Used by", name, "Open it in Materials", None).select {
                                    asks.push(PAsk::OpenMaterial(*id));
                                }
                            }
                            many => {
                                let shown = format!("{} materials", many.len());
                                if crate::props::relation(ui, "Used by", &shown, &many.iter().map(|u| u.1.as_str()).collect::<Vec<_>>().join(", "), None).select {
                                    asks.push(PAsk::OpenMaterial(many[0].0));
                                }
                            }
                        }
                    });
            });
        });
        if !label.is_empty() && edited != *pack {
            asks.push(PAsk::Commit(label, Box::new(edited)));
        }
    }

    fn patterns_library(&mut self, ui: &mut egui::Ui, pack: &StylePack, asks: &mut Vec<PAsk>) {
        let f = self.pat.filter.trim().to_lowercase();
        let height = ui.available_height().max(380.0);
        let lib = pattern::library();
        self.pat.preset = self.pat.preset.min(lib.len().saturating_sub(1));
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(240.0);
                egui::ScrollArea::vertical()
                    .id_salt("pat_lib")
                    .max_height(height - 10.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let mut groups: Vec<&str> = lib.iter().map(|p| p.group.as_str()).collect();
                        groups.dedup();
                        for g in groups {
                            let rows: Vec<(usize, &pattern::LibraryEntry)> = lib
                                .iter()
                                .enumerate()
                                .filter(|(_, p)| {
                                    p.group == g
                                        && (f.is_empty() || p.name.to_lowercase().contains(&f))
                                })
                                .collect();
                            if rows.is_empty() {
                                continue;
                            }
                            // the library's accordion and rows (MU42)
                            let open = crate::library::group(ui, "pattern-library", g, rows.len());
                            // the rows fold smoothly (AMD-017); a search shows them all
                            crate::props::fold(ui, open || !f.is_empty(), |ui| {
                                for (i, pr) in rows {
                                    let p = pr.pattern.clone();
                                    let have = pack.pattern(&pr.name).is_some();
                                    let mark = move |pa: &egui::Painter, r: egui::Rect| {
                                        paint_pattern(pa, r, &p, 20.0, 20.0 / 50.0);
                                        pa.rect_stroke(
                                            r,
                                            2.0,
                                            egui::Stroke::new(1.0, Color32::from_black_alpha(60)),
                                            egui::StrokeKind::Inside,
                                        );
                                    };
                                    if crate::library::item(
                                        ui,
                                        self.pat.preset == i,
                                        &pr.name,
                                        if have { "in project" } else { "" },
                                        Some(&mark),
                                    )
                                    .on_hover_text(if have {
                                        "Already in the project"
                                    } else {
                                        "Not in the project yet"
                                    })
                                    .clicked()
                                    {
                                        self.pat.preset = i;
                                    }
                                }
                            });
                        }
                    });
            });
            ui.separator();
            let Some(pr) = lib.get(self.pat.preset) else {
                return;
            };
            let mut p = pr.pattern.clone();
            ui.vertical(|ui| {
                ui.heading(&pr.name);
                ui.label(
                    egui::RichText::new(if pr.true_size {
                        format!(
                            "{} · made by {} · true size",
                            pr.group,
                            p.recipe.as_ref().map_or("lines", |r| r.label())
                        )
                    } else {
                        format!("{} · lines and a tile · on paper", pr.group)
                    })
                    .weak(),
                );
                // the line on what it is for takes its room from the
                // swatch, so the foot's button stays in the window
                let about_h = if pr.about.is_empty() {
                    0.0
                } else {
                    ui.add(egui::Label::new(egui::RichText::new(&pr.about).size(12.0)).wrap())
                        .rect
                        .height()
                        + 4.0
                };
                ui.add_space(6.0);
                let side = (ui.available_width() - 20.0)
                    .clamp(160.0, 420.0)
                    .min(height - 120.0 - about_h);
                let (r, _) = ui.allocate_exact_size(Vec2::splat(side), Sense::hover());
                let paper = 80.0 * (side as f64 / 320.0);
                paint_pattern(ui.painter(), r, &p, 20.0, paper * 20.0 / 1000.0);
                ui.painter().rect_stroke(
                    r,
                    4.0,
                    egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
                    egui::StrokeKind::Inside,
                );
                ui.label(
                    egui::RichText::new(format!(
                        "{} across at 1:20",
                        crate::units::len(paper * 20.0 / 1000.0)
                    ))
                    .weak()
                    .size(11.0),
                );
                // the foot: whether the project has it, and the main button
                let have = pack.pattern(&pr.name).is_some();
                crate::library::foot(
                    ui,
                    if have {
                        "In the project already"
                    } else {
                        "Not in the project yet"
                    },
                    |ui| {
                        let text = if have {
                            "Add a copy to the project"
                        } else {
                            "Add to the project"
                        };
                        if crate::library::primary(ui, true, text).clicked() {
                            let mut edited = pack.clone();
                            p.name = free_name(&edited, &pr.name);
                            self.pat.current = Some(p.name.clone());
                            self.pat.tab = PatTab::Project;
                            edited.patterns.push(p.clone());
                            asks.push(PAsk::Commit("Add pattern", Box::new(edited)));
                        }
                    },
                );
            });
        });
    }

    fn import_pat_dialog(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Import hatch patterns")
            .add_filter("Hatch patterns (.pat)", &["pat", "PAT"])
            .pick_file()
        else {
            return;
        };
        let text = match std::fs::read(&path) {
            Ok(b) => String::from_utf8_lossy(&b).into_owned(),
            Err(e) => {
                self.report(Err(e), "");
                return;
            }
        };
        let read = patfile::read(&text, false);
        let (id, mut pack) = self.edited_pack();
        let mut added = 0;
        let mut replaced = 0;
        for p in read.patterns {
            match pack
                .patterns
                .iter_mut()
                .find(|x| x.name.eq_ignore_ascii_case(&p.name))
            {
                Some(x) => {
                    *x = p;
                    replaced += 1;
                }
                None => {
                    pack.patterns.push(p);
                    added += 1;
                }
            }
        }
        let file = path
            .file_name()
            .map_or_else(String::new, |f| f.to_string_lossy().into_owned());
        let mut msg = format!("Read {file}: {added} pattern(s) added, {replaced} replaced.");
        if !read.skipped.is_empty() {
            msg = format!("{msg} Skipped: {}.", read.skipped.join(", "));
        }
        if added + replaced > 0 {
            let r = match id {
                Some(id) => self.doc.edit_merged(id, "Import patterns", |e| {
                    e.kind = ElementKind::StylePack(Box::new(pack.clone()));
                }),
                None => {
                    let tx = styles::apply_pack(self.doc.model(), &pack);
                    self.doc.commit(Transaction {
                        label: "Import patterns".into(),
                        ..tx
                    })
                }
            };
            self.report(r, msg);
        } else {
            self.report(Ok::<(), String>(()), msg);
        }
    }

    fn export_pat_dialog(&mut self) {
        let (_, pack) = self.edited_pack();
        if pack.patterns.is_empty() {
            self.report(
                Ok::<(), String>(()),
                "The project has no patterns of its own to export.",
            );
            return;
        }
        let Some(path) = rfd::FileDialog::new()
            .set_title("Export hatch patterns")
            .set_file_name("creabim.pat")
            .add_filter("Hatch patterns (.pat)", &["pat"])
            .save_file()
        else {
            return;
        };
        let (text, left) = patfile::write(&pack.patterns);
        let r = std::fs::write(&path, text);
        let mut msg = format!(
            "Wrote {} pattern(s) to {}.",
            pack.patterns.len() - left.len(),
            path.display()
        );
        if !left.is_empty() {
            msg = format!("{msg} Not written (no line form): {}.", left.join(", "));
        }
        self.report(r, msg);
    }
}

#[cfg(test)]
mod exact_size_tests {
    use super::*;

    fn brick() -> DrawnTile {
        DrawnTile {
            size: [0.3, 0.1],
            repeat: Repeat::Grid,
            strokes: vec![DrawnStroke {
                points: vec![[0.0, 0.0], [0.2, 0.0], [0.2, 0.065], [0.0, 0.065]],
                closed: true,
                job: Job::Unit,
                ..Default::default()
            }],
            dims: vec![[[0.0, 0.0], [0.2, 0.0]]],
        }
    }

    #[test]
    fn a_dimension_typed_longer_stretches_the_unit_it_measures() {
        let mut t = brick();
        resize_dim(&mut t, 0, 0.215);
        let xs: Vec<f64> = t.strokes[0].points.iter().map(|q| q[0]).collect();
        assert!(
            (xs[1] - 0.215).abs() < 1e-9 && (xs[2] - 0.215).abs() < 1e-9,
            "{xs:?}"
        );
        assert!(
            xs[0].abs() < 1e-9 && xs[3].abs() < 1e-9,
            "the near side stays: {xs:?}"
        );
        assert!(
            (t.dims[0][1][0] - 0.215).abs() < 1e-9,
            "the dimension follows"
        );
    }

    #[test]
    fn an_offset_out_of_a_unit_grows_it_by_the_distance_on_every_side() {
        let t = brick();
        let o = offset_stroke(&t.strokes[0], 0.01).unwrap();
        let lo = o
            .points
            .iter()
            .fold([f64::MAX; 2], |a, q| [a[0].min(q[0]), a[1].min(q[1])]);
        let hi = o
            .points
            .iter()
            .fold([f64::MIN; 2], |a, q| [a[0].max(q[0]), a[1].max(q[1])]);
        assert!(
            (lo[0] + 0.01).abs() < 1e-9 && (hi[0] - 0.21).abs() < 1e-9,
            "{lo:?} {hi:?}"
        );
        assert!(
            (lo[1] + 0.01).abs() < 1e-9 && (hi[1] - 0.075).abs() < 1e-9,
            "{lo:?} {hi:?}"
        );
    }

    #[test]
    fn the_pointer_inside_a_shape_offsets_it_in() {
        let t = brick();
        assert!(side_distance(&t.strokes[0], DVec2::new(0.1, 0.03)) < 0.0);
        assert!(side_distance(&t.strokes[0], DVec2::new(0.1, 0.09)) > 0.0);
    }
}
