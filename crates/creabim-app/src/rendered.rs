//! The 3D view's rendered modes (Real-time, Toon, Clay, the styles and
//! Path traced): while the view moves the GPU draws it shaded as ever;
//! once it has been still a moment, creabim-render renders it on a worker
//! thread, a rough image first and then the full one (or pass after pass
//! of the path tracer), shown over the view as each is ready. Any change
//! (the view, its size, the model, the mode or the sun) stops the work and
//! the shaded view shows until the view is still again.

use creabim_geom::scene::SceneItem;
use creabim_render::job::{self, Mode, Progress, Request};
use creabim_render::scene::Scene;
use creabim_render::sky::{sun_direction, Sky};
use creabim_render::util::{Camera, Image};
use eframe::egui;
use glam::{Mat4, Vec3};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};

/// How long the view has to be still before rendering starts, seconds.
pub const SETTLE: f64 = 0.25;
/// The widest a view is rendered, in pixels: wider views are rendered at
/// this and shown scaled.
const MAX_WIDTH: f32 = 1600.0;

/// Where the sun is: the project's place, a day and an hour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SunAt {
    pub latitude: f32,
    pub true_north: f32,
    /// Day of the year, 1 to 365.
    pub day: f32,
    /// Solar time, hours.
    pub hour: f32,
    /// Brightness, in stops: 0 outdoors.
    pub ev: f32,
    /// What the sky is like, and its own light's strength (1, as it is).
    pub sky: creabim_render::sky::SkyKind,
    pub strength: f32,
    /// The white balance, kelvin: light of this colour reads as white.
    /// 6500 is daylight, and changes nothing.
    pub white: f32,
}

impl SunAt {
    pub fn sky(&self) -> Sky {
        let sky = Sky::with(
            sun_direction(self.latitude, self.day, self.hour, self.true_north),
            self.sky,
            self.strength,
        );
        if (self.white - 6500.0).abs() < 1.0 {
            sky
        } else {
            sky.tinted(creabim_render::scene::white_gain(self.white))
        }
    }
}

/// What an image was rendered for: when it differs, the image is stale.
#[derive(Clone, Debug, PartialEq)]
struct Key {
    mode: Mode,
    view_proj: [u32; 16],
    size: [usize; 2],
    scene: u64,
    sun: [u32; 8],
}

struct Job {
    key: Key,
    cancel: Arc<AtomicBool>,
    rx: Receiver<Progress>,
}

impl Drop for Job {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

#[derive(Default)]
pub struct Rendered {
    /// What the view showed last frame, and when that last changed.
    seen: Option<(Key, f64)>,
    job: Option<Job>,
    /// The image shown, the key it is for, and its stage.
    shown: Option<(Key, egui::TextureHandle)>,
    pub stage: String,
    pub seconds: f64,
    pub done: bool,
    /// The latest image, kept to save.
    pub image: Option<Arc<Image>>,
    /// The renderer's scene, by the 3D view's geometry generation: built
    /// on the worker, once for each model change.
    #[allow(clippy::type_complexity)]
    scene: Arc<Mutex<Option<(u64, Arc<Scene>)>>>,
}

/// One frame's view to render.
pub struct View<'a> {
    pub mode: Mode,
    pub view_proj: Mat4,
    pub eye: Vec3,
    pub target: Vec3,
    /// The view's size in pixels.
    pub pixels: [u32; 2],
    pub scene_gen: u64,
    pub items: &'a [SceneItem],
    /// A ground to stand on (height and colour) when the model has none.
    pub ground: Option<(f32, [f32; 3])>,
    pub sun: SunAt,
    /// The model's lamps as they are lit now, and a number that changes
    /// when they do.
    pub lamps: Vec<creabim_render::scene::Lamp>,
    pub lamps_key: u64,
    pub now: f64,
}

impl View<'_> {
    /// What the renderer's scene is made from: the geometry and the lamps.
    fn scene_key(&self) -> u64 {
        self.scene_gen
            .wrapping_mul(1_000_003)
            .wrapping_add(self.lamps_key)
    }
}

impl Rendered {
    /// Stop any work and forget the image (the mode is a GPU one).
    pub fn clear(&mut self) {
        self.job = None;
        self.shown = None;
        self.seen = None;
        self.image = None;
        self.stage.clear();
    }

    /// Is the image shown the one for the view as it is?
    fn current(&self) -> Option<&egui::TextureHandle> {
        let (k, _) = self.seen.as_ref()?;
        self.shown.as_ref().filter(|(s, _)| s == k).map(|(_, t)| t)
    }

