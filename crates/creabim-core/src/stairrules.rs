//! Stair rule sets and the checks a stair is held to.
//!
//! A rule set is a list of limits from a building code (or a comfortable
//! house stair with no code behind it): riser and going, 2R + G, pitch,
//! width, winders, nosing, landings and the railings' heights and gaps.
//! The four presets are starting points; a project keeps its own, edited
//! and saved by name. Checks advise; they don't certify. Each value needs
//! checking against the edition the project works to.

use crate::members::{self, RailInfill, Stair};
use crate::{ElementId, ElementKind, Model};
use glam::DVec2;
use serde::{Deserialize, Serialize};

/// Which rule set a stair is checked against.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StairCode {
    /// A comfortable house stair, no code behind it.
    Comfort,
    /// Ontario Building Code 2024, a stair in a dwelling (private stair).
    #[default]
    Ontario,
    /// US International Residential Code, a dwelling.
    Us,
    /// UK Approved Document K, a private stair.
    Uk,
    /// One of the project's own rule sets, by name.
    Own(String),
}

impl StairCode {
    pub const PRESETS: [StairCode; 4] = [
        StairCode::Comfort,
        StairCode::Ontario,
        StairCode::Us,
        StairCode::Uk,
    ];

    pub fn is_default(&self) -> bool {
        *self == StairCode::default()
    }

    /// Its name in a JSON model: the preset's key or the own set's name.
    pub fn key(&self) -> String {
        match self {
            StairCode::Comfort => "comfort".into(),
            StairCode::Ontario => "ontario".into(),
            StairCode::Us => "us".into(),
            StairCode::Uk => "uk".into(),
            StairCode::Own(n) => n.clone(),
        }
    }

    pub fn from_key(k: &str) -> Self {
        match k {
            "comfort" => StairCode::Comfort,
            "ontario" | "" => StairCode::Ontario,
            "us" => StairCode::Us,
            "uk" => StairCode::Uk,
            n => StairCode::Own(n.into()),
        }
    }

    pub fn label(&self) -> String {
        match self {
            StairCode::Own(n) => n.clone(),
            c => c.preset().map(|r| r.name).unwrap_or_default(),
        }
    }

