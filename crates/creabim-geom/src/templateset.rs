//! CreaBIM's template sets put onto a project (Template sets REV01): what
//! New project does with a set, and what `creabim template-set` writes to
//! a file.
//!
//! A set comes as data: its masters, drawn for each paper size of the ISO
//! and US standards, its palette, its font and its pen set. This puts a
//! set onto the starter house (or an empty project): every sheet takes the
//! master of its paper, the drawings on it are moved into the room the
//! master leaves, and a sheet whose drawings no longer fit steps up to the
//! next paper. The file carries the masters of every paper size of its
//! standard, so a sheet of any size has its title block.

use crate::paper::{Anchor, Canvas, Stroke};
use crate::sheet::{draw_sheet_notes, draw_view_title, placed_views};
use creabim_core::titleblock::SheetTemplate;
use creabim_core::{Document, Element, ElementId, ElementKind, Model, Transaction};
use glam::DVec2;
use serde::Deserialize;

#[derive(Deserialize)]
struct Set {
    #[serde(default)]
    code: String,
    name: String,
    #[serde(default)]
    line: String,
    #[serde(default)]
    palette: Option<String>,
    #[serde(default)]
    font: Option<creabim_core::Font>,
    #[serde(default)]
    pen_set: Option<String>,
    masters: Vec<Master>,
}

#[derive(Deserialize)]
struct Master {
    name: String,
    /// "ISO" or "US".
    standard: String,
    /// The paper's name, "A3" or "ARCH B".
    paper: String,
    /// The kind of drawing it is for ("Plans", "Sections"), in a set that
    /// has a master per kind; empty in the others.
    #[serde(default)]
    series: String,
    /// The room it leaves for drawings, mm from the sheet's lower left:
    /// low x, low y, high x, high y.
    area: [f64; 4],
    /// Which of the template's items are CreaBIM's mark: left off when
    /// the project is started without the mark.
    #[serde(default)]
    mark_items: Vec<usize>,
    template: SheetTemplate,
}

impl Master {
    /// Its title block, with or without CreaBIM's mark.
    fn drawn(&self, mark: bool) -> SheetTemplate {
        let mut t = self.template.clone();
        if !mark {
            let mut k = 0;
            t.items.retain(|_| {
                k += 1;
                !self.mark_items.contains(&(k - 1))
            });
        }
        t
    }
}

/// What a set is called, for a list of sets.
pub struct SetInfo {
    /// Its place in the list ("S1" … "S10").
    pub code: String,
    pub name: String,
    pub line: String,
}

/// A set's name and line, read from its data.
pub fn info(text: &str) -> Result<SetInfo, String> {
    let set: Set = serde_json::from_str(text).map_err(|e| e.to_string())?;
    Ok(SetInfo {
        code: set.code,
        name: set.name,
        line: set.line,
    })
}

/// The set's master for `paper` ("A3"), with or without the mark: the one
/// its list shows a picture of.
pub fn master(text: &str, paper: &str, mark: bool) -> Option<SheetTemplate> {
    let set: Set = serde_json::from_str(text).ok()?;
    let of = |series: &str| {
        set.masters
            .iter()
            .find(|m| m.paper == paper && m.series == series)
    };
    of("").or_else(|| of("Plans")).map(|m| m.drawn(mark))
}

/// The papers of each standard, small to large, and the US paper an ISO
/// sheet goes to.
const ISO: [&str; 5] = ["A4", "A3", "A2", "A1", "A0"];
const ARCH: [&str; 5] = ["ARCH A", "ARCH B", "ARCH C", "ARCH D", "ARCH E"];

/// The inch-foot scale a metric scale goes to.
fn us_scale(metric: f64) -> f64 {
    const PAIRS: [(f64, f64); 9] = [
        (5.0, 4.0),
        (10.0, 12.0),
        (20.0, 24.0),
        (25.0, 24.0),
        (50.0, 48.0),
        (100.0, 96.0),
        (200.0, 192.0),
        (500.0, 480.0),
        (1000.0, 960.0),
    ];
    PAIRS
        .iter()
        .find(|p| (p.0 - metric).abs() < 1e-6)
        .map_or(metric, |p| p.1)
}

