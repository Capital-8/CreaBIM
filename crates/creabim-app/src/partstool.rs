//! The Extrusion, Sweep and Void tools, and the site tools (Terrain, Hard
//! landscape, Planting): their settings for new elements, what a click or
//! a drawn outline makes, and the Properties panel for every element they
//! make (see `creabim_core::parts` and `creabim_core::site`).
//!
//! The outline tools draw with the Slab tool's modes (polygon, rectangle,
//! fill room, from sketch). Sizes, patterns and profiles are only starting
//! values: every one is the element's own afterwards.

use crate::planview::{PlanState, Tool};
use crate::selection::Selection;
use creabim_core::parts::{self, Form, Part, Path, WallEdge};
use creabim_core::site::{GroundLineKind, Pattern, PlantForm, Site, SiteForm};
use creabim_core::tree::{
    Branching, ClumpForm, CrownShape, LeafHabit, TreeCut, TreeDetail, TreeElevation, TreePlan,
    TreePreset, TreeShape,
};
use creabim_core::{Document, Element, ElementId, ElementKind, Model, ModelError, Transaction};
use eframe::egui;
use glam::DVec2;

/// A profile the Sweep tool starts from. The sizes are the tool bar's two
/// numbers: what they mean depends on the profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SweepProfile {
    /// Over a wall's top, proud of both faces.
    Coping,
    /// Along a wall's base, proud of its exterior face.
    Plinth,
    /// A plain rectangle: a kerb, a bar, a rail.
    Bar,
    /// A flight of steps, climbing to the path's left.
    Steps,
}

impl SweepProfile {
    pub const ALL: [SweepProfile; 4] = [
        SweepProfile::Coping,
        SweepProfile::Plinth,
        SweepProfile::Bar,
        SweepProfile::Steps,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SweepProfile::Coping => "Coping",
            SweepProfile::Plinth => "Plinth",
            SweepProfile::Bar => "Bar",
            SweepProfile::Steps => "Steps",
        }
    }

    /// The labels of the tool bar's two sizes.
    fn sizes(self) -> (&'static str, &'static str) {
        match self {
            SweepProfile::Coping => ("Overhang", "Depth"),
            SweepProfile::Plinth => ("Proud", "Height"),
            SweepProfile::Bar => ("Width", "Height"),
            SweepProfile::Steps => ("Going", "Rise"),
        }
    }

    /// Its usual sizes.
    fn defaults(self) -> (f64, f64) {
        match self {
            SweepProfile::Coping => (0.04, 0.06),
            SweepProfile::Plinth => (0.03, 0.3),
            SweepProfile::Bar => (0.15, 0.15),
            SweepProfile::Steps => (0.3, 0.17),
        }
    }

    /// The wall edge it follows when placed on a wall.
    fn edge(self) -> WallEdge {
        match self {
            SweepProfile::Plinth | SweepProfile::Steps => WallEdge::Base,
            SweepProfile::Coping | SweepProfile::Bar => WallEdge::Top,
        }
    }

    /// The profile, for a wall `wall` thick.
    fn profile(self, wall: f64, a: f64, b: f64, steps: u32) -> Vec<DVec2> {
        match self {
            SweepProfile::Coping => parts::coping(wall, a, b),
            SweepProfile::Plinth => parts::plinth(wall, a, b, false),
            SweepProfile::Bar => parts::rectangle(a, b),
            SweepProfile::Steps => parts::steps(steps, b, a),
        }
    }
}

/// A hard landscape area the tool starts from: its pattern, depth and
/// material from the library.
pub struct AreaPreset {
    pub label: &'static str,
    pub pattern: Pattern,
    pub thickness: f64,
    pub material: &'static str,
    /// It lays parking bays in itself: a car park.
    pub parking: bool,
    /// Its icon in the Site menu.
    pub icon: &'static str,
}

pub const AREAS: &[AreaPreset] = &[
    AreaPreset {
        label: "Paving",
        pattern: Pattern::Units {
            size: DVec2::new(0.6, 0.4),
            stagger: true,
            angle: 0.0,
        },
        thickness: 0.1,
        material: "Paving, concrete flags",
        parking: false,
        icon: "area-paving",
    },
    AreaPreset {
        label: "Driveway",
        pattern: Pattern::Units {
            size: DVec2::new(0.2, 0.1),
            stagger: true,
            angle: 90.0,
        },
        thickness: 0.15,
        material: "Block paving",
        parking: false,
        icon: "area-driveway",
    },
    AreaPreset {
        label: "Gravel",
        pattern: Pattern::Stipple { spacing: 0.12 },
        thickness: 0.1,
        material: "Gravel",
        parking: false,
        icon: "area-gravel",
    },
    AreaPreset {
        label: "Decking",
        pattern: Pattern::Lines {
            spacing: 0.145,
            angle: 0.0,
        },
        thickness: 0.05,
        material: "Timber decking",
        parking: false,
        icon: "area-decking",
    },
    AreaPreset {
        label: "Asphalt",
        pattern: Pattern::None,
        thickness: 0.1,
        material: "Asphalt",
        parking: false,
        icon: "area-asphalt",
    },
    AreaPreset {
        label: "Car park",
        pattern: Pattern::None,
        thickness: 0.15,
        material: "Asphalt",
        parking: true,
        icon: "parking",
    },
];

/// The IFC classes offered for an extrusion or a sweep.
pub const IFC_CLASSES: &[&str] = &[
    parts::PROXY,
    "IfcChimney",
    "IfcCovering",
    "IfcMember",
    "IfcPlate",
    "IfcSlab",
    "IfcStair",
    "IfcRamp",
    "IfcColumn",
    "IfcBeam",
    "IfcWall",
    "IfcFooting",
    "IfcShadingDevice",
    "IfcFurniture",
    "IfcCivilElement",
    "IfcBuildingElementPart",
];

/// Sizes for new parts and site elements, set in the tool bar.
#[derive(Clone, Debug)]
pub struct PartSettings {
    pub extrusion_base: f64,
    pub extrusion_height: f64,
    pub void_base: f64,
    pub void_height: f64,
    pub profile: SweepProfile,
    /// The profile's two sizes (see [`SweepProfile::sizes`]).
    pub size: (f64, f64),
    pub steps: u32,
    /// The sweep path clicked so far.
    pub path: Vec<DVec2>,
    pub terrain_thickness: f64,
    /// What the Terrain tool draws.
    pub ground: GroundDraw,
    /// The elevation of the next contour line or spot level drawn.
    pub ground_height: f64,
    /// The elevation of the next pad drawn; none starts it at the ground
    /// under its first corner.
    pub pad_level: Option<f64>,
    /// What is added to a height above the project's zero to show it as
    /// the project reads its site levels (its datum, or nothing), and
    /// the metres in the unit they are typed in. Kept up by the app.
    pub level_shift: f64,
    pub level_unit: f64,
    /// How much higher each next contour line is (lower, when negative).
    pub ground_step: f64,
    /// How deep the next pit or trench is dug below the ground.
    pub ground_depth: f64,
    /// The next click with Property line picks a lot from a map or a
    /// drawing under the plan, instead of starting one.
    pub pick_parcel: bool,
    /// How wide the next trench is.
    pub ground_width: f64,
    /// How wide the next road is.
    pub road_width: f64,
    /// What the next drain carries.
    pub drain_use: creabim_core::services::DrainUse,
    /// What the next tank is.
    pub tank_use: creabim_core::services::TankUse,
    /// What the next investigation point is.
    pub probe_kind: creabim_core::investigation::ProbeKind,
    /// The next path's surface.
    pub path_surface: creabim_core::walk::Surface,
    /// How wide the next path is.
    pub path_width: f64,
    pub area: usize,
    /// How deep a void drawn on a face in 3D cuts into it.
    pub face_depth: f64,
    pub plant: PlantForm,
    pub canopy: f64,
    pub plant_height: f64,
    /// A profile from the library (a preset or the project's own) new
    /// sweeps take instead of `profile` (MU27 block 5).
    pub library: Option<String>,
    /// A library profile the Extrusion tool places, pulled up (MU36).
    pub extrusion_library: Option<String>,
}

impl PartSettings {
    /// The profile new sweeps take, for a wall `wall` thick, and its name.
    pub fn sweep_profile(&self, model: &Model, wall: f64) -> (Vec<DVec2>, String) {
        if let Some(p) = self
            .library
            .as_deref()
            .and_then(|n| creabim_core::profiles::find(model, n))
        {
            return (p, self.library.clone().unwrap_or_default());
        }
        let (a, b) = self.size;
        (
            self.profile.profile(wall, a, b, self.steps),
            self.profile.label().to_string(),
        )
    }

    /// The wall edge new sweeps follow: a library profile the top.
    fn sweep_edge(&self) -> WallEdge {
        if self.library.is_some() {
            WallEdge::Top
        } else {
            self.profile.edge()
        }
    }
}

impl Default for PartSettings {
    fn default() -> Self {
        Self {
            extrusion_base: 0.0,
            extrusion_height: 1.0,
            void_base: 0.3,
            void_height: 0.6,
            face_depth: 0.5,
            profile: SweepProfile::Coping,
            size: SweepProfile::Coping.defaults(),
            steps: 3,
            path: vec![],
            terrain_thickness: 0.5,
            ground: GroundDraw::Outline,
            ground_height: 0.5,
            pad_level: None,
            level_shift: 0.0,
            level_unit: 1.0,
            ground_step: 0.5,
            ground_depth: 1.0,
            pick_parcel: false,
            ground_width: 0.6,
            road_width: 5.5,
            drain_use: creabim_core::services::DrainUse::Foul,
            tank_use: creabim_core::services::TankUse::Septic,
            probe_kind: creabim_core::investigation::ProbeKind::Borehole,
            path_surface: creabim_core::walk::Surface::Gravel,
            path_width: 1.2,
            area: 0,
            plant: PlantForm::Tree,
            canopy: 4.0,
            plant_height: 6.0,
            library: None,
            extrusion_library: None,
        }
    }
}

/// What the Terrain tool draws.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GroundDraw {
    /// A new terrain's outline.
    #[default]
    Outline,
    /// A contour line on a terrain, at a height.
    Contour,
    /// A spot height on a terrain.
    Spot,
    /// A levelled pad worked into a terrain, with its banks.
    Pad,
    /// A pit dug so deep below the ground inside an outline.
    Pit,
    /// A trench dug so deep and so wide along a line.
    Trench,
    /// A property: the lot's boundary, with its setbacks.
    Property,
    /// A road along a centreline, following the ground.
    Road,
    /// A drain run, falling from its first point to its last.
    Drain,
    /// A tank buried in the ground.
    Tank,
    /// A path laid on the ground along a line.
    Path,
    /// A borehole, trial pit or probe of the soils report.
    Investigation,
}

impl GroundDraw {
    pub const ALL: [GroundDraw; 12] = [
        GroundDraw::Outline,
        GroundDraw::Contour,
        GroundDraw::Spot,
        GroundDraw::Pad,
        GroundDraw::Pit,
        GroundDraw::Trench,
        GroundDraw::Property,
        GroundDraw::Road,
        GroundDraw::Path,
        GroundDraw::Drain,
        GroundDraw::Tank,
        GroundDraw::Investigation,
    ];

    /// The fewest points its line or outline can be finished with.
    pub fn fewest(self) -> usize {
        match self {
            GroundDraw::Pad | GroundDraw::Pit => 3,
            _ => 2,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            GroundDraw::Outline => "Outline",
            GroundDraw::Contour => "Contour line",
            GroundDraw::Spot => "Spot level",
            GroundDraw::Pad => "Pad",
            GroundDraw::Pit => "Pit",
            GroundDraw::Trench => "Trench",
            GroundDraw::Property => "Property line",
            GroundDraw::Road => "Road",
            GroundDraw::Drain => "Drain",
            GroundDraw::Tank => "Tank",
            GroundDraw::Path => "Path",
            GroundDraw::Investigation => "Investigation point",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            GroundDraw::Outline => "Draw a new terrain's outline; it starts flat",
            GroundDraw::Contour => "Draw or trace a contour line on a terrain at the Level: click its points; Enter or a click on the last one again finishes it (the first again closes it). The next line is the Step higher",
            GroundDraw::Spot => "Click spot levels onto a terrain, each at the Level",
            GroundDraw::Pad => "Level a platform into a terrain: click its corners, Enter or the first again closes it. It starts at the ground under its first corner unless set to a level. The ground is dug where it is above the pad and made up where it is below, with a bank each way, and the volumes are counted",
            GroundDraw::Pit => "Dig a pit into a terrain, the Depth below the ground all over: click its corners, the first again to close it",
            GroundDraw::Trench => "Dig a trench into a terrain, the Width wide and the Depth below the ground all along: click along its line, the last point again to finish",
            GroundDraw::Road => "Lay a road, drive or path the Width wide: click along its middle, the last point again to finish. It sits on the ground, eased and no steeper than 1:8; its levels, fall, kerbs, footpaths, banks and parking bays are in Properties",
            GroundDraw::Drain => "Lay a drain run from its head down to where it goes: click each chamber, the last again to finish. It falls 1 in 80 from 600 mm under the ground at its head; its pipe, fall, depth and chambers are in Properties",
            GroundDraw::Tank => "Bury a tank: click where its middle goes. Its size, depth, turn and cover are in Properties",
            GroundDraw::Investigation => "Place where the soils report looked: click each borehole, trial pit or probe. Its ground level, depth and log (layers, water, tests) are in Properties, with a box to paste the report's rows",
            GroundDraw::Path => "Lay a path on the ground the Width wide: click along its middle, the last point again to finish. It follows the ground; its surface, edging and make-up are in Properties",
            GroundDraw::Property => "Draw the lot's boundary like an outline; its first edge is the front. Its setbacks, height limit, coverage and floor area ratio are in Properties, and the building is checked against them. From the map picks a lot drawn on a map or a DXF under the plan",
        }
    }
}

/// The terrain a contour or a spot height drawn at `p` goes to: the
/// selected one, else the smallest one under `p`.
pub fn ground_target(model: &Model, selection: &Selection, p: DVec2) -> Option<ElementId> {
    let terrain = |e: &Element| match &e.kind {
        ElementKind::Site(Site {
            form: SiteForm::Terrain { boundary, .. },
            ..
        }) => Some(boundary.clone()),
        _ => None,
    };
    let picked = selection
        .all()
        .into_iter()
        .find(|id| model.get(*id).and_then(terrain).is_some());
    picked.or_else(|| {
        model
            .iter()
            .filter_map(|e| Some((e.id, terrain(e)?)))
            .filter(|(_, b)| creabim_core::polygon::contains(b, p))
            .min_by(|a, b| {
                creabim_core::polygon::signed_area2(&a.1)
                    .abs()
                    .total_cmp(&creabim_core::polygon::signed_area2(&b.1).abs())
            })
            .map(|(id, _)| id)
    })
}

/// The smallest closed ring of the references' lines round `p`, in
/// model coordinates: a lot on a map or a survey drawing.
pub fn parcel_at(model: &Model, p: DVec2) -> Option<Vec<DVec2>> {
    let mut best: Option<(f64, Vec<DVec2>)> = None;
    for e in model.iter() {
        let ElementKind::Reference(r) = &e.kind else {
            continue;
        };
        if r.hidden {
            continue;
        }
        let crate::trace::Shown::Lines(lines) = crate::trace::read(model, &r.source) else {
            continue;
        };
        for l in lines.iter() {
            let (Some(first), Some(last)) = (l.first(), l.last()) else {
                continue;
            };
            if l.len() < 4 || first.distance(*last) > 1e-6 {
                continue;
            }
            let ring: Vec<DVec2> = l[..l.len() - 1].iter().map(|q| r.place(*q)).collect();
            let area = creabim_core::polygon::signed_area2(&ring).abs();
            if area > 1e-6
                && creabim_core::polygon::contains(&ring, p)
                && best.as_ref().is_none_or(|b| area < b.0)
            {
                best = Some((area, ring));
            }
        }
    }
    best.map(|b| b.1)
}

/// A click with Property line's From the map: the lot round the click.
fn pick_parcel(
    doc: &mut Document,
    level: ElementId,
    p: DVec2,
    state: &mut PlanState,
    selection: &mut Selection,
) -> Result<(), ModelError> {
    state.parts.pick_parcel = false;
    let Some(ring) = parcel_at(doc.model(), p) else {
        state.status = Some("No closed lot on a map or drawing under the plan there. Bring one in with + Map or + DXF (Trace and reference), or draw the boundary.".to_string());
        return Ok(());
    };
    let n = ring.len();
    let e = Element::new("Property", ElementKind::Site(Site::property(level, ring)));
    let id = e.id;
    doc.commit(Transaction::new("Property from the map").insert(e))?;
    selection.only(id);
    state.status = Some(format!(
        "Property of {n} corners taken from the map. Its first edge is the front: set what each edge is, the setbacks and the limits in Properties."
    ));
    Ok(())
}

/// A click with the Terrain tool set to Road: a point on its line, or
/// the last again to lay it, each point at the ground's height there.
fn road_click(
    doc: &mut Document,
    level: ElementId,
    p: DVec2,
    state: &mut PlanState,
    selection: &mut Selection,
) -> Result<(), ModelError> {
    let width = state.parts.road_width;
    let path = &mut state.parts.path;
    if !path.last().is_some_and(|q| q.distance(p) < 0.05) {
        path.push(p);
        state.pending = Some(p);
        state.status = Some(
            "Road: click along its middle; Enter or a click on the last point again finishes it. Right-click takes back a point."
                .to_string(),
        );
        return Ok(());
    }
    let pts = std::mem::take(path);
    state.pending = None;
    if pts.len() < 2 {
        return Ok(());
    }
    let model = doc.model();
    let base = creabim_core::members::elevation(model, level);
    // each point at the ground as it was there, from the terrain under it
    let ground = |q: DVec2| -> Option<f64> {
        let id = ground_target(model, &Selection::default(), q)?;
        let ElementKind::Site(t) = &model.get(id)?.kind else {
            return None;
        };
        let g = creabim_geom::earth::shaped(model, t)?;
        let tb = creabim_core::members::elevation(model, t.level);
        Some(g.existing.height(q)? + tb - base)
    };
    let mut site = Site::road(level, pts.clone(), width);
    let mut followed = false;
    if let SiteForm::Road(r) = &mut site.form {
        // a new road sits on the land: its levels follow the ground
        r.levels = creabim_core::road::Levels::ground();
        for (c, q) in r.path.iter_mut().zip(&pts) {
            if let Some(z) = ground(*q) {
                c.z = z;
                followed = true;
            }
        }
    }
    let e = Element::new("Road", ElementKind::Site(site));
    let id = e.id;
    doc.commit(Transaction::new("Lay road").insert(e))?;
    selection.only(id);
    state.status = Some(if followed {
        "Road laid on the ground, eased and no steeper than 1:8. Set its levels, fall, kerbs and banks in Properties."
    } else {
        "Road laid. Set its levels, fall, kerbs and parking in Properties."
    }
    .to_string());
    Ok(())
}

/// A click with the Terrain tool set to Path: a point on its line, or
/// the last again to lay it.
fn walk_click(
    doc: &mut Document,
    level: ElementId,
    p: DVec2,
    state: &mut PlanState,
    selection: &mut Selection,
) -> Result<(), ModelError> {
    let (surface, width) = (state.parts.path_surface, state.parts.path_width);
    let path = &mut state.parts.path;
    if !path.last().is_some_and(|q| q.distance(p) < 0.05) {
        path.push(p);
        state.pending = Some(p);
        state.status = Some(
            "Path: click along its middle; Enter or a click on the last point again finishes it. Right-click takes back a point."
                .to_string(),
        );
        return Ok(());
    }
    let pts = std::mem::take(path);
    state.pending = None;
    if pts.len() < 2 {
        return Ok(());
    }
    let mut site = Site::walk(level, pts, surface);
    if let creabim_core::SiteForm::Walk(w) = &mut site.form {
        w.width = width.max(0.1);
    }
    let e = Element::new(format!("{} path", surface.label()), ElementKind::Site(site));
    let id = e.id;
    doc.commit(Transaction::new("Lay path").insert(e))?;
    selection.only(id);
    state.status = Some(
        "Path laid on the ground. Set its surface, width, edging and make-up in Properties."
            .to_string(),
    );
    Ok(())
}

/// A click with the Terrain tool set to Drain: a chamber on its run, or
/// the last again to lay it.
fn drain_click(
    doc: &mut Document,
    level: ElementId,
    p: DVec2,
    state: &mut PlanState,
    selection: &mut Selection,
) -> Result<(), ModelError> {
    let service = state.parts.drain_use;
    let path = &mut state.parts.path;
    if !path.last().is_some_and(|q| q.distance(p) < 0.05) {
        path.push(p);
        state.pending = Some(p);
        state.status = Some(
            "Drain: click each chamber down the run; Enter or a click on the last again finishes it. Right-click takes back a point."
                .to_string(),
        );
        return Ok(());
    }
    let pts = std::mem::take(path);
    state.pending = None;
    if pts.len() < 2 {
        return Ok(());
    }
    let site = Site::drain(level, pts, service);
    let name = site.label();
    let e = Element::new(name, ElementKind::Site(site));
    let id = e.id;
    doc.commit(Transaction::new("Lay drain").insert(e))?;
    selection.only(id);
    state.status = Some(
        "Drain laid at 1 in 80 from 600 mm deep. Set its pipe, fall and depth in Properties, where its levels and trench are measured."
            .to_string(),
    );
    Ok(())
}

/// A click with the Terrain tool set to draw a contour line or a spot
/// height.
fn ground_click(
    doc: &mut Document,
    p: DVec2,
    state: &mut PlanState,
    selection: &mut Selection,
) -> Result<(), ModelError> {
    let s = state.parts.clone();
    let none = "Click on a terrain: draw its outline first (the Terrain tool's Outline), or select the terrain the heights are for.";
    // a line: its points first, then the terrain under its start
    let mut line = None;
    let pit = s.ground == GroundDraw::Pit;
    let trench = s.ground == GroundDraw::Trench;
    let pad = s.ground == GroundDraw::Pad || pit;
    if s.ground == GroundDraw::Contour || pad || trench {
        let path = &mut state.parts.path;
        let ends = path.last().is_some_and(|q| q.distance(p) < 0.05);
        let closes = path.len() >= 3 && path[0].distance(p) < 0.05;
        if !(ends || closes) {
            path.push(p);
            state.pending = Some(p);
            state.status = Some(if pit {
                "Pit: click its next corner; Enter or a click on the first one again closes it. Right-click takes back a point."
                    .to_string()
            } else if trench {
                "Trench: click along its line; Enter or a click on the last point again finishes it. Right-click takes back a point."
                    .to_string()
            } else if pad {
                match s.pad_level {
                    Some(z) => format!(
                        "Pad at {}: click its next corner; Enter or a click on the first one again closes it. Right-click takes back a point.",
                        level_text(doc.model(), z)
                    ),
                    None => "Pad at the ground under its first corner: click its next corner; Enter or a click on the first one again closes it. Right-click takes back a point.".to_string(),
                }
            } else {
                format!(
                    "Contour at {}: click its next point; Enter or a click on the last one again finishes it, the first closes it. Right-click takes back a point.",
                    level_text(doc.model(), s.ground_height)
                )
            });
            return Ok(());
        }
        let pts = std::mem::take(path);
        state.pending = None;
        if pts.len() < if pad { 3 } else { 2 } {
            return Ok(());
        }
        line = Some((pts, (closes || pad) && !trench));
    }
    let at = line.as_ref().map_or(p, |l| l.0[0]);
    let Some(before) = ground_target(doc.model(), selection, at)
        .and_then(|id| doc.model().get(id))
        .cloned()
    else {
        state.status = Some(none.to_string());
        return Ok(());
    };
    // a pad, a pit or a trench is an element of its own, on the terrain's
    // level, worked into the ground under it
    if pad || trench {
        let (Some((pts, _)), ElementKind::Site(ground)) = (line, &before.kind) else {
            return Ok(());
        };
        let base = creabim_core::members::elevation(doc.model(), ground.level);
        // a pad with no level typed starts at the ground under its first
        // corner, so it changes little until its level is set
        let pad_at = s.pad_level.unwrap_or_else(|| {
            creabim_geom::earth::shaped(doc.model(), ground)
                .and_then(|g| g.existing.height(pts[0]))
                .map_or(base, |z| ((z + base) * 20.0).round() / 20.0)
        });
        let (work, label) = if trench {
            (
                Site::trench(ground.level, pts, s.ground_width, s.ground_depth),
                "Dig trench",
            )
        } else if pit {
            // a pit's sides lie back 1 in 2; a trench's stand upright
            (Site::pit(ground.level, pts, s.ground_depth, 0.5), "Dig pit")
        } else {
            (Site::pad(ground.level, pts, pad_at - base), "Draw pad")
        };
        state.status = Some(if pad && !pit {
            format!(
                "Pad at {} worked into {}. Drag it, double-click to reshape it; its level and banks are in Properties.",
                level_text(doc.model(), pad_at),
                before.name
            )
        } else {
            format!(
                "{} {} deep dug into {}. Drag it, double-click to reshape it; its depth and sides are in Properties.",
                work.label(),
                crate::units::len(s.ground_depth),
                before.name
            )
        });
        let e = Element::new(work.label(), ElementKind::Site(work));
        selection.only(e.id);
        return doc.commit(Transaction::new(label).insert(e));
    }
    // a spot level and a contour are elements of their own too, on the
    // terrain's level: its ground passes through them
    let ElementKind::Site(site) = &before.kind else {
        return Ok(());
    };
    if !matches!(site.form, SiteForm::Terrain { .. }) {
        return Ok(());
    }
    let base = creabim_core::members::elevation(doc.model(), site.level);
    let first = !creabim_core::site::surveyed(doc.model(), site);
    let z = s.ground_height - base;
    let mut tx = Transaction::new("Spot level");
    let thing = match line {
        Some((pts, closed)) => {
            let mut c = Site::contour(site.level, pts, z);
            if let SiteForm::Line(l) = &mut c.form {
                l.closed = closed;
            }
            tx = Transaction::new("Draw contour");
            if first {
                // traced from a map: the edge follows the lines, and the
                // contours between them show at the step drawn
                let mut after = before.clone();
                if let ElementKind::Site(Site {
                    form: SiteForm::Terrain { contour, data, .. },
                    ..
                }) = &mut after.kind
                {
                    data.survey = true;
                    data.labels = true;
                    if *contour == 0.0 {
                        *contour = s.ground_step.abs();
                    }
                }
                tx = tx.replace(before.clone(), after);
            }
            state.parts.ground_height += s.ground_step;
            state.status = Some(format!(
                "Contour at {} drawn on {}. The next one is at {}. Click it to move it or change its level; double-click to reshape it.",
                level_text(doc.model(), s.ground_height),
                before.name,
                level_text(doc.model(), state.parts.ground_height)
            ));
            c
        }
        None => {
            state.status = Some(format!(
                "Spot level {} on {}. Click on for more; click one to move it or change its level.",
                level_text(doc.model(), s.ground_height),
                before.name
            ));
            Site::spot(site.level, p, z)
        }
    };
    let e = Element::new(thing.label(), ElementKind::Site(thing));
    selection.only(e.id);
    doc.commit(tx.insert(e))
}

