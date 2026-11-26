//! Reaching a material from where it is used (Mockup_Material_Access_REV00,
//! owner A on all four, 2026-11-22): a material's swatch is a button that
//! opens its card in place, in every row that sets a material. The card
//! edits what is changed most (colour, cut hatch, surface pattern, name),
//! says how many use it, makes a copy for this one use, and opens the
//! Materials window for the rest.

use creabim_core::{Element, ElementId, ElementKind, Hatch, Material, Model};
use eframe::egui::{self, Color32, Rect, Sense, Vec2};
use std::cell::RefCell;

/// What a card asks the app to do, done once a frame ([`take`]).
pub enum MatAsk {
    /// Replace a material element with this one (its look or its name).
    Edit(Element),
    /// Open the Materials window on a material.
    Open(ElementId),
    /// Add a material: a copy made for one use, already set there.
    Insert(Element),
    /// Make a copy of material `.1` and use it on element `.0` in its
    /// place (in its layer, for a layered element).
    CopyFor(ElementId, ElementId),
}

/// A card opened from a view (right-click "Material: …" or Alt+click),
/// where the pointer was: its material, the element it was asked from,
/// where, and whether it is yet to be drawn once.
struct Floating {
    material: ElementId,
    element: Option<ElementId>,
    at: egui::Pos2,
    fresh: bool,
}

thread_local! {
    static ASKS: RefCell<Vec<MatAsk>> = const { RefCell::new(Vec::new()) };
    static FLOAT: RefCell<Option<Floating>> = const { RefCell::new(None) };
}

/// Open material `material`'s card at `at`, asked from `element`
/// (Mockup_Material_Section_REV00, decision 3).
pub fn open_at(material: ElementId, element: Option<ElementId>, at: egui::Pos2) {
    FLOAT.with(|f| {
        *f.borrow_mut() = Some(Floating {
            material,
            element,
            at,
            fresh: true,
        });
    });
}

/// Draw the card opened by [`open_at`], if one is open: once a frame.
pub fn floating(ctx: &egui::Context, model: &Model) {
    let Some((mid, element, at, fresh)) = FLOAT.with(|f| {
        f.borrow()
            .as_ref()
            .map(|f| (f.material, f.element, f.at, f.fresh))
    }) else {
        return;
    };
    let id = egui::Id::new("matcard.floating");
    let (mut keep, copy) = card(
        ctx,
        model,
        mid,
        Rect::from_min_size(at, Vec2::ZERO),
        id,
        "this element",
    );
    // the click that opened it is not a click outside it
    if fresh && !copy {
        keep = true;
    }
    if copy {
        if let Some(e) = element {
            push(MatAsk::CopyFor(e, mid));
        }
    }
    FLOAT.with(|f| {
        let mut f = f.borrow_mut();
        match (keep, f.as_mut()) {
            (true, Some(fl)) => fl.fresh = false,
            _ => *f = None,
        }
    });
}

pub fn push(a: MatAsk) {
    ASKS.with(|q| q.borrow_mut().push(a));
}

