//! A sheet's paper (pens stage 7, decision 3 A): its master sets a colour
//! (white, the project palette's paper, or its own) and a texture (grain,
//! laid lines, a grid). Sheets follow their master. "Print on white" drops
//! the tint and texture for an export but keeps the drawing's colours.

use crate::annotation::TextLook;
use crate::paper::{Anchor, Canvas, Stroke};
use crate::raster::Raster;
use creabim_core::palette::Roles;
use creabim_core::titleblock::{PaperColour, PaperTexture};
use creabim_core::{Model, Sheet, SheetTemplate};
use glam::DVec2;
use std::sync::{Arc, Mutex};

const WHITE: [f32; 3] = [1.0, 1.0, 1.0];

/// A sheet's paper, resolved.
#[derive(Clone, Copy, Debug)]
pub struct SheetPaper {
    /// The paper's colour.
    pub rgb: [f32; 3],
    pub texture: PaperTexture,
    /// How strongly the texture shows (1 as shipped).
    pub strength: f32,
    /// The palette the frame and notes take (the master's paper is the
    /// palette's), else none: black ink.
    pub roles: Option<Roles>,
}

impl SheetPaper {
    pub const PLAIN: SheetPaper = SheetPaper {
        rgb: WHITE,
        texture: PaperTexture::None,
        strength: 1.0,
        roles: None,
    };

    /// White, plain, black ink: drawn as sheets always were.
    pub fn is_plain(&self) -> bool {
        self.rgb == WHITE && self.texture == PaperTexture::None && self.roles.is_none()
    }

    /// Does printing on white change anything? Not on dark paper, whose
    /// light ink would vanish on white stock.
    pub fn dark(&self) -> bool {
        self.roles.is_some_and(|r| r.dark)
    }
}

/// The paper `sheet` is drawn on: its master's; on white when
/// `print_white` (dark paper keeps its colour, or its ink would vanish).
pub fn sheet_paper(model: &Model, sheet: &Sheet, print_white: bool) -> SheetPaper {
    match crate::sheet::template_of(model, sheet) {
        Some(t) => master_paper(model, t, print_white),
        None => SheetPaper::PLAIN,
    }
}

/// The paper master `t` gives its sheets.
pub fn master_paper(model: &Model, t: &SheetTemplate, print_white: bool) -> SheetPaper {
    let mut p = match t.stock.colour {
        PaperColour::White => SheetPaper::PLAIN,
        PaperColour::Own(c) => SheetPaper {
            rgb: c,
            ..SheetPaper::PLAIN
        },
        PaperColour::Palette => {
            let r = creabim_core::styles::project_palette(model).roles();
            if r.mode == creabim_core::palette::PaletteMode::Ink {
                SheetPaper::PLAIN
            } else {
                SheetPaper {
                    rgb: r.paper,
                    roles: Some(r),
                    ..SheetPaper::PLAIN
                }
            }
        }
    };
    p.texture = t.stock.texture;
    p.strength = t.stock.strength.clamp(0.25, 2.0);
    if print_white && !p.dark() {
        p.rgb = WHITE;
        p.texture = PaperTexture::None;
        if let Some(r) = &mut p.roles {
            r.paper = WHITE;
        }
    }
    p
}

/// A colour `k` of the way from the paper to the ink.
fn ink_mix(p: &SheetPaper, k: f32) -> [f32; 3] {
    let ink = p.roles.map_or([0.1; 3], |r| r.ink);
    std::array::from_fn(|i| p.rgb[i] + (ink[i] - p.rgb[i]) * k)
}

