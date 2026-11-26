//! How walls and their doors and windows behave, in the app: a wall's
//! location line, base and top constraints, end joins and attached top in
//! Properties, and the flip, join and attach edits picked from a button,
//! a key or a right-click menu. Every change is one transaction, so it
//! undoes like any edit.

use super::*;
use creabim_core::{walls, LocationLine, Opening, OpeningKind, Wall, WallTop};

/// A length field in a row, as the rest of Properties draws them.
fn num(ui: &mut egui::Ui, text: &str, v: &mut f64, lo: f64, hi: f64) -> bool {
    crate::props::row(ui, text, |ui| {
        crate::props::fill(ui, crate::units::drag(v).range(lo..=hi)).changed()
    })
}

/// The model's levels, lowest first.
fn levels(model: &Model) -> Vec<(ElementId, String, f64)> {
    let mut v: Vec<_> = model
        .levels()
        .map(|(e, l)| (e.id, e.name.clone(), l.elevation))
        .collect();
    v.sort_by(|a, b| a.2.total_cmp(&b.2));
    v
}

fn name_of(model: &Model, id: ElementId) -> String {
    model
        .get(id)
        .map_or("(missing)".to_string(), |e| e.name.clone())
}

/// Which side a wall's exterior is on, looking from its start to its end.
pub(super) fn exterior_side(w: &Wall) -> &'static str {
    if w.flip {
        "Left, start to end"
    } else {
        "Right, start to end"
    }
}

/// The picture of a location line: a wall in section, exterior at the
/// top, the line through it (MU42).
pub(super) fn location_icon(l: LocationLine) -> &'static str {
    match l {
        LocationLine::Centre => "loc-centre",
        LocationLine::CoreCentre => "loc-core-centre",
        LocationLine::ExteriorFace => "loc-exterior",
        LocationLine::InteriorFace => "loc-interior",
        LocationLine::CoreExterior => "loc-core-exterior",
        LocationLine::CoreInterior => "loc-core-interior",
        LocationLine::Boundary(_) => "loc-core-centre",
    }
}

/// A wall's location line, flip, constraints, joins and attached top.
/// Returns the edit's name when something changed.
pub(super) fn wall_fields(
    ui: &mut egui::Ui,
    model: &Model,
    id: ElementId,
    w: &mut Wall,
) -> &'static str {
    let mut label = "";
    crate::props::flow(ui, |ui| {
        crate::app::helpui::combo(ui, "Location line")
            .selected_text(walls::location_label(model, w))
            .width(ui.available_width().min(130.0))
            .show_ui(ui, |ui| {
                // each with its picture: where the line lies in the wall,
                // as Archicad shows them (MU42)
                for l in LocationLine::ALL {
                    let r = ui
                        .horizontal(|ui| {
                            let (rect, _) =
                                ui.allocate_exact_size(egui::vec2(26.0, 18.0), egui::Sense::hover());
                            crate::svgicon::paint(
                                ui.painter(),
                                rect,
                                location_icon(l),
                                ui.visuals().text_color(),
                            );
                            ui.selectable_value(&mut w.location, l, l.label())
                        })
                        .inner;
                    if r.changed() {
                        label = "Wall location line"
                    }
                }
                let lines = walls::layer_boundaries(model, w);
                if !lines.is_empty() {
                    ui.separator();
                    ui.weak("By a layer boundary");
                    for (l, name) in lines {
                        if ui
                            .selectable_value(&mut w.location, l, name)
                            .on_hover_text("Draw and dimension the wall on this line between layers. The wall stays where it is.")
                            .changed()
                        {
                            label = "Wall location line"
                        }
                    }
                }
            })
            .response
            .on_hover_text(
                "The line the wall is drawn and dimensioned on. It stays put when the wall gets thicker, changes its layers or is flipped.",
            );
    });

    ui.add_space(4.0);
    // its level is the Element tab's Level row, at the top (REV04 §1)
    let lv = levels(model);
    if num(ui, "Base offset", &mut w.base_offset, -10.0, 50.0) {
        label = "Wall base offset"
    }
    let shown = match w.top {
        WallTop::Unconnected => "Unconnected".to_string(),
        WallTop::UpToLevel { level, .. } => format!("Up to {}", name_of(model, level)),
    };
    crate::props::flow(ui, |ui| {
        crate::app::helpui::combo(ui, "Top")
            .selected_text(shown)
            .width(ui.available_width().min(130.0))
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(w.top.is_unconnected(), "Unconnected")
                    .clicked()
                    && !w.top.is_unconnected()
                {
                    w.top = WallTop::Unconnected;
                    label = "Wall top constraint"
                }
                for (l, n, _) in &lv {
                    let on = matches!(w.top, WallTop::UpToLevel { level, .. } if level == *l);
                    if ui.selectable_label(on, format!("Up to {n}")).clicked() && !on {
                        w.top = WallTop::UpToLevel {
                            level: *l,
                            offset: 0.0,
                        };
                        label = "Wall top constraint"
                    }
                }
            })
            .response
            .on_hover_text(
                "Unconnected: the wall is as tall as its height. Up to a level: its top follows that level when it moves.",
            );
    });
    match &mut w.top {
        WallTop::UpToLevel { offset, .. } => {
            if num(ui, "Top offset", offset, -50.0, 50.0) {
                label = "Wall top offset"
            }
            let h = walls::constrained_height(model, w).unwrap_or(w.height);
            crate::props::readout(
                ui,
                "Height",
                crate::units::len(h.max(walls::MIN_HEIGHT)),
                "Worked out from its top constraint: change the level or offset to change it",
            );
        }
        WallTop::Unconnected => {
            if num(ui, "Height", &mut w.height, 0.1, 50.0) {
                label = "Wall height"
            }
        }
    }

    ui.add_space(4.0);
    crate::props::row(ui, "Joins at", |ui| {
        // a fixed row in whole pixels: wrapping, it flickered as the panel was resized (MU59)
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 1.0;
            let h = ui.spacing().interact_size.y;
            let bw = ((ui.available_width() - 1.0) / 2.0).floor().max(40.0);
            for (i, end) in ["Start", "End"].into_iter().enumerate() {
                let on = !w.disallow_join[i];
                if ui
                    .add_sized([bw, h], egui::Button::selectable(on, end))
                    .on_hover_text("Off: this end stays square and free, and is not pulled along when the wall it touches moves. Right-click the end in plan to switch it too.")
                    .clicked()
                {
                    w.disallow_join[i] = on;
                    label = if on { "Disallow wall join" } else { "Allow wall join" }
                }
            }
        })
    });

    if w.attach_top.is_empty() {
        let found = walls::attach_candidates(model, id);
        let tip = if found.is_empty() {
            "There is no roof or slab above this wall."
        } else {
            "The top follows the underside of the roof or slab above, cut to its slope."
        };
        if crate::props::action(ui, "Top", "Attach to roof / slab", tip, !found.is_empty())
            .clicked()
        {
            w.attach_top = found;
            label = "Attach wall top"
        }
    } else {
        let names: Vec<String> = w.attach_top.iter().map(|a| name_of(model, *a)).collect();
        // a relation as a row (AMD-012): the chip selects what it is
        // attached to, x lets go and the top goes back to its own height
        let rel = crate::props::relation(
            ui,
            "Top attached to",
            &names.join(", "),
            "Its top follows the underside of these. Click to select them",
            Some("Detach: the top goes back to the wall's own height"),
        );
        if rel.select {
            if let Some(first) = w.attach_top.first() {
                crate::props::ask_select(ui, *first);
            }
        }
        if rel.let_go {
            w.attach_top.clear();
            label = "Detach wall top"
        }
    }

    if w.is_arc() {
        // a curved wall's radius: its ends stay, it bends more or less
        let chord = w.start.distance(w.end);
        let creabim_core::sketch::Curve::Arc { radius, sweep, .. } = w.curve() else {
            unreachable!()
        };
        let mut r = radius;
        if num(ui, "Radius", &mut r, chord / 2.0 + 1e-3, 1e4) {
            let minor = 2.0 * (chord / (2.0 * r)).clamp(-1.0, 1.0).asin();
            // the side it bulges to, and more or less than a half circle, stay
            let s = if sweep.abs() > std::f64::consts::PI {
                std::f64::consts::TAU - minor
            } else {
                minor
            };
            w.bulge = w.bulge.signum() * (s / 4.0).tan();
            label = "Change wall radius"
        }
    }
    let (mut a, mut b) = walls::location_line(model, w);
    let moved_a = num(ui, "Start X", &mut a.x, -1e4, 1e4) | num(ui, "Start Y", &mut a.y, -1e4, 1e4);
    let moved_b = num(ui, "End X", &mut b.x, -1e4, 1e4) | num(ui, "End Y", &mut b.y, -1e4, 1e4);
    if moved_a || moved_b {
        walls::set_location_line(model, w, a, b);
        label = if moved_a {
            "Move wall start"
        } else {
            "Move wall end"
        }
    }
    label
}

