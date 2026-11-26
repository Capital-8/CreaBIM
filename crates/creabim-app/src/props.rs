//! One rule for every row of Properties and the settings windows: the
//! label on the left, the control on the right, every control lined up in
//! one column. On and off settings are switches; choices of four or more
//! are drop-downs; section headers are full-width rows with a chevron at
//! the right.

use eframe::egui::{self, Color32, Response, Sense, Ui, Vec2};

/// The label column's width for a row `width` wide: wide enough for most
/// labels, never more than 40% of the row.
pub fn label_width(width: f32) -> f32 {
    (width * 0.38).clamp(44.0, 160.0)
}

/// Narrower than this, a row stacks: its label above, its control the
/// full width below (A4-1). As Blender, rows keep one line down to a
/// narrow panel and cut their labels instead (MU22).
pub const STACK_BELOW: f32 = 110.0;

/// A section open on this layer: where its box starts, filled once the
/// next section starts or [`close_box`] is called (MU22, as Blender's
/// panels: each section a gentle box, a gap between boxes).
#[derive(Clone, Copy)]
struct OpenBox {
    shape: egui::layers::ShapeIdx,
    left: f32,
    right: f32,
    top: f32,
    header_bottom: f32,
    pass: u64,
}

fn box_id(ui: &Ui) -> egui::Id {
    egui::Id::new(("props.box", ui.layer_id()))
}

/// A small icon button, 20 px, the icon in the text colour, a tint when
/// hovered: for row tools (copy, browse, remove, reload), as Blender's.
pub fn icon_button(ui: &mut Ui, icon: &str, tip: &str) -> Response {
    let (r, resp) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::click());
    if resp.hovered() {
        ui.painter()
            .rect_filled(r, 4.0, ui.visuals().widgets.hovered.weak_bg_fill);
    }
    let ink = ui.visuals().text_color();
    crate::svgicon::paint(ui.painter(), r.shrink(3.0), icon, ink);
    resp.on_hover_text(tip)
}

/// A block of a tab's own, folding as a section does (a box with its band),
/// for panels drawn away from the app (the Tree tab): true while open.
/// `key` keeps it folded or open.
pub fn block(ui: &mut Ui, key: &str, title: &str, default_open: bool) -> bool {
    let id = egui::Id::new(("props.block", key));
    let open = ui
        .ctx()
        .data(|d| d.get_temp::<bool>(id))
        .unwrap_or(default_open);
    let (resp, _) = header_keyed(ui, title, open, false, key);
    if resp.clicked() {
        ui.ctx().data_mut(|d| d.insert_temp(id, !open));
        return !open;
    }
    open
}

/// One of a list's side buttons (AMD-014): 24 x 22 on raise, its icon in
/// the ink, lighter when hovered.
fn list_button(ui: &mut Ui, icon: &str, tip: &str) -> Response {
    let (r, resp) = ui.allocate_exact_size(Vec2::new(24.0, 22.0), Sense::click());
    if ui.is_rect_visible(r) {
        let v = ui.visuals();
        let fill = if resp.hovered() {
            v.widgets.hovered.weak_bg_fill
        } else {
            v.widgets.inactive.weak_bg_fill
        };
        ui.painter().rect_filled(r, 5.0, fill);
        let ink = v.text_color();
        crate::svgicon::paint(ui.painter(), r.shrink2(Vec2::new(6.0, 5.0)), icon, ink);
    }
    resp.on_hover_text(tip)
}

/// [`block`] with its help as the ? on its heading (the design system's
/// way, Site polish 3): a few lines on hover-click, and its page in the
/// guide. True while open.
pub fn block_help(
    ui: &mut Ui,
    key: &str,
    title: &str,
    default_open: bool,
    text: &str,
    page: &str,
) -> bool {
    let id = egui::Id::new(("props.block", key));
    let open = ui
        .ctx()
        .data(|d| d.get_temp::<bool>(id))
        .unwrap_or(default_open);
    let (resp, help_at) = header_keyed(ui, title, open, false, key);
    let mut toggle = resp.clicked();
    if help_at.is_positive() {
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(help_at));
        help_ring(&mut child, title, text, page);
        if child.response().clicked() {
            toggle = false;
        }
    }
    if toggle {
        ui.ctx().data_mut(|d| d.insert_temp(id, !open));
        return !open;
    }
    open
}

/// A sub-fold at the end of a block (AMD-003): the picked item's settings,
/// on the block's own label column, never a box inside the block
/// (AMD-018). True while open; `key` keeps it folded or open.
pub fn sub_fold(ui: &mut Ui, key: &str, title: &str, default_open: bool) -> bool {
    let id = egui::Id::new(("props.subfold", key));
    let open = ui
        .ctx()
        .data(|d| d.get_temp::<bool>(id))
        .unwrap_or(default_open);
    // the body under it folds by [`fold`], which finds this heading here
    ui.ctx()
        .data_mut(|d| d.insert_temp(last_fold_id(), egui::Id::new(("props.fold", key))));
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 22.0), Sense::click());
    if ui.is_rect_visible(rect) {
        // AMD-003, DSM05_08: never a band. Its words in the medium weight
        // (the strong ink, the body face), 14 px in from the block's own
        // heading, the block's chevron; hovered, only they brighten
        let v = ui.visuals();
        let base = v.widgets.noninteractive.fg_stroke.color;
        let ink = if resp.hovered() {
            v.strong_text_color()
        } else {
            base
        };
        let t = ui.ctx().animate_bool_responsive(resp.id, open);
        let c = egui::pos2(rect.left() + 14.0 + 4.0, rect.center().y);
        let ang = t * std::f32::consts::FRAC_PI_2;
        let rot = |p: Vec2| {
            let (sn, co) = ang.sin_cos();
            c + Vec2::new(p.x * co - p.y * sn, p.x * sn + p.y * co)
        };
        let stroke = egui::Stroke::new(1.3, ink);
        let pts = [
            rot(Vec2::new(-1.8, -3.6)),
            rot(Vec2::new(1.8, 0.0)),
            rot(Vec2::new(-1.8, 3.6)),
        ];
        ui.painter().line_segment([pts[0], pts[1]], stroke);
        ui.painter().line_segment([pts[1], pts[2]], stroke);
        let mut job = egui::text::LayoutJob::single_section(
            title.to_string(),
            egui::TextFormat::simple(egui::FontId::proportional(12.0), ink),
        );
        job.wrap = egui::text::TextWrapping::truncate_at_width((w - 40.0).max(10.0));
        let g = ui.painter().layout_job(job);
        ui.painter().galley(
            egui::pos2(c.x + 9.0, rect.center().y - g.size().y / 2.0),
            g,
            ink,
        );
    }
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    if resp.clicked() {
        ui.ctx().data_mut(|d| d.insert_temp(id, !open));
        return !open;
    }
    open
}

/// A tab's title line (the Properties rule, MU52): which tab is open and
/// what it is about, "Tree · how it grows", with a thin bar at its left
/// and the tab's icon. One line, cut with … when narrow, the whole of it
/// on hover.
/// `room` points are kept free at its right end for the tab's own buttons
/// (MU88: the notes' rename and delete); its rectangle.
pub fn title_line_room(
    ui: &mut Ui,
    icon: &str,
    title: &str,
    about: &str,
    hue: Option<Color32>,
    room: f32,
) -> egui::Rect {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 22.0), Sense::hover());
    if !ui.is_rect_visible(rect) {
        return rect;
    }
    let p = crate::theme::Palette::of(ui.ctx());
    // the bar and the icon in the tab group's quiet colour (REV04 §1b),
    // or grey with the colour hints off
    ui.painter().rect_filled(
        egui::Rect::from_min_size(
            rect.left_top() + Vec2::new(0.0, 3.0),
            Vec2::new(3.0, rect.height() - 6.0),
        ),
        1.5,
        hue.unwrap_or(p.muted.gamma_multiply(0.6)),
    );
    let ic = egui::Rect::from_center_size(
        egui::pos2(rect.left() + 16.0, rect.center().y),
        Vec2::splat(15.0),
    );
    crate::svgicon::paint(ui.painter(), ic, icon, hue.unwrap_or(p.muted));
    let style = ui.style();
    let strong = ui.visuals().strong_text_color();
    let mut job = egui::text::LayoutJob::default();
    job.append(
        title,
        0.0,
        egui::TextFormat::simple(egui::FontId::proportional(12.5), strong),
    );
    if !about.is_empty() {
        job.append(
            &format!("  ·  {about}"),
            0.0,
            egui::TextFormat::simple(egui::TextStyle::Small.resolve(style), p.muted),
        );
    }
    job.wrap = egui::text::TextWrapping::truncate_at_width((w - 30.0 - room).max(10.0));
    let g = ui.painter().layout_job(job);
    let cut = g.elided;
    ui.painter().galley(
        egui::pos2(rect.left() + 28.0, rect.center().y - g.size().y / 2.0),
        g,
        strong,
    );
    if cut {
        resp.on_hover_text(if about.is_empty() {
            title.to_string()
        } else {
            format!("{title} · {about}")
        });
    }
    rect
}

/// The Element tab's pointer to an element's own tab (the Properties
/// rule): a soft row with a chevron, "Grown in the Tree tab: form, crown…".
/// True when clicked: the caller opens that tab.
pub fn tab_link(ui: &mut Ui, text: &str, tip: &str) -> bool {
    ui.add_space(2.0);
    let w = ui.available_width();
    let (r, resp) = ui.allocate_exact_size(Vec2::new(w, 26.0), Sense::click());
    if ui.is_rect_visible(r) {
        let v = ui.visuals();
        let accent = v.selection.bg_fill;
        let ink = v.hyperlink_color;
        ui.painter().rect_filled(
            r,
            6.0,
            accent.gamma_multiply(if resp.hovered() { 0.26 } else { 0.14 }),
        );
        let c = egui::pos2(r.left() + 11.0, r.center().y);
        let stroke = egui::Stroke::new(1.3, ink);
        ui.painter()
            .line_segment([c + Vec2::new(-1.8, -3.6), c + Vec2::new(1.8, 0.0)], stroke);
        ui.painter()
            .line_segment([c + Vec2::new(1.8, 0.0), c + Vec2::new(-1.8, 3.6)], stroke);
        let mut job = egui::text::LayoutJob::single_section(
            text.to_string(),
            egui::TextFormat::simple(egui::TextStyle::Body.resolve(ui.style()), ink),
        );
        job.wrap = egui::text::TextWrapping::truncate_at_width((w - 28.0).max(10.0));
        let g = ui.painter().layout_job(job);
        ui.painter().galley(
            egui::pos2(r.left() + 22.0, r.center().y - g.size().y / 2.0),
            g,
            ink,
        );
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(tip)
        .clicked()
}

/// A drop-down's arrow: the sections' thin chevron, pointing down (up
/// while open), not a filled triangle (MU22).
pub fn combo_icon(ui: &Ui, rect: egui::Rect, visuals: &egui::style::WidgetVisuals, open: bool) {
    let c = rect.center();
    let s = if open { -1.0 } else { 1.0 };
    let pts = [
        c + Vec2::new(-3.6, -1.8 * s),
        c + Vec2::new(0.0, 1.8 * s),
        c + Vec2::new(3.6, -1.8 * s),
    ];
    let stroke = egui::Stroke::new(1.3, visuals.fg_stroke.color);
    ui.painter().line_segment([pts[0], pts[1]], stroke);
    ui.painter().line_segment([pts[1], pts[2]], stroke);
}

/// The fill of a section's box: a tone off the panel.
pub fn box_fill(v: &egui::Visuals) -> Color32 {
    let p = v.panel_fill;
    let (to, t) = if v.dark_mode {
        (255.0, 0.045)
    } else {
        (0.0, 0.035)
    };
    let m = |c: u8| (c as f32 + (to - c as f32) * t).round() as u8;
    Color32::from_rgb(m(p.r()), m(p.g()), m(p.b()))
}

/// Draw sections as boxes on this layer from now on (`true`), or stop
/// (Properties turns them on round its sections).
pub fn boxes(ui: &mut Ui, on: bool) {
    let id = box_id(ui).with("on");
    let pass = ui.ctx().cumulative_pass_nr();
    ui.ctx().data_mut(|d| {
        if on {
            d.insert_temp(id, pass);
        } else {
            d.remove::<u64>(id);
        }
    });
}

/// Is a section's box open to put a sub-section in? (A sub-section with
/// none above it is a section of its own: MU40.)
pub fn box_open(ui: &Ui) -> bool {
    let pass = ui.ctx().cumulative_pass_nr();
    ui.ctx()
        .data(|d| d.get_temp::<OpenBox>(box_id(ui)))
        .is_some_and(|b| b.pass == pass)
}

fn boxes_on(ui: &Ui) -> bool {
    let id = box_id(ui).with("on");
    let pass = ui.ctx().cumulative_pass_nr();
    ui.ctx().data(|d| d.get_temp::<u64>(id)) == Some(pass)
}

/// Close the open section's box, if any: it ends under its last row.
pub fn close_box(ui: &mut Ui) {
    let id = box_id(ui);
    let Some(b) = ui.ctx().data_mut(|d| {
        let got = d.get_temp::<OpenBox>(id);
        d.remove::<OpenBox>(id);
        got
    }) else {
        return;
    };
    if b.pass != ui.ctx().cumulative_pass_nr() {
        return;
    }
    let y = ui.cursor().top();
    // (the few pixels a header leaves under itself are not content)
    let content = y > b.header_bottom + ui.spacing().item_spacing.y + 1.0;
    if content {
        ui.add_space(4.0);
    }
    let bottom = if content { y + 4.0 } else { b.header_bottom };
    let v = ui.visuals();
    let rect = egui::Rect::from_min_max(
        egui::pos2(b.left - PAD, b.top),
        egui::pos2(b.right + PAD, bottom),
    );
    ui.painter()
        .set(b.shape, egui::Shape::rect_filled(rect, 4.0, box_fill(v)));
    ui.add_space(4.0);
}

/// A box reaches this far past the content column on each side, which
/// is the margin its rows keep inside it.
const PAD: f32 = 6.0;

/// (Rows needed an inset before boxes took a margin; none now.)
fn inset(_ui: &Ui) -> f32 {
    0.0
}

/// A header row (MU24): a short `label` from the far left, its control
/// taking the rest, as Blender's name field at the top of Properties.
pub fn head_row<R>(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    let w = ui.available_width();
    let h = ui.spacing().interact_size.y;
    ui.allocate_ui_with_layout(
        Vec2::new(w, h),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
            let muted = ui.visuals().widgets.noninteractive.fg_stroke.color;
            let (rect, _) = ui.allocate_exact_size(Vec2::new(58.0, h), Sense::hover());
            let mut job = egui::text::LayoutJob::single_section(
                label.to_string(),
                egui::TextFormat::simple(egui::TextStyle::Body.resolve(ui.style()), muted),
            );
            job.wrap = egui::text::TextWrapping::truncate_at_width(54.0);
            let g = ui.painter().layout_job(job);
            ui.painter().galley(
                rect.left_center() - Vec2::new(0.0, g.size().y / 2.0),
                g,
                muted,
            );
            add(ui)
        },
    )
    .inner
}