    /// Follow the view: start, stop and collect rendering. Returns the
    /// texture to draw over the view, when there is one for it as it is
    /// now; asks for a repaint while work is going on.
    pub fn update(&mut self, ctx: &egui::Context, v: View) -> Option<egui::TextureHandle> {
        let scale = (MAX_WIDTH / v.pixels[0].max(1) as f32).min(1.0);
        let size = [
            ((v.pixels[0] as f32 * scale) as usize).max(16),
            ((v.pixels[1] as f32 * scale) as usize).max(16),
        ];
        let s = v.sun;
        let key = Key {
            mode: v.mode,
            view_proj: v.view_proj.to_cols_array().map(f32::to_bits),
            size,
            scene: v.scene_key(),
            sun: [
                s.latitude,
                s.true_north,
                s.day,
                s.hour,
                s.ev,
                s.sky as u8 as f32,
                s.strength,
                s.white,
            ]
            .map(f32::to_bits),
        };
        let since = match &self.seen {
            Some((k, t)) if *k == key => *t,
            _ => {
                self.seen = Some((key.clone(), v.now));
                v.now
            }
        };
        // a job for another view stops
        if self.job.as_ref().is_some_and(|j| j.key != key) {
            self.job = None;
        }
        let have = self.shown.as_ref().is_some_and(|(k, _)| *k == key);
        if self.job.is_none() && !have {
            if v.now - since >= SETTLE {
                self.start(key.clone(), &v);
            } else {
                ctx.request_repaint_after(std::time::Duration::from_secs_f64(
                    SETTLE - (v.now - since) + 0.01,
                ));
            }
        }
        // collect what the worker has finished
        if let Some(j) = &self.job {
            let mut last = None;
            let mut gone = false;
            loop {
                match j.rx.try_recv() {
                    Ok(p) => last = Some(p),
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        gone = true;
                        break;
                    }
                }
            }
            if let Some(p) = last {
                let tex = ctx.load_texture("rendered-3d", to_colour_image(&p.image), {
                    egui::TextureOptions::LINEAR
                });
                self.shown = Some((j.key.clone(), tex));
                self.stage = p.stage;
                self.seconds = p.seconds;
                self.done = p.done;
                self.image = Some(Arc::new(p.image));
            }
            if gone || self.done && self.shown.as_ref().is_some_and(|(k, _)| *k == j.key) {
                self.job = None;
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(100));
            }
        }
        self.current().cloned()
    }

    /// Is work going on for the view as it is?
    pub fn working(&self) -> bool {
        self.job.is_some()
    }

    fn start(&mut self, key: Key, v: &View) {
        let cancel = Arc::new(AtomicBool::new(false));
        let (tx, rx) = channel();
        // the scene is built on the worker when the model has changed
        let cache = self.scene.clone();
        let cached = cache.lock().ok().and_then(|c| {
            c.as_ref()
                .filter(|(g, _)| *g == v.scene_key())
                .map(|(_, s)| s.clone())
        });
        let items = if cached.is_none() {
            Some(v.items.to_vec())
        } else {
            None
        };
        let gen = v.scene_key();
        let lamps = v.lamps.clone();
        let ground = v.ground;
        let mut camera = Camera::from_view_proj(v.view_proj, v.eye, v.target);
        camera.fov = 45.0;
        let sky = v.sun.sky();
        let (w, h) = (key.size[0], key.size[1]);
        let mode = v.mode;
        let exposure = 2f32.powf(v.sun.ev);
        let c2 = cancel.clone();
        self.stage = "Starting".into();
        self.done = false;
        let spawned = std::thread::Builder::new()
            .name("render-3d".into())
            .spawn(move || {
                let scene = match cached {
                    Some(s) => s,
                    None => {
                        let mut s = Scene::from_items_on(&items.unwrap_or_default(), ground);
                        if !lamps.is_empty() {
                            s.add_lamps(lamps);
                        }
                        let s = Arc::new(s);
                        if let Ok(mut c) = cache.lock() {
                            *c = Some((gen, s.clone()));
                        }
                        s
                    }
                };
                if c2.load(Ordering::Relaxed) {
                    return;
                }
                let req = Request {
                    scene,
                    camera,
                    sky,
                    width: w,
                    height: h,
                    mode,
                    exposure,
                };
                job::run(&req, c2, |p| {
                    let _ = tx.send(p);
                });
            });
        if spawned.is_ok() {
            self.job = Some(Job { key, cancel, rx });
        }
    }
}

