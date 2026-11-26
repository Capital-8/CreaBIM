//! Angular, radial, diameter and arc-length dimensions and text notes:
//! what they measure and the linework, arrows and text they are drawn
//! with. Everything is in drawing units; `mm` is drawing units per paper
//! millimetre, so arrows, gaps and text keep their size on paper.

use crate::paper::{text_width, Anchor};
use creabim_core::{DimensionKind, Font, TagShape, Terminator, TextStyle, Units};
use glam::DVec2;
use std::f64::consts::{FRAC_PI_2, PI, TAU};

/// How an annotation's text and linework look: from its text style, with
/// the element's colour on top.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextLook {
    /// Letter height, paper mm.
    pub size: f64,
    /// sRGB 0..1; `None` is the view's ink (black on paper).
    pub rgb: Option<[f32; 3]>,
    pub font: Font,
    pub bold: bool,
    pub italic: bool,
    /// Dimension line ends; none is each kind's usual one.
    pub end: Option<Terminator>,
    /// A leader's end; none is an arrow.
    pub leader_end: Option<Terminator>,
    /// Size of the ends, paper mm.
    pub end_size: f64,
    pub shape: TagShape,
    /// A dimension line leaves gaps where tags and notes cross it.
    pub breaks: bool,
    /// How a note's lines line up, each by its real width (MU29: room
    /// tags, placed by estimated widths, read ragged; MU31: the style's
    /// Align).
    pub align: creabim_core::TextAlign,
    /// A note's leader curves to its end.
    pub curved: bool,
    /// The CreaBIM symbol a tag or level mark is drawn with (its kind's
    /// pick, Mockup_Symbols_REV00); none is the usual shape.
    pub symbol: Option<&'static creabim_core::symbols::SymbolStyle>,
    /// The symbol a leader ends in; none is `leader_end`.
    pub leader_symbol: Option<&'static creabim_core::symbols::SymbolStyle>,
}

/// Size of dimension ticks and arrows, paper mm.
pub const END_SIZE: f64 = 2.5;

impl Default for TextLook {
    fn default() -> Self {
        TextStyle::default().into()
    }
}

impl From<TextStyle> for TextLook {
    fn from(s: TextStyle) -> Self {
        Self {
            size: s.size,
            rgb: s.color,
            font: s.font,
            bold: s.bold,
            italic: s.italic,
            end: s.terminator,
            leader_end: s.leader_end,
            end_size: s.end_size.unwrap_or(END_SIZE),
            shape: s.tag_shape,
            breaks: s.break_at_tags,
            align: s.align,
            curved: s.leader_shape == creabim_core::LeaderShape::Curved,
            symbol: None,
            leader_symbol: None,
        }
    }
}

/// The marks of a dimension line's or leader's end, in drawing units.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ends {
    pub thin: Vec<Vec<DVec2>>,
    /// Drawn with the heavier tick pen.
    pub heavy: Vec<Vec<DVec2>>,
    pub fills: Vec<Vec<DVec2>>,
}

impl Ends {
    pub fn extend(&mut self, other: Ends) {
        self.thin.extend(other.thin);
        self.heavy.extend(other.heavy);
        self.fills.extend(other.fills);
    }
}

/// Does a line ending in `t` run on a little past its point, as a line
/// ending in a tick does (arrows end on the point)?
pub fn runs_past(t: Terminator) -> bool {
    matches!(
        t,
        Terminator::Tick | Terminator::Slash | Terminator::TickDot | Terminator::Slug
    )
}

/// A slug: a thick stroke with round ends from `a` to `b`, `w` across,
/// as one filled outline so it reads the same on screen, sheets and PDF.
fn slug(a: DVec2, b: DVec2, w: f64) -> Vec<DVec2> {
    let d = (b - a).normalize_or(DVec2::X);
    let r = w / 2.0;
    let base = d.perp().to_angle();
    let mut out = Vec::with_capacity(20);
    for i in 0..=9 {
        // round end at b, then round end at a
        out.push(b + DVec2::from_angle(base - std::f64::consts::PI * i as f64 / 9.0) * r);
    }
    for i in 0..=9 {
        out.push(
            a + DVec2::from_angle(
                base + std::f64::consts::PI - std::f64::consts::PI * i as f64 / 9.0,
            ) * r,
        );
    }
    out
}

/// The end of a leader at `tip`, arriving along `dir`: as [`end_marks`],
/// but a slug lies along the leader rather than across it.
pub fn leader_marks(t: Terminator, tip: DVec2, dir: DVec2, size: f64, mm: f64) -> Ends {
    if t == Terminator::Slug {
        let d = dir.normalize_or(DVec2::X);
        let s = size * mm;
        let mut out = Ends::default();
        out.fills.push(slug(tip - d * s * 0.75, tip, s * 0.24));
        return out;
    }
    end_marks(t, tip, dir, size, mm)
}

/// A leader made a smooth curve: its shoulder kept, then one sweep to the
/// end, leaving the shoulder along it (so the line flows out of the text)
/// and bending through the elbow when it has one.
pub fn curve_leader(pts: &[DVec2]) -> Vec<DVec2> {
    if pts.len() < 2 {
        return pts.to_vec();
    }
    let n = pts.len();
    let end = pts[n - 1];
    let (keep, from, control) = match n {
        2 => {
            let (a, b) = (pts[0], pts[1]);
            let mid = (a + b) / 2.0;
            // a gentle bow to one side
            let bow = (b - a).perp() * 0.2;
            (vec![a], a, mid + bow)
        }
        3 => {
            let (a, s) = (pts[0], pts[1]);
            let along = (s - a).normalize_or(DVec2::X);
            let reach = s.distance(end) * 0.55;
            (vec![a, s], s, s + along * reach)
        }
        _ => (pts[..n - 2].to_vec(), pts[n - 3], pts[n - 2]),
    };
    let mut out = keep;
    for i in 1..=24 {
        let t = i as f64 / 24.0;
        let u = 1.0 - t;
        out.push(from * (u * u) + control * (2.0 * u * t) + end * (t * t));
    }
    out.dedup_by(|a, b| a.distance(*b) < 1e-12);
    out
}

fn circle_pts(c: DVec2, r: f64, n: usize) -> Vec<DVec2> {
    (0..n)
        .map(|i| c + DVec2::from_angle(i as f64 / n as f64 * TAU) * r)
        .collect()
}