/// The kind of drawing a sheet holds, by its number, for a set with a
/// master per kind.
fn series_of(number: &str) -> &'static str {
    match number.chars().find(|c| c.is_ascii_digit()) {
        Some('1') => "Plans",
        Some('2') | Some('3') | Some('4') => "Sections",
        Some('5') | Some('6') | Some('7') | Some('8') => "Details",
        _ => "Cover",
    }
}

/// A canvas that only measures: the box round everything drawn on it.
#[derive(Default)]
struct Bounds(Option<(DVec2, DVec2)>);

impl Bounds {
    fn add(&mut self, p: DVec2) {
        if p.is_finite() {
            self.0 = Some(match self.0 {
                Some((lo, hi)) => (lo.min(p), hi.max(p)),
                None => (p, p),
            });
        }
    }
}

impl Canvas for Bounds {
    fn line(&mut self, pts: &[DVec2], _: Stroke) {
        pts.iter().for_each(|p| self.add(*p));
    }
    fn fill(&mut self, pts: &[DVec2], _: f64) {
        pts.iter().for_each(|p| self.add(*p));
    }
    fn text_angle(&mut self, at: DVec2, size: f64, text: &str, anchor: Anchor, angle: f64) {
        let w = crate::paper::text_width(text, size);
        let along = DVec2::from_angle(angle);
        let (a, b) = match anchor {
            Anchor::Start => (0.0, w),
            Anchor::Middle => (-w / 2.0, w / 2.0),
            Anchor::End => (-w, 0.0),
        };
        for k in [a, b] {
            for up in [-0.5, 0.5] {
                self.add(at + along * k + along.perp() * size * up);
            }
        }
    }
    fn image(
        &mut self,
        lo: DVec2,
        hi: DVec2,
        clip: Option<(DVec2, DVec2)>,
        _: &std::sync::Arc<crate::raster::Raster>,
    ) {
        let (lo, hi) = clip.map_or((lo, hi), |(a, b)| (lo.max(a), hi.min(b)));
        self.add(lo);
        self.add(hi);
    }
}

/// The box round what a sheet holds but its frame: its views, their
/// titles, and the notes written on it.
fn content_box(model: &Model, sheet: ElementId) -> Option<(DVec2, DVec2)> {
    let views = placed_views(model, sheet, &mut |v| {
        crate::section::view_drawing(model, v)
    });
    let mut b = Bounds::default();
    for v in &views {
        crate::paper::draw_looks(&mut b, &v.drawing, &v.map, &v.looks);
        draw_view_title(&mut b, v);
    }
    draw_sheet_notes(&mut b, model, sheet);
    b.0
}

/// What [`apply`] made: the project, the set's name, how many masters it
/// has, and a line on each sheet.
pub struct Applied {
    pub model: Model,
    pub name: String,
    pub masters: usize,
    pub report: Vec<String>,
}

