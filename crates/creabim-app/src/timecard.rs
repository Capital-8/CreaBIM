//! The unfolding card of Project time (Mockup_Project_Time_REV04, owner
//! 2026-11-18, from FocusFlow's smart card and FocusCraftJob's unfold):
//! a rounded card, its first line with a chevron, a short summary under
//! it, and the rest unfolding smoothly (the house fold, AMD-017). The one
//! it is about now is lit softly in the theme's own accent. No coloured
//! band down its side: a stage's colour is a small swatch, as in legends.

use crate::theme::Palette;
use eframe::egui::{self, epaint, Color32, Pos2, Rect, Sense, Stroke, Ui, Vec2};

/// Mix `a` toward `b` by `t` (0..1), as sRGB.
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

/// A rounded rectangle filled left to right from `left` to `right`, the
/// colour reaching `right` at `reach` of the width: a mesh fanned from its
/// centre, so the gradient keeps the corners round.
fn gradient_rect(r: Rect, radius: f32, left: Color32, right: Color32, reach: f32) -> egui::Shape {
    let col = |x: f32| {
        let t = ((x - r.left()) / (r.width() * reach).max(1.0)).clamp(0.0, 1.0);
        mix(left, right, t)
    };
    let mut pts: Vec<Pos2> = Vec::new();
    let rad = radius.min(r.width() / 2.0).min(r.height() / 2.0);
    let corners = [
        (Pos2::new(r.right() - rad, r.top() + rad), -90.0f32),
        (Pos2::new(r.right() - rad, r.bottom() - rad), 0.0),
        (Pos2::new(r.left() + rad, r.bottom() - rad), 90.0),
        (Pos2::new(r.left() + rad, r.top() + rad), 180.0),
    ];
    for (c, a0) in corners {
        for k in 0..=6 {
            let a = (a0 + 15.0 * k as f32).to_radians();
            pts.push(c + Vec2::new(a.cos(), a.sin()) * rad);
        }
    }
    let mut mesh = epaint::Mesh::default();
    let centre = r.center();
    mesh.colored_vertex(centre, col(centre.x));
    for q in &pts {
        mesh.colored_vertex(*q, col(q.x));
    }
    let n = pts.len() as u32;
    for i in 0..n {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
    }
    egui::Shape::mesh(mesh)
}

/// A stage's swatch: a small rounded square in its colour.
pub fn swatch(ui: &mut Ui, c: Color32) {
    let (r, _) = ui.allocate_exact_size(Vec2::splat(9.0), Sense::hover());
    ui.painter().rect_filled(r, 3.0, c);
}

/// A soft rounded bar, `used` of 1.
pub fn bar(ui: &mut Ui, width: f32, used: Option<f64>, fill: Color32) {
    let p = Palette::of(ui.ctx());
    let (r, _) = ui.allocate_exact_size(Vec2::new(width, 6.0), Sense::hover());
    ui.painter().rect_filled(r, 3.0, p.raise2);
    if let Some(u) = used {
        let w = (u.clamp(0.0, 1.0) as f32 * r.width()).max(if u > 0.0 { 6.0 } else { 0.0 });
        ui.painter().rect_filled(
            Rect::from_min_size(r.min, Vec2::new(w, r.height())),
            3.0,
            fill,
        );
    }
}

/// A line of text cut with … to `width`, its whole on hover when cut.
pub fn cut_label(ui: &mut Ui, text: egui::RichText, width: f32) -> egui::Response {
    // laid out and painted here, left-aligned, one line, cut with …
    // (screen check: add_sized centred it)
    let full = text.text().to_string();
    let width = width.max(10.0);
    let mut job = (*egui::WidgetText::from(text).into_layout_job(
        ui.style(),
        egui::FontSelection::Default,
        egui::Align::Min,
    ))
    .clone();
    job.wrap = egui::text::TextWrapping {
        max_width: width,
        max_rows: 1,
        break_anywhere: true,
        overflow_character: Some('…'),
    };
    let galley = ui.painter().layout_job(job);
    let cut = galley.elided;
    // a short text takes only its own width, so what follows sits by it
    let w = if cut {
        width
    } else {
        galley.size().x.min(width)
    };
    let (r, resp) = ui.allocate_exact_size(Vec2::new(w, galley.size().y.max(16.0)), Sense::hover());
    let colour = ui.visuals().text_color();
    ui.painter().galley(
        egui::pos2(r.left(), r.center().y - galley.size().y / 2.0),
        galley,
        colour,
    );
    if cut {
        resp.on_hover_text(full)
    } else {
        resp
    }
}

/// A section heading inside a card: an icon and small capitals in the
/// accent, at the left.
pub fn heading(ui: &mut Ui, icon: &str, title: &str) {
    let p = Palette::of(ui.ctx());
    let w = ui.available_width();
    let (r, _) = ui.allocate_exact_size(Vec2::new(w, 16.0), Sense::hover());
    crate::svgicon::paint(
        ui.painter(),
        Rect::from_min_size(egui::pos2(r.left(), r.center().y - 6.5), Vec2::splat(13.0)),
        icon,
        p.accent,
    );
    ui.painter().text(
        egui::pos2(r.left() + 18.0, r.center().y),
        egui::Align2::LEFT_CENTER,
        title.to_uppercase(),
        egui::FontId::new(10.5, egui::FontFamily::Proportional),
        p.accent,
    );
}

