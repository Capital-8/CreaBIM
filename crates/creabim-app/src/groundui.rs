//! The Import survey window: a surveyor's table of points (Excel, CSV,
//! text, or rows pasted from a spreadsheet) or a DXF drawing with its
//! contours, read into a terrain. The columns, the units and where the
//! survey lands are shown before anything is brought in; importing is one
//! edit, so it undoes.

use super::*;
use creabim_core::{GroundLine, GroundLineKind, Site, SiteForm};
use creabim_exchange::survey::{self, Columns, DxfLayer, DxfSurvey, Table};
use std::sync::Mutex;

/// What a survey's numbers are in.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub(super) enum SurveyUnit {
    #[default]
    Metres,
    Centimetres,
    Millimetres,
    Feet,
}

impl SurveyUnit {
    const ALL: [(SurveyUnit, &'static str, &'static str); 4] = [
        (SurveyUnit::Metres, "m", "Metres"),
        (SurveyUnit::Centimetres, "cm", "Centimetres"),
        (SurveyUnit::Millimetres, "mm", "Millimetres"),
        (SurveyUnit::Feet, "ft", "Feet"),
    ];

    pub fn metres(self) -> f64 {
        match self {
            SurveyUnit::Metres => 1.0,
            SurveyUnit::Centimetres => 0.01,
            SurveyUnit::Millimetres => 0.001,
            SurveyUnit::Feet => 0.3048,
        }
    }

    fn of(metres: f64) -> Self {
        Self::ALL
            .iter()
            .map(|u| u.0)
            .find(|u| (u.metres() - metres).abs() < 1e-9)
            .unwrap_or_default()
    }
}

/// Where a survey lands in the drawing.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub(super) enum Place {
    /// At its own coordinates.
    #[default]
    Keep,
    /// Its middle on the drawing's origin.
    Origin,
    /// Where the project sits on the map: the project's map position
    /// comes off it, so it lies with everything else placed so.
    Map,
}

/// What was read, waiting to be brought in.
#[derive(Default)]
pub(super) enum Source {
    #[default]
    None,
    Table {
        table: Table,
        cols: Columns,
        /// The codes found, how many points carry each, and whether its
        /// points are joined as a break line.
        codes: Vec<(String, usize, bool)>,
    },
    Dxf {
        survey: DxfSurvey,
        /// Each layer, and whether it is brought in.
        layers: Vec<(DxfLayer, bool)>,
    },
}

/// The Import survey window's state, for the session.
#[derive(Default)]
pub(super) struct GroundUi {
    pub open: bool,
    /// The terrain the survey goes into; none makes a new one.
    target: Option<ElementId>,
    source: Source,
    file: String,
    /// Rows pasted in.
    paste: String,
    unit: SurveyUnit,
    place: Place,
    /// What comes off the survey's heights.
    take_off: f64,
    /// Give the terrain the outline of the data.
    fit: bool,
    /// Replace what the terrain holds, or add to it.
    replace: bool,
    error: Option<String>,
    raise: bool,
    /// The coordinate system the file names, and what its reader says
    /// was done to it.
    crs: Option<String>,
    notes: Vec<String>,
    /// Where the project sits on the map, as the window last saw it,
    /// and its zero above sea level.
    map: Option<creabim_core::MapPlace>,
    datum: f64,
}

/// What an import would bring in, in metres, at the survey's own
/// coordinates.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Reading {
    pub spots: Vec<DVec3>,
    pub lines: Vec<GroundLine>,
    /// Rows left out because they hold no three numbers.
    pub skipped: usize,
}

static REQUEST: Mutex<bool> = Mutex::new(false);

/// Open the Import survey window (from a terrain's Properties or the
/// Terrain tool).
pub(crate) fn request_open() {
    if let Ok(mut r) = REQUEST.lock() {
        *r = true;
    }
}

impl GroundUi {
    fn load_table(&mut self, table: Table, file: String) {
        let Some(cols) = survey::guess_columns(&table) else {
            self.error = Some(
                "This needs three columns of numbers: easting, northing and height.".to_string(),
            );
            return;
        };
        self.error = None;
        self.file = file;
        self.source = Source::Table {
            table,
            cols,
            codes: vec![],
        };
        self.recode();
        self.defaults();
    }

    /// The codes of the table as its columns are picked now, the ones
    /// that usually name a line ticked.
    fn recode(&mut self) {
        let unit = self.unit.metres();
        if let Source::Table { table, cols, codes } = &mut self.source {
            let (pts, _) = survey::points(table, *cols, unit);
            let was = std::mem::take(codes);
            *codes = survey::codes(&pts)
                .into_iter()
                .map(|(c, n)| {
                    let on = was
                        .iter()
                        .find(|w| w.0 == c)
                        .map_or(survey::is_line_code(&c) && n >= 2, |w| w.2);
                    (c, n, on)
                })
                .collect();
        }
    }