/// Lay the paper down: its colour over the whole sheet, then its texture.
pub fn draw_paper(c: &mut dyn Canvas, size: DVec2, p: &SheetPaper) {
    if p.rgb != WHITE {
        c.fill_rgb(
            &[
                DVec2::ZERO,
                DVec2::new(size.x, 0.0),
                size,
                DVec2::new(0.0, size.y),
            ],
            p.rgb,
        );
    }
    match p.texture {
        PaperTexture::None => {}
        PaperTexture::Laid => {
            let ink = ink_mix(p, (0.14 * p.strength).min(0.6));
            let mut y = 1.2;
            while y < size.y {
                c.line_rgb(
                    &[DVec2::new(0.0, y), DVec2::new(size.x, y)],
                    Stroke::solid(0.05, 0.0),
                    ink,
                );
                y += 1.2;
            }
        }
        PaperTexture::Grid => {
            let ink = ink_mix(p, (0.18 * p.strength).min(0.6));
            let s = Stroke::solid(0.08, 0.0);
            let mut x = 5.0;
            while x < size.x {
                c.line_rgb(&[DVec2::new(x, 0.0), DVec2::new(x, size.y)], s, ink);
                x += 5.0;
            }
            let mut y = 5.0;
            while y < size.y {
                c.line_rgb(&[DVec2::new(0.0, y), DVec2::new(size.x, y)], s, ink);
                y += 5.0;
            }
        }
        PaperTexture::Grain => {
            let r = grain(size, ink_mix(p, 1.0), p.strength);
            c.image(DVec2::ZERO, size, None, &r);
        }
        t => {
            // one small seamless tile laid edge to edge; a PDF keeps it
            // once however many times it is laid
            let Some((r, mm)) = paper_tile(t, ink_mix(p, 1.0), p.strength) else {
                return;
            };
            let clip = Some((DVec2::ZERO, size));
            let mut y = 0.0;
            while y < size.y {
                let mut x = 0.0;
                while x < size.x {
                    c.image(DVec2::new(x, y), DVec2::new(x + mm, y + mm), clip, &r);
                    x += mm;
                }
                y += mm;
            }
        }
    }
}

/// The owner's texture tiles (2026-11-18): grey around 128, darker and
/// lighter by up to about 30 levels; each tile's width on paper, mm.
fn tile_source(t: PaperTexture) -> Option<(&'static [u8], f64)> {
    Some(match t {
        PaperTexture::ColdPress => (include_bytes!("../assets/paper/coldpress.png"), 85.0),
        PaperTexture::Kraft => (include_bytes!("../assets/paper/kraft.png"), 85.0),
        PaperTexture::Recycled => (include_bytes!("../assets/paper/recycled.png"), 85.0),
        PaperTexture::Linen => (include_bytes!("../assets/paper/linen.png"), 21.3),
        _ => return None,
    })
}

fn decode_grey(bytes: &[u8]) -> Option<(usize, usize, Vec<u8>)> {
    let mut d = png::Decoder::new(std::io::Cursor::new(bytes));
    d.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut r = d.read_info().ok()?;
    let mut buf = vec![0; r.output_buffer_size()?];
    let info = r.next_frame(&mut buf).ok()?;
    let (w, h) = (info.width as usize, info.height as usize);
    let ch = info.color_type.samples();
    Some((w, h, buf.chunks(ch).take(w * h).map(|p| p[0]).collect()))
}

/// Texture `t`'s tile in this ink: darker than mid-grey as ink, lighter
/// as white, see-through by how far from mid-grey, `strength` times.
pub fn paper_tile(t: PaperTexture, ink: [f32; 3], strength: f32) -> Option<(Arc<Raster>, f64)> {
    static KEPT: Mutex<Vec<Arc<Raster>>> = Mutex::new(Vec::new());
    let (bytes, mm) = tile_source(t)?;
    let rgb = ink.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8);
    let key = 0x7400_0000_0000_0000u64
        | ((t as u64) << 40)
        | (((strength * 100.0).round() as u64) << 24)
        | ((rgb[0] as u64) << 16)
        | ((rgb[1] as u64) << 8)
        | rgb[2] as u64;
    let mut kept = KEPT.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(r) = kept.iter().find(|r| r.key == key) {
        return Some((r.clone(), mm));
    }
    let (w, h, grey) = decode_grey(bytes)?;
    // ±30 grey levels at most: twice that in alpha, about a quarter
    const GAIN: f32 = 2.0;
    let mut rgba = Vec::with_capacity(w * h * 4);
    for v in grey {
        let d = v as f32 - 128.0;
        let a = (d.abs() * GAIN * strength).round().min(255.0) as u8;
        if d < 0.0 {
            rgba.extend_from_slice(&[rgb[0], rgb[1], rgb[2], a]);
        } else {
            rgba.extend_from_slice(&[255, 255, 255, a]);
        }
    }
    let r = Arc::new(Raster {
        width: w,
        height: h,
        rgba,
        key,
    });
    if kept.len() > 8 {
        kept.remove(0);
    }
    kept.push(r.clone());
    Some((r, mm))
}

