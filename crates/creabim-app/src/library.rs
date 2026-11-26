//! The library windows' one frame (MU42): Levels, Types, Assemblies,
//! Materials and the Catalogue are laid out alike. The list on the left in
//! accordions (bold headers with a count, as Properties' sections), the
//! actions in one bar under the list (New, Duplicate, Delete, then what
//! brings things in, on the right), the editor on the right, with what
//! uses the item and Assign at its foot.

use eframe::egui::{self, Color32, Response, RichText, Sense, Ui, Vec2};

/// How wide the list is.
pub const LIST_WIDTH: f32 = 290.0;

/// Lay a library window out: `list` scrolls in the left column over the
/// action bar `bar`; `editor` takes the rest. Each is handed `state`
/// in turn (what they ask the window to do).
pub fn frame<S>(
    ui: &mut Ui,
    salt: &str,
    list_width: f32,
    state: &mut S,
    list: impl FnOnce(&mut Ui, &mut S),
    bar: impl FnOnce(&mut Ui, &mut S),
    editor: impl FnOnce(&mut Ui, &mut S),
) {
    fit(ui, |ui| {
        frame_in(ui, salt, list_width, state, list, bar, editor)
    });
}

/// Draw `add` inside the room the window has now, clipped to it, and take
/// exactly that room. A window's body sized from the room it had grew the
/// window when it drew a pixel more, which gave it more room the next
/// frame: it crept larger on its own, and the preview of Patterns with it
/// (MU42, MU62). Here only the person resizing the window changes it.
pub fn fit<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let rect = ui.available_rect_before_wrap();
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    child.set_clip_rect(rect.intersect(ui.clip_rect()));
    let r = add(&mut child);
    ui.allocate_rect(rect, Sense::hover());
    r
}

fn frame_in<S>(
    ui: &mut Ui,
    salt: &str,
    list_width: f32,
    state: &mut S,
    list: impl FnOnce(&mut Ui, &mut S),
    bar: impl FnOnce(&mut Ui, &mut S),
    editor: impl FnOnce(&mut Ui, &mut S),
) {
    // inside [`fit`]: the room is fixed for this frame
    let height = (ui.available_height() - 6.0).max(260.0);
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(list_width);
            ui.set_max_height(height);
            let bar_h = ui.spacing().interact_size.y + 16.0;
            egui::ScrollArea::vertical()
                .id_salt((salt, "list"))
                .max_height(height - bar_h)
                .auto_shrink([false, false])
                .show(ui, |ui| list(ui, &mut *state));
            action_bar(ui, |ui| bar(ui, &mut *state));
        });
        ui.separator();
        ui.vertical(|ui| {
            ui.set_max_height(height);
            egui::ScrollArea::vertical()
                .id_salt((salt, "editor"))
                .max_height(height)
                .auto_shrink([false, false])
                .show(ui, |ui| editor(ui, &mut *state));
        });
    });
}

/// The bar under the list, for a window that lays its list out itself
/// (Patterns): New, Duplicate, Delete, then what brings things in.
pub fn bar(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    action_bar(ui, add)
}

/// The bar under the list: a band a step darker, its buttons in a row.
fn action_bar(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    let band = band_colour(ui);
    egui::Frame::new()
        .fill(band)
        .corner_radius(6.0)
        .inner_margin(egui::Margin::symmetric(6, 5))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 5.0;
                add(ui)
            });
        });
}

fn band_colour(ui: &Ui) -> Color32 {
    let v = ui.visuals();
    let p = v.panel_fill;
    let (to, t) = if v.dark_mode {
        (255.0, 0.05)
    } else {
        (0.0, 0.045)
    };
    let m = |c: u8| (c as f32 + (to - c as f32) * t).round() as u8;
    Color32::from_rgb(m(p.r()), m(p.g()), m(p.b()))
}

