//! Small shared pieces: random numbers, noise, a camera, the image
//! buffer, and running rows on every core.

use glam::{Vec2, Vec3};
use std::sync::atomic::{AtomicUsize, Ordering};

/// A fast random number generator, one per pixel and pass.
#[derive(Clone)]
pub struct Rng(u64);
impl Rng {
    pub fn new(a: u32, b: u32, c: u32) -> Rng {
        let mut r =
            Rng((((a as u64) << 32) ^ ((b as u64) * 0x9E37_79B9) ^ ((c as u64) * 0x85EB_CA6B)) | 1);
        r.next();
        r.next();
        r
    }
    #[inline(always)]
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> u32 {
        // PCG-XSH-RR
        let old = self.0;
        self.0 = old
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let x = (((old >> 18) ^ old) >> 27) as u32;
        x.rotate_right((old >> 59) as u32)
    }
    #[inline(always)]
    pub fn f(&mut self) -> f32 {
        (self.next() >> 8) as f32 / (1u32 << 24) as f32
    }
}

/// Two directions square to `n` and to each other.
pub fn basis(n: Vec3) -> (Vec3, Vec3) {
    let s = if n.z >= 0.0 { 1.0 } else { -1.0 };
    let a = -1.0 / (s + n.z);
    let b = n.x * n.y * a;
    (
        Vec3::new(1.0 + s * n.x * n.x * a, s * b, -s * n.x),
        Vec3::new(b, s + n.y * n.y * a, -n.y),
    )
}

/// A direction about `n`, more likely near it (cosine weighted).
pub fn cosine_dir(n: Vec3, r1: f32, r2: f32) -> Vec3 {
    let (t, b) = basis(n);
    let r = r1.sqrt();
    let ph = 2.0 * std::f32::consts::PI * r2;
    (t * (r * ph.cos()) + b * (r * ph.sin()) + n * (1.0 - r1).max(0.0).sqrt()).normalize()
}

pub fn luminance(c: Vec3) -> f32 {
    c.dot(Vec3::new(0.2126, 0.7152, 0.0722))
}

// --- noise, for the hand-drawn looks ---

fn hash2(x: i32, y: i32, seed: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(374761393)
        ^ (y as u32).wrapping_mul(668265263)
        ^ seed.wrapping_mul(2246822519);
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    h ^= h >> 16;
    (h & 0xFFFFFF) as f32 / 0xFFFFFF as f32
}

/// Smooth value noise in 0..1.
pub fn noise(p: Vec2, seed: u32) -> f32 {
    let i = p.floor();
    let f = p - i;
    let u = f * f * (Vec2::splat(3.0) - 2.0 * f);
    let (x, y) = (i.x as i32, i.y as i32);
    let a = hash2(x, y, seed);
    let b = hash2(x + 1, y, seed);
    let c = hash2(x, y + 1, seed);
    let d = hash2(x + 1, y + 1, seed);
    let ab = a + (b - a) * u.x;
    let cd = c + (d - c) * u.x;
    ab + (cd - ab) * u.y
}

/// Fractal noise, several octaves, about 0..1.
pub fn fbm(p: Vec2, octaves: u32, seed: u32) -> f32 {
    let (mut s, mut a, mut f, mut norm) = (0.0, 0.5, 1.0, 0.0);
    for o in 0..octaves {
        s += a * noise(p * f, seed + o * 31);
        norm += a;
        a *= 0.5;
        f *= 2.03;
    }
    s / norm
}

pub fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

// --- camera ---

/// An architectural camera: verticals stay vertical (a shifted lens, as
/// in two-point perspective), the target framed in the middle.
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub eye: Vec3,
    pub target: Vec3,
    /// Vertical field of view, degrees.
    pub fov: f32,
    pub two_point: bool,
    /// Depth of field: lens radius (metres) and focus distance; 0 is sharp.
    pub aperture: f32,
    pub focus: f32,
    /// A camera given by its inverse view-projection matrix instead (the
    /// app's own 3D view, perspective or parallel): the other fields are
    /// then only used for depth of field.
    pub inverse: Option<glam::Mat4>,
}

impl Camera {
    /// The camera of a view drawn with `view_proj`.
    pub fn from_view_proj(view_proj: glam::Mat4, eye: Vec3, target: Vec3) -> Camera {
        Camera {
            eye,
            target,
            fov: 45.0,
            two_point: false,
            aperture: 0.0,
            focus: 10.0,
            inverse: Some(view_proj.inverse()),
        }
    }

    /// The ray through a point of the image: x and y from 0 to 1, y down.
    pub fn ray(&self, x: f32, y: f32, aspect: f32, lens: Option<(f32, f32)>) -> (Vec3, Vec3) {
        if let Some(inv) = self.inverse {
            let n = glam::Vec2::new(2.0 * x - 1.0, 1.0 - 2.0 * y);
            let near = inv.project_point3(n.extend(0.0));
            let far = inv.project_point3(n.extend(1.0));
            return (near, (far - near).normalize());
        }
        let tan = (self.fov.to_radians() * 0.5).tan();
        let to = self.target - self.eye;
        let (fwd, right, up, shift) = if self.two_point {
            let f = Vec3::new(to.x, to.y, 0.0).normalize();
            let r = f.cross(Vec3::Z).normalize();
            let horiz = Vec3::new(to.x, to.y, 0.0).length();
            (f, r, Vec3::Z, to.z / horiz / tan)
        } else {
            let f = to.normalize();
            let r = f.cross(Vec3::Z).normalize();
            (f, r, r.cross(f), 0.0)
        };
        let sx = (2.0 * x - 1.0) * tan * aspect;
        let sy = (1.0 - 2.0 * y + shift) * tan;
        let d = (fwd + right * sx + up * sy).normalize();
        match lens {
            Some((a, b)) if self.aperture > 0.0 => {
                let focal = self.eye + d * (self.focus / d.dot(fwd));
                let r = self.aperture * a.sqrt();
                let ph = 2.0 * std::f32::consts::PI * b;
                let o = self.eye + right * (r * ph.cos()) + up * (r * ph.sin());
                (o, (focal - o).normalize())
            }
            _ => (self.eye, d),
        }
    }
}

