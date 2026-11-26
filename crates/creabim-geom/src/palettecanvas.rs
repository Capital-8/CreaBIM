//! The drawing palette on paper (pens stage 7): a [`Canvas`] that hands
//! what is drawn on to another in the palette's colours, so sheets, PDF
//! and SVG take a view's palette the way the screen does. Greys become
//! the ink ramp (lines) or the fill and surface colours (fills); colours
//! take the palette's hue at their own lightness.

use crate::annotation::TextLook;
use crate::paper::{Anchor, Canvas, Stroke};
use crate::raster::Raster;
use creabim_core::palette::Roles;
use glam::DVec2;
use std::sync::Arc;

pub struct PaletteCanvas<'a> {
    pub inner: &'a mut dyn Canvas,
    pub roles: Roles,
}

fn is_grey(c: [f32; 3]) -> bool {
    let hi = c[0].max(c[1]).max(c[2]);
    let lo = c[0].min(c[1]).min(c[2]);
    hi - lo <= 0.11
}

impl PaletteCanvas<'_> {
    fn line_colour(&self, c: [f32; 3]) -> [f32; 3] {
        if is_grey(c) {
            self.roles.line_grey((c[0] + c[1] + c[2]) / 3.0)
        } else {
            self.roles.colour(c)
        }
    }

    fn fill_colour(&self, c: [f32; 3]) -> [f32; 3] {
        if is_grey(c) {
            self.roles.fill_grey((c[0] + c[1] + c[2]) / 3.0)
        } else {
            self.roles.colour(c)
        }
    }
}

impl Canvas for PaletteCanvas<'_> {
    fn line(&mut self, pts: &[DVec2], stroke: Stroke) {
        let c = self.roles.line_grey(stroke.gray as f32);
        self.inner.line_rgb(pts, stroke, c);
    }
    fn line_rgb(&mut self, pts: &[DVec2], stroke: Stroke, rgb: [f32; 3]) {
        let c = self.line_colour(rgb);
        self.inner.line_rgb(pts, stroke, c);
    }
    fn fill(&mut self, pts: &[DVec2], gray: f64) {
        let c = self.roles.fill_grey(gray as f32);
        self.inner.fill_rgb(pts, c);
    }
    fn fill_rgb(&mut self, pts: &[DVec2], rgb: [f32; 3]) {
        let c = self.fill_colour(rgb);
        self.inner.fill_rgb(pts, c);
    }
    fn tint(&mut self, pts: &[DVec2], rgb: [f32; 3]) {
        let c = self.fill_colour(rgb);
        self.inner.tint(pts, c);
    }
    fn text_angle(&mut self, at: DVec2, size: f64, text: &str, anchor: Anchor, angle: f64) {
        let look = TextLook {
            size,
            rgb: Some(self.roles.ink),
            ..TextLook::default()
        };
        self.inner.styled_text(at, text, anchor, angle, &look);
    }
    fn styled_text(&mut self, at: DVec2, text: &str, anchor: Anchor, angle: f64, look: &TextLook) {
        let mut look = *look;
        look.rgb = Some(match look.rgb {
            Some(c) => self.line_colour(c),
            None => self.roles.ink,
        });
        self.inner.styled_text(at, text, anchor, angle, &look);
    }
    fn image(&mut self, lo: DVec2, hi: DVec2, clip: Option<(DVec2, DVec2)>, raster: &Arc<Raster>) {
        self.inner.image(lo, hi, clip, raster);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paper::SvgCanvas;

    #[test]
    fn a_palette_gives_ink_lines_and_grey_fills_its_colours_on_paper() {
        let roles = creabim_core::palette::shipped("Sepia").unwrap().roles();
        let mut svg = SvgCanvas::new(DVec2::new(100.0, 100.0));
        {
            let mut pal = PaletteCanvas {
                inner: &mut svg,
                roles,
            };
            pal.line(
                &[DVec2::ZERO, DVec2::new(10.0, 0.0)],
                Stroke::solid(0.35, 0.0),
            );
            pal.fill(&[DVec2::ZERO, DVec2::X, DVec2::ONE], 0.2);
        }
        let out = svg.finish();
        let ink = crate::paper::hex(roles.ink);
        assert!(out.contains(&ink), "{ink} not in {out}");
        assert!(!out.contains("#000000"), "black left in {out}");
    }
}