/// The end mark `t` at `tip`, for a line arriving at it along `dir`;
/// `size` is its length on paper, mm.
pub fn end_marks(t: Terminator, tip: DVec2, dir: DVec2, size: f64, mm: f64) -> Ends {
    let d = dir.normalize_or(DVec2::X);
    let n = d.perp();
    let s = size * mm;
    let mut out = Ends::default();
    let tick = (d + n).normalize() * s / 2.0;
    let head = |len: f64, half: f64| {
        let base = tip - d * s * len;
        vec![tip, base + n * s * half, base - n * s * half]
    };
    match t {
        Terminator::Tick => out.heavy.push(vec![tip - tick, tip + tick]),
        Terminator::Slash => out.thin.push(vec![tip - tick * 1.2, tip + tick * 1.2]),
        Terminator::Arrow => out.fills.push(head(1.0, 0.22)),
        Terminator::ArrowSlim => out.fills.push(head(1.2, 0.12)),
        Terminator::ArrowOpen => {
            let h = head(1.0, 0.3);
            out.thin.push(vec![h[1], tip, h[2]]);
        }
        Terminator::ArrowClosed => {
            let mut h = head(1.0, 0.25);
            h.push(tip);
            out.thin.push(h);
        }
        Terminator::Dot => out.fills.push(circle_pts(tip, s * 0.2, 16)),
        Terminator::DotOpen => {
            let mut c = circle_pts(tip, s * 0.24, 16);
            c.push(c[0]);
            out.thin.push(c);
        }
        Terminator::Square => {
            let (a, b) = (d * s * 0.18, n * s * 0.18);
            out.fills
                .push(vec![tip - a - b, tip + a - b, tip + a + b, tip - a + b]);
        }
        Terminator::TickDot => {
            out.heavy.push(vec![tip - tick, tip + tick]);
            out.fills.push(circle_pts(tip, s * 0.13, 12));
        }
        // across a dimension line at 45°, as a tick drawn with a full pen
        Terminator::Slug => out
            .fills
            .push(slug(tip - tick * 0.85, tip + tick * 0.85, s * 0.2)),
        Terminator::None => {}
    }
    out
}

/// A line [`lay_out_note`] wants underlined starts with this.
pub const UNDERLINE: char = '\u{1}';

/// Switches bold on or off within a note's line (from `**bold**`).
pub const BOLD: char = '\u{2}';
/// Switches italic on or off within a note's line (from `*italic*`).
pub const ITALIC: char = '\u{3}';

/// `**bold**` and `*italic*` turned into the switches above, the marks
/// themselves gone; backticks dropped.
fn inline_marks(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        if (c == '*' && next == Some('*')) || (c == '_' && next == Some('_')) {
            out.push(BOLD);
            i += 2;
            continue;
        }
        if c == '*' {
            // a lone star round a word: italic; a star alone ("5 * 3") stays
            let before = i.checked_sub(1).and_then(|j| chars.get(j)).copied();
            let opens = next.is_some_and(|n| !n.is_whitespace());
            let closes = before.is_some_and(|b| !b.is_whitespace());
            if opens || closes {
                out.push(ITALIC);
                i += 1;
                continue;
            }
        }
        if c != '`' {
            out.push(c);
        }
        i += 1;
    }
    out
}

/// A line wrapped onto several carries its open bold and italic on: each
/// line after the first starts with the switches still on.
fn carry_marks(lines: &mut [String]) {
    let (mut bold, mut italic) = (false, false);
    for line in lines.iter_mut() {
        let mut head = String::new();
        if bold {
            head.push(BOLD);
        }
        if italic {
            head.push(ITALIC);
        }
        for c in line.chars() {
            match c {
                BOLD => bold = !bold,
                ITALIC => italic = !italic,
                _ => {}
            }
        }
        if !head.is_empty() {
            let under = line.starts_with(UNDERLINE);
            let rest: String = line.chars().filter(|c| *c != UNDERLINE).collect();
            *line = format!(
                "{}{head}{rest}",
                if under {
                    UNDERLINE.to_string()
                } else {
                    String::new()
                }
            );
        }
    }
}

/// A line's runs: how far each starts from the line's start (paper mm at
/// the look's size), its text and its look, bold and italic switched as
/// the line says. One run when the line has no switches.
pub fn text_runs(text: &str, look: &TextLook) -> Vec<(f64, String, TextLook)> {
    let mut out = vec![];
    let mut cur = *look;
    let mut run = String::new();
    let mut at = 0.0;
    for c in text.chars() {
        if c == BOLD || c == ITALIC {
            if !run.is_empty() {
                let w = cur.width(&run);
                out.push((at, std::mem::take(&mut run), cur));
                at += w;
            }
            if c == BOLD {
                cur.bold = !cur.bold;
            } else {
                cur.italic = !cur.italic;
            }
            continue;
        }
        run.push(c);
    }
    if !run.is_empty() || out.is_empty() {
        out.push((at, run, cur));
    }
    out
}

/// The whole width of a line with switches in it, paper mm.
pub fn runs_width(text: &str, look: &TextLook) -> f64 {
    text_runs(text, look)
        .last()
        .map_or(0.0, |(at, t, l)| at + l.width(t))
}

