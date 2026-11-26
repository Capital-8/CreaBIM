//! The Time area's views and Notes › Work log, on the unfolding card
//! (Mockup_Project_Time_REV04, owner 2026-11-18): Stages, Entries, Week,
//! Work log. Every width is the area's, taken once a frame, so nothing is
//! measured from content that changes while it scrolls.

use super::timeui::TimeView;
use super::*;
use crate::projecttime::{self as pt, Activity, Entry, Scheme, Tracking};
use crate::timecard::{bar, card, cut_label, mix, swatch};

/// The colours stages take: the theme's accent first, then the house hues.
pub(super) fn stage_colour(i: usize, p: &crate::theme::Palette) -> egui::Color32 {
    use crate::props::Hue;
    let hues = [
        p.accent,
        Hue::View.colour(p.dark),
        Hue::Data.colour(p.dark),
        Hue::Project.colour(p.dark),
        Hue::Element.colour(p.dark),
    ];
    hues.get(i % hues.len()).copied().unwrap_or(p.muted)
}

pub(super) fn activity_colour(a: Activity, p: &crate::theme::Palette) -> egui::Color32 {
    use crate::props::Hue;
    match a {
        Activity::Modelling => p.accent,
        Activity::Drawing => Hue::View.colour(p.dark),
        Activity::Sheets => Hue::Element.colour(p.dark),
        Activity::Other => p.muted,
    }
}

fn budget_colour(b: pt::Budget, p: &crate::theme::Palette) -> egui::Color32 {
    match b {
        pt::Budget::Fine => crate::props::Hue::Project.colour(p.dark),
        pt::Budget::Near => egui::Color32::from_rgb(214, 150, 40),
        pt::Budget::Over => egui::Color32::from_rgb(214, 72, 72),
    }
}

/// A pill: a short word on a soft ground; `tone` its colour.
fn pill(ui: &mut egui::Ui, text: &str, tone: egui::Color32) {
    let p = crate::theme::Palette::of(ui.ctx());
    let g = ui
        .painter()
        .layout_no_wrap(text.to_string(), egui::FontId::proportional(10.5), tone);
    let (r, _) = ui.allocate_exact_size(egui::vec2(g.size().x + 14.0, 18.0), egui::Sense::hover());
    ui.painter()
        .rect_filled(r, 9.0, mix(p.panel, tone, if p.dark { 0.22 } else { 0.15 }));
    ui.painter().galley(r.center() - g.size() / 2.0, g, tone);
}

/// A dot alone, for a pill with no room for its word (the counting dot).
fn dot(ui: &mut egui::Ui, tone: egui::Color32, tip: &str) {
    let p = crate::theme::Palette::of(ui.ctx());
    let (r, resp) = ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
    ui.painter()
        .circle_filled(r.center(), 8.0, mix(p.panel, tone, 0.18));
    ui.painter().circle_filled(r.center(), 3.0, tone);
    resp.on_hover_text(tip);
}

/// The activity split, as a thin rounded bar.
fn split_bar(ui: &mut egui::Ui, width: f32, act: &[f64; 4]) {
    let p = crate::theme::Palette::of(ui.ctx());
    let (r, _) = ui.allocate_exact_size(egui::vec2(width, 6.0), egui::Sense::hover());
    ui.painter().rect_filled(r, 3.0, p.raise2);
    let total: f64 = act.iter().sum();
    if total <= 0.0 {
        return;
    }
    let mut shapes = vec![];
    let mut x = r.left();
    for (k, a) in Activity::ALL.iter().enumerate() {
        let w = (act[k] / total) as f32 * r.width();
        if w > 0.5 {
            shapes.push(egui::Shape::rect_filled(
                egui::Rect::from_min_size(egui::pos2(x, r.top()), egui::vec2(w, r.height())),
                0.0,
                activity_colour(*a, &p),
            ));
        }
        x += w;
    }
    // rounded at both ends: drawn clipped to the rounded bar
    let clip = ui.painter().with_clip_rect(r);
    for s in shapes {
        clip.add(s);
    }
    // the ends rounded by painting the corners in the ground colour
    let _ = p;
}

/// The legend of the activity split, each chip one piece so a wrap never
/// splits it.
fn legend(ui: &mut egui::Ui, items: &[(String, egui::Color32)]) {
    let p = crate::theme::Palette::of(ui.ctx());
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(12.0, 4.0);
        for (label, c) in items {
            let g = ui.painter().layout_no_wrap(
                label.clone(),
                egui::FontId::proportional(11.0),
                p.muted,
            );
            let (r, _) = ui.allocate_exact_size(
                egui::vec2(14.0 + g.size().x, g.size().y.max(12.0)),
                egui::Sense::hover(),
            );
            ui.painter().rect_filled(
                egui::Rect::from_center_size(
                    egui::pos2(r.left() + 4.5, r.center().y),
                    egui::vec2(9.0, 9.0),
                ),
                3.0,
                *c,
            );
            ui.painter().galley(
                egui::pos2(r.left() + 14.0, r.center().y - g.size().y / 2.0),
                g,
                p.muted,
            );
        }
    });
}

