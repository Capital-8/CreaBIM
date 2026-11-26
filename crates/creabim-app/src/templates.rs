//! Templates: a project kept as a start for new ones (Templates in the
//! app, REV05). A template is an ordinary project file ending
//! `.creabim-template`; it always opens as a new, unsaved project, so the
//! template itself is never changed. The user's own are kept in one
//! folder, `Documents/CreaBIM/Templates`.

use super::CreaBimApp;
use creabim_core::{Document, ElementKind, Model, Transaction};
use std::path::{Path, PathBuf};

/// A template file's ending.
pub(crate) const EXT: &str = "creabim-template";

/// One of the user's templates: its name (the file's, without its
/// ending), its file, and when it was saved.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Template {
    pub name: String,
    pub path: PathBuf,
    pub when: Option<std::time::SystemTime>,
}

/// Where the user's templates are kept: `Documents/CreaBIM/Templates`
/// (the Documents folder as this machine has it, in OneDrive when it has
/// been moved there), else beside the app's settings.
pub(crate) fn templates_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("CREABIM_TEMPLATES") {
        return PathBuf::from(d);
    }
    match documents_dir().or_else(crate::files::settings_dir) {
        Some(d) => d.join("CreaBIM").join("Templates"),
        None => PathBuf::from("Templates"),
    }
}

/// The user's Documents folder as Windows has it, which is in OneDrive
/// when it has been moved there (the folder under the user's own may
/// still exist, empty and unseen). Asked once.
fn documents_dir() -> Option<PathBuf> {
    static DOCS: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
    DOCS.get_or_init(|| {
        let home = std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from);
        shell_documents(home.as_deref())
            .or_else(|| home.map(|h| h.join("Documents")))
            .filter(|d| d.is_dir())
    })
    .clone()
}

/// Windows' own record of where Documents is.
#[cfg(windows)]
fn shell_documents(home: Option<&Path>) -> Option<PathBuf> {
    use std::os::windows::process::CommandExt;
    let out = std::process::Command::new("reg")
        .args([
            "query",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\User Shell Folders",
            "/v",
            "Personal",
        ])
        // no console window flashes up
        .creation_flags(0x0800_0000)
        .output()
        .ok()?;
    documents_from(&String::from_utf8_lossy(&out.stdout), home)
}

#[cfg(not(windows))]
fn shell_documents(_home: Option<&Path>) -> Option<PathBuf> {
    None
}

/// The folder in the registry's answer: the text after the value's type,
/// with `%USERPROFILE%` filled in.
#[cfg_attr(not(windows), allow(dead_code))]
fn documents_from(answer: &str, home: Option<&Path>) -> Option<PathBuf> {
    let line = answer
        .lines()
        .find(|l| l.trim_start().starts_with("Personal"))?;
    let value = line
        .split_once("REG_EXPAND_SZ")
        .or_else(|| line.split_once("REG_SZ"))?
        .1
        .trim();
    let value = match home {
        Some(h) => value.replace("%USERPROFILE%", &h.to_string_lossy()),
        None => value.to_string(),
    };
    (!value.is_empty() && !value.contains('%')).then(|| PathBuf::from(value))
}

/// The templates in `dir`, by name.
pub(crate) fn list(dir: &Path) -> Vec<Template> {
    let mut out: Vec<Template> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|f| {
            let path = f.path();
            path.extension()
                .is_some_and(|x| x.eq_ignore_ascii_case(EXT))
                .then(|| Template {
                    name: path
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string(),
                    when: f.metadata().and_then(|m| m.modified()).ok(),
                    path,
                })
        })
        .collect();
    out.sort_by_key(|a| a.name.to_lowercase());
    out
}

/// The parts a template can keep (Save as template's ticks): a key, its
/// name and what it holds. The building is `BUILDING`, the job's details
/// `DETAILS`.
pub(crate) const PARTS: [(&str, &str, &str); 7] = [
    (
        "sheets",
        "Sheets and masters",
        "sheets, their layout and title blocks",
    ),
    (
        "graphic",
        "Graphic styles",
        "pens, palettes, hatches, symbols",
    ),
    ("annotation", "Annotation styles", "text, dimensions, tags"),
    (
        "types",
        "Types and materials",
        "wall, door and window types, assemblies, materials",
    ),
    (
        "views",
        "Grids and views",
        "with their view settings; levels are always kept",
    ),
    (
        "building",
        "The building",
        "walls, slabs, rooms, furniture, with their dimensions and tags",
    ),
    (
        "details",
        "Project details",
        "the job's name and number, client, address, issue date",
    ),
];
pub(crate) const BUILDING: usize = 5;
pub(crate) const DETAILS: usize = 6;

/// The look only: every part but the building and the job's details.
pub(crate) fn look_only() -> [bool; 7] {
    [true, true, true, true, true, false, false]
}

/// The part an element belongs to; none for what every project keeps
/// (its levels, its units, the project itself).
fn part_of(kind: &ElementKind) -> Option<usize> {
    use ElementKind as K;
    Some(match kind {
        K::Sheet(_) | K::Viewport(_) | K::PaperMark(_) | K::SheetTemplate(_) => 0,
        K::StylePack(_) | K::MarkRules(_) => 1,
        K::TextStyle(_) => 2,
        K::ElementType(_) | K::Assembly(_) | K::Material(_) => 3,
        K::Grid(_) | K::GridGroup(_) | K::View(_) | K::CropBox(_) => 4,
        K::Level(_) | K::Units(_) | K::Project(_) => return None,
        // everything built, and what is drawn on it
        _ => BUILDING,
    })
}