/// A row: `label` at the left, in the label column, then `add`'s controls
/// filling the rest. A label too long for its column is cut with … and
/// shows whole when hovered.
pub fn row<R>(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    row_inner(ui, label, None, add)
}

/// [`row`] with a short `label` and its longer wording `tip`, shown when
/// the pointer rests on the label: for a label shortened to fit its
/// column, so nothing it said is lost (MU119: "Handrail at most" was cut
/// at the panel's default width; it reads "Rail at most", and says the
/// rest here).
pub fn row_tip<R>(ui: &mut Ui, label: &str, tip: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    row_inner(ui, label, Some(tip), add)
}

/// [`row_tip`] for a length: the field every number row has.
pub fn num_tip(ui: &mut Ui, label: &str, tip: &str, v: &mut f64, lo: f64, hi: f64) -> bool {
    row_tip(ui, label, tip, |ui| {
        fill(ui, crate::units::drag(v).range(lo..=hi)).changed()
    })
}

fn row_inner<R>(ui: &mut Ui, label: &str, tip: Option<&str>, add: impl FnOnce(&mut Ui) -> R) -> R {
    // a control's own text never wraps: a value too long is cut with …
    // (A4-1)
    let add = |ui: &mut Ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
        add(ui)
    };
    if ui.available_width() < STACK_BELOW {
        return ui
            .vertical(|ui| {
                let muted = ui.visuals().widgets.noninteractive.fg_stroke.color;
                ui.add(egui::Label::new(egui::RichText::new(label).color(muted)).truncate());
                ui.horizontal(add).inner
            })
            .inner;
    }
    let w = ui.available_width() - inset(ui);
    let lw = label_width(w);
    let h0 = ui.spacing().interact_size.y;
    ui.allocate_ui_with_layout(
        Vec2::new(w, h0),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            let h = ui.spacing().interact_size.y;
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(lw, h), Sense::hover());
            let muted = ui.visuals().widgets.noninteractive.fg_stroke.color;
            let galley = ui.painter().layout(
                label.to_string(),
                egui::TextStyle::Body.resolve(ui.style()),
                muted,
                f32::INFINITY,
            );
            if galley.size().x > lw - 6.0 {
                // cut to fit, with the whole label on hover
                let mut job = egui::text::LayoutJob::single_section(
                    label.to_string(),
                    egui::TextFormat::simple(egui::TextStyle::Body.resolve(ui.style()), muted),
                );
                job.wrap = egui::text::TextWrapping::truncate_at_width(lw - 6.0);
                let g = ui.painter().layout_job(job);
                ui.painter().galley(
                    rect.right_center() - Vec2::new(g.size().x + 6.0, g.size().y / 2.0),
                    g,
                    muted,
                );
                // its longer wording when it has one, else itself, whole
                match tip {
                    Some(t) => resp.on_hover_text(t),
                    None => resp.on_hover_text(label),
                };
            } else {
                if let Some(t) = tip {
                    resp.on_hover_text(t);
                }
                // against its control, as Blender's
                ui.painter().galley(
                    rect.right_center() - Vec2::new(galley.size().x + 6.0, galley.size().y / 2.0),
                    galley,
                    muted,
                );
            }
            add(ui)
        },
    )
    .inner
}

/// Add a control at the full width left in the row, as fields and boxes
/// line up in one column.
pub fn fill(ui: &mut Ui, w: impl egui::Widget) -> Response {
    let h = ui.spacing().interact_size.y;
    // a slider's rail and value box together fit the column: at its own
    // width it pushed every block after it past the others' right edge
    ui.spacing_mut().slider_width = (ui.available_width() - 56.0).max(40.0);
    ui.add_sized([ui.available_width().max(40.0), h], w)
}

/// A one-line text field `width` wide at the row height, its text centred
/// top to bottom: the one look every single-line field shares, the same
/// as the master's Name field. A field added straight with `ui.add` fell
/// to egui's shorter default and read squeezed beside it (DSM05).
/// `f32::INFINITY` takes the full width left, as `fill` does.
pub fn text_field(ui: &mut Ui, edit: egui::TextEdit<'_>, width: f32) -> Response {
    let h = ui.spacing().interact_size.y;
    let w = if width.is_finite() {
        width
    } else {
        ui.available_width()
    };
    ui.add_sized([w.max(24.0), h], edit.vertical_align(egui::Align::Center))
}

/// A row of controls that moves whole controls to the next line when the
/// panel is narrow, never splitting a button's or a label's words (MU34:
/// "+ Picture" broke onto two lines).
pub fn flow<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> egui::InnerResponse<R> {
    ui.horizontal_wrapped(|ui| {
        // cut with … rather than growing: Extend let one long label widen
        // the whole panel and push every field past its edge (MU36)
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
        add(ui)
    })
}

/// A choice of a few as one row of equal joined buttons filling the field
/// column, the chosen one lit: Blender's expanded list (MU31). Returns
/// true when it changed.
/// One segment of a row of joined buttons: rounded at the row's ends.
fn segment_radius(i: usize, n: usize) -> egui::CornerRadius {
    let (first, last) = (i == 0, i + 1 == n);
    egui::CornerRadius {
        nw: if first { 4 } else { 0 },
        sw: if first { 4 } else { 0 },
        ne: if last { 4 } else { 0 },
        se: if last { 4 } else { 0 },
    }
}

/// As [`choice`], each option a picture rather than a word (MU39: words
/// cut to … in a narrow panel): `(value, icon, name, what it does)`.
pub fn choice_icons<T: PartialEq + Copy>(
    ui: &mut Ui,
    label: &str,
    value: &mut T,
    options: &[(T, &str, &str, &str)],
) -> bool {
    let mut changed = false;
    row(ui, label, |ui| {
        ui.spacing_mut().item_spacing.x = 1.0;
        let n = options.len().max(1);
        let w = ((ui.available_width() - (n as f32 - 1.0)) / n as f32).max(22.0);
        let h = ui.spacing().interact_size.y;
        for (i, (v, icon, name, tip)) in options.iter().enumerate() {
            let on = *value == *v;
            let mut b = egui::Button::selectable(on, "").corner_radius(segment_radius(i, n));
            if !on {
                b = b
                    .frame_when_inactive(true)
                    .fill(faint_button(ui.visuals()))
                    .stroke(faint_edge(ui.visuals()));
            }
            let resp = ui.add_sized([w, h], b);
            let ink = if on {
                ui.visuals().selection.stroke.color
            } else {
                crate::theme::Palette::of(ui.ctx()).muted
            };
            let r = egui::Rect::from_center_size(resp.rect.center(), Vec2::splat(15.0));
            crate::svgicon::paint(ui.painter(), r, icon, ink);
            if resp.on_hover_text(format!("{name}: {tip}")).clicked() && !on {
                *value = *v;
                changed = true;
            }
        }
    });
    changed
}

/// A row of joined on/off buttons, any number on at once (the sides a
/// modifier works on). True when one changed.
pub fn toggles(ui: &mut Ui, label: &str, on: &mut [bool], names: &[(&str, &str)]) -> bool {
    let mut changed = false;
    row(ui, label, |ui| {
        ui.spacing_mut().item_spacing.x = 1.0;
        let n = names.len().max(1);
        let w = ((ui.available_width() - (n as f32 - 1.0)) / n as f32).max(20.0);
        let h = ui.spacing().interact_size.y;
        for (i, (text, tip)) in names.iter().enumerate() {
            let Some(v) = on.get_mut(i) else { break };
            // the lit part's word made legible on its fill, as choice's:
            // in the dark theme lit and unlit read alike (owner check)
            let mut b = egui::Button::selectable(*v, lit_text(ui.visuals(), *v, text))
                .corner_radius(segment_radius(i, n));
            if !*v {
                b = b
                    .frame_when_inactive(true)
                    .fill(faint_button(ui.visuals()))
                    .stroke(faint_edge(ui.visuals()));
            }
            if ui.add_sized([w, h], b).on_hover_text(*tip).clicked() {
                *v = !*v;
                changed = true;
            }
        }
    });
    changed
}

/// A segment's label: on the lit part, the accent made legible on its
/// fill, so a light accent (Ink's yellow) never goes unread (MU78).
fn lit_text(v: &egui::Visuals, on: bool, text: &str) -> egui::WidgetText {
    if !on {
        return text.into();
    }
    // plain white or near-black, whichever reads on the lit fill: a pale
    // accent tint on a darker accent stayed borderline (MU78)
    let ground = crate::theme::over(v.selection.bg_fill, v.panel_fill);
    egui::RichText::new(text)
        .color(crate::theme::ink_on(ground))
        .into()
}

pub fn choice<T: PartialEq + Copy>(
    ui: &mut Ui,
    label: &str,
    value: &mut T,
    options: &[(T, &str, &str)],
) -> bool {
    let mut changed = false;
    row(ui, label, |ui| {
        ui.spacing_mut().item_spacing.x = 1.0;
        let n = options.len().max(1) as f32;
        let w = ((ui.available_width() - (n - 1.0)) / n).max(24.0);
        let h = ui.spacing().interact_size.y;
        for (i, (v, text, tip)) in options.iter().enumerate() {
            let on = *value == *v;
            let (first, last) = (i == 0, i + 1 == options.len());
            let r = egui::CornerRadius {
                nw: if first { 4 } else { 0 },
                sw: if first { 4 } else { 0 },
                ne: if last { 4 } else { 0 },
                se: if last { 4 } else { 0 },
            };
            // a label too long for its button is cut with …, its whole name
            // and what it does on hover, as Blender's (MU34)
            let mut b = egui::Button::selectable(on, lit_text(ui.visuals(), on, text))
                .corner_radius(r)
                .truncate();
            if !on {
                b = b
                    .frame_when_inactive(true)
                    .fill(faint_button(ui.visuals()))
                    .stroke(faint_edge(ui.visuals()));
            }
            let resp = ui.add_sized([w, h], b);
            let resp = if tip.is_empty() {
                resp.on_hover_text(*text)
            } else {
                resp.on_hover_text(format!("{text}: {tip}"))
            };
            if resp.clicked() && !on {
                *value = *v;
                changed = true;
            }
        }
    });
    changed
}

/// Segmented buttons filling the width they are given, as [`choice`]'s
/// but without its label: the lit part 1 px from its neighbours, the ends
/// rounded (DSM05). True when the value changed.
pub fn segment<T: PartialEq + Copy>(
    ui: &mut Ui,
    value: &mut T,
    options: &[(T, &str, &str)],
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 1.0;
        let n = options.len().max(1) as f32;
        let w = ((ui.available_width() - (n - 1.0)) / n).max(24.0);
        let h = ui.spacing().interact_size.y;
        for (i, (v, text, tip)) in options.iter().enumerate() {
            let on = *value == *v;
            let (first, last) = (i == 0, i + 1 == options.len());
            let r = egui::CornerRadius {
                nw: if first { 4 } else { 0 },
                sw: if first { 4 } else { 0 },
                ne: if last { 4 } else { 0 },
                se: if last { 4 } else { 0 },
            };
            let mut b = egui::Button::selectable(on, lit_text(ui.visuals(), on, text))
                .corner_radius(r)
                .truncate();
            if !on {
                b = b
                    .frame_when_inactive(true)
                    .fill(faint_button(ui.visuals()))
                    .stroke(faint_edge(ui.visuals()));
            }
            let resp = ui.add_sized([w, h], b);
            let resp = if tip.is_empty() {
                resp.on_hover_text(*text)
            } else {
                resp.on_hover_text(format!("{text}: {tip}"))
            };
            if resp.clicked() && !on {
                *value = *v;
                changed = true;
            }
        }
    });
    changed
}

/// A colour as a field (DS-01 4.3): the field's height and rim, a small
/// swatch and the colour's value, filling the field column; a click opens
/// the picker under it. `srgb` as the model keeps it. True when changed.
pub fn colour_field(ui: &mut Ui, label: &str, srgb: &mut [u8; 3], tip: &str) -> bool {
    let mut changed = false;
    row(ui, label, |ui| {
        let h = ui.spacing().interact_size.y;
        let w = ui.available_width().max(40.0);
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
        let popup_id = ui.auto_id_with("colour_field");
        let open = egui::Popup::is_id_open(ui.ctx(), popup_id);
        if ui.is_rect_visible(rect) {
            let v = ui.visuals();
            let stroke = if open {
                v.selection.stroke
            } else if resp.hovered() {
                v.widgets.hovered.bg_stroke
            } else {
                v.widgets.inactive.bg_stroke
            };
            let p = ui.painter();
            p.rect(
                rect,
                v.widgets.inactive.corner_radius,
                v.text_edit_bg_color(),
                stroke,
                egui::StrokeKind::Inside,
            );
            let c = Color32::from_rgb(srgb[0], srgb[1], srgb[2]);
            let sw = egui::Rect::from_center_size(
                egui::pos2(rect.left() + 14.0, rect.center().y),
                Vec2::splat(14.0),
            );
            p.rect(
                sw,
                3.0,
                c,
                egui::Stroke::new(1.0, Color32::from_black_alpha(60)),
                egui::StrokeKind::Inside,
            );
            p.text(
                egui::pos2(rect.left() + 28.0, rect.center().y),
                egui::Align2::LEFT_CENTER,
                format!("#{:02X}{:02X}{:02X}", srgb[0], srgb[1], srgb[2]),
                egui::TextStyle::Body.resolve(ui.style()),
                v.text_color(),
            );
        }
        let resp = resp.on_hover_text(tip);
        egui::Popup::menu(&resp)
            .id(popup_id)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                // the house picker (MU101): wheel, sliders, Hex
                if colour_picker(ui, popup_id.with("p"), srgb) {
                    changed = true;
                }
            });
    });
    changed
}

/// sRGB bytes from hue (degrees), saturation and value (0..1).
pub fn hsv_to_srgb(h: f32, s: f32, v: f32) -> [u8; 3] {
    let f = |n: f32| {
        let k = (n + h / 60.0) % 6.0;
        v - v * s * k.min(4.0 - k).clamp(0.0, 1.0)
    };
    [f(5.0), f(3.0), f(1.0)].map(|x| (x * 255.0).round().clamp(0.0, 255.0) as u8)
}

