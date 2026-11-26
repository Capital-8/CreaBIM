//! Angle, radius, diameter and arc-length dimensions and text notes in the
//! 2D panes and on sheets: drawing them on screen, the tools that place
//! them, dragging them, and their properties and text styles.

use crate::planview::{Tool, Xf};
use crate::selection::Selection;
use crate::snaps::SnapKind;
use creabim_core::{
    Dimension, DimensionKind, Document, Element, ElementId, ElementKind, Font, HostPoint, Model,
    ModelError, Note, StyleUse, TagShape, Terminator, TextStyle, Transaction, View,
};
use creabim_geom::annotate::attach;
use creabim_geom::annotation::{
    angle_text, circle_through, line_intersection, measure, sweep, Annotation, Ends, Shape,
    TextLook,
};
use creabim_geom::paper::Anchor;
use creabim_geom::PlanItem;
use eframe::egui::{self, Color32, Pos2, Stroke, Vec2};
use glam::DVec2;

/// Annotations are drawn in this blue on screen unless they have a colour.
pub const INK: Color32 = Color32::from_rgb(30, 60, 120);
fn interaction_ink() -> Color32 {
    crate::theme::interaction()
}

pub fn color32([r, g, b]: [f32; 3]) -> Color32 {
    let c = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgb(c(r), c(g), c(b))
}

/// The screen colour of an annotation.
pub fn ink(look: &TextLook, selected: bool, highlight: Color32) -> Color32 {
    match (selected, look.rgb) {
        (true, _) => highlight,
        (false, Some(rgb)) => color32(rgb),
        (false, None) => INK,
    }
}

/// Text height on screen, in pixels, for a look in a pane at `mm` drawing
/// units per paper mm.
pub fn text_px(look: &TextLook, mm: f64, xf: &Xf) -> f32 {
    let px = (look.size * mm * xf.scale as f64) as f32;
    if (mm - 1.0).abs() < 1e-9 {
        // on a sheet's paper (mm = 1): its true size, shrinking with the
        // sheet and left out once too small, never spilling off (MU23)
        px.max(1.0)
    } else {
        // in a view while working: up to 9 px so a detail's tags read
        // (A5-7), but never more than twice their true size, so a view
        // zoomed far out does not fill with oversized words (MU28)
        px.max(9.0_f32.min(px * 2.0))
    }
}

/// Text on screen in a look's family, weight and slant, turned `angle`
/// radians counter-clockwise about its anchor.
/// A text size on screen snapped to a ladder of twelve sizes a doubling:
/// egui draws each glyph once per size, and a zoom asked it for a new size
/// every frame, the fonts' glyphs drawn again each time (MU85: a plan
/// stalled for up to a second while zooming).
pub fn snap_px(px: f32) -> f32 {
    if px <= 0.0 {
        return px;
    }
    // steps of 1/24 octave (3 %): few sizes for the glyph cache, too fine
    // to see jump when zooming (MU89: the text flickered in size)
    ((px.log2() * 24.0).round() / 24.0).exp2().max(1.0)
}

/// The egui family a text style's font draws in on screen.
pub fn family_of(f: Font) -> egui::FontFamily {
    match f {
        Font::Mono => egui::FontFamily::Monospace,
        Font::Sans | Font::Serif => egui::FontFamily::Proportional,
        f => egui::FontFamily::Name(f.family().into()),
    }
}

