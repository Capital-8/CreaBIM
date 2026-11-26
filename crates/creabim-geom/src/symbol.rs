//! Drawing with CreaBIM Symbols (Mockup_Symbols_REV00): a view's marks swap
//! to the symbol their kind picks, and a symbol's strokes, fills and rings
//! come out as lines and shapes in drawing units, its text in its slot.

use creabim_core::symbols::SymbolStyle;
use creabim_core::{ElementKind, Model, View};
use glam::DVec2;

use crate::annotation::Ends;
use crate::plan::{Pen, PlanItem};

/// Paper millimetres per symbol grid step at the 2.5 mm text size: a grid
/// bubble's ring (radius 10 steps) comes out at the 5 mm bubble it replaces.
pub const STEP_MM: f64 = 0.25;

/// Paper millimetres per step for view marks (sections, elevations,
/// callouts, north arrows): a ring of 10 steps is the 10 mm bubble.
pub const MARK_STEP_MM: f64 = 0.5;

/// Which point of a symbol is put on the spot it is placed at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// Its text slot (or its anchor when it has none): bubbles and tags.
    Text,
    /// Its anchor: a leader's tip, a level's datum, a callout's tail.
    Anchor,
    /// The middle of its grid: section, elevation and north marks.
    Centre,
}

/// A symbol's linework in drawing units.
pub struct SymbolShapes {
    /// Strokes and rings as polylines, each with its pen width in paper mm.
    pub lines: Vec<(Vec<DVec2>, f64)>,
    pub fills: Vec<Vec<DVec2>>,
    /// The text slot's centre, where the text goes.
    pub text_at: DVec2,
}

impl SymbolShapes {
    /// Every point it draws.
    pub fn points(&self) -> impl Iterator<Item = DVec2> + '_ {
        self.lines
            .iter()
            .flat_map(|l| l.0.iter().copied())
            .chain(self.fills.iter().flatten().copied())
    }

    /// Its box, or none when it draws nothing.
    pub fn bounds(&self) -> Option<(DVec2, DVec2)> {
        let mut it = self.points();
        let first = it.next()?;
        Some(it.fold((first, first), |(lo, hi), p| (lo.min(p), hi.max(p))))
    }

    /// As line ends: strokes heavier than a normal line go to the heavy pen.
    pub fn into_ends(self) -> Ends {
        let mut e = Ends::default();
        for (l, w) in self.lines {
            if w > 0.2 {
                e.heavy.push(l);
            } else {
                e.thin.push(l);
            }
        }
        e.fills = self.fills;
        e
    }
}

/// Symbol `s` placed with its `origin` on `at`, `unit` drawing units a
/// grid step (across and up), turned `angle` radians about `at`. Its text
/// slot turns with it; the text itself stays upright.
pub fn shapes_at(
    s: &SymbolStyle,
    at: DVec2,
    unit: DVec2,
    angle: f64,
    origin: Origin,
) -> SymbolShapes {
    let o = match origin {
        Origin::Text => s
            .text
            .as_ref()
            .map(|t| DVec2::from(t.at))
            .unwrap_or(DVec2::from(s.anchor.at)),
        Origin::Anchor => DVec2::from(s.anchor.at),
        Origin::Centre => DVec2::ZERO,
    };
    let turn = DVec2::from_angle(angle);
    let to = |p: [f64; 2]| at + turn.rotate((DVec2::from(p) - o) * unit);
    let mut lines = vec![];
    for st in &s.strokes {
        let w = 0.18 * (st.weight / 3.0).clamp(0.6, 2.0);
        lines.push((st.pts.iter().map(|p| to(*p)).collect(), w));
    }
    for c in &s.circles {
        let ring: Vec<DVec2> = (0..=36)
            .map(|i| {
                let a = i as f64 / 36.0 * std::f64::consts::TAU;
                to([c[0] + a.cos() * c[2], c[1] + a.sin() * c[2]])
            })
            .collect();
        lines.push((ring, 0.18));
    }
    let fills = s
        .fills
        .iter()
        .map(|f| f.pts.iter().map(|p| to(*p)).collect())
        .collect();
    let text_at = s.text.as_ref().map_or(at, |t| to(text_slot(s, t.at)));
    SymbolShapes {
        lines,
        fills,
        text_at,
    }
}