/// A note's text as the lines it draws (MU26): read as markdown when
/// `markdown` (`#` headings in capitals and underlined, `-` and `*`
/// bullets, `1.` numbers, `- [ ]` and `- [x]` checkboxes, emphasis markers
/// dropped), and wrapped to `width` paper mm when it has one, a list
/// item's later lines set in under its text.
pub fn lay_out_note(text: &str, look: &TextLook, width: Option<f64>, markdown: bool) -> String {
    let mut out: Vec<String> = vec![];
    for raw in text.split('\n') {
        let (mut head, mut body, mut under) = (String::new(), raw.to_string(), false);
        if markdown {
            let t = raw.trim_start();
            let indent = " ".repeat((raw.len() - t.len()).min(8));
            let strip = |s: &str| inline_marks(s);
            if let Some(h) = t
                .strip_prefix("### ")
                .or(t.strip_prefix("## "))
                .or(t.strip_prefix("# "))
            {
                body = strip(h).to_uppercase();
                under = t.starts_with("# ") || t.starts_with("## ");
            } else if let Some(r) = t.strip_prefix("- [ ] ").or(t.strip_prefix("* [ ] ")) {
                (head, body) = (format!("{indent}[ ] "), strip(r));
            } else if let Some(r) = t
                .strip_prefix("- [x] ")
                .or(t.strip_prefix("- [X] "))
                .or(t.strip_prefix("* [x] "))
            {
                (head, body) = (format!("{indent}[x] "), strip(r));
            } else if let Some(r) = t
                .strip_prefix("- ")
                .or(t.strip_prefix("* "))
                .or(t.strip_prefix("+ "))
            {
                (head, body) = (format!("{indent}\u{2022} "), strip(r));
            } else if let Some((n, r)) = t
                .split_once(". ")
                .filter(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
            {
                (head, body) = (format!("{indent}{n}. "), strip(r));
            } else {
                body = strip(t);
            }
        }
        let Some(w) = width.filter(|w| *w > 1.0) else {
            out.push(format!(
                "{}{head}{body}",
                if under {
                    UNDERLINE.to_string()
                } else {
                    String::new()
                }
            ));
            continue;
        };
        // wrap word by word; later lines set in as far as the list mark
        let pad = " ".repeat(head.chars().count() + 1);
        let mut line = head.clone();
        let mut first = true;
        for word in body.split(' ') {
            let trial = if line.trim().is_empty() || line == head {
                format!("{line}{word}")
            } else {
                format!("{line} {word}")
            };
            if look.width(&trial) > w && !(line.trim().is_empty() || line == head) {
                out.push(format!(
                    "{}{line}",
                    if under && first {
                        UNDERLINE.to_string()
                    } else {
                        String::new()
                    }
                ));
                first = false;
                line = format!(
                    "{}{word}",
                    if head.is_empty() {
                        String::new()
                    } else {
                        pad.clone()
                    }
                );
            } else {
                line = trial;
            }
        }
        out.push(format!(
            "{}{line}",
            if under && first {
                UNDERLINE.to_string()
            } else {
                String::new()
            }
        ));
    }
    carry_marks(&mut out);
    out.join("\n")
}

impl TextLook {
    /// Width of `text` on paper, mm: a shipped font's own widths, else
    /// close to Helvetica's, Times' or Courier's.
    pub fn width(&self, text: &str) -> f64 {
        let clean: String;
        let text = if text.contains([BOLD, ITALIC, UNDERLINE]) {
            clean = text
                .chars()
                .filter(|c| !matches!(*c, BOLD | ITALIC | UNDERLINE))
                .collect();
            clean.as_str()
        } else {
            text
        };
        let w = match self.font {
            Font::Mono => text.chars().count() as f64 * 0.6 * self.size,
            Font::Serif => text_width(text, self.size) * 0.95,
            Font::Sans => text_width(text, self.size),
            f => crate::fonts::width(f, text, self.size)
                .unwrap_or_else(|| text_width(text, self.size)),
        };
        if self.bold {
            w * 1.08
        } else {
            w
        }
    }

    /// The same look `factor` times as big, for drawings shown at another scale.
    pub fn scaled(self, factor: f64) -> Self {
        Self {
            size: self.size * factor,
            ..self
        }
    }
}

// ---- measuring --------------------------------------------------------------

/// The angle at `vertex` turning counter-clockwise from `a` to `b`, in
/// radians, in (0, 2π]. Zero-length arms measure 0.
pub fn sweep(vertex: DVec2, a: DVec2, b: DVec2) -> f64 {
    let (da, db) = (a - vertex, b - vertex);
    if da.length_squared() < 1e-18 || db.length_squared() < 1e-18 {
        return 0.0;
    }
    let s = (db.to_angle() - da.to_angle()).rem_euclid(TAU);
    if s < 1e-12 {
        TAU
    } else {
        s
    }
}

/// An angle as written: degrees, to a tenth where it is not whole.
pub fn angle_text(radians: f64) -> String {
    let d = (radians.to_degrees() * 10.0).round() / 10.0;
    if (d - d.round()).abs() < 1e-9 {
        format!("{d:.0}°")
    } else {
        format!("{d:.1}°")
    }
}

/// A length as dimensions write it in `units`.
pub fn length_text(units: &Units, metres: f64) -> String {
    units.dimension(metres)
}

/// Where the endless lines through `a0`,`a1` and `b0`,`b1` cross; none when
/// they are parallel.
pub fn line_intersection(a0: DVec2, a1: DVec2, b0: DVec2, b1: DVec2) -> Option<DVec2> {
    let (da, db) = (a1 - a0, b1 - b0);
    let den = da.perp_dot(db);
    if den.abs() < 1e-12 * da.length() * db.length() || den == 0.0 {
        return None;
    }
    let t = (b0 - a0).perp_dot(db) / den;
    Some(a0 + da * t)
}

/// The circle through three points, as (centre, radius); none when they
/// are in a line.
pub fn circle_through(p: DVec2, q: DVec2, r: DVec2) -> Option<(DVec2, f64)> {
    let (b, c) = (q - p, r - p);
    let d = 2.0 * b.perp_dot(c);
    if d.abs() < 1e-12 {
        return None;
    }
    let (bb, cc) = (b.length_squared(), c.length_squared());
    let o = DVec2::new(c.y * bb - b.y * cc, b.x * cc - c.x * bb) / d;
    Some((p + o, o.length()))
}

/// What a dimension of `kind` reads, from its resolved points.
pub fn measure(units: &Units, kind: DimensionKind, center: DVec2, a: DVec2, b: DVec2) -> String {
    let l = |m: f64| length_text(units, m);
    match kind {
        DimensionKind::Linear => l(a.distance(b)),
        DimensionKind::Angular => angle_text(sweep(center, a, b)),
        DimensionKind::Radius => format!("R{}", l(center.distance(a))),
        DimensionKind::Diameter => format!("Ø{}", l(2.0 * center.distance(a))),
        DimensionKind::ArcLength => l(center.distance(a) * sweep(center, a, b)),
        // its height comes from the model, not its points
        DimensionKind::Spot => String::new(),
    }
}

// ---- shapes -----------------------------------------------------------------

/// The geometry of an annotation, resolved in its view.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    /// An arc `radius` from `vertex`, from the arm through `a`
    /// counter-clockwise to the arm through `b`.
    Angle {
        vertex: DVec2,
        a: DVec2,
        b: DVec2,
        radius: f64,
    },
    /// A line from the centre (or, for a diameter, from across the circle)
    /// to `at` on it, running on `extra` past the circle.
    Radius {
        center: DVec2,
        at: DVec2,
        extra: f64,
        diameter: bool,
    },
    /// An arc `offset` outside the arc about `center` from `a`
    /// counter-clockwise to `b`.
    ArcLength {
        center: DVec2,
        a: DVec2,
        b: DVec2,
        offset: f64,
    },
    /// Lines of text from `at`, with a leader to `leader`, bent at
    /// `elbow` when it has one.
    Note {
        at: DVec2,
        leader: Option<DVec2>,
        elbow: Option<DVec2>,
    },
    /// A spot level: a mark on the point `at` (a triangle standing on it
    /// in a section or elevation, a target in a plan, where it is a level
    /// marker) and a leader to `text_at`, where its height (the
    /// annotation's text) sits on a short line.
    Spot {
        at: DVec2,
        text_at: DVec2,
        plan: bool,
    },
    /// A level's head where its datum line ends at `at`, Revit style: a
    /// target symbol, the level's name (the annotation's text) above the
    /// line and its `elevation` below.
    Level {
        at: DVec2,
        elevation: String,
        /// Where the datum line ends, when the head was moved off it (to
        /// clear another level's, or by a drag): a dog-leg joins the two.
        line_end: Option<DVec2>,
    },
}

