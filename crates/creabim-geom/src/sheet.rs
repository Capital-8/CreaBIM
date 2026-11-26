//! Sheets: views laid out on paper with a border and title block, exported
//! as SVG or a multi-page PDF. The same layout drives the app's sheet view.

use crate::paper::{self, Anchor, Canvas, PaperMap, PdfWriter, Stroke, SvgCanvas};
use crate::section::{view_drawing, Drawing};
use creabim_core::titleblock::{fill_fields, TextAlign, TitleItem};
use creabim_core::{ElementId, ElementKind, Model, Sheet, SheetTemplate, View, Viewport};
use glam::DVec2;
use std::collections::BTreeMap;

/// Text for the title block that does not live on the sheet itself.
#[derive(Clone, Debug, Default)]
pub struct SheetInfo {
    pub project: String,
    pub date: String,
    /// Print on white stock: the master's paper tint and texture are
    /// left off, the drawing keeps its colours (pens stage 7).
    pub print_white: bool,
}

/// A viewport resolved for drawing: its view's live drawing and where it lands.
#[derive(Clone, Debug)]
pub struct PlacedView {
    pub viewport: ElementId,
    pub view: ElementId,
    /// Position on the sheet, 1-based, in model order.
    pub number: usize,
    pub title: String,
    pub map: PaperMap,
    pub drawing: Drawing,
    /// The pens it is drawn with: its view's line weights.
    pub weights: creabim_core::LineWeights,
    /// Those pens with the object styles, patterns and overrides of the
    /// project's style pack and the view.
    pub looks: std::sync::Arc<crate::looks::Looks>,
    /// The drawing's extent on paper, mm.
    pub bounds: (DVec2, DVec2),
    /// How its title reads (MU27).
    pub title_look: creabim_core::SheetTitle,
    /// Its title's text style, resolved (MU28).
    pub title_text: crate::annotation::TextLook,
    /// Its scale as the project's drawing standard writes it
    /// (`1/4" = 1'-0"`, `1:50`).
    pub scale_text: String,
}

/// A view's title height on its sheet: its own, or, left at the default,
/// the scale table's for the view's row.
pub fn title_size(model: &Model, view: &creabim_core::View) -> f64 {
    let own = view.sheet_title.size;
    if (own - creabim_core::SheetTitle::default().size).abs() < 1e-9 {
        creabim_core::styles::view_rule(model, view).title_text
    } else {
        own
    }
}

impl PlacedView {
    /// Where the view title sits, below the drawing.
    pub fn title_at(&self) -> DVec2 {
        DVec2::new(self.bounds.0.x, self.bounds.0.y - 7.0) + self.title_look.offset
    }

    /// The title's extent on paper, to pick and drag it.
    pub fn title_box(&self) -> Option<(DVec2, DVec2)> {
        if self.title_look.hidden || self.map.scale == 1000.0 {
            return None;
        }
        let at = self.title_at();
        let s = self.title_look.size;
        let w = 9.0 + paper::text_width(&self.title, s).max(30.0);
        Some((at - DVec2::new(0.5, s * 1.4), at + DVec2::new(w, s * 1.2)))
    }
}

/// Today's date as YYYY-MM-DD (UTC), for title blocks.
pub fn today() -> String {
    let days = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() / 86_400) as i64;
    // days since 1970-01-01 to a civil date (Howard Hinnant's algorithm)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

pub const MARGIN: f64 = 10.0;
/// Extra margin on the binding (left) edge.
pub const BINDING: f64 = 20.0;
pub const TITLE_BLOCK: DVec2 = DVec2::new(170.0, 45.0);

pub fn sheet_of(model: &Model, id: ElementId) -> Option<&Sheet> {
    match &model.get(id)?.kind {
        ElementKind::Sheet(s) => Some(s),
        _ => None,
    }
}

fn view_of(model: &Model, id: ElementId) -> Option<&View> {
    match &model.get(id)?.kind {
        ElementKind::View(v) => Some(v),
        _ => None,
    }
}

/// Every sheet in the model, ordered by sheet number.
pub fn sheets(model: &Model) -> Vec<(ElementId, &Sheet)> {
    let mut v: Vec<(ElementId, &Sheet)> = model
        .iter()
        .filter_map(|e| match &e.kind {
            ElementKind::Sheet(s) => Some((e.id, s)),
            _ => None,
        })
        .collect();
    v.sort_by_key(|a| set_order(&a.1.number));
    v
}

/// Where a sheet number falls in a set: by its discipline, in the order
/// US sets bind them (the National CAD Standard: general first, then
/// civil, landscape, structural, architectural, interiors, the services),
/// then by the number. A number that does not start with a discipline's
/// letters (A101, 1.01) keeps plain order after the general and civil
/// sheets, so ISO-style sets sort as they always did.
pub fn set_order(number: &str) -> (usize, String) {
    const DISCIPLINES: [&str; 21] = [
        "G", "H", "V", "B", "C", "L", "S", "A", "I", "Q", "F", "P", "D", "M", "E", "W", "T", "R",
        "X", "Z", "O",
    ];
    let letters: String = number
        .chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .collect();
    let rest = &number[letters.len()..];
    // a discipline is one or two letters followed by a separator or a digit
    let is_discipline = !letters.is_empty()
        && letters.len() <= 2
        && rest.starts_with(|c: char| c == '-' || c == '.' || c == ' ' || c.is_ascii_digit());
    let first = letters
        .chars()
        .next()
        .map(|c| c.to_ascii_uppercase().to_string());
    let rank = match first.filter(|_| is_discipline) {
        Some(f) => DISCIPLINES
            .iter()
            .position(|d| *d == f)
            .unwrap_or(DISCIPLINES.len()),
        None => DISCIPLINES.iter().position(|d| *d == "A").unwrap(),
    };
    (rank, number.to_string())
}

/// The viewports on a sheet with their drawings. `drawing` supplies each
/// view's drawing, so callers can cache them.
pub fn placed_views(
    model: &Model,
    sheet: ElementId,
    drawing: &mut dyn FnMut(ElementId) -> Option<Drawing>,
) -> Vec<PlacedView> {
    placed_views_with(model, sheet, drawing, &mut |id, v| {
        std::sync::Arc::new(crate::looks::Looks::for_view(model, id, v))
    })
}

/// [`placed_views`] with `looks` supplying each view's looks (given the
/// view as its template resolves it), so callers can cache those too.
pub fn placed_views_with(
    model: &Model,
    sheet: ElementId,
    drawing: &mut dyn FnMut(ElementId) -> Option<Drawing>,
    looks: &mut dyn FnMut(ElementId, &View) -> std::sync::Arc<crate::looks::Looks>,
) -> Vec<PlacedView> {
    let mut out = Vec::new();
    for e in model.iter() {
        let ElementKind::Viewport(vp) = &e.kind else {
            continue;
        };
        if vp.sheet != sheet {
            continue;
        }
        let (Some(view), Some(mut d)) = (view_of(model, vp.view), drawing(vp.view)) else {
            continue;
        };
        // its scale and pens as its view template gives them
        let view = creabim_core::styles::resolved(model, view);
        let looks = looks(vp.view, &view);
        let (crop, target) = viewport_frame(model, vp);
        if let Some([lo, hi]) = crop {
            d = crate::crop::crop(&d, lo, hi);
        }
        let map = PaperMap {
            model: target,
            paper: vp.center,
            scale: view.paper_scale(),
        };
        let bounds = d
            .bounds()
            .map(|(lo, hi)| (map.to_paper(lo), map.to_paper(hi)))
            .unwrap_or((vp.center, vp.center));
        out.push(PlacedView {
            viewport: e.id,
            view: vp.view,
            number: out.len() + 1,
            title: Some(view.sheet_title.text.trim())
                .filter(|t| !t.is_empty())
                .map(str::to_string)
                .or_else(|| model.get(vp.view).map(|v| v.name.clone()))
                .unwrap_or_default(),
            title_look: {
                let mut t = (*view.sheet_title).clone();
                t.size = title_size(model, &view);
                t
            },
            title_text: {
                let mut l = crate::annotate::look_of(model, view.sheet_title.style, None);
                l.size = title_size(model, &view);
                l
            },
            scale_text: creabim_core::standards::scale_label(model, map.scale),
            map,
            drawing: d,
            weights: looks.weights,
            looks,
            bounds,
        });
    }
    arrange_schedules(model, sheet, &mut out);
    out
}

/// Between arranged schedules, paper mm.
pub const ARRANGE_GAP: f64 = 10.0;

