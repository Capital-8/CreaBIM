//! The mark moments (Mark Moments REV02): Export and Import as cards in
//! the Save the changes window's family. An export is chosen, then runs off
//! the window's thread while the woven nest works and a real bar counts the
//! elements; it ends on what was written, with Show in folder. Opening an
//! IFC file reads it the same way and ends on the import's answer, which
//! takes the place of the Import report window.
//!
//! Only where the app has its pages; without them (and in tests) exports
//! and imports run as before, in the window's own thread.

use super::*;
use serde_json::json;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Instant;

/// How far a job has got, shared with its thread.
#[derive(Default)]
pub(crate) struct Meter {
    n: AtomicUsize,
    total: AtomicUsize,
    step: Mutex<String>,
    /// Stop was pressed: the job writes nothing.
    stop: AtomicBool,
}

impl Meter {
    fn tick(&self, n: usize, total: usize, step: &str) {
        self.n.store(n, Ordering::Relaxed);
        self.total.store(total, Ordering::Relaxed);
        if let Ok(mut s) = self.step.lock() {
            if *s != step {
                *s = step.to_string();
            }
        }
    }

    fn read(&self) -> (usize, usize, String) {
        let step = self.step.lock().map(|s| s.clone()).unwrap_or_default();
        (
            self.n.load(Ordering::Relaxed),
            self.total.load(Ordering::Relaxed),
            step,
        )
    }

    fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }
}

/// What a card is about.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum CardKind {
    /// An export: `export.ifc4`, `export.ifcx`, `export.glb`,
    /// `export.fcstd` or `export.pdf`.
    Export(String),
    Import,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Phase {
    /// The export's choices, as radio rows.
    Choose,
    Working,
    Done,
    Failed(String),
}

/// What an export wrote.
#[derive(Clone, Debug)]
pub(crate) struct Written {
    /// The file, or the folder for a PDF a sheet.
    pub path: std::path::PathBuf,
    pub bytes: u64,
    pub count: usize,
    /// "element", or "sheet" for PDFs.
    pub unit: &'static str,
    pub files: usize,
}

enum Outcome {
    Wrote(Written),
    Imported(Box<creabim_ifc::ImportResult>),
}

pub(crate) struct MarkCard {
    pub kind: CardKind,
    pub phase: Phase,
    /// The file being written or read, as shown.
    pub file: String,
    /// Its full path.
    pub path: String,
    meter: Arc<Meter>,
    rx: Option<mpsc::Receiver<Result<Outcome, String>>>,
    started: Instant,
    secs: f32,
    pub written: Option<Written>,
    /// Where the card's top stays once it has its height (see the shell).
    pub top: std::cell::Cell<Option<f32>>,
    /// Sheets as PDF: which sheets, and one PDF or a PDF each.
    pub scope: ExportScope,
    pub each: bool,
    /// Print on white: the masters' paper tint and texture left off.
    pub white: bool,
}

impl MarkCard {
    fn new(kind: CardKind, phase: Phase) -> Self {
        Self {
            kind,
            phase,
            file: String::new(),
            path: String::new(),
            meter: Arc::new(Meter::default()),
            rx: None,
            started: Instant::now(),
            secs: 0.0,
            written: None,
            top: Default::default(),
            scope: ExportScope::All,
            each: false,
            white: false,
        }
    }
}

const SHEETS_FMT: &str = "export.pdf";

/// What each export is called on its card.
fn export_name(what: &str) -> &'static str {
    match what {
        "export.ifc4" => "IFC4",
        "export.ifcx" => "IFC5",
        "export.glb" => "glTF",
        "export.fcstd" => "FreeCAD",
        SHEETS_FMT => "sheets as PDF",
        _ => "",
    }
}

fn human_size(bytes: u64) -> String {
    let b = bytes as f64;
    if b >= 1024.0 * 1024.0 {
        format!("{:.1} MB", b / 1024.0 / 1024.0)
    } else if b >= 1024.0 {
        format!("{:.0} KB", b / 1024.0)
    } else {
        format!("{bytes} bytes")
    }
}

fn plural(n: usize, one: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {one}s")
    }
}

/// The elements an export writes, as people count them: no levels, views,
/// sheets or the like.
fn element_count(model: &Model) -> usize {
    model.iter().filter(|e| is_element(e.ifc_class())).count()
}