/// The shipped fonts, each a family of its own, falling back to the
/// interface's font for anything it lacks (2026-11-16 font set).
pub fn install_fonts(ctx: &egui::Context) {
    let mut defs = egui::FontDefinitions::default();
    let fallback = defs
        .families
        .get(&egui::FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    for f in Font::ALL {
        let Some(bytes) = creabim_geom::fonts::bytes(f) else {
            continue;
        };
        let name = f.family().to_string();
        defs.font_data.insert(
            name.clone(),
            std::sync::Arc::new(egui::FontData::from_static(bytes)),
        );
        let mut list = vec![name.clone()];
        list.extend(fallback.iter().cloned());
        defs.families
            .insert(egui::FontFamily::Name(name.into()), list);
    }
    ctx.set_fonts(defs);
}

#[allow(clippy::too_many_arguments)]
pub fn text(
    painter: &egui::Painter,
    at: Pos2,
    text: &str,
    px: f32,
    color: Color32,
    angle: f64,
    anchor: Anchor,
    look: &TextLook,
) {
    if px < 3.0 {
        // the bar as long as the line is on paper (owner after MU91: the
        // guessed length ran past the note's box)
        let wide = look.width(text) as f32;
        let w = (look.size > 0.0 && wide > 0.0).then(|| wide * px / look.size as f32);
        greek(painter, at, text, px, anchor, angle, color, w);
        return;
    }
    let family = family_of(look.font);
    let lay = |px: f32| {
        let mut job = egui::text::LayoutJob::simple_singleline(
            text.to_string(),
            egui::FontId::new(px, family.clone()),
            color,
        );
        for s in &mut job.sections {
            s.format.italics = look.italic;
        }
        painter.layout_job(job)
    };
    // as wide on screen as on paper: the screen's face is not the paper's,
    // so the size is set from the paper width (MU89: lines ran past a
    // note's box and changed length while zooming)
    let snapped = snap_px(px);
    let mut galley = lay(snapped);
    let paper = look.width(text) as f32;
    if look.size > 0.0 && paper > 0.0 && galley.size().x > 1.0 {
        let want = paper * px / look.size as f32;
        // stepped down, never up, so a line never runs past its box at any
        // zoom (MU91)
        let exact = snapped * (want / galley.size().x).clamp(0.6, 1.6);
        let fit = ((exact.log2() * 24.0).floor() / 24.0).exp2().max(1.0);
        if (fit - snapped).abs() > 0.01 {
            galley = lay(fit);
        }
    }
    let w = galley.size().x;
    let shift = match anchor {
        Anchor::Start => w / 2.0,
        Anchor::Middle => 0.0,
        Anchor::End => -w / 2.0,
    };
    // the screen's y runs down, so a counter-clockwise turn is a negative angle
    let rot = egui::emath::Rot2::from_angle(-angle as f32);
    let centre = at + rot * Vec2::new(shift, 0.0);
    let pos = centre - rot * (galley.size() / 2.0);
    if look.bold {
        // no bold face on screen: the text again, a hair to the right
        let nudge = rot * Vec2::new((px * 0.05).max(0.6), 0.0);
        painter.add(
            egui::epaint::TextShape::new(pos + nudge, galley.clone(), color)
                .with_angle(-angle as f32),
        );
    }
    painter.add(egui::epaint::TextShape::new(pos, galley, color).with_angle(-angle as f32));
}

/// Text too small to read drawn as a faint bar the length of its line, in
/// its own colour (a selection's blue stays blue), so notes and lists stay
/// in sight at any zoom (MU88, MU89).
#[allow(clippy::too_many_arguments)]
pub fn greek(
    painter: &egui::Painter,
    o: Pos2,
    text: &str,
    px: f32,
    anchor: Anchor,
    angle: f64,
    color: Color32,
    width: Option<f32>,
) {
    let chars = text.trim_end().chars().count() as f32;
    if chars == 0.0 || px < 0.15 {
        return;
    }
    // its paper width when known; else about half its height per letter
    let w = width.unwrap_or(chars * px * 0.52);
    let start = match anchor {
        Anchor::Start => 0.0,
        Anchor::Middle => -w / 2.0,
        Anchor::End => -w,
    };
    let rot = egui::emath::Rot2::from_angle(-angle as f32);
    let a = o + rot * Vec2::new(start, 0.0);
    let b = o + rot * Vec2::new(start + w, 0.0);
    painter.line_segment(
        [a, b],
        egui::Stroke::new(px.clamp(0.6, 2.0), color.gamma_multiply(0.45)),
    );
}

/// Draw an annotation in a pane.
pub fn paint(painter: &egui::Painter, xf: &Xf, item: &Annotation, color: Color32, mm: f64) {
    paint_weighted(painter, xf, item, color, mm, 1.0);
}

/// Draw an annotation with its strokes `w` times their weight: a hover or
/// selection traces the annotation itself, heavier, in its colour.
pub fn paint_weighted(
    painter: &egui::Painter,
    xf: &Xf,
    item: &Annotation,
    color: Color32,
    mm: f64,
    w: f32,
) {
    let m = item.marks(mm);
    for l in &m.lines {
        let pts: Vec<Pos2> = l.iter().map(|p| xf.to_screen(*p)).collect();
        painter.add(egui::Shape::line(pts, Stroke::new(w, color)));
    }
    for a in &m.arrows {
        let pts: Vec<Pos2> = a.iter().map(|p| xf.to_screen(*p)).collect();
        painter.add(egui::Shape::convex_polygon(pts, color, Stroke::NONE));
    }
    paint_ends_weighted(painter, xf, &m.ends, color, w);
    let px = text_px(&item.look, mm, xf);
    for t in &m.texts {
        // bold and italic runs one after the other (notes' **bold**)
        use creabim_geom::annotation::{runs_width, text_runs, BOLD, ITALIC};
        if !t.text.contains([BOLD, ITALIC]) || item.look.size <= 0.0 {
            text(
                painter,
                xf.to_screen(t.at),
                &t.text,
                px,
                color,
                t.angle,
                t.anchor,
                &item.look,
            );
            continue;
        }
        let k = px / item.look.size as f32;
        let total = runs_width(&t.text, &item.look) as f32 * k;
        let start = match t.anchor {
            Anchor::Start => 0.0,
            Anchor::Middle => -total / 2.0,
            Anchor::End => -total,
        };
        let rot = egui::emath::Rot2::from_angle(-t.angle as f32);
        let o = xf.to_screen(t.at);
        for (off, run, l) in text_runs(&t.text, &item.look) {
            if !run.is_empty() {
                let at = o + rot * Vec2::new(start + off as f32 * k, 0.0);
                text(painter, at, &run, px, color, t.angle, Anchor::Start, &l);
            }
        }
    }
}

/// Draw dimension and leader ends in a pane: thin and heavy strokes and
/// filled shapes.
pub fn paint_ends(painter: &egui::Painter, xf: &Xf, ends: &Ends, color: Color32) {
    paint_ends_weighted(painter, xf, ends, color, 1.0);
}

/// Dimension and leader ends with their strokes `w` times their weight.
pub fn paint_ends_weighted(painter: &egui::Painter, xf: &Xf, ends: &Ends, color: Color32, w: f32) {
    let screen = |l: &Vec<DVec2>| -> Vec<Pos2> { l.iter().map(|p| xf.to_screen(*p)).collect() };
    for l in &ends.thin {
        painter.add(egui::Shape::line(screen(l), Stroke::new(w, color)));
    }
    for l in &ends.heavy {
        painter.add(egui::Shape::line(screen(l), Stroke::new(2.0 * w, color)));
    }
    for f in &ends.fills {
        painter.add(egui::Shape::convex_polygon(screen(f), color, Stroke::NONE));
    }
}

// ---- tools ------------------------------------------------------------------

/// Is this one of the tools here?
pub fn is_tool(t: Tool) -> bool {
    matches!(
        t,
        Tool::Angle
            | Tool::Radius
            | Tool::Diameter
            | Tool::ArcLength
            | Tool::Note
            | Tool::Symbol
            | Tool::SpotLevel
    )
}

type Picked = (HostPoint, DVec2);

/// What a tool has picked, waiting for the click that places it.
#[derive(Clone, Copy, Debug)]
enum Ready {
    Angle {
        vertex: Picked,
        a: Picked,
        b: Picked,
    },
    Circle {
        center: Picked,
        r: f64,
        host: Option<ElementId>,
        diameter: bool,
    },
    Arc {
        center: Picked,
        a: Picked,
        b: Picked,
    },
    Leader {
        target: Picked,
    },
    /// The point a spot level measures.
    Spot {
        point: Picked,
        plan: bool,
    },
}

/// What a drag in the Select tool holds.
#[derive(Clone, Copy, Debug)]
pub enum Grab {
    /// A note's text: where the drag began and the note's position then.
    Note(ElementId, DVec2, DVec2),
    /// A note's leader arrow.
    Leader(ElementId),
    /// A dimension's line or arc.
    Shape(ElementId),
    /// A note's leader elbow.
    Elbow(ElementId),
    /// A tag's leader end, and its host's centre in the drawing.
    TagEnd(ElementId, DVec2),
    /// A tag's leader elbow, and its host's centre in the drawing.
    TagElbow(ElementId, DVec2),
    /// A level's head in a section: the level, and its datum line's end.
    LevelHead(ElementId, DVec2),
    /// A lot edge's note: the lot, the edge, where the drag began and the
    /// note's offset then (MU98_06).
    LotNote(ElementId, usize, DVec2, DVec2),
}

/// Where the elbow grip of a leader running `from` → `to` sits: its
/// elbow, or halfway along when it has none.
fn elbow_grip(elbow: Option<DVec2>, from: DVec2, to: DVec2) -> DVec2 {
    elbow.unwrap_or((from + to) / 2.0)
}

/// Is `press` (on the screen) on a grip of a selected item?
pub fn on_grip(
    model: &Model,
    items: &[PlanItem],
    selection: &Selection,
    xf: &Xf,
    press: egui::Pos2,
    mm: f64,
) -> bool {
    items.iter().any(|it| {
        selection.contains(it.id())
            && grips(model, it, mm)
                .iter()
                .any(|(p, _)| xf.to_screen(*p).distance(press) < 9.0)
    })
}

/// The grips of a selected item: (where, what dragging it does).
fn grips(model: &Model, it: &PlanItem, mm: f64) -> Vec<(DVec2, Grab)> {
    match it {
        PlanItem::Annotation { id, item } => match &item.shape {
            Shape::Note {
                leader: Some(tip),
                elbow,
                ..
            } => {
                let m = item.marks(mm);
                let from = m
                    .lines
                    .last()
                    .and_then(|l| l.first().copied())
                    .unwrap_or(*tip);
                // a room's tag: its leader's end and bend are the tag's own,
                // kept from the room's middle (MU142: these grips moved
                // only notes, so a room tag's showed and did nothing)
                match model.get(*id).map(|e| &e.kind) {
                    Some(ElementKind::Tag(t)) => {
                        let centre = *tip - t.end.unwrap_or(DVec2::ZERO);
                        vec![
                            (*tip, Grab::TagEnd(*id, centre)),
                            (elbow_grip(*elbow, from, *tip), Grab::TagElbow(*id, centre)),
                        ]
                    }
                    Some(ElementKind::Note(_)) => vec![
                        (*tip, Grab::Leader(*id)),
                        (elbow_grip(*elbow, from, *tip), Grab::Elbow(*id)),
                    ],
                    // drawn for something else (a section's level tags,
                    // the terrain's): these grips edit notes only and did
                    // nothing (MU142); a level tag has grips of its own
                    _ => vec![],
                }
            }
            // a level's head in a section or elevation, for this view only
            Shape::Level { at, line_end, .. } => {
                vec![(*at, Grab::LevelHead(*id, line_end.unwrap_or(*at)))]
            }
            _ => vec![],
        },
        PlanItem::Tag {
            id,
            anchor,
            at,
            elbow,
            ..
        } => {
            let Some(ElementKind::Tag(t)) = model.get(*id).map(|e| &e.kind) else {
                return vec![];
            };
            let centre = *anchor - t.end.unwrap_or(DVec2::ZERO);
            vec![
                (*anchor, Grab::TagEnd(*id, centre)),
                (
                    elbow_grip(*elbow, *at, *anchor),
                    Grab::TagElbow(*id, centre),
                ),
            ]
        }
        // a lot edge's note: dragged from its place below the setback
        PlanItem::Text {
            id,
            at,
            text,
            size,
            angle,
            ..
        } if *size == creabim_geom::parts::LOT_NOTE_MM => {
            // the grip just before its first letter, clear of the words
            let half = (text.chars().count() as f64 * 0.3 + 0.35) * size * mm;
            let grip = *at - DVec2::from_angle(*angle) * (half + 0.8 * size * mm);
            let Some(ElementKind::Site(creabim_core::Site {
                form:
                    creabim_core::SiteForm::Property {
                        boundary, rules, ..
                    },
                ..
            })) = model.get(*id).map(|e| &e.kind)
            else {
                return vec![];
            };
            (0..boundary.len())
                .find(|i| rules.note_at(*i) == Some(text.trim()))
                .map(|i| {
                    let off = rules.note_off(i).unwrap_or(DVec2::ZERO);
                    vec![(grip, Grab::LotNote(*id, i, DVec2::ZERO, off))]
                })
                .unwrap_or_default()
        }
        _ => vec![],
    }
}

/// The tools' clicks so far, and their options.
#[derive(Debug)]
pub struct Draft {
    points: Vec<Picked>,
    /// The first line picked for an angle: its element, ends, and the click.
    line: Option<(ElementId, DVec2, DVec2, DVec2)>,
    ready: Option<Ready>,
    /// The Note tool draws a leader first.
    pub leader: bool,
    /// A note was just placed: Properties should put the cursor in its text.
    pub focus_text: bool,
    pub grab: Option<Grab>,
    /// The area whose drag took `grab`: another area, not dragged, must
    /// not let it go (MU31: a sheet note moved one frame, then stopped).
    pub grab_by: Option<egui::Id>,
    /// The styles new dimensions, notes and tags get (none: the default).
    pub dim_style: Option<ElementId>,
    pub note_style: Option<ElementId>,
    pub tag_style: Option<ElementId>,
    /// New tags read their host's material build-up (Annotate step 5).
    pub tag_material: bool,
}

impl Default for Draft {
    fn default() -> Self {
        Self {
            points: vec![],
            line: None,
            ready: None,
            leader: true,
            focus_text: false,
            grab: None,
            grab_by: None,
            dim_style: None,
            note_style: None,
            tag_style: None,
            tag_material: false,
        }
    }
}

impl Draft {
    pub fn clear(&mut self) {
        self.points.clear();
        self.line = None;
        self.ready = None;
    }

    /// The style a new element of this kind gets, if one is picked.
    pub fn style_for(&self, kind: &ElementKind) -> Option<ElementId> {
        match kind {
            ElementKind::Dimension(_) => self.dim_style,
            ElementKind::Note(_) => self.note_style,
            ElementKind::Tag(_) => self.tag_style,
            _ => None,
        }
    }

    /// Give a new dimension, note or tag the style picked for its kind.
    pub fn styled(&self, mut e: Element) -> Element {
        if let Some(id) = self.style_for(&e.kind) {
            set_style(&mut e.kind, id);
        }
        e
    }

    pub fn has_input(&self) -> bool {
        !self.points.is_empty() || self.line.is_some() || self.ready.is_some()
    }
}

/// The straight edge of the drawing nearest `p`, within `tol`.
pub fn segment_at(items: &[PlanItem], p: DVec2, tol: f64) -> Option<(ElementId, DVec2, DVec2)> {
    let mut best: Option<(f64, ElementId, DVec2, DVec2)> = None;
    let mut consider = |id: ElementId, a: DVec2, b: DVec2| {
        if a.distance(b) < 1e-9 {
            return;
        }
        let d = segment_distance(p, a, b);
        if d < tol && best.is_none_or(|(bd, ..)| d < bd) {
            best = Some((d, id, a, b));
        }
    };
    for it in items {
        match it {
            PlanItem::Line { id, a, b, .. } => consider(*id, *a, *b),
            PlanItem::Polygon { id, points, .. } => {
                for i in 0..points.len() {
                    consider(*id, points[i], points[(i + 1) % points.len()]);
                }
            }
            _ => {}
        }
    }
    best.map(|(_, id, a, b)| (id, a, b))
}

/// The arc or circle of the drawing nearest `p`, within `tol`: its element,
/// centre, radius and start and end angles.
fn arc_at(items: &[PlanItem], p: DVec2, tol: f64) -> Option<(ElementId, DVec2, f64, f64, f64)> {
    let mut best: Option<(f64, ElementId, DVec2, f64, f64, f64)> = None;
    for it in items {
        let PlanItem::Arc {
            id,
            center,
            radius,
            start,
            end,
            ..
        } = it
        else {
            continue;
        };
        let d = (p.distance(*center) - radius).abs();
        let span = (end - start).abs();
        let t = ((p - *center).to_angle() - start.min(*end)).rem_euclid(std::f64::consts::TAU);
        let on = span >= std::f64::consts::TAU - 1e-6 || t <= span + 1e-6;
        if on && d < tol && best.is_none_or(|b| d < b.0) {
            best = Some((d, *id, *center, *radius, *start, *end));
        }
    }
    best.map(|(_, id, c, r, s, e)| (id, c, r, s.min(e), s.max(e)))
}

fn segment_distance(p: DVec2, a: DVec2, b: DVec2) -> f64 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-18)).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