/// Radius of a level head's target symbol, paper mm.
pub const LEVEL_HEAD: f64 = 1.5;

/// Clear paper a level head keeps above and below its text, mm.
pub const LEVEL_ROOM: f64 = 6.0;

/// An annotation ready to draw: its shape, what it reads and how it looks.
#[derive(Clone, Debug, PartialEq)]
pub struct Annotation {
    pub shape: Shape,
    /// The value of a dimension, or a note's text (lines split by `\n`).
    pub text: String,
    pub look: TextLook,
    /// Drawing units per paper mm at the view's own scale, for its extent.
    pub mm: f64,
}

/// A piece of text in [`Marks`].
#[derive(Clone, Debug, PartialEq)]
pub struct MarkText {
    pub at: DVec2,
    pub text: String,
    /// Radians counter-clockwise, kept readable.
    pub angle: f64,
    pub anchor: Anchor,
}

/// The linework of an annotation, in drawing units.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Marks {
    /// Thin open polylines.
    pub lines: Vec<Vec<DVec2>>,
    /// Filled arrowheads.
    pub arrows: Vec<[DVec2; 3]>,
    pub texts: Vec<MarkText>,
    /// Other line ends a style picked: ticks, dots, open arrows.
    pub ends: Ends,
}

/// Arrowhead length and half width, paper mm.
const ARROW: (f64, f64) = (2.5, 0.55);
/// Gap between text and its line, paper mm.
const TEXT_GAP: f64 = 1.0;

/// An arrowhead with its tip at `tip`, pointing along `dir`.
fn arrow(tip: DVec2, dir: DVec2, mm: f64) -> [DVec2; 3] {
    let d = dir.normalize_or(DVec2::X);
    let base = tip - d * ARROW.0 * mm;
    let n = d.perp() * ARROW.1 * mm;
    [tip, base + n, base - n]
}

impl Marks {
    /// A line's end at `tip`, arriving along `dir`: `t`, or the usual
    /// filled arrow.
    fn end(&mut self, t: Option<Terminator>, tip: DVec2, dir: DVec2, look: &TextLook, mm: f64) {
        match t.unwrap_or(Terminator::Arrow) {
            Terminator::Arrow => self
                .arrows
                .push(arrow(tip, dir, mm * look.end_size / END_SIZE)),
            t => self
                .ends
                .extend(leader_marks(t, tip, dir, look.end_size, mm)),
        }
    }
}

/// A direction's angle turned so text along it reads left to right (or
/// bottom to top), and the side that is then "up".
pub fn upright(angle: f64) -> (f64, DVec2) {
    let mut a = angle.rem_euclid(TAU);
    if a > PI {
        a -= TAU;
    }
    if a > FRAC_PI_2 + 1e-6 || a <= -FRAC_PI_2 + 1e-6 {
        a += PI;
        if a > PI {
            a -= TAU;
        }
    }
    (a, DVec2::from_angle(a).perp())
}

/// Points along an arc about `c`, from angle `t0` turning `s`.
fn arc_points(c: DVec2, r: f64, t0: f64, s: f64) -> Vec<DVec2> {
    let n = ((s.abs() / TAU * 72.0).ceil() as usize).max(6);
    (0..=n)
        .map(|i| c + DVec2::from_angle(t0 + s * i as f64 / n as f64) * r)
        .collect()
}

/// An arc dimension line about `c` at radius `r` from `t0` turning `s`,
/// with arrows at both ends pointing out, and its text outside the middle.
fn arc_line(out: &mut Marks, c: DVec2, r: f64, t0: f64, s: f64, look: &TextLook, mm: f64) -> DVec2 {
    out.lines.push(arc_points(c, r, t0, s));
    let (p0, p1) = (DVec2::from_angle(t0), DVec2::from_angle(t0 + s));
    out.end(look.end, c + p0 * r, -p0.perp(), look, mm);
    out.end(look.end, c + p1 * r, p1.perp(), look, mm);
    let mid = DVec2::from_angle(t0 + s / 2.0);
    c + mid * (r + (TEXT_GAP + look.size / 2.0) * mm)
}

/// Text centred on `at`, reading along the direction `angle`.
fn text_along(at: DVec2, angle: f64, text: &str) -> MarkText {
    MarkText {
        at,
        text: text.to_string(),
        angle: upright(angle).0,
        anchor: Anchor::Middle,
    }
}

/// Lines and arrows from `from` out radially to `to`, with a gap at the
/// measured point, for an arc's witness lines.
fn witness(out: &mut Marks, c: DVec2, dir: DVec2, from: f64, to: f64, mm: f64) {
    let s = (to - from).signum();
    if (to - from).abs() > 2.0 * mm {
        out.lines.push(vec![
            c + dir * (from + s * mm),
            c + dir * (to + s * 1.5 * mm),
        ]);
    }
}