/// A built thing, as people count elements: not a level, grid, material,
/// type or the project itself.
fn is_element(class: &str) -> bool {
    class.starts_with("Ifc")
        && !class.starts_with("IfcMaterial")
        && !class.ends_with("Type")
        && !class.ends_with("Style")
        && !matches!(
            class,
            "IfcBuildingStorey" | "IfcGrid" | "IfcProject" | "IfcSite" | "IfcBuilding"
        )
}

/// The import's first note, said plainly, with how many elements it is
/// about; known notes get their own words.
pub(crate) fn note_sentence(note: &str) -> String {
    let (group, n) = match note.rsplit_once(" (×") {
        Some((g, rest)) => (g, rest.trim_end_matches(')').parse().unwrap_or(1)),
        None => (note, 1usize),
    };
    if group == "walls whose clipping was ignored to keep them native walls" {
        let walls = plural(n, "wall");
        let were = if n == 1 { "was" } else { "were" };
        let they = if n == 1 {
            "It came in as a plain wall"
        } else {
            "They came in as plain walls"
        };
        return format!("{walls} {were} clipped by a roof in the file. {they}: check their tops.");
    }
    let mut s: String = group.to_string();
    if let Some(f) = s.get(0..1) {
        s = f.to_uppercase() + &s[1..];
    }
    if n > 1 {
        format!("{s}: {n}.")
    } else {
        format!("{s}.")
    }
}

/// The elements an import's notes name, by the GlobalId in brackets at
/// the end of each name.
pub(crate) fn noted_ids(details: &[String], model: &Model) -> Vec<ElementId> {
    let mut out = Vec::new();
    for d in details {
        let Some(open) = d.find(" (") else { continue };
        let rest = &d[open + 2..];
        let Some(close) = rest.find(')') else {
            continue;
        };
        if let Some(id) = ElementId::from_ifc_guid(&rest[..close]) {
            if model.get(id).is_some() && !out.contains(&id) {
                out.push(id);
            }
        }
    }
    out
}

/// A class as a chip says it: "walls", "furniture".
fn kind_name(class: &str, n: usize) -> String {
    let base = class.trim_start_matches("Ifc");
    let one = match base {
        "Wall" | "WallStandardCase" => "wall",
        "Window" => "window",
        "Door" => "door",
        "Slab" => "slab",
        "Roof" => "roof",
        "Beam" => "beam",
        "Column" => "column",
        "Stair" | "StairFlight" => "stair",
        "Railing" => "railing",
        "Covering" => "covering",
        "Member" => "member",
        "Plate" => "plate",
        "Space" => "room",
        "Footing" => "footing",
        "CurtainWall" => "curtain wall",
        "Furniture" | "FurnishingElement" => return "furniture".into(),
        _ => return base.to_string(),
    };
    if n == 1 {
        one.to_string()
    } else {
        format!("{one}s")
    }
}

impl CreaBimApp {
    /// Cards are drawn where the app has its pages.
    pub(crate) fn mark_cards(&self) -> bool {
        self.shell.web.is_some() && !cfg!(test)
    }

    /// Open an export's card: its choices first when it has some, else
    /// straight to where it goes.
    pub(super) fn open_export_card(&mut self, what: &str) {
        let mut card = MarkCard::new(CardKind::Export(what.to_string()), Phase::Choose);
        if what == SHEETS_FMT {
            card.scope = self.sheets_export.scope;
            card.each = self.sheets_export.each;
            card.white = self.sheets_export.white;
        }
        let has_choices = matches!(what, "export.ifc4" | SHEETS_FMT);
        self.mark = Some(card);
        if !has_choices {
            self.start_export();
        }
    }

