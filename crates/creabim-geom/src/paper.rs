//! Drawings on paper. A [`Canvas`] takes linework in millimetres with y up;
//! SVG, PDF and the app's sheet view all implement it, so a sheet looks the
//! same on screen as in its exports.

use crate::annotate::tag_geometry;
use crate::annotation::{Ends, Marks, TextLook};
use crate::looks::Looks;
use crate::plan::{Hatch, Pen, PlanItem, PlanView};
use crate::raster::Raster;
use creabim_core::{Font, LineWeights};
use glam::DVec2;
use std::fmt::Write;
use std::sync::Arc;

/// Line style on paper.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stroke {
    pub width: f64,
    /// 0 is black, 1 is white.
    pub gray: f64,
    /// Dash and gap lengths in mm, for dashed lines.
    pub dash: Option<[f64; 2]>,
    /// Short dashes between the long ones, each as long as the gap: 1
    /// is a dash-dot line (a setback), 2 a dash and two dots (a property
    /// line). 0 is a plain dashed line.
    pub dots: u8,
}

impl Stroke {
    /// Its dashes and gaps in turn, in mm: the long dash and a gap, then
    /// each short dash and a gap. None for a continuous line.
    pub fn dash_array(&self) -> Option<Vec<f64>> {
        let [a, b] = self.dash?;
        let mut out = vec![a, b];
        for _ in 0..self.dots {
            out.extend([b, b]);
        }
        Some(out)
    }