/// How many elements of `model` each part holds.
pub(crate) fn part_counts(model: &Model) -> [usize; 7] {
    let mut n = [0; 7];
    for e in model.iter() {
        if let Some(i) = part_of(&e.kind) {
            n[i] += 1;
        }
    }
    n
}

/// `model` as a template keeps it: the parts ticked in `keep` ([`PARTS`]),
/// and no undo history. A building needs its views and types, so they
/// stay with it. Without the job's details, its name and number, the
/// client, the address and the issue date are left blank.
pub(crate) fn as_template_with(model: &Model, mut keep: [bool; 7]) -> Document {
    if keep[BUILDING] {
        keep[3] = true;
        keep[4] = true;
    }
    let mut doc = Document::from_model(model.clone());
    let gone: Vec<creabim_core::ElementId> = doc
        .model()
        .iter()
        .filter(|e| part_of(&e.kind).is_some_and(|i| !keep[i]))
        .map(|e| e.id)
        .collect();
    if !gone.is_empty() {
        let tx = Transaction::new("Template").remove_all_cascade(doc.model(), &gone);
        let _ = doc.commit(tx);
    }
    if keep[DETAILS] {
        return Document::from_model(doc.model().clone());
    }
    if let Some((id, _)) = doc.model().project_info() {
        if let Some(before) = doc.model().get(id).cloned() {
            let mut after = before.clone();
            if let ElementKind::Project(p) = &mut after.kind {
                p.name.clear();
                p.number.clear();
                p.client.clear();
                p.address.clear();
                p.issue_date.clear();
            }
            if after != before {
                let _ = doc.commit(Transaction::new("Template").replace(before, after));
            }
        }
    }
    // a file with no way back to the job it came from
    Document::from_model(doc.model().clone())
}

/// A name a file can take: no path marks, never empty.
pub(crate) fn safe_name(name: &str) -> String {
    let s: String = name
        .trim()
        .chars()
        .map(|c| {
            if "\\/:*?\"<>|".contains(c) || c.is_control() {
                ' '
            } else {
                c
            }
        })
        .collect();
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if s.is_empty() {
        "Template".into()
    } else {
        s
    }
}

/// Bytes as base64 text, for a picture kept as a data URL.
pub(crate) fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if c.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// How long ago `when` was, as the lists say it: "today", "yesterday",
/// "5 days ago", "3 months ago". A date read off the clock would be the
/// universal one, a day out in the evening or the morning.
pub(crate) fn ago_label(now: std::time::SystemTime, when: std::time::SystemTime) -> String {
    let days = now.duration_since(when).map_or(0, |d| d.as_secs() / 86_400);
    match days {
        0 => "today".into(),
        1 => "yesterday".into(),
        2..=59 => format!("{days} days ago"),
        60..=729 => format!("{} months ago", days / 30),
        _ => format!("{} years ago", days / 365),
    }
}

/// A date as the lists show it: "18 Nov 2026".
#[cfg(test)]
pub(crate) fn day_label(t: std::time::SystemTime) -> String {
    let secs = t
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    // days since 1970 to a civil date (Howard Hinnant's algorithm)
    let z = secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    format!("{d} {} {y}", MONTHS[(m - 1) as usize])
}

impl CreaBimApp {
    /// File, Save as template…: ask for its name, starting in the
    /// templates folder, and keep this project there as a start.
    pub(crate) fn save_as_template(&mut self) {
        let dir = templates_dir();
        if let Err(e) = std::fs::create_dir_all(&dir) {
            self.status = format!("Could not make the templates folder {}: {e}", dir.display());
            return;
        }
        let stem = Path::new(&self.path)
            .file_stem()
            .map_or("Template".to_string(), |s| s.to_string_lossy().to_string());
        let start = dir.join(format!("{}.{EXT}", safe_name(&stem)));
        if let Some(path) =
            crate::files::pick_save(&start.to_string_lossy(), "Save as template", EXT)
        {
            self.save_template_to(Path::new(&path));
        }
    }

    /// Save as template's card: keep the ticked parts under `name`, in My
    /// templates (never over one there: a name taken gets a number) or in
    /// a file asked for.
    pub(crate) fn save_template_card(&mut self, name: &str, keep: [bool; 7], to_file: bool) {
        let dir = templates_dir();
        if let Err(e) = std::fs::create_dir_all(&dir) {
            self.status = format!("Could not make the templates folder {}: {e}", dir.display());
            return;
        }
        let name = safe_name(name);
        let path = if to_file {
            let start = dir.join(format!("{name}.{EXT}"));
            match crate::files::pick_save(&start.to_string_lossy(), "Save as template", EXT) {
                Some(p) => PathBuf::from(p),
                None => return,
            }
        } else {
            let mut path = dir.join(format!("{name}.{EXT}"));
            let mut n = 2;
            while path.exists() {
                path = dir.join(format!("{name} {n}.{EXT}"));
                n += 1;
            }
            path
        };
        self.save_template_parts(&path, keep);
    }

    /// Keep this project as the template file `path`. The project itself,
    /// its file and its history are left as they are.
    pub(crate) fn save_template_to(&mut self, path: &Path) {
        let mut keep = [true; 7];
        keep[DETAILS] = false;
        self.save_template_parts(path, keep);
    }