/// Hue (degrees), saturation and value of sRGB bytes.
pub fn srgb_to_hsv(c: [u8; 3]) -> [f32; 3] {
    let [r, g, b] = c.map(|x| x as f32 / 255.0);
    let mx = r.max(g).max(b);
    let mn = r.min(g).min(b);
    let d = mx - mn;
    let mut h = 0.0;
    if d > 0.0 {
        h = if mx == r {
            ((g - b) / d).rem_euclid(6.0)
        } else if mx == g {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        } * 60.0;
    }
    [h, if mx > 0.0 { d / mx } else { 0.0 }, mx]
}

/// A hex colour as typed or pasted: `#C24529`, `C24529` or `c45`.
pub fn parse_hex(t: &str) -> Option<[u8; 3]> {
    let t = t.trim().trim_start_matches('#');
    let t: String = match t.len() {
        3 => t.chars().flat_map(|c| [c, c]).collect(),
        6 => t.to_string(),
        _ => return None,
    };
    let b = |i: usize| u8::from_str_radix(&t[i..i + 2], 16).ok();
    Some([b(0)?, b(2)?, b(4)?])
}

#[derive(Clone)]
struct PickerState {
    hsv: [f32; 3],
    rgb: [u8; 3],
    hex: String,
}

/// The house colour picker (MU100/MU101, A, after Blender's): a hue and
/// saturation wheel with the value strip at the right edge, lined up with
/// the rows under it; HSV or RGB sliders; a Hex field to type or paste
/// into; then the colours used recently. True when the colour changed.
pub fn colour_picker(ui: &mut Ui, id: egui::Id, srgb: &mut [u8; 3]) -> bool {
    let rgb_mode_id = egui::Id::new("colour.picker.rgb");
    let recent_id = egui::Id::new("colour.picker.recent");
    // the hue is kept while the colour is grey or black, where sRGB has none
    let mut st = ui
        .data(|d| d.get_temp::<PickerState>(id))
        .filter(|s| s.rgb == *srgb)
        .unwrap_or_else(|| PickerState {
            hsv: srgb_to_hsv(*srgb),
            rgb: *srgb,
            hex: format!("#{:02X}{:02X}{:02X}", srgb[0], srgb[1], srgb[2]),
        });
    let mut rgb_mode = ui
        .data(|d| d.get_temp::<bool>(rgb_mode_id))
        .unwrap_or(false);
    let mut recent: Vec<[u8; 3]> = ui.data(|d| d.get_temp(recent_id)).unwrap_or_default();
    let (mut changed, mut settled) = (false, false);
    // narrower, as the owner asked (MU102)
    let w = 212.0;
    ui.set_width(w);
    ui.spacing_mut().item_spacing.y = 4.0;
    let v = ui.visuals().clone();
    let ring = |p: &egui::Painter, c: egui::Pos2, r: f32| {
        p.circle_stroke(
            c,
            r + 1.0,
            egui::Stroke::new(1.0, Color32::from_black_alpha(140)),
        );
        p.circle_stroke(c, r, egui::Stroke::new(2.0, Color32::WHITE));
    };

    // the wheel at the left, the value strip at the right edge
    // the wheel fills the width but for the strip and one gap, the same
    // gap as the card's margin, so the spacing is even all round (MU101)
    let gap = ui.spacing().menu_margin.left as f32;
    let d = w - 16.0 - gap;
    let (top, _) = ui.allocate_exact_size(Vec2::new(w, d), Sense::hover());
    let r = d / 2.0;
    let centre = egui::pos2(top.left() + r, top.center().y);
    let wheel_rect = egui::Rect::from_center_size(centre, Vec2::splat(2.0 * r));
    let strip = egui::Rect::from_min_max(
        egui::pos2(top.right() - 16.0, top.top()),
        egui::pos2(top.right(), top.bottom()),
    );
    let wr = ui.interact(wheel_rect, id.with("wheel"), Sense::click_and_drag());
    if let Some(p) = wr.interact_pointer_pos() {
        if wr.is_pointer_button_down_on() || wr.clicked() {
            let d = p - centre;
            st.hsv[0] = (-d.y).atan2(d.x).to_degrees().rem_euclid(360.0);
            st.hsv[1] = (d.length() / r).min(1.0);
            changed = true;
        }
    }
    settled |= wr.drag_stopped() || wr.clicked();
    let sr = ui.interact(strip.expand(3.0), id.with("value"), Sense::click_and_drag());
    if let Some(p) = sr.interact_pointer_pos() {
        if sr.is_pointer_button_down_on() || sr.clicked() {
            st.hsv[2] = (1.0 - (p.y - strip.top()) / strip.height()).clamp(0.0, 1.0);
            changed = true;
        }
    }
    settled |= sr.drag_stopped() || sr.clicked();
    let [h, s, val] = st.hsv;
    let col = |h: f32, s: f32, v: f32| {
        let c = hsv_to_srgb(h, s, v);
        Color32::from_rgb(c[0], c[1], c[2])
    };
    let p = ui.painter();
    let mut mesh = egui::Mesh::default();
    let (steps, rings) = (96u32, 8u32);
    for k in 0..=rings {
        for i in 0..steps {
            let a = i as f32 / steps as f32 * std::f32::consts::TAU;
            let f = k as f32 / rings as f32;
            let pos = centre + Vec2::new(a.cos(), -a.sin()) * r * f;
            mesh.colored_vertex(pos, col(a.to_degrees(), f, val));
        }
    }
    for k in 0..rings {
        for i in 0..steps {
            let (a, b) = (k * steps + i, k * steps + (i + 1) % steps);
            let (c, d) = (a + steps, b + steps);
            mesh.add_triangle(a, b, c);
            mesh.add_triangle(b, d, c);
        }
    }
    p.add(egui::Shape::mesh(mesh));
    p.circle_stroke(
        centre,
        r,
        egui::Stroke::new(1.0, v.widgets.inactive.bg_stroke.color),
    );
    let a = h.to_radians();
    ring(p, centre + Vec2::new(a.cos(), -a.sin()) * r * s, 6.0);
    let mut sm = egui::Mesh::default();
    let (top_c, bottom_c) = (col(h, s, 1.0), Color32::BLACK);
    sm.colored_vertex(strip.left_top(), top_c);
    sm.colored_vertex(strip.right_top(), top_c);
    sm.colored_vertex(strip.left_bottom(), bottom_c);
    sm.colored_vertex(strip.right_bottom(), bottom_c);
    sm.add_triangle(0, 1, 2);
    sm.add_triangle(1, 3, 2);
    p.add(egui::Shape::mesh(sm));
    p.rect_stroke(
        strip,
        3.0,
        egui::Stroke::new(1.0, v.widgets.inactive.bg_stroke.color),
        egui::StrokeKind::Outside,
    );
    let y = strip.top() + (1.0 - val) * strip.height();
    let knob = egui::Rect::from_min_max(
        egui::pos2(strip.left() - 3.0, y - 2.5),
        egui::pos2(strip.right() + 3.0, y + 2.5),
    );
    p.rect_stroke(
        knob,
        2.0,
        egui::Stroke::new(1.0, Color32::from_black_alpha(160)),
        egui::StrokeKind::Outside,
    );
    p.rect_stroke(
        knob,
        2.0,
        egui::Stroke::new(1.5, Color32::WHITE),
        egui::StrokeKind::Inside,
    );
    ui.add_space(gap - 4.0);

    // HSV or RGB, as Blender's
    if segment(
        ui,
        &mut rgb_mode,
        &[
            (false, "HSV", "Hue, saturation and value"),
            (true, "RGB", "Red, green and blue, 0 to 255"),
        ],
    ) {
        ui.data_mut(|d| d.insert_temp(rgb_mode_id, rgb_mode));
    }
    // a slider is a bar filled to its value, its name at the left and its
    // value at the right; drag or click along it
    let mut bar = |ui: &mut Ui, key: &str, name: &str, frac: f32, text: String| -> Option<f32> {
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 22.0), Sense::click_and_drag());
        let vv = ui.visuals();
        let p = ui.painter();
        p.rect(
            rect,
            4.0,
            vv.text_edit_bg_color(),
            vv.widgets.inactive.bg_stroke,
            egui::StrokeKind::Inside,
        );
        let fill = egui::Rect::from_min_max(
            rect.min,
            egui::pos2(
                rect.left() + rect.width() * frac.clamp(0.0, 1.0),
                rect.bottom(),
            ),
        );
        p.rect_filled(fill.shrink(1.0), 3.0, vv.selection.bg_fill);
        let font = egui::TextStyle::Body.resolve(ui.style());
        let ground = crate::theme::over(vv.selection.bg_fill, vv.panel_fill);
        let name_ink = if frac > 0.3 {
            crate::theme::ink_on(ground)
        } else {
            vv.text_color()
        };
        let val_ink = if frac > 0.85 {
            crate::theme::ink_on(ground)
        } else {
            vv.text_color()
        };
        p.text(
            rect.left_center() + Vec2::new(8.0, 0.0),
            egui::Align2::LEFT_CENTER,
            name,
            font.clone(),
            name_ink,
        );
        p.text(
            rect.right_center() - Vec2::new(8.0, 0.0),
            egui::Align2::RIGHT_CENTER,
            text,
            font,
            val_ink,
        );
        let resp = resp.on_hover_text(format!("{name}: drag or click along the bar"));
        if resp.drag_stopped() || resp.clicked() {
            settled = true;
        }
        let _ = key;
        match resp.interact_pointer_pos() {
            Some(q) if resp.is_pointer_button_down_on() || resp.clicked() => {
                Some(((q.x - rect.left()) / rect.width()).clamp(0.0, 1.0))
            }
            _ => None,
        }
    };
    if rgb_mode {
        let mut c = hsv_to_srgb(st.hsv[0], st.hsv[1], st.hsv[2]);
        let mut any = false;
        for (i, name) in ["Red", "Green", "Blue"].into_iter().enumerate() {
            if let Some(f) = bar(ui, name, name, c[i] as f32 / 255.0, c[i].to_string()) {
                c[i] = (f * 255.0).round() as u8;
                any = true;
            }
        }
        if any {
            let hsv = srgb_to_hsv(c);
            // a grey keeps the hue it had
            st.hsv = [
                if hsv[1] > 0.0 { hsv[0] } else { st.hsv[0] },
                hsv[1],
                hsv[2],
            ];
            changed = true;
        }
    } else {
        if let Some(f) = bar(
            ui,
            "h",
            "Hue",
            st.hsv[0] / 360.0,
            format!("{:.0}°", st.hsv[0]),
        ) {
            st.hsv[0] = (f * 360.0).min(359.9);
            changed = true;
        }
        if let Some(f) = bar(
            ui,
            "s",
            "Saturation",
            st.hsv[1],
            format!("{:.0} %", st.hsv[1] * 100.0),
        ) {
            st.hsv[1] = f;
            changed = true;
        }
        if let Some(f) = bar(
            ui,
            "v",
            "Value",
            st.hsv[2],
            format!("{:.0} %", st.hsv[2] * 100.0),
        ) {
            st.hsv[2] = f;
            changed = true;
        }
    }
    if changed {
        let c = hsv_to_srgb(st.hsv[0], st.hsv[1], st.hsv[2]);
        st.hex = format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2]);
    }
    ui.add_space(4.0);

    // the colour as it is, and its hex to type or paste into
    ui.horizontal(|ui| {
        let now = hsv_to_srgb(st.hsv[0], st.hsv[1], st.hsv[2]);
        let (chip, _) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::hover());
        ui.painter().rect(
            chip,
            5.0,
            Color32::from_rgb(now[0], now[1], now[2]),
            egui::Stroke::new(1.0, Color32::from_black_alpha(70)),
            egui::StrokeKind::Inside,
        );
        ui.label(egui::RichText::new("Hex").weak());
        // left aligned and centred top to bottom in its field (MU102), the
        // copy button after it
        let te = ui.add_sized(
            [ui.available_width() - 24.0, 24.0],
            egui::TextEdit::singleline(&mut st.hex)
                .font(egui::TextStyle::Monospace)
                .vertical_align(egui::Align::Center)
                .char_limit(7),
        );
        if icon_button(ui, "copy", "Copy the hex").clicked() {
            let c = hsv_to_srgb(st.hsv[0], st.hsv[1], st.hsv[2]);
            ui.ctx()
                .copy_text(format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2]));
        }
        let te =
            te.on_hover_text("Type or paste a colour: #C24529, C24529 or c45. Enter applies it.");
        if te.changed() {
            if let Some(c) = parse_hex(&st.hex) {
                let hsv = srgb_to_hsv(c);
                st.hsv = [
                    if hsv[1] > 0.0 { hsv[0] } else { st.hsv[0] },
                    hsv[1],
                    hsv[2],
                ];
                changed = true;
            }
        }
        if te.lost_focus() {
            let c = hsv_to_srgb(st.hsv[0], st.hsv[1], st.hsv[2]);
            st.hex = format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2]);
            settled = true;
        }
    });

    // the colours used lately, one click away
    if !recent.is_empty() {
        ui.add_space(2.0);
        ui.label(egui::RichText::new("RECENT").small().weak());
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::splat(4.0);
            for c in recent.clone() {
                let (rect, resp) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::click());
                let edge = if resp.hovered() {
                    v.widgets.hovered.fg_stroke
                } else {
                    egui::Stroke::new(1.0, Color32::from_black_alpha(70))
                };
                ui.painter().rect(
                    rect,
                    4.0,
                    Color32::from_rgb(c[0], c[1], c[2]),
                    edge,
                    egui::StrokeKind::Inside,
                );
                if resp
                    .on_hover_text(format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2]))
                    .clicked()
                {
                    st.hsv = srgb_to_hsv(c);
                    st.hex = format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2]);
                    changed = true;
                    settled = true;
                }
            }
        });
    }

    let now = hsv_to_srgb(st.hsv[0], st.hsv[1], st.hsv[2]);
    if changed && now != *srgb {
        *srgb = now;
    } else {
        changed = false;
    }
    st.rgb = *srgb;
    if settled {
        recent.retain(|c| *c != *srgb);
        recent.insert(0, *srgb);
        recent.truncate(10);
        ui.data_mut(|d| d.insert_temp(recent_id, recent));
    }
    ui.data_mut(|d| d.insert_temp(id, st));
    changed
}