/// Does `sheet` arrange its schedules: its own say, else when every
/// viewport on it shows a schedule.
pub fn arranges(model: &Model, sheet: ElementId) -> bool {
    let Some(ElementKind::Sheet(sh)) = model.get(sheet).map(|e| &e.kind) else {
        return false;
    };
    if let Some(on) = sh.arrange {
        return on;
    }
    let mut any = false;
    for e in model.iter() {
        let ElementKind::Viewport(vp) = &e.kind else {
            continue;
        };
        if vp.sheet != sheet {
            continue;
        }
        match view_of(model, vp.view).map(|v| &v.kind) {
            Some(creabim_core::ViewKind::Schedule { .. }) => any = true,
            _ => return false,
        }
    }
    any
}

/// Lay a sheet's schedules out in columns from its top left, each under
/// the one before, a table too tall for what is left of a column starting
/// the next (MU131: a schedule grown taller ran over the one under it).
/// Schedules placed by hand (`pinned`), drawings and the title block stay
/// and are kept clear of.
fn arrange_schedules(model: &Model, sheet: ElementId, out: &mut [PlacedView]) {
    if !arranges(model, sheet) {
        return;
    }
    let Some(ElementKind::Sheet(sh)) = model.get(sheet).map(|e| &e.kind) else {
        return;
    };
    let pinned = |id: ElementId| matches!(model.get(id).map(|e| &e.kind), Some(ElementKind::Viewport(vp)) if vp.pinned);
    let schedule = |v: &PlacedView| {
        matches!(
            view_of(model, v.view).map(|w| &w.kind),
            Some(creabim_core::ViewKind::Schedule { .. })
        )
    };
    // the free area: inside the border, beside or above the title block
    let (mut x0, mut y0, mut x1, y1) = (
        MARGIN + 10.0,
        MARGIN + 10.0,
        sh.width - MARGIN - 10.0,
        sh.height - MARGIN - 10.0,
    );
    let mut taken: Vec<(DVec2, DVec2)> = vec![];
    if let Some((lo, hi)) = title_block_box(model, sheet) {
        if hi.x - lo.x < sh.width / 2.0 {
            x1 = x1.min(lo.x - 10.0);
        } else {
            y0 = y0.max(hi.y + 10.0);
        }
        taken.push((lo, hi));
    }
    for v in out.iter() {
        if !schedule(v) || pinned(v.viewport) {
            taken.push(v.bounds);
        }
    }
    let gap = ARRANGE_GAP;
    let clear = |lo: DVec2, hi: DVec2, taken: &[(DVec2, DVec2)]| {
        !taken.iter().any(|(a, b)| {
            lo.x < b.x + gap / 2.0
                && a.x < hi.x + gap / 2.0
                && lo.y < b.y + gap / 2.0
                && a.y < hi.y + gap / 2.0
        })
    };
    let (mut x, mut top, mut col_w) = (x0, y1, 0.0_f64);
    for v in out.iter_mut() {
        if !schedule(v) || pinned(v.viewport) {
            continue;
        }
        let size = v.bounds.1 - v.bounds.0;
        // down this column, else the next, past what is taken
        let mut guard = 0;
        let spot = loop {
            guard += 1;
            if guard > 400 {
                break None;
            }
            let lo = DVec2::new(x, top - size.y);
            let hi = DVec2::new(x + size.x, top);
            if lo.y < y0 {
                // the column is full: the next one
                x += col_w.max(1.0) + gap;
                top = y1;
                col_w = 0.0;
                if x + size.x > x1 + 1e-6 && x > x0 {
                    break None;
                }
                continue;
            }
            match taken.iter().find(|(a, b)| !clear(lo, hi, &[(*a, *b)])) {
                // under whatever is in the way
                Some((a, _)) => top = a.y - gap,
                None => break Some(lo),
            }
        };
        // no room left on the sheet: it stays where it was put
        let Some(lo) = spot else { continue };
        let d = lo - v.bounds.0;
        v.map.paper += d;
        v.bounds = (v.bounds.0 + d, v.bounds.1 + d);
        taken.push(v.bounds);
        top = lo.y - gap;
        col_w = col_w.max(size.x);
        let _ = x0;
        x0 = x0.min(x);
    }
}

/// What a viewport shows of its drawing and the drawing point pinned to
/// its centre: its own crop and target, or, while it follows an element
/// (`fit`) that is still there, that element's footprint with a margin and
/// the footprint's middle.
pub fn viewport_frame(model: &Model, vp: &Viewport) -> (Option<[DVec2; 2]>, DVec2) {
    let plan = matches!(
        view_of(model, vp.view).map(|v| &v.kind),
        Some(creabim_core::ViewKind::Plan { .. })
    );
    match vp
        .fit
        .filter(|_| plan)
        .and_then(|id| crate::stairsheet::fitted(model, id))
    {
        Some([lo, hi]) => (Some([lo, hi]), (lo + hi) / 2.0),
        None => (vp.crop, vp.target),
    }
}

/// A viewport for `view` centred on `at` (sheet mm), pinning the middle of
/// the view's current drawing there.
pub fn new_viewport(model: &Model, sheet: ElementId, view: ElementId, at: DVec2) -> Viewport {
    let target = view_drawing(model, view)
        .and_then(|d| d.bounds())
        .map_or(DVec2::ZERO, |(lo, hi)| (lo + hi) / 2.0);
    Viewport {
        sheet,
        view,
        center: at,
        target,
        crop: None,
        fit: None,
        pinned: false,
    }
}

/// A spot on the sheet where `view` fits without covering the views
/// already there or the title block; the middle of the sheet if none does.
pub fn free_spot(model: &Model, sheet: ElementId, view: ElementId) -> DVec2 {
    let Some(s) = sheet_of(model, sheet) else {
        return DVec2::ZERO;
    };
    let middle = DVec2::new(s.width, s.height) / 2.0;
    let (Some(v), Some((lo, hi))) = (
        view_of(model, view),
        view_drawing(model, view).and_then(|d| d.bounds()),
    ) else {
        return middle;
    };
    let k = 1000.0 / v.paper_scale();
    // half size on paper, with room for the title below
    let half = (hi - lo) * k / 2.0 + DVec2::new(4.0, 6.0);
    let mut taken: Vec<(DVec2, DVec2)> =
        placed_views(model, sheet, &mut |v| view_drawing(model, v))
            .iter()
            .map(|p| {
                (
                    p.bounds.0 - DVec2::new(4.0, 14.0),
                    p.bounds.1 + DVec2::splat(4.0),
                )
            })
            .collect();
    taken.push(title_block_bounds(model, s));
    let (x0, x1) = (BINDING + half.x + 5.0, s.width - MARGIN - half.x - 5.0);
    let (y0, y1) = (MARGIN + half.y + 10.0, s.height - MARGIN - half.y - 5.0);
    let mut y = y1;
    while y >= y0 {
        let mut x = x0;
        while x <= x1 {
            let c = DVec2::new(x, y);
            let (a, b) = (c - half - DVec2::new(0.0, 8.0), c + half);
            if taken
                .iter()
                .all(|(p, q)| b.x < p.x || a.x > q.x || b.y < p.y || a.y > q.y)
            {
                return c;
            }
            x += 10.0;
        }
        y -= 10.0;
    }
    middle
}

/// The template a sheet's title block comes from, if it has one that
/// still exists.
pub fn template_of<'a>(model: &'a Model, sheet: &Sheet) -> Option<&'a SheetTemplate> {
    match &model.get(sheet.template?)?.kind {
        ElementKind::SheetTemplate(t) => Some(t),
        _ => None,
    }
}

/// The values a sheet's title block fields read: the built-in ones, then
/// its template's own (their defaults, unless the sheet sets them).
pub fn field_values(
    sheet: &Sheet,
    template: Option<&SheetTemplate>,
    title: &str,
    scales: &str,
    info: &SheetInfo,
) -> BTreeMap<String, String> {
    let mut v = BTreeMap::new();
    if let Some(t) = template {
        for (k, d) in &t.fields {
            v.insert(k.clone(), d.clone());
        }
    }
    for (k, val) in &sheet.fields {
        v.insert(k.clone(), val.clone());
    }
    v.insert("sheet".into(), title.to_string());
    v.insert("number".into(), sheet.number.clone());
    v.insert("project".into(), info.project.clone());
    v.insert("scale".into(), scales.to_string());
    v.insert("date".into(), info.date.clone());
    v
}

/// A sheet's title-block values with the project's information (A7-5)
/// filled in: over the template's defaults, under the sheet's own.
pub fn frame_values(
    model: &Model,
    sheet: &Sheet,
    template: Option<&SheetTemplate>,
    title: &str,
    scales: &str,
    info: &SheetInfo,
) -> BTreeMap<String, String> {
    let mut values = field_values(sheet, template, title, scales, info);
    if let Some((_, project)) = model.project_info() {
        for (k, v) in project.fields() {
            if !sheet.fields.contains_key(k) {
                values.insert(k.to_string(), v);
            }
        }
    }
    values
}