/// What is worked out about a wall, last in its block (AMD-015): its
/// length and its exterior side, with Flip (⇄) at the side's row end.
/// Returns the undo label when it was flipped.
pub(super) fn wall_worked(ui: &mut egui::Ui, w: &mut Wall) -> &'static str {
    let flip = crate::props::worked_out(
        ui,
        "wall",
        &[
            crate::props::Worked::new("Length", crate::units::len(w.length())),
            crate::props::Worked::new("Exterior", exterior_side(w)).action(
                "flip-horizontal-2",
                "Flip inside / outside (Space): swap the exterior and interior faces about the location line; its doors and windows turn with it",
            ),
        ],
    );
    if flip == Some(1) {
        walls::flip_wall(w);
        return "Flip wall";
    }
    ""
}

thread_local! {
    /// The Opens menu is left out: Properties shows the kind in its own
    /// block, with the picker (Mockup_Door_Window_Picker_REV00).
    pub(super) static HIDE_OPENS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// The Opens row was clicked: where, to open the picker under it.
    pub(super) static PICK_KIND: std::cell::Cell<Option<egui::Rect>> = const { std::cell::Cell::new(None) };
}

/// The Opens row as Properties shows it, first in Geometry: the kind's
/// name in the field, a chevron, as any menu (owner MU105: no picture in
/// a field, the Preview shows it); a click opens the picker.
pub(super) fn opens_row(ui: &mut egui::Ui, kind: OpeningKind, frame: &creabim_core::OpeningFrame) {
    let pick = crate::kindpick::Pick::of_frame(kind, frame);
    crate::props::row(ui, "Opens", |ui| {
        let v = ui.visuals().clone();
        let size = egui::vec2(ui.available_width(), ui.spacing().interact_size.y);
        let (r, resp) = ui.allocate_exact_size(size, egui::Sense::click());
        let stroke = if resp.hovered() {
            v.widgets.hovered.bg_stroke
        } else {
            v.widgets.inactive.bg_stroke
        };
        let p = ui.painter();
        p.rect(
            r,
            v.widgets.inactive.corner_radius,
            v.text_edit_bg_color(),
            stroke,
            egui::StrokeKind::Inside,
        );
        let pic =
            egui::Rect::from_min_size(r.min + egui::vec2(2.0, 0.0), egui::vec2(0.0, r.height()));
        let name = pick.map_or("Custom grid", |k| k.label());
        let font = egui::TextStyle::Body.resolve(ui.style());
        let room = r.right() - 18.0 - (pic.right() + 6.0);
        let g = p.layout(name.to_string(), font, v.text_color(), f32::INFINITY);
        let gal = if g.size().x > room {
            p.layout(
                format!(
                    "{}…",
                    &name[..name
                        .len()
                        .min(((room / g.size().x) * name.len() as f32) as usize)
                        .saturating_sub(1)]
                ),
                egui::TextStyle::Body.resolve(ui.style()),
                v.text_color(),
                f32::INFINITY,
            )
        } else {
            g
        };
        p.galley(
            egui::pos2(pic.right() + 6.0, r.center().y - gal.size().y / 2.0),
            gal,
            v.text_color(),
        );
        let c = egui::pos2(r.right() - 10.0, r.center().y);
        p.line_segment(
            [c + egui::vec2(-3.5, -1.5), c + egui::vec2(0.0, 2.0)],
            egui::Stroke::new(1.2, v.weak_text_color()),
        );
        p.line_segment(
            [c + egui::vec2(0.0, 2.0), c + egui::vec2(3.5, -1.5)],
            egui::Stroke::new(1.2, v.weak_text_color()),
        );
        if resp
            .on_hover_text(format!(
                "{name}: click to choose how it opens, or one of your types"
            ))
            .clicked()
        {
            PICK_KIND.with(|k| k.set(Some(r)));
        }
    });
}