    /// The preset's values; none for a project's own.
    pub fn preset(&self) -> Option<StairRules> {
        let mut r = StairRules {
            name: String::new(),
            riser_min: None,
            riser_max: None,
            going_min: None,
            going_max: None,
            step_min: None,
            step_max: None,
            pitch_max: None,
            flight_risers_max: None,
            flight_rise_max: None,
            width_min: None,
            headroom: 2.0,
            landing_cap: None,
            nosing_max: None,
            winder_angle: None,
            winder_at: 0.27,
            winder_going_min: None,
            narrow_min: None,
            spiral_riser_max: None,
            spiral_going_min: None,
            handrail_min: 0.865,
            handrail_max: 1.0,
            guard_min: 0.9,
            sphere: 0.1,
            both_sides_from: None,
            ramp_slope_max: Some(1.0 / 12.0),
            ramp_flight_rise_max: Some(0.75),
            ramp_width_min: Some(0.9),
            ramp_landing_min: Some(1.5),
            ramp_both_sides: true,
        };
        match self {
            StairCode::Comfort => {
                r.name = "Comfortable house stair (no code)".into();
                r.riser_min = Some(0.15);
                r.riser_max = Some(0.18);
                r.going_min = Some(0.27);
                r.going_max = Some(0.30);
                r.step_min = Some(0.60);
                r.step_max = Some(0.65);
                r.pitch_max = Some(37.0);
                r.flight_risers_max = Some(16);
                r.width_min = Some(0.9);
                r.narrow_min = Some(0.05);
                r.winder_going_min = Some(0.25);
                r.guard_min = 1.07;
                r.ramp_width_min = Some(1.0);
            }
            StairCode::Ontario => {
                r.name = "Ontario OBC 2024, dwelling".into();
                r.riser_min = Some(0.125);
                r.riser_max = Some(0.20);
                r.going_min = Some(0.255);
                r.going_max = Some(0.355);
                r.width_min = Some(0.86);
                r.headroom = 1.95;
                r.landing_cap = Some(1.1);
                r.nosing_max = Some(0.025);
                r.winder_angle = Some(30.0);
                r.winder_at = 0.3;
                r.spiral_riser_max = Some(0.24);
                r.spiral_going_min = Some(0.19);
                r.handrail_max = 1.07;
                r.both_sides_from = Some(1.1);
            }
            StairCode::Us => {
                r.name = "US IRC, dwelling".into();
                r.riser_max = Some(0.196);
                r.going_min = Some(0.254);
                r.flight_rise_max = Some(3.835);
                r.width_min = Some(0.914);
                r.headroom = 2.032;
                r.nosing_max = Some(0.032);
                r.winder_at = 0.305;
                r.winder_going_min = Some(0.254);
                r.narrow_min = Some(0.152);
                r.spiral_riser_max = Some(0.241);
                r.spiral_going_min = Some(0.171);
                r.handrail_min = 0.864;
                r.handrail_max = 0.965;
                r.guard_min = 0.864;
                r.sphere = 0.111;
                r.ramp_flight_rise_max = Some(0.76);
                r.ramp_width_min = Some(0.915);
                r.ramp_landing_min = Some(1.525);
            }
            StairCode::Uk => {
                r.name = "UK Part K, private stair".into();
                r.riser_min = Some(0.15);
                r.riser_max = Some(0.22);
                r.going_min = Some(0.22);
                r.going_max = Some(0.30);
                r.step_min = Some(0.55);
                r.step_max = Some(0.70);
                r.pitch_max = Some(42.0);
                r.flight_risers_max = Some(16);
                r.narrow_min = Some(0.05);
                r.winder_going_min = Some(0.22);
                r.handrail_min = 0.9;
                r.guard_min = 0.9;
                r.both_sides_from = Some(1.0);
                r.ramp_flight_rise_max = Some(0.5);
                r.ramp_width_min = Some(1.5);
            }
            StairCode::Own(_) => return None,
        }
        Some(r)
    }

    /// The values it stands for in `model`: a preset's, or the project's
    /// own set of that name (the Ontario preset when it has gone).
    pub fn rules(&self, model: &Model) -> StairRules {
        match self {
            StairCode::Own(n) => own_rules(model)
                .into_iter()
                .find(|r| &r.name == n)
                .unwrap_or_else(|| StairCode::Ontario.preset().unwrap()),
            c => c.preset().unwrap(),
        }
    }
}

/// The limits a stair is checked against, in metres and degrees. A limit
/// left out isn't checked.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StairRules {
    pub name: String,
    pub riser_min: Option<f64>,
    pub riser_max: Option<f64>,
    /// The going (run) of a rectangular tread.
    pub going_min: Option<f64>,
    pub going_max: Option<f64>,
    /// Twice the riser plus the going.
    pub step_min: Option<f64>,
    pub step_max: Option<f64>,
    pub pitch_max: Option<f64>,
    pub flight_risers_max: Option<u32>,
    /// The rise of one flight, floor or landing to the next.
    pub flight_rise_max: Option<f64>,
    pub width_min: Option<f64>,
    /// Clear height over the pitch line (checked in part 6b).
    pub headroom: f64,
    /// A landing is at least as deep as the stair is wide, up to this.
    pub landing_cap: Option<f64>,
    pub nosing_max: Option<f64>,
    /// Each winder turns this many degrees.
    pub winder_angle: Option<f64>,
    /// How far in from the inside edge a winder's going is measured.
    pub winder_at: f64,
    pub winder_going_min: Option<f64>,
    /// The going at a winder's narrow end.
    pub narrow_min: Option<f64>,
    /// A spiral stair's own limits: its riser, and its going `winder_at`
    /// in from the inside edge. None: held to the usual riser and going.
    #[serde(default)]
    pub spiral_riser_max: Option<f64>,
    #[serde(default)]
    pub spiral_going_min: Option<f64>,
    /// Handrail height over the nosings.
    pub handrail_min: f64,
    pub handrail_max: f64,
    /// A guard's height over the nosings.
    pub guard_min: f64,
    /// The sphere no gap in a guard may let through.
    pub sphere: f64,
    /// Handrails on both sides once the stair is this wide.
    pub both_sides_from: Option<f64>,
    /// A ramp's steepest slope, rise over run (1:12 is 0.0833).
    #[serde(default)]
    pub ramp_slope_max: Option<f64>,
    /// How far one flight of a ramp may climb before a landing.
    #[serde(default)]
    pub ramp_flight_rise_max: Option<f64>,
    /// A ramp's clear width.
    #[serde(default)]
    pub ramp_width_min: Option<f64>,
    /// How long a ramp's landing is, along the way.
    #[serde(default)]
    pub ramp_landing_min: Option<f64>,
    /// A ramp has handrails on both sides.
    #[serde(default)]
    pub ramp_both_sides: bool,
}

