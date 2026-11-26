//! Hero views of a project, path traced: daylight under the sun where the
//! project is on the map, and dusk with the project's own light fittings
//! lit.
//!
//! `cargo run --profile fast -p creabim-app --example showcase_render -- <model.creabim> <out-dir> [WxH] [spp]`

use creabim_core::{ElementKind, Model};
use creabim_render::path::Accum;
use creabim_render::scene::{kelvin, profile, Lamp, Scene, GROUPS};
use creabim_render::sky::{sun_direction, Sky};
use creabim_render::util::Camera;
use glam::Vec3;
use std::path::PathBuf;
use std::time::Instant;

/// The project's light fittings as lamps: downlights, pendants and wall
/// lights (groups 1, 2 and 3).
fn fittings(m: &Model, items: &[creabim_geom::scene::SceneItem]) -> Vec<Lamp> {
    let mut out = vec![];
    for e in m.iter() {
        if !matches!(e.kind, ElementKind::Object(_)) {
            continue;
        }
        let name = e.name.to_lowercase();
        let kind = if name.contains("downlight") {
            0
        } else if name.contains("ceiling light") || name.contains("pendant") {
            1
        } else if name.contains("wall light") {
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
        if lo.x > hi.x {
            continue;
        }
        let c = (lo + hi) * 0.5;
        out.push(match kind {
            0 => Lamp {
                pos: Vec3::new(c.x, c.y, lo.z - 0.01),
                axis: -Vec3::Z,
                radius: 0.035,
                candela: profile(22.0, 38.0, 0.0),
                lumens: 650.0,
                colour: kelvin(3000.0),
                group: 1,
            },
            1 => Lamp {
                pos: Vec3::new(c.x, c.y, lo.z - 0.02),
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
        });
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

fn cam(eye: [f32; 3], target: [f32; 3], fov: f32) -> Camera {
    Camera {
        eye: Vec3::from(eye),
        target: Vec3::from(target),
        fov,
        two_point: true,
        aperture: 0.0,
        focus: 10.0,
        inverse: None,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let doc = creabim_store::load(&args[1]).expect("the project opens");
    let out = PathBuf::from(&args[2]);
    std::fs::create_dir_all(&out).unwrap();
    let (w, h): (usize, usize) = match args.get(3) {
        Some(s) => {
            let mut it = s.split('x').map(|v| v.parse().unwrap());
            (it.next().unwrap(), it.next().unwrap())
        }
        None => (1600, 1000),
    };
    let spp: u32 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(128);
    let m = doc.model();
    let (latitude, north) = m
        .project_info()
        .map(|(_, p)| (p.latitude.unwrap_or(45.0) as f32, p.true_north as f32))
        .unwrap_or((45.0, 0.0));
    let items = creabim_geom::scene::build_scene(m);
    let mut scene = Scene::from_items(&items);
    let lamps = fittings(m, &items);
    println!(
        "{} triangles, {} lamps, latitude {latitude}",
        scene.triangles(),
        lamps.len()
    );
    scene.add_lamps(lamps);
    let (z, o) = (Vec3::ZERO, Vec3::ONE);
    let dusk_sky = Vec3::new(0.50, 0.66, 1.0) * 0.006;
    // (name, camera, day of the year, solar hour, weights: sun, three
    // groups of lamps, sky)
    let shots: [(&str, Camera, f32, f32, [Vec3; GROUPS]); 3] = [
        // the terrace side on an afternoon at the equinox
        (
            "daylight-south-west",
            cam([-8.5, -16.0, 0.1], [7.0, 3.2, 3.3], 42.0),
            80.0,
            14.5,
            [o, z, z, z, o],
        ),
        // the arrival from the court, a December dusk, the lights on
        (
            "dusk-arrival",
            cam([15.5, -15.5, -0.3], [9.5, 3.0, 3.2], 42.0),
            355.0,
            16.6,
            [z, o, o, o, dusk_sky],
        ),
        // the same view by day, a June morning
        (
            "daylight-arrival",
            cam([15.5, -15.5, -0.3], [9.5, 3.0, 3.2], 42.0),
            172.0,
            9.5,
            [o, z, z, z, o],
        ),
    ];
    for (name, camera, day, hour, weights) in shots {
        let sun = sun_direction(latitude, day, hour, north);
        // after sunset the sky is lit from just under the horizon
        let sky = Sky::new(if sun.z < 0.03 {
            Vec3::new(sun.x, sun.y, 0.03).normalize()
        } else {
            sun
        });
        let t = Instant::now();
        let mut acc = Accum::new(w, h);
        while acc.spp < spp {
            acc.pass(&scene, &camera, &sky);
        }
        let mixed = acc.mix(weights);
        let dn = creabim_render::denoise::denoise(&mixed, 5);
        let ex = auto_exposure(&dn, if weights[0] == z { 0.07 } else { 0.2 });
        creabim_render::denoise::develop(&dn, w, h, ex)
            .save_png(&out.join(format!("{name}.png")))
            .unwrap();
        println!(
            "{name}: sun {:.1} deg, {:.0} s",
            sun.z.asin().to_degrees(),
            t.elapsed().as_secs_f64()
        );
    }
}
