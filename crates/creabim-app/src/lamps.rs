//! Lamps: the model's light fittings as the renderer's lamps (Visuals
//! REV02, Light). A fitting is a placed object whose type is an
//! IfcLightFixture, or any object type given a light. What it gives is
//! its type's Light (output, colour, beam, group); where it leaves the
//! fitting follows how the fitting is mounted.

use super::CreaBimApp;
use creabim_core::objects::Mount;
use creabim_core::{ElementKind, Light, Spread};
use creabim_render::scene::{kelvin, profile, Lamp};
use glam::Vec3;

/// One fitting found in the model: the name of its group, and its lamp at
/// full strength (its light group's number not yet given).
pub(crate) type Fitting = (String, Lamp);

/// The lamp of a fitting whose geometry fills the box `lo` to `hi`
/// (metres, z up) and gives `light`: where the light leaves it and which
/// way follow how it is mounted.
pub(crate) fn lamp_for(lo: Vec3, hi: Vec3, mount: Mount, light: &Light) -> Lamp {
    let mid = (lo + hi) * 0.5;
    let radius = ((hi.x - lo.x).min(hi.y - lo.y) * 0.3).clamp(0.02, 0.12);
    // one hung from the ceiling lights downwards from its underside; a
    // wall bracket or a standing lamp, upwards from its top
    let (pos, axis) = match mount {
        Mount::Ceiling => (Vec3::new(mid.x, mid.y, lo.z - 0.01), -Vec3::Z),
        _ => (Vec3::new(mid.x, mid.y, hi.z + 0.01), Vec3::Z),
    };
    // (the beam's half-angle, where it ends, how much spills everywhere)
    let (half, edge, spill) = match light.beam {
        Spread::Narrow => (25.0, 45.0, 0.0),
        Spread::Medium => (40.0, 65.0, 0.1),
        Spread::Wide => (60.0, 90.0, 0.35),
        Spread::AllRound => (180.0, 181.0, 0.0),
    };
    Lamp {
        pos,
        axis,
        radius,
        candela: profile(half, edge, spill),
        lumens: light.lumens.clamp(0.0, 200_000.0) as f32,
        colour: kelvin((light.kelvin as f32).clamp(1500.0, 12_000.0)),
        group: 1,
    }
}