/// The point on the endless line through `a`, `b` nearest `p`.
fn project(p: DVec2, a: DVec2, b: DVec2) -> DVec2 {
    let d = (b - a).normalize_or_zero();
    a + d * (p - a).dot(d)
}

/// A point on the arm from `vertex` through the line `a`,`b` on the side of
/// `click`, at least a little way out.
fn arm(vertex: DVec2, a: DVec2, b: DVec2, click: DVec2) -> DVec2 {
    let q = project(click, a, b);
    if q.distance(vertex) > 1e-3 {
        return q;
    }
    // a click right on the corner: along the line to its further end
    if a.distance(vertex) > b.distance(vertex) {
        a
    } else {
        b
    }
}

/// A click with an annotation tool in a plan or section. `hit` is the
/// snapped point under the pointer and what it attaches to; `raw` the
/// pointer itself. Returns a hint for the status bar.
#[allow(clippy::too_many_arguments)]
pub fn click(
    doc: &mut Document,
    view_id: ElementId,
    view: &View,
    tool: Tool,
    hit: Option<(HostPoint, DVec2, Option<SnapKind>)>,
    raw: DVec2,
    items: &[PlanItem],
    tol: f64,
    d: &mut Draft,
    selection: &mut Selection,
) -> Result<String, ModelError> {
    let model = doc.model();
    let (hp, p, snapped) = match hit {
        Some((hp, p, k)) => (hp, p, k.is_some_and(|k| k.is_point())),
        None => (HostPoint::free(raw), raw, false),
    };
    if let Some(ready) = d.ready.take() {
        d.clear();
        let mut e = d.styled(place(model, view_id, view, ready, tool, raw));
        if let ElementKind::Dimension(dim) = &mut e.kind {
            if dim.kind == DimensionKind::Spot {
                // a new spot level reads like the last one made
                dim.pattern = model
                    .iter()
                    .filter_map(|e| match &e.kind {
                        ElementKind::Dimension(d) if d.kind == DimensionKind::Spot => {
                            Some(d.pattern.clone())
                        }
                        _ => None,
                    })
                    .last()
                    .flatten();
            }
        }
        let id = e.id;
        let label = match &e.kind {
            ElementKind::Note(_) => "Add note",
            _ => "Add dimension",
        };
        doc.commit(Transaction::new(label).insert(e))?;
        selection.only(id);
        return Ok(added(tool, d));
    }
    match tool {
        Tool::Symbol => {
            // the kind placed last, so a run of one kind is a click each
            let kind = model
                .iter()
                .filter_map(|e| match &e.kind {
                    ElementKind::Note(n) => n.symbol.as_ref().map(|s| s.kind.clone()),
                    _ => None,
                })
                .last()
                .unwrap_or_else(|| "points".into());
            let mut n = Note::symbol(view_id, p, &kind);
            // the style placed last of that kind, too
            n.symbol.as_mut().unwrap().style = model
                .iter()
                .filter_map(|e| match &e.kind {
                    ElementKind::Note(n) => n
                        .symbol
                        .as_ref()
                        .filter(|s| s.kind == kind)
                        .map(|s| s.style.clone()),
                    _ => None,
                })
                .last()
                .flatten();
            let e = Element::new("Symbol", ElementKind::Note(n));
            selection.only(e.id);
            doc.commit(Transaction::new("Add symbol").insert(e))?;
            Ok("Symbol placed: its kind, style, turn and text are in Properties.".into())
        }
        Tool::Note if !d.leader => {
            let e = d.styled(Element::new(
                "Note",
                ElementKind::Note(Note::new(view_id, raw, "Note")),
            ));
            selection.only(e.id);
            doc.commit(Transaction::new("Add note").insert(e))?;
            Ok(added(tool, d))
        }
        Tool::SpotLevel => {
            let point = match view.kind {
                // in a plan, the top of the slab under the click
                creabim_core::ViewKind::Plan { level, .. } => {
                    (attach(model, view, slab_under(model, level, raw), raw), raw)
                }
                _ => (hp, p),
            };
            let plan = matches!(view.kind, creabim_core::ViewKind::Plan { .. });
            d.ready = Some(Ready::Spot { point, plan });
            Ok("Now click where its text goes.".into())
        }
        Tool::Note => {
            d.ready = Some(Ready::Leader { target: (hp, p) });
            Ok("Now click where the note's text goes.".into())
        }
        Tool::Angle => {
            if d.points.is_empty() && !snapped {
                if let Some((id, a, b)) = segment_at(items, raw, tol) {
                    match d.line.take() {
                        None => {
                            d.line = Some((id, a, b, raw));
                            return Ok("Now click the second line.".into());
                        }
                        Some((id0, a0, b0, c0)) => {
                            let Some(v) = line_intersection(a0, b0, a, b) else {
                                return Ok("Those lines are parallel: pick two that meet.".into());
                            };
                            let (pa, pb) = (arm(v, a0, b0, c0), arm(v, a, b, raw));
                            d.ready = Some(Ready::Angle {
                                vertex: (attach(model, view, Some(id0), v), v),
                                a: (attach(model, view, Some(id0), pa), pa),
                                b: (attach(model, view, Some(id), pb), pb),
                            });
                            return Ok("Now click where the arc goes.".into());
                        }
                    }
                }
            }
            if d.line.is_some() {
                return Ok("Click a second line, or Esc to start again.".into());
            }
            d.points.push((hp, p));
            match d.points.as_slice() {
                [vertex, a, b] => {
                    d.ready = Some(Ready::Angle {
                        vertex: *vertex,
                        a: *a,
                        b: *b,
                    });
                    d.points.clear();
                    Ok("Now click where the arc goes.".into())
                }
                [_] => Ok("Now click a point on the first arm.".into()),
                _ => Ok("Now click a point on the second arm.".into()),
            }
        }
        Tool::Radius | Tool::Diameter | Tool::ArcLength => {
            if d.points.is_empty() {
                if let Some((id, c, r, start, end)) = arc_at(items, raw, tol) {
                    let center = (attach(model, view, Some(id), c), c);
                    d.ready = Some(if tool == Tool::ArcLength {
                        let (a, b) = (
                            c + DVec2::from_angle(start) * r,
                            c + DVec2::from_angle(end) * r,
                        );
                        Ready::Arc {
                            center,
                            a: (attach(model, view, Some(id), a), a),
                            b: (attach(model, view, Some(id), b), b),
                        }
                    } else {
                        Ready::Circle {
                            center,
                            r,
                            host: Some(id),
                            diameter: tool == Tool::Diameter,
                        }
                    });
                    return Ok("Now click where the dimension line goes.".into());
                }
            }
            d.points.push((hp, p));
            if d.points.len() < 3 {
                return Ok(format!(
                    "Point {} of 3 on the {}. Or click an arc or circle in the drawing.",
                    d.points.len(),
                    if tool == Tool::ArcLength {
                        "arc: its start, a point on it, then its end"
                    } else {
                        "circle"
                    }
                ));
            }
            let pts = std::mem::take(&mut d.points);
            let Some((c, r)) = circle_through(pts[0].1, pts[1].1, pts[2].1) else {
                return Ok("Those points are in a line: pick three on a curve.".into());
            };
            // the centre is kept on what the first point is on, so it moves with it
            let host = pts[0].0.host;
            let center = (attach(model, view, host, c), c);
            d.ready = Some(if tool == Tool::ArcLength {
                let (mut a, mut b) = (pts[0], pts[2]);
                if sweep(c, a.1, pts[1].1) > sweep(c, a.1, b.1) {
                    std::mem::swap(&mut a, &mut b);
                }
                Ready::Arc { center, a, b }
            } else {
                Ready::Circle {
                    center,
                    r,
                    host,
                    diameter: tool == Tool::Diameter,
                }
            });
            Ok("Now click where the dimension line goes.".into())
        }
        _ => Ok(String::new()),
    }
}

/// The topmost slab on `level` whose outline holds `p`.
fn slab_under(model: &Model, level: ElementId, p: DVec2) -> Option<ElementId> {
    model
        .iter()
        .filter_map(|e| match &e.kind {
            ElementKind::Slab(s)
                if s.level == level && creabim_core::polygon::contains(&s.boundary, p) =>
            {
                Some((e.id, s.top_offset))
            }
            _ => None,
        })
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|s| s.0)
}