/// One render at a size of its own, made in the background to keep
/// (Output's size): its images as they come, until it is done.
pub struct Final {
    cancel: Arc<AtomicBool>,
    rx: Receiver<Progress>,
    pub size: [usize; 2],
    pub stage: String,
    pub seconds: f64,
    pub image: Option<Arc<Image>>,
    pub done: bool,
}

impl Drop for Final {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

impl Final {
    /// Collect what the worker has finished; whether it is done (or gone).
    pub fn poll(&mut self) -> bool {
        loop {
            match self.rx.try_recv() {
                Ok(p) => {
                    self.stage = p.stage;
                    self.seconds = p.seconds;
                    self.done = p.done;
                    self.image = Some(Arc::new(p.image));
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.done = true;
                    break;
                }
            }
        }
        self.done
    }
}

impl Rendered {
    /// Render `v` once at `size`, in the background, whatever the view's
    /// own size: `v.view_proj` is the camera's at that size's shape.
    pub fn start_final(&self, v: &View, size: [usize; 2]) -> Option<Final> {
        let cancel = Arc::new(AtomicBool::new(false));
        let (tx, rx) = channel();
        let cache = self.scene.clone();
        let cached = cache.lock().ok().and_then(|c| {
            c.as_ref()
                .filter(|(g, _)| *g == v.scene_key())
                .map(|(_, s)| s.clone())
        });
        let items = if cached.is_none() {
            Some(v.items.to_vec())
        } else {
            None
        };
        let gen = v.scene_key();
        let lamps = v.lamps.clone();
        let ground = v.ground;
        let mut camera = Camera::from_view_proj(v.view_proj, v.eye, v.target);
        camera.fov = 45.0;
        let sky = v.sun.sky();
        let mode = v.mode;
        let exposure = 2f32.powf(v.sun.ev);
        let c2 = cancel.clone();
        std::thread::Builder::new()
            .name("render-final".into())
            .spawn(move || {
                let scene = match cached {
                    Some(s) => s,
                    None => {
                        let mut s = Scene::from_items_on(&items.unwrap_or_default(), ground);
                        if !lamps.is_empty() {
                            s.add_lamps(lamps);
                        }
                        let s = Arc::new(s);
                        if let Ok(mut c) = cache.lock() {
                            *c = Some((gen, s.clone()));
                        }
                        s
                    }
                };
                if c2.load(Ordering::Relaxed) {
                    return;
                }
                let req = Request {
                    scene,
                    camera,
                    sky,
                    width: size[0],
                    height: size[1],
                    mode,
                    exposure,
                };
                job::run(&req, c2, |p| {
                    let _ = tx.send(p);
                });
            })
            .ok()?;
        Some(Final {
            cancel,
            rx,
            size,
            stage: "Starting".into(),
            seconds: 0.0,
            image: None,
            done: false,
        })
    }
}

fn to_colour_image(img: &Image) -> egui::ColorImage {
    let mut rgba = Vec::with_capacity(img.w * img.h * 4);
    for p in &img.px {
        let c = p.clamp(Vec3::ZERO, Vec3::ONE) * 255.0 + Vec3::splat(0.5);
        rgba.extend_from_slice(&[c.x as u8, c.y as u8, c.z as u8, 255]);
    }
    egui::ColorImage::from_rgba_unmultiplied([img.w, img.h], &rgba)
}

/// "19 Jul" for a day of the year.
pub fn day_label(day: f32) -> String {
    const MONTHS: [(&str, u32); 12] = [
        ("Jan", 31),
        ("Feb", 28),
        ("Mar", 31),
        ("Apr", 30),
        ("May", 31),
        ("Jun", 30),
        ("Jul", 31),
        ("Aug", 31),
        ("Sep", 30),
        ("Oct", 31),
        ("Nov", 30),
        ("Dec", 31),
    ];
    let mut d = (day.round() as u32).clamp(1, 365);
    for (m, n) in MONTHS {
        if d <= n {
            return format!("{d} {m}");
        }
        d -= n;
    }
    "31 Dec".into()
}

/// "10:30" for an hour.
pub fn hour_label(hour: f32) -> String {
    let m = (hour * 60.0).round() as i32;
    format!("{:02}:{:02}", m / 60, m % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels() {
        assert_eq!(day_label(1.0), "1 Jan");
        assert_eq!(day_label(200.0), "19 Jul");
        assert_eq!(day_label(365.0), "31 Dec");
        assert_eq!(hour_label(10.5), "10:30");
        assert_eq!(hour_label(6.25), "06:15");
    }
}
