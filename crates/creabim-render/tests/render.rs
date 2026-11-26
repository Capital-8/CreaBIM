use creabim_render::bvh::Bvh;
use creabim_render::sky::{sun_direction, Sky};
use creabim_render::util::Rng;
use glam::Vec3;

/// The tree finds the same nearest triangle as testing every one.
#[test]
fn bvh_matches_brute_force() {
    let mut r = Rng::new(1, 2, 3);
    let mut v = |s: f32| Vec3::new(r.f() * s, r.f() * s, r.f() * s);
    let tris: Vec<[Vec3; 3]> = (0..2000)
        .map(|_| {
            let c = v(20.0);
            [
                c,
                c + v(1.5) - Vec3::splat(0.75),
                c + v(1.5) - Vec3::splat(0.75),
            ]
        })
        .collect();
    let bvh = Bvh::build(&tris);
    let mut r = Rng::new(9, 8, 7);
    for _ in 0..500 {
        let o = Vec3::new(r.f(), r.f(), r.f()) * 30.0 - Vec3::splat(5.0);
        let d = (Vec3::new(r.f(), r.f(), r.f()) - Vec3::splat(0.5)).normalize();
        let brute = tris
            .iter()
            .enumerate()
            .filter_map(|(i, t)| hit(o, d, t).map(|h| (h, i as u32)))
            .min_by(|a, b| a.0.total_cmp(&b.0));
        let fast = bvh.intersect(o, d, 1e6).map(|h| (h.t, h.tri));
        match (brute, fast) {
            (None, None) => {}
            (Some(a), Some(b)) => assert!((a.0 - b.0).abs() < 1e-3, "{a:?} vs {b:?}"),
            other => panic!("{other:?}"),
        }
    }
}

fn hit(o: Vec3, d: Vec3, t: &[Vec3; 3]) -> Option<f32> {
    let (e1, e2) = (t[1] - t[0], t[2] - t[0]);
    let p = d.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-12 {
        return None;
    }
    let s = o - t[0];
    let u = s.dot(p) / det;
    let q = s.cross(e1);
    let v = d.dot(q) / det;
    let tt = e2.dot(q) / det;
    (u >= 0.0 && v >= 0.0 && u + v <= 1.0 && tt > 1e-5).then_some(tt)
}

/// Northern summer: the sun rises in the north-east, stands high in the
/// south at noon and sets in the north-west.
#[test]
fn sun_follows_the_day() {
    let morning = sun_direction(48.85, 172.0, 6.0, 0.0);
    let noon = sun_direction(48.85, 172.0, 12.0, 0.0);
    let evening = sun_direction(48.85, 172.0, 18.0, 0.0);
    assert!(morning.x > 0.5 && morning.y > 0.0 && morning.z > 0.0);
    assert!(noon.y < 0.0 && noon.x.abs() < 1e-3);
    // 90 - 48.85 + 23.44
    assert!((noon.z.asin().to_degrees() - 64.59).abs() < 0.2);
    assert!(evening.x < -0.5 && evening.y > 0.0);
    // true north turned a quarter: the noon sun turns with it
    let turned = sun_direction(48.85, 172.0, 12.0, 90.0);
    assert!(turned.x > 0.4 && turned.y.abs() < 1e-3);
}

#[test]
fn sky_is_brighter_up_than_down() {
    let sky = Sky::new(sun_direction(48.85, 172.0, 12.0, 0.0));
    assert!(sky.ambient(Vec3::Z).length() > sky.ambient(-Vec3::Z).length());
    assert!(
        sky.radiance(Vec3::Z).z > sky.radiance(Vec3::Z).x,
        "a blue sky"
    );
}

#[test]
fn a_scene_with_nothing_in_it_renders_as_sky() {
    use creabim_render::job::{self, Mode, Request};
    use creabim_render::look::Look;
    use creabim_render::scene::Scene;
    use creabim_render::sky::{sun_direction, Sky};
    use creabim_render::util::Camera;
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;
    let vp = glam::Mat4::perspective_rh(0.8, 1.0, 0.1, 100.0)
        * glam::Mat4::look_at_rh(Vec3::new(5.0, -5.0, 3.0), Vec3::ZERO, Vec3::Z);
    let req = Request {
        scene: Arc::new(Scene::from_items_on(&[], None)),
        camera: Camera::from_view_proj(vp, Vec3::new(5.0, -5.0, 3.0), Vec3::ZERO),
        sky: Sky::new(sun_direction(48.85, 200.0, 10.0, 0.0)),
        width: 24,
        height: 24,
        mode: Mode::Look(Look::Clay),
        exposure: 1.0,
    };
    let mut done = false;
    job::run(&req, Arc::new(AtomicBool::new(false)), |p| done |= p.done);
    assert!(done);
}