/// Ask the app to stand a retaining wall on edge `edge` of the selected
/// earthwork, or to take it away.
pub fn ask_wall(ui: &egui::Ui, edge: usize, add: bool) {
    ui.ctx()
        .data_mut(|d| d.insert_temp(egui::Id::new("site.wall"), (edge, add)));
}

/// The retaining wall an Earthwork tab asked for, if any.
pub fn take_wall(ctx: &egui::Context) -> Option<(usize, bool)> {
    ctx.data_mut(|d| d.remove_temp::<(usize, bool)>(egui::Id::new("site.wall")))
}

/// How thick a retaining wall starts.
pub const RETAINING_WALL: f64 = 0.25;

/// The change that stands a retaining wall on edge `edge` of earthwork
/// `id`, or takes it away: an ordinary wall outside the outline, its
/// face on the edge, from the work's level to just above the ground it
/// holds, kept by the work so the ground is cut at its back. None when
/// `id` is no earthwork, has no such edge, or there is nothing to do.
pub fn retaining_wall(model: &Model, id: ElementId, edge: usize, add: bool) -> Option<Transaction> {
    let before = model.get(id)?.clone();
    let ElementKind::Site(site) = &before.kind else {
        return None;
    };
    let SiteForm::Pad {
        boundary,
        top,
        depth,
        walls,
        cut,
        retained,
        ..
    } = &site.form
    else {
        return None;
    };
    let n = boundary.len();
    if edge >= n {
        return None;
    }
    let (bank, banked_before, banked_after) = (
        *cut,
        !retained.contains(&((edge + n - 1) % n)),
        !retained.contains(&((edge + 1) % n)),
    );
    let old = walls.iter().find(|w| w.0 == edge).map(|w| w.1);
    let mut after = before.clone();
    let set = |after: &mut Element, wall: Option<ElementId>| {
        if let ElementKind::Site(Site {
            form: SiteForm::Pad {
                retained, walls, ..
            },
            ..
        }) = &mut after.kind
        {
            walls.retain(|w| w.0 != edge);
            if let Some(w) = wall {
                walls.push((edge, w));
                if !retained.contains(&edge) {
                    retained.push(edge);
                    retained.sort_unstable();
                }
            }
        }
    };
    if !add {
        let old = old?;
        set(&mut after, None);
        let tx = Transaction::new("Remove retaining wall").replace(before, after);
        return Some(if model.get(old).is_some() {
            tx.remove_cascade(model, old)
        } else {
            tx
        });
    }
    if old.is_some_and(|w| model.get(w).is_some()) {
        return None;
    }
    let (a, b) = (boundary[edge], boundary[(edge + 1) % n]);
    let d = (b - a).normalize_or_zero();
    if d == DVec2::ZERO {
        return None;
    }
    // the outline runs counter-clockwise: out is to its right
    let out = DVec2::new(d.y, -d.x);
    let t = RETAINING_WALL;
    let base = creabim_core::members::elevation(model, site.level);
    // how high the ground it holds stands above the work's floor
    let ground = model.iter().find_map(|e| match &e.kind {
        ElementKind::Site(g) => match &g.form {
            SiteForm::Terrain { boundary: tb, .. }
                if creabim_core::polygon::contains(tb, (a + b) / 2.0) =>
            {
                Some(g)
            }
            _ => None,
        },
        _ => None,
    });
    let mut held: f64 = 0.0;
    let mut floor = *top;
    // how far the wall runs on past each end: as far as the bank of the
    // edge beside it runs out there, so that bank ends against the wall
    // and not in a raw upright face
    let (mut on_a, mut on_b) = (0.0_f64, 0.0_f64);
    if let Some(g) = ground {
        if let Some(sh) = creabim_geom::earth::shaped(model, g) {
            let gb = creabim_core::members::elevation(model, g.level);
            let rise = |q: DVec2, floor: f64| {
                sh.existing
                    .height(q)
                    .map_or(0.0, |z| (z + gb - base - floor).max(0.0))
            };
            if *depth <= 0.0 {
                for (corner, along, banked, on) in [
                    (a, -d, banked_before, &mut on_a),
                    (b, d, banked_after, &mut on_b),
                ] {
                    if !banked || bank <= 0.0 {
                        continue;
                    }
                    // where a bank of that run from the floor meets the
                    // ground, found in a few steps
                    let mut t = 0.0;
                    for _ in 0..6 {
                        t = (bank * rise(corner + along * t + out * (RETAINING_WALL + 0.1), *top))
                            .min(30.0);
                    }
                    *on = (t / 0.05).ceil() * 0.05;
                }
            }
            let mut lowest = f64::INFINITY;
            for k in 0..=8 {
                let q = (a - d * on_a).lerp(b + d * on_b, k as f64 / 8.0) + out * (t + 0.1);
                if let Some(z) = sh.existing.height(q) {
                    let z = z + gb - base;
                    held = held.max(z);
                    lowest = lowest.min(z);
                }
            }
            // a pit's floor is its depth below the ground
            if *depth > 0.0 && lowest.is_finite() {
                floor = lowest - depth;
            }
        }
    }
    let height = (((held - floor + 0.15) / 0.05).ceil() * 0.05).max(0.5);
    let mut wall = creabim_core::Wall::new(
        site.level,
        a - d * on_a + out * (t / 2.0),
        b + d * on_b + out * (t / 2.0),
        t,
        height,
    );
    wall.base_offset = floor;
    // kept on its face along the work's edge (its left, the outline
    // running counter-clockwise): thickened, it grows into the ground
    // behind it and never onto the pad
    wall.location = creabim_core::LocationLine::InteriorFace;
    let wall = Element::new("Retaining wall", ElementKind::Wall(wall));
    set(&mut after, Some(wall.id));
    let mut tx = Transaction::new("Add retaining wall")
        .insert(wall)
        .replace(before, after);
    if let Some(old) = old.filter(|w| model.get(*w).is_some()) {
        tx = tx.remove_cascade(model, old);
    }
    Some(tx)
}

/// A site level as the project reads them: from its datum (100.45) or
/// from its zero (+0.45). `z` is above the project's zero.
fn level_text(model: &Model, z: f64) -> String {
    match creabim_core::units::level_reading(model) {
        creabim_core::LevelReading::Datum => creabim_core::site_level(model, z),
        creabim_core::LevelReading::Zero => creabim_core::units::level_from_zero(model, z),
    }
}

/// What an outline drawn with one of the outline tools makes.
#[derive(Clone, Debug)]
pub enum Make {
    Extrusion { base: f64, height: f64, void: bool },
    Terrain { thickness: f64 },
    Area(usize),
    Property,
}

/// What an outline drawn now with `state`'s tool makes.
pub fn make_for(state: &PlanState) -> Option<Make> {
    let s = &state.parts;
    Some(match state.tool {
        Tool::Extrusion => Make::Extrusion {
            base: s.extrusion_base,
            height: s.extrusion_height,
            void: false,
        },
        Tool::Void => Make::Extrusion {
            base: s.void_base,
            height: s.void_height,
            void: true,
        },
        Tool::Terrain if s.ground == GroundDraw::Property => Make::Property,
        Tool::Terrain => Make::Terrain {
            thickness: s.terrain_thickness,
        },
        // the earlier pad tool: pads are drawn with Terrain now
        Tool::Pad => return None,
        Tool::Paving => Make::Area(s.area),
        _ => return None,
    })
}

/// Where a part drawn on a face in 3D starts off the face and how far it
/// runs, along the face's outward normal: an extrusion stands out of the
/// face, a void cuts into it (and pokes 10 mm out, for a clean cut).
pub fn on_face(state: &PlanState) -> Option<(f64, f64, bool)> {
    let s = &state.parts;
    match state.tool {
        Tool::Extrusion => Some((0.0, s.extrusion_height, false)),
        Tool::Void => Some((-s.face_depth, s.face_depth + 0.01, true)),
        _ => None,
    }
}

/// The profile and name of a sweep drawn now on a face in the 3D view.
pub fn face_sweep(state: &PlanState, model: &Model) -> Option<(Vec<DVec2>, String)> {
    (state.tool == Tool::Sweep).then(|| state.parts.sweep_profile(model, 0.2))
}

/// The new element (and any library material it needs, first) for an
/// outline on `level`, with the undo label and a message.
pub fn from_outline(
    model: &Model,
    level: ElementId,
    outline: Vec<DVec2>,
    make: &Make,
) -> (Vec<Element>, &'static str, String) {
    let mut out = vec![];
    let mut material = |name: &str| {
        let (id, new) = creabim_core::library::find_or_new(model, name)?;
        out.extend(new);
        Some(id)
    };
    let (e, label, msg) = match *make {
        Make::Extrusion { base, height, void } => {
            let p = Part::extrusion(level, outline, base, height);
            if void {
                (
                    Element::new("Void", ElementKind::Part(p.as_void())),
                    "Draw void",
                    "Void added: it cuts every wall, slab, roof and solid part it meets. Set its base and height in Properties. Ctrl+Z to undo.".to_string(),
                )
            } else {
                (
                    Element::new("Extrusion", ElementKind::Part(p)),
                    "Draw extrusion",
                    "Extrusion added. Set its base, height, material and IFC class in Properties; draw a Void to cut into it. Ctrl+Z to undo.".to_string(),
                )
            }
        }
        Make::Terrain { thickness } => {
            let mut s = Site::terrain(level, outline, 0.0, thickness);
            s.material = material("Grass");
            (
                Element::new("Terrain", ElementKind::Site(s)),
                "Draw terrain",
                "Terrain added, flat at the level. Shape it with Import survey, or the Site tools' Contour line and Spot level. Ctrl+Z to undo.".to_string(),
            )
        }
        Make::Property => (
            Element::new("Property", ElementKind::Site(Site::property(level, outline))),
            "Draw property line",
            "Property added; its first edge is the front. Set what each edge is, the setbacks and the limits in Properties. Ctrl+Z to undo.".to_string(),
        ),
        Make::Area(i) => {
            let p = AREAS.get(i).unwrap_or(&AREAS[0]);
            let mut s = Site::area(level, outline, p.thickness, p.pattern);
            s.material = material(p.material);
            if p.parking {
                if let SiteForm::Area { parking, .. } = &mut s.form {
                    *parking = Some(creabim_core::carpark::CarPark::default());
                }
            }
            (
                Element::new(p.label, ElementKind::Site(s)),
                if p.parking { "Draw car park" } else { "Draw hard landscape" },
                if p.parking {
                    "Car park added: it has laid its own bays. Set its bays, aisles and count in its Parking tab; draw a road up to its edge for its entrance. Ctrl+Z to undo.".to_string()
                } else {
                    format!(
                        "{} added. Change its pattern and depth in Properties. Ctrl+Z to undo.",
                        p.label
                    )
                },
            )
        }
    };
    out.push(e);
    (out, label, msg)
}

/// Commit an outline drawn with an outline tool. Returns a message and the
/// new element.
pub fn finish_outline(
    doc: &mut Document,
    level: ElementId,
    outline: Vec<DVec2>,
    make: &Make,
) -> Result<(String, Option<ElementId>), ModelError> {
    let (elements, label, msg) = from_outline(doc.model(), level, outline, make);
    let id = elements.last().map(|e| e.id);
    let mut tx = Transaction::new(label);
    for e in elements {
        tx = tx.insert(e);
    }
    doc.commit(tx)?;
    Ok((msg, id))
}

/// A click with the Sweep or Planting tool on `level`.
pub fn click(
    doc: &mut Document,
    level: ElementId,
    tool: Tool,
    p: DVec2,
    state: &mut PlanState,
    selection: &mut Selection,
) -> Result<(), ModelError> {
    let s = state.parts.clone();
    let add = |doc: &mut Document, selection: &mut Selection, e: Element, label: &str| {
        selection.only(e.id);
        doc.commit(Transaction::new(label).insert(e))
    };
    match tool {
        Tool::Terrain if s.ground == GroundDraw::Property => {
            pick_parcel(doc, level, p, state, selection)
        }
        Tool::Terrain if s.ground == GroundDraw::Road => {
            road_click(doc, level, p, state, selection)
        }
        Tool::Terrain if s.ground == GroundDraw::Drain => {
            drain_click(doc, level, p, state, selection)
        }
        Tool::Terrain if s.ground == GroundDraw::Path => {
            walk_click(doc, level, p, state, selection)
        }
        Tool::Terrain if s.ground == GroundDraw::Investigation => {
            let model = doc.model();
            let name = creabim_core::investigation::next_name(
                s.probe_kind,
                model.iter().map(|e| e.name.as_str()),
            );
            let site = Site::probe(level, p, s.probe_kind);
            let r = add(
                doc,
                selection,
                Element::new(name.clone(), ElementKind::Site(site)),
                "Place investigation point",
            );
            state.status = Some(format!(
                "{name} placed. Type its log in Properties (or paste the report's rows); click on to place the next."
            ));
            r
        }
        Tool::Terrain if s.ground == GroundDraw::Tank => {
            let site = Site::tank(level, p, s.tank_use);
            let name = s.tank_use.label();
            let r = add(
                doc,
                selection,
                Element::new(name, ElementKind::Site(site)),
                "Bury tank",
            );
            state.status = Some(format!(
                "{name} buried. Set its size, depth, turn and cover in Properties; its dig is measured there."
            ));
            r
        }
        Tool::Terrain => ground_click(doc, p, state, selection),
        Tool::Planting => {
            // a new tree is grown, with a seed from where it stands
            let plant = match s.plant {
                PlantForm::Tree => Site::tree(
                    level,
                    p,
                    TreeShape {
                        crown_base: (s.plant_height * 0.35).min(2.2),
                        trunk: (s.canopy * 0.06).clamp(0.08, 0.6),
                        ..TreeShape::new_at(s.canopy, p)
                    },
                    s.canopy,
                    s.plant_height,
                ),
                f => Site::plant(level, p, f, s.canopy, s.plant_height),
            };
            state.status = Some(format!(
                "{} placed, {} across. Click on to plant more.",
                s.plant.label(),
                crate::units::len(s.canopy)
            ));
            add(
                doc,
                selection,
                Element::new(s.plant.label(), ElementKind::Site(plant)),
                "Place plant",
            )
        }
        Tool::Sweep => {
            let (a, b) = s.size;
            let _ = (a, b);
            if s.path.is_empty() && (s.library.is_some() || s.profile != SweepProfile::Steps) {
                if let Some(wall) = crate::memberstool::wall_at(doc.model(), level, p) {
                    let Some(ElementKind::Wall(w)) = doc.model().get(wall).map(|e| &e.kind) else {
                        return Ok(());
                    };
                    let (profile, name) = s.sweep_profile(doc.model(), w.thickness);
                    let mut part = Part::sweep(
                        level,
                        profile,
                        Path::Wall {
                            wall,
                            edge: s.sweep_edge(),
                            offset: 0.0,
                            extend: [0.0, 0.0],
                        },
                    );
                    part.profile = s.library.clone();
                    state.status = Some(format!(
                        "A sweep of {name} added along the wall's {}: it follows the wall. Click another wall for the next one.",
                        s.sweep_edge().label().to_lowercase()
                    ));
                    return add(
                        doc,
                        selection,
                        Element::new("Sweep", ElementKind::Part(part)),
                        "Sweep along wall",
                    );
                }
            }
            let path = &mut state.parts.path;
            if path.last().is_some_and(|q| q.distance(p) < 0.05) {
                // clicking the last point again finishes
                let pts = std::mem::take(path);
                state.pending = None;
                if pts.len() < 2 {
                    return Ok(());
                }
                let (profile, name) = s.sweep_profile(doc.model(), 0.2);
                let mut part = Part::sweep(
                    level,
                    profile,
                    Path::Points {
                        points: pts.iter().map(|q| q.extend(0.0)).collect(),
                        closed: false,
                        normal: None,
                    },
                );
                part.profile = s.library.clone();
                state.status = Some(format!("A sweep of {name} added."));
                return add(
                    doc,
                    selection,
                    Element::new("Sweep", ElementKind::Part(part)),
                    "Draw sweep",
                );
            }
            path.push(p);
            state.pending = Some(p);
            state.status = Some(
                "Click the path's next point; click the last one again to finish. Esc cancels."
                    .into(),
            );
            Ok(())
        }
        _ => Ok(()),
    }
}

/// The tool bar options of these tools.
pub fn tool_options(state: &PlanState) -> Vec<serde_json::Value> {
    use serde_json::json;
    let s = &state.parts;
    let num = |key: &str, label: &str, value: f64, hint: &str| {
        json!({
            "kind": "number", "key": format!("part.{key}"), "label": label, "value": value,
            "unit": "m", "step": 0.01, "min": -100.0, "max": 1000.0, "hint": hint,
        })
    };
    // a site level, typed as the project reads them: metres (or feet)
    // from the datum or from the project's zero
    let level = |key: &str, z: f64, hint: &str| {
        let per = if s.level_unit > 0.0 {
            s.level_unit
        } else {
            1.0
        };
        let shown = (((z + s.level_shift) / per) * 1000.0).round() / 1000.0;
        json!({
            "kind": "number", "key": format!("part.{key}"), "label": "Level", "value": shown,
            "unit": if per == 1.0 { " m" } else { " ft" }, "step": 0.05,
            "min": -1000.0, "max": 10000.0, "hint": hint,
        })
    };
    let seg = |items: Vec<serde_json::Value>| json!({"kind": "seg", "items": items});
    let opt = |id: String, label: &str, on: bool, hint: &str| json!({"id": id, "label": label, "on": on, "hint": hint});
    let mut out = match state.tool {
        Tool::Extrusion => vec![
            json!({
                "kind": "menu", "key": "extrusion.library", "label": "Profile",
                "value": s.extrusion_library.clone().unwrap_or_else(|| "Draw".into()), "icon": "profile",
                "hint": "Draw the outline, or place a profile from the library at a click, pulled up by the Height",
            }),
            num(
                "extrusion_base",
                "Base",
                s.extrusion_base,
                "Height of new extrusions' undersides above the level",
            ),
            num(
                "extrusion_height",
                "Height",
                s.extrusion_height,
                "Height of new extrusions",
            ),
        ],
        Tool::Void => vec![
            num(
                "void_base",
                "Base",
                s.void_base,
                "Height of new voids' undersides above the level",
            ),
            num(
                "void_height",
                "Height",
                s.void_height,
                "Height of new voids",
            ),
            num(
                "face_depth",
                "Depth on a face",
                s.face_depth,
                "How deep a void drawn on a face in the 3D view cuts into it, square to the face",
            ),
        ],
        Tool::Sweep => {
            let (la, lb) = s.profile.sizes();
            let mut v = vec![seg(SweepProfile::ALL
                .iter()
                .map(|p| {
                    opt(
                        format!("part:profile.{}", p.label()),
                        p.label(),
                        s.library.is_none() && s.profile == *p,
                        "The profile new sweeps start from",
                    )
                })
                .collect())];
            // the profile library: presets, the project's own, and ways to
            // make one (MU27 block 5)
            v.push(json!({
                "kind": "menu", "key": "sweep.library", "label": "Profile",
                "value": s.library.clone().unwrap_or_else(|| "Library".into()), "icon": "profile",
                "hint": "Pick a profile from the library: presets and the project's own; or make one from a shape",
            }));
            if s.library.is_none() {
                v.push(num("size_a", la, s.size.0, "A size of the profile"));
                v.push(num("size_b", lb, s.size.1, "A size of the profile"));
            }
            if s.library.is_none() && s.profile == SweepProfile::Steps {
                v.push(json!({
                    "kind": "number", "key": "part.steps", "label": "Steps", "value": s.steps,
                    "unit": "", "step": 1, "min": 1, "max": 40, "hint": "Number of steps",
                }));
            }
            v
        }
        Tool::Terrain => {
            // what it draws is picked in the Site family's panel (tool
            // families REV04: the strip says how, and the values)
            let mut v = vec![];
            match s.ground {
                GroundDraw::Outline => v.push(num(
                    "terrain_thickness",
                    "Depth",
                    s.terrain_thickness,
                    "How deep new terrain is below its surface",
                )),
                GroundDraw::Contour => {
                    v.push(level(
                        "ground_level",
                        s.ground_height,
                        "The level of the contour line drawn next, read as the project reads its site levels",
                    ));
                    v.push(num(
                        "ground_step",
                        "Step",
                        s.ground_step,
                        "How much higher each next line is; negative steps down",
                    ));
                }
                GroundDraw::Spot => v.push(level(
                    "ground_level",
                    s.ground_height,
                    "The level of the spot levels clicked next, read as the project reads its site levels",
                )),
                GroundDraw::Pad => {
                    v.push(seg(vec![
                        opt(
                            "part:padlevel.ground".to_string(),
                            "At the ground",
                            s.pad_level.is_none(),
                            "The pad drawn next starts at the ground under its first corner; set its level after, in Properties",
                        ),
                        opt(
                            "part:padlevel.set".to_string(),
                            "At a level",
                            s.pad_level.is_some(),
                            "The pad drawn next is at the level typed here",
                        ),
                    ]));
                    if let Some(z) = s.pad_level {
                        v.push(level("pad_level", z, "The level of the pad drawn next"));
                    }
                }
                GroundDraw::Pit => v.push(num(
                    "ground_depth",
                    "Depth",
                    s.ground_depth,
                    "How deep the pit drawn next is dug below the ground",
                )),
                GroundDraw::Property => v.push(json!({
                    "kind": "button", "id": "part:ground.parcel", "icon": "map", "label": "From the map",
                    "hint": "Click inside a lot drawn on a map or a DXF under the plan (+ Map or + DXF in Trace and reference) to make it the property",
                })),
                GroundDraw::Drain => v.push(seg(creabim_core::services::DrainUse::ALL
                    .iter()
                    .map(|u| {
                        opt(
                            format!("part:drain.{}", u.label()),
                            u.label(),
                            s.drain_use == *u,
                            "What the drain laid next carries",
                        )
                    })
                    .collect())),
                GroundDraw::Investigation => v.push(seg(creabim_core::investigation::ProbeKind::ALL
                    .iter()
                    .map(|k| {
                        opt(
                            format!("part:probe.{}", k.label()),
                            k.label(),
                            s.probe_kind == *k,
                            "What the investigation point placed next is",
                        )
                    })
                    .collect())),
                GroundDraw::Tank => v.push(seg(creabim_core::services::TankUse::ALL
                    .iter()
                    .map(|u| {
                        opt(
                            format!("part:tank.{}", u.label()),
                            u.label(),
                            s.tank_use == *u,
                            "What the tank buried next is",
                        )
                    })
                    .collect())),
                GroundDraw::Path => {
                    v.push(seg(creabim_core::walk::Surface::ALL
                        .iter()
                        .map(|u| {
                            opt(
                                format!("part:path.{}", u.label()),
                                u.label(),
                                s.path_surface == *u,
                                "The surface of the path laid next",
                            )
                        })
                        .collect()));
                    v.push(num(
                        "path_width",
                        "Width",
                        s.path_width,
                        "How wide the path laid next is, between its edging",
                    ))
                }
                GroundDraw::Road => v.push(num(
                    "road_width",
                    "Width",
                    s.road_width,
                    "How wide the road drawn next is, kerb to kerb",
                )),
                GroundDraw::Trench => {
                    v.push(num(
                        "ground_width",
                        "Width",
                        s.ground_width,
                        "How wide the trench drawn next is",
                    ));
                    v.push(num(
                        "ground_depth",
                        "Depth",
                        s.ground_depth,
                        "How deep the trench drawn next is dug below the ground",
                    ));
                }
            }
            v.push(json!({
                "kind": "button", "id": "part:ground.import", "icon": "download", "label": "Import survey",
                "hint": "Bring in a survey: a table of points (Excel, CSV, text or pasted rows) or a DXF drawing with contours",
            }));
            v
        }
        // the kind of area and of plant is picked in the Site panel
        Tool::Paving => vec![],
        Tool::Planting => {
            vec![
                num("canopy", "Canopy", s.canopy, "Spread of new plants' crowns"),
                num(
                    "plant_height",
                    "Height",
                    s.plant_height,
                    "Height of new plants",
                ),
            ]
        }
        _ => vec![],
    };
    if !out.is_empty() {
        out.push(json!({"kind": "sep"}));
    }
    out
}