    pub(super) fn load_file(&mut self, path: &std::path::Path) {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let dxf = path
            .extension()
            .is_some_and(|e| e.to_string_lossy().eq_ignore_ascii_case("dxf"));
        self.crs = None;
        self.notes.clear();
        if creabim_exchange::gis::is_map_file(path) {
            match creabim_exchange::gis::read(path) {
                Ok(d) => {
                    self.crs = d.crs;
                    self.notes = d.notes;
                    self.load_layers(d.survey, name);
                }
                Err(e) => self.error = Some(e),
            }
            return;
        }
        if dxf {
            let read = std::fs::read(path)
                .map_err(|e| format!("couldn't read {}: {e}", path.display()))
                .and_then(|b| survey::dxf_survey(&String::from_utf8_lossy(&b)));
            match read {
                Ok(s)
                    if s.points.is_empty()
                        && s.lines.iter().all(|l| !l.is_contour() && !l.is_break()) =>
                {
                    self.error = Some("Nothing in this drawing carries a height: its lines all lie at zero. Trace them with the Terrain tool's Contour line and type their heights.".to_string());
                }
                Ok(s) => self.load_layers(s, name),
                Err(e) => self.error = Some(e),
            }
            return;
        }
        match survey::table_from_file(path) {
            Ok(t) => self.load_table(t, name),
            Err(e) => self.error = Some(e),
        }
    }

    /// A layered survey (a DXF drawing, a map file) as the source.
    pub(super) fn load_layers(&mut self, s: DxfSurvey, name: String) {
        if s.points.is_empty() && s.lines.iter().all(|l| !l.is_contour() && !l.is_break()) {
            self.error = Some("Nothing in this file carries a height, so it can't shape the ground. Lines without heights (boundaries, roads, buildings) are not brought in yet.".to_string());
            return;
        }
        self.error = None;
        self.file = name;
        self.unit = s.unit.map_or(SurveyUnit::Metres, SurveyUnit::of);
        // layers with heights are brought in, plain drawing isn't
        let layers = s
            .layers()
            .into_iter()
            .map(|l| {
                let on = l.points + l.contours + l.breaks > 0;
                (l, on)
            })
            .collect();
        self.source = Source::Dxf { survey: s, layers };
        self.defaults();
    }

    /// What the source gives as it is set up now.
    pub(super) fn reading(&self) -> Reading {
        let unit = self.unit.metres();
        match &self.source {
            Source::None => Reading::default(),
            Source::Table { table, cols, codes } => {
                let (pts, skipped) = survey::points(table, *cols, unit);
                let joined: Vec<String> =
                    codes.iter().filter(|c| c.2).map(|c| c.0.clone()).collect();
                let (spots, lines) = survey::assemble(&pts, &joined);
                Reading {
                    spots,
                    lines,
                    skipped,
                }
            }
            Source::Dxf { survey, layers } => {
                let on = |name: &str| layers.iter().any(|l| l.1 && l.0.name == name);
                let (spots, lines) = survey.ground(on, unit);
                Reading {
                    spots,
                    lines,
                    skipped: 0,
                }
            }
        }
    }

    /// A survey far from zero comes to the origin, unless told otherwise.
    fn defaults(&mut self) {
        let r = self.reading();
        let far = survey::centre(&r.spots, &r.lines).is_some_and(|c| c.length() > 2000.0);
        // a project placed on the map takes map data where it belongs
        let near_map = self.map.as_ref().is_some_and(|m| {
            survey::centre(&r.spots, &r.lines).is_some_and(|c| m.to_model(c).length() < 20_000.0)
        });
        self.place = if far && near_map {
            Place::Map
        } else if far {
            Place::Origin
        } else {
            Place::Keep
        };
        // heights above sea level come down to the project's zero: the
        // one it has when it is placed, else the middle of the survey's
        self.take_off = 0.0;
        if far {
            let n = r.spots.len() + r.lines.iter().map(|l| l.points.len()).sum::<usize>();
            let sum: f64 = r
                .spots
                .iter()
                .chain(r.lines.iter().flat_map(|l| l.points.iter()))
                .map(|p| p.z)
                .sum();
            let mean = sum / (n.max(1) as f64);
            if self.place == Place::Map {
                self.take_off = self.datum;
            } else if mean.abs() > 20.0 {
                self.take_off = mean.floor();
            }
        }
    }

