//! Site services under the ground: drain runs laid to a fall from chamber
//! to chamber, and tanks (a septic tank, a rainwater tank, a soakaway).
//! Each is dug in and backfilled. Its trench or pit is measured apart
//! from the earthworks, because it leaves the ground as it was.

use glam::DVec2;
use serde::{Deserialize, Serialize};

/// What a drain carries.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DrainUse {
    /// Foul water, to a sewer or a septic tank.
    #[default]
    Foul,
    /// Rain off roofs and paving, to a soakaway, a tank or a sewer.
    Surface,
    /// Both in one pipe.
    Combined,
    /// A drainage field: perforated pipe laid near level in a gravel
    /// trench, letting treated water soak into the ground.
    Field,
}

impl DrainUse {
    pub const ALL: [DrainUse; 4] = [
        DrainUse::Foul,
        DrainUse::Surface,
        DrainUse::Combined,
        DrainUse::Field,
    ];

    pub fn label(self) -> &'static str {
        match self {
            DrainUse::Foul => "Foul",
            DrainUse::Surface => "Surface water",
            DrainUse::Combined => "Combined",
            DrainUse::Field => "Drainage field",
        }
    }

    /// The letter its chambers are numbered with: F1, S1, C1.
    pub fn letter(self) -> &'static str {
        match self {
            DrainUse::Foul => "F",
            DrainUse::Surface => "S",
            DrainUse::Combined => "C",
            DrainUse::Field => "D",
        }
    }
}

/// A drain run: a pipe along `path` in plan, from its head (the first
/// point) down to its outfall (the last), falling evenly all the way.
/// The levels it is laid to are worked out from the ground: see
/// [`Drain::inverts`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Drain {
    pub path: Vec<DVec2>,
    #[serde(default)]
    pub service: DrainUse,
    /// The pipe's bore.
    #[serde(default = "point_one")]
    pub diameter: f64,
    /// Its fall, per cent: 1.25 is 1 in 80.
    #[serde(default = "one_and_a_quarter")]
    pub fall: f64,
    /// How deep its invert (the inside of the pipe's bottom) is under the
    /// ground at its head.
    #[serde(default = "point_six")]
    pub depth: f64,
    /// A chamber at its head, at each bend and at its end.
    #[serde(default = "yes")]
    pub chambers: bool,
    /// A chamber's inside size, square.
    #[serde(default = "point_six")]
    pub chamber: f64,
    /// How wide its trench is dug.
    #[serde(default = "point_six")]
    pub trench: f64,
    /// The granular bed under the pipe.
    #[serde(default = "point_one")]
    pub bed: f64,
}

/// What a tank is for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TankUse {
    /// Settles foul water before it goes to a drainage field.
    #[default]
    Septic,
    /// A package plant that treats foul water.
    Treatment,
    /// Keeps rain for use.
    Rainwater,
    /// Holds water back and lets it out slowly.
    Attenuation,
    /// Lets surface water soak away into the ground: a pit of stone or
    /// crates.
    Soakaway,
}

impl TankUse {
    pub const ALL: [TankUse; 5] = [
        TankUse::Septic,
        TankUse::Treatment,
        TankUse::Rainwater,
        TankUse::Attenuation,
        TankUse::Soakaway,
    ];

    pub fn label(self) -> &'static str {
        match self {
            TankUse::Septic => "Septic tank",
            TankUse::Treatment => "Treatment plant",
            TankUse::Rainwater => "Rainwater tank",
            TankUse::Attenuation => "Attenuation tank",
            TankUse::Soakaway => "Soakaway",
        }
    }
}

/// A tank buried in the ground: a box `length` by `width` in plan round
/// `centre`, turned `angle` degrees anticlockwise, `depth` deep inside,
/// its top `cover` under the ground.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tank {
    pub centre: DVec2,
    #[serde(default)]
    pub service: TankUse,
    pub length: f64,
    pub width: f64,
    pub depth: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub angle: f64,
    /// How deep its top is under the ground.
    #[serde(default = "point_three")]
    pub cover: f64,
    /// Its walls, floor and lid.
    #[serde(default = "point_one_five")]
    pub wall: f64,
    /// The room dug round it to set it in, each side.
    #[serde(default = "point_five")]
    pub working: f64,
}

fn point_one() -> f64 {
    0.1
}
fn point_three() -> f64 {
    0.3
}
fn point_five() -> f64 {
    0.5
}
fn point_six() -> f64 {
    0.6
}
fn point_one_five() -> f64 {
    0.15
}
fn one_and_a_quarter() -> f64 {
    1.25
}
fn yes() -> bool {
    true
}
fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

fn positive(v: f64, what: &str) -> Result<(), String> {
    if v.is_finite() && v > 0.0 {
        Ok(())
    } else {
        Err(format!("{what} must be positive, got {v}"))
    }
}