/// How a door or window opens: its operation and, for a pair of leaves,
/// the active leaf's share of the width. Returns whether it changed.
pub(super) fn operation_fields(
    ui: &mut egui::Ui,
    model: &Model,
    kind: OpeningKind,
    frame: &mut creabim_core::OpeningFrame,
) -> bool {
    if kind == OpeningKind::Window {
        return window_fields(ui, model, frame);
    }
    let ops = creabim_core::Operation::for_kind(kind);
    if ops.len() < 2 {
        return false;
    }
    let mut changed = false;
    let mut current = frame.operation(kind);
    // the button shows a short name, so it is never cut off at the
    // narrowest panel; the menu lists the full names
    if HIDE_OPENS.with(|h| h.get()) {
    } else {
        crate::app::helpui::combo(ui, "Opens")
            .selected_text(current.short_label(kind))
            .show_ui(ui, |ui| {
                for &op in ops {
                    if ui
                        .selectable_value(&mut current, op, op.label(kind))
                        .changed()
                    {
                        frame.set_operation(kind, op);
                        changed = true;
                    }
                }
            });
    }
    if current == creabim_core::Operation::MultiSlide {
        let mut n = frame.panels().clamp(2, 4);
        let mut fixed = frame.fixed();
        let edited = crate::props::row(ui, "Panels", |ui| {
            crate::props::fill(ui, egui::DragValue::new(&mut n).range(2..=4))
                .on_hover_text("Glazed panels side by side, on two tracks.")
                .changed()
        });
        if edited {
            fixed &= (1 << n) - 1;
            frame.set_panels(kind, n, fixed);
            changed = true;
        }
        // joined on/off buttons, as every such row (house widget): lit
        // is fixed, and the letters read as the schedule's "F S S F"
        let mut on: Vec<bool> = (0..n).map(|i| fixed >> i & 1 == 1).collect();
        let tips: Vec<String> = (0..n)
            .map(|i| {
                format!(
                    "Panel {} from the door's start: F fixed, S sliding. Click to change.",
                    i + 1
                )
            })
            .collect();
        let names: Vec<(&str, &str)> = on
            .iter()
            .zip(&tips)
            .map(|(f, tip)| (if *f { "F" } else { "S" }, tip.as_str()))
            .collect();
        if crate::props::toggles(ui, "Fixed", &mut on, &names) {
            let fixed = on
                .iter()
                .enumerate()
                .fold(0, |m, (i, f)| if *f { m | 1 << i } else { m });
            frame.set_panels(kind, n, fixed);
            changed = true;
        }
    }
    if current == creabim_core::Operation::Folding {
        let mut n = frame.panels().clamp(3, 6);
        let edited = crate::props::row(ui, "Panels", |ui| {
            crate::props::fill(ui, egui::DragValue::new(&mut n).range(3..=6))
                .on_hover_text("Equal panels side by side, folding and stacking against the door's hinge side.")
                .changed()
        });
        if edited {
            frame.set_folding_panels(kind, n);
            changed = true;
        }
    }
    if current.pair() {
        let mut pc = (frame.active_share() * 100.0).round();
        let edited = crate::props::row(ui, "Active leaf", |ui| {
            crate::props::fill(
                ui,
                egui::DragValue::new(&mut pc)
                    .range(50.0..=85.0)
                    .suffix(" %"),
            )
            .on_hover_text("The share of the clear width taken by the leaf with the handle, hung on the door's hand side. 50 % makes equal leaves.")
            .changed()
        });
        if edited {
            frame.set_active_share(pc / 100.0);
            changed = true;
        }
    }
    changed
}

/// The lights round a door or window: a fixed sidelight at either end
/// or both, and a transom light over it all, fixed or opening, with
/// their sizes. `w` and `h` are the opening's, for the sizes. Returns
/// whether it changed.
pub(super) fn lights_fields(
    ui: &mut egui::Ui,
    frame: &mut creabim_core::OpeningFrame,
    w: f64,
    h: f64,
) -> bool {
    use creabim_core::cells::{Cell, Mechanism};
    // only round a door or window that is one core: not a free grid
    let Some(mut a) = frame.grid.assembly() else {
        return false;
    };
    let mut changed = false;
    let mut on = [a.left.is_some(), a.right.is_some()];
    let names = [
        (
            "Start",
            "A fixed sidelight at the opening's start, the end nearer the wall's start.",
        ),
        ("End", "A fixed sidelight at the opening's other end."),
    ];
    if crate::props::toggles(ui, "Sidelights", &mut on, &names) {
        let share = a.left.or(a.right).unwrap_or(0.3 / w.max(0.6));
        a.left = on[0].then_some(share);
        a.right = on[1].then_some(share);
        changed = true;
    }
    if let Some(share) = a.left.or(a.right) {
        let mut sw = share * w;
        if num(ui, "Sidelight", &mut sw, 0.1, (w * 0.4).max(0.1)) {
            let s = (sw / w.max(0.1)).clamp(0.02, 0.4);
            a.left = a.left.map(|_| s);
            a.right = a.right.map(|_| s);
            changed = true;
        }
    }
    let lights = [
        (None, "None", "No transom light."),
        (
            Some(Mechanism::Fixed),
            "Fixed",
            "A fixed light over the whole opening.",
        ),
        (
            Some(Mechanism::TopHung),
            "Awning",
            "A light over the whole opening, hinged at its top and opening out.",
        ),
        (
            Some(Mechanism::BottomHung),
            "Hopper",
            "A light over the whole opening, hinged at its bottom and opening in.",
        ),
    ];
    let current = a.transom.map(|(_, c)| c.mechanism);
    let label = lights
        .iter()
        .find(|l| l.0 == current)
        .map_or("Custom", |l| l.1);
    crate::app::helpui::combo(ui, "Transom")
        .selected_text(label)
        .show_ui(ui, |ui| {
            for (m, text, tip) in lights {
                if ui
                    .selectable_label(current == m, text)
                    .on_hover_text(tip)
                    .clicked()
                    && current != m
                {
                    let share = a.transom.map_or(0.4 / h.max(1.0), |(s, _)| s);
                    a.transom = m.map(|m| {
                        (
                            share,
                            Cell {
                                // a hopper opens into the room
                                flipped: m == Mechanism::BottomHung,
                                ..Cell::new(m)
                            },
                        )
                    });
                    changed = true;
                }
            }
        });
    if let Some((share, cell)) = a.transom {
        let mut th = share * h;
        if num(ui, "Its height", &mut th, 0.1, (h * 0.5).max(0.1)) {
            a.transom = Some(((th / h.max(0.1)).clamp(0.05, 0.5), cell));
            changed = true;
        }
    }
    if changed {
        frame.grid = a.grid();
    }
    changed
}

/// The opening's head: flat, a segmental or round arch, or raked, with
/// its rise or fall. Arching a door or window that opens, with no transom
/// yet, adds a fixed transom light under the arch, so its leaves and
/// sashes stay square.
pub(super) fn head_fields(
    ui: &mut egui::Ui,
    frame: &mut creabim_core::OpeningFrame,
    w: f64,
    h: f64,
) -> bool {
    use creabim_core::cells::{Cell, Mechanism, Outline};
    let mut changed = false;
    let now = frame.grid.outline.fitted(w, h);
    #[derive(Clone, Copy, PartialEq)]
    enum Head {
        Flat,
        Segmental,
        Round,
        Raked,
    }
    let head = match now {
        Outline::Rectangle => Head::Flat,
        Outline::Segmental { .. } => Head::Segmental,
        Outline::Round => Head::Round,
        Outline::Raked { .. } => Head::Raked,
    };
    let heads = [
        (Head::Flat, "Flat", "A square head."),
        (
            Head::Segmental,
            "Segmental",
            "A segmental arch: a shallow arch, part of a circle, over upright sides.",
        ),
        (
            Head::Round,
            "Round",
            "A round arch: a semicircle, rising half the width.",
        ),
        (
            Head::Raked,
            "Raked",
            "A sloping head, as under a stair or a roof.",
        ),
    ];
    let label = heads.iter().find(|x| x.0 == head).map_or("Flat", |x| x.1);
    let mut pick = None;
    crate::app::helpui::combo(ui, "Head")
        .selected_text(label)
        .show_ui(ui, |ui| {
            for (k, text, tip) in heads {
                if ui
                    .selectable_label(head == k, text)
                    .on_hover_text(tip)
                    .clicked()
                    && head != k
                {
                    pick = Some(k);
                }
            }
        });
    let mut outline = now;
    if let Some(k) = pick {
        outline = match k {
            Head::Flat => Outline::Rectangle,
            Head::Segmental => Outline::Segmental {
                rise: (w * 0.2).min(h * 0.3),
            },
            Head::Round => Outline::Round,
            Head::Raked => Outline::Raked {
                drop: (w * 0.3).min(h * 0.3),
            },
        }
        .fitted(w, h);
        changed = true;
    }
    match outline {
        Outline::Segmental { rise } => {
            let mut r = rise;
            if num(ui, "Rise", &mut r, 0.02, (w / 2.0).min(h * 0.9)) {
                outline = Outline::Segmental { rise: r }.fitted(w, h);
                changed = true;
            }
        }
        Outline::Raked { drop } => {
            let mut end = drop >= 0.0;
            let sides = [
                (false, "Start", "The head is lower at the opening's start."),
                (true, "End", "The head is lower at the opening's end."),
            ];
            let mut fall = drop.abs();
            let a = crate::props::choice(ui, "Falls to", &mut end, &sides);
            let b = num(ui, "Fall", &mut fall, 0.02, h * 0.9);
            if a || b {
                outline = Outline::Raked {
                    drop: if end { fall } else { -fall },
                }
                .fitted(w, h);
                changed = true;
            }
        }
        _ => {}
    }
    if changed {
        frame.grid.outline = outline;
        // leaves and sashes stay square under a fixed light in the head
        let opens = frame
            .core()
            .cells
            .iter()
            .any(|c| c.mechanism != Mechanism::Fixed);
        if !outline.is_rectangle() && opens {
            if let Some(mut a) = frame.grid.assembly() {
                if a.transom.is_none() {
                    let rise = outline.rise(w);
                    a.transom = Some(((rise + 0.08) / h.max(0.1), Cell::new(Mechanism::Fixed)));
                    frame.grid = a.grid();
                }
            }
        }
    }
    changed
}