/// Put the set whose data is `text` onto `base`: its masters for every
/// paper of the standard (US or ISO), each sheet on the master of its
/// paper with its drawings in the room the master leaves (a sheet whose
/// drawings no longer fit steps up a paper), the set's palette, font and
/// pens. `first_sheet` adds one sheet, for a project that has none.
/// `mark` keeps CreaBIM's mark on the masters.
pub fn apply(
    text: &str,
    base: Model,
    us: bool,
    first_sheet: bool,
    mark: bool,
) -> Result<Applied, String> {
    let empty = first_sheet;
    let set: Set = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let standard = if us { "US" } else { "ISO" };
    let masters: Vec<&Master> = set
        .masters
        .iter()
        .filter(|m| m.standard == standard)
        .collect();
    if masters.is_empty() {
        return Err(format!("no {standard} masters"));
    }
    for m in &masters {
        m.template
            .validate()
            .map_err(|e| format!("{}: {e}", m.name))?;
    }
    let by_series = masters.iter().any(|m| !m.series.is_empty());
    let ladder: &[&str] = if us { &ARCH } else { &ISO };
    let master_for = |paper: &str, series: &str| {
        masters
            .iter()
            .find(|m| m.paper == paper && (!by_series || m.series == series))
            .copied()
    };

    let mut doc = Document::from_model(base);

    // in US sizes the views take inch-foot scales first, so what is
    // measured below is what the sheets will hold
    if us {
        let mut tx = Transaction::new("US scales");
        let mut any = false;
        for e in doc.model().iter() {
            if let ElementKind::View(v) = &e.kind {
                let s = us_scale(v.scale);
                if (s - v.scale).abs() > 1e-9 {
                    let mut after = e.clone();
                    if let ElementKind::View(v) = &mut after.kind {
                        v.scale = s;
                    }
                    tx = tx.replace(e.clone(), after);
                    any = true;
                }
            }
        }
        if any {
            doc.commit(tx).map_err(|e| format!("{e:?}"))?;
        }
    }

    // the masters: the first takes the place of the project's own, so
    // nothing is left pointing at a master that is gone
    let model = doc.model().clone();
    let mut tx = Transaction::new(format!("Template set: {}", set.name));
    let mut ids: Vec<(usize, ElementId)> = vec![];
    let mut olds = model
        .iter()
        .filter(|e| matches!(e.kind, ElementKind::SheetTemplate(_)));
    for (k, m) in masters.iter().enumerate() {
        let kind = ElementKind::SheetTemplate(m.drawn(mark));
        match olds.next() {
            Some(e) => {
                let mut after = e.clone();
                after.name = m.name.clone();
                after.kind = kind;
                ids.push((k, e.id));
                tx = tx.replace(e.clone(), after);
            }
            None => {
                let e = Element::new(m.name.clone(), kind);
                ids.push((k, e.id));
                tx = tx.insert(e);
            }
        }
    }
    let id_of = |m: &Master| {
        ids.iter()
            .find(|(k, _)| std::ptr::eq(masters[*k], m))
            .map(|p| p.1)
    };

    // every sheet: its paper, its master, its drawings in the master's room
    let mut report = vec![];
    for e in model.iter() {
        let ElementKind::Sheet(s) = &e.kind else {
            continue;
        };
        let was = creabim_core::Sheet::SIZES
            .iter()
            .find(|z| {
                (z.1 - s.width.max(s.height)).abs() < 0.6
                    && (z.2 - s.width.min(s.height)).abs() < 0.6
            })
            .map(|z| z.0)
            .unwrap_or("A3");
        let start = ISO.iter().position(|p| *p == was).unwrap_or(1);
        let series = series_of(&s.number);
        let content = content_box(&model, e.id);
        // the smallest paper, from its own up, whose room holds the drawings
        let mut pick = None;
        for paper in &ladder[start..] {
            let Some(m) = master_for(paper, series) else {
                continue;
            };
            let room = DVec2::new(m.area[2] - m.area[0], m.area[3] - m.area[1]);
            let fits = content.is_none_or(|(lo, hi)| (hi - lo).cmple(room).all());
            pick = Some(m);
            if fits {
                break;
            }
        }
        let Some(m) = pick else {
            return Err(format!("no master for sheet {} ({was})", s.number));
        };
        let paper = m.template.paper.unwrap_or(DVec2::new(s.width, s.height));
        let shift = content.map_or(DVec2::ZERO, |(lo, hi)| {
            DVec2::new(m.area[0] + m.area[2], m.area[1] + m.area[3]) / 2.0 - (lo + hi) / 2.0
        });
        let mut after = e.clone();
        if let ElementKind::Sheet(s) = &mut after.kind {
            s.width = paper.x;
            s.height = paper.y;
            s.template = id_of(m);
        }
        tx = tx.replace(e.clone(), after);
        for other in model.iter() {
            let mut moved = other.clone();
            match &mut moved.kind {
                ElementKind::Viewport(vp) if vp.sheet == e.id => vp.center += shift,
                ElementKind::Note(n) if n.view == e.id => {
                    n.at += shift;
                    if let Some(b) = &mut n.elbow {
                        *b += shift;
                    }
                }
                _ => continue,
            }
            if moved != *other {
                tx = tx.replace(other.clone(), moved);
            }
        }
        let fits = content.is_none_or(|(lo, hi)| {
            (hi - lo)
                .cmple(DVec2::new(m.area[2] - m.area[0], m.area[3] - m.area[1]))
                .all()
        });
        let need = content.map_or(DVec2::ZERO, |(lo, hi)| hi - lo);
        report.push(format!(
            "{:6} {was:3} -> {:7} {:22} holds {:4.0} x {:4.0} in {:4.0} x {:4.0}{}",
            s.number,
            m.paper,
            m.name,
            need.x,
            need.y,
            m.area[2] - m.area[0],
            m.area[3] - m.area[1],
            if fits { "" } else { "   DOES NOT FIT" }
        ));
    }
    // an empty project starts with one sheet, on the paper its standard
    // gives a new sheet
    if empty {
        let std = if us {
            creabim_core::standards::us()
        } else {
            creabim_core::standards::iso()
        };
        let m = master_for(&std.default_sheet, "Plans")
            .or_else(|| master_for(ladder[1], "Plans"))
            .ok_or("no master for the first sheet")?;
        let paper = m.template.paper.ok_or("a master without its paper")?;
        let mut sheet = creabim_core::Sheet::a("A101", "A3");
        sheet.width = paper.x;
        sheet.height = paper.y;
        sheet.template = id_of(m);
        tx = tx.insert(Element::new("Sheet", ElementKind::Sheet(sheet)));
        report.push(format!("A101   new -> {:7} {}", m.paper, m.name));
    }

    // the text styles that were in the plain face take the set's, so the
    // drawings are lettered as their title block is
    if let Some(font) = set.font {
        for e in model.iter() {
            if let ElementKind::TextStyle(t) = &e.kind {
                if t.font == creabim_core::Font::Sans {
                    let mut after = e.clone();
                    if let ElementKind::TextStyle(t) = &mut after.kind {
                        t.font = font;
                    }
                    tx = tx.replace(e.clone(), after);
                }
            }
        }
    }
    // the project's pack: its palette, its font, its pens, its standard
    let found = creabim_core::styles::packs(&model)
        .into_iter()
        .find(|(_, p)| p.default)
        .or(creabim_core::styles::packs(&model).into_iter().next())
        .map(|(id, _)| id);
    let mut pack = creabim_core::styles::active_pack(&model).into_owned();
    pack.default = true;
    if let Some(name) = &set.palette {
        if creabim_core::palette::shipped(name).is_none() {
            // the set's own: light line on charcoal (Gallery)
            let mut own =
                creabim_core::palette::shipped("Blueprint").ok_or("no Blueprint palette")?;
            own.name = name.clone();
            own.hue = 95.0;
            own.paper = 0.0;
            own.ink = 88.0;
            own.fill = 12.0;
            pack.palettes.retain(|p| p.name != *name);
            pack.palettes.push(own);
        }
    }
    pack.palette = set.palette.clone();
    pack.text_font = set.font;
    pack.pen_set = set.pen_set.clone();
    pack.standard = us.then(creabim_core::standards::us);
    match found.and_then(|id| model.get(id)) {
        Some(e) => {
            let mut after = e.clone();
            after.kind = ElementKind::StylePack(Box::new(pack));
            tx = tx.replace(e.clone(), after);
        }
        None => {
            let n = if pack.name.is_empty() {
                set.name.clone()
            } else {
                pack.name.clone()
            };
            tx = tx.insert(Element::new(n, ElementKind::StylePack(Box::new(pack))));
        }
    }
    if us {
        let units = creabim_core::units::Units::imperial();
        match model
            .iter()
            .find(|e| matches!(e.kind, ElementKind::Units(_)))
        {
            Some(e) => {
                let mut after = e.clone();
                after.kind = ElementKind::Units(units);
                tx = tx.replace(e.clone(), after);
            }
            None => tx = tx.insert(Element::new("Units", ElementKind::Units(units))),
        }
    }
    doc.commit(tx).map_err(|e| format!("{e:?}"))?;

    Ok(Applied {
        model: doc.model().clone(),
        name: set.name.clone(),
        masters: masters.len(),
        report,
    })
}