    /// What comes off the survey's coordinates and heights.
    fn shift(&self, r: &Reading) -> DVec3 {
        let xy = match self.place {
            Place::Keep => DVec2::ZERO,
            Place::Origin => survey::centre(&r.spots, &r.lines)
                .unwrap_or_default()
                .round(),
            Place::Map => self
                .map
                .as_ref()
                .map_or(DVec2::ZERO, |m| DVec2::new(m.easting, m.northing)),
        };
        xy.extend(self.take_off)
    }
}

/// The terrain a reading makes, or `before` with it brought in: the data
/// moved by `shift` and set on `level` (`base` its elevation). Returns
/// the terrain and how many points fell outside its outline.
pub(super) fn bring_in(
    before: Option<&Site>,
    level: ElementId,
    base: f64,
    reading: &Reading,
    shift: DVec3,
    fit: bool,
    replace: bool,
) -> Option<(Site, usize)> {
    let (mut spots, mut lines) = (reading.spots.clone(), reading.lines.clone());
    survey::shift(&mut spots, &mut lines, shift + DVec3::Z * base);
    let mut site = before
        .cloned()
        .unwrap_or_else(|| Site::terrain(level, vec![], 0.0, 0.5));
    let fresh = before.is_none() || site.heights().next().is_none();
    let SiteForm::Terrain {
        boundary,
        points,
        contour,
        data,
        ..
    } = &mut site.form
    else {
        return None;
    };
    if replace {
        points.clear();
        data.lines.clear();
    }
    points.extend(spots);
    data.lines.extend(lines);
    data.shift = shift;
    if fit || boundary.len() < 3 {
        let all: Vec<DVec2> = points
            .iter()
            .chain(data.lines.iter().flat_map(|l| l.points.iter()))
            .map(|p| p.truncate())
            .collect();
        let hull = survey::outline_around(&all, 0.0);
        if hull.len() < 3 {
            return None;
        }
        *boundary = hull;
    }
    if fresh || replace {
        // a survey: its edge follows it, its contours are shown and named
        data.survey = true;
        data.labels = true;
        if fresh {
            // a surveyed ground is dug out under the building on it
            data.under_buildings = true;
        }
        if let Some((lo, hi)) = survey::heights(points, &data.lines) {
            *contour = survey::contour_interval(hi - lo);
        }
    }
    let outside = {
        let b = boundary.clone();
        points
            .iter()
            .filter(|p| !creabim_core::polygon::contains(&b, p.truncate()))
            .filter(|p| {
                // on the outline counts as in
                let n = b.len();
                !(0..n).any(|i| {
                    let (a, c) = (b[i], b[(i + 1) % n]);
                    let d = c - a;
                    let t =
                        ((p.truncate() - a).dot(d) / d.length_squared().max(1e-18)).clamp(0.0, 1.0);
                    (a + d * t).distance(p.truncate()) < 1e-3
                })
            })
            .count()
    };
    Some((site, outside))
}

/// A file Import survey reads that is nothing else to the app: a table
/// of points, a height grid, LandXML, GeoJSON or a shapefile.
pub(super) fn is_survey_file(path: &str) -> bool {
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    matches!(
        ext.as_str(),
        "xlsx"
            | "xlsm"
            | "xls"
            | "ods"
            | "csv"
            | "xyz"
            | "pts"
            | "asc"
            | "grd"
            | "landxml"
            | "tif"
            | "tiff"
            | "geojson"
            | "shp"
            | "osm"
    ) || (ext == "xml"
        && std::fs::read(path)
            .is_ok_and(|b| String::from_utf8_lossy(&b[..b.len().min(600)]).contains("<LandXML")))
}

impl CreaBimApp {
    /// Open Import survey on a file dropped on the window or named when
    /// the app was started.
    pub(super) fn open_survey_file(&mut self, path: &str) {
        let p = std::path::Path::new(path);
        // an aerial photo, or map data with no heights (boundaries, roads,
        // buildings), goes under the plan as a reference instead
        let lines_only = creabim_exchange::gis::is_map_file(p)
            && (creabim_exchange::gis::is_picture(p)
                || creabim_exchange::gis::read(p).is_ok_and(|d| {
                    d.survey.points.is_empty()
                        && d.survey
                            .lines
                            .iter()
                            .all(|l| !l.is_contour() && !l.is_break())
                }));
        if lines_only {
            if let Some((view, _)) = self.active_plan().or_else(|| self.first_plan()) {
                self.add_map_reference(view, path);
                return;
            }
        }
        self.open_ground_import();
        self.ground.map = self
            .doc
            .model()
            .project_info()
            .and_then(|(_, p)| p.map.clone());
        self.ground.datum = self
            .doc
            .model()
            .project_info()
            .map_or(0.0, |(_, p)| p.elevation);
        self.ground.load_file(p);
    }