/// The asks of this frame's cards.
pub fn take() -> Vec<MatAsk> {
    ASKS.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// What the card's owner has to do itself: make a copy of `from` and use
/// the copy in its place, here only.
pub struct CopyHere(pub ElementId);

/// A copy of material `from`, named after it, to insert.
pub fn copy_of(model: &Model, from: ElementId) -> Option<Element> {
    let e = model.get(from)?;
    let ElementKind::Material(m) = &e.kind else {
        return None;
    };
    Some(Element::new(
        creabim_core::library::unique_name(model, &format!("{} copy", e.name)),
        ElementKind::Material(m.clone()),
    ))
}

/// Paint a material's look into `r`: its colour, its cut hatch drawn over.
fn paint(ui: &egui::Ui, r: Rect, m: Option<&Material>) {
    let p = ui.painter().with_clip_rect(r);
    let Some(m) = m else {
        // no material: an empty, dashed box
        p.rect_stroke(
            r,
            3.0,
            egui::Stroke::new(1.0, ui.visuals().weak_text_color()),
            egui::StrokeKind::Inside,
        );
        return;
    };
    let c = m.color.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8);
    p.rect_filled(r, 3.0, Color32::from_rgb(c[0], c[1], c[2]));
    let ink = if (c[0] as u32 + c[1] as u32 + c[2] as u32) < 300 {
        Color32::from_white_alpha(70)
    } else {
        Color32::from_black_alpha(70)
    };
    let s = egui::Stroke::new(1.0, ink);
    let step = 5.0;
    let diag = |dir: f32| {
        let mut k = -r.height();
        while k < r.width() + r.height() {
            let a = if dir > 0.0 {
                egui::pos2(r.left() + k, r.bottom())
            } else {
                egui::pos2(r.left() + k, r.top())
            };
            let b = a + egui::vec2(r.height(), -dir * r.height());
            p.line_segment([a, b], s);
            k += step;
        }
    };
    match m.hatch {
        Hatch::Diagonal | Hatch::Brick | Hatch::Concrete => diag(1.0),
        Hatch::Cross => {
            diag(1.0);
            diag(-1.0);
        }
        Hatch::Timber => {
            let mut y = r.top() + 3.0;
            while y < r.bottom() {
                p.line_segment([egui::pos2(r.left(), y), egui::pos2(r.right(), y)], s);
                y += 4.0;
            }
        }
        _ => {}
    }
    p.rect_stroke(
        r,
        3.0,
        egui::Stroke::new(1.0, Color32::from_black_alpha(40)),
        egui::StrokeKind::Inside,
    );
}

/// A material's swatch as a button, `size` across: a click opens its card
/// under it. `here` names the use for the card's copy button ("this
/// layer", "this element"). Returns a copy the owner should make and use.
pub fn swatch(
    ui: &mut egui::Ui,
    model: &Model,
    material: Option<ElementId>,
    salt: impl std::hash::Hash + std::fmt::Debug,
    size: Vec2,
    here: &str,
) -> Option<CopyHere> {
    let id = ui.id().with(("matcard", salt));
    let (r, resp) = ui.allocate_exact_size(size, Sense::click());
    let m = material.and_then(|mid| match model.get(mid).map(|e| &e.kind) {
        Some(ElementKind::Material(m)) => Some(m),
        _ => None,
    });
    paint(ui, r, m);
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    let resp = match material.and_then(|mid| model.get(mid)) {
        Some(e) => resp.on_hover_text(format!("{}: click to open", e.name)),
        None => resp.on_hover_text("No material"),
    };
    if resp.hovered() && m.is_some() {
        ui.painter().text(
            r.center(),
            egui::Align2::CENTER_CENTER,
            "✎",
            egui::FontId::proportional(11.0),
            Color32::WHITE,
        );
    }
    let mut open = ui.data(|d| d.get_temp::<bool>(id).unwrap_or(false));
    if resp.clicked() && material.is_some() {
        open = !open;
    }
    let mut out = None;
    if let (true, Some(mid)) = (open, material) {
        let (keep, copy) = card(ui.ctx(), model, mid, r, id, here);
        open = keep;
        out = copy.then_some(CopyHere(mid));
    }
    ui.data_mut(|d| d.insert_temp(id, open));
    out
}

