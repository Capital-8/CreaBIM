//! Renders as views (Visuals REV02, decision 3).
//!
//! A render kept from the 3D view is an image file in the project's
//! Renders folder, beside the project file. The project browser lists them
//! in a Renders group, and one opens in a Render area like a plan or a
//! sheet. Each keeps the look and the light it was made with, beside it,
//! in a small text file.

use super::CreaBimApp;
use crate::areas::Shows;
use crate::ui_model::Item;
use eframe::egui;
use std::path::PathBuf;

/// What a render was made with, kept beside its image.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Made {
    pub look: crate::render3d::RenderMode,
    pub day: f32,
    pub hour: f32,
    pub ev: f32,
    pub width: usize,
    pub height: usize,
    /// Its sky; renders kept before skies were a choice had a clear one.
    #[serde(default)]
    pub sky: String,
    #[serde(default = "one")]
    pub strength: f32,
    /// Its white balance, kelvin; 0 in renders kept before there was one
    /// (daylight).
    #[serde(default)]
    pub white: f32,
    /// Where the camera stood and its lens, so the 3D view can be put
    /// back there; none in renders kept before it was recorded, and in
    /// pictures added from a file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera: Option<MadeCamera>,
}

fn one() -> f32 {
    1.0
}

/// The 3D view's camera as a render was made.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct MadeCamera {
    pub target: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    /// The lens, mm on a full-frame camera.
    pub lens: f32,
    #[serde(default)]
    pub straight: bool,
    #[serde(default)]
    pub ortho: bool,
}

impl MadeCamera {
    pub fn of(c: &crate::camera::OrbitCamera) -> Self {
        Self {
            target: c.target.to_array(),
            yaw: c.yaw,
            pitch: c.pitch,
            distance: c.distance,
            lens: c.lens_mm(),
            straight: c.straight,
            ortho: c.ortho,
        }
    }

    /// Put `c` where this camera stood, with its lens.
    pub fn give(&self, c: &mut crate::camera::OrbitCamera) {
        c.target = self.target.into();
        c.yaw = self.yaw;
        c.pitch = self.pitch;
        c.ortho = self.ortho;
        c.straight = self.straight;
        c.fov_y = 2.0 * (12.0 / self.lens.clamp(8.0, 400.0)).atan();
        c.distance = self.distance;
    }
}

/// One kept render: its name (the file's, without ".png") and its file.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Kept {
    pub name: String,
    pub path: PathBuf,
}

/// The project's renders, as the folder holds them.
#[derive(Default)]
pub(crate) struct Renders {
    pub list: Vec<Kept>,
    /// The one a Render area shows.
    pub current: Option<String>,
    /// Its image, loaded once.
    tex: Option<(String, egui::TextureHandle)>,
    /// The folder the list was read from, and when.
    read: Option<(PathBuf, f64)>,
    /// How large the one shown is drawn, screen points to a pixel of it:
    /// None fits it to the area.
    zoom: Option<f32>,
    /// How far it is moved from the area's middle.
    pan: egui::Vec2,
    /// A render being made at Output's size, to keep when it is done.
    pub making: Option<crate::rendered::Final>,
    /// The crop being set on the one shown (Render crop REV00).
    pub crop: Option<Crop>,
    /// The shape of the "Own" crop, width to height.
    own: [u32; 2],
    /// The pixels of the one shown, to read a colour from.
    rgb: Option<image::RgbImage>,
    /// Colour picking on (the pipette): a click keeps the colour.
    pub pick: bool,
    /// The pixel under the pointer: where it is, and its colour.
    under: Option<([u32; 2], [u8; 3])>,
    /// Colours picked since CreaBIM opened, the newest first.
    pub picked: Vec<[u8; 3]>,
    /// The model's light fittings, for the 3D view's geometry they were
    /// found in.
    pub(crate) fittings: std::cell::RefCell<Option<(u64, Vec<super::lamps::Fitting>)>>,
    /// What the render last asked about was made with, read once: its
    /// file was read and parsed on every frame, twice (MU116).
    made: std::cell::RefCell<Option<(String, Option<Made>)>>,
}

/// A crop being set: its preset, its frame in the image's pixels (not
/// yet placed when it is empty), and what a drag has hold of.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Crop {
    pub preset: usize,
    pub frame: egui::Rect,
    grab: Option<Grab>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Grab {
    /// A corner: the one opposite stays (in the image's pixels).
    Corner(egui::Pos2),
    /// A side: 0 left, 1 top, 2 right, 3 bottom. The one opposite stays.
    Side(usize),
    /// The whole frame, held this far from its top left corner.
    Held(egui::Vec2),
    /// The image itself, under the frame.
    Image,
}

/// A colour as designers write it: #RRGGBB.
pub(crate) fn hex(c: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}