    /// Ask where the export goes, then write it off the window's thread.
    fn start_export(&mut self) {
        let Some(card) = &self.mark else { return };
        let CardKind::Export(what) = card.kind.clone() else {
            return;
        };
        let model = self.doc.model().clone();
        let meter = Arc::new(Meter::default());
        let (tx, rx) = mpsc::channel();
        let m = meter.clone();
        let count = element_count(&model);
        let label = stem(&self.path);
        // where, asked here: the system's save window belongs to this thread
        let job: Box<dyn FnOnce() -> Result<Outcome, String> + Send> = match what.as_str() {
            "export.ifc4" => {
                let Some(out) = self.export_to(with_extension(&self.path, "ifc")) else {
                    return self.mark_cancelled();
                };
                let mut opts = creabim_ifc::ExportOptions::for_model(&model, &label);
                opts.body = match self.shell.settings.ifc_body.as_str() {
                    "joined" => creabim_ifc::IfcBody::Joined,
                    "layers" => creabim_ifc::IfcBody::Layers,
                    _ => creabim_ifc::IfcBody::Modelled,
                };
                self.set_card_file(&out);
                Box::new(move || {
                    let text =
                        creabim_ifc::export_ifc4_with(&model, &opts, &|n, t, s| m.tick(n, t, s));
                    write_one(&m, &out, text.as_bytes(), count, "element")
                })
            }
            "export.ifcx" => {
                let Some(out) = self.export_to(with_extension(&self.path, "ifcx")) else {
                    return self.mark_cancelled();
                };
                self.set_card_file(&out);
                Box::new(move || {
                    let text = creabim_ifc::export_ifcx_with(&model, &|n, t, s| m.tick(n, t, s));
                    write_one(&m, &out, text.as_bytes(), count, "element")
                })
            }
            "export.glb" => {
                let Some(out) = self.export_to(with_extension(&self.path, "glb")) else {
                    return self.mark_cancelled();
                };
                self.set_card_file(&out);
                Box::new(move || {
                    let b =
                        creabim_exchange::gltf::export_glb_with(&model, &|n, t, s| m.tick(n, t, s));
                    write_one(&m, &out, &b, count, "element")
                })
            }
            "export.fcstd" => {
                let Some(out) = self.export_to(with_extension(&self.path, "FCStd")) else {
                    return self.mark_cancelled();
                };
                self.set_card_file(&out);
                Box::new(move || {
                    let b =
                        creabim_exchange::freecad::export_fcstd_with(&model, &label, &|n, t, s| {
                            m.tick(n, t, s)
                        })
                        .map_err(|e| e.to_string())?;
                    write_one(&m, &out, &b, count, "element")
                })
            }
            SHEETS_FMT => {
                let Some(job) = self.sheets_job(model, m) else {
                    return self.mark_cancelled();
                };
                job
            }
            _ => return,
        };
        std::thread::spawn(move || {
            let _ = tx.send(job());
        });
        if let Some(card) = &mut self.mark {
            card.meter = meter;
            card.rx = Some(rx);
            card.started = Instant::now();
            card.phase = Phase::Working;
        }
    }

    /// Sheets as PDF: which, as one PDF or a PDF each, asked where.
    fn sheets_job(
        &mut self,
        model: Model,
        m: Arc<Meter>,
    ) -> Option<Box<dyn FnOnce() -> Result<Outcome, String> + Send>> {
        let card = self.mark.as_ref()?;
        let (scope, each, white) = (card.scope, card.each, card.white);
        self.sheets_export.scope = scope;
        self.sheets_export.each = each;
        self.sheets_export.white = white;
        // chosen: the sheets selected in the project browser
        if scope == ExportScope::Chosen {
            self.sheets_export.chosen = self.chosen_sheets().into_iter().collect();
        }
        let ids = self.sheets_to_export(scope == ExportScope::This);
        if ids.is_empty() {
            self.status = "No sheets to export: open a sheet, or choose All.".into();
            return None;
        }
        let info = sheets::SheetInfo {
            print_white: white,
            ..self.sheet_info()
        };
        let names: Vec<String> = ids.iter().map(|id| sheet_file_name(&model, *id)).collect();
        if each && ids.len() > 1 {
            let start = std::path::absolute(&self.path)
                .ok()
                .and_then(|p| p.parent().map(|d| d.to_path_buf()));
            let mut d = rfd::FileDialog::new().set_title("Export sheets: a PDF each, into…");
            if let Some(s) = start {
                d = d.set_directory(s);
            }
            let dir = d.pick_folder()?;
            self.last_export_dir = Some(dir.clone());
            self.set_card_file(&dir.display().to_string());
            return Some(Box::new(move || {
                let mut bytes = 0;
                let mut files = 0;
                for (i, (id, name)) in ids.iter().zip(&names).enumerate() {
                    m.tick(i, ids.len(), &format!("Sheet {name}"));
                    let pdf = sheets::sheets_pdf_of(&model, &info, &[*id]);
                    if m.stopped() {
                        return Err("stopped".into());
                    }
                    std::fs::write(dir.join(format!("{name}.pdf")), &pdf)
                        .map_err(|e| e.to_string())?;
                    bytes += pdf.len() as u64;
                    files += 1;
                }
                Ok(Outcome::Wrote(Written {
                    path: dir,
                    bytes,
                    count: ids.len(),
                    unit: "sheet",
                    files,
                }))
            }));
        }
        let default = if ids.len() == 1 {
            std::path::Path::new(&self.path)
                .parent()
                .unwrap_or(std::path::Path::new(""))
                .join(format!("{}.pdf", names[0]))
                .to_string_lossy()
                .into_owned()
        } else {
            with_extension(&self.path, "pdf")
        };
        let out = self.export_to(default)?;
        self.set_card_file(&out);
        Some(Box::new(move || {
            let pdf = sheets::sheets_pdf_of_with(&model, &info, &ids, &|n, t, s| m.tick(n, t, s));
            write_one(&m, &out, &pdf, ids.len(), "sheet")
        }))
    }