/// The project's own rule sets.
pub fn own_rules(model: &Model) -> Vec<StairRules> {
    model
        .project_info()
        .map(|(_, p)| p.stair_rules.clone())
        .unwrap_or_default()
}

/// Write the project's own rule sets (one undo step).
pub fn set_own_rules(model: &Model, rules: Vec<StairRules>, label: &str) -> crate::Transaction {
    let mut info = model
        .project_info()
        .map(|(_, p)| p.clone())
        .unwrap_or_default();
    info.stair_rules = rules;
    let mut tx = crate::project::set_project(model, info);
    tx.label = label.into();
    tx
}

/// How a check came out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Passes,
    Fails,
    /// Not in this rule set, or nothing to check.
    NotChecked,
}

/// One line of a stair's check list.
#[derive(Clone, Debug, PartialEq)]
pub struct StairCheck {
    pub what: &'static str,
    /// What the stair has, as text in millimetres or degrees.
    pub value: String,
    /// The limit, as text.
    pub limit: String,
    pub verdict: Verdict,
}

fn mm(v: f64) -> String {
    format!("{:.0}", v * 1000.0)
}

fn range(lo: Option<f64>, hi: Option<f64>, f: fn(f64) -> String) -> String {
    match (lo, hi) {
        (Some(a), Some(b)) => format!("{} to {}", f(a), f(b)),
        (Some(a), None) => format!("at least {}", f(a)),
        (None, Some(b)) => format!("at most {}", f(b)),
        (None, None) => "not in this rule set".into(),
    }
}

fn within(v: f64, lo: Option<f64>, hi: Option<f64>) -> Verdict {
    if lo.is_none() && hi.is_none() {
        return Verdict::NotChecked;
    }
    let ok = lo.is_none_or(|a| v >= a - 5e-4) && hi.is_none_or(|b| v <= b + 5e-4);
    if ok {
        Verdict::Passes
    } else {
        Verdict::Fails
    }
}

/// How long a stretch of `path` runs inside `poly` (sampled every 2 mm).
fn length_inside(path: &[DVec2], poly: &[DVec2]) -> f64 {
    let mut total = 0.0;
    for w in path.windows(2) {
        let len = w[0].distance(w[1]);
        let n = (len / 0.002).ceil().max(1.0) as usize;
        let step = len / n as f64;
        for k in 0..n {
            let p = w[0].lerp(w[1], (k as f64 + 0.5) / n as f64);
            if crate::polygon::contains(poly, p) {
                total += step;
            }
        }
    }
    total
}

/// The narrowest a polygon is, across any of its edges' directions.
fn breadth(poly: &[DVec2]) -> f64 {
    let n = poly.len();
    (0..n)
        .filter_map(|i| {
            let e = poly[(i + 1) % n] - poly[i];
            (e.length() > 1e-6).then(|| {
                let a = e.perp().normalize();
                let (lo, hi) = poly.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| {
                    (lo.min(p.dot(a)), hi.max(p.dot(a)))
                });
                hi - lo
            })
        })
        .fold(f64::MAX, f64::min)
}