    /// The first plan view of the project, with its level.
    fn first_plan(&self) -> Option<(ElementId, ElementId)> {
        self.doc.model().iter().find_map(|e| match &e.kind {
            ElementKind::View(v) => match v.kind {
                creabim_core::ViewKind::Plan { level, .. } => Some((e.id, level)),
                _ => None,
            },
            _ => None,
        })
    }

    /// Open the Import survey window, for the selected terrain if one is.
    pub(super) fn open_ground_import(&mut self) {
        let m = self.doc.model();
        self.ground.target = self.selection.all().into_iter().find(|id| {
            matches!(
                m.get(*id).map(|e| &e.kind),
                Some(ElementKind::Site(Site {
                    form: SiteForm::Terrain { .. },
                    ..
                }))
            )
        });
        self.ground.fit = self.ground.target.and_then(|id| m.get(id)).is_none_or(
            |e| !matches!(&e.kind, ElementKind::Site(s) if s.heights().next().is_some()),
        );
        self.ground.replace = false;
        self.ground.open = true;
        self.ground.raise = true;
    }

    fn import_ground(&mut self) {
        let reading = self.ground.reading();
        let shift = self.ground.shift(&reading);
        let m = self.doc.model();
        let before = self.ground.target.and_then(|id| m.get(id)).cloned();
        let before_site = before.as_ref().and_then(|e| match &e.kind {
            ElementKind::Site(s) => Some(s),
            _ => None,
        });
        let level = before_site
            .map(|s| s.level)
            .or_else(|| self.work_level().map(|l| l.0));
        let Some(level) = level else {
            self.ground.error = Some("The project needs a level to put the ground on.".into());
            return;
        };
        let base = m.level(level).map_or(0.0, |l| l.elevation);
        let made = bring_in(
            before_site,
            level,
            base,
            &reading,
            shift,
            self.ground.fit,
            self.ground.replace,
        );
        let Some((mut site, outside)) = made else {
            self.ground.error =
                Some("Three points not in one line are needed to make a ground.".into());
            return;
        };
        let (n, l) = (reading.spots.len(), reading.lines.len());
        let mut tx = Transaction::new("Import survey");
        let id = match before {
            Some(b) => {
                let mut after = b.clone();
                after.kind = ElementKind::Site(site);
                let id = b.id;
                tx = tx.replace(b, after);
                id
            }
            None => {
                if let Some((mat, new)) = creabim_core::library::find_or_new(m, "Grass") {
                    site.material = Some(mat);
                    if let Some(e) = new {
                        tx = tx.insert(e);
                    }
                }
                let e = Element::new("Terrain", ElementKind::Site(site));
                let id = e.id;
                tx = tx.insert(e);
                id
            }
        };
        // a survey moved to the origin from far away says where the
        // project is on the map, when nothing has yet
        let far = shift.truncate().length() > 2000.0;
        let places = self.ground.place == Place::Origin && far && self.ground.map.is_none();
        if places {
            let mut info = m.project_info().map(|(_, p)| p.clone()).unwrap_or_default();
            info.map = Some(creabim_core::MapPlace {
                crs: self.ground.crs.clone().unwrap_or_default(),
                easting: shift.x,
                northing: shift.y,
            });
            if info.elevation == 0.0 {
                info.elevation = shift.z;
            }
            tx.ops.extend(creabim_core::set_project(m, info).ops);
        }
        let r = self.doc.commit(tx);
        let ok = r.is_ok();
        let mut msg = format!(
            "Survey brought in: {n} spot heights and {l} lines from {}.",
            self.ground.file
        );
        if outside > 0 {
            msg.push_str(&format!(
                " {outside} points lie outside the terrain's outline and don't shape it: tick Fit the outline, or stretch the outline over them."
            ));
        }
        if shift.truncate() != DVec2::ZERO {
            msg.push_str(&format!(
                " Moved to the drawing's origin: {:.0} east and {:.0} north were taken off, and are kept with the terrain.",
                shift.x, shift.y
            ));
        }
        if places {
            msg.push_str(" The project is now placed on the map there (the Project tab); map data brought in later lands in the right place.");
        }
        if self.ground.place == Place::Map {
            msg.push_str(" Placed by the project's map position.");
        }
        msg.push_str(" Ctrl+Z to undo.");
        self.report(r, msg);
        if ok {
            self.selection.only(id);
            self.ground.open = false;
            self.ground.source = Source::None;
            self.ground.paste.clear();
            self.frame_all();
        }
    }

