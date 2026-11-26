//! Renders the starter house in every mode the prototype has, with times.
//!
//!     cargo run --release -p creabim-render --bin render-demo -- <out-dir> [probe|looks|path|all]

use creabim_core::site::PlantForm;
use creabim_core::{library, Document, Element, ElementId, ElementKind, Model, Site, Transaction};
use creabim_render::look::{self, Look, Settings};
use creabim_render::path::Accum;
use creabim_render::scene::Scene;
use creabim_render::sky::{sun_direction, Sky};
use creabim_render::util::Camera;
use glam::{DVec2, Vec3};
use std::path::{Path, PathBuf};
use std::time::Instant;

fn rect(a: (f64, f64), b: (f64, f64)) -> Vec<DVec2> {
    creabim_core::polygon::rectangle(DVec2::new(a.0, a.1), DVec2::new(b.0, b.1))
}

fn material(doc: &mut Document, name: &str) -> Option<ElementId> {
    let (id, new) = library::find_or_new(doc.model(), name)?;
    if let Some(e) = new {
        doc.commit(Transaction::new("Material").insert(e)).ok()?;
    }
    Some(id)
}

/// The starter house on a plot: lawn, a path and a terrace, a few trees,
/// and the project placed on the map so the sun is where it would be.
fn model() -> Model {
    let mut doc = Document::from_model(creabim_core::starter::starter_house());
    let g = doc
        .model()
        .iter()
        .find(|e| e.name == "Ground Floor")
        .map(|e| e.id)
        .expect("levels");
    let grass = material(&mut doc, "Grass");
    // a lawn a little deeper than the library's swatch, as grass photographs
    if let Some(e) = grass.and_then(|id| doc.model().get(id).cloned()) {
        let mut after = e.clone();
        if let ElementKind::Material(m) = &mut after.kind {
            m.color = [0.36, 0.49, 0.25];
        }
        doc.commit(Transaction::new("Lawn colour").replace(e, after))
            .unwrap();
    }
    let flags = material(&mut doc, "Paving, concrete flags");
    let gravel = material(&mut doc, "Gravel");
    let mut tx = Transaction::new("Plot");
    let mut ground = Site::terrain(g, rect((-150.0, -150.0), (150.0, 150.0)), -0.15, 0.6);
    ground.material = grass;
    tx = tx.insert(Element::new("Lawn", ElementKind::Site(ground)));
    let area = |outline, material, pattern| {
        let mut a = Site::area(g, outline, 0.1, pattern);
        if let creabim_core::SiteForm::Area { top, .. } = &mut a.form {
            *top = -0.12;
        }
        a.material = material;
        ElementKind::Site(a)
    };
    let flag = creabim_core::site::Pattern::Units {
        size: DVec2::new(0.6, 0.6),
        stagger: false,
        angle: 0.0,
    };
    tx = tx
        .insert(Element::new(
            "Terrace",
            area(rect((-0.6, 10.6), (11.2, 14.0)), flags, flag),
        ))
        .insert(Element::new(
            "Front path",
            area(
                rect((5.0, -14.0), (6.6, -4.0)),
                gravel,
                creabim_core::site::Pattern::Stipple { spacing: 0.12 },
            ),
        ));
    for (name, at, form, canopy, height) in [
        ("Oak", (-13.0, -2.0), PlantForm::Tree, 7.0, 10.0),
        ("Birch", (16.0, -13.0), PlantForm::Tree, 4.5, 8.0),
        ("Birch", (24.0, 4.0), PlantForm::Tree, 4.0, 7.5),
        ("Pine", (-9.0, 16.0), PlantForm::Conifer, 3.5, 9.0),
        ("Shrub", (6.0, -5.5), PlantForm::Shrub, 1.4, 1.1),
        ("Shrub", (8.0, -5.6), PlantForm::Shrub, 1.2, 1.0),
        ("Shrub", (10.0, -5.4), PlantForm::Shrub, 1.5, 1.2),
        ("Shrub", (18.0, 8.0), PlantForm::Shrub, 1.6, 1.3),
    ] {
        let at = DVec2::new(at.0, at.1);
        let mut p = match form {
            PlantForm::Tree => {
                let (shape, ..) = creabim_core::tree::TreePreset::Shade.shape();
                let shape = creabim_core::tree::TreeShape {
                    depth: canopy * 0.92,
                    crown_base: height * 0.3,
                    trunk: canopy * 0.06,
                    seed: creabim_core::tree::seed_at(at),
                    ..shape
                };
                Site::tree(g, at, shape, canopy, height)
            }
            _ => Site::plant(g, at, form, canopy, height),
        };
        if let creabim_core::SiteForm::Plant { base, .. } = &mut p.form {
            *base = -0.15;
        }
        tx = tx.insert(Element::new(name, ElementKind::Site(p)));
    }
    doc.commit(tx).expect("the plot is valid");
    doc.model().clone()
}

