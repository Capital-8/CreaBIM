//! Probes from the stage 0 walk of the site tools (2026-11-16): what the
//! ground does with a handful of spot heights. Opt-in, they print.
//!
//! ```text
//! cargo test --profile fast -p creabim-geom --test site_probe -- --ignored --nocapture
//! ```

use creabim_core::{Element, ElementKind, Level, Site, SiteForm};
use glam::{DVec2, DVec3};

fn ground(survey: bool, faceted: bool) -> Site {
    let lv = Element::new(
        "Ground",
        ElementKind::Level(Level {
            elevation: 0.0,
            height: 3.0,
        }),
    );
    let mut t = Site::terrain(
        lv.id,
        creabim_core::polygon::rectangle(DVec2::ZERO, DVec2::new(25.0, 32.0)),
        0.0,
        0.5,
    );
    if let SiteForm::Terrain { points, data, .. } = &mut t.form {
        // as clicked in the walk: two low in the south, two high in the north
        points.push(DVec3::new(2.5, 3.0, 0.5));
        points.push(DVec3::new(22.5, 3.0, 0.5));
        points.push(DVec3::new(2.5, 30.0, 2.0));
        points.push(DVec3::new(22.5, 30.0, 2.0));
        data.survey = survey;
        data.faceted = faceted;
    }
    t
}

fn range(t: &Site) -> (f64, f64, f64) {
    let g = creabim_geom::ground::of(t).expect("a ground");
    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    for i in 0..=50 {
        for j in 0..=64 {
            if let Some(z) = g.height(DVec2::new(i as f64 * 0.5, j as f64 * 0.5)) {
                lo = lo.min(z);
                hi = hi.max(z);
            }
        }
    }
    let mid = g.height(DVec2::new(12.5, 16.0)).unwrap_or(f64::NAN);
    (lo, hi, mid)
}

#[test]
#[ignore = "a probe, run by hand"]
fn four_spot_heights() {
    for (survey, faceted) in [(false, false), (false, true), (true, false), (true, true)] {
        let (lo, hi, mid) = range(&ground(survey, faceted));
        println!(
            "edge follows data {survey:5} | faceted {faceted:5} | lowest {lo:6.2} highest {hi:6.2} middle {mid:6.2}  (spots 0.50 to 2.00)"
        );
    }
}
