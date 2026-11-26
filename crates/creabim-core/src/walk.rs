//! Paths laid on the ground: gravel, paving, asphalt or bark along a
//! line, following the ground as it is (after the works), with edging
//! each side. Unlike a road, a path is not worked into the terrain: it
//! is dug only as deep as its make-up.

use glam::DVec2;
use serde::{Deserialize, Serialize};

/// What a path's surface is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Surface {
    /// Loose gravel or hoggin.
    #[default]
    Gravel,
    /// Slabs or blocks, laid in a pattern.
    Paving,
    /// Bound asphalt or tarmac.
    Asphalt,
    /// Bark or wood chip.
    Bark,
}

impl Surface {
    pub const ALL: [Surface; 4] = [
        Surface::Gravel,
        Surface::Paving,
        Surface::Asphalt,
        Surface::Bark,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Surface::Gravel => "Gravel",
            Surface::Paving => "Paving",
            Surface::Asphalt => "Asphalt",
            Surface::Bark => "Bark",
        }
    }

    /// Its colour in plan and 3D, sRGB 0..1.
    pub fn rgb(self) -> [f32; 3] {
        match self {
            Surface::Gravel => [0.85, 0.81, 0.72],
            Surface::Paving => [0.80, 0.78, 0.74],
            Surface::Asphalt => [0.45, 0.46, 0.47],
            Surface::Bark => [0.55, 0.40, 0.28],
        }
    }
}

/// A path along `path` in plan, `width` wide, on the ground.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Walk {
    pub path: Vec<DVec2>,
    pub width: f64,
    #[serde(default)]
    pub surface: Surface,
    /// Edging each side (timber, steel or a pin kerb), this wide; 0 is
    /// none.
    #[serde(default = "point_zero_five")]
    pub edging: f64,
    /// How deep its surface and sub-base go.
    #[serde(default = "point_one_five")]
    pub thickness: f64,
    /// Its bends are rounded to this radius on its middle line; 0 leaves
    /// them as corners.
    #[serde(default, skip_serializing_if = "no_radius")]
    pub radius: f64,
}

fn no_radius(v: &f64) -> bool {
    *v == 0.0
}

fn point_zero_five() -> f64 {
    0.05
}
fn point_one_five() -> f64 {
    0.15
}

impl Walk {
    /// A gravel path along `path`, 1.2 m wide, with 50 mm edging.
    pub fn new(path: Vec<DVec2>, surface: Surface) -> Self {
        Self {
            path,
            width: 1.2,
            surface,
            edging: 0.05,
            thickness: if surface == Surface::Asphalt {
                0.2
            } else {
                0.15
            },
            radius: 0.0,
        }
    }

    /// Its middle line as it is laid: through its points, each bend
    /// rounded to its radius.
    pub fn line(&self) -> Vec<DVec2> {
        crate::road::round_plan(&self.path, self.radius)
    }

    /// The path as it is laid: its line with its bends rounded, to draw
    /// and build from.
    pub fn rounded(&self) -> Walk {
        Walk {
            path: self.line(),
            radius: 0.0,
            ..self.clone()
        }
    }

    /// How long its middle line is.
    pub fn length(&self) -> f64 {
        self.line().windows(2).map(|w| w[0].distance(w[1])).sum()
    }

    /// Its two edges in plan, left then right of its line, mitred at
    /// each bend, `half` out from it.
    /// Each pairs point for point with [`Walk::line`]; round a bend the
    /// inner edge is a smaller arc, or a sharp corner (see
    /// [`crate::road::offset_round`]).
    pub fn edges(&self, half: f64) -> (Vec<DVec2>, Vec<DVec2>) {
        (
            crate::road::offset_round(&self.path, self.radius, half),
            crate::road::offset_round(&self.path, self.radius, -half),
        )
    }

    /// Its outline in plan, outside its edging, anticlockwise.
    pub fn outline(&self) -> Vec<DVec2> {
        let (l, r) = self.edges(self.width / 2.0 + self.edging);
        let mut out = r;
        out.extend(l.into_iter().rev());
        out
    }

    /// Its surface area, between its edging.
    pub fn area(&self) -> f64 {
        self.length() * self.width
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.path.len() < 2 {
            return Err("a path needs at least two points".into());
        }
        if self.path.iter().any(|p| !p.is_finite()) {
            return Err("a path's points must be finite".into());
        }
        if !(self.width.is_finite() && self.width > 0.0) {
            return Err(format!(
                "a path's width must be positive, got {}",
                self.width
            ));
        }
        for (v, what) in [(self.edging, "edging"), (self.thickness, "thickness")] {
            if !(v.is_finite() && v >= 0.0) {
                return Err(format!("a path's {what} must be 0 or more, got {v}"));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_has_two_edges_and_an_area() {
        let w = Walk::new(
            vec![DVec2::ZERO, DVec2::new(10.0, 0.0), DVec2::new(10.0, 5.0)],
            Surface::Gravel,
        );
        assert!(w.validate().is_ok());
        assert!((w.area() - 18.0).abs() < 1e-9);
        let (l, r) = w.edges(0.6);
        assert!((l[0].y - 0.6).abs() < 1e-9 && (r[0].y + 0.6).abs() < 1e-9);
        // the bend's corner is mitred: square out from both legs
        assert!((l[1] - DVec2::new(9.4, 0.6)).length() < 1e-9, "{:?}", l[1]);
        assert_eq!(w.outline().len(), 6);
    }
}