/// A colour swatch button that opens the house picker, for a colour in
/// sRGB bytes (in place of egui's own colour button).
pub fn colour_button_srgb(ui: &mut Ui, srgb: &mut [u8; 3]) -> Response {
    // a swatch and its hex, as the colour field: one look for every
    // colour in the app (MU101)
    let size = Vec2::new(
        ui.available_width().clamp(96.0, 160.0),
        ui.spacing().interact_size.y,
    );
    let (rect, mut resp) = ui.allocate_exact_size(size, Sense::click());
    let pid = resp.id.with("picker");
    let open = egui::Popup::is_id_open(ui.ctx(), pid);
    if ui.is_rect_visible(rect) {
        let v = ui.visuals();
        let stroke = if open {
            v.selection.stroke
        } else if resp.hovered() {
            v.widgets.hovered.bg_stroke
        } else {
            v.widgets.inactive.bg_stroke
        };
        let p = ui.painter();
        p.rect(
            rect,
            v.widgets.inactive.corner_radius,
            v.text_edit_bg_color(),
            stroke,
            egui::StrokeKind::Inside,
        );
        let sw = egui::Rect::from_center_size(
            egui::pos2(rect.left() + 14.0, rect.center().y),
            Vec2::splat(14.0),
        );
        p.rect(
            sw,
            3.0,
            Color32::from_rgb(srgb[0], srgb[1], srgb[2]),
            egui::Stroke::new(1.0, Color32::from_black_alpha(60)),
            egui::StrokeKind::Inside,
        );
        p.text(
            egui::pos2(rect.left() + 28.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            format!("#{:02X}{:02X}{:02X}", srgb[0], srgb[1], srgb[2]),
            egui::TextStyle::Body.resolve(ui.style()),
            v.text_color(),
        );
    }
    let mut changed = false;
    egui::Popup::menu(&resp)
        .id(pid)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            changed = colour_picker(ui, pid.with("p"), srgb);
        });
    if changed {
        resp.mark_changed();
    }
    resp
}

/// As [`colour_button_srgb`], for a colour in linear RGB (as egui's
/// `color_edit_button_rgb` took it, so nothing stored changes meaning).
pub fn colour_button_rgb(ui: &mut Ui, rgb: &mut [f32; 3]) -> Response {
    let c = Color32::from(egui::Rgba::from_rgb(rgb[0], rgb[1], rgb[2]));
    let mut s = [c.r(), c.g(), c.b()];
    let resp = colour_button_srgb(ui, &mut s);
    if resp.changed() {
        let l = egui::Rgba::from(Color32::from_rgb(s[0], s[1], s[2]));
        *rgb = [l.r(), l.g(), l.b()];
    }
    resp
}

/// A button filling the field column after `label` (MU31: buttons stood
/// at the left, under the labels).
pub fn action(ui: &mut Ui, label: &str, text: &str, tip: &str, enabled: bool) -> Response {
    row(ui, label, |ui| {
        let h = ui.spacing().interact_size.y;
        let w = ui.available_width().max(40.0);
        let r = ui
            .add_enabled_ui(enabled, |ui| ui.add_sized([w, h], egui::Button::new(text)))
            .inner;
        if tip.is_empty() {
            r
        } else {
            r.on_hover_text(tip)
        }
    })
}

/// [`action`] with an icon before its words (MU90: Edit in Notes).
pub fn action_icon(
    ui: &mut Ui,
    label: &str,
    icon: &str,
    text: &str,
    tip: &str,
    enabled: bool,
) -> Response {
    // no label and too long for the field column: the row's full width,
    // never cut (owner: "Edit the m…" in a narrow panel)
    let need = ui
        .painter()
        .layout_no_wrap(
            format!("      {text}"),
            egui::TextStyle::Button.resolve(ui.style()),
            egui::Color32::WHITE,
        )
        .size()
        .x
        + 16.0;
    if label.is_empty() && need > field_width(ui) {
        return action_icon_body(ui, icon, text, tip, enabled);
    }
    row(ui, label, |ui| {
        action_icon_body(ui, icon, text, tip, enabled)
    })
}

fn action_icon_body(ui: &mut Ui, icon: &str, text: &str, tip: &str, enabled: bool) -> Response {
    {
        let h = ui.spacing().interact_size.y;
        let w = ui.available_width().max(40.0);
        let r = ui
            .add_enabled_ui(enabled, |ui| {
                ui.add_sized([w, h], egui::Button::new(format!("      {text}")))
            })
            .inner;
        let ink = if enabled {
            ui.visuals().text_color()
        } else {
            ui.visuals().weak_text_color()
        };
        let galley_w = ui
            .painter()
            .layout_no_wrap(
                format!("      {text}"),
                egui::TextStyle::Button.resolve(ui.style()),
                ink,
            )
            .size()
            .x;
        let left = r.rect.center().x - galley_w / 2.0;
        // a clear gap between icon and words (MU91)
        let ic = egui::Rect::from_center_size(
            egui::pos2(left + 6.0, r.rect.center().y),
            Vec2::splat(13.0),
        );
        crate::svgicon::paint(ui.painter(), ic, icon, ink);
        if tip.is_empty() {
            r
        } else {
            r.on_hover_text(tip)
        }
    }
}

/// The field column's width in a row of this Ui (what [`row`] leaves
/// after its label).
fn field_width(ui: &Ui) -> f32 {
    let w = ui.available_width() - inset(ui);
    w - label_width(w) - ui.spacing().item_spacing.x
}

/// Two buttons sharing the field column, as Blender's paired operators.
/// Each is (text, tip, enabled); returns which was clicked.
pub fn actions2(
    ui: &mut Ui,
    label: &str,
    a: (&str, &str, bool),
    b: (&str, &str, bool),
) -> (bool, bool) {
    row(ui, label, |ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let h = ui.spacing().interact_size.y;
        let w = ((ui.available_width() - 4.0) / 2.0).max(30.0);
        let go = |ui: &mut Ui, (t, tip, on): (&str, &str, bool)| {
            let r = ui
                .add_enabled_ui(on, |ui| ui.add_sized([w, h], egui::Button::new(t)))
                .inner;
            if tip.is_empty() {
                r.clicked()
            } else {
                r.on_hover_text(tip).clicked()
            }
        };
        let x = go(ui, a);
        (x, go(ui, b))
    })
}

// Explanations in Properties (AMD-012, AMD-015, the 2026-11-12 notes
// audit): no sentence wraps in the field column any more. What a note
// really was decides where it goes now: a relation is a row with a chip,
// a worked-out value a row of the "Worked out" plate, a why the ? box at
// the foot of the block, a problem a warning row, how to drag or click
// the status bar, and nothing-yet an empty state.

/// One value worked out from the model in the place of the field it
/// stands for (AMD-015): a wall's thickness from its layers, its height
/// from its top constraint. The same sunken look as the Worked out
/// plate, on the field column, `why` on hover.
pub fn readout(ui: &mut Ui, label: &str, value: impl Into<String>, why: &str) {
    let value = value.into();
    let v = ui.visuals().clone();
    let resp = egui::Frame::new()
        .fill(v.extreme_bg_color)
        .corner_radius(6.0)
        .show(ui, |ui| {
            // the band runs the full row, as the plate does
            ui.set_width(ui.available_width());
            row(ui, label, |ui| {
                ui.add_space(8.0);
                ui.add(
                    egui::Label::new(egui::RichText::new(&value).color(v.strong_text_color()))
                        .truncate(),
                );
            })
        })
        .response;
    if !why.is_empty() {
        resp.on_hover_text(why);
    }
}

/// The value of a row of [`read_rows`].
#[derive(Clone)]
pub enum Read {
    /// A value to read.
    Text(String),
    /// A value to read, monospace and whole (an IFC GlobalId), with a copy
    /// button after it: the type steps down to fit a narrow panel.
    Copy(String),
    /// Nothing known yet: a muted "—", as the mockup's Volume row.
    Unset,
}

/// One row of [`read_rows`]: its label, its value, and its tooltip.
#[derive(Clone)]
pub struct ReadRow {
    pub label: String,
    pub value: Read,
    pub tip: String,
}

impl ReadRow {
    pub fn new(label: impl Into<String>, value: Read, tip: impl Into<String>) -> Self {
        ReadRow {
            label: label.into(),
            value,
            tip: tip.into(),
        }
    }
}

/// Read-only rows straight on the panel (Properties order REV04, the Data
/// tab): the label grey and right-aligned in the label column, the value
/// beside it in the text colour, no box or plate. The label column widens
/// to the group's longest label (as far as 65% of the row) so an IFC name
/// such as ThermalTransmittance reads whole on a narrow panel. Returns the
/// index of the row whose copy button was clicked.
pub fn read_rows(ui: &mut Ui, rows: &[ReadRow]) -> Option<usize> {
    const GAP: f32 = 8.0;
    let mut clicked = None;
    let w = ui.available_width();
    let v = ui.visuals().clone();
    let body = egui::TextStyle::Body.resolve(ui.style());
    let muted = v.widgets.noninteractive.fg_stroke.color;
    let strong = v.strong_text_color();
    let widest = rows
        .iter()
        .map(|r| {
            ui.painter()
                .layout_no_wrap(r.label.clone(), body.clone(), muted)
                .size()
                .x
        })
        .fold(0.0, f32::max);
    let lw = label_width(w).max(widest + 8.0).min(w * 0.65);
    for (i, r) in rows.iter().enumerate() {
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 22.0), Sense::hover());
        // a copy row may take a second line, so it is always laid out
        if !ui.is_rect_visible(rect) && !matches!(r.value, Read::Copy(_)) {
            continue;
        }
        let mid = rect.center().y;
        let mut job = egui::text::LayoutJob::single_section(
            r.label.clone(),
            egui::TextFormat::simple(body.clone(), muted),
        );
        job.wrap = egui::text::TextWrapping::truncate_at_width(lw - 4.0);
        let g = ui.painter().layout_job(job);
        let mut cut = g.elided;
        ui.painter().galley(
            egui::pos2(rect.left() + lw - g.size().x, mid - g.size().y / 2.0),
            g,
            muted,
        );
        let x = rect.left() + lw + GAP;
        let room = (rect.right() - x).max(8.0);
        if let Read::Copy(t) = &r.value {
            // a GlobalId stays on its row: where it is longer than the
            // room, it fades into the copy button at the row's end; its
            // whole on hover (MU59)
            let room = (room - 26.0).max(8.0);
            let g = ui
                .painter()
                .layout_no_wrap(t.clone(), egui::FontId::monospace(11.0), strong);
            let long = g.size().x > room;
            let clip = egui::Rect::from_min_max(
                egui::pos2(x, rect.top()),
                egui::pos2(x + room, rect.bottom()),
            );
            ui.painter().with_clip_rect(clip).galley(
                egui::pos2(x, mid - g.size().y / 2.0),
                g.clone(),
                strong,
            );
            if long {
                // the fade: the card's own colour over the last 28 pt
                let bg = if box_open(ui) {
                    box_fill(ui.visuals())
                } else {
                    ui.visuals().panel_fill
                };
                let steps = 14;
                let fade = 28.0f32.min(room);
                for s in 0..steps {
                    let a = (s + 1) as f32 / steps as f32;
                    let x0 = clip.right() - fade + fade * s as f32 / steps as f32;
                    let x1 = x0 + fade / steps as f32 + 0.5;
                    ui.painter().rect_filled(
                        egui::Rect::from_min_max(
                            egui::pos2(x0, rect.top()),
                            egui::pos2(x1.min(clip.right()), rect.bottom()),
                        ),
                        0.0,
                        bg.gamma_multiply(a),
                    );
                }
            }
            let shown = g.size().x.min(room);
            let b =
                egui::Rect::from_center_size(egui::pos2(x + shown + 14.0, mid), Vec2::splat(20.0));
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(b));
            if icon_button(&mut child, "copy", "Copy it").clicked() {
                clicked = Some(i);
            }
            let tip = if long {
                format!("{t}\n{}", r.tip)
            } else {
                r.tip.clone()
            };
            if !tip.is_empty() {
                resp.on_hover_text(tip);
            }
            continue;
        }
        let (text, font, ink) = match &r.value {
            Read::Text(t) => (t.clone(), body.clone(), strong),
            Read::Unset => ("—".to_string(), body.clone(), muted),
            Read::Copy(_) => unreachable!(),
        };
        let mut job =
            egui::text::LayoutJob::single_section(text, egui::TextFormat::simple(font, ink));
        job.wrap = egui::text::TextWrapping::truncate_at_width(room);
        let g = ui.painter().layout_job(job);
        cut |= g.elided;
        ui.painter()
            .galley(egui::pos2(x, mid - g.size().y / 2.0), g, ink);
        let tip = match (&r.value, cut) {
            (Read::Text(t), true) => format!("{}: {t}\n{}", r.label, r.tip),
            _ => r.tip.clone(),
        };
        if !tip.is_empty() {
            resp.on_hover_text(tip);
        }
    }
    clicked
}

/// A Properties tab group, for its quiet colour hint (REV04 §1b): the
/// element and how it is made, its IFC data, the view, the project.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hue {
    Element,
    Data,
    View,
    Project,
}

impl Hue {
    /// Its colour, lighter on the dark themes and darker on the light
    /// ones (the mockup's --g-el, --g-data, --g-view, --g-proj).
    pub fn colour(self, dark: bool) -> Color32 {
        let rgb = |c: u32| Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8);
        match (self, dark) {
            (Hue::Element, false) => rgb(0xa2702a),
            (Hue::Data, false) => rgb(0x2a8279),
            (Hue::View, false) => rgb(0x3a6dab),
            (Hue::Project, false) => rgb(0x4a8445),
            (Hue::Element, true) => rgb(0xd6a560),
            (Hue::Data, true) => rgb(0x6fc4b9),
            (Hue::View, true) => rgb(0x8fb6ec),
            (Hue::Project, true) => rgb(0x93c88d),
        }
    }
}

/// The teal of the Data (IFC) group (REV04's --g-data), for the set
/// names' code pills.
pub fn data_teal(ui: &Ui) -> Color32 {
    Hue::Data.colour(crate::theme::Palette::of(ui.ctx()).dark)
}

/// An IFC set's name as REV04 shows it: monospace in a teal code pill
/// (`Pset_WallCommon`), cut with … only if the panel is narrower than the
/// name. Draws in the current (horizontal) layout, leaving room after it
/// for a ? button.
pub fn ifc_set_name(ui: &mut Ui, name: &str) -> Response {
    let teal = data_teal(ui);
    let room = (ui.available_width() - 30.0).max(20.0);
    let mut job = egui::text::LayoutJob::single_section(
        name.to_string(),
        egui::TextFormat::simple(egui::FontId::monospace(11.0), teal),
    );
    job.wrap = egui::text::TextWrapping::truncate_at_width(room - 8.0);
    let g = ui.painter().layout_job(job);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(g.size().x + 8.0, 18.0), Sense::hover());
    if ui.is_rect_visible(rect) {
        ui.painter()
            .rect_filled(rect, 4.0, teal.gamma_multiply(0.12));
        ui.painter().galley(
            egui::pos2(rect.left() + 4.0, rect.center().y - g.size().y / 2.0),
            g,
            teal,
        );
    }
    resp.on_hover_text(name)
}