/// A small square of colour `c`, in a row.
fn swatch(ui: &mut egui::Ui, c: [u8; 3]) {
    let (r, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
    ui.painter()
        .rect_filled(r, 3.0, egui::Color32::from_rgb(c[0], c[1], c[2]));
    ui.painter().rect_stroke(
        r,
        3.0,
        egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
        egui::StrokeKind::Inside,
    );
}

/// The crop's presets: a name, its shape (0 by 0, any), and the size it
/// is made for (0 by 0, none). The last is the user's own shape.
pub(crate) const CROPS: [(&str, [u32; 2], [u32; 2]); 8] = [
    ("Free", [0, 0], [0, 0]),
    ("Wide 16:9", [16, 9], [1920, 1080]),
    ("Tall 9:16", [9, 16], [1080, 1920]),
    ("Square", [1, 1], [1080, 1080]),
    ("Portrait 4:5", [4, 5], [1080, 1350]),
    ("Banner", [16, 9], [2560, 1440]),
    ("Sheet", [297, 210], [3508, 2480]),
    ("Own", [0, 0], [0, 0]),
];

/// The largest frame of shape `ratio` (width over height; None, a free
/// one a little inside) that fits an image of `size`, centred.
pub(crate) fn crop_frame(size: egui::Vec2, ratio: Option<f32>) -> egui::Rect {
    let wh = match ratio {
        None => egui::vec2(size.x * 0.62, size.y * 0.78),
        Some(r) if size.x / size.y > r => egui::vec2(size.y * r, size.y),
        Some(r) => egui::vec2(size.x, size.x / r),
    };
    egui::Rect::from_center_size((size / 2.0).to_pos2(), wh)
}

/// The frame a side of `f` dragged to `to` gives (0 left, 1 top, 2 right,
/// 3 bottom), its opposite side staying. A free frame moves that side
/// alone; one kept to `ratio` grows across too, about its middle, and
/// stops where that would leave an image of `size`.
pub(crate) fn crop_side(
    f: egui::Rect,
    side: usize,
    to: egui::Pos2,
    ratio: Option<f32>,
    size: egui::Vec2,
) -> egui::Rect {
    let across = side.is_multiple_of(2); // a left or right side: the width changes
    let fixed = match side {
        0 => f.max.x,
        1 => f.max.y,
        2 => f.min.x,
        _ => f.min.y,
    };
    let (at, far) = if across {
        (to.x, size.x)
    } else {
        (to.y, size.y)
    };
    let low = side < 2; // the left and top sides sit before the fixed one
    let mut len = if low {
        fixed - at.clamp(0.0, fixed)
    } else {
        at.clamp(fixed, far) - fixed
    }
    .max(8.0);
    let mut other = if across { f.height() } else { f.width() };
    let mid = if across { f.center().y } else { f.center().x };
    if let Some(r) = ratio {
        // width over height
        let k = if across { 1.0 / r } else { r };
        let room = 2.0 * mid.min((if across { size.y } else { size.x }) - mid);
        other = len * k;
        if other > room {
            other = room;
            len = other / k;
        }
    }
    let (a, b) = if low {
        (fixed - len, fixed)
    } else {
        (fixed, fixed + len)
    };
    let (c, d) = if ratio.is_some() {
        (mid - other / 2.0, mid + other / 2.0)
    } else if across {
        (f.min.y, f.max.y)
    } else {
        (f.min.x, f.max.x)
    };
    if across {
        egui::Rect::from_min_max(egui::pos2(a, c), egui::pos2(b, d))
    } else {
        egui::Rect::from_min_max(egui::pos2(c, a), egui::pos2(d, b))
    }
}

/// The frame a corner dragged to `to` gives, its opposite corner `fixed`
/// staying, kept to shape `ratio` and inside an image of `size`.
pub(crate) fn crop_resized(
    fixed: egui::Pos2,
    to: egui::Pos2,
    ratio: Option<f32>,
    size: egui::Vec2,
) -> egui::Rect {
    let to = egui::pos2(to.x.clamp(0.0, size.x), to.y.clamp(0.0, size.y));
    let (sx, sy) = (
        if to.x < fixed.x { -1.0 } else { 1.0 },
        if to.y < fixed.y { -1.0 } else { 1.0 },
    );
    let room = egui::vec2(
        if sx < 0.0 { fixed.x } else { size.x - fixed.x },
        if sy < 0.0 { fixed.y } else { size.y - fixed.y },
    );
    let mut w = (to.x - fixed.x).abs().max(8.0);
    let mut h = (to.y - fixed.y).abs().max(8.0);
    if let Some(r) = ratio {
        w = w.max(h * r);
        h = w / r;
        if w > room.x {
            w = room.x;
            h = w / r;
        }
        if h > room.y {
            h = room.y;
            w = h * r;
        }
    }
    egui::Rect::from_two_pos(
        fixed,
        fixed + egui::vec2(sx * w.min(room.x), sy * h.min(room.y)),
    )
}

/// A file name that every system takes.
fn safe(name: &str) -> String {
    name.chars()
        .map(|c| if "\\/:*?\"<>|".contains(c) { '-' } else { c })
        .collect::<String>()
        .trim()
        .to_string()
}

/// The first of `base`, `base 2`, `base 3`… that `taken` doesn't hold.
pub(crate) fn free_name(base: &str, taken: &[String]) -> String {
    let base = safe(base);
    if !taken.iter().any(|t| t.eq_ignore_ascii_case(&base)) {
        return base;
    }
    (2..)
        .map(|n| format!("{base} {n}"))
        .find(|c| !taken.iter().any(|t| t.eq_ignore_ascii_case(c)))
        .unwrap_or(base)
}

impl CreaBimApp {
    /// Where the project's renders are kept: a folder named after the
    /// project file, beside it; for a project not saved yet, one with the
    /// app's own files.
    pub(crate) fn renders_dir(&self) -> PathBuf {
        let p = std::path::Path::new(&self.path);
        match (p.parent().filter(|d| d.is_dir()), p.file_stem()) {
            (Some(dir), Some(stem)) if p.exists() => {
                dir.join(format!("{}_renders", stem.to_string_lossy()))
            }
            _ => crate::actlog::path()
                .and_then(|l| l.parent().map(|d| d.join("renders_unsaved")))
                .unwrap_or_else(|| std::env::temp_dir().join("creabim_renders")),
        }
    }

    /// Read the folder again when it is another, or a moment has passed.
    pub(crate) fn renders_refresh(&mut self, now: f64, force: bool) {
        let dir = self.renders_dir();
        let fresh = self
            .renders
            .read
            .as_ref()
            .is_some_and(|(d, t)| *d == dir && now - *t < 2.0);
        if fresh && !force {
            return;
        }
        let mut found: Vec<(std::time::SystemTime, Kept)> = std::fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|f| {
                let path = f.path();
                let png = path
                    .extension()
                    .is_some_and(|x| x.eq_ignore_ascii_case("png"));
                let name = path.file_stem()?.to_string_lossy().to_string();
                let when = f.metadata().and_then(|m| m.modified()).ok()?;
                png.then_some((when, Kept { name, path }))
            })
            .collect();
        // the newest first
        found.sort_by_key(|f| std::cmp::Reverse(f.0));
        self.renders.list = found.into_iter().map(|f| f.1).collect();
        self.renders.read = Some((dir, now));
        *self.renders.made.borrow_mut() = None;
        if self
            .renders
            .current
            .as_ref()
            .is_some_and(|c| !self.renders.list.iter().any(|k| &k.name == c))
        {
            self.renders.current = self.renders.list.first().map(|k| k.name.clone());
        }
    }

    /// Output's sizes to pick from: a name and its pixels (0 by 0, the 3D
    /// view's own). The same shapes as the crop's presets.
    pub(crate) const SIZES: [(&'static str, [u32; 2]); 7] = [
        ("As the 3D view", [0, 0]),
        ("Wide 1920 × 1080", [1920, 1080]),
        ("Tall 1080 × 1920", [1080, 1920]),
        ("Square 1080 × 1080", [1080, 1080]),
        ("Portrait 1080 × 1350", [1080, 1350]),
        ("Banner 2560 × 1440", [2560, 1440]),
        ("Sheet 3508 × 2480", [3508, 2480]),
    ];

    /// Make a render at Output's size, in the background, to keep when it
    /// is done; at the 3D view's own size, keep what is shown.
    pub(crate) fn render_and_keep(&mut self, now: f64) {
        let [w, h] = self.shell.settings.render_size;
        if w == 0 || h == 0 {
            return self.keep_render(now);
        }
        let Some(mode) = self.shell.settings.render_3d.rendered() else {
            self.status = "Pick a Style or Rendered look in the 3D view first.".into();
            return;
        };
        let model = self.doc.model();
        let info = model.project_info().map(|(_, p)| p.clone());
        let terrain = model.iter().any(|e| {
            matches!(
                &e.kind,
                creabim_core::ElementKind::Site(s)
                    if matches!(s.form, creabim_core::SiteForm::Terrain { .. })
            )
        });
        let s = &self.shell.settings;
        let sun = crate::rendered::SunAt {
            latitude: info
                .as_ref()
                .and_then(|p| p.latitude)
                .unwrap_or(super::DEFAULT_LATITUDE) as f32,
            true_north: info.as_ref().map_or(0.0, |p| p.true_north) as f32,
            day: s.render_day,
            hour: s.render_hour,
            ev: s.render_ev,
            sky: creabim_render::sky::SkyKind::of_key(&s.render_sky),
            strength: s.render_sky_strength,
            white: s.render_white,
        };
        let aspect = w as f32 / h as f32;
        let (lamps, lamps_key) = self.lamps_now();
        let view = crate::rendered::View {
            mode,
            view_proj: self.camera.view_proj(aspect),
            eye: self.camera.eye(),
            target: self.camera.target,
            pixels: [w, h],
            scene_gen: self.geometry_gen,
            items: &self.scene,
            ground: (!terrain).then_some((-0.15, [0.36, 0.49, 0.25])),
            sun,
            lamps,
            lamps_key,
            now,
        };
        self.renders.making = self.rendered.start_final(&view, [w as usize, h as usize]);
        self.status = if self.renders.making.is_some() {
            format!("Rendering at {w} × {h} to keep: the 3D view stays free to work in.")
        } else {
            "Could not start the render.".into()
        };
    }

    /// Once a frame: a render being made, kept when it is done.
    pub(crate) fn renders_poll(&mut self, ctx: &egui::Context) {
        let Some(job) = &mut self.renders.making else {
            return;
        };
        if !job.poll() {
            ctx.request_repaint_after(std::time::Duration::from_millis(150));
            return;
        }
        let img = job.image.clone();
        self.renders.making = None;
        let now = ctx.input(|i| i.time);
        match img {
            Some(img) => self.keep_image(img, now),
            None => self.status = "The render stopped before it gave an image.".into(),
        }
    }

    /// Keep the 3D view as it is rendered now: its image into the Renders
    /// folder, named by its look and its light, and shown in a Render area.
    pub(crate) fn keep_render(&mut self, now: f64) {
        let Some(img) = self.rendered.image.clone() else {
            self.status =
                "Nothing is rendered yet: pick a Style or Rendered look in the 3D view first."
                    .into();
            return;
        };
        self.keep_image(img, now);
    }

    /// Keep `img` as a render made with the 3D view's look and light.
    fn keep_image(&mut self, img: std::sync::Arc<creabim_render::util::Image>, now: f64) {
        let s = &self.shell.settings;
        let made = Made {
            look: s.render_3d,
            day: s.render_day,
            hour: s.render_hour,
            ev: s.render_ev,
            width: img.w,
            height: img.h,
            sky: s.render_sky.clone(),
            strength: s.render_sky_strength,
            white: s.render_white,
            // (a render made at Output's size takes a while: the camera
            // is the one as it ends, which is the one it began with
            // unless the view was turned meanwhile)
            camera: Some(MadeCamera::of(&self.camera)),
        };
        let dir = self.renders_dir();
        if let Err(e) = std::fs::create_dir_all(&dir) {
            self.status = format!("Could not make the Renders folder {}: {e}", dir.display());
            return;
        }
        self.renders_refresh(now, true);
        let taken: Vec<String> = self.renders.list.iter().map(|k| k.name.clone()).collect();
        let name = free_name(
            &format!(
                "{} · {} {}",
                made.look.label(),
                crate::rendered::day_label(made.day),
                crate::rendered::hour_label(made.hour).replace(':', "h"),
            ),
            &taken,
        );
        let path = dir.join(format!("{name}.png"));
        if let Err(e) = img.save_png(&path) {
            self.status = format!("Could not keep the render: {e}");
            return;
        }
        if let Ok(text) = serde_json::to_string_pretty(&made) {
            let _ = std::fs::write(dir.join(format!("{name}.json")), text);
        }
        self.renders_refresh(now, true);
        self.open_render(&name);
        self.status = format!(
            "Kept as \"{name}\" ({} × {}) in {}.",
            made.width,
            made.height,
            dir.display()
        );
    }

    /// Add the picture in file `src` to the project's renders: a photo, a
    /// reference, a render made elsewhere. It is copied in as a PNG; the
    /// file picked is left as it is.
    pub(crate) fn add_image(&mut self, src: &std::path::Path, now: f64) {
        let im = match image::open(src) {
            Ok(im) => im,
            Err(e) => {
                self.status = format!("Could not read {} as a picture: {e}", src.display());
                return;
            }
        };
        let dir = self.renders_dir();
        if let Err(e) = std::fs::create_dir_all(&dir) {
            self.status = format!("Could not make the Renders folder {}: {e}", dir.display());
            return;
        }
        self.renders_refresh(now, true);
        let taken: Vec<String> = self.renders.list.iter().map(|k| k.name.clone()).collect();
        let stem = src
            .file_stem()
            .map_or("Image".to_string(), |s| s.to_string_lossy().to_string());
        let name = free_name(&stem, &taken);
        let path = dir.join(format!("{name}.png"));
        if let Err(e) = im.to_rgb8().save(&path) {
            self.status = format!("Could not add the picture: {e}");
            return;
        }
        self.renders_refresh(now, true);
        self.open_render(&name);
        self.status = format!(
            "Added \"{name}\" ({} × {} px) to the project's renders; the file you picked is as it was.",
            im.width(),
            im.height()
        );
    }

    /// Keep colour `c` at the head of the picked ones: no colour twice,
    /// eight at most.
    pub(crate) fn pick_colour(&mut self, c: [u8; 3]) {
        self.renders.picked.retain(|x| *x != c);
        self.renders.picked.insert(0, c);
        self.renders.picked.truncate(8);
        self.status = format!(
            "Picked {} (red {}, green {}, blue {}); its code is copied.",
            hex(c),
            c[0],
            c[1],
            c[2]
        );
    }

    /// Show render `name` in a Render area: one open already, else the
    /// focused area turns into one.
    pub(crate) fn open_render(&mut self, name: &str) {
        self.renders.current = Some(name.to_string());
        if !self.areas.root.areas().contains(&Shows::Renders) {
            let i = self.areas.focus;
            if let Some(s) = self.areas.root.get_mut(i) {
                *s = Shows::Renders;
            }
            self.store_areas();
        }
    }

    /// What `name` was made with, from the file beside it.
    pub(crate) fn render_made(&self, name: &str) -> Option<Made> {
        if let Some((n, m)) = self.renders.made.borrow().as_ref() {
            if n == name {
                return m.clone();
            }
        }
        let m: Option<Made> = self
            .renders
            .list
            .iter()
            .find(|k| k.name == name)
            .and_then(|k| std::fs::read_to_string(k.path.with_extension("json")).ok())
            .and_then(|text| serde_json::from_str(&text).ok());
        *self.renders.made.borrow_mut() = Some((name.to_string(), m.clone()));
        m
    }

    /// Take the render shown out of the list: its files go to a
    /// "removed" folder inside Renders, not away for good.
    fn remove_render(&mut self, now: f64) {
        let Some(k) = self
            .renders
            .current
            .clone()
            .and_then(|c| self.renders.list.iter().find(|k| k.name == c).cloned())
        else {
            return;
        };
        let bin = self.renders_dir().join("removed");
        let moved = std::fs::create_dir_all(&bin)
            .and_then(|()| std::fs::rename(&k.path, bin.join(format!("{}.png", k.name))));
        let _ = std::fs::rename(
            k.path.with_extension("json"),
            bin.join(format!("{}.json", k.name)),
        );
        self.status = match moved {
            Ok(()) => format!(
                "\"{}\" is out of the list; its image is in {}.",
                k.name,
                bin.display()
            ),
            Err(e) => format!("Could not remove \"{}\": {e}", k.name),
        };
        self.renders.tex = None;
        self.renders_refresh(now, true);
    }

    /// Render actions from the browser, a header, a menu or Properties.
    pub(crate) fn renders_action(&mut self, ctx: &egui::Context, head: &str, arg: &str) -> bool {
        let now = ctx.input(|i| i.time);
        match head {
            "render.keep" => self.keep_render(now),
            "render.make" => self.render_and_keep(now),
            // the crop on, with the shape used last, or off
            "render.crop" => {
                self.renders.crop = match self.renders.crop {
                    Some(_) => None,
                    None => Some(Crop {
                        preset: 1,
                        frame: egui::Rect::NOTHING,
                        grab: None,
                    }),
                };
                if self.renders.crop.is_some() {
                    // its shapes and Keep are in Properties
                    self.prop_tab = crate::app::PropTab::Render;
                    self.renders.pick = false;
                }
            }
            // the pipette on or off; what it picks is listed in Properties
            "render.pipette" => {
                self.renders.pick = !self.renders.pick;
                if self.renders.pick {
                    self.renders.crop = None;
                    self.prop_tab = crate::app::PropTab::Render;
                    self.status = "Click the picture to keep the colour under the pointer; its code is copied.".into();
                }
            }
            "render.add" => {
                if let Some(p) = crate::files::pick_image() {
                    self.add_image(std::path::Path::new(&p), now);
                }
            }
            "render.stop" => {
                if self.renders.making.take().is_some() {
                    self.status = "The render was stopped; nothing was kept.".into();
                }
            }
            "render.open" => self.open_render(arg),
            "render.remove" => self.remove_render(now),
            "render.folder" => {
                let dir = self.renders_dir();
                let _ = std::fs::create_dir_all(&dir);
                self.status = match crate::files::open_folder(&dir) {
                    Ok(()) => format!("The project's renders are in {}.", dir.display()),
                    Err(e) => e,
                };
            }
            // the look and the light it was made with, back on the 3D view
            "render.settings" => {
                if let Some(m) = self
                    .renders
                    .current
                    .clone()
                    .and_then(|c| self.render_made(&c))
                {
                    let s = &mut self.shell.settings;
                    s.render_day = m.day;
                    s.render_hour = m.hour;
                    s.render_ev = m.ev;
                    s.render_sky = creabim_render::sky::SkyKind::of_key(&m.sky).key().into();
                    s.render_sky_strength = m.strength;
                    s.render_white = if m.white >= 1000.0 { m.white } else { 6500.0 };
                    // and the camera where it stood, with its lens
                    if let Some(c) = &m.camera {
                        c.give(&mut self.camera);
                        s.camera_lens = c.lens;
                        s.camera_straight = c.straight;
                    }
                    s.store();
                    self.set_look(m.look);
                    self.status = format!(
                        "The 3D view has this render's look and light{}: {}, {} {}.",
                        if m.camera.is_some() {
                            ", and stands where it was made from"
                        } else {
                            ""
                        },
                        m.look.label(),
                        crate::rendered::day_label(m.day),
                        crate::rendered::hour_label(m.hour)
                    );
                } else {
                    self.status = "This render has no settings kept with it.".into();
                }
            }
            _ => return false,
        }
        true
    }

    /// The header's list of renders.
    pub(crate) fn renders_menu(&self, key: &str) -> Option<Vec<Item>> {
        (key == "render.pick").then(|| {
            if self.renders.list.is_empty() {
                return vec![Item::new("render.none", "No render kept yet").enabled(false)];
            }
            self.renders
                .list
                .iter()
                .map(|k| {
                    Item::new(format!("render.open:{}", k.name), k.name.clone())
                        .checked(self.renders.current.as_deref() == Some(k.name.as_str()))
                })
                .collect()
        })
    }

    /// The Render area's header buttons.
    pub(crate) fn renders_controls(&self) -> Vec<serde_json::Value> {
        let has = self.renders.current.is_some();
        vec![
            serde_json::json!({"kind": "button", "id": "render.keep", "icon": "plus", "label": "Keep the 3D view",
                "hint": "Keep the 3D view as it is rendered now, as a new render"}),
            serde_json::json!({"kind": "button", "id": "render.add", "icon": "image-plus", "label": "Add an image",
                "hint": "Add a picture from a file: a photo, a reference, a render made elsewhere (PNG or JPEG). It is copied into the project's Renders"}),
            serde_json::json!({"kind": "toggle", "id": "render.crop", "icon": "crop", "label": "Crop", "on": self.renders.crop.is_some(), "off": !has,
                "hint": "Crop this render to a shape: for a video, a phone, a square, a banner or a sheet. The crop is kept as a new render"}),
            serde_json::json!({"kind": "toggle", "id": "render.pipette", "icon": "pipette", "label": "Pick colours", "on": self.renders.pick, "off": !has,
                "hint": "Read a colour off the picture: click to keep it in Properties, Render, and copy its code"}),
            serde_json::json!({"kind": "button", "id": "render.settings", "icon": "visuals", "label": "Its look, light and view", "off": !has,
                "hint": "Give the 3D view this render's look, light and camera (where it stood, its lens), to render it again"}),
            serde_json::json!({"kind": "button", "id": "render.folder", "icon": "folder-open", "label": "Open the folder",
                "hint": "Open the project's Renders folder"}),
            serde_json::json!({"kind": "button", "id": "render.remove", "icon": "trash-2", "label": "Remove", "off": !has,
                "hint": "Take this render out of the list; its image moves to the folder's \"removed\""}),
        ]
    }

    /// The name a Render area's header shows.
    pub(crate) fn renders_title(&self) -> String {
        self.renders
            .current
            .clone()
            .unwrap_or_else(|| "No render yet".to_string())
    }

    /// The Render tab of Properties, with a Render area in front (MU115):
    /// what the render shown is and was made with, then what to do with it.
    pub(crate) fn render_tab(&mut self, ui: &mut egui::Ui) {
        let Some(name) = self.renders.current.clone() else {
            crate::props::note(ui, "No render kept yet. Keep one from the 3D view (Visuals, Output), or add a picture with the Render area's Add an image.");
            return;
        };
        let made = self.render_made(&name);
        let dims = self.renders.rgb.as_ref().map(|r| (r.width(), r.height()));
        let open = self.prop_header(ui, "render-what", "Render", true, Some("navigate"));
        crate::props::fold(ui, open, |ui| {
            ui.vertical(|ui| {
                let line = |ui: &mut egui::Ui, label: &str, text: String| {
                    crate::props::row(ui, label, |ui| {
                        ui.add(egui::Label::new(text).truncate());
                    });
                };
                line(ui, "Name", name.clone());
                match &made {
                    Some(m) => {
                        line(ui, "Look", m.look.label().to_string());
                        line(ui, "Date", crate::rendered::day_label(m.day));
                        line(ui, "Time", crate::rendered::hour_label(m.hour));
                        line(
                            ui,
                            "Sky",
                            creabim_render::sky::SkyKind::of_key(&m.sky)
                                .label()
                                .to_string(),
                        );
                        line(ui, "Exposure", format!("{:+.1} EV", m.ev));
                        if m.white >= 1000.0 && (m.white - 6500.0).abs() >= 1.0 {
                            line(ui, "Balance", format!("{:.0} K", m.white));
                        }
                        if let Some(c) = &m.camera {
                            line(
                                ui,
                                "Lens",
                                if c.ortho {
                                    "none: a parallel view".to_string()
                                } else {
                                    format!(
                                        "{:.0} mm{}",
                                        c.lens,
                                        if c.straight {
                                            ", verticals straight"
                                        } else {
                                            ""
                                        }
                                    )
                                },
                            );
                        }
                        line(ui, "Size", format!("{} × {} px", m.width, m.height));
                    }
                    None => {
                        // a picture added from a file, or a render whose
                        // settings were not kept
                        line(ui, "Kind", "A picture, not made here".to_string());
                        if let Some((w, h)) = dims {
                            line(ui, "Size", format!("{w} × {h} px"));
                        }
                    }
                }
            });
        });
        // the crop: its shape, what it gives, Keep (MU116: not a strip
        // over the picture)
        let open = self.prop_header(ui, "render-crop", "Crop", true, Some("navigate"));
        let mut act: Option<&'static str> = None;
        let mut keep = false;
        let mut crop = self.renders.crop;
        let mut own = if self.renders.own == [0, 0] {
            [3, 2]
        } else {
            self.renders.own
        };
        let (was, own_was) = (crop.map(|c| c.preset), own);
        crate::props::fold(ui, open, |ui| {
            ui.vertical(|ui| {
                let Some(c) = &mut crop else {
                    if crate::props::action(
                        ui,
                        "",
                        "Crop…",
                        "Crop it to a shape: for a video, a phone, a square, a banner or a sheet. The crop is kept as a new render",
                        true,
                    )
                    .clicked()
                    {
                        act = Some("render.crop");
                    }
                    return;
                };
                let last = CROPS.len() - 1;
                crate::props::row(ui, "Shape", |ui| {
                    egui::ComboBox::from_id_salt("render.crop.shape")
                        .icon(crate::props::combo_icon)
                        .selected_text(CROPS[c.preset.min(last)].0)
                        .width(ui.available_width())
                        .truncate()
                        .show_ui(ui, |ui| {
                            for (i, (name, _, made)) in CROPS.iter().enumerate() {
                                let r = ui.selectable_value(&mut c.preset, i, *name);
                                if *made != [0, 0] {
                                    r.on_hover_text(format!("Made for {} × {} px", made[0], made[1]));
                                }
                            }
                        });
                });
                if c.preset == last {
                    for (label, v) in [("Across", 0usize), ("Up", 1)] {
                        crate::props::row(ui, label, |ui| {
                            crate::props::fill(ui, egui::DragValue::new(&mut own[v]).range(1..=999))
                        });
                    }
                }
                if c.frame.is_positive() {
                    let (w, h) = (c.frame.width().round() as u32, c.frame.height().round() as u32);
                    crate::props::row(ui, "Gives", |ui| ui.label(format!("{w} × {h} px")));
                    let made = CROPS[c.preset.min(last)].2;
                    if made != [0, 0] {
                        crate::props::row(ui, "Made for", |ui| ui.label(format!("{} × {} px", made[0], made[1])));
                        if w + 1 < made[0] {
                            crate::props::note(
                                ui,
                                format!(
                                    "Smaller than that, so it will look soft. Render at {} × {} first: Visuals, Output.",
                                    made[0], made[1]
                                ),
                            );
                        }
                    }
                }
                if crate::props::action(
                    ui,
                    "",
                    "Keep the crop",
                    "Keep what is inside the frame as a new render; this one stays as it is",
                    c.frame.is_positive(),
                )
                .clicked()
                {
                    keep = true;
                }
                if crate::props::action(ui, "", "Cancel", "Stop cropping; nothing is kept", true).clicked() {
                    act = Some("render.crop");
                }
            });
        });
        if let Some(c) = &mut crop {
            // another shape: the largest of it that fits, placed by the area
            if Some(c.preset) != was || own != own_was {
                c.frame = egui::Rect::NOTHING;
            }
            self.renders.own = own;
            self.renders.crop = Some(*c);
            if keep {
                let k = self.renders.list.iter().find(|k| k.name == name).cloned();
                if let Some(k) = k {
                    let what = CROPS[c.preset.min(CROPS.len() - 1)].0;
                    let now = ui.input(|i| i.time);
                    self.keep_crop(&k, c.frame, what, now);
                }
            }
        }
        // colours read off the picture (the pipette)
        let open = self.prop_header(ui, "render-colour", "Colour", true, Some("navigate"));
        let mut copy: Option<[u8; 3]> = None;
        let (under, picked, picking) = (
            self.renders.under,
            self.renders.picked.clone(),
            self.renders.pick,
        );
        crate::props::fold(ui, open, |ui| {
            ui.vertical(|ui| {
                crate::props::row(ui, "Pointer", |ui| match under {
                    Some((at, c)) => {
                        swatch(ui, c);
                        ui.add(egui::Label::new(hex(c)).truncate())
                            .on_hover_text(format!("Red {}, green {}, blue {}, at pixel {}, {}", c[0], c[1], c[2], at[0], at[1]));
                    }
                    None => {
                        ui.label("not over the picture");
                    }
                });
                for (i, c) in picked.iter().enumerate() {
                    crate::props::row(ui, if i == 0 { "Picked" } else { "" }, |ui| {
                        swatch(ui, *c);
                        let text = hex(*c);
                        if ui
                            .add(egui::Label::new(text).truncate().sense(egui::Sense::click()))
                            .on_hover_text(format!("Red {}, green {}, blue {}. Click to copy the code", c[0], c[1], c[2]))
                            .clicked()
                        {
                            copy = Some(*c);
                        }
                    });
                }
                if crate::props::action(
                    ui,
                    "",
                    if picking { "Stop picking" } else { "Pick colours" },
                    "Click the picture to keep the colour under the pointer here, and copy its code",
                    true,
                )
                .clicked()
                {
                    act = Some("render.pipette");
                }
                if picked.is_empty() {
                    crate::props::note(ui, "Colours you pick are listed here until CreaBIM closes.");
                }
            });
        });
        if let Some(c) = copy {
            ui.ctx().copy_text(hex(c));
            self.status = format!("Copied {}.", hex(c));
        }
        let open = self.prop_header(ui, "render-do", "With it", true, Some("navigate"));
        crate::props::fold(ui, open, |ui| {
            ui.vertical(|ui| {
                for (id, text, tip, on) in [
                    ("render.settings", "Back to its view",
                     "Give the 3D view this render's look, light and camera (where it stood, its lens), to render it again", made.is_some()),
                    ("render.folder", "Open the folder", "Open the project's Renders folder", true),
                    ("render.remove", "Remove", "Take it out of the list; its image moves to the folder's \"removed\"", true),
                ] {
                    if crate::props::action(ui, "", text, tip, on).clicked() {
                        act = Some(id);
                    }
                }
            });
        });
        if let Some(id) = act {
            let ctx = ui.ctx().clone();
            self.renders_action(&ctx, id, "");
        }
    }

    /// The browser's Renders group.
    pub(crate) fn renders_group(&self) -> serde_json::Value {
        let rows: Vec<serde_json::Value> = self
            .renders
            .list
            .iter()
            .map(|k| {
                serde_json::json!({
                    "id": format!("render:{}", k.name), "name": k.name, "icon": "image",
                    "sel": self.renders.current.as_deref() == Some(k.name.as_str()),
                    "hint": format!("{}: click to open it in a Render view", k.name),
                })
            })
            .collect();
        serde_json::json!({
            "key": "renders", "title": "Renders", "icon": "image", "open": !rows.is_empty(),
            "rows": rows,
            "note": if self.renders.list.is_empty() { "none kept yet".to_string() } else { String::new() },
            "acts": [
                {"id": "render.keep", "icon": "plus", "hint": "Keep the 3D view as it is rendered now"},
                {"id": "render.add", "icon": "image-plus", "hint": "Add a picture from a file (PNG or JPEG)"},
                {"id": "render.folder", "icon": "ellipsis", "hint": "Open the project's Renders folder"},
            ],
        })
    }

    /// Keep the part of render `k` inside `frame` (its pixels) as a new
    /// render named after it and `what`, with what it was made with.
    fn keep_crop(&mut self, k: &Kept, frame: egui::Rect, what: &str, now: f64) {
        let Ok(im) = image::open(&k.path) else {
            self.status = format!("Could not read {}", k.path.display());
            return;
        };
        let x = (frame.min.x.round().max(0.0) as u32).min(im.width().saturating_sub(1));
        let y = (frame.min.y.round().max(0.0) as u32).min(im.height().saturating_sub(1));
        let w = (frame.width().round() as u32).clamp(1, im.width() - x);
        let h = (frame.height().round() as u32).clamp(1, im.height() - y);
        let taken: Vec<String> = self.renders.list.iter().map(|k| k.name.clone()).collect();
        let short = what.split(' ').next().unwrap_or(what);
        let name = free_name(&format!("{} · {short}", k.name), &taken);
        let path = k.path.with_file_name(format!("{name}.png"));
        if let Err(e) = im.crop_imm(x, y, w, h).to_rgb8().save(&path) {
            self.status = format!("Could not keep the crop: {e}");
            return;
        }
        if let Some(mut made) = self.render_made(&k.name) {
            made.width = w as usize;
            made.height = h as usize;
            if let Ok(text) = serde_json::to_string_pretty(&made) {
                let _ = std::fs::write(path.with_extension("json"), text);
            }
        }
        self.renders.crop = None;
        self.renders_refresh(now, true);
        self.open_render(&name);
        self.status = format!(
            "Kept the crop as \"{name}\" ({w} × {h} px). \"{}\" is as it was.",
            k.name
        );
    }

    /// A Render area: the render picked, on the theme's ground, viewed as
    /// an image (zoom, pan, fit), with what it was made with under it;
    /// with Crop on, a frame to set (its shapes are in Properties).
    pub(crate) fn renders_area(&mut self, ui: &mut egui::Ui, full: egui::Rect) {
        let now = ui.input(|i| i.time);
        // not while something is dragged: the folder is read from disk
        if !ui.input(|i| i.pointer.any_down()) {
            self.renders_refresh(now, false);
        }
        let pal = self.shell.theme.palette();
        ui.painter_at(full).rect_filled(full, 0.0, pal.panel2);
        let Some(k) = self
            .renders
            .current
            .clone()
            .or_else(|| self.renders.list.first().map(|k| k.name.clone()))
            .and_then(|c| self.renders.list.iter().find(|k| k.name == c).cloned())
        else {
            self.renders.crop = None;
            ui.painter_at(full).text(
                full.center(),
                egui::Align2::CENTER_CENTER,
                "No render kept yet.\nPick a Style or Rendered look in the 3D view, then Keep as a render\n(Properties, Visuals, Output, or the + of the browser's Renders).",
                egui::FontId::proportional(13.0),
                pal.muted,
            );
            return;
        };
        self.renders.current = Some(k.name.clone());
        if self.renders.tex.as_ref().is_none_or(|(n, _)| *n != k.name) {
            // another render: fitted again, and no crop carried over
            self.renders.zoom = None;
            self.renders.pan = egui::Vec2::ZERO;
            if let Some(c) = &mut self.renders.crop {
                c.frame = egui::Rect::NOTHING;
            }
            let rgb = image::open(&k.path).ok().map(|im| im.to_rgb8());
            self.renders.tex = rgb.as_ref().map(|rgb| {
                let size = [rgb.width() as usize, rgb.height() as usize];
                let ci = egui::ColorImage::from_rgb(size, rgb.as_raw());
                (
                    k.name.clone(),
                    ui.ctx().load_texture(
                        format!("render:{}", k.name),
                        ci,
                        egui::TextureOptions::LINEAR,
                    ),
                )
            });
            self.renders.rgb = rgb;
        }
        let Some(tex) = self.renders.tex.as_ref().map(|(_, t)| t.clone()) else {
            ui.painter_at(full).text(
                full.center(),
                egui::Align2::CENTER_CENTER,
                format!("Could not read {}", k.path.display()),
                egui::FontId::proportional(13.0),
                pal.muted,
            );
            return;
        };
        let size = tex.size_vec2();
        if self.renders.own == [0, 0] {
            self.renders.own = [3, 2];
        }
        let own = self.renders.own;
        let ratio_of = |preset: usize| -> Option<f32> {
            let (_, shape, _) = CROPS[preset.min(CROPS.len() - 1)];
            if preset == CROPS.len() - 1 {
                Some(own[0].max(1) as f32 / own[1].max(1) as f32)
            } else if shape == [0, 0] {
                None
            } else {
                Some(shape[0] as f32 / shape[1] as f32)
            }
        };
        // the crop's shapes and Keep are in Properties, Render (MU116):
        // here only its frame, placed when it has none yet
        if let Some(c) = &mut self.renders.crop {
            if !c.frame.is_positive() {
                c.frame = crop_frame(size, ratio_of(c.preset));
            }
        }
        let body = full;
        let painter = ui.painter_at(body);
        // an image viewer (MU113): fitted at first, never enlarged past
        // its own pixels until asked; the wheel (Ctrl + wheel too) zooms
        // about the pointer, a drag or two fingers pan, a double-click fits
        let room = body.shrink2(egui::vec2(16.0, 28.0));
        // min then max, not clamp: a picture of zero size gives NaN, which
        // min and max turn into 1 where clamp would pass it on
        #[allow(clippy::manual_clamp)]
        let fit = (room.width() / size.x)
            .min(room.height() / size.y)
            .min(1.0)
            .max(0.01);
        let resp = ui.interact(
            body,
            ui.id().with("render.view"),
            egui::Sense::click_and_drag(),
        );
        let mut zoom = self.renders.zoom.unwrap_or(fit);
        let mut pan = self.renders.pan;
        let home = egui::pos2(room.center().x, room.center().y - 6.0);
        let zoom_to = |zoom: &mut f32, pan: &mut egui::Vec2, to: f32, about: egui::Pos2| {
            let to = to.clamp((fit * 0.25).min(0.05), 16.0);
            let c = home + *pan;
            *pan = (about - (about - c) * (to / *zoom)) - home;
            *zoom = to;
        };
        let mut refit = resp.double_clicked() && self.renders.crop.is_none() && !self.renders.pick;
        let mut zoomed = false;
        if resp.hovered() {
            let (two, scroll) = crate::planview::wheel(ui);
            pan += two;
            if scroll != 0.0 {
                // as a plan zooms (MU114)
                let about = resp.hover_pos().unwrap_or(home);
                let to = zoom * (1.0 + scroll * 0.002).clamp(0.5, 2.0);
                zoom_to(&mut zoom, &mut pan, to, about);
                zoomed = true;
            }
        }
        // its viewing buttons, as a plan's: a click steps, a press held and
        // dragged up or down zooms smoothly; the hand pans (MU114)
        let col = egui::Rect::from_min_size(
            egui::pos2(body.right() - 40.0, body.top() + 10.0),
            egui::vec2(30.0, 5.0 * 28.0 + 4.0),
        );
        let mut buttons: Vec<(egui::Rect, &str, bool)> = vec![];
        for (n, (icon, tip)) in [
            ("zoom-in", "Zoom in: click a step, or drag up and down"),
            ("zoom-out", "Zoom out: click a step, or drag up and down"),
            ("scan", "Fit the whole image (or double-click it)"),
            ("square", "Its real size: one pixel to a pixel"),
            ("hand", "Pan: drag here to move the image"),
        ]
        .into_iter()
        .enumerate()
        {
            let r = egui::Rect::from_min_size(
                egui::pos2(col.left() + 2.0, col.top() + 2.0 + 28.0 * n as f32),
                egui::vec2(26.0, 26.0),
            );
            let b = ui
                .interact(
                    r,
                    ui.id().with(("render.nav", n)),
                    egui::Sense::click_and_drag(),
                )
                .on_hover_text(tip);
            match n {
                0 | 1 if b.dragged() => {
                    let to = zoom * (1.0 - b.drag_delta().y * 0.01).clamp(0.5, 2.0);
                    zoom_to(&mut zoom, &mut pan, to, home);
                    zoomed = true;
                }
                0 if b.clicked() => {
                    let to = zoom * 1.25;
                    zoom_to(&mut zoom, &mut pan, to, home);
                    zoomed = true;
                }
                1 if b.clicked() => {
                    let to = zoom / 1.25;
                    zoom_to(&mut zoom, &mut pan, to, home);
                    zoomed = true;
                }
                2 if b.clicked() => refit = true,
                3 if b.clicked() => {
                    let px = 1.0 / ui.ctx().pixels_per_point();
                    zoom_to(&mut zoom, &mut pan, px, home);
                    zoomed = true;
                }
                4 if b.dragged() => pan += b.drag_delta(),
                _ => {}
            }
            buttons.push((r, icon, b.hovered() || b.dragged()));
        }
        if refit {
            zoom = fit;
            pan = egui::Vec2::ZERO;
        }
        // a drag in the view: with Crop on, the frame or a corner of it
        // where it starts on one; else the image
        let at0 = egui::Rect::from_center_size(home + pan, size * zoom);
        let to_img = |p: egui::Pos2| ((p - at0.min) / zoom).to_pos2();
        let to_screen = |p: egui::Pos2| at0.min + p.to_vec2() * zoom;
        // the pixel under the pointer, for the colour readout
        self.renders.under = resp.hover_pos().and_then(|p| {
            let q = to_img(p);
            let rgb = self.renders.rgb.as_ref()?;
            (q.x >= 0.0 && q.y >= 0.0 && (q.x as u32) < rgb.width() && (q.y as u32) < rgb.height())
                .then(|| {
                    (
                        [q.x as u32, q.y as u32],
                        rgb.get_pixel(q.x as u32, q.y as u32).0,
                    )
                })
        });
        let mut cursor: Option<egui::CursorIcon> = None;
        if let Some(mut crop) = self.renders.crop {
            let ratio = ratio_of(crop.preset);
            let f = crop.frame;
            let corners = [
                f.left_top(),
                f.right_top(),
                f.right_bottom(),
                f.left_bottom(),
            ];
            let fs = egui::Rect::from_two_pos(to_screen(f.min), to_screen(f.max));
            // what is under a point: a corner, a side, the frame, the image
            let under = |p: egui::Pos2| -> Grab {
                if let Some(i) = corners
                    .iter()
                    .position(|c| to_screen(*c).distance(p) < 10.0)
                {
                    return Grab::Corner(corners[(i + 2) % 4]);
                }
                let along_x = p.x > fs.left() && p.x < fs.right();
                let along_y = p.y > fs.top() && p.y < fs.bottom();
                for (i, edge) in [fs.left(), fs.top(), fs.right(), fs.bottom()]
                    .into_iter()
                    .enumerate()
                {
                    let (d, along) = if i % 2 == 0 {
                        ((p.x - edge).abs(), along_y)
                    } else {
                        ((p.y - edge).abs(), along_x)
                    };
                    if d < 7.0 && along {
                        return Grab::Side(i);
                    }
                }
                if fs.contains(p) {
                    Grab::Held(to_img(p) - f.min)
                } else {
                    Grab::Image
                }
            };
            let icon = |g: Grab| match g {
                Grab::Corner(fixed) => {
                    // the corner held is the one opposite `fixed`
                    let c = f.center();
                    Some(if (fixed.x < c.x) == (fixed.y < c.y) {
                        egui::CursorIcon::ResizeNwSe
                    } else {
                        egui::CursorIcon::ResizeNeSw
                    })
                }
                Grab::Side(i) if i % 2 == 0 => Some(egui::CursorIcon::ResizeHorizontal),
                Grab::Side(_) => Some(egui::CursorIcon::ResizeVertical),
                Grab::Held(_) => Some(egui::CursorIcon::Move),
                Grab::Image => None,
            };
            if let Some(p) = resp.hover_pos() {
                cursor = icon(crop.grab.unwrap_or_else(|| under(p)));
            }
            if resp.drag_started() {
                // where the press began, not where the drag was noticed a
                // few points on: a handle was missed and the frame trailed
                // the pointer by that much (MU116)
                let origin = ui
                    .input(|i| i.pointer.press_origin())
                    .or(resp.interact_pointer_pos());
                crop.grab = origin.map(under);
            }
            if resp.dragged() {
                let p = resp.interact_pointer_pos();
                match (crop.grab, p) {
                    (Some(Grab::Corner(fixed)), Some(p)) => {
                        crop.frame = crop_resized(fixed, to_img(p), ratio, size)
                    }
                    (Some(Grab::Side(i)), Some(p)) => {
                        crop.frame = crop_side(f, i, to_img(p), ratio, size)
                    }
                    (Some(Grab::Held(by)), Some(p)) => {
                        let min = to_img(p) - by;
                        let min = egui::pos2(
                            min.x.clamp(0.0, (size.x - f.width()).max(0.0)),
                            min.y.clamp(0.0, (size.y - f.height()).max(0.0)),
                        );
                        crop.frame = egui::Rect::from_min_size(min, f.size());
                    }
                    _ => pan += resp.drag_delta(),
                }
            }
            if resp.drag_stopped() {
                crop.grab = None;
            }
            self.renders.crop = Some(crop);
        } else if resp.dragged() {
            pan += resp.drag_delta();
        }
        // the pipette: a click on the picture keeps its colour
        if self.renders.pick && self.renders.crop.is_none() {
            if let Some((_, c)) = self.renders.under {
                cursor = Some(egui::CursorIcon::Crosshair);
                if resp.clicked() {
                    self.pick_colour(c);
                    ui.ctx().copy_text(hex(c));
                }
            }
        }
        if let Some(c) = cursor {
            ui.ctx().set_cursor_icon(c);
        }
        if refit {
            self.renders.zoom = None;
            ui.ctx().request_repaint();
        } else if zoomed {
            self.renders.zoom = Some(zoom);
            ui.ctx().request_repaint();
        }
        self.renders.pan = pan;
        let at = egui::Rect::from_center_size(home + pan, size * zoom);
        painter.image(
            tex.id(),
            at,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
        painter.rect_stroke(
            at,
            0.0,
            egui::Stroke::new(1.0, pal.border),
            egui::StrokeKind::Outside,
        );
        let percent = (zoom * ui.ctx().pixels_per_point() * 100.0).round();
        let mut caption = match self.render_made(&k.name) {
            Some(m) => format!(
                "{} · {} {} · {:+.1} EV · {} × {} px · {percent:.0} %",
                m.look.label(),
                crate::rendered::day_label(m.day),
                crate::rendered::hour_label(m.hour),
                m.ev,
                m.width,
                m.height
            ),
            None => format!("{} × {} px · {percent:.0} %", size.x as u32, size.y as u32),
        };
        if let (Some((at_px, c)), None) = (self.renders.under, self.renders.crop) {
            caption += &format!(" · {}, {} · {}", at_px[0], at_px[1], hex(c));
        }
        let mut caption_ink = pal.muted;
        // the crop: what is outside it shaded, its frame, thirds and
        // corners, and what it gives
        if let Some(crop) = self.renders.crop {
            let s = |p: egui::Pos2| at.min + p.to_vec2() * zoom;
            let fs = egui::Rect::from_two_pos(s(crop.frame.min), s(crop.frame.max));
            let shade = egui::Color32::from_black_alpha(150);
            for r in [
                egui::Rect::from_min_max(at.min, egui::pos2(fs.left(), at.bottom())),
                egui::Rect::from_min_max(egui::pos2(fs.right(), at.top()), at.max),
                egui::Rect::from_min_max(
                    egui::pos2(fs.left(), at.top()),
                    egui::pos2(fs.right(), fs.top()),
                ),
                egui::Rect::from_min_max(
                    egui::pos2(fs.left(), fs.bottom()),
                    egui::pos2(fs.right(), at.bottom()),
                ),
            ] {
                if r.is_positive() {
                    painter.rect_filled(r, 0.0, shade);
                }
            }
            let thin = egui::Stroke::new(1.0, egui::Color32::from_white_alpha(110));
            for t in [1.0 / 3.0, 2.0 / 3.0] {
                let x = fs.left() + fs.width() * t;
                let y = fs.top() + fs.height() * t;
                painter.line_segment([egui::pos2(x, fs.top()), egui::pos2(x, fs.bottom())], thin);
                painter.line_segment([egui::pos2(fs.left(), y), egui::pos2(fs.right(), y)], thin);
            }
            painter.rect_stroke(
                fs,
                0.0,
                egui::Stroke::new(3.0, egui::Color32::from_black_alpha(120)),
                egui::StrokeKind::Outside,
            );
            painter.rect_stroke(
                fs,
                0.0,
                egui::Stroke::new(1.5, egui::Color32::WHITE),
                egui::StrokeKind::Outside,
            );
            for c in [
                fs.left_top(),
                fs.right_top(),
                fs.right_bottom(),
                fs.left_bottom(),
            ] {
                let h = egui::Rect::from_center_size(c, egui::vec2(9.0, 9.0));
                painter.rect_filled(h, 2.0, egui::Color32::WHITE);
                painter.rect_stroke(
                    h,
                    2.0,
                    egui::Stroke::new(1.0, egui::Color32::from_black_alpha(160)),
                    egui::StrokeKind::Outside,
                );
            }
            // its sides' handles: a short bar at the middle of each
            for (i, c) in [
                fs.left_center(),
                fs.center_top(),
                fs.right_center(),
                fs.center_bottom(),
            ]
            .into_iter()
            .enumerate()
            {
                let wh = if i % 2 == 0 {
                    egui::vec2(5.0, 22.0)
                } else {
                    egui::vec2(22.0, 5.0)
                };
                if fs.width() < 60.0 && i % 2 == 1 || fs.height() < 60.0 && i % 2 == 0 {
                    continue;
                }
                let h = egui::Rect::from_center_size(c, wh);
                painter.rect_filled(h, 2.5, egui::Color32::WHITE);
                painter.rect_stroke(
                    h,
                    2.5,
                    egui::Stroke::new(1.0, egui::Color32::from_black_alpha(160)),
                    egui::StrokeKind::Outside,
                );
            }
            let (name, _, made) = CROPS[crop.preset.min(CROPS.len() - 1)];
            // a channel banner's band that every device shows
            if name == "Banner" {
                let band = egui::Rect::from_center_size(
                    fs.center(),
                    egui::vec2(fs.width() * 1546.0 / 2560.0, fs.height() * 423.0 / 1440.0),
                );
                let orange = egui::Color32::from_rgb(0xE6, 0x78, 0x1E);
                let pts = [
                    band.left_top(),
                    band.right_top(),
                    band.right_bottom(),
                    band.left_bottom(),
                    band.left_top(),
                ];
                painter.extend(egui::Shape::dashed_line(
                    &pts,
                    egui::Stroke::new(1.0, orange),
                    5.0,
                    4.0,
                ));
            }
            let (w, h) = (
                crop.frame.width().round() as u32,
                crop.frame.height().round() as u32,
            );
            caption = format!("{name} · the crop is {w} × {h} px");
            if made != [0, 0] {
                caption += &format!(" · made for {} × {}", made[0], made[1]);
                if w + 1 < made[0] {
                    // why, and what to do, is in Properties, Crop
                    caption += " · smaller than that";
                    caption_ink = egui::Color32::from_rgb(0xD0, 0x8A, 0x2A);
                }
            }
        }
        // the buttons, over the image
        painter.rect_filled(col, 8.0, pal.panel);
        for (r, icon, hot) in buttons {
            if hot {
                painter.rect_filled(r, 5.0, pal.raise2);
            }
            let ic = egui::Rect::from_center_size(r.center(), egui::vec2(16.0, 16.0));
            crate::svgicon::paint(&painter, ic, icon, if hot { pal.text } else { pal.muted });
        }
        let below = egui::pos2(
            at.center().x.clamp(body.left() + 40.0, body.right() - 40.0),
            (at.bottom() + 8.0).min(body.bottom() - 18.0),
        );
        painter.text(
            below,
            egui::Align2::CENTER_TOP,
            caption,
            egui::FontId::proportional(12.0),
            caption_ink,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_render_gets_a_name_no_other_has_and_a_file_can_take() {
        assert_eq!(free_name("Toon · 19 Jul 10h00", &[]), "Toon · 19 Jul 10h00");
        let taken = vec![
            "Toon · 19 Jul 10h00".to_string(),
            "toon · 19 jul 10h00 2".to_string(),
        ];
        assert_eq!(
            free_name("Toon · 19 Jul 10h00", &taken),
            "Toon · 19 Jul 10h00 3"
        );
        // nothing a file name can't hold
        assert_eq!(free_name("Quick: 10:00 <a/b>", &[]), "Quick- 10-00 -a-b-");
    }

    #[test]
    fn a_render_is_kept_beside_the_project_listed_opened_and_removed() {
        let dir = std::env::temp_dir().join(format!("creabim-renders-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let project = dir.join("house.creabim");
        std::fs::write(&project, b"x").unwrap();
        let mut app = super::super::test_app();
        app.path = project.to_string_lossy().to_string();
        assert_eq!(app.renders_dir(), dir.join("house_renders"));
        // a small image stands for the 3D view's render
        app.rendered.image = Some(std::sync::Arc::new(creabim_render::util::Image {
            w: 4,
            h: 3,
            px: vec![glam::Vec3::splat(0.5); 12],
        }));
        app.set_look(crate::render3d::RenderMode::Toon);
        app.keep_render(0.0);
        app.keep_render(1.0);
        // two files, two names, the newest shown, in a Render area
        assert_eq!(app.renders.list.len(), 2, "{}", app.status);
        let names: Vec<&str> = app.renders.list.iter().map(|k| k.name.as_str()).collect();
        assert!(names.iter().all(|n| n.starts_with("Toon · ")), "{names:?}");
        assert_ne!(names[0], names[1]);
        assert!(app
            .renders
            .list
            .iter()
            .all(|k| k.path.exists() && k.path.with_extension("json").exists()));
        assert!(app.areas.root.areas().contains(&Shows::Renders));
        let shown = app.renders.current.clone().unwrap();
        // what it was made with comes back from the file beside it
        let made = app.render_made(&shown).unwrap();
        assert_eq!(
            (made.look, made.width, made.height),
            (crate::render3d::RenderMode::Toon, 4, 3)
        );
        // the browser lists both, the one shown marked
        let g = app.renders_group();
        assert_eq!(g["rows"].as_array().unwrap().len(), 2);
        assert!(g["rows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["sel"] == true));
        // removed, it leaves the list and its image is kept in "removed"
        app.remove_render(2.0);
        assert_eq!(app.renders.list.len(), 1);
        assert!(dir
            .join("house_renders")
            .join("removed")
            .join(format!("{shown}.png"))
            .exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_render_is_made_at_outputs_size_whatever_the_views() {
        let dir = std::env::temp_dir().join(format!("creabim-final-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let project = dir.join("house.creabim");
        std::fs::write(&project, b"x").unwrap();
        let mut app = super::super::test_app();
        app.path = project.to_string_lossy().to_string();
        app.set_look(crate::render3d::RenderMode::Clay);
        // small, so the test is quick; taller than wide, unlike any view
        app.shell.settings.render_size = [48, 80];
        app.render_and_keep(0.0);
        assert!(app.renders.making.is_some(), "{}", app.status);
        let ctx = egui::Context::default();
        let t0 = std::time::Instant::now();
        while app.renders.making.is_some() && t0.elapsed().as_secs() < 120 {
            app.renders_poll(&ctx);
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(
            app.renders.making.is_none(),
            "still rendering after two minutes"
        );
        assert_eq!(app.renders.list.len(), 1, "{}", app.status);
        let made = app.render_made(&app.renders.list[0].name).unwrap();
        assert_eq!((made.width, made.height), (48, 80));
        let im = image::open(&app.renders.list[0].path).unwrap();
        assert_eq!((im.width(), im.height()), (48, 80));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_crop_frame_fits_its_shape_and_keeps_it_when_a_corner_moves() {
        let size = egui::vec2(1146.0, 345.0);
        // Tall in a wide image: as high as the image, centred
        let tall = crop_frame(size, Some(9.0 / 16.0));
        assert!(
            (tall.height() - 345.0).abs() < 0.01 && (tall.width() - 194.06).abs() < 0.1,
            "{tall:?}"
        );
        assert!((tall.center().x - 573.0).abs() < 0.01);
        // a shape wider than the image: as wide as it
        let strip = crop_frame(size, Some(8.0));
        assert!((strip.width() - 1146.0).abs() < 0.01 && (strip.height() - 143.25).abs() < 0.01);
        // a corner dragged keeps the shape and stays inside the image
        let r = crop_resized(
            egui::pos2(100.0, 100.0),
            egui::pos2(5000.0, 5000.0),
            Some(16.0 / 9.0),
            size,
        );
        assert!((r.width() / r.height() - 16.0 / 9.0).abs() < 1e-3, "{r:?}");
        assert!(r.max.x <= 1146.01 && r.max.y <= 345.01 && r.min == egui::pos2(100.0, 100.0));
        // dragged up and left past the corner, it turns over, not negative
        let r = crop_resized(egui::pos2(100.0, 100.0), egui::pos2(40.0, 70.0), None, size);
        assert_eq!(
            (r.min, r.max),
            (egui::pos2(40.0, 70.0), egui::pos2(100.0, 100.0))
        );
    }

    #[test]
    fn a_crop_frames_side_moves_alone_or_keeps_the_shape() {
        let size = egui::vec2(1000.0, 500.0);
        let f = egui::Rect::from_min_max(egui::pos2(200.0, 100.0), egui::pos2(600.0, 400.0));
        // free: the right side alone, and not past the image
        let r = crop_side(f, 2, egui::pos2(700.0, 9.0), None, size);
        assert_eq!((r.min, r.max), (f.min, egui::pos2(700.0, 400.0)));
        let r = crop_side(f, 2, egui::pos2(5000.0, 0.0), None, size);
        assert_eq!(r.max.x, 1000.0);
        // the top side: the bottom stays, never turned over
        let r = crop_side(f, 1, egui::pos2(0.0, 50.0), None, size);
        assert_eq!((r.min, r.max), (egui::pos2(200.0, 50.0), f.max));
        let r = crop_side(f, 1, egui::pos2(0.0, 900.0), None, size);
        assert!(r.height() >= 8.0 && r.max.y == 400.0, "{r:?}");
        // kept to a shape: the left side out, the height follows about
        // the middle, and stops where the image ends
        let sq = egui::Rect::from_center_size(egui::pos2(500.0, 250.0), egui::vec2(200.0, 200.0));
        let r = crop_side(sq, 0, egui::pos2(300.0, 0.0), Some(1.0), size);
        assert_eq!(
            (r.min, r.max),
            (egui::pos2(300.0, 100.0), egui::pos2(600.0, 400.0))
        );
        let r = crop_side(sq, 0, egui::pos2(-50.0, 0.0), Some(1.0), size);
        assert!(
            (r.width() - 500.0).abs() < 0.01 && (r.height() - 500.0).abs() < 0.01,
            "{r:?}"
        );
        assert!(r.min.y >= -0.01 && r.max.y <= 500.01 && r.max.x == 600.0);
    }

    #[test]
    fn what_a_render_was_made_with_is_read_once_until_the_folder_is_read_again() {
        let dir = std::env::temp_dir().join(format!("creabim-made-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let project = dir.join("house.creabim");
        std::fs::write(&project, b"x").unwrap();
        let mut app = super::super::test_app();
        app.path = project.to_string_lossy().to_string();
        app.rendered.image = Some(std::sync::Arc::new(creabim_render::util::Image {
            w: 40,
            h: 20,
            px: vec![glam::Vec3::splat(0.5); 800],
        }));
        app.set_look(crate::render3d::RenderMode::Toon);
        app.keep_render(0.0);
        let k = app.renders.list[0].clone();
        assert!(app.render_made(&k.name).is_some());
        // its file gone: nothing is read again, so it is still known
        std::fs::remove_file(k.path.with_extension("json")).unwrap();
        assert!(
            app.render_made(&k.name).is_some(),
            "read from disk on every ask"
        );
        // until the folder is read again
        app.renders_refresh(10.0, true);
        assert!(app.render_made(&k.name).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_picture_from_a_file_joins_the_renders_and_the_file_stays() {
        let dir =
            std::env::temp_dir().join(format!("creabim-addimage-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let project = dir.join("house.creabim");
        std::fs::write(&project, b"x").unwrap();
        let photo = dir.join("site photo.jpg");
        image::RgbImage::from_pixel(6, 4, image::Rgb([200, 120, 40]))
            .save(&photo)
            .unwrap();
        let mut app = super::super::test_app();
        app.path = project.to_string_lossy().to_string();
        app.add_image(&photo, 0.0);
        assert_eq!(app.renders.list.len(), 1, "{}", app.status);
        let k = app.renders.list[0].clone();
        assert_eq!(k.name, "site photo");
        let im = image::open(&k.path).unwrap();
        assert_eq!((im.width(), im.height()), (6, 4));
        assert!(photo.exists() && k.path.starts_with(app.renders_dir()));
        // it is shown, and has no look or light of ours
        assert_eq!(app.renders.current.as_deref(), Some("site photo"));
        assert!(app.render_made(&k.name).is_none());
        // the same picture again takes a name of its own
        app.add_image(&photo, 1.0);
        assert_eq!(app.renders.list.len(), 2);
        // what is not a picture says so and adds nothing
        app.add_image(&project, 2.0);
        assert!(app.status.contains("Could not read"), "{}", app.status);
        assert_eq!(app.renders.list.len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn picked_colours_are_kept_newest_first_none_twice() {
        let mut app = super::super::test_app();
        assert_eq!(hex([255, 0, 171]), "#FF00AB");
        app.pick_colour([1, 2, 3]);
        app.pick_colour([4, 5, 6]);
        app.pick_colour([1, 2, 3]);
        assert_eq!(app.renders.picked, vec![[1, 2, 3], [4, 5, 6]]);
        for i in 0..20 {
            app.pick_colour([i, 0, 0]);
        }
        assert_eq!(app.renders.picked.len(), 8);
        assert_eq!(app.renders.picked[0], [19, 0, 0]);
    }

    #[test]
    fn a_render_keeps_its_camera_lens_and_balance_and_gives_them_back() {
        let dir = std::env::temp_dir().join(format!("creabim-madecam-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let project = dir.join("house.creabim");
        std::fs::write(&project, b"x").unwrap();
        let mut app = super::super::test_app();
        app.path = project.to_string_lossy().to_string();
        app.rendered.image = Some(std::sync::Arc::new(creabim_render::util::Image {
            w: 40,
            h: 20,
            px: vec![glam::Vec3::splat(0.5); 800],
        }));
        app.set_look(crate::render3d::RenderMode::Toon);
        // a view of its own: turned, a wide lens, verticals straight, lamplight
        app.camera.yaw = 0.7;
        app.camera.pitch = 0.2;
        app.camera.target = glam::Vec3::new(1.0, 2.0, 3.0);
        app.camera.set_lens_mm(18.0);
        app.camera.straight = true;
        app.shell.settings.render_white = 2700.0;
        let stood = app.camera.clone();
        app.keep_render(0.0);
        let name = app.renders.list[0].name.clone();
        let made = app.render_made(&name).unwrap();
        assert_eq!(made.white, 2700.0);
        let cam = made.camera.unwrap();
        assert!((cam.lens - 18.0).abs() < 1e-3 && cam.straight);
        // the view is turned away and reset; the render brings it back
        app.camera = crate::camera::OrbitCamera::default();
        app.shell.settings.render_white = 6500.0;
        assert!(app.renders_action(&egui::Context::default(), "render.settings", ""));
        assert_eq!(app.camera.view_proj(1.5), stood.view_proj(1.5));
        assert!(app.camera.straight && (app.camera.lens_mm() - 18.0).abs() < 1e-3);
        assert_eq!(app.shell.settings.render_white, 2700.0);
        assert!(
            app.status.contains("stands where it was made from"),
            "{}",
            app.status
        );
        // a render kept before any of this reads as daylight, with no camera
        let old: Made = serde_json::from_str(
            r#"{"look":"Toon","day":200.0,"hour":10.0,"ev":0.0,"width":4,"height":4}"#,
        )
        .unwrap();
        assert_eq!((old.white, old.camera, old.strength), (0.0, None, 1.0));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_crop_is_kept_as_a_new_render_and_the_first_stays() {
        let dir = std::env::temp_dir().join(format!("creabim-crop-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let project = dir.join("house.creabim");
        std::fs::write(&project, b"x").unwrap();
        let mut app = super::super::test_app();
        app.path = project.to_string_lossy().to_string();
        app.rendered.image = Some(std::sync::Arc::new(creabim_render::util::Image {
            w: 40,
            h: 20,
            px: vec![glam::Vec3::splat(0.5); 800],
        }));
        app.set_look(crate::render3d::RenderMode::Toon);
        app.keep_render(0.0);
        let first = app.renders.list[0].clone();
        app.keep_crop(
            &first,
            egui::Rect::from_min_size(egui::pos2(10.0, 0.0), egui::vec2(11.25, 20.0)),
            "Tall 9:16",
            1.0,
        );
        assert_eq!(app.renders.list.len(), 2, "{}", app.status);
        let new = app
            .renders
            .list
            .iter()
            .find(|k| k.name != first.name)
            .unwrap()
            .clone();
        assert!(new.name.ends_with("· Tall"), "{}", new.name);
        let im = image::open(&new.path).unwrap();
        assert_eq!((im.width(), im.height()), (11, 20));
        // the first is whole still, and the crop says its own size
        let im = image::open(&first.path).unwrap();
        assert_eq!((im.width(), im.height()), (40, 20));
        let made = app.render_made(&new.name).unwrap();
        assert_eq!((made.width, made.height), (11, 20));
        assert_eq!(app.renders.current.as_deref(), Some(new.name.as_str()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn renders_have_a_folder_a_group_and_an_area() {
        let mut app = super::super::test_app();
        // a project not saved yet keeps them with the app's own files
        assert!(
            app.renders_dir().ends_with("renders_unsaved")
                || app.renders_dir().ends_with("creabim_renders")
        );
        let g = app.renders_group();
        assert_eq!(g["title"], "Renders");
        assert!(g["acts"]
            .as_array()
            .is_some_and(|a| a.iter().any(|x| x["id"] == "render.keep")));
        // opening one turns the focused area into a Render area
        app.open_render("Toon");
        assert!(app.areas.root.areas().contains(&Shows::Renders));
        assert_eq!(app.renders_title(), "Toon");
        // with nothing rendered, keeping says why and keeps nothing
        app.keep_render(0.0);
        assert!(
            app.status.contains("Nothing is rendered yet"),
            "{}",
            app.status
        );
    }
}
