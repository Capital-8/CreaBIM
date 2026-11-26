//! The hand-line engine (pens stage 4): a drawing's straight lines drawn as
//! a hand would, from a [`LineStyle`]'s settings.
//!
//! [`hand_marks`] turns a polyline into marks: ribbons (a centreline with a
//! width at each point), dots and small hooks. Paper ([`HandCanvas`]: PDF,
//! SVG, sheets, the sheet view) fills them as shapes; views on screen draw
//! them as short segments. Everything is a pure function of the line and the
//! style's seed, so a drawing comes out the same at every redraw and
//! export. Ink stays black; a line keeps its pen's weight. The style's
//! numbers are the Gemini studios' own; the factors below turn them into
//! paper millimetres, as the approved mockup (Mockup_Drawing_Styles_REV00)
//! does.

use std::sync::Arc;

use creabim_core::linestyle::{LineEnd, LineStyle};
use glam::DVec2;

use crate::paper::{Anchor, Canvas, Stroke};
use crate::raster::Raster;

/// Pens this fine or finer are hatch lines: laid by hand, without
/// overshoot or blobs (a drawing's hatches use the finest pen).
pub const HATCH_PEN: f64 = 0.14;
/// Corners sharper than this split a polyline into strokes, each with its
/// own ends, as a hand lifts the pen at a corner; gentler bends (arcs) run
/// on in one stroke.
const CORNER: f64 = 0.5; // radians, about 30°

/// One mark of a hand-drawn line, in the units it was given (paper mm).
#[derive(Clone, Debug, PartialEq)]
pub enum Mark {
    /// A stroke: its centreline and its width at each point.
    Ribbon { pts: Vec<DVec2>, widths: Vec<f64> },
    /// An ink blob or a round pen end.
    Dot { at: DVec2, r: f64 },
    /// A lift-off hook.
    Hook([DVec2; 3]),
}

impl Mark {
    /// The mark as filled outlines.
    pub fn outlines(&self) -> Vec<Vec<DVec2>> {
        match self {
            Mark::Ribbon { pts, widths } => {
                let n = pts.len();
                if n < 2 {
                    return vec![];
                }
                let (mut left, mut right) = (Vec::with_capacity(n), Vec::with_capacity(n));
                for i in 0..n {
                    let dir = if i + 1 < n {
                        pts[i + 1] - pts[i]
                    } else {
                        pts[i] - pts[i - 1]
                    };
                    let nrm = dir.normalize_or_zero().perp() * widths[i] / 2.0;
                    left.push(pts[i] + nrm);
                    right.push(pts[i] - nrm);
                }
                right.reverse();
                left.extend(right);
                vec![left]
            }
            Mark::Dot { at, r } => vec![disc(*at, *r)],
            Mark::Hook(t) => vec![t.to_vec()],
        }
    }
}

/// Settings in paper millimetres.
#[derive(Clone, Copy)]
struct Feel {
    overshoot: f64,
    wobble: f64,
    wave: f64,
    end: LineEnd,
    end_amount: f64,
    pressure: f64,
    double: f64,
    double_strength: f64,
}

impl Feel {
    fn of(s: &LineStyle) -> Self {
        Feel {
            overshoot: s.overshoot * 0.22,
            wobble: s.wobble * 0.22,
            wave: (s.wobble_length * 0.09).max(1.5),
            end: s.end,
            end_amount: s.end_amount,
            pressure: s.pressure,
            double: s.double_pass * 0.3,
            double_strength: s.double_strength,
        }
    }
}

/// A small, fast, seeded random source (SplitMix64).
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// A seed for one line: the style's seed and where the line is, so each
/// line has its own hand and keeps it.
fn line_seed(style_seed: u32, pts: &[DVec2]) -> u64 {
    let mut h: u64 = style_seed as u64;
    for p in pts.iter().take(2).chain(pts.last()) {
        for v in [p.x, p.y] {
            h = h.rotate_left(13) ^ ((v * 100.0).round() as i64 as u64);
            h = h.wrapping_mul(0x100_0000_01B3);
        }
    }
    h
}

/// A round dot as a polygon: eight sides read round at pen sizes.
fn disc(c: DVec2, r: f64) -> Vec<DVec2> {
    (0..8)
        .map(|i| c + DVec2::from_angle(i as f64 / 8.0 * std::f64::consts::TAU) * r)
        .collect()
}