/// How a door or window is made: its leaves (a door's), the trim round
/// it, and the ratings declared for it, information only. Returns the
/// edit's name, empty when nothing changed.
pub(super) fn construction_fields(
    ui: &mut egui::Ui,
    kind: OpeningKind,
    frame: &mut creabim_core::OpeningFrame,
    size: (f64, f64),
) -> &'static str {
    use creabim_core::construction::{GlassFittings, LeafStyle};
    let mut label = "";
    if kind == OpeningKind::Hole {
        return label;
    }
    ui.separator();
    ui.label(egui::RichText::new("Construction").strong());
    // a sectional, roller or revolving door is made its own way: its
    // panels, slats or wings, not a leaf style
    let op = frame.operation(kind);
    if kind == OpeningKind::Window {
        // how its sashes and glass are made: as usual, French (small
        // panes) or slim steel (thin black members and panes)
        let name = |s: LeafStyle| {
            if s == LeafStyle::Flush {
                "Standard"
            } else {
                s.label()
            }
        };
        let now = frame.construction.leaf;
        let mut picked = None;
        crate::app::helpui::combo(ui, "Make")
            .selected_text(name(now))
            .show_ui(ui, |ui| {
                for s in [LeafStyle::Flush, LeafStyle::French, LeafStyle::SlimSteel] {
                    let tip = match s {
                        LeafStyle::Flush => "Sashes and glass as set by Opens and the panes.",
                        LeafStyle::French => "Glass in small panes between glazing bars.",
                        _ => "Thin black steel frame, sashes and bars round small panes.",
                    };
                    if ui
                        .selectable_label(now == s, name(s))
                        .on_hover_text(tip)
                        .clicked()
                        && now != s
                    {
                        picked = Some(s);
                    }
                }
            });
        if let Some(s) = picked {
            frame.construction.leaf = s;
            frame.construction.panes = None;
            // slim steel's frame is thin
            if s == LeafStyle::SlimSteel {
                frame.width = frame.width.min(0.04);
            }
            label = "Window make";
        }
    }
    let (frame_cells, frame_w) = (frame.core().cells.len(), frame.width);
    let c = &mut frame.construction;
    if kind == OpeningKind::Door && !op.lifts() && op != creabim_core::Operation::Revolving {
        let now = c.leaf;
        crate::app::helpui::combo(ui, "Leaf")
            .selected_text(now.label())
            .show_ui(ui, |ui| {
                for s in LeafStyle::ALL {
                    if ui
                        .selectable_label(now == s, s.label())
                        .on_hover_text(s.tip())
                        .clicked()
                        && now != s
                    {
                        c.leaf = s;
                        label = "Leaf";
                    }
                }
            });
        if c.leaf == LeafStyle::AllGlass {
            let now = c.fittings;
            crate::app::helpui::combo(ui, "Fittings")
                .selected_text(now.label())
                .show_ui(ui, |ui| {
                    for s in GlassFittings::ALL {
                        if ui
                            .selectable_label(now == s, s.label())
                            .on_hover_text(s.tip())
                            .clicked()
                            && now != s
                        {
                            c.fittings = s;
                            label = "Glass fittings";
                        }
                    }
                });
        }
        if c.leaf.has_panes() {
            let leaves = frame_cells.max(1);
            let [mut across, mut up] = c.panes_about(size.0, size.1, frame_w, leaves);
            // one row, across × up, so it reads whole at the narrowest
            let edited = crate::props::row(ui, "Panes", |ui| {
                let a = ui
                    .add(egui::DragValue::new(&mut across).range(1..=12))
                    .on_hover_text("Each leaf's panes across.")
                    .changed();
                ui.label("×");
                let b = ui
                    .add(egui::DragValue::new(&mut up).range(1..=12))
                    .on_hover_text("Each leaf's panes up.")
                    .changed();
                a || b
            });
            let (a, b) = (edited, false);
            if a || b {
                c.panes = Some([across, up]);
                label = "Leaf panes";
            }
        }
    }
    let t = &mut c.trim;
    if num(ui, "Lining", &mut t.lining, 0.0, 0.05) {
        label = "Lining";
    }
    if num(ui, "Architrave", &mut t.architrave, 0.0, 0.5) {
        label = "Architrave";
    }
    if kind == OpeningKind::Door && num(ui, "Threshold", &mut t.threshold, 0.0, 0.1) {
        label = "Threshold";
    }
    ui.separator();
    ui.label(egui::RichText::new("Declared ratings").strong())
        .on_hover_text("Information only: carried to schedules and IFC as given, never checked.");
    let r = &mut c.ratings;
    let text = |ui: &mut egui::Ui, name: &str, v: &mut String, hint: &str| {
        crate::props::row(ui, name, |ui| {
            // the house field: its text centred top to bottom (MU107)
            crate::props::text_field(
                ui,
                egui::TextEdit::singleline(v).hint_text(hint),
                f32::INFINITY,
            )
            .changed()
        })
    };
    if text(ui, "Fire", &mut r.fire, "FD30") {
        label = "Fire rating";
    }
    if text(ui, "Acoustic", &mut r.acoustic, "Rw 32 dB") {
        label = "Acoustic rating";
    }
    if text(ui, "Security", &mut r.security, "RC2") {
        label = "Security rating";
    }
    let mut u = r.u_value.unwrap_or(0.0);
    let changed = crate::props::row(ui, "U-value", |ui| {
        crate::props::fill(
            ui,
            egui::DragValue::new(&mut u)
                .range(0.0..=10.0)
                .speed(0.01)
                .max_decimals(2)
                .custom_formatter(|v, _| {
                    if v <= 0.0 {
                        "None".into()
                    } else {
                        format!("{v:.2} W/m²K")
                    }
                }),
        )
        .changed()
    });
    if changed {
        r.u_value = (u > 0.0).then_some(u);
        label = "U-value";
    }
    let mut on = [r.accessible];
    if crate::props::toggles(
        ui,
        "Accessible",
        &mut on,
        &[(
            "Declared",
            "Declared suitable for people with disabilities.",
        )],
    ) {
        r.accessible = on[0];
        label = "Accessible";
    }
    label
}