/// What a click on a card asked for.
#[derive(Default)]
pub struct CardOut {
    /// The first line or the summary was clicked: unfold or fold it.
    pub toggle: bool,
}

/// One card, `width` wide (the area's, fixed for the frame: never the
/// content's, which changed while scrolling and made it flicker). `head`
/// draws the first line, given the width it has beside the chevron;
/// `summary` the folded lines; `body` what unfolds.
#[allow(clippy::too_many_arguments)]
pub fn card(
    ui: &mut Ui,
    key: impl std::hash::Hash + std::fmt::Debug + Clone,
    width: f32,
    now: bool,
    open: bool,
    head: impl FnOnce(&mut Ui, f32),
    summary: impl FnOnce(&mut Ui, f32),
    body: impl FnOnce(&mut Ui, f32),
) -> CardOut {
    let p = Palette::of(ui.ctx());
    let pad = if width < 260.0 { 9.0 } else { 12.0 };
    let inner = (width - 2.0 * pad).max(40.0);
    let ground = ui.painter().add(egui::Shape::Noop);
    let top = ui.cursor().min;
    let mut out = CardOut::default();
    let resp = ui
        .allocate_ui_with_layout(
            Vec2::new(width, 0.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_width(width);
                ui.add_space(pad - 2.0);
                let open_t = crate::props::fold_openness(ui.ctx(), key.clone(), open);
                // the first line, the chevron at its end, on the same line
                let head_r = ui
                    .horizontal(|ui| {
                        ui.add_space(pad);
                        ui.spacing_mut().item_spacing.x = 6.0;
                        let hw = inner - 26.0;
                        ui.allocate_ui_with_layout(
                            Vec2::new(hw, 20.0),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                ui.set_width(hw);
                                head(ui, hw)
                            },
                        );
                        let (cr, _) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::hover());
                        let icon = if open_t > 0.5 {
                            "chevron-up"
                        } else {
                            "chevron-down"
                        };
                        crate::svgicon::paint(ui.painter(), cr.shrink(3.0), icon, p.muted);
                    })
                    .response
                    .rect;
                let sum_r = ui
                    .horizontal(|ui| {
                        ui.add_space(pad);
                        ui.vertical(|ui| {
                            ui.set_width(inner);
                            ui.spacing_mut().item_spacing.y = 4.0;
                            summary(ui, inner)
                        });
                    })
                    .response
                    .rect;
                let click = ui.interact(
                    head_r.union(sum_r),
                    ui.id().with(("card-click", key.clone())),
                    Sense::click(),
                );
                if click.clicked() {
                    out.toggle = true;
                }
                if click.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                crate::props::set_fold_key(ui, key.clone());
                crate::props::fold(ui, open, |ui| {
                    ui.horizontal(|ui| {
                        ui.add_space(pad);
                        ui.vertical(|ui| {
                            ui.set_width(inner);
                            ui.add_space(4.0);
                            // a dashed rule over what unfolds
                            let (rr, _) =
                                ui.allocate_exact_size(Vec2::new(inner, 1.0), Sense::hover());
                            let mut x = rr.left();
                            while x < rr.right() {
                                ui.painter().line_segment(
                                    [
                                        Pos2::new(x, rr.top()),
                                        Pos2::new((x + 4.0).min(rr.right()), rr.top()),
                                    ],
                                    Stroke::new(1.0, p.border),
                                );
                                x += 7.0;
                            }
                            ui.add_space(6.0);
                            ui.spacing_mut().item_spacing.y = 6.0;
                            body(ui, inner)
                        });
                    });
                });
                ui.add_space(pad - 2.0);
            },
        )
        .response;
    // the card's ground, under what was drawn: the panel, lit softly in the
    // accent when it is the one now; a fine border and a soft shadow
    let r = Rect::from_min_size(top, Vec2::new(width, resp.rect.height()));
    let radius = 10.0;
    let shadow = epaint::Shadow {
        offset: [0, 2],
        blur: 8,
        spread: 0,
        color: Color32::from_black_alpha(if p.dark { 60 } else { 16 }),
    }
    .as_shape(r, radius);
    let border = if now {
        mix(p.border, p.accent, 0.35)
    } else {
        p.border
    };
    let mut shapes = vec![egui::Shape::from(shadow)];
    if now {
        let tint = mix(p.panel, p.accent, if p.dark { 0.16 } else { 0.11 });
        shapes.push(gradient_rect(r, radius, tint, p.panel, 0.7));
    } else {
        shapes.push(egui::Shape::rect_filled(r, radius, p.panel));
    }
    shapes.push(egui::Shape::rect_stroke(
        r,
        radius,
        Stroke::new(1.0, border),
        egui::StrokeKind::Inside,
    ));
    ui.painter().set(ground, egui::Shape::Vec(shapes));
    out
}