/// Specks of ink, sparse and see-through, about 0.75 mm each: the same
/// every time for a size and colour (kept, so a sheet on screen does not
/// make it again each frame).
pub fn grain(size: DVec2, ink: [f32; 3], strength: f32) -> Arc<Raster> {
    static KEPT: Mutex<Vec<Arc<Raster>>> = Mutex::new(Vec::new());
    let w = (size.x / 0.75).round().clamp(1.0, 4000.0) as usize;
    let h = (size.y / 0.75).round().clamp(1.0, 4000.0) as usize;
    let rgb = ink.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8);
    let key = 0x6700_0000_0000_0000u64
        | ((w as u64) << 40)
        | ((h as u64) << 24)
        | ((rgb[0] as u64) << 16)
        | ((rgb[1] as u64) << 8)
        | rgb[2] as u64 ^ (((strength * 100.0).round() as u64) << 52);
    let mut kept = KEPT.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(r) = kept.iter().find(|r| r.key == key) {
        return r.clone();
    }
    let mut rgba = vec![0u8; w * h * 4];
    let mut s: u32 = 0x9e37_79b9;
    for px in rgba.chunks_mut(4) {
        // xorshift: the same specks every time
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        px[..3].copy_from_slice(&rgb);
        let v = s % 100;
        px[3] = if v < 9 {
            ((18 + (s >> 8) % 28) as f32 * strength).round().min(255.0) as u8
        } else {
            0
        };
    }
    let r = Arc::new(Raster {
        width: w,
        height: h,
        rgba,
        key,
    });
    if kept.len() > 6 {
        kept.remove(0);
    }
    kept.push(r.clone());
    r
}

/// What is drawn on a sheet's paper: white fills take the paper's colour
/// (a view's masking, a title block's ground) and, on the palette's
/// paper, grey ink takes the palette's ink.
pub struct OnPaper<'a> {
    pub inner: &'a mut dyn Canvas,
    pub paper: SheetPaper,
}

fn is_grey(c: [f32; 3]) -> bool {
    let hi = c[0].max(c[1]).max(c[2]);
    let lo = c[0].min(c[1]).min(c[2]);
    hi - lo <= 0.11
}

fn is_white(c: [f32; 3]) -> bool {
    c.iter().all(|v| *v >= 0.985)
}

impl OnPaper<'_> {
    fn line_colour(&self, c: [f32; 3]) -> [f32; 3] {
        match &self.paper.roles {
            Some(r) if is_grey(c) => r.line_grey((c[0] + c[1] + c[2]) / 3.0),
            _ => c,
        }
    }
}