/// Where a symbol's text sits in its grid. A section head's slot sits
/// just off its split line, which a letter of the drawn size would cross
/// (seen on the first sheets, 2026-11-16): it moves out to clear it.
fn text_slot(s: &SymbolStyle, at: [f64; 2]) -> [f64; 2] {
    if s.id.starts_with("sections.") && at[1].abs() >= 0.5 {
        return [
            at[0],
            at[1].signum() * at[1].abs().max(SECTION_TEXT_MM / MARK_STEP_MM / 2.0 + 1.0),
        ];
    }
    at
}

/// The letter height of section and elevation marks drawn with a symbol.
pub const SECTION_TEXT_MM: f64 = 2.8;

/// Symbol `s` with its text slot on `at`, `unit` drawing units a step.
pub fn shapes(s: &SymbolStyle, at: DVec2, unit: f64) -> SymbolShapes {
    shapes_at(s, at, DVec2::splat(unit), 0.0, Origin::Text)
}

/// The text height, paper mm, of a symbol drawn `step_mm` paper mm a step.
pub fn text_mm(step_mm: f64) -> f64 {
    (step_mm * 10.0).min(3.5)
}

/// The width of a symbol's grid it draws over, in steps.
pub fn span(s: &SymbolStyle) -> (f64, f64) {
    let sh = shapes_at(s, DVec2::ZERO, DVec2::ONE, 0.0, Origin::Centre);
    sh.bounds()
        .map_or((20.0, 20.0), |(lo, hi)| (hi.x - lo.x, hi.y - lo.y))
}

#[allow(clippy::too_many_arguments)]
fn symbol(
    id: creabim_core::ElementId,
    s: &'static SymbolStyle,
    at: DVec2,
    text: String,
    unit: f64,
    angle: f64,
    origin: Origin,
    size: f64,
) -> PlanItem {
    PlanItem::Symbol {
        id,
        symbol: s,
        at,
        text,
        unit,
        angle,
        origin,
        size,
    }
}

