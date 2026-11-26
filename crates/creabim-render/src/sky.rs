//! Sun and sky for a place and a moment: the sun's direction from the
//! project's latitude, the day and the hour, and a simple analytic sky
//! whose colours follow how high the sun is.

use glam::Vec3;
use std::f32::consts::PI;

#[derive(Clone, Copy, Debug)]
pub struct Sky {
    /// Towards the sun.
    pub sun: Vec3,
    /// What the sun lights a surface facing it with (irradiance).
    pub sun_light: Vec3,
    /// The sun's angular radius, radians: wider gives softer shadows.
    pub sun_radius: f32,
    zenith: Vec3,
    horizon: Vec3,
    ground: Vec3,
    /// Sky light falling on an upward surface, a sideways one and a
    /// downward one, for the real-time mode's ambient term.
    pub irradiance: [Vec3; 3],
}

/// What the sky is like: it sets how strong and how sharp the sun is,
/// and the sky's own colours.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SkyKind {
    /// A clear sky: a strong sun, sharp shadows, blue overhead.
    #[default]
    Clear,
    /// Haze: a weaker sun through it, soft-edged shadows, a pale sky.
    Hazy,
    /// Cloud all over: almost no sun, no shadows to speak of, grey light
    /// from everywhere.
    Overcast,
}

impl SkyKind {
    pub const ALL: [SkyKind; 3] = [SkyKind::Clear, SkyKind::Hazy, SkyKind::Overcast];

    pub fn key(self) -> &'static str {
        match self {
            SkyKind::Clear => "clear",
            SkyKind::Hazy => "hazy",
            SkyKind::Overcast => "overcast",
        }
    }

    /// The kind a key names; a clear sky for one not known.
    pub fn of_key(key: &str) -> SkyKind {
        SkyKind::ALL
            .into_iter()
            .find(|k| k.key() == key)
            .unwrap_or_default()
    }

    pub fn label(self) -> &'static str {
        match self {
            SkyKind::Clear => "Clear",
            SkyKind::Hazy => "Hazy",
            SkyKind::Overcast => "Overcast",
        }
    }
}

/// Sunrise and sunset, solar hours, for a latitude (degrees north) and a
/// day of the year; None where the sun does not rise or set that day.
pub fn sun_hours(latitude: f32, day: f32) -> Option<(f32, f32)> {
    let lat = latitude.to_radians();
    let decl = (23.44f32).to_radians() * (2.0 * PI * (284.0 + day) / 365.0).sin();
    let c = -lat.tan() * decl.tan();
    (c.abs() < 1.0).then(|| {
        let h = c.acos().to_degrees() / 15.0;
        (12.0 - h, 12.0 + h)
    })
}

/// The sun's direction for a latitude (degrees north), a day of the
/// year (1-365) and solar time in hours; `north` turns true north from the
/// drawings' up, degrees anticlockwise. X is east, Y north, Z up.
pub fn sun_direction(latitude: f32, day: f32, hour: f32, north: f32) -> Vec3 {
    let lat = latitude.to_radians();
    let decl = (23.44f32).to_radians() * (2.0 * PI * (284.0 + day) / 365.0).sin();
    let h = (15.0 * (hour - 12.0)).to_radians();
    let sin_el = lat.sin() * decl.sin() + lat.cos() * decl.cos() * h.cos();
    let el = sin_el.clamp(-1.0, 1.0).asin();
    // azimuth from north, clockwise (east positive)
    let az =
        (-h.sin() * decl.cos()).atan2(decl.sin() * lat.cos() - decl.cos() * lat.sin() * h.cos());
    let d = Vec3::new(az.sin() * el.cos(), az.cos() * el.cos(), el.sin());
    let (s, c) = north.to_radians().sin_cos();
    Vec3::new(d.x * c - d.y * s, d.x * s + d.y * c, d.z)
}

impl Sky {
    /// A clear sky at its own strength.
    pub fn new(sun: Vec3) -> Sky {
        Sky::with(sun, SkyKind::Clear, 1.0)
    }