    /// The sheets selected in the project browser.
    fn chosen_sheets(&self) -> Vec<ElementId> {
        let all: Vec<ElementId> = sheets::sheets(self.doc.model())
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        self.selection
            .all()
            .iter()
            .copied()
            .filter(|id| all.contains(id))
            .collect()
    }

    fn set_card_file(&mut self, out: &str) {
        if let Some(card) = &mut self.mark {
            card.path = out.to_string();
            card.file = file_label(out);
        }
    }

    /// The system's save window was cancelled: back to the choices, or
    /// nothing at all when there were none.
    fn mark_cancelled(&mut self) {
        let choices = matches!(&self.mark, Some(MarkCard { kind: CardKind::Export(w), .. }) if w == "export.ifc4" || w == SHEETS_FMT);
        if !choices {
            self.close_mark();
        }
    }

    /// Open an IFC file with its card: read off the window's thread, then
    /// made the project here.
    pub(super) fn start_import(&mut self, path: &str) {
        let mut card = MarkCard::new(CardKind::Import, Phase::Working);
        card.path = path.to_string();
        card.file = file_label(path);
        let meter = card.meter.clone();
        let (tx, rx) = mpsc::channel();
        card.rx = Some(rx);
        let path = path.to_string();
        std::thread::spawn(move || {
            let lower = path.to_ascii_lowercase();
            let r = std::fs::read_to_string(&path)
                .map_err(|e| e.to_string())
                .and_then(|t| {
                    if lower.ends_with(".ifcx") {
                        meter.tick(0, 0, "Reading the file");
                        creabim_ifc::import_ifcx(&t)
                    } else {
                        creabim_ifc::import_ifc_with(&t, &|n, tt, s| meter.tick(n, tt, s))
                    }
                    .map_err(|e| e.to_string())
                })
                .map(|r| Outcome::Imported(Box::new(r)));
            let _ = tx.send(r);
        });
        self.mark = Some(card);
    }

    /// File › Import report: the last import's card again, as it ended.
    pub(super) fn reopen_import_card(&mut self) {
        let Some(r) = &self.import_report else { return };
        let mut card = MarkCard::new(CardKind::Import, Phase::Done);
        card.file = r.file.clone();
        self.mark = Some(card);
    }

    pub(super) fn close_mark(&mut self) {
        self.mark = None;
        self.hide_mark_card();
    }

    /// Each frame: take a finished job's answer, keep the card drawn and
    /// the project behind it waiting.
    pub(super) fn mark_take_flags(&mut self) {
        if !self.mark_cards() {
            return;
        }
        // the old windows' flags open the cards instead
        if std::mem::take(&mut self.ifc_dialog) {
            self.open_export_card("export.ifc4");
        }
        if std::mem::take(&mut self.sheets_export_open) {
            self.open_export_card(SHEETS_FMT);
        }
        if self.import_report.as_ref().is_some_and(|r| r.open) {
            if let Some(r) = &mut self.import_report {
                r.open = false;
            }
            if self.mark.is_none() {
                self.reopen_import_card();
            }
        }
    }

    pub(super) fn mark_card(&mut self, ctx: &egui::Context) {
        if !self.mark_cards() {
            return;
        }
        let Some(card) = &mut self.mark else {
            return;
        };
        if card.phase == Phase::Working {
            let got = card.rx.as_ref().and_then(|rx| rx.try_recv().ok());
            match got {
                Some(r) => {
                    card.secs = card.started.elapsed().as_secs_f32();
                    card.rx = None;
                    self.mark_finished(r);
                }
                None => ctx.request_repaint_after(std::time::Duration::from_millis(80)),
            }
        }
        if self.mark.is_none() {
            return;
        }
        self.show_mark_card(ctx);
        if let Some(web) = &mut self.shell.web {
            web.shade(true);
        }
        crate::app::shell::card_shade(ctx, "mark-card");
        // the keys, while the views have the keyboard
        if let Some(key) = self.mark_key.take() {
            self.mark_do(ctx, key, "");
        }
    }