/// A number typed in the tool bar.
pub fn set_option(state: &mut PlanState, key: &str, v: f64) {
    let s = &mut state.parts;
    match key.trim_start_matches("part.") {
        "extrusion_base" => s.extrusion_base = v.clamp(-100.0, 1000.0),
        "extrusion_height" => s.extrusion_height = v.clamp(0.01, 1000.0),
        "void_base" => s.void_base = v.clamp(-100.0, 1000.0),
        "void_height" => s.void_height = v.clamp(0.01, 1000.0),
        "face_depth" => s.face_depth = v.clamp(0.01, 100.0),
        "size_a" => s.size.0 = v.clamp(0.0, 20.0),
        "size_b" => s.size.1 = v.clamp(0.005, 20.0),
        "steps" => s.steps = v.round().clamp(1.0, 40.0) as u32,
        "terrain_thickness" => s.terrain_thickness = v.clamp(0.05, 100.0),
        "ground_height" => s.ground_height = v.clamp(-1000.0, 10000.0),
        "ground_level" => {
            s.ground_height = (v * s.level_unit - s.level_shift).clamp(-1000.0, 10000.0)
        }
        "pad_level" => {
            s.pad_level = Some((v * s.level_unit - s.level_shift).clamp(-1000.0, 10000.0))
        }
        "ground_step" => s.ground_step = v.clamp(-100.0, 100.0),
        "ground_depth" => s.ground_depth = v.clamp(0.05, 100.0),
        "ground_width" => s.ground_width = v.clamp(0.1, 100.0),
        "road_width" => s.road_width = v.clamp(0.5, 60.0),
        "path_width" => s.path_width = v.clamp(0.3, 10.0),
        "canopy" => s.canopy = v.clamp(0.2, 50.0),
        "plant_height" => s.plant_height = v.clamp(0.2, 100.0),
        _ => {}
    }
}

/// A choice clicked in the tool bar.
pub fn toggle_option(state: &mut PlanState, what: &str) {
    if what == "ground.import" {
        crate::app::groundui::request_open();
        return;
    }
    if what == "ground.parcel" {
        state.parts.ground = GroundDraw::Property;
        state.parts.pick_parcel = true;
        state.parts.path.clear();
        state.pending = None;
        state.status = Some(
            "Click inside the lot on the map or drawing under the plan. Esc cancels.".to_string(),
        );
        return;
    }
    if what == "padlevel.ground" {
        state.parts.pad_level = None;
        return;
    }
    if what == "padlevel.set" {
        // the project's zero to start from: the building's ground floor
        state.parts.pad_level.get_or_insert(0.0);
        return;
    }
    if let Some(g) = what.strip_prefix("ground.") {
        if let Some(g) = GroundDraw::ALL.into_iter().find(|x| x.label() == g) {
            state.parts.ground = g;
            state.parts.pick_parcel = false;
            state.parts.path.clear();
            state.pending = None;
            state.outline = None;
            state.status = Some(g.hint().to_string());
        }
        return;
    }
    let s = &mut state.parts;
    if let Some(p) = what.strip_prefix("profile.") {
        if let Some(p) = SweepProfile::ALL.into_iter().find(|x| x.label() == p) {
            s.library = None;
            if p != s.profile {
                s.profile = p;
                s.size = p.defaults();
            }
        }
    } else if let Some(u) = what.strip_prefix("drain.") {
        if let Some(u) = creabim_core::services::DrainUse::ALL
            .into_iter()
            .find(|x| x.label() == u)
        {
            s.drain_use = u;
        }
    } else if let Some(u) = what.strip_prefix("path.") {
        if let Some(u) = creabim_core::walk::Surface::ALL
            .into_iter()
            .find(|x| x.label() == u)
        {
            s.path_surface = u;
        }
    } else if let Some(k) = what.strip_prefix("probe.") {
        if let Some(k) = creabim_core::investigation::ProbeKind::ALL
            .into_iter()
            .find(|x| x.label() == k)
        {
            s.probe_kind = k;
        }
    } else if let Some(u) = what.strip_prefix("tank.") {
        if let Some(u) = creabim_core::services::TankUse::ALL
            .into_iter()
            .find(|x| x.label() == u)
        {
            s.tank_use = u;
        }
    } else if let Some(i) = what.strip_prefix("area.") {
        s.area = i.parse().unwrap_or(0).min(AREAS.len() - 1);
    } else if let Some(f) = what.strip_prefix("plant.") {
        if let Some(f) = PlantForm::ALL.into_iter().find(|x| x.label() == f) {
            s.plant = f;
            (s.canopy, s.plant_height) = match f {
                PlantForm::Tree => (4.0, 6.0),
                PlantForm::Conifer => (2.5, 7.0),
                PlantForm::Palm => (4.0, 7.0),
                PlantForm::Shrub => (1.2, 1.0),
            };
        }
    }
}

// ---- Properties ----

pub(crate) fn num(ui: &mut egui::Ui, text: &str, v: &mut f64, lo: f64, hi: f64) -> bool {
    crate::props::row(ui, text, |ui| {
        crate::props::fill(ui, crate::units::drag(v).range(lo..=hi)).changed()
    })
}

/// A lot checked against the building, kept while neither changes.
fn lot_check(model: &Model, s: &Site, live: bool) -> Option<creabim_geom::planning::LotCheck> {
    type Kept = Option<(
        u64,
        String,
        creabim_geom::planning::LotCheck,
        std::sync::Arc<Vec<creabim_geom::scene::SceneItem>>,
    )>;
    static KEPT: std::sync::Mutex<Kept> = std::sync::Mutex::new(None);
    let key = format!("{:?}", s.form);
    let rev = model.revision();
    let mut kept = KEPT.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((r, k, c, _)) = kept.as_ref() {
        if *r == rev && *k == key && rev != 0 {
            return Some(c.clone());
        }
    }
    // while the lot is dragged or typed in, the building is as it was: the
    // scene it is measured from is the one held, not the whole model built
    // again each frame (MU98: 200 to 270 ms frames stretching a lot)
    let scene = match kept.as_ref() {
        Some((_, _, _, sc)) if live => sc.clone(),
        _ => creabim_geom::scene::shared_scene(model),
    };
    let e = Element::new("Property", ElementKind::Site(s.clone()));
    let c = creabim_geom::planning::check(model, &e, &scene)?;
    *kept = Some((rev, key, c.clone(), scene));
    Some(c)
}

/// The Properties of a lot: what each edge is, its rules, how the
/// building keeps to them, the site's figures, and typing it by bearings.
fn lot_fields(ui: &mut egui::Ui, model: &Model, s: &mut Site) -> &'static str {
    use creabim_core::{lot, LotEdge};
    let mut label = "";
    let live = ui.ctx().dragged_id().is_some() || ui.input(|i| i.pointer.any_down());
    let check = lot_check(model, s, live);
    let north = model.project_info().map_or(0.0, |(_, p)| p.true_north);
    let imperial = crate::units::get().is_imperial();
    let SiteForm::Property {
        boundary,
        edges,
        rules,
    } = &mut s.form
    else {
        return "";
    };
    let n = boundary.len();
    // the edges: a list, and the picked edge's box
    sfold_d(ui, "lot.edges", &format!("Edges ({n})"), true, |ui| {
        let run = |d: DVec2| {
            if imperial {
                crate::units::len(d.length())
            } else {
                format!("{:.2} m", d.length())
            }
        };
        let rows: Vec<(Option<String>, String)> = (0..n)
            .map(|i| {
                let d = boundary[(i + 1) % n] - boundary[i];
                (
                    Some(run(d)),
                    format!("{} · {}", i + 1, lot::edge_kind(edges, i, n).label()),
                )
            })
            .collect();
        let mut at = picked(ui, "lot.edges").min(n.saturating_sub(1));
        let ring = boundary.clone();
        let pic = |pa: &egui::Painter, r: egui::Rect, k: usize, ink: egui::Color32| {
            edge_picture(pa, r, &ring, k, ink, (false, false));
        };
        crate::props::item_list_drawn(
            ui,
            "lot.edges",
            &rows,
            &mut at,
            rows.len().clamp(1, 6),
            (false, false, false),
            Some(&pic),
        );
        pick(ui, "lot.edges", at);
        if at >= n {
            return;
        }
        let i = at;
        let d = boundary[(i + 1) % n] - boundary[i];
        let mut kind = lot::edge_kind(edges, i, n);
        let title = format!("Edge {} · {}", i + 1, kind.label());
        let open = crate::props::sub_fold(ui, "lot.edge", &title, true);
        crate::props::fold(ui, open, |ui| {
            let mut changed = false;
            crate::props::Combo::salted("Kind", "lot-edge")
                .selected_text(kind.label())
                .show_ui(ui, |ui| {
                    for k in LotEdge::ALL {
                        changed |= ui.selectable_value(&mut kind, k, k.label()).changed();
                    }
                });
            if changed {
                *edges = (0..n).map(|j| lot::edge_kind(edges, j, n)).collect();
                edges[i] = kind;
                label = "Property edge";
            }
            if kind != LotEdge::Open {
                // its setback: its kind's, or one of its own typed here or
                // slid by its grip on the plan
                let own = rules.own_setback(i);
                let mut far = rules.setback_at(edges, i, n);
                let mut back = false;
                let typed = crate::props::row(ui, "Setback", |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        let h = ui.spacing().interact_size.y;
                        // following the rule its value reads greyed; the
                        // end column's dot says which (AMD-007, AMD-018)
                        let w = (ui.available_width() - 18.0).floor().max(40.0);
                        let r = ui
                            .scope(|ui| {
                                if own.is_none() {
                                    let v = ui.visuals_mut();
                                    v.override_text_color = Some(v.weak_text_color());
                                }
                                ui.add_sized([w, h], crate::units::drag(&mut far).range(0.0..=500.0))
                            })
                            .inner
                            .on_hover_text(if own.is_some() {
                                "This edge's own setback"
                            } else {
                                "The rule for its kind of edge, under Rules: setbacks. Type one here, or slide the grip on its setback line, to give this edge its own"
                            });
                        if end_dot(
                            ui,
                            own.is_some(),
                            "Follows the rule for its kind of edge (Rules: setbacks)",
                            "Its own setback: click to put the rule back",
                        ) {
                            back = true;
                        }
                        r.changed()
                    })
                    .inner
                });
                if back {
                    rules.set_own_setback(i, None);
                    label = "Edge setback";
                } else if typed {
                    rules.set_own_setback(i, Some(far));
                    label = "Edge setback";
                }
                let mut note = rules.note_at(i).unwrap_or("").to_string();
                let noted = crate::props::row(ui, "Note", |ui| {
                    crate::props::text_field(
                        ui,
                        egui::TextEdit::singleline(&mut note).hint_text(kind.label()),
                        f32::INFINITY,
                    )
                    .on_hover_text("Written on its setback line in place of its kind, before the distance: \"Fire lane\", \"Street B\", \"No build\"")
                    .changed()
                });
                if noted {
                    rules.set_note(i, &note);
                    label = "Edge note";
                }
                if rules.note_off(i).is_some()
                    && crate::props::action(
                        ui,
                        "",
                        "Back in place",
                        "Below its setback again, where it was before it was dragged",
                        true,
                    )
                    .clicked()
                {
                    rules.set_note_off(i, None);
                    label = "Edge note place";
                }
            }
            // as on the plat: metres to the centimetre
            crate::props::worked_out(
                ui,
                "lot.edge",
                &[
                    crate::props::Worked::new("Bearing", lot::bearing_text(lot::azimuth(d, north))),
                    crate::props::Worked::new("Length", run(d)),
                ],
            );
        });
    });
    // the rules
    // the planning rule, by kind of edge: every edge of that kind takes
    // it unless it has its own (MU98_04)
    sfold_h(ui, "lot.setbacks", "Rules: setbacks", "Setback lines and bearings are each view's: the View tab, under Site. The envelope is the 3D view's.", |ui| {
        if crate::props::check(ui, "Setbacks", &mut rules.has_setbacks, "This lot has them")
            .on_hover_text("Off, the lot has no setbacks: no setback lines, no buildable area short of the boundary, nothing checked against them. Its front, sides and rear are kept for when you turn it back on")
            .changed()
        {
            label = "Lot setbacks";
        }
        if !rules.has_setbacks {
            return;
        }
        if num(ui, "Front", &mut rules.front, 0.0, 200.0) {
            label = "Front setback";
        }
        if num(ui, "Sides", &mut rules.side, 0.0, 200.0) {
            label = "Side setback";
        }
        if num(ui, "Rear", &mut rules.rear, 0.0, 200.0) {
            label = "Rear setback";
        }
    });
    sfold_h(
        ui,
        "lot.limits",
        "Limits",
        "A limit of 0 is none. Heights are measured from the project's zero.",
        |ui| {
            if num(ui, "Height", &mut rules.height, 0.0, 1000.0) {
                label = "Height limit";
            }
            let mut pc = rules.coverage * 100.0;
            let changed = crate::props::row(ui, "Coverage", |ui| {
                crate::props::fill(
                    ui,
                    egui::DragValue::new(&mut pc)
                        .range(0.0..=100.0)
                        .speed(0.5)
                        .max_decimals(1)
                        .suffix(" %"),
                )
                .on_hover_text(
                    "How much of the lot the building may cover, seen from above; 0 is no limit",
                )
                .changed()
            });
            if changed {
                rules.coverage = pc / 100.0;
                label = "Coverage limit";
            }
            let changed = crate::props::row(ui, "Floor ratio", |ui| {
                crate::props::fill(
                ui,
                egui::DragValue::new(&mut rules.floor_ratio)
                    .range(0.0..=50.0)
                    .speed(0.01)
                    .max_decimals(2),
            )
            .on_hover_text("Floor area ratio: the floor area allowed for each unit of lot area; 0 is no limit")
            .changed()
            });
            if changed {
                label = "Floor area ratio";
            }
            let changed = crate::props::row(ui, "Angle", |ui| {
                crate::props::fill(
                ui,
                egui::DragValue::new(&mut rules.angle)
                    .range(0.0..=85.0)
                    .speed(0.5)
                    .max_decimals(1)
                    .suffix("°"),
            )
            .on_hover_text("Daylight planes: from the height below over each edge with a setback, a plane rising inward at this angle that the building stays under; 0 is none")
            .changed()
            });
            if changed {
                label = "Angle planes";
            }
            if rules.angle > 0.0 && num(ui, "Starts at", &mut rules.angle_from, 0.0, 100.0) {
                label = "Angle plane start";
            }
        },
    );
    // strips kept clear: a list, and the picked one's fields
    let title = match rules.easements.len() {
        0 => "Easements".to_string(),
        k => format!("Easements ({k})"),
    };
    sfold_d(ui, "lot.easements", &title, true, |ui| {
        let rows: Vec<(Option<String>, String)> = rules
            .easements
            .iter()
            .map(|e| (Some(crate::units::len(e.width)), e.name.clone()))
            .collect();
        let mut at = picked(ui, "lot.easements").min(rows.len().saturating_sub(1));
        let act = crate::props::item_list(
            ui,
            "lot.easements",
            &rows,
            &mut at,
            rows.len().clamp(1, 4),
            (n >= 3, false, !rows.is_empty()),
        );
        if act.add {
            // a strip along an edge, inside the lot: starts 3 m wide
            // along the rear
            let rear = (0..n)
                .find(|i| lot::edge_kind(edges, *i, n) == LotEdge::Rear)
                .unwrap_or(0);
            rules.easements.push(creabim_core::Easement {
                name: "Easement".into(),
                edge: rear,
                width: 3.0,
            });
            at = rules.easements.len() - 1;
            label = "Add easement";
        } else if act.remove && at < rules.easements.len() {
            rules.easements.remove(at);
            at = at.saturating_sub(1);
            label = "Remove easement";
        }
        pick(ui, "lot.easements", at);
        let title = rules.easements.get(at).map(|e| e.name.clone());
        let open = title
            .as_ref()
            .is_some_and(|t| crate::props::sub_fold(ui, "lot.easement", t, true));
        if let (Some(e), true) = (rules.easements.get_mut(at), open) {
            if crate::props::row(ui, "Name", |ui| {
                crate::props::text_field(ui, egui::TextEdit::singleline(&mut e.name), f32::INFINITY)
                    .changed()
            }) {
                label = "Easement name";
            }
            let mut edge = e.edge.min(n.saturating_sub(1));
            let mut changed = false;
            crate::props::Combo::salted("Along", "easement-edge")
                .selected_text(format!(
                    "{} · {}",
                    edge + 1,
                    lot::edge_kind(edges, edge, n).label()
                ))
                .show_ui(ui, |ui| {
                    for i in 0..n {
                        let t = format!("{} · {}", i + 1, lot::edge_kind(edges, i, n).label());
                        changed |= ui.selectable_value(&mut edge, i, t).changed();
                    }
                });
            if changed {
                e.edge = edge;
                label = "Easement edge";
            }
            if num(ui, "Width", &mut e.width, 0.1, 100.0) {
                label = "Easement width";
            }
        } else if title.is_none() {
            crate::props::empty(
                ui,
                "None yet: + adds a strip along an edge kept clear of building: a drain, a sewer, a right of way.",
            );
        }
    });
    // how the building keeps to them: what the rules give, worked out
    if let Some(c) = &check {
        sfold(ui, "lot.checks", "Checks", |ui| {
            let verdicts = creabim_geom::planning::verdicts(c, rules, crate::units::len);
            if verdicts.is_empty() {
                crate::props::empty(
                    ui,
                    if c.building.is_empty() {
                        "Nothing is built on the lot yet."
                    } else {
                        "Set a limit above to check it."
                    },
                );
            }
            let rows: Vec<crate::props::Worked> = verdicts
                .iter()
                .map(|v| {
                    let is = if v.allowed.is_empty() {
                        v.is.clone()
                    } else {
                        format!("{} of {}", v.is, v.allowed)
                    };
                    crate::props::Worked::new(v.rule, is).warn_if(!v.ok)
                })
                .collect();
            crate::props::worked_out(ui, "lot.checks", &rows);
            let w = crate::props::Worked::new;
            let mut stats = vec![
                w("Lot", crate::units::area(c.area)),
                w(
                    "Buildable",
                    if c.buildable.is_some() {
                        crate::units::area(c.buildable_area)
                    } else {
                        "none".into()
                    },
                ),
                w("Footprint", crate::units::area(c.footprint)),
                w("Floor area", crate::units::area(c.floor_area)),
                w("Coverage", format!("{:.1}%", c.coverage() * 100.0)),
                w("Floor ratio", format!("{:.2}", c.floor_ratio())),
            ];
            if let Some(h) = c.height {
                stats.push(w("Height", crate::units::len(h)));
            }
            stats.push(w(
                "Open",
                crate::units::area((c.area - c.footprint).max(0.0)),
            ));
            crate::props::plate(ui, "lot.stats", "STATISTICS", &stats);
        });
    }
    // typed from the deed
    sfold_d(ui, "lot.bearings", "From bearings", false, |ui| {
        let id = ui.make_persistent_id("lot-traverse");
        let mut text: String = ui.data(|d| d.get_temp(id)).unwrap_or_default();
        let edit = ui.add(
            egui::TextEdit::multiline(&mut text)
                .desired_rows(4)
                .desired_width(f32::INFINITY)
                .hint_text("N 12°30'15\" E 24.35\nS 77 29 45 E 31.20\n…"),
        );
        if edit.changed() {
            ui.data_mut(|d| d.insert_temp(id, text.clone()));
        }
        let unit = if imperial { "feet" } else { "metres" };
        crate::props::info(
            ui,
            format!("One leg a line from the first corner: a bearing (N 12°30' E, or degrees from north) then a distance in {unit}. The last corner joins the first."),
            "site",
        );
        let to_model = |d: f64| if imperial { d * 0.3048 } else { d };
        let tried =
            (!text.trim().is_empty()).then(|| lot::traverse(&text, boundary[0], north, to_model));
        match &tried {
            Some(Ok((_, miss))) => {
                crate::props::worked_out(
                    ui,
                    "lot.bearings",
                    &[crate::props::Worked::new(
                        "Misclosure",
                        crate::units::len(*miss),
                    )],
                );
            }
            Some(Err(e)) => {
                crate::props::warning(ui, e.clone(), None);
            }
            None => {}
        }
        if crate::props::action(
            ui,
            "Boundary",
            "Make from these",
            "Redraws the boundary from the first corner along the legs typed",
            matches!(tried, Some(Ok(_))),
        )
        .clicked()
        {
            if let Some(Ok((pts, _))) = tried {
                *boundary = pts;
                edges.clear();
                lot::make_ccw(boundary, edges, &mut rules.easements, &mut rules.own);
                label = "Property from bearings";
            }
        }
    });
    label
}

/// A per cent field.
pub(crate) fn percent(ui: &mut egui::Ui, text: &str, v: &mut f64, hi: f64, tip: &str) -> bool {
    crate::props::row(ui, text, |ui| {
        crate::props::fill(
            ui,
            egui::DragValue::new(v)
                .range(0.0..=hi)
                .speed(0.1)
                .max_decimals(1)
                .suffix(" %"),
        )
        .on_hover_text(tip)
        .changed()
    })
}

/// A grade as it is written: "1:20 (5.0%)".
fn grade_text(g: f64) -> String {
    if g.abs() < 1e-4 {
        "level".into()
    } else {
        format!("1:{:.0} ({:.1}%)", 1.0 / g.abs(), g.abs() * 100.0)
    }
}

/// The Properties of a road: its section, its levels along it, its
/// banks and its bays.
/// A fold in a Site panel (DS-1.0 4.4): its heading, and its rows while
/// it is open. `key` names it for as long as the app runs.
pub(crate) fn sfold(ui: &mut egui::Ui, key: &str, title: &str, add: impl FnOnce(&mut egui::Ui)) {
    sfold_d(ui, key, title, true, add);
}

/// [`sfold`] with its help as the ? on its heading (Site polish 3).
pub(crate) fn sfold_h(
    ui: &mut egui::Ui,
    key: &str,
    title: &str,
    help: &str,
    add: impl FnOnce(&mut egui::Ui),
) {
    let open = crate::props::block_help(ui, key, title, true, help, "site");
    crate::props::fold(ui, open, |ui| {
        add(ui);
        crate::props::flush_later(ui);
    });
}