/// A line of grey text under what it explains, wrapping, no box (REV04's
/// note under an IFC set's name).
pub fn plain_note(ui: &mut Ui, text: &str) {
    // information, not a control: quieter, and kept in from the edges as
    // the rows are (MU68: in a box it sat against the left edge)
    let weak = ui.visuals().weak_text_color();
    // a margin, not a space: in a row, the wrapped lines went back to the
    // left edge and only the first was kept in (MU69)
    egui::Frame::NONE
        .inner_margin(egui::Margin {
            left: 6,
            right: 6,
            top: 0,
            bottom: 0,
        })
        .show(ui, |ui| {
            ui.add(egui::Label::new(egui::RichText::new(text).size(11.0).color(weak)).wrap());
        });
}

/// An unpicked segment's fill: a faint button of its own, as Blender's (on
/// the box's fill it vanished and read as a label, MU68).
pub fn faint_button(v: &egui::Visuals) -> egui::Color32 {
    // the fields' own dark fill: the panel's fill made it invisible
    v.extreme_bg_color
}

/// An unpicked segment's hairline.
pub fn faint_edge(v: &egui::Visuals) -> egui::Stroke {
    egui::Stroke::new(1.0, v.widgets.inactive.bg_stroke.color)
}

/// A state chip after a heading (REV04's tags), drawn from `at` (its left
/// middle): `accent` for something shown or done ("imported only",
/// "written"), quiet for "planned". Returns its width.
pub fn state_chip(ui: &Ui, at: egui::Pos2, text: &str, accent: bool) -> f32 {
    let (fill, ink) = if accent {
        let a = crate::theme::Palette::of(ui.ctx()).accent;
        (a.gamma_multiply(0.18), a)
    } else {
        // REV04's quiet tag: a raised grey pill, visible on every theme
        let v = ui.visuals();
        (v.text_color().gamma_multiply(0.10), v.weak_text_color())
    };
    let g = ui
        .painter()
        .layout_no_wrap(text.to_string(), egui::FontId::proportional(10.0), ink);
    let r = egui::Rect::from_min_size(
        egui::pos2(at.x, at.y - 8.0),
        Vec2::new(g.size().x + 12.0, 16.0),
    );
    if ui.is_rect_visible(r) {
        ui.painter().rect_filled(r, 8.0, fill);
        ui.painter().galley(
            egui::pos2(r.left() + 6.0, r.center().y - g.size().y / 2.0),
            g,
            ink,
        );
    }
    r.width()
}

/// A [`state_chip`] laid out in the current layout.
pub fn chip(ui: &mut Ui, text: &str, accent: bool) -> Response {
    let w = ui
        .painter()
        .layout_no_wrap(
            text.to_string(),
            egui::FontId::proportional(10.0),
            Color32::WHITE,
        )
        .size()
        .x
        + 12.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 18.0), Sense::hover());
    state_chip(ui, rect.left_center(), text, accent);
    resp
}

/// What a click on a [`relation`] row asked for.
#[derive(Default, Clone, Copy)]
pub struct Relation {
    /// The chip: select the other thing.
    pub select: bool,
    /// The ×: let go of it.
    pub let_go: bool,
}

/// A relation as a row (AMD-012): its label, a chip with the other
/// thing's name (a click selects it) and, when it can be undone, a × at
/// the row's end to let go of it, its tooltip `let_go`.
pub fn relation(ui: &mut Ui, label: &str, name: &str, tip: &str, let_go: Option<&str>) -> Relation {
    let mut out = Relation::default();
    row(ui, label, |ui| {
        let end = if let_go.is_some() { 22.0 } else { 0.0 };
        let h = ui.spacing().interact_size.y;
        let w = (ui.available_width() - end).max(24.0);
        let (r, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
        let v = ui.visuals();
        let accent = v.selection.bg_fill;
        let ink = v.hyperlink_color;
        let fill = accent.gamma_multiply(if resp.hovered() { 0.30 } else { 0.18 });
        ui.painter().rect_filled(r, 6.0, fill);
        let icon = egui::Rect::from_center_size(
            egui::pos2(r.left() + 13.0, r.center().y),
            Vec2::splat(14.0),
        );
        crate::svgicon::paint(ui.painter(), icon, "link", ink);
        let mut job = egui::text::LayoutJob::single_section(
            name.to_string(),
            egui::TextFormat::simple(egui::TextStyle::Body.resolve(ui.style()), ink),
        );
        job.wrap = egui::text::TextWrapping::truncate_at_width((w - 30.0).max(8.0));
        let g = ui.painter().layout_job(job);
        let at = egui::pos2(r.left() + 24.0, r.center().y - g.size().y / 2.0);
        ui.painter().galley(at, g, ink);
        if resp.on_hover_text(tip).clicked() {
            out.select = true;
        }
        if let Some(t) = let_go {
            if icon_button(ui, "x", t).clicked() {
                out.let_go = true;
            }
        }
    });
    out
}

/// One value of a [`worked_out`] plate: its label, its value, and
/// whether it is out of range (drawn in the warn colour).
#[derive(Clone)]
pub struct Worked {
    pub label: String,
    pub value: String,
    pub warn: bool,
    /// A small button at the row's end that changes it (an icon and its
    /// tooltip): the wall's exterior side gets Flip (⇄).
    pub action: Option<(&'static str, &'static str)>,
}

impl Worked {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Worked {
            label: label.into(),
            value: value.into(),
            warn: false,
            action: None,
        }
    }
    pub fn action(mut self, icon: &'static str, tip: &'static str) -> Self {
        self.action = Some((icon, tip));
        self
    }
    pub fn warn_if(mut self, warn: bool) -> Self {
        self.warn = warn;
        self
    }
}

/// Values worked out from the model, read and never typed (AMD-015): one
/// sunken plate after a block's fields, its rows on the block's label
/// column, folding under "WORKED OUT" (closed: one row and a count),
/// remembered for each `key` (a kind of element). When a value changes
/// its heading lights for a moment, open or closed. Returns the index of
/// the row whose action button was clicked, if any.
pub fn worked_out(ui: &mut Ui, key: &str, values: &[Worked]) -> Option<usize> {
    plate(ui, key, "WORKED OUT", values)
}

/// What a panel asks to be drawn once its whole block is done, so its
/// Worked out plate and ? box come last (AMD-015) even when the app adds
/// more fields after the panel's own (holes, material).
#[derive(Clone)]
enum Later {
    Worked(String, Vec<Worked>),
    Info(String),
}

fn later_id() -> egui::Id {
    egui::Id::new("props.later")
}

/// The Worked out plate, drawn at [`flush_later`].
pub fn worked_later(ui: &mut Ui, key: &str, values: &[Worked]) {
    ui.ctx().data_mut(|d| {
        let later = d.get_temp_mut_or_default::<Vec<Later>>(later_id());
        // asked for again under the same key: more rows of the one plate
        // (a site element's level, then its area, AMD-018)
        for item in later.iter_mut() {
            if let Later::Worked(k, rows) = item {
                if k == key {
                    rows.extend_from_slice(values);
                    return;
                }
            }
        }
        later.push(Later::Worked(key.to_string(), values.to_vec()));
    });
}

/// The ? box, drawn at [`flush_later`] (its Help page is the selected
/// element's).
pub fn info_later(ui: &mut Ui, text: impl Into<String>, _page: &str) {
    let item = Later::Info(text.into());
    ui.ctx().data_mut(|d| {
        d.get_temp_mut_or_default::<Vec<Later>>(later_id())
            .push(item)
    });
}

/// Draw what was asked for later, at the foot of the block.
pub fn flush_later(ui: &mut Ui) {
    let items = ui
        .ctx()
        .data_mut(|d| d.remove_temp::<Vec<Later>>(later_id()))
        .unwrap_or_default();
    for item in items {
        match item {
            Later::Worked(key, values) => {
                worked_out(ui, &key, &values);
            }
            Later::Info(text) => info(ui, text, ""),
        }
    }
}

/// Forget anything asked for later and never drawn.
pub fn clear_later(ctx: &egui::Context) {
    ctx.data_mut(|d| d.remove_temp::<Vec<Later>>(later_id()));
}

/// A sunken plate of read values under a folding `title` (AMD-015): the
/// Worked out plate, or an assembly's LAYERS.
pub fn plate(ui: &mut Ui, key: &str, title: &str, values: &[Worked]) -> Option<usize> {
    let mut clicked = None;
    if values.is_empty() {
        return None;
    }
    // one key can head two plates in one panel (a terrain's survey facts
    // and its earthworks, both "site.facts"): each its own by its first
    // row, or they lit each other every frame and folded together
    // (owner, cut test: WORKED OUT stayed light blue)
    let id = egui::Id::new(("props.worked", key, values[0].label.as_str()));
    let mut state =
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, true);
    let now = ui.input(|i| i.time);
    let sig: String = values.iter().map(|v| v.value.as_str()).collect();
    let flash_id = id.with("flash");
    let (last, since) = ui
        .ctx()
        .data(|d| d.get_temp::<(String, f64)>(flash_id))
        .unwrap_or((sig.clone(), f64::NEG_INFINITY));
    let since = if last != sig { now } else { since };
    ui.ctx().data_mut(|d| d.insert_temp(flash_id, (sig, since)));
    // it lights when a value the person changed is worked out again; the
    // Log's counts change by themselves with every line logged, so they
    // never light (MU62: WORKED OUT blinked as a fold elsewhere moved)
    let lit = key != "log" && now - since < 0.8;
    if lit {
        ui.ctx().request_repaint();
    }
    let v = ui.visuals().clone();
    // light themes: a hairline round it, or white on the block's light
    // grey all but disappears (Site polish 2A); dark ones read already
    let edge = if v.dark_mode {
        egui::Stroke::NONE
    } else {
        egui::Stroke::new(
            1.0,
            v.widgets.noninteractive.bg_stroke.color.gamma_multiply(0.8),
        )
    };
    egui::Frame::new()
        .stroke(edge)
        .fill(v.extreme_bg_color)
        .corner_radius(6.0)
        .inner_margin(egui::Margin {
            left: 0,
            right: 0,
            top: 0,
            bottom: 0,
        })
        .show(ui, |ui| {
            // the same 200 ms as every fold (AMD-017)
            ui.style_mut().animation_time = 0.2;
            ui.spacing_mut().item_spacing.y = 2.0;
            let w = ui.available_width();
            let (r, resp) = ui.allocate_exact_size(Vec2::new(w, 24.0), Sense::click());
            if resp.clicked() {
                state.toggle(ui);
            }
            let openness = state.openness(ui.ctx());
            let ink = if lit {
                v.selection.bg_fill
            } else if resp.hovered() {
                v.text_color()
            } else {
                v.weak_text_color()
            };
            // the chevron, turning as the plate opens (200 ms)
            let c = egui::pos2(r.left() + 13.0, r.center().y);
            let a = openness * std::f32::consts::FRAC_PI_2;
            let rot = |x: f32, y: f32| {
                c + egui::vec2(x * a.cos() - y * a.sin(), x * a.sin() + y * a.cos())
            };
            ui.painter().add(egui::Shape::line(
                vec![rot(-2.0, -4.0), rot(2.0, 0.0), rot(-2.0, 4.0)],
                egui::Stroke::new(1.6, ink),
            ));
            let font = egui::FontId::proportional(10.0);
            ui.painter().text(
                egui::pos2(r.left() + 22.0, r.center().y),
                egui::Align2::LEFT_CENTER,
                title,
                font.clone(),
                ink,
            );
            if openness < 0.5 {
                ui.painter().text(
                    egui::pos2(r.right() - 10.0, r.center().y),
                    egui::Align2::RIGHT_CENTER,
                    values.len().to_string(),
                    font,
                    v.weak_text_color(),
                );
            }
            let resp = resp.on_hover_text(if state.is_open() {
                "Worked out from the model: change the fields to change them. Click to fold"
            } else {
                "Worked out from the model. Click to show them"
            });
            let _ = resp;
            state.show_body_unindented(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                for (i, val) in values.iter().enumerate() {
                    row(ui, &val.label, |ui| {
                        let color = if val.warn {
                            v.warn_fg_color
                        } else {
                            v.strong_text_color()
                        };
                        ui.add_space(8.0);
                        let end = if val.action.is_some() { 24.0 } else { 0.0 };
                        let w = (ui.available_width() - end).max(16.0);
                        ui.allocate_ui(Vec2::new(w, ui.spacing().interact_size.y), |ui| {
                            ui.add(
                                egui::Label::new(egui::RichText::new(&val.value).color(color))
                                    .truncate(),
                            );
                        });
                        if let Some((icon, tip)) = val.action {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if icon_button(ui, icon, tip).clicked() {
                                        clicked = Some(i);
                                    }
                                },
                            );
                        }
                    });
                }
                ui.add_space(4.0);
            });
        });
    clicked
}

/// A sub-container inside a section (MU60: "we had a DS of sub containers
/// within a container"): the inset plate the LAYERS and WORKED OUT blocks
/// already use. A sunken box, its heading in small capitals with a chevron
/// that turns, its count at the right while shut, its rows inside it, the
/// body opening and shutting smoothly (AMD-017). `key` keeps it open or
/// shut. Returns what `add` returned while open.
pub fn sub_box<R>(
    ui: &mut Ui,
    key: &str,
    title: &str,
    count: Option<usize>,
    default_open: bool,
    add: impl FnOnce(&mut Ui) -> R,
) -> Option<R> {
    sub_box_help(ui, key, title, count, default_open, false, add).0
}