fn added(tool: Tool, d: &mut Draft) -> String {
    if tool == Tool::Note {
        d.focus_text = true;
        "Note added. Type its text in Properties; drag it to move it.".into()
    } else if tool == Tool::SpotLevel {
        "Spot level added. Its text pattern is in Properties; drag its text to move it.".into()
    } else {
        "Dimension added. Drag it to move its line; its style is in Properties.".into()
    }
}

/// The element a placing click at `at` makes.
fn place(
    model: &Model,
    view_id: ElementId,
    view: &View,
    ready: Ready,
    tool: Tool,
    at: DVec2,
) -> Element {
    match ready {
        Ready::Angle { vertex, a, b } => {
            let (a, b) = if sweep(vertex.1, a.1, at) > sweep(vertex.1, a.1, b.1) {
                (b, a)
            } else {
                (a, b)
            };
            let radius = at.distance(vertex.1);
            Element::new(
                "Angle",
                ElementKind::Dimension(Dimension::about(
                    DimensionKind::Angular,
                    view_id,
                    vertex.0,
                    a.0,
                    b.0,
                    radius,
                )),
            )
        }
        Ready::Circle {
            center,
            r,
            host,
            diameter,
        } => {
            let dir = (at - center.1).normalize_or(DVec2::X);
            let on = attach(model, view, host, center.1 + dir * r);
            let extra = (at.distance(center.1) - r).max(0.0);
            let (name, kind) = if diameter {
                ("Diameter", DimensionKind::Diameter)
            } else {
                ("Radius", DimensionKind::Radius)
            };
            Element::new(
                name,
                ElementKind::Dimension(Dimension::about(kind, view_id, center.0, on, on, extra)),
            )
        }
        Ready::Spot { point, .. } => Element::new(
            "Spot level",
            ElementKind::Dimension(Dimension::spot(
                view_id,
                point.0,
                attach(model, view, point.0.host, at),
                None,
            )),
        ),
        Ready::Arc { center, a, b } => {
            let offset = at.distance(center.1) - center.1.distance(a.1);
            Element::new(
                "Arc length",
                ElementKind::Dimension(Dimension::about(
                    DimensionKind::ArcLength,
                    view_id,
                    center.0,
                    a.0,
                    b.0,
                    offset,
                )),
            )
        }
        Ready::Leader { target } => {
            let _ = tool;
            let mut n = Note::new(view_id, at, "Note");
            n.leader = Some(target.0);
            Element::new("Note", ElementKind::Note(n))
        }
    }
}

/// The annotation a placing click at `at` would make, for the preview.
fn shape_of(ready: Ready, at: DVec2) -> (Shape, String) {
    match ready {
        Ready::Spot { point, plan } => (
            Shape::Spot {
                at: point.1,
                text_at: at,
                plan,
            },
            "Spot level".into(),
        ),
        Ready::Angle { vertex, a, b } => {
            let (a, b) = if sweep(vertex.1, a.1, at) > sweep(vertex.1, a.1, b.1) {
                (b.1, a.1)
            } else {
                (a.1, b.1)
            };
            (
                Shape::Angle {
                    vertex: vertex.1,
                    a,
                    b,
                    radius: at.distance(vertex.1),
                },
                angle_text(sweep(vertex.1, a, b)),
            )
        }
        Ready::Circle {
            center,
            r,
            diameter,
            ..
        } => {
            let c = center.1;
            let on = c + (at - c).normalize_or(DVec2::X) * r;
            let kind = if diameter {
                DimensionKind::Diameter
            } else {
                DimensionKind::Radius
            };
            (
                Shape::Radius {
                    center: c,
                    at: on,
                    extra: (at.distance(c) - r).max(0.0),
                    diameter,
                },
                measure(&crate::units::get(), kind, c, on, on),
            )
        }
        Ready::Arc { center, a, b } => (
            Shape::ArcLength {
                center: center.1,
                a: a.1,
                b: b.1,
                offset: at.distance(center.1) - center.1.distance(a.1),
            },
            measure(
                &crate::units::get(),
                DimensionKind::ArcLength,
                center.1,
                a.1,
                b.1,
            ),
        ),
        Ready::Leader { target } => (
            Shape::Note {
                at,
                leader: Some(target.1),
                elbow: None,
            },
            "Note".into(),
        ),
    }
}

/// What a tool will make, drawn in the accent colour as the pointer moves.
#[allow(clippy::too_many_arguments)]
pub fn preview(
    painter: &egui::Painter,
    xf: &Xf,
    tool: Tool,
    d: &Draft,
    hover: Option<DVec2>,
    raw: Option<DVec2>,
    items: &[PlanItem],
    tol: f64,
    mm: f64,
) {
    let accent = Stroke::new(1.5, interaction_ink());
    if let (Some(ready), Some(at)) = (d.ready, raw) {
        let (shape, text) = shape_of(ready, at);
        let item = Annotation {
            shape,
            text,
            look: TextLook::default(),
            mm,
        };
        paint(painter, xf, &item, interaction_ink(), mm);
        return;
    }
    if let Some((_, a, b, _)) = d.line {
        painter.line_segment(
            [xf.to_screen(a), xf.to_screen(b)],
            Stroke::new(3.0, interaction_ink()),
        );
    }
    for (_, p) in &d.points {
        painter.circle_stroke(xf.to_screen(*p), 4.0, accent);
    }
    if let (Some(first), Some(h)) = (d.points.first(), hover) {
        let pts: Vec<Pos2> = d
            .points
            .iter()
            .map(|p| xf.to_screen(p.1))
            .chain([xf.to_screen(h)])
            .collect();
        if tool == Tool::Angle {
            // the arms from the vertex
            for p in &pts[1..] {
                painter.line_segment([xf.to_screen(first.1), *p], accent);
            }
        } else {
            painter.add(egui::Shape::dashed_line(&pts, accent, 6.0, 4.0));
        }
    }
    // what a click would pick
    if d.points.is_empty() {
        let Some(p) = raw else { return };
        match tool {
            Tool::Radius | Tool::Diameter | Tool::ArcLength => {
                if let Some((_, c, r, s, e)) = arc_at(items, p, tol) {
                    let n = 32;
                    let pts: Vec<Pos2> = (0..=n)
                        .map(|i| {
                            xf.to_screen(
                                c + DVec2::from_angle(s + (e - s) * i as f64 / n as f64) * r,
                            )
                        })
                        .collect();
                    painter.add(egui::Shape::line(pts, Stroke::new(3.0, interaction_ink())));
                }
            }
            Tool::Angle => {
                if let Some((_, a, b)) = segment_at(items, p, tol) {
                    painter.line_segment(
                        [xf.to_screen(a), xf.to_screen(b)],
                        Stroke::new(2.0, interaction_ink().gamma_multiply(0.6)),
                    );
                }
            }
            _ => {}
        }
    }
}

// ---- dragging ---------------------------------------------------------------

/// Drag a note (its text, or its leader's arrow when it is selected) or a
/// dimension's line in the Select tool. `snapped` is the point under the
/// pointer and what it attaches to, for a leader. Returns `Some` while a
/// drag is in progress.
#[allow(clippy::too_many_arguments)]
pub fn drag(
    doc: &mut Document,
    view_id: Option<ElementId>,
    view: Option<&View>,
    items: &[PlanItem],
    xf: &Xf,
    response: &egui::Response,
    d: &mut Draft,
    selection: &mut Selection,
    mm: f64,
    snapped: Option<(HostPoint, DVec2)>,
) -> Option<Option<String>> {
    let tol = 6.0 / xf.scale as f64;
    if response.drag_started_by(egui::PointerButton::Primary) {
        let press = response
            .ctx
            .input(|i| i.pointer.press_origin())
            .or(response.interact_pointer_pos())?;
        let w = xf.to_world(press);
        let model = doc.model();
        // the grips of what is selected come first
        let grip = items.iter().rev().find_map(|it| {
            if !selection.contains(it.id()) {
                return None;
            }
            grips(model, it, mm)
                .into_iter()
                .find(|(p, _)| xf.to_screen(*p).distance(press) < 9.0)
                .map(|g| match g.1 {
                    // where this drag began
                    Grab::LotNote(id, i, _, off) => Grab::LotNote(id, i, w, off),
                    g => g,
                })
        });
        d.grab = grip.or_else(|| {
            items.iter().rev().find_map(|it| {
                let PlanItem::Annotation { id, item } = it else {
                    return None;
                };
                if !item.hit(w, tol, mm) {
                    return None;
                }
                Some(match item.shape {
                    Shape::Note { at, .. } => Grab::Note(*id, w, at),
                    _ => Grab::Shape(*id),
                })
            })
        });
        match d.grab {
            Some(
                Grab::Note(id, ..)
                | Grab::Leader(id)
                | Grab::Shape(id)
                | Grab::Elbow(id)
                | Grab::TagEnd(id, _)
                | Grab::TagElbow(id, _)
                | Grab::LevelHead(id, _)
                | Grab::LotNote(id, ..),
            ) => selection.only(id),
            None => {}
        }
        d.grab_by = d.grab.map(|_| response.id);
    }
    if d.grab.is_some() && d.grab_by.is_some_and(|o| o != response.id) {
        return None;
    }
    let grab = d.grab?;
    if response.drag_stopped() || !response.dragged() {
        d.grab = None;
        d.grab_by = None;
        return Some(None);
    }
    let p = xf.to_world(response.interact_pointer_pos()?);
    let r = match grab {
        Grab::Note(id, from, start) => doc.edit_merged(id, "Move note", |e| {
            if let ElementKind::Note(n) = &mut e.kind {
                n.at = start + p - from;
            }
        }),
        Grab::Leader(id) => {
            let target = snapped.map_or(HostPoint::free(p), |s| s.0);
            doc.edit_merged(id, "Move leader", |e| {
                if let ElementKind::Note(n) = &mut e.kind {
                    n.leader = Some(target);
                }
            })
        }
        Grab::Shape(id) => {
            let shape = items.iter().find_map(|it| match it {
                PlanItem::Annotation { id: i, item } if *i == id => Some(item.shape.clone()),
                _ => None,
            });
            let (Some(shape), Some(view)) = (shape, view) else {
                return Some(None);
            };
            let model = doc.model().clone();
            doc.edit_merged(id, "Move dimension line", |e| {
                let ElementKind::Dimension(dim) = &mut e.kind else {
                    return;
                };
                match shape {
                    Shape::Angle { vertex, .. } => dim.offset = p.distance(vertex),
                    Shape::Radius { center, at, .. } => {
                        let r = at.distance(center);
                        let dir = (p - center).normalize_or(DVec2::X);
                        let on = attach(&model, view, dim.a.host, center + dir * r);
                        dim.a = on;
                        dim.b = on;
                        dim.offset = (p.distance(center) - r).max(0.0);
                    }
                    Shape::ArcLength { center, a, .. } => {
                        dim.offset = p.distance(center) - a.distance(center)
                    }
                    // a spot level's text moves, staying with what it is on
                    Shape::Spot { .. } => dim.b = attach(&model, view, dim.a.host, p),
                    Shape::Note { .. } | Shape::Level { .. } => {}
                }
            })
        }
        Grab::Elbow(id) => doc.edit_merged(id, "Move leader elbow", |e| {
            if let ElementKind::Note(n) = &mut e.kind {
                n.elbow = Some(p);
            }
        }),
        Grab::TagEnd(id, centre) => doc.edit_merged(id, "Move leader", |e| {
            if let ElementKind::Tag(t) = &mut e.kind {
                t.end = Some(p - centre);
            }
        }),
        Grab::TagElbow(id, centre) => doc.edit_merged(id, "Move leader elbow", |e| {
            if let ElementKind::Tag(t) = &mut e.kind {
                t.elbow = Some(p - centre);
            }
        }),
        Grab::LotNote(id, i, from, off) => doc.edit_merged(id, "Move edge note", |e| {
            if let ElementKind::Site(creabim_core::Site {
                form: creabim_core::SiteForm::Property { rules, .. },
                ..
            }) = &mut e.kind
            {
                rules.set_note_off(i, Some(off + p - from));
            }
        }),
        Grab::LevelHead(level, end) => {
            let Some(view_id) = view_id else {
                return Some(None);
            };
            doc.edit_merged(view_id, "Move level head", |e| {
                if let ElementKind::View(v) = &mut e.kind {
                    v.level_heads.entry(level).or_default().offset = Some(p - end);
                }
            })
        }
    };
    Some(r.err().map(|e| e.to_string()))
}