/// A winder's going `at` in from the inside edge of the stair, and its
/// going at the narrow end (10 mm in).
fn winder_goings(walk: &[DVec2], half: f64, outline: &[DVec2], at: f64) -> (f64, f64) {
    let going = |d: f64| {
        [1.0, -1.0]
            .into_iter()
            .map(|s| {
                let by = s * (half - d).max(0.0);
                let line = if by == 0.0 {
                    walk.to_vec()
                } else {
                    crate::offset::offset_path(walk, by)
                };
                length_inside(&line, outline)
            })
            .filter(|l| *l > 1e-6)
            .fold(f64::MAX, f64::min)
    };
    let g = going(at.min(half));
    let n = going(0.01);
    (
        if g == f64::MAX { 0.0 } else { g },
        if n == f64::MAX { 0.0 } else { n },
    )
}

/// The stair `id`'s checks against its rule set.
pub fn check_stair(model: &Model, id: ElementId) -> Vec<StairCheck> {
    let Some(ElementKind::Stair(s)) = model.get(id).map(|e| &e.kind) else {
        return vec![];
    };
    let rules = s.rules.rules(model);
    check_with(model, id, s, &rules)
}

/// The checks of stair `s` (element `id`) against `rules`.
pub fn check_with(model: &Model, id: ElementId, s: &Stair, rules: &StairRules) -> Vec<StairCheck> {
    let rise = s.rise(model);
    let lay = s.layout(rise);
    let r = lay.riser;
    let g = s.tread;
    let mut out = vec![];
    let mut push = |what, value: String, limit: String, verdict| {
        out.push(StairCheck {
            what,
            value,
            limit,
            verdict,
        })
    };
    if s.is_ramp() {
        ramp_checks(s, &lay, rules, &mut push);
    } else {
        // a spiral is held to its own riser and going, where the rule set
        // has them: the going measured in from the inside edge
        let spiral = s.ifc_type(rise) == "SPIRAL_STAIR"
            && (rules.spiral_riser_max.is_some() || rules.spiral_going_min.is_some());
        if spiral {
            push(
                "Riser (spiral)",
                mm(r),
                range(None, rules.spiral_riser_max, mm),
                within(r, None, rules.spiral_riser_max),
            );
            let walk = s.walk_in_plan(rise);
            let inner = s
                .centre()
                .zip(walk.first())
                .map_or(0.0, |(c, p)| (c.distance(*p) - s.width / 2.0).max(0.0));
            let at = rules.winder_at.min(s.width);
            let going = g * (inner + at) / (inner + s.width / 2.0).max(1e-6);
            push(
                "Going (spiral)",
                mm(going),
                match rules.spiral_going_min {
                    Some(v) => format!("at least {} at {} from the inside", mm(v), mm(at)),
                    None => "not in this rule set".into(),
                },
                within(going, rules.spiral_going_min, None),
            );
        } else {
            push(
                "Riser",
                mm(r),
                range(rules.riser_min, rules.riser_max, mm),
                within(r, rules.riser_min, rules.riser_max),
            );
            let has_flight_going = lay.flights.iter().any(|f| f.risers > 1);
            push(
                "Going",
                mm(g),
                range(rules.going_min, rules.going_max, mm),
                if has_flight_going {
                    within(g, rules.going_min, rules.going_max)
                } else {
                    Verdict::NotChecked
                },
            );
        }
        let step = 2.0 * r + g;
        push(
            "2R + G",
            mm(step),
            range(rules.step_min, rules.step_max, mm),
            within(step, rules.step_min, rules.step_max),
        );
        let pitch = r.atan2(g).to_degrees();
        let deg = |v: f64| format!("{v:.0}°");
        push(
            "Pitch",
            format!("{pitch:.1}°"),
            range(None, rules.pitch_max, deg),
            within(pitch, None, rules.pitch_max),
        );
        // flights: from a floor or landing to the next
        let most = lay.flights.iter().map(|f| f.risers).max().unwrap_or(0);
        let count = |v: f64| format!("{v:.0}");
        push(
            "Risers in a flight",
            most.to_string(),
            range(None, rules.flight_risers_max.map(|v| v as f64), count),
            within(most as f64, None, rules.flight_risers_max.map(|v| v as f64)),
        );
        let tallest = most as f64 * r;
        push(
            "Rise of a flight",
            mm(tallest),
            range(None, rules.flight_rise_max, mm),
            within(tallest, None, rules.flight_rise_max),
        );
        push(
            "Width",
            mm(s.width),
            range(rules.width_min, None, mm),
            within(s.width, rules.width_min, None),
        );
        // landings: as deep as the stair is wide, up to the cap
        if !lay.landings.is_empty() {
            let shallowest = lay
                .landings
                .iter()
                .map(|(p, _)| breadth(p))
                .fold(f64::MAX, f64::min);
            let need = rules.landing_cap.map_or(s.width, |c| s.width.min(c));
            push(
                "Landing depth",
                mm(shallowest),
                format!("at least {}", mm(need)),
                within(shallowest, Some(need), None),
            );
        }
        // winders
        if !lay.winders.is_empty() {
            let walk = s.walk_in_plan(rise);
            let half = s.width / 2.0;
            let (mut going, mut narrow) = (f64::MAX, f64::MAX);
            for w in &lay.winders {
                let (a, b) = winder_goings(&walk, half, &w.outline, rules.winder_at);
                going = going.min(a);
                narrow = narrow.min(b);
            }
            if let Some(a) = rules.winder_angle {
                // each set of winders shares its turn
                let mut turns: Vec<usize> = lay.winders.iter().map(|w| w.turn).collect();
                turns.dedup();
                let each: Vec<f64> = turns
                    .iter()
                    .map(|&t| {
                        turn_at(&walk, t)
                            / lay.winders.iter().filter(|w| w.turn == t).count() as f64
                    })
                    .collect();
                let worst = each.iter().map(|e| (e - a).abs()).fold(0.0, f64::max);
                let each = each.iter().copied().fold(0.0, f64::max);
                push(
                    "Winder turn",
                    format!("{each:.0}° each"),
                    format!("{a:.0}° each"),
                    if worst <= 1.0 {
                        Verdict::Passes
                    } else {
                        Verdict::Fails
                    },
                );
            }
            push(
                "Winder going",
                mm(going),
                match rules.winder_going_min {
                    Some(v) => format!(
                        "at least {} at {} from the inside",
                        mm(v),
                        mm(rules.winder_at)
                    ),
                    None => "not in this rule set".into(),
                },
                within(going, rules.winder_going_min, None),
            );
            push(
                "Winder narrow end",
                mm(narrow),
                range(rules.narrow_min, None, mm),
                within(narrow, rules.narrow_min, None),
            );
        }
        if s.finish.nosing > 0.0 || rules.nosing_max.is_some() {
            push(
                "Nosing",
                mm(s.finish.nosing),
                range(None, rules.nosing_max, mm),
                within(s.finish.nosing, None, rules.nosing_max),
            );
        }
    }
    let head = crate::stairwell::headroom(model, s, rules.headroom);
    push(
        "Headroom",
        if head.least.is_finite() {
            mm(head.least)
        } else {
            "nothing over it".into()
        },
        format!("at least {}", mm(rules.headroom)),
        within(head.least, Some(rules.headroom), None),
    );
    push(
        "Reaches the floor",
        if lay.short == 0 {
            "yes".into()
        } else {
            format!("{} treads short", lay.short)
        },
        String::new(),
        if lay.short == 0 {
            Verdict::Passes
        } else {
            Verdict::Fails
        },
    );
    // its railings
    let rails: Vec<&members::Railing> = model
        .iter()
        .filter_map(|e| match &e.kind {
            ElementKind::Railing(r) if r.stair.is_some_and(|x| x.stair == id) => Some(r),
            _ => None,
        })
        .collect();
    let sides = |left: bool| {
        rails
            .iter()
            .any(|r| r.stair.is_some_and(|x| x.left == left))
    };
    let n = sides(true) as u32 + sides(false) as u32;
    let both_from = if s.is_ramp() && rules.ramp_both_sides {
        Some(0.0)
    } else {
        rules.both_sides_from
    };
    push(
        "Handrails",
        match n {
            0 => "none".into(),
            1 => "one side".into(),
            _ => "both sides".into(),
        },
        match both_from {
            Some(w) if w <= 0.0 => "both sides on a ramp".into(),
            Some(w) => format!("both sides at {} wide or more", mm(w)),
            None => "at least one".into(),
        },
        if n == 0 || both_from.is_some_and(|w| s.width >= w - 5e-4) && n < 2 {
            Verdict::Fails
        } else {
            Verdict::Passes
        },
    );
    if let Some(lo) = rails.iter().map(|r| r.height).reduce(f64::min) {
        let hi = rails.iter().map(|r| r.height).fold(lo, f64::max);
        push(
            "Handrail height",
            if (hi - lo).abs() < 5e-4 {
                mm(lo)
            } else {
                format!("{} to {}", mm(lo), mm(hi))
            },
            range(Some(rules.handrail_min), Some(rules.handrail_max), mm),
            if within(lo, Some(rules.handrail_min), None) == Verdict::Passes
                && within(hi, None, Some(rules.handrail_max)) == Verdict::Passes
            {
                Verdict::Passes
            } else {
                Verdict::Fails
            },
        );
    }
    let guards: Vec<&&members::Railing> = rails.iter().filter(|r| !r.on_wall).collect();
    if let Some(low) = guards.iter().map(|r| r.top_height()).reduce(f64::min) {
        push(
            "Guard height",
            mm(low),
            format!("at least {}", mm(rules.guard_min)),
            within(low, Some(rules.guard_min), None),
        );
        let open = guards.iter().any(|r| r.infill == RailInfill::Rail);
        push(
            "Guard openings",
            if open {
                "a bottom rail only".into()
            } else {
                format!("gaps under {}", mm(members::BALUSTER_GAP))
            },
            format!("no {} sphere through", mm(rules.sphere)),
            if open || members::BALUSTER_GAP > rules.sphere {
                Verdict::Fails
            } else {
                Verdict::Passes
            },
        );
    }
    if !rails.is_empty() {
        let bad = rails.iter().find(|r| {
            let outline = r
                .rail_profile
                .as_deref()
                .and_then(|n| crate::profiles::find(model, n));
            !members::hand_grip(outline.as_deref()).ok
        });
        push(
            "Grip",
            match bad {
                Some(r) => format!(
                    "{} is hard to grip",
                    r.rail_profile.as_deref().unwrap_or("the square bar")
                ),
                None => "every rail".into(),
            },
            "round 30 to 43, or 100 to 125 round".into(),
            if bad.is_some() {
                Verdict::Fails
            } else {
                Verdict::Passes
            },
        );
    }
    out
}