    /// Enter and Esc for the card, out of the frame's input before the
    /// views see them.
    pub(super) fn mark_keys(&mut self, raw: &mut egui::RawInput) {
        if self.mark.is_none() || !self.mark_cards() {
            return;
        }
        let mut got = None;
        raw.events.retain(|e| match e {
            egui::Event::Key {
                key: k @ (egui::Key::Enter | egui::Key::Escape),
                pressed: true,
                ..
            } => {
                got = Some(if *k == egui::Key::Enter {
                    "enter"
                } else {
                    "esc"
                });
                false
            }
            _ => true,
        });
        if got.is_some() {
            self.mark_key = got;
        }
    }

    fn mark_finished(&mut self, r: Result<Outcome, String>) {
        match r {
            Ok(Outcome::Wrote(w)) => {
                let Some(card) = &mut self.mark else { return };
                let what = match &card.kind {
                    CardKind::Export(w) => w.clone(),
                    CardKind::Import => String::new(),
                };
                let path = w.path.display().to_string();
                self.last_export_dir = if w.files > 1 {
                    Some(w.path.clone())
                } else {
                    std::path::absolute(&w.path)
                        .ok()
                        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                };
                self.status = match what.as_str() {
                    "export.glb" => format!("Exported {path}. In Blender: File > Import > glTF, or use the CreaBIM add-on."),
                    SHEETS_FMT => format!("Exported {} to {path}", plural(w.count, "sheet")),
                    _ => format!("Exported {path}"),
                };
                self.last_export = Some(self.status.clone());
                crate::actlog::line(format!("export: {} in {:.1} s", self.status, card.secs));
                card.written = Some(w);
                card.phase = Phase::Done;
            }
            Ok(Outcome::Imported(imported)) => {
                let Some(card) = &mut self.mark else { return };
                card.phase = Phase::Done;
                let path = card.path.clone();
                self.apply_import(&path, *imported);
            }
            Err(e) if e == "stopped" => self.close_mark(),
            Err(e) => {
                let Some(card) = &mut self.mark else { return };
                let doing = if card.kind == CardKind::Import {
                    "open"
                } else {
                    "export"
                };
                self.status = format!("Couldn't {doing} {}: {e}", card.file);
                card.phase = Phase::Failed(e);
            }
        }
    }

    /// A button or key on the card.
    pub(super) fn mark_do(&mut self, ctx: &egui::Context, what: &str, value: &str) {
        let Some(card) = &mut self.mark else { return };
        let phase = card.phase.clone();
        match (what, &phase) {
            ("ifcbody", Phase::Choose) => {
                if ["modelled", "joined", "layers"].contains(&value) {
                    self.shell.settings.ifc_body = value.into();
                    self.shell.settings.store();
                }
            }
            ("scope", Phase::Choose) => {
                card.scope = match value {
                    "this" => ExportScope::This,
                    "chosen" => ExportScope::Chosen,
                    _ => ExportScope::All,
                };
            }
            ("each", Phase::Choose) => card.each = value == "1",
            ("white", Phase::Choose) => card.white = value == "1",
            ("go" | "enter", Phase::Choose) => self.start_export(),
            ("cancel" | "esc", Phase::Choose) => self.close_mark(),
            // Stop: the job writes nothing; the card goes at once
            ("stop" | "esc", Phase::Working) => {
                card.meter.stop.store(true, Ordering::Relaxed);
                let import = card.kind == CardKind::Import;
                self.status = if import {
                    "Stopped opening the file. Your project is as it was.".into()
                } else {
                    "Export stopped: nothing was written.".into()
                };
                crate::actlog::line("export: stopped");
                self.close_mark();
            }
            ("done" | "enter" | "esc", Phase::Done | Phase::Failed(_)) => self.close_mark(),
            ("folder", Phase::Done) => {
                let r = match &card.written {
                    Some(w) if w.files > 1 => crate::files::open_folder(&w.path),
                    Some(w) => crate::files::show_in_folder(&w.path),
                    None => Ok(()),
                };
                self.report(r, "Opened the folder.");
            }
            ("copy", Phase::Done) => {
                if let Some(r) = &self.import_report {
                    ctx.copy_text(r.text());
                    self.status = "Import report copied.".into();
                }
            }
            ("select", Phase::Done) => {
                if let Some(r) = &self.import_report {
                    let ids = noted_ids(&r.details, self.doc.model());
                    let n = ids.len();
                    self.selection.extend(ids, false);
                    self.status = format!(
                        "Selected {} named in the import's notes.",
                        plural(n, "element")
                    );
                    self.close_mark();
                }
            }
            ("again", Phase::Failed(_)) => {
                if let CardKind::Export(_) = card.kind {
                    card.phase = Phase::Choose;
                    self.start_export_if_no_choices();
                }
            }
            _ => {}
        }
    }