/// The grips drawn on selected notes' and tags' leaders (their ends and
/// elbows) and on selected levels' heads.
pub fn draw_grips(
    painter: &egui::Painter,
    xf: &Xf,
    model: &Model,
    items: &[PlanItem],
    selection: &Selection,
    mm: f64,
) {
    // a selected room tag: traced in the accent (planview::trace) and one
    // square to move it by, no box round it
    let mut boxes: Vec<(ElementId, egui::Rect)> = vec![];
    for it in items {
        let PlanItem::Annotation { id, item } = it else {
            continue;
        };
        if !selection.contains(*id) || !matches!(item.shape, Shape::Note { .. }) {
            continue;
        }
        if !matches!(model.get(*id).map(|e| &e.kind), Some(ElementKind::Tag(_))) {
            continue;
        }
        for b in item.text_boxes() {
            let r = egui::Rect::from_points(&b.map(|p| xf.to_screen(p)));
            match boxes.iter_mut().find(|(i, _)| i == id) {
                Some((_, all)) => *all = all.union(r),
                None => boxes.push((*id, r)),
            }
        }
    }
    for (_, r) in boxes {
        let r = r.expand(5.0);
        painter.rect(
            egui::Rect::from_center_size(r.center_top(), Vec2::splat(8.0)),
            1.0,
            Color32::WHITE,
            Stroke::new(1.5, interaction_ink()),
            egui::StrokeKind::Middle,
        );
    }
    for it in items {
        if !selection.contains(it.id()) {
            continue;
        }
        for (p, g) in grips(model, it, mm) {
            let elbow = matches!(g, Grab::Elbow(_) | Grab::TagElbow(..));
            let r = egui::Rect::from_center_size(xf.to_screen(p), Vec2::splat(7.0));
            if elbow {
                painter.circle(
                    r.center(),
                    3.5,
                    Color32::WHITE,
                    Stroke::new(1.5, interaction_ink()),
                );
            } else {
                painter.rect(
                    r,
                    0.0,
                    Color32::WHITE,
                    Stroke::new(1.5, interaction_ink()),
                    egui::StrokeKind::Middle,
                );
            }
        }
    }
}

// ---- sheets -----------------------------------------------------------------

/// Notes on a sheet itself: placing them with the Note tool, picking and
/// dragging them. `xf` maps sheet millimetres to the screen. Returns true
/// when it took the pointer.
#[allow(clippy::too_many_arguments)]
pub fn sheet_input(
    doc: &mut Document,
    sheet: ElementId,
    painter: &egui::Painter,
    response: &egui::Response,
    tool: Tool,
    d: &mut Draft,
    selection: &mut Selection,
    xf: &Xf,
    status: &mut Option<String>,
    error: &mut Option<String>,
) -> bool {
    let items = creabim_geom::annotate::sheet_notes(doc.model(), sheet);
    let to_paper = |p: Pos2| xf.to_world(p);
    let zoom = xf.scale;
    for it in &items {
        if let PlanItem::Annotation { id, item } = it {
            // the sheet draws the note itself; a selected one is traced
            if selection.contains(*id) {
                paint_weighted(
                    painter,
                    xf,
                    item,
                    interaction_ink(),
                    1.0,
                    crate::planview::TRACE,
                );
            }
        }
    }
    draw_grips(painter, xf, doc.model(), &items, selection, 1.0);
    let pointer = response.hover_pos().or(response.interact_pointer_pos());
    if tool == Tool::Note {
        let raw = pointer.map(to_paper);
        preview(painter, xf, tool, d, raw, raw, &[], 0.0, 1.0);
        if response.clicked() {
            if let Some(p) = raw {
                let mut ghost = Selection::default();
                let r = sheet_note_click(doc, sheet, p, d, &mut ghost);
                match r {
                    Ok(msg) => *status = Some(msg),
                    Err(e) => *error = Some(e.to_string()),
                }
                if let Some(id) = ghost.active {
                    selection.only(id);
                }
            }
        }
        return true;
    }
    if tool != Tool::Select {
        return false;
    }
    if let Some(r) = drag(
        doc, None, None, &items, xf, response, d, selection, 1.0, None,
    ) {
        if let Some(e) = r {
            *error = Some(e);
        }
        return true;
    }
    if response.clicked() {
        if let Some(p) = pointer.map(to_paper) {
            let hit = items.iter().rev().find_map(|it| match it {
                PlanItem::Annotation { id, item } if item.hit(p, 6.0 / zoom as f64, 1.0) => {
                    Some(*id)
                }
                _ => None,
            });
            if let Some(id) = hit {
                selection.only(id);
                return true;
            }
        }
    }
    false
}

/// A click with the Note tool on a sheet: the leader's point, then the text.
fn sheet_note_click(
    doc: &mut Document,
    sheet: ElementId,
    p: DVec2,
    d: &mut Draft,
    selection: &mut Selection,
) -> Result<String, ModelError> {
    if d.leader && d.ready.is_none() {
        d.ready = Some(Ready::Leader {
            target: (HostPoint::free(p), p),
        });
        return Ok("Now click where the note's text goes.".into());
    }
    let mut n = Note::new(sheet, p, "Note");
    if let Some(Ready::Leader { target }) = d.ready.take() {
        n.leader = Some(target.0);
    }
    d.clear();
    let e = d.styled(Element::new("Note", ElementKind::Note(n)));
    selection.only(e.id);
    doc.commit(Transaction::new("Add note").insert(e))?;
    Ok(added(Tool::Note, d))
}

// ---- properties -------------------------------------------------------------