impl Annotation {
    /// Linework at `mm` drawing units per paper millimetre.
    pub fn marks(&self, mm: f64) -> Marks {
        let mut out = Marks::default();
        let look = &self.look;
        match self.shape {
            Shape::Angle {
                vertex,
                a,
                b,
                radius,
            } => {
                let s = sweep(vertex, a, b);
                if s <= 0.0 {
                    return out;
                }
                let r = radius.abs().max(3.0 * mm);
                let t0 = (a - vertex).to_angle();
                for (p, t) in [(a, t0), (b, t0 + s)] {
                    let d = DVec2::from_angle(t);
                    let reach = p.distance(vertex);
                    if r > reach {
                        witness(&mut out, vertex, d, reach, r, mm);
                    }
                }
                let at = arc_line(&mut out, vertex, r, t0, s, look, mm);
                out.texts
                    .push(text_along(at, t0 + s / 2.0 + FRAC_PI_2, &self.text));
            }
            Shape::Radius {
                center,
                at,
                extra,
                diameter,
            } => {
                let r = center.distance(at);
                let d = (at - center).normalize_or(DVec2::X);
                let end = at + d * extra.max(0.0);
                let start = if diameter { center - d * r } else { center };
                out.lines.push(vec![start, end]);
                out.end(look.end, at, d, look, mm);
                if diameter {
                    out.end(look.end, center - d * r, -d, look, mm);
                }
                // a small cross at the centre
                let c = 1.5 * mm;
                out.lines
                    .push(vec![center - DVec2::X * c, center + DVec2::X * c]);
                out.lines
                    .push(vec![center - DVec2::Y * c, center + DVec2::Y * c]);
                // text beside the line: past the circle when the line runs
                // on out, else halfway along the radius
                let along = if extra > look.width(&self.text) * mm {
                    at + d * (extra / 2.0)
                } else {
                    center + (at - center) * 0.55
                };
                let (angle, up) = upright(d.to_angle());
                out.texts.push(MarkText {
                    at: along + up * (TEXT_GAP + look.size / 2.0) * mm,
                    text: self.text.clone(),
                    angle,
                    anchor: Anchor::Middle,
                });
            }
            Shape::ArcLength {
                center,
                a,
                b,
                offset,
            } => {
                let s = sweep(center, a, b);
                let r = center.distance(a);
                if s <= 0.0 || r <= 0.0 {
                    return out;
                }
                let rr = if r + offset > 1e-9 { r + offset } else { r };
                let t0 = (a - center).to_angle();
                for t in [t0, t0 + s] {
                    witness(&mut out, center, DVec2::from_angle(t), r, rr, mm);
                }
                let at = arc_line(&mut out, center, rr, t0, s, look, mm);
                let mid = t0 + s / 2.0 + FRAC_PI_2;
                out.texts.push(text_along(at, mid, &self.text));
                // the arc-length sign: a small arc over the figures
                let (angle, up) = upright(mid);
                let along = DVec2::from_angle(angle);
                let half = look.width(&self.text) * 0.35 * mm;
                let lift = at + up * look.size * 0.75 * mm;
                let h = look.size * 0.35 * mm;
                out.lines.push(
                    (0..=12)
                        .map(|i| {
                            let t = PI * i as f64 / 12.0;
                            lift + along * (-half * t.cos()) + up * (h * t.sin())
                        })
                        .collect(),
                );
            }
            Shape::Spot { at, text_at, plan } => {
                let w = look.width(&self.text) * mm;
                let dir = if text_at.x >= at.x { 1.0 } else { -1.0 };
                let shoulder = text_at + DVec2::X * dir * (w + 2.0 * TEXT_GAP * mm);
                let from = if plan {
                    target(&mut out, at, LEVEL_HEAD * mm);
                    at + (text_at - at).normalize_or(DVec2::X) * LEVEL_HEAD * mm
                } else {
                    // a filled triangle standing on the point
                    let (h, half) = (2.0 * mm, 1.15 * mm);
                    out.arrows
                        .push([at, at + DVec2::new(-half, h), at + DVec2::new(half, h)]);
                    at + DVec2::Y * h
                };
                if from.distance(text_at) > 0.5 * mm {
                    out.lines.push(vec![from, text_at, shoulder]);
                } else {
                    out.lines.push(vec![text_at, shoulder]);
                }
                out.texts.push(MarkText {
                    at: text_at + DVec2::new(dir * TEXT_GAP * mm, (look.size * 0.5 + 0.6) * mm),
                    text: self.text.clone(),
                    angle: 0.0,
                    anchor: if dir > 0.0 {
                        Anchor::Start
                    } else {
                        Anchor::End
                    },
                });
            }
            Shape::Level {
                at,
                ref elevation,
                line_end,
            } => {
                let r = LEVEL_HEAD * mm;
                if let Some(end) = line_end {
                    // a dog-leg from the datum's end across to the head
                    let s = if at.x < end.x { -1.0 } else { 1.0 };
                    let x = DVec2::X * s;
                    out.lines.push(vec![
                        end,
                        end + x * 0.75 * mm,
                        at - x * (r + 0.75 * mm),
                        at - x * r,
                    ]);
                }
                // a target: two opposite quarters filled in, or the
                // level symbol picked
                let x = match look.symbol {
                    Some(s) => {
                        let sh = crate::symbol::level_shapes(s, at, r);
                        // the text clear of the mark itself (its datum
                        // line aside)
                        let body = sh
                            .lines
                            .iter()
                            .filter(|(l, _)| {
                                let (lo, hi) = l.iter().fold((f64::MAX, f64::MIN), |(a, b), p| {
                                    (a.min(p.y), b.max(p.y))
                                });
                                hi - lo > 1e-9
                            })
                            .flat_map(|(l, _)| l.iter())
                            .chain(sh.fills.iter().flatten())
                            .map(|p| p.x)
                            .fold(at.x, f64::max);
                        let x = body + TEXT_GAP * mm;
                        out.ends.extend(sh.into_ends());
                        x
                    }
                    None => {
                        target(&mut out, at, r);
                        at.x + r + TEXT_GAP * mm
                    }
                };
                let dy = (look.size * 0.5 + 0.8) * mm;
                for (y, text) in [(dy, self.text.clone()), (-dy, elevation.clone())] {
                    out.texts.push(MarkText {
                        at: DVec2::new(x, at.y + y),
                        text,
                        angle: 0.0,
                        anchor: Anchor::Start,
                    });
                }
            }
            Shape::Note { at, leader, elbow } => {
                let lines: Vec<&str> = self.text.split('\n').collect();
                let step = look.size * 1.5 * mm;
                let width = lines
                    .iter()
                    .map(|l| look.width(l.trim_start_matches(UNDERLINE)))
                    .fold(0.0, f64::max)
                    * mm;
                for (i, l) in lines.iter().enumerate() {
                    let y = at - DVec2::Y * step * i as f64;
                    // centred lines on the block's middle, right-aligned on
                    // its right edge: the block's box stays put
                    let (y, anchor) = match look.align {
                        creabim_core::TextAlign::Centre => {
                            (y + DVec2::X * width / 2.0, Anchor::Middle)
                        }
                        creabim_core::TextAlign::Right => (y + DVec2::X * width, Anchor::End),
                        _ => (y, Anchor::Start),
                    };
                    // a markdown heading: underlined (MU26)
                    let text = match l.strip_prefix(UNDERLINE) {
                        Some(t) => {
                            let under = y - DVec2::Y * look.size * 0.75 * mm;
                            out.lines
                                .push(vec![under, under + DVec2::X * look.width(t) * mm]);
                            t
                        }
                        None => l,
                    };
                    out.texts.push(MarkText {
                        at: y,
                        text: text.to_string(),
                        angle: 0.0,
                        anchor,
                    });
                }
                if let Some(target) = leader {
                    // the block's foot, under its last line
                    let below = step * (lines.len() - 1) as f64 + look.size * 0.75 * mm;
                    let mut lines = note_leader_from(at, width, below, target, elbow, mm);
                    if look.curved {
                        for l in &mut lines {
                            *l = curve_leader(l);
                        }
                    }
                    out.lines.extend(lines);
                    let from = out
                        .lines
                        .last()
                        .and_then(|l| l.iter().rev().nth(1).copied());
                    if let Some(from) = from {
                        match look.leader_symbol {
                            Some(s) => out.ends.extend(crate::symbol::leader_end(
                                s,
                                target,
                                target - from,
                                look.end_size,
                                mm,
                            )),
                            None => out.end(look.leader_end, target, target - from, look, mm),
                        }
                    }
                }
            }
        }
        out
    }

