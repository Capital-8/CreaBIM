//! The drawing palette on screen (pens stage 7): a view's shapes, drawn as
//! on white paper, are given the palette's colours afterwards, the way
//! dark paper turns them over. Lines take the ink ramp, fills the fill and
//! surface colours by how dark they are, coloured marks the palette's hue
//! at their own lightness. The selection keeps its orange, or turns to the
//! palette's opposite hue; hover and pictures are left alone. Nothing is
//! rebuilt: a palette change is a repaint.

use creabim_core::palette::{Rgb, Roles};
use eframe::egui::{self, epaint, Color32, Shape};

fn rgb(c: Color32) -> (Rgb, u8) {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    ([r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0], a)
}

fn back(c: Rgb, a: u8) -> Color32 {
    let u = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgba_unmultiplied(u(c[0]), u(c[1]), u(c[2]), a)
}

/// Is `c` a grey (a drawing's ink or paper), not a colour?
fn is_grey(c: Color32) -> bool {
    let [r, g, b, _] = c.to_srgba_unmultiplied();
    let (hi, lo) = (r.max(g).max(b), r.min(g).min(b));
    hi.saturating_sub(lo) <= 28
}

fn near(c: Color32, k: Color32) -> bool {
    let [r, g, b, _] = c.to_srgba_unmultiplied();
    let [x, y, z, _] = k.to_srgba_unmultiplied();
    (r as i32 - x as i32).abs() + (g as i32 - y as i32).abs() + (b as i32 - z as i32).abs() < 60
}

/// The colours a view must keep: the selection's and the hover's.
#[derive(Clone, Copy)]
struct Keep {
    selection: Color32,
    hover: Color32,
}

/// One colour of a line (or text) in the palette.
fn line(c: Color32, r: &Roles, keep: &Keep) -> Color32 {
    if c.a() == 0 {
        return c;
    }
    if near(c, keep.selection) {
        let (_, a) = rgb(c);
        return back(r.selection, a);
    }
    if near(c, keep.hover) {
        return c;
    }
    let (v, a) = rgb(c);
    if is_grey(c) {
        back(r.line_grey((v[0] + v[1] + v[2]) / 3.0), a)
    } else {
        back(r.colour(v), a)
    }
}

/// One colour of a fill in the palette.
fn fill(c: Color32, r: &Roles, keep: &Keep) -> Color32 {
    if c.a() == 0 {
        return c;
    }
    if near(c, keep.selection) {
        let (_, a) = rgb(c);
        return back(r.selection, a);
    }
    if near(c, keep.hover) {
        return c;
    }
    let (v, a) = rgb(c);
    if is_grey(c) {
        back(r.fill_grey((v[0] + v[1] + v[2]) / 3.0), a)
    } else {
        back(r.colour(v), a)
    }
}

fn stroke(s: &mut egui::Stroke, r: &Roles, k: &Keep) {
    s.color = line(s.color, r, k);
}

fn path_stroke(s: &mut epaint::PathStroke, r: &Roles, k: &Keep) {
    if let epaint::ColorMode::Solid(c) = &mut s.color {
        *c = line(*c, r, k);
    }
}

fn shape(sh: &mut Shape, r: &Roles, k: &Keep) {
    match sh {
        Shape::Vec(v) => v.iter_mut().for_each(|s| shape(s, r, k)),
        Shape::Circle(c) => {
            c.fill = fill(c.fill, r, k);
            stroke(&mut c.stroke, r, k);
        }
        Shape::Ellipse(e) => {
            e.fill = fill(e.fill, r, k);
            stroke(&mut e.stroke, r, k);
        }
        Shape::LineSegment { stroke: s, .. } => stroke(s, r, k),
        Shape::Path(p) => {
            p.fill = fill(p.fill, r, k);
            path_stroke(&mut p.stroke, r, k);
        }
        Shape::Rect(x) => {
            x.fill = fill(x.fill, r, k);
            stroke(&mut x.stroke, r, k);
        }
        Shape::Mesh(m) => {
            // a picture keeps its colours
            if m.texture_id != egui::TextureId::default() {
                return;
            }
            let m = std::sync::Arc::make_mut(m);
            for v in &mut m.vertices {
                v.color = fill(v.color, r, k);
            }
        }
        Shape::QuadraticBezier(b) => {
            b.fill = fill(b.fill, r, k);
            path_stroke(&mut b.stroke, r, k);
        }
        Shape::CubicBezier(b) => {
            b.fill = fill(b.fill, r, k);
            path_stroke(&mut b.stroke, r, k);
        }
        Shape::Text(_) => {
            let (r, k) = (*r, *k);
            epaint::shape_transform::adjust_colors(sh, move |c| *c = line(*c, &r, &k));
        }
        _ => {}
    }
}

/// Give the shapes `from..` on `layer` the palette's colours, this frame
/// (those the zoom column keeps are left alone).
pub fn apply(
    ctx: &egui::Context,
    layer: egui::LayerId,
    from: Option<egui::layers::ShapeIdx>,
    roles: &Roles,
) {
    let start = from.map_or(0, |i| i.0);
    let kept = crate::planview::kept_colours(ctx, layer);
    let keep = Keep {
        selection: crate::planview::accent_selected(),
        hover: crate::hoverpick::HOVER,
    };
    ctx.graphics_mut(|g| {
        let list = g.entry(layer);
        let end = list.next_idx().0;
        for i in start..end {
            if kept.iter().any(|(a, b)| i >= *a && i <= *b) {
                continue;
            }
            list.mutate_shape(egui::layers::ShapeIdx(i), |cs| {
                shape(&mut cs.shape, roles, &keep)
            });
        }
    });
}

/// Show the shapes `from..` on `layer` as `vision` would see them (the
/// palette page's preview).
pub fn see(
    ctx: &egui::Context,
    layer: egui::LayerId,
    from: Option<egui::layers::ShapeIdx>,
    vision: creabim_core::palette::Vision,
) {
    let start = from.map_or(0, |i| i.0);
    ctx.graphics_mut(|g| {
        let list = g.entry(layer);
        let end = list.next_idx().0;
        for i in start..end {
            list.mutate_shape(egui::layers::ShapeIdx(i), |cs| {
                epaint::shape_transform::adjust_colors(&mut cs.shape, move |c| {
                    let (v, a) = rgb(*c);
                    *c = back(vision.see(v), a);
                });
            });
        }
    });
}

/// A colour of the palette for egui.
pub fn color32(c: Rgb) -> Color32 {
    back(c, 255)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ink_lines_take_the_palettes_ink_and_white_takes_its_paper() {
        let r = creabim_core::palette::shipped("Sepia").unwrap().roles();
        let k = Keep {
            selection: Color32::from_rgb(230, 120, 30),
            hover: crate::hoverpick::HOVER,
        };
        assert_eq!(line(Color32::BLACK, &r, &k), color32(r.ink));
        assert_eq!(fill(Color32::WHITE, &r, &k), color32(r.paper));
        // the selection keeps its colour role, the hover its blue
        assert_eq!(
            line(crate::hoverpick::HOVER, &r, &k),
            crate::hoverpick::HOVER
        );
        assert_eq!(line(k.selection, &r, &k), color32(r.selection));
    }
}