/// "+ New": the action most wanted, in the accent colour.
pub fn new_button(ui: &mut Ui, tip: &str) -> Response {
    let accent = crate::theme::Palette::of(ui.ctx()).accent;
    ui.add(
        egui::Button::new(RichText::new("+ New").color(crate::theme::ink_on(accent)))
            .fill(accent)
            .stroke(egui::Stroke::new(1.0, accent)),
    )
    .on_hover_text(tip)
}

pub fn duplicate_button(ui: &mut Ui, enabled: bool) -> Response {
    ui.add_enabled(enabled, egui::Button::new("Duplicate"))
}

/// Delete: the red bin, as in Properties' header (MU42).
pub fn delete_button(ui: &mut Ui, enabled: bool, tip: &str) -> Response {
    let red = Color32::from_rgb(190, 50, 40);
    let h = ui.spacing().interact_size.y;
    // disabled, it can't be clicked at all
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (r, resp) = ui.allocate_exact_size(Vec2::splat(h), sense);
    if ui.is_rect_visible(r) {
        let v = ui.visuals();
        let w = if resp.hovered() && enabled {
            &v.widgets.hovered
        } else {
            &v.widgets.inactive
        };
        ui.painter().rect(
            r,
            4.0,
            w.weak_bg_fill,
            w.bg_stroke,
            egui::StrokeKind::Inside,
        );
        let ink = if enabled { red } else { v.weak_text_color() };
        crate::svgicon::paint(ui.painter(), r.shrink(4.0), "trash-2", ink);
    }
    resp.on_hover_text(format!("Delete. {tip}"))
}

/// What brings things in (Starters, Import), on the right of the bar.
pub fn right<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), add)
        .inner
}

/// An accordion header in the list: a band, a chevron, the title in bold,
/// a count at the right. Folded state kept per window and group. Returns
/// whether it is open.
pub fn group(ui: &mut Ui, salt: &str, title: &str, count: usize) -> bool {
    let id = egui::Id::new(("library.group", salt, title));
    // the rows under it fold smoothly, as Properties' do (AMD-017)
    crate::props::set_fold_key(ui, ("library.group", salt, title));
    let open = ui.ctx().data(|d| d.get_temp::<bool>(id)).unwrap_or(true);
    ui.add_space(3.0);
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 24.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let v = ui.visuals();
        let mut band = band_colour(ui);
        if resp.hovered() {
            band = v.widgets.hovered.weak_bg_fill;
        }
        let p = ui.painter();
        p.rect_filled(rect, 5.0, band);
        let text = v.text_color();
        // the chevron, as Properties' sections
        let c = rect.left_center() + Vec2::new(11.0, 0.0);
        // turning with the rows as they fold (AMD-017)
        let t = crate::props::fold_openness(ui.ctx(), ("library.group", salt, title), open);
        let a = t * std::f32::consts::FRAC_PI_2;
        let rot =
            |x: f32, y: f32| c + Vec2::new(x * a.cos() - y * a.sin(), x * a.sin() + y * a.cos());
        let pts = vec![rot(-2.0, -4.0), rot(2.0, 0.0), rot(-2.0, 4.0)];
        p.add(egui::Shape::line(pts, egui::Stroke::new(1.5, text)));
        p.text(
            rect.left_center() + Vec2::new(22.0, 0.0),
            egui::Align2::LEFT_CENTER,
            title,
            egui::FontId::proportional(13.0),
            text,
        );
        // the bold weight, by a second pass a hair to the right
        p.text(
            rect.left_center() + Vec2::new(22.4, 0.0),
            egui::Align2::LEFT_CENTER,
            title,
            egui::FontId::proportional(13.0),
            text,
        );
        let n = count.to_string();
        let g = p.layout_no_wrap(
            n.clone(),
            egui::FontId::proportional(11.0),
            v.weak_text_color(),
        );
        let pill = egui::Rect::from_center_size(
            rect.right_center() - Vec2::new(8.0 + g.size().x / 2.0 + 5.0, 0.0),
            Vec2::new(g.size().x + 10.0, 16.0),
        );
        p.rect_filled(pill, 8.0, v.extreme_bg_color);
        p.galley(pill.center() - g.size() / 2.0, g, v.weak_text_color());
    }
    if resp.clicked() {
        ui.ctx().data_mut(|d| d.insert_temp(id, !open));
        return !open;
    }
    open
}

