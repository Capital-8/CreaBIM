//! The Column, Beam, Stair and Railing tools (the Roof tool draws its
//! outline with the Slab tool's modes), their settings for new elements,
//! and the Properties panel for every element they make.

use crate::planview::{PlanState, Tool};
use crate::selection::Selection;
use creabim_core::members::{self, Profile, RailExtend, RoofShape, StairLine};
use creabim_core::{
    Beam, Column, Document, Element, ElementId, ElementKind, Model, ModelError, Railing, Roof,
    Slab, Stair, Transaction,
};
use eframe::egui::{self, Pos2, Stroke};
use glam::{DVec2, DVec3};

/// Sizes and shapes for new elements, set in the tool bar.
#[derive(Clone, Debug)]
pub struct MemberSettings {
    pub column: Profile,
    pub beam_width: f64,
    pub beam_depth: f64,
    pub roof_shape: RoofShape,
    pub roof_pitch: f64,
    pub stair_width: f64,
    /// Which line of a new stair the Stair tool draws.
    pub stair_drawn: StairLine,
    /// The stair's line clicked so far.
    pub stair_line: Vec<DVec2>,
    /// New stairs get a railing on their left, and on their right: a
    /// wall handrail where a wall runs beside that side, else a guard.
    pub stair_rail_left: bool,
    /// New stairs turn on winders, not landings.
    pub stair_winders: bool,
    /// New stairs are spirals: the first click is the centre, the second
    /// where the stair starts, on the line it is drawn by.
    pub stair_spiral: bool,
    /// New stairs are ramps (at 1:12).
    pub stair_ramp: bool,
    /// New stairs are split: the line is one arm, mirrored.
    pub stair_split: bool,
    pub stair_rail_right: bool,
    /// The gap between the runs of a half-turn (a U drawn with one click
    /// at its turn): the last typed, kept for the next (REV02 9 A).
    pub stair_gap: f64,
    pub railing_height: f64,
    /// The railing path clicked so far.
    pub path: Vec<DVec2>,
    /// New footings are pads (else strips), unless the tool's type says.
    pub footing_pad: bool,
    /// Width of new strips, and the sides of new pads.
    pub footing_width: f64,
    pub footing_thickness: f64,
    /// New doors', windows' and bare holes' width, height and sill, set in
    /// the bar before placing (MU18); a type, when picked, sets its own.
    pub door: [f64; 3],
    pub window: [f64; 3],
    /// The kind new doors and windows are made as, picked in the picker
    /// (none: the usual hinged door, fixed window).
    pub door_kind: Option<crate::kindpick::Pick>,
    pub window_kind: Option<crate::kindpick::Pick>,
    pub hole: [f64; 3],
    /// The library profile new columns and beams take; none, their own
    /// square, round or width × depth (MU40).
    pub column_section: Option<String>,
    pub beam_section: Option<String>,
    /// How a chain of beams meets at its corners (MU40).
    pub beam_corner: creabim_core::members::BeamCorner,
    /// Beams drawn in 3D lie on the face clicked (a roof slope, a wall),
    /// square to it, rather than level (MU40).
    pub beam_on_face: bool,
    /// The library profile new railings' top rail takes (MU40).
    pub railing_section: Option<String>,
    /// The beam this chain placed last: the next joins only it, and a new
    /// chain starts with none (MU40: a new Beam command joined the old one).
    pub last_beam: Option<ElementId>,
}

impl Default for MemberSettings {
    fn default() -> Self {
        Self {
            column: Profile::Rectangle {
                width: 0.3,
                depth: 0.3,
            },
            beam_width: 0.2,
            beam_depth: 0.4,
            roof_shape: RoofShape::Hip,
            roof_pitch: 30.0,
            stair_width: 1.0,
            stair_drawn: StairLine::Walk,
            stair_line: vec![],
            stair_rail_left: true,
            stair_winders: false,
            stair_spiral: false,
            stair_ramp: false,
            stair_split: false,
            stair_rail_right: true,
            stair_gap: 0.0,
            railing_height: 1.0,
            path: vec![],
            footing_pad: false,
            footing_width: creabim_core::footing::STRIP_WIDTH,
            footing_thickness: creabim_core::footing::STRIP_THICKNESS,
            door: METRIC_DOOR,
            door_kind: None,
            window_kind: None,
            window: METRIC_WINDOW,
            hole: [1.0, 1.0, 1.0],
            column_section: None,
            beam_section: None,
            beam_corner: Default::default(),
            beam_on_face: false,
            railing_section: None,
            last_beam: None,
        }
    }
}

/// The sizes a new door and window start at, width, height and sill, by
/// the project's units: metric, or feet and inches, where a door is 3'-0"
/// by 6'-8" and a window 3'-0" by 4'-0" with its head at the door's 6'-8".
const METRIC_DOOR: [f64; 3] = [0.9, 2.1, 0.0];
const METRIC_WINDOW: [f64; 3] = [1.2, 1.3, 0.9];
const INCH: f64 = 0.0254;
const IMPERIAL_DOOR: [f64; 3] = [36.0 * INCH, 80.0 * INCH, 0.0];
const IMPERIAL_WINDOW: [f64; 3] = [36.0 * INCH, 48.0 * INCH, 32.0 * INCH];

impl MemberSettings {
    /// The door's and window's sizes follow the project's units while
    /// they are still the ones they started at: a project in feet and
    /// inches started its doors at 900 × 2100 mm (film session,
    /// 2026-11-25). Sizes set by hand stay as they are.
    pub fn suit_units(&mut self, imperial: bool) {
        let (door, window) = if imperial {
            (
                (METRIC_DOOR, IMPERIAL_DOOR),
                (METRIC_WINDOW, IMPERIAL_WINDOW),
            )
        } else {
            (
                (IMPERIAL_DOOR, METRIC_DOOR),
                (IMPERIAL_WINDOW, METRIC_WINDOW),
            )
        };
        if self.door == door.0 {
            self.door = door.1;
        }
        if self.window == window.0 {
            self.window = window.1;
        }
    }

    /// Width (or diameter) of new columns.
    pub fn column_size(&self) -> f64 {
        match self.column {
            Profile::Rectangle { width, .. } => width,
            Profile::Round { diameter } => diameter,
        }
    }

    pub fn set_column_size(&mut self, v: f64) {
        self.column = match self.column {
            Profile::Rectangle { .. } => Profile::Rectangle { width: v, depth: v },
            Profile::Round { .. } => Profile::Round { diameter: v },
        };
    }

    pub fn round_columns(&self) -> bool {
        matches!(self.column, Profile::Round { .. })
    }

    pub fn set_round(&mut self, round: bool) {
        self.column_section = None;
        let v = self.column_size();
        self.column = if round {
            Profile::Round { diameter: v }
        } else {
            Profile::Rectangle { width: v, depth: v }
        };
    }
}

/// A roof from an outline drawn with the Slab tool's modes.
pub fn roof_from_slab(slab: &Slab, shape: RoofShape, pitch: f64) -> Element {
    let mut r = Roof::new(slab.level, slab.outline(), shape);
    r.pitch = pitch;
    r.base_offset = slab.top_offset;
    Element::new("Roof", ElementKind::Roof(r))
}

/// What the Slab tool's modes make: a slab, or with the Roof tool a roof.
pub fn outline_element(
    model: &creabim_core::Model,
    state: &PlanState,
    slab: Slab,
) -> (Element, &'static str) {
    if state.tool == Tool::Roof {
        let m = &state.members;
        (
            state.typed(model, roof_from_slab(&slab, m.roof_shape, m.roof_pitch)),
            "Draw roof",
        )
    } else if state.tool == Tool::Shaft {
        (
            crate::outline::shaft_from_outline(model, slab.level, slab.outline()),
            "Draw shaft",
        )
    } else {
        (
            state.typed(model, Element::new("Slab", ElementKind::Slab(slab))),
            "Draw slab",
        )
    }
}

/// A click with the Column, Beam, Stair or Railing tool on `level`.
pub fn click(
    doc: &mut Document,
    level: ElementId,
    tool: Tool,
    p: DVec2,
    state: &mut PlanState,
    selection: &mut Selection,
) -> Result<(), ModelError> {
    let set = state.members.clone();
    let add = |doc: &mut Document, selection: &mut Selection, e: Element, label: &str| {
        selection.only(e.id);
        doc.commit(Transaction::new(label).insert(e))
    };
    match tool {
        Tool::Column => {
            let mut c = Column::new(level, p, set.column);
            // a profile from the library, if one is picked (MU40)
            let lib = set
                .column_section
                .as_deref()
                .and_then(|n| Some((n, creabim_core::profiles::find(doc.model(), n)?)));
            if let Some((n, pts)) = &lib {
                c.section = Some(creabim_core::members::Section {
                    name: n.to_string(),
                    points: pts.clone(),
                });
            }
            state.status = Some(format!(
                "Column {} placed. Its height runs to the next level; drag its top handle or set a height in Properties.",
                lib.map_or_else(|| set.column.label(), |(n, _)| n.to_string())
            ));
            let e = state.typed(doc.model(), Element::new("Column", ElementKind::Column(c)));
            add(doc, selection, e, "Place column")
        }
        Tool::Beam => {
            match state.pending {
                None => {
                    state.members.last_beam = None;
                    state.pending = Some(p);
                    state.status = Some("Click the beam's other end.".into());
                    Ok(())
                }
                Some(a) if a.distance(p) > 0.05 => {
                    let mut b = Beam::new(level, a, p);
                    b.width = set.beam_width;
                    b.depth = set.beam_depth;
                    // a profile from the library, if one is picked (MU40)
                    if let Some(n) = set.beam_section.as_deref() {
                        if let Some(pts) = creabim_core::profiles::find(doc.model(), n) {
                            b.section = Some(creabim_core::members::Section {
                                name: n.to_string(),
                                points: pts,
                            });
                        }
                    }
                    // under the slab above, not inside it
                    b.top_offset = creabim_core::members::beam_top_offset(doc.model(), level, a, p);
                    state.pending = Some(p);
                    state.status = Some(
                        "Beam added. Click on to chain another; Esc or right-click ends.".into(),
                    );
                    let e = state.typed(doc.model(), Element::new("Beam", ElementKind::Beam(b)));
                    // the beam before it in the chain, ending here: the two
                    // meet as the tool's Corners says (MU40)
                    let prev = state
                    .members
                    .last_beam
                    .and_then(|id| doc.model().get(id).cloned())
                    .filter(|pe| matches!(&pe.kind, ElementKind::Beam(pb) if pb.end.distance(a) < 1e-6));
                    state.members.last_beam = Some(e.id);
                    match prev {
                        Some(pe) => {
                            let (mut pe2, mut ne) = (pe.clone(), e);
                            if let (ElementKind::Beam(pb), ElementKind::Beam(nb)) =
                                (&mut pe2.kind, &mut ne.kind)
                            {
                                creabim_core::members::join_beams(pb, nb, set.beam_corner);
                            }
                            selection.only(ne.id);
                            doc.commit(Transaction::new("Draw beam").replace(pe, pe2).insert(ne))
                        }
                        None => add(doc, selection, e, "Draw beam"),
                    }
                }
                Some(_) => Ok(()),
            }
        }
        Tool::Stair => {
            let line = &mut state.members.stair_line;
            // clicking the last point again (or a double-click) finishes
            if line.last().is_some_and(|q| q.distance(p) < 0.05) {
                return finish_stair(doc, level, state, selection);
            }
            line.push(p);
            state.pending = Some(p);
            let line = line.clone();
            match new_stair(doc.model(), level, &line, &set) {
                None if set.stair_spiral && line.len() >= 2 => {
                    line_too_close(state);
                    Ok(())
                }
                None if set.stair_spiral => {
                    state.status = Some(
                        "Click where the spiral starts: as far from its centre as its walk line runs.".into(),
                    );
                    Ok(())
                }
                None => {
                    state.status = Some(
                        "Click where the first flight turns, or where the stair ends. It finishes by itself when it reaches the floor above.".into(),
                    );
                    Ok(())
                }
                Some(s) => {
                    let lay = s.layout(s.rise(doc.model()));
                    if lay.short == 0 {
                        return finish_stair(doc, level, state, selection);
                    }
                    let placed = lay.treads() - lay.short.min(lay.treads());
                    state.status = Some(if s.is_ramp() {
                        // a ramp's run in metres, not its slices (MU99:
                        // "83 of 499 treads")
                        let run = |n: u32| crate::units::len(n as f64 * members::RAMP_SLICE);
                        format!(
                            "{} of {} run placed, {} to go. Click the next turn, or Enter to end here (the last flight runs on).",
                            run(placed),
                            run(lay.treads()),
                            run(lay.short)
                        )
                    } else {
                        format!(
                            "{placed} of {} treads placed, {} to go. Click the next turn, or Enter to end here (the last flight runs on).",
                            lay.treads(),
                            lay.short
                        )
                    });
                    Ok(())
                }
            }
        }
        Tool::Footing => place_footing(doc, level, p, state, selection),
        Tool::Railing => {
            if state.members.path.is_empty() {
                if let Some(stair) = stair_at(doc.model(), level, p) {
                    let before = doc.model().iter().count();
                    add_stair_railings(doc, stair, set.railing_height, selection)?;
                    state.status = Some(if doc.model().iter().count() == before {
                        "This stair has its railings on both sides already.".into()
                    } else {
                        "Railings added along the stair.".into()
                    });
                    return Ok(());
                }
            }
            let path = &mut state.members.path;
            if path.last().is_some_and(|q| q.distance(p) < 0.05) {
                // clicking the last point again finishes
                let pts = std::mem::take(path);
                state.pending = None;
                if pts.len() < 2 {
                    return Ok(());
                }
                let mut r = Railing::new(level, pts.iter().map(|q| q.extend(0.0)).collect());
                r.height = set.railing_height;
                r.rail_profile = set.railing_section.clone();
                state.status = Some("Railing added.".into());
                return add(
                    doc,
                    selection,
                    Element::new("Railing", ElementKind::Railing(r)),
                    "Draw railing",
                );
            }
            path.push(p);
            state.pending = Some(p);
            state.status = Some(
                "Click the next point; click the last one again to finish. Esc cancels.".into(),
            );
            Ok(())
        }
        _ => Ok(()),
    }
}

/// The kind of footing the Footing tool places: its type's, or the bar's.
pub fn footing_kind(model: &Model, state: &PlanState) -> creabim_core::FootingKind {
    use creabim_core::{types::TypeParams, FootingKind, TypeCategory};
    let typed = state
        .types
        .get(&TypeCategory::Footing)
        .and_then(|t| match &model.get(*t)?.kind {
            ElementKind::ElementType(t) => match &t.params {
                TypeParams::Footing(f) => Some(f.kind),
                _ => None,
            },
            _ => None,
        });
    typed.unwrap_or(if state.members.footing_pad {
        FootingKind::Pad
    } else {
        FootingKind::Strip
    })
}

/// A new footing sized from the tool bar, of the tool's type if it fits.
fn sized_footing(model: &Model, state: &PlanState, mut f: creabim_core::Footing) -> Element {
    let m = &state.members;
    f.width = match (f.kind, f.host) {
        // a strip under a wall is at least as wide as the wall
        (creabim_core::FootingKind::Strip, Some(_)) => f.width.max(m.footing_width),
        _ => m.footing_width,
    };
    if f.kind == creabim_core::FootingKind::Pad {
        f.length = m.footing_width;
    }
    f.thickness = m.footing_thickness;
    let name = f.kind.label();
    state.typed(model, Element::new(name, ElementKind::Footing(f)))
}