/// [`sfold`], open or shut to start with. Whatever its fields worked out
/// (asked for with `props::worked_later`) is its Worked out plate, at the
/// foot of the block whose fields produce it (AMD-015, AMD-018).
pub(crate) fn sfold_d(
    ui: &mut egui::Ui,
    key: &str,
    title: &str,
    default_open: bool,
    add: impl FnOnce(&mut egui::Ui),
) {
    let open = crate::props::block(ui, key, title, default_open);
    crate::props::fold(ui, open, |ui| {
        add(ui);
        crate::props::flush_later(ui);
    });
}

/// A length on the site as Properties reads it out: metres to two
/// places, beside levels in metres (or the project's own, in feet).
pub(crate) fn metres(v: f64) -> String {
    let u = crate::units::get();
    if u.is_imperial() {
        u.length(v)
    } else {
        format!("{v:.2} m")
    }
}

/// Which row of the list `key` is picked, kept while the app runs.
pub(crate) fn picked(ui: &egui::Ui, key: &str) -> usize {
    ui.ctx()
        .data(|d| d.get_temp::<usize>(egui::Id::new(("site.pick", key))))
        .unwrap_or(0)
}

pub(crate) fn pick(ui: &egui::Ui, key: &str, i: usize) {
    ui.ctx()
        .data_mut(|d| d.insert_temp(egui::Id::new(("site.pick", key)), i));
}

/// A picture for a list row: `ring` with edge `i` drawn heavy, dashed
/// when `dashed` (cut upright with nothing holding it), heavier when
/// `wall`.
pub(crate) fn edge_picture(
    pa: &egui::Painter,
    r: egui::Rect,
    ring: &[DVec2],
    i: usize,
    ink: egui::Color32,
    (dashed, wall): (bool, bool),
) {
    let n = ring.len();
    if n < 2 {
        return;
    }
    let (lo, hi) = ring.iter().fold(
        (DVec2::splat(f64::MAX), DVec2::splat(f64::MIN)),
        |(lo, hi), q| (lo.min(*q), hi.max(*q)),
    );
    let span = (hi - lo).max_element().max(1e-6);
    let k = ((r.width().min(r.height() * 1.4) - 3.0) as f64) / span;
    let to = |q: DVec2| {
        r.center()
            + egui::vec2(
                ((q.x - (lo.x + hi.x) / 2.0) * k) as f32,
                (-(q.y - (lo.y + hi.y) / 2.0) * k) as f32,
            )
    };
    pa.add(egui::Shape::closed_line(
        ring.iter().map(|q| to(*q)).collect(),
        egui::Stroke::new(1.0, ink.gamma_multiply(0.5)),
    ));
    let (a, b) = (to(ring[i % n]), to(ring[(i + 1) % n]));
    let heavy = egui::Stroke::new(if wall { 3.0 } else { 2.2 }, ink);
    if dashed {
        pa.extend(egui::Shape::dashed_line(&[a, b], heavy, 3.0, 2.0));
    } else {
        pa.line_segment([a, b], heavy);
    }
}

/// A soil layer's picture: a box hatched as layer `i`'s turn of the
/// usual soil hatches (topsoil, clay, sand, rock...).
pub(crate) fn soil_picture(pa: &egui::Painter, r: egui::Rect, i: usize, ink: egui::Color32) {
    let st = egui::Stroke::new(1.0, ink.gamma_multiply(0.7));
    pa.rect_stroke(r, 1.0, st, egui::StrokeKind::Inside);
    let b = r.shrink(2.0);
    match i % 3 {
        0 => {
            for k in 0..4 {
                let x = b.left() + k as f32 * b.width() / 3.0;
                pa.line_segment(
                    [
                        egui::pos2(x - 3.0, b.bottom()),
                        egui::pos2(x + 1.0, b.top()),
                    ],
                    st,
                );
            }
        }
        1 => {
            for k in 1..3 {
                let y = b.top() + k as f32 * b.height() / 3.0;
                pa.line_segment([egui::pos2(b.left(), y), egui::pos2(b.right(), y)], st);
            }
        }
        _ => {
            for k in 0..3 {
                for j in 0..2 {
                    let p = egui::pos2(
                        b.left() + (k as f32 + 0.5) * b.width() / 3.0,
                        b.top() + (j as f32 + 0.5) * b.height() / 2.0,
                    );
                    pa.circle_filled(p, 0.9, st.color);
                }
            }
        }
    }
}

/// A chamber's picture: its plan mark, a circle crossed.
pub(crate) fn chamber_picture(pa: &egui::Painter, r: egui::Rect, ink: egui::Color32) {
    let c = r.center();
    let rad = r.height().min(r.width()) / 2.0 - 0.5;
    let st = egui::Stroke::new(1.0, ink);
    pa.circle_stroke(c, rad, st);
    pa.line_segment([c - egui::vec2(rad, 0.0), c + egui::vec2(rad, 0.0)], st);
    pa.line_segment([c - egui::vec2(0.0, rad), c + egui::vec2(0.0, rad)], st);
}

/// The end column's dot (AMD-007, tried on a lot's setbacks, AMD-018):
/// faint while the value follows its rule, in the accent when it is its
/// own; clicked then, it puts the rule back. True when clicked so.
pub(crate) fn end_dot(ui: &mut egui::Ui, own: bool, tip_rule: &str, tip_own: &str) -> bool {
    let h = ui.spacing().interact_size.y;
    let (r, resp) = ui.allocate_exact_size(egui::vec2(16.0, h), egui::Sense::click());
    let v = ui.visuals();
    let c = r.center();
    if own {
        let fill = v.selection.bg_fill;
        let rad = if resp.hovered() { 4.5 } else { 3.5 };
        ui.painter().circle_filled(c, rad, fill);
    } else {
        ui.painter()
            .circle_filled(c, 2.5, v.weak_text_color().gamma_multiply(0.45));
    }
    let resp = resp.on_hover_text(if own { tip_own } else { tip_rule });
    own && resp.clicked()
}

/// A bank's slope as a row: "1 in" so many.
pub(crate) fn bank(ui: &mut egui::Ui, text: &str, v: &mut f64, tip: &str) -> bool {
    crate::props::row(ui, text, |ui| {
        crate::props::fill(
            ui,
            egui::DragValue::new(v)
                .range(0.0..=20.0)
                .speed(0.05)
                .prefix("1 in "),
        )
        .on_hover_text(tip)
        .changed()
    })
}

fn road_fields(ui: &mut egui::Ui, model: &Model, s: &mut Site) -> &'static str {
    use creabim_core::road::Side;
    let mut label = "";
    let s_id = element_of(model, s);
    let s_level = s.level;
    let base = creabim_core::members::elevation(model, s.level);
    let SiteForm::Road(r) = &mut s.form else {
        return label;
    };
    // its section: what it is across
    sfold(ui, "road.section", "Section", |ui| {
        if num(ui, "Width", &mut r.width, 1.0, 60.0) {
            label = "Road width";
        }
        if percent(
            ui,
            "Fall across",
            &mut r.fall,
            10.0,
            "How it falls across to its edges so water runs off: 2.5% is usual",
        ) {
            label = "Road fall";
        }
        if crate::props::check(ui, "Crown", &mut r.crowned, "Crowned")
            .on_hover_text("On, it falls both ways from its middle. Off, it falls one way, to its right looking along it")
            .changed()
        {
            label = "Road crown";
        }
        if num(ui, "Kerbs", &mut r.kerb, 0.0, 0.5) {
            label = "Road kerbs";
        }
        if num(ui, "Footpaths", &mut r.footpath, 0.0, 10.0) {
            label = "Footpaths";
        }
        if num(ui, "Depth", &mut r.thickness, 0.05, 2.0) {
            label = "Road depth";
        }
        let mut gullies = r.gullies > 0.0;
        if crate::props::check(ui, "Gullies", &mut gullies, "Along kerbs")
            .on_hover_text("A channel by the kerb on each low side, with a gully every so often taking its water to the surface water drains")
            .changed()
        {
            r.gullies = if gullies { 20.0 } else { 0.0 };
            label = "Road gullies";
        }
        if r.gullies > 0.0 && num(ui, "Gully every", &mut r.gullies, 2.0, 100.0) {
            label = "Gully spacing";
        }
    });
    // its line: its bends and its levels along it
    sfold(ui, "road.line", "Line", |ui| {
        let mut kerb = r.kerb_radius();
        if crate::props::row(ui, "Kerb radius", |ui| {
            crate::props::fill(ui, crate::units::drag(&mut kerb))
                .on_hover_text("Each bend's radius on its inside kerb, as a bend is set out on site; its middle and its outside follow. 0 is a sharp inside corner")
                .changed()
        }) {
            r.kerb_radius = Some(kerb.clamp(0.0, 500.0));
            label = "Road bend radius";
        }
        let mut levels = r.levels;
        let mut chose = false;
        crate::props::Combo::salted("Levels", "road-levels")
            .selected_text(levels.label())
            .show_ui(ui, |ui| {
                for (l, tip) in [
                    (creabim_core::road::Levels::Points, "Each point's own level, on straight grades between them"),
                    (creabim_core::road::Levels::ground(), "The ground along its middle, eased and held to its steepest grade, so it sits on the land with shallow banks"),
                ] {
                    if ui
                        .selectable_label(levels.label() == l.label(), l.label())
                        .on_hover_text(tip)
                        .clicked()
                        && levels.label() != l.label()
                    {
                        levels = l;
                        chose = true;
                    }
                }
            });
        if chose {
            r.levels = levels;
            label = "Road levels";
        }
        if let creabim_core::road::Levels::Ground { steepest, ease } = &mut r.levels {
            let mut pc = *steepest * 100.0;
            if percent(ui, "Steepest", &mut pc, 30.0, "The steepest it may climb or fall: 12.5% (1:8) suits most drives, 8.3% (1:12) a wheelchair") {
                *steepest = (pc / 100.0).clamp(0.01, 0.3);
                label = "Road steepest";
            }
            if num(ui, "Ease over", ease, 0.0, 100.0) {
                label = "Road easing";
            }
            // its ends: on the ground, or held at a level (a drive meeting
            // the garage floor), so many mm above the road's own level
            let level_name = model
                .get(s_level)
                .map_or_else(|| "its level".to_string(), |e| e.name.clone());
            for (end, at, z_now) in [
                (
                    "Start",
                    &mut r.start_at,
                    r.path.first().map_or(0.0, |p| p.z),
                ),
                ("End", &mut r.end_at, r.path.last().map_or(0.0, |p| p.z)),
            ] {
                let mut held = at.is_some();
                if crate::props::check(ui, end, &mut held, format!("At {level_name}"))
                    .on_hover_text(format!("On, its {} is held this far above {level_name} and moves with it; the road eases from there to the ground. Off, it follows the ground there too", end.to_lowercase()))
                    .changed()
                {
                    *at = held.then_some(z_now);
                    label = "Road end level";
                }
                if let Some(v) = at {
                    let mut off = *v;
                    if crate::props::row(ui, "Above it", |ui| {
                        crate::props::fill(ui, crate::units::drag(&mut off))
                            .on_hover_text(format!(
                                "How far its {} is above {level_name}",
                                end.to_lowercase()
                            ))
                            .changed()
                    }) {
                        *v = off;
                        label = "Road end level";
                    }
                }
            }
        }
        let n = r.path.len();
        let rows: Vec<(Option<String>, String)> = (0..n)
            .map(|i| {
                let name = match i {
                    0 => "Start".to_string(),
                    _ if i + 1 == n => "End".to_string(),
                    _ => format!("Point {}", i + 1),
                };
                (Some(crate::units::len(base + r.path[i].z)), name)
            })
            .collect();
        let mut at = picked(ui, "road.points").min(n.saturating_sub(1));
        crate::props::item_list(
            ui,
            "road.points",
            &rows,
            &mut at,
            rows.len().clamp(1, 6),
            (false, false, false),
        );
        pick(ui, "road.points", at);
        if n > 2
            && r.levels.is_points()
            && crate::props::action(
                ui,
                "Between",
                "Even fall",
                "Sets the points between the start and the end on one even grade between them",
                true,
            )
            .clicked()
        {
            let total: f64 = r
                .path
                .windows(2)
                .map(|w| w[0].truncate().distance(w[1].truncate()))
                .sum();
            let (z0, z1) = (r.path[0].z, r.path[n - 1].z);
            let mut gone = 0.0;
            for i in 1..n - 1 {
                gone += r.path[i - 1].truncate().distance(r.path[i].truncate());
                r.path[i].z = z0 + (z1 - z0) * gone / total.max(1e-9);
            }
            label = "Even fall";
        }
        let steepest = r.steepest();
        let mut worked = vec![];
        if r.centre_radius() > 0.0 {
            worked.push(crate::props::Worked::new(
                "Middle radius",
                crate::units::len(r.centre_radius()),
            ));
        }
        worked.push(
            crate::props::Worked::new("Steepest", grade_text(steepest))
                .warn_if(steepest > 1.0 / 12.0),
        );
        crate::props::worked_out(ui, "road.line", &worked);
        for (i, got) in r.bends_held() {
            crate::props::warning(
                ui,
                format!(
                    "Point {}: its straights are too short for a {} kerb radius; it is built at {}.",
                    i + 1,
                    crate::units::len(r.kerb_radius()),
                    crate::units::len(got)
                ),
                None,
            );
        }
        if steepest > 1.0 / 8.0 {
            crate::props::warning(ui, "Steeper than 1:8: too steep for most drives.", None);
        } else if steepest > 1.0 / 12.0 {
            crate::props::warning(ui, "Steeper than 1:12: too steep for a wheelchair.", None);
        }
        // the picked point's own settings, at the end of the block
        if let Some(name) = rows.get(at).map(|r| r.1.clone()) {
            if crate::props::sub_fold(ui, "road.point", &name, true) {
                if let Some(c) = r.path.get_mut(at) {
                    let mut level = base + c.z;
                    let own = r.levels.is_points();
                    if !own {
                        crate::props::note(
                            ui,
                            "Its levels follow the ground. Choose At its points to set each one.",
                        );
                    }
                    if own
                        && crate::props::row(ui, "Level", |ui| {
                            crate::props::fill(ui, crate::units::drag(&mut level))
                                .on_hover_text("The crown's level at the picked point")
                                .changed()
                        })
                    {
                        c.z = level - base;
                        label = "Road level";
                    }
                }
            }
        }
    });
    // how it meets the ground
    sfold(ui, "road.ground", "Into the ground", |ui| {
        if crate::props::check(ui, "Works", &mut r.works, "Into terrain")
            .on_hover_text("The terrain under it is dug and made up to its levels, with banks out to the ground as it was. The terrain's volumes count it")
            .changed()
        {
            label = "Road into the ground";
        }
        if r.works {
            if bank(
                ui,
                "Cut bank",
                &mut r.cut,
                "Where it is dug in: how far its bank runs out for each metre it climbs",
            ) {
                label = "Road banks";
            }
            if bank(
                ui,
                "Fill bank",
                &mut r.fill,
                "Where it is made up: how far its bank runs out for each metre it falls",
            ) {
                label = "Road banks";
            }
            let (mut left, mut right) = (r.held.left(), r.held.right());
            let mut changed = false;
            crate::props::row(ui, "Hold upright", |ui| {
                changed |= ui
                    .checkbox(&mut left, "Left")
                    .on_hover_text("Its left side (looking from its start) is cut and made up upright, a retained face, instead of banked")
                    .changed();
                changed |= ui
                    .checkbox(&mut right, "Right")
                    .on_hover_text("Its right side (looking from its start) is cut and made up upright, a retained face, instead of banked")
                    .changed();
                changed
            });
            if changed {
                r.held = match (left, right) {
                    (true, true) => Side::Both,
                    (true, false) => Side::Left,
                    (false, true) => Side::Right,
                    _ => Side::None,
                };
                label = "Road held upright";
            }
            clash_note(ui, model, s_id);
        }
    });
    // its bays
    sfold(ui, "road.parking", "Parking", |ui| {
        let mut side = r.parking.side;
        let mut changed = false;
        crate::props::Combo::salted("Bays", "road-bays")
            .selected_text(side.label())
            .show_ui(ui, |ui| {
                for k in Side::ALL {
                    changed |= ui.selectable_value(&mut side, k, k.label()).changed();
                }
            });
        if changed {
            r.parking.side = side;
            label = "Parking bays";
        }
        if !r.parking.is_none() {
            let p = &mut r.parking;
            if num(ui, "Bay width", &mut p.width, 1.5, 6.0) {
                label = "Bay size";
            }
            if num(ui, "Bay length", &mut p.length, 2.0, 12.0) {
                label = "Bay size";
            }
            let changed = crate::props::row(ui, "Angle", |ui| {
                crate::props::fill(
                    ui,
                    egui::DragValue::new(&mut p.angle)
                        .range(0.0..=90.0)
                        .speed(1.0)
                        .suffix("°"),
                )
                .on_hover_text(
                    "The bays' angle to the road: 90° square on, 45° or 60° echelon, 0° parallel",
                )
                .changed()
            });
            if changed {
                label = "Bay angle";
            }
            let bays = r.bays().len();
            crate::props::worked_out(
                ui,
                "road.parking",
                &[crate::props::Worked::new("Bays fit", bays.to_string())],
            );
        }
    });
    label
}

/// The Properties fields of a part or a site element. Returns the undo
/// label of a change, or "".
pub fn properties(ui: &mut egui::Ui, model: &Model, kind: &mut ElementKind) -> &'static str {
    match kind {
        ElementKind::Part(p) => part_fields(ui, model, p),
        ElementKind::Site(s) => site_fields(ui, model, s, false),
        _ => "",
    }
}

/// The tab of its own a site element has beside Element (the Properties
/// rule, MU52; site rework, stage 5): how a terrain is shaped, how an
/// earthwork meets the ground, how a road is set out, how a drain is laid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SiteOwn {
    Ground,
    Earthwork,
    Road,
    Drain,
    Path,
    Tank,
    Property,
    Paving,
}

impl SiteOwn {
    /// What the Element tab says to point to it.
    pub fn link(self) -> &'static str {
        match self {
            SiteOwn::Ground => "Ground",
            SiteOwn::Earthwork => "Earthwork",
            SiteOwn::Road => "Road",
            SiteOwn::Drain => "Drain",
            SiteOwn::Path => "Path",
            SiteOwn::Tank => "Tank",
            SiteOwn::Property => "Property",
            SiteOwn::Paving => "Paving",
        }
    }
}

/// Which tab of its own `kind` has, if any.
pub fn site_own(kind: &ElementKind) -> Option<SiteOwn> {
    let ElementKind::Site(s) = kind else {
        return None;
    };
    match &s.form {
        SiteForm::Terrain { .. } => Some(SiteOwn::Ground),
        SiteForm::Pad { .. } => Some(SiteOwn::Earthwork),
        SiteForm::Road(_) => Some(SiteOwn::Road),
        SiteForm::Drain(_) => Some(SiteOwn::Drain),
        SiteForm::Walk(_) => Some(SiteOwn::Path),
        SiteForm::Tank(_) => Some(SiteOwn::Tank),
        SiteForm::Property { .. } => Some(SiteOwn::Property),
        SiteForm::Area { .. } => Some(SiteOwn::Paving),
        _ => None,
    }
}

/// The Parking tab of hard landscape (Mockup_Site_Car_Park REV03).
/// Returns the undo label of a change, or "".
pub fn parking_tab(ui: &mut egui::Ui, model: &Model, kind: &mut ElementKind) -> &'static str {
    let ElementKind::Site(s) = kind else {
        return "";
    };
    let layout = s.car_park_layout(model);
    let SiteForm::Area {
        boundary,
        holes,
        parking,
        ..
    } = &mut s.form
    else {
        return "";
    };
    car_park_fields(ui, parking, layout.as_ref(), boundary, holes)
}

/// A site element's own tab (see [`SiteOwn`]). Returns the undo label of
/// a change, or "".
pub fn site_tab(ui: &mut egui::Ui, model: &Model, kind: &mut ElementKind) -> &'static str {
    match kind {
        ElementKind::Site(s) => site_fields(ui, model, s, true),
        _ => "",
    }
}

fn part_fields(ui: &mut egui::Ui, model: &Model, p: &mut Part) -> &'static str {
    let mut label = "";
    if crate::props::switch(ui, &mut p.void, "Void: cuts what it meets")
        .on_hover_text("A void cuts every wall, slab, roof, site surface and solid part it meets, and is drawn only as a see-through outline in 3D")
        .changed()
    {
        label = "Void";
    }
    match &mut p.form {
        Form::Extrusion {
            outline,
            holes,
            base,
            height,
            plane,
        } => {
            let (b, h) = if plane.is_some() {
                ("Offset from the face", "Depth")
            } else {
                ("Base", "Height")
            };
            if num(ui, b, base, -100.0, 1000.0) {
                label = "Extrusion base";
            }
            if num(ui, h, height, 0.001, 1000.0) {
                label = "Extrusion height";
            }
            let mut worked = vec![crate::props::Worked::new(
                "Corners",
                outline.len().to_string(),
            )];
            if !holes.is_empty() {
                worked.push(crate::props::Worked::new("Holes", holes.len().to_string()));
            }
            if plane.is_some() {
                worked.push(crate::props::Worked::new("Drawn on", "A face"));
            } else {
                worked.push(crate::props::Worked::new("Base", crate::units::len(*base)));
            }
            crate::props::worked_later(ui, "extrusion", &worked);
            if plane.is_some() {
                crate::props::info_later(
                    ui,
                    "Drawn on a face: it runs square to the face from its offset, as deep as its depth.",
                    "",
                );
            }
        }
        Form::Sweep { profile, path } => {
            // the profile's width and height, rescaling it
            let (lo, hi) = profile.iter().fold(
                (DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY)),
                |(a, b), q| (a.min(*q), b.max(*q)),
            );
            let size = hi - lo;
            let (mut w, mut h) = (size.x, size.y);
            if num(ui, "Profile width", &mut w, 0.005, 50.0) && size.x > 1e-9 {
                let k = w / size.x;
                for q in profile.iter_mut() {
                    q.x *= k;
                }
                label = "Profile width";
            }
            if num(ui, "Profile height", &mut h, 0.005, 50.0) && size.y > 1e-9 {
                let k = h / size.y;
                for q in profile.iter_mut() {
                    q.y = lo.y + (q.y - lo.y) * k;
                }
                label = "Profile height";
            }
            if ui
                .button("Mirror profile")
                .on_hover_text("Swap the profile to the path's other side")
                .clicked()
            {
                for q in profile.iter_mut() {
                    q.x = -q.x;
                }
                profile.reverse();
                label = "Mirror profile";
            }
            match path {
                Path::Wall {
                    wall,
                    edge,
                    offset,
                    extend,
                } => {
                    if let Some(w) = model.get(*wall) {
                        if crate::props::relation(
                            ui,
                            "Follows",
                            &w.name,
                            "The wall it runs along: click to select it",
                            None,
                        )
                        .select
                        {
                            crate::props::ask_select(ui, w.id);
                        }
                    }
                    crate::props::flow(ui, |ui| {
                        ui.label("Along its");
                        for e in [WallEdge::Top, WallEdge::Base] {
                            if ui.selectable_value(edge, e, e.label()).changed() {
                                label = "Sweep edge";
                            }
                        }
                    });
                    if num(ui, "Lift", offset, -100.0, 100.0) {
                        label = "Sweep offset";
                    }
                    if num(ui, "Run past start", &mut extend[0], -50.0, 50.0)
                        | num(ui, "Run past end", &mut extend[1], -50.0, 50.0)
                    {
                        label = "Sweep ends";
                    }
                }
                Path::Points { points, closed, .. } => {
                    if crate::props::switch(ui, closed, "Closed path").changed() {
                        label = "Sweep path";
                    }
                    let mut z = points.first().map_or(0.0, |q| q.z);
                    if crate::props::num_tip(
                        ui,
                        "Above level",
                        "Height above level: how high the path runs over its level",
                        &mut z,
                        -100.0,
                        1000.0,
                    ) {
                        for q in points.iter_mut() {
                            q.z = z;
                        }
                        label = "Sweep height";
                    }
                    crate::props::worked_later(
                        ui,
                        "sweep",
                        &[crate::props::Worked::new(
                            "Points",
                            points.len().to_string(),
                        )],
                    );
                }
            }
        }
    }
    if !p.void {
        // a row in the field column, cut to the panel (MU36: it ran past)
        crate::props::row(ui, "Exports as", |ui| {
            let w = ui.available_width();
            egui::ComboBox::from_id_salt("part_ifc_class")
                .icon(crate::props::combo_icon)
                .selected_text(p.ifc_class.clone())
                .width(w)
                .truncate()
                .show_ui(ui, |ui| {
                    for c in IFC_CLASSES {
                        if ui.selectable_label(p.ifc_class == *c, *c).clicked() {
                            p.ifc_class = c.to_string();
                            label = "IFC class";
                        }
                    }
                })
                .response
                .on_hover_text(format!("The IFC class it is exported as: {}", p.ifc_class));
        });
    }
    if let Some((lo, hi)) = p.bounds(model) {
        crate::props::worked_later(
            ui,
            "part",
            &[
                crate::props::Worked::new("Bottom", crate::units::elevation(lo.z)),
                crate::props::Worked::new("Top", crate::units::elevation(hi.z)),
            ],
        );
    }
    label
}