    /// Ctrl+P on a sheet: straight to where it goes.
    pub(super) fn mark_do_start(&mut self) {
        self.start_export();
    }

    fn start_export_if_no_choices(&mut self) {
        let Some(MarkCard {
            kind: CardKind::Export(w),
            ..
        }) = &self.mark
        else {
            return;
        };
        if !matches!(w.as_str(), "export.ifc4" | SHEETS_FMT) {
            self.start_export();
        }
    }

    /// The card's state for its page.
    pub(super) fn mark_state_json(&self) -> serde_json::Value {
        let Some(card) = &self.mark else {
            return serde_json::Value::Null;
        };
        let (n, total, step) = card.meter.read();
        let phase = match &card.phase {
            Phase::Choose => "choose",
            Phase::Working => "working",
            Phase::Done => "done",
            Phase::Failed(_) => "failed",
        };
        let error = match &card.phase {
            Phase::Failed(e) => e.clone(),
            _ => String::new(),
        };
        let mut s = json!({
            "phase": phase,
            "file": card.file,
            "path": card.path,
            "n": n,
            "total": total,
            "step": step,
            "error": error,
        });
        match &card.kind {
            CardKind::Export(what) => {
                s["kind"] = "export".into();
                s["what"] = what.clone().into();
                s["name"] = export_name(what).into();
                s["unit"] = if what == SHEETS_FMT {
                    "sheet"
                } else {
                    "element"
                }
                .into();
                if what == "export.ifc4" {
                    s["choices"] = json!({
                        "title": "How walls, slabs and roofs go out",
                        "key": "ifcbody",
                        "on": self.shell.settings.ifc_body,
                        "rows": [
                            {"id": "modelled", "title": "As modelled elements (recommended)", "line": "Each wall and slab is one IfcWall or IfcSlab, its layers carried as its layer set. Every BIM program reads it, and it comes back into CreaBIM as it went out."},
                            {"id": "joined", "title": "Joined shapes, as drawn", "line": "Bodies cut where walls run through slabs, as CreaBIM shows them: for coordination and clash checks. A cut slab may come back as a generic shape."},
                            {"id": "layers", "title": "Layers as separate elements", "line": "The structure stays the IfcWall or IfcSlab; finishes, insulation and membranes become IfcCovering, each covering its element. For trades and quantities by layer."},
                        ],
                        "note": "Doors, windows, openings and everything else go out the same way in all three.",
                    });
                }
                if what == SHEETS_FMT {
                    let all = sheets::sheets(self.doc.model()).len();
                    let chosen = self.chosen_sheets().len();
                    let this = self.active_view.is_some_and(|id| self.is_sheet(id));
                    let tinted = papers_tinted(self.doc.model());
                    let on = match card.scope {
                        ExportScope::This if this => "this",
                        ExportScope::Chosen if chosen > 0 => "chosen",
                        _ => "all",
                    };
                    s["choices"] = json!({
                        "title": "Which sheets",
                        "key": "scope",
                        "on": on,
                        "rows": [
                            {"id": "all", "title": format!("All sheets ({all})"), "line": if all > 0 { "Every sheet, a page each in sheet-number order." } else { "This project has no sheets yet." }, "off": all == 0},
                            {"id": "chosen", "title": format!("Chosen sheets ({chosen})"), "line": if chosen > 0 { "The sheets selected in the project browser." } else { "Select sheets in the project browser first." }, "off": chosen == 0},
                            {"id": "this", "title": "This sheet", "line": if this { "The sheet in front." } else { "Open a sheet first." }, "off": !this},
                        ],
                        "each": card.each,
                        "note": if tinted {
                            "At the sheets' sizes. Print on white keeps the drawing's colours but leaves the paper's tint and texture off."
                        } else {
                            "As printed: white paper, at the sheets' sizes."
                        },
                    });
                    // the paper choice, only when a master has paper of
                    // its own (pens stage 7)
                    if tinted {
                        s["choices"]["white"] = card.white.into();
                    }
                }
                if let Some(w) = &card.written {
                    s["size"] = human_size(w.bytes).into();
                    s["count"] = w.count.into();
                    s["unit"] = w.unit.into();
                    s["files"] = w.files.into();
                    s["secs"] = format!("{:.1} s", card.secs.max(0.1)).into();
                    s["path"] = w.path.display().to_string().into();
                }
            }
            CardKind::Import => {
                s["kind"] = "import".into();
                if let (Phase::Done, Some(r)) = (&card.phase, &self.import_report) {
                    let mut counts: Vec<(String, usize, usize)> = r
                        .counts
                        .iter()
                        .filter(|c| is_element(&c.0))
                        .cloned()
                        .collect();
                    let all: usize = counts.iter().map(|c| c.1).sum();
                    let meshes: usize = counts.iter().map(|c| c.2).sum();
                    s["file"] = r.file.clone().into();
                    let took = r
                        .secs
                        .map(|s| format!(" · {:.1} s", s.max(0.1)))
                        .unwrap_or_default();
                    s["line"] = if meshes == 0 {
                        format!(
                            "{}{took} · {} came in as CreaBIM elements.",
                            r.schema,
                            plural(all, "element")
                        )
                    } else {
                        format!(
                            "{}{took} · {} came in as CreaBIM elements, {} as meshes.",
                            r.schema,
                            plural(all - meshes, "element"),
                            meshes
                        )
                    }
                    .into();
                    s["good"] = r.summary.is_empty().into();
                    if let Some(first) = r.summary.first() {
                        s["flag"] = note_sentence(first).into();
                        s["more"] = (r.summary.len() - 1).into();
                        s["select"] = (!noted_ids(&r.details, self.doc.model()).is_empty()).into();
                    }
                    counts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
                    let chips: Vec<serde_json::Value> = counts
                        .iter()
                        .take(5)
                        .map(|(c, n, _)| json!({"n": n, "kind": kind_name(c, *n)}))
                        .collect();
                    s["chips"] = chips.into();
                    s["morekinds"] = counts.len().saturating_sub(5).into();
                    s["details"] = r.details.clone().into();
                    s["notes"] = r.summary.clone().into();
                    // the whole report: each note over the elements it names
                    s["groups"] = r
                        .grouped()
                        .into_iter()
                        .map(|(note, items)| json!({"note": note.unwrap_or("Other elements"), "items": items}))
                        .collect::<Vec<_>>()
                        .into();
                }
                if card.phase == Phase::Working {
                    s["secs"] =
                        format!("{:.0} s", card.started.elapsed().as_secs_f32().floor()).into();
                }
            }
        }
        s
    }