/// Swap view `v`'s marks for the symbols its kinds pick: grid bubbles,
/// section and elevation marks, tags, level marks and leader ends.
/// Callouts are drawn with theirs where they are made.
pub fn apply(model: &Model, v: &View, items: &mut Vec<PlanItem>) {
    let mm = v.paper_scale() / 1000.0;
    let pick = |k: &str| creabim_core::symbols::pick(model, v, k);
    if let Some(grid) = pick("grids") {
        let unit = STEP_MM * mm;
        for it in items.iter_mut() {
            if let PlanItem::Label { id, at, text } = it {
                let is_grid = matches!(model.get(*id).map(|e| &e.kind), Some(ElementKind::Grid(_)));
                if is_grid && crate::plan::is_bubble(text) {
                    *it = symbol(
                        *id,
                        grid,
                        *at,
                        std::mem::take(text),
                        unit,
                        0.0,
                        Origin::Text,
                        text_mm(STEP_MM),
                    );
                }
            }
        }
    }
    if matches!(v.kind, creabim_core::ViewKind::Plan { .. }) {
        let (sec, ele) = (pick("sections"), pick("elevations"));
        if sec.is_some() || ele.is_some() {
            sections(model, items, sec, ele, mm);
        }
    }
    let (tag, level, arrow) = (pick("tags"), pick("levels"), pick("arrows"));
    if tag.is_some() || level.is_some() || arrow.is_some() {
        for it in items.iter_mut() {
            match it {
                PlanItem::Tag { look, .. } => {
                    look.symbol = tag.or(look.symbol);
                    look.leader_symbol = arrow.or(look.leader_symbol);
                }
                PlanItem::Annotation { item, .. } => match item.shape {
                    crate::annotation::Shape::Level { .. } => {
                        item.look.symbol = level.or(item.look.symbol)
                    }
                    crate::annotation::Shape::Note { .. } => {
                        item.look.leader_symbol = arrow.or(item.look.leader_symbol)
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }
}

/// A plan's section and elevation marks redrawn with their symbols: the
/// line between a section's ends kept, its heads and arrows replaced by
/// the symbol, turned to look the way the view looks.
fn sections(
    model: &Model,
    items: &mut Vec<PlanItem>,
    sec: Option<&'static SymbolStyle>,
    ele: Option<&'static SymbolStyle>,
    mm: f64,
) {
    let mut views: Vec<(
        creabim_core::ElementId,
        DVec2,
        DVec2,
        bool,
        &'static SymbolStyle,
    )> = vec![];
    for e in model.iter() {
        let ElementKind::View(v) = &e.kind else {
            continue;
        };
        if v.callout.is_some() {
            continue;
        }
        if let creabim_core::ViewKind::Section {
            start, end, cut, ..
        } = v.kind
        {
            if let Some(s) = if cut { sec } else { ele } {
                if items.iter().any(|it| it.id() == e.id) {
                    views.push((e.id, start, end, cut, s));
                }
            }
        }
    }
    if views.is_empty() {
        return;
    }
    items.retain(|it| {
        let Some((_, a, b, cut, _)) = views.iter().find(|w| w.0 == it.id()) else {
            return true;
        };
        // a section keeps the lines that run along it, between its ends
        let on = |p: DVec2| {
            let d = *b - *a;
            let t = (p - *a).dot(d) / d.length_squared().max(1e-12);
            (-1e-6..=1.0 + 1e-6).contains(&t) && (*a + d * t).distance(p) < 1e-6
        };
        *cut && matches!(it, PlanItem::Line { a: p, b: q, .. } if on(*p) && on(*q))
    });
    let unit = MARK_STEP_MM * mm;
    for (id, a, b, cut, s) in views {
        let along = (b - a).normalize_or_zero();
        let look = along.perp();
        let angle = look.to_angle() - std::f64::consts::FRAC_PI_2;
        let number = crate::markers::number_of(model, id);
        let size = SECTION_TEXT_MM;
        if cut {
            if !items.iter().any(|it| it.id() == id) {
                items.push(PlanItem::Line {
                    id,
                    pen: Pen::Grid,
                    a,
                    b,
                });
            }
            let r = crate::markers::BUBBLE * mm;
            for (p, d) in [(a, -along), (b, along)] {
                items.push(symbol(
                    id,
                    s,
                    p + d * r,
                    number.clone(),
                    unit,
                    angle,
                    Origin::Centre,
                    size,
                ));
            }
        } else {
            items.push(symbol(
                id,
                s,
                (a + b) / 2.0,
                number,
                unit,
                angle,
                Origin::Centre,
                size,
            ));
        }
    }
}

/// A callout's head drawn as symbol `s`: its tail's end on the box's
/// `corner`, the ring out beyond it.
pub fn callout_head(
    id: creabim_core::ElementId,
    s: &'static SymbolStyle,
    corner: DVec2,
    number: String,
    mm: f64,
) -> PlanItem {
    symbol(
        id,
        s,
        corner,
        number,
        MARK_STEP_MM * mm,
        std::f64::consts::PI,
        Origin::Anchor,
        text_mm(MARK_STEP_MM).min(3.2),
    )
}

/// A leader's end drawn as symbol `s`: its tip on `tip`, arriving along
/// `dir`, `size` paper mm long.
pub fn leader_end(s: &SymbolStyle, tip: DVec2, dir: DVec2, size: f64, mm: f64) -> Ends {
    let (w, _) = span(s);
    // the head, not the stem: most ends draw a 14-step stem behind a
    // 7-step head
    let unit = size * mm / (w * 0.5).max(4.0);
    shapes_at(s, tip, DVec2::splat(unit), dir.to_angle(), Origin::Anchor).into_ends()
}

/// A tag's outline drawn as symbol `s` about `at`, sized to hold `text_w`
/// (paper mm) of text `text_h` tall.
pub fn tag_shapes(s: &SymbolStyle, at: DVec2, text_w: f64, text_h: f64, mm: f64) -> SymbolShapes {
    let (w, h) = span(s);
    // tall enough for the text, then wide enough inside the outline where
    // the text sits: a diamond or hexagon narrows away from its middle
    // (MU84: tag text crowded a small diamond's edges)
    let mut step = text_h * 1.75 / h.max(1.0);
    let slot = s
        .text
        .as_ref()
        .map_or(DVec2::from(s.anchor.at), |t| DVec2::from(t.at));
    let need = text_w + text_h * 0.9;
    for _ in 0..4 {
        let half = text_h * 0.55 / step;
        let inside = inner_width(s, slot, half).unwrap_or(w * 0.8);
        step = step.max(need / inside.max(1.0));
    }
    shapes_at(s, at, DVec2::splat(step * mm), 0.0, Origin::Text)
}

/// The width, in grid steps, free inside symbol `s` across a band `half`
/// steps above and below `slot`: the narrowest of its top, middle and
/// bottom lines between the outline's nearest crossings either side.
/// None when the outline is open on a side there.
pub fn inner_width(s: &SymbolStyle, slot: DVec2, half: f64) -> Option<f64> {
    let mut segs: Vec<(DVec2, DVec2)> = vec![];
    for st in &s.strokes {
        for w in st.pts.windows(2) {
            segs.push((DVec2::from(w[0]), DVec2::from(w[1])));
        }
    }
    for c in &s.circles {
        let ring: Vec<DVec2> = (0..=48)
            .map(|i| {
                let a = i as f64 / 48.0 * std::f64::consts::TAU;
                DVec2::new(c[0] + a.cos() * c[2], c[1] + a.sin() * c[2])
            })
            .collect();
        for w in ring.windows(2) {
            segs.push((w[0], w[1]));
        }
    }
    let mut narrowest: Option<f64> = None;
    for y in [slot.y - half, slot.y, slot.y + half] {
        let (mut left, mut right) = (None::<f64>, None::<f64>);
        for (a, b) in &segs {
            if (a.y - y) * (b.y - y) > 0.0 || (a.y - b.y).abs() < 1e-9 {
                continue;
            }
            let x = a.x + (b.x - a.x) * (y - a.y) / (b.y - a.y);
            if x < slot.x {
                left = Some(left.map_or(x, |l: f64| l.max(x)));
            } else {
                right = Some(right.map_or(x, |r: f64| r.min(x)));
            }
        }
        let w = right? - left?;
        narrowest = Some(narrowest.map_or(w, |n: f64| n.min(w)));
    }
    narrowest
}

/// The paper size of a symbol's text drawn `step_mm` a grid step: its
/// slot's size, as the Symbols page draws it.
pub fn slot_text_mm(s: &SymbolStyle, step_mm: f64) -> f64 {
    s.text.as_ref().map_or(2.5, |t| t.size * 1.4 * step_mm)
}

/// A level mark drawn as symbol `s`, its anchor on the datum at `at`, the
/// size of the usual head of radius `head` (drawing units).
pub fn level_shapes(s: &SymbolStyle, at: DVec2, head: f64) -> SymbolShapes {
    shapes_at(s, at, DVec2::splat(head / 6.0), 0.0, Origin::Anchor)
}

/// Paper millimetres per step of a symbol placed with the Symbol tool: a
/// 20-step symbol is 10 mm on paper, its text about 2 mm (MU84: 2.5 mm
/// text swamped a 7 mm slope arrow).
pub const PLACED_STEP_MM: f64 = 0.5;

/// A symbol placed with the Symbol tool: its middle on `at`, turned
/// `angle` degrees; `mm` drawing units a paper millimetre.
pub fn placed(
    id: creabim_core::ElementId,
    s: &'static SymbolStyle,
    at: DVec2,
    text: String,
    angle: f64,
    mm: f64,
) -> PlanItem {
    symbol(
        id,
        s,
        at,
        text,
        PLACED_STEP_MM * mm,
        angle.to_radians(),
        Origin::Centre,
        slot_text_mm(s, PLACED_STEP_MM),
    )
}