    pub const fn solid(width: f64, gray: f64) -> Self {
        Self {
            width,
            gray,
            dash: None,
            dots: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor {
    Start,
    Middle,
    End,
}

/// Something that draws in paper millimetres, y up.
pub trait Canvas {
    /// An open polyline.
    fn line(&mut self, pts: &[DVec2], stroke: Stroke);
    /// A filled polygon with no outline.
    fn fill(&mut self, pts: &[DVec2], gray: f64);
    /// Text centred vertically on `at`, turned `angle` radians counter-clockwise.
    fn text_angle(&mut self, at: DVec2, size: f64, text: &str, anchor: Anchor, angle: f64);
    /// An image spanning `lo`..`hi`, shown only inside `clip`.
    fn image(&mut self, lo: DVec2, hi: DVec2, clip: Option<(DVec2, DVec2)>, raster: &Arc<Raster>);

    /// An open polyline in a colour (sRGB, 0..1). Canvases without colour
    /// draw it as `line` would.
    fn line_rgb(&mut self, pts: &[DVec2], stroke: Stroke, _rgb: [f32; 3]) {
        self.line(pts, stroke);
    }

    /// A filled polygon in a colour (sRGB, 0..1); black without colour.
    fn fill_rgb(&mut self, pts: &[DVec2], _rgb: [f32; 3]) {
        self.fill(pts, 0.0);
    }

    /// A see-through wash of a colour over what is already drawn, at
    /// [`TINT_ALPHA`]. Canvases without transparency fill it with the
    /// colour mixed toward white instead.
    fn tint(&mut self, pts: &[DVec2], rgb: [f32; 3]) {
        self.fill_rgb(pts, rgb.map(|c| 1.0 - (1.0 - c) * TINT_ALPHA));
    }

    /// Text in a typeface, weight and colour. Canvases that only have one
    /// font draw it as `text_angle` would, at the look's size.
    fn styled_text(&mut self, at: DVec2, text: &str, anchor: Anchor, angle: f64, look: &TextLook) {
        self.text_angle(at, look.size, text, anchor, angle);
    }

    /// Horizontal text centred vertically on `at`.
    fn text(&mut self, at: DVec2, size: f64, text: &str, anchor: Anchor) {
        self.text_angle(at, size, text, anchor, 0.0);
    }

    fn polygon(&mut self, pts: &[DVec2], stroke: Stroke) {
        if let Some(first) = pts.first() {
            let mut closed = pts.to_vec();
            closed.push(*first);
            self.line(&closed, stroke);
        }
    }

    fn rect(&mut self, lo: DVec2, hi: DVec2, stroke: Stroke) {
        self.polygon(
            &[lo, DVec2::new(hi.x, lo.y), hi, DVec2::new(lo.x, hi.y)],
            stroke,
        );
    }

    fn circle(&mut self, c: DVec2, r: f64, stroke: Stroke) {
        let pts: Vec<DVec2> = (0..32)
            .map(|i| c + DVec2::from_angle(i as f64 / 32.0 * std::f64::consts::TAU) * r)
            .collect();
        self.polygon(&pts, stroke);
    }
}

/// Maps drawing coordinates (model metres) to paper millimetres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaperMap {
    /// Drawing point that lands on `paper`.
    pub model: DVec2,
    pub paper: DVec2,
    /// Scale denominator: 100 means 1:100.
    pub scale: f64,
}

impl PaperMap {
    /// Millimetres on paper per metre in the model.
    pub fn k(&self) -> f64 {
        1000.0 / self.scale
    }

    pub fn to_paper(&self, p: DVec2) -> DVec2 {
        self.paper + (p - self.model) * self.k()
    }

    pub fn to_model(&self, p: DVec2) -> DVec2 {
        self.model + (p - self.paper) / self.k()
    }
}

pub const CUT: Stroke = Stroke::solid(0.5, 0.0);
pub const PROJECTION: Stroke = Stroke::solid(0.25, 0.2);
pub const SYMBOL: Stroke = Stroke::solid(0.18, 0.0);
pub const GRID: Stroke = Stroke {
    width: 0.13,
    gray: 0.5,
    dash: Some([4.0, 1.0]),
    dots: 0,
};
/// Lines above the cut plane.
pub const HIDDEN: Stroke = Stroke {
    width: 0.13,
    gray: 0.25,
    dash: Some([2.0, 1.0]),
    dots: 0,
};
const HATCH: Stroke = Stroke::solid(0.09, 0.2);
/// Dimension and leader lines.
pub const DIMENSION: Stroke = Stroke::solid(0.13, 0.0);
pub const TICK: Stroke = Stroke::solid(0.35, 0.0);
/// Hatch line spacing on paper, mm.
pub const HATCH_SPACING: f64 = 1.2;

/// A pen's stroke at 1:100.
pub fn pen_stroke(pen: Pen) -> Stroke {
    pen_stroke_with(pen, &LineWeights::for_scale(100.0))
}

/// A pen's stroke with a view's line weights: cut heavy, projection
/// medium, symbols, furniture and hatching light, hidden lines light and
/// dashed.
pub fn pen_stroke_with(pen: Pen, w: &LineWeights) -> Stroke {
    match pen {
        Pen::Cut => Stroke {
            width: w.cut,
            ..CUT
        },
        Pen::Projection => Stroke {
            width: w.projection,
            ..PROJECTION
        },
        Pen::Symbol => Stroke {
            width: w.symbol,
            ..SYMBOL
        },
        Pen::Fine | Pen::Pattern => Stroke::solid(w.fine, 0.15),
        // contours: the lightest line, and one a good deal heavier, so
        // the heavier ones read first
        Pen::Contour => Stroke::solid(w.fine * 0.75, 0.38),
        Pen::ContourHeavy => Stroke::solid(w.projection * 1.2, 0.12),
        // a property line: heavier than what is built, a long dash and
        // two short ones, as surveys and site plans draw it
        Pen::Boundary => Stroke {
            width: w.cut * 0.8,
            gray: 0.0,
            dash: Some([9.0, 1.2]),
            dots: 2,
        },
        // a setback line: light, a dash and a dot
        Pen::Setback => Stroke {
            width: w.hidden * 1.2,
            gray: 0.2,
            dash: Some([5.0, 1.0]),
            dots: 1,
        },
        // the ground as it was: as light as a contour, dashed
        Pen::Existing => Stroke {
            width: w.fine * 0.75,
            gray: 0.45,
            dash: Some([2.0, 1.2]),
            dots: 0,
        },
        Pen::Grid | Pen::Guide | Pen::Cutter => GRID,
        Pen::Hidden => Stroke {
            width: w.hidden,
            ..HIDDEN
        },
        Pen::Underlay => Stroke {
            width: w.hatch,
            ..HATCH
        },
    }
}

/// Fill shade for a cut hatch: solid cuts are darker, hatched ones light.
pub fn hatch_fill(h: Hatch) -> f64 {
    match h {
        Hatch::Empty => 1.0,
        Hatch::Solid => 0.6,
        _ => 0.88,
    }
}

/// How strongly a [`PlanItem::Tint`] shows over what it lies on.
pub const TINT_ALPHA: f32 = 0.55;

/// Draw a generated drawing onto a canvas, with the line weights of its
/// scale.
pub fn draw(canvas: &mut dyn Canvas, drawing: &PlanView, map: &PaperMap) {
    draw_with(canvas, drawing, map, &LineWeights::for_scale(map.scale));
}

/// [`draw`] with a view's own line weights.
pub fn draw_with(canvas: &mut dyn Canvas, drawing: &PlanView, map: &PaperMap, w: &LineWeights) {
    draw_looks(canvas, drawing, map, &crate::looks::Looks::plain(*w));
}

/// [`draw`] with a view's looks: its pens, and its style pack's object
/// styles, patterns and overrides.
pub fn draw_looks(canvas: &mut dyn Canvas, drawing: &PlanView, map: &PaperMap, looks: &Looks) {
    // a palette other than Ink: every colour given through it (stage 7)
    if let Some(roles) = &looks.palette {
        let mut pal = crate::palettecanvas::PaletteCanvas {
            inner: canvas,
            roles: *roles,
        };
        return draw_looks_hand(&mut pal, drawing, map, looks);
    }
    draw_looks_hand(canvas, drawing, map, looks)
}

fn draw_looks_hand(canvas: &mut dyn Canvas, drawing: &PlanView, map: &PaperMap, looks: &Looks) {
    // a line style other than Ruled: every line drawn by hand, the rest as is
    if let Some(style) = &looks.hand {
        let mut hand = crate::handline::HandCanvas {
            inner: canvas,
            style: style.clone(),
        };
        // its leaders and ticks as the style draws them
        let mut styled = drawing.clone();
        styled.items = crate::handline::restyle(&drawing.items, style);
        return draw_looks_on(&mut hand, &styled, map, looks);
    }
    draw_looks_on(canvas, drawing, map, looks)
}

fn draw_looks_on(canvas: &mut dyn Canvas, drawing: &PlanView, map: &PaperMap, looks: &Looks) {
    let to = |p: DVec2| map.to_paper(p);
    if looks.has_surfaces() {
        let drawn = drawing.items.iter().map(|it| it.id()).collect();
        looks.surfaces(canvas, &drawn, map);
    }
    // true-size patterns run on across the pieces a layer was cut into
    let frames = crate::looks::layer_frames(&drawing.items, looks);
    for it in looks.order(&drawing.items) {
        match it {
            // guides and underlays are for working, not for print
            PlanItem::Line {
                pen: Pen::Guide | Pen::Underlay | Pen::Cutter,
                ..
            }
            | PlanItem::Arc {
                pen: Pen::Underlay, ..
            } => {}
            PlanItem::Fill {
                id,
                points,
                hatch,
                rgb,
                material,
            } => {
                let frame = crate::looks::frame_of(&frames, *id, *material, points);
                looks.fill(canvas, *id, points, *hatch, *rgb, *material, frame, map)
            }
            PlanItem::Tint { points, rgb, .. }
            | PlanItem::Room {
                points,
                rgb: Some(rgb),
                ..
            } => {
                let pts: Vec<DVec2> = points.iter().map(|p| to(*p)).collect();
                canvas.tint(&pts, *rgb);
            }
            PlanItem::Room { rgb: None, .. } => {}
            PlanItem::Shade { id, points, rgb } => looks.shade(canvas, *id, points, *rgb, map),
            PlanItem::Polygon { id, pen, points } => {
                if *pen == Pen::Cut {
                    looks.fill(canvas, *id, points, Hatch::Solid, None, None, None, map);
                }
                let mut pts: Vec<DVec2> = points.iter().map(|p| to(*p)).collect();
                if let Some(first) = pts.first() {
                    pts.push(*first);
                }
                looks.line(canvas, *id, *pen, &pts);
            }
            PlanItem::Line { id, pen, a, b } => looks.line(canvas, *id, *pen, &[to(*a), to(*b)]),
            PlanItem::Arc {
                id,
                pen,
                center,
                radius,
                start,
                end,
            } => {
                let pts: Vec<DVec2> = (0..=24)
                    .map(|i| {
                        let t = start + (end - start) * i as f64 / 24.0;
                        to(*center + DVec2::from_angle(t) * *radius)
                    })
                    .collect();
                looks.line(canvas, *id, *pen, &pts);
            }
            PlanItem::Symbol {
                symbol,
                at,
                text,
                unit,
                angle,
                origin,
                size,
                ..
            } => {
                let sh =
                    crate::symbol::shapes_at(symbol, *at, DVec2::splat(*unit), *angle, *origin);
                for f in &sh.fills {
                    let pts: Vec<DVec2> = f.iter().map(|p| to(*p)).collect();
                    canvas.fill(&pts, 0.0);
                }
                for (l, w) in &sh.lines {
                    let pts: Vec<DVec2> = l.iter().map(|p| to(*p)).collect();
                    canvas.line(&pts, Stroke::solid(*w, 0.0));
                }
                // text only where the symbol has a slot for it (MU84)
                if !text.is_empty() && symbol.text.is_some() {
                    canvas.text(to(sh.text_at), *size, text, Anchor::Middle);
                }
            }
            PlanItem::Label { at, text, .. } => {
                let p = to(*at);
                // short tags (grids, section marks) sit in a bubble
                if crate::plan::is_bubble(text) {
                    let r = 2.5 * crate::plan::bubble_scale(text);
                    // filled with the paper, so a grid line run on into
                    // it stops at its ring (MU74)
                    let disc: Vec<DVec2> = (0..32)
                        .map(|i| p + DVec2::from_angle(i as f64 / 32.0 * std::f64::consts::TAU) * r)
                        .collect();
                    canvas.fill(&disc, 1.0);
                    canvas.circle(p, r, Stroke::solid(0.18, 0.0));
                }
                canvas.text(p, 2.5, text, Anchor::Middle);
            }
            PlanItem::Text {
                at,
                text,
                size,
                align,
                rgb,
                bold,
                angle,
                ..
            } => {
                if rgb.is_some() || *bold {
                    let look = TextLook {
                        size: *size,
                        rgb: *rgb,
                        bold: *bold,
                        ..TextLook::default()
                    };
                    canvas.styled_text(to(*at), text, *align, *angle, &look);
                } else if *angle != 0.0 {
                    canvas.text_angle(to(*at), *size, text, *align, *angle)
                } else {
                    canvas.text(to(*at), *size, text, *align)
                }
            }
            PlanItem::Dimension {
                a,
                b,
                offset,
                text,
                look,
                witness,
                breaks,
                label,
                ..
            } => {
                let mut g = crate::annotate::dimension_layout(
                    *a,
                    *b,
                    *offset,
                    *witness,
                    look,
                    1.0 / map.k(),
                    text,
                    label,
                );
                g.break_line(breaks);
                for (p, q) in g.lines {
                    ink_line(canvas, &[to(p), to(q)], DIMENSION, look);
                }
                draw_ends(canvas, &g.ends, look, &to);
                canvas.styled_text(to(g.text_at), text, Anchor::Middle, g.angle, look);
                if !label.below.is_empty() {
                    let small = TextLook {
                        size: look.size * crate::annotate::BELOW_SIZE,
                        ..*look
                    };
                    canvas.styled_text(
                        to(g.below_at),
                        &label.below,
                        Anchor::Middle,
                        g.angle,
                        &small,
                    );
                }
            }
            PlanItem::Tag {
                anchor,
                at,
                elbow,
                text,
                round,
                look,
                ..
            } => {
                let g = tag_geometry(*anchor, *at, *elbow, text, *round, look, 1.0 / map.k());
                let mut pts: Vec<DVec2> = g.outline.iter().map(|p| to(*p)).collect();
                if g.closed {
                    canvas.fill(&pts, 1.0);
                    pts.push(pts[0]);
                }
                if pts.len() > 1 {
                    ink_line(canvas, &pts, SYMBOL, look);
                }
                draw_ends(canvas, &g.symbol, look, &to);
                if !g.leader.is_empty() {
                    let pts: Vec<DVec2> = g.leader.iter().map(|p| to(*p)).collect();
                    ink_line(canvas, &pts, DIMENSION, look);
                    draw_ends(canvas, &g.ends, look, &to);
                }
                canvas.styled_text(to(*at), text, Anchor::Middle, 0.0, look);
            }
            PlanItem::Annotation { item, .. } => {
                draw_marks(canvas, &item.marks(1.0 / map.k()), &item.look, &to);
            }
            PlanItem::Image {
                raster,
                lo,
                hi,
                clip,
                ..
            } => canvas.image(to(*lo), to(*hi), clip.map(|(a, b)| (to(a), to(b))), raster),
        }
    }
}

/// A line in an annotation's colour, or black without one.
fn ink_line(canvas: &mut dyn Canvas, pts: &[DVec2], stroke: Stroke, look: &TextLook) {
    match look.rgb {
        Some(rgb) => canvas.line_rgb(pts, stroke, rgb),
        None => canvas.line(pts, stroke),
    }
}

/// Draw an annotation's linework; `to` takes drawing units to paper mm.
pub fn draw_marks(
    canvas: &mut dyn Canvas,
    marks: &Marks,
    look: &TextLook,
    to: &dyn Fn(DVec2) -> DVec2,
) {
    for l in &marks.lines {
        let pts: Vec<DVec2> = l.iter().map(|p| to(*p)).collect();
        ink_line(canvas, &pts, DIMENSION, look);
    }
    for a in &marks.arrows {
        let pts: Vec<DVec2> = a.iter().map(|p| to(*p)).collect();
        canvas.fill_rgb(&pts, look.rgb.unwrap_or([0.0; 3]));
    }
    draw_ends(canvas, &marks.ends, look, to);
    for t in &marks.texts {
        draw_runs(canvas, to(t.at), &t.text, t.anchor, t.angle, look);
    }
}

/// A line of text, its bold and italic runs each in their own look, set
/// one after the other from the line's start (notes' `**bold**` and
/// `*italic*`).
pub fn draw_runs(
    canvas: &mut dyn Canvas,
    at: DVec2,
    text: &str,
    anchor: Anchor,
    angle: f64,
    look: &TextLook,
) {
    use crate::annotation::{runs_width, text_runs, BOLD, ITALIC};
    if !text.contains([BOLD, ITALIC]) {
        canvas.styled_text(at, text, anchor, angle, look);
        return;
    }
    let total = runs_width(text, look);
    let start = match anchor {
        Anchor::Start => 0.0,
        Anchor::Middle => -total / 2.0,
        Anchor::End => -total,
    };
    let dir = DVec2::from_angle(angle);
    for (off, run, l) in text_runs(text, look) {
        if !run.is_empty() {
            canvas.styled_text(at + dir * (start + off), &run, Anchor::Start, angle, &l);
        }
    }
}

/// Draw the ends of dimension lines and leaders: thin and heavy strokes
/// and filled shapes, in the annotation's colour.
pub fn draw_ends(
    canvas: &mut dyn Canvas,
    ends: &Ends,
    look: &TextLook,
    to: &dyn Fn(DVec2) -> DVec2,
) {
    let map = |l: &Vec<DVec2>| -> Vec<DVec2> { l.iter().map(|p| to(*p)).collect() };
    for l in &ends.thin {
        ink_line(canvas, &map(l), DIMENSION, look);
    }
    for l in &ends.heavy {
        ink_line(canvas, &map(l), TICK, look);
    }
    for f in &ends.fills {
        canvas.fill_rgb(&map(f), look.rgb.unwrap_or([0.0; 3]));
    }
}

/// An sRGB colour, 0..1, as `#rrggbb`.
pub fn hex(rgb: [f32; 3]) -> String {
    let c = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", c(rgb[0]), c(rgb[1]), c(rgb[2]))
}

/// Rough width of Helvetica text, mm.
pub fn text_width(text: &str, size: f64) -> f64 {
    text.chars()
        .map(|c| match c {
            'i' | 'j' | 'l' | '.' | ',' | ':' | ';' | '\'' | '|' | '!' => 0.25,
            ' ' | 'f' | 't' | 'r' | 'I' | '(' | ')' | '-' => 0.33,
            'm' | 'w' | 'M' | 'W' => 0.85,
            c if c.is_uppercase() || c.is_ascii_digit() => 0.63,
            _ => 0.54,
        })
        .sum::<f64>()
        * size
}

// ---- SVG -----------------------------------------------------------------

/// An SVG page; `finish` returns the document.
pub struct SvgCanvas {
    size: DVec2,
    body: String,
    clips: usize,
    /// Shipped fonts used, carried inside the file.
    fonts: Vec<Font>,
}

impl SvgCanvas {
    pub fn new(size: DVec2) -> Self {
        Self {
            size,
            body: String::new(),
            clips: 0,
            fonts: vec![],
        }
    }

    fn y(&self, p: DVec2) -> DVec2 {
        DVec2::new(p.x, self.size.y - p.y)
    }

    pub fn finish(self) -> String {
        // the shipped fonts it uses, inside it, so it reads the same anywhere
        let mut faces = String::new();
        if !self.fonts.is_empty() {
            faces.push_str("<defs><style>\n");
            for f in &self.fonts {
                if let Some(b) = crate::fonts::bytes(*f) {
                    let _ = writeln!(
                        faces,
                        "@font-face {{ font-family: '{}'; src: url(data:font/ttf;base64,{}) format('truetype'); }}",
                        f.family(),
                        crate::fonts::base64(b)
                    );
                }
            }
            faces.push_str("</style></defs>\n");
        }
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w:.1}mm\" height=\"{h:.1}mm\" viewBox=\"0 0 {w:.3} {h:.3}\">\n\
             {faces}<rect width=\"100%\" height=\"100%\" fill=\"white\"/>\n{}</svg>\n",
            self.body,
            w = self.size.x,
            h = self.size.y
        )
    }
}