/// What a style picker did.
pub enum LookEdit {
    None,
    Changed(&'static str),
    /// "New text style…" was picked.
    NewStyle,
}

/// The styles meant for `purpose` (and those for any annotation), by name.
pub fn styles_for(model: &Model, purpose: StyleUse) -> Vec<(ElementId, String)> {
    model
        .iter()
        .filter(|e| match &e.kind {
            ElementKind::TextStyle(s) => s.purpose == purpose || s.purpose == StyleUse::Any,
            _ => false,
        })
        .map(|e| (e.id, e.name.clone()))
        .collect()
}

/// The dimensions, tags and notes that use a style.
pub fn style_user_ids(model: &Model, id: ElementId) -> Vec<ElementId> {
    model
        .iter()
        .filter(|e| style_of(&e.kind) == Some(id))
        .map(|e| e.id)
        .collect()
}

/// The text styles in the model, by name.
pub fn styles(model: &Model) -> Vec<(ElementId, String)> {
    model
        .iter()
        .filter(|e| matches!(e.kind, ElementKind::TextStyle(_)))
        .map(|e| (e.id, e.name.clone()))
        .collect()
}

/// A text style picker and a colour of its own, for a dimension, tag or note.
pub fn look_ui(
    ui: &mut egui::Ui,
    model: &Model,
    style: &mut Option<ElementId>,
    color: &mut Option<[f32; 3]>,
) -> LookEdit {
    let all = styles(model);
    let name = style
        .and_then(|id| all.iter().find(|s| s.0 == id))
        .map_or("Default", |s| s.1.as_str())
        .to_string();
    let mut out = LookEdit::None;
    crate::app::helpui::combo(ui, "Text style")
        .selected_text(name)
        .show_ui(ui, |ui| {
            if ui.selectable_value(style, None, "Default").changed() {
                out = LookEdit::Changed("Text style");
            }
            for (id, n) in &all {
                if ui.selectable_value(style, Some(*id), n).changed() {
                    out = LookEdit::Changed("Text style");
                }
            }
            ui.separator();
            if ui.button("New text style…").clicked() {
                out = LookEdit::NewStyle;
            }
        });
    // the switch, then the swatch on a row of its own (MU34: the swatch
    // sat past the panel's edge)
    let mut own = color.is_some();
    if crate::props::switch(ui, &mut own, "Own colour")
        .on_hover_text("Override the style's colour for this one")
        .changed()
    {
        *color = own.then_some([0.8, 0.1, 0.1]);
        out = LookEdit::Changed("Annotation colour");
    }
    if let Some(c) = color {
        crate::props::row(ui, "Colour", |ui| {
            let h = ui.spacing().interact_size.y;
            ui.spacing_mut().interact_size.x = ui.available_width().max(40.0);
            ui.spacing_mut().interact_size.y = h;
            if crate::props::colour_button_rgb(ui, c).changed() {
                out = LookEdit::Changed("Annotation colour");
            }
        });
    }
    out
}

/// Add a text style, named after how many there are. Returns its id.
pub fn new_text_style(doc: &mut Document) -> Result<ElementId, ModelError> {
    let n = styles(doc.model()).len() + 1;
    let e = Element::new(
        format!("Text style {n}"),
        ElementKind::TextStyle(TextStyle::default()),
    );
    let id = e.id;
    doc.commit(Transaction::new("New text style").insert(e))?;
    Ok(id)
}

/// Point an annotation at a text style.
pub fn set_style(kind: &mut ElementKind, id: ElementId) {
    match kind {
        ElementKind::Dimension(d) => d.style = Some(id),
        ElementKind::Tag(t) => t.style = Some(id),
        ElementKind::Note(n) => n.style = Some(id),
        _ => {}
    }
}

/// A text style's font, size, colour, weight and slant.
pub fn style_ui(ui: &mut egui::Ui, s: &mut TextStyle) -> Option<&'static str> {
    let mut label = None;
    crate::app::helpui::combo(ui, "Font")
        .selected_text(s.font.label())
        .height(560.0)
        .show_ui(ui, |ui| {
            ui.set_min_width(230.0);
            // grouped, each name written in its own face, so the list
            // shows what a font looks like before it is picked
            let mut group = None;
            for f in Font::ALL {
                if group != Some(f.group()) {
                    group = Some(f.group());
                    ui.add_space(4.0);
                    let head = match f.group() {
                        creabim_core::FontGroup::Standard => "IN EVERY PDF",
                        creabim_core::FontGroup::CreaBim => "CREABIM",
                        creabim_core::FontGroup::Classic => "CLASSICS",
                    };
                    ui.label(egui::RichText::new(head).small().weak());
                }
                let text = egui::RichText::new(f.label())
                    .family(family_of(f))
                    .size(14.0);
                let r = ui.selectable_value(&mut s.font, f, text);
                let r = if f.bundled() {
                    r.on_hover_text(
                        "Shipped with CreaBIM: it goes inside the SVGs and PDFs that use it",
                    )
                } else {
                    r
                };
                if r.changed() {
                    label = Some("Text font");
                }
            }
        });
    // full-width fields in the label column, as every other row (MU29)
    crate::props::row(ui, "Size on paper", |ui| {
        if crate::props::fill(ui, crate::units::paper_small(&mut s.size).range(0.5..=50.0))
            .changed()
        {
            label = Some("Text size");
        }
    });
    crate::props::flow(ui, |ui| {
        let mut own = s.color.is_some();
        if crate::props::switch(ui, &mut own, "Colour")
            .on_hover_text("Off: black on paper, blue on screen")
            .changed()
        {
            s.color = own.then_some([0.0, 0.0, 0.0]);
            label = Some("Text colour");
        }
        if let Some(c) = &mut s.color {
            if crate::props::colour_button_rgb(ui, c).changed() {
                label = Some("Text colour");
            }
        }
    });
    // Blender's rows (MU31): toggles in the field column, then how the
    // lines line up
    crate::props::row(ui, "Emphasis", |ui| {
        // a fixed row in whole pixels: wrapping, it flickered as the panel was resized (MU59)
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 1.0;
            let h = ui.spacing().interact_size.y;
            let w = ((ui.available_width() - 1.0) / 2.0).floor().max(30.0);
            if ui
                .add_sized([w, h], egui::Button::selectable(s.bold, "Bold"))
                .clicked()
            {
                s.bold = !s.bold;
                label = Some("Bold text");
            }
            if ui
                .add_sized([w, h], egui::Button::selectable(s.italic, "Italic"))
                .clicked()
            {
                s.italic = !s.italic;
                label = Some("Italic text");
            }
        })
    });
    use creabim_core::TextAlign;
    // a list, not four buttons: at the panel's narrowest they ran past its
    // edge, and every row under them with them (MU141)
    let aligns = [
        (
            TextAlign::Usual,
            "Usual",
            "Each kind's own: notes from the left, room tags centred",
        ),
        (
            TextAlign::Left,
            "Left",
            "Every line from the block's left edge",
        ),
        (
            TextAlign::Centre,
            "Centre",
            "Every line on the block's middle",
        ),
        (
            TextAlign::Right,
            "Right",
            "Every line against the block's right edge",
        ),
    ];
    let now = aligns
        .iter()
        .find(|a| a.0 == s.align)
        .map_or("Usual", |a| a.1);
    crate::app::helpui::combo(ui, "Align")
        .selected_text(now)
        .show_ui(ui, |ui| {
            for (v, name, tip) in aligns {
                if ui
                    .selectable_value(&mut s.align, v, name)
                    .on_hover_text(tip)
                    .changed()
                {
                    label = Some("Text alignment");
                }
            }
        });
    if let Some(l) = ends_ui(ui, s) {
        label = Some(l);
    }
    label
}

/// A terminator picker, with "Usual" for none.
fn terminator_combo(
    ui: &mut egui::Ui,
    salt: &str,
    text: &str,
    usual: &str,
    t: &mut Option<Terminator>,
) -> bool {
    let mut changed = false;
    crate::props::row(ui, text, |ui| {
        egui::ComboBox::from_id_salt(salt)
            .icon(crate::props::combo_icon)
            .selected_text(t.map_or(usual.to_string(), |t| t.label().to_string()))
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                changed |= ui.selectable_value(t, None, usual).changed();
                for k in Terminator::ALL {
                    changed |= ui.selectable_value(t, Some(k), k.label()).changed();
                }
            });
    });
    changed
}

/// The end of one tag's or note's leader, and its elbow: picked here or
/// left to its style. Returns the undo label of a change.
pub fn leader_ui(
    ui: &mut egui::Ui,
    end: &mut Option<Terminator>,
    usual: &str,
    elbow: &mut Option<DVec2>,
) -> Option<&'static str> {
    let mut label = None;
    crate::props::flow(ui, |ui| {
        if terminator_combo(ui, "leader_end_own", "Leader end", usual, end) {
            label = Some("Leader end");
        }
    });
    if elbow.is_some()
        && ui
            .button("Straighten the leader")
            .on_hover_text("Take its elbow out; drag the round grip on the leader to bend it again")
            .clicked()
    {
        *elbow = None;
        label = Some("Straighten leader");
    }
    label
}

/// A style's line ends, tag shape and what it is for.
pub fn ends_ui(ui: &mut egui::Ui, s: &mut TextStyle) -> Option<&'static str> {
    let mut label = None;
    crate::props::flow(ui, |ui| {
        if terminator_combo(
            ui,
            "style_end",
            "Dimension ends",
            "Usual (tick, arrow)",
            &mut s.terminator,
        ) {
            label = Some("Dimension ends");
        }
    });
    crate::props::flow(ui, |ui| {
        if terminator_combo(
            ui,
            "style_leader",
            "Leader end",
            "Usual (arrow)",
            &mut s.leader_end,
        ) {
            label = Some("Leader end");
        }
    });
    crate::props::row(ui, "Leader", |ui| {
        if crate::props::segment(
            ui,
            &mut s.leader_shape,
            &[
                (
                    creabim_core::LeaderShape::Straight,
                    "Straight",
                    "Straight runs, bent at the elbow",
                ),
                (
                    creabim_core::LeaderShape::Curved,
                    "Curved",
                    "A smooth curve to what it points at",
                ),
            ],
        ) {
            label = Some("Leader shape");
        }
    });
    crate::props::row(ui, "End size", |ui| {
        let mut size = s.end_size.unwrap_or(creabim_geom::annotation::END_SIZE);
        if crate::props::fill(ui, crate::units::paper_small(&mut size).range(0.5..=10.0)).changed()
        {
            s.end_size = Some(size);
            label = Some("Terminator size");
        }
    });
    crate::app::helpui::combo(ui, "Tag shape")
        .selected_text(s.tag_shape.label())
        .show_ui(ui, |ui| {
            for k in TagShape::ALL {
                if ui
                    .selectable_value(&mut s.tag_shape, k, k.label())
                    .changed()
                {
                    label = Some("Tag shape");
                }
            }
        });
    if crate::props::switch(ui, &mut s.break_at_tags, "Break dimensions at tags")
        .on_hover_text(
            "Dimension lines in this style leave a gap where a tag or a note sits across them",
        )
        .changed()
    {
        label = Some("Dimension breaks");
    }
    crate::app::helpui::combo(ui, "Meant for")
        .selected_text(s.purpose.label())
        .show_ui(ui, |ui| {
            for k in StyleUse::ALL {
                if ui.selectable_value(&mut s.purpose, k, k.label()).changed() {
                    label = Some("Style use");
                }
            }
        });
    label
}