/// A ramp's own checks: its slope, how far each flight climbs, its width
/// and its landings' length.
fn ramp_checks(
    s: &Stair,
    lay: &crate::members::StairLayout,
    rules: &StairRules,
    push: &mut dyn FnMut(&'static str, String, String, Verdict),
) {
    let slope = lay.riser / s.tread.max(1e-9);
    let ratio = |v: f64| format!("1:{:.1} ({:.1}%)", 1.0 / v.max(1e-9), v * 100.0);
    push(
        "Slope",
        ratio(slope),
        match rules.ramp_slope_max {
            Some(m) => format!("at most {}", ratio(m)),
            None => "not in this rule set".into(),
        },
        within(slope, None, rules.ramp_slope_max.map(|m| m + 1e-6)),
    );
    let most = lay
        .flights
        .iter()
        .map(|f| f.risers as f64 * lay.riser)
        .fold(0.0, f64::max);
    push(
        "Rise of a flight",
        mm(most),
        range(None, rules.ramp_flight_rise_max, mm),
        within(most, None, rules.ramp_flight_rise_max),
    );
    push(
        "Width",
        mm(s.width),
        range(rules.ramp_width_min, None, mm),
        within(s.width, rules.ramp_width_min, None),
    );
    if !lay.landings.is_empty() {
        let shortest = lay
            .landings
            .iter()
            .map(|(p, _)| breadth(p))
            .fold(f64::MAX, f64::min);
        push(
            "Landing length",
            mm(shortest),
            range(rules.ramp_landing_min, None, mm),
            within(shortest, rules.ramp_landing_min, None),
        );
    }
}

/// How far the walk line turns at turn `t` (between straight `t` and the
/// next), degrees.
fn turn_at(walk: &[DVec2], t: usize) -> f64 {
    let (Some(a), Some(b), Some(c)) = (walk.get(t), walk.get(t + 1), walk.get(t + 2)) else {
        return 90.0;
    };
    let (d1, d2) = ((*b - *a).normalize_or_zero(), (*c - *b).normalize_or_zero());
    d1.dot(d2).clamp(-1.0, 1.0).acos().to_degrees()
}

/// The stairs in `model` failing a check, each with the checks it fails.
pub fn failing_stairs(model: &Model) -> Vec<(ElementId, Vec<StairCheck>)> {
    model
        .iter()
        .filter(|e| matches!(e.kind, ElementKind::Stair(_)))
        .filter_map(|e| {
            let bad: Vec<StairCheck> = check_stair(model, e.id)
                .into_iter()
                .filter(|c| c.verdict == Verdict::Fails)
                .collect();
            (!bad.is_empty()).then_some((e.id, bad))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::members::{Railing, StairLine, StairSide, TurnKind};
    use crate::{Element, Level, Transaction};

    /// A model with a 2.8 m storey and the stair drawn by `pts`; the
    /// stair's id.
    fn stair(pts: &[(f64, f64)], edit: impl FnOnce(&mut Stair)) -> (Model, ElementId) {
        let mut m = Model::new();
        let l = Element::new(
            "Ground",
            ElementKind::Level(Level {
                elevation: 0.0,
                height: 2.8,
            }),
        );
        let pts: Vec<DVec2> = pts.iter().map(|p| DVec2::new(p.0, p.1)).collect();
        let mut s = Stair::from_line(l.id, &pts, StairLine::Walk).unwrap();
        edit(&mut s);
        let e = Element::new("Stair", ElementKind::Stair(s));
        let id = e.id;
        Transaction::new("t")
            .insert(l)
            .insert(e)
            .apply(&mut m)
            .unwrap();
        (m, id)
    }

    fn verdict(m: &Model, id: ElementId, what: &str) -> Verdict {
        check_stair(m, id)
            .into_iter()
            .find(|c| c.what == what)
            .unwrap_or_else(|| panic!("no {what} check"))
            .verdict
    }

    #[test]
    fn a_house_stair_passes_ontario_and_a_shallow_going_fails() {
        let (m, id) = stair(&[(0.0, 0.0), (6.0, 0.0)], |_| {});
        // 16 risers of 175, goings of 280
        for c in check_stair(&m, id) {
            // no railings drawn yet: the only fail
            assert_eq!(c.verdict == Verdict::Fails, c.what == "Handrails", "{c:?}");
        }
        let (m, id) = stair(&[(0.0, 0.0), (6.0, 0.0)], |s| s.tread = 0.24);
        assert_eq!(verdict(&m, id, "Going"), Verdict::Fails);
        // the comfort set holds 2R + G too: 2 × 175 + 240 = 590 is short
        let (m, id) = stair(&[(0.0, 0.0), (6.0, 0.0)], |s| {
            s.tread = 0.24;
            s.rules = StairCode::Comfort;
        });
        assert_eq!(verdict(&m, id, "2R + G"), Verdict::Fails);
    }

    #[test]
    fn a_ramp_is_checked_for_slope_rise_and_width_instead_of_steps() {
        // 0.6 m up at 1:12 is 7.2 m along
        let (m, id) = stair(&[(0.0, 0.0), (7.2, 0.0)], |s| {
            s.height = Some(0.6);
            s.width = 1.0;
            s.make_ramp(1.0 / 12.0);
        });
        let c = check_stair(&m, id);
        assert!(
            c.iter().all(|c| c.what != "Riser" && c.what != "Going"),
            "{c:?}"
        );
        assert_eq!(verdict(&m, id, "Slope"), Verdict::Passes);
        assert_eq!(verdict(&m, id, "Rise of a flight"), Verdict::Passes);
        assert_eq!(verdict(&m, id, "Width"), Verdict::Passes);
        // steeper than 1:12 fails, and so does a whole storey in one go
        let (m, id) = stair(&[(0.0, 0.0), (7.2, 0.0)], |s| {
            s.height = Some(0.6);
            s.make_ramp(1.0 / 8.0);
        });
        assert_eq!(verdict(&m, id, "Slope"), Verdict::Fails);
        let (m, id) = stair(&[(0.0, 0.0), (34.0, 0.0)], |s| s.make_ramp(1.0 / 12.0));
        assert_eq!(verdict(&m, id, "Rise of a flight"), Verdict::Fails);
        // a ramp's railings go on both sides whatever its width
        let ramp = |m: &Model| match &m.get(id).unwrap().kind {
            ElementKind::Stair(s) => s.is_ramp(),
            _ => false,
        };
        assert!(ramp(&m));
        assert_eq!(verdict(&m, id, "Handrails"), Verdict::Fails);
    }

    #[test]
    fn winders_turn_thirty_degrees_each_but_come_to_a_point() {
        let pts = [(0.0, 3.0), (3.31, 3.0), (3.31, -1.0)];
        let (m, id) = stair(&pts, |s| s.at_turns = TurnKind::Winders(0));
        assert_eq!(verdict(&m, id, "Winder turn"), Verdict::Passes);
        let c = check_stair(&m, id);
        let narrow = c.iter().find(|c| c.what == "Winder narrow end").unwrap();
        // fanning from the inside corner: next to nothing at the narrow end
        assert!(narrow.value.parse::<f64>().unwrap() < 20.0, "{narrow:?}");
        // which the UK set doesn't allow; it wants 50
        let (m, id) = stair(&pts, |s| {
            s.at_turns = TurnKind::Winders(0);
            s.rules = StairCode::Uk;
        });
        assert_eq!(verdict(&m, id, "Winder narrow end"), Verdict::Fails);
        // on the walk line, a winder's going is about a third of the
        // quarter circle there
        let g = check_stair(&m, id)
            .into_iter()
            .find(|c| c.what == "Winder going")
            .unwrap();
        assert!(g.value.parse::<f64>().unwrap() > 150.0, "{g:?}");
    }

    #[test]
    fn a_guard_with_only_a_bottom_rail_leaves_openings() {
        let (mut m, id) = stair(&[(0.0, 0.0), (6.0, 0.0)], |_| {});
        let level = m
            .iter()
            .find(|e| matches!(e.kind, ElementKind::Level(_)))
            .unwrap()
            .id;
        let mut r = Railing::new(level, vec![]);
        r.stair = Some(StairSide {
            stair: id,
            left: true,
        });
        let e = Element::new("Railing", ElementKind::Railing(r.clone()));
        let rid = e.id;
        Transaction::new("r").insert(e).apply(&mut m).unwrap();
        assert_eq!(verdict(&m, id, "Guard openings"), Verdict::Fails);
        // one side only on a 1 m stair is fine in Ontario
        assert_eq!(verdict(&m, id, "Handrails"), Verdict::Passes);
        // balusters close the gaps
        let mut after = m.get(rid).unwrap().clone();
        if let ElementKind::Railing(r) = &mut after.kind {
            r.infill = RailInfill::Balusters;
        }
        let before = m.get(rid).unwrap().clone();
        Transaction::new("b")
            .replace(before, after)
            .apply(&mut m)
            .unwrap();
        assert_eq!(verdict(&m, id, "Guard openings"), Verdict::Passes);
    }

    #[test]
    fn a_projects_own_rule_set_is_found_by_name() {
        let (mut m, id) = stair(&[(0.0, 0.0), (6.0, 0.0)], |s| s.tread = 0.24);
        let mut own = StairCode::Ontario.preset().unwrap();
        own.name = "Office stair".into();
        own.going_min = Some(0.22);
        set_own_rules(&m, vec![own], "own").apply(&mut m).unwrap();
        let mut after = m.get(id).unwrap().clone();
        if let ElementKind::Stair(s) = &mut after.kind {
            s.rules = StairCode::Own("Office stair".into());
        }
        let before = m.get(id).unwrap().clone();
        Transaction::new("s")
            .replace(before, after)
            .apply(&mut m)
            .unwrap();
        assert_eq!(verdict(&m, id, "Going"), Verdict::Passes);
        let fails = failing_stairs(&m);
        assert_eq!(fails.len(), 1);
        assert!(fails[0].1.iter().all(|c| c.what == "Handrails"));
    }
}
