//! Rendering a view progressively, for the app: a quick rough image
//! first, then the full one, or pass after pass of the path tracer, each
//! handed back as it is ready and all of it stopped when the view moves.

use crate::look::{self, Look, Settings};
use crate::path::Accum;
use crate::scene::Scene;
use crate::sky::Sky;
use crate::util::{cancelled, with_cancel, Camera, Image};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

/// What to draw a view as.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    Look(Look),
    /// The path tracer, stopping after this many samples a pixel.
    Path {
        samples: u32,
    },
}

pub struct Request {
    pub scene: Arc<Scene>,
    pub camera: Camera,
    /// The sun and sky of the place and moment.
    pub sky: Sky,
    pub width: usize,
    pub height: usize,
    pub mode: Mode,
    /// Brightness for the real-time look and the path tracer: 1 outdoors.
    pub exposure: f32,
}

/// One image along the way, and how far it has got.
pub struct Progress {
    pub image: Image,
    /// "Preview", "Final", or "16 of 256 samples".
    pub stage: String,
    pub done: bool,
    /// Seconds since the request started.
    pub seconds: f64,
}

/// Render `req`, calling `emit` with each better image, until it is done
/// or `cancel` is set.
pub fn run(req: &Request, cancel: Arc<AtomicBool>, mut emit: impl FnMut(Progress)) {
    let t0 = std::time::Instant::now();
    with_cancel(cancel, || match req.mode {
        Mode::Look(l) => {
            // the toon preset has its own light and a short cavity reach
            let toon = l == Look::Toon;
            let sky = if toon {
                Sky::new(Look::TOON_LIGHT.normalize())
            } else {
                req.sky
            };
            let base = Settings {
                ao_radius: if toon { 0.6 } else { 2.5 },
                ..Settings::default()
            };
            // rough: half size, one sample a pixel, few rays
            let rough = Settings {
                ss: 1,
                shadow_rays: 2,
                ao_rays: 3,
                ..base
            };
            let steps = [
                (
                    rough,
                    (req.width / 2).max(1),
                    (req.height / 2).max(1),
                    "Preview",
                ),
                (base, req.width, req.height, "Final"),
            ];
            for (i, (s, w, h, stage)) in steps.into_iter().enumerate() {
                let mut t = look::Timings {
                    primary_ms: 0.0,
                    lighting_ms: 0.0,
                    look_ms: 0.0,
                    rays: 0,
                };
                let g = look::gbuffer(&req.scene, &req.camera, &sky, w, h, &s, &mut t);
                if cancelled() {
                    return;
                }
                let img = look::shade(&req.scene, &sky, &g, w, h, l, &s, req.exposure);
                if cancelled() {
                    return;
                }
                emit(Progress {
                    image: img,
                    stage: stage.into(),
                    done: i == 1,
                    seconds: t0.elapsed().as_secs_f64(),
                });
            }
        }
        Mode::Path { samples } => {
            // pass after pass, shown denoised at 1, 2, 4, 8... samples
            let mut acc = Accum::new(req.width, req.height);
            let mut next = 1u32;
            while acc.spp < samples {
                acc.pass(&req.scene, &req.camera, &req.sky);
                if cancelled() {
                    return;
                }
                if acc.spp == next || acc.spp == samples {
                    let px = crate::denoise::denoise(&acc, 5);
                    if cancelled() {
                        return;
                    }
                    emit(Progress {
                        image: crate::denoise::develop(
                            &px,
                            req.width,
                            req.height,
                            0.85 * req.exposure,
                        ),
                        stage: format!("{} of {} samples", acc.spp, samples),
                        done: acc.spp == samples,
                        seconds: t0.elapsed().as_secs_f64(),
                    });
                    next *= 2;
                }
            }
        }
    });
}