/// [`sub_box`] that also leaves room for a ringed ? after its title (the
/// help rule, MU60): returns where to draw it, for the caller's
/// `quick_help_button` (MU64: a hint line moved into it).
pub fn sub_box_help<R>(
    ui: &mut Ui,
    key: &str,
    title: &str,
    count: Option<usize>,
    default_open: bool,
    help: bool,
    add: impl FnOnce(&mut Ui) -> R,
) -> (Option<R>, Option<egui::Rect>) {
    let mut help_at = None;
    let id = egui::Id::new(("props.sub", key));
    let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(
        ui.ctx(),
        id,
        default_open,
    );
    let v = ui.visuals().clone();
    let mut out = None;
    ui.add_space(2.0);
    egui::Frame::new()
        .fill(v.extreme_bg_color)
        .corner_radius(6.0)
        .show(ui, |ui| {
            // the same 200 ms as every fold (AMD-017)
            ui.style_mut().animation_time = 0.2;
            ui.spacing_mut().item_spacing.y = 2.0;
            let w = ui.available_width();
            let (r, resp) = ui.allocate_exact_size(Vec2::new(w, 24.0), Sense::click());
            if resp.clicked() {
                state.toggle(ui);
            }
            let openness = state.openness(ui.ctx());
            let ink = if resp.hovered() {
                v.text_color()
            } else {
                v.weak_text_color()
            };
            let c = egui::pos2(r.left() + 13.0, r.center().y);
            let a = openness * std::f32::consts::FRAC_PI_2;
            let rot = |x: f32, y: f32| {
                c + egui::vec2(x * a.cos() - y * a.sin(), x * a.sin() + y * a.cos())
            };
            ui.painter().add(egui::Shape::line(
                vec![rot(-2.0, -4.0), rot(2.0, 0.0), rot(-2.0, 4.0)],
                egui::Stroke::new(1.6, ink),
            ));
            let font = egui::FontId::proportional(10.0);
            let t = ui.painter().text(
                egui::pos2(r.left() + 22.0, r.center().y),
                egui::Align2::LEFT_CENTER,
                title.to_uppercase(),
                font.clone(),
                ink,
            );
            if help {
                help_at = Some(egui::Rect::from_center_size(
                    egui::pos2(t.right() + 14.0, r.center().y),
                    egui::vec2(16.0, 16.0),
                ));
            }
            if let Some(n) = count.filter(|_| openness < 0.5) {
                ui.painter().text(
                    egui::pos2(r.right() - 10.0, r.center().y),
                    egui::Align2::RIGHT_CENTER,
                    n.to_string(),
                    font,
                    v.weak_text_color(),
                );
            }
            resp.on_hover_text(if state.is_open() {
                "Click to fold"
            } else {
                "Click to open"
            });
            state.show_body_unindented(ui, |ui| {
                egui::Frame::new()
                    .inner_margin(egui::Margin {
                        left: 0,
                        right: PAD as i8,
                        top: 0,
                        bottom: 0,
                    })
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 4.0;
                        out = Some(add(ui));
                        ui.add_space(4.0);
                    });
            });
        });
    (out, help_at)
}

/// Full-width controls in a [`sub_box`] card, buttons above all: the card's
/// margin on their left as on their right. Its rows need none on the left,
/// their keys standing to the right of the key column; a button does, or it
/// sits on the card's edge (MU145, owner: "the select button isn't fitting
/// well with its left padding").
pub fn card_inset<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new()
        .inner_margin(egui::Margin {
            left: PAD as i8,
            right: 0,
            top: 0,
            bottom: 0,
        })
        .show(ui, add)
        .inner
}

/// Ask for Help at `page` ("" for the selected element's page): the app
/// opens it once Properties is drawn.
/// The ringed ? the folding sections carry, for a single row: click it
/// for `text` and a link to `page` in the guide (owner after MU91: the
/// Wrap at ? looked different and only showed on hover).
pub fn help_ring(ui: &mut Ui, title: &str, text: &str, page: &str) {
    let r = ui
        .add(
            egui::Button::new(egui::RichText::new("?").size(11.0).strong())
                .small()
                .corner_radius(8.0)
                .min_size(egui::vec2(16.0, 16.0)),
        )
        .on_hover_text(format!(
            "About {}: a few lines, and its page in the guide",
            title.to_lowercase()
        ));
    let mut read = false;
    egui::Popup::from_toggle_button_response(&r)
        .align(egui::RectAlign::BOTTOM_END)
        .align_alternatives(&[egui::RectAlign::TOP_END, egui::RectAlign::LEFT_START])
        .kind(egui::PopupKind::Tooltip)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.set_max_width(300.0);
            ui.label(egui::RichText::new(title).strong());
            ui.add_space(2.0);
            ui.label(text);
            ui.add_space(4.0);
            if ui.link("Read more in the guide \u{2192}").clicked() {
                read = true;
                ui.close();
            }
        });
    if read {
        want_help(ui, page);
    }
}

fn want_help(ui: &Ui, page: &str) {
    ui.ctx()
        .data_mut(|d| d.insert_temp(egui::Id::new("props.help"), page.to_string()));
}

/// Ask the app to select `id` (a relation chip was clicked).
pub fn ask_select(ui: &Ui, id: creabim_core::ElementId) {
    ui.ctx()
        .data_mut(|d| d.insert_temp(egui::Id::new("props.select"), id));
}

/// The element a relation chip asked to select, if any.
pub fn take_select(ctx: &egui::Context) -> Option<creabim_core::ElementId> {
    ctx.data_mut(|d| d.remove_temp::<creabim_core::ElementId>(egui::Id::new("props.select")))
}

/// The page Help was asked for while Properties was drawn, if any.
pub fn take_help(ctx: &egui::Context) -> Option<String> {
    ctx.data_mut(|d| d.remove_temp::<String>(egui::Id::new("props.help")))
}

/// The ? box at the foot of a block (AMD-012 C): why it behaves so, the
/// full width, at most three lines; ? and More… open the longer text in
/// Help, at `page` ("" for the selected element's page).
pub fn info(ui: &mut Ui, text: impl Into<String>, page: &str) {
    let text = text.into();
    let v = ui.visuals().clone();
    ui.add_space(2.0);
    egui::Frame::new()
        .fill(v.widgets.inactive.weak_bg_fill)
        .corner_radius(6.0)
        .inner_margin(egui::Margin::symmetric(8, 7))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_top(|ui| {
                let (r, q) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::click());
                let ink = if q.hovered() {
                    v.hyperlink_color
                } else {
                    v.weak_text_color()
                };
                crate::svgicon::paint(ui.painter(), r, "circle-help", ink);
                if q.on_hover_text("Explain this (F1)").clicked() {
                    want_help(ui, page);
                }
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    let mut job = egui::text::LayoutJob::single_section(
                        text.clone(),
                        egui::TextFormat::simple(
                            egui::TextStyle::Small.resolve(ui.style()),
                            v.text_color(),
                        ),
                    );
                    job.wrap = egui::text::TextWrapping {
                        max_width: ui.available_width(),
                        max_rows: 3,
                        break_anywhere: false,
                        overflow_character: Some('…'),
                    };
                    let g = ui.painter().layout_job(job);
                    // a cut note shows whole on hover: the label does that
                    // by itself (MU103: a second tip here showed it twice)
                    ui.add(egui::Label::new(g).sense(Sense::hover()));
                    if ui
                        .add(
                            egui::Label::new(
                                egui::RichText::new("More…")
                                    .small()
                                    .color(v.hyperlink_color),
                            )
                            .sense(Sense::click()),
                        )
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        want_help(ui, page);
                    }
                });
            });
        });
}

/// A problem (AMD-012): a warn-coloured box with one line and, when it
/// can be fixed from here, the button that fixes it. Returns whether the
/// button was clicked.
pub fn warning(ui: &mut Ui, text: impl Into<String>, button: Option<&str>) -> bool {
    let v = ui.visuals().clone();
    let mut clicked = false;
    ui.add_space(2.0);
    egui::Frame::new()
        .fill(v.warn_fg_color.gamma_multiply(0.16))
        .corner_radius(6.0)
        .inner_margin(egui::Margin::symmetric(8, 7))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_top(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
                crate::svgicon::paint(ui.painter(), r, "problem", v.warn_fg_color);
                ui.vertical(|ui| {
                    ui.add(egui::Label::new(egui::RichText::new(text.into()).small()).wrap());
                    if let Some(b) = button {
                        // a button's words stay on one line, cut with … if
                        // the panel is too narrow (A4-1, MU136)
                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                        clicked = ui.button(b).on_hover_text(b).clicked();
                    }
                });
            });
        });
    clicked
}

/// Nothing here yet: one muted line centred across the block. The
/// button that adds the first one follows it, from the caller.
pub fn empty(ui: &mut Ui, text: impl Into<String>) {
    ui.add_space(4.0);
    let text = egui::WidgetText::from(egui::RichText::new(text.into()).weak());
    let galley = text.into_galley(
        ui,
        Some(egui::TextWrapMode::Wrap),
        ui.available_width(),
        egui::TextStyle::Body,
    );
    // one line, centred; more, flush left: centred line by line, a
    // wrapped note looked indented (owner, cut test: Soil layers)
    if galley.rows.len() > 1 {
        ui.add(egui::Label::new(galley));
    } else {
        ui.vertical_centered(|ui| {
            ui.add(egui::Label::new(galley));
        });
    }
    ui.add_space(2.0);
}

/// How to drag, click or type something about what is selected: said in
/// the status bar while the pointer is over a view, never in Properties.
pub fn howto(ui: &Ui, text: impl Into<String>) {
    let pass = ui.ctx().cumulative_pass_nr();
    ui.ctx()
        .data_mut(|d| d.insert_temp(egui::Id::new("props.howto"), (pass, text.into())));
}

/// The how-to Properties gave this frame or the one before, if any.
pub fn current_howto(ctx: &egui::Context) -> Option<String> {
    let pass = ctx.cumulative_pass_nr();
    ctx.data(|d| d.get_temp::<(u64, String)>(egui::Id::new("props.howto")))
        .filter(|(p, _)| p + 2 >= pass)
        .map(|(_, t)| t)
}

/// A line of explanation under the controls it explains. The site panels
/// call this; it shows as the standard note until they get their DS-01 pass.
pub fn note(ui: &mut Ui, text: impl Into<String>) {
    info(ui, text, "");
}

/// A drop-down list entry with a small picture before its label, painted
/// by `paint` in a square of the text's height.
pub fn picture_option(
    ui: &mut Ui,
    selected: bool,
    label: &str,
    paint: impl FnOnce(&egui::Painter, egui::Rect, egui::Stroke),
) -> Response {
    let h = ui.spacing().interact_size.y + 4.0;
    let w = ui.available_width().max(140.0);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
    if ui.is_rect_visible(rect) {
        let v = ui.visuals();
        if selected || resp.hovered() {
            let fill = if selected {
                v.selection.bg_fill
            } else {
                v.widgets.hovered.weak_bg_fill
            };
            ui.painter().rect_filled(rect, 3.0, fill);
        }
        let ink = v.text_color();
        let pic = egui::Rect::from_min_size(
            egui::pos2(rect.left() + 6.0, rect.top() + 3.0),
            Vec2::splat(h - 6.0),
        );
        paint(ui.painter(), pic, egui::Stroke::new(1.5, ink));
        ui.painter().text(
            egui::pos2(pic.right() + 8.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            label,
            egui::TextStyle::Body.resolve(ui.style()),
            ink,
        );
    }
    resp
}

/// A plain heading inside a window, for grouping rows.
pub fn section(ui: &mut Ui, title: &str) {
    if boxes_on(ui) {
        // in Properties, a box of its own with the title as its band (MU22)
        close_box(ui);
        let shape = ui.painter().add(egui::Shape::Noop);
        let w = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 24.0), Sense::hover());
        let ink = ui.visuals().strong_text_color();
        let mut job = egui::text::LayoutJob::single_section(
            title.to_string(),
            egui::TextFormat::simple(egui::FontId::proportional(12.5), ink),
        );
        job.wrap = egui::text::TextWrapping::truncate_at_width((w - 24.0).max(10.0));
        let g = ui.painter().layout_job(job);
        ui.painter().galley(
            egui::pos2(rect.left() + 1.0, rect.center().y - g.size().y / 2.0),
            g,
            ink,
        );
        let b = OpenBox {
            shape,
            left: rect.left(),
            right: rect.right(),
            top: rect.top(),
            header_bottom: rect.bottom(),
            pass: ui.ctx().cumulative_pass_nr(),
        };
        ui.ctx().data_mut(|d| d.insert_temp(box_id(ui), b));
        return;
    }
    ui.add_space(4.0);
    ui.add(egui::Label::new(egui::RichText::new(title).strong()).truncate());
    ui.add_space(1.0);
}

/// An on/off switch in the theme's accent colour, the label at the left
/// and the switch at the right. It answers like a checkbox: `.changed()`
/// when clicked.
pub fn switch(ui: &mut Ui, on: &mut bool, label: impl Into<egui::WidgetText>) -> Response {
    check(ui, "", on, label)
}

/// A small check box and its text, sized to its text: for grids of them
/// (a view's categories), as Blender's (MU22).
pub fn tick(ui: &mut Ui, on: &mut bool, label: &str, width: f32) -> Response {
    let h = 19.0;
    let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(width, h), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let enabled = ui.is_enabled();
    resp.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, enabled, *on, label)
    });
    if ui.is_rect_visible(rect) {
        let v = ui.visuals();
        let accent = v.selection.bg_fill;
        let ink = if enabled {
            v.text_color()
        } else {
            v.weak_text_color()
        };
        let b = egui::Rect::from_center_size(
            egui::pos2(rect.left() + 7.0, rect.center().y),
            Vec2::splat(13.0),
        );
        if *on {
            ui.painter().rect_filled(
                b,
                3.0,
                if enabled {
                    accent
                } else {
                    accent.gamma_multiply(0.5)
                },
            );
            let t = egui::Stroke::new(1.7, Color32::WHITE);
            ui.painter().line_segment(
                [
                    b.left_center() + Vec2::new(3.0, 0.5),
                    b.center() + Vec2::new(-1.0, 3.0),
                ],
                t,
            );
            ui.painter().line_segment(
                [
                    b.center() + Vec2::new(-1.0, 3.0),
                    b.right_top() + Vec2::new(-3.0, 3.5),
                ],
                t,
            );
        } else {
            ui.painter()
                .rect_filled(b, 3.0, v.widgets.inactive.weak_bg_fill);
            ui.painter().rect_stroke(
                b,
                3.0,
                v.widgets.inactive.bg_stroke,
                egui::StrokeKind::Inside,
            );
        }
        if resp.hovered() && enabled {
            ui.painter().rect_stroke(
                b.expand(1.0),
                4.0,
                egui::Stroke::new(1.0, accent),
                egui::StrokeKind::Outside,
            );
        }
        let mut job = egui::text::LayoutJob::single_section(
            label.to_string(),
            egui::TextFormat::simple(egui::TextStyle::Body.resolve(ui.style()), ink),
        );
        job.wrap = egui::text::TextWrapping::truncate_at_width((width - 22.0).max(10.0));
        let g = ui.painter().layout_job(job);
        let elided = g.elided;
        ui.painter().galley(
            egui::pos2(rect.left() + 19.0, rect.center().y - g.size().y / 2.0),
            g,
            ink,
        );
        if elided {
            resp = resp.on_hover_text(label.to_string());
        }
    }
    resp
}