/// A click with the Footing tool: a strip under the wall clicked, or the
/// ends of one on its own; a pad under the column clicked, or one on its own.
fn place_footing(
    doc: &mut Document,
    level: ElementId,
    p: DVec2,
    state: &mut PlanState,
    selection: &mut Selection,
) -> Result<(), ModelError> {
    use creabim_core::{Footing, FootingKind};
    let model = doc.model();
    let kind = footing_kind(model, state);
    // the host the pointer showed (Tab may have moved past the first)
    let shown = state
        .host_pick
        .filter(|h| hosts_at(model, level, Tool::Footing, state, p).contains(h));
    let host = match kind {
        FootingKind::Strip if state.pending.is_none() => shown.or_else(|| wall_at(model, level, p)),
        FootingKind::Pad => shown.or_else(|| column_at(model, level, p)),
        FootingKind::Strip => None,
    };
    if let Some(h) = host {
        if !creabim_core::footing::footings_under(model, h).is_empty() {
            state.status = Some("That already has a footing under it.".into());
            return Ok(());
        }
        let (tx, ids) = creabim_core::footing::under(model, &[h], &|e| {
            if let ElementKind::Footing(f) = &e.kind {
                *e = sized_footing(model, state, f.clone());
            }
        });
        let name = model.get(h).map_or(String::new(), |e| e.name.clone());
        state.status = Some(format!(
            "{} footing placed under {name}. Ctrl+Z to undo.",
            match kind {
                FootingKind::Strip => "Strip",
                FootingKind::Pad => "Pad",
            }
        ));
        state.footing_placed = true;
        if let Some(id) = ids.first() {
            selection.only(*id);
        }
        let mut tx = tx;
        tx.label = format!("Place {}", kind.label().to_lowercase());
        return doc.commit(tx);
    }
    let add = |doc: &mut Document, selection: &mut Selection, e: Element| {
        selection.only(e.id);
        let label = format!("Place {}", kind.label().to_lowercase());
        doc.commit(Transaction::new(label).insert(e))
    };
    match kind {
        FootingKind::Pad => {
            let e = sized_footing(model, state, Footing::pad(level, p));
            state.status = Some("Pad footing placed. Set its size and depth in Properties.".into());
            add(doc, selection, e)
        }
        FootingKind::Strip => match state.pending.take() {
            None => {
                state.pending = Some(p);
                state.status = Some("Click the strip's other end.".into());
                Ok(())
            }
            Some(a) if a.distance(p) > 0.05 => {
                let e = sized_footing(model, state, Footing::strip(level, a, p));
                state.status = Some("Strip footing placed.".into());
                add(doc, selection, e)
            }
            Some(_) => Ok(()),
        },
    }
}

/// The wall on `level` under `p` (on it, or within a few centimetres).
pub fn wall_at(model: &Model, level: ElementId, p: DVec2) -> Option<ElementId> {
    walls_at(model, level, p).first().copied()
}

/// Every wall on `level` under `p`, nearest its centre first: across the
/// wall's whole thickness about its centreline (`start` and `end`), and a
/// few centimetres round it, whatever line it was drawn on.
pub fn walls_at(model: &Model, level: ElementId, p: DVec2) -> Vec<ElementId> {
    let mut hits: Vec<(f64, ElementId)> = model
        .iter()
        .filter_map(|e| match &e.kind {
            ElementKind::Wall(w) if w.level == level && w.curtain.is_none() => {
                // round the arc on a curved wall
                let (_, s) = w.along(p);
                (s <= w.thickness / 2.0 + 0.05).then_some((s, e.id))
            }
            _ => None,
        })
        .collect();
    hits.sort_by(|a, b| a.0.total_cmp(&b.0));
    hits.into_iter().map(|(_, id)| id).collect()
}

/// The project's foundation plan: a plan view whose name says Foundation.
pub fn foundation_plan(model: &Model) -> Option<ElementId> {
    model.iter().find_map(|e| match &e.kind {
        ElementKind::View(v)
            if matches!(v.kind, creabim_core::ViewKind::Plan { .. })
                && e.name.to_lowercase().contains("foundation") =>
        {
            Some(e.id)
        }
        _ => None,
    })
}

/// Does `tool` go on a host it is clicked onto (a footing under a wall or
/// column, a door or window in a wall)?
pub fn hosted(tool: Tool) -> bool {
    matches!(
        tool,
        Tool::Footing | Tool::Door | Tool::Window | Tool::Opening
    )
}

/// The hosts `tool` could go on at `p`, best first, before its first
/// click: these win over snaps (A3-2). None once a free strip has its
/// first point.
pub fn hosts_at(
    model: &Model,
    level: ElementId,
    tool: Tool,
    state: &PlanState,
    p: DVec2,
) -> Vec<ElementId> {
    use creabim_core::FootingKind;
    if state.pending.is_some() {
        return vec![];
    }
    match tool {
        Tool::Footing => match footing_kind(model, state) {
            FootingKind::Strip => walls_at(model, level, p),
            FootingKind::Pad => column_at(model, level, p).into_iter().collect(),
        },
        Tool::Door | Tool::Window | Tool::Opening => walls_at(model, level, p),
        _ => vec![],
    }
}

/// What a click would do on `host`, as the preview by the pointer says it:
/// "Strip 600 × 300 under W12", "Door in W12".
pub fn host_preview(model: &Model, state: &PlanState, tool: Tool, host: ElementId) -> String {
    let name = model.get(host).map_or(String::new(), |e| e.name.clone());
    match tool {
        Tool::Footing => {
            let kind = footing_kind(model, state);
            let mut f = match kind {
                creabim_core::FootingKind::Strip => {
                    creabim_core::Footing::strip(ElementId::NIL, DVec2::ZERO, DVec2::X)
                }
                creabim_core::FootingKind::Pad => {
                    creabim_core::Footing::pad(ElementId::NIL, DVec2::ZERO)
                }
            };
            f.host = Some(host);
            if let Some(ElementKind::Wall(w)) = model.get(host).map(|e| &e.kind) {
                f.width = w.thickness;
            }
            let ElementKind::Footing(f) = sized_footing(model, state, f).kind else {
                return name;
            };
            let mm = |v: f64| crate::units::len(v).trim_end_matches(" mm").to_string();
            let taken = !creabim_core::footing::footings_under(model, host).is_empty();
            if taken {
                return format!("{name} already has a footing");
            }
            match kind {
                creabim_core::FootingKind::Strip => {
                    format!("Strip {} × {} under {name}", mm(f.width), mm(f.thickness))
                }
                creabim_core::FootingKind::Pad => format!(
                    "Pad {} × {} × {} under {name}",
                    mm(f.width),
                    mm(f.length),
                    mm(f.thickness)
                ),
            }
        }
        Tool::Door => format!("Door in {name}"),
        Tool::Window => format!("Window in {name}"),
        _ => format!("Opening in {name}"),
    }
}

/// The column standing on `level` under `p`.
pub fn column_at(model: &Model, level: ElementId, p: DVec2) -> Option<ElementId> {
    model.iter().find_map(|e| match &e.kind {
        ElementKind::Column(c) if c.level == level => {
            let o = c.outline();
            (inside(&o, p) || c.position.distance(p) < 0.1).then_some(e.id)
        }
        _ => None,
    })
}

/// A stair drawn by `line`, sized from the tool's settings; none until
/// the line has a length.
pub fn new_stair(
    model: &Model,
    level: ElementId,
    line: &[DVec2],
    set: &MemberSettings,
) -> Option<Stair> {
    let mut s = if set.stair_spiral {
        new_spiral(model, level, line, set)?
    } else {
        let mut s = Stair::from_line(level, line, set.stair_drawn)?;
        s.width = set.stair_width;
        // its sharp turns made half-turns at its own width, the runs the
        // gap apart (MU101, REV02 §5)
        s.set_line_gap(line, set.stair_gap);
        if set.stair_winders {
            s.at_turns = members::TurnKind::Winders(0);
        }
        s
    };
    if set.stair_ramp {
        s.make_ramp(1.0 / 12.0);
    }
    s.split = set.stair_split;
    Some(s)
}

/// A stair's landings' sizes at their centres, as temporary dimensions:
/// "Landing 1 000 · gap 200" (Stair Guides REV02 §5).
pub fn draw_landing_sizes(
    painter: &egui::Painter,
    at: &dyn Fn(DVec2) -> Option<Pos2>,
    sizes: &[(DVec2, f64, Option<f64>)],
) {
    let accent = crate::theme::interaction();
    for (c, depth, gap) in sizes {
        let Some(p) = at(*c) else { continue };
        let mut text = format!("Landing {}", crate::units::len(*depth));
        if let Some(g) = gap {
            text.push_str(&format!(" · gap {}", crate::units::len(*g)));
        }
        let galley = painter.layout_no_wrap(text, egui::FontId::proportional(11.5), accent);
        let r = egui::Rect::from_center_size(p, galley.size()).expand2(egui::vec2(5.0, 2.0));
        painter.rect(
            r,
            4.0,
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 225),
            Stroke::new(1.0, accent),
            egui::StrokeKind::Inside,
        );
        painter.galley(r.min + egui::vec2(5.0, 2.0), galley, accent);
    }
}

/// A spiral's start clicked too close to its centre for the stair's
/// width: take it back and say so.
fn line_too_close(state: &mut PlanState) {
    state.members.stair_line.pop();
    state.pending = state.members.stair_line.last().copied();
    state.status = Some("Too close to the centre for the stair's width: click further out.".into());
}

/// A spiral stair round centre `line[0]`, starting at `line[1]` and
/// climbing anticlockwise round arcs of half a turn until it reaches the
/// floor above. None until the start is far enough out for its width.
fn new_spiral(
    model: &Model,
    level: ElementId,
    line: &[DVec2],
    set: &MemberSettings,
) -> Option<Stair> {
    let [centre, start, ..] = line else {
        return None;
    };
    let (centre, start) = (*centre, *start);
    let r0 = centre.distance(start);
    // the walk line's radius: a side drawn is that far out or in
    let mut probe = Stair::from_line(level, &[start, start + DVec2::X], set.stair_drawn)?;
    probe.width = set.stair_width;
    let walk_r = r0 - set.stair_drawn.to_walk(set.stair_width);
    if walk_r < set.stair_width / 2.0 + 0.05 {
        return None;
    }
    let rise = probe.rise(model);
    let n = members::riser_count(rise, probe.max_riser);
    let length = n.saturating_sub(1) as f64 * probe.tread + probe.tread;
    let sweep = length / walk_r;
    let arcs = (sweep / std::f64::consts::PI).ceil().max(1.0) as usize;
    let each = sweep / arcs as f64;
    let pts: Vec<DVec2> = (0..=arcs)
        .map(|k| centre + DVec2::from_angle(each * k as f64).rotate(start - centre))
        .collect();
    let mut s = Stair::from_line(level, &pts, set.stair_drawn)?;
    s.width = set.stair_width;
    s.line = pts.iter().map(|p| s.to_local(*p)).collect();
    for k in 0..arcs {
        s.set_bend(k, each.to_degrees());
    }
    Some(s)
}

/// The Stair tool's line made into a stair, with its railings.
pub fn finish_stair(
    doc: &mut Document,
    level: ElementId,
    state: &mut PlanState,
    selection: &mut Selection,
) -> Result<(), ModelError> {
    let line = std::mem::take(&mut state.members.stair_line);
    state.pending = None;
    let set = state.members.clone();
    let Some(mut s) = new_stair(doc.model(), level, &line, &set) else {
        state.status = Some("A stair needs a line with a length.".into());
        return Ok(());
    };
    let rise = s.rise(doc.model());
    s.trim(rise);
    let lay = s.layout(rise);
    let ramp = s.is_ramp();
    let e = Element::new(
        if ramp { "Ramp" } else { "Stair" },
        ElementKind::Stair(s.clone()),
    );
    let id = e.id;
    let mut tx = Transaction::new(if ramp { "Draw ramp" } else { "Draw stair" }).insert(e);
    let mut kinds = vec![];
    for (left, on) in [(true, set.stair_rail_left), (false, set.stair_rail_right)] {
        let side = if left { "left" } else { "right" };
        if !on {
            kinds.push(format!("none {side}"));
            continue;
        }
        let r = railing_for(doc.model(), &s, id, left);
        kinds.push(format!(
            "{} {side}",
            if r.on_wall {
                "a wall handrail"
            } else {
                "a guard"
            }
        ));
        tx = tx.insert(Element::new("Railing", ElementKind::Railing(r)));
    }
    let rails = if set.stair_rail_left || set.stair_rail_right {
        format!(" Railings: {}.", kinds.join(", "))
    } else {
        String::new()
    };
    let what = if ramp {
        format!(
            "Ramp added: {}, rise {}",
            creabim_geom::members::ramp_tag(s.tread / lay.riser.max(1e-9)),
            crate::units::len(rise)
        )
    } else {
        format!(
            "Stair added: {} risers of {}",
            lay.risers,
            crate::units::len(lay.riser)
        )
    };
    let short = if ramp {
        crate::units::len(lay.short as f64 * s.tread)
    } else {
        format!("{} treads", lay.short)
    };
    state.status = Some(format!(
        "{what}, {} flight{}, {} landing{}.{}{}",
        lay.flights.len(),
        if lay.flights.len() == 1 { "" } else { "s" },
        lay.landings.len(),
        if lay.landings.len() == 1 { "" } else { "s" },
        if lay.short > 0 {
            format!(" The line was {short} short: the last flight runs on past it.")
        } else {
            String::new()
        },
        rails
    ));
    selection.only(id);
    doc.commit(tx)
}

/// The railing for one side of a stair: a handrail on brackets where a
/// wall runs beside it, else a guard; both run on past the ends.
pub fn stair_railing(model: &Model, stair: ElementId, left: bool) -> Option<Railing> {
    let ElementKind::Stair(s) = &model.get(stair)?.kind else {
        return None;
    };
    Some(railing_for(model, s, stair, left))
}

/// [`stair_railing`] for a stair that may not be in the model yet.
fn railing_for(model: &Model, s: &Stair, stair: ElementId, left: bool) -> Railing {
    let mut r = Railing::new(s.level, vec![]);
    r.stair = Some(members::StairSide { stair, left });
    r.on_wall = members::stair_side_on_wall(model, s, left);
    r.height = if r.on_wall { HANDRAIL } else { GUARD };
    r.extend = Some(RailExtend::default());
    // a rail a hand can hold (NBC 9.8.7.5)
    r.rail_profile = Some("Handrail".into());
    // a guard: balusters no 100 mm sphere passes, newels at its ends and
    // corners
    if !r.on_wall {
        r.infill = members::RailInfill::Balusters;
        r.newels = true;
    }
    r
}

/// A stair's handrail height over the nosings, and its guard's.
pub const HANDRAIL: f64 = 0.9;
pub const GUARD: f64 = 0.9;

/// The stair on `level` whose footprint holds `p`.
fn stair_at(model: &Model, level: ElementId, p: DVec2) -> Option<ElementId> {
    model.iter().find_map(|e| match &e.kind {
        ElementKind::Stair(s) if s.level == level => {
            let shapes = s.outlines(s.rise(model));
            shapes.iter().any(|o| inside(o, p)).then_some(e.id)
        }
        _ => None,
    })
}