    /// Corners of what it covers, at its own view's scale.
    pub fn extent(&self) -> Vec<DVec2> {
        let mm = self.mm;
        let m = self.marks(mm);
        let mut pts: Vec<DVec2> = m.lines.iter().flatten().copied().collect();
        pts.extend(m.arrows.iter().flatten());
        pts.extend(
            m.ends
                .thin
                .iter()
                .chain(&m.ends.heavy)
                .chain(&m.ends.fills)
                .flatten(),
        );
        for t in &m.texts {
            pts.extend(text_box(t, &self.look, mm));
        }
        // a level head keeps clear paper above and below it, so the top
        // and bottom levels of a section don't sit on the drawing's edge
        if let Shape::Level { at, .. } = self.shape {
            let room = (LEVEL_ROOM + self.look.size * 2.0) * mm;
            pts.extend([at + DVec2::Y * room, at - DVec2::Y * room]);
        }
        pts
    }

    /// Is `p` on its linework or text, within `tol`?
    pub fn hit(&self, p: DVec2, tol: f64, mm: f64) -> bool {
        let m = self.marks(mm);
        let near = m
            .lines
            .iter()
            .any(|l| l.windows(2).any(|w| segment_distance(p, w[0], w[1]) < tol));
        near || m.texts.iter().any(|t| {
            let w = self.look.width(&t.text) * mm;
            let h = self.look.size * mm;
            let (dir, up) = (
                DVec2::from_angle(t.angle),
                DVec2::from_angle(t.angle).perp(),
            );
            let x0 = match t.anchor {
                Anchor::Start => 0.0,
                Anchor::Middle => -w / 2.0,
                Anchor::End => -w,
            };
            let x = (p - t.at).dot(dir);
            let y = (p - t.at).dot(up);
            x >= x0 - tol && x <= x0 + w + tol && y.abs() <= h * 0.7 + tol
        })
    }

    /// The point a drag grabs it by: a note's first line, a dimension's text.
    pub fn grip(&self, mm: f64) -> Option<DVec2> {
        match self.shape {
            Shape::Note { at, .. } => Some(at),
            _ => self.marks(mm).texts.first().map(|t| t.at),
        }
    }
}

/// A level target at `at`: a circle with two opposite quarters filled.
fn target(out: &mut Marks, at: DVec2, r: f64) {
    out.lines.push(arc_points(at, r, 0.0, TAU));
    for q in [0.0, PI] {
        for i in 0..4 {
            let t = |k: usize| q + FRAC_PI_2 * k as f64 / 4.0;
            out.arrows.push([
                at,
                at + DVec2::from_angle(t(i)) * r,
                at + DVec2::from_angle(t(i + 1)) * r,
            ]);
        }
    }
}

impl Annotation {
    /// The boxes of its text, at its own view's scale: what a dimension
    /// line breaks around.
    pub fn text_boxes(&self) -> Vec<[DVec2; 4]> {
        self.marks(self.mm)
            .texts
            .iter()
            .map(|t| text_box(t, &self.look, self.mm))
            .collect()
    }
}

/// Corners of a piece of text's box.
fn text_box(t: &MarkText, look: &TextLook, mm: f64) -> [DVec2; 4] {
    let w = look.width(&t.text) * mm;
    let h = look.size * 0.6 * mm;
    let (dir, up) = (
        DVec2::from_angle(t.angle),
        DVec2::from_angle(t.angle).perp(),
    );
    let x0 = match t.anchor {
        Anchor::Start => 0.0,
        Anchor::Middle => -w / 2.0,
        Anchor::End => -w,
    };
    let o = t.at + dir * x0;
    [
        o - up * h,
        o + dir * w - up * h,
        o + dir * w + up * h,
        o + up * h,
    ]
}