/// One stroke along `pts` (no sharp corners inside), `w` wide.
fn stroke(pts: &[DVec2], w: f64, f: &Feel, rng: &mut Rng, hatch: bool, out: &mut Vec<Mark>) {
    if pts.len() < 2 {
        return;
    }
    // a short piece (a jamb, a hinge, a symbol's stroke) overshoots in
    // proportion and takes no blob: a hand doesn't blot every millimetre
    let drawn: f64 = pts.windows(2).map(|s| s[0].distance(s[1])).sum();
    let ov = if hatch {
        0.0
    } else {
        f.overshoot.min(drawn * 0.25)
    };
    let blots = drawn >= 2.5;
    let mut path: Vec<DVec2> = pts.to_vec();
    let n = path.len();
    let d0 = (path[0] - path[1]).normalize_or_zero();
    let d1 = (path[n - 1] - path[n - 2]).normalize_or_zero();
    path[0] += d0 * ov * (0.4 + 0.9 * rng.next());
    path[n - 1] += d1 * ov * (0.4 + 0.9 * rng.next());
    let lens: Vec<f64> = path.windows(2).map(|s| s[0].distance(s[1])).collect();
    let total: f64 = lens.iter().sum();
    if total < 1e-6 {
        return;
    }
    // as few points as the hand needs: a wobble wants a few per wave, a
    // plain stroke only enough for its pressure to swell (files stay small)
    let step = if f.wobble > 0.0 {
        (f.wave / 6.0).clamp(0.6, 2.5)
    } else {
        2.5
    };
    let steps = ((total / step).ceil() as usize).clamp(2, 400);
    let at = |d: f64| -> (DVec2, DVec2) {
        let mut d = d.clamp(0.0, total);
        for (i, l) in lens.iter().enumerate() {
            if d <= *l || i == lens.len() - 1 {
                let dir = (path[i + 1] - path[i]).normalize_or_zero();
                return (path[i] + dir * d.min(*l), dir);
            }
            d -= l;
        }
        (path[n - 1], d1)
    };
    let amp = f.wobble * if hatch { 0.6 } else { 1.0 };
    let (ph, ph2) = (
        rng.next() * std::f64::consts::TAU,
        rng.next() * std::f64::consts::TAU,
    );
    let mut line = Vec::with_capacity(steps + 1);
    let mut widths = Vec::with_capacity(steps + 1);
    for i in 0..=steps {
        let t = i as f64 / steps as f64;
        let d = t * total;
        let (p, dir) = at(d);
        // straight-edge discipline: the wobble is small and fades at the ends
        let fade = (t * 6.0).min((1.0 - t) * 6.0).min(1.0);
        let off = amp
            * fade
            * ((std::f64::consts::TAU * d / f.wave + ph).sin() * 0.75
                + (std::f64::consts::TAU * d / (f.wave * 0.37) + ph2).sin() * 0.25);
        let mut ww = w * (1.0 + f.pressure * 0.9 * (std::f64::consts::TAU * t * 1.1 + ph2).sin());
        if f.end == LineEnd::Taper && !hatch {
            let k = (0.18 * f.end_amount).max(0.05);
            ww *= 0.3 + 0.7 * (t / k).min((1.0 - t) / k).min(1.0);
        }
        line.push(p + dir.perp() * off);
        widths.push(ww);
    }
    let (a, b) = (line[0], line[steps]);
    out.push(Mark::Ribbon { pts: line, widths });
    // round pen ends on heavy strokes, where a corner must read joined;
    // finer ones end square, too small to see (files stay light)
    if w > 0.35 {
        out.push(Mark::Dot { at: a, r: w * 0.48 });
        out.push(Mark::Dot { at: b, r: w * 0.48 });
    }
    if hatch {
        return;
    }
    match f.end {
        _ if !blots => {}
        LineEnd::InkBlob => {
            for p in [a, b] {
                if rng.next() < 0.75 {
                    out.push(Mark::Dot {
                        at: p,
                        r: w * (0.55 + 0.9 * f.end_amount),
                    });
                }
            }
        }
        LineEnd::Flick if total > 3.0 => {
            let k = 0.9 * f.end_amount;
            let side = if rng.next() < 0.5 { 1.0 } else { -1.0 };
            let nrm = d1.perp();
            let tip = b + d1 * k * 0.7 + nrm * side * k * 0.5;
            out.push(Mark::Hook([b + nrm * w / 2.0, tip, b - nrm * w / 2.0]));
        }
        _ => {}
    }
    // a second pass beside the first: finer, never grey
    if f.double > 0.01 && f.double_strength > 0.01 {
        let side = if rng.next() < 0.5 { 1.0 } else { -1.0 };
        let shift: Vec<DVec2> = pts
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let dir = if i + 1 < pts.len() {
                    pts[i + 1] - pts[i]
                } else {
                    pts[i] - pts[i - 1]
                };
                *p + dir.normalize_or_zero().perp() * f.double * side
            })
            .collect();
        let second = Feel {
            overshoot: f.overshoot * 0.6,
            end: LineEnd::None,
            double: 0.0,
            ..*f
        };
        stroke(
            &shift,
            w * (0.25 + 0.5 * f.double_strength),
            &second,
            rng,
            true,
            out,
        );
    }
}