fn inside(poly: &[DVec2], p: DVec2) -> bool {
    let n = poly.len();
    let mut c = false;
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x) {
            c = !c;
        }
    }
    c
}

/// The railing along the `left` (else right) side of `stair`, if any.
pub fn stair_side_railing(model: &Model, stair: ElementId, left: bool) -> Option<ElementId> {
    model.iter().find_map(|e| match &e.kind {
        ElementKind::Railing(r)
            if r.stair
                .is_some_and(|sd| sd.stair == stair && sd.left == left) =>
        {
            Some(e.id)
        }
        _ => None,
    })
}

/// Give `stair` a railing along its `left` (else right) side, or take
/// it away: the stair's Left and Right railing boxes.
pub fn set_stair_railing(
    doc: &mut Document,
    stair: ElementId,
    left: bool,
    on: bool,
) -> Result<(), ModelError> {
    let side = if left { "left" } else { "right" };
    match (stair_side_railing(doc.model(), stair, left), on) {
        (None, true) => {
            let Some(r) = stair_railing(doc.model(), stair, left) else {
                return Ok(());
            };
            doc.commit(
                Transaction::new(format!("Add {side} stair railing"))
                    .insert(Element::new("Railing", ElementKind::Railing(r))),
            )
        }
        (Some(id), false) => doc.commit(
            Transaction::new(format!("Remove {side} stair railing"))
                .remove_cascade(doc.model(), id),
        ),
        _ => Ok(()),
    }
}

/// Railings on both sides of `stair`, as one undo step.
pub fn add_stair_railings(
    doc: &mut Document,
    stair: ElementId,
    height: f64,
    selection: &mut Selection,
) -> Result<(), ModelError> {
    let mut tx = Transaction::new("Add stair railings");
    let mut first = None;
    let _ = height;
    for left in [true, false] {
        // a side that has its railing already keeps it
        let railed = doc.model().iter().any(|e| {
            matches!(&e.kind, ElementKind::Railing(r)
                if r.stair.is_some_and(|sd| sd.stair == stair && sd.left == left))
        });
        if railed {
            continue;
        }
        if let Some(r) = stair_railing(doc.model(), stair, left) {
            let e = Element::new("Railing", ElementKind::Railing(r));
            first.get_or_insert(e.id);
            tx = tx.insert(e);
        }
    }
    let Some(id) = first else {
        return Ok(());
    };
    selection.only(id);
    doc.commit(tx)
}

/// What the tool would make, drawn while its second point is chosen:
/// a beam's width, a stair's flights, a railing's path so far.
#[allow(clippy::too_many_arguments)]
pub fn preview(
    painter: &egui::Painter,
    project: &dyn Fn(DVec3) -> Option<Pos2>,
    model: &Model,
    level: Option<ElementId>,
    tool: Tool,
    state: &PlanState,
    a: DVec3,
    b: DVec3,
) {
    let stroke = Stroke::new(1.0, crate::theme::interaction());
    let z = a.z;
    let ring = |pts: &[DVec2]| {
        let scr: Option<Vec<Pos2>> = pts
            .iter()
            .chain(pts.first())
            .map(|p| project(p.extend(z)))
            .collect();
        if let Some(scr) = scr {
            painter.add(egui::Shape::line(scr, stroke));
        }
    };
    let set = &state.members;
    match (tool, level) {
        (Tool::Beam, Some(level)) if a.distance(b) > 1e-3 => {
            let mut bm = Beam::new(level, a.truncate(), b.truncate());
            bm.width = set.beam_width;
            ring(&bm.outline());
        }
        (Tool::Stair, Some(level)) if a.distance(b) > 1e-3 => {
            let mut line = set.stair_line.clone();
            line.push(b.truncate());
            let Some(s) = new_stair(model, level, &line, set) else {
                return;
            };
            let rise = s.rise(model);
            for o in s.outlines(rise) {
                ring(&o);
            }
            let lay = s.layout(rise);
            // the landings as they come, with their depth and a half-turn's
            // gap between runs (Tab types the gap: REV02 §5)
            draw_landing_sizes(painter, &|p| project(p.extend(z)), &s.landing_sizes(rise));
            let walk: Option<Vec<Pos2>> = lay
                .walk
                .iter()
                .map(|p| project(p.truncate().extend(z)))
                .collect();
            if let Some(walk) = walk {
                painter.add(egui::Shape::dashed_line(&walk, stroke, 6.0, 4.0));
            }
            // the drawn line, so a side-drawn stair shows what is being drawn
            let drawn: Option<Vec<Pos2>> = line.iter().map(|p| project(p.extend(z))).collect();
            if let Some(drawn) = drawn {
                painter.add(egui::Shape::line(drawn, Stroke::new(2.0, stroke.color)));
            }
            if let Some(at) = project(b) {
                let treads = lay.treads();
                let text = if s.is_ramp() {
                    // a ramp's slices are no use to count: its length
                    let len = treads as f64 * s.tread;
                    let to_go = lay.short as f64 * s.tread;
                    if lay.short > 0 {
                        format!("ramp {:.1} m · {:.1} m to go", len - to_go, to_go)
                    } else {
                        format!("ramp {len:.1} m · reaches the floor above")
                    }
                } else if lay.short > 0 {
                    format!(
                        "{} of {} treads · {} to go",
                        treads - lay.short.min(treads),
                        treads,
                        lay.short
                    )
                } else {
                    format!("{} treads · reaches the floor above", treads)
                };
                let color = if lay.short > 0 {
                    crate::theme::interaction()
                } else {
                    egui::Color32::from_rgb(0x2e, 0x9e, 0x5b)
                };
                painter.text(
                    at + egui::vec2(12.0, -12.0),
                    egui::Align2::LEFT_BOTTOM,
                    text,
                    egui::FontId::proportional(13.0),
                    color,
                );
            }
        }
        (Tool::Footing, Some(level)) if a.distance(b) > 1e-3 => {
            let mut f = creabim_core::Footing::strip(level, a.truncate(), b.truncate());
            f.width = set.footing_width;
            let n = (b - a).truncate().normalize_or_zero().perp() * f.width / 2.0;
            ring(&[
                a.truncate() - n,
                b.truncate() - n,
                b.truncate() + n,
                a.truncate() + n,
            ]);
        }
        (Tool::Railing, _) => {
            let scr: Option<Vec<Pos2>> = set.path.iter().map(|p| project(p.extend(z))).collect();
            if let Some(scr) = scr {
                painter.add(egui::Shape::line(scr, Stroke::new(2.0, stroke.color)));
            }
        }
        _ => {}
    }
}