/// The lamps to render from the fittings found: each group's by its
/// strength (`strength(name)`, 1 as made, 0 off), in the renderer's light
/// groups by name, the third taking every group past the second.
pub(crate) fn lamps_lit(fittings: &[Fitting], strength: impl Fn(&str) -> f32) -> Vec<Lamp> {
    let mut names: Vec<&str> = fittings.iter().map(|f| f.0.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    fittings
        .iter()
        .filter_map(|(name, lamp)| {
            let k = strength(name).clamp(0.0, 8.0);
            (k > 0.0).then(|| {
                let mut l = lamp.clone();
                l.lumens *= k;
                l.group = names.iter().position(|n| n == name).unwrap_or(0).min(2) + 1;
                l
            })
        })
        .collect()
}

impl CreaBimApp {
    /// The model's light fittings, found once for each change of the 3D
    /// view's geometry.
    pub(crate) fn fittings(&self) -> Vec<Fitting> {
        if let Some((g, f)) = self.renders.fittings.borrow().as_ref() {
            if *g == self.geometry_gen {
                return f.clone();
            }
        }
        let model = self.doc.model();
        let mut out = vec![];
        for e in model.iter() {
            if !matches!(e.kind, ElementKind::Object(_)) {
                continue;
            }
            let Some(t) = creabim_core::objects::type_of(model, e) else {
                continue;
            };
            let Some(light) = t.light_given().filter(|l| l.on && l.lumens > 0.0) else {
                continue;
            };
            // where it stands: the box round what the 3D view draws of it
            let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
            for item in self.scene.iter().filter(|i| i.id == e.id && !i.ghost) {
                for p in &item.mesh.positions {
                    lo = lo.min(Vec3::from(*p));
                    hi = hi.max(Vec3::from(*p));
                }
            }
            if lo.x > hi.x {
                continue;
            }
            // its group: the one named on its type, else its type's name
            let name = Some(light.group.trim().to_string())
                .filter(|g| !g.is_empty())
                .or_else(|| creabim_core::types::type_of(model, e).map(|(te, _)| te.name.clone()))
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| "Light fittings".to_string());
            out.push((name, lamp_for(lo, hi, t.mount, &light)));
        }
        *self.renders.fittings.borrow_mut() = Some((self.geometry_gen, out.clone()));
        out
    }

    /// The fittings' groups, in name order, and how many lamps in each.
    pub(crate) fn lamp_types(&self) -> Vec<(String, usize)> {
        let mut by: std::collections::BTreeMap<String, usize> = Default::default();
        for (name, _) in self.fittings() {
            *by.entry(name).or_default() += 1;
        }
        by.into_iter().collect()
    }

    /// The lamps a render is made with now, and a number that changes
    /// when they do (with the geometry's own).
    pub(crate) fn lamps_now(&self) -> (Vec<Lamp>, u64) {
        let set = &self.shell.settings.render_lamps;
        let mut lamps = lamps_lit(&self.fittings(), |n| set.get(n).copied().unwrap_or(1.0));
        // the camera's white balance, on every lamp as on the sky
        let white = self.shell.settings.render_white;
        if (white - 6500.0).abs() >= 1.0 {
            let gain = creabim_render::scene::white_gain(white);
            for l in &mut lamps {
                l.colour *= gain;
            }
        }
        let mut key = (lamps.len() as u64) ^ ((white.to_bits() as u64) << 20);
        for l in &lamps {
            key = key
                .wrapping_mul(1_000_003)
                .wrapping_add(l.lumens.to_bits() as u64 + l.group as u64);
        }
        (lamps, key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fitting_lights_from_where_it_hangs_or_stands_with_its_own_light() {
        let (lo, hi) = (Vec3::new(1.0, 2.0, 2.0), Vec3::new(1.3, 2.3, 2.6));
        // a pendant: from its underside, downwards, wide
        let pendant = Light::typical(Mount::Ceiling, false);
        let p = lamp_for(lo, hi, Mount::Ceiling, &pendant);
        assert!(p.pos.z < 2.0 && p.pos.z > 1.98 && p.axis == -Vec3::Z);
        assert!((p.pos.x - 1.15).abs() < 1e-5 && (p.pos.y - 2.15).abs() < 1e-5);
        assert_eq!(p.lumens, 1100.0);
        // a downlight: narrower than the pendant (more candela on its axis
        // for the same thousand lumens)
        let d = lamp_for(
            lo,
            hi,
            Mount::Ceiling,
            &Light::typical(Mount::Ceiling, true),
        );
        assert!(
            d.candela[0] > 2.0 * p.candela[0],
            "{} {}",
            d.candela[0],
            p.candela[0]
        );
        // a wall bracket: from its top, upwards
        let w = lamp_for(lo, hi, Mount::Wall, &Light::typical(Mount::Wall, true));
        assert!(w.pos.z > 2.6 && w.axis == Vec3::Z);
        // its glowing face is small, whatever the fitting's size
        let big = lamp_for(
            Vec3::ZERO,
            Vec3::new(3.0, 3.0, 1.0),
            Mount::Ceiling,
            &pendant,
        );
        assert!(big.radius <= 0.12 && p.radius >= 0.02);
        // its own output, colour and beam are what the lamp takes
        let own = Light {
            lumens: 2500.0,
            kelvin: 6500.0,
            beam: Spread::AllRound,
            ..pendant.clone()
        };
        let o = lamp_for(lo, hi, Mount::Ceiling, &own);
        assert_eq!(o.lumens, 2500.0);
        // daylight is bluer than a warm bulb
        assert!(o.colour.z / o.colour.x > 1.5 * p.colour.z / p.colour.x);
        // all round: as much straight up as straight down
        assert!((o.candela[0] - o.candela[36]).abs() < 0.01 * o.candela[0]);
    }

    #[test]
    fn a_fitting_gives_its_kinds_usual_light_until_it_has_its_own() {
        let starters = creabim_core::catalogue::starters();
        let lit: Vec<_> = starters
            .iter()
            .filter(|c| c.object.is_light_fitting())
            .collect();
        assert!(lit.len() >= 3, "the starter catalogue's light fittings");
        for c in &lit {
            let l = c.object.light_given().unwrap();
            assert!(l.on && l.lumens >= 500.0, "{}", c.name);
        }
        // a chair gives none, until it is given one
        let mut chair = starters
            .iter()
            .find(|c| !c.object.is_light_fitting())
            .unwrap()
            .object
            .clone();
        assert!(chair.light_given().is_none());
        chair.light = Some(Light::typical(Mount::Floor, false));
        assert_eq!(chair.light_given().unwrap().lumens, 800.0);
        // and one switched off is kept, as off
        let mut off = lit[0].object.clone();
        off.light = Some(Light {
            on: false,
            ..off.light_given().unwrap()
        });
        assert!(!off.light_given().unwrap().on);
        // a type with no light of its own is written as before
        let text = serde_json::to_string(&lit[0].object).unwrap();
        assert!(!text.contains("\"light\""), "{text}");
    }

    #[test]
    fn lamps_are_dimmed_by_type_put_off_at_zero_and_grouped_by_name() {
        let one = |n: &str| {
            (
                n.to_string(),
                lamp_for(
                    Vec3::ZERO,
                    Vec3::ONE,
                    Mount::Ceiling,
                    &Light::typical(Mount::Ceiling, false),
                ),
            )
        };
        let f = vec![
            one("Pendant"),
            one("Downlight"),
            one("Wall light"),
            one("Zed lamp"),
            one("Pendant"),
        ];
        let lit = lamps_lit(&f, |n| match n {
            "Pendant" => 0.5,
            "Wall light" => 0.0,
            _ => 1.0,
        });
        // the wall light is off; the pendants are at half
        assert_eq!(lit.len(), 4);
        assert_eq!(lit[0].lumens, 550.0);
        assert_eq!(lit[1].lumens, 1100.0);
        // groups by name order: Downlight 1, Pendant 2, the rest 3
        assert_eq!(
            lit.iter().map(|l| l.group).collect::<Vec<_>>(),
            vec![2, 1, 3, 2]
        );
        assert!(lit
            .iter()
            .all(|l| (1..creabim_render::scene::SKY).contains(&l.group)));
    }
}