/// [`note_leader`] for a block of text reaching `below` under its first
/// line: when the target (or the elbow) lies under the block, within its
/// width, the leader drops plumb from under the block to it, as a level
/// is tagged over the ground (Mockup_Tags_And_Splay_REV00, MU142: from
/// beside the text it ran out along a shoulder and back).
pub fn note_leader_from(
    at: DVec2,
    width: f64,
    below: f64,
    target: DVec2,
    elbow: Option<DVec2>,
    mm: f64,
) -> Vec<Vec<DVec2>> {
    let toward = elbow.unwrap_or(target);
    let foot = DVec2::new(toward.x, at.y - below - mm);
    if toward.y < foot.y - mm && toward.x >= at.x && toward.x <= at.x + width {
        return vec![match elbow {
            Some(e) => vec![foot, e, target],
            None => vec![foot, target],
        }];
    }
    note_leader(at, width, target, elbow, mm)
}

/// A note's leader: from beside its first line (left or right, whichever
/// faces the target, or the elbow) along a short shoulder, then to the
/// target; through `elbow` when it has one.
pub fn note_leader(
    at: DVec2,
    width: f64,
    target: DVec2,
    elbow: Option<DVec2>,
    mm: f64,
) -> Vec<Vec<DVec2>> {
    let toward = elbow.unwrap_or(target);
    let left = toward.x < at.x + width / 2.0;
    let (start, dir) = if left {
        (at - DVec2::X * mm, -1.0)
    } else {
        (at + DVec2::X * (width + mm), 1.0)
    };
    let shoulder = start + DVec2::X * dir * 2.5 * mm;
    if let Some(elbow) = elbow {
        // a shoulder only when the elbow is out past it
        if (elbow.x - start.x) * dir < 3.0 * mm {
            return vec![vec![start, elbow, target]];
        }
        return vec![vec![start, shoulder, elbow, target]];
    }
    // no shoulder when the target is right beside the text
    if (target.x - start.x) * dir < 3.0 * mm {
        return vec![vec![start, target]];
    }
    vec![vec![start, shoulder, target]]
}