/// The Properties panel rows for a column, beam, roof, stair or railing.
/// Returns the undo label when something changed.
pub fn properties(ui: &mut egui::Ui, model: &Model, kind: &mut ElementKind) -> &'static str {
    if matches!(kind, ElementKind::Part(_) | ElementKind::Site(_)) {
        return crate::partstool::properties(ui, model, kind);
    }
    let mut label = "";
    let num = |ui: &mut egui::Ui, text: &str, v: &mut f64, lo: f64, hi: f64| -> bool {
        crate::props::row(ui, text, |ui| {
            crate::props::fill(ui, crate::units::drag(v).range(lo..=hi)).changed()
        })
    };
    let deg = |ui: &mut egui::Ui, text: &str, v: &mut f64, lo: f64, hi: f64| -> bool {
        crate::props::row(ui, text, |ui| {
            crate::props::fill(
                ui,
                egui::DragValue::new(v)
                    .speed(0.5)
                    .range(lo..=hi)
                    .suffix("°"),
            )
            .changed()
        })
    };
    // what is worked out from the model reads in the Worked out plate,
    // problems in a warning row, the why in the ? box (AMD-012, AMD-015)
    use crate::props::Worked;
    match kind {
        ElementKind::Column(c) => {
            let mut round = matches!(c.profile, Profile::Round { .. });
            if crate::props::choice(
                ui,
                "Profile",
                &mut round,
                &[(false, "Rectangular", ""), (true, "Round", "")],
            ) {
                label = "Column profile";
            }
            if label == "Column profile" {
                c.profile = match (round, c.profile) {
                    (true, Profile::Rectangle { width, .. }) => Profile::Round { diameter: width },
                    (false, Profile::Round { diameter }) => Profile::Rectangle {
                        width: diameter,
                        depth: diameter,
                    },
                    (_, p) => p,
                };
            }
            match &mut c.profile {
                Profile::Rectangle { width, depth } => {
                    if num(ui, "Width", width, 0.05, 5.0) | num(ui, "Depth", depth, 0.05, 5.0) {
                        label = "Column size"
                    }
                    if deg(ui, "Rotation", &mut c.rotation, -360.0, 360.0) {
                        label = "Turn column"
                    }
                }
                Profile::Round { diameter } => {
                    if num(ui, "Diameter", diameter, 0.05, 5.0) {
                        label = "Column size"
                    }
                }
            }
            if c.top.is_unconnected() {
                let mut fixed = c.height.is_some();
                if crate::props::check(ui, "Height", &mut fixed, "Fixed")
                    .on_hover_text(
                        "Its own height; unticked, it runs up to the next level and follows it",
                    )
                    .changed()
                {
                    c.height = fixed.then(|| c.height_in(model));
                    label = "Column height";
                }
                if let Some(h) = &mut c.height {
                    if num(ui, "Height", h, 0.1, 100.0) {
                        label = "Column height"
                    }
                }
            }
            let l = column_levels(ui, model, c);
            if !l.is_empty() {
                label = l;
            }
            if num(ui, "X", &mut c.position.x, -1e4, 1e4)
                | num(ui, "Y", &mut c.position.y, -1e4, 1e4)
            {
                label = "Move column"
            }
            let (z0, z1) = c.span(model);
            crate::props::worked_later(
                ui,
                "column",
                &[
                    Worked::new("Base", crate::units::elevation(z0)),
                    Worked::new("Top", crate::units::elevation(z1)),
                ],
            );
        }
        ElementKind::Footing(f) => {
            let l = footing_fields(ui, model, f);
            if !l.is_empty() {
                label = l;
            }
        }
        ElementKind::Beam(b) => {
            if num(ui, "Width", &mut b.width, 0.02, 3.0) {
                label = "Beam width"
            }
            if num(ui, "Depth", &mut b.depth, 0.02, 5.0) {
                label = "Beam depth"
            }
            if crate::props::num_tip(
                ui,
                "Below top",
                "Below storey top: how far the beam's top is under the top of its storey",
                &mut b.top_offset,
                -10.0,
                50.0,
            ) {
                label = "Beam top offset"
            }
            if num(ui, "Start X", &mut b.start.x, -1e4, 1e4)
                | num(ui, "Start Y", &mut b.start.y, -1e4, 1e4)
                | num(ui, "End X", &mut b.end.x, -1e4, 1e4)
                | num(ui, "End Y", &mut b.end.y, -1e4, 1e4)
            {
                label = "Move beam"
            }
            // a sloped beam: each end's top above the beam's top (MU40)
            if num(ui, "Start rise", &mut b.rise[0], -50.0, 50.0)
                | num(ui, "End rise", &mut b.rise[1], -50.0, 50.0)
            {
                label = "Beam slope"
            }
            // each end run past its point, or pulled back where a column
            // or wall runs through (MU40)
            if num(ui, "Start extension", &mut b.extend[0], -20.0, 20.0)
                | num(ui, "End extension", &mut b.extend[1], -20.0, 20.0)
            {
                label = "Beam extension"
            }
            let cuts = b.cuts.iter().filter(|c| c.is_some()).count();
            if cuts > 0 || b.up.is_some() {
                crate::props::row(ui, "Ends", |ui| {
                    ui.label(match cuts {
                        0 => "Square".to_string(),
                        1 => "One mitred".to_string(),
                        _ => "Both mitred".to_string(),
                    })
                });
                let (square, plumb) = crate::props::actions2(
                    ui,
                    "",
                    ("Square ends", "Take the mitre cuts off its ends", cuts > 0),
                    (
                        "Stand plumb",
                        "Its section upright again, not square to the face it was drawn on",
                        b.up.is_some(),
                    ),
                );
                if square {
                    b.cuts = [None, None];
                    label = "Square ends";
                }
                if plumb {
                    b.up = None;
                    label = "Beam plumb";
                }
            }
            crate::props::worked_later(
                ui,
                "beam",
                &[
                    Worked::new("Length", crate::units::len(b.length())),
                    Worked::new("Top", crate::units::elevation(b.top(model))),
                ],
            );
            crate::props::info_later(ui, "Plans show a beam dashed when it is above the cut.", "");
        }
        ElementKind::Roof(r) => {
            // each shape with its own picture
            crate::props::Combo::new("Shape")
                .selected_text(r.shape.label())
                .show_ui(ui, |ui| {
                    for s in RoofShape::ALL {
                        let pick = crate::props::picture_option(
                            ui,
                            r.shape == s,
                            s.label(),
                            |p, rect, st| roof_glyph(p, rect, st, s),
                        );
                        if pick.clicked() && r.shape != s {
                            r.shape = s;
                            label = "Roof shape"
                        }
                    }
                });
            if r.shape != RoofShape::Flat && deg(ui, "Pitch", &mut r.pitch, 1.0, 75.0) {
                label = "Roof pitch"
            }
            if num(ui, "Overhang", &mut r.overhang, 0.0, 5.0) {
                label = "Roof overhang"
            }
            if num(ui, "Thickness", &mut r.thickness, 0.02, 2.0) {
                label = "Roof thickness"
            }
            if num(ui, "Base offset", &mut r.base_offset, -10.0, 100.0) {
                label = "Roof base offset"
            }
            // where the eaves end: a steep pitch with a long overhang hangs
            // them down past the walls, which then look gone (seen
            // replaying a 55° hip with 2 m eaves over 3 m walls)
            let mut eaves = None;
            if r.shape != RoofShape::Flat && r.overhang > 0.0 {
                let drop = r.overhang * r.pitch.to_radians().tan();
                let floor = creabim_core::members::elevation(model, r.level);
                eaves = Some((r.base(model) - drop - floor, drop));
            }
            let n = r.boundary.len();
            if matches!(r.shape, RoofShape::Shed | RoofShape::Gable) && n > 0 {
                let what = if r.shape == RoofShape::Shed {
                    "Low edge"
                } else {
                    "Eave edge"
                };
                crate::props::row(ui, what, |ui| {
                    // a fixed row in whole pixels: a wrapping one sent ›
                    // to a second line and back as the panel was dragged
                    // (MU59)
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 1.0;
                        let h = ui.spacing().interact_size.y;
                        let w = ((ui.available_width() - 2.0) / 3.0).floor().max(24.0);
                        if ui
                            .add_sized([w, h], egui::Button::new("‹"))
                            .on_hover_text("The edge before")
                            .clicked()
                        {
                            r.edge = (r.edge + n - 1) % n;
                            label = "Roof edge";
                        }
                        ui.add_sized(
                            [w, h],
                            egui::Label::new(format!("{} of {n}", r.edge % n + 1)),
                        );
                        if ui
                            .add_sized([w, h], egui::Button::new("›"))
                            .on_hover_text("The next edge")
                            .clicked()
                        {
                            r.edge = (r.edge + 1) % n;
                            label = "Roof edge";
                        }
                    })
                });
            }
            let mut worked = vec![
                Worked::new("Corners", n.to_string()),
                Worked::new("Area", crate::units::area(r.plan_area())),
                Worked::new("Height", crate::units::len(r.peak())),
            ];
            if let Some((eave, drop)) = eaves {
                worked.push(Worked::new("Eaves", crate::units::len(eave)).warn_if(eave < 2.1));
                worked.push(Worked::new("Eave drop", crate::units::len(drop)));
            }
            crate::props::worked_later(ui, "roof", &worked);
            if r.effective_shape() != r.shape {
                crate::props::warning(
                    ui,
                    "Built flat: gable and hip roofs need a convex outline.",
                    None,
                );
            }
            if let Some((eave, _)) = eaves.filter(|(e, _)| *e < 2.1) {
                if crate::props::warning(
                    ui,
                    format!(
                        "Eaves at {} hang below head height and hide the walls under them.",
                        crate::units::len(eave)
                    ),
                    Some("Raise the base"),
                ) {
                    r.base_offset += 2.1 - eave;
                    label = "Roof base offset";
                }
            }
        }
        ElementKind::Stair(s) => {
            let before = s.clone();
            if num(ui, "Width", &mut s.width, 0.5, 5.0) {
                label = "Stair width"
            }
            // the end that stays put when its treads change (REV00 §3, A)
            if !s.line.is_empty() && !s.curved() {
                let mut foot = s.fixed_foot;
                if crate::props::choice(
                    ui,
                    "Stays put",
                    &mut foot,
                    &[
                        (false, "Head", "Its head stays against the floor above; more treads grow it from the foot"),
                        (true, "Foot", "Its foot stays; more treads grow it from the head"),
                    ],
                ) {
                    s.fixed_foot = foot;
                    label = "Stair fixed end";
                }
            }
            // a ramp: the same line, landings and railings, one smooth slope
            let mut ramp = s.is_ramp();
            if crate::props::check(ui, "Ramp", &mut ramp, "One slope")
                .on_hover_text("One smooth slope instead of steps, on the same line, with its landings and railings")
                .changed()
            {
                if ramp {
                    s.make_ramp(1.0 / 12.0);
                } else {
                    s.make_steps();
                }
                label = if ramp { "Make a ramp" } else { "Make steps" };
            }
            // split: the line is one arm, mirrored at its first turn
            if s.line.len() >= 3 && !s.curved() {
                let mut split = s.split;
                if crate::props::check(ui, "Split", &mut split, "Two arms")
                    .on_hover_text(
                        "Its mirror image beside it: one flight twice as wide up to the \
                         first landing, then two arms. Its line is one arm.",
                    )
                    .changed()
                {
                    s.split = split;
                    label = if split {
                        "Split stair"
                    } else {
                        "Unsplit stair"
                    };
                }
            }
            if let Some(slope) = s.ramp {
                // shown as its run for one of rise: 1:12
                let mut run = 1.0 / slope.max(1e-6);
                let changed = crate::props::row(ui, "Slope 1:", |ui| {
                    crate::props::fill(
                        ui,
                        egui::DragValue::new(&mut run).speed(0.1).range(2.0..=50.0),
                    )
                    .changed()
                });
                if changed {
                    s.set_slope(1.0 / run);
                    label = "Ramp slope";
                }
                if num(ui, "Thickness", &mut s.waist, 0.05, 0.6) {
                    label = "Ramp thickness"
                }
            } else {
                if num(ui, "Max riser", &mut s.max_riser, 0.1, 0.3) {
                    label = "Stair riser"
                }
                if num(ui, "Tread", &mut s.tread, 0.15, 0.6) {
                    label = "Stair tread"
                }
                // how it is built: what carries the treads
                crate::props::row(ui, "Built as", |ui| {
                    let w = ui.available_width();
                    egui::ComboBox::from_id_salt("stair.build")
                        .icon(crate::props::combo_icon)
                        .width(w)
                        .truncate()
                        .selected_text(s.build.label())
                        .show_ui(ui, |ui| {
                            for b in members::StairBuild::ALL {
                                if ui.selectable_label(s.build == b, b.label()).clicked() {
                                    s.build = b;
                                    label = "Stair construction";
                                }
                            }
                        });
                });
                let thick = match s.build {
                    members::StairBuild::FoldedPlate => Some("Plate"),
                    b if b.open() => None,
                    _ => Some("Waist"),
                };
                if let Some(name) = thick {
                    let low = if name == "Plate" { 0.005 } else { 0.05 };
                    if num(ui, name, &mut s.waist, low, 0.6) {
                        label = "Stair waist"
                    }
                }
                if s.build == members::StairBuild::Post && s.centre().is_none() {
                    crate::props::warning(
                        ui,
                        "A spiral post needs a curved line to stand at the centre of: until then it is built on a central spine.",
                        None,
                    );
                }
            }
            if deg(ui, "Direction", &mut s.direction, -360.0, 360.0) {
                label = "Turn stair"
            }
            let mut fixed = s.height.is_some();
            if crate::props::check(ui, "Rise", &mut fixed, "Fixed")
                .on_hover_text("Its own rise; unticked, it climbs to the next level and follows it")
                .changed()
            {
                s.height = fixed.then(|| s.rise(model));
                label = "Stair rise";
            }
            if let Some(h) = &mut s.height {
                if num(ui, "Rise", h, 0.1, 20.0) {
                    label = "Stair rise"
                }
            }
            let rise = s.rise(model);
            let n = members::riser_count(rise, s.max_riser);
            if !s.line.is_empty() {
                let name = |d: StairLine| match d {
                    StairLine::Walk => "Walk line",
                    StairLine::Left => "Left side",
                    StairLine::Right => "Right side",
                };
                crate::props::row(ui, "Drawn by", |ui| {
                    egui::ComboBox::from_id_salt("stair.drawn")
                        .icon(crate::props::combo_icon)
                        .width(ui.available_width())
                        .selected_text(name(s.drawn))
                        .show_ui(ui, |ui| {
                            for (d, tip) in [
                                (StairLine::Walk, "The line runs up the middle"),
                                (StairLine::Left, "The line is the stair's left edge"),
                                (StairLine::Right, "The line is the stair's right edge"),
                            ] {
                                // the tool's pictures (REV00 A)
                                if crate::props::picture_option(
                                    ui,
                                    s.drawn == d,
                                    name(d),
                                    |p, r, st| {
                                        crate::svgicon::paint(p, r, stair_line_icon(d), st.color)
                                    },
                                )
                                .on_hover_text(tip)
                                .clicked()
                                {
                                    // the stair stays put; its line moves (MU101)
                                    s.set_drawn_in_place(d, rise);
                                    label = "Stair drawn by";
                                }
                            }
                        });
                });
            }
            let mut winders = !s.at_turns.is_landing();
            if s.line.len() >= 3
                && !s.is_ramp()
                && turn_combo(ui, "Turns", "stair.turns", &mut winders, true)
            {
                s.at_turns = if winders {
                    members::TurnKind::Winders(0)
                } else {
                    members::TurnKind::Landing
                };
                // turns given their own kind keep it where it differs
                let at = s.at_turns;
                s.pieces.turns.retain(|t| t.kind != at);
                label = "Stair turns";
            }
            // a stair from before lines: its flights and landing as a line,
            // to reshape like any other
            if s.line.is_empty()
                && crate::props::action(
                    ui,
                    "Line",
                    "Make it a line stair",
                    "Its walk line as it is now, which then opens with it (Enter) to drag, add turns or take them away.",
                    true,
                )
                .clicked()
            {
                let line = s.line_in_plan(rise);
                s.drawn = StairLine::Walk;
                s.set_line(&line);
                label = "Stair by its line";
            }
            let mut turn = s.landing.is_some();
            if s.line.is_empty()
                && crate::props::check(ui, "Landing", &mut turn, "L-shaped")
                    .on_hover_text("A landing partway up, the stair turning at it")
                    .changed()
            {
                s.landing = turn.then_some(members::Landing {
                    after: n / 2,
                    left: true,
                });
                label = "Stair landing";
            }
            if let Some(l) = &mut s.landing {
                // full rows in the label and field columns (MU31)
                crate::props::row(ui, "Landing after", |ui| {
                    let d = egui::DragValue::new(&mut l.after)
                        .range(1..=n.saturating_sub(1).max(1))
                        .suffix(" risers");
                    if crate::props::fill(ui, d).changed() {
                        label = "Stair landing";
                    }
                });
                if crate::props::choice(
                    ui,
                    "Turns",
                    &mut l.left,
                    &[(true, "Left", ""), (false, "Right", "")],
                ) {
                    label = "Stair turn";
                }
            }
            let lay = s.layout(rise);
            let going: f64 = lay.flights.iter().map(|f| f.run(s.tread)).sum();
            let r2g = 2.0 * lay.riser + s.tread;
            if !s.line.is_empty() && lay.short > 0 {
                crate::props::warning(
                    ui,
                    format!(
                        "The line is {} tread{} short: the last flight runs on past its end. Drag the line's end on, or Enter to reshape it.",
                        lay.short,
                        if lay.short == 1 { "" } else { "s" }
                    ),
                    None,
                );
            }
            let counts = || {
                let mut c = vec![Worked::new("Flights", lay.flights.len().to_string())];
                if !lay.landings.is_empty() {
                    c.push(Worked::new("Landings", lay.landings.len().to_string()));
                }
                if !lay.winders.is_empty() {
                    c.push(Worked::new("Winders", lay.winders.len().to_string()));
                }
                c
            };
            if s.is_ramp() {
                let mut rows = vec![
                    Worked::new(
                        "Slope",
                        // "Ramp 1:12 · 8.3%" without its word, to fit the plate
                        creabim_geom::members::ramp_tag(1.0 / s.slope(model).max(1e-9))
                            .trim_start_matches("Ramp ")
                            .to_string(),
                    ),
                    Worked::new("Rise", crate::units::len(rise)),
                    Worked::new("Run", crate::units::len(going)),
                ];
                rows.extend(counts());
                crate::props::worked_later(ui, "stair", &rows);
            } else {
                crate::props::worked_later(ui, "stair", &counts());
                crate::props::worked_later(
                    ui,
                    "stair",
                    &[
                        Worked::new("Risers", lay.risers.to_string()),
                        Worked::new("Riser", crate::units::len(lay.riser)),
                        Worked::new("Rise", crate::units::len(rise)),
                        Worked::new("Going", crate::units::len(going)),
                        // outside the usual 550–700 mm, in the warn colour
                        Worked::new("2R + G", crate::units::len(r2g))
                            .warn_if(!(0.55..=0.70).contains(&r2g)),
                    ],
                );
            }
            // a change to its treads (not a move, turn or new line) keeps
            // the fixed end where it was
            // ...and only that: making it a ramp or steps, or splitting it,
            // is not a change of treads (MU103: ticking Ramp moved the
            // foot the length of the ramp, and unticking didn't bring it
            // back)
            let treads_changed = before.tread != s.tread
                || before.max_riser != s.max_riser
                || before.height != s.height;
            if !label.is_empty()
                && treads_changed
                && !before.is_ramp()
                && !s.is_ramp()
                && before.split == s.split
                && before.line == s.line
                && before.start == s.start
                && before.direction == s.direction
            {
                s.keep_fixed_end(&before, model);
            }
        }
        ElementKind::Railing(r) => {
            if num(ui, "Height", &mut r.height, 0.2, 3.0) {
                label = "Railing height"
            }
            if num(ui, "Spacing", &mut r.post_spacing, 0.1, 5.0) {
                label = "Railing posts"
            }
            // its top rail from the profile library (MU36)
            crate::props::row(ui, "Top rail", |ui| {
                let w = ui.available_width();
                let mut names: Vec<String> = creabim_core::profiles::project_profiles(model)
                    .into_iter()
                    .map(|p| p.name)
                    .collect();
                names.extend(["Handrail", "Tube", "Bar"].iter().map(|s| s.to_string()));
                egui::ComboBox::from_id_salt("rail.profile")
                    .icon(crate::props::combo_icon)
                    .width(w)
                    .truncate()
                    .selected_text(
                        r.rail_profile
                            .clone()
                            .unwrap_or_else(|| "Square bar".into()),
                    )
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(r.rail_profile.is_none(), "Square bar")
                            .clicked()
                        {
                            r.rail_profile = None;
                            label = "Railing top rail";
                        }
                        for n in &names {
                            if ui
                                .selectable_label(r.rail_profile.as_deref() == Some(n.as_str()), n)
                                .clicked()
                            {
                                r.rail_profile = Some(n.clone());
                                label = "Railing top rail";
                            }
                        }
                    });
            });
            // how well a hand holds its rail
            let outline = r
                .rail_profile
                .as_deref()
                .and_then(|n| creabim_core::profiles::find(model, n));
            let g = members::hand_grip(outline.as_deref());
            if !g.ok {
                crate::props::warning(
                    ui,
                    if g.round {
                        "Too thick or thin to grip: a round rail is 30 to 43 mm across (NBC 9.8.7.5)."
                    } else {
                        "Hard to grip: a shaped rail is 100 to 125 mm round and at most 45 mm across (NBC 9.8.7.5)."
                    },
                    None,
                );
            }
            let guard = !(r.on_wall && r.stair.is_some());
            if guard {
                crate::props::row(ui, "Infill", |ui| {
                    let w = ui.available_width();
                    egui::ComboBox::from_id_salt("rail.infill")
                        .icon(crate::props::combo_icon)
                        .width(w)
                        .selected_text(r.infill.label())
                        .show_ui(ui, |ui| {
                            for k in members::RailInfill::ALL {
                                if ui.selectable_label(r.infill == k, k.label()).clicked() {
                                    r.infill = k;
                                    label = "Railing infill";
                                }
                            }
                        });
                });
                let mut taller = r.guard_height.is_some();
                if crate::props::check(ui, "Guard", &mut taller, "Taller")
                    .on_hover_text(
                        "A guard taller than the handrail, as landings and well edges need",
                    )
                    .changed()
                {
                    r.guard_height = taller.then_some(1.07f64.max(r.height + 0.1));
                    label = "Guard height";
                }
                if let Some(h) = &mut r.guard_height {
                    if num(ui, "Guard height", h, 0.5, 2.0) {
                        label = "Guard height"
                    }
                }
                if crate::props::check(ui, "Newels", &mut r.newels, "Posts")
                    .on_hover_text("A heavier post at each end and where the rail breaks")
                    .changed()
                {
                    label = "Railing newels";
                }
            }
            if let Some(side) = &mut r.stair {
                if crate::props::choice(
                    ui,
                    "Side",
                    &mut side.left,
                    &[(true, "Left", ""), (false, "Right", "")],
                ) {
                    label = "Railing side";
                }
                if crate::props::choice(
                    ui,
                    "Kind",
                    &mut r.on_wall,
                    &[
                        (
                            true,
                            "Wall",
                            "A wall handrail: on brackets, returning to the wall",
                        ),
                        (false, "Guard", "A guard: posts and infill on the open side"),
                    ],
                ) {
                    label = "Railing kind";
                }
                let mut ext = r.extend.is_some();
                if crate::props::check(ui, "Run on", &mut ext, "Past the ends")
                    .on_hover_text(
                        "The rail runs on past the bottom and the top, as accessible stairs ask",
                    )
                    .changed()
                {
                    r.extend = ext.then(RailExtend::default);
                    label = "Railing extensions";
                }
                if let Some(e) = &mut r.extend {
                    if num(ui, "At bottom", &mut e.bottom, 0.0, 1.0) {
                        label = "Railing extensions"
                    }
                    if num(ui, "At top", &mut e.top, 0.0, 1.0) {
                        label = "Railing extensions"
                    }
                    if crate::props::check(ui, "Bottom", &mut e.slope_bottom, "Sloped")
                        .on_hover_text(
                            "At the bottom it runs on one going on the slope before the level part",
                        )
                        .changed()
                    {
                        label = "Railing extensions";
                    }
                }
                if let Some(st) = model.get(side.stair) {
                    let rel = crate::props::relation(
                        ui,
                        "Follows",
                        &st.name,
                        "Its stair: click to select it",
                        None,
                    );
                    if rel.select {
                        crate::props::ask_select(ui, st.id);
                    }
                }
            }
            crate::props::worked_later(
                ui,
                "railing",
                &[
                    Worked::new("Length", crate::units::len(r.length(model))),
                    Worked::new("Posts", r.posts(model).len().to_string()),
                    Worked::new(
                        "Grip",
                        format!(
                            "{} {}",
                            if g.round { "round" } else { "shaped" },
                            crate::units::len(g.across)
                        ),
                    )
                    .warn_if(!g.ok),
                ],
            );
        }
        _ => {}
    }
    label
}