    /// The Import survey window, when open (or asked for).
    pub(super) fn ground_window(&mut self, ctx: &egui::Context) {
        if REQUEST
            .lock()
            .ok()
            .is_some_and(|mut r| std::mem::take(&mut *r))
        {
            self.open_ground_import();
        }
        if let Some(path) = crate::servicesui::take_long_section() {
            self.open_long_section(&path);
        }
        if let Some(at) = crate::investigationui::take_log() {
            self.open_borehole_log(at);
        }
        if !self.ground.open {
            return;
        }
        let area = self.mat.area.unwrap_or_else(|| ctx.content_rect());
        let target_name = self
            .ground
            .target
            .and_then(|id| self.doc.model().get(id))
            .map(|e| e.name.clone());
        self.ground.map = self
            .doc
            .model()
            .project_info()
            .and_then(|(_, p)| p.map.clone());
        self.ground.datum = self
            .doc
            .model()
            .project_info()
            .map_or(0.0, |(_, p)| p.elevation);
        let mut open = true;
        let (mut pick, mut read_paste, mut go, mut recode) = (false, false, false, false);
        let g = &mut self.ground;
        let reading = g.reading();
        let shown = crate::app::shell::window("Import survey")
            .open(&mut open)
            .default_pos(area.min + Vec2::new(90.0, 50.0))
            .default_size(Vec2::new(620.0, 560.0))
            .min_width(520.0)
            .constrain_to(area)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.label(match &target_name {
                    Some(n) => format!("Into the selected terrain, {n}."),
                    None => "Makes a new terrain. (Select a terrain first to bring a survey into it.)".to_string(),
                });
                ui.add_space(4.0);
                crate::props::flow(ui, |ui| {
                    pick = ui
                        .button("Open a file…")
                        .on_hover_text("A table of points (Excel .xlsx, .xls, .ods, .csv, .txt, .xyz) or a DXF drawing with contours, 3D lines and points")
                        .clicked();
                    ui.label("or paste rows copied from a spreadsheet:");
                });
                // a long paste scrolls in its box rather than pushing the
                // rest of the window off the screen
                egui::ScrollArea::vertical()
                    .max_height(64.0)
                    .id_salt("ground.paste")
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut g.paste)
                                .desired_rows(2)
                                .desired_width(f32::INFINITY)
                                .hint_text("Point  Easting  Northing  Height  Code"),
                        );
                    });
                if !g.paste.trim().is_empty() {
                    read_paste = ui.button("Read the pasted rows").clicked();
                }
                if let Some(e) = &g.error {
                    ui.colored_label(ui.visuals().warn_fg_color, e);
                }
                ui.separator();
                let unit_tip = "What the survey's numbers are in";
                match &mut g.source {
                    Source::None => {
                        ui.label("Nothing read yet.");
                        crate::props::note(ui, "A table needs a column each for easting (X), northing (Y) and height (Z). A column of point numbers and one of codes (KERB, TOP, GL…) are understood too. Contour lines on paper or in a flat drawing are traced instead: the Terrain tool, set to Contour line.");
                    }
                    Source::Table { table, cols, codes } => {
                        ui.label(egui::RichText::new(format!("{}: {} rows", g.file, table.rows.len())).strong());
                        let n = table.columns();
                        let mut pick_col = |ui: &mut egui::Ui, label: &str, v: &mut usize| {
                            crate::props::row(ui, label, |ui| {
                                egui::ComboBox::from_id_salt(("ground.col", label))
                                    .icon(crate::props::combo_icon)
                                    .selected_text(table.name(*v))
                                    .show_ui(ui, |ui| {
                                        for c in 0..n {
                                            recode |= ui.selectable_value(v, c, table.name(c)).changed();
                                        }
                                    });
                            });
                        };
                        pick_col(ui, "Easting (X)", &mut cols.x);
                        pick_col(ui, "Northing (Y)", &mut cols.y);
                        pick_col(ui, "Height (Z)", &mut cols.z);
                        crate::props::row(ui, "Code", |ui| {
                            egui::ComboBox::from_id_salt("ground.col.code")
                                .icon(crate::props::combo_icon)
                                .selected_text(cols.code.map_or("None".to_string(), |c| table.name(c)))
                                .show_ui(ui, |ui| {
                                    recode |= ui.selectable_value(&mut cols.code, None, "None").changed();
                                    for c in 0..n {
                                        recode |= ui.selectable_value(&mut cols.code, Some(c), table.name(c)).changed();
                                    }
                                });
                        });
                        // the first rows, as they will be read
                        egui::ScrollArea::horizontal().id_salt("ground.preview").show(ui, |ui| {
                            egui::Grid::new("ground.rows").striped(true).min_col_width(56.0).min_row_height(14.0).spacing([12.0, 2.0]).show(ui, |ui| {
                                for c in 0..n {
                                    let role = if c == cols.x {
                                        "X"
                                    } else if c == cols.y {
                                        "Y"
                                    } else if c == cols.z {
                                        "Z"
                                    } else if Some(c) == cols.code {
                                        "Code"
                                    } else {
                                        ""
                                    };
                                    let name = table.name(c);
                                    let text = if role.is_empty() || role == name {
                                        name
                                    } else {
                                        format!("{name} · {role}")
                                    };
                                    let mut t = egui::RichText::new(text).small();
                                    if !role.is_empty() {
                                        t = t.strong();
                                    }
                                    ui.label(t);
                                }
                                ui.end_row();
                                for r in table.rows.iter().take(4) {
                                    for c in 0..n {
                                        ui.label(egui::RichText::new(r.get(c).map_or("", |s| s.as_str())).small());
                                    }
                                    ui.end_row();
                                }
                            });
                        });
                        recode |= crate::props::choice(ui, "Units", &mut g.unit, &SurveyUnit::ALL.map(|u| (u.0, u.1, unit_tip)));
                        if !codes.is_empty() {
                            ui.add_space(4.0);
                            ui.label("Codes: tick the ones surveyed as a line. Their points are joined in order as a break line, and the ground folds along it.");
                            ui.horizontal_wrapped(|ui| {
                                for (c, n, on) in codes.iter_mut() {
                                    crate::props::tickbox(ui, on, format!("{c} ({n})"));
                                }
                            });
                        }
                    }
                    Source::Dxf { survey: _, layers } => {
                        ui.label(egui::RichText::new(format!("{}: {} with something in it", g.file, if layers.len() == 1 { "1 layer".to_string() } else { format!("{} layers", layers.len()) })).strong());
                        crate::props::note(ui, "Points with a height become spot heights, lines at one height contours, lines with heights break lines. Lines without a height are left out.");
                        egui::ScrollArea::vertical().max_height(170.0).id_salt("ground.layers").show(ui, |ui| {
                            for (l, on) in layers.iter_mut() {
                                let mut parts = vec![];
                                for (n, what) in [(l.points, "points"), (l.contours, "contours"), (l.breaks, "3D lines"), (l.flat, "flat lines")] {
                                    if n > 0 {
                                        parts.push(format!("{n} {what}"));
                                    }
                                }
                                ui.add_enabled_ui(l.points + l.contours + l.breaks > 0, |ui| crate::props::tickbox(ui, on, format!("{}: {}", l.name, parts.join(", "))));
                            }
                        });
                        crate::props::choice(ui, "Units", &mut g.unit, &SurveyUnit::ALL.map(|u| (u.0, u.1, unit_tip)));
                    }
                }
                if !matches!(g.source, Source::None) {
                    ui.separator();
                    let mut places = vec![
                        (Place::Keep, "As surveyed", "At the survey's own coordinates"),
                        (Place::Origin, "At the drawing's origin", "The middle of the survey on the drawing's origin. A survey in map coordinates is hundreds of kilometres from zero, too far to draw at; what is taken off is kept with the terrain, and places the project on the map if nothing has yet"),
                    ];
                    if g.map.is_some() {
                        places.push((Place::Map, "On the map", "Where the project sits on the map (the Project tab): its map position comes off the data, so it lies with everything else brought in this way"));
                    } else if g.place == Place::Map {
                        g.place = Place::Origin;
                    }
                    crate::props::choice(ui, "Place", &mut g.place, &places);
                    if let Some(crs) = &g.crs {
                        let theirs = g.map.as_ref().map(|m| m.crs.trim()).filter(|c| !c.is_empty());
                        match theirs {
                            Some(ours) if !ours.eq_ignore_ascii_case(crs.trim()) => crate::props::note(ui, format!("This file is in {crs}, the project is placed in {ours}. Nothing is converted between the two: it will land in the wrong place unless they are the same system under two names.")),
                            _ => crate::props::note(ui, format!("Coordinate system: {crs}.")),
                        }
                    }
                    for n in &g.notes {
                        crate::props::note(ui, n);
                    }
                    crate::props::row_tip(ui, "Heights from", "Take off heights: what is typed here comes off every height of the survey, so the site can lie by a ground floor at 0", |ui| {
                        crate::props::flow(ui, |ui| {
                            ui.add(egui::DragValue::new(&mut g.take_off).speed(0.1).suffix(" m"))
                                .on_hover_text("A survey's heights are above sea level or a site datum. What is typed here comes off them, so a site at 312.40 can lie by a ground floor at 0");
                            if let Some((lo, _)) = survey::heights(&reading.spots, &reading.lines) {
                                if ui.button("Lowest point at 0").on_hover_text("Take off the survey's lowest height, to the whole metre").clicked() {
                                    g.take_off = lo.floor();
                                }
                            }
                        });
                    });
                    if target_name.is_some() {
                        crate::props::tickbox(ui, &mut g.fit, "Fit the terrain's outline to the data");
                        crate::props::tickbox(ui, &mut g.replace, "Replace the spot heights and lines it has");
                    }
                    // what will come in
                    let shift = g.shift(&reading);
                    let mut all = reading.spots.iter().chain(reading.lines.iter().flat_map(|l| l.points.iter()));
                    let summary = match all.next() {
                        None => "Nothing to bring in as set up.".to_string(),
                        Some(first) => {
                            let (lo, hi) = all.fold((*first, *first), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
                            let size = hi - lo;
                            let (c, b) = reading.lines.iter().fold((0, 0), |(c, b), l| {
                                if l.kind == GroundLineKind::Contour { (c + 1, b) } else { (c, b + 1) }
                            });
                            let mut s = format!(
                                "{} spot heights, {c} contours, {b} break lines. {:.1} m by {:.1} m, heights {:.2} to {:.2} m.",
                                reading.spots.len(), size.x, size.y, lo.z - shift.z, hi.z - shift.z
                            );
                            if reading.skipped > 0 {
                                s.push_str(&format!(" {} rows without three numbers are left out.", reading.skipped));
                            }
                            if size.x.max(size.y) > 20_000.0 || (size.x.max(size.y) < 1.0 && reading.spots.len() > 3) {
                                s.push_str(" That is an unlikely size for a site: check the units.");
                            }
                            s
                        }
                    };
                    ui.add_space(4.0);
                    ui.label(summary);
                }
                ui.add_space(6.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let n = reading.spots.len() + reading.lines.len();
                    let text = match &g.source {
                        Source::None => "Import".to_string(),
                        _ => format!("Import {} points", reading.spots.len() + reading.lines.iter().map(|l| l.points.len()).sum::<usize>()),
                    };
                    go = ui.add_enabled(n > 0, egui::Button::new(text)).clicked();
                });
            });
        if std::mem::take(&mut g.raise) {
            if let Some(s) = &shown {
                ctx.move_to_top(s.response.layer_id);
            }
        }
        if recode {
            g.recode();
        }
        if read_paste {
            let t = survey::table_from_text(&g.paste);
            g.load_table(t, "the pasted rows".to_string());
        }
        if pick {
            let picked = rfd::FileDialog::new()
                .set_title("Import a survey")
                .add_filter(
                    "Surveys and map data",
                    &[
                        "xlsx", "xlsm", "xls", "ods", "csv", "txt", "xyz", "pts", "asc", "dxf",
                        "xml", "landxml", "tif", "tiff", "grd", "geojson", "json", "shp",
                    ],
                )
                .add_filter(
                    "Tables of points",
                    &[
                        "xlsx", "xlsm", "xls", "ods", "csv", "txt", "xyz", "pts", "asc",
                    ],
                )
                .add_filter("DXF drawings", &["dxf"])
                .add_filter("LandXML surfaces", &["xml", "landxml"])
                .add_filter(
                    "Height grids (GeoTIFF, Esri ASCII)",
                    &["tif", "tiff", "asc", "grd"],
                )
                .add_filter("GeoJSON and shapefiles", &["geojson", "json", "shp"])
                .pick_file();
            if let Some(p) = picked {
                self.ground.load_file(&p);
            }
        }
        if go {
            self.import_ground();
        }
        if !open {
            self.ground.open = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table_ui(text: &str) -> GroundUi {
        let mut g = GroundUi::default();
        g.load_table(survey::table_from_text(text), "test".into());
        g
    }

    #[test]
    fn a_pasted_table_makes_a_terrain_round_its_points() {
        let g = table_ui("P,E,N,Z,Code\n1,0,0,10.0,GL\n2,20,0,10.5,KERB\n3,20,15,11.0,KERB\n4,0,15,10.2,GL\n5,10,8,10.9,GL\n");
        let r = g.reading();
        assert_eq!((r.spots.len(), r.lines.len()), (5, 1));
        let level = ElementId::new();
        let (site, outside) = bring_in(None, level, 0.0, &r, DVec3::Z * 10.0, true, false).unwrap();
        assert_eq!(outside, 0);
        let SiteForm::Terrain {
            boundary,
            points,
            contour,
            data,
            ..
        } = &site.form
        else {
            panic!()
        };
        assert_eq!(boundary.len(), 4);
        assert!((points[4].z - 0.9).abs() < 1e-9);
        assert!(data.survey && data.labels);
        assert_eq!(*contour, 0.05);
        assert_eq!(data.shift, DVec3::Z * 10.0);
        let ground = creabim_geom::ground::of(&site).unwrap();
        assert!((ground.height(DVec2::new(10.0, 8.0)).unwrap() - 0.9).abs() < 1e-9);
    }

    #[test]
    fn a_survey_in_map_coordinates_is_set_at_the_origin() {
        let g = table_ui("512330.4 4362118.1 312.4\n512390.4 4362118.1 313.0\n512390.4 4362158.1 318.9\n512330.4 4362158.1 316.0\n");
        assert_eq!(g.place, Place::Origin);
        let r = g.reading();
        let shift = g.shift(&r);
        assert_eq!(shift, DVec3::new(512360.0, 4362138.0, 315.0));
        let (site, _) = bring_in(None, ElementId::new(), 0.0, &r, shift, true, false).unwrap();
        let (lo, hi) = site.bounds(&Model::default()).unwrap();
        assert!(lo.x.abs() < 40.0 && hi.y.abs() < 40.0);
    }

    #[test]
    fn points_added_to_a_terrain_outside_its_outline_are_counted() {
        let level = ElementId::new();
        let before = Site::terrain(
            level,
            creabim_core::polygon::rectangle(DVec2::ZERO, DVec2::splat(10.0)),
            0.0,
            0.5,
        );
        let g = table_ui("5 5 1\n6 2 1.5\n30 30 2\n");
        let r = g.reading();
        let (site, outside) =
            bring_in(Some(&before), level, 0.0, &r, DVec3::ZERO, false, false).unwrap();
        assert_eq!(outside, 1);
        assert_eq!(site.boundary().unwrap().len(), 4);
        // fitted, the outline takes them all in
        let (site, outside) =
            bring_in(Some(&before), level, 0.0, &r, DVec3::ZERO, true, false).unwrap();
        assert_eq!(outside, 0);
        assert_eq!(site.boundary().unwrap().len(), 3);
    }

    #[test]
    fn map_files_come_in_by_layer_and_land_where_the_project_is() {
        let dir = std::env::temp_dir().join(format!("creabim-gis-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("contours.geojson");
        std::fs::write(&path, r#"{"type":"FeatureCollection","features":[
          {"type":"Feature","properties":{"elev":100},"geometry":{"type":"LineString","coordinates":[[-79.3871,43.6426],[-79.3861,43.6426]]}},
          {"type":"Feature","properties":{"elev":101},"geometry":{"type":"LineString","coordinates":[[-79.3871,43.6430],[-79.3861,43.6430]]}},
          {"type":"Feature","properties":{"highway":"service"},"geometry":{"type":"LineString","coordinates":[[-79.3871,43.6428],[-79.3861,43.6428]]}}
        ]}"#).unwrap();
        let mut g = GroundUi::default();
        g.load_file(&path);
        assert!(g.error.is_none(), "{:?}", g.error);
        assert_eq!(g.crs.as_deref(), Some("EPSG:32617"));
        let Source::Dxf { layers, .. } = &g.source else {
            panic!()
        };
        // the road carries no height: listed, not brought in
        assert_eq!(layers.iter().filter(|l| l.1).count(), 1);
        assert_eq!(layers.len(), 2);
        let r = g.reading();
        assert_eq!((r.spots.len(), r.lines.len()), (0, 2));
        // nothing placed yet: its middle comes to the origin
        assert_eq!(g.place, Place::Origin);
        let first = g.shift(&r);
        assert!((first.x - 630_127.0).abs() < 2.0, "{first}");
        // once the project is on the map, map data lands by that
        g.map = Some(creabim_core::MapPlace {
            crs: "EPSG:32617".into(),
            easting: 630_000.0,
            northing: 4_833_400.0,
        });
        g.datum = 98.0;
        g.load_file(&path);
        assert_eq!(g.place, Place::Map);
        assert_eq!(
            g.shift(&g.reading()),
            DVec3::new(630_000.0, 4_833_400.0, 98.0)
        );
        // a file with nothing to shape the ground says so
        let flat = dir.join("roads.geojson");
        std::fs::write(&flat, r#"{"type":"Feature","properties":{},"geometry":{"type":"LineString","coordinates":[[0.1,0.1],[0.2,0.2]]}}"#).unwrap();
        g.load_file(&flat);
        assert!(g.error.as_deref().is_some_and(|e| e.contains("height")));
        std::fs::remove_dir_all(&dir).ok();
    }
}