struct Shot {
    name: &'static str,
    cam: Camera,
    /// Day of the year and solar hour.
    day: f32,
    hour: f32,
    exposure: f32,
    /// Samples per pixel at which the path tracer saves an image.
    marks: &'static [u32],
}

const LATITUDE: f32 = 48.85; // an assumed site: a temperate latitude (Paris)

fn cam(eye: [f32; 3], target: [f32; 3], fov: f32, two_point: bool) -> Camera {
    Camera {
        eye: Vec3::from(eye),
        target: Vec3::from(target),
        fov,
        two_point,
        aperture: 0.0,
        focus: 10.0,
        inverse: None,
    }
}

fn shots() -> Vec<Shot> {
    vec![
        // the street side, a summer morning
        Shot {
            name: "street",
            cam: cam([-7.0, -17.5, 1.6], [6.5, 2.0, 3.6], 40.0, true),
            day: 200.0,
            hour: 10.0,
            exposure: 1.0,
            marks: &[1, 4, 16, 64, 256],
        },
        // the garden side, a summer evening
        Shot {
            name: "garden",
            cam: cam([20.5, 24.0, 1.6], [7.0, 6.0, 3.4], 40.0, true),
            day: 172.0,
            hour: 18.0,
            exposure: 1.0,
            marks: &[1, 4, 16, 64, 256],
        },
        Shot {
            name: "aerial",
            cam: cam([30.0, -22.0, 20.0], [7.5, 4.0, 2.0], 36.0, false),
            day: 200.0,
            hour: 15.5,
            exposure: 1.0,
            marks: &[1, 16, 256],
        },
        // the kitchen and garden room, looking west
        Shot {
            name: "interior",
            cam: cam([14.5, 5.2, 1.45], [6.0, 8.6, 1.1], 62.0, true),
            day: 172.0,
            hour: 6.5,
            exposure: 7.0,
            marks: &[1, 16, 64, 256, 1024],
        },
    ]
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = PathBuf::from(args.get(1).map(String::as_str).unwrap_or("render-out"));
    let what = args.get(2).map(String::as_str).unwrap_or("all");
    let (w, h): (usize, usize) = match args.get(3) {
        Some(s) => {
            let mut it = s.split('x').map(|v| v.parse().unwrap());
            (it.next().unwrap(), it.next().unwrap())
        }
        None => (1600, 1000),
    };
    std::fs::create_dir_all(&out).unwrap();
    let t = Instant::now();
    let m = model();
    let t_model = t.elapsed().as_secs_f64();
    let t = Instant::now();
    let items = creabim_geom::scene::build_scene(&m);
    let t_mesh = t.elapsed().as_secs_f64();
    let t = Instant::now();
    let scene = Scene::from_items(&items);
    let t_bvh = t.elapsed().as_secs_f64();
    println!(
        "model {:.2}s, meshes {:.2}s, bvh {:.3}s: {} triangles, {} materials, bounds {:?}..{:?}",
        t_model,
        t_mesh,
        t_bvh,
        scene.triangles(),
        scene.materials.len(),
        scene.lo,
        scene.hi
    );
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    println!("threads {threads}");
    if what == "lamps" {
        lamps(&m, &items, &out, w, h);
        return;
    }
    let only = std::env::var("SHOTS").unwrap_or_default();
    for shot in shots() {
        if !only.is_empty() && !only.split(',').any(|n| n == shot.name) {
            continue;
        }
        let sun = sun_direction(LATITUDE, shot.day, shot.hour, 0.0);
        let sky = Sky::new(sun);
        println!(
            "{}: sun {:?} elevation {:.1} deg",
            shot.name,
            sun,
            sun.z.asin().to_degrees()
        );
        if what == "probe" || what == "looks" || what == "all" {
            let looks: &[Look] = if what == "probe" {
                &[Look::Clay]
            } else {
                &[
                    Look::Today,
                    Look::Realtime { traced: false },
                    Look::Realtime { traced: true },
                    Look::Clay,
                    Look::LineWash,
                    Look::Watercolour,
                    Look::Sketch,
                ]
            };
            let settings = Settings::default();
            let mut t = look::Timings {
                primary_ms: 0.0,
                lighting_ms: 0.0,
                look_ms: 0.0,
                rays: 0,
            };
            let g = look::gbuffer(&scene, &shot.cam, &sky, w, h, &settings, &mut t);
            println!(
                "  G-buffer at {}x{} ({} samples a pixel): camera rays {:.0} ms, shadows + occlusion {:.0} ms, {:.1} Mrays",
                w * settings.ss,
                h * settings.ss,
                settings.ss * settings.ss,
                t.primary_ms,
                t.lighting_ms,
                t.rays as f64 / 1e6
            );
            for &l in looks {
                let t = Instant::now();
                let img = look::shade(&scene, &sky, &g, w, h, l, &settings, shot.exposure);
                let ms = t.elapsed().as_secs_f64() * 1e3;
                img.save_png(&out.join(format!("{}-{}.png", shot.name, l.name())))
                    .unwrap();
                println!("  {:<26} look {:6.0} ms", l.name(), ms);
            }
        }
        if what == "toon" || what == "all" {
            // its own light and a short reach for the cavity darkening
            let sky = Sky::new(Look::TOON_LIGHT.normalize());
            let settings = Settings {
                ao_radius: 0.6,
                ..Settings::default()
            };
            let mut t = look::Timings {
                primary_ms: 0.0,
                lighting_ms: 0.0,
                look_ms: 0.0,
                rays: 0,
            };
            let g = look::gbuffer(&scene, &shot.cam, &sky, w, h, &settings, &mut t);
            let t1 = Instant::now();
            let img = look::shade(&scene, &sky, &g, w, h, Look::Toon, &settings, 1.0);
            img.save_png(&out.join(format!("{}-toon.png", shot.name)))
                .unwrap();
            println!(
                "  toon: G-buffer {:.0} ms + look {:.0} ms",
                t.primary_ms + t.lighting_ms,
                t1.elapsed().as_secs_f64() * 1e3
            );
        }
        if what == "path" || what == "all" {
            path_trace(&scene, &shot, &sky, w, h, &out);
        }
    }
}