/// A stair's sub-folds, last in its Element block after its rows, well,
/// sheet and railings (AMD-003): its finishes, the curves of its line,
/// and its flights and landings. Returns the undo label of a change.
pub fn stair_folds(ui: &mut egui::Ui, model: &Model, s: &mut members::Stair) -> &'static str {
    let num = |ui: &mut egui::Ui, text: &str, v: &mut f64, lo: f64, hi: f64| -> bool {
        crate::props::row(ui, text, |ui| {
            crate::props::fill(ui, crate::units::drag(v).range(lo..=hi)).changed()
        })
    };
    let mut label = "";
    if !s.is_ramp() {
        // finishes over the structure: risers measured finish to
        // finish, the structure the tread finish lower
        if crate::props::sub_fold(ui, "stair-finish", "Finishes", !s.finish.is_none()) {
            let f = &mut s.finish;
            if num(ui, "Tread", &mut f.tread, 0.0, 0.1) {
                label = "Stair tread finish"
            }
            if f.tread > 0.0
                && material_pick(
                    ui,
                    model,
                    "stair.tread_mat",
                    "Made of",
                    &mut f.tread_material,
                )
            {
                label = "Stair tread finish"
            }
            if s.build.open() {
                crate::props::readout(
                    ui,
                    "Riser",
                    "None: open risers",
                    "A stair built with open risers has no riser to finish",
                );
            } else {
                if num(ui, "Riser", &mut f.riser, 0.0, 0.05) {
                    label = "Stair riser finish"
                }
                if f.riser > 0.0
                    && material_pick(
                        ui,
                        model,
                        "stair.riser_mat",
                        "Made of",
                        &mut f.riser_material,
                    )
                {
                    label = "Stair riser finish"
                }
            }
            if num(ui, "Nosing", &mut f.nosing, 0.0, 0.05) {
                label = "Stair nosing"
            }
            if f.tread > 0.0 {
                crate::props::info(
                    ui,
                    format!(
                        "Risers are measured finish to finish: the structure sits {} lower. Landings and winders get the tread finish too.",
                        crate::units::len(f.tread)
                    ),
                    "stairs-railings",
                );
            }
        }
    }
    let rise = s.rise(model);
    let lay = s.layout(rise);
    // each straight of the line bent into an arc, or not
    if s.line.len() >= 2 && crate::props::sub_fold(ui, "stair-bends", "Curves", s.curved()) {
        for i in 0..s.line.len() - 1 {
            let mut deg = s.bend(i);
            crate::props::row(ui, &format!("Straight {}", i + 1), |ui| {
                let d = egui::DragValue::new(&mut deg)
                    .range(-members::MAX_BEND..=members::MAX_BEND)
                    .speed(1.0)
                    .suffix("°");
                if crate::props::fill(ui, d)
                    .on_hover_text(
                        "How far this straight of the line bends into an arc: the angle it turns through, left positive. 0 is straight.",
                    )
                    .changed()
                {
                    s.set_bend(i, deg);
                    label = "Stair curve";
                }
            });
        }
        crate::props::howto(
            ui,
            "Curves: right-click a straight's middle in Edit mode (Enter) to curve or straighten it.",
        );
    }
    // each flight and landing on its own, kept when the stair
    // fills its line again: a list (AMD-014, AMD-018), the picked
    // piece's rows under it
    if s.line.len() >= 2
        && lay.flights.len() >= 2
        && crate::props::sub_fold(ui, "stair-pieces", "Flights and landings", true)
    {
        let l = pieces_ui(ui, s, &lay);
        if !l.is_empty() {
            label = l;
        }
        crate::props::howto(
            ui,
            "Flights and landings: Shift+Tab in Edit mode (Enter) shows a grip on each to drag.",
        );
    }
    label
}

/// Landings or winders at a stair's turns (`all`) or at one turn, as a
/// drop-down: two segments don't fit the narrowest panel.
fn turn_combo(ui: &mut egui::Ui, name: &str, salt: &str, winders: &mut bool, all: bool) -> bool {
    let word = |w: bool| match (w, all) {
        (false, true) => "Landings",
        (false, false) => "A landing",
        (true, _) => "Winders",
    };
    let mut changed = false;
    crate::props::row(ui, name, |ui| {
        egui::ComboBox::from_id_salt(salt)
            .icon(crate::props::combo_icon)
            .width(ui.available_width())
            .selected_text(word(*winders))
            .show_ui(ui, |ui| {
                for (w, tip) in [
                    (false, "A level landing at the turn"),
                    (true, "Tapered treads round the turn, one for each 30°"),
                ] {
                    let icon = if w { "stair-winders" } else { "stair-landing" };
                    if crate::props::picture_option(ui, *winders == w, word(w), |p, r, st| {
                        crate::svgicon::paint(p, r, icon, st.color)
                    })
                    .on_hover_text(tip)
                    .clicked()
                        && *winders != w
                    {
                        *winders = w;
                        changed = true;
                    }
                }
            });
    });
    changed
}

/// One row of a stair's flights and landings list.
#[derive(Clone, Copy, PartialEq)]
enum Piece {
    /// The `i`th flight of the layout.
    Flight(usize),
    /// The turn after straight `t`: a landing or winders.
    Turn(usize),
}

/// A stair's flights and turns as a list in walking order, each with its
/// main value at the right and a picture that says which it is; the
/// picked one's rows under the list. Returns the undo label of a change.
fn pieces_ui(
    ui: &mut egui::Ui,
    s: &mut members::Stair,
    lay: &members::StairLayout,
) -> &'static str {
    let num = |ui: &mut egui::Ui, text: &str, v: &mut f64, lo: f64, hi: f64| -> bool {
        crate::props::row(ui, text, |ui| {
            crate::props::fill(ui, crate::units::drag(v).range(lo..=hi)).changed()
        })
    };
    let mut label = "";
    let ramp = s.is_ramp();
    let last = lay.flights.len().saturating_sub(1);
    let turns = s.line.len().saturating_sub(2);
    let mut pieces = vec![];
    for k in 0..s.line.len().saturating_sub(1) {
        for (i, f) in lay.flights.iter().enumerate() {
            if f.straight == k {
                pieces.push(Piece::Flight(i));
            }
        }
        if k < turns {
            pieces.push(Piece::Turn(k));
        }
    }
    let treads = |i: usize| lay.flights[i].risers.saturating_sub(1);
    let run = |t: u32| crate::units::len(t as f64 * members::RAMP_SLICE);
    let kind_of = |t: usize| s.pieces.turn_kind(t).unwrap_or(s.at_turns);
    let rows: Vec<(Option<String>, String)> = pieces
        .iter()
        .map(|p| match *p {
            Piece::Flight(i) => {
                let v = if ramp {
                    run(treads(i))
                } else {
                    format!("{} treads", treads(i))
                };
                (Some(v), format!("Flight {}", i + 1))
            }
            Piece::Turn(t) => {
                let v = if !kind_of(t).is_landing() && !ramp {
                    let n = lay.winders.iter().filter(|w| w.turn == t).count();
                    format!("{n} winders")
                } else if s.pieces.deeper(t) > 0.0 {
                    format!("+{}", crate::units::len(s.pieces.deeper(t)))
                } else {
                    "Landing".to_string()
                };
                (Some(v), format!("Turn {}", t + 1))
            }
        })
        .collect();
    // a flight: three steps; a landing: its square; winders: their fan
    let shapes: Vec<(bool, bool)> = pieces
        .iter()
        .map(|p| match *p {
            Piece::Flight(_) => (true, false),
            Piece::Turn(t) => (false, !kind_of(t).is_landing() && !ramp),
        })
        .collect();
    let pic = |pa: &egui::Painter, r: egui::Rect, k: usize, ink: egui::Color32| {
        let st = Stroke::new(1.2, ink);
        let r = r.shrink(2.0);
        match shapes.get(k) {
            Some((true, _)) => {
                let (w, h) = (r.width() / 3.0, r.height() / 3.0);
                let mut pts = vec![r.left_bottom()];
                for i in 0..3 {
                    let x = r.left() + w * i as f32;
                    let y = r.bottom() - h * (i + 1) as f32;
                    pts.push(egui::pos2(x, y));
                    pts.push(egui::pos2(x + w, y));
                }
                pa.add(egui::Shape::line(pts, st));
            }
            Some((false, true)) => {
                let c = r.left_bottom();
                for i in 0..4 {
                    let a = std::f32::consts::FRAC_PI_2 * i as f32 / 3.0;
                    pa.line_segment([c, c + egui::vec2(a.cos(), -a.sin()) * r.width()], st);
                }
            }
            _ => {
                pa.rect_stroke(r, 1.0, st, egui::StrokeKind::Inside);
            }
        }
    };
    let mut at = crate::partstool::picked(ui, "stair.pieces").min(rows.len().saturating_sub(1));
    crate::props::item_list_drawn(
        ui,
        "stair.pieces",
        &rows,
        &mut at,
        rows.len().clamp(1, 6),
        (false, false, false),
        Some(&pic),
    );
    crate::partstool::pick(ui, "stair.pieces", at);
    match pieces.get(at).copied() {
        Some(Piece::Flight(i)) if i == last => {
            let rest = if ramp {
                run(treads(i))
            } else {
                format!("{} treads", treads(i))
            };
            crate::props::readout(
                ui,
                if ramp { "Length" } else { "Treads" },
                format!("{rest}, the rest"),
                "The last flight takes what the others leave",
            );
        }
        Some(Piece::Flight(i)) => {
            let f = &lay.flights[i];
            let kept = s.pieces.treads(f.straight).is_some();
            let mut t = treads(i);
            if ramp {
                // a ramp's flight by its length, in slices underneath
                let mut len = t as f64 * members::RAMP_SLICE;
                if num(ui, "Length", &mut len, 0.1, 100.0) {
                    let n = (len / members::RAMP_SLICE).round().max(1.0) as u32;
                    s.pieces.set_treads(f.straight, Some(n));
                    label = "Ramp flight";
                }
            } else {
                let changed = crate::props::row(ui, "Treads", |ui| {
                    crate::props::fill(ui, egui::DragValue::new(&mut t).range(1..=200))
                        .on_hover_text("Its own number of treads, kept when the line changes; the flights after it take the rest")
                        .changed()
                });
                if changed {
                    s.pieces.set_treads(f.straight, Some(t));
                    label = "Stair flight";
                }
            }
            if kept
                && crate::props::action(
                    ui,
                    "",
                    "Fit to the line",
                    "Back to as many as fit before the turn",
                    true,
                )
                .clicked()
            {
                s.pieces.set_treads(f.straight, None);
                label = if ramp { "Ramp flight" } else { "Stair flight" };
            }
        }
        Some(Piece::Turn(turn)) => {
            let mut winders = !kind_of(turn).is_landing();
            if !ramp && turn_combo(ui, "Turns on", "stair.turn", &mut winders, false) {
                let new = if winders {
                    members::TurnKind::Winders(0)
                } else {
                    members::TurnKind::Landing
                };
                let same = new.is_landing() == s.at_turns.is_landing();
                s.pieces.set_turn_kind(turn, (!same).then_some(new));
                label = "Stair turn";
            }
            if winders && !ramp {
                let mut n = lay.winders.iter().filter(|w| w.turn == turn).count() as u32;
                let changed = crate::props::row(ui, "Winders", |ui| {
                    crate::props::fill(ui, egui::DragValue::new(&mut n).range(2..=12))
                        .on_hover_text(
                            "How many winders turn the stair here; one for each 30° is the usual",
                        )
                        .changed()
                });
                if changed {
                    s.pieces
                        .set_turn_kind(turn, Some(members::TurnKind::Winders(n)));
                    label = "Stair winders";
                }
            } else if lay.turns.contains(&turn) {
                let mut by = s.pieces.deeper(turn);
                if num(ui, "Deeper by", &mut by, 0.0, 10.0) {
                    s.pieces.set_deeper(turn, by);
                    label = "Stair landing";
                }
            } else {
                crate::props::readout(
                    ui,
                    "Landing",
                    "Shared",
                    "Two turns too close for a flight between them share one half-landing",
                );
            }
        }
        None => {}
    }
    label
}

/// A column's base and top constraints: the level it stands on and how
/// far above it, and the level its top is tied to (or none) and how far
/// above that. Standing it on another level keeps its top where it was.
/// A material picker over the model's materials; `None` is the stair's
/// own (or its type's).
fn material_pick(
    ui: &mut egui::Ui,
    model: &Model,
    salt: &str,
    name: &str,
    current: &mut Option<ElementId>,
) -> bool {
    let materials: Vec<(ElementId, String)> = model
        .iter()
        .filter(|e| matches!(e.kind, ElementKind::Material(_)))
        .map(|e| (e.id, e.name.clone()))
        .collect();
    let shown = current
        .and_then(|id| materials.iter().find(|m| m.0 == id))
        .map_or("Plain finish".to_string(), |m| m.1.clone());
    let mut changed = false;
    crate::props::row(ui, name, |ui| {
        let w = ui.available_width();
        egui::ComboBox::from_id_salt(salt)
            .icon(crate::props::combo_icon)
            .width(w)
            .truncate()
            .selected_text(shown)
            .show_ui(ui, |ui| {
                changed |= ui.selectable_value(current, None, "Plain finish").changed();
                for (id, n) in &materials {
                    changed |= ui.selectable_value(current, Some(*id), n).changed();
                }
            });
    });
    changed
}

fn column_levels(ui: &mut egui::Ui, model: &Model, c: &mut Column) -> &'static str {
    let mut label = "";
    let levels = creabim_core::levels::sorted(model);
    let name = |id: Option<ElementId>| {
        id.and_then(|i| levels.iter().find(|l| l.0 == i))
            .map_or("Unconnected".to_string(), |l| l.1.clone())
    };
    // its base level is the Element tab's Level row (REV04 §1), which
    // keeps the top where it was, as this picker did
    let num = |ui: &mut egui::Ui, text: &str, v: &mut f64| -> bool {
        crate::props::row(ui, text, |ui| {
            crate::props::fill(ui, crate::units::drag(v).range(-50.0..=50.0)).changed()
        })
    };
    if num(ui, "Base offset", &mut c.base_offset) {
        label = "Column base offset";
    }
    let mut top = c.top_level();
    crate::app::helpui::combo(ui, "Top level")
        .selected_text(name(top))
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut top, None, "Unconnected")
                .on_hover_text("A fixed height, or up to the next level");
            for (lid, n, _) in levels.iter().rev() {
                ui.selectable_value(&mut top, Some(*lid), n);
            }
        });
    if top != c.top_level() {
        // tied to a level: the top lands on it, wherever it was
        c.set_top_level(model, top, 0.0);
        label = "Column top level";
    }
    if let creabim_core::WallTop::UpToLevel { offset, .. } = &mut c.top {
        if num(ui, "Top offset", offset) {
            label = "Column top offset";
        }
    }
    label
}