impl Drain {
    /// A drain along `path`, carrying `service`, its defaults a 100 mm
    /// pipe at 1 in 80, 600 mm deep at its head (a drainage field's 1 in
    /// 200, 500 mm deep, on 300 mm of gravel, with no chambers).
    pub fn new(path: Vec<DVec2>, service: DrainUse) -> Self {
        if service == DrainUse::Field {
            // near level, shallow, on a deep bed of gravel, no chambers
            return Self {
                path,
                service,
                diameter: 0.1,
                fall: 0.5,
                depth: 0.5,
                chambers: false,
                chamber: 0.6,
                trench: 0.6,
                bed: 0.3,
            };
        }
        Self {
            path,
            service,
            diameter: 0.1,
            fall: 1.25,
            depth: 0.6,
            chambers: true,
            chamber: 0.6,
            trench: 0.6,
            bed: 0.1,
        }
    }

    /// How long it is.
    pub fn length(&self) -> f64 {
        self.path.windows(2).map(|w| w[0].distance(w[1])).sum()
    }

    /// The invert at each point of its path, from `head`, the invert at
    /// its first, falling by its fall along it.
    pub fn inverts(&self, head: f64) -> Vec<f64> {
        let f = self.fall / 100.0;
        let mut z = head;
        let mut out = Vec::with_capacity(self.path.len());
        for (i, p) in self.path.iter().enumerate() {
            if i > 0 {
                z -= f * self.path[i - 1].distance(*p);
            }
            out.push(z);
        }
        out
    }

    /// "1:80" for its fall, or the per cent when it isn't near a round
    /// one in so many.
    pub fn gradient_text(&self) -> String {
        if self.fall <= 0.0 {
            return "level".into();
        }
        let n = 100.0 / self.fall;
        if (n - n.round()).abs() < 0.05 {
            format!("1:{:.0}", n.round())
        } else {
            format!("{:.2}%", self.fall)
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.path.len() < 2 {
            return Err("a drain needs at least two points".into());
        }
        if self.path.iter().any(|p| !p.is_finite()) {
            return Err("a drain's points must be finite".into());
        }
        positive(self.diameter, "pipe diameter")?;
        positive(self.trench, "trench width")?;
        positive(self.chamber, "chamber size")?;
        if !(self.fall.is_finite() && (0.0..=50.0).contains(&self.fall)) {
            return Err(format!(
                "a drain's fall must be 0 to 50%, got {}",
                self.fall
            ));
        }
        if !(self.depth.is_finite() && self.depth >= 0.0) {
            return Err(format!(
                "a drain's depth must be 0 or more, got {}",
                self.depth
            ));
        }
        if !(self.bed.is_finite() && self.bed >= 0.0) {
            return Err(format!("a drain's bed must be 0 or more, got {}", self.bed));
        }
        Ok(())
    }
}

impl Tank {
    /// A tank of `service`, its usual size, at `centre`.
    pub fn new(centre: DVec2, service: TankUse) -> Self {
        let (length, width, depth) = match service {
            TankUse::Septic => (2.4, 1.2, 1.5),
            TankUse::Treatment => (2.0, 2.0, 2.0),
            TankUse::Rainwater => (2.4, 1.2, 1.5),
            TankUse::Attenuation => (4.0, 2.0, 1.0),
            TankUse::Soakaway => (2.0, 2.0, 1.0),
        };
        Self {
            centre,
            service,
            length,
            width,
            depth,
            angle: 0.0,
            cover: 0.3,
            wall: 0.15,
            working: 0.5,
        }
    }

    /// Its outline in plan, outside its walls, anticlockwise.
    pub fn outline(&self) -> Vec<DVec2> {
        self.rect(self.wall)
    }

    /// Its outline grown by `by` each side of its inside.
    pub fn rect(&self, by: f64) -> Vec<DVec2> {
        let (hl, hw) = (self.length / 2.0 + by, self.width / 2.0 + by);
        let r = DVec2::from_angle(self.angle.to_radians());
        [(-hl, -hw), (hl, -hw), (hl, hw), (-hl, hw)]
            .into_iter()
            .map(|(x, y)| self.centre + r.rotate(DVec2::new(x, y)))
            .collect()
    }

    /// What it holds, in cubic metres.
    pub fn capacity(&self) -> f64 {
        self.length * self.width * self.depth
    }

    pub fn validate(&self) -> Result<(), String> {
        if !self.centre.is_finite() {
            return Err("a tank's position must be finite".into());
        }
        positive(self.length, "tank length")?;
        positive(self.width, "tank width")?;
        positive(self.depth, "tank depth")?;
        for (v, what) in [
            (self.cover, "cover"),
            (self.wall, "wall"),
            (self.working, "working space"),
        ] {
            if !(v.is_finite() && v >= 0.0) {
                return Err(format!("a tank's {what} must be 0 or more, got {v}"));
            }
        }
        if !self.angle.is_finite() {
            return Err("a tank's angle must be finite".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_drain_falls_evenly_from_its_head() {
        let d = Drain::new(
            vec![DVec2::ZERO, DVec2::new(8.0, 0.0), DVec2::new(8.0, 8.0)],
            DrainUse::Foul,
        );
        assert_eq!(d.gradient_text(), "1:80");
        let z = d.inverts(10.0);
        assert!((z[1] - 9.9).abs() < 1e-9 && (z[2] - 9.8).abs() < 1e-9);
        assert!(d.validate().is_ok());
        let t = Tank::new(DVec2::new(1.0, 1.0), TankUse::Septic);
        assert!((t.capacity() - 4.32).abs() < 1e-9);
        assert_eq!(t.outline().len(), 4);
    }
}