/// A site element's fields: its Element tab, or with `own` the tab of
/// its own (see [`SiteOwn`]).
fn site_fields(ui: &mut egui::Ui, model: &Model, s: &mut Site, own: bool) -> &'static str {
    let mut label = "";

    let s_level = s.level;
    let s_id = if own && matches!(s.form, SiteForm::Pad { .. }) {
        element_of(model, s)
    } else {
        None
    };
    // the pad itself, for what is built in it (the Element tab's notes)
    let pad_id = if !own && matches!(s.form, SiteForm::Pad { .. }) {
        element_of(model, s)
    } else {
        None
    };
    let base = creabim_core::members::elevation(model, s.level);
    // the works, measured on the terrain as it stands
    let earth = creabim_geom::earth::shaped(model, s)
        .filter(|e| e.changed())
        .map(|e| (e.cut, e.fill, e.area, e.cut_layers.clone()));
    let slabs = creabim_geom::earth::building_pads(model, s).len();
    // the earthworks lying on a terrain, by name, with what each does
    let works: Vec<(ElementId, String, Option<creabim_geom::earth::WorkVolumes>)> =
        creabim_geom::earth::work_pads(model, s)
            .into_iter()
            .map(|(id, _)| {
                (
                    id,
                    model.get(id).map_or(String::new(), |e| e.name.clone()),
                    creabim_geom::earth::work_alone(model, s, id),
                )
            })
            .collect();
    // the spot levels and lines lying on a terrain, as elements
    let (on_spots, on_lines) = {
        let (a, b) = creabim_core::site::ground_objects(model, s);
        (a.len(), b.len())
    };
    // the terrain a spot level or a ground line lies on, by name
    let lies_on = |at: DVec2| {
        model.iter().find_map(|e| match &e.kind {
            ElementKind::Site(Site {
                form: SiteForm::Terrain { boundary, .. },
                ..
            }) if creabim_core::polygon::contains(boundary, at) => Some((e.id, e.name.clone())),
            _ => None,
        })
    };
    // the terrain it lies on, as a row that selects it
    let on_row = |ui: &mut egui::Ui, on: Option<(ElementId, String)>, tip: &str| match on {
        Some((id, name)) => {
            if crate::props::relation(ui, "On", &name, tip, None).select {
                crate::props::ask_select(ui, id);
            }
        }
        None => {
            crate::props::warning(
                ui,
                "It is not on a terrain, so it shapes nothing: move it onto one, or draw the ground first.",
                None,
            );
        }
    };
    // an earthwork's own volumes, on the terrain under it
    let alone: Option<creabim_geom::earth::WorkVolumes> = s.work_kind().and_then(|_| {
        let me = model.iter().find_map(|e| match &e.kind {
            ElementKind::Site(o) if o == &*s => Some(e.id),
            _ => None,
        })?;
        model.iter().find_map(|e| match &e.kind {
            ElementKind::Site(t) if matches!(t.form, SiteForm::Terrain { .. }) => {
                creabim_geom::earth::work_alone(model, t, me)
            }
            _ => None,
        })
    });
    let area_of = |ring: &[DVec2]| creabim_core::polygon::signed_area2(ring).abs() / 2.0;
    if s.is_property() {
        if own {
            return lot_fields(ui, model, s);
        }
        if let SiteForm::Property { boundary, .. } = &s.form {
            crate::props::worked_later(
                ui,
                "site.element",
                &[crate::props::Worked::new(
                    "Area",
                    crate::units::area(area_of(boundary)),
                )],
            );
        }
        return "";
    }
    // a road and a drain are set out in their own tab; Element keeps
    // what they are and how long
    if let Some(r) = s.as_road() {
        if own {
            return road_fields(ui, model, s);
        }
        let plan = r.plan();
        let length: f64 = plan.windows(2).map(|w| w[0].distance(w[1])).sum();
        crate::props::worked_later(
            ui,
            "site.element",
            &[crate::props::Worked::new("Length", metres(length))],
        );
        on_row(
            ui,
            plan.first().and_then(|p| lies_on(*p)),
            "The terrain it runs over: click to select it",
        );
        return "";
    }
    if let SiteForm::Drain(d) = &s.form {
        if own {
            return crate::servicesui::service_fields(ui, model, s);
        }
        crate::props::worked_later(
            ui,
            "site.element",
            &[crate::props::Worked::new("Length", metres(d.length()))],
        );
        return "";
    }
    if s.is_service() {
        return if own {
            crate::servicesui::service_fields(ui, model, s)
        } else {
            crate::servicesui::tank_size(ui, s)
        };
    }
    if let SiteForm::Walk(w) = &s.form {
        if own {
            return crate::servicesui::walk_fields(ui, model, s);
        }
        crate::props::worked_later(
            ui,
            "site.element",
            &[crate::props::Worked::new("Length", metres(w.length()))],
        );
        return "";
    }
    if own
        && !matches!(
            s.form,
            SiteForm::Terrain { .. } | SiteForm::Pad { .. } | SiteForm::Area { .. }
        )
    {
        return "";
    }
    if s.as_probe().is_some() {
        return crate::investigationui::probe_fields(ui, model, s);
    }
    match &mut s.form {
        SiteForm::Terrain {
            boundary,
            top,
            thickness,
            points,
            contour: _,
            data,
        } => {
            let shaped = !points.is_empty() || !data.lines.is_empty();
            if !own {
                if num(ui, "Top", top, -100.0, 1000.0) {
                    label = "Terrain top";
                }
                if num(ui, "Depth", thickness, 0.05, 100.0) {
                    label = "Terrain depth";
                }
                crate::props::worked_later(
                    ui,
                    "site.element",
                    &[crate::props::Worked::new(
                        "Area",
                        crate::units::area(area_of(boundary)),
                    )],
                );
            } else {
                let count = |n: usize, one: &str| match n {
                    1 => format!("1 {one}"),
                    n => format!("{n} {one}s"),
                };
                // what shapes it
                sfold(ui, "ground.survey", "Survey", |ui| {
                    let (contours, breaks) = data.lines.iter().fold((0, 0), |(c, b), l| {
                        if l.kind == GroundLineKind::Contour {
                            (c + 1, b)
                        } else {
                            (c, b + 1)
                        }
                    });
                    if shaped {
                        crate::props::worked_later(
                            ui,
                            "site.facts",
                            &[crate::props::Worked::new(
                                "From a file",
                                count(points.len(), "point"),
                            )],
                        );
                        if contours + breaks > 0 {
                            crate::props::worked_later(
                                ui,
                                "site.facts",
                                &[crate::props::Worked::new(
                                    "Lines",
                                    match (contours, breaks) {
                                        (c, 0) => format!("{c} contour"),
                                        (0, b) => format!("{b} break"),
                                        (c, b) => format!("{c} cont., {b} break"),
                                    },
                                )],
                            );
                        }
                    }
                    if on_spots + on_lines > 0 {
                        crate::props::worked_later(
                            ui,
                            "site.facts",
                            &[crate::props::Worked::new(
                                "Drawn on it",
                                format!(
                                    "{}, {}",
                                    count(on_spots, "level"),
                                    count(on_lines, "line")
                                ),
                            )],
                        );
                    }
                    if !shaped && on_spots + on_lines == 0 {
                        crate::props::worked_later(
                            ui,
                            "site.facts",
                            &[crate::props::Worked::new(
                                "Flat at",
                                crate::units::elevation(base + *top),
                            )],
                        );
                    }
                    if crate::props::action(
                        ui,
                        "File",
                        "Import…",
                        "Bring in a survey: a table of points (Excel, CSV, text or pasted) or a DXF drawing with contours",
                        true,
                    )
                    .clicked()
                    {
                        crate::app::groundui::request_open();
                    }
                    if shaped
                        && crate::props::action(
                            ui,
                            "",
                            "Clear survey",
                            "Remove the survey's spot heights and lines. Spot levels and contours drawn on the plan stay: delete those there",
                            true,
                        )
                        .clicked()
                    {
                        points.clear();
                        data.lines.clear();
                        label = "Clear ground data";
                    }
                });
                sfold_h(ui, "ground.surface", "Surface", "How it is shown (contours, heights, shading, water) is each view's: the View tab, under Site.", |ui| {
                    let mut smooth = !data.faceted;
                    if crate::props::check(ui, "Smooth", &mut smooth, "Curved")
                        .on_hover_text("On, the ground curves through every surveyed height and its contours flow. Off, it is the survey's flat triangles between the points, as surveyed. Plans, sections, 3D and heights all read the one shown")
                        .changed()
                    {
                        data.faceted = !smooth;
                        label = "Smooth ground";
                    }
                    if crate::props::check(ui, "Edge", &mut data.survey, "Follows data")
                        .on_hover_text("On, the outline's corners take their height from the surveyed heights nearest them, as a survey's should. Off, they stay at the Top, for a flat ground with a mound or a dip in it")
                        .changed()
                    {
                        label = "Terrain edge";
                    }
                });
                // what the design does to it
                let title = match works.len() {
                    0 => "Works".to_string(),
                    n => format!("Works ({n})"),
                };
                sfold_d(ui, "ground.works", &title, true, |ui| {
                    let mut gone_pad = None;
                    for (i, pad) in data.pads.iter_mut().enumerate() {
                        let mut level = base + pad.level;
                        let mut changed = false;
                        let dig = pad.depth > 0.0;
                        let name = if dig {
                            format!("Dig {}", i + 1)
                        } else {
                            format!("Pad {}", i + 1)
                        };
                        crate::props::row(ui, &name, |ui| {
                            crate::props::flow(ui, |ui| {
                                changed |= if dig {
                                    ui.add(crate::units::drag(&mut pad.depth).range(0.05..=100.0))
                                        .on_hover_text(
                                            "How deep it is dug below the ground, all over",
                                        )
                                        .changed()
                                } else {
                                    ui.add(crate::units::drag(&mut level))
                                        .on_hover_text("The pad's level")
                                        .changed()
                                };
                                if crate::props::icon_button(
                                    ui,
                                    "x",
                                    "Remove this pad: the ground goes back",
                                )
                                .clicked()
                                {
                                    gone_pad = Some(i);
                                }
                            });
                        });
                        crate::props::row(ui, if dig { "Sides" } else { "Banks" }, |ui| {
                            crate::props::flow(ui, |ui| {
                                changed |= ui
                                        .add(
                                            egui::DragValue::new(&mut pad.cut)
                                                .range(0.0..=20.0)
                                                .speed(0.05)
                                                .prefix(if dig { "1 in " } else { "dug 1 in " }),
                                        )
                                        .on_hover_text("Where it is dug in: how far its bank runs out for each metre it climbs. 0 is a retained, upright face")
                                        .changed();
                                if dig {
                                    return;
                                }
                                changed |= ui
                                        .add(
                                            egui::DragValue::new(&mut pad.fill)
                                                .range(0.0..=20.0)
                                                .speed(0.05)
                                                .prefix("made up 1 in "),
                                        )
                                        .on_hover_text("Where the pad is made up: how far its bank runs out for each metre it falls. 0 is a retained, upright face")
                                        .changed();
                            });
                        });
                        if changed {
                            pad.level = level - base;
                            label = if dig { "Dig" } else { "Pad" };
                        }
                    }
                    if let Some(i) = gone_pad {
                        data.pads.remove(i);
                        label = "Remove pad";
                    }
                    // the earthworks lying on it, elements of their own: a list,
                    // and the picked one's volumes and link (Mockup_Site_Works)
                    if works.is_empty() && data.pads.is_empty() {
                        crate::props::empty(
                            ui,
                            "No pads, pits or trenches yet. Draw them with the Site tools.",
                        );
                    } else if !works.is_empty() {
                        let rows: Vec<(Option<String>, String)> = works
                            .iter()
                            .map(|(_, name, v)| {
                                let value = match v {
                                    Some(v) if v.fill > 0.05 => {
                                        format!("−{:.1} +{:.1}", v.cut, v.fill)
                                    }
                                    Some(v) => format!("−{:.1}", v.cut),
                                    None => String::new(),
                                };
                                (Some(value), name.clone())
                            })
                            .collect();
                        let kinds: Vec<&'static str> = works
                            .iter()
                            .map(|(id, _, _)| match model.get(*id).map(|e| &e.kind) {
                                Some(ElementKind::Site(w)) => match w.work_kind() {
                                    Some(creabim_core::WorkKind::Pit) => "pit",
                                    Some(creabim_core::WorkKind::Trench) => "trench",
                                    _ => "pad",
                                },
                                _ => "pad",
                            })
                            .collect();
                        let pic =
                            |pa: &egui::Painter, r: egui::Rect, k: usize, ink: egui::Color32| {
                                crate::svgicon::paint(pa, r.shrink(0.5), kinds[k], ink);
                            };
                        let mut at = picked(ui, "ground.works").min(works.len() - 1);
                        let act = crate::props::item_list_drawn(
                            ui,
                            "ground.works",
                            &rows,
                            &mut at,
                            rows.len().clamp(1, 6),
                            (false, false, false),
                            Some(&pic),
                        );
                        pick(ui, "ground.works", at);
                        if act.opened {
                            crate::props::ask_select(ui, works[at].0);
                        }
                    }
                    // works that leave the ground where it was have nothing to total
                    match earth.clone().filter(|e| e.0 + e.1 > 0.005) {
                        Some((cut, fill, area, layers)) => {
                            let u = crate::units::get();
                            crate::props::worked_later(
                                ui,
                                "site.facts",
                                &[crate::props::Worked::new("Dug out", u.volume(cut))],
                            );
                            crate::props::worked_later(
                                ui,
                                "site.facts",
                                &[crate::props::Worked::new("Made up", u.volume(fill))],
                            );
                            crate::props::worked_later(
                                ui,
                                "site.facts",
                                &[crate::props::Worked::new("Over", u.area(area))],
                            );
                            if cut > fill {
                                crate::props::worked_later(
                                    ui,
                                    "site.facts",
                                    &[crate::props::Worked::new("Take away", u.volume(cut - fill))],
                                );
                            } else {
                                crate::props::worked_later(
                                    ui,
                                    "site.facts",
                                    &[crate::props::Worked::new("Bring in", u.volume(fill - cut))],
                                );
                            }
                            let _ = layers;
                        }
                        None if !data.pads.is_empty() || data.under_buildings => {
                            crate::props::empty(ui, "Nothing dug or made up: the ground is already where the works want it.");
                        }
                        None => {}
                    }
                    if crate::props::check(ui, "Buildings", &mut data.under_buildings, "Dig under")
                        .on_hover_text("The ground is dug out under every slab that stands on this terrain, down to the slab's underside, with upright sides. Where the ground is already below a slab it is left alone")
                        .changed()
                    {
                        label = "Dig under buildings";
                    }
                    if data.under_buildings {
                        crate::props::worked_later(
                            ui,
                            "site.facts",
                            &[crate::props::Worked::new("Slabs on it", slabs.to_string())],
                        );
                    }
                    // the terrain's totals, then (AMD-003: a sub-fold is last in
                    // its block) the picked work: its link, then its volumes
                    crate::props::flush_later(ui);
                    if !works.is_empty() {
                        let u = crate::units::get();
                        let at = picked(ui, "ground.works").min(works.len() - 1);
                        let (id, name, v) = &works[at];
                        if crate::props::sub_fold(ui, "ground.work", name, true) {
                            if crate::props::relation(
                                ui,
                                "Element",
                                name,
                                "Select it on the plan: its values are edited there. A double-click on its row does the same",
                                None,
                            )
                            .select
                            {
                                crate::props::ask_select(ui, *id);
                            }
                            if let Some(v) = v {
                                let w = crate::props::Worked::new;
                                let mut rows = vec![w("Dug out", u.volume(v.cut))];
                                if v.fill > 0.005 {
                                    rows.push(w("Made up", u.volume(v.fill)));
                                }
                                rows.push(w("Over", u.area(v.area)));
                                crate::props::worked_out(ui, "ground.work", &rows);
                            }
                        }
                    }
                });
                // what lies under it: a list, and the picked layer's fields
                let title = match data.strata.len() {
                    0 => "Soil layers".to_string(),
                    n => format!("Soil layers ({n})"),
                };
                sfold_d(ui, "ground.soil", &title, true, |ui| {
                    let dug: Vec<f64> = earth.as_ref().map_or(vec![], |e| e.3.clone());
                    let rows: Vec<(Option<String>, String)> = data
                        .strata
                        .iter()
                        .map(|l| (Some(crate::units::len(l.thickness)), l.name.clone()))
                        .collect();
                    let mut at = picked(ui, "ground.soil").min(rows.len().saturating_sub(1));
                    let pic = |pa: &egui::Painter, r: egui::Rect, k: usize, ink: egui::Color32| {
                        soil_picture(pa, r, k, ink);
                    };
                    let act = crate::props::item_list_drawn(
                        ui,
                        "ground.soil",
                        &rows,
                        &mut at,
                        rows.len().clamp(1, 5),
                        (true, false, !rows.is_empty()),
                        Some(&pic),
                    );
                    if act.add {
                        let name = match data.strata.len() {
                            0 => "Topsoil",
                            1 => "Subsoil",
                            _ => "Layer",
                        };
                        data.strata.push(creabim_core::Stratum {
                            name: name.to_string(),
                            thickness: if data.strata.is_empty() { 0.3 } else { 1.0 },
                        });
                        at = data.strata.len() - 1;
                        label = "Add soil layer";
                    } else if act.remove && at < data.strata.len() {
                        data.strata.remove(at);
                        at = at.saturating_sub(1);
                        label = "Remove soil layer";
                    }
                    pick(ui, "ground.soil", at);
                    let title = data.strata.get(at).map(|l| l.name.clone());
                    let open = title
                        .as_ref()
                        .is_some_and(|t| crate::props::sub_fold(ui, "ground.layer", t, true));
                    if let (Some(layer), true) = (data.strata.get_mut(at), open) {
                        if crate::props::row(ui, "Name", |ui| {
                            crate::props::text_field(
                                ui,
                                egui::TextEdit::singleline(&mut layer.name),
                                f32::INFINITY,
                            )
                            .on_hover_text("What this layer is: topsoil, clay, rock…")
                            .changed()
                        }) {
                            label = "Soil layer";
                        }
                        if num(ui, "Thickness", &mut layer.thickness, 0.01, 100.0) {
                            label = "Soil layer";
                        }
                        if let Some(v) = dug.get(at).filter(|v| **v > 0.005) {
                            crate::props::worked_later(
                                ui,
                                "site.facts",
                                &[crate::props::Worked::new(
                                    "Dug",
                                    crate::units::get().volume(*v),
                                )],
                            );
                        }
                    } else if title.is_none() {
                        crate::props::empty(ui, "None yet: + adds a layer from the ground down. Sections draw its foot and its name.");
                    }
                });
            }
        }
        SiteForm::Road(_)
        | SiteForm::Drain(_)
        | SiteForm::Tank(_)
        | SiteForm::Walk(_)
        | SiteForm::Investigation(_) => {}
        SiteForm::Spot { position, height } => {
            let mut level = *height + base;
            if crate::sitelevel::field(ui, model, "Level", &mut level) {
                *height = level - base;
                label = "Spot level";
            }
            on_row(
                ui,
                lies_on(*position),
                "The terrain whose ground passes through it: click to select it",
            );
            crate::props::info(ui, "Drag it to move it.", "site");
        }
        SiteForm::Line(l) => {
            let contour = l.kind == GroundLineKind::Contour;
            if contour {
                let mut level = l.points.first().map_or(0.0, |p| p.z) + base;
                if crate::sitelevel::field(ui, model, "Level", &mut level) {
                    l.points.iter_mut().for_each(|p| p.z = level - base);
                    label = "Contour level";
                }
            }
            let n = l.points.len();
            let last = if l.closed { n } else { n.saturating_sub(1) };
            let length: f64 = (0..last)
                .map(|i| {
                    l.points[i]
                        .truncate()
                        .distance(l.points[(i + 1) % n].truncate())
                })
                .sum();
            crate::props::worked_later(
                ui,
                "site.element",
                &[crate::props::Worked::new("Length", metres(length))],
            );
            if crate::props::check(ui, "Closed", &mut l.closed, "Joined up")
                .on_hover_text("Joined back to its first point, as a contour round a hill is")
                .changed()
            {
                label = "Ground line closed";
            }
            on_row(
                ui,
                l.points.iter().find_map(|p| lies_on(p.truncate())),
                if contour {
                    "The terrain whose ground passes through it smoothly: click to select it"
                } else {
                    "The terrain whose ground folds along it: click to select it"
                },
            );
            crate::props::info(
                ui,
                "Drag it to move it; double-click it to reshape it.",
                "site",
            );
        }
        SiteForm::Pad {
            boundary,
            top,
            base: course,
            cut,
            fill,
            depth,
            path,
            width,
            only_cut,
            retained,
            walls,
            bend,
        } => {
            let trench = !path.is_empty();
            let dug = *depth > 0.0;
            // the terrain it is worked into: the one under its first corner
            let under = boundary.first().and_then(|at| {
                model.iter().find_map(|e| match &e.kind {
                    ElementKind::Site(Site {
                        form: SiteForm::Terrain { boundary, .. },
                        ..
                    }) if creabim_core::polygon::contains(boundary, *at) => {
                        Some((e.id, e.name.clone()))
                    }
                    _ => None,
                })
            });
            if !own {
                if dug {
                    if num(ui, "Depth", depth, 0.05, 100.0) {
                        label = "Dig depth";
                    }
                } else {
                    // as it is written on drawings, from the datum or the
                    // project's zero
                    let mut level = *top + base;
                    if crate::sitelevel::field(ui, model, "Level", &mut level) {
                        *top = level - base;
                        label = "Pad level";
                    }
                    if tied_offset(ui, model, s_level, top) {
                        label = "Pad level";
                    }
                }
                if trench {
                    let line = creabim_core::road::round_plan(path, *bend);
                    let length: f64 = line.windows(2).map(|w| w[0].distance(w[1])).sum();
                    crate::props::worked_later(
                        ui,
                        "site.element",
                        &[crate::props::Worked::new("Length", metres(length))],
                    );
                } else {
                    crate::props::worked_later(
                        ui,
                        "site.element",
                        &[crate::props::Worked::new(
                            "Area",
                            crate::units::area(area_of(boundary)),
                        )],
                    );
                }
                match &under {
                    Some((id, name)) => {
                        if crate::props::relation(ui, "On", name, "The terrain it is worked into: click to select it", None).select {
                            crate::props::ask_select(ui, *id);
                        }
                    }
                    None => if crate::props::warning(
                        ui,
                        "It is not on a terrain, so it shapes nothing: move it onto one, or draw the ground first.",
                        None,
                    ) {  } ,
                }
                // what is built in it: blue when all is well, orange with
                // the level that puts it right when not (MU140)
                if !dug && !trench {
                    if let Some(fix) = pad_notes_ui(ui, model, pad_id, base) {
                        *top = fix - base;
                        label = "Pad level";
                    }
                }
                crate::props::info(
                    ui,
                    if trench {
                        "Drag it to move it; double-click it to reshape its line."
                    } else {
                        "Drag it to move it; double-click it to reshape its outline."
                    },
                    "site",
                );
            }
            if own && trench {
                // how it is set out, as a road's width and radius are
                let mut relaid = "";
                sfold(ui, "work.line", "Line", |ui| {
                    if num(ui, "Width", width, 0.1, 50.0) {
                        relaid = "Trench width";
                    }
                    if num(ui, "Radius", bend, 0.0, 500.0) {
                        relaid = "Trench bend radius";
                    }
                });
                if !relaid.is_empty() {
                    // its outline follows its line, its bends and its width
                    let mut laid = creabim_core::site::trench_round(path, *bend, *width);
                    if creabim_core::polygon::signed_area2(&laid) < 0.0 {
                        laid.reverse();
                    }
                    *boundary = laid;
                    label = relaid;
                }
            }
            // what it digs and makes up on its own, worked out: at the foot
            // of the block whose fields produce it (AMD-018)
            let vols: Vec<crate::props::Worked> = match alone {
                Some(v) => {
                    let u = crate::units::get();
                    let mut w = vec![crate::props::Worked::new("Dug out", u.volume(v.cut))];
                    if !dug {
                        w.push(crate::props::Worked::new("Made up", u.volume(v.fill)));
                    }
                    w.push(crate::props::Worked::new("Over", u.area(v.area)));
                    w
                }
                None => vec![],
            };
            if own {
                sfold(
                    ui,
                    if dug { "work.sides" } else { "work.banks" },
                    if dug { "Sides" } else { "Banks" },
                    |ui| {
                        if bank(
                        ui,
                        if dug { "Lie back" } else { "Cut bank" },
                        cut,
                        "Where it is dug in: how far its bank runs out for each metre it climbs. 0 is an upright face",
                    ) {
                        label = "Bank";
                    }
                        if !dug
                        && bank(
                            ui,
                            "Fill bank",
                            fill,
                            "Where it is made up: how far its bank runs out for each metre it falls. 0 is an upright face",
                        )
                    {
                        label = "Bank";
                    }
                        if dug {
                            crate::props::worked_later(ui, "work.volumes", &vols);
                        }
                    },
                );
                if !dug {
                    sfold(ui, "work.platform", "Platform", |ui| {
                        if num(ui, "Base", course, 0.0, 5.0) {
                            label = "Pad base course";
                        }
                        if crate::props::check(ui, "Digs", only_cut, "Only digs")
                            .on_hover_text("On, it digs where the ground is above it and leaves lower ground as it is, as the hole for a floor does. Off, it also makes the ground up to its level")
                            .changed()
                        {
                            label = "Pad only digs";
                        }
                        crate::props::worked_later(ui, "work.volumes", &vols);
                    });
                }
            }
            if own && !dug {
                clash_note(ui, model, s_id);
            }
            if own && !trench {
                // its edges: one cut upright, and held by a wall, at a time
                let held = retained.len();
                let title = match held {
                    0 => "Edges".to_string(),
                    n => format!("Edges ({n} held)"),
                };
                sfold_d(ui, "work.edges", &title, true, |ui| {
                    let n = boundary.len();
                    let c = boundary.iter().copied().sum::<DVec2>() / n.max(1) as f64;
                    let side = |i: usize| {
                        let (a, b) = (boundary[i], boundary[(i + 1) % n]);
                        let out = ((a + b) / 2.0 - c).normalize_or_zero();
                        if out.y.abs() >= out.x.abs() {
                            if out.y > 0.0 {
                                "north"
                            } else {
                                "south"
                            }
                        } else if out.x > 0.0 {
                            "east"
                        } else {
                            "west"
                        }
                    };
                    let mut at = picked(ui, "work.edges").min(n.saturating_sub(1));
                    let rows: Vec<(Option<String>, String)> = (0..n)
                        .map(|i| {
                            let (a, b) = (boundary[i], boundary[(i + 1) % n]);
                            (
                                Some(metres(a.distance(b))),
                                format!("{} · {}", i + 1, side(i)),
                            )
                        })
                        .collect();
                    let ring = boundary.clone();
                    let looks: Vec<(bool, bool)> = (0..n)
                        .map(|i| {
                            let up = retained.contains(&i);
                            let wall = walls.iter().any(|w| w.0 == i && model.get(w.1).is_some());
                            // dashed when cut upright with nothing holding it
                            (up && !wall, wall)
                        })
                        .collect();
                    let pic = |pa: &egui::Painter, r: egui::Rect, k: usize, ink: egui::Color32| {
                        edge_picture(pa, r, &ring, k, ink, looks[k]);
                    };
                    crate::props::item_list_drawn(
                        ui,
                        "work.edges",
                        &rows,
                        &mut at,
                        rows.len().clamp(1, 6),
                        (false, false, false),
                        Some(&pic),
                    );
                    pick(ui, "work.edges", at);
                    if at < n {
                        let title = format!("Edge {} · {}", at + 1, side(at));
                        let open = crate::props::sub_fold(ui, "work.edge", &title, true);
                        crate::props::fold(ui, open, |ui| {
                            let mut on = retained.contains(&at);
                            if crate::props::check(ui, "Upright", &mut on, "Cut upright")
                                .on_hover_text("On, this edge is cut upright: no bank, and the ground beyond it stays as it is. The other edges keep their banks. With no wall the ground simply stands, as rock does")
                                .changed()
                            {
                                if on {
                                    retained.push(at);
                                    retained.sort_unstable();
                                } else {
                                    retained.retain(|e| *e != at);
                                    // its wall goes with it
                                    if walls.iter().any(|w| w.0 == at) {
                                        ask_wall(ui, at, false);
                                    }
                                }
                                label = "Upright edge";
                            }
                            if on {
                                let wall = walls
                                    .iter()
                                    .find(|w| w.0 == at)
                                    .and_then(|w| model.get(w.1));
                                let mut has = wall.is_some();
                                if crate::props::check(ui, "Retain", &mut has, "With a wall")
                                    .on_hover_text("A wall of its own along this edge, outside the outline with its face on the edge, from the floor to just above the ground it holds. The ground is cut at its back. It is an ordinary wall: select it to move, stretch or thicken it")
                                    .changed()
                                {
                                    ask_wall(ui, at, has);
                                }
                                if let Some(w) = wall {
                                    let r = crate::props::relation(
                                        ui,
                                        "Wall",
                                        &w.name,
                                        "The wall holding this edge: click to select it",
                                        Some("Let go of the wall: it stays as an ordinary wall and no longer follows this edge. Untick Retain to remove it instead"),
                                    );
                                    if r.select {
                                        crate::props::ask_select(ui, w.id);
                                    }
                                    if r.let_go {
                                        walls.retain(|k| k.0 != at);
                                        label = "Let go of wall";
                                    }
                                }
                            }
                        });
                    }
                });
            }
        }
        SiteForm::Area {
            boundary,
            holes,
            top,
            thickness,
            pattern,
            parking,
        } => {
            if !own {
                if num(ui, "Top", top, -100.0, 1000.0) {
                    label = "Area top";
                }
                if num(ui, "Depth", thickness, 0.005, 10.0) {
                    label = "Area depth";
                }
                if parking.is_some() {
                    crate::props::note(
                        ui,
                        "A car park: its bays and aisles are in the Parking tab.",
                    );
                }
                let a = area_of(boundary) - holes.iter().map(|h| area_of(h)).sum::<f64>();
                crate::props::worked_later(
                    ui,
                    "site.element",
                    &[crate::props::Worked::new("Area", crate::units::area(a))],
                );
                return label;
            }
            {
                let choices = [
                    Pattern::None,
                    Pattern::Units {
                        size: DVec2::new(0.6, 0.4),
                        stagger: true,
                        angle: 0.0,
                    },
                    Pattern::Lines {
                        spacing: 0.15,
                        angle: 0.0,
                    },
                    Pattern::Stipple { spacing: 0.12 },
                ];
                // four choices: a drop-down
                crate::props::Combo::new("Pattern")
                    .selected_text(pattern.label())
                    .show_ui(ui, |ui| {
                        for c in choices {
                            let on = pattern.label() == c.label();
                            if ui.selectable_label(on, c.label()).clicked() && !on {
                                *pattern = c;
                                label = "Area pattern";
                            }
                        }
                    });
            }
            let deg = |ui: &mut egui::Ui, v: &mut f64| {
                crate::props::row(ui, "Angle", |ui| {
                    crate::props::fill(ui, egui::DragValue::new(v).speed(1.0).suffix("°")).changed()
                })
            };
            match pattern {
                Pattern::None => {}
                Pattern::Units {
                    size,
                    stagger,
                    angle,
                } => {
                    if num(ui, "Length", &mut size.x, 0.01, 10.0)
                        | num(ui, "Width", &mut size.y, 0.01, 10.0)
                    {
                        label = "Pattern size";
                    }
                    if crate::props::check(ui, "Bond", stagger, "Staggered")
                        .on_hover_text("Each course set half a unit along from the last")
                        .changed()
                    {
                        label = "Pattern bond";
                    }
                    if deg(ui, angle) {
                        label = "Pattern angle";
                    }
                }
                Pattern::Lines { spacing, angle } => {
                    if num(ui, "Spacing", spacing, 0.01, 10.0) {
                        label = "Pattern spacing";
                    }
                    if deg(ui, angle) {
                        label = "Pattern angle";
                    }
                }
                Pattern::Stipple { spacing } => {
                    if num(ui, "Spacing", spacing, 0.01, 10.0) {
                        label = "Pattern spacing";
                    }
                }
            }
        }
        SiteForm::Plant {
            position,
            base: b,
            form,
            canopy,
            height,
            tree,
        } => {
            // what it is and where (the Properties rule, MU52): where it
            // stands and its overall size; its form, crown and trunk are
            // how it is made, in the Tree tab
            if num(ui, "X", &mut position.x, -1e4, 1e4) | num(ui, "Y", &mut position.y, -1e4, 1e4) {
                label = "Move plant";
            }
            if num(ui, "Base", b, -100.0, 1000.0) {
                label = "Plant base";
            }
            if num(ui, "Height", height, 0.1, 100.0) {
                label = "Plant height";
            }
            let width = if *form == PlantForm::Shrub {
                "Width"
            } else {
                "Crown"
            };
            if num(ui, width, canopy, 0.1, 60.0) {
                label = "Crown width";
            }
            // the crown starts below the top
            if let Some(t) = tree.as_deref_mut() {
                t.crown_base = t.crown_base.min((*height - 0.3).max(0.0));
            }
        }
        SiteForm::Property { .. } => {}
    }
    label
}