fn svg_gray(g: f64) -> String {
    let v = (g.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{v:02x}{v:02x}{v:02x}")
}

fn points(pts: &[DVec2]) -> String {
    pts.iter()
        .map(|p| format!("{:.3},{:.3}", p.x, p.y))
        .collect::<Vec<_>>()
        .join(" ")
}

impl SvgCanvas {
    fn polyline(&mut self, pts: &[DVec2], s: Stroke, color: &str) {
        let pts: Vec<DVec2> = pts.iter().map(|p| self.y(*p)).collect();
        let dash = s
            .dash_array()
            .map(|d| {
                let d: Vec<String> = d.iter().map(|v| v.to_string()).collect();
                format!(" stroke-dasharray=\"{}\"", d.join(" "))
            })
            .unwrap_or_default();
        let _ = writeln!(
            self.body,
            "<polyline points=\"{}\" fill=\"none\" stroke=\"{color}\" stroke-width=\"{}\"{dash}/>",
            points(&pts),
            s.width
        );
    }

    fn polygon_fill(&mut self, pts: &[DVec2], color: &str) {
        let pts: Vec<DVec2> = pts.iter().map(|p| self.y(*p)).collect();
        let _ = writeln!(
            self.body,
            "<polygon points=\"{}\" fill=\"{color}\" stroke=\"none\"/>",
            points(&pts),
        );
    }

    fn svg_text(&mut self, at: DVec2, text: &str, anchor: Anchor, angle: f64, look: &TextLook) {
        let size = look.size;
        let shipped;
        let family = match look.font {
            Font::Sans => "Helvetica, Arial, sans-serif",
            Font::Serif => "Times, 'Times New Roman', serif",
            Font::Mono => "Courier, 'Courier New', monospace",
            f => {
                if !self.fonts.contains(&f) {
                    self.fonts.push(f);
                }
                shipped = format!("'{}', Helvetica, Arial, sans-serif", f.family());
                &shipped
            }
        };
        let mut extra = String::new();
        if look.bold {
            extra.push_str(" font-weight=\"bold\"");
        }
        if look.italic {
            extra.push_str(" font-style=\"italic\"");
        }
        if let Some(rgb) = look.rgb {
            let _ = write!(extra, " fill=\"{}\"", hex(rgb));
        }
        let p = self.y(at);
        let anchor = match anchor {
            Anchor::Start => "start",
            Anchor::Middle => "middle",
            Anchor::End => "end",
        };
        let rotate = if angle.abs() > 1e-9 {
            format!(
                " transform=\"rotate({:.3} {:.3} {:.3})\"",
                -angle.to_degrees(),
                p.x,
                p.y
            )
        } else {
            String::new()
        };
        let _ = writeln!(
            self.body,
            "<text x=\"{:.3}\" y=\"{:.3}\"{rotate} font-family=\"{family}\" font-size=\"{size}\"{extra} text-anchor=\"{anchor}\" dominant-baseline=\"middle\">{}</text>",
            p.x,
            p.y,
            xml_escape(text)
        );
    }
}

impl Canvas for SvgCanvas {
    fn line(&mut self, pts: &[DVec2], s: Stroke) {
        self.polyline(pts, s, &svg_gray(s.gray));
    }

    fn line_rgb(&mut self, pts: &[DVec2], s: Stroke, rgb: [f32; 3]) {
        self.polyline(pts, s, &hex(rgb));
    }

    fn fill(&mut self, pts: &[DVec2], gray: f64) {
        self.polygon_fill(pts, &svg_gray(gray));
    }

    fn fill_rgb(&mut self, pts: &[DVec2], rgb: [f32; 3]) {
        self.polygon_fill(pts, &hex(rgb));
    }

    fn tint(&mut self, pts: &[DVec2], rgb: [f32; 3]) {
        let pts: Vec<DVec2> = pts.iter().map(|p| self.y(*p)).collect();
        let _ = writeln!(
            self.body,
            "<polygon points=\"{}\" fill=\"{}\" fill-opacity=\"{TINT_ALPHA}\" stroke=\"none\"/>",
            points(&pts),
            hex(rgb),
        );
    }

    fn text_angle(&mut self, at: DVec2, size: f64, text: &str, anchor: Anchor, angle: f64) {
        let look = TextLook {
            size,
            ..TextLook::default()
        };
        self.svg_text(at, text, anchor, angle, &look);
    }

    fn styled_text(&mut self, at: DVec2, text: &str, anchor: Anchor, angle: f64, look: &TextLook) {
        self.svg_text(at, text, anchor, angle, look);
    }

    fn image(&mut self, lo: DVec2, hi: DVec2, clip: Option<(DVec2, DVec2)>, raster: &Arc<Raster>) {
        let clip_attr = match clip {
            Some((a, b)) => {
                self.clips += 1;
                let (ta, tb) = (self.y(DVec2::new(a.x, b.y)), b - a);
                let _ = writeln!(
                    self.body,
                    "<clipPath id=\"c{}\"><rect x=\"{:.3}\" y=\"{:.3}\" width=\"{:.3}\" height=\"{:.3}\"/></clipPath>",
                    self.clips, ta.x, ta.y, tb.x, tb.y
                );
                format!(" clip-path=\"url(#c{})\"", self.clips)
            }
            None => String::new(),
        };
        let top_left = self.y(DVec2::new(lo.x, hi.y));
        let size = hi - lo;
        let _ = writeln!(
            self.body,
            "<image x=\"{:.3}\" y=\"{:.3}\" width=\"{:.3}\" height=\"{:.3}\" preserveAspectRatio=\"none\"{clip_attr} href=\"data:image/png;base64,{}\"/>",
            top_left.x,
            top_left.y,
            size.x,
            size.y,
            crate::raster::base64(&raster.png())
        );
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

// ---- PDF -----------------------------------------------------------------

/// A multi-page vector PDF. Text uses the built-in Helvetica font.
#[derive(Default)]
pub struct PdfWriter {
    pages: Vec<PageData>,
}

struct PageData {
    size: DVec2,
    content: String,
    /// The stroke state last written (colour, width, dash) and where the
    /// content ended after that stroke: the next stroke with the same state
    /// writes only its path (82% of a permit set's lines are hatch lines
    /// of one pen).
    pen: Option<(String, usize)>,
    images: Vec<Arc<Raster>>,
    /// Shipped fonts written in it, embedded once for the whole file.
    fonts: Vec<Font>,
}

/// One page of a [`PdfWriter`], drawn in millimetres.
pub struct PdfPage<'a> {
    page: &'a mut PageData,
}

const PT: f64 = 72.0 / 25.4;

/// The standard PDF fonts, /F1 to /F12: each family plain, bold, italic,
/// bold italic.
const PDF_FONTS: [&str; 12] = [
    "Helvetica",
    "Helvetica-Bold",
    "Helvetica-Oblique",
    "Helvetica-BoldOblique",
    "Times-Roman",
    "Times-Bold",
    "Times-Italic",
    "Times-BoldItalic",
    "Courier",
    "Courier-Bold",
    "Courier-Oblique",
    "Courier-BoldOblique",
];

/// The /F number of a look's font (a shipped font is /B, see
/// [`PdfPage::pdf_text`]).
fn pdf_font(look: &TextLook) -> usize {
    let family = match look.font {
        Font::Serif => 4,
        Font::Mono => 8,
        _ => 0,
    };
    family + usize::from(look.bold) + 2 * usize::from(look.italic) + 1
}

/// The shipped fonts a file embeds, in [`Font::ALL`]'s order.
fn embedded(pages: &[PageData]) -> Vec<Font> {
    Font::ALL
        .into_iter()
        .filter(|f| pages.iter().any(|p| p.fonts.contains(f)))
        .collect()
}

impl PdfWriter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Start a page of `size` mm; draw on the returned canvas.
    pub fn page(&mut self, size: DVec2) -> PdfPage<'_> {
        // millimetre units, round line ends
        self.pages.push(PageData {
            size,
            content: format!("{PT:.5} 0 0 {PT:.5} 0 0 cm 1 J 1 j\n"),
            images: vec![],
            fonts: vec![],
            pen: None,
        });
        PdfPage {
            page: self.pages.last_mut().expect("just pushed"),
        }
    }

    pub fn finish(self) -> Vec<u8> {
        let mut out: Vec<u8> = b"%PDF-1.4\n".to_vec();
        let mut offsets: Vec<usize> = Vec::new();
        // objects: 1 catalog, 2 pages, 3.. the fonts, then per page: the
        // page, its content and an (image, alpha mask) pair per image
        let used = embedded(&self.pages);
        // a shipped font is three objects: the font, its descriptor, its file
        let mut next = 3 + PDF_FONTS.len() + 3 * used.len();
        // one picture drawn on several pages (a sheet paper's texture) is
        // written once and shared: (its object, written here first)
        let mut seen: std::collections::HashMap<*const Raster, usize> = Default::default();
        let ids: Vec<(usize, Vec<(usize, bool)>)> = self
            .pages
            .iter()
            .map(|p| {
                let page = next;
                next += 2;
                let images = p
                    .images
                    .iter()
                    .map(|r| match seen.get(&Arc::as_ptr(r)) {
                        Some(&id) => (id, false),
                        None => {
                            next += 2;
                            seen.insert(Arc::as_ptr(r), next - 2);
                            (next - 2, true)
                        }
                    })
                    .collect();
                (page, images)
            })
            .collect();
        let mut obj = |out: &mut Vec<u8>, body: &[u8]| {
            offsets.push(out.len());
            let id = offsets.len();
            out.extend_from_slice(format!("{id} 0 obj\n").as_bytes());
            out.extend_from_slice(body);
            out.extend_from_slice(b"\nendobj\n");
        };
        let stream = |dict: &str, data: &[u8]| {
            let mut body = format!("<< {dict} /Length {} >>\nstream\n", data.len()).into_bytes();
            body.extend_from_slice(data);
            body.extend_from_slice(b"\nendstream");
            body
        };
        obj(&mut out, b"<< /Type /Catalog /Pages 2 0 R >>");
        let kids: Vec<String> = ids.iter().map(|(p, _)| format!("{p} 0 R")).collect();
        obj(
            &mut out,
            format!(
                "<< /Type /Pages /Kids [{}] /Count {} >>",
                kids.join(" "),
                ids.len()
            )
            .as_bytes(),
        );
        for font in PDF_FONTS {
            obj(
                &mut out,
                format!(
                    "<< /Type /Font /Subtype /Type1 /BaseFont /{font} /Encoding /WinAnsiEncoding >>"
                )
                .as_bytes(),
            );
        }
        let mut fonts: String = (0..PDF_FONTS.len())
            .map(|i| format!("/F{} {} 0 R ", i + 1, i + 3))
            .collect();
        for (k, f) in used.iter().enumerate() {
            let id = 3 + PDF_FONTS.len() + 3 * k;
            let (Some(m), Some(data)) = (crate::fonts::metrics(*f), crate::fonts::bytes(*f)) else {
                // never: a used font is a shipped one; keep the numbering
                for _ in 0..3 {
                    obj(&mut out, b"null");
                }
                continue;
            };
            let em = |v: f64| (v * 1000.0).round() as i64;
            let widths: Vec<String> = (32..=255usize)
                .map(|c| em(m.widths[c].unwrap_or(m.average)).to_string())
                .collect();
            obj(
                &mut out,
                format!(
                    "<< /Type /Font /Subtype /TrueType /BaseFont /{ps} /FirstChar 32 /LastChar 255 /Widths [{}] /Encoding /WinAnsiEncoding /FontDescriptor {} 0 R >>",
                    widths.join(" "),
                    id + 1,
                    ps = m.ps_name
                )
                .as_bytes(),
            );
            obj(
                &mut out,
                format!(
                    "<< /Type /FontDescriptor /FontName /{ps} /Flags 32 /FontBBox [{} {} {} {}] /ItalicAngle 0 /Ascent {} /Descent {} /CapHeight {} /StemV 80 /FontFile2 {} 0 R >>",
                    em(m.bbox[0]),
                    em(m.bbox[1]),
                    em(m.bbox[2]),
                    em(m.bbox[3]),
                    em(m.ascent),
                    em(m.descent),
                    em(m.cap),
                    id + 2,
                    ps = m.ps_name
                )
                .as_bytes(),
            );
            obj(
                &mut out,
                &stream(
                    &format!("/Length1 {} /Filter /FlateDecode", data.len()),
                    &crate::raster::deflate(data),
                ),
            );
            let _ = write!(fonts, "/B{} {} 0 R ", k + 1, id);
        }
        for (page, (id, images)) in self.pages.iter().zip(&ids) {
            let xobjects: String = images
                .iter()
                .enumerate()
                .map(|(k, (img, _))| format!("/Im{k} {img} 0 R "))
                .collect();
            obj(
                &mut out,
                format!(
                    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {:.2} {:.2}] /Resources << /Font << {fonts}>> /XObject << {xobjects}>> >> /Contents {} 0 R >>",
                    page.size.x * PT,
                    page.size.y * PT,
                    id + 1
                )
                .as_bytes(),
            );
            // a shipped font's placeholder, its index in Font::ALL, becomes
            // its number among the fonts this file embeds
            let mut content = page.content.clone();
            for (k, f) in used.iter().enumerate() {
                let i = Font::ALL.iter().position(|x| x == f).unwrap_or(0);
                content = content.replace(&format!("/B{{font:{i}}}"), &format!("/B{}", k + 1));
            }
            // the drawing compressed: it was 18 of the 23 MB of a permit set
            obj(
                &mut out,
                &stream(
                    "/Filter /FlateDecode",
                    &crate::raster::deflate(content.as_bytes()),
                ),
            );
            for (raster, (img, first)) in page.images.iter().zip(images) {
                if !first {
                    continue;
                }
                let (rgb, alpha) = raster.split();
                let dict = |space: &str, mask: &str| {
                    format!(
                        "/Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /{space} /BitsPerComponent 8 /Filter /FlateDecode{mask}",
                        raster.width, raster.height
                    )
                };
                obj(
                    &mut out,
                    &stream(
                        &dict("DeviceRGB", &format!(" /SMask {} 0 R", img + 1)),
                        &crate::raster::deflate(&rgb),
                    ),
                );
                obj(
                    &mut out,
                    &stream(&dict("DeviceGray", ""), &crate::raster::deflate(&alpha)),
                );
            }
        }
        let xref = out.len();
        let mut tail = format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len() + 1);
        for o in &offsets {
            let _ = writeln!(tail, "{o:010} 00000 n ");
        }
        let _ = write!(
            tail,
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            offsets.len() + 1
        );
        out.extend_from_slice(tail.as_bytes());
        out
    }
}