mod lamps {
    use creabim_core::ElementId;
    use creabim_geom::{SceneItem, TriMesh};
    use creabim_render::job::{self, Mode, Request};
    use creabim_render::look::Look;
    use creabim_render::scene::{kelvin, profile, Lamp, Scene};
    use creabim_render::sky::{sun_direction, Sky};
    use creabim_render::util::{Camera, Image};
    use glam::Vec3;
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;

    /// A flat plate `half` metres each way about (x, y), at height z.
    fn plate(x: f32, y: f32, z: f32, half: f32) -> SceneItem {
        SceneItem {
            id: ElementId::NIL,
            mesh: TriMesh {
                positions: vec![
                    [x - half, y - half, z],
                    [x + half, y - half, z],
                    [x + half, y + half, z],
                    [x - half, y + half, z],
                ],
                normals: vec![[0.0, 0.0, 1.0]; 4],
                indices: vec![0, 1, 2, 0, 2, 3],
            },
            color: [0.7, 0.7, 0.7],
            surface: [0.9, 0.0, 1.0],
            ghost: false,
            layer: None,
            finish: None,
        }
    }

    fn lamp(x: f32, y: f32) -> Lamp {
        Lamp {
            pos: Vec3::new(x, y, 2.4),
            axis: -Vec3::Z,
            radius: 0.05,
            candela: profile(60.0, 90.0, 0.35),
            lumens: 1100.0,
            colour: kelvin(2700.0),
            group: 1,
        }
    }

    /// The quick look from straight above the origin, 12 m across.
    fn quick(
        items: &[SceneItem],
        lamps: Vec<Lamp>,
        hour: f32,
        exposure: f32,
        size: usize,
    ) -> (Image, f64) {
        let mut scene = Scene::from_items_on(items, None);
        if !lamps.is_empty() {
            scene.add_lamps(lamps);
        }
        let eye = Vec3::new(0.0, 0.0, 14.0);
        let vp = glam::Mat4::perspective_rh(0.8, 1.0, 0.1, 100.0)
            * glam::Mat4::look_at_rh(eye, Vec3::ZERO, Vec3::Y);
        let req = Request {
            scene: Arc::new(scene),
            camera: Camera::from_view_proj(vp, eye, Vec3::ZERO),
            sky: Sky::new(sun_direction(48.85, 172.0, hour, 0.0)),
            width: size,
            height: size,
            mode: Mode::Look(Look::Realtime { traced: false }),
            exposure,
        };
        let mut last = None;
        let t = std::time::Instant::now();
        job::run(&req, Arc::new(AtomicBool::new(false)), |p| {
            if p.done {
                last = Some(p.image);
            }
        });
        (last.expect("a finished image"), t.elapsed().as_secs_f64())
    }

    fn at(im: &Image, x: usize, y: usize) -> f32 {
        let c = im.px[y * im.w + x];
        c.x + c.y + c.z
    }