/// A window's *Opens* menu (its style, a grid of cells) and, for its
/// casements, which way each opens.
fn window_fields(ui: &mut egui::Ui, model: &Model, frame: &mut creabim_core::OpeningFrame) -> bool {
    use creabim_core::cells::{Mechanism, WindowStyle};
    let mut changed = false;
    let core = frame.core();
    let style = WindowStyle::of(&core);
    let inward = model
        .project_info()
        .is_some_and(|(_, p)| p.openings.casements == creabim_core::CasementSide::Inward);
    if HIDE_OPENS.with(|h| h.get()) {
    } else {
        crate::app::helpui::combo(ui, "Opens")
            .selected_text(style.map_or("Custom grid", |s| s.short_label()))
            .show_ui(ui, |ui| {
                for s in WindowStyle::ALL {
                    if ui.selectable_label(style == Some(s), s.label()).clicked()
                        && style != Some(s)
                    {
                        // the lights round it stay
                        let g = s.grid(inward, frame.panes());
                        frame.grid = match frame.assembly() {
                            Some(a) => creabim_core::cells::Assembly { core: g, ..a }.grid(),
                            None => g,
                        };
                        changed = true;
                    }
                }
            });
    }
    // each casement opens out or in, whatever the project's default
    let casements: Vec<usize> = (0..frame.grid.cells.len())
        .filter(|&i| frame.grid.cells[i].mechanism == Mechanism::Hinged)
        .collect();
    if !casements.is_empty() {
        let mut on: Vec<bool> = casements
            .iter()
            .map(|&i| frame.grid.cells[i].flipped)
            .collect();
        let names: Vec<(String, String)> = casements
            .iter()
            .enumerate()
            .map(|(k, &i)| {
                // the word follows the state: a button reading "In" while
                // the casement opened out read as "opens in: in" (owner check)
                let side = if frame.grid.cells[i].flipped {
                    "in"
                } else {
                    "out"
                };
                let n = if casements.len() == 1 {
                    let mut c = side.chars();
                    c.next()
                        .map_or(String::new(), |f| f.to_uppercase().chain(c).collect())
                } else {
                    format!("{} {side}", k + 1)
                };
                (
                    n,
                    format!(
                        "Casement {} from the window's start opens {side}. Click to open it {}.",
                        k + 1,
                        if side == "in" { "out" } else { "into the room" }
                    ),
                )
            })
            .collect();
        let names: Vec<(&str, &str)> = names
            .iter()
            .map(|(a, b)| (a.as_str(), b.as_str()))
            .collect();
        if crate::props::toggles(ui, "Opens to", &mut on, &names) {
            for (k, &i) in casements.iter().enumerate() {
                frame.grid.cells[i].flipped = on[k];
            }
            changed = true;
        }
    }
    // a sliding window's panels, as a sliding door's: which is fixed
    if style == Some(WindowStyle::Sliding) {
        let mut on: Vec<bool> = core
            .cells
            .iter()
            .map(|c| c.mechanism == Mechanism::Fixed)
            .collect();
        let tips: Vec<String> = (0..on.len())
            .map(|i| format!("Panel {} from the window's start: F fixed, S sliding. Click to change; one panel at least slides.", i + 1))
            .collect();
        let names: Vec<(&str, &str)> = on
            .iter()
            .zip(&tips)
            .map(|(f, tip)| (if *f { "F" } else { "S" }, tip.as_str()))
            .collect();
        if crate::props::toggles(ui, "Fixed", &mut on, &names) && on.iter().any(|f| !f) {
            let mut g = core.clone();
            for (c, f) in g.cells.iter_mut().zip(&on) {
                c.mechanism = if *f {
                    Mechanism::Fixed
                } else {
                    Mechanism::Sliding
                };
            }
            frame.grid = match frame.assembly() {
                Some(a) => creabim_core::cells::Assembly { core: g, ..a }.grid(),
                None => creabim_core::cells::Grid {
                    outline: frame.grid.outline,
                    ..g
                },
            };
            changed = true;
        }
    }
    changed
}

/// A door's hand and swing, or a window's facing. Returns the edit's name.
/// A roof window's rows: how it opens (fixed, top hung or centre pivot),
/// and, to read, the roof it is set in and that roof's pitch there.
/// Returns the edit's name, empty when nothing changed.
pub(super) fn roof_window_fields(
    ui: &mut egui::Ui,
    model: &Model,
    o: &mut Opening,
) -> &'static str {
    use creabim_core::cells::WindowStyle;
    let mut label = "";
    let style = WindowStyle::of(&o.frame.grid);
    crate::app::helpui::combo(ui, "Opens")
        .selected_text(style.map_or("Custom grid", |s| s.roof_label()))
        .show_ui(ui, |ui| {
            for s in WindowStyle::ROOF {
                if ui
                    .selectable_label(style == Some(s), s.roof_label())
                    .clicked()
                    && style != Some(s)
                {
                    o.frame.grid = s.grid(false, o.frame.panes());
                    label = "Window operation";
                }
            }
        });
    let roof = model
        .get(o.host)
        .map_or("-", |e| e.name.as_str())
        .to_string();
    crate::props::row(ui, "Roof", |ui| {
        ui.label(roof).on_hover_text(
            "The roof it is set in. Its width runs across the slope and its height up it.",
        )
    });
    if let Some(seat) = creabim_core::roofwindow::seat(model, o) {
        crate::props::row(ui, "Pitch", |ui| {
            ui.label(format!("{:.0}°", seat.pitch()))
                .on_hover_text("The slope it lies in, from level.")
        });
    }
    label
}