/// The text style a dimension, tag or note refers to.
pub fn style_of(kind: &ElementKind) -> Option<ElementId> {
    match kind {
        ElementKind::Dimension(d) => d.style,
        ElementKind::Tag(t) => t.style,
        ElementKind::Note(n) => n.style,
        _ => None,
    }
}

/// The style an annotation uses, editable in place under its picker, so a
/// change shows at once on everything using it. `open` unfolds it (for a
/// style just made). Returns a status message after an edit.
pub fn style_inline(
    ui: &mut egui::Ui,
    doc: &mut Document,
    kind: &ElementKind,
    open: bool,
) -> Option<String> {
    let id = style_of(kind)?;
    let e = doc.model().get(id)?;
    let ElementKind::TextStyle(mut s) = e.kind else {
        return None;
    };
    let users = style_users(doc.model(), id);
    let mut label = None;
    // a section of its own, with the panel's chevron (MU26)
    let key = egui::Id::new(("style.open", id));
    if open {
        ui.ctx().data_mut(|d| d.insert_temp(key, true));
    }
    let shown = ui.ctx().data(|d| d.get_temp::<bool>(key)).unwrap_or(false);
    let title = format!("Text style \u{201c}{}\u{201d}", e.name);
    let (resp, _) = crate::props::header(ui, &title, shown, false);
    if resp.clicked() {
        ui.ctx().data_mut(|d| d.insert_temp(key, !shown));
    }
    // its body folds smoothly (AMD-017)
    crate::props::fold(ui, shown, |ui| {
        label = style_ui(ui, &mut s);
        crate::props::worked_out(
            ui,
            "text-style",
            &[crate::props::Worked::new("Used by", users.to_string())],
        );
        crate::props::info(ui, "A change here changes every one of them.", "");
    });
    let label = label?;
    let r = doc.edit_merged(id, label, |e| e.kind = ElementKind::TextStyle(s));
    Some(match r {
        Ok(()) => format!("{label}."),
        Err(e) => format!("Couldn't do that: {e}"),
    })
}

/// How many dimensions, tags and notes use a style.
pub fn style_users(model: &Model, id: ElementId) -> usize {
    model
        .iter()
        .filter(|e| match &e.kind {
            ElementKind::Dimension(d) => d.style == Some(id),
            ElementKind::Tag(t) => t.style == Some(id),
            ElementKind::Note(n) => n.style == Some(id),
            _ => false,
        })
        .count()
}

/// What a dimension's offset means, for its kind.
pub fn offset_label(kind: DimensionKind) -> &'static str {
    match kind {
        DimensionKind::Linear => "Line offset",
        DimensionKind::Angular => "Arc radius",
        DimensionKind::Radius | DimensionKind::Diameter => "Line past circle",
        DimensionKind::ArcLength => "Line offset",
        DimensionKind::Spot => "Offset",
    }
}

/// The note's text (MU83, the owner's sketch): "Text" on the left and the
/// marks and large-editor buttons on the right of one line, the box under
/// them across the whole panel, as text areas always span it.
fn note_text_row(
    ui: &mut egui::Ui,
    text: &mut String,
    pop_out: &mut bool,
    marked: &mut bool,
) -> egui::Response {
    crate::props::flow(ui, |ui| {
        let muted = ui.visuals().widgets.noninteractive.fg_stroke.color;
        ui.label(egui::RichText::new("Text").color(muted));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if crate::props::icon_button(
                ui,
                "maximise",
                "Edit in a larger window (or double-click the note)",
            )
            .clicked()
            {
                *pop_out = true;
            }
            *marked = marks_menu(ui, text, false);
        });
    });
    egui::ScrollArea::vertical()
        .id_salt("note.text")
        .max_height(110.0)
        .show(ui, |ui| {
            ui.add(
                egui::TextEdit::multiline(text)
                    .desired_rows(4)
                    .desired_width(f32::INFINITY),
            )
        })
        .inner
}

/// Signs and words that drawings often carry (MU81): offered beside text
/// fields, so they need no typing.
pub const MARKS: [(&str, &str); 24] = [
    ("±", "plus or minus"),
    ("Ø", "diameter"),
    ("°", "degrees"),
    ("²", "squared"),
    ("³", "cubed"),
    ("×", "by"),
    ("≈", "about"),
    ("′", "feet"),
    ("″", "inches"),
    ("℄", "centre line"),
    ("∆", "change, revision"),
    ("½", "half"),
    ("¼", "quarter"),
    ("¾", "three quarters"),
    ("TYP.", "typical"),
    ("CLR", "clear"),
    ("MIN", "minimum"),
    ("MAX", "maximum"),
    ("O.C.", "on centre"),
    ("EQ", "equal"),
    ("U.N.O.", "unless noted otherwise"),
    ("V.I.F.", "verify in field"),
    ("FFL", "finished floor level"),
    ("NTS", "not to scale"),
];

/// A small button that opens [`MARKS`]; a pick is added to `text` (after a
/// space when it is a word, or always when `words` says the field takes
/// whole words). True when the text changed.
pub fn marks_menu(ui: &mut egui::Ui, text: &mut String, words: bool) -> bool {
    let mut changed = false;
    let r = crate::props::icon_button(ui, "diameter", "Add a sign or word: ±, Ø, °, TYP., CLR…");
    egui::Popup::menu(&r).show(|ui| {
        ui.set_min_width(200.0);
        egui::Grid::new(ui.id().with("marks"))
            .num_columns(2)
            .show(ui, |ui| {
                for (i, (m, what)) in MARKS.iter().enumerate() {
                    if ui.button(*m).on_hover_text(*what).clicked() {
                        let word = words || m.chars().count() > 1;
                        if word && !text.is_empty() && !text.ends_with(' ') && !text.ends_with('\n')
                        {
                            text.push(' ');
                        }
                        text.push_str(m);
                        changed = true;
                        ui.close();
                    }
                    if i % 2 == 1 {
                        ui.end_row();
                    }
                }
            });
    });
    changed
}

/// A linear dimension's Text fields (MU80, A on all three; MU81): its own
/// text instead of the measure, words before and after, a note below, its
/// place along the line, the leader of moved text, and Move text, which
/// gives the text a grip. Returns the undo label when something changed.
pub fn dim_text_ui(ui: &mut egui::Ui, t: &mut creabim_core::DimText) -> Option<&'static str> {
    use creabim_core::{DimLeader, DimPlace};
    let mut label = None;
    // a tick and a list, not segments: four segments in a narrow panel cut
    // their names to "…" (checked in the test copy)
    if crate::props::check(ui, "Value", &mut t.instead, "Text instead")
        .on_hover_text("Your own text instead of the measure; <> in it brings the measure back. Off, the measure, kept up to date.")
        .changed()
    {
        label = Some("Dimension value");
    }
    // each field fills the value column; its hint shows greyed inside it
    // and in full on hover, never as a loose line (MU82)
    let field =
        |ui: &mut egui::Ui, name: &str, s: &mut String, hint: &str, tip: &str, words: bool| {
            let mut changed = false;
            crate::props::row(ui, name, |ui| {
                let w = (ui.available_width() - 24.0).max(40.0);
                changed |=
                    crate::props::text_field(ui, egui::TextEdit::singleline(s).hint_text(hint), w)
                        .on_hover_text(tip)
                        .changed();
                changed |= marks_menu(ui, s, words);
            });
            changed
        };
    if t.instead
        && field(
            ui,
            "Text",
            &mut t.text,
            "EQ or <> MIN",
            "Your text instead of the number. <> puts the measure back in: <> MIN reads 3810 MIN",
            false,
        )
    {
        label = Some("Dimension text");
    }
    if field(
        ui,
        "Before",
        &mut t.before,
        "±",
        "On the line before the number: ±, ≈, Ø, R",
        false,
    ) {
        label = Some("Dimension text");
    }
    if field(
        ui,
        "After",
        &mut t.after,
        "CLR",
        "On the line after the number: TYP., CLR, MIN, O.C.",
        true,
    ) {
        label = Some("Dimension text");
    }
    if field(
        ui,
        "Below",
        &mut t.below,
        "FACE OF STUD",
        "A line of smaller text under the dimension line",
        true,
    ) {
        label = Some("Dimension note");
    }
    let mut place = t.place;
    let places = [
        (DimPlace::Centre, "Centre", "Over the middle of the line"),
        (
            DimPlace::Left,
            "Left",
            "Past the line's start, the line running on under it",
        ),
        (
            DimPlace::Right,
            "Right",
            "Past the line's end, the line running on under it",
        ),
        (
            DimPlace::Moved,
            "Moved",
            "Where its grip was dragged (tick Move text)",
        ),
    ];
    crate::app::helpui::combo(ui, "Place")
        .selected_text(
            places
                .iter()
                .find(|p| p.0 == place)
                .map_or("Centre", |p| p.1),
        )
        .show_ui(ui, |ui| {
            for (v, name, tip) in places {
                ui.selectable_value(&mut place, v, name).on_hover_text(tip);
            }
        });
    if place != t.place {
        t.place = place;
        if place == DimPlace::Moved {
            t.movable = true;
        }
        label = Some("Dimension text place");
    }
    if t.place == DimPlace::Moved {
        let mut leader = t.leader;
        crate::app::helpui::combo(ui, "Leader")
            .selected_text(match leader {
                DimLeader::WhenOutside => "When outside",
                DimLeader::Always => "Always",
                DimLeader::Never => "Never",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut leader, DimLeader::WhenOutside, "When outside")
                    .on_hover_text("Only when the text sits outside the witness lines");
                ui.selectable_value(&mut leader, DimLeader::Always, "Always");
                ui.selectable_value(&mut leader, DimLeader::Never, "Never");
            });
        if leader != t.leader {
            t.leader = leader;
            label = Some("Dimension leader");
        }
    }
    if crate::props::check(ui, "", &mut t.movable, "Move text")
        .on_hover_text("Gives this dimension's text a grip to drag it by; the others stay put, so none moves by mistake")
        .changed()
    {
        label = Some("Move dimension text");
    }
    if !t.is_plain()
        && crate::props::action(
            ui,
            "",
            "Back to the plain number",
            "Clears all of the above",
            true,
        )
        .clicked()
    {
        *t = Default::default();
        label = Some("Dimension text");
    }
    label
}