    /// An IFC file read: it becomes the project, with its report.
    pub(super) fn apply_import(&mut self, path: &str, imported: creabim_ifc::ImportResult) {
        let n = imported.model.len();
        let warnings = imported.warnings.len();
        let mut report = importui::ImportReport::new(&file_label(path), &imported);
        // how long it took, when it was opened through the card
        report.secs = self
            .mark
            .as_ref()
            .filter(|c| c.kind == CardKind::Import && c.secs > 0.0)
            .map(|c| c.secs);
        if let Some(s) = report.secs {
            crate::actlog::line(format!("IFC import: opened in {s:.1} s"));
        }
        // in the Activity log too: the released app has no console
        // window to print them in (MU59)
        for w in &imported.warnings {
            eprintln!("IFC import: {w}");
            crate::actlog::line(format!("IFC import: {w}"));
        }
        for d in &imported.details {
            eprintln!("IFC import:   {d}");
            crate::actlog::line(format!("IFC import:   {d}"));
        }
        // how faithfully it came in, after the notes, where it is read
        // first when the log is opened at its end (owner 2026-11-23)
        for f in &imported.fidelity {
            eprintln!("IFC fidelity: {f}");
            crate::actlog::line(format!("IFC fidelity: {f}"));
        }
        // the card shows it; the window doesn't open over it
        if self.mark_cards() {
            report.open = false;
        }
        self.import_report = Some(report);
        self.load_model(imported.model);
        self.watch = Default::default();
        self.recent.add(path);
        self.path = with_extension(path, "creabim");
        self.status = format!(
            "Opened {} ({}): {n} elements{}. Save keeps it as {}.",
            file_label(path),
            imported.schema,
            if warnings > 0 {
                format!(", {warnings} notes (File › Import report)")
            } else {
                String::new()
            },
            file_label(&self.path)
        );
    }
}