pub(super) fn opening_fields(ui: &mut egui::Ui, model: &Model, o: &mut Opening) -> &'static str {
    let mut label = "";
    let host = match model.get(o.host).map(|e| &e.kind) {
        Some(ElementKind::Wall(w)) => w.clone(),
        _ => return label,
    };
    if operation_fields(ui, model, o.kind, &mut o.frame) {
        label = if o.kind == OpeningKind::Window {
            "Window operation"
        } else {
            "Door operation"
        }
    }
    if o.kind != OpeningKind::Hole && lights_fields(ui, &mut o.frame, o.width, o.height) {
        label = "Sidelights and transom"
    }
    if head_fields(ui, &mut o.frame, o.width, o.height) {
        label = "Head"
    }
    match o.kind {
        OpeningKind::Door => {
            // a label-column row, then two equal buttons, as Blender (MU29)
            let op = o.frame.operation(o.kind);
            let slides = op.slides();
            // a door that rises, or revolves, has no hand to flip
            let lifts = op.lifts();
            let revolves = op == creabim_core::Operation::Revolving;
            crate::props::row(ui, if slides { "Slides" } else { "Hung" }, |ui| {
                let multi = o.frame.operation(o.kind) == creabim_core::Operation::MultiSlide;
                let text = creabim_geom::opening::door_hand(o, &host).to_lowercase();
                let text = if multi {
                    // its panels as the schedule reads them, not "sliding, in"
                    format!(
                        "{}, {}",
                        creabim_geom::opening::slide_pattern(o, &host),
                        if o.flip_facing { "out" } else { "in" }
                    )
                } else {
                    text.strip_prefix("slides ").unwrap_or(&text).to_string()
                };
                ui.label(text).on_hover_text(if lifts {
                    "It opens upwards, and which side of the wall it lies on: in, inside the building."
                } else if revolves {
                    "It turns in its drum, either way."
                } else if multi {
                    "Its panels left to right, seen from the side it faces (in: from inside the building): F fixed, S sliding."
                } else if slides {
                        "Which way it slides open, seen from the side its leaves are on, and which side of the wall that is."
                    } else {
                        "The hinge side seen from the side it opens to, and whether it opens into the building or out of it."
                    })
            });
            let (hand_tip, swing_tip) = if slides {
                (
                    "Slide it the other way (Shift+Space).",
                    "Put it on the other face of the wall (Space).",
                )
            } else {
                (
                    "Hinge it on the other jamb (Shift+Space).",
                    "Open it to the other side of the wall (Space).",
                )
            };
            if lifts {
                crate::props::row(ui, "Flip", |ui| {
                    let w = ui.available_width();
                    if ui
                        .add_sized([w, 22.0], egui::Button::new("Inside / outside"))
                        .on_hover_text("Put it on the other side of the wall (Space).")
                        .clicked()
                    {
                        walls::flip_facing(o);
                        label = "Flip door swing"
                    }
                });
            } else if !revolves {
                crate::props::row(ui, "Flip", |ui| {
                    // a fixed row in whole pixels: wrapping, it flickered as the panel was resized (MU59)
                    ui.horizontal(|ui| {
                        let w =
                            ((ui.available_width() - ui.spacing().item_spacing.x) / 2.0).floor();
                        if ui
                            .add_sized([w, 22.0], egui::Button::new("Hand"))
                            .on_hover_text(hand_tip)
                            .clicked()
                        {
                            walls::flip_hand(o);
                            label = "Flip door hand"
                        }
                        if ui
                            .add_sized([w, 22.0], egui::Button::new("Swing"))
                            .on_hover_text(swing_tip)
                            .clicked()
                        {
                            walls::flip_facing(o);
                            label = "Flip door swing"
                        }
                    })
                });
            }
        }
        OpeningKind::Window => {
            // the exterior is on the wall's left when it is flipped
            let out = o.faces_left(&host) == host.flip;
            // the same rows as a door's (MU29)
            crate::props::row(ui, "Outside", |ui| {
                ui.label(if out {
                    "faces the exterior"
                } else {
                    "turned to the interior"
                })
            });
            // a casement hinges on one jamb, as a door does: Hand swaps it
            // (owner, 2026-11-22: "I could not flip windows like doors")
            let hinged = o.frame.grid.cells.iter().any(|c| {
                matches!(
                    c.mechanism,
                    creabim_core::cells::Mechanism::Hinged
                        | creabim_core::cells::Mechanism::TiltTurn
                )
            });
            crate::props::row(ui, "Flip", |ui| {
                ui.horizontal(|ui| {
                    let w = if hinged {
                        ((ui.available_width() - ui.spacing().item_spacing.x) / 2.0).floor()
                    } else {
                        ui.available_width()
                    };
                    if hinged
                        && ui
                            .add_sized([w, 22.0], egui::Button::new("Hand"))
                            .on_hover_text("Hinge its sashes on their other jambs (Shift+Space).")
                            .clicked()
                    {
                        walls::flip_hand(o);
                        label = "Flip window hand"
                    }
                    if ui
                        .add_sized([w, 22.0], egui::Button::new(if hinged { "In / out" } else { "Inside / outside" }))
                        .on_hover_text("Turn it round in its wall: its sill board and frame move to the other face (Space).")
                        .clicked()
                    {
                        walls::flip_facing(o);
                        label = "Flip window"
                    }
                })
            });
        }
        OpeningKind::Hole => {
            ui.label("A bare opening: a hole through the wall with no door or window in it");
        }
    }
    // a pocket door that cannot work where it stands says why (AMD-012);
    // nothing is moved for it
    let pocket_cells = creabim_geom::opening::pocket_cells(o);
    if !pocket_cells.is_empty() {
        use creabim_geom::opening::PocketProblem as P;
        let others: Vec<Opening> = model
            .iter()
            .filter_map(|e| match &e.kind {
                ElementKind::Opening(n) if n.host == o.host => Some(n.clone()),
                _ => None,
            })
            .collect();
        // each leaf that slides into a pocket is checked on its own; a
        // pair says each trouble once, the longer shortfall if both
        let mut problems: Vec<P> = vec![];
        for &cell in &pocket_cells {
            for p in creabim_geom::opening::pocket_problems(o, cell, &host, &others) {
                match (
                    p,
                    problems
                        .iter_mut()
                        .find(|q| std::mem::discriminant(*q) == std::mem::discriminant(&p)),
                ) {
                    (P::PastEnd { short }, Some(P::PastEnd { short: s })) => *s = s.max(short),
                    (_, Some(_)) => {}
                    (_, None) => problems.push(p),
                }
            }
        }
        for p in problems {
            let text = match p {
                P::ThinWall { needs } => format!(
                    "The wall is too thin for a pocket: it needs {} at least. No cavity is cut.",
                    crate::units::len(needs)
                ),
                P::CurvedWall => {
                    "A flat leaf cannot slide into a curved wall. Use a surface sliding door here."
                        .to_string()
                }
                P::PastEnd { short } => format!(
                    "No room to slide open: the wall ends {} short of the pocket.",
                    crate::units::len(short)
                ),
                P::MeetsOpening => {
                    "The pocket runs into another door or window in this wall.".to_string()
                }
            };
            crate::props::warning(ui, text, None);
        }
    }
    label
}

impl CreaBimApp {
    /// Flip the selected walls, doors and windows: a wall's faces, a door's
    /// swing (or, with `hand`, its hinge side), a window's facing.
    pub(super) fn flip_selected(&mut self, hand: bool) {
        let model = self.doc.model();
        let mut tx = Transaction::new(if hand { "Flip door hand" } else { "Flip" });
        let mut n = 0;
        for id in self.selection.all() {
            let Some(before) = model.get(id) else {
                continue;
            };
            // a section or elevation looks the other way, its notes mirrored
            if let (false, Some(flip)) = (hand, creabim_core::transform::flip_view(model, id)) {
                tx.ops.extend(flip.ops);
                n += 1;
                continue;
            }
            let mut after = before.clone();
            let changed = match &mut after.kind {
                ElementKind::Wall(w) if !hand => {
                    walls::flip_wall(w);
                    true
                }
                ElementKind::Opening(o) if hand => {
                    // a door, or a window with a hinged sash
                    let hinged = o.kind == OpeningKind::Door
                        || (o.kind == OpeningKind::Window
                            && o.frame.grid.cells.iter().any(|c| {
                                matches!(
                                    c.mechanism,
                                    creabim_core::cells::Mechanism::Hinged
                                        | creabim_core::cells::Mechanism::TiltTurn
                                )
                            }));
                    if hinged {
                        walls::flip_hand(o);
                    }
                    hinged
                }
                ElementKind::Opening(o) => {
                    walls::flip_facing(o);
                    true
                }
                // furniture turns round about its middle
                ElementKind::Object(_) if !hand => {
                    crate::handles::turn_object(model, &mut after, 180.0);
                    true
                }
                _ => false,
            };
            if changed {
                tx = tx.replace(before.clone(), after);
                n += 1;
            }
        }
        if n == 0 {
            self.status = if hand {
                "Select a door to flip its hand.".into()
            } else {
                "Select a wall, door, window, section or elevation to flip it.".into()
            };
            return;
        }
        let r = self.doc.commit(tx);
        self.report(
            r,
            format!("Flipped {n} element{}.", if n == 1 { "" } else { "s" }),
        );
    }