/// A polyline as hand marks: split at its sharp corners, each piece a
/// stroke with its own ends. `width` is its pen's; everything is in the
/// units of `pts`, read as paper millimetres.
pub fn hand_marks(pts: &[DVec2], width: f64, style: &LineStyle) -> Vec<Mark> {
    let mut out = vec![];
    if pts.len() < 2 {
        return out;
    }
    let f = Feel::of(style);
    let mut rng = Rng::new(line_seed(style.seed, pts));
    let hatch = width <= HATCH_PEN;
    let mut piece = vec![pts[0]];
    for i in 1..pts.len() {
        piece.push(pts[i]);
        let sharp = i + 1 < pts.len() && {
            let (u, v) = (
                (pts[i] - pts[i - 1]).normalize_or_zero(),
                (pts[i + 1] - pts[i]).normalize_or_zero(),
            );
            u.angle_to(v).abs() > CORNER
        };
        if sharp {
            stroke(&piece, width, &f, &mut rng, hatch, &mut out);
            piece = vec![pts[i]];
        }
    }
    stroke(&piece, width, &f, &mut rng, hatch, &mut out);
    out
}

/// [`hand_marks`] as filled outlines.
pub fn hand_strokes(pts: &[DVec2], width: f64, style: &LineStyle) -> Vec<Vec<DVec2>> {
    hand_marks(pts, width, style)
        .iter()
        .flat_map(|m| m.outlines())
        .collect()
}

/// A hatch line laid by hand: shifted and turned a little, sometimes
/// broken. Paper millimetres in and out.
pub fn hatch_by_hand(a: DVec2, b: DVec2, style: &LineStyle) -> Vec<[DVec2; 2]> {
    let mut rng = Rng::new(line_seed(style.seed ^ 0xA5A5, &[a, b]));
    let mid = (a + b) / 2.0;
    let turn = (rng.next() - 0.5) * style.hatch_angle_jitter.to_radians();
    let rot = DVec2::from_angle(turn);
    let shift =
        (b - a).normalize_or_zero().perp() * (rng.next() - 0.5) * style.hatch_spacing_jitter * 0.6;
    let (a, b) = (
        mid + rot.rotate(a - mid) + shift,
        mid + rot.rotate(b - mid) + shift,
    );
    let len = a.distance(b);
    if len > 3.0 && rng.next() < style.hatch_broken {
        let t = 0.3 + 0.4 * rng.next();
        let g = (0.6 + 0.8 * rng.next()) / len / 2.0;
        let p = |k: f64| a + (b - a) * k;
        return vec![[a, p(t - g)], [p(t + g), b]];
    }
    vec![[a, b]]
}

/// A drawing's annotations in a line style's leaders and ticks: dimension
/// ticks, leader ends, and curved leaders when the style curves them
/// (stepped leaders draw straight until they are built).
pub fn restyle(items: &[crate::plan::PlanItem], s: &LineStyle) -> Vec<crate::plan::PlanItem> {
    use crate::plan::PlanItem;
    use creabim_core::linestyle::LeaderRun;
    let set = |look: &mut crate::annotation::TextLook| {
        look.end = Some(s.tick);
        look.leader_end = Some(s.leader_end);
        look.curved = s.leader == LeaderRun::Curved;
    };
    items
        .iter()
        .cloned()
        .map(|mut it| {
            match &mut it {
                PlanItem::Dimension { look, .. } | PlanItem::Tag { look, .. } => set(look),
                PlanItem::Annotation { item, .. } => set(&mut item.look),
                _ => {}
            }
            it
        })
        .collect()
}

/// A canvas that draws every line it is given by hand, in black, and passes
/// everything else (fills, text, images) to the canvas under it.
pub struct HandCanvas<'a> {
    pub inner: &'a mut dyn Canvas,
    pub style: LineStyle,
}