/// A note's text and leader. `unit` is a drawing unit's worth of a
/// leader's default reach (larger on sheets, which are in mm).
pub fn note_ui(
    ui: &mut egui::Ui,
    n: &mut Note,
    focus: &mut bool,
    unit: f64,
    pop_out: &mut bool,
) -> Option<&'static str> {
    let mut label = None;
    // the field in the value column like every other (MU81), keeping its
    // size and scrolling inside; the buttons beside it add a drawing mark
    // or open the large editor (MU27)
    let mut marked = false;
    // a note showing a notes section is written in the Notes area
    let linked = n.section.is_some();
    let r = (!linked).then(|| note_text_row(ui, &mut n.text, pop_out, &mut marked));
    if marked {
        label = Some("Note text");
    }
    if let Some(r) = r.as_ref().filter(|_| std::mem::take(focus)) {
        r.request_focus();
        // select it all, so typing replaces the placeholder
        if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), r.id) {
            let all = egui::text::CCursorRange::two(
                egui::text::CCursor::new(0),
                egui::text::CCursor::new(n.text.chars().count()),
            );
            state.cursor.set_char_range(Some(all));
            state.store(ui.ctx(), r.id);
        }
    }
    if r.is_some_and(|r| r.changed()) {
        label = Some("Note text");
    }
    // a text box (MU26): wraps to a width, reads markdown
    let mut boxed = n.width.is_some();
    crate::props::row(ui, "Box", |ui| {
        crate::props::flow(ui, |ui| {
            if crate::props::tickbox(ui, &mut boxed, "Wrap at")
                .on_hover_text(
                    "Lines wrap to this width on paper; off, each line runs as long as it is",
                )
                .changed()
            {
                n.width = boxed.then_some(60.0);
                label = Some("Note box");
            }
            if let Some(w) = &mut n.width {
                if ui.add(crate::units::paper(w).range(5.0..=800.0)).changed() {
                    label = Some("Note box");
                }
                // how the box and Continue tie together (MU91)
                crate::props::help_ring(
                    ui,
                    "Wrap at",
                    "The box's width: every line wraps to it. On a sheet, Text flow adds a height; \
                     the text stops there and Continue carries the rest into blocks of the same width, \
                     so changing the width here reflows every part.",
                    "notes#long-notes",
                );
            }
        });
    });
    if !linked
        && crate::props::switch(ui, &mut n.markdown, "Markdown")
        .on_hover_text("# heading (capitals, underlined), - bullet, 1. numbered, - [ ] and - [x] checkboxes; **bold** and *italic* marks are dropped on paper")
        .changed()
    {
        label = Some("Note markdown");
    }
    let mut leader = n.leader.is_some();
    if crate::props::switch(ui, &mut leader, "Leader")
        .on_hover_text("An arrow from the note to what it is about. Drag its square to move it.")
        .changed()
    {
        // off keeps its point, so on again points the same way (MU135)
        let fresh = n.at + DVec2::new(-1.5, -1.0) * unit;
        n.set_leader(leader, fresh);
        label = Some("Note leader");
    }
    if n.leader.is_some_and(|l| l.host.is_some()) {
        crate::props::worked_out(
            ui,
            "note-leader",
            &[crate::props::Worked::new(
                "Arrow",
                "Attached: follows what it points at",
            )],
        );
    }
    label
}

#[cfg(test)]
mod tests {
    use super::*;
    use creabim_core::ViewKind;
    use creabim_geom::plan::Pen;

    /// MU142 (seen in wave3m): a section's level tags are notes drawn for
    /// the terrain; with the terrain selected they showed a note's leader
    /// grips, which edit Note elements only and so did nothing. A note
    /// drawn for anything but a Note or a Tag has none of those (a level
    /// tag has grips of its own, over it).
    #[test]
    fn a_note_drawn_for_a_terrain_shows_no_note_grips() {
        let lv = creabim_core::Element::new(
            "Ground",
            ElementKind::Level(creabim_core::Level {
                elevation: 0.0,
                height: 3.0,
            }),
        );
        let t = creabim_core::Site::terrain(
            lv.id,
            creabim_core::polygon::rectangle(DVec2::ZERO, DVec2::new(10.0, 10.0)),
            0.0,
            1.0,
        );
        let te = creabim_core::Element::new("Terrain", ElementKind::Site(t));
        let (tid, m) = (te.id, Model::from_elements([lv, te]).unwrap());
        let level = creabim_geom::annotate::room_tag_item(
            tid,
            "+2.00".into(),
            DVec2::new(5.0, 3.0),
            creabim_geom::annotate::look_of(&m, None, None),
            0.1,
        );
        let mut level = level;
        if let PlanItem::Annotation { item, .. } = &mut level {
            if let Shape::Note { leader, .. } = &mut item.shape {
                *leader = Some(DVec2::new(5.0, 1.0));
            }
        }
        assert!(grips(&m, &level, 0.1).is_empty());
    }

    fn plan_of(doc: &Document) -> (ElementId, View) {
        doc.model()
            .iter()
            .find_map(|e| match &e.kind {
                ElementKind::View(v) if matches!(v.kind, ViewKind::Plan { .. }) => {
                    Some((e.id, v.clone()))
                }
                _ => None,
            })
            .unwrap()
    }

    /// The one element added since the model had `before`.
    fn added_since(doc: &Document, before: &[ElementId]) -> Element {
        let new: Vec<&Element> = doc
            .model()
            .iter()
            .filter(|e| !before.contains(&e.id))
            .collect();
        assert_eq!(new.len(), 1);
        new[0].clone()
    }

    fn ids(doc: &Document) -> Vec<ElementId> {
        doc.model().iter().map(|e| e.id).collect()
    }

    #[test]
    fn two_lines_make_an_angle_on_the_clicked_side() {
        let mut doc = Document::from_model(creabim_core::sample::sample_house());
        let (vid, view) = plan_of(&doc);
        let line = |a: DVec2, b: DVec2| PlanItem::Line {
            id: ElementId::NIL,
            pen: Pen::Projection,
            a,
            b,
        };
        let items = [
            line(DVec2::new(0.0, 0.0), DVec2::new(4.0, 0.0)),
            line(DVec2::new(1.0, 1.0), DVec2::new(1.0, 5.0)),
        ];
        let (mut d, mut sel) = (Draft::default(), Selection::default());
        let before = ids(&doc);
        for at in [
            DVec2::new(3.0, 0.02),
            DVec2::new(1.03, 3.0),
            DVec2::new(2.0, 1.5),
        ] {
            click(
                &mut doc,
                vid,
                &view,
                Tool::Angle,
                None,
                at,
                &items,
                0.1,
                &mut d,
                &mut sel,
            )
            .unwrap();
        }
        let e = added_since(&doc, &before);
        let ElementKind::Dimension(dim) = &e.kind else {
            panic!("not a dimension")
        };
        assert_eq!(dim.kind, DimensionKind::Angular);
        assert!((dim.offset - DVec2::new(1.0, 1.5).length()).abs() < 1e-9);
        let drawing = creabim_geom::section::view_drawing(doc.model(), vid).unwrap();
        let text = drawing.items.iter().find_map(|i| match i {
            PlanItem::Annotation { id, item } if *id == e.id => Some(item.text.clone()),
            _ => None,
        });
        assert_eq!(text.as_deref(), Some("90°"));
        // undo takes it away
        doc.undo();
        assert_eq!(ids(&doc), before);
    }

    #[test]
    fn an_arc_gives_its_radius_and_a_note_its_leader() {
        let mut doc = Document::from_model(creabim_core::sample::sample_house());
        let (vid, view) = plan_of(&doc);
        let items = [PlanItem::Arc {
            id: ElementId::NIL,
            pen: Pen::Symbol,
            center: DVec2::new(5.0, 5.0),
            radius: 2.0,
            start: 0.0,
            end: std::f64::consts::PI,
        }];
        let (mut d, mut sel) = (Draft::default(), Selection::default());
        let before = ids(&doc);
        let mut go = |doc: &mut Document, tool: Tool, at: DVec2, items: &[PlanItem]| {
            click(
                doc, vid, &view, tool, None, at, items, 0.1, &mut d, &mut sel,
            )
            .unwrap();
        };
        go(&mut doc, Tool::Radius, DVec2::new(5.0, 7.03), &items);
        go(&mut doc, Tool::Radius, DVec2::new(8.0, 5.0), &items);
        let e = added_since(&doc, &before);
        let ElementKind::Dimension(dim) = &e.kind else {
            panic!("not a dimension")
        };
        assert_eq!(dim.kind, DimensionKind::Radius);
        assert!(dim.a.local.distance(DVec2::new(7.0, 5.0)) < 1e-9);
        assert!((dim.offset - 1.0).abs() < 1e-9);
        doc.undo();

        // a note: its arrow's point, then its text
        go(&mut doc, Tool::Note, DVec2::new(1.0, 1.0), &[]);
        go(&mut doc, Tool::Note, DVec2::new(3.0, 2.0), &[]);
        let e = added_since(&doc, &before);
        let ElementKind::Note(note) = &e.kind else {
            panic!("not a note")
        };
        assert_eq!(note.at, DVec2::new(3.0, 2.0));
        assert_eq!(note.leader.unwrap().local, DVec2::new(1.0, 1.0));
        assert!(d.focus_text);
        assert_eq!(sel.active, Some(e.id));
    }
}