fn path_trace(scene: &Scene, shot: &Shot, sky: &Sky, w: usize, h: usize, out: &Path) {
    let mut acc = Accum::new(w, h);
    let marks = shot.marks;
    let mut secs = 0.0;
    let mut rays = 0u64;
    for &mark in marks {
        let t = Instant::now();
        while acc.spp < mark {
            rays += acc.pass(scene, &shot.cam, sky);
        }
        secs += t.elapsed().as_secs_f64();
        let img = creabim_render::denoise::develop(&acc.mean(), w, h, 0.85 * shot.exposure);
        img.save_png(&out.join(format!("{}-path-{:04}spp.png", shot.name, mark)))
            .unwrap();
        let td = Instant::now();
        let dn = creabim_render::denoise::denoise(&acc, 5);
        let dsecs = td.elapsed().as_secs_f64();
        creabim_render::denoise::develop(&dn, w, h, 0.85 * shot.exposure)
            .save_png(&out.join(format!("{}-path-{:04}spp-denoised.png", shot.name, mark)))
            .unwrap();
        println!(
            "  path {:4} spp: {:7.1} s  ({:.1} Mrays/s), denoise {:.2} s",
            mark,
            secs,
            rays as f64 / secs / 1e6,
            dsecs
        );
    }
}

// --- lamps (trial for the lighting proposal) ---