/// A PDF written by [`PdfWriter`] as text, its pages' drawing inflated:
/// for tests and tools that look for what a page draws.
pub fn pdf_readable(pdf: &[u8]) -> String {
    const HEAD: &[u8] = b"<< /Filter /FlateDecode /Length ";
    let mut out: Vec<u8> = Vec::with_capacity(pdf.len() * 4);
    let mut i = 0;
    while i < pdf.len() {
        let Some(at) = pdf[i..]
            .windows(HEAD.len())
            .position(|w| w == HEAD)
            .map(|k| i + k)
        else {
            out.extend_from_slice(&pdf[i..]);
            break;
        };
        out.extend_from_slice(&pdf[i..at]);
        let rest = &pdf[at + HEAD.len()..];
        let digits = rest.iter().take_while(|b| b.is_ascii_digit()).count();
        let len: usize = std::str::from_utf8(&rest[..digits])
            .ok()
            .and_then(|t| t.parse().ok())
            .unwrap_or(0);
        let start = at + HEAD.len() + digits + b" >>\nstream\n".len();
        let data = &pdf[start.min(pdf.len())..(start + len).min(pdf.len())];
        match miniz_oxide::inflate::decompress_to_vec_zlib(data) {
            Ok(plain) => {
                out.extend_from_slice(b"<< >>\nstream\n");
                out.extend_from_slice(&plain);
            }
            Err(_) => out.extend_from_slice(&pdf[at..start + len]),
        }
        i = start + len;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Could `ops` have changed the stroke colour, width or dash, or restored
/// an older state? Erring toward yes only costs a few bytes.
fn touches_stroke_state(ops: &str) -> bool {
    [" w ", " G ", " RG", " d ", "Q", " gs"]
        .iter()
        .any(|op| ops.contains(op))
        || ops.starts_with("0 G")
}

fn pdf_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('(');
    for c in s.chars() {
        match c {
            '(' | ')' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            ' '..='~' => out.push(c),
            // Latin-1 letters and signs (², °, é) are the same in WinAnsi
            '\u{a0}'..='\u{ff}' => {
                let _ = write!(out, "\\{:03o}", c as u32);
            }
            _ => out.push('?'),
        }
    }
    out.push(')');
    out
}

impl PdfPage<'_> {
    /// A stroked polyline; `color` sets the stroke colour (`G` or `RG`).
    fn stroke(&mut self, pts: &[DVec2], s: Stroke, color: &str) {
        if pts.len() < 2 {
            return;
        }
        let mut state = format!("{color} {:.3} w ", s.width);
        match s.dash_array() {
            Some(d) => {
                let d: Vec<String> = d.iter().map(|v| v.to_string()).collect();
                let _ = write!(state, "[{}] 0 d ", d.join(" "));
            }
            None => state.push_str("[] 0 d "),
        }
        let c = &mut self.page.content;
        // the same pen, and what came since changed no stroke state (fills
        // set the other colour; bold text and images do): only the path.
        // Each line stays its own path: joined, a renderer softens the
        // edges of lines that nearly touch differently
        let same = self
            .page
            .pen
            .as_ref()
            .is_some_and(|(last, end)| *last == state && !touches_stroke_state(&c[*end..]));
        if !same {
            c.push_str(&state);
        }
        for (i, p) in pts.iter().enumerate() {
            let _ = write!(
                c,
                "{:.3} {:.3} {} ",
                p.x,
                p.y,
                if i == 0 { "m" } else { "l" }
            );
        }
        c.push_str("S\n");
        let end = c.len();
        self.page.pen = Some((state, end));
    }

    /// A filled polygon; `color` sets the fill colour (`g` or `rg`).
    fn filled(&mut self, pts: &[DVec2], color: &str) {
        if pts.len() < 3 {
            return;
        }
        let c = &mut self.page.content;
        let _ = write!(c, "{color} ");
        for (i, p) in pts.iter().enumerate() {
            let _ = write!(
                c,
                "{:.3} {:.3} {} ",
                p.x,
                p.y,
                if i == 0 { "m" } else { "l" }
            );
        }
        c.push_str("h f\n");
    }

    fn pdf_text(&mut self, at: DVec2, text: &str, anchor: Anchor, angle: f64, look: &TextLook) {
        let size = look.size;
        let w = look.width(text);
        let along = match anchor {
            Anchor::Start => 0.0,
            Anchor::Middle => -w / 2.0,
            Anchor::End => -w,
        };
        // baseline sits a little below the middle of capital letters
        let (dir, up) = (DVec2::from_angle(angle), DVec2::from_angle(angle).perp());
        let o = at + dir * along - up * size * 0.35;
        let color = match look.rgb {
            Some([r, g, b]) => format!("{r:.3} {g:.3} {b:.3} rg"),
            None => "0 g".into(),
        };
        if look.font.bundled() {
            // a shipped font, embedded: its name in the file is /B and its
            // place among the fonts the file uses, so it is noted here.
            // It has one weight: bold is drawn with an outline, italic
            // slanted (as the screen does)
            if !self.page.fonts.contains(&look.font) {
                self.page.fonts.push(look.font);
            }
            let slant = if look.italic { 0.2 } else { 0.0 };
            let (c, d) = (up.x + dir.x * slant, up.y + dir.y * slant);
            let (fill, end) = if look.bold {
                let rgb = look.rgb.map_or("0 G".to_string(), |[r, g, b]| {
                    format!("{r:.3} {g:.3} {b:.3} RG")
                });
                (format!("{rgb} {:.3} w 2 Tr ", size * 0.035), " 0 Tr")
            } else {
                (String::new(), "")
            };
            // a character the font lacks (Structa has no ²) is written in
            // Helvetica, run by run, as a browser falls back in an SVG
            let has = |ch: char| {
                crate::fonts::metrics(look.font)
                    .is_some_and(|m| (ch as u32) < 256 && m.widths[ch as usize].is_some())
            };
            let mut runs: Vec<(bool, String)> = vec![];
            for ch in text.chars() {
                let h = has(ch) || ch == ' ';
                match runs.last_mut() {
                    Some((k, s)) if *k == h => s.push(ch),
                    _ => runs.push((h, ch.to_string())),
                }
            }
            let index = Font::ALL.iter().position(|f| *f == look.font).unwrap_or(0);
            let mut x = 0.0;
            for (own, run) in runs {
                let p = o + dir * x;
                let name = if own {
                    format!("B{{font:{index}}}")
                } else {
                    format!(
                        "F{}",
                        1 + usize::from(look.bold) + 2 * usize::from(look.italic)
                    )
                };
                let _ = writeln!(
                    self.page.content,
                    "{color} {fill}BT /{name} {size:.3} Tf {:.5} {:.5} {:.5} {:.5} {:.3} {:.3} Tm {} Tj{end} ET",
                    dir.x,
                    dir.y,
                    c,
                    d,
                    p.x,
                    p.y,
                    pdf_string(&run)
                );
                x += if own {
                    crate::fonts::width(look.font, &run, size).unwrap_or(0.0)
                } else {
                    text_width(&run, size)
                };
            }
            return;
        }
        let _ = writeln!(
            self.page.content,
            "{color} BT /F{} {size:.3} Tf {:.5} {:.5} {:.5} {:.5} {:.3} {:.3} Tm {} Tj ET",
            pdf_font(look),
            dir.x,
            dir.y,
            up.x,
            up.y,
            o.x,
            o.y,
            pdf_string(text)
        );
    }
}