    /// A sky of `kind`, its own light (not the sun's) times `strength`.
    pub fn with(sun: Vec3, kind: SkyKind, strength: f32) -> Sky {
        let sun = sun.normalize();
        let el = sun.z.max(0.02);
        let t = (el / 0.6).clamp(0.0, 1.0);
        // warmer and weaker near the horizon
        let tint = Vec3::new(1.0, 0.60, 0.36).lerp(Vec3::new(1.0, 0.95, 0.89), t.sqrt());
        let sun_light = tint * 3.2 * (0.35 + 0.65 * t);
        let zenith = Vec3::new(0.10, 0.20, 0.42).lerp(Vec3::new(0.16, 0.30, 0.62), t) * 0.85;
        let horizon = Vec3::new(0.62, 0.52, 0.48).lerp(Vec3::new(0.62, 0.72, 0.85), t);
        let ground = Vec3::new(0.18, 0.17, 0.15);
        let grey = |c: Vec3| Vec3::splat(c.dot(Vec3::new(0.2126, 0.7152, 0.0722)));
        let (sun_light, radius, zenith, horizon) = match kind {
            SkyKind::Clear => (sun_light, 0.6f32, zenith, horizon),
            SkyKind::Hazy => (
                sun_light * 0.55,
                3.0,
                zenith.lerp(Vec3::new(0.50, 0.56, 0.64), 0.6),
                horizon.lerp(Vec3::new(0.78, 0.80, 0.82), 0.6),
            ),
            // brighter overhead than at the horizon, as a clouded sky is
            SkyKind::Overcast => (
                grey(sun_light) * 0.05,
                10.0,
                Vec3::new(0.60, 0.63, 0.67) * (0.45 + 0.55 * t),
                Vec3::new(0.40, 0.42, 0.45) * (0.45 + 0.55 * t),
            ),
        };
        let k = strength.clamp(0.0, 4.0);
        // the sun under the horizon: no sunlight, and the sky goes dark
        // with it, nearly black by six degrees under
        let (sun_light, k) = if sun.z < 0.0 {
            let dusk = ((sun.z + 0.10) / 0.10).clamp(0.0, 1.0);
            (Vec3::ZERO, k * (0.001 + 0.3 * dusk * dusk))
        } else {
            (sun_light, k)
        };
        let (zenith, horizon) = (zenith * k, horizon * k);
        let ground = ground * if sun.z < 0.0 { 0.02 } else { 1.0 };
        let mut sky = Sky {
            sun,
            sun_light,
            sun_radius: radius.to_radians(),
            zenith,
            horizon,
            ground,
            irradiance: [Vec3::ZERO; 3],
        };
        // integrate the sky over three hemispheres, once
        for (k, n) in [Vec3::Z, Vec3::X, -Vec3::Z].into_iter().enumerate() {
            let mut acc = Vec3::ZERO;
            let steps = 48;
            for i in 0..steps {
                for j in 0..steps * 2 {
                    let th = (i as f32 + 0.5) / steps as f32 * PI;
                    let ph = (j as f32 + 0.5) / (steps * 2) as f32 * 2.0 * PI;
                    let d = Vec3::new(th.sin() * ph.cos(), th.sin() * ph.sin(), th.cos());
                    let c = d.dot(n);
                    if c > 0.0 {
                        let dw = th.sin() * (PI / steps as f32) * (PI / steps as f32);
                        acc += sky.radiance(d) * c * dw;
                    }
                }
            }
            sky.irradiance[k] = acc;
        }
        sky
    }

    /// The same sky with all its light times `gain` (a white balance).
    pub fn tinted(mut self, gain: Vec3) -> Sky {
        self.sun_light *= gain;
        self.zenith *= gain;
        self.horizon *= gain;
        self.ground *= gain;
        for i in &mut self.irradiance {
            *i *= gain;
        }
        self
    }

    /// Sky light from a direction, without the sun's disc (its glow is in).
    pub fn radiance(&self, d: Vec3) -> Vec3 {
        if d.z < 0.0 {
            let fade = (-d.z * 6.0).min(1.0);
            return self.horizon.lerp(self.ground, fade) * 0.9;
        }
        let up = d.z.max(0.0);
        let g = (1.0 - up).powf(4.0);
        let mut c = self.zenith.lerp(self.horizon, g);
        let cos = d.dot(self.sun).max(-1.0);
        // glow round the sun
        c +=
            self.sun_light * (0.05 * ((cos - 1.0) * 30.0).exp() + 0.02 * ((cos - 1.0) * 3.0).exp());
        c
    }

    /// Radiance of the sun's disc itself, for a ray that meets it.
    pub fn sun_disc(&self) -> Vec3 {
        let solid = PI * self.sun_radius * self.sun_radius;
        self.sun_light / solid
    }