    /// Allow or disallow the join at a wall's end (0 start, 1 end).
    pub(super) fn toggle_join(&mut self, id: ElementId, end: usize) {
        let Some(ElementKind::Wall(w)) = self.doc.model().get(id).map(|e| &e.kind) else {
            return;
        };
        let allow = w.disallow_join[end];
        let label = if allow {
            "Allow wall join"
        } else {
            "Disallow wall join"
        };
        let r = self.doc.edit(id, label, |e| {
            if let ElementKind::Wall(w) = &mut e.kind {
                w.disallow_join[end] = !allow;
            }
        });
        let which = if end == 0 { "start" } else { "end" };
        self.report(
            r,
            if allow {
                format!("The wall's {which} joins again.")
            } else {
                format!("The wall's {which} is free: it stays square and is not pulled along.")
            },
        );
    }

    /// Attach the selected walls' tops to the roof or slab above them, or
    /// detach them.
    pub(super) fn attach_selected(&mut self, attach: bool) {
        let model = self.doc.model();
        let mut tx = Transaction::new(if attach {
            "Attach wall top"
        } else {
            "Detach wall top"
        });
        let mut n = 0;
        for id in self.selection.all() {
            let Some(before) = model.get(id) else {
                continue;
            };
            let ElementKind::Wall(w) = &before.kind else {
                continue;
            };
            let to = if attach {
                walls::attach_candidates(model, id)
            } else {
                vec![]
            };
            if to == w.attach_top || (attach && to.is_empty()) {
                continue;
            }
            let mut after = before.clone();
            if let ElementKind::Wall(w) = &mut after.kind {
                w.attach_top = to;
            }
            tx = tx.replace(before.clone(), after);
            n += 1;
        }
        if n == 0 {
            self.status = if attach {
                "Select walls with a roof or slab above them.".into()
            } else {
                "Select walls whose tops are attached.".into()
            };
            return;
        }
        let r = self.doc.commit(tx);
        let what = if attach {
            "attached to the roof or slab above"
        } else {
            "detached"
        };
        self.report(
            r,
            format!("{n} wall top{} {what}.", if n == 1 { "" } else { "s" }),
        );
    }
}

/// The wall end nearer `p`: 0 its start, 1 its end.
pub(super) fn nearest_end(w: &Wall, p: DVec2) -> usize {
    usize::from(p.distance(w.end) < p.distance(w.start))
}

impl CreaBimApp {
    /// The right-click menu's wall, door and window items, for the
    /// selection and where the menu was opened.
    pub(super) fn wall_menu_items(&self) -> Vec<crate::ui_model::Item> {
        use crate::ui_model::Item;
        let model = self.doc.model();
        let mut items = vec![];
        let sel: Vec<&Element> = self
            .selection
            .all()
            .into_iter()
            .filter_map(|id| model.get(id))
            .collect();
        let walls: Vec<(ElementId, &Wall)> = sel
            .iter()
            .filter_map(|e| match &e.kind {
                ElementKind::Wall(w) => Some((e.id, w)),
                _ => None,
            })
            .collect();
        let doors = sel
            .iter()
            .any(|e| matches!(&e.kind, ElementKind::Opening(o) if o.kind == OpeningKind::Door));
        let openings = sel
            .iter()
            .any(|e| matches!(&e.kind, ElementKind::Opening(_)));
        if !walls.is_empty() || openings {
            let what = match (walls.is_empty(), doors) {
                (false, _) => "Flip wall",
                (true, true) => "Flip door swing",
                (true, false) => "Flip window inside/outside",
            };
            items.push(Item::new("wall.flip", what).key("Space").icon("brick-wall"));
        }
        let views = sel.iter().any(|e| {
            matches!(&e.kind, ElementKind::View(v) if matches!(v.kind, ViewKind::Section { .. }))
        });
        if views && walls.is_empty() && !openings {
            items.push(
                Item::new("wall.flip", "Flip view direction")
                    .key("Space")
                    .icon("flip-horizontal-2")
                    .hint("Look the other way along the same line"),
            );
        }
        if doors {
            items.push(
                Item::new("wall.hand", "Flip door hand")
                    .key("Shift+Space")
                    .icon("door-open"),
            );
        }
        // the join at the end nearest the click, for one wall
        let at = if self.shell.context_3d {
            self.menu_at_3d
        } else {
            self.plan.menu_at
        };
        if let ([(id, w)], Some(at)) = (walls.as_slice(), at) {
            let end = nearest_end(w, at.truncate());
            let which = if end == 0 { "start" } else { "end" };
            let text = if w.disallow_join[end] {
                format!("Allow join at its {which}")
            } else {
                format!("Disallow join at its {which}")
            };
            items.push(
                Item::new(format!("wall.join:{}:{end}", id.0), text)
                    .icon("link")
                    .hint("A disallowed end stays square and free: it doesn't join, and isn't pulled along"),
            );
            // the corner at that end, if it meets one wall there
            let model = self.doc.model();
            if let Some(other) = creabim_core::corner_partner(model, *id, end == 1) {
                let rule = creabim_core::joins::rule_for(model, *id, other).map(|(_, r)| r);
                let now = creabim_core::CornerPreset::of(rule.as_ref(), *id);
                let presets = creabim_core::CornerPreset::ALL
                    .iter()
                    .enumerate()
                    .map(|(k, p)| {
                        Item::new(format!("wall.corner:{}:{end}:{k}", id.0), p.label())
                            .checked(*p == now)
                    })
                    .collect();
                items.push(Item::sub("Corner join", presets).icon("link"));
            }
            items.push(
                Item::new(format!("wall.split:{}", id.0), "Split wall here")
                    .icon("scissors")
                    .hint("Two walls, meeting where you right-clicked; doors and windows stay where they are"),
            );
        }
        if !walls.is_empty() {
            // as in Properties: only when there is a roof or slab overhead
            let overhead = walls
                .iter()
                .any(|(id, _)| !walls::attach_candidates(model, *id).is_empty());
            items.push(
                Item::new("wall.attach", "Attach top to roof/slab")
                    .enabled(overhead)
                    .hint(if overhead {
                        "The top follows the underside of the roof or slab above"
                    } else {
                        "There is no roof or slab above this wall"
                    }),
            );
            if walls.iter().any(|(_, w)| !w.attach_top.is_empty()) {
                items.push(Item::new("wall.detach", "Detach top"));
            }
        }
        if !items.is_empty() {
            items.push(Item::Separator);
        }
        items
    }