impl HandCanvas<'_> {
    fn draw(&mut self, pts: &[DVec2], stroke: Stroke, rgb: Option<[f32; 3]>) {
        // dashed lines (hidden, centre lines) keep their dashes, ruled
        if stroke.dash.is_some() || pts.len() < 2 {
            match rgb {
                Some(c) => self.inner.line_rgb(pts, stroke, c),
                None => self.inner.line(pts, stroke),
            }
            return;
        }
        let pieces: Vec<Vec<DVec2>> = if stroke.width <= HATCH_PEN && pts.len() == 2 {
            hatch_by_hand(pts[0], pts[1], &self.style)
                .into_iter()
                .map(|[a, b]| vec![a, b])
                .collect()
        } else {
            vec![pts.to_vec()]
        };
        for piece in pieces {
            for shape in hand_strokes(&piece, stroke.width, &self.style) {
                match rgb {
                    Some(c) => self.inner.fill_rgb(&shape, c),
                    None => self.inner.fill(&shape, stroke.gray),
                }
            }
        }
    }
}

impl Canvas for HandCanvas<'_> {
    fn line(&mut self, pts: &[DVec2], stroke: Stroke) {
        self.draw(pts, stroke, None);
    }
    fn line_rgb(&mut self, pts: &[DVec2], stroke: Stroke, rgb: [f32; 3]) {
        self.draw(pts, stroke, Some(rgb));
    }
    fn fill(&mut self, pts: &[DVec2], gray: f64) {
        self.inner.fill(pts, gray);
    }
    fn fill_rgb(&mut self, pts: &[DVec2], rgb: [f32; 3]) {
        self.inner.fill_rgb(pts, rgb);
    }
    fn tint(&mut self, pts: &[DVec2], rgb: [f32; 3]) {
        self.inner.tint(pts, rgb);
    }
    fn text_angle(&mut self, at: DVec2, size: f64, text: &str, anchor: Anchor, angle: f64) {
        self.inner.text_angle(at, size, text, anchor, angle);
    }
    fn styled_text(
        &mut self,
        at: DVec2,
        text: &str,
        anchor: Anchor,
        angle: f64,
        look: &crate::annotation::TextLook,
    ) {
        self.inner.styled_text(at, text, anchor, angle, look);
    }
    fn image(&mut self, lo: DVec2, hi: DVec2, clip: Option<(DVec2, DVec2)>, raster: &Arc<Raster>) {
        self.inner.image(lo, hi, clip, raster);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use creabim_core::linestyle::{shipped_style, LineStyle};

    #[test]
    fn hand_lines_are_the_same_every_time_and_overshoot_their_ends() {
        let hand = shipped_style("Hand").unwrap();
        let pts = [DVec2::ZERO, DVec2::new(40.0, 0.0)];
        let a = hand_strokes(&pts, 0.5, &hand);
        assert_eq!(a, hand_strokes(&pts, 0.5, &hand));
        let xs = a.iter().flatten().map(|p| p.x);
        let (lo, hi) = xs.fold((f64::MAX, f64::MIN), |(l, h), x| (l.min(x), h.max(x)));
        assert!(lo < -0.1 && hi > 40.1, "overshoot {lo} {hi}");
        // the line stays straight: the wobble is a fraction of a millimetre
        assert!(a[0].iter().all(|p| p.y.abs() < 0.6));
    }

    #[test]
    fn a_box_lifts_the_pen_at_each_corner() {
        let hand = shipped_style("Hand").unwrap();
        let sq = [
            DVec2::ZERO,
            DVec2::X * 10.0,
            DVec2::new(10.0, 10.0),
            DVec2::Y * 10.0,
            DVec2::ZERO,
        ];
        let ribbons = |s: &LineStyle| {
            hand_marks(&sq, 0.5, s)
                .iter()
                .filter(|m| matches!(m, Mark::Ribbon { .. }))
                .count()
        };
        // four strokes, by hand and ruled alike
        assert_eq!(ribbons(&hand), 4);
        assert_eq!(ribbons(&LineStyle::ruled()), 4);
    }

    #[test]
    fn a_plain_stroke_stays_light() {
        // no wobble: a 40 mm stroke needs a point every 2.5 mm, not every 0.5
        let mut s = shipped_style("Hand").unwrap();
        s.wobble = 0.0;
        let marks = hand_marks(&[DVec2::ZERO, DVec2::X * 40.0], 0.5, &s);
        let Mark::Ribbon { pts, .. } = &marks[0] else {
            panic!()
        };
        assert!(pts.len() <= 20, "{}", pts.len());
    }
}