/// The project's own light fittings as lamps: downlights, ceiling lights
/// and wall lights, each a photometric profile, colour temperature and
/// output; grouped for mixing (1 kitchen, 2 ceiling lights, 3 the rest).
fn fittings(
    m: &Model,
    items: &[creabim_geom::scene::SceneItem],
) -> Vec<creabim_render::scene::Lamp> {
    use creabim_render::scene::{kelvin, profile, Lamp};
    let mut out = vec![];
    for e in m.iter() {
        let name = e.name.to_lowercase();
        let kind = if name.contains("downlight") {
            0
        } else if name.contains("ceiling light")
            || name.contains("ceiling-light")
            || name.contains("pendant")
        {
            1
        } else if name.contains("wall light") || name.contains("wall-light") {
            2
        } else {
            continue;
        };
        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for it in items.iter().filter(|i| i.id == e.id) {
            for p in &it.mesh.positions {
                lo = lo.min(Vec3::from(*p));
                hi = hi.max(Vec3::from(*p));
            }
        }
        if lo.x > hi.x || lo.z > 3.0 {
            continue; // ground floor only
        }
        let c = (lo + hi) * 0.5;
        let kitchen = c.x > 11.9 && c.y > 4.7;
        let l = match kind {
            0 => Lamp {
                pos: Vec3::new(c.x, c.y, lo.z - 0.01),
                axis: -Vec3::Z,
                radius: 0.035,
                candela: profile(22.0, 38.0, 0.0),
                lumens: 650.0,
                colour: kelvin(3000.0),
                group: if kitchen { 1 } else { 3 },
            },
            1 => Lamp {
                pos: Vec3::new(c.x, c.y, lo.z + 0.02),
                axis: -Vec3::Z,
                radius: 0.10,
                candela: profile(50.0, 85.0, 0.35),
                lumens: 1100.0,
                colour: kelvin(2700.0),
                group: 2,
            },
            _ => Lamp {
                pos: Vec3::new(c.x, c.y, c.z),
                axis: -Vec3::Z,
                radius: 0.05,
                candela: profile(35.0, 70.0, 0.4),
                lumens: 400.0,
                colour: kelvin(2700.0),
                group: 3,
            },
        };
        println!(
            "  lamp {:?} {} at {:.2?} group {}",
            e.name, l.lumens, l.pos, l.group
        );
        out.push(l);
    }
    out
}

/// Log-average brightness to a middle grey: automatic exposure.
fn auto_exposure(px: &[Vec3], key: f32) -> f32 {
    let mut s = 0.0f64;
    for c in px {
        s += (creabim_render::util::luminance(*c).max(1e-5) as f64).ln();
    }
    key / (s / px.len() as f64).exp() as f32
}

/// Lux to a false colour, in the bands lighting software uses.
fn lux_colour(lux: f32) -> Vec3 {
    const STOPS: [(f32, [f32; 3]); 9] = [
        (0.0, [0.10, 0.10, 0.35]),
        (25.0, [0.15, 0.25, 0.75]),
        (50.0, [0.10, 0.55, 0.90]),
        (100.0, [0.10, 0.75, 0.65]),
        (200.0, [0.35, 0.85, 0.30]),
        (300.0, [0.80, 0.90, 0.20]),
        (500.0, [1.00, 0.75, 0.15]),
        (750.0, [1.00, 0.45, 0.10]),
        (1000.0, [0.90, 0.15, 0.15]),
    ];
    for k in 0..STOPS.len() - 1 {
        let (a, ca) = STOPS[k];
        let (b, cb) = STOPS[k + 1];
        if lux < b {
            let t = ((lux - a) / (b - a)).clamp(0.0, 1.0);
            return Vec3::from(ca).lerp(Vec3::from(cb), t);
        }
    }
    Vec3::from(STOPS[8].1)
}