    /// Run a wall item picked from a menu or the tool strip; false when
    /// `head` is not one.
    pub(super) fn wall_action(&mut self, head: &str, arg: &str) -> bool {
        match head {
            "wall.flip" => self.flip_selected(false),
            "wall.hand" => self.flip_selected(true),
            "wall.attach" => self.attach_selected(true),
            "wall.split" => {
                let at = if self.shell.context_3d {
                    self.menu_at_3d
                } else {
                    self.plan.menu_at
                };
                let id = self
                    .doc
                    .model()
                    .iter()
                    .find(|e| e.id.0.to_string() == arg)
                    .map(|e| e.id);
                let r = match (id, at) {
                    (Some(id), Some(at)) => {
                        creabim_core::walls::split_wall(self.doc.model(), id, at.truncate())
                            .and_then(|tx| self.doc.commit(tx).map_err(|e| e.to_string()))
                    }
                    _ => Err("right-click the wall where it should split".into()),
                };
                self.report(r, "Split the wall in two. Ctrl+Z to undo.");
            }
            "wall.detach" => self.attach_selected(false),
            "wall.corner" => {
                let mut parts = arg.split(':');
                let (id, end, k) = (parts.next(), parts.next(), parts.next());
                let id = self
                    .doc
                    .model()
                    .iter()
                    .find(|e| Some(e.id.0.to_string().as_str()) == id)
                    .map(|e| e.id);
                let preset = k
                    .and_then(|k| k.parse::<usize>().ok())
                    .and_then(|k| creabim_core::CornerPreset::ALL.get(k).copied());
                if let (Some(id), Some(p)) = (id, preset) {
                    let at_end = end == Some("1");
                    match creabim_core::corner_partner(self.doc.model(), id, at_end) {
                        Some(other) => {
                            let tx = creabim_core::joins::set_rule(
                                self.doc.model(),
                                id,
                                other,
                                "Corner join",
                                |r| p.apply(r, id),
                            );
                            let r = self.doc.commit(tx);
                            self.report(
                                r,
                                format!("Corner: {}. Ctrl+Z to undo.", p.label().to_lowercase()),
                            );
                        }
                        None => self.status = "That end meets no one wall.".into(),
                    }
                }
            }
            "wall.join" => {
                let (id, end) = arg.rsplit_once(':').unwrap_or((arg, "0"));
                let id = self
                    .doc
                    .model()
                    .iter()
                    .find(|e| e.id.0.to_string() == id)
                    .map(|e| e.id);
                if let Some(id) = id {
                    self.toggle_join(id, usize::from(end == "1"));
                }
            }
            "wall.loc" => {
                if let Some(l) = arg
                    .parse::<usize>()
                    .ok()
                    .and_then(|i| LocationLine::ALL.get(i))
                {
                    if self.plan.wall_location != *l {
                        // a location line of its own: new walls have no type
                        self.plan.types.remove(&creabim_core::TypeCategory::Wall);
                    }
                    self.plan.wall_location = *l;
                    self.status =
                        format!("New walls are drawn on their {}.", l.label().to_lowercase());
                }
            }
            "wall.shape" => {
                self.plan.wall_shape = arg.parse().unwrap_or(0);
                self.plan.pending = None;
                self.plan.wall_arc_end = None;
                self.status = match self.plan.wall_shape {
                    1 => "Arc wall: click its start, its end, then a point it passes through.",
                    2 => "Circle wall: click its centre, then a point on it.",
                    3 => "Rectangle of walls: click a corner, then the opposite one.",
                    _ => "Straight walls: click to start, click for each corner.",
                }
                .into();
            }
            "wall.flipnew" => {
                self.plan.wall_flip = !self.plan.wall_flip;
                self.status = format!(
                    "New walls have their exterior on the {} (Space flips it).",
                    if self.plan.wall_flip { "left" } else { "right" }
                );
            }
            _ => return false,
        }
        true
    }

    /// The Wall tool's location line choices, for its option strip.
    pub(super) fn wall_location_items(&self) -> Vec<crate::ui_model::Item> {
        LocationLine::ALL
            .iter()
            .enumerate()
            .map(|(i, l)| {
                crate::ui_model::Item::new(format!("wall.loc:{i}"), l.label())
                    .icon(location_icon(*l))
                    .checked(self.plan.wall_location == *l)
            })
            .collect()
    }
}

/// A window's panes across × up, in one row so it reads whole at the
/// narrowest panel. A French or slim steel window's go to its make (each
/// sash or light where it has one pane); any other's to its cells.
/// Returns whether they changed.
pub(super) fn window_panes(
    ui: &mut egui::Ui,
    frame: &mut creabim_core::OpeningFrame,
    size: (f64, f64),
) -> bool {
    use creabim_core::construction::LeafStyle;
    let make = frame.construction.leaf;
    let by_make = make.has_panes() && frame.panes() == [1, 1];
    let [mut across, mut up] = if by_make {
        // about what each cell shows
        let (cols, rows) = (
            frame.grid.columns.len().max(1) as f64,
            frame.grid.rows.len().max(1) as f64,
        );
        let sw = if make == LeafStyle::SlimSteel {
            0.03
        } else {
            frame.width * 0.9
        };
        let cw = (size.0 - 2.0 * frame.width) / cols - 2.0 * sw;
        let ch = (size.1 - 2.0 * frame.width) / rows - 2.0 * sw;
        let [a, u] = frame.construction.panes_for(cw, ch);
        [a as u32, u as u32]
    } else {
        frame.panes()
    };
    // one row filling the field column, as the other rows (owner check)
    let (a, b) = crate::props::row(ui, "Panes", |ui| {
        crate::app::panes_pair(ui, &mut across, &mut up)
    });
    let edited = a || b;
    if edited {
        if make.has_panes() {
            frame.construction.panes = Some([across as u8, up as u8]);
            frame.set_panes(1, 1);
        } else {
            frame.set_panes(across, up);
        }
    }
    edited
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Whether the right-click menu's attach item can be picked.
    fn attach_enabled(app: &CreaBimApp) -> bool {
        app.wall_menu_items().iter().any(|i| {
            matches!(i, crate::ui_model::Item::Action { id, enabled: true, .. } if id == "wall.attach")
        })
    }

    #[test]
    fn attach_top_is_offered_only_with_a_roof_or_slab_overhead() {
        let mut app = crate::app::test_app();
        let model = app.doc.model();
        let covered = model
            .iter()
            .find(|e| {
                matches!(e.kind, ElementKind::Wall(_))
                    && !walls::attach_candidates(model, e.id).is_empty()
            })
            .map(|e| e.id)
            .expect("the sample has a wall under its roof");
        app.selection.only(covered);
        assert!(attach_enabled(&app));
        // the same wall far from the house has nothing overhead
        app.doc
            .edit(covered, "Move wall", |e| {
                if let ElementKind::Wall(w) = &mut e.kind {
                    w.start += DVec2::splat(500.0);
                    w.end += DVec2::splat(500.0);
                }
            })
            .unwrap();
        assert!(walls::attach_candidates(app.doc.model(), covered).is_empty());
        assert!(!attach_enabled(&app));
    }
}