/// A footing's Properties rows: its size, how far below its level (or its
/// host's base) it sits, where it is when it stands free, and its host.
fn footing_fields(ui: &mut egui::Ui, model: &Model, f: &mut creabim_core::Footing) -> &'static str {
    use creabim_core::FootingKind;
    let mut label = "";
    let num = |ui: &mut egui::Ui, text: &str, v: &mut f64, lo: f64, hi: f64| -> bool {
        crate::props::row(ui, text, |ui| {
            crate::props::fill(ui, crate::units::drag(v).range(lo..=hi)).changed()
        })
    };
    // under the fields, in their column, as Blender's notes (MU31)
    if num(ui, "Width", &mut f.width, 0.1, 10.0) {
        label = "Footing width";
    }
    if f.kind == FootingKind::Pad && num(ui, "Length", &mut f.length, 0.1, 10.0) {
        label = "Footing length";
    }
    if num(ui, "Thickness", &mut f.thickness, 0.05, 5.0) {
        label = "Footing thickness";
    }
    let host = f.host.and_then(|h| model.get(h));
    let below = if host.is_some() {
        match f.kind {
            FootingKind::Strip => "Top below wall base",
            FootingKind::Pad => "Top below column base",
        }
    } else {
        "Top below level"
    };
    if num(ui, below, &mut f.below, -50.0, 50.0) {
        label = "Footing depth";
    }
    match host {
        Some(h) => {
            let rel = crate::props::relation(
                ui,
                "Follows",
                &h.name,
                "It moves with this and goes when it is deleted. Click to select it",
                Some("Let go of it: the footing stays where it is, on its own"),
            );
            if rel.select {
                crate::props::ask_select(ui, h.id);
            }
            if rel.let_go {
                *f = creabim_core::footing::detached(model, f);
                label = "Let go of host";
            }
        }
        None => {
            // its level is the Element tab's Level row (REV04 §1)
            match f.kind {
                FootingKind::Strip => {
                    if num(ui, "Start X", &mut f.start.x, -1e4, 1e4)
                        | num(ui, "Start Y", &mut f.start.y, -1e4, 1e4)
                        | num(ui, "End X", &mut f.end.x, -1e4, 1e4)
                        | num(ui, "End Y", &mut f.end.y, -1e4, 1e4)
                    {
                        label = "Move footing";
                    }
                }
                FootingKind::Pad => {
                    if num(ui, "X", &mut f.start.x, -1e4, 1e4)
                        | num(ui, "Y", &mut f.start.y, -1e4, 1e4)
                    {
                        f.end = f.start;
                        label = "Move footing";
                    }
                    let r = crate::props::flow(ui, |ui| {
                        ui.label("Rotation");
                        ui.add(
                            egui::DragValue::new(&mut f.rotation)
                                .speed(0.5)
                                .range(-360.0..=360.0)
                                .suffix("°"),
                        )
                        .changed()
                    });
                    if r.inner {
                        label = "Turn footing";
                    }
                }
            }
        }
    }
    let top = f.top(model);
    let mut worked = vec![
        crate::props::Worked::new("Top", crate::units::elevation(top)),
        crate::props::Worked::new("Underside", crate::units::elevation(top - f.thickness)),
    ];
    if matches!(f.kind, FootingKind::Strip) {
        worked.push(crate::props::Worked::new(
            "Length",
            crate::units::len(f.strip_length(model)),
        ));
    }
    crate::props::worked_later(ui, "footing", &worked);
    if host.is_some() {
        crate::props::info_later(
            ui,
            "It moves with what it is under and is deleted with it. Let go of it with × to keep it on its own.",
            "",
        );
    }
    label
}

/// The tool bar's settings for the active tool: sizes as numbers, shapes
/// as toggles (`member:<what>` actions).
pub fn tool_options(state: &PlanState) -> Vec<serde_json::Value> {
    use serde_json::json;
    let m = &state.members;
    let num = |key: &str,
               label: &str,
               value: f64,
               unit: &str,
               step: f64,
               min: f64,
               max: f64,
               hint: &str| {
        json!({
            "kind": "number", "key": format!("member.{key}"), "label": label, "value": value,
            "unit": unit, "step": step, "min": min, "max": max, "hint": hint,
        })
    };
    let _check = |id: &str, label: &str, on: bool, hint: &str| {
        json!({
            "kind": "check", "id": format!("member:{id}"), "label": label, "on": on,
            "hint": hint,
        })
    };
    let toggle = |id: &str, label: &str, on: bool, icon: &str, hint: &str| {
        json!({
            "kind": "toggle", "id": format!("member:{id}"), "label": label, "on": on,
            "icon": icon, "hint": hint,
        })
    };
    let opening = |key: &str, s: [f64; 3], what: &str| {
        vec![
            num(
                &format!("{key}_width"),
                "Width",
                s[0],
                "m",
                0.01,
                0.1,
                20.0,
                &format!("Width of new {what}s. Typing a size here places it without a type."),
            ),
            num(
                &format!("{key}_height"),
                "Height",
                s[1],
                "m",
                0.01,
                0.1,
                20.0,
                &format!("Height of new {what}s"),
            ),
            num(
                &format!("{key}_sill"),
                "Sill",
                s[2],
                "m",
                0.01,
                0.0,
                20.0,
                &format!("How high new {what}s start above the wall's base"),
            ),
        ]
    };
    let mut out = match state.tool {
        Tool::Door => opening("door", m.door, "door"),
        Tool::Window => opening("window", m.window, "window"),
        Tool::Opening => opening("hole", m.hole, "opening"),
        Tool::Column => vec![
            toggle(
                "square",
                "Square",
                m.column_section.is_none() && !m.round_columns(),
                "square-dashed",
                "Rectangular columns",
            ),
            toggle(
                "round",
                "Round",
                m.column_section.is_none() && m.round_columns(),
                "circle-dot",
                "Round columns",
            ),
            // or any profile of the library: an I, a T, an L, your own (MU40)
            json!({
                "kind": "menu", "key": "column.library", "label": "Profile",
                "value": m.column_section.clone().unwrap_or_else(|| "Library".into()), "icon": "profile",
                "on": m.column_section.is_some(),
                "hint": "Columns of a profile from the library: presets and the project's own",
            }),
            num(
                "column",
                if m.round_columns() {
                    "Diameter"
                } else {
                    "Size"
                },
                m.column_size(),
                "m",
                0.01,
                0.05,
                3.0,
                "Size of new columns",
            ),
        ],
        Tool::Beam => vec![
            json!({
                "kind": "menu", "key": "beam.corner", "label": "Corners",
                "value": m.beam_corner.label(), "icon": m.beam_corner.icon(),
                "hint": "How the beams of a chain meet: Separate (concrete pieces), Mitred (welded steel) or Butt (one runs through, the next stops against it)",
            }),
            toggle(
                "beam.onface",
                "On face",
                m.beam_on_face,
                "scan",
                "In 3D, beams lie on the face clicked (a roof slope, a wall), square to it: purlins, rafters, braces",
            ),
            json!({
                "kind": "menu", "key": "beam.library", "label": "Profile",
                "value": m.beam_section.clone().unwrap_or_else(|| "Its own".into()), "icon": "profile",
                "on": m.beam_section.is_some(),
                "hint": "Beams of a profile from the library (an I, a T, an L, your own), or their own width × depth",
            }),
            num(
                "beam_width",
                "Width",
                m.beam_width,
                "m",
                0.01,
                0.05,
                2.0,
                "Width of new beams",
            ),
            num(
                "beam_depth",
                "Depth",
                m.beam_depth,
                "m",
                0.01,
                0.05,
                3.0,
                "Depth of new beams",
            ),
        ],
        Tool::Roof => {
            let mut v: Vec<serde_json::Value> = RoofShape::ALL
                .iter()
                .map(|s| {
                    toggle(
                        &format!("roof.{}", s.label()),
                        s.label(),
                        m.roof_shape == *s,
                        roof_icon(*s),
                        "Shape of new roofs",
                    )
                })
                .collect();
            if m.roof_shape != RoofShape::Flat {
                v.push(num(
                    "pitch",
                    "Pitch",
                    m.roof_pitch,
                    "°",
                    1.0,
                    1.0,
                    75.0,
                    "Slope of new roofs",
                ));
            }
            v
        }
        Tool::Stair => vec![
            num(
                "stair_width",
                "Width",
                m.stair_width,
                "m",
                0.05,
                0.5,
                5.0,
                "Width of new stairs",
            ),
            // four questions, one answer each (Mockup_Stair_Toolbar_REV00,
            // A, owner 2026-11-19): the line, the turns, steps or a
            // slope, the railings
            json!({
                "kind": "menu", "key": "stair.line", "label": "", "compact": true,
                "value": stair_line_name(m.stair_drawn), "icon": stair_line_icon(m.stair_drawn),
                "enabled": !m.stair_spiral,
                "hint": if m.stair_spiral {
                    "A spiral is drawn by its centre and its start, on its walk line"
                } else {
                    "The line you click: up the middle, or along its left or right edge"
                },
            }),
            json!({
                "kind": "menu", "key": "stair.turns", "label": "",
                "value": stair_turns_name(m), "icon": stair_turns_icon(m),
                "hint": "What the stair does at its turns: landings, winders, a spiral or a split",
            }),
            json!({"kind": "seg", "items": [
                {"id": "member:stair.steps", "label": "Steps", "icon": "stair", "on": !m.stair_ramp, "hint": "Steps"},
                {"id": "member:stair.ramp", "label": "Ramp", "icon": "ramp", "on": m.stair_ramp, "hint": "A ramp: one smooth slope at 1:12, with landings at its turns"},
            ]}),
            json!({
                "kind": "menu", "key": "stair.rails", "label": "", "compact": true,
                "value": stair_rails_name(m), "icon": stair_rails_icon(m),
                "hint": "Railings on new stairs: a wall handrail where a wall runs beside a side, else a guard",
            }),
        ],
        Tool::Footing => vec![
            toggle(
                "footing.strip",
                "Strip",
                !m.footing_pad,
                "footing",
                "Strip footings: click a wall, or two points",
            ),
            toggle(
                "footing.pad",
                "Pad",
                m.footing_pad,
                "square-dashed",
                "Pad footings: click a column, or anywhere",
            ),
            num(
                "footing_width",
                if m.footing_pad { "Size" } else { "Width" },
                m.footing_width,
                "m",
                0.05,
                0.1,
                10.0,
                "Width of new strips, or the sides of new pads",
            ),
            num(
                "footing_thickness",
                "Thickness",
                m.footing_thickness,
                "m",
                0.05,
                0.05,
                5.0,
                "Thickness of new footings",
            ),
        ],
        Tool::Railing => vec![
            json!({
                "kind": "menu", "key": "railing.library", "label": "Rail",
                "value": m.railing_section.clone().unwrap_or_else(|| "Its own".into()), "icon": "profile",
                "on": m.railing_section.is_some(),
                "hint": "The top rail's profile from the library (a handrail, a tube, your own), or its own square bar",
            }),
            num(
                "railing_height",
                "Height",
                m.railing_height,
                "m",
                0.05,
                0.3,
                3.0,
                "Height of new railings",
            ),
        ],
        _ => vec![],
    };
    if !out.is_empty() {
        out.push(json!({"kind": "sep"}));
    }
    out
}

/// A number typed in the tool bar.
pub fn set_option(state: &mut PlanState, key: &str, v: f64) {
    let m = &mut state.members;
    match key.trim_start_matches("member.") {
        "column" => m.set_column_size(v.clamp(0.05, 3.0)),
        "beam_width" => m.beam_width = v.clamp(0.05, 2.0),
        "beam_depth" => m.beam_depth = v.clamp(0.05, 3.0),
        "pitch" => m.roof_pitch = v.clamp(1.0, 75.0),
        "stair_width" => m.stair_width = v.clamp(0.5, 5.0),
        "railing_height" => m.railing_height = v.clamp(0.3, 3.0),
        "footing_width" => m.footing_width = v.clamp(0.1, 10.0),
        "footing_thickness" => m.footing_thickness = v.clamp(0.05, 5.0),
        k if k.starts_with("door_") || k.starts_with("window_") || k.starts_with("hole_") => {
            let (kind, what) = k.split_once('_').unwrap_or((k, ""));
            let s = match kind {
                "door" => &mut m.door,
                "window" => &mut m.window,
                _ => &mut m.hole,
            };
            match what {
                "width" => s[0] = v.clamp(0.1, 20.0),
                "height" => s[1] = v.clamp(0.1, 20.0),
                "sill" => s[2] = v.clamp(0.0, 20.0),
                _ => {}
            }
        }
        _ => {}
    }
}

/// A toggle clicked in the tool bar.
pub fn toggle_option(state: &mut PlanState, what: &str) {
    let m = &mut state.members;
    match what {
        "beam.onface" => m.beam_on_face = !m.beam_on_face,
        "square" => m.set_round(false),
        "round" => m.set_round(true),
        "stair.walk" | "stair.line:walk" => m.stair_drawn = StairLine::Walk,
        "stair.line:left" => m.stair_drawn = StairLine::Left,
        "stair.line:right" => m.stair_drawn = StairLine::Right,
        // what a turn becomes: one answer (REV00 A)
        "stair.turns:landing"
        | "stair.turns:winders"
        | "stair.turns:spiral"
        | "stair.turns:split" => {
            m.stair_winders = what.ends_with("winders");
            m.stair_spiral = what.ends_with("spiral");
            m.stair_split = what.ends_with("split");
            // a ramp has no winders: back to steps
            if m.stair_winders {
                m.stair_ramp = false;
            }
            // a spiral is drawn on its walk line
            if m.stair_spiral {
                m.stair_drawn = StairLine::Walk;
            }
        }
        "stair.steps" => m.stair_ramp = false,
        "stair.rails:both" | "stair.rails:left" | "stair.rails:right" | "stair.rails:none" => {
            m.stair_rail_left = what.ends_with("both") || what.ends_with("left");
            m.stair_rail_right = what.ends_with("both") || what.ends_with("right");
        }
        "stair.left" => m.stair_drawn = StairLine::Left,
        "stair.right" => m.stair_drawn = StairLine::Right,
        "stair.rail_left" => m.stair_rail_left = !m.stair_rail_left,
        // ticking one unticks what it can't go with (MU99: all four on
        // at once made a split spiral ramp on winders): a ramp has no
        // winders, a spiral turns on its own and can't be split
        "stair.winders" => {
            m.stair_winders = !m.stair_winders;
            if m.stair_winders {
                m.stair_ramp = false;
                m.stair_spiral = false;
            }
        }
        "stair.spiral" => {
            m.stair_spiral = !m.stair_spiral;
            if m.stair_spiral {
                m.stair_winders = false;
                m.stair_split = false;
            }
        }
        // the Steps / Ramp segment: a ramp has no winders
        "stair.ramp" => {
            m.stair_ramp = true;
            m.stair_winders = false;
        }
        "stair.split" => {
            m.stair_split = !m.stair_split;
            if m.stair_split {
                m.stair_spiral = false;
            }
        }
        "stair.rail_right" => m.stair_rail_right = !m.stair_rail_right,
        "footing.strip" | "footing.pad" => {
            let pad = what == "footing.pad";
            if pad != m.footing_pad {
                m.footing_pad = pad;
                // each kind starts at its usual size
                (m.footing_width, m.footing_thickness) = if pad {
                    (
                        creabim_core::footing::PAD_SIZE,
                        creabim_core::footing::PAD_THICKNESS,
                    )
                } else {
                    (
                        creabim_core::footing::STRIP_WIDTH,
                        creabim_core::footing::STRIP_THICKNESS,
                    )
                };
            }
        }
        w => {
            if let Some(s) = RoofShape::ALL
                .into_iter()
                .find(|s| w == format!("roof.{}", s.label()))
            {
                m.roof_shape = s;
            }
        }
    }
}