/// Lux on what each pixel sees, from the denoised light over its colour.
fn lux_image(
    acc: &Accum,
    dn: &[Vec3],
    shade: &creabim_render::util::Image,
) -> (creabim_render::util::Image, Vec<f32>) {
    use creabim_render::scene::PER_LUX;
    use creabim_render::util::luminance;
    let k = 1.0 / acc.spp.max(1) as f32;
    let mut lux = vec![0.0; dn.len()];
    let px = (0..dn.len())
        .map(|i| {
            let a = luminance(acc.albedo[i] * k);
            if a < 0.03 {
                return Vec3::splat(0.25);
            }
            let e = std::f32::consts::PI * luminance(dn[i]) / a / PER_LUX;
            lux[i] = e;
            // a touch of the picture under it, for where things are
            let g = luminance(shade.px[i]);
            lux_colour(e) * (0.75 + 0.25 * g)
        })
        .collect();
    (
        creabim_render::util::Image {
            w: acc.w,
            h: acc.h,
            px,
        },
        lux,
    )
}

fn lamps(m: &Model, items: &[creabim_geom::scene::SceneItem], out: &Path, w: usize, h: usize) {
    use creabim_render::scene::GROUPS;
    let lamps = fittings(m, items);
    let mut scene = Scene::from_items(items);
    scene.add_lamps(lamps);
    // a midsummer evening, the sun low in the west, through the windows
    let sun = sun_direction(LATITUDE, 172.0, 19.6, 0.0);
    let sky = Sky::new(sun);
    println!(
        "sun elevation {:.1} deg, {} lamps",
        sun.z.asin().to_degrees(),
        scene.lamps.len()
    );
    let spp: u32 = std::env::var("SPP")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(256);
    let warm = Vec3::new(1.0, 0.92, 0.80);
    let blue_hour = Vec3::new(0.55, 0.70, 1.0);
    // weights: sun, kitchen, ceiling lights, the rest, sky
    let z = Vec3::ZERO;
    let o = Vec3::ONE;
    let mixes: [(&str, [Vec3; GROUPS], f32); 6] = [
        ("daylight", [o, z, z, z, o], 0.0),
        ("evening-lamps-on", [z, o, o, o, o * 0.05], 0.0),
        ("blue-hour", [z, o, o, o, blue_hour * 0.015], 0.0),
        ("night", [z, o, o, o, blue_hour * 0.002], 0.0),
        ("night-kitchen-only", [z, o, z, z, blue_hour * 0.002], 0.0),
        (
            "night-dining-dimmed",
            [z, o * 0.15, warm * 0.5, o * 0.3, blue_hour * 0.002],
            0.0,
        ),
    ];
    // the kitchen view, then a plan cut at 2.45 m over the ground floor
    let view = Camera {
        eye: Vec3::new(14.5, 5.2, 1.45),
        target: Vec3::new(6.0, 8.6, 1.1),
        fov: 62.0,
        two_point: true,
        aperture: 0.0,
        focus: 10.0,
        inverse: None,
    };
    let (pw, ph) = (w, (w as f32 / 1.6) as usize);
    let plan = {
        let c = Vec3::new(8.0, 4.9, 0.0);
        let hw = 9.6;
        let hh = hw / (pw as f32 / ph as f32);
        let v = glam::Mat4::look_at_rh(Vec3::new(c.x, c.y, 2.25), c, Vec3::Y);
        let p = glam::Mat4::orthographic_rh(-hw, hw, -hh, hh, 0.0, 10.0);
        Camera::from_view_proj(p * v, Vec3::new(c.x, c.y, 2.25), c)
    };
    for (name, cam, cw, chh) in [("kitchen", view, w, h), ("plan", plan, pw, ph)] {
        if std::env::var("ONLY").is_ok_and(|o| o != name) {
            continue;
        }
        let t = Instant::now();
        let mut acc = Accum::new(cw, chh);
        let mut rays = 0;
        while acc.spp < spp {
            rays += acc.pass(&scene, &cam, &sky);
        }
        let secs = t.elapsed().as_secs_f64();
        println!(
            "{name}: {spp} spp in {secs:.1} s ({:.1} Mrays/s)",
            rays as f64 / secs / 1e6
        );
        // a camera's white balance, part way to the lamps' warmth, as a
        // photographer would set it: warm, not orange
        let wb = {
            let k = creabim_render::scene::kelvin(3000.0);
            let v = Vec3::new(1.0 / k.x, 1.0 / k.y, 1.0 / k.z).powf(0.6);
            v / creabim_render::util::luminance(v)
        };
        for (mix, wts, _) in &mixes {
            let wts = &{
                let mut x = *wts;
                for g in x.iter_mut().take(GROUPS - 1).skip(1) {
                    *g *= wb;
                }
                x
            };
            if name == "plan" && !mix.starts_with("night") && *mix != "daylight" {
                continue;
            }
            let a = acc.mix(*wts);
            let dn = creabim_render::denoise::denoise(&a, 5);
            let ex = auto_exposure(&dn, 0.20);
            let img = creabim_render::denoise::develop(&dn, cw, chh, ex);
            img.save_png(&out.join(format!("lamps-{name}-{mix}.png")))
                .unwrap();
            println!("  {mix}: exposure {:+.1} EV", ex.log2());
            if *mix == "night" || *mix == "daylight" {
                let (li, lux) = lux_image(&a, &dn, &img);
                li.save_png(&out.join(format!("lamps-{name}-{mix}-lux.png")))
                    .unwrap();
                // lux as numbers for the mock-up's readout: log-encoded grey
                let mut buf = image::GrayImage::new(cw as u32, chh as u32);
                for (i, p) in buf.pixels_mut().enumerate() {
                    let e = lux[i].max(0.0);
                    *p = image::Luma([
                        ((e + 1.0).log2() / 16.0 * 255.0).round().clamp(0.0, 255.0) as u8
                    ]);
                }
                buf.save(out.join(format!("lamps-{name}-{mix}-luxdata.png")))
                    .unwrap();
                let mut v: Vec<f32> = lux.iter().copied().filter(|x| *x > 0.0).collect();
                v.sort_by(|a, b| a.total_cmp(b));
                if !v.is_empty() {
                    println!(
                        "  {mix} lux: median {:.0}, 90th {:.0}",
                        v[v.len() / 2],
                        v[v.len() * 9 / 10]
                    );
                }
            }
        }
        // each group alone, kept as linear light (log-encoded, dithered,
        // 8 bits) for the mock-up's live light mix, and as a picture
        let mut ranges = vec![];
        for g in 0..GROUPS {
            let mut wts = [Vec3::ZERO; GROUPS];
            wts[g] = if g == 0 || g == GROUPS - 1 {
                Vec3::ONE
            } else {
                wb
            };
            let a = acc.mix(wts);
            let dn = creabim_render::denoise::denoise(&a, 5);
            let (lo, hi) = (-26.0f32, 4.0f32);
            let mut buf = image::RgbImage::new(cw as u32, chh as u32);
            for (i, p) in buf.pixels_mut().enumerate() {
                let (x, y) = (i % cw, i / cw);
                // a small ordered dither hides the 8-bit steps
                let bayer = [
                    [0., 8., 2., 10.],
                    [12., 4., 14., 6.],
                    [3., 11., 1., 9.],
                    [15., 7., 13., 5.],
                ];
                let d = (bayer[y % 4][x % 4] + 0.5) / 16.0;
                let c = dn[i];
                let q = |v: f32| {
                    let l = (v.max(2f32.powf(lo)).log2() - lo) / (hi - lo);
                    (l * 255.0 + d).floor().clamp(0.0, 255.0) as u8
                };
                *p = image::Rgb([q(c.x), q(c.y), q(c.z)]);
            }
            buf.save(out.join(format!("lamps-{name}-light{g}.png")))
                .unwrap();
            ranges.push(format!("[{lo}, {hi}]"));
            if g == 0 || g == GROUPS - 1 {
                continue;
            }
            let a = acc.mix(wts);
            let dn = creabim_render::denoise::denoise(&a, 5);
            let ex = auto_exposure(&dn, 0.20);
            creabim_render::denoise::develop(&dn, cw, chh, ex)
                .save_png(&out.join(format!("lamps-{name}-group{g}.png")))
                .unwrap();
        }
    }
}