    fn save_template_parts(&mut self, path: &Path, keep: [bool; 7]) {
        let path = path.with_extension(EXT);
        let doc = as_template_with(self.doc.model(), keep);
        self.status = match creabim_store::save(&path, &doc) {
            Ok(()) => {
                // its tile's picture: the 3D view as it stands, when the
                // building is kept
                if let Some(p) = keep[BUILDING].then(|| self.template_picture()).flatten() {
                    let _ = creabim_store::set_picture(&path, &p);
                }
                let name = path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let mine = path.parent().is_some_and(|p| p == templates_dir());
                format!(
                    "Saved the template \"{name}\"{}. It keeps {}{}.",
                    if mine {
                        ": it is under My templates in New project"
                    } else {
                        ""
                    },
                    if keep[BUILDING] {
                        "the look and the building"
                    } else {
                        "the look only"
                    },
                    if keep[DETAILS] {
                        ", with the job's details"
                    } else {
                        ", not the job's name, client or address"
                    }
                )
            }
            Err(e) => format!("Could not save the template: {e}"),
        };
    }

    /// A small picture of the building as the 3D view stands, as a data
    /// URL: a template's tile. None with nothing to show.
    pub(crate) fn template_picture(&self) -> Option<String> {
        use creabim_render::job::{self, Mode, Request};
        use std::sync::atomic::AtomicBool;
        use std::sync::Arc;
        if self.scene.iter().all(|i| i.ghost) {
            return None;
        }
        let (w, h) = (480usize, 300usize);
        let mut camera = creabim_render::util::Camera::from_view_proj(
            self.camera.view_proj(w as f32 / h as f32),
            self.camera.eye(),
            self.camera.target,
        );
        camera.fov = 45.0;
        let req = Request {
            scene: Arc::new(creabim_render::scene::Scene::from_items_on(
                &self.scene,
                Some((-0.15, [0.36, 0.49, 0.25])),
            )),
            camera,
            sky: creabim_render::sky::Sky::new(creabim_render::sky::sun_direction(
                48.85, 172.0, 10.0, 0.0,
            )),
            width: w,
            height: h,
            mode: Mode::Look(creabim_render::look::Look::Realtime { traced: false }),
            exposure: 1.0,
        };
        let mut last = None;
        job::run(&req, Arc::new(AtomicBool::new(false)), |p| {
            if p.done {
                last = Some(p.image);
            }
        });
        let img = last?;
        let tmp = std::env::temp_dir().join(format!(
            "creabim-template-picture-{}.png",
            std::process::id()
        ));
        img.save_png(&tmp).ok()?;
        let bytes = std::fs::read(&tmp).ok();
        let _ = std::fs::remove_file(&tmp);
        Some(format!("data:image/png;base64,{}", base64(&bytes?)))
    }

    /// Start a new, unsaved project from the template file `path`.
    pub(crate) fn open_template(&mut self, path: &str) {
        let name = Path::new(path)
            .file_stem()
            .map_or("Template".to_string(), |s| s.to_string_lossy().to_string());
        match creabim_store::load(path) {
            Ok(doc) => {
                // a name of its own and no file yet: Save asks where
                self.path = format!("{name}.creabim");
                self.load_model(doc.model().clone());
                self.status = format!(
                    "A new project from the template \"{name}\". The template itself is unchanged."
                );
            }
            Err(e) => self.status = format!("Could not open the template {path}: {e}"),
        }
    }

    /// Bring the look of the template file `path` into this project, as
    /// one step that can be undone: its graphic styles, its annotation
    /// styles and its masters. One of the same name here takes the
    /// template's content (so what uses it follows); the others are added.
    /// Sheets, views, types and the building are not touched.
    pub(crate) fn apply_template_look(&mut self, path: &str) {
        let from = match creabim_store::load(path) {
            Ok(d) => d,
            Err(e) => {
                self.status = format!("Could not open the template {path}: {e}");
                return;
            }
        };
        let look = |k: &ElementKind| {
            matches!(
                k,
                ElementKind::StylePack(_)
                    | ElementKind::MarkRules(_)
                    | ElementKind::TextStyle(_)
                    | ElementKind::SheetTemplate(_)
            )
        };
        let model = self.doc.model();
        let mut tx = Transaction::new("Apply a template's look");
        let (mut changed, mut added) = (0, 0);
        for t in from.model().iter().filter(|e| look(&e.kind)) {
            let same = model.iter().find(|e| {
                std::mem::discriminant(&e.kind) == std::mem::discriminant(&t.kind)
                    && e.name == t.name
            });
            match same {
                Some(e) => {
                    let mut after = e.clone();
                    after.kind = t.kind.clone();
                    if after != *e {
                        tx = tx.replace(e.clone(), after);
                        changed += 1;
                    }
                }
                None => {
                    let mut new = t.clone();
                    if model.get(new.id).is_some() {
                        new.id = creabim_core::ElementId::new();
                    }
                    tx = tx.insert(new);
                    added += 1;
                }
            }
        }
        let name = Path::new(path)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        if changed + added == 0 {
            self.status =
                format!("This project already has the look of \"{name}\": nothing changed.");
            return;
        }
        let r = self.doc.commit(tx);
        self.drawings.clear();
        self.status = match r {
            Ok(_) => format!(
                "The look of \"{name}\" is in this project: {changed} of its styles and masters changed, {added} added. Undo takes it back."
            ),
            Err(e) => format!("Could not apply the template's look: {e:?}"),
        };
    }