/// The row of numbers at the top: four across, two by two, one by one.
fn kpis(ui: &mut egui::Ui, width: f32, items: &[(String, String)]) {
    let p = crate::theme::Palette::of(ui.ctx());
    let cols = if width >= 560.0 {
        4
    } else if width >= 140.0 {
        2
    } else {
        1
    };
    let gap = 8.0;
    let w = (width - (cols - 1) as f32 * gap) / cols as f32;
    for row in items.chunks(cols) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for (big, small) in row {
                egui::Frame::new()
                    .fill(p.panel)
                    .stroke(egui::Stroke::new(1.0, p.border))
                    .corner_radius(10.0)
                    .inner_margin(if w < 110.0 {
                        egui::Margin::symmetric(7, 6)
                    } else {
                        egui::Margin::symmetric(10, 8)
                    })
                    .show(ui, |ui| {
                        let pad = if w < 110.0 { 16.0 } else { 22.0 };
                        let w = w + 22.0 - pad;
                        ui.set_width(w - 22.0);
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 1.0;
                            cut_label(
                                ui,
                                egui::RichText::new(big).strong().size(if w < 110.0 {
                                    14.0
                                } else {
                                    16.0
                                }),
                                w - 22.0,
                            );
                            cut_label(
                                ui,
                                egui::RichText::new(small).color(p.muted).size(11.0),
                                w - 22.0,
                            );
                        });
                    });
            }
        });
        ui.add_space(gap - 4.0);
    }
}

/// A section heading inside a card: small capitals in the accent.
fn sec(ui: &mut egui::Ui, icon: &str, title: &str) {
    crate::timecard::heading(ui, icon, title);
}

/// A label and its field: side by side, or the label over the field when
/// the card is narrow.
pub(super) fn trow<R>(
    ui: &mut egui::Ui,
    width: f32,
    label: &str,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    if width < 300.0 {
        let p = crate::theme::Palette::of(ui.ctx());
        ui.label(egui::RichText::new(label).color(p.muted).size(11.0));
        ui.horizontal(|ui| {
            ui.set_width(width);
            add(ui)
        })
        .inner
    } else {
        crate::props::row(ui, label, add)
    }
}

/// A label and a value, read only.
fn kv(ui: &mut egui::Ui, width: f32, label: &str, value: &str) {
    trow(ui, width, label, |ui| {
        ui.label(egui::RichText::new(value).size(12.0));
    });
}

/// A day heading: its name and its hours.
fn day_head(ui: &mut egui::Ui, label: &str, hours: f64) {
    let p = crate::theme::Palette::of(ui.ctx());
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).strong().size(12.5));
        ui.label(egui::RichText::new(pt::hm(hours)).color(p.muted).size(11.5));
    });
}

fn hours_text(h: f64) -> String {
    if (h - h.round()).abs() < 1e-9 {
        format!("{h:.0}")
    } else {
        format!("{h:.1}")
    }
}

impl CreaBimApp {
    /// The width a card takes in the Time area this frame: the area's,
    /// less its margins and the scroll bar, never the content's.
    fn time_w(&self) -> f32 {
        self.time.w.max(120.0)
    }