/// A whole number in a row.
fn whole(ui: &mut egui::Ui, label: &str, v: &mut u32, most: u32, tip: &str) -> bool {
    crate::props::row(ui, label, |ui| {
        crate::props::fill(ui, egui::DragValue::new(v).range(0..=most).speed(0.1))
            .on_hover_text(tip)
            .changed()
    })
}

/// A hard landscape area's car park: whether it is one, its bays and
/// aisles, its special bays, what the permit asks and how it counts
/// (Mockup_Site_Car_Park REV02, P1). Returns the undo label of a change.
fn car_park_fields(
    ui: &mut egui::Ui,
    parking: &mut Option<creabim_core::carpark::CarPark>,
    layout: Option<&creabim_core::carpark::Layout>,
    boundary: &[DVec2],
    holes: &[Vec<DVec2>],
) -> &'static str {
    use creabim_core::carpark::{Aisle, BayKind, CarPark, Requirement};
    let mut label = "";
    let mut on = parking.is_some();
    if crate::props::check(ui, "Car park", &mut on, "Lay bays in it")
        .on_hover_text("On, it lays its own aisles, rows and bays inside its outline and counts them. Draw a road up to its edge for its entrance, and a path for the door its accessible bays go nearest")
        .changed()
    {
        *parking = on.then(CarPark::default);
        label = "Car park";
    }
    let Some(cp) = parking else {
        return label;
    };
    if crate::props::check(ui, "Ground", &mut cp.ground, "Worked to it")
        .on_hover_text("On, the ground under the car park is dug or made up to its underside, with banks where it meets the ground round it, so it lies in its own bed on any slope. Off, it sits on the ground as it is: for a car park on a pad of its own")
        .changed()
    {
        label = "Car park ground";
    }
    sfold_h(ui, "carpark.bays", "Bays", "The size of a bay and the width of the aisles every row follows. Rows run along the car park's longest edge, or across it.", |ui| {
    if num(ui, "Width", &mut cp.bay_width, 0.8, 6.0) | num(ui, "Length", &mut cp.bay_length, 1.5, 20.0) {
        label = "Bay size";
    }
    if num(ui, "Aisle", &mut cp.aisle, 2.0, 20.0) {
        label = "Aisle width";
    }
    if crate::props::check(ui, "Rows", &mut cp.across, "Across its longest edge")
        .on_hover_text("Off, the rows run along its longest edge; on, across it")
        .changed()
    {
        label = "Rows' direction";
    }
    if crate::props::check(ui, "Numbers", &mut cp.numbers, "In plan")
        .on_hover_text("Each bay's number written in it in plan, as the bay schedule lists them")
        .changed()
    {
        label = "Bay numbers";
    }
    // its aisles: its own, or lines drawn and bent by hand
    });
    sfold_h(ui, "carpark.aisles", "Aisles", "It lays its own aisles, and lays them again when its outline changes. Draw my own turns them into lines of yours: double-click the car park in a plan to drag their points, pull a bend from the middle of a stretch, or delete a point. The rows hang off them, and each row's ends can be slid along its aisle.", |ui| {
    if cp.aisles.is_empty() {
        if crate::props::action(
            ui,
            "",
            "Draw my own",
            "Its aisles become lines of your own, as they stand now. Double-click the car park in a plan to drag their points, pull a bend out of the middle of a stretch, or delete a point. The rows hang off them",
            true,
        )
        .clicked()
        {
            cp.take_aisles(boundary, holes);
            label = "Aisles by hand";
        }
    } else {
        let mut gone: Option<usize> = None;
        for (k, a) in cp.aisles.iter_mut().enumerate() {
            let name = format!("Aisle {}", k + 1);
            if !crate::props::sub_fold(ui, &format!("carpark.aisle.{k}"), &name, k == 0) {
                continue;
            }
            let mut width = a.width.unwrap_or(cp.aisle);
            if num(ui, "Width", &mut width, 2.0, 20.0) {
                a.width = Some(width);
                label = "Aisle width";
            }
            if crate::props::check(ui, "Flow", &mut a.one_way, "One-way")
                .on_hover_text("One-way, from its first point to its last, with an arrow in plan. Off, two-way")
                .changed()
            {
                label = "Aisle flow";
            }
            if a.path.len() > 2 && num(ui, "Kerb radius", &mut a.radius, 0.0, 30.0) {
                label = "Aisle kerb radius";
            }
            let mut sides = [a.left, a.right];
            if crate::props::toggles(ui, "Rows", &mut sides, &[("Left", "A row of bays down its left, looking along it"), ("Right", "A row of bays down its right")]) {
                a.left = sides[0];
                a.right = sides[1];
                label = "Aisle rows";
            }
            let angles: [(f64, &str, &str); 4] = [
                (90.0, "90°", "Square on to the aisle"),
                (60.0, "60°", "Echelon, leaning 60° to the aisle: easier to drive into, fewer bays"),
                (45.0, "45°", "Echelon, leaning 45° to the aisle"),
                (0.0, "0°", "Parallel, along the aisle's kerb"),
            ];
            // the one nearest what it has
            let mut angle = angles
                .iter()
                .map(|o| o.0)
                .min_by(|x, y| (x - a.angle).abs().total_cmp(&(y - a.angle).abs()))
                .unwrap_or(90.0);
            if crate::props::choice(ui, "Bay angle", &mut angle, &angles) {
                a.angle = angle;
                label = "Bay angle";
            }
            let mut heads = [a.head_start, a.head_end];
            if crate::props::toggles(ui, "Turn head", &mut heads, &[("Start", "A pocket kept clear beside its start, to turn in at a dead end"), ("End", "A pocket kept clear beside its end")]) {
                a.head_start = heads[0];
                a.head_end = heads[1];
                label = "Turning head";
            }
            if !a.trims.is_empty()
                && crate::props::action(ui, "", "Full rows", "Its rows drawn short by hand run their full length again", true).clicked()
            {
                a.trims.clear();
                label = "Rows full length";
            }
            if crate::props::action(ui, "", "Remove aisle", "Take this aisle and its rows away", true).clicked() {
                gone = Some(k);
            }
        }
        if let Some(k) = gone {
            cp.aisles.remove(k);
            label = "Remove aisle";
        }
        if crate::props::action(
            ui,
            "",
            "Add an aisle",
            "A new straight aisle across the middle of the car park. Double-click the car park in a plan and drag its points to where it goes",
            true,
        )
        .clicked()
        {
            let n = boundary.len().max(1);
            let c = boundary.iter().copied().sum::<DVec2>() / n as f64;
            let mut u = DVec2::X;
            let mut longest = 0.0;
            for i in 0..n {
                let e = boundary[(i + 1) % n] - boundary[i];
                if e.length() > longest {
                    longest = e.length();
                    u = e / longest;
                }
            }
            cp.aisles.push(Aisle::new(vec![c - u * 6.0, c + u * 6.0]));
            label = "Add aisle";
        }
        if crate::props::action(
            ui,
            "",
            "Lay its own",
            "Take the aisles drawn by hand away: it lays its own again and re-fits when its outline changes",
            true,
        )
        .clicked()
        {
            cp.aisles.clear();
            label = "Aisles laid again";
        }
    }
    // strips along its edges, and what its bays keep clear of
    });
    sfold_h(ui, "carpark.edges", "Edges", "A strip along an edge keeps bays off it: a walkway, planting or a clearance. Double-click the car park in a plan, then right-click an edge of its outline to give it one; drag the strip's inner line to set its width. Bays also keep off the trunk of any tree standing in the car park.", |ui| {
    let mut gone: Option<usize> = None;
    for st in cp.strips.iter_mut() {
        use creabim_core::carpark::StripUse;
        let name = format!("Edge {}", st.edge + 1);
        let mut pick = None;
        crate::props::Combo::new(&name).selected_text(st.what.label()).show_ui(ui, |ui| {
            for u in StripUse::ALL {
                if ui.selectable_label(st.what == u, u.label()).clicked() {
                    pick = Some(u);
                }
            }
            if ui.selectable_label(false, "None").clicked() {
                gone = Some(st.edge);
            }
        });
        if let Some(u) = pick {
            st.what = u;
            label = "Edge strip";
        }
        if num(ui, "Its width", &mut st.width, 0.1, 30.0) {
            label = "Edge strip width";
        }
    }
    if let Some(i) = gone {
        cp.set_strip(i, None);
        label = "Edge strip";
    }
    if num(ui, "Off trees", &mut cp.tree_clear, 0.0, 10.0) {
        label = "Clearance to trees";
    }
    });
    sfold_h(ui, "carpark.special", "Special bays", "How many accessible, charging and parent bays it has. Accessible bays go nearest the door (where a path meets the car park), in twos sharing a zone. To change one bay, type its number, or right-click it in a plan while editing the car park.", |ui| {
    if whole(ui, "Accessible", &mut cp.accessible, 500, "How many accessible bays. They go nearest the door (where a path meets the car park), in twos sharing an access zone") {
        label = "Accessible bays";
    }
    if cp.accessible > 0 && num(ui, "Zone", &mut cp.access_zone, 0.0, 3.0) {
        label = "Access zone";
    }
    if whole(ui, "EV bays", &mut cp.ev, 5000, "How many bays have a charger. They go at the far end of the row farthest from the door") {
        label = "EV bays";
    }
    if whole(ui, "Parent", &mut cp.parent, 5000, "How many parent and child bays. They go next nearest the door") {
        label = "Parent bays";
    }
    // one bay changed by hand: its number, and what it becomes
    if let Some(lay) = layout {
        let key = egui::Id::new("carpark.change.bay");
        let mut typed: String = ui.data_mut(|d| d.get_temp(key).unwrap_or_default());
        crate::props::row(ui, "Bay no.", |ui| {
            crate::props::text_field(ui, egui::TextEdit::singleline(&mut typed).hint_text("its number"), f32::INFINITY)
                .on_hover_text("A bay's number as written in plan. Choose below what it becomes; it stays that way however the car park is laid out again");
        });
        ui.data_mut(|d| d.insert_temp(key, typed.clone()));
        let bay = lay.bays.iter().find(|b| b.label.eq_ignore_ascii_case(typed.trim()));
        if let Some(bay) = bay {
            let mut pick: Option<Option<BayKind>> = None;
            crate::props::Combo::new("Make it")
                .selected_text(bay.kind.label())
                .show_ui(ui, |ui| {
                    for k in BayKind::ALL {
                        // accessible bays come from their count, with zones
                        if k == BayKind::Accessible || bay.kind == BayKind::Accessible {
                            continue;
                        }
                        if ui.selectable_label(bay.kind == k, k.label()).clicked() && bay.kind != k {
                            pick = Some(Some(k));
                        }
                    }
                });
            if bay.kind == BayKind::Accessible {
                crate::props::note(ui, "An accessible bay comes from the count above, with its zone.");
            }
            if let Some(kind) = pick {
                cp.change(bay.middle(), kind);
                label = "Change bay";
            }
        } else if !typed.trim().is_empty() {
            crate::props::note(ui, "No bay has that number.");
        }
        let mut undo: Option<usize> = None;
        for (i, c) in cp.changes.iter().enumerate() {
            let name = lay
                .bays
                .iter()
                .filter(|b| b.middle().distance(c.at) < cp.bay_width.max(cp.bay_length) / 2.0)
                .min_by(|a, b| a.middle().distance(c.at).total_cmp(&b.middle().distance(c.at)))
                .map_or_else(|| "A bay".to_string(), |b| format!("Bay {}", b.label));
            if crate::props::action(
                ui,
                &name,
                &format!("{} · undo", c.kind.label()),
                "Changed by hand. Click to let the layout decide it again",
                true,
            )
            .clicked()
            {
                undo = Some(i);
            }
        }
        if let Some(i) = undo {
            cp.changes.remove(i);
            label = "Change bay";
        }
    }
    });
    sfold_h(ui, "carpark.count", "Count", "What the permit asks: a number of bays, or one per so many square metres of a building's floor, and one accessible bay per so many. The count below reads against it.", |ui| {
    let kinds = ["None", "A number", "Per floor area"];
    let now = match cp.required {
        Requirement::None => 0,
        Requirement::Number(_) => 1,
        Requirement::PerFloor { .. } => 2,
    };
    let mut pick = now;
    crate::props::Combo::new("Required")
        .selected_text(kinds[now])
        .show_ui(ui, |ui| {
            for (i, k) in kinds.iter().enumerate() {
                if ui.selectable_label(now == i, *k).clicked() {
                    pick = i;
                }
            }
        });
    if pick != now {
        cp.required = match pick {
            1 => Requirement::Number(layout.map_or(0, |l| l.cars() as u32)),
            2 => Requirement::PerFloor { floor: 1000.0, per: 30.0 },
            _ => Requirement::None,
        };
        label = "Bays required";
    }
    match &mut cp.required {
        Requirement::None => {}
        Requirement::Number(n) => {
            if whole(ui, "Bays", n, 100_000, "How many bays the permit asks for") {
                label = "Bays required";
            }
        }
        Requirement::PerFloor { floor, per } => {
            let mut changed = false;
            for (name, v, tip) in [
                ("Floor area", floor, "The floor area of the building it serves, in m²"),
                ("Per bay", per, "One bay for every so many m² of floor"),
            ] {
                changed |= crate::props::row(ui, name, |ui| {
                    crate::props::fill(
                        ui,
                        egui::DragValue::new(v).range(1.0..=1e7).speed(1.0).suffix(" m²"),
                    )
                    .on_hover_text(tip)
                    .changed()
                });
            }
            if changed {
                label = "Bays required";
            }
        }
    }
    if crate::props::row(ui, "Accessible", |ui| {
        crate::props::fill(
            ui,
            egui::DragValue::new(&mut cp.accessible_per)
                .range(0..=1000)
                .speed(0.1)
                .custom_formatter(|v, _| if v < 0.5 { "None".into() } else { format!("1 per {v:.0}") })
                .custom_parser(|t| t.trim().trim_start_matches("1 per").trim().parse().ok().or(Some(0.0))),
        )
        .on_hover_text("One accessible bay asked for every so many bays; 0 asks none")
        .changed()
    }) {
        label = "Accessible required";
    }
    if let Some(lay) = layout {
        for (text, short) in cp.verdict(lay) {
            if short {
                crate::props::warning(ui, text, None);
            } else {
                crate::props::note(ui, text);
            }
        }
        for n in &lay.notes {
            crate::props::warning(ui, n.clone(), None);
        }
        let mut worked = vec![crate::props::Worked::new("Rows", lay.rows.to_string())];
        for k in BayKind::LISTED {
            let c = lay.count(k);
            if c > 0 {
                worked.push(crate::props::Worked::new(k.label(), c.to_string()));
            }
        }
        crate::props::worked_out(ui, "site.carpark", &worked);
    }
    });
    label
}

/// A work's height above the level it is on, named: it moves with that
/// level (owner, cut test: tie works to a level). The level itself is
/// chosen in the Element tab.
fn tied_offset(ui: &mut egui::Ui, model: &Model, level: ElementId, top: &mut f64) -> bool {
    let name = model
        .get(level)
        .map_or_else(|| "its level".to_string(), |e| e.name.clone());
    let mut off = *top;
    let changed = crate::props::row(ui, "Above", |ui| {
        crate::props::fill(ui, crate::units::drag(&mut off))
            .on_hover_text(format!(
                "How far it is above {name}. It moves with {name}; choose another level in the Element tab"
            ))
            .changed()
    });
    crate::props::note(ui, format!("Tied to {name}: it moves when {name} does."));
    if changed {
        *top = off;
    }
    changed
}