    /// Start a new project with one of CreaBIM's sets. `arg` is the set's
    /// key, then what New project chose, each after a bar: "us" or
    /// "metric", "starter" (this version's starter house), "maple" (the
    /// starter of 1.1) or "empty", "mark" or "plain". Left out, a choice
    /// is metric, empty, with the mark.
    fn new_from_set(&mut self, ctx: &eframe::egui::Context, arg: &str) {
        let mut parts = arg.split('|');
        let key = parts.next().unwrap_or_default();
        let rest: Vec<&str> = parts.collect();
        let (us, mark) = (rest.contains(&"us"), !rest.contains(&"plain"));
        let maple = rest.contains(&"maple");
        let starter = maple || rest.contains(&"starter");
        let house = if maple {
            "Maple Lane House"
        } else {
            "Ridgeline House"
        };
        let Some(set) = sets().iter().find(|s| s.key == key) else {
            self.status = format!("There is no template set named \"{key}\".");
            return;
        };
        // the start it is put onto: this version's starter house (its
        // hero views sheet left out: those pictures sit in its own title
        // block), the starter of 1.1, or an empty project in the
        // standard's units
        let base = if maple {
            creabim_core::starter::starter_house()
        } else if starter {
            creabim_core::ridgeline::ridgeline_house()
        } else {
            self.run_action(
                ctx,
                if us {
                    "new.empty:us"
                } else {
                    "new.empty:metric"
                },
            );
            self.doc.model().clone()
        };
        let made = match creabim_geom::templateset::apply(set.data, base, us, !starter, mark) {
            Ok(m) => m,
            Err(e) => {
                self.status = format!("Could not start from the {} set: {e}", set.name);
                return;
            }
        };
        // loaded whole, so the set is how the project starts and not a
        // step to undo
        self.load_model(made.model);
        self.path = super::shell::new_path(if starter { house } else { "Untitled" });
        self.status = format!(
            "New project with the {} set{}{}. Its title blocks are under Sheets › Masters.",
            set.name,
            if starter {
                format!(", on {house}")
            } else {
                String::new()
            },
            if us { ", in US sizes" } else { "" },
        );
    }

    /// The templates' actions. False when `head` is not one.
    pub(crate) fn templates_action(
        &mut self,
        ctx: &eframe::egui::Context,
        head: &str,
        arg: &str,
    ) -> bool {
        match head {
            "new.template" => self.open_template(arg),
            "new.set" => self.new_from_set(ctx, arg),
            "template.save" => {
                // the card when the app has its web panels, else a plain
                // file dialog
                if !self.open_template_card(ctx) {
                    self.save_as_template();
                }
            }
            "template.apply" => {
                let dir = templates_dir();
                let start = dir.join(format!("template.{EXT}"));
                if let Some(p) = crate::files::pick_template(&start) {
                    self.apply_template_look(&p);
                }
            }
            "template.folder" => {
                let dir = templates_dir();
                let _ = std::fs::create_dir_all(&dir);
                self.status = match crate::files::open_folder(&dir) {
                    Ok(()) => format!("Your templates are in {}.", dir.display()),
                    Err(e) => e,
                };
            }
            "templates.more" => {
                ctx.open_url(eframe::egui::OpenUrl::new_tab(MORE));
            }
            _ => return false,
        }
        true
    }
}

/// One of CreaBIM's template sets, as New project lists it.
pub(crate) struct SetTile {
    /// Its key in an action ("studio").
    pub key: &'static str,
    pub name: String,
    /// What it is, in a sentence.
    pub line: String,
    /// A picture of its A3 sheet, a data URL.
    pub picture: String,
    /// Its sheet as New project shows it for the choices there (MU102):
    /// A3 or ARCH B, with or without CreaBIM's mark, keyed
    /// "metric|mark", "metric|plain", "us|mark", "us|plain".
    pub pictures: Vec<(&'static str, String)>,
    /// The set itself (Template sets REV01): its masters for every paper,
    /// its palette, font and pens.
    pub data: &'static str,
}

/// The sets that ship, in their order (Studio first). Read once.
pub(crate) fn sets() -> &'static [SetTile] {
    static SETS: std::sync::OnceLock<Vec<SetTile>> = std::sync::OnceLock::new();
    SETS.get_or_init(|| {
        const DATA: [(&str, &str); 10] = [
            (
                "studio",
                include_str!("../assets/template-sets/studio.json"),
            ),
            (
                "ledger",
                include_str!("../assets/template-sets/ledger.json"),
            ),
            (
                "field-book",
                include_str!("../assets/template-sets/field-book.json"),
            ),
            (
                "monograph",
                include_str!("../assets/template-sets/monograph.json"),
            ),
            (
                "blueprint",
                include_str!("../assets/template-sets/blueprint.json"),
            ),
            (
                "workshop",
                include_str!("../assets/template-sets/workshop.json"),
            ),
            (
                "gallery",
                include_str!("../assets/template-sets/gallery.json"),
            ),
            (
                "interior",
                include_str!("../assets/template-sets/interior.json"),
            ),
            (
                "sketchbook",
                include_str!("../assets/template-sets/sketchbook.json"),
            ),
            (
                "series",
                include_str!("../assets/template-sets/series.json"),
            ),
        ];
        DATA.iter()
            .filter_map(|&(key, data)| {
                let info = creabim_geom::templateset::info(data).ok()?;
                // a tile says it in one sentence
                let line = info
                    .line
                    .split_inclusive(". ")
                    .next()
                    .unwrap_or("")
                    .trim()
                    .trim_end_matches('.')
                    .to_string();
                let pictures = vec![
                    ("metric|mark", set_picture_on(data, "A3", true)),
                    ("metric|plain", set_picture_on(data, "A3", false)),
                    ("us|mark", set_picture_on(data, "ARCH B", true)),
                    ("us|plain", set_picture_on(data, "ARCH B", false)),
                ];
                Some(SetTile {
                    key,
                    name: info.name,
                    line,
                    picture: set_picture(data),
                    pictures,
                    data,
                })
            })
            .collect()
    })
}