/// Write an export's bytes, unless it was stopped.
fn write_one(
    m: &Meter,
    out: &str,
    bytes: &[u8],
    count: usize,
    unit: &'static str,
) -> Result<Outcome, String> {
    if m.stopped() {
        return Err("stopped".into());
    }
    m.tick(
        m.total.load(Ordering::Relaxed),
        m.total.load(Ordering::Relaxed),
        "Writing the file",
    );
    std::fs::write(out, bytes).map_err(|e| e.to_string())?;
    Ok(Outcome::Wrote(Written {
        path: out.into(),
        bytes: bytes.len() as u64,
        count,
        unit,
        files: 1,
    }))
}

/// Does any sheet sit on paper other than plain white (its master's
/// colour or texture)?
pub(super) fn papers_tinted(model: &Model) -> bool {
    sheets::sheets(model)
        .iter()
        .any(|(_, s)| !creabim_geom::sheetpaper::sheet_paper(model, s, false).is_plain())
}

/// A sheet's file name: its number and name, without the characters a
/// file name can't have.
pub(super) fn sheet_file_name(model: &Model, id: ElementId) -> String {
    let n = sheets::sheet_of(model, id)
        .map(|s| s.number.clone())
        .unwrap_or_default();
    let t = model.get(id).map(|e| e.name.clone()).unwrap_or_default();
    format!("{n} {t}")
        .trim()
        .chars()
        .map(|c| if "\\/:*?\"<>|".contains(c) { '-' } else { c })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_are_said_plainly() {
        assert_eq!(
            note_sentence("walls whose clipping was ignored to keep them native walls (×14)"),
            "14 walls were clipped by a roof in the file. They came in as plain walls: check their tops."
        );
        assert_eq!(
            note_sentence("units were read as millimetres"),
            "Units were read as millimetres."
        );
        assert_eq!(
            note_sentence("elements kept as meshes (×3)"),
            "Elements kept as meshes: 3."
        );
    }

    #[test]
    fn noted_elements_are_found_by_their_global_id() {
        let m = creabim_core::sample::sample_house();
        let wall = m
            .iter()
            .find(|e| matches!(e.kind, ElementKind::Wall(_)))
            .unwrap();
        let details = vec![
            format!(
                "IfcWall 'GF North' ({}) is clipped (by a roof or a sloped cut)",
                wall.id.ifc_guid()
            ),
            "IfcWall 'Gone' (0000000000000000000000) is clipped".to_string(),
        ];
        assert_eq!(noted_ids(&details, &m), vec![wall.id]);
    }

    #[test]
    fn only_built_things_count_as_elements() {
        assert!(is_element("IfcWall") && is_element("IfcFurniture"));
        assert!(!is_element("IfcMaterial") && !is_element("IfcWallType"));
        assert!(!is_element("IfcBuildingStorey") && !is_element("IfcMaterialLayerSet"));
    }

    #[test]
    fn chips_name_kinds_for_people() {
        assert_eq!(kind_name("IfcWall", 24), "walls");
        assert_eq!(kind_name("IfcWindow", 1), "window");
        assert_eq!(kind_name("IfcFurniture", 3), "furniture");
        assert_eq!(
            kind_name("IfcBuildingElementProxy", 2),
            "BuildingElementProxy"
        );
        assert_eq!(human_size(4_404_019), "4.2 MB");
    }

    #[test]
    fn an_export_runs_off_the_window_and_writes_its_file() {
        let model = creabim_core::sample::sample_house();
        let m = Meter::default();
        let out = std::env::temp_dir().join("creabim-markcard-test.ifc");
        let text = creabim_ifc::export_ifc4_with(
            &model,
            &creabim_ifc::ExportOptions::for_model(&model, "t"),
            &|n, t, s| m.tick(n, t, s),
        );
        let (n, total, step) = m.read();
        assert!(
            total > 0 && n == total,
            "the bar reached its end: {n} of {total}"
        );
        assert_eq!(step, "Writing the file");
        let r = write_one(&m, out.to_str().unwrap(), text.as_bytes(), 3, "element");
        assert!(matches!(r, Ok(Outcome::Wrote(Written { files: 1, .. }))));
        assert!(out.exists());
        // stopped, nothing is written
        let _ = std::fs::remove_file(&out);
        m.stop.store(true, Ordering::Relaxed);
        let r = write_one(&m, out.to_str().unwrap(), text.as_bytes(), 3, "element");
        assert!(matches!(r, Err(e) if e == "stopped"));
        assert!(!out.exists());
    }
}