/// Where a sheet's title block sits, mm from its lower left: the
/// template's drawing, or the built-in block.
pub fn title_block_bounds(model: &Model, sheet: &Sheet) -> (DVec2, DVec2) {
    if let Some((lo, hi)) = template_of(model, sheet).and_then(|t| {
        let o = t.corner.at(sheet.width, sheet.height);
        t.bounds().map(|(lo, hi)| (lo + o, hi + o))
    }) {
        return (lo, hi);
    }
    let lo = DVec2::new(sheet.width - MARGIN - TITLE_BLOCK.x, MARGIN);
    (lo, lo + TITLE_BLOCK)
}

/// A template's title block on a sheet, its fields filled in.
pub fn draw_template(
    c: &mut dyn Canvas,
    sheet: &Sheet,
    t: &SheetTemplate,
    values: &BTreeMap<String, String>,
) {
    let o = t.corner.at(sheet.width, sheet.height);
    if t.border {
        // where its margins put it (MU40: the border stretches)
        let (lo, hi) = t.border_box(DVec2::new(sheet.width, sheet.height));
        c.rect(lo, hi, Stroke::solid(0.5, 0.0));
    }
    // a logo is drawn as the shapes it holds, laid over its box
    let laid: Vec<TitleItem> = t
        .items
        .iter()
        .flat_map(|it| match it {
            TitleItem::Logo { .. } => it.logo_shapes(),
            _ => vec![it.clone()],
        })
        .collect();
    for it in &laid {
        match it {
            TitleItem::Line { points, width } => {
                let pts: Vec<DVec2> = points.iter().map(|p| *p + o).collect();
                c.line(&pts, Stroke::solid(*width, 0.0));
            }
            TitleItem::Fill { points, gray } => {
                let pts: Vec<DVec2> = points.iter().map(|p| *p + o).collect();
                c.fill(&pts, *gray);
            }
            TitleItem::Text {
                at,
                size,
                text,
                align,
                angle,
                colour,
                font,
            } => {
                let anchor = match align {
                    TextAlign::Start => Anchor::Start,
                    TextAlign::Middle => Anchor::Middle,
                    TextAlign::End => Anchor::End,
                };
                let text = fill_fields(text, values);
                if colour.is_some() || font.is_some() {
                    // its own colour or face (template sets)
                    let look = crate::annotation::TextLook {
                        size: *size,
                        rgb: *colour,
                        font: font.unwrap_or_default(),
                        ..Default::default()
                    };
                    c.styled_text(*at + o, &text, anchor, *angle, &look);
                } else {
                    c.text_angle(*at + o, *size, &text, anchor, *angle);
                }
            }
            TitleItem::Shape {
                points,
                closed,
                pen,
                colour,
                fill,
                opacity,
            } => {
                let mut pts: Vec<DVec2> = points.iter().map(|p| *p + o).collect();
                // see-through on white paper: the colour mixed toward white
                let wash = |c: [f32; 3]| c.map(|v| 1.0 - (1.0 - v) * *opacity as f32);
                if let (Some(f), true) = (fill, pts.len() >= 3) {
                    c.fill_rgb(&pts, wash(*f));
                }
                if let Some(w) = pen {
                    if *closed && pts.len() >= 2 && pts.first() != pts.last() {
                        pts.push(pts[0]);
                    }
                    c.line_rgb(&pts, Stroke::solid(*w, 0.0), wash(*colour));
                }
            }
            TitleItem::Image {
                lo,
                hi,
                width,
                height,
                rgba,
                opacity,
            } => {
                if let Some(r) = crate::raster::stored_image(*width, *height, rgba) {
                    let r = crate::raster::faded(&r, *opacity);
                    c.image(*lo + o, *hi + o, None, &r);
                }
            }
            TitleItem::Logo { .. } => {}
        }
    }
}

/// Border and title block: the sheet's template, or the built-in block.
pub fn draw_frame(
    c: &mut dyn Canvas,
    model: &Model,
    sheet: &Sheet,
    title: &str,
    scales: &str,
    info: &SheetInfo,
) {
    // the project's information (A7-5): over the template's defaults and
    // the file's name, under what this sheet sets itself
    let project = model
        .project_info()
        .map(|(_, p)| p.clone())
        .unwrap_or_default();
    let named;
    let info = if project.name.trim().is_empty() {
        info
    } else {
        named = SheetInfo {
            project: project.name.trim().to_string(),
            date: info.date.clone(),
            ..Default::default()
        };
        &named
    };
    match template_of(model, sheet) {
        Some(t) => {
            let values = frame_values(model, sheet, Some(t), title, scales, info);
            draw_template(c, sheet, t, &values);
        }
        None => draw_builtin_frame(c, sheet, title, scales, info),
    }
}

/// The built-in border and title block.
pub fn draw_builtin_frame(
    c: &mut dyn Canvas,
    sheet: &Sheet,
    title: &str,
    scales: &str,
    info: &SheetInfo,
) {
    let (w, h) = (sheet.width, sheet.height);
    let lo = DVec2::new(BINDING, MARGIN);
    let hi = DVec2::new(w - MARGIN, h - MARGIN);
    c.rect(lo, hi, Stroke::solid(0.5, 0.0));

    let thin = Stroke::solid(0.18, 0.0);
    let tb_lo = DVec2::new(hi.x - TITLE_BLOCK.x, lo.y);
    let tb_hi = DVec2::new(hi.x, lo.y + TITLE_BLOCK.y);
    c.rect(tb_lo, tb_hi, Stroke::solid(0.35, 0.0));
    // rows from the top: project, sheet title, then number / scale / date
    let row = |k: f64| tb_hi.y - k;
    c.line(
        &[
            DVec2::new(tb_lo.x, row(12.0)),
            DVec2::new(tb_hi.x, row(12.0)),
        ],
        thin,
    );
    c.line(
        &[
            DVec2::new(tb_lo.x, row(27.0)),
            DVec2::new(tb_hi.x, row(27.0)),
        ],
        thin,
    );
    let split = tb_lo.x + 70.0;
    c.line(
        &[DVec2::new(split, tb_lo.y), DVec2::new(split, row(27.0))],
        thin,
    );
    let x = tb_lo.x + 3.0;
    let small = 2.0;
    c.text(DVec2::new(x, row(3.5)), small, "PROJECT", Anchor::Start);
    c.text(DVec2::new(x, row(8.5)), 4.0, &info.project, Anchor::Start);
    c.text(DVec2::new(x, row(15.5)), small, "SHEET", Anchor::Start);
    c.text(DVec2::new(x, row(21.5)), 5.0, title, Anchor::Start);
    c.text(DVec2::new(x, row(30.5)), small, "NUMBER", Anchor::Start);
    c.text(DVec2::new(x, row(38.5)), 8.0, &sheet.number, Anchor::Start);
    let x2 = split + 3.0;
    c.text(DVec2::new(x2, row(30.5)), small, "SCALE", Anchor::Start);
    c.text(DVec2::new(x2, row(35.5)), 3.0, scales, Anchor::Start);
    c.text(DVec2::new(x2, row(40.5)), small, &info.date, Anchor::Start);
    c.text(
        DVec2::new(tb_hi.x - 3.0, row(40.5)),
        small,
        "CreaBIM",
        Anchor::End,
    );
}

/// Records what is drawn on it as title block items, from `origin`.
struct Recorder {
    origin: DVec2,
    items: Vec<TitleItem>,
}

impl Canvas for Recorder {
    fn line(&mut self, pts: &[DVec2], stroke: Stroke) {
        self.items.push(TitleItem::Line {
            points: pts.iter().map(|p| *p - self.origin).collect(),
            width: stroke.width,
        });
    }

    fn fill(&mut self, pts: &[DVec2], gray: f64) {
        self.items.push(TitleItem::Fill {
            points: pts.iter().map(|p| *p - self.origin).collect(),
            gray,
        });
    }

    fn text_angle(&mut self, at: DVec2, size: f64, text: &str, anchor: Anchor, angle: f64) {
        self.items.push(TitleItem::Text {
            at: at - self.origin,
            size,
            text: text.to_string(),
            align: match anchor {
                Anchor::Start => TextAlign::Start,
                Anchor::Middle => TextAlign::Middle,
                Anchor::End => TextAlign::End,
            },
            angle,
            colour: None,
            font: None,
        });
    }

    fn image(
        &mut self,
        _lo: DVec2,
        _hi: DVec2,
        _clip: Option<(DVec2, DVec2)>,
        _raster: &std::sync::Arc<crate::raster::Raster>,
    ) {
    }
}

