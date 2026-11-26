//! A prototype renderer for CreaBIM: one scene and one set of materials,
//! drawn three ways. The fast looks ([`look`]) are what a GPU would do
//! every frame; the path tracer ([`path`]) is the slow, physically based
//! one for final images; [`denoise`] cleans a path-traced image up early.
//!
//! Everything runs on the CPU here, so it can render without a window or a
//! graphics card (for the demo and for tests); in the app the fast looks
//! would be wgpu shaders on the 3D view.

pub mod bvh;
pub mod denoise;
pub mod job;
pub mod look;
pub mod path;
pub mod scene;
pub mod sky;
pub mod util;