/// A check box with `group` in the label column, as Blender's "Show In":
/// the first of a group carries its name, the rest an empty label.
pub fn check(
    ui: &mut Ui,
    group: &str,
    on: &mut bool,
    label: impl Into<egui::WidgetText>,
) -> Response {
    let label: egui::WidgetText = label.into();
    let text = label.text().to_string();
    let w = ui.available_width() - inset(ui);
    // as Blender's (MU22): a check box in the value column, its text after
    // it; the label column left empty, so on/off settings line up under
    // the fields above them
    let h = (ui.spacing().interact_size.y - 2.0).max(20.0);
    let lw = if w < STACK_BELOW { 0.0 } else { label_width(w) };
    let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let enabled = ui.is_enabled();
    resp.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, enabled, *on, &text)
    });
    if ui.is_rect_visible(rect) {
        let v = ui.visuals();
        let ink = if enabled {
            v.text_color()
        } else {
            v.weak_text_color()
        };
        if !group.is_empty() && lw > 0.0 {
            let muted = v.widgets.noninteractive.fg_stroke.color;
            let mut job = egui::text::LayoutJob::single_section(
                group.to_string(),
                egui::TextFormat::simple(egui::TextStyle::Body.resolve(ui.style()), muted),
            );
            job.wrap = egui::text::TextWrapping::truncate_at_width(lw - 6.0);
            let g = ui.painter().layout_job(job);
            ui.painter().galley(
                rect.left_center() + Vec2::new(lw - 6.0 - g.size().x, -g.size().y / 2.0),
                g,
                muted,
            );
        }
        let accent = v.selection.bg_fill;
        let field = v.widgets.inactive.weak_bg_fill;
        let edge = v.widgets.inactive.bg_stroke;
        // the value column starts one gap after the label, as `row` lays
        // out its fields; without the gap these boxes sat that much left
        // of a tick box inside a row (MU135: Box › Wrap at vs Markdown)
        let gap = if lw > 0.0 {
            ui.spacing().item_spacing.x
        } else {
            0.0
        };
        let x0 = rect.left() + lw + gap;
        let b =
            egui::Rect::from_center_size(egui::pos2(x0 + 8.0, rect.center().y), Vec2::splat(14.0));
        if *on {
            ui.painter().rect_filled(
                b,
                3.0,
                if enabled {
                    accent
                } else {
                    accent.gamma_multiply(0.5)
                },
            );
            let t = egui::Stroke::new(1.8, Color32::WHITE);
            ui.painter().line_segment(
                [
                    b.left_center() + Vec2::new(3.0, 0.5),
                    b.center() + Vec2::new(-1.0, 3.5),
                ],
                t,
            );
            ui.painter().line_segment(
                [
                    b.center() + Vec2::new(-1.0, 3.5),
                    b.right_top() + Vec2::new(-3.0, 3.5),
                ],
                t,
            );
        } else {
            ui.painter().rect_filled(b, 3.0, field);
            ui.painter()
                .rect_stroke(b, 3.0, edge, egui::StrokeKind::Inside);
        }
        if resp.hovered() && enabled {
            ui.painter().rect_stroke(
                b.expand(1.0),
                4.0,
                egui::Stroke::new(1.0, accent),
                egui::StrokeKind::Outside,
            );
        }
        let mut job = egui::text::LayoutJob::single_section(
            text.clone(),
            egui::TextFormat::simple(egui::TextStyle::Body.resolve(ui.style()), ink),
        );
        job.wrap =
            egui::text::TextWrapping::truncate_at_width((rect.right() - x0 - 24.0).max(10.0));
        let g = ui.painter().layout_job(job);
        let elided = g.elided;
        ui.painter().galley(
            egui::pos2(x0 + 22.0, rect.center().y - g.size().y / 2.0),
            g,
            ink,
        );
        if elided {
            resp = resp.on_hover_text(text.clone());
        }
    }
    resp
}

/// The DS check box (14 px, 3 px corners, the accent with a white tick
/// when on), painted in `b`.
pub fn paint_tick_box(ui: &Ui, b: egui::Rect, on: bool, enabled: bool, hovered: bool) {
    let v = ui.visuals();
    let accent = v.selection.bg_fill;
    if on {
        ui.painter().rect_filled(
            b,
            3.0,
            if enabled {
                accent
            } else {
                accent.gamma_multiply(0.5)
            },
        );
        let t = egui::Stroke::new(1.8, Color32::WHITE);
        ui.painter().line_segment(
            [
                b.left_center() + Vec2::new(3.0, 0.5),
                b.center() + Vec2::new(-1.0, 3.5),
            ],
            t,
        );
        ui.painter().line_segment(
            [
                b.center() + Vec2::new(-1.0, 3.5),
                b.right_top() + Vec2::new(-3.0, 3.5),
            ],
            t,
        );
    } else {
        ui.painter()
            .rect_filled(b, 3.0, v.widgets.inactive.weak_bg_fill);
        ui.painter().rect_stroke(
            b,
            3.0,
            v.widgets.inactive.bg_stroke,
            egui::StrokeKind::Inside,
        );
    }
    if hovered && enabled {
        ui.painter().rect_stroke(
            b.expand(1.0),
            4.0,
            egui::Stroke::new(1.0, accent),
            egui::StrokeKind::Outside,
        );
    }
}

/// An inline check box with its text after it, in the layout where it
/// stands (MU77: egui's own checkbox takes the theme's round corners and
/// reads as a radio button). Use it wherever `ui.checkbox` was.
pub fn tickbox(ui: &mut Ui, on: &mut bool, label: impl Into<egui::WidgetText>) -> Response {
    let text = label.into().text().to_string();
    let ink = if ui.is_enabled() {
        ui.visuals().text_color()
    } else {
        ui.visuals().weak_text_color()
    };
    let g =
        ui.painter()
            .layout_no_wrap(text.clone(), egui::TextStyle::Body.resolve(ui.style()), ink);
    let h = (ui.spacing().interact_size.y - 2.0).max(20.0);
    let w = 16.0
        + if text.is_empty() {
            0.0
        } else {
            6.0 + g.size().x
        };
    let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let enabled = ui.is_enabled();
    resp.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, enabled, *on, &text)
    });
    if ui.is_rect_visible(rect) {
        let b = egui::Rect::from_center_size(
            egui::pos2(rect.left() + 8.0, rect.center().y),
            Vec2::splat(14.0),
        );
        paint_tick_box(ui, b, *on, enabled, resp.hovered());
        if !text.is_empty() {
            ui.painter().galley(
                egui::pos2(rect.left() + 22.0, rect.center().y - g.size().y / 2.0),
                g,
                ink,
            );
        }
    }
    resp
}

/// An icon button with the DS frame (MU77: bare icons read as loose
/// glyphs beside a field): 24 px, the field's fill and rim, the icon in
/// the text colour; for a row's own tools (add, load, save).
pub fn tool_button(ui: &mut Ui, icon: &str, tip: &str) -> Response {
    let h = ui.spacing().interact_size.y.max(24.0);
    let (r, resp) = ui.allocate_exact_size(Vec2::splat(h), Sense::click());
    if ui.is_rect_visible(r) {
        let v = ui.visuals();
        let (fill, edge) = if resp.hovered() {
            (v.widgets.hovered.weak_bg_fill, v.widgets.hovered.bg_stroke)
        } else {
            (faint_button(v), faint_edge(v))
        };
        ui.painter()
            .rect(r, 4.0, fill, edge, egui::StrokeKind::Inside);
        crate::svgicon::paint(ui.painter(), r.shrink(5.0), icon, v.text_color());
    }
    resp.on_hover_text(tip)
}

/// A section header of Properties: a full-width row with the title in
/// semibold, a chevron at the right that turns as it opens, and a tint
/// under the pointer. `indent` draws a sub-section's header, set in.
/// Returns the row's response and the rect left for a ? button beside the
/// title.
pub fn header(ui: &mut Ui, title: &str, open: bool, indent: bool) -> (Response, egui::Rect) {
    // its key: the title, less a count it may carry ("Overrides (3)")
    let key = title.split(" (").next().unwrap_or(title).to_string();
    header_keyed(ui, title, open, indent, &key)
}

/// [`header`] with the fold's own `key`, which names the section for as
/// long as the app runs. The fold's motion and measured height are kept
/// by this key, never by the heading's place in the panel: keyed by place,
/// opening one section gave every section under it a neighbour's motion,
/// and they jumped and danced (MU60).
pub fn header_keyed(
    ui: &mut Ui,
    title: &str,
    open: bool,
    indent: bool,
    key: &str,
) -> (Response, egui::Rect) {
    let boxed = !indent && boxes_on(ui);
    let out = header_band(ui, title, open, indent);
    // the body under it folds by [`fold`], which finds this heading here
    let id = egui::Id::new(("props.fold", key));
    ui.ctx().data_mut(|d| d.insert_temp(last_fold_id(), id));
    // a folded section's box ends under its band, so whatever comes next
    // never hangs under it as a lip (MU40), once it has finished shutting
    if boxed && !open && fold_amount(ui.ctx(), id, open) <= 0.0 {
        close_box(ui);
    }
    out
}

/// How far the fold named `key` (as [`set_fold_key`] names it) is open,
/// 0 to 1, for a chevron that turns with it.
pub fn fold_openness(
    ctx: &egui::Context,
    key: impl std::hash::Hash + std::fmt::Debug,
    open: bool,
) -> f32 {
    fold_amount(ctx, egui::Id::new(("props.fold", key)), open)
}

/// Name the fold that [`fold`] draws next, for a heading drawn outside
/// this module (a window's list group).
pub fn set_fold_key(ui: &Ui, key: impl std::hash::Hash + std::fmt::Debug) {
    let id = egui::Id::new(("props.fold", key));
    ui.ctx().data_mut(|d| d.insert_temp(last_fold_id(), id));
}

fn last_fold_id() -> egui::Id {
    egui::Id::new("props.fold.last")
}

/// How far a fold is open, 0 to 1, eased (AMD-017: about 150 ms, fast at
/// first and slow at the end).
fn fold_amount(ctx: &egui::Context, id: egui::Id, open: bool) -> f32 {
    let t = ctx.animate_bool_with_time(id.with("fold"), open, 0.2);
    // scrolled to its foot, the panel cannot keep still while a fold
    // slides: it clamps a frame late and shook (owner, cut test). There
    // the fold opens or shuts at once.
    if at_foot(ctx) {
        return if open { 1.0 } else { 0.0 };
    }
    1.0 - (1.0 - t).powi(3)
}

fn scroll_id() -> egui::Id {
    egui::Id::new("props.scroll.foot")
}

/// What Properties' scroll did this frame: how far down it is, and how
/// far it can go. Its folds read it next frame.
pub fn note_scroll(ctx: &egui::Context, offset: f32, most: f32) {
    let foot = most > 0.5 && offset >= most - 2.0;
    ctx.data_mut(|d| d.insert_temp(scroll_id(), foot));
}

/// Is Properties scrolled as far down as it goes?
fn at_foot(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(scroll_id()))
        .unwrap_or(false)
}

/// The body of the section whose heading was drawn last (AMD-017,
/// animated folding, owner 2026-11-15): drawn in full while it is open,
/// not at all once shut, and while it opens or shuts, clipped to a height
/// that grows or shrinks with it, so what is under it moves smoothly
/// instead of jumping. Returns what `add` returned, when it was drawn.
pub fn fold<R>(ui: &mut Ui, open: bool, add: impl FnOnce(&mut Ui) -> R) -> Option<R> {
    let id = ui
        .ctx()
        .data(|d| d.get_temp::<egui::Id>(last_fold_id()))
        .unwrap_or_else(|| ui.id());
    let t = fold_amount(ui.ctx(), id, open);
    if t <= 0.0 {
        return None;
    }
    if t >= 1.0 {
        return Some(add(ui));
    }
    // moving: the body at its full height last time, shown as far as t
    let full_key = id.with("fold.height");
    let full: f32 = ui.ctx().data(|d| d.get_temp(full_key)).unwrap_or(0.0);
    // drawn in a child area, clipped at the eased height; the panel then
    // takes only that height, so what is under it slides with it
    let area = ui.available_rect_before_wrap();
    let top = area.top();
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(id.with("fold.body"))
            .max_rect(area)
            .layout(*ui.layout()),
    );
    // a fold never measured yet is laid out unseen on its first frame and
    // takes no room, then grows from nothing: showing it whole for that one
    // frame made what was under it jump down and back (MU60)
    let shown = full * t;
    let mut clip = child.clip_rect();
    clip.max.y = clip.max.y.min(top + shown);
    child.set_clip_rect(clip);
    let r = add(&mut child);
    let height = (child.min_rect().bottom() - top).max(0.0);
    ui.ctx().data_mut(|d| d.insert_temp(full_key, height));
    let take = shown;
    ui.allocate_space(Vec2::new(area.width(), take));
    ui.ctx().request_repaint();
    Some(r)
}

fn header_band(ui: &mut Ui, title: &str, open: bool, indent: bool) -> (Response, egui::Rect) {
    // as Blender's panels (MU20): a band a step darker than the panel, a
    // thin open chevron at the left that turns down when the section is
    // open, the title in bold after it, a dotted grip at the right
    let boxed = !indent && boxes_on(ui);
    if boxed {
        close_box(ui);
    }
    let w = ui.available_width() - if indent { inset(ui) } else { 0.0 };
    let h = if indent { 22.0 } else { 24.0 };
    let shape = boxed.then(|| ui.painter().add(egui::Shape::Noop));
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
    if let Some(shape) = shape {
        let b = OpenBox {
            shape,
            left: rect.left(),
            right: rect.right(),
            top: rect.top(),
            header_bottom: rect.bottom(),
            pass: ui.ctx().cumulative_pass_nr(),
        };
        ui.ctx().data_mut(|d| d.insert_temp(box_id(ui), b));
    }
    if ui.is_rect_visible(rect) {
        let v = ui.visuals();
        let band = {
            let p = v.panel_fill;
            let (to, t) = if v.dark_mode {
                (255.0, 0.05)
            } else {
                (0.0, 0.045)
            };
            let m = |c: u8| (c as f32 + (to - c as f32) * t).round() as u8;
            Color32::from_rgb(m(p.r()), m(p.g()), m(p.b()))
        };
        // in a box, the box itself is the band (MU22); hovered, lighter
        if !boxed && !indent {
            ui.painter().rect_filled(rect, 4.0, band);
        }
        if resp.hovered() {
            let band = if boxed {
                rect.expand2(Vec2::new(PAD, 0.0))
            } else {
                rect
            };
            ui.painter().rect_filled(
                band,
                4.0,
                v.widgets.hovered.weak_bg_fill.gamma_multiply(0.5),
            );
        }
        let ink = v.strong_text_color();
        let soft = v.weak_text_color();
        // the chevron: › when shut, ˅ when open, turning between
        let t = ui.ctx().animate_bool_responsive(resp.id, open);
        if indent {
            // a sub-section, as Blender's: flush with its section, its
            // header on a band a shade deeper than the box (MU22)
            let deeper = if v.dark_mode {
                Color32::from_black_alpha(40)
            } else {
                Color32::from_black_alpha(14)
            };
            ui.painter().rect_filled(rect, 3.0, deeper);
        }
        let c = egui::pos2(rect.left() + 4.0, rect.center().y);
        let ang = t * std::f32::consts::FRAC_PI_2;
        let rot = |p: Vec2| {
            let (s, co) = ang.sin_cos();
            c + Vec2::new(p.x * co - p.y * s, p.x * s + p.y * co)
        };
        let pts = [
            rot(Vec2::new(-1.8, -3.6)),
            rot(Vec2::new(1.8, 0.0)),
            rot(Vec2::new(-1.8, 3.6)),
        ];
        let stroke = egui::Stroke::new(1.3, ink);
        ui.painter().line_segment([pts[0], pts[1]], stroke);
        ui.painter().line_segment([pts[1], pts[2]], stroke);
        let left = c.x + 9.0;
        let font = egui::FontId::proportional(if indent { 12.0 } else { 12.5 });
        // one line, cut with … (MU22): a band never grows taller
        let mut job = egui::text::LayoutJob::single_section(
            title.to_string(),
            egui::TextFormat::simple(font, ink),
        );
        job.wrap = egui::text::TextWrapping::truncate_at_width((w - 70.0).max(10.0));
        let g = ui.painter().layout_job(job);
        let tw = g.size().x;
        ui.painter()
            .galley(egui::pos2(left, rect.center().y - g.size().y / 2.0), g, ink);
        // a dotted grip at the right, as Blender's
        if !indent {
            for row in 0..2 {
                for col in 0..3 {
                    ui.painter().circle_filled(
                        egui::pos2(
                            rect.right() - 18.0 + col as f32 * 3.5,
                            rect.center().y - 1.8 + row as f32 * 3.6,
                        ),
                        0.8,
                        soft.gamma_multiply(0.7),
                    );
                }
            }
        }
        let help = egui::Rect::from_min_size(
            egui::pos2(left + tw + 6.0, rect.center().y - 8.0),
            Vec2::splat(16.0),
        );
        return (resp, help);
    }
    (resp, egui::Rect::NOTHING)
}