/// The Stair tool's line, as its menu names and draws it.
fn stair_line_name(d: StairLine) -> &'static str {
    match d {
        StairLine::Walk => "Walk line",
        StairLine::Left => "Left side",
        StairLine::Right => "Right side",
    }
}

fn stair_line_icon(d: StairLine) -> &'static str {
    match d {
        StairLine::Walk => "stair-line-walk",
        StairLine::Left => "stair-line-left",
        StairLine::Right => "stair-line-right",
    }
}

fn stair_turns_name(m: &MemberSettings) -> &'static str {
    if m.stair_spiral {
        "Spiral"
    } else if m.stair_split {
        "Split"
    } else if m.stair_winders {
        "Winders"
    } else {
        "Landings"
    }
}

fn stair_turns_icon(m: &MemberSettings) -> &'static str {
    if m.stair_spiral {
        "stair-spiral"
    } else if m.stair_split {
        "stair-split"
    } else if m.stair_winders {
        "stair-winders"
    } else {
        "stair-landing"
    }
}

fn stair_rails_name(m: &MemberSettings) -> &'static str {
    match (m.stair_rail_left, m.stair_rail_right) {
        (true, true) => "Both sides",
        (true, false) => "Left side",
        (false, true) => "Right side",
        (false, false) => "No railings",
    }
}

fn stair_rails_icon(m: &MemberSettings) -> &'static str {
    match (m.stair_rail_left, m.stair_rail_right) {
        (true, true) => "railing-both",
        (true, false) => "railing-left",
        (false, true) => "railing-right",
        (false, false) => "railing-none",
    }
}

/// The Stair tool's menus, by key: a picture and a line for each answer
/// (Mockup_Stair_Toolbar_REV00, A).
pub fn stair_menu(state: &PlanState, key: &str) -> Option<Vec<crate::ui_model::Item>> {
    use crate::ui_model::Item;
    let m = &state.members;
    Some(match key {
        "stair.line" => vec![
            Item::Header("The line you click".into()),
            Item::new("member:stair.line:walk", "Walk line")
                .icon("stair-line-walk")
                .hint("Up the middle, where people walk")
                .checked(m.stair_drawn == StairLine::Walk),
            Item::new("member:stair.line:left", "Left side")
                .icon("stair-line-left")
                .hint("Its left edge, along a wall")
                .checked(m.stair_drawn == StairLine::Left),
            Item::new("member:stair.line:right", "Right side")
                .icon("stair-line-right")
                .hint("Its right edge, along a wall")
                .checked(m.stair_drawn == StairLine::Right),
        ],
        "stair.turns" => {
            let now = stair_turns_name(m);
            let head = if m.stair_ramp {
                "At its turns · a ramp"
            } else {
                "At its turns"
            };
            let mut v = vec![
                Item::Header(head.into()),
                Item::new("member:stair.turns:landing", "Landings")
                    .icon("stair-landing")
                    .hint("A level landing at each turn")
                    .checked(now == "Landings"),
            ];
            // a ramp has no winders (REV00 §4)
            if !m.stair_ramp {
                v.push(
                    Item::new("member:stair.turns:winders", "Winders")
                        .icon("stair-winders")
                        .hint("Tapered treads round each turn, one for each 30°")
                        .checked(now == "Winders"),
                );
            }
            v.push(
                Item::new("member:stair.turns:spiral", "Spiral")
                    .icon("stair-spiral")
                    .hint("Click its centre, then where it starts; it climbs to the floor above")
                    .checked(now == "Spiral"),
            );
            v.push(
                Item::new("member:stair.turns:split", "Split")
                    .icon("stair-split")
                    .hint("Draw one arm: up the middle, a landing, back down one side; its mirror comes too")
                    .checked(now == "Split"),
            );
            v
        }
        "stair.rails" => {
            let now = stair_rails_name(m);
            vec![
                Item::Header("Railings".into()),
                Item::new("member:stair.rails:both", "Both sides")
                    .icon("railing-both")
                    .hint("A wall handrail by a wall, else a guard")
                    .checked(now == "Both sides"),
                Item::new("member:stair.rails:left", "Left side")
                    .icon("railing-left")
                    .checked(now == "Left side"),
                Item::new("member:stair.rails:right", "Right side")
                    .icon("railing-right")
                    .checked(now == "Right side"),
                Item::new("member:stair.rails:none", "None")
                    .icon("railing-none")
                    .hint("Add them later in Properties")
                    .checked(now == "No railings"),
            ]
        }
        _ => return None,
    })
}

/// The interface icon of a roof shape: each its own.
pub(crate) fn roof_icon(s: RoofShape) -> &'static str {
    match s {
        RoofShape::Flat => "roof-flat",
        RoofShape::Shed => "roof-mono",
        RoofShape::Gable => "roof-gable",
        RoofShape::Hip => "roof-hip",
    }
}

/// A roof shape's outline, drawn small in `r` for its drop-down: the
/// same pictures as the icons.
fn roof_glyph(p: &egui::Painter, r: egui::Rect, st: egui::Stroke, s: RoofShape) {
    let at = |x: f32, y: f32| egui::pos2(r.left() + x * r.width(), r.top() + y * r.height());
    let line = |pts: &[(f32, f32)]| {
        let pts: Vec<egui::Pos2> = pts.iter().map(|&(x, y)| at(x, y)).collect();
        p.add(egui::Shape::line(pts, st));
    };
    match s {
        RoofShape::Flat => {
            line(&[(0.1, 0.4), (0.9, 0.4), (0.9, 0.55), (0.1, 0.55), (0.1, 0.4)]);
            line(&[(0.2, 0.55), (0.2, 0.85)]);
            line(&[(0.8, 0.55), (0.8, 0.85)]);
        }
        RoofShape::Shed => {
            line(&[(0.1, 0.85), (0.1, 0.6), (0.9, 0.3), (0.9, 0.85)]);
        }
        RoofShape::Gable => {
            line(&[(0.1, 0.55), (0.5, 0.2), (0.9, 0.55)]);
            line(&[(0.2, 0.47), (0.2, 0.85), (0.8, 0.85), (0.8, 0.47)]);
        }
        RoofShape::Hip => {
            line(&[
                (0.1, 0.65),
                (0.3, 0.3),
                (0.7, 0.3),
                (0.9, 0.65),
                (0.1, 0.65),
            ]);
            line(&[(0.2, 0.65), (0.2, 0.85), (0.8, 0.85), (0.8, 0.65)]);
        }
    }
}

/// The stairs failing a check of their rule set, and a message listing
/// them.
pub fn check_stairs(model: &Model) -> (Vec<ElementId>, String) {
    let bad = creabim_core::stairrules::failing_stairs(model);
    if bad.is_empty() {
        return (
            vec![],
            "Every stair passes the checks of its rule set.".into(),
        );
    }
    let lines: Vec<String> = bad
        .iter()
        .map(|(id, checks)| {
            let name = model.get(*id).map_or("Stair", |e| e.name.as_str());
            let what: Vec<&str> = checks.iter().map(|c| c.what).collect();
            format!("{name}: {}", what.join(", ").to_lowercase())
        })
        .collect();
    (
        bad.iter().map(|(id, _)| *id).collect(),
        format!(
            "{} (selected): {}. Their Properties list each check.",
            match bad.len() {
                1 => "1 stair fails a check".to_string(),
                n => format!("{n} stairs fail a check"),
            },
            lines.join("; ")
        ),
    )
}