/// The built-in title block as a template to change: its lines and text,
/// with fields where it writes the sheet's own values.
pub fn builtin_template() -> SheetTemplate {
    let sheet = Sheet::a("{number}", "A3");
    let info = SheetInfo {
        project: "{project}".into(),
        date: "{date}".into(),
        ..Default::default()
    };
    let mut rec = Recorder {
        origin: DVec2::new(sheet.width, 0.0),
        items: vec![],
    };
    draw_builtin_frame(&mut rec, &sheet, "{sheet}", "{scale}", &info);
    // the border is the template's own switch
    rec.items.remove(0);
    SheetTemplate {
        corner: creabim_core::titleblock::Corner::BottomRight,
        items: rec.items,
        border: true,
        fields: vec![],
        paper: None,
        guides: vec![],
        margins: None,
        stock: Default::default(),
    }
}

/// The scale text for a title block: one scale, or "As indicated".
pub fn scale_text(model: &Model, views: &[PlacedView]) -> String {
    let mut scales: Vec<i64> = views
        .iter()
        .filter_map(|v| view_of(model, v.view))
        .filter(|v| !matches!(v.kind, creabim_core::ViewKind::Schedule { .. }))
        .map(|v| v.scale.round() as i64)
        .collect();
    scales.sort();
    scales.dedup();
    match scales.as_slice() {
        [] => "-".into(),
        [s] => creabim_core::standards::scale_label(model, *s as f64),
        _ => "As indicated".into(),
    }
}

/// A view's number bubble, title and scale under its drawing.
pub fn draw_view_title(c: &mut dyn Canvas, v: &PlacedView) {
    // a schedule carries its own title
    if v.map.scale == 1000.0 {
        return;
    }
    let t = &v.title_look;
    if t.hidden {
        return;
    }
    let at = v.title_at();
    let k = t.size / 3.5;
    let thin = Stroke::solid(0.18, 0.0);
    let x0 = if t.show_number {
        c.circle(at + DVec2::new(3.5 * k, 0.0), 3.5 * k, thin);
        c.text(
            at + DVec2::new(3.5 * k, 0.0),
            3.0 * k,
            &v.number.to_string(),
            Anchor::Middle,
        );
        9.0 * k
    } else {
        0.0
    };
    c.styled_text(
        at + DVec2::new(x0, 1.2 * k),
        &v.title,
        Anchor::Start,
        0.0,
        &v.title_text,
    );
    let width = paper::text_width(&v.title, t.size).max(30.0 * k);
    c.line(
        &[at + DVec2::new(x0, -k), at + DVec2::new(x0 + width, -k)],
        Stroke::solid(0.35, 0.0),
    );
    if t.show_scale {
        c.text(
            at + DVec2::new(x0, -3.5 * k),
            2.5 * k,
            &v.scale_text,
            Anchor::Start,
        );
    }
}

/// Draw a whole sheet: frame, every view and its title.
pub fn draw_sheet(
    c: &mut dyn Canvas,
    model: &Model,
    sheet: ElementId,
    views: &[PlacedView],
    info: &SheetInfo,
) {
    let Some(s) = sheet_of(model, sheet) else {
        return;
    };
    let title = model.get(sheet).map(|e| e.name.as_str()).unwrap_or("");
    // the master's paper (pens stage 7): white and plain draws as always
    let stock = crate::sheetpaper::sheet_paper(model, s, info.print_white);
    if stock.is_plain() && !info.print_white {
        draw_frame(c, model, s, title, &scale_text(model, views), info);
        for v in views {
            paper::draw_looks(c, &v.drawing, &v.map, &v.looks);
            draw_view_title(c, v);
        }
        draw_sheet_notes(c, model, sheet);
        return;
    }
    crate::sheetpaper::draw_paper(c, DVec2::new(s.width, s.height), &stock);
    let mut on = crate::sheetpaper::OnPaper {
        inner: c,
        paper: stock,
    };
    draw_frame(&mut on, model, s, title, &scale_text(model, views), info);
    for v in views {
        paper::draw_looks(
            &mut on,
            &v.drawing,
            &v.map,
            &on_white(&v.looks, info.print_white),
        );
        draw_view_title(&mut on, v);
    }
    draw_sheet_notes(&mut on, model, sheet);
}

/// A view's looks for printing on white: its palette's paper turned white
/// (its ink and colours kept); dark paper keeps its own.
pub fn on_white(
    looks: &std::sync::Arc<crate::looks::Looks>,
    print_white: bool,
) -> std::sync::Arc<crate::looks::Looks> {
    match looks.palette {
        Some(r) if print_white && !r.dark && r.paper != [1.0; 3] => {
            let mut l = (**looks).clone();
            if let Some(p) = &mut l.palette {
                p.paper = [1.0; 3];
            }
            std::sync::Arc::new(l)
        }
        _ => looks.clone(),
    }
}

/// Paper millimetres as drawing units, for what is drawn on a sheet itself.
pub const PAPER: PaperMap = PaperMap {
    model: DVec2::ZERO,
    paper: DVec2::ZERO,
    scale: 1000.0,
};

/// The notes written on the sheet itself, outside any view.
pub fn draw_sheet_notes(c: &mut dyn Canvas, model: &Model, sheet: ElementId) {
    let notes = crate::PlanView {
        items: crate::annotate::sheet_notes(model, sheet),
    };
    paper::draw(c, &notes, &PAPER);
    draw_paper_marks(c, model, sheet);
}