/// A picture of a set's sheet (A3, with the mark) for its tile: its
/// master drawn as a small SVG, as a data URL. Empty with no A3 master.
pub(crate) fn set_picture(data: &str) -> String {
    set_picture_on(data, "A3", true)
}

/// A picture of a set's sheet on `paper` ("A3", "ARCH B"), with or
/// without CreaBIM's mark.
pub(crate) fn set_picture_on(data: &str, paper_name: &str, mark: bool) -> String {
    use creabim_core::titleblock::{PaperColour, TextAlign, TitleItem};
    let paper = match paper_name {
        "ARCH B" => glam::DVec2::new(457.2, 304.8),
        _ => glam::DVec2::new(420.0, 297.0),
    };
    let Some(t) = creabim_geom::templateset::master(data, paper_name, mark) else {
        return String::new();
    };
    // the paper and its ink, near what the sheet shows
    let palette = data
        .split("\"palette\":\"")
        .nth(1)
        .and_then(|r| r.split('"').next())
        .unwrap_or("");
    let own;
    let (ground, ink) = match (&t.stock.colour, palette) {
        (PaperColour::Own(c), _) => {
            own = format!(
                "rgb({:.0},{:.0},{:.0})",
                c[0] * 255.0,
                c[1] * 255.0,
                c[2] * 255.0
            );
            (
                own.as_str(),
                if c[0] + c[1] + c[2] < 1.2 {
                    "#e6e9ee"
                } else {
                    "#23282b"
                },
            )
        }
        (PaperColour::Palette, "Blueprint") => ("#1b2434", "#dfe7f3"),
        (PaperColour::Palette, "Gallery") => ("#1d1e1c", "#e4e6df"),
        (PaperColour::Palette, "Sepia") => ("#f3ead8", "#3d2f22"),
        _ => ("#ffffff", "#23282b"),
    };
    let (x, y) = (|v: f64| paper.x + v, |v: f64| paper.y - v);
    let path = |pts: &[glam::DVec2], closed: bool| {
        let mut d = String::new();
        for (i, p) in pts.iter().enumerate() {
            d += &format!(
                "{}{:.1} {:.1}",
                if i == 0 { "M" } else { "L" },
                x(p.x),
                y(p.y)
            );
        }
        if closed {
            d += "Z";
        }
        d
    };
    // on dark paper the app draws a near-white shape (the faint mark) as a
    // shade just off the paper, so the picture does too
    let dark = ink.starts_with("#d") || ink.starts_with("#e");
    let rgb = |c: [f32; 3]| {
        if dark && c.iter().all(|v| *v > 0.9) {
            "rgba(255,255,255,0.07)".to_string()
        } else {
            format!(
                "rgb({:.0},{:.0},{:.0})",
                c[0] * 255.0,
                c[1] * 255.0,
                c[2] * 255.0
            )
        }
    };
    let sample = |s: &str| {
        let mut s = s.to_string();
        for (k, v) in [
            ("{practice}", "Your practice"),
            ("{practice_line}", "Architecture and design"),
            ("{project}", "Maple Lane House"),
            ("{client}", "The client"),
            ("{sheet}", "Ground floor plan"),
            ("{number}", "A-101"),
            ("{scale}", "1:100"),
            ("{date}", "Nov 2026"),
            ("{rev}", "-"),
        ] {
            s = s.replace(k, v);
        }
        // a field with no sample shows nothing
        while let (Some(a), Some(b)) = (s.find('{'), s.find('}')) {
            if a < b {
                s.replace_range(a..=b, "")
            } else {
                break;
            }
        }
        s.replace('&', "&amp;").replace('<', "&lt;")
    };
    let mut svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 {} {}' font-family='sans-serif'><rect width='100%' height='100%' fill='{ground}'/>",
        paper.x, paper.y
    );
    if t.border {
        svg += &format!(
            "<rect x='20' y='10' width='{}' height='{}' fill='none' stroke='{ink}' stroke-width='0.5'/>",
            paper.x - 30.0,
            paper.y - 20.0
        );
    }
    // the title block's upright lines, to fit each text to its cell: the
    // picture's plain sans is wider than the sheet's own face, and A-101
    // ran out of its box (MU102)
    let mut uprights: Vec<(f64, f64, f64)> = vec![];
    for i in &t.items {
        let (pts, closed): (&[glam::DVec2], bool) = match i {
            TitleItem::Line { points, .. } => (points, false),
            TitleItem::Shape { points, closed, .. } => (points, *closed),
            TitleItem::Fill { points, .. } => (points, true),
            _ => continue,
        };
        let n = pts.len();
        let segs = if closed { n } else { n.saturating_sub(1) };
        for k in 0..segs {
            let (a, b) = (pts[k], pts[(k + 1) % n]);
            if (a.x - b.x).abs() < 0.05 && (a.y - b.y).abs() > 0.5 {
                uprights.push((a.x, a.y.min(b.y), a.y.max(b.y)));
            }
        }
    }
    if t.border {
        // the frame drawn round the sheet (in mm from the bottom-right)
        uprights.push((-10.0, 10.0, paper.y - 10.0));
        uprights.push((20.0 - paper.x, 10.0, paper.y - 10.0));
    }
    let room = |at: glam::DVec2, align: &TextAlign| -> Option<f64> {
        let across = |right: bool| {
            uprights
                .iter()
                .filter(|(_, y0, y1)| at.y > *y0 - 0.1 && at.y < *y1 + 0.1)
                .map(|(x, _, _)| if right { x - at.x } else { at.x - x })
                .filter(|d| *d > 0.3)
                .fold(f64::INFINITY, f64::min)
        };
        let w = match align {
            TextAlign::Start => across(true),
            TextAlign::End => across(false),
            TextAlign::Middle => 2.0 * across(true).min(across(false)),
        };
        w.is_finite().then_some(w)
    };
    for i in &t.items {
        match i {
            TitleItem::Line { points, width } => {
                svg += &format!(
                    "<path d='{}' fill='none' stroke='{ink}' stroke-width='{:.2}' stroke-linecap='round' stroke-linejoin='round'/>",
                    path(points, false),
                    width.max(0.3)
                );
            }
            TitleItem::Shape {
                points,
                closed,
                pen,
                colour,
                fill,
                opacity,
            } => {
                svg += &format!(
                    "<path d='{}' fill='{}' stroke='{}' stroke-width='{:.2}' stroke-linecap='round' opacity='{opacity}'/>",
                    path(points, *closed),
                    fill.map_or("none".to_string(), rgb),
                    if pen.is_some() { rgb(*colour) } else { "none".into() },
                    pen.unwrap_or(0.0)
                );
            }
            TitleItem::Fill { points, gray } => {
                svg += &format!(
                    "<path d='{}' fill='rgb({g:.0},{g:.0},{g:.0})'/>",
                    path(points, true),
                    g = gray * 255.0
                );
            }
            TitleItem::Text {
                at,
                size,
                text,
                align,
                angle,
                colour,
                ..
            } => {
                let anchor = match align {
                    TextAlign::Start => "start",
                    TextAlign::Middle => "middle",
                    TextAlign::End => "end",
                };
                // too wide for its cell (by a plain sans's usual width):
                // narrowed to fit, with a hair of margin
                let shown = sample(text);
                let est = size * 1.4 * 0.56 * shown.chars().count() as f64;
                let fit = match room(*at, align) {
                    Some(w) if angle.abs() < 1e-6 && est > w - 1.2 => {
                        format!(
                            " textLength='{:.1}' lengthAdjust='spacingAndGlyphs'",
                            (w - 1.2).max(1.0)
                        )
                    }
                    _ => String::new(),
                };
                svg += &format!(
                    "<text x='{x:.1}' y='{y:.1}' font-size='{:.1}' text-anchor='{anchor}' dominant-baseline='central' fill='{}' transform='rotate({:.1} {x:.1} {y:.1})'{fit}>{}</text>",
                    size * 1.4,
                    colour.map_or(ink.to_string(), |c| format!("rgb({:.0},{:.0},{:.0})", c[0] * 255.0, c[1] * 255.0, c[2] * 255.0)),
                    -angle.to_degrees(),
                    shown,
                    x = x(at.x),
                    y = y(at.y),
                );
            }
            _ => {}
        }
    }
    svg += "</svg>";
    format!("data:image/svg+xml;base64,{}", base64(svg.as_bytes()))
}