fn segment_distance(p: DVec2, a: DVec2, b: DVec2) -> f64 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-18)).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn angles_are_measured_counter_clockwise() {
        let o = DVec2::new(2.0, 1.0);
        let east = o + DVec2::X * 3.0;
        let north = o + DVec2::Y * 0.5;
        assert!(close(sweep(o, east, north), FRAC_PI_2));
        // the other way round is the reflex angle
        assert!(close(sweep(o, north, east), 3.0 * FRAC_PI_2));
        let diag = o + DVec2::new(1.0, 1.0);
        assert_eq!(angle_text(sweep(o, east, diag)), "45°");
        assert_eq!(angle_text(30.25f64.to_radians()), "30.3°");
        assert_eq!(
            measure(&Units::metric(), DimensionKind::Angular, o, east, north),
            "90°"
        );
    }

    #[test]
    fn radius_diameter_and_arc_length_read_in_millimetres() {
        let c = DVec2::new(1.0, 1.0);
        let a = c + DVec2::X * 1.5;
        let b = c + DVec2::Y * 1.5;
        assert_eq!(
            measure(&Units::metric(), DimensionKind::Radius, c, a, a),
            "R1500"
        );
        assert_eq!(
            measure(&Units::metric(), DimensionKind::Diameter, c, a, a),
            "Ø3000"
        );
        // a quarter of a 1.5 m circle
        assert_eq!(
            measure(&Units::metric(), DimensionKind::ArcLength, c, a, b),
            "2356"
        );
        assert_eq!(
            measure(&Units::metric(), DimensionKind::Linear, c, a, b),
            "2121"
        );
    }

    #[test]
    fn circles_and_lines_meet_where_expected() {
        let (c, r) = circle_through(
            DVec2::new(3.0, 0.0),
            DVec2::new(0.0, 3.0),
            DVec2::new(-3.0, 0.0),
        )
        .unwrap();
        assert!(c.length() < 1e-9 && close(r, 3.0));
        assert!(circle_through(DVec2::ZERO, DVec2::X, DVec2::X * 2.0).is_none());
        let x = line_intersection(
            DVec2::ZERO,
            DVec2::X,
            DVec2::new(2.0, 1.0),
            DVec2::new(2.0, 3.0),
        )
        .unwrap();
        assert!(x.distance(DVec2::new(2.0, 0.0)) < 1e-9);
        assert!(line_intersection(DVec2::ZERO, DVec2::X, DVec2::Y, DVec2::ONE).is_none());
    }

    fn note(text: &str, leader: Option<DVec2>) -> Annotation {
        Annotation {
            shape: Shape::Note {
                elbow: None,
                at: DVec2::new(10.0, 5.0),
                leader,
            },
            text: text.into(),
            look: TextLook::default(),
            mm: 0.1,
        }
    }

    #[test]
    fn a_text_box_reads_markdown_and_wraps_to_its_width() {
        let look = TextLook::default();
        let t = "# Notes\n- one **two** three four five six seven\n- [x] done\n2. next";
        let flat = lay_out_note(t, &look, None, true);
        let lines: Vec<&str> = flat.split('\n').collect();
        assert_eq!(lines[0], format!("{UNDERLINE}NOTES"));
        // **two** is bold on paper: its stars become the bold switch
        assert_eq!(
            lines[1],
            "\u{2022} one \u{2}two\u{2} three four five six seven"
        );
        assert_eq!(lines[2], "[x] done");
        assert_eq!(lines[3], "2. next");
        // 30 mm wide: the bullet wraps, its later lines set in
        let boxed = lay_out_note(t, &look, Some(30.0), true);
        let lines: Vec<&str> = boxed.split('\n').collect();
        assert!(lines.len() > 4, "{lines:?}");
        assert!(lines[2].starts_with("  "), "{lines:?}");
        for l in &lines {
            assert!(
                look.width(l.trim_start_matches(UNDERLINE)) <= 30.0 + look.width("seven"),
                "{l}"
            );
        }
        // plain text, no box: as it was
        assert_eq!(lay_out_note("a\n- b", &look, None, false), "a\n- b");
    }

    /// MU142 (Mockup_Tags_And_Splay_REV00): a target under the text,
    /// within its width, takes a leader dropped plumb from under the text,
    /// not one out from its end along a shoulder; a target off to the side
    /// keeps the shoulder.
    #[test]
    fn a_leader_to_a_point_under_the_text_drops_plumb_to_it() {
        let w = TextLook::default().width("Note") * 0.1;
        let under = DVec2::new(10.0 + w * 0.3, 2.0);
        let m = note("Note", Some(under)).marks(0.1);
        let leader = &m.lines[0];
        assert_eq!(leader.len(), 2, "{leader:?}");
        assert!(
            (leader[0].x - under.x).abs() < 1e-9,
            "plumb over it: {leader:?}"
        );
        assert!(leader[0].y < 5.0, "from under the text: {leader:?}");
        assert_eq!(leader[1], under);
        // off to the side: from its end, with a shoulder, as before
        let m = note("Note", Some(DVec2::new(7.0, 3.0))).marks(0.1);
        assert_eq!(m.lines[0].len(), 3);
    }

    #[test]
    fn notes_stack_their_lines_and_point_their_leader() {
        let n = note("First\nSecond\nThird", None);
        let m = n.marks(0.1);
        assert_eq!(m.texts.len(), 3);
        assert!(m.lines.is_empty() && m.arrows.is_empty());
        // lines run down the page, 1.5 letter heights apart
        assert!(close(m.texts[0].at.y - m.texts[1].at.y, 2.5 * 1.5 * 0.1));
        assert!(m.texts.iter().all(|t| t.anchor == Anchor::Start));
        // a target to the lower left: the leader leaves the left end with a shoulder
        let target = DVec2::new(7.0, 3.0);
        let m = note("Note", Some(target)).marks(0.1);
        let leader = &m.lines[0];
        assert_eq!(leader.len(), 3);
        assert!(leader[0].x < 10.0 && close(leader[0].y, 5.0));
        assert!(close(leader[1].y, 5.0) && leader[1].x < leader[0].x);
        assert_eq!(*leader.last().unwrap(), target);
        // its arrow's tip is on the target, pointing at it
        let [tip, b0, b1] = m.arrows[0];
        assert_eq!(tip, target);
        let back = ((b0 + b1) / 2.0 - tip).normalize();
        let along = (leader[1] - target).normalize();
        assert!(back.distance(along) < 1e-9);
        // to the right, it leaves the right end
        let m = note("Note", Some(DVec2::new(14.0, 8.0))).marks(0.1);
        assert!(m.lines[0][0].x > 10.0 + TextLook::default().width("Note") * 0.1);
        // it can be picked by its text or its leader
        let n = note("Note", Some(target));
        assert!(n.hit(DVec2::new(10.2, 5.0), 0.05, 0.1));
        assert!(n.hit((leader[1] + target) / 2.0, 0.05, 0.1));
        assert!(!n.hit(DVec2::new(20.0, 20.0), 0.05, 0.1));
    }

    #[test]
    fn angle_arcs_run_from_arm_to_arm() {
        let a = Annotation {
            shape: Shape::Angle {
                vertex: DVec2::ZERO,
                a: DVec2::X * 5.0,
                b: DVec2::Y * 5.0,
                radius: 2.0,
            },
            text: "90°".into(),
            look: TextLook::default(),
            mm: 0.1,
        };
        let m = a.marks(0.1);
        let arc = &m.lines[0];
        assert!(arc[0].distance(DVec2::X * 2.0) < 1e-9);
        assert!(arc.last().unwrap().distance(DVec2::Y * 2.0) < 1e-9);
        assert_eq!(m.arrows.len(), 2);
        // the text sits outside the arc, on the bisector, reading upright
        let t = &m.texts[0];
        assert!(t.at.length() > 2.0 && close(t.at.x, t.at.y));
        assert!(t.angle.abs() <= FRAC_PI_2 + 1e-9);
        // an arc further out than the arms gets witness lines
        let far = Annotation {
            shape: Shape::Angle {
                vertex: DVec2::ZERO,
                a: DVec2::X,
                b: DVec2::Y,
                radius: 3.0,
            },
            ..a
        };
        assert_eq!(far.marks(0.1).lines.len(), 3);
    }

    #[test]
    fn readable_text_never_hangs_upside_down() {
        for deg in (-360..=360).step_by(15) {
            let (a, up) = upright((deg as f64).to_radians());
            assert!(a > -FRAC_PI_2 - 1e-6 && a <= FRAC_PI_2 + 1e-6, "{deg}");
            assert!(up.y >= -1e-6, "{deg}");
        }
    }

    #[test]
    fn a_slug_lies_across_a_dimension_and_along_a_leader() {
        let dim = end_marks(Terminator::Slug, DVec2::ZERO, DVec2::X, 2.5, 1.0);
        let leader = leader_marks(Terminator::Slug, DVec2::ZERO, DVec2::X, 2.5, 1.0);
        assert_eq!((dim.fills.len(), leader.fills.len()), (1, 1));
        let span = |pts: &Vec<DVec2>| {
            let (lo, hi) = pts
                .iter()
                .fold((DVec2::MAX, DVec2::MIN), |(l, h), p| (l.min(*p), h.max(*p)));
            hi - lo
        };
        // across: as tall as it is long; along: long in x, thin in y
        let d = span(&dim.fills[0]);
        assert!((d.x - d.y).abs() < 0.05, "{d}");
        let l = span(&leader.fills[0]);
        assert!(l.x > 3.0 * l.y, "{l}");
        // it ends on the leader's point, from behind it
        assert!(leader.fills[0].iter().all(|p| p.x <= 0.31));
        assert!(runs_past(Terminator::Slug));
    }

    #[test]
    fn a_curved_leader_keeps_its_ends_and_leaves_the_shoulder_smoothly() {
        let (start, shoulder, target) =
            (DVec2::ZERO, DVec2::new(-2.5, 0.0), DVec2::new(-20.0, -15.0));
        let c = curve_leader(&[start, shoulder, target]);
        assert_eq!(c[0], start);
        assert_eq!(c[1], shoulder);
        assert!(c.last().unwrap().distance(target) < 1e-9);
        assert!(c.len() > 10);
        // the first step off the shoulder still runs along it (left)
        let first = (c[2] - c[1]).normalize();
        assert!(first.x < -0.9, "{first}");
        // through an elbow: it bends toward it, ends on the target
        let elbow = DVec2::new(-10.0, 0.0);
        let c = curve_leader(&[start, shoulder, elbow, target]);
        assert!(c.last().unwrap().distance(target) < 1e-9);
        assert!(c.iter().any(|p| p.x < -6.0 && p.y > -3.0));
    }
}