    /// Ambient sky light on a surface facing `n`, unshadowed.
    pub fn ambient(&self, n: Vec3) -> Vec3 {
        let side = self.irradiance[1];
        if n.z >= 0.0 {
            side.lerp(self.irradiance[0], n.z)
        } else {
            side.lerp(self.irradiance[2], -n.z)
        }
    }

    /// A direction towards the sun's disc, from two random numbers.
    pub fn sample_sun(&self, r1: f32, r2: f32) -> Vec3 {
        let (t, b) = crate::util::basis(self.sun);
        let cos_max = self.sun_radius.cos();
        let c = 1.0 - r1 * (1.0 - cos_max);
        let s = (1.0 - c * c).max(0.0).sqrt();
        let ph = 2.0 * PI * r2;
        (t * (s * ph.cos()) + b * (s * ph.sin()) + self.sun * c).normalize()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_overcast_sky_has_almost_no_sun_and_grey_light_a_hazy_one_soft_shadows() {
        let sun = sun_direction(48.85, 172.0, 12.0, 0.0);
        let clear = Sky::with(sun, SkyKind::Clear, 1.0);
        let hazy = Sky::with(sun, SkyKind::Hazy, 1.0);
        let over = Sky::with(sun, SkyKind::Overcast, 1.0);
        // Sky::new is the clear sky, as before
        let was = Sky::new(sun);
        assert_eq!(
            (was.sun_light, was.sun_radius, was.irradiance),
            (clear.sun_light, clear.sun_radius, clear.irradiance)
        );
        assert!(over.sun_light.max_element() < 0.1 * clear.sun_light.max_element());
        assert!(hazy.sun_light.max_element() < clear.sun_light.max_element());
        assert!(hazy.sun_radius > 3.0 * clear.sun_radius && over.sun_radius > hazy.sun_radius);
        // a clear sky lights a roof blue, a clouded one grey
        let (c, o) = (clear.irradiance[0], over.irradiance[0]);
        assert!(c.z > 1.3 * c.x, "{c:?}");
        assert!((o.z / o.x) < 1.2, "{o:?}");
        // and the sky's strength scales its own light, not the sun's
        let half = Sky::with(sun, SkyKind::Overcast, 0.5);
        assert!(half.irradiance[0].y < 0.6 * o.y && half.irradiance[0].y > 0.4 * o.y);
        assert_eq!(half.sun_light, over.sun_light);
    }

    #[test]
    fn after_sunset_there_is_no_sunlight_and_the_sky_is_dark() {
        let day = Sky::new(sun_direction(48.85, 172.0, 12.0, 0.0));
        let night = Sky::new(sun_direction(48.85, 172.0, 22.0, 0.0));
        assert!(night.sun.z < -0.1);
        assert_eq!(night.sun_light, Vec3::ZERO);
        assert!(night.irradiance[0].max_element() < 0.01 * day.irradiance[0].max_element());
        // dusk is between the two
        let (_, set) = sun_hours(48.85, 172.0).unwrap();
        let dusk = Sky::new(sun_direction(48.85, 172.0, set + 0.25, 0.0));
        assert!(
            dusk.irradiance[0].y > night.irradiance[0].y
                && dusk.irradiance[0].y < day.irradiance[0].y
        );
    }

    #[test]
    fn the_sun_rises_and_sets_about_noon_and_days_are_long_in_summer() {
        let (rise, set) = sun_hours(48.85, 172.0).unwrap();
        assert!((rise + set - 24.0).abs() < 1e-3);
        assert!(set - rise > 15.5 && set - rise < 16.5, "{rise} {set}");
        let (rise, set) = sun_hours(48.85, 355.0).unwrap();
        assert!(set - rise > 7.5 && set - rise < 8.8, "{rise} {set}");
        // at the equator, twelve hours all year
        let (rise, set) = sun_hours(0.0, 100.0).unwrap();
        assert!((set - rise - 12.0).abs() < 0.01);
        // no sunrise in the polar night
        assert!(sun_hours(80.0, 355.0).is_none());
        for k in SkyKind::ALL {
            assert_eq!(SkyKind::of_key(k.key()), k);
        }
        assert_eq!(SkyKind::of_key("nonsense"), SkyKind::Clear);
    }
}