/// Where more templates are offered. Opened only when asked.
pub(crate) const MORE: &str = "https://chopinregis.github.io/CreaBIM";

#[cfg(test)]
mod tests {
    use super::*;
    use creabim_core::Element;

    #[test]
    fn a_template_keeps_the_project_without_the_jobs_details_and_opens_as_a_new_one() {
        let dir =
            std::env::temp_dir().join(format!("creabim-template-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut app = super::super::test_app();
        app.path = "Garden Lane house.creabim".into();
        // a job with a client and an address
        let details = |p: &mut creabim_core::project::ProjectInfo| {
            p.client = "Mrs Example".into();
            p.address = "14 Garden Lane".into();
            p.building_name = "House".into();
        };
        let model = app.doc.model();
        let tx = match model.project_info() {
            Some((id, _)) => {
                let before = model.get(id).unwrap().clone();
                let mut after = before.clone();
                if let ElementKind::Project(p) = &mut after.kind {
                    details(p);
                }
                Transaction::new("details").replace(before, after)
            }
            None => {
                let mut p = creabim_core::project::ProjectInfo::default();
                details(&mut p);
                Transaction::new("details").insert(Element::new("Project", ElementKind::Project(p)))
            }
        };
        app.doc.commit(tx).unwrap();
        let walls = |m: &Model| {
            m.iter()
                .filter(|e| matches!(e.kind, ElementKind::Wall(_)))
                .count()
        };
        let had = walls(app.doc.model());
        assert!(had > 0);

        app.save_template_to(&dir.join("Office standard"));
        let file = dir.join("Office standard.creabim-template");
        assert!(file.exists(), "{}", app.status);
        assert_eq!(
            list(&dir)
                .iter()
                .map(|t| t.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Office standard"]
        );
        // the project it was saved from is as it was
        assert_eq!(app.path, "Garden Lane house.creabim");
        assert_eq!(
            app.doc.model().project_info().unwrap().1.client,
            "Mrs Example"
        );

        // opened, it is a new project with no file yet, the building and
        // its name kept, the job's client and address gone
        app.open_template(&file.to_string_lossy());
        assert_eq!(app.path, "Office standard.creabim");
        assert!(!Path::new(&app.path).exists());
        assert_eq!(walls(app.doc.model()), had);
        let info = app.doc.model().project_info().unwrap().1.clone();
        assert_eq!(
            (
                info.client.as_str(),
                info.address.as_str(),
                info.building_name.as_str()
            ),
            ("", "", "House")
        );
        assert!(!app.doc.can_undo(), "a template carries no history");

        // what is not a template says so and changes nothing
        std::fs::write(dir.join("broken.creabim-template"), b"not a project").unwrap();
        let before = app.path.clone();
        app.open_template(&dir.join("broken.creabim-template").to_string_lossy());
        assert!(
            app.status.starts_with("Could not open the template"),
            "{}",
            app.status
        );
        assert_eq!(app.path, before);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn documents_is_where_windows_says_it_is() {
        let answer = "\r\nHKEY_CURRENT_USER\\Software\\...\\User Shell Folders\r\n    Personal    REG_EXPAND_SZ    C:\\Users\\ann\\OneDrive\\Documents\r\n\r\n";
        assert_eq!(
            documents_from(answer, None),
            Some(PathBuf::from("C:\\Users\\ann\\OneDrive\\Documents"))
        );
        let plain = "    Personal    REG_EXPAND_SZ    %USERPROFILE%\\Documents";
        assert_eq!(
            documents_from(plain, Some(Path::new("C:\\Users\\ann"))),
            Some(PathBuf::from("C:\\Users\\ann\\Documents"))
        );
        // an answer that cannot be read gives none, and the fallback is used
        assert_eq!(
            documents_from("ERROR: The system was unable to find the key", None),
            None
        );
        assert_eq!(documents_from(plain, None), None);
    }

    #[test]
    fn the_look_only_leaves_the_building_out_and_a_look_is_applied_by_name() {
        let app = super::super::test_app();
        let model = app.doc.model();
        let walls = |m: &Model| {
            m.iter()
                .filter(|e| matches!(e.kind, ElementKind::Wall(_)))
                .count()
        };
        let levels = |m: &Model| {
            m.iter()
                .filter(|e| matches!(e.kind, ElementKind::Level(_)))
                .count()
        };
        assert!(walls(model) > 0 && levels(model) > 0);
        // the look only: no building, the levels still there
        let look = as_template_with(model, look_only());
        assert_eq!(walls(look.model()), 0);
        assert_eq!(levels(look.model()), levels(model));
        assert!(look
            .model()
            .iter()
            .all(|e| part_of(&e.kind) != Some(BUILDING)));
        // ticked, the building brings its views and types whatever the ticks say
        let mut keep = [false; 7];
        keep[BUILDING] = true;
        let built = as_template_with(model, keep);
        assert_eq!(walls(built.model()), walls(model));
        let count = |m: &Model, i: usize| m.iter().filter(|e| part_of(&e.kind) == Some(i)).count();
        assert_eq!(count(built.model(), 3), count(model, 3));
        assert_eq!(count(built.model(), 4), count(model, 4));
        assert_eq!(part_counts(model)[BUILDING], count(model, BUILDING));

        // a look applied: a text style of the same name takes the
        // template's content and keeps its id; one not here is added
        let dir =
            std::env::temp_dir().join(format!("creabim-applylook-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut office = super::super::test_app();
        let styles: Vec<creabim_core::Element> = office
            .doc
            .model()
            .iter()
            .filter(|e| matches!(e.kind, ElementKind::TextStyle(_)))
            .cloned()
            .collect();
        let mut tx = Transaction::new("office styles");
        if let Some(first) = styles.first() {
            let mut after = first.clone();
            if let ElementKind::TextStyle(s) = &mut after.kind {
                s.size *= 2.0;
            }
            tx = tx.replace(first.clone(), after);
        }
        let mut extra = creabim_core::Element::new(
            "Office heading",
            ElementKind::TextStyle(Default::default()),
        );
        extra.id = creabim_core::ElementId::new();
        tx = tx.insert(extra);
        office.doc.commit(tx).unwrap();
        office.save_template_to(&dir.join("Office"));

        let mut job = super::super::test_app();
        let before = job
            .doc
            .model()
            .iter()
            .filter(|e| matches!(e.kind, ElementKind::TextStyle(_)))
            .count();
        let had_walls = walls(job.doc.model());
        job.apply_template_look(&dir.join("Office.creabim-template").to_string_lossy());
        let after = job
            .doc
            .model()
            .iter()
            .filter(|e| matches!(e.kind, ElementKind::TextStyle(_)))
            .count();
        assert_eq!(after, before + 1, "{}", job.status);
        assert!(job.doc.model().iter().any(|e| e.name == "Office heading"));
        assert_eq!(
            walls(job.doc.model()),
            had_walls,
            "the building is not touched"
        );
        if let Some(first) = styles.first() {
            let now = job.doc.model().get(first.id).expect("its id is kept");
            let (ElementKind::TextStyle(a), ElementKind::TextStyle(b)) = (&now.kind, &first.kind)
            else {
                panic!()
            };
            assert_eq!(a.size, b.size * 2.0);
        }
        // one step back takes it all away
        assert!(job.doc.can_undo());
        assert!(job.doc.undo());
        assert_eq!(
            job.doc
                .model()
                .iter()
                .filter(|e| matches!(e.kind, ElementKind::TextStyle(_)))
                .count(),
            before
        );
        // applied twice, the second changes nothing
        job.apply_template_look(&dir.join("Office.creabim-template").to_string_lossy());
        job.apply_template_look(&dir.join("Office.creabim-template").to_string_lossy());
        assert!(job.status.contains("nothing changed"), "{}", job.status);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_picture_is_kept_in_the_file_and_text_is_base64() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        let dir = std::env::temp_dir().join(format!("creabim-picture-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut app = super::super::test_app();
        app.save_template_to(&dir.join("With picture"));
        let file = dir.join("With picture.creabim-template");
        // none of its own until one is set; the file opens either way
        creabim_store::set_picture(&file, "data:image/png;base64,AAAA").unwrap();
        assert_eq!(
            creabim_store::picture(&file).as_deref(),
            Some("data:image/png;base64,AAAA")
        );
        creabim_store::set_picture(&file, "data:image/png;base64,BBBB").unwrap();
        assert_eq!(
            creabim_store::picture(&file).as_deref(),
            Some("data:image/png;base64,BBBB")
        );
        assert!(creabim_store::load(&file).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_set_starts_a_project_as_the_window_chose_it() {
        let ctx = eframe::egui::Context::default();
        let mut app = super::super::test_app();
        let sheets = |app: &super::super::CreaBimApp| {
            app.doc
                .model()
                .iter()
                .filter_map(|e| match &e.kind {
                    ElementKind::Sheet(s) => Some((s.width, s.height)),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let master = |app: &super::super::CreaBimApp| {
            app.doc.model().iter().find_map(|e| match &e.kind {
                ElementKind::SheetTemplate(t) => Some((e.name.clone(), t.clone())),
                _ => None,
            })
        };
        // the starter house, US sizes, without the mark
        assert!(app.templates_action(&ctx, "new.set", "blueprint|us|starter|plain"));
        let (name, _) = master(&app).unwrap();
        assert!(name.starts_with("Blueprint ARCH"), "{name}");
        assert!(!sheets(&app).is_empty());
        // every sheet is on a US paper (whole inches)
        assert!(sheets(&app)
            .iter()
            .all(|&(w, h)| ((w / 25.4).round() * 25.4 - w).abs() < 0.01
                && ((h / 25.4).round() * 25.4 - h).abs() < 0.01));
        // and on the master drawn for its paper
        for e in app.doc.model().iter() {
            if let ElementKind::Sheet(s) = &e.kind {
                let ElementKind::SheetTemplate(t) = &app
                    .doc
                    .model()
                    .get(s.template.expect("a master"))
                    .unwrap()
                    .kind
                else {
                    panic!()
                };
                assert_eq!(
                    t.paper,
                    Some(glam::DVec2::new(s.width, s.height)),
                    "{}",
                    e.name
                );
            }
        }
        assert_eq!(
            app.doc.model().units().length,
            creabim_core::units::LengthUnit::FeetInches
        );
        assert!(app
            .doc
            .model()
            .iter()
            .any(|e| matches!(e.kind, ElementKind::Wall(_))));
        // it is how the project starts: nothing to undo
        assert!(!app.doc.undo());
        // an empty project, metric, the mark kept (what is left out): it
        // has a first sheet, on the set's master
        assert!(app.templates_action(&ctx, "new.set", "workshop"));
        let (name, _) = master(&app).unwrap();
        assert!(name.starts_with("Workshop A"), "{name}");
        assert!(!app
            .doc
            .model()
            .iter()
            .any(|e| matches!(e.kind, ElementKind::Wall(_))));
        assert_eq!(sheets(&app).len(), 1);
        // no such set: said, and the project stays
        assert!(app.templates_action(&ctx, "new.set", "nonsense"));
        assert!(master(&app).unwrap().0.starts_with("Workshop A"));
    }

    #[test]
    fn all_ten_sets_ship_and_the_mark_is_what_the_tick_takes_away() {
        let all = sets();
        assert_eq!(all.len(), 10);
        assert_eq!(all[0].name, "Studio");
        for s in all {
            assert!(
                !s.line.is_empty() && !s.line.contains(". "),
                "{}: {}",
                s.name,
                s.line
            );
            assert!(
                s.picture.starts_with("data:image/svg+xml;base64,"),
                "{}",
                s.name
            );
            // both standards, with and without the mark, on both starts
            let with = creabim_geom::templateset::master(s.data, "A3", true).expect("an A3 master");
            let without = creabim_geom::templateset::master(s.data, "A3", false).unwrap();
            assert!(
                with.items.len() > without.items.len(),
                "{}: no mark to take away",
                s.name
            );
            assert!(without.validate().is_ok(), "{}", s.name);
            assert!(
                creabim_geom::templateset::master(s.data, "ARCH B", true).is_some(),
                "{}",
                s.name
            );
        }
        // one whole set through both standards and every start
        for us in [false, true] {
            for start in 0..3 {
                let base = match start {
                    0 => creabim_core::Model::default(),
                    1 => creabim_core::starter::starter_house(),
                    _ => creabim_core::ridgeline::ridgeline_house(),
                };
                let made =
                    creabim_geom::templateset::apply(all[9].data, base, us, start == 0, false)
                        .unwrap();
                for e in made.model.iter() {
                    assert!(e.validate().is_ok(), "{}: {:?}", e.name, e.validate());
                }
                assert!(
                    !made.report.iter().any(|r| r.contains("DOES NOT FIT")),
                    "{:?}",
                    made.report
                );
            }
        }
    }

    #[test]
    fn names_and_dates_read_as_the_lists_show_them() {
        assert_eq!(
            safe_name("  Office / standard: 2026?  "),
            "Office standard 2026"
        );
        assert_eq!(safe_name(" :: "), "Template");
        let at = |secs: u64| std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs);
        assert_eq!(day_label(at(0)), "1 Jan 1970");
        assert_eq!(day_label(at(951_868_800)), "1 Mar 2000");
        assert_eq!(day_label(at(1_795_046_400)), "19 Nov 2026");
        let day = 86_400;
        assert_eq!(ago_label(at(10 * day), at(10 * day - 3600)), "today");
        assert_eq!(ago_label(at(10 * day), at(9 * day - 60)), "yesterday");
        assert_eq!(ago_label(at(10 * day), at(5 * day)), "5 days ago");
        assert_eq!(ago_label(at(100 * day), at(10 * day)), "3 months ago");
        assert_eq!(ago_label(at(1000 * day), at(10 * day)), "2 years ago");
        // a file dated after now (a clock set back) reads as today
        assert_eq!(ago_label(at(5 * day), at(6 * day)), "today");
    }
}