    #[test]
    fn at_night_a_lamp_lights_the_floor_under_it_and_a_plate_shades_it() {
        let floor = plate(0.0, 0.0, 0.0, 30.0);
        let (lit, _) = quick(
            std::slice::from_ref(&floor),
            vec![lamp(0.0, 0.0)],
            23.0,
            128.0,
            33,
        );
        let (dark, _) = quick(std::slice::from_ref(&floor), vec![], 23.0, 128.0, 33);
        // under the lamp is far brighter than the night, and than the
        // floor five metres off
        assert!(
            at(&lit, 17, 17) > at(&dark, 17, 17) + 0.2,
            "{} {}",
            at(&lit, 17, 17),
            at(&dark, 17, 17)
        );
        assert!(
            at(&lit, 17, 17) > at(&lit, 1, 1) + 0.15,
            "{} {}",
            at(&lit, 17, 17),
            at(&lit, 1, 1)
        );
        // a plate between them: the floor at its edge of shadow, seen
        // beside the plate, is dark again
        let shade = plate(0.0, 0.0, 1.8, 0.5);
        let (shaded, _) = quick(&[floor, shade], vec![lamp(0.0, 0.0)], 23.0, 128.0, 33);
        // 1.5 m off the axis the plate's shadow (2 m across on the floor)
        // has passed; 0.8 m off, it has not, and the plate does not hide it
        let px = |m: f32| 16 + (m / 11.8 * 33.0).round() as usize;
        assert!(
            at(&shaded, px(0.8), 16) < 0.5 * at(&lit, px(0.8), 16),
            "{} {}",
            at(&shaded, px(0.8), 16),
            at(&lit, px(0.8), 16)
        );
        assert!(at(&shaded, px(2.5), 16) > 0.8 * at(&lit, px(2.5), 16));
    }

    #[test]
    fn by_day_forty_lamps_cost_the_quick_look_next_to_nothing() {
        let floor = plate(0.0, 0.0, 0.0, 30.0);
        let lamps: Vec<Lamp> = (0..40)
            .map(|k| lamp((k % 8) as f32 * 1.5 - 5.0, (k / 8) as f32 * 1.5 - 3.0))
            .collect();
        let best = |lamps: &[Lamp]| {
            (0..3)
                .map(|_| quick(std::slice::from_ref(&floor), lamps.to_vec(), 12.0, 1.0, 160).1)
                .fold(f64::MAX, f64::min)
        };
        let (without, with) = (best(&[]), best(&lamps));
        // looking for forty shadows from every point took many times as
        // long; passing over lamps that add nothing by day, about the same
        assert!(
            with < 2.0 * without + 0.05,
            "with lamps {with:.3} s, without {without:.3} s"
        );
        // and the picture is the daylight one
        let (a, _) = quick(std::slice::from_ref(&floor), lamps, 12.0, 1.0, 33);
        let (b, _) = quick(&[floor], vec![], 12.0, 1.0, 33);
        assert!(
            (at(&a, 20, 20) - at(&b, 20, 20)).abs() < 0.02,
            "{} {}",
            at(&a, 20, 20),
            at(&b, 20, 20)
        );
    }
}

/// White balance: light of the balance's own colour reads as white, and
/// the gain changes colour, not brightness.
#[test]
fn a_white_balance_makes_its_own_light_white() {
    use creabim_render::scene::{kelvin, white_gain};
    use creabim_render::util::luminance;
    // daylight balance changes nothing
    assert!((white_gain(6500.0) - Vec3::ONE).abs().max_element() < 1e-3);
    for k in [2700.0, 3500.0, 9000.0] {
        // its own light is brought most of the way to white: nearer
        // than half, and still on its own side (a lamp stays a little warm)
        let off = |c: Vec3| (c.x / c.z).ln();
        let (raw, seen) = (off(kelvin(k)), off(kelvin(k) * white_gain(k)));
        assert!(
            seen.abs() < 0.5 * raw.abs() && seen.signum() == raw.signum(),
            "{k}: {raw} {seen}"
        );
        assert!((luminance(white_gain(k)) - 1.0).abs() < 1e-3);
    }
    // lamplight at the daylight balance is orange; balanced for lamps, daylight goes blue
    let lamp_day = kelvin(2700.0) * white_gain(6500.0);
    assert!(lamp_day.x > 1.5 * lamp_day.z);
    let day_lamp = kelvin(6500.0) * white_gain(2700.0);
    assert!(day_lamp.z > 1.5 * day_lamp.x);
    // but not one solid blue: green grass keeps green as its strongest
    let grass = Vec3::new(0.10, 0.20, 0.05) * white_gain(2700.0);
    assert!(grass.y > grass.z, "{grass}");
    // a sky takes the gain whole
    let sky = Sky::new(sun_direction(48.85, 172.0, 12.0, 0.0));
    let warm = sky.tinted(white_gain(2700.0));
    assert!(warm.sun_light.z / warm.sun_light.x > 1.5 * sky.sun_light.z / sky.sun_light.x);
}