impl Canvas for PdfPage<'_> {
    fn line(&mut self, pts: &[DVec2], s: Stroke) {
        self.stroke(pts, s, &format!("{:.3} G", s.gray));
    }

    fn line_rgb(&mut self, pts: &[DVec2], s: Stroke, [r, g, b]: [f32; 3]) {
        self.stroke(pts, s, &format!("{r:.3} {g:.3} {b:.3} RG"));
    }

    fn fill(&mut self, pts: &[DVec2], gray: f64) {
        self.filled(pts, &format!("{gray:.3} g"));
    }

    fn fill_rgb(&mut self, pts: &[DVec2], [r, g, b]: [f32; 3]) {
        self.filled(pts, &format!("{r:.3} {g:.3} {b:.3} rg"));
    }

    fn text_angle(&mut self, at: DVec2, size: f64, text: &str, anchor: Anchor, angle: f64) {
        let look = TextLook {
            size,
            ..TextLook::default()
        };
        self.pdf_text(at, text, anchor, angle, &look);
    }

    fn styled_text(&mut self, at: DVec2, text: &str, anchor: Anchor, angle: f64, look: &TextLook) {
        self.pdf_text(at, text, anchor, angle, look);
    }

    fn image(&mut self, lo: DVec2, hi: DVec2, clip: Option<(DVec2, DVec2)>, raster: &Arc<Raster>) {
        let k = self.page.images.len();
        self.page.images.push(raster.clone());
        let c = &mut self.page.content;
        c.push_str("q ");
        if let Some((a, b)) = clip {
            let _ = write!(
                c,
                "{:.3} {:.3} {:.3} {:.3} re W n ",
                a.x,
                a.y,
                b.x - a.x,
                b.y - a.y
            );
        }
        let size = hi - lo;
        let _ = writeln!(
            c,
            "{:.4} 0 0 {:.4} {:.3} {:.3} cm /Im{k} Do Q",
            size.x, size.y, lo.x, lo.y
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pen_is_written_once_for_the_lines_drawn_with_it_and_the_drawing_compressed() {
        let mut pdf = PdfWriter::new();
        let hatch = Stroke::solid(0.09, 0.2);
        let cut = Stroke::solid(0.5, 0.0);
        {
            let mut page = pdf.page(DVec2::new(100.0, 100.0));
            for i in 0..100 {
                let y = i as f64;
                page.line(&[DVec2::new(0.0, y), DVec2::new(50.0, y)], hatch);
            }
            // a fill between keeps the pen; bold text changes it
            page.fill(&[DVec2::ZERO, DVec2::X, DVec2::Y], 0.5);
            page.line(&[DVec2::ZERO, DVec2::ONE], hatch);
            page.line(&[DVec2::ZERO, DVec2::ONE], cut);
        }
        let c = pdf.pages[0].content.clone();
        // the hatch pen once for all 101 hatch lines, then the cut pen
        assert_eq!(c.matches("0.090 w").count(), 1, "{c}");
        assert_eq!(c.matches("0.500 w").count(), 1, "{c}");
        // each line still its own path
        assert_eq!(c.matches(" S\n").count(), 102, "{c}");
        // bold text sets its own stroke width; images restore an older state
        assert!(touches_stroke_state("0 G 0.088 w 2 Tr BT /B1 2.5 Tf"));
        assert!(touches_stroke_state("q 1 0 0 1 0 0 cm /Im0 Do Q\n"));
        assert!(!touches_stroke_state("0.500 g 0 0 m 1 0 l 0 1 l h f\n"));
        let file = pdf.finish();
        let raw = String::from_utf8_lossy(&file);
        assert!(
            raw.contains("<< /Filter /FlateDecode /Length"),
            "the page's drawing is compressed"
        );
        assert!(!raw.contains("0.090 w"));
        assert!(
            pdf_readable(&file).contains(&c),
            "and reads back as written"
        );
    }

    #[test]
    fn pdf_has_a_valid_xref() {
        let mut pdf = PdfWriter::new();
        {
            let mut p = pdf.page(DVec2::new(420.0, 297.0));
            p.rect(DVec2::splat(10.0), DVec2::new(410.0, 287.0), CUT);
            p.text(DVec2::new(20.0, 20.0), 3.0, "Plan (1:100)", Anchor::Start);
        }
        pdf.page(DVec2::new(210.0, 297.0));
        let bytes = pdf.finish();
        // one character a byte, so the xref offsets still index it (the
        // drawing is compressed: binary)
        let s: String = bytes
            .iter()
            .map(|&b| if b < 128 { b as char } else { '?' })
            .collect();
        assert!(s.starts_with("%PDF-1.4"));
        assert!(s.contains("/Count 2"));
        assert!(pdf_readable(&bytes).contains("(Plan \\(1:100\\)) Tj"));
        // every xref offset points at its object
        let xref_at: usize = s
            .rsplit("startxref\n")
            .next()
            .unwrap()
            .lines()
            .next()
            .unwrap()
            .parse()
            .unwrap();
        let table = &s[xref_at..];
        for (i, line) in table.lines().skip(3).take(7).enumerate() {
            let off: usize = line[..10].parse().unwrap();
            assert!(
                s[off..].starts_with(&format!("{} 0 obj", i + 1)),
                "object {}",
                i + 1
            );
        }
    }
}