/// The card of material `mid` under `anchor`. Returns (still open, make a
/// copy for this use).
fn card(
    ctx: &egui::Context,
    model: &Model,
    mid: ElementId,
    anchor: Rect,
    id: egui::Id,
    here: &str,
) -> (bool, bool) {
    let Some(e) = model.get(mid) else {
        return (false, false);
    };
    let ElementKind::Material(m) = &e.kind else {
        return (false, false);
    };
    let mut after = e.clone();
    let mut changed = false;
    let mut copy = false;
    let users = creabim_core::library::reach(model, mid).len();
    let area = egui::Area::new(id.with("card"))
        .order(egui::Order::Foreground)
        .fixed_pos(anchor.left_bottom() + egui::vec2(0.0, 4.0))
        .constrain(true)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_width(272.0);
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(36.0), Sense::hover());
                    paint(ui, r, Some(m));
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(&e.name).strong());
                        ui.label(
                            egui::RichText::new(format!(
                                "Material · {} hatch",
                                m.hatch.label().to_lowercase()
                            ))
                            .weak()
                            .size(11.5),
                        );
                    });
                });
                ui.add_space(4.0);
                let ElementKind::Material(n) = &mut after.kind else {
                    return;
                };
                // the house colour field and its wheel picker (DS-01 4.3),
                // never egui's own picker (owner markup 2026-11-22); the
                // material keeps linear RGB, the field shows sRGB
                let c = Color32::from(egui::Rgba::from_rgb(n.color[0], n.color[1], n.color[2]));
                let mut s = [c.r(), c.g(), c.b()];
                if crate::props::colour_field(
                    ui,
                    "Colour",
                    &mut s,
                    "Its colour in 3D and in coloured drawings",
                ) {
                    let l = egui::Rgba::from(Color32::from_rgb(s[0], s[1], s[2]));
                    n.color = [l.r(), l.g(), l.b()];
                    changed = true;
                }
                crate::props::row(ui, "Cut hatch", |ui| {
                    egui::ComboBox::from_id_salt(id.with("hatch"))
                        .icon(crate::props::combo_icon)
                        .width(ui.available_width())
                        .selected_text(n.hatch.label())
                        .show_ui(ui, |ui| {
                            for h in Hatch::ALL {
                                changed |=
                                    ui.selectable_value(&mut n.hatch, h, h.label()).changed();
                            }
                        });
                });
                // "Surface", not "Elevation": not every surface is drawn
                // in an elevation (owner, 2026-11-22)
                crate::props::row(ui, "Surface", |ui| {
                    let shown = n.surface_pattern.clone().unwrap_or_else(|| "None".into());
                    egui::ComboBox::from_id_salt(id.with("surface"))
                        .icon(crate::props::combo_icon)
                        .width(ui.available_width())
                        .selected_text(shown)
                        .show_ui(ui, |ui| {
                            changed |= ui
                                .selectable_value(&mut n.surface_pattern, None, "None")
                                .changed();
                            for p in creabim_core::pattern::library()
                                .iter()
                                .filter(|p| p.true_size)
                            {
                                changed |= ui
                                    .selectable_value(
                                        &mut n.surface_pattern,
                                        Some(p.name.clone()),
                                        &p.name,
                                    )
                                    .changed();
                            }
                        });
                });
                crate::props::row(ui, "Name", |ui| {
                    let mut name = after.name.clone();
                    if crate::props::text_field(
                        ui,
                        egui::TextEdit::singleline(&mut name),
                        f32::INFINITY,
                    )
                    .lost_focus()
                        && !name.trim().is_empty()
                        && name != after.name
                    {
                        after.name = name;
                        changed = true;
                    }
                });
                crate::props::warning(
                    ui,
                    &match users {
                        0 | 1 => "Used here only.".to_string(),
                        n => format!("Used by {n} elements: a change here changes them all."),
                    },
                    None,
                );
                ui.add_space(2.0);
                let w = ui.available_width();
                if ui
                    .add_sized(
                        [w, 22.0],
                        egui::Button::new(format!("Make a copy for {here}")),
                    )
                    .on_hover_text("A copy of this material, used here only, to change on its own")
                    .clicked()
                {
                    copy = true;
                }
                if ui
                    .add_sized([w, 22.0], egui::Button::new("More in Materials…"))
                    .clicked()
                {
                    push(MatAsk::Open(mid));
                }
            });
        });
    if changed {
        push(MatAsk::Edit(after));
    }
    // a click outside it, or Esc, closes it
    // (a click in its own colour picker or drop-down is not outside)
    let outside = ctx.input(|i| i.pointer.any_click())
        && !egui::Popup::is_any_open(ctx)
        && ctx
            .pointer_interact_pos()
            .is_some_and(|p| !area.response.rect.contains(p) && !anchor.contains(p));
    let esc = ctx.input(|i| i.key_pressed(egui::Key::Escape));
    (!(outside || esc || copy), copy)
}