/// A stair's rule set and its live check list, in its Properties. Returns
/// the undo label of a change to the stair, and the project's own rule
/// sets when they were edited (to save first).
pub fn stair_rules_ui(
    ui: &mut egui::Ui,
    model: &Model,
    id: ElementId,
    s: &mut members::Stair,
) -> (
    &'static str,
    Option<Vec<creabim_core::stairrules::StairRules>>,
) {
    use crate::partstool::num;
    use creabim_core::stairrules::{self as sr, StairCode, Verdict};
    let deg = |ui: &mut egui::Ui, text: &str, v: &mut f64, lo: f64, hi: f64| -> bool {
        crate::props::row(ui, text, |ui| {
            crate::props::fill(
                ui,
                egui::DragValue::new(v)
                    .speed(0.5)
                    .range(lo..=hi)
                    .suffix("°"),
            )
            .changed()
        })
    };
    let mut label = "";
    let mut saved = None;
    let own = sr::own_rules(model);
    let checks = sr::check_with(model, id, s, &s.rules.rules(model));
    let fails = checks
        .iter()
        .filter(|c| c.verdict == Verdict::Fails)
        .count();
    if !crate::props::sub_fold(ui, "stair-checks", "Rules and checks", fails > 0) {
        return (label, saved);
    }
    crate::props::Combo::new("Rule set")
        .selected_text(s.rules.label())
        .show_ui(ui, |ui| {
            let mut codes: Vec<StairCode> = StairCode::PRESETS.to_vec();
            codes.extend(own.iter().map(|r| StairCode::Own(r.name.clone())));
            for c in codes {
                if ui.selectable_label(s.rules == c, c.label()).clicked() {
                    s.rules = c;
                    label = "Stair rule set";
                }
            }
        });
    // the checks as a list (AMD-014): a dot by its verdict, what, and the
    // stair's value at the right with its limit
    let rows: Vec<(Option<String>, String)> = checks
        .iter()
        .map(|c| {
            let word = match c.verdict {
                Verdict::Passes => "Passes",
                Verdict::Fails => "Fails",
                Verdict::NotChecked => "Not checked",
            };
            (Some(word.to_string()), c.what.to_string())
        })
        .collect();
    let verdicts: Vec<Verdict> = checks.iter().map(|c| c.verdict).collect();
    let dot = |pa: &egui::Painter, r: egui::Rect, k: usize, ink: egui::Color32| {
        let colour = match verdicts.get(k) {
            Some(Verdict::Passes) => egui::Color32::from_rgb(46, 160, 67),
            Some(Verdict::Fails) => egui::Color32::from_rgb(207, 34, 46),
            _ => ink.gamma_multiply(0.5),
        };
        pa.circle_filled(r.center(), 4.0, colour);
    };
    let mut at = crate::partstool::picked(ui, "stair.checks").min(rows.len().saturating_sub(1));
    crate::props::item_list_drawn(
        ui,
        "stair.checks",
        &rows,
        &mut at,
        rows.len().clamp(1, 8),
        (false, false, false),
        Some(&dot),
    );
    crate::partstool::pick(ui, "stair.checks", at);
    // the picked check: what the stair has and what the rule asks
    if let Some(c) = checks.get(at) {
        crate::props::readout(ui, "This stair", c.value.clone(), c.what);
        if !c.limit.is_empty() {
            crate::props::readout(ui, "Rule", c.limit.clone(), "What the rule set asks");
        }
    }
    match fails {
        0 => crate::props::info(
            ui,
            "Passes every check here. Checks advise; they don't certify: check the values against the edition you work to.",
            "stairs-railings",
        ),
        n => {
            crate::props::warning(
                ui,
                if n == 1 {
                    "1 check fails. Checks advise; they don't certify.".to_string()
                } else {
                    format!("{n} checks fail. Checks advise; they don't certify.")
                },
                None,
            );
        }
    }
    // the rule set's values: editing a preset makes a project rule set
    if crate::props::sub_fold(ui, "stair-rule-values", "Rule set values", false) {
        let mut r = s.rules.rules(model);
        let before = r.clone();
        let opt = |ui: &mut egui::Ui, text: &str, v: &mut Option<f64>, hi: f64| {
            if let Some(x) = v {
                num(ui, text, x, 0.0, hi)
            } else {
                false
            }
        };
        let mut changed = opt(ui, "Riser at least", &mut r.riser_min, 0.5)
            | opt(ui, "Riser at most", &mut r.riser_max, 0.5)
            | opt(ui, "Going at least", &mut r.going_min, 1.0)
            | opt(ui, "Going at most", &mut r.going_max, 1.0)
            | opt(ui, "2R + G at least", &mut r.step_min, 2.0)
            | opt(ui, "2R + G at most", &mut r.step_max, 2.0)
            | opt(ui, "Width at least", &mut r.width_min, 5.0)
            | num(ui, "Headroom", &mut r.headroom, 1.0, 5.0)
            | opt(ui, "Nosing at most", &mut r.nosing_max, 0.1)
            | opt(ui, "Winder going", &mut r.winder_going_min, 1.0)
            | opt(ui, "Narrow end", &mut r.narrow_min, 0.5)
            | crate::props::num_tip(
                ui,
                "Rail at least",
                "Handrail at least: the lowest a handrail may be above the pitch line",
                &mut r.handrail_min,
                0.3,
                2.0,
            )
            | crate::props::num_tip(
                ui,
                "Rail at most",
                "Handrail at most: the highest a handrail may be above the pitch line",
                &mut r.handrail_max,
                0.3,
                2.0,
            )
            | num(ui, "Guard at least", &mut r.guard_min, 0.3, 2.0)
            | num(ui, "Sphere", &mut r.sphere, 0.01, 0.5);
        if let Some(p) = &mut r.pitch_max {
            changed |= deg(ui, "Pitch at most", p, 10.0, 60.0);
        }
        crate::props::info(
            ui,
            "Limits this rule set leaves out aren't checked. Changing a preset's value saves a copy as the project's own.",
            "stairs-railings",
        );
        if changed && r != before {
            let mut all = own.clone();
            match &s.rules {
                StairCode::Own(n) => {
                    if let Some(x) = all.iter_mut().find(|x| &x.name == n) {
                        *x = r;
                    } else {
                        all.push(r);
                    }
                }
                _ => {
                    // a copy, named after its preset
                    let base = format!("{}, edited", before.name);
                    let mut name = base.clone();
                    let mut k = 2;
                    while all.iter().any(|x| x.name == name) {
                        name = format!("{base} {k}");
                        k += 1;
                    }
                    r.name = name.clone();
                    all.push(r);
                    s.rules = StairCode::Own(name);
                    label = "Stair rule set";
                }
            }
            saved = Some(all);
        }
    }
    (label, saved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use creabim_core::sample::sample_house;

    #[test]
    fn sizes_set_by_hand_stay_when_the_units_change() {
        let mut m = MemberSettings {
            door: [1.0, 2.2, 0.0],
            ..Default::default()
        };
        m.suit_units(true);
        assert_eq!(
            m.door,
            [1.0, 2.2, 0.0],
            "a door sized by hand keeps its size"
        );
        assert_eq!(m.window, IMPERIAL_WINDOW, "the untouched window follows");
        m.suit_units(false);
        assert_eq!(m.window, METRIC_WINDOW);
    }

    #[test]
    fn a_wall_is_hit_across_its_whole_thickness_whatever_its_location_line() {
        let (doc, level, _, _) = setup();
        let m = doc.model();
        // the starter house's walls are drawn on several location lines
        let walls: Vec<_> = m
            .iter()
            .filter_map(|e| match &e.kind {
                ElementKind::Wall(w) if w.level == level && w.curtain.is_none() => {
                    Some((e.id, w.clone()))
                }
                _ => None,
            })
            .collect();
        assert!(!walls.is_empty());
        for (id, w) in walls {
            let n = w.direction().perp();
            let mid = (w.start + w.end) / 2.0;
            for s in [-0.45, 0.0, 0.45] {
                let p = mid + n * s * w.thickness;
                assert!(
                    walls_at(m, level, p).contains(&id),
                    "{s} of its thickness off the centre"
                );
            }
            assert!(!walls_at(m, level, mid + n * (w.thickness / 2.0 + 0.2)).contains(&id));
        }
    }

    #[test]
    fn a_host_tool_finds_its_host_before_the_first_click_only() {
        let (doc, level, mut state, _) = setup();
        let m = doc.model();
        let (id, w) = m
            .iter()
            .find_map(|e| match &e.kind {
                ElementKind::Wall(w) if w.level == level && w.curtain.is_none() => {
                    Some((e.id, w.clone()))
                }
                _ => None,
            })
            .unwrap();
        let p = (w.start + w.end) / 2.0;
        for tool in [Tool::Footing, Tool::Door, Tool::Window] {
            assert_eq!(
                hosts_at(m, level, tool, &state, p).first(),
                Some(&id),
                "{tool:?}"
            );
        }
        assert!(host_preview(m, &state, Tool::Footing, id).contains("under"));
        // a free strip with its first point snaps as usual
        state.pending = Some(DVec2::ZERO);
        assert!(hosts_at(m, level, Tool::Footing, &state, p).is_empty());
        assert!(!hosted(Tool::Wall));
    }

    #[test]
    fn stair_options_that_cannot_go_together_untick_each_other() {
        let mut st = PlanState::default();
        // the turns are one answer
        for k in [
            "stair.turns:winders",
            "stair.ramp",
            "stair.turns:split",
            "stair.turns:spiral",
        ] {
            toggle_option(&mut st, k);
        }
        let m = &st.members;
        // a ramp spiral: the ramp put winders back to landings
        assert!(m.stair_ramp && m.stair_spiral);
        assert!(!m.stair_winders && !m.stair_split);
        toggle_option(&mut st, "stair.turns:winders");
        let m = &st.members;
        assert!(m.stair_winders && !m.stair_ramp && !m.stair_spiral);
        // a ramp's turns offer no winders
        toggle_option(&mut st, "stair.ramp");
        let ids: Vec<String> = stair_menu(&st, "stair.turns")
            .unwrap()
            .iter()
            .filter_map(|i| match i {
                crate::ui_model::Item::Action { id, .. } => Some(id.clone()),
                _ => None,
            })
            .collect();
        assert!(!ids.iter().any(|i| i.ends_with("winders")), "{ids:?}");
        // railings: one answer for both sides
        toggle_option(&mut st, "stair.rails:left");
        assert!(st.members.stair_rail_left && !st.members.stair_rail_right);
    }

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

    fn count(doc: &Document, f: fn(&ElementKind) -> bool) -> usize {
        doc.model().iter().filter(|e| f(&e.kind)).count()
    }

    fn active<'a>(doc: &'a Document, sel: &Selection) -> Option<&'a ElementKind> {
        sel.active
            .and_then(|id| doc.model().get(id))
            .map(|e| &e.kind)
    }

    #[test]
    fn tools_place_each_member_and_undo() {
        let (mut doc, level, mut st, mut sel) = setup();
        let mut go = |doc: &mut Document, st: &mut PlanState, tool, x, y| {
            click(doc, level, tool, DVec2::new(x, y), st, &mut sel).unwrap();
        };
        go(&mut doc, &mut st, Tool::Column, 1.0, 1.0);
        assert_eq!(count(&doc, |k| matches!(k, ElementKind::Column(_))), 1);
        for (x, y) in [(0.0, 0.0), (4.0, 0.0), (4.0, 3.0)] {
            go(&mut doc, &mut st, Tool::Beam, x, y);
        }
        assert_eq!(count(&doc, |k| matches!(k, ElementKind::Beam(_))), 2);
        st.cancel();
        // a line with a corner: a landing there; the last point again finishes
        // (no railings: the Railing tool adds them below)
        (st.members.stair_rail_left, st.members.stair_rail_right) = (false, false);
        for (x, y) in [(20.0, 0.0), (22.0, 0.0), (22.0, 1.0), (22.0, 1.0)] {
            go(&mut doc, &mut st, Tool::Stair, x, y);
        }
        assert!(st.members.stair_line.is_empty());
        assert_eq!(
            count(&doc, |k| matches!(k, ElementKind::Stair(s)
                if s.line.len() == 3 && s.layout(3.0).landings.len() == 1)),
            1
        );
        // a click on the stair rails both its sides, once
        go(&mut doc, &mut st, Tool::Railing, 20.5, 0.0);
        assert_eq!(count(&doc, |k| matches!(k, ElementKind::Railing(_))), 2);
        go(&mut doc, &mut st, Tool::Railing, 20.5, 0.0);
        assert_eq!(count(&doc, |k| matches!(k, ElementKind::Railing(_))), 2);
        // a free path: click the last point again to finish
        for (x, y) in [(30.0, 0.0), (33.0, 0.0), (33.0, 0.0)] {
            go(&mut doc, &mut st, Tool::Railing, x, y);
        }
        assert_eq!(count(&doc, |k| matches!(k, ElementKind::Railing(_))), 3);
        assert!(st.members.path.is_empty());
        doc.undo();
        assert_eq!(count(&doc, |k| matches!(k, ElementKind::Railing(_))), 2);
    }

    #[test]
    fn a_stair_line_reaching_the_floor_above_finishes_with_its_railings() {
        let (mut doc, level, mut st, mut sel) = setup();
        let mut go = |doc: &mut Document, st: &mut PlanState, x: f64, y: f64| {
            click(doc, level, Tool::Stair, DVec2::new(x, y), st, &mut sel).unwrap();
        };
        let stairs = |doc: &Document| count(doc, |k| matches!(k, ElementKind::Stair(_)));
        go(&mut doc, &mut st, 40.0, 0.0);
        go(&mut doc, &mut st, 41.0, 0.0);
        // too short: still drawing, with the treads still to place
        assert_eq!(stairs(&doc), 0);
        assert!(st.status.as_deref().unwrap_or("").contains("to go"));
        // long enough: it finishes itself, trimmed to its last riser
        go(&mut doc, &mut st, 41.0, 12.0);
        assert_eq!(stairs(&doc), 1);
        assert!(st.members.stair_line.is_empty());
        let s = doc
            .model()
            .iter()
            .find_map(|e| match &e.kind {
                ElementKind::Stair(s) => Some(s.clone()),
                _ => None,
            })
            .unwrap();
        let lay = s.layout(s.rise(doc.model()));
        assert_eq!(lay.short, 0);
        assert!(lay.spare < 1e-6, "spare {}", lay.spare);
        assert_eq!(lay.landings.len(), 1);
        // railings came with it, each following its side of the stair
        let rails: Vec<&Railing> = doc
            .model()
            .iter()
            .filter_map(|e| match &e.kind {
                ElementKind::Railing(r) => Some(r),
                _ => None,
            })
            .collect();
        assert_eq!(rails.len(), 2);
        assert!(rails
            .iter()
            .all(|r| r.stair.is_some() && r.extend.is_some()));
        // each side's railing comes off and back by itself
        let id = doc
            .model()
            .iter()
            .find(|e| matches!(e.kind, ElementKind::Stair(_)))
            .unwrap()
            .id;
        let rails = |doc: &Document| count(doc, |k| matches!(k, ElementKind::Railing(_)));
        set_stair_railing(&mut doc, id, true, false).unwrap();
        assert_eq!(rails(&doc), 1);
        assert!(stair_side_railing(doc.model(), id, true).is_none());
        assert!(stair_side_railing(doc.model(), id, false).is_some());
        set_stair_railing(&mut doc, id, true, true).unwrap();
        assert_eq!(rails(&doc), 2);
        doc.undo();
        doc.undo();
        assert_eq!(rails(&doc), 2);
        // one undo takes the stair and its railings
        doc.undo();
        assert_eq!(stairs(&doc), 0);
        assert_eq!(count(&doc, |k| matches!(k, ElementKind::Railing(_))), 0);
        // the left railing box off: a railing on the right only
        st.members.stair_rail_left = false;
        go(&mut doc, &mut st, 60.0, 0.0);
        go(&mut doc, &mut st, 72.0, 0.0);
        assert_eq!(stairs(&doc), 1);
        let rails: Vec<bool> = doc
            .model()
            .iter()
            .filter_map(|e| match &e.kind {
                ElementKind::Railing(r) => r.stair.map(|sd| sd.left),
                _ => None,
            })
            .collect();
        assert_eq!(rails, vec![false]);
        // with Winders ticked, an L turns on winders
        st.members.stair_winders = true;
        toggle_option(&mut st, "stair.winders");
        assert!(!st.members.stair_winders);
        toggle_option(&mut st, "stair.winders");
        let s = new_stair(
            doc.model(),
            level,
            &[
                DVec2::new(80.0, 0.0),
                DVec2::new(83.3, 0.0),
                DVec2::new(83.3, 3.0),
            ],
            &st.members,
        )
        .unwrap();
        let lay = s.layout(s.rise(doc.model()));
        assert_eq!(lay.winders.len(), 3);
        assert!(lay.landings.is_empty());
        // with Spiral ticked: a click at its centre, one where it starts,
        // and it climbs round to the floor above
        st.members.stair_winders = false;
        toggle_option(&mut st, "stair.spiral");
        assert!(st.members.stair_spiral);
        go(&mut doc, &mut st, 100.0, 0.0);
        // too close for its width: taken back
        go(&mut doc, &mut st, 100.3, 0.0);
        assert_eq!(st.members.stair_line.len(), 1);
        go(&mut doc, &mut st, 101.2, 0.0);
        assert_eq!(stairs(&doc), 2);
        let s = doc
            .model()
            .iter()
            .filter_map(|e| match &e.kind {
                ElementKind::Stair(s) if s.curved() => Some(s.clone()),
                _ => None,
            })
            .next()
            .unwrap();
        let rise = s.rise(doc.model());
        let lay = s.layout(rise);
        assert_eq!(lay.flights.len(), 1);
        assert_eq!(lay.short, 0);
        assert!(lay.spare < 0.02);
        assert_eq!(s.ifc_type(rise), "SPIRAL_STAIR");
        // every nosing on the walk line 1.2 m from the centre
        let c = DVec2::new(100.0, 0.0);
        assert!(lay
            .walk
            .iter()
            .all(|p| (p.truncate().distance(c) - 1.2).abs() < 0.01));
    }

    #[test]
    fn the_footing_tool_puts_strips_under_walls_and_pads_under_columns() {
        use creabim_core::{footing, FootingKind};
        let (mut doc, level, mut st, mut sel) = setup();
        let mut go = |doc: &mut Document, st: &mut PlanState, x: f64, y: f64| {
            click(doc, level, Tool::Footing, DVec2::new(x, y), st, &mut sel).unwrap();
        };
        let (wall, mid) = doc
            .model()
            .iter()
            .find_map(|e| match &e.kind {
                ElementKind::Wall(w) if w.level == level && w.curtain.is_none() => {
                    Some((e.id, (w.start + w.end) / 2.0))
                }
                _ => None,
            })
            .unwrap();
        // a click on a wall: a strip under it, hosted
        go(&mut doc, &mut st, mid.x, mid.y);
        assert_eq!(footing::footings_under(doc.model(), wall).len(), 1);
        // a second click there adds none
        go(&mut doc, &mut st, mid.x, mid.y);
        assert_eq!(footing::footings_under(doc.model(), wall).len(), 1);
        assert!(st.status.as_deref().unwrap_or("").contains("already"));
        // two clicks away from walls: a strip on its own
        go(&mut doc, &mut st, 50.0, 50.0);
        go(&mut doc, &mut st, 54.0, 50.0);
        let free = |doc: &Document, kind: FootingKind| {
            doc.model()
                .iter()
                .filter(|e| matches!(&e.kind, ElementKind::Footing(f) if f.kind == kind && f.host.is_none()))
                .count()
        };
        assert_eq!(free(&doc, FootingKind::Strip), 1);
        // pads: under a column, and on their own, at the pad size
        click(
            &mut doc,
            level,
            Tool::Column,
            DVec2::new(70.0, 70.0),
            &mut st,
            &mut sel,
        )
        .unwrap();
        let column = sel.active.unwrap();
        toggle_option(&mut st, "footing.pad");
        assert!(st.members.footing_pad);
        let mut go = |doc: &mut Document, st: &mut PlanState, x: f64, y: f64| {
            click(doc, level, Tool::Footing, DVec2::new(x, y), st, &mut sel).unwrap();
        };
        go(&mut doc, &mut st, 70.0, 70.0);
        let under = footing::footings_under(doc.model(), column);
        assert_eq!(under.len(), 1);
        assert!(matches!(
            doc.model().get(under[0]).map(|e| &e.kind),
            Some(ElementKind::Footing(f)) if (f.width - footing::PAD_SIZE).abs() < 1e-9
        ));
        go(&mut doc, &mut st, 80.0, 80.0);
        assert_eq!(free(&doc, FootingKind::Pad), 1);
        doc.undo();
        assert_eq!(free(&doc, FootingKind::Pad), 0);
    }

    #[test]
    fn the_roof_tool_draws_rectangles_and_outlines() {
        let (mut doc, level, mut st, mut sel) = setup();
        st.tool = Tool::Roof;
        st.members.roof_shape = RoofShape::Gable;
        st.slab_mode = crate::slabtool::SlabMode::Rectangle;
        crate::slabtool::click(&mut doc, level, DVec2::ZERO, &mut st, &mut sel).unwrap();
        crate::slabtool::click(&mut doc, level, DVec2::new(8.0, 5.0), &mut st, &mut sel).unwrap();
        assert!(matches!(
            active(&doc, &sel),
            Some(ElementKind::Roof(r)) if r.shape == RoofShape::Gable
        ));
        st.slab_mode = crate::slabtool::SlabMode::Polygon;
        crate::slabtool::click(&mut doc, level, DVec2::new(20.0, 0.0), &mut st, &mut sel).unwrap();
        let mut o = st.outline.take().unwrap();
        assert!(o.roof.is_some());
        for (x, y) in [(24.0, 0.0), (24.0, 4.0), (20.0, 4.0)] {
            o.click(DVec2::new(x, y), false, false);
        }
        o.click(DVec2::new(20.0, 0.0), true, false);
        let (_, id) = o.finish(&mut doc, true).unwrap();
        assert!(matches!(
            doc.model().get(id.unwrap()).map(|e| &e.kind),
            Some(ElementKind::Roof(_))
        ));
    }
}