thread_local! {
    /// A pad's notes, by the model's revision: worked out over the whole
    /// model, so not at every frame.
    static PAD_NOTES: std::cell::RefCell<Option<(u64, ElementId, Vec<creabim_geom::earth::PadNote>)>> =
        const { std::cell::RefCell::new(None) };
}

/// What is built in pad `id` (MU140, mock-up Pad Note REV01): a blue note
/// when all is well, or orange ones, each with its fix. Returns the pad
/// level (absolute) a fix asked for.
fn pad_notes_ui(ui: &mut egui::Ui, model: &Model, id: Option<ElementId>, base: f64) -> Option<f64> {
    use creabim_geom::earth::PadNote;
    let id = id?;
    let rev = model.revision();
    let notes = PAD_NOTES.with(|c| {
        if let Some((r, i, n)) = &*c.borrow() {
            if *r == rev && *i == id && rev != 0 {
                return n.clone();
            }
        }
        let n = creabim_geom::earth::pad_notes(model, id);
        *c.borrow_mut() = Some((rev, id, n.clone()));
        n
    });
    let len = |m: f64| crate::units::len(m);
    let mut asked = None;
    for n in &notes {
        match n {
            PadNote::Sunk { slab, by, dug_to } => crate::props::info(
                ui,
                format!(
                    "Built in this pad: {slab} sinks {} into it. Under it the ground is dug to {}. Its stem walls and footings pass through the pad.",
                    len(*by),
                    crate::units::elevation(*dug_to - base)
                ),
                "site",
            ),
            PadNote::Crawl { floor, room, below } => crate::props::info(
                ui,
                if *below > 1e-3 {
                    format!(
                        "Crawl space under {floor}: {} under its joists. Its floor is {} below this pad: keep water out with a drain round the footings.",
                        len(*room),
                        len(*below)
                    )
                } else {
                    format!("Crawl space under {floor}: {} under its joists.", len(*room))
                },
                "site",
            ),
            PadNote::Buried { slab, by, fix } => {
                let rules = creabim_core::GroundRules::of(model);
                if crate::props::warning(
                    ui,
                    format!("The floor is buried: this pad lies {} above the top of {slab}.", len(*by)),
                    Some(&format!("Set it {} below the floor", len(rules.wood))),
                ) {
                    asked = Some(*fix);
                }
            }
            PadNote::Shallow { floor, room, need, fix } => {
                if crate::props::warning(
                    ui,
                    format!(
                        "The crawl space under {floor} is {} deep: {} is the usual least under the joists.",
                        len(room.max(0.0)),
                        len(*need)
                    ),
                    Some(&format!("Lower it to {}", len(*need))),
                ) {
                    asked = Some(*fix);
                }
            }
            PadNote::AgainstWood { wall, gap, need, fix } => {
                if crate::props::warning(
                    ui,
                    format!(
                        "The ground is against the wood: this pad lies {} below the foot of {wall}; the wood keeps {} off the ground.",
                        len(*gap),
                        len(*need)
                    ),
                    Some(&format!("Set it {} below", len(*need))),
                ) {
                    asked = Some(*fix);
                }
            }
        }
    }
    asked
}

/// The element a site element being edited is: the one in the model
/// like it (it is a copy, and changes only once a change is made).
fn element_of(model: &Model, s: &Site) -> Option<ElementId> {
    model
        .iter()
        .find(|e| matches!(&e.kind, ElementKind::Site(x) if x == s))
        .map(|e| e.id)
}

thread_local! {
    /// The works each work's banks run onto, by the model's revision.
    static CLASHES: std::cell::RefCell<Option<(u64, ElementId, Vec<String>)>> = const { std::cell::RefCell::new(None) };
}

/// Says which works this one's banks run onto: there they stop at that
/// work's edge and the ground steps (owner, cut test: spill-overs).
fn clash_note(ui: &mut egui::Ui, model: &Model, id: Option<ElementId>) {
    let Some(id) = id else {
        return;
    };
    let rev = model.revision();
    let names = CLASHES.with(|c| {
        if let Some((r, i, n)) = &*c.borrow() {
            if *r == rev && *i == id && rev != 0 {
                return n.clone();
            }
        }
        let n: Vec<String> = creabim_geom::earth::runs_onto(model, id)
            .into_iter()
            .filter_map(|o| model.get(o).map(|e| e.name.clone()))
            .collect();
        *c.borrow_mut() = Some((rev, id, n.clone()));
        n
    });
    if names.is_empty() {
        return;
    }
    crate::props::warning(
        ui,
        format!(
            "Its banks run onto {}. A bank stops at another work's edge, so the ground steps there; hold that side upright for a retained face.",
            names.join(", ")
        ),
        None,
    );
}

/// Is `kind` a plant (a tree, conifer, palm or shrub)? It then has a Tree
/// tab: how it is made lives there, whatever the plant (MU52).
pub fn is_plant(kind: &ElementKind) -> bool {
    matches!(kind, ElementKind::Site(s) if matches!(s.form, SiteForm::Plant { .. }))
}

/// The Tree tab (a subject of its own, as Blender's): how the plant is
/// made. Its Form first (kind, simple or generated, a preset), then, for a
/// generated tree, its crown, trunk, branches, leaves and seasons, 3D
/// detail, plan symbol, sections and seed. Its place and overall size are
/// in the Element tab (the Properties rule, MU52). Returns the undo label
/// of a change, or "".
pub fn tree_tab(ui: &mut egui::Ui, model: &Model, kind: &mut ElementKind) -> &'static str {
    let month = model.project_info().map_or(7, |(_, p)| p.season());
    let ElementKind::Site(s) = kind else {
        return "";
    };
    let SiteForm::Plant {
        position,
        form,
        canopy,
        height,
        tree,
        ..
    } = &mut s.form
    else {
        return "";
    };
    let mut label = "";
    let open = crate::props::block(ui, "tree.form", "Form", true);
    crate::props::fold(ui, open, |ui| {
        crate::props::Combo::new("Kind")
            .selected_text(form.label())
            .show_ui(ui, |ui| {
                for f in PlantForm::ALL {
                    if ui.selectable_value(form, f, f.label()).changed() {
                        label = "Plant form";
                    }
                }
            });
        if *form == PlantForm::Tree {
            let mut grown = tree.is_some();
            if crate::props::choice(
                ui,
                "Crown",
                &mut grown,
                &[
                    (false, "Simple", "one round crown on a trunk"),
                    (true, "Generated", "grown from a shape and a seed: clumps, limbs, a crown you can shape on each side"),
                ],
            ) {
                *tree = grown.then(|| Box::new(TreeShape::new_at(*canopy, *position)));
                if let Some(t) = tree.as_deref_mut() {
                    t.crown_base = t.crown_base.min(*height * 0.6);
                }
                label = "Tree crown";
            }
        }
        if let Some(t) = tree.as_deref_mut().filter(|_| *form == PlantForm::Tree) {
            let l = tree_preset(ui, t, canopy, height);
            if !l.is_empty() {
                label = l;
            }
        }
    });
    match tree.as_deref_mut() {
        Some(t) if *form == PlantForm::Tree => {
            let l = tree_fields(ui, t, canopy, height, month);
            if !l.is_empty() {
                label = l;
            }
        }
        _ => {
            crate::props::info(
                ui,
                "Its height and width are in the Element tab. A tree can be generated: clumps, limbs, leaves by season and a crown shaped on each side.",
                "trees",
            );
        }
    }
    label
}

/// "Start from": a preset gives a grown tree a fresh shape, keeping where
/// it stands, its seed and how it is drawn. Returns the undo label, or "".
fn tree_preset(
    ui: &mut egui::Ui,
    t: &mut TreeShape,
    canopy: &mut f64,
    height: &mut f64,
) -> &'static str {
    let mut label = "";
    crate::props::Combo::new("Start from")
        .selected_text("A preset…")
        .show_ui(ui, |ui| {
            for p in TreePreset::ALL {
                if ui
                    .selectable_label(false, p.label())
                    .on_hover_text(p.hint())
                    .clicked()
                {
                    // a fresh shape; where it stands, its seed and its
                    // turn stay, so its plan doesn't jump
                    let (mut shape, c, h) = p.shape();
                    shape.seed = t.seed;
                    shape.angle = t.angle;
                    // and how it is drawn and built
                    shape.plan = t.plan;
                    shape.plan_trunk = t.plan_trunk;
                    shape.plan_tint = t.plan_tint;
                    shape.plan_bold = t.plan_bold;
                    shape.elevation = t.elevation;
                    shape.elevation_limbs = t.elevation_limbs;
                    shape.form = t.form;
                    shape.facets = t.facets;
                    shape.cut = t.cut;
                    shape.solid = t.solid;
                    shape.detail = t.detail;
                    // its leaves are its kind's, not the preset's shape's
                    shape.leaves = t.leaves;
                    shape.month = t.month;
                    shape.own_month = t.own_month;
                    shape.leaf_colour = t.leaf_colour;
                    shape.autumn_colour = t.autumn_colour;
                    *t = shape;
                    (*canopy, *height) = (c, h);
                    label = "Tree preset";
                }
            }
        });
    label
}

/// A grown tree's blocks: its crown, trunk, limbs, leaves, plan symbol
/// and seed (its size is in the Element tab). Returns the undo
/// label of a change, or "".
fn tree_fields(
    ui: &mut egui::Ui,
    t: &mut TreeShape,
    canopy: &mut f64,
    height: &mut f64,
    project_month: u8,
) -> &'static str {
    let mut label = "";
    // what is worked out about the tree, one plate at the block's end
    // (AMD-015)
    let mut worked: Vec<crate::props::Worked> = Vec::new();
    let frac = |ui: &mut egui::Ui, text: &str, v: &mut f64, tip: &str| {
        crate::props::row(ui, text, |ui| {
            crate::props::fill(
                ui,
                egui::Slider::new(v, 0.0..=1.0)
                    .show_value(true)
                    .fixed_decimals(2),
            )
            .on_hover_text(tip)
            .changed()
        })
    };
    let open = crate::props::block(ui, "tree.crown", "Crown", true);
    crate::props::fold(ui, open, |ui| {
        if num(ui, "Depth", &mut t.depth, 0.3, 60.0) {
            label = "Crown depth";
        }
        crate::props::Combo::new("Shape")
            .selected_text(t.crown.label())
            .show_ui(ui, |ui| {
                for c in CrownShape::ALL {
                    if ui.selectable_value(&mut t.crown, c, c.label()).changed() {
                        label = "Crown shape";
                    }
                }
            });
        // "Clump form": the plant's own Form is above
        crate::props::Combo::new("Clump form")
            .selected_text(t.form.label())
            .show_ui(ui, |ui| {
                for f in ClumpForm::ALL {
                    if ui
                        .selectable_value(&mut t.form, f, f.label())
                        .on_hover_text(f.hint())
                        .changed()
                    {
                        label = "Crown form";
                    }
                }
            });
        if t.form == ClumpForm::LowPoly {
            let mut n = t.facets as f64;
            if crate::props::row(ui, "Facets", |ui| {
                crate::props::fill(
                    ui,
                    egui::Slider::new(&mut n, 1.0..=5.0).step_by(1.0).fixed_decimals(0),
                )
                .on_hover_text("How finely the crown is cut: 1 a few big faces, 5 the most (a hundred or so, never thousands)")
                .changed()
            }) {
                t.facets = n.round() as u8;
                label = "Crown facets";
            }
        }
        let mut n = t.clumps as f64;
        if crate::props::row(ui, "Clumps", |ui| {
            crate::props::fill(
                ui,
                egui::DragValue::new(&mut n)
                    .range(1.0..=60.0)
                    .speed(0.2)
                    .fixed_decimals(0),
            )
            .on_hover_text("How many masses of foliage the crown is made of")
            .changed()
        }) {
            t.clumps = n.round() as u32;
            label = "Crown clumps";
        }
        if frac(
            ui,
            "Clump size",
            &mut t.clump_size,
            "Small, many-lobed masses (0) to big billows (1)",
        ) {
            label = "Clump size";
        }
        if frac(
            ui,
            "Irregularity",
            &mut t.irregularity,
            "A tidy crown (0) to a ragged one (1)",
        ) {
            label = "Crown irregularity";
        }
        if frac(
            ui,
            "Openness",
            &mut t.openness,
            "A dense crown (0) to an open one with gaps (1)",
        ) {
            label = "Crown openness";
        }
        let pulled: Vec<&str> = (0..8)
            .filter(|i| (t.reach[*i] - 1.0).abs() > 0.02)
            .map(|i| creabim_core::tree::SIDES[i])
            .collect();
        if pulled.is_empty() {
            crate::props::howto(
                ui,
                "Drag its side grips in plan, a section or 3D to pull the crown in on one side, off a wall; the trunk stays",
            );
        } else {
            worked.push(crate::props::Worked::new("Shaped", pulled.join(", ")));
            if crate::props::action(
                ui,
                "Sides",
                "Even out",
                "Every side back to the crown's width and depth",
                true,
            )
            .clicked()
            {
                t.reach = [1.0; 8];
                label = "Even out crown";
            }
        }
    });
    let open = crate::props::block(ui, "tree.trunk", "Trunk", true);
    crate::props::fold(ui, open, |ui| {
        if num(ui, "Clear trunk", &mut t.crown_base, 0.0, 50.0) {
            label = "Clear trunk";
        }
        if num(ui, "Trunk", &mut t.trunk, 0.02, 5.0) {
            label = "Trunk diameter";
        }
        t.crown_base = t.crown_base.min((*height - 0.3).max(0.0));
    });
    let open = crate::props::block(ui, "tree.limbs", "Branches", false);
    crate::props::fold(ui, open, |ui| {
        crate::props::Combo::new("Limbs")
            .selected_text(t.branching.label())
            .show_ui(ui, |ui| {
                for b in Branching::ALL {
                    if ui
                        .selectable_value(&mut t.branching, b, b.label())
                        .changed()
                    {
                        label = "Tree limbs";
                    }
                }
            });
        if crate::props::choice(
            ui,
            "Wood",
            &mut t.solid,
            &[
                (
                    true,
                    "Solid",
                    "the trunk and limbs are closed bodies: a section fills their cut",
                ),
                (
                    false,
                    "Hollow",
                    "the trunk and limbs are open tubes: a section draws only their sides",
                ),
            ],
        ) {
            label = "Tree wood";
        }
    });
    let open = crate::props::block_help(
        ui,
        "tree.leaves",
        "Leaves and seasons",
        true,
        "3D shows a tree in its month; plans, sections and elevations always draw its crown. South of the equator (the project's latitude) its year runs six months on. Without a month of its own it follows the project's (the Project tab).",
        "site",
    );
    crate::props::fold(ui, open, |ui| {
        crate::props::Combo::new("Leaves")
            .selected_text(t.leaves.label())
            .show_ui(ui, |ui| {
                for h in LeafHabit::ALL {
                    if ui
                        .selectable_value(&mut t.leaves, h, h.label())
                        .on_hover_text(h.hint())
                        .changed()
                    {
                        label = "Tree leaves";
                    }
                }
            });
        if crate::props::check(ui, "Month", &mut t.own_month, "Its own")
            .on_hover_text("Keep this tree in a month of its own, whatever month the project shows")
            .changed()
        {
            if t.own_month {
                t.month = project_month;
            }
            label = "Tree own month";
        }
        // the month strip: one button a month across the panel, the month
        // 3D shows it in lit, a bar under each for how much is in leaf;
        // picking a month on a tree that follows the project keeps it there
        let shown = t.month_in(project_month);
        let own = t.own_month;
        crate::props::row(ui, "", |ui| {
            // twelve cells across the field column, drawn to fit it, so
            // the strip never pushes the panel wider (Site polish 1)
            let w = ui.available_width().max(60.0);
            let h = ui.spacing().interact_size.y;
            let (strip, _) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::hover());
            let cell = (w - 11.0) / 12.0;
            let v = ui.visuals().clone();
            let lit_ink =
                crate::theme::ink_on(crate::theme::over(v.selection.bg_fill, v.panel_fill));
            let fade = if own { 1.0 } else { 0.6 };
            for (i, m) in creabim_core::tree::MONTHS.iter().enumerate() {
                let n = i as u8 + 1;
                let state = t.leaves.state(n);
                let r = egui::Rect::from_min_size(
                    egui::pos2(strip.left() + i as f32 * (cell + 1.0), strip.top()),
                    egui::vec2(cell, h),
                );
                let resp = ui.interact(r, ui.id().with(("tree.month", i)), egui::Sense::click());
                let corner = egui::CornerRadius {
                    nw: if i == 0 { 5 } else { 0 },
                    sw: if i == 0 { 5 } else { 0 },
                    ne: if i == 11 { 5 } else { 0 },
                    se: if i == 11 { 5 } else { 0 },
                };
                let on = shown == n;
                let fill = if on {
                    v.selection.bg_fill
                } else if resp.hovered() {
                    v.widgets.hovered.weak_bg_fill
                } else {
                    v.widgets.inactive.weak_bg_fill
                };
                ui.painter()
                    .rect_filled(r, corner, fill.gamma_multiply(fade));
                let ink = if on { lit_ink } else { v.text_color() };
                ui.painter().text(
                    r.center() - egui::vec2(0.0, 1.5),
                    egui::Align2::CENTER_CENTER,
                    *m,
                    egui::FontId::proportional(9.5),
                    ink.gamma_multiply(fade),
                );
                // how much of it is in leaf, a thin bar in its colour
                if state.leaf > 0.0 {
                    let mut c = t.clone();
                    c.month = n;
                    let [r_, g_, b_] = c.foliage_colour(false).map(|v| (v * 255.0).round() as u8);
                    let bar = egui::Rect::from_min_size(
                        egui::pos2(r.left() + 2.0, r.bottom() - 3.5),
                        egui::vec2((r.width() - 4.0) * state.leaf as f32, 2.0),
                    );
                    ui.painter().rect_filled(
                        bar,
                        0.0,
                        egui::Color32::from_rgb(r_, g_, b_).gamma_multiply(fade),
                    );
                }
                let tip = format!(
                    "{}: {}",
                    creabim_core::tree::MONTH_NAMES[i],
                    if state.leaf <= 0.0 {
                        "bare".to_string()
                    } else if state.leaf >= 1.0 && state.turned > 0.5 {
                        "turning".into()
                    } else if state.leaf >= 1.0 {
                        "in full leaf".into()
                    } else {
                        format!("{:.0}% in leaf", state.leaf * 100.0)
                    }
                );
                if resp.on_hover_text(tip).clicked() && (shown != n || !t.own_month) {
                    t.month = n;
                    t.own_month = true;
                    label = "Tree month";
                }
            }
        });
        // a colour field each (DS-01 4.3), sRGB as the tree keeps it
        let colour = |ui: &mut egui::Ui, text: &str, c: &mut [f32; 3], tip: &str| {
            let mut b = c.map(|v| (v * 255.0).round() as u8);
            let changed = crate::props::colour_field(ui, text, &mut b, tip);
            if changed {
                *c = b.map(|v| v as f32 / 255.0);
            }
            changed
        };
        if colour(
            ui,
            "Leaf colour",
            &mut t.leaf_colour,
            "Its leaves in full leaf",
        ) {
            label = "Leaf colour";
        }
        if t.leaves != LeafHabit::Evergreen
            && colour(
                ui,
                "Autumn",
                &mut t.autumn_colour,
                "Its leaves as they turn, before they fall",
            )
        {
            label = "Autumn colour";
        }
        // its month, worked out, at the foot of this block
        if !t.own_month {
            crate::props::worked_out(
                ui,
                "tree.month",
                &[crate::props::Worked::new(
                    "Month",
                    format!(
                        "{}, the project's",
                        creabim_core::tree::MONTH_NAMES[(shown - 1) as usize]
                    ),
                )],
            );
        }
    });
    let open = crate::props::block(ui, "tree.detail", "3D detail", false);
    crate::props::fold(ui, open, |ui| {
        crate::props::Combo::new("Build")
            .selected_text(t.detail.label())
            .show_ui(ui, |ui| {
                for d in TreeDetail::ALL {
                    if ui
                        .selectable_value(&mut t.detail, d, d.label())
                        .on_hover_text(d.hint())
                        .changed()
                    {
                        label = "Tree detail";
                    }
                }
            });
        // what it builds, at the foot of the block that sets it
        let g = creabim_geom::tree::grow(t, *canopy, *height);
        crate::props::worked_out(
            ui,
            "tree.detail",
            &[
                crate::props::Worked::new(
                    "Triangles",
                    (g.foliage.triangle_count() + g.wood.triangle_count()).to_string(),
                ),
                crate::props::Worked::new(
                    "Clumps",
                    g.clumps.iter().filter(|c| !c.core).count().to_string(),
                ),
            ],
        );
    });
    let open = crate::props::block(ui, "tree.plan", "Plan symbol", true);
    crate::props::fold(ui, open, |ui| {
        crate::props::Combo::new("Draw as")
            .selected_text(t.plan.label())
            .show_ui(ui, |ui| {
                for p in TreePlan::ALL {
                    if ui
                        .selectable_value(&mut t.plan, p, p.label())
                        .on_hover_text(p.hint())
                        .changed()
                    {
                        label = "Tree plan symbol";
                    }
                }
            });
        if crate::props::check(ui, "Outline", &mut t.plan_bold, "Bold")
            .on_hover_text("The crown's outline heavy, what is inside it light")
            .changed()
        {
            label = "Tree bold outline";
        }
        if crate::props::check(ui, "Trunk", &mut t.plan_trunk, "And centre")
            .on_hover_text("Its trunk drawn, and a mark at its centre")
            .changed()
        {
            label = "Tree trunk mark";
        }
        if crate::props::check(ui, "Canopy", &mut t.plan_tint, "Tinted")
            .on_hover_text("A light tint over its crown")
            .changed()
        {
            label = "Tree canopy tint";
        }
    });
    let open = crate::props::block(ui, "tree.elevation", "Sections and elevations", true);
    crate::props::fold(ui, open, |ui| {
        crate::props::Combo::salted("Draw as", "tree.elevation")
            .selected_text(t.elevation.label())
            .show_ui(ui, |ui| {
                for e in TreeElevation::ALL {
                    if ui
                        .selectable_value(&mut t.elevation, e, e.label())
                        .on_hover_text(e.hint())
                        .changed()
                    {
                        label = "Tree section look";
                    }
                }
            });
        if crate::props::check(ui, "Limbs", &mut t.elevation_limbs, "Through crown")
            .on_hover_text("Its trunk and main limbs drawn on up into the crown")
            .changed()
        {
            label = "Tree limbs in elevation";
        }
        crate::props::Combo::salted("Where cut", "tree.cut")
            .selected_text(t.cut.label())
            .show_ui(ui, |ui| {
                for c in TreeCut::ALL {
                    if ui
                        .selectable_value(&mut t.cut, c, c.label())
                        .on_hover_text(c.hint())
                        .changed()
                    {
                        label = "Tree cut look";
                    }
                }
            });
    });
    let open = crate::props::block(ui, "tree.seed", "Variation", true);
    crate::props::fold(ui, open, |ui| {
        // its field, and the dice at the row's end: a row's own action is
        // an icon there, as Flip on a wall's Exterior row
        let mut seed = t.seed as f64;
        crate::props::row(ui, "Seed", |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let w = (ui.available_width() - 24.0).max(40.0);
                let h = ui.spacing().interact_size.y;
                if ui
                    .add_sized(
                        [w, h],
                        egui::DragValue::new(&mut seed)
                            .range(1.0..=99_999.0)
                            .speed(1.0),
                    )
                    .on_hover_text("The same seed always grows the same tree")
                    .changed()
                {
                    t.seed = seed.round() as u32;
                    label = "Tree seed";
                }
                if crate::props::icon_button(
                    ui,
                    "dice",
                    "A new variation: another tree of the same shape",
                )
                .clicked()
                {
                    let jump = ui.ctx().input(|i| i.time * 1000.0) as u32 % 9_973 + 1;
                    t.seed = (t.seed + jump) % 99_999 + 1;
                    label = "New tree variation";
                }
            })
        });
    });
    crate::props::worked_later(ui, "tree", &worked);
    label
}

#[cfg(test)]
mod tests {
    use super::*;
    use creabim_core::sample::sample_house;

    fn setup() -> (Document, ElementId, PlanState, Selection) {
        let doc = Document::from_model(sample_house());
        let level = doc
            .model()
            .levels()
            .min_by(|a, b| a.1.elevation.total_cmp(&b.1.elevation))
            .unwrap()
            .0
            .id;
        (doc, level, PlanState::default(), Selection::default())
    }