/// What draws a row's mark at its left: a swatch, a section.
pub type Mark = dyn Fn(&egui::Painter, egui::Rect);

/// One row of the list: `mark` drawn at its left (a swatch, a section),
/// the name, and `note` at the right in the muted colour.
pub fn item(ui: &mut Ui, selected: bool, name: &str, note: &str, mark: Option<&Mark>) -> Response {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 24.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let v = ui.visuals();
        let p = ui.painter();
        if selected {
            p.rect(
                rect,
                5.0,
                v.selection.bg_fill,
                v.selection.stroke,
                egui::StrokeKind::Inside,
            );
        } else if resp.hovered() {
            p.rect_filled(rect, 5.0, v.widgets.hovered.weak_bg_fill);
        }
        let mut x = rect.left() + 14.0;
        if let Some(mark) = mark {
            let r = egui::Rect::from_min_size(
                egui::pos2(x, rect.center().y - 7.0),
                Vec2::new(24.0, 14.0),
            );
            mark(p, r);
            x += 32.0;
        }
        let note_w = if note.is_empty() {
            0.0
        } else {
            let g = p.layout_no_wrap(
                note.to_string(),
                egui::FontId::proportional(11.5),
                v.weak_text_color(),
            );
            let w = g.size().x;
            p.galley(
                egui::pos2(rect.right() - 8.0 - w, rect.center().y - g.size().y / 2.0),
                g,
                v.weak_text_color(),
            );
            w + 12.0
        };
        let mut job = egui::text::LayoutJob::single_section(
            name.to_string(),
            egui::TextFormat::simple(egui::FontId::proportional(13.0), v.text_color()),
        );
        job.wrap = egui::text::TextWrapping::truncate_at_width(
            (rect.right() - 8.0 - note_w - x).max(20.0),
        );
        let g = p.layout_job(job);
        p.galley(
            egui::pos2(x, rect.center().y - g.size().y / 2.0),
            g,
            v.text_color(),
        );
    }
    resp
}

/// A colour swatch for [`item`].
pub fn swatch(rgb: [f32; 3]) -> impl Fn(&egui::Painter, egui::Rect) {
    move |p, r| {
        let c = Color32::from_rgb(
            (rgb[0] * 255.0) as u8,
            (rgb[1] * 255.0) as u8,
            (rgb[2] * 255.0) as u8,
        );
        let r = egui::Rect::from_center_size(r.center(), Vec2::splat(12.0));
        p.rect(
            r,
            3.0,
            c,
            egui::Stroke::new(1.0, Color32::from_black_alpha(50)),
            egui::StrokeKind::Inside,
        );
    }
}

/// A section heading in the editor: small capitals, muted, a line over it.
pub fn heading(ui: &mut Ui, title: &str) {
    ui.add_space(6.0);
    ui.separator();
    ui.label(
        RichText::new(title.to_uppercase())
            .size(10.5)
            .color(ui.visuals().weak_text_color()),
    );
    ui.add_space(2.0);
}

/// The editor's foot: what uses the item, then its buttons on the right.
pub fn foot(ui: &mut Ui, used: &str, buttons: impl FnOnce(&mut Ui)) {
    ui.add_space(8.0);
    ui.separator();
    ui.horizontal(|ui| {
        ui.label(RichText::new(used).color(ui.visuals().weak_text_color()));
        right(ui, buttons);
    });
}

/// The main button of a foot (Assign to the selection), in the accent.
pub fn primary(ui: &mut Ui, enabled: bool, text: &str) -> Response {
    let accent = crate::theme::Palette::of(ui.ctx()).accent;
    if enabled {
        ui.add(
            egui::Button::new(RichText::new(text).color(crate::theme::ink_on(accent)))
                .fill(accent)
                .stroke(egui::Stroke::new(1.0, accent)),
        )
    } else {
        ui.add_enabled(false, egui::Button::new(text))
    }
}