/// A labelled drop-down that draws as a Properties row: the label at the
/// left, the box filling the rest. It is made like an egui `ComboBox`.
pub struct Combo {
    label: String,
    inner: egui::ComboBox,
    width: Option<f32>,
}

impl Combo {
    pub fn new(label: &str) -> Self {
        Self {
            label: label.to_string(),
            inner: egui::ComboBox::from_id_salt(("props.combo", label.to_string()))
                .icon(crate::props::combo_icon),
            width: None,
        }
    }

    /// A second list with the same label in one panel: `salt` tells them
    /// apart.
    pub fn salted(label: &str, salt: &str) -> Self {
        Self {
            label: label.to_string(),
            inner: egui::ComboBox::from_id_salt((
                "props.combo",
                label.to_string(),
                salt.to_string(),
            ))
            .icon(crate::props::combo_icon),
            width: None,
        }
    }

    pub fn selected_text(mut self, t: impl Into<egui::WidgetText>) -> Self {
        self.inner = self.inner.selected_text(t);
        self
    }

    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }

    /// The open list's height at most, for a long list (the fonts).
    pub fn height(mut self, h: f32) -> Self {
        self.inner = self.inner.height(h);
        self
    }

    pub fn show_ui<R>(
        self,
        ui: &mut Ui,
        add: impl FnOnce(&mut Ui) -> R,
    ) -> egui::InnerResponse<Option<R>> {
        let Combo {
            label,
            inner,
            width,
        } = self;
        // the field column's whole width, as every field in Properties
        // (MU42: drop-downs asked for less and ended short of the rest), so
        // a width asked for is not used
        let _ = width;
        row(ui, &label, |ui| {
            inner.width(ui.available_width()).show_ui(ui, add)
        })
    }
}

#[cfg(test)]
mod tests {
    /// The painted 14 px check boxes, by their left edge.
    fn box_lefts(shapes: &[egui::epaint::ClippedShape]) -> Vec<f32> {
        fn walk(s: &egui::Shape, out: &mut Vec<f32>) {
            match s {
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                egui::Shape::Rect(r)
                    if (r.rect.width() - 14.0).abs() < 0.01
                        && (r.rect.height() - 14.0).abs() < 0.01 =>
                {
                    out.push(r.rect.left())
                }
                _ => {}
            }
        }
        let mut out = vec![];
        for c in shapes {
            walk(&c.shape, &mut out);
        }
        out
    }

    /// A tick box in a row (Box › Wrap at) and an on/off row below it
    /// (Markdown, Leader) put their boxes at one left edge, under the
    /// fields (MU135: Wrap at's box sat one gap to the right).
    #[test]
    fn tick_boxes_in_and_under_rows_line_up() {
        let ctx = egui::Context::default();
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(320.0, 300.0),
            )),
            ..Default::default()
        };
        let mut out = ctx.run_ui(raw, |ui| {
            let (mut a, mut b) = (false, false);
            super::row(ui, "Box", |ui| {
                super::flow(ui, |ui| super::tickbox(ui, &mut a, "Wrap at"))
            });
            super::switch(ui, &mut b, "Markdown");
        });
        // the font atlas this frame made goes to no GPU here
        out.textures_delta.clear();
        // each box is painted as its fill and its rim
        let lefts = box_lefts(&out.shapes);
        assert!(lefts.len() >= 2, "both boxes painted: {lefts:?}");
        let lo = lefts.iter().cloned().fold(f32::INFINITY, f32::min);
        let hi = lefts.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        assert!(
            hi - lo < 0.5,
            "boxes at {lefts:?} should share one left edge"
        );
    }

    /// Every one-line field goes through `text_field`, which centres its
    /// text top to bottom; added through `fill` the text sat at the top of
    /// the field (MU107: the declared ratings).
    #[test]
    fn text_fields_are_never_added_through_fill() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        for f in std::fs::read_dir(&dir).unwrap() {
            let f = f.unwrap().path();
            if f.extension().is_none_or(|e| e != "rs") {
                continue;
            }
            let text = std::fs::read_to_string(&f).unwrap();
            let bad = concat!("props::fill(ui, egui::", "TextEdit");
            assert!(
                !text.contains(bad),
                "{}: a TextEdit added through fill; use props::text_field",
                f.display()
            );
        }
    }

    /// Every drop-down draws the house chevron. One made straight from
    /// egui's `ComboBox` drew its filled triangle beside the others'
    /// chevrons (MU124: Ground and Set in View > Site).
    #[test]
    fn every_drop_down_draws_the_house_chevron() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let made = concat!("ComboBox::", "from_");
        for f in std::fs::read_dir(&dir).unwrap() {
            let f = f.unwrap().path();
            if f.extension().is_none_or(|e| e != "rs") {
                continue;
            }
            let text = std::fs::read_to_string(&f).unwrap();
            let lines: Vec<&str> = text.lines().collect();
            for (i, l) in lines.iter().enumerate() {
                if !l.contains(made) {
                    continue;
                }
                let next = lines[i..(i + 6).min(lines.len())].join("\n");
                assert!(
                    next.contains(".icon("),
                    "{}:{}: a drop-down without .icon(crate::props::combo_icon)",
                    f.display(),
                    i + 1
                );
            }
        }
    }

    use super::*;

    #[test]
    fn the_label_column_is_two_fifths_of_the_row_and_narrow_rows_stack() {
        // about 38 %, never under 44 px, no wider than 160 (MU36: a narrow
        // panel keeps one value a line, the field stretching)
        assert!((label_width(300.0) - 114.0).abs() < 1e-3);
        assert_eq!(label_width(1000.0), 160.0);
        assert_eq!(label_width(100.0), 44.0);
        // only below 110 px does the label go above its control
        assert_eq!(STACK_BELOW, 110.0);
    }
}

/// One of several, drawn as the DS check box (MU77: no round radio
/// buttons): ticked when `value` is `this`; a click picks it.
pub fn radio_tick<T: PartialEq>(
    ui: &mut Ui,
    value: &mut T,
    this: T,
    label: impl Into<egui::WidgetText>,
) -> Response {
    let was = *value == this;
    let mut on = was;
    let mut r = tickbox(ui, &mut on, label);
    if was {
        // a ticked one stays ticked: it is one of several
        r.flags.remove(egui::response::Flags::CHANGED);
    } else if r.clicked() {
        *value = this;
    }
    r
}

/// What a click at an [`item_list`]'s side asked for.
#[derive(Default, Clone, Copy, Debug)]
pub struct ListAct {
    pub add: bool,
    pub duplicate: bool,
    pub remove: bool,
    /// A row was double-clicked: the picked one, to open or select.
    pub opened: bool,
}

/// A list of things with their own fields, as Blender's (Properties order
/// REV04, lists option A): a short framed list, each row its name and a
/// quiet value at the right, the picked one lit; +, duplicate and the bin
/// at its side. The picked item's fields go under or beside it. `rows`
/// are (name, value); `None` for a heading row that can't be picked.
pub fn item_list(
    ui: &mut Ui,
    key: &str,
    rows: &[(Option<String>, String)],
    pick: &mut usize,
    visible: usize,
    tools: (bool, bool, bool),
) -> ListAct {
    item_list_drawn(ui, key, rows, pick, visible, tools, None)
}

/// A row's picture in [`item_list_drawn`]: the painter, the picture's
/// rectangle, the row's index and the ink to draw in.
pub type RowPicture<'a> = &'a dyn Fn(&egui::Painter, egui::Rect, usize, Color32);

/// [`item_list`] with a picture at the start of each row that says which
/// item it is (AMD-018: an outline with that edge heavy, a soil layer's
/// hatch, a chamber mark), the row's main value at its right.
pub fn item_list_drawn(
    ui: &mut Ui,
    key: &str,
    rows: &[(Option<String>, String)],
    pick: &mut usize,
    visible: usize,
    tools: (bool, bool, bool),
    picture: Option<RowPicture<'_>>,
) -> ListAct {
    let mut act = ListAct::default();
    ui.push_id(("item_list", key), |ui| {
        ui.horizontal_top(|ui| {
            let side = if tools.0 || tools.1 || tools.2 {
                28.0
            } else {
                0.0
            };
            let w = (ui.available_width() - side).max(80.0);
            let row_h = 22.0;
            let h = row_h * visible.max(3) as f32 + 6.0;
            let (frame, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
            let v = ui.visuals().clone();
            ui.painter().rect(
                frame,
                4.0,
                v.extreme_bg_color,
                v.widgets.inactive.bg_stroke,
                egui::StrokeKind::Inside,
            );
            let mut inner = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(frame.shrink(3.0))
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            egui::ScrollArea::vertical()
                .id_salt("rows")
                .max_height(h - 6.0)
                .auto_shrink([false, false])
                .show(&mut inner, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for (i, (value, name)) in rows.iter().enumerate() {
                        let rw = ui.available_width();
                        let Some(value) = value else {
                            // a group's heading
                            // set in the strong ink, drawn twice half a pixel
                            // apart for weight, with a hairline to the edge:
                            // a heading over its rows, not one of them (MU80)
                            let (r, _) =
                                ui.allocate_exact_size(Vec2::new(rw, row_h), Sense::hover());
                            let at = r.left_center() + Vec2::new(6.0, 1.0);
                            let ink = v.strong_text_color();
                            let font = egui::FontId::proportional(10.5);
                            let up = name.to_uppercase();
                            let tr = ui.painter().text(
                                at,
                                egui::Align2::LEFT_CENTER,
                                &up,
                                font.clone(),
                                ink,
                            );
                            ui.painter().text(
                                at + Vec2::new(0.5, 0.0),
                                egui::Align2::LEFT_CENTER,
                                &up,
                                font,
                                ink,
                            );
                            if tr.right() + 14.0 < r.right() - 6.0 {
                                ui.painter().hline(
                                    (tr.right() + 8.0)..=(r.right() - 6.0),
                                    r.center().y + 1.0,
                                    egui::Stroke::new(
                                        1.0,
                                        v.widgets.noninteractive.bg_stroke.color,
                                    ),
                                );
                            }
                            continue;
                        };
                        let (r, resp) =
                            ui.allocate_exact_size(Vec2::new(rw, row_h), Sense::click());
                        let on = *pick == i;
                        // the chosen row in the accent, all of it on-accent
                        // (AMD-014); hover a raise, as before
                        if on {
                            ui.painter().rect_filled(r, 3.0, v.selection.bg_fill);
                        } else if resp.hovered() {
                            ui.painter()
                                .rect_filled(r, 3.0, v.widgets.hovered.weak_bg_fill);
                        }
                        // white or near-black, whichever reads on the accent,
                        // as the segmented choice does (MU78)
                        let on_accent = crate::theme::ink_on(crate::theme::over(
                            v.selection.bg_fill,
                            v.panel_fill,
                        ));
                        let ink = if on { on_accent } else { v.text_color() };
                        let soft = if on { on_accent } else { v.weak_text_color() };
                        let vg = ui.painter().layout_no_wrap(
                            value.clone(),
                            egui::TextStyle::Small.resolve(ui.style()),
                            soft,
                        );
                        let mut job = egui::text::LayoutJob::single_section(
                            name.clone(),
                            egui::TextFormat::simple(
                                egui::TextStyle::Body.resolve(ui.style()),
                                ink,
                            ),
                        );
                        job.wrap = egui::text::TextWrapping::truncate_at_width(
                            (rw - vg.size().x - if picture.is_some() { 42.0 } else { 22.0 })
                                .max(30.0),
                        );
                        // its picture first, then its name after it
                        let lead = if let Some(paint) = picture {
                            let pr = egui::Rect::from_center_size(
                                egui::pos2(r.left() + 15.0, r.center().y),
                                Vec2::new(18.0, 13.0),
                            );
                            paint(ui.painter(), pr, i, ink);
                            28.0
                        } else {
                            8.0
                        };
                        let g = ui.painter().layout_job(job);
                        let elided = g.elided;
                        ui.painter().galley(
                            egui::pos2(r.left() + lead, r.center().y - g.size().y / 2.0),
                            g,
                            ink,
                        );
                        ui.painter().galley(
                            egui::pos2(
                                r.right() - 6.0 - vg.size().x,
                                r.center().y - vg.size().y / 2.0,
                            ),
                            vg,
                            soft,
                        );
                        let resp = if elided {
                            resp.on_hover_text(name)
                        } else {
                            resp
                        };
                        if resp.clicked() {
                            *pick = i;
                        }
                        if resp.double_clicked() {
                            *pick = i;
                            act.opened = true;
                        }
                    }
                });
            if side > 0.0 {
                // AMD-014: a 24 px column of 24 x 22 buttons on raise,
                // only those that apply
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    if tools.0 && list_button(ui, "plus", "Add one").clicked() {
                        act.add = true;
                    }
                    if tools.2 && list_button(ui, "minus", "Remove the picked one").clicked() {
                        act.remove = true;
                    }
                    if tools.1 && list_button(ui, "copy", "Duplicate the picked one").clicked() {
                        act.duplicate = true;
                    }
                });
            }
        });
    });
    act
}