    #[test]
    fn a_coping_follows_the_wall_clicked() {
        let (mut doc, level, mut st, mut sel) = setup();
        let (wid, mid) = doc
            .model()
            .iter()
            .find_map(|e| match &e.kind {
                ElementKind::Wall(w) if w.level == level && w.curtain.is_none() => {
                    Some((e.id, (w.start + w.end) / 2.0))
                }
                _ => None,
            })
            .unwrap();
        click(&mut doc, level, Tool::Sweep, mid, &mut st, &mut sel).unwrap();
        let id = sel.all()[0];
        let e = doc.model().get(id).unwrap();
        let ElementKind::Part(p) = &e.kind else {
            panic!("not a part")
        };
        assert_eq!(p.host(), Some(wid));
        // undo takes it away again
        doc.undo();
        assert!(doc.model().get(id).is_none());
    }

    #[test]
    fn a_free_sweep_is_clicked_point_by_point() {
        let (mut doc, level, mut st, mut sel) = setup();
        st.parts.profile = SweepProfile::Steps;
        st.parts.size = SweepProfile::Steps.defaults();
        let far = DVec2::new(-40.0, -40.0);
        for p in [far, far + DVec2::X * 1.5, far + DVec2::X * 1.5] {
            click(&mut doc, level, Tool::Sweep, p, &mut st, &mut sel).unwrap();
        }
        let ElementKind::Part(p) = &doc.model().get(sel.all()[0]).unwrap().kind else {
            panic!("not a part")
        };
        assert!(
            matches!(&p.form, Form::Sweep { path: Path::Points { points, .. }, .. } if points.len() == 2)
        );
        assert!(st.parts.path.is_empty());
    }

    #[test]
    fn outlines_make_parts_and_site_with_their_materials() {
        let (mut doc, level, _, _) = setup();
        let sq = creabim_core::polygon::rectangle(DVec2::new(30.0, 30.0), DVec2::new(34.0, 36.0));
        for make in [
            Make::Extrusion {
                base: 0.0,
                height: 1.0,
                void: false,
            },
            Make::Terrain { thickness: 0.5 },
            Make::Area(1),
        ] {
            let (_, id) = finish_outline(&mut doc, level, sq.clone(), &make).unwrap();
            let e = doc.model().get(id.unwrap()).unwrap();
            match make {
                Make::Extrusion { .. } => assert!(matches!(e.kind, ElementKind::Part(_))),
                _ => assert!(creabim_core::material_of(doc.model(), e).is_some()),
            }
        }
        // the driveway's material came from the library, once
        let (_, id) = finish_outline(&mut doc, level, sq, &Make::Area(1)).unwrap();
        assert!(id.is_some());
        let n = doc
            .model()
            .iter()
            .filter(|e| e.name == "Block paving" && matches!(e.kind, ElementKind::Material(_)))
            .count();
        assert_eq!(n, 1);
    }

    #[test]
    fn planting_places_a_plant() {
        let (mut doc, level, mut st, mut sel) = setup();
        st.parts.plant = PlantForm::Palm;
        click(
            &mut doc,
            level,
            Tool::Planting,
            DVec2::new(20.0, 0.0),
            &mut st,
            &mut sel,
        )
        .unwrap();
        let e = doc.model().get(sel.all()[0]).unwrap();
        assert!(matches!(&e.kind, ElementKind::Site(s) if s.is_plant()));
        assert_eq!(e.name, "Palm");
    }

    #[test]
    fn a_pad_is_drawn_into_a_terrain_and_measured() {
        let (mut doc, level, mut st, mut sel) = setup();
        let outline =
            creabim_core::polygon::rectangle(DVec2::new(30.0, 0.0), DVec2::new(60.0, 20.0));
        let (_, id) =
            finish_outline(&mut doc, level, outline, &Make::Terrain { thickness: 0.5 }).unwrap();
        let id = id.unwrap();
        st.parts.ground = GroundDraw::Pad;
        st.parts.pad_level = Some(-1.0);
        // its corners, then the first again closes it
        for p in [
            (40.0, 5.0),
            (50.0, 5.0),
            (50.0, 15.0),
            (40.0, 15.0),
            (40.0, 5.0),
        ] {
            click(
                &mut doc,
                level,
                Tool::Terrain,
                DVec2::new(p.0, p.1),
                &mut st,
                &mut sel,
            )
            .unwrap();
        }
        assert!(st.parts.path.is_empty());
        let ElementKind::Site(site) = &doc.model().get(id).unwrap().kind else {
            panic!()
        };
        // it is an element of its own, selected, on the terrain's level
        let pad = sel.active.expect("the pad is selected");
        assert_ne!(pad, id);
        let ElementKind::Site(work) = &doc.model().get(pad).unwrap().kind else {
            panic!()
        };
        assert_eq!(work.work_kind(), Some(creabim_core::WorkKind::Pad));
        assert_eq!(work.boundary().unwrap().len(), 4);
        assert_eq!(work.level, site.level);
        // a metre dug out of flat ground over 10 m by 10 m, and its banks
        let e = creabim_geom::earth::shaped(doc.model(), site).unwrap();
        assert!(e.changed());
        assert!(e.cut > 100.0 && e.cut < 125.0, "{}", e.cut);
        assert!(e.fill < 0.01);
        let g = &e.ground;
        assert!((g.height(DVec2::new(45.0, 10.0)).unwrap() + 1.0).abs() < 1e-6);
        assert!(g.height(DVec2::new(35.0, 10.0)).unwrap().abs() < 1e-6);
        // one undo takes it out again
        assert!(doc.undo());
        let ElementKind::Site(site) = &doc.model().get(id).unwrap().kind else {
            panic!()
        };
        assert!(!creabim_geom::earth::shaped(doc.model(), site)
            .unwrap()
            .changed());
    }

    #[test]
    fn a_trench_and_a_pit_are_dug_by_their_depth() {
        let (mut doc, level, mut st, mut sel) = setup();
        let outline =
            creabim_core::polygon::rectangle(DVec2::new(30.0, 0.0), DVec2::new(60.0, 20.0));
        let (_, id) =
            finish_outline(&mut doc, level, outline, &Make::Terrain { thickness: 0.5 }).unwrap();
        let id = id.unwrap();
        // a trench along a line, finished by clicking its last point again
        st.parts.ground = GroundDraw::Trench;
        st.parts.ground_width = 0.5;
        st.parts.ground_depth = 0.8;
        for p in [(35.0, 10.0), (45.0, 10.0), (45.0, 10.0)] {
            click(
                &mut doc,
                level,
                Tool::Terrain,
                DVec2::new(p.0, p.1),
                &mut st,
                &mut sel,
            )
            .unwrap();
        }
        // a pit by its corners
        st.parts.ground = GroundDraw::Pit;
        st.parts.ground_depth = 2.0;
        for p in [
            (50.0, 5.0),
            (55.0, 5.0),
            (55.0, 10.0),
            (50.0, 10.0),
            (50.0, 5.0),
        ] {
            click(
                &mut doc,
                level,
                Tool::Terrain,
                DVec2::new(p.0, p.1),
                &mut st,
                &mut sel,
            )
            .unwrap();
        }
        let ElementKind::Site(site) = &doc.model().get(id).unwrap().kind else {
            panic!()
        };
        // each is an element: a trench with its line, a pit with its sides
        let works: Vec<_> = doc
            .model()
            .iter()
            .filter_map(|e| match &e.kind {
                ElementKind::Site(s) => Some((s.work_kind()?, s.as_work()?, e.name.clone())),
                _ => None,
            })
            .collect();
        assert_eq!(works.len(), 2);
        let of = |k| works.iter().find(|w| w.0 == k).unwrap();
        let (trench, pit) = (
            of(creabim_core::WorkKind::Trench),
            of(creabim_core::WorkKind::Pit),
        );
        assert_eq!(
            (trench.1.depth, trench.1.cut, trench.2.as_str()),
            (0.8, 0.0, "Trench")
        );
        assert_eq!((pit.1.depth, pit.1.cut, pit.2.as_str()), (2.0, 0.5, "Pit"));
        let e = creabim_geom::earth::shaped(doc.model(), site).unwrap();
        let h = |x: f64, y: f64| e.ground.height(DVec2::new(x, y)).unwrap();
        assert!((h(40.0, 10.0) + 0.8).abs() < 1e-3, "{}", h(40.0, 10.0));
        assert!(h(40.0, 11.0).abs() < 1e-6);
        assert!((h(52.5, 7.5) + 2.0).abs() < 1e-3);
        // the trench is 10 by 0.5 by 0.8 (4 m3); the pit 5 by 5 by 2 with
        // sides lying back 1 in 2 (a little over 50 m3 and under 75)
        assert!(e.cut > 54.0 && e.cut < 79.0, "{}", e.cut);
    }

    #[test]
    fn contours_and_spot_heights_are_drawn_onto_the_terrain_under_them() {
        let (mut doc, level, mut st, mut sel) = setup();
        let outline =
            creabim_core::polygon::rectangle(DVec2::new(30.0, 0.0), DVec2::new(60.0, 20.0));
        let (_, id) =
            finish_outline(&mut doc, level, outline, &Make::Terrain { thickness: 0.5 }).unwrap();
        let id = id.unwrap();
        let site = |doc: &Document| match &doc.model().get(id).unwrap().kind {
            ElementKind::Site(s) => s.clone(),
            _ => panic!(),
        };
        // off any terrain, a click says so and adds nothing
        st.parts.ground = GroundDraw::Spot;
        click(
            &mut doc,
            level,
            Tool::Terrain,
            DVec2::new(0.0, 50.0),
            &mut st,
            &mut sel,
        )
        .unwrap();
        assert!(st
            .status
            .as_deref()
            .unwrap()
            .starts_with("Click on a terrain"));
        // two contours traced across it, the second a step up
        st.parts.ground = GroundDraw::Contour;
        st.parts.ground_height = 1.0;
        st.parts.ground_step = 0.5;
        for x in [35.0, 55.0] {
            for p in [DVec2::new(x, 0.0), DVec2::new(x, 10.0), DVec2::new(x, 20.0)] {
                click(&mut doc, level, Tool::Terrain, p, &mut st, &mut sel).unwrap();
            }
            // the last point again finishes the line
            click(
                &mut doc,
                level,
                Tool::Terrain,
                DVec2::new(x, 20.0),
                &mut st,
                &mut sel,
            )
            .unwrap();
            assert!(st.parts.path.is_empty());
        }
        assert_eq!(st.parts.ground_height, 2.0);
        let s = site(&doc);
        let SiteForm::Terrain { data, contour, .. } = &s.form else {
            panic!()
        };
        // each is an element of its own, lying on the terrain; the last
        // drawn is the one selected
        assert!(data.lines.is_empty());
        let (spots, lines) = creabim_core::site::ground_objects(doc.model(), &s);
        assert_eq!((spots.len(), lines.len()), (0, 2));
        assert_eq!(sel.all(), vec![lines[1].0]);
        assert!(data.survey && data.labels);
        assert_eq!(*contour, 0.5);
        // the ground runs through them: half way between, half way up
        let g = creabim_geom::earth::shaped(doc.model(), &s)
            .unwrap()
            .ground
            .clone();
        assert!((g.height(DVec2::new(35.0, 5.0)).unwrap() - 1.0).abs() < 1e-9);
        assert!((g.height(DVec2::new(45.0, 10.0)).unwrap() - 1.25).abs() < 0.05);
        // and its plan shows the contour between them
        let e = doc.model().get(id).unwrap();
        let items = creabim_geom::parts::plan_items(doc.model(), e, 1.2);
        assert!(items.iter().any(|i| matches!(
            i,
            creabim_geom::plan::PlanItem::Line {
                pen: creabim_geom::plan::Pen::Contour | creabim_geom::plan::Pen::ContourHeavy,
                ..
            }
        )));
        // a spot height, then each undone in turn
        st.parts.ground = GroundDraw::Spot;
        st.parts.ground_height = 3.0;
        click(
            &mut doc,
            level,
            Tool::Terrain,
            DVec2::new(45.0, 10.0),
            &mut st,
            &mut sel,
        )
        .unwrap();
        assert!(
            (creabim_geom::earth::shaped(doc.model(), &site(&doc))
                .unwrap()
                .ground
                .height(DVec2::new(45.0, 10.0))
                .unwrap()
                - 3.0)
                .abs()
                < 1e-9
        );
        let on = |doc: &Document| {
            let (a, b) = creabim_core::site::ground_objects(doc.model(), &site(doc));
            (a.len(), b.len())
        };
        assert_eq!(on(&doc), (1, 2));
        doc.undo();
        assert_eq!(on(&doc), (0, 2));
        doc.undo();
        assert_eq!(on(&doc), (0, 1));
    }

    #[test]
    fn levels_are_typed_as_the_project_reads_them_and_a_pad_starts_at_the_ground() {
        let (mut doc, level, mut st, mut sel) = setup();
        let outline =
            creabim_core::polygon::rectangle(DVec2::new(30.0, 0.0), DVec2::new(60.0, 20.0));
        let (_, id) =
            finish_outline(&mut doc, level, outline, &Make::Terrain { thickness: 0.5 }).unwrap();
        sel.only(id.unwrap());
        // the strip's Level, from a datum of 100 m: 100.5 is half a metre up
        st.tool = Tool::Terrain;
        st.parts.ground = GroundDraw::Spot;
        st.parts.level_shift = 100.0;
        let opts = serde_json::to_string(&tool_options(&st)).unwrap();
        assert!(
            opts.contains("\"Level\"") && opts.contains("100.5"),
            "{opts}"
        );
        assert!(!opts.contains("\"Height\""));
        set_option(&mut st, "part.ground_level", 101.25);
        assert!((st.parts.ground_height - 1.25).abs() < 1e-9);
        // read from the project's zero, the same field says 1.25
        st.parts.level_shift = 0.0;
        let opts = serde_json::to_string(&tool_options(&st)).unwrap();
        assert!(opts.contains("1.25") && !opts.contains("101.25"));
        // four spot levels at 1.25 m: sloping ground is not needed, a
        // level ground a way above the terrain's top is
        for p in [(32.0, 2.0), (58.0, 2.0), (58.0, 18.0), (32.0, 18.0)] {
            let at = DVec2::new(p.0, p.1);
            click(&mut doc, level, Tool::Terrain, at, &mut st, &mut sel).unwrap();
        }
        // a pad with no level typed starts at the ground under its first
        // corner, not at the level the spot levels were given
        st.parts.ground = GroundDraw::Pad;
        let opts = serde_json::to_string(&tool_options(&st)).unwrap();
        assert!(opts.contains("At the ground") && !opts.contains("\"Level\""));
        sel.only(id.unwrap());
        let corners = [(40.0, 5.0), (50.0, 5.0), (50.0, 15.0), (40.0, 15.0)];
        for p in corners.iter().chain(&corners[..1]) {
            let at = DVec2::new(p.0, p.1);
            click(&mut doc, level, Tool::Terrain, at, &mut st, &mut sel).unwrap();
        }
        let pad = sel.active.expect("the pad is selected");
        let ElementKind::Site(Site {
            form: SiteForm::Pad { top, .. },
            ..
        }) = &doc.model().get(pad).unwrap().kind
        else {
            panic!("a pad")
        };
        let ElementKind::Site(t) = &doc.model().get(id.unwrap()).unwrap().kind else {
            panic!()
        };
        let ground = creabim_geom::earth::shaped(doc.model(), t)
            .unwrap()
            .existing
            .height(DVec2::new(40.0, 5.0))
            .unwrap();
        assert!(
            (top - ground).abs() < 0.026,
            "pad at {top}, ground at {ground}"
        );
        // typed, the level is the pad's
        toggle_option(&mut st, "padlevel.set");
        assert_eq!(st.parts.pad_level, Some(0.0));
        set_option(&mut st, "part.pad_level", 0.6);
        assert_eq!(st.parts.pad_level, Some(0.6));
        toggle_option(&mut st, "padlevel.ground");
        assert_eq!(st.parts.pad_level, None);
        // a right-click or Esc takes back one point of a line being drawn
        st.tool = Tool::Terrain;
        st.parts.ground = GroundDraw::Trench;
        for x in [35.0, 40.0, 45.0] {
            let at = DVec2::new(x, 10.0);
            click(&mut doc, level, Tool::Terrain, at, &mut st, &mut sel).unwrap();
        }
        assert!(st.step_back());
        assert_eq!(st.parts.path.len(), 2);
        assert_eq!(st.pending, Some(DVec2::new(40.0, 10.0)));
        assert!(st.step_back() && st.step_back());
        assert!(st.parts.path.is_empty() && st.pending.is_none());
    }

    #[test]
    fn a_retaining_wall_is_a_wall_of_its_own_and_the_ground_is_cut_at_its_back() {
        let (mut doc, level, _, _) = setup();
        let outline =
            creabim_core::polygon::rectangle(DVec2::new(30.0, 0.0), DVec2::new(70.0, 40.0));
        finish_outline(&mut doc, level, outline, &Make::Terrain { thickness: 0.5 }).unwrap();
        // a pad dug a metre into the flat ground, 10 m square
        let pad = Element::new(
            "Pad",
            ElementKind::Site(Site::pad(
                level,
                creabim_core::polygon::rectangle(DVec2::new(45.0, 15.0), DVec2::new(55.0, 25.0)),
                -1.0,
            )),
        );
        let id = pad.id;
        doc.commit(Transaction::new("pad").insert(pad)).unwrap();
        let ground = |doc: &Document, p: DVec2| {
            let m = doc.model();
            let t = m
                .iter()
                .find_map(|e| match &e.kind {
                    ElementKind::Site(s) if matches!(s.form, SiteForm::Terrain { .. }) => Some(s),
                    _ => None,
                })
                .unwrap();
            creabim_geom::earth::shaped(m, t)
                .unwrap()
                .ground
                .height(p)
                .unwrap()
        };
        // with its bank, the ground half a metre north of the edge is half way up
        let north = DVec2::new(50.0, 25.5);
        assert!((ground(&doc, north) + 0.5).abs() < 0.06);
        // a wall on the north edge (edge 2): the edge is retained with it
        let tx = retaining_wall(doc.model(), id, 2, true).expect("a wall to add");
        doc.commit(tx).unwrap();
        let (wall_id, wall) = doc
            .model()
            .iter()
            .find_map(|e| match &e.kind {
                ElementKind::Wall(w) if e.name == "Retaining wall" => Some((e.id, w.clone())),
                _ => None,
            })
            .expect("the wall");
        assert_eq!(wall.thickness, RETAINING_WALL);
        // its location line is its face on the pad's edge, so a thicker
        // wall grows into the ground: the line is 25.0, the pad's edge
        assert_eq!(wall.location, creabim_core::LocationLine::InteriorFace);
        // (the offset is to the left of start to end: here, south)
        let line = wall.start.y - creabim_core::walls::location_offset(doc.model(), &wall);
        assert!((line - 25.0).abs() < 1e-9, "its line is at {line}");
        // outside the outline, its face on the edge: its middle 125 mm out
        assert!((wall.start.y - 25.125).abs() < 1e-9 && (wall.end.y - 25.125).abs() < 1e-9);
        // from the pad's level to just above the ground it holds
        assert_eq!(wall.base_offset, -1.0);
        assert!((wall.height - 1.15).abs() < 1e-6, "{}", wall.height);
        let ElementKind::Site(Site {
            form: SiteForm::Pad {
                retained, walls, ..
            },
            ..
        }) = &doc.model().get(id).unwrap().kind
        else {
            panic!()
        };
        assert_eq!(
            (retained.as_slice(), walls.as_slice()),
            (&[2][..], &[(2, wall_id)][..])
        );
        // the ground is cut at the wall's back: dug where the wall stands,
        // as it was just behind it
        assert!((ground(&doc, DVec2::new(50.0, 25.1)) + 1.0).abs() < 0.03);
        assert!(ground(&doc, DVec2::new(50.0, 25.6)).abs() < 0.03);
        assert!(
            retaining_wall(doc.model(), id, 2, true).is_none(),
            "it has one"
        );
        // thickened by hand, the cut follows it
        let before = doc.model().get(wall_id).unwrap().clone();
        let mut after = before.clone();
        if let ElementKind::Wall(w) = &mut after.kind {
            w.thickness = 0.6;
        }
        doc.commit(Transaction::new("thicken").replace(before, after))
            .unwrap();
        assert!((ground(&doc, DVec2::new(50.0, 25.5)) + 1.0).abs() < 0.03);
        // taken away: the wall goes, the edge stays cut upright at the outline
        let tx = retaining_wall(doc.model(), id, 2, false).expect("a wall to remove");
        doc.commit(tx).unwrap();
        assert!(doc.model().get(wall_id).is_none());
        assert!(ground(&doc, DVec2::new(50.0, 25.2)).abs() < 0.03);
    }

    #[test]
    fn the_terrain_tool_still_draws_outlines_and_offers_its_other_modes() {
        let (_, _, mut st, _) = setup();
        st.tool = Tool::Terrain;
        assert!(make_for(&st).is_some());
        let opts = serde_json::to_string(&tool_options(&st)).unwrap();
        // the strip says how and the values; what it draws is picked in
        // the Site family's panel (tool families REV04)
        for word in ["Import survey", "Depth"] {
            assert!(opts.contains(word), "{word}");
        }
        for word in ["Contour line", "Spot level", "Trench"] {
            assert!(!opts.contains(word), "{word} is a what, not a how");
            assert!(
                crate::families::Family::Site
                    .members()
                    .iter()
                    .any(|m| m.label == word),
                "{word} is in the Site panel"
            );
        }
        toggle_option(&mut st, "ground.Contour line");
        assert_eq!(st.parts.ground, GroundDraw::Contour);
        let opts = serde_json::to_string(&tool_options(&st)).unwrap();
        assert!(opts.contains("Step") && !opts.contains("\"Depth\""));
    }
    #[test]
    fn a_property_is_drawn_or_picked_from_the_map() {
        let (mut doc, level, mut st, mut sel) = setup();
        st.tool = Tool::Terrain;
        toggle_option(&mut st, "ground.Property line");
        assert_eq!(st.parts.ground, GroundDraw::Property);
        // drawn like an outline, clockwise: made counter-clockwise
        let make = make_for(&st).unwrap();
        assert!(matches!(make, Make::Property));
        let cw = vec![
            DVec2::new(0.0, 0.0),
            DVec2::new(0.0, 20.0),
            DVec2::new(15.0, 20.0),
            DVec2::new(15.0, 0.0),
        ];
        let (_, id) = finish_outline(&mut doc, level, cw, &make).unwrap();
        let lot = doc.model().get(id.unwrap()).unwrap();
        let ElementKind::Site(Site {
            form: SiteForm::Property { boundary, .. },
            ..
        }) = &lot.kind
        else {
            panic!("a property")
        };
        assert!(creabim_core::polygon::signed_area2(boundary) > 0.0);
        // a lot on a map under the plan, picked by a click inside it
        let dir = std::env::temp_dir().join(format!("creabim-parcel-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("lots.geojson");
        std::fs::write(
            &path,
            r#"{"type":"FeatureCollection","crs":{"type":"name","properties":{"name":"EPSG:32613"}},"features":[
              {"type":"Feature","properties":{},"geometry":{"type":"Polygon","coordinates":[[[30,0],[60,0],[60,25],[30,25],[30,0]]]}},
              {"type":"Feature","properties":{},"geometry":{"type":"Polygon","coordinates":[[[40,5],[45,5],[45,10],[40,10],[40,5]]]}}]}"#,
        )
        .unwrap();
        let source = creabim_core::RefSource::Map {
            path: path.to_string_lossy().into_owned(),
        };
        let r = Element::new(
            "Map",
            ElementKind::Reference(creabim_core::Reference::new(source, vec![])),
        );
        doc.commit(Transaction::new("Map").insert(r)).unwrap();
        toggle_option(&mut st, "ground.parcel");
        assert!(st.parts.pick_parcel);
        // outside the small ring: the lot round it
        click(
            &mut doc,
            level,
            Tool::Terrain,
            DVec2::new(50.0, 20.0),
            &mut st,
            &mut sel,
        )
        .unwrap();
        assert!(!st.parts.pick_parcel);
        let picked = sel.active.and_then(|id| doc.model().get(id)).unwrap();
        let ElementKind::Site(s) = &picked.kind else {
            panic!()
        };
        let b = s.boundary().unwrap();
        assert_eq!(b.len(), 4);
        assert!((creabim_core::lot::area(b) - 750.0).abs() < 1e-6);
        // nothing round an empty spot
        assert!(parcel_at(doc.model(), DVec2::new(100.0, 100.0)).is_none());
        let _ = std::fs::remove_dir_all(dir);
    }
}