// --- images ---

#[derive(Clone)]
pub struct Image {
    pub w: usize,
    pub h: usize,
    pub px: Vec<Vec3>,
}

impl Image {
    pub fn new(w: usize, h: usize) -> Image {
        Image {
            w,
            h,
            px: vec![Vec3::ZERO; w * h],
        }
    }
    pub fn at(&self, x: isize, y: isize) -> Vec3 {
        let x = x.clamp(0, self.w as isize - 1) as usize;
        let y = y.clamp(0, self.h as isize - 1) as usize;
        self.px[y * self.w + x]
    }
    /// Bilinear sample in pixel units.
    pub fn sample(&self, p: Vec2) -> Vec3 {
        let p = p - Vec2::splat(0.5);
        let i = p.floor();
        let f = p - i;
        let (x, y) = (i.x as isize, i.y as isize);
        let a = self.at(x, y).lerp(self.at(x + 1, y), f.x);
        let b = self.at(x, y + 1).lerp(self.at(x + 1, y + 1), f.x);
        a.lerp(b, f.y)
    }
    /// Average each `k` by `k` block into one pixel.
    pub fn downsample(&self, k: usize) -> Image {
        let mut out = Image::new(self.w / k, self.h / k);
        for y in 0..out.h {
            for x in 0..out.w {
                let mut s = Vec3::ZERO;
                for j in 0..k {
                    for i in 0..k {
                        s += self.px[(y * k + j) * self.w + x * k + i];
                    }
                }
                out.px[y * out.w + x] = s / (k * k) as f32;
            }
        }
        out
    }
    /// Save as an 8-bit PNG; the pixels are display values (0..1, sRGB).
    pub fn save_png(&self, path: &std::path::Path) -> image::ImageResult<()> {
        let mut buf = image::RgbImage::new(self.w as u32, self.h as u32);
        for (i, p) in buf.pixels_mut().enumerate() {
            let c = self.px[i].clamp(Vec3::ZERO, Vec3::ONE) * 255.0 + Vec3::splat(0.5);
            *p = image::Rgb([c.x as u8, c.y as u8, c.z as u8]);
        }
        buf.save(path)
    }
}

thread_local! {
    static CANCEL: std::cell::RefCell<Option<std::sync::Arc<std::sync::atomic::AtomicBool>>> =
        const { std::cell::RefCell::new(None) };
}

/// Run `f` so that the rendering it does stops early, leaving rows blank,
/// once `cancel` is set: a view that moved on no longer needs it.
pub fn with_cancel<R>(
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    f: impl FnOnce() -> R,
) -> R {
    CANCEL.with(|c| *c.borrow_mut() = Some(cancel));
    let r = f();
    CANCEL.with(|c| *c.borrow_mut() = None);
    r
}

/// Has the rendering on this thread been called off?
pub fn cancelled() -> bool {
    CANCEL.with(|c| {
        c.borrow()
            .as_ref()
            .is_some_and(|a| a.load(Ordering::Relaxed))
    })
}

/// Run `f` for each row on every core; it fills that row.
pub fn par_rows<T: Send + Clone>(
    w: usize,
    h: usize,
    init: T,
    f: impl Fn(usize, &mut [T]) + Sync,
) -> Vec<T> {
    let mut out = vec![init; w * h];
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let next = AtomicUsize::new(0);
    let cancel = CANCEL.with(|c| c.borrow().clone());
    let rows: Vec<std::sync::Mutex<&mut [T]>> =
        out.chunks_mut(w).map(std::sync::Mutex::new).collect();
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| loop {
                let y = next.fetch_add(1, Ordering::Relaxed);
                if y >= h || cancel.as_ref().is_some_and(|c| c.load(Ordering::Relaxed)) {
                    break;
                }
                let mut row = rows[y].lock().unwrap();
                f(y, &mut row);
            });
        }
    });
    drop(rows);
    out
}

/// sRGB encoding of a linear value.
pub fn to_srgb(c: Vec3) -> Vec3 {
    let f = |x: f32| {
        let x = x.clamp(0.0, 1.0);
        if x <= 0.0031308 {
            x * 12.92
        } else {
            1.055 * x.powf(1.0 / 2.4) - 0.055
        }
    };
    Vec3::new(f(c.x), f(c.y), f(c.z))
}

/// Filmic tone curve (the ACES fit), linear in, linear 0..1 out.
pub fn aces(c: Vec3) -> Vec3 {
    let a = c * (c * 2.51 + Vec3::splat(0.03));
    let b = c * (c * 2.43 + Vec3::splat(0.59)) + Vec3::splat(0.14);
    (a / b).clamp(Vec3::ZERO, Vec3::ONE)
}