impl Canvas for OnPaper<'_> {
    fn line(&mut self, pts: &[DVec2], stroke: Stroke) {
        match &self.paper.roles {
            Some(r) => {
                let c = r.line_grey(stroke.gray as f32);
                self.inner.line_rgb(pts, stroke, c);
            }
            None => self.inner.line(pts, stroke),
        }
    }
    fn line_rgb(&mut self, pts: &[DVec2], stroke: Stroke, rgb: [f32; 3]) {
        let c = self.line_colour(rgb);
        self.inner.line_rgb(pts, stroke, c);
    }
    fn fill(&mut self, pts: &[DVec2], gray: f64) {
        if gray >= 0.985 {
            self.inner.fill_rgb(pts, self.paper.rgb);
        } else if let Some(r) = &self.paper.roles {
            let c = r.fill_grey(gray as f32);
            self.inner.fill_rgb(pts, c);
        } else {
            self.inner.fill(pts, gray);
        }
    }
    fn fill_rgb(&mut self, pts: &[DVec2], rgb: [f32; 3]) {
        let c = if is_white(rgb) {
            self.paper.rgb
        } else {
            match &self.paper.roles {
                Some(r) if is_grey(rgb) => r.fill_grey((rgb[0] + rgb[1] + rgb[2]) / 3.0),
                _ => rgb,
            }
        };
        self.inner.fill_rgb(pts, c);
    }
    fn tint(&mut self, pts: &[DVec2], rgb: [f32; 3]) {
        self.inner.tint(pts, rgb);
    }
    fn text_angle(&mut self, at: DVec2, size: f64, text: &str, anchor: Anchor, angle: f64) {
        match &self.paper.roles {
            Some(r) => {
                let look = TextLook {
                    size,
                    rgb: Some(r.ink),
                    ..TextLook::default()
                };
                self.inner.styled_text(at, text, anchor, angle, &look);
            }
            None => self.inner.text_angle(at, size, text, anchor, angle),
        }
    }
    fn styled_text(&mut self, at: DVec2, text: &str, anchor: Anchor, angle: f64, look: &TextLook) {
        if self.paper.roles.is_some() {
            let mut look = *look;
            look.rgb = Some(self.line_colour(look.rgb.unwrap_or([0.0; 3])));
            self.inner.styled_text(at, text, anchor, angle, &look);
        } else {
            self.inner.styled_text(at, text, anchor, angle, look);
        }
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
    fn white_fills_take_the_papers_colour_and_grain_is_the_same_each_time() {
        let paper = SheetPaper {
            rgb: [0.96, 0.92, 0.84],
            ..SheetPaper::PLAIN
        };
        let mut svg = SvgCanvas::new(DVec2::new(100.0, 100.0));
        {
            let mut on = OnPaper {
                inner: &mut svg,
                paper,
            };
            on.fill(&[DVec2::ZERO, DVec2::X, DVec2::ONE], 1.0);
        }
        let out = svg.finish();
        assert!(out.contains(&crate::paper::hex(paper.rgb)), "{out}");
        let a = grain(DVec2::new(420.0, 297.0), [0.1; 3], 1.0);
        let b = grain(DVec2::new(420.0, 297.0), [0.1; 3], 1.0);
        assert!(Arc::ptr_eq(&a, &b));
        let specks = a.rgba.chunks(4).filter(|p| p[3] > 0).count();
        let share = specks as f64 / (a.width * a.height) as f64;
        assert!((0.05..0.15).contains(&share), "{share}");
    }

    #[test]
    fn the_owners_textures_decode_and_a_pdf_keeps_each_tile_once() {
        for t in [
            PaperTexture::ColdPress,
            PaperTexture::Kraft,
            PaperTexture::Linen,
            PaperTexture::Recycled,
        ] {
            let (r, mm) = paper_tile(t, [0.1; 3], 1.0).expect("decodes");
            assert!(r.width >= 256 && r.width == r.height && mm > 10.0, "{t:?}");
            // see-through: never more than about a quarter
            assert!(r.rgba.chunks(4).all(|p| p[3] <= 70), "{t:?}");
        }
        let paper = SheetPaper {
            rgb: [0.96, 0.92, 0.84],
            texture: PaperTexture::Kraft,
            ..SheetPaper::PLAIN
        };
        let mut pdf = crate::paper::PdfWriter::new();
        for _ in 0..3 {
            let mut page = pdf.page(DVec2::new(420.0, 297.0));
            draw_paper(&mut page, DVec2::new(420.0, 297.0), &paper);
        }
        let bytes = pdf.finish();
        let images = bytes
            .windows(15)
            .filter(|w| w == b"/Subtype /Image")
            .count();
        // the tile and its alpha mask, once for three A3 pages of 15 tiles
        assert_eq!(images, 2);
        assert!(bytes.len() < 400_000, "{} bytes", bytes.len());
    }
}