    /// The Time area: Stages, Entries or Week.
    pub(super) fn time_area(&mut self, ui: &mut egui::Ui, body: Rect) {
        let pal = self.shell.theme.palette();
        ui.painter().rect_filled(body, 0.0, pal.panel2);
        let inner = body.shrink2(egui::vec2(12.0, 10.0));
        // fixed for the frame from the area itself (REV04: the width taken
        // from content flickered while scrolling)
        self.time.w = (inner.width() - 12.0).floor();
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(inner)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        let ui = &mut child;
        if self.time.book.tracking == Tracking::Off {
            return self.time_off(ui);
        }
        let view = self.time.view;
        egui::ScrollArea::vertical()
            .id_salt(("time-area", view as u8))
            .auto_shrink(false)
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 8.0;
                match view {
                    TimeView::Stages => self.time_stages(ui),
                    TimeView::Entries => self.time_entries(ui),
                    TimeView::Week => self.time_week(ui),
                }
            });
    }

    fn time_stages(&mut self, ui: &mut egui::Ui) {
        let pal = self.shell.theme.palette();
        let width = self.time_w();
        let b = &self.time.book;
        let now_i = b.stage_now;
        let this = b.stage_hours(now_i);
        let total = b.total_hours();
        let items = [
            (pt::hm(this), "this stage".to_string()),
            (
                b.stage_use(now_i)
                    .map_or("—".to_string(), |u| format!("{:.0} %", u * 100.0)),
                "of its budget".to_string(),
            ),
            (pt::hm(total), "project so far".to_string()),
            if b.rate > 0.0 {
                (
                    pt::money(&b.currency, total * b.rate),
                    format!("value at {}/h", pt::money(&b.currency, b.rate)),
                )
            } else {
                ("—".to_string(), "set a rate in Project".to_string())
            },
        ];
        kpis(ui, width, &items);

        let n = self.time.book.stages.len();
        let hours: Vec<f64> = (0..n).map(|i| self.time.book.stage_hours(i)).collect();
        let cur = self.time.book.currency.clone();
        let own = self.time.book.scheme == Scheme::Own;
        let rate = self.time.book.rate;
        let mut toggle: Option<usize> = None;
        let mut set_now: Option<usize> = None;
        let mut changed = false;
        let mut remove: Option<usize> = None;
        for i in 0..n {
            let open = self.time.stage_edit == Some(i);
            let now = i == now_i;
            let s = self.time.book.stages[i].clone();
            let use_ = (s.budget_h > 0.0).then(|| hours[i] / s.budget_h);
            let colour = stage_colour(i, &pal);
            let (num, name) = match s.name.split_once(' ') {
                Some((a, b)) if a.chars().next().is_some_and(|c| c.is_ascii_digit()) => {
                    (a.to_string(), b.to_string())
                }
                _ => (String::new(), s.name.clone()),
            };
            let out = card(
                ui,
                ("time-stage", i),
                width,
                now,
                open,
                |ui, w| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    swatch(ui, colour);
                    let mut left = w - 15.0;
                    if !num.is_empty() {
                        let g = ui.label(egui::RichText::new(&num).strong().size(12.0));
                        left -= g.rect.width() + 6.0;
                    }
                    // the pill at the end, the name cut to what is left
                    let pill_w = if now || s.done || use_.is_some_and(|u| u > 1.0) {
                        58.0
                    } else {
                        0.0
                    };
                    let near = use_.map(|u| pt::budget_state(Some(u)));
                    cut_label(
                        ui,
                        egui::RichText::new(&name).strong(),
                        (left - pill_w).max(30.0),
                    )
                    .on_hover_text(&s.name);
                    if now {
                        pill(ui, "now", pal.accent);
                    } else if s.done {
                        pill(ui, "done", crate::props::Hue::Project.colour(pal.dark));
                    } else if near == Some(pt::Budget::Over) {
                        pill(ui, "over", budget_colour(pt::Budget::Over, &pal));
                    }
                },
                |ui, w| {
                    let spent = if hours[i] > 0.0 {
                        pt::hm(hours[i])
                    } else {
                        "—".into()
                    };
                    let mut said = if s.budget_h > 0.0 {
                        format!("{spent} of {} h", hours_text(s.budget_h))
                    } else if hours[i] > 0.0 {
                        format!("{spent} · no budget")
                    } else {
                        "Not started · no budget".to_string()
                    };
                    if s.fee > 0.0 && w > 230.0 {
                        said.push_str(&format!(" · {} fee", pt::money(&cur, s.fee)));
                    }
                    cut_label(ui, egui::RichText::new(said).size(11.5).color(pal.muted), w);
                    bar(ui, w, use_, budget_colour(pt::budget_state(use_), &pal));
                    if let Some(u) = use_ {
                        ui.label(
                            egui::RichText::new(format!("{:.0} % used", u * 100.0))
                                .size(11.0)
                                .color(pal.muted),
                        );
                    }
                },
                |ui, w| {
                    sec(ui, "coins", "Fee and hours");
                    let st = &mut self.time.book.stages[i];
                    if own {
                        trow(ui, w, "Name", |ui| {
                            changed |= crate::props::text_field(
                                ui,
                                egui::TextEdit::singleline(&mut st.name),
                                f32::INFINITY,
                            )
                            .changed();
                        });
                    }
                    trow(ui, w, "Fee", |ui| {
                        changed |= crate::props::fill(
                            ui,
                            egui::DragValue::new(&mut st.fee)
                                .range(0.0..=10_000_000.0)
                                .speed(50.0)
                                .prefix(cur.clone())
                                .max_decimals(0),
                        )
                        .on_hover_text("The stage's fee; 0 when it is paid by the hour")
                        .changed();
                    });
                    trow(ui, w, "Hours it allows", |ui| {
                        changed |= crate::props::fill(
                            ui,
                            egui::DragValue::new(&mut st.budget_h)
                                .range(0.0..=100_000.0)
                                .speed(1.0)
                                .suffix(" h")
                                .max_decimals(1),
                        )
                        .on_hover_text("The bar turns amber at 80 %, red past 100 %")
                        .changed();
                    });
                    if rate > 0.0 {
                        kv(
                            ui,
                            w,
                            "Value so far",
                            &format!(
                                "{} at {}/h",
                                pt::money(&cur, hours[i] * rate),
                                pt::money(&cur, rate)
                            ),
                        );
                    }
                    ui.horizontal_wrapped(|ui| {
                        if i != now_i
                            && !st.done
                            && ui.add(egui::Button::new("Time goes here now")).clicked()
                        {
                            set_now = Some(i);
                        }
                        let label = if st.done { "Not done" } else { "Mark done" };
                        if ui
                            .button(label)
                            .on_hover_text("The stage is finished: never offered as the stage now")
                            .clicked()
                        {
                            st.done = !st.done;
                            changed = true;
                        }
                        if own
                            && hours[i] <= 0.0
                            && n > 1
                            && crate::props::icon_button(ui, "trash-2", "Remove this stage")
                                .clicked()
                        {
                            remove = Some(i);
                        }
                    });
                },
            );
            if out.toggle {
                toggle = Some(i);
            }
        }
        if own
            && ui
                .button("+ Stage")
                .on_hover_text("Add a stage to your own list")
                .clicked()
        {
            self.time.book.stages.push(pt::Stage::new("New stage"));
            self.time.stage_edit = Some(self.time.book.stages.len() - 1);
            changed = true;
        }
        if let Some(i) = toggle {
            self.time.stage_edit = if self.time.stage_edit == Some(i) {
                None
            } else {
                Some(i)
            };
        }
        if let Some(i) = remove {
            self.time.book.stages.remove(i);
            for e in &mut self.time.book.entries {
                if e.stage > i {
                    e.stage -= 1;
                }
            }
            if self.time.book.stage_now >= self.time.book.stages.len() {
                self.time.book.stage_now = self.time.book.stages.len().saturating_sub(1);
            }
            self.time.stage_edit = None;
            changed = true;
        }
        if let Some(i) = set_now {
            let ctx = ui.ctx().clone();
            self.time_action(&ctx, "time.stage", &i.to_string());
        }
        if changed {
            self.time_changed();
        }
        let b = &self.time.book;
        if let (Some(s), Some(u)) = (b.stage(), b.stage_use(b.stage_now)) {
            crate::props::plain_note(
                ui,
                &format!(
                    "{} is at {:.0} % of its {} hours: amber from 80 %, red past 100 %.",
                    s.short(),
                    u * 100.0,
                    hours_text(s.budget_h)
                ),
            );
        } else {
            crate::props::plain_note(ui, "Click a stage to give it its fee and the hours it allows: its bar then fills as time is spent.");
        }
    }

    fn time_entries(&mut self, ui: &mut egui::Ui) {
        let pal = self.shell.theme.palette();
        let width = self.time_w();
        let items: Vec<(String, egui::Color32)> = Activity::ALL
            .iter()
            .map(|a| (a.label().to_string(), activity_colour(*a, &pal)))
            .collect();
        legend(ui, &items);
        let mut order: Vec<usize> = (0..self.time.book.entries.len()).collect();
        order.sort_by_key(|&i| std::cmp::Reverse(self.time.book.entries[i].start));
        if order.is_empty() {
            crate::props::empty(ui, "Nothing counted yet. Work in the project and it appears here; or add a manual entry for work outside CreaBIM.");
            return;
        }
        let today = pt::local_day(pt::now_s() as i64, self.time.tz);
        let mut day = i64::MIN;
        let mut toggle: Option<usize> = None;
        let mut to_notes: Option<usize> = None;
        let mut delete: Option<usize> = None;
        let mut changed = false;
        let stages: Vec<String> = self
            .time
            .book
            .stages
            .iter()
            .map(|s| s.name.clone())
            .collect();
        for i in order {
            let e_day = self.time.book.entries[i].day();
            if e_day != day {
                day = e_day;
                let label = match today - e_day {
                    0 => "Today".to_string(),
                    1 => "Yesterday".to_string(),
                    _ => pt::day_label(e_day),
                };
                let h: f64 = self
                    .time
                    .book
                    .entries
                    .iter()
                    .filter(|e| e.day() == e_day)
                    .map(Entry::hours)
                    .sum();
                day_head(ui, &label, h);
            }
            let open = self.time.picked == Some(i);
            let counting = self.time.clock.open == Some(i);
            let e = self.time.book.entries[i].clone();
            let colour = stage_colour(e.stage, &pal);
            let range = format!(
                "{}–{}",
                pt::clock_hm(e.start, e.tz_min),
                if counting {
                    "now".to_string()
                } else {
                    pt::clock_hm(e.end, e.tz_min)
                }
            );
            let note = if e.note.is_empty() {
                "No note".to_string()
            } else {
                e.note.clone()
            };
            let out = card(
                ui,
                ("time-entry", i),
                width,
                counting,
                open,
                |ui, _w| {
                    // the first line is the time; the chevron sits on it
                    ui.label(
                        egui::RichText::new(&range)
                            .monospace()
                            .size(11.0)
                            .color(pal.muted),
                    );
                },
                |ui, w| {
                    ui.horizontal(|ui| {
                        let dur = pt::hm(e.hours());
                        let dw = ui
                            .painter()
                            .layout_no_wrap(dur.clone(), egui::FontId::proportional(13.0), pal.text)
                            .size()
                            .x;
                        let t = egui::RichText::new(&note).strong();
                        let t = if e.note.is_empty() {
                            t.color(pal.muted)
                        } else {
                            t
                        };
                        cut_label(ui, t, w - dw - 10.0).on_hover_text(&note);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(egui::RichText::new(dur).strong());
                        });
                    });
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        swatch(ui, colour);
                        let sname = stages.get(e.stage).map_or("—", |s| s.as_str());
                        cut_label(
                            ui,
                            egui::RichText::new(sname).size(11.5).color(pal.muted),
                            (w * 0.55).min(220.0),
                        );
                        if w > 240.0 {
                            ui.label(egui::RichText::new(&e.person).size(11.5).color(pal.muted));
                        }
                        if counting {
                            if w > 220.0 {
                                pill(ui, "counting", pal.accent);
                            } else {
                                dot(ui, pal.accent, "Counting now");
                            }
                        }
                        if e.manual {
                            pill(ui, "typed in", pal.muted);
                        }
                    });
                    split_bar(ui, w, &e.act_s);
                },
                |ui, w| {
                    sec(ui, "sticky-note", "Note");
                    let en = &mut self.time.book.entries[i];
                    changed |= crate::props::text_field(
                        ui,
                        egui::TextEdit::singleline(&mut en.note).hint_text("What was done"),
                        w,
                    )
                    .changed();
                    sec(ui, "clock", "Details");
                    trow(ui, w, "Stage", |ui| {
                        egui::ComboBox::from_id_salt(("time-entry-stage", i))
                            .icon(crate::props::combo_icon)
                            .width(ui.available_width())
                            .selected_text(stages.get(en.stage).cloned().unwrap_or_default())
                            .show_ui(ui, |ui| {
                                for (k, s) in stages.iter().enumerate() {
                                    changed |= ui.selectable_value(&mut en.stage, k, s).changed();
                                }
                            });
                    });
                    if en.manual {
                        changed |= super::timeui::manual_times(ui, en, i);
                    } else {
                        kv(ui, w, "Working", &pt::hm(en.working_s / 3600.0));
                        kv(ui, w, "Open, away", &pt::hm(en.open_s / 3600.0));
                        kv(ui, w, "Idle", &pt::hm(en.idle_s / 3600.0));
                    }
                    let total: f64 = en.act_s.iter().sum();
                    if total > 0.0 {
                        sec(ui, "list", "What it was spent on");
                        split_bar(ui, w, &en.act_s);
                        let parts: Vec<(String, egui::Color32)> = Activity::ALL
                            .iter()
                            .enumerate()
                            .filter(|(k, _)| en.act_s[*k] > 0.0)
                            .map(|(k, a)| {
                                (
                                    format!("{} {:.0} %", a.label(), en.act_s[k] / total * 100.0),
                                    activity_colour(*a, &pal),
                                )
                            })
                            .collect();
                        legend(ui, &parts);
                    }
                    ui.horizontal(|ui| {
                        if ui
                            .button("Open in Notes")
                            .on_hover_text("This note in Notes › Work log")
                            .clicked()
                        {
                            to_notes = Some(i);
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if !counting
                                && ui
                                    .button("Delete")
                                    .on_hover_text("Remove this entry from the project's time")
                                    .clicked()
                            {
                                delete = Some(i);
                            }
                        });
                    });
                },
            );
            if out.toggle {
                toggle = Some(i);
            }
        }
        if let Some(i) = toggle {
            self.time.picked = if self.time.picked == Some(i) {
                None
            } else {
                Some(i)
            };
        }
        if let Some(i) = to_notes {
            let ctx = ui.ctx().clone();
            self.time_open_in_notes(i, &ctx);
        }
        if let Some(i) = delete {
            self.time.book.entries.remove(i);
            if let Some(o) = self.time.clock.open {
                if o > i {
                    self.time.clock.open = Some(o - 1);
                }
            }
            self.time.picked = None;
            changed = true;
            self.status = "Entry removed.".into();
        }
        if changed {
            self.time_changed();
        }
    }

    fn time_week(&mut self, ui: &mut egui::Ui) {
        let pal = self.shell.theme.palette();
        let width = self.time_w();
        let now_s = pt::now_s() as i64;
        let now = now_s - self.time.week_back * 7 * 86_400;
        let week = self.time.book.week(now, self.time.tz);
        let monday = pt::week_start(pt::local_day(now, self.time.tz));
        let today = pt::local_day(now_s, self.time.tz);
        ui.horizontal(|ui| {
            if crate::props::icon_button(ui, "chevron-left", "The week before").clicked() {
                self.time.week_back += 1;
            }
            ui.label(
                egui::RichText::new(format!(
                    "{} – {}",
                    pt::day_label(monday),
                    pt::day_label(monday + 6)
                ))
                .strong(),
            );
            if self.time.week_back > 0
                && crate::props::icon_button(ui, "chevron-right", "The week after").clicked()
            {
                self.time.week_back -= 1;
            }
            if self.time.week_back == 0 && width > 260.0 {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    pill(ui, "this week", pal.accent)
                });
            }
        });
        let total: f64 = week.iter().flatten().sum();
        let days_worked = week
            .iter()
            .filter(|d| d.iter().sum::<f64>() > 0.0)
            .count()
            .max(1);
        let rate = self.time.book.rate;
        let cur = self.time.book.currency.clone();
        kpis(
            ui,
            width,
            &[
                (pt::hm(total), "this week".into()),
                (pt::hm(total / days_worked as f64), "a working day".into()),
                (
                    format!(
                        "{}",
                        week.iter().filter(|d| d.iter().sum::<f64>() > 0.0).count()
                    ),
                    "days worked".into(),
                ),
                if rate > 0.0 {
                    (pt::money(&cur, total * rate), "value".into())
                } else {
                    ("—".into(), "set a rate in Project".into())
                },
            ],
        );
        let n = self.time.book.stages.len();
        let used: Vec<(String, egui::Color32)> = (0..n)
            .filter(|&s| week.iter().any(|d| d.get(s).copied().unwrap_or(0.0) > 0.0))
            .map(|s| (self.time.book.stages[s].name.clone(), stage_colour(s, &pal)))
            .collect();
        legend(ui, &used);
        let max = week
            .iter()
            .map(|d| d.iter().sum::<f64>())
            .fold(0.0, f64::max)
            .max(4.0)
            .ceil();
        let label_w = 30.0;
        if width >= 420.0 {
            // a column a day, its hours over it, today in the accent
            let h = 170.0;
            let (r, _) = ui.allocate_exact_size(egui::vec2(width, h + 24.0), egui::Sense::hover());
            let plot = egui::Rect::from_min_max(
                r.min + egui::vec2(0.0, 16.0),
                egui::pos2(r.right(), r.top() + h),
            );
            let paint = ui.painter();
            for k in 0..=4 {
                let yy = plot.bottom() - k as f32 / 4.0 * plot.height();
                paint.line_segment(
                    [egui::pos2(plot.left(), yy), egui::pos2(plot.right(), yy)],
                    egui::Stroke::new(1.0, mix(pal.panel2, pal.border, 0.7)),
                );
            }
            let slot = plot.width() / 7.0;
            for (d, stages) in week.iter().enumerate() {
                let sum: f64 = stages.iter().sum();
                let is_today = monday + d as i64 == today;
                let bw = (slot * 0.56).min(52.0);
                let x = plot.left() + slot * d as f32 + (slot - bw) / 2.0;
                let full = (sum / max) as f32 * plot.height();
                let col = egui::Rect::from_min_max(
                    egui::pos2(x, plot.bottom() - full),
                    egui::pos2(x + bw, plot.bottom()),
                );
                if sum > 0.0 {
                    // stacked by stage, clipped to one rounded column
                    let clip = paint.with_clip_rect(col);
                    let mut base = plot.bottom();
                    for (s, v) in stages.iter().enumerate() {
                        if *v <= 0.0 {
                            continue;
                        }
                        let hh = (*v / max) as f32 * plot.height();
                        clip.rect_filled(
                            egui::Rect::from_min_max(
                                egui::pos2(x, base - hh),
                                egui::pos2(x + bw, base + 6.0),
                            ),
                            egui::CornerRadius {
                                nw: 6,
                                ne: 6,
                                sw: 2,
                                se: 2,
                            },
                            stage_colour(s, &pal),
                        );
                        base -= hh;
                    }
                    paint.text(
                        egui::pos2(x + bw / 2.0, col.top() - 8.0),
                        egui::Align2::CENTER_CENTER,
                        pt::hm(sum),
                        egui::FontId::proportional(10.5),
                        if is_today { pal.accent } else { pal.muted },
                    );
                }
                paint.text(
                    egui::pos2(x + bw / 2.0, plot.bottom() + 12.0),
                    egui::Align2::CENTER_CENTER,
                    pt::WEEKDAYS[d],
                    egui::FontId::proportional(if is_today { 11.5 } else { 11.0 }),
                    if is_today { pal.accent } else { pal.muted },
                );
            }
        } else {
            // narrow: a bar a day, across
            for (d, stages) in week.iter().enumerate() {
                let sum: f64 = stages.iter().sum();
                let is_today = monday + d as i64 == today;
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    let c = if is_today { pal.accent } else { pal.muted };
                    ui.add_sized(
                        [label_w, 14.0],
                        egui::Label::new(egui::RichText::new(pt::WEEKDAYS[d]).size(11.5).color(c)),
                    );
                    let bw = (width - label_w - 54.0).max(40.0);
                    let (r, _) = ui.allocate_exact_size(egui::vec2(bw, 10.0), egui::Sense::hover());
                    ui.painter().rect_filled(r, 5.0, pal.raise2);
                    let clip = ui.painter().with_clip_rect(egui::Rect::from_min_size(
                        r.min,
                        egui::vec2((sum / max) as f32 * bw, r.height()),
                    ));
                    let mut x = r.left();
                    for (s, v) in stages.iter().enumerate() {
                        let w = (*v / max) as f32 * bw;
                        if w > 0.0 {
                            clip.rect_filled(
                                egui::Rect::from_min_size(
                                    egui::pos2(x, r.top()),
                                    egui::vec2(w, r.height()),
                                ),
                                5.0,
                                stage_colour(s, &pal),
                            );
                        }
                        x += w;
                    }
                    ui.label(
                        egui::RichText::new(if sum > 0.0 { pt::hm(sum) } else { "—".into() })
                            .size(11.5),
                    );
                });
            }
        }
        // by day: a card a day worked, its entries under it
        ui.add_space(4.0);
        ui.label(egui::RichText::new("By day").strong().size(12.5));
        let mut toggle = None;
        for d in (0..7).rev() {
            let day = monday + d as i64;
            let sum: f64 = week[d].iter().sum();
            if sum <= 0.0 {
                continue;
            }
            let mut es: Vec<&Entry> = self
                .time
                .book
                .entries
                .iter()
                .filter(|e| e.day() == day)
                .collect();
            es.sort_by_key(|e| e.start);
            let main_stage = (0..n)
                .max_by(|a, b| week[d][*a].total_cmp(&week[d][*b]))
                .unwrap_or(0);
            let notes: Vec<&str> = es
                .iter()
                .filter(|e| !e.note.is_empty())
                .map(|e| e.note.as_str())
                .collect();
            let label = if day == today {
                format!("{} · today", pt::day_label(day))
            } else {
                pt::day_label(day)
            };
            let open = self.time.week_open == Some(day);
            let out = card(
                ui,
                ("time-day", day),
                width,
                day == today,
                open,
                |ui, w| {
                    swatch(ui, stage_colour(main_stage, &pal));
                    let dur = pt::hm(sum);
                    cut_label(ui, egui::RichText::new(&label).strong(), w - 70.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new(dur).strong());
                    });
                },
                |ui, w| {
                    let said = if notes.is_empty() {
                        format!("{} stretches", es.len())
                    } else {
                        notes.join(", ")
                    };
                    cut_label(ui, egui::RichText::new(said).size(11.5).color(pal.muted), w);
                },
                |ui, w| {
                    for e in &es {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(pt::clock_hm(e.start, e.tz_min))
                                    .monospace()
                                    .size(11.0)
                                    .color(pal.muted),
                            );
                            let dur = pt::hm(e.hours());
                            let note = if e.note.is_empty() {
                                "No note"
                            } else {
                                e.note.as_str()
                            };
                            cut_label(ui, egui::RichText::new(note).size(12.0), w - 110.0);
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(egui::RichText::new(dur).size(12.0));
                                },
                            );
                        });
                    }
                },
            );
            if out.toggle {
                toggle = Some(day);
            }
        }
        if let Some(d) = toggle {
            self.time.week_open = if self.time.week_open == Some(d) {
                None
            } else {
                Some(d)
            };
        }
    }

    /// Notes › Work log (REV04): a timeline, a dot a stretch on a rail in
    /// its stage's colour, its note typed on the card.
    pub(super) fn time_worklog(&mut self, ui: &mut egui::Ui) {
        let pal = self.shell.theme.palette();
        crate::props::plain_note(
            ui,
            "Every stretch of Project time and its note. Kept beside the project, private: never in the file or in IFC.",
        );
        if self.time.book.tracking == Tracking::Off && self.time.book.entries.is_empty() {
            crate::props::empty(ui, "Project time is off for this project: switch it on in the Project tab's Time and fees, and what you work on is logged here.");
            return;
        }
        let mut order: Vec<usize> = (0..self.time.book.entries.len()).collect();
        order.sort_by_key(|&i| std::cmp::Reverse(self.time.book.entries[i].start));
        if order.is_empty() {
            crate::props::empty(ui, "Nothing logged yet: work in the project and each stretch of time appears here, ready for a note.");
            return;
        }
        let stages: Vec<String> = self
            .time
            .book
            .stages
            .iter()
            .map(|s| s.name.clone())
            .collect();
        let today = pt::local_day(pt::now_s() as i64, self.time.tz);
        // the area's own width, fixed for the frame
        let full = (ui.available_width() - 14.0).floor().max(120.0);
        let rail = if full < 300.0 { 16.0 } else { 20.0 };
        let width = full - rail;
        let mut day = i64::MIN;
        let mut changed = false;
        let mut goto: Option<usize> = None;
        egui::ScrollArea::vertical()
            .id_salt(ui.id().with("time-worklog"))
            .auto_shrink([false, true])
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 8.0;
                for i in order {
                    let e_day = self.time.book.entries[i].day();
                    if e_day != day {
                        day = e_day;
                        let label = match today - e_day {
                            0 => "Today".to_string(),
                            1 => "Yesterday".to_string(),
                            _ => pt::day_label(e_day),
                        };
                        let h: f64 = self.time.book.entries.iter().filter(|e| e.day() == e_day).map(Entry::hours).sum();
                        day_head(ui, &label, h);
                    }
                    let counting = self.time.clock.open == Some(i);
                    let hot = self.time.log_hot == Some(i);
                    let colour = stage_colour(self.time.book.entries[i].stage, &pal);
                    let top = ui.cursor().min;
                    let resp = ui
                        .horizontal(|ui| {
                            ui.add_space(rail);
                            egui::Frame::new()
                                .fill(pal.panel)
                                .stroke(egui::Stroke::new(1.0, if hot { pal.accent } else { pal.border }))
                                .corner_radius(10.0)
                                .shadow(egui::epaint::Shadow { offset: [0, 2], blur: 8, spread: 0, color: egui::Color32::from_black_alpha(if pal.dark { 60 } else { 14 }) })
                                .inner_margin(egui::Margin::symmetric(10, 8))
                                .show(ui, |ui| ui.vertical(|ui| {
                                    let w = width - 22.0;
                                    ui.set_width(w);
                                    ui.spacing_mut().item_spacing.y = 5.0;
                                    let e = &mut self.time.book.entries[i];
                                    ui.horizontal(|ui| {
                                        let range = format!(
                                            "{}–{}",
                                            pt::clock_hm(e.start, e.tz_min),
                                            if counting { "now".to_string() } else { pt::clock_hm(e.end, e.tz_min) }
                                        );
                                        let dur = pt::hm(e.hours());
                                        let dw = ui.painter().layout_no_wrap(dur.clone(), egui::FontId::proportional(13.0), pal.text).size().x;
                                        let pw = if counting { if w > 220.0 { 74.0 } else { 24.0 } } else { 0.0 };
                                        cut_label(ui, egui::RichText::new(range).monospace().size(11.0).color(pal.muted), (w - dw - pw - 12.0).max(30.0));
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            ui.label(egui::RichText::new(dur).strong());
                                            if counting {
                                                if w > 220.0 {
                                                    pill(ui, "counting", pal.accent);
                                                } else {
                                                    dot(ui, pal.accent, "Counting now");
                                                }
                                            }
                                        });
                                    });
                                    changed |= crate::props::text_field(
                                        ui,
                                        egui::TextEdit::singleline(&mut e.note)
                                            .hint_text("What was done…")
                                            .id(ui.id().with(("time-worklog-note", i))),
                                        w,
                                    )
                                    .changed();
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = 6.0;
                                        swatch(ui, colour);
                                        let sname = stages.get(e.stage).map_or("—", |s| s.as_str());
                                        cut_label(ui, egui::RichText::new(sname).size(11.5).color(pal.muted), (w - 110.0).max(40.0));
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            if ui.link(egui::RichText::new("Open in Time").size(11.5)).on_hover_text("This stretch in Time › Entries, opened").clicked() {
                                                goto = Some(i);
                                            }
                                        });
                                    });
                                }));
                        })
                        .response;
                    // the rail and the dot, in the stage's colour
                    let x = top.x + rail / 2.0 - 1.0;
                    let r = resp.rect;
                    ui.painter().line_segment(
                        [egui::pos2(x, r.top() - 8.0), egui::pos2(x, r.bottom())],
                        egui::Stroke::new(2.0, pal.border),
                    );
                    let c = egui::pos2(x, r.top() + 15.0);
                    ui.painter().circle_filled(c, 5.5, if counting { colour } else { pal.panel });
                    ui.painter().circle_stroke(c, 5.0, egui::Stroke::new(2.0, colour));
                    if hot && ui.ctx().data(|d| d.get_temp::<usize>(egui::Id::new("time-worklog-scroll"))) == Some(i) {
                        resp.scroll_to_me(Some(egui::Align::Center));
                        ui.ctx().data_mut(|d| d.remove::<usize>(egui::Id::new("time-worklog-scroll")));
                    }
                }
            });
        if changed {
            self.time_changed();
        }
        if let Some(i) = goto {
            self.time.view = TimeView::Entries;
            self.time.picked = Some(i);
            self.time_show_area();
        }
    }
}