/// Where a sheet's title block is, as drawn (its template's or the built-in
/// one): the box round what its frame draws in the lower half of the
/// sheet, inside the border (MU22). Paper marks are placed clear of it.
pub fn title_block_box(model: &Model, sheet: ElementId) -> Option<(DVec2, DVec2)> {
    struct Points(Vec<DVec2>);
    impl Canvas for Points {
        fn line(&mut self, pts: &[DVec2], _: Stroke) {
            self.0.extend_from_slice(pts);
        }
        fn fill(&mut self, pts: &[DVec2], _: f64) {
            self.0.extend_from_slice(pts);
        }
        fn text_angle(&mut self, at: DVec2, size: f64, _: &str, _: Anchor, _: f64) {
            self.0.push(at);
            self.0.push(at + DVec2::new(0.0, size));
        }
        fn image(
            &mut self,
            lo: DVec2,
            hi: DVec2,
            _: Option<(DVec2, DVec2)>,
            _: &std::sync::Arc<crate::raster::Raster>,
        ) {
            self.0.push(lo);
            self.0.push(hi);
        }
    }
    let s = sheet_of(model, sheet)?;
    let mut c = Points(vec![]);
    let title = model.get(sheet).map(|e| e.name.as_str()).unwrap_or("");
    draw_frame(&mut c, model, s, title, "1:100", &SheetInfo::default());
    let (w, h) = (s.width, s.height);
    let on_border = |p: &DVec2| {
        (p.x - BINDING).abs() < 0.05
            || (p.x - (w - MARGIN)).abs() < 0.05
            || (p.y - MARGIN).abs() < 0.05
            || (p.y - (h - MARGIN)).abs() < 0.05
            || p.x <= 0.05
            || p.y <= 0.05
            || p.x >= w - 0.05
            || p.y >= h - 0.05
    };
    let inside: Vec<DVec2> =
        c.0.into_iter()
            .filter(|p| p.y < h * 0.5 && !on_border(p))
            .collect();
    let first = *inside.first()?;
    let (lo, hi) = inside
        .iter()
        .fold((first, first), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
    Some((lo, hi))
}

/// A drawing's scale on paper, if it has one: plans, sections,
/// elevations and long sections (not schedules or 3D views).
pub fn drawing_scale(model: &Model, viewport: ElementId) -> Option<f64> {
    let ElementKind::Viewport(vp) = &model.get(viewport)?.kind else {
        return None;
    };
    let view = creabim_core::styles::resolved(model, view_of(model, vp.view)?);
    match view.kind {
        creabim_core::ViewKind::Schedule { .. } | creabim_core::ViewKind::Axonometric { .. } => {
            None
        }
        _ => Some(view.paper_scale()),
    }
}

/// Whether a viewport shows a plan (for north arrows).
pub fn is_plan(model: &Model, viewport: ElementId) -> bool {
    let Some(ElementKind::Viewport(vp)) = model.get(viewport).map(|e| &e.kind) else {
        return false;
    };
    matches!(
        view_of(model, vp.view).map(|v| &v.kind),
        Some(creabim_core::ViewKind::Plan { .. })
    )
}

/// The viewports on `sheet`, in model order, with their view's name.
pub fn viewports_on(model: &Model, sheet: ElementId) -> Vec<(ElementId, &Viewport, String)> {
    model
        .iter()
        .filter_map(|e| match &e.kind {
            ElementKind::Viewport(vp) if vp.sheet == sheet => Some((
                e.id,
                vp,
                model
                    .get(vp.view)
                    .map(|v| v.name.clone())
                    .unwrap_or_default(),
            )),
            _ => None,
        })
        .collect()
}

/// The viewport whose middle is nearest `at` on `sheet` among those that
/// `ok` accepts: the drawing a new scale bar belongs to.
pub fn nearest_viewport(
    model: &Model,
    sheet: ElementId,
    at: DVec2,
    ok: impl Fn(ElementId) -> bool,
) -> Option<ElementId> {
    viewports_on(model, sheet)
        .into_iter()
        .filter(|(id, ..)| ok(*id))
        .min_by(|a, b| a.1.center.distance(at).total_cmp(&b.1.center.distance(at)))
        .map(|(id, ..)| id)
}

/// Where a copied mark goes for a drawing (Mockup_Copy_To_Sheets_REV00):
/// a scale bar under the drawing's title, a north arrow by the drawing's
/// upper right corner; kept on the paper. None when the drawing is empty.
pub fn mark_spot(model: &Model, viewport: ElementId, north: bool, size: f64) -> Option<DVec2> {
    let ElementKind::Viewport(vp) = &model.get(viewport)?.kind else {
        return None;
    };
    let view = creabim_core::styles::resolved(model, view_of(model, vp.view)?);
    let mut d = crate::section::view_drawing(model, vp.view)?;
    if let Some([lo, hi]) = vp.crop {
        d = crate::crop::crop(&d, lo, hi);
    }
    let map = PaperMap {
        model: vp.target,
        paper: vp.center,
        scale: view.paper_scale(),
    };
    let (a, b) = d.bounds()?;
    let (pa, pb) = (map.to_paper(a), map.to_paper(b));
    let (lo, hi) = (pa.min(pb), pa.max(pb));
    let at = if north {
        DVec2::new(hi.x - size * 0.6, hi.y - size * 0.6)
    } else {
        // the title sits 7 mm under the drawing; the bar under the title
        DVec2::new(lo.x, lo.y - 7.0 - title_size(model, &view) - 7.0) + view.sheet_title.offset
    };
    let sheet = sheet_of(model, vp.sheet)?;
    Some(DVec2::new(
        at.x.clamp(
            MARGIN + 2.0,
            (sheet.width - MARGIN - 20.0).max(MARGIN + 2.0),
        ),
        at.y.clamp(
            MARGIN + 6.0,
            (sheet.height - MARGIN - 6.0).max(MARGIN + 6.0),
        ),
    ))
}

/// A mark as drawn: a scale bar that measures a drawing takes that
/// drawing's scale, now.
pub fn measured(model: &Model, m: &creabim_core::PaperMark) -> creabim_core::PaperMark {
    match (&m.kind, m.measures.and_then(|v| drawing_scale(model, v))) {
        (creabim_core::PaperMarkKind::ScaleBar { .. }, Some(s)) => m.at_scale(s),
        _ => m.clone(),
    }
}

/// The north arrows and scale bars on a sheet (MU22).
pub fn draw_paper_marks(c: &mut dyn Canvas, model: &Model, sheet: ElementId) {
    let north = model.project_info().map_or(0.0, |(_, p)| p.true_north);
    // the sheet's revisions, for its table
    let mut revs: Vec<(String, String, String)> = model
        .iter()
        .filter_map(|e| match &e.kind {
            ElementKind::PaperMark(m) if m.sheet == sheet => match &m.kind {
                creabim_core::PaperMarkKind::Cloud {
                    rev, date, what, ..
                } => Some((rev.clone(), date.clone(), what.clone())),
                _ => None,
            },
            _ => None,
        })
        .collect();
    revs.sort();
    revs.dedup_by(|a, b| a.0 == b.0);
    let pack = creabim_core::styles::active_pack(model);
    let picks = &pack.symbols;
    let standard = creabim_core::standards::of(model);
    for e in model.iter() {
        if let ElementKind::PaperMark(m) = &e.kind {
            if m.sheet == sheet {
                if matches!(m.kind, creabim_core::PaperMarkKind::RevisionTable) {
                    draw_revision_table(c, m, &revs);
                } else {
                    draw_paper_mark(c, &measured(model, m), north, picks, &standard);
                }
            }
        }
    }
}

/// The revisions table: REV, DATE, DESCRIPTION, a row per revision.
fn draw_revision_table(
    c: &mut dyn Canvas,
    m: &creabim_core::PaperMark,
    revs: &[(String, String, String)],
) {
    let t = m.size;
    let row = t * 2.4;
    let cols = [0.0, 14.0, 40.0, 110.0];
    let thin = Stroke::solid(0.18, 0.0);
    let rows = revs.len() + 1;
    let top = m.at.y + row * rows as f64;
    for i in 0..=rows {
        let y = m.at.y + row * i as f64;
        c.line(
            &[DVec2::new(m.at.x, y), DVec2::new(m.at.x + cols[3], y)],
            thin,
        );
    }
    for x in cols {
        c.line(
            &[DVec2::new(m.at.x + x, m.at.y), DVec2::new(m.at.x + x, top)],
            thin,
        );
    }
    let cell = |c: &mut dyn Canvas, r: usize, col: usize, s: &str| {
        let y = top - row * (r as f64 + 1.0) + row * 0.32;
        c.text(DVec2::new(m.at.x + cols[col] + 1.5, y), t, s, Anchor::Start);
    };
    cell(c, 0, 0, "REV");
    cell(c, 0, 1, "DATE");
    cell(c, 0, 2, "DESCRIPTION");
    for (i, (rev, date, what)) in revs.iter().enumerate() {
        cell(c, i + 1, 0, rev);
        cell(c, i + 1, 1, date);
        cell(c, i + 1, 2, what);
    }
}

/// A symbol's linework on a sheet, in sheet millimetres.
fn paper_symbol(c: &mut dyn Canvas, sh: &crate::symbol::SymbolShapes, rgb: Option<[f32; 3]>) {
    for f in &sh.fills {
        match rgb {
            Some(rgb) => c.fill_rgb(f, rgb),
            None => c.fill(f, 0.0),
        }
    }
    for (l, w) in &sh.lines {
        match rgb {
            Some(rgb) => c.line_rgb(l, Stroke::solid(*w, 0.0), rgb),
            None => c.line(l, Stroke::solid(*w, 0.0)),
        }
    }
}

/// One paper mark, in sheet millimetres; `picks` are the style pack's
/// symbols (Mockup_Symbols_REV00): north arrows, revision marks and scale
/// bars draw with theirs.
pub fn draw_paper_mark(
    c: &mut dyn Canvas,
    m: &creabim_core::PaperMark,
    true_north: f64,
    picks: &creabim_core::symbols::SymbolPicks,
    standard: &creabim_core::standards::DrawingStandard,
) {
    use crate::symbol::{shapes_at, Origin};
    use creabim_core::PaperMarkKind;
    let pick = |k: &str| picks.get(k).and_then(|id| creabim_core::symbols::style(id));
    match &m.kind {
        PaperMarkKind::Picture {
            width,
            height,
            rgba,
            opacity,
            ..
        } => {
            let (lo, hi) = m.bounds();
            if let Some(r) = crate::raster::stored_image(*width, *height, rgba) {
                c.image(lo, hi, None, &crate::raster::faded(&r, *opacity));
            }
            return;
        }
        PaperMarkKind::Vector { shapes, .. } => {
            // drawn as a master's logo is, from the sheet's lower left
            let (lo, hi) = m.bounds();
            let t = SheetTemplate {
                corner: creabim_core::titleblock::Corner::BottomLeft,
                items: vec![TitleItem::Logo {
                    lo,
                    hi,
                    shapes: shapes.clone(),
                }],
                border: false,
                fields: vec![],
                paper: None,
                guides: vec![],
                margins: None,
                stock: Default::default(),
            };
            draw_template(c, &Sheet::a("", "A3"), &t, &BTreeMap::new());
            return;
        }
        _ => {}
    }
    // a mark's own style, else the project's pick; "creabim" is ours
    let own = |kind: &str, style: &Option<String>| match style.as_deref() {
        Some("creabim") => None,
        Some(id) => creabim_core::symbols::style(id).or_else(|| pick(kind)),
        None => pick(kind),
    };
    let north = match &m.kind {
        PaperMarkKind::NorthArrow { style, .. } => own("north", style),
        _ => None,
    };
    let scales = match &m.kind {
        PaperMarkKind::ScaleBar { style, .. } => own("scales", style),
        _ => None,
    };
    match &m.kind {
        PaperMarkKind::NorthArrow { .. } if north.is_some() => {
            // the ring of 10 steps fills the usual circle
            let sym = north.unwrap();
            let unit = 0.42 * m.size / 10.0;
            let angle = m.north_angle(true_north).to_radians();
            let sh = shapes_at(sym, m.at, DVec2::splat(unit), angle, Origin::Centre);
            paper_symbol(c, &sh, None);
            if sym.text.is_some() {
                c.text(sh.text_at, 3.5, "N", Anchor::Middle);
            }
        }
        PaperMarkKind::NorthArrow { .. } => {
            let s = m.size;
            let turn = DVec2::from_angle(m.north_angle(true_north).to_radians());
            let at = |p: DVec2| m.at + turn.rotate(p);
            // the arrow inside its circle, the tip just touching it (MU29)
            let tip = at(DVec2::new(0.0, 0.40 * s));
            let notch = at(DVec2::new(0.0, -0.14 * s));
            let left = at(DVec2::new(-0.21 * s, -0.31 * s));
            let right = at(DVec2::new(0.21 * s, -0.31 * s));
            c.circle(m.at, 0.42 * s, Stroke::solid(0.25, 0.0));
            // half black, half white, as drawn by hand
            c.fill(&[tip, notch, left], 0.0);
            c.polygon(&[tip, right, notch, left], Stroke::solid(0.3, 0.0));
            c.text(
                at(DVec2::new(0.0, 0.42 * s + 3.0)),
                3.5,
                "N",
                Anchor::Middle,
            );
        }
        PaperMarkKind::Cloud { rev, arc, .. } => {
            // scallops of about `arc` mm round the outline, bulging out
            let corners = m.cloud_outline();
            let stroke = Stroke::solid(0.35, 0.0);
            let ink = |c: &mut dyn Canvas, pts: &[DVec2]| match m.color {
                Some(rgb) => c.line_rgb(pts, stroke, rgb),
                None => c.line(pts, stroke),
            };
            let k = corners.len();
            for q in 0..k {
                let (a, b) = (corners[q], corners[(q + 1) % k]);
                let len = a.distance(b);
                if len < 1e-6 {
                    continue;
                }
                let n = (len / arc.max(1.0)).ceil().max(1.0) as usize;
                let dir = (b - a) / len;
                let out = DVec2::new(dir.y, -dir.x);
                for i in 0..n {
                    let (s, e) = (
                        a + (b - a) * (i as f64 / n as f64),
                        a + (b - a) * ((i + 1) as f64 / n as f64),
                    );
                    let mid = (s + e) / 2.0;
                    let r = s.distance(e) / 2.0;
                    let pts: Vec<DVec2> = (0..=8)
                        .map(|j| {
                            let t = std::f64::consts::PI * j as f64 / 8.0;
                            mid - dir * r * t.cos() + out * r * t.sin()
                        })
                        .collect();
                    ink(c, &pts);
                }
            }
            let hi = corners
                .iter()
                .fold(DVec2::splat(f64::MIN), |a, p| a.max(*p));
            // its revision in a triangle at the top right, or in the
            // revision symbol picked
            let tip = hi + DVec2::new(4.0, 4.0);
            let s = m.size * 1.6;
            if let Some(sym) = pick("revisions") {
                let unit = s / 11.0;
                let sh = shapes_at(sym, tip, DVec2::splat(unit), 0.0, Origin::Centre);
                paper_symbol(c, &sh, m.color);
                c.text(sh.text_at, m.size, rev, Anchor::Middle);
                return;
            }
            let tri = [
                tip + DVec2::new(-s, -s * 0.6),
                tip + DVec2::new(s, -s * 0.6),
                tip + DVec2::new(0.0, s * 1.1),
            ];
            c.polygon(&tri, Stroke::solid(0.3, 0.0));
            c.text(
                tip + DVec2::new(0.0, -s * 0.25),
                m.size,
                rev,
                Anchor::Middle,
            );
        }
        PaperMarkKind::RevisionTable
        | PaperMarkKind::Picture { .. }
        | PaperMarkKind::Vector { .. } => {}
        PaperMarkKind::ScaleBar {
            scale,
            length,
            parts,
            feet,
            ..
        } => {
            // its numbers in feet or metres; its scale as the project's
            // standard writes it (1/8" = 1'-0" on a US sheet)
            let (shown, unit) = if *feet {
                (length / 0.3048, "'")
            } else {
                (*length, " m")
            };
            let caption = standard.scale_label(*scale);
            let parts = (*parts).max(1) as usize;
            let w = length * 1000.0 / scale.max(1.0);
            let h = m.size;
            let seg = w / parts as f64;
            let o = m.at;
            if let Some(sym) = scales {
                // the symbol stretched to the bar's true length: its
                // left end on the mark's corner, the scale in its slot,
                // the lengths at its ends
                let (sw, sh_) = crate::symbol::span(sym);
                let unit = DVec2::new(w / sw.max(1.0), h / sh_.max(1.0));
                let lo = shapes_at(sym, DVec2::ZERO, DVec2::ONE, 0.0, Origin::Centre)
                    .bounds()
                    .map_or(DVec2::ZERO, |b| b.0);
                let at = o - lo * unit;
                let sh = shapes_at(sym, at, unit, 0.0, Origin::Centre);
                paper_symbol(c, &sh, None);
                let top = sh.bounds().map_or(o.y + h, |b| b.1.y);
                c.text(DVec2::new(o.x, top + 1.2), 2.2, "0", Anchor::Middle);
                let end = if (shown - shown.round()).abs() < 1e-6 {
                    format!("{shown:.0}{unit}")
                } else {
                    format!("{shown:.1}{unit}")
                };
                c.text(DVec2::new(o.x + w, top + 1.2), 2.2, &end, Anchor::Middle);
                c.text(DVec2::new(o.x, o.y - 3.6), 2.2, &caption, Anchor::Start);
                return;
            }
            for i in 0..parts {
                let x0 = o.x + seg * i as f64;
                let box_ = [
                    DVec2::new(x0, o.y),
                    DVec2::new(x0 + seg, o.y),
                    DVec2::new(x0 + seg, o.y + h),
                    DVec2::new(x0, o.y + h),
                ];
                if i % 2 == 0 {
                    c.fill(&box_, 0.0);
                }
                c.polygon(&box_, Stroke::solid(0.2, 0.0));
            }
            let step = shown / parts as f64;
            let num = |v: f64| {
                if (v - v.round()).abs() < 1e-6 {
                    format!("{v:.0}")
                } else {
                    format!("{v:.1}")
                }
            };
            for i in 0..=parts {
                let x = o.x + seg * i as f64;
                let t = if i == parts {
                    format!("{}{unit}", num(step * i as f64))
                } else {
                    num(step * i as f64)
                };
                c.text(DVec2::new(x, o.y + h + 1.2), 2.2, &t, Anchor::Middle);
            }
            c.text(DVec2::new(o.x, o.y - 3.6), 2.2, &caption, Anchor::Start);
        }
    }
}

pub fn sheet_svg(model: &Model, sheet: ElementId, info: &SheetInfo) -> Option<String> {
    let s = sheet_of(model, sheet)?;
    let views = placed_views(model, sheet, &mut |v| view_drawing(model, v));
    let mut svg = SvgCanvas::new(DVec2::new(s.width, s.height));
    draw_sheet(&mut svg, model, sheet, &views, info);
    Some(svg.finish())
}

/// All sheets as one PDF, a page per sheet in sheet-number order.
pub fn sheets_pdf(model: &Model, info: &SheetInfo) -> Vec<u8> {
    let all: Vec<ElementId> = sheets(model).into_iter().map(|(id, _)| id).collect();
    sheets_pdf_of(model, info, &all)
}

/// The sheets `ids` as one PDF, a page each in sheet-number order (MU22).
pub fn sheets_pdf_of(model: &Model, info: &SheetInfo, ids: &[ElementId]) -> Vec<u8> {
    sheets_pdf_of_with(model, info, ids, &creabim_core::progress::quiet)
}

/// [`sheets_pdf_of`], telling `tick` how far it has got, in sheets.
pub fn sheets_pdf_of_with(
    model: &Model,
    info: &SheetInfo,
    ids: &[ElementId],
    tick: creabim_core::progress::Tick,
) -> Vec<u8> {
    let mut pdf = PdfWriter::new();
    let total = ids.len();
    for (i, (id, s)) in sheets(model)
        .into_iter()
        .filter(|(id, _)| ids.contains(id))
        .enumerate()
    {
        let name = model.get(id).map_or("", |e| e.name.as_str());
        tick(i, total, &format!("Sheet {} {name}", s.number));
        let views = placed_views(model, id, &mut |v| view_drawing(model, v));
        let mut page = pdf.page(DVec2::new(s.width, s.height));
        draw_sheet(&mut page, model, id, &views, info);
    }
    pdf.finish()
}

#[cfg(test)]
mod arrange_tests {
    use super::*;

    fn schedule_sheet(m: &Model) -> ElementId {
        sheets(m)
            .into_iter()
            .find(|(_, s)| s.number == "A-602")
            .map(|(id, _)| id)
            .expect("Ridgeline has its schedule sheet")
    }

    fn overlap(a: (DVec2, DVec2), b: (DVec2, DVec2)) -> bool {
        a.0.x < b.1.x && b.0.x < a.1.x && a.0.y < b.1.y && b.0.y < a.1.y
    }

    #[test]
    fn a_schedule_sheet_arranges_its_tables_and_keeps_one_moved_by_hand() {
        let mut m = creabim_core::ridgeline::ridgeline_house();
        let sheet = schedule_sheet(&m);
        assert!(arranges(&m, sheet), "a sheet of schedules arranges them");
        let placed = placed_views(&m, sheet, &mut |v| view_drawing(&m, v));
        assert!(placed.len() >= 3);
        // no two tables on each other, all on the paper
        for (i, a) in placed.iter().enumerate() {
            for b in &placed[i + 1..] {
                assert!(!overlap(a.bounds, b.bounds), "{} on {}", a.title, b.title);
            }
        }
        // one moved by hand stays where it was put, the rest keep clear of it
        let (vid, before) = m
            .iter()
            .find_map(|e| match &e.kind {
                ElementKind::Viewport(vp) if vp.sheet == sheet => Some((e.id, e.clone())),
                _ => None,
            })
            .unwrap();
        let mut after = before.clone();
        if let ElementKind::Viewport(vp) = &mut after.kind {
            vp.center = DVec2::new(400.0, 300.0);
            vp.pinned = true;
        }
        creabim_core::Transaction::new("pin")
            .replace(before, after)
            .apply(&mut m)
            .unwrap();
        let placed = placed_views(&m, sheet, &mut |v| view_drawing(&m, v));
        let mine = placed.iter().find(|p| p.viewport == vid).unwrap();
        assert_eq!(
            mine.map.paper,
            DVec2::new(400.0, 300.0),
            "it stays where it was put"
        );
        for p in placed.iter().filter(|p| p.viewport != vid) {
            assert!(
                !overlap(p.bounds, mine.bounds),
                "{} runs over the one moved by hand",
                p.title
            );
        }
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn a_us_set_binds_general_and_civil_before_the_architecture() {
        let mut numbers = vec![
            "A-101", "C-101", "A-902", "G-001", "S-201", "A-100", "M-101",
        ];
        numbers.sort_by_key(|n| super::set_order(n));
        assert_eq!(
            numbers,
            ["G-001", "C-101", "S-201", "A-100", "A-101", "A-902", "M-101"]
        );
        // an ISO-style set keeps its plain order
        let mut iso = vec!["A901", "A000", "A101", "A501"];
        iso.sort_by_key(|n| super::set_order(n));
        assert_eq!(iso, ["A000", "A101", "A501", "A901"]);
    }
    use super::*;
    use creabim_core::sample::sample_house;

    #[test]
    fn project_information_fills_the_title_block_under_the_sheets_own() {
        let mut m = sample_house();
        let info = creabim_core::ProjectInfo {
            name: "Maple Lane House".into(),
            client: "A. Client".into(),
            status: "Tender".into(),
            ..Default::default()
        };
        let mut doc = creabim_core::Document::from_model(m.clone());
        doc.commit(creabim_core::set_project(doc.model(), info))
            .unwrap();
        m = doc.model().clone();
        let t = SheetTemplate {
            corner: Default::default(),
            items: vec![],
            border: true,
            fields: vec![
                ("client".into(), "Client".into()),
                ("status".into(), "Draft".into()),
            ],
            paper: None,
            guides: vec![],
            margins: None,
            stock: Default::default(),
        };
        let (_, s) = sheets(&m).into_iter().next().unwrap();
        let mut s = s.clone();
        s.fields.insert("status".into(), "Construction".into());
        let base = SheetInfo {
            project: "Starter house".into(),
            date: String::new(),
            ..Default::default()
        };
        let v = frame_values(&m, &s, Some(&t), "Plan", "1:100", &base);
        assert_eq!(
            v["client"], "A. Client",
            "the project's client over the template's default"
        );
        assert_eq!(v["status"], "Construction", "the sheet's own value wins");
        assert_eq!(v["job"], "Maple Lane House");
    }

    #[test]
    fn sample_sheets_place_their_views_inside_the_border() {
        let m = sample_house();
        let all = sheets(&m);
        let numbers: Vec<&str> = all.iter().map(|(_, s)| s.number.as_str()).collect();
        assert_eq!(numbers, ["A001", "A101", "A201", "A601"]);
        for (id, s) in all {
            let views = placed_views(&m, id, &mut |v| view_drawing(&m, v));
            assert!(!views.is_empty());
            for v in &views {
                let (lo, hi) = v.bounds;
                assert!(
                    lo.x >= BINDING
                        && lo.y >= MARGIN
                        && hi.x <= s.width - MARGIN
                        && hi.y <= s.height - MARGIN,
                    "{} on {} spills off the sheet: {lo} {hi}",
                    v.title,
                    s.number
                );
            }
        }
    }

    #[test]
    fn free_spot_avoids_views_already_placed() {
        let mut doc = creabim_core::Document::from_model(sample_house());
        let plan = |name: &str, m: &Model| m.iter().find(|e| e.name == name).unwrap().id;
        let (g, f) = (
            plan("Ground Floor plan", doc.model()),
            plan("First Floor plan", doc.model()),
        );
        let sheet = creabim_core::Element::new("Empty", ElementKind::Sheet(Sheet::a("X1", "A1")));
        let sid = sheet.id;
        let vp = new_viewport(doc.model(), sid, g, DVec2::new(200.0, 400.0));
        doc.commit(
            creabim_core::Transaction::new("t")
                .insert(sheet)
                .insert(creabim_core::Element::new("vp", ElementKind::Viewport(vp))),
        )
        .unwrap();
        let at = free_spot(doc.model(), sid, f);
        let placed = &placed_views(doc.model(), sid, &mut |v| view_drawing(doc.model(), v))[0];
        let (lo, hi) = placed.bounds;
        // the new plan's own half width keeps it clear of the first
        let half = 90.0;
        assert!(
            at.x - half > hi.x || at.x + half < lo.x || at.y + 70.0 < lo.y || at.y - 70.0 > hi.y,
            "{at} overlaps the plan at {lo} {hi}"
        );
    }

    #[test]
    fn styled_annotations_reach_svg_and_pdf() {
        use creabim_core::{
            Dimension, DimensionKind, Element, Font, HostPoint, Note, TextStyle, Transaction,
        };
        let mut doc = creabim_core::Document::from_model(sample_house());
        let m = doc.model();
        let (sheet, _) = sheets(m)[1];
        let plan = m.iter().find(|e| e.name == "Ground Floor plan").unwrap().id;
        let style = Element::new(
            "Callouts",
            ElementKind::TextStyle(TextStyle {
                font: Font::Serif,
                size: 3.5,
                color: Some([0.8, 0.0, 0.0]),
                bold: true,
                italic: false,
                ..TextStyle::default()
            }),
        );
        let mut on_sheet = Note::new(sheet, DVec2::new(60.0, 60.0), "General note\nSecond line");
        on_sheet.style = Some(style.id);
        on_sheet.leader = Some(HostPoint::free(DVec2::new(40.0, 40.0)));
        let free = |x: f64, y: f64| HostPoint::free(DVec2::new(x, y));
        let angle = Dimension::about(
            DimensionKind::Angular,
            plan,
            free(0.0, 0.0),
            free(3.0, 0.0),
            free(0.0, 3.0),
            1.5,
        );
        let tx = Transaction::new("Annotate")
            .insert(Element::new("Note", ElementKind::Note(on_sheet)))
            .insert(Element::new("Angle", ElementKind::Dimension(angle)))
            .insert(style);
        doc.commit(tx).unwrap();
        let m = doc.model();
        let info = SheetInfo::default();
        let svg = sheet_svg(m, sheet, &info).unwrap();
        assert!(svg.contains(">General note</text>") && svg.contains(">Second line</text>"));
        assert!(svg.contains("font-weight=\"bold\"") && svg.contains("fill=\"#cc0000\""));
        assert!(svg.contains("Times"));
        // the angle in the plan, which sits on this sheet
        assert!(svg.contains(">90°</text>"), "angle missing");
        let pdf = crate::paper::pdf_readable(&sheets_pdf(m, &info));
        assert!(pdf.contains("/BaseFont /Times-Bold"));
        assert!(pdf.contains("0.800 0.000 0.000 rg BT /F6"));
        assert!(pdf.contains("(General note) Tj"));
    }

    #[test]
    fn pdf_has_a_page_per_sheet() {
        let m = sample_house();
        let pdf = sheets_pdf(
            &m,
            &SheetInfo {
                project: "Sample house".into(),
                date: "2026-10-10".into(),
                ..Default::default()
            },
        );
        let s = crate::paper::pdf_readable(&pdf);
        assert!(s.contains("/Count 4"));
        assert!(
            s.contains("/Subtype /Image"),
            "shaded views are embedded as images"
        );
        assert!(s.contains("(Floor Plans) Tj"));
        assert!(s.contains("(Ground Floor plan) Tj"));
    }

    #[test]
    fn cropped_viewports_show_only_their_crop() {
        let m = sample_house();
        let (sheet, _) = sheets(&m)
            .into_iter()
            .find(|(_, s)| s.number == "A601")
            .unwrap();
        let views = placed_views(&m, sheet, &mut |v| view_drawing(&m, v));
        let entrance = views.iter().find(|v| v.title == "Entrance plan").unwrap();
        let (lo, hi) = entrance.drawing.bounds().unwrap();
        assert!(
            lo.x >= -0.8 - 1e-9 && hi.x <= 5.0 + 1e-9 && lo.y >= -2.8 - 1e-9 && hi.y <= 2.0 + 1e-9
        );
        // the uncropped plan is much bigger
        let full = view_drawing(&m, entrance.view).unwrap().bounds().unwrap();
        assert!(full.1.x > 9.0);
    }

    #[test]
    fn viewports_follow_model_edits() {
        let mut doc = creabim_core::Document::from_model(sample_house());
        let (sheet, _) = sheets(doc.model())[1];
        let before = placed_views(doc.model(), sheet, &mut |v| view_drawing(doc.model(), v));
        let wall = doc
            .model()
            .iter()
            .find(|e| e.name == "GF Wall 1")
            .unwrap()
            .id;
        doc.edit(wall, "Thicken", |e| {
            if let ElementKind::Wall(w) = &mut e.kind {
                w.thickness = 0.5;
            }
        })
        .unwrap();
        let after = placed_views(doc.model(), sheet, &mut |v| view_drawing(doc.model(), v));
        assert_ne!(before[0].drawing, after[0].drawing);
        // the viewport stays pinned: same mapping
        assert_eq!(before[0].map, after[0].map);
    }

    #[test]
    fn the_built_in_block_as_a_template_draws_the_same() {
        let m = sample_house();
        let (id, s) = sheets(&m)[0];
        let views = placed_views(&m, id, &mut |v| view_drawing(&m, v));
        let info = SheetInfo {
            project: "House".into(),
            date: "2026-10-21".into(),
            ..Default::default()
        };
        let title = m.get(id).unwrap().name.clone();
        let scales = scale_text(&m, &views);
        let mut a = SvgCanvas::new(DVec2::new(s.width, s.height));
        draw_builtin_frame(&mut a, s, &title, &scales, &info);
        let t = builtin_template();
        assert!(t.used_fields().contains(&"project".to_string()));
        let values = field_values(s, Some(&t), &title, &scales, &info);
        let mut b = SvgCanvas::new(DVec2::new(s.width, s.height));
        draw_template(&mut b, s, &t, &values);
        assert_eq!(a.finish(), b.finish());
    }

    #[test]
    fn a_template_title_block_fills_its_fields_on_every_sheet() {
        use creabim_core::titleblock::{Corner, SheetTemplate, TextAlign, TitleItem};
        use creabim_core::{Element, Transaction};
        let mut doc = creabim_core::Document::from_model(sample_house());
        let t = SheetTemplate {
            corner: Corner::BottomRight,
            items: vec![
                TitleItem::Line {
                    points: vec![DVec2::new(-100.0, 10.0), DVec2::new(-10.0, 10.0)],
                    width: 0.35,
                },
                TitleItem::Text {
                    at: DVec2::new(-95.0, 20.0),
                    size: 4.0,
                    text: "{number} · {client}".into(),
                    align: TextAlign::Start,
                    angle: 0.0,
                    colour: None,
                    font: None,
                },
            ],
            border: false,
            fields: vec![("client".into(), "Client".into())],
            paper: None,
            guides: vec![],
            margins: None,
            stock: Default::default(),
        };
        let te = Element::new("Office", ElementKind::SheetTemplate(t));
        let tid = te.id;
        doc.commit(Transaction::new("Template").insert(te)).unwrap();
        let (sid, _) = sheets(doc.model())[0];
        let info = SheetInfo::default();
        let before = sheet_svg(doc.model(), sid, &info).unwrap();
        assert!(before.contains("PROJECT"), "the built-in block");
        doc.edit(sid, "Use template", |e| {
            if let ElementKind::Sheet(s) = &mut e.kind {
                s.template = Some(tid);
                s.fields.insert("client".into(), "Ana Lopes".into());
            }
        })
        .unwrap();
        let svg = sheet_svg(doc.model(), sid, &info).unwrap();
        assert!(!svg.contains("PROJECT"), "the template replaces it");
        let s = sheet_of(doc.model(), sid).unwrap();
        assert!(
            svg.contains(&format!("{} · Ana Lopes", s.number)),
            "fields filled"
        );
        // its bounds are measured from the corner, and views keep clear of it
        let (lo, hi) = title_block_bounds(doc.model(), s);
        assert!(
            (hi.x - (s.width - 10.0)).abs() < 1e-9 && lo.y > 9.0,
            "{lo} {hi}"
        );
        // deleting the template brings the built-in block back
        doc.commit(Transaction::new("Delete").remove_cascade(doc.model(), tid))
            .unwrap();
        assert!(sheet_svg(doc.model(), sid, &info)
            .unwrap()
            .contains("PROJECT"));
    }

    #[test]
    fn a_masters_paper_reaches_its_sheets_and_print_on_white_leaves_it_off() {
        use creabim_core::titleblock::{standard_strip, PaperColour, PaperLook, PaperTexture};
        use creabim_core::{Element, Transaction};
        let mut doc = creabim_core::Document::from_model(sample_house());
        let mut t = standard_strip();
        let cream = [0.96, 0.92, 0.84];
        t.stock = PaperLook {
            colour: PaperColour::Own(cream),
            texture: PaperTexture::Grid,
            ..Default::default()
        };
        let te = Element::new("Cream", ElementKind::SheetTemplate(t));
        let tid = te.id;
        doc.commit(Transaction::new("Master").insert(te)).unwrap();
        let (sid, _) = sheets(doc.model())[0];
        let plain = sheet_svg(doc.model(), sid, &SheetInfo::default()).unwrap();
        doc.edit(sid, "Use master", |e| {
            if let ElementKind::Sheet(s) = &mut e.kind {
                s.template = Some(tid);
            }
        })
        .unwrap();
        let hex = paper::hex(cream);
        let on = sheet_svg(doc.model(), sid, &SheetInfo::default()).unwrap();
        assert!(on.contains(&hex), "the paper's colour");
        assert!(!plain.contains(&hex));
        // a grid line every 5 mm: far more lines than without it
        let grid = paper::hex(std::array::from_fn(|i| cream[i] + (0.1 - cream[i]) * 0.18));
        assert!(on.matches(&grid).count() > 100, "{grid}");
        let white = SheetInfo {
            print_white: true,
            ..Default::default()
        };
        let printed = sheet_svg(doc.model(), sid, &white).unwrap();
        assert!(!printed.contains(&hex), "no tint on white");
        assert!(printed.len() < on.len(), "no texture on white");
        // the PDF carries it too
        let pdf = sheets_pdf(doc.model(), &SheetInfo::default());
        assert!(pdf.starts_with(b"%PDF"));
    }

    #[test]
    fn master_text_with_its_own_colour_and_font_is_drawn_with_them() {
        use creabim_core::titleblock::{Corner, SheetTemplate, TextAlign, TitleItem};
        let text = |colour, font| TitleItem::Text {
            at: DVec2::new(-50.0, 20.0),
            size: 5.0,
            text: "A101".into(),
            align: TextAlign::Start,
            angle: 0.0,
            colour,
            font,
        };
        let draw = |item| {
            let t = SheetTemplate {
                corner: Corner::BottomRight,
                items: vec![item],
                border: false,
                fields: vec![],
                paper: None,
                guides: vec![],
                margins: None,
                stock: Default::default(),
            };
            let mut svg = crate::paper::SvgCanvas::new(DVec2::new(420.0, 297.0));
            draw_template(&mut svg, &Sheet::a("A101", "A3"), &t, &BTreeMap::new());
            svg.finish()
        };
        let own = draw(text(Some([1.0, 1.0, 1.0]), Some(creabim_core::Font::Jost)));
        assert!(own.contains("#ffffff") && own.contains("Jost"), "{own}");
        // text without either is drawn as it always was
        let plain = draw(text(None, None));
        assert!(
            !plain.contains("#ffffff") && !plain.contains("Jost"),
            "{plain}"
        );
    }
}
